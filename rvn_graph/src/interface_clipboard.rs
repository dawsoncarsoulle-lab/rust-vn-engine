//! Typed, in-memory authoring clipboard. Expressions are graph producers,
//! never the JSON values obtained by evaluating an interface preview.
use crate::*;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone)]
pub struct InterfaceClipboard {
    source_id: GraphId,
    source_kind: GraphKind,
    root: NodeId,
    nodes: BTreeMap<NodeId, GraphNode>,
    pins: BTreeMap<PinId, GraphPin>,
    edges: Vec<GraphEdge>,
    identities: BTreeMap<NodeId, String>,
    bindings: BTreeMap<NodeId, Binding>,
    functions: BTreeSet<(String, usize)>,
}
#[derive(Debug, Clone)]
struct Binding {
    name: String,
    scope: VariableScope,
    global_type: Option<ValueType>,
    assignment_output: Option<PinId>,
}
impl InterfaceClipboard {
    pub fn component_count(&self) -> usize {
        self.identities.len()
    }
    /// Function calls keep their project identities and argument expressions.
    /// A project-aware caller evaluates the trial before committing its undo.
    pub fn function_dependencies(&self) -> impl Iterator<Item = (&str, usize)> {
        self.functions
            .iter()
            .map(|(name, arity)| (name.as_str(), *arity))
    }
}
fn find(tree: &InterfaceAuthoringNode, node: NodeId) -> Option<&InterfaceAuthoringNode> {
    if tree.node == node {
        Some(tree)
    } else {
        tree.children.iter().find_map(|child| find(child, node))
    }
}
fn authored(
    tree: &InterfaceAuthoringNode,
    identities: &mut BTreeMap<NodeId, String>,
) -> Result<(), String> {
    if tree.computed || tree.computed_children.is_some() || tree.id.is_none() {
        return Err(
            "Generated components need explicit identity parameters: copy in the Blueprint graph"
                .into(),
        );
    }
    if identities
        .insert(tree.node, tree.id.clone().unwrap())
        .is_some()
    {
        return Err("A shared component needs a unique authored identity before copying".into());
    }
    for child in &tree.children {
        authored(child, identities)?;
    }
    Ok(())
}
impl GraphDocument {
    /// Capture an authored subtree, including its live property/style/value
    /// dependencies. Normalization happens only in this private clipboard.
    pub fn copy_interface_subtree(
        &self,
        return_node: NodeId,
        node: NodeId,
    ) -> Result<InterfaceClipboard, String> {
        // Validate the screen's return, but read this expression independently:
        // a dynamic parent collection need not hide an authored leaf from copy.
        self.interface_tree(return_node)?;
        let subtree = self.interface_expression_tree(node)?;
        let mut identities = BTreeMap::new();
        authored(&subtree, &mut identities)?;
        let mut source = self.clone();
        for id in identities.keys() {
            source.interface_componentize(*id)?;
        }
        let mut clip = InterfaceClipboard {
            source_id: self.graph_id,
            source_kind: self.kind.clone(),
            root: node,
            nodes: BTreeMap::new(),
            pins: BTreeMap::new(),
            edges: Vec::new(),
            identities,
            bindings: BTreeMap::new(),
            functions: BTreeSet::new(),
        };
        fn visit(
            graph: &GraphDocument,
            node: NodeId,
            clip: &mut InterfaceClipboard,
            active: &mut BTreeSet<NodeId>,
            depth: usize,
        ) -> Result<(), String> {
            if depth > 128 || clip.nodes.len() >= 4096 {
                return Err("Interface expression is too complex to copy".into());
            }
            if active.contains(&node) {
                return Err("Interface expression has a cycle".into());
            }
            if clip.nodes.contains_key(&node) {
                return Ok(());
            }
            active.insert(node);
            let model = graph
                .nodes
                .get(&node)
                .ok_or("Missing interface expression node")?
                .clone();
            let assignment = matches!(
                model.kind,
                NodeKind::SetVariable | NodeKind::LocalVariable | NodeKind::ForEach
            );
            let mut boundary = None;
            if matches!(
                model.kind,
                NodeKind::VariableGet | NodeKind::VariableReference
            ) || assignment
            {
                let name = graph
                    .variable_node_name(node)
                    .ok_or("Computed variable identity cannot be copied safely")?;
                let scope = graph
                    .variable_scope(&name)
                    .ok_or_else(|| format!("Unknown variable dependency: {name}"))?;
                let output = if assignment {
                    Some(
                        graph
                            .pin_by_key(node, "value_out")
                            .ok_or("Assignment has no reusable value output")?
                            .id,
                    )
                } else {
                    None
                };
                clip.bindings.insert(
                    node,
                    Binding {
                        name: name.clone(),
                        scope,
                        global_type: graph
                            .variables
                            .get(&name)
                            .map(|definition| definition.value_type.clone()),
                        assignment_output: output,
                    },
                );
                boundary = output;
            }
            let pins = model
                .pins
                .iter()
                .map(|id| {
                    graph
                        .pins
                        .get(id)
                        .cloned()
                        .ok_or("Missing expression pin".to_owned())
                })
                .collect::<Result<Vec<_>, _>>()?;
            if !assignment && pins.iter().any(|pin| pin.value_type.is_execution()) {
                return Err(
                    "Execution producers cannot be copied as interface data expressions".into(),
                );
            }
            if model.kind == NodeKind::FunctionCall {
                let name = match model.properties.get("function") {
                    Some(PropertyValue::String(name)) => name.clone(),
                    _ => return Err("Missing function identity".into()),
                };
                let arity = match model.properties.get("input_count") {
                    Some(PropertyValue::Int(count)) if (0..=128).contains(count) => *count as usize,
                    _ => return Err("Invalid function argument count".into()),
                };
                clip.functions.insert((name, arity));
            }
            for pin in &pins {
                clip.pins.insert(pin.id, pin.clone());
            }
            clip.nodes.insert(node, model);
            if boundary.is_none() {
                for pin in pins
                    .iter()
                    .filter(|pin| pin.direction == PinDirection::Input)
                {
                    let producers = graph
                        .edges
                        .values()
                        .filter(|edge| edge.input == pin.id)
                        .collect::<Vec<_>>();
                    if producers.len() > 1 {
                        return Err("Interface data input has multiple producers".into());
                    }
                    for edge in producers {
                        let output = graph
                            .pins
                            .get(&edge.output)
                            .ok_or("Missing expression producer")?;
                        if output.value_type.is_execution() {
                            return Err("Interface depends on an execution wire".into());
                        }
                        visit(graph, output.node, clip, active, depth + 1)?;
                        clip.edges.push(edge.clone());
                    }
                }
            }
            active.remove(&node);
            Ok(())
        }
        visit(&source, node, &mut clip, &mut BTreeSet::new(), 0)?;
        Ok(clip)
    }

    /// Paste as one atomic source graph edit. Shared producers from this same
    /// unchanged graph are reused; cross-screen expressions are remapped with
    /// their scoped bindings intact. No function definition is duplicated.
    pub fn paste_interface_subtree(
        &mut self,
        return_node: NodeId,
        parent: NodeId,
        clip: &InterfaceClipboard,
    ) -> Result<NodeId, String> {
        let tree = self.interface_tree(return_node)?;
        let target = find(&tree, parent).ok_or("Unknown target container")?;
        if target.computed
            || target.computed_children.is_some()
            || !matches!(target.kind.as_deref(), Some("panel" | "row" | "column"))
        {
            return Err("Paste into an authored panel, column or row".into());
        }
        for binding in clip.bindings.values() {
            if self.variable_scope(&binding.name) != Some(binding.scope) {
                return Err(format!(
                    "Missing or differently scoped variable: {}",
                    binding.name
                ));
            }
            if binding.scope == VariableScope::Global
                && self
                    .variables
                    .get(&binding.name)
                    .map(|definition| &definition.value_type)
                    != binding.global_type.as_ref()
            {
                return Err(format!(
                    "Incompatible global variable type: {}",
                    binding.name
                ));
            }
        }
        let mut next = self.clone();
        let same = self.graph_id == clip.source_id && self.kind == clip.source_kind;
        fn reusable(
            graph: &GraphDocument,
            clip: &InterfaceClipboard,
            node: NodeId,
            same: bool,
            memo: &mut BTreeMap<NodeId, bool>,
        ) -> bool {
            if let Some(value) = memo.get(&node) {
                return *value;
            }
            memo.insert(node, false);
            if !same
                || clip.identities.contains_key(&node)
                || clip
                    .bindings
                    .get(&node)
                    .is_some_and(|binding| binding.assignment_output.is_some())
                || graph.nodes.get(&node) != clip.nodes.get(&node)
            {
                return false;
            }
            let model = &clip.nodes[&node];
            if model
                .pins
                .iter()
                .any(|id| graph.pins.get(id) != clip.pins.get(id))
            {
                return false;
            }
            let edges = clip
                .edges
                .iter()
                .filter(|edge| clip.pins[&edge.input].node == node)
                .collect::<Vec<_>>();
            if graph
                .edges
                .values()
                .filter(|edge| model.pins.contains(&edge.input))
                .count()
                != edges.len()
            {
                return false;
            }
            let result = edges.iter().all(|edge| {
                graph.edges.get(&edge.id) == Some(*edge)
                    && reusable(graph, clip, clip.pins[&edge.output].node, same, memo)
            });
            memo.insert(node, result);
            result
        }
        let mut reused = BTreeMap::new();
        for node in clip.nodes.keys() {
            reusable(self, clip, *node, same, &mut reused);
        }
        let mut node_map = BTreeMap::new();
        let mut pin_map = BTreeMap::new();
        for (id, model) in &clip.nodes {
            if reused[id] {
                node_map.insert(*id, *id);
                for pin in &model.pins {
                    pin_map.insert(*pin, *pin);
                }
                continue;
            }
            let position = [model.position[0] + 40.0, model.position[1] + 80.0];
            if let Some(binding) = clip
                .bindings
                .get(id)
                .filter(|binding| binding.assignment_output.is_some())
            {
                let new = next
                    .add_catalog_node(NodeKind::VariableGet, position)
                    .map_err(|error| error.to_string())?;
                next.set_property(new, "name", PropertyValue::String(binding.name.clone()))
                    .map_err(|error| error.to_string())?;
                let output = next
                    .pin_by_key(new, "value")
                    .ok_or("Missing scoped variable value")?
                    .id;
                pin_map.insert(binding.assignment_output.unwrap(), output);
                node_map.insert(*id, new);
                continue;
            }
            let new = next.add_node(model.kind, position);
            node_map.insert(*id, new);
            let new_model = next.nodes.get_mut(&new).unwrap();
            new_model.properties = model.properties.clone();
            new_model.title_override = model.title_override.clone();
            for id in &model.pins {
                let pin = &clip.pins[id];
                let new_pin = next
                    .add_pin(
                        new,
                        &pin.key,
                        &pin.label,
                        pin.direction,
                        pin.value_type.clone(),
                        pin.cardinality,
                    )
                    .map_err(|error| error.to_string())?;
                next.pins.get_mut(&new_pin).unwrap().default_value = pin.default_value.clone();
                pin_map.insert(*id, new_pin);
            }
        }
        for edge in &clip.edges {
            if reused[&clip.pins[&edge.input].node] {
                continue;
            }
            let output = *pin_map
                .get(&edge.output)
                .ok_or("Missing copied data output")?;
            let input = *pin_map
                .get(&edge.input)
                .ok_or("Missing copied data input")?;
            next.connect(output, input)
                .map_err(|error| error.to_string())?;
        }
        let mut used: BTreeSet<_> = next.interface_component_ids().into_keys().collect();
        for (old, identity) in &clip.identities {
            let base = identity.chars().take(96).collect::<String>();
            let mut index = 1;
            let fresh = loop {
                let value = format!("{base}_copy_{index}");
                if used.insert(value.clone()) {
                    break value;
                }
                index += 1;
            };
            next.set_literal_input(node_map[old], "id", PropertyValue::String(fresh))?;
        }
        let root = node_map[&clip.root];
        next.interface_componentize(parent)?;
        next.append_list_input(parent, "children", root, "value")?;
        *self = next;
        Ok(root)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn screen(source: &str) -> GraphDocument {
        import_script(&rvn_parser::parse(source).unwrap())
            .unwrap()
            .into_iter()
            .find(|graph| matches!(graph.kind, GraphKind::Screen { .. }))
            .unwrap()
    }
    fn returned(graph: &GraphDocument) -> NodeId {
        graph
            .nodes
            .values()
            .find(|node| node.kind == NodeKind::FunctionReturn)
            .unwrap()
            .id
    }
    fn producer(graph: &GraphDocument, node: NodeId, key: &str) -> PinId {
        let input = graph.pin_by_key(node, key).unwrap().id;
        graph
            .edges
            .values()
            .find(|edge| edge.input == input)
            .unwrap()
            .output
    }
    #[test]
    fn copying_reuses_unchanged_shared_producers_without_freezing_parameters_or_calls() {
        let mut project=import_script(&rvn_parser::parse(r#"function caption(t){return t} screen form(title){return component("root","column",{},[component("label","text",{"text":caption(title)},[])])}"#).unwrap()).unwrap();
        let index=project.iter().position(|graph|matches!(graph.kind,GraphKind::Screen{..})).unwrap();
        let mut graph=project[index].clone();
        let ret = returned(&graph);
        let ids = graph.interface_component_ids();
        let properties = producer(&graph, ids["label"], "properties");
        let before = graph.clone();
        let clip = graph.copy_interface_subtree(ret, ids["label"]).unwrap();
        assert_eq!(graph, before);
        assert!(clip
            .function_dependencies()
            .any(|item| item == ("caption", 1)));
        let pasted = graph
            .paste_interface_subtree(ret, ids["root"], &clip)
            .unwrap();
        assert_eq!(producer(&graph, pasted, "properties"), properties);
        assert!(graph.interface_component_ids().contains_key("label_copy_1"));
        project[index]=graph;
        let text = transpile_project(&project).unwrap().source;
        assert!(text.contains("caption(title)"));
    }
    #[test]
    fn cross_screen_paste_preserves_parameter_expression_and_rejects_scope_changes_atomically() {
        let source = screen(
            r#"screen source(title){return {"id":"root","kind":"column","children":[{"id":"label","kind":"text","text":title}]} }"#,
        );
        let clip = source
            .copy_interface_subtree(returned(&source), source.interface_component_ids()["label"])
            .unwrap();
        let mut target =
            screen(r#"screen target(title){return component("target","column",{},[])}"#);
        let ret = returned(&target);
        let root = target.interface_component_ids()["target"];
        target.paste_interface_subtree(ret, root, &clip).unwrap();
        let text = transpile_project(&[target]).unwrap().source;
        assert!(text.contains("title"));
        assert!(text.contains("label_copy_1"));
        let mut wrong =
            screen(r#"screen target(other){return component("target","column",{},[])}"#);
        let before = wrong.clone();
        let ret = returned(&wrong);
        let root = wrong.interface_component_ids()["target"];
        assert!(wrong
            .paste_interface_subtree(ret, root, &clip)
            .unwrap_err()
            .contains("title"));
        assert_eq!(wrong, before);
    }
    #[test]
    fn nested_pastes_own_component_collections_and_generate_all_new_identities() {
        let mut graph = screen(
            r#"screen form(){return component("root","row",{},[component("column","column",{},[component("label","text",{"text":"Hi"},[])])])}"#,
        );
        let ret = returned(&graph);
        let ids = graph.interface_component_ids();
        let clip = graph.copy_interface_subtree(ret, ids["column"]).unwrap();
        assert_eq!(clip.component_count(), 2);
        let pasted = graph
            .paste_interface_subtree(ret, ids["root"], &clip)
            .unwrap();
        graph
            .set_component_property(pasted, &["padding"], PropertyValue::Int(24))
            .unwrap();
        assert_eq!(
            graph
                .dictionary_property(ids["column"], "properties", &["padding"])
                .unwrap(),
            None
        );
        let tree = graph.interface_tree(ret).unwrap();
        assert_eq!(tree.children.len(), 2);
        assert_eq!(
            tree.children[1].children[0].id.as_deref(),
            Some("label_copy_1")
        );
    }
    #[test]
    fn computed_children_are_refused_and_changed_producers_are_captured_as_expressions() {
        let generated = screen(r#"screen form(items){return component("root","column",{},items)}"#);
        assert!(generated
            .copy_interface_subtree(
                returned(&generated),
                generated.interface_component_ids()["root"]
            )
            .is_err());
        let mut graph = screen(
            r#"screen form(title){return component("root","column",{},[component("label","text",{"text":title},[])])}"#,
        );
        let ret = returned(&graph);
        let ids = graph.interface_component_ids();
        let clip = graph.copy_interface_subtree(ret, ids["label"]).unwrap();
        let old = producer(&graph, ids["label"], "properties");
        let props = graph.pins[&old].node;
        graph.nodes.get_mut(&props).unwrap().title_override = Some("Edited expression".into());
        let new = graph
            .paste_interface_subtree(ret, ids["root"], &clip)
            .unwrap();
        assert_ne!(producer(&graph, new, "properties"), old);
        assert!(transpile_project(&[graph])
            .unwrap()
            .source
            .contains("title"));
    }
    #[test]
    fn authored_canvas_in_a_dynamic_parent_keeps_local_producers_and_function_identity() {
        let mut project=import_script(&rvn_parser::parse(r#"function inventory(){return [1,2]} function atlas_draw(state,props,event){return []} screen atlas(context){local items=inventory() local children=[component("surface","canvas",{"draw":"atlas_draw","state":{"hover":-1},"props":{"items":items,"context":context}},[])] set children=list_append(children,component("dynamic","text",{"text":context["title"]},[])) return component("root","panel",{},children)}"#).unwrap()).unwrap();
        let index=project.iter().position(|graph|matches!(graph.kind,GraphKind::Screen{..})).unwrap();
        let mut graph=project[index].clone();
        let ids=graph.interface_component_ids();let ret=returned(&graph);let before=graph.clone();
        assert!(graph.interface_tree(ret).unwrap().computed_children.is_some());
        let clip=graph.copy_interface_subtree(ret,ids["surface"]).unwrap();
        assert_eq!(clip.component_count(),1);assert_eq!(graph,before);
        assert!(clip.bindings.values().any(|binding|binding.name=="items"&&binding.scope==VariableScope::Local));
        assert!(clip.bindings.values().any(|binding|binding.name=="context"&&binding.scope==VariableScope::Parameter));
        assert_eq!(graph.paste_interface_subtree(ret,ids["root"],&clip).unwrap_err(),"Paste into an authored panel, column or row");
        assert_eq!(graph,before);
        project[index]=graph;
        let source=transpile_project(&project).unwrap().source;
        assert!(source.contains("inventory()"));assert!(source.contains("list_append"));assert!(source.contains("atlas_draw"));
    }
}
