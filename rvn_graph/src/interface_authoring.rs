//! A source-first designer view over the existing expression graph. Computed
//! components are navigation placeholders, not snapshots of their current data.
use crate::*;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterfaceAuthoringNode {
    pub node: NodeId,
    pub id: Option<String>,
    pub kind: Option<String>,
    pub raw_dictionary: bool,
    pub computed: bool,
    pub computed_children: Option<NodeId>,
    pub children: Vec<InterfaceAuthoringNode>,
}

fn producer(graph: &GraphDocument, pin: PinId) -> Result<Option<PinId>, String> {
    let mut edges = graph.edges.values().filter(|edge| edge.input == pin);
    let output = edges.next().map(|edge| edge.output);
    if edges.next().is_some() {
        return Err("Designer input has several producers".into());
    }
    Ok(output)
}
fn literal(graph: &GraphDocument, pin: PinId) -> Option<PropertyValue> {
    if let Ok(Some(output)) = producer(graph, pin) {
        let node = &graph.nodes[&graph.pins[&output].node];
        if matches!(node.kind, NodeKind::Literal | NodeKind::TextValue) {
            return node.properties.get("value").cloned();
        }
        return None;
    }
    graph.pins[&pin].default_value.clone()
}
fn fields(
    graph: &GraphDocument,
    node: NodeId,
) -> Result<Option<Vec<(String, PinId, PinId)>>, String> {
    let model = graph.nodes.get(&node).ok_or("Unknown interface node")?;
    if model.kind != NodeKind::FunctionCall
        || model.properties.get("function") != Some(&PropertyValue::String("dict".into()))
    {
        return Ok(None);
    }
    let count = match model.properties.get("input_count") {
        Some(PropertyValue::Int(count)) if (0..=128).contains(count) && count % 2 == 0 => {
            *count as usize
        }
        _ => return Err("Invalid interface dictionary pairs".into()),
    };
    let mut result = Vec::new();
    let mut seen = BTreeSet::new();
    for index in (0..count).step_by(2) {
        let key = graph
            .pin_by_key(node, &format!("item_{index}"))
            .ok_or("Missing dictionary key")?
            .id;
        let value = graph
            .pin_by_key(node, &format!("item_{}", index + 1))
            .ok_or("Missing dictionary value")?
            .id;
        let Some(PropertyValue::String(name)) = literal(graph, key) else {
            return Ok(None);
        };
        if !seen.insert(name.clone()) {
            return Err("Duplicate interface dictionary property".into());
        }
        result.push((name, key, value));
    }
    Ok(Some(result))
}
fn component_inputs(
    graph: &GraphDocument,
    node: NodeId,
) -> Result<Option<(PinId, PinId, Option<PinId>, bool)>, String> {
    if graph.nodes[&node].kind == NodeKind::UiComponent {
        return Ok(Some((
            graph
                .pin_by_key(node, "id")
                .ok_or("Missing component identity")?
                .id,
            graph
                .pin_by_key(node, "kind")
                .ok_or("Missing component kind")?
                .id,
            graph.pin_by_key(node, "children").map(|pin| pin.id),
            false,
        )));
    }
    let Some(fields) = fields(graph, node)? else {
        return Ok(None);
    };
    let field = |name: &str| {
        fields
            .iter()
            .find(|(key, _, _)| key == name)
            .map(|(_, _, pin)| *pin)
    };
    Ok(field("id")
        .zip(field("kind"))
        .map(|(id, kind)| (id, kind, field("children"), true)))
}

impl GraphDocument {
    /// Duplicate an authored subtree with fresh component identities. Reused
    /// data/style expressions remain connected and become copy-on-write when
    /// edited. Generated children are not duplicated blindly: their functions
    /// need explicit identity parameters in the graph.
    pub fn interface_duplicate(
        &mut self,
        return_node: NodeId,
        node: NodeId,
    ) -> Result<NodeId, String> {
        let tree = self.interface_tree(return_node)?;
        fn find(tree: &InterfaceAuthoringNode, node: NodeId) -> Option<&InterfaceAuthoringNode> {
            if tree.node == node {
                Some(tree)
            } else {
                tree.children.iter().find_map(|child| find(child, node))
            }
        }
        fn parent(tree: &InterfaceAuthoringNode, node: NodeId) -> Option<NodeId> {
            if tree.children.iter().any(|child| child.node == node) {
                Some(tree.node)
            } else {
                tree.children.iter().find_map(|child| parent(child, node))
            }
        }
        let subtree = find(&tree, node).ok_or("Unknown authored component")?;
        let parent = parent(&tree, node).ok_or("Duplicate a child, not the screen root")?;
        fn check(tree: &InterfaceAuthoringNode) -> Result<(), String> {
            if tree.computed || tree.computed_children.is_some() || tree.id.is_none() {
                return Err("Generated components need explicit identity parameters: duplicate in the graph".into());
            }
            for child in &tree.children {
                check(child)?;
            }
            Ok(())
        }
        check(subtree)?;
        let mut next = self.clone();
        let mut ids: BTreeSet<_> = next.interface_component_ids().into_keys().collect();
        fn copy(
            graph: &mut GraphDocument,
            tree: &InterfaceAuthoringNode,
            ids: &mut BTreeSet<String>,
        ) -> Result<NodeId, String> {
            let old = graph.nodes[&tree.node].clone();
            let new = graph.add_node(old.kind, [old.position[0] + 40.0, old.position[1] + 80.0]);
            graph.nodes.get_mut(&new).unwrap().properties = old.properties;
            graph.nodes.get_mut(&new).unwrap().title_override = old.title_override;
            for pin in old.pins {
                let model = graph.pins[&pin].clone();
                let cloned = graph
                    .add_pin(
                        new,
                        model.key,
                        model.label,
                        model.direction,
                        model.value_type,
                        model.cardinality,
                    )
                    .map_err(|error| error.to_string())?;
                graph.pins.get_mut(&cloned).unwrap().default_value = model.default_value;
                if model.direction == PinDirection::Input {
                    if let Some(output) = producer(graph, pin)? {
                        graph
                            .connect(output, cloned)
                            .map_err(|error| error.to_string())?;
                    }
                }
            }
            graph.interface_componentize(new)?;
            let base = tree
                .id
                .as_deref()
                .unwrap()
                .chars()
                .take(96)
                .collect::<String>();
            let mut index = 1;
            let identity = loop {
                let candidate = format!("{base}_copy_{index}");
                if ids.insert(candidate.clone()) {
                    break candidate;
                }
                index += 1;
            };
            graph.set_literal_input(new, "id", PropertyValue::String(identity))?;
            let children = graph.pin_by_key(new, "children").unwrap().id;
            graph.edges.retain(|_, edge| edge.input != children);
            graph.pins.get_mut(&children).unwrap().default_value = None;
            for child in &tree.children {
                let cloned = copy(graph, child, ids)?;
                graph.append_list_input(new, "children", cloned, "value")?;
            }
            Ok(new)
        }
        let new = copy(&mut next, subtree, &mut ids)?;
        next.interface_componentize(parent)?;
        next.append_list_input(parent, "children", new, "value")?;
        *self = next;
        Ok(new)
    }

    /// Move one authored child to an authored container. Cycles, calculated
    /// collections and roots are rejected before any edit is committed.
    pub fn interface_reparent(
        &mut self,
        return_node: NodeId,
        node: NodeId,
        new_parent: NodeId,
    ) -> Result<NodeId, String> {
        let tree = self.interface_tree(return_node)?;
        fn find(tree: &InterfaceAuthoringNode, node: NodeId) -> Option<&InterfaceAuthoringNode> {
            if tree.node == node {
                Some(tree)
            } else {
                tree.children.iter().find_map(|child| find(child, node))
            }
        }
        fn parent(tree: &InterfaceAuthoringNode, node: NodeId) -> Option<(NodeId, usize)> {
            tree.children
                .iter()
                .position(|child| child.node == node)
                .map(|index| (tree.node, index))
                .or_else(|| tree.children.iter().find_map(|child| parent(child, node)))
        }
        let child = find(&tree, node).ok_or("Unknown authored component")?;
        if child.computed || find(child, new_parent).is_some() {
            return Err("A component cannot move inside itself or a computed expression".into());
        }
        let target = find(&tree, new_parent).ok_or("Unknown target container")?;
        if target.computed
            || target.computed_children.is_some()
            || !matches!(target.kind.as_deref(), Some("panel" | "column" | "row"))
        {
            return Err("Target must be an authored panel, column or row".into());
        }
        let (old_parent, index) =
            parent(&tree, node).ok_or("The root component cannot be reparented")?;
        let mut next = self.clone();
        next.interface_componentize(old_parent)?;
        next.interface_componentize(new_parent)?;
        let owned = next.own_list_input_node(old_parent, "children", index)?;
        next.remove_list_input(old_parent, "children", index)?;
        let owned = next.interface_componentize(owned)?;
        next.append_list_input(new_parent, "children", owned, "value")?;
        *self = next;
        Ok(owned)
    }

    /// Read the returned expression without changing any graph or source file.
    pub fn interface_tree(&self, return_node: NodeId) -> Result<InterfaceAuthoringNode, String> {
        if self
            .nodes
            .get(&return_node)
            .is_none_or(|node| node.kind != NodeKind::FunctionReturn)
        {
            return Err("Expected a screen return node".into());
        }
        let pin = self
            .pin_by_key(return_node, "value")
            .ok_or("Missing screen return value")?
            .id;
        let output =
            producer(self, pin)?.ok_or("Connect a screen component to the return value")?;
        self.interface_expression_tree(self.pins[&output].node)
    }

    /// Read an authored component expression even when its parent's collection
    /// is assembled by control flow. No evaluation or materialization occurs.
    pub(crate) fn interface_expression_tree(&self, node: NodeId) -> Result<InterfaceAuthoringNode, String> {
        if !self.nodes.contains_key(&node){return Err("Unknown authored component".into());}
        fn walk(
            graph: &GraphDocument,
            node: NodeId,
            active: &mut BTreeSet<NodeId>,
            depth: usize,
            count: &mut usize,
        ) -> Result<InterfaceAuthoringNode, String> {
            if depth > 32 || *count >= 512 {
                return Err("Designer limit: 512 components and 32 nesting levels".into());
            }
            *count += 1;
            if !active.insert(node) {
                return Err("Cyclic interface expression".into());
            }
            let mut result = InterfaceAuthoringNode {
                node,
                id: None,
                kind: None,
                raw_dictionary: false,
                computed: true,
                computed_children: None,
                children: Vec::new(),
            };
            if let Some((id, kind, children, raw)) = component_inputs(graph, node)? {
                let text = |pin| match literal(graph, pin) {
                    Some(PropertyValue::String(value)) => Some(value),
                    _ => None,
                };
                result.id = text(id);
                result.kind = text(kind);
                result.raw_dictionary = raw;
                result.computed = false;
                if let Some(pin) = children {
                    if let Some(output)=producer(graph,pin)?{
                        let list=graph.pins[&output].node;
                        if graph.nodes[&list].kind==NodeKind::ListLiteral{
                            let child_count=match graph.nodes[&list].properties.get("input_count"){Some(PropertyValue::Int(count))if (0..=128).contains(count)=>*count as usize,_=>return Err("Invalid child collection".into())};
                            for index in 0..child_count{
                                let pin=graph.pin_by_key(list,&format!("item_{index}")).ok_or("Missing child collection item")?.id;
                                if let Some(output)=producer(graph,pin)?{result.children.push(walk(graph,graph.pins[&output].node,active,depth+1,count)?);}
                                else{return Err("Interface child must be an authored component or a connected expression".into());}
                            }
                        }else{result.computed_children=Some(list);}
                    }else if graph.pins[&pin].default_value.as_ref().is_some_and(|value|!matches!(value,PropertyValue::StringList(values) if values.is_empty())){return Err("Interface children are not a component list".into());}
                }
            }
            active.remove(&node);
            Ok(result)
        }
        walk(
            self,
            node,
            &mut BTreeSet::new(),
            0,
            &mut 0,
        )
    }

    /// Stable, literal identities are editable even when their visible text,
    /// bindings, style or descendants are computed expressions.
    pub fn interface_component_ids(&self) -> BTreeMap<String, NodeId> {
        let mut result = BTreeMap::new();
        let mut duplicates = BTreeSet::new();
        for node in self.nodes.keys() {
            if let Ok(Some((id, _, _, _))) = component_inputs(self, *node) {
                if let Some(PropertyValue::String(id)) = literal(self, id) {
                    if result.insert(id.clone(), *node).is_some() {
                        duplicates.insert(id);
                    }
                }
            }
        }
        for id in duplicates {
            result.remove(&id);
        }
        result
    }

    /// Intentional, atomic normalization of {id, kind, ...} to component(...).
    /// Every value expression and the main node identity are retained. This
    /// does not flatten function calls, computed property names or collections.
    pub fn interface_componentize(&mut self, node: NodeId) -> Result<NodeId, String> {
        if self
            .nodes
            .get(&node)
            .is_some_and(|node| node.kind == NodeKind::UiComponent)
        {
            return Ok(node);
        }
        let pairs = fields(self, node)?
            .ok_or("This component is computed: edit its Blueprint expression")?;
        if !pairs.iter().any(|(name, _, _)| name == "id")
            || !pairs.iter().any(|(name, _, _)| name == "kind")
        {
            return Err("Component dictionaries need id and kind".into());
        }
        let mut next = self.clone();
        let position = next.nodes[&node].position;
        let properties = next
            .add_catalog_node(
                NodeKind::FunctionCall,
                [position[0] - 320.0, position[1] + 160.0],
            )
            .map_err(|error| error.to_string())?;
        next.nodes.get_mut(&properties).unwrap().properties = BTreeMap::from([
            ("function".into(), PropertyValue::String("dict".into())),
            ("input_count".into(), PropertyValue::Int(0)),
        ]);
        // Keep the original data output identity: all connected expressions
        // continue to address this component after the equivalent conversion.
        let output = next
            .pin_by_key(node, "result")
            .ok_or("Missing dictionary output")?
            .id;
        {
            let pin = next.pins.get_mut(&output).unwrap();
            pin.key = "value".into();
            pin.label = "Composant".into();
        }
        let mut component_pins = vec![output];
        let mut prop_pins = next.nodes[&properties].pins.clone();
        let mut count = 0;
        for (name, key, value) in pairs {
            if ["id", "kind", "children"].contains(&name.as_str()) {
                next.edges
                    .retain(|_, edge| edge.input != key && edge.output != key);
                next.pins.remove(&key);
                let pin = next.pins.get_mut(&value).unwrap();
                pin.key = name.clone();
                pin.label = name.clone();
                pin.value_type = if name == "children" {
                    ValueType::List(Box::new(ValueType::Any))
                } else {
                    ValueType::String
                };
                component_pins.push(value);
            } else {
                for (pin, offset) in [(key, 0), (value, 1)] {
                    let model = next.pins.get_mut(&pin).unwrap();
                    model.node = properties;
                    model.key = format!("item_{}", count + offset);
                    model.label = format!("[{}]", count + offset);
                    prop_pins.push(pin);
                }
                count += 2;
            }
        }
        next.nodes.get_mut(&properties).unwrap().pins = prop_pins;
        next.nodes
            .get_mut(&properties)
            .unwrap()
            .properties
            .insert("input_count".into(), PropertyValue::Int(count));
        {
            let model = next.nodes.get_mut(&node).unwrap();
            model.kind = NodeKind::UiComponent;
            model.properties.clear();
            model.pins = component_pins;
        }
        if next.pin_by_key(node, "children").is_none() {
            next.add_pin(
                node,
                "children",
                "Enfants",
                PinDirection::Input,
                ValueType::List(Box::new(ValueType::Any)),
                PinCardinality::One,
            )
            .map_err(|error| error.to_string())?;
        }
        let prop_input = next
            .add_pin(
                node,
                "properties",
                "Propriétés",
                PinDirection::Input,
                ValueType::Any,
                PinCardinality::One,
            )
            .map_err(|error| error.to_string())?;
        next.connect(
            next.pin_by_key(properties, "result").unwrap().id,
            prop_input,
        )
        .map_err(|error| error.to_string())?;
        *self = next;
        Ok(node)
    }
}
