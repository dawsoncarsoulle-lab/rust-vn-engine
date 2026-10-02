use rvn_graph::*;
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

#[test]
fn literal_dictionaries_normalize_without_freezing_expressions_or_repositioning_nodes() {
    let source = r#"// Original screen comment.
screen form(title){
 // Dynamic content stays connected.
 return {"id":"root","kind":"column","padding":8,"children":[{"id":"caption","kind":"text","text":title}]}
}"#;
    let mut project = SourceProject::open(source, &[]).unwrap();
    let mut graphs = project.graphs().to_vec();
    let graph = graphs
        .iter_mut()
        .find(|graph| matches!(graph.kind, GraphKind::Screen { .. }))
        .unwrap();
    let before = graph.clone();
    let ret = returned(graph);
    let tree = graph.interface_tree(ret).unwrap();
    assert_eq!(tree.id.as_deref(), Some("root"));
    assert!(tree.raw_dictionary);
    assert_eq!(tree.children[0].id.as_deref(), Some("caption"));
    let root = graph.interface_componentize(tree.node).unwrap();
    let caption = graph.interface_componentize(tree.children[0].node).unwrap();
    assert_eq!(root, tree.node);
    assert_eq!(graph.interface_component_ids()["caption"], caption);
    graph
        .set_component_property(root, &["padding"], PropertyValue::Int(16))
        .unwrap();
    let edited = graph.clone();
    assert!(graph
        .set_component_property(
            caption,
            &["text"],
            PropertyValue::String("Do not freeze".into())
        )
        .is_err());
    assert_eq!(*graph, edited);
    for (id, node) in &before.nodes {
        assert_eq!(graph.nodes[id].position, node.position);
    }
    let output = transpile_project(&graphs).unwrap();
    assert!(output.source.contains("component("));
    assert!(output.source.contains("title"));
    project.apply_visual(source, &graphs).unwrap();
    assert!(project
        .source()
        .contains("// Dynamic content stays connected."));
    assert!(project.source().contains("// Original screen comment."));
    let again = reimport_script(&rvn_parser::parse(project.source()).unwrap(), &graphs).unwrap();
    assert_eq!(
        again
            .iter()
            .find(|graph| matches!(graph.kind, GraphKind::Screen { .. }))
            .unwrap()
            .interface_component_ids()["root"],
        root
    );
}

#[test]
fn function_children_and_computed_collections_remain_graph_navigation_placeholders() {
    let graph = screen(
        r#"function item(caption){return component("reused","text",{"text":caption},[])}
screen form(rows){return component("root","column",{},[item("Label"),component("list","column",{},rows)])}"#,
    );
    let tree = graph.interface_tree(returned(&graph)).unwrap();
    assert_eq!(tree.children.len(), 2);
    assert!(tree.children[0].computed);
    assert!(tree.children[1].computed_children.is_some());
    assert!(!graph.interface_component_ids().contains_key("reused"));
    let mut trial = graph.clone();
    assert!(trial.interface_componentize(tree.children[0].node).is_err());
    assert_eq!(trial, graph);
}

#[test]
fn reparenting_is_atomic_ordered_and_rejects_cycles() {
    let mut graph = screen(
        r#"screen form(){return component("root","row",{},[component("left","column",{},[component("caption","text",{"text":"Hello"},[])]),component("right","column",{},[])])}"#,
    );
    let ret = returned(&graph);
    let ids = graph.interface_component_ids();
    let before = graph.clone();
    assert!(graph
        .interface_reparent(ret, ids["left"], ids["caption"])
        .is_err());
    assert_eq!(graph, before);
    graph
        .interface_reparent(ret, ids["caption"], ids["right"])
        .unwrap();
    let tree = graph.interface_tree(ret).unwrap();
    assert!(tree.children[0].children.is_empty());
    assert_eq!(tree.children[1].children[0].id.as_deref(), Some("caption"));
    assert!(transpile_project(&[graph]).is_ok());
}

#[test]
fn shared_style_connections_copy_only_the_edited_dictionary_and_preserve_calculated_values() {
    let mut graph = screen(
        r#"screen form(){return component("root","row",{},[component("left","button",{"text":"Left"},[]),component("right","button",{"text":"Right"},[])])}"#,
    );
    let ids = graph.interface_component_ids();
    let left = ids["left"];
    let right = ids["right"];
    let properties = graph
        .edges
        .values()
        .find(|edge| edge.input == graph.pin_by_key(left, "properties").unwrap().id)
        .unwrap()
        .output;
    let right_input = graph.pin_by_key(right, "properties").unwrap().id;
    graph.edges.retain(|_, edge| edge.input != right_input);
    graph.connect(properties, right_input).unwrap();
    let style = graph
        .add_catalog_node(NodeKind::FunctionCall, [600.0, 800.0])
        .unwrap();
    graph
        .set_property(
            style,
            "function",
            PropertyValue::String("card_style".into()),
        )
        .unwrap();
    graph.resize_value_inputs(style, 0).unwrap();
    graph
        .connect_dictionary_property(left, "properties", &["style"], style, "result")
        .unwrap();
    assert_eq!(graph.component_property(right, &["style"]).unwrap(), None);
    assert!(transpile_value_input(&graph, left, "properties")
        .unwrap()
        .contains("card_style()"));
    let before = graph.clone();
    assert!(graph
        .connect_dictionary_property(left, "properties", &["style"], style, "result")
        .is_err());
    assert_eq!(graph, before);
}

#[test]
fn duplication_uses_fresh_subtree_ids_but_keeps_shared_styles_and_dynamic_text() {
    let mut graph = screen(
        r#"screen form(title){return component("root","row",{},[component("card","column",{},[component("caption","text",{"text":title},[])])])}"#,
    );
    let ret = returned(&graph);
    let original = graph.interface_component_ids();
    let copied = graph.interface_duplicate(ret, original["card"]).unwrap();
    let tree = graph.interface_tree(ret).unwrap();
    assert_eq!(tree.children.len(), 2);
    assert_eq!(tree.children[1].node, copied);
    assert_eq!(tree.children[1].id.as_deref(), Some("card_copy_1"));
    assert_eq!(
        tree.children[1].children[0].id.as_deref(),
        Some("caption_copy_1")
    );
    let clone = graph.interface_component_ids()["caption_copy_1"];
    let before = graph.clone();
    assert!(graph
        .set_component_property(clone, &["text"], PropertyValue::String("No freeze".into()))
        .is_err());
    assert_eq!(graph, before);
    assert!(transpile_value_input(&graph, clone, "properties")
        .unwrap()
        .contains("title"));
    assert_eq!(
        graph.interface_component_ids()["caption"],
        original["caption"]
    );
}
