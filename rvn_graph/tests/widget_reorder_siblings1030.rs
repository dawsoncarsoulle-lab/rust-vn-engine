//! Source/graph/runtime semantics. No native drag/drop or pixel-parity claim.
use rvn_graph::*;
use rvn_ui::programmable::Component;

const SOURCE: &str = r#"// Keep this authored source comment.
screen panel(title,pad) {
 return component("root","column",{"layout_contract":"widget_v1"},[
  component("a","text",{"text":title,"parent_slot":{"kind":"vertical_box","padding":{"left":pad}}},[]),
  component("b","text",{"text":"B","parent_slot":{"kind":"vertical_box"}},[]),
  component("c","text",{"text":"C","parent_slot":{"kind":"vertical_box"}},[]),
  component("branch","column",{"parent_slot":{"kind":"vertical_box"}},[
   component("nested","text",{"text":"Nested","parent_slot":{"kind":"vertical_box"}},[])
  ])
 ])
}
"#;
fn graph(source: &str) -> GraphDocument {
    import_script(&rvn_parser::parse(source).unwrap()).unwrap().into_iter()
        .find(|graph| matches!(graph.kind, GraphKind::Screen { .. })).unwrap()
}
fn returned(graph: &GraphDocument) -> NodeId {
    graph.nodes.values().find(|node| node.kind == NodeKind::FunctionReturn).unwrap().id
}
fn names(graph: &GraphDocument) -> Vec<String> {
    graph.interface_tree(returned(graph)).unwrap().children.iter()
        .map(|child| child.id.clone().unwrap()).collect()
}
fn evaluate(graph: &GraphDocument, text: &str, padding: f32) -> Component {
    let generated = transpile(graph).unwrap();
    let library = rvn_core::eval::FunctionLibrary::from_script(&generated.ast).unwrap();
    let (parameters, body) = generated.ast.iter().find_map(|statement| match statement {
        rvn_parser::Statement::Screen { parameters, body, .. } => Some((parameters, body)),
        _ => None,
    }).unwrap();
    let value = library.describe_screen(parameters, body,
        &[rvn_parser::Value::Str(text.into()), rvn_parser::Value::Float(padding)],
        &Default::default(), &mut rvn_core::random::RandomState::default()).unwrap();
    Component::parse(rvn_core::ui::value_to_json(&value).unwrap()).unwrap()
}

#[test]
fn widget_reorder1030_before_after_all_forward_backward_pairs_match_removal_then_insertion() {
    for from in 0..4 {
        for anchor in 0..4 {
            if from == anchor { continue; }
            for placement in [WidgetSiblingPlacement::Before, WidgetSiblingPlacement::After] {
                let mut graph = graph(SOURCE);
                let ids = graph.interface_component_ids();
                let original = ["a", "b", "c", "branch"];
                let mut expected = original.iter().map(|name| (*name).to_owned()).collect::<Vec<_>>();
                let item = expected.remove(from);
                let target = expected.iter().position(|name| name == original[anchor]).unwrap()
                    + usize::from(placement == WidgetSiblingPlacement::After);
                expected.insert(target, item);
                let returned = returned(&graph);
                graph.interface_widget_reorder_sibling(returned, ids[original[from]], ids[original[anchor]], placement).unwrap();
                assert_eq!(names(&graph), expected, "from={from} anchor={anchor} placement={placement:?}");
                assert_eq!(graph.interface_component_ids(), ids);
            }
        }
    }
}

#[test]
fn widget_reorder1030_adjacent_noop_preserves_exact_graph_and_source_without_allocation() {
    for (moving, anchor, placement) in [
        ("a", "b", WidgetSiblingPlacement::Before),
        ("b", "a", WidgetSiblingPlacement::After),
    ] {
        let mut graph = graph(SOURCE);
        let ids = graph.interface_component_ids();
        let before = graph.clone();
        let source = transpile(&graph).unwrap().source;
        let returned = returned(&graph);
        assert!(!graph.interface_widget_reorder_sibling(returned, ids[moving], ids[anchor], placement).unwrap());
        assert_eq!(graph, before);
        assert_eq!(transpile(&graph).unwrap().source, source);
    }
}

#[test]
fn widget_reorder1030_dynamic_text_and_slot_remain_live_through_source_publication_and_reimport() {
    let mut project = SourceProject::open(SOURCE, &[]).unwrap();
    let mut graphs = project.graphs().to_vec();
    let graph = graphs.iter_mut().find(|graph| matches!(graph.kind, GraphKind::Screen { .. })).unwrap();
    let before = graph.clone();
    let ids = graph.interface_component_ids();
    let returned = returned(graph);
    let original_a = graph.nodes[&ids["a"]].clone();
    let original_properties = transpile_value_input(graph, ids["a"], "properties").unwrap();
    assert!(graph.interface_widget_reorder_sibling(returned, ids["a"], ids["c"], WidgetSiblingPlacement::After).unwrap());
    assert_eq!(graph.nodes[&ids["a"]], original_a);
    assert_eq!(transpile_value_input(graph, ids["a"], "properties").unwrap(), original_properties);
    for (id, node) in &before.nodes { assert_eq!(graph.nodes[id].position, node.position); }
    let first = evaluate(graph, "First", 13.0);
    let second = evaluate(graph, "Second", 21.0);
    assert_eq!(first.children.iter().map(|child| child.id.as_str()).collect::<Vec<_>>(), vec!["b", "c", "a", "branch"]);
    assert_eq!(first.find("a").unwrap().text, "First");
    assert_eq!(second.find("a").unwrap().text, "Second");
    assert_ne!(first.find("a").unwrap().parent_slot, second.find("a").unwrap().parent_slot);
    project.apply_visual(SOURCE, &graphs).unwrap();
    assert!(project.source().contains("// Keep this authored source comment."));
    let reopened = SourceProject::open(project.source(), &graphs).unwrap();
    let saved = reopened.graphs().iter().find(|item| item.graph_id == before.graph_id).unwrap();
    assert_eq!(saved.interface_component_ids(), ids);
    assert_eq!(evaluate(saved, "Second", 21.0), second);
}

#[test]
fn widget_reorder1030_horizontal_box_uses_same_order_with_original_typed_slots() {
    let source = SOURCE.replace("\"column\"", "\"row\"").replace("\"vertical_box\"", "\"horizontal_box\"");
    let mut graph = graph(&source);
    let ids = graph.interface_component_ids();
    let before = evaluate(&graph, "Row", 7.0);
    let returned = returned(&graph);
    assert!(graph.interface_widget_reorder_sibling(returned, ids["c"], ids["a"], WidgetSiblingPlacement::Before).unwrap());
    let after = evaluate(&graph, "Row", 7.0);
    assert_eq!(names(&graph), vec!["c", "a", "b", "branch"]);
    for name in ["a", "b", "c", "branch", "nested"] {
        assert_eq!(after.find(name).unwrap().parent_slot, before.find(name).unwrap().parent_slot);
    }
}

#[test]
fn widget_reorder1030_self_root_cross_parent_and_missing_identity_refuse_without_changes() {
    let mut graph = graph(SOURCE);
    let ids = graph.interface_component_ids();
    let before = graph.clone();
    let returned = returned(&graph);
    for (node, sibling) in [
        (ids["a"], ids["a"]), (ids["root"], ids["a"]),
        (ids["a"], ids["root"]), (ids["a"], ids["nested"]),
        (ids["nested"], ids["a"]), (returned, ids["a"]),
    ] {
        assert!(graph.interface_widget_reorder_sibling(returned, node, sibling, WidgetSiblingPlacement::After).is_err());
        assert_eq!(graph, before);
    }
}

#[test]
fn widget_reorder1030_shared_child_collection_is_copied_without_changing_other_consumer() {
    let mut graph = graph(SOURCE);
    let ids = graph.interface_component_ids();
    let parent = ids["root"];
    let children = graph.pin_by_key(parent, "children").unwrap().id;
    let output = graph.edges.values().find(|edge| edge.input == children).unwrap().output;
    let other = graph.add_catalog_node(NodeKind::UiComponent, [930.0, 10.0]).unwrap();
    graph.set_literal_input(other, "id", PropertyValue::String("other".into())).unwrap();
    graph.set_literal_input(other, "kind", PropertyValue::String("column".into())).unwrap();
    let other_input = graph.pin_by_key(other, "children").unwrap().id;
    graph.connect(output, other_input).unwrap();
    let original_other = transpile_value_input(&graph, other, "children").unwrap();
    let original_list = graph.nodes[&graph.pins[&output].node].clone();
    let returned = returned(&graph);
    assert!(graph.interface_widget_reorder_sibling(returned, ids["a"], ids["c"], WidgetSiblingPlacement::After).unwrap());
    assert_eq!(names(&graph), vec!["b", "c", "a", "branch"]);
    assert_eq!(graph.nodes[&original_list.id], original_list);
    assert_eq!(transpile_value_input(&graph, other, "children").unwrap(), original_other);
}

#[test]
fn widget_reorder1030_ambiguous_component_consumer_refuses_before_copying_collection() {
    let mut graph = graph(SOURCE);
    let ids = graph.interface_component_ids();
    let other = graph.add_catalog_node(NodeKind::UiComponent, [930.0, 10.0]).unwrap();
    graph.set_literal_input(other, "id", PropertyValue::String("other".into())).unwrap();
    graph.set_literal_input(other, "kind", PropertyValue::String("column".into())).unwrap();
    graph.append_list_input(other, "children", ids["a"], "value").unwrap();
    let before = graph.clone();
    let returned = returned(&graph);
    assert!(graph.interface_widget_reorder_sibling(returned, ids["a"], ids["c"], WidgetSiblingPlacement::After).is_err());
    assert_eq!(graph, before);
}

#[test]
fn widget_reorder1030_legacy_or_computed_children_are_not_materialized_as_widget_tree() {
    for source in [
        SOURCE.replace("\"layout_contract\":\"widget_v1\"", "\"spacing\":0"),
        "screen panel(children) {return component(\"root\",\"column\",{\"layout_contract\":\"widget_v1\"},children)}".into(),
    ] {
        let mut graph = graph(&source);
        let ids = graph.interface_component_ids();
        let before = graph.clone();
        let returned = returned(&graph);
        let node = ids.get("a").copied().unwrap_or(ids["root"]);
        let sibling = ids.get("c").copied().unwrap_or(returned);
        assert!(graph.interface_widget_reorder_sibling(returned, node, sibling, WidgetSiblingPlacement::After).is_err());
        assert_eq!(graph, before);
    }
}
