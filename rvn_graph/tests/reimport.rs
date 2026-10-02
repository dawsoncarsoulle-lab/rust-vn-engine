use rvn_graph::*;

#[test]
fn unchanged_reimport_keeps_all_ids_layout_and_titles() {
    let script = rvn_parser::parse(
        "init { set n = 1 }\nlabel start\nset n = n + 1\nnarrator \"Hello\"\nreturn\n",
    )
    .unwrap();
    let mut graphs = import_script(&script).unwrap();
    for graph in &mut graphs {
        for node in graph.nodes.values_mut() {
            node.position = [node.id.get() as f64 * 17.0, -500.0];
            node.title_override = Some(format!("My node {}", node.id));
        }
    }
    assert_eq!(reimport_script(&script, &graphs).unwrap(), graphs);
}

#[test]
fn inserted_statement_and_new_label_do_not_shift_existing_identities() {
    let old_source = "label start\nnarrator \"Hello\"\nreturn\n";
    let mut previous = import_script(&rvn_parser::parse(old_source).unwrap()).unwrap();
    previous[1].graph_id = GraphId::new(80);
    let text = previous[1]
        .nodes
        .values()
        .find(|n| n.kind == NodeKind::TextValue)
        .unwrap()
        .id;
    previous[1].nodes.get_mut(&text).unwrap().position = [12000.0, 2000.0];
    let old = previous.clone();
    let source = rvn_parser::parse(
        "label start\nnarrator \"New\"\nnarrator \"Hello\"\nreturn\nlabel later\nreturn\n",
    )
    .unwrap();
    let graphs = reimport_script(&source, &previous).unwrap();
    assert_eq!(previous, old);
    assert_eq!(graphs[1].graph_id, GraphId::new(80));
    assert_eq!(graphs[1].nodes[&text].position, [12000.0, 2000.0]);
    let dialogue = previous[1]
        .nodes
        .values()
        .find(|node| node.kind == NodeKind::Dialogue)
        .unwrap()
        .id;
    let text_pin = graphs[1].pin_by_key(dialogue, "text").unwrap().id;
    assert!(graphs[1]
        .edges
        .values()
        .any(|edge| edge.input == text_pin && graphs[1].pins[&edge.output].node == text));
    assert!(graphs[2].graph_id.get() > 80);
    for graph in &graphs {
        assert!(!graph
            .validate()
            .iter()
            .any(|d| d.severity == DiagnosticSeverity::Error));
        assert_eq!(
            GraphDocument::from_json(&graph.to_pretty_json().unwrap()).unwrap(),
            *graph
        );
    }
    assert!(transpile_project(&graphs)
        .unwrap()
        .source
        .contains("\"New\""));
}

#[test]
fn changing_a_value_in_source_keeps_its_connected_node_identity_and_layout() {
    let source = "label start\nset number = 1 + 2\nreturn";
    let mut previous = import_script(&rvn_parser::parse(source).unwrap()).unwrap();
    let graph = &mut previous[1];
    let id = graph
        .nodes
        .values()
        .find(|node| node.properties.get("value") == Some(&PropertyValue::Int(2)))
        .unwrap()
        .id;
    graph.nodes.get_mut(&id).unwrap().position = [8000.0, 3100.0];
    let edited = rvn_parser::parse("label start\nset number = 1 + 7\nreturn").unwrap();
    let next = reimport_script(&edited, &previous).unwrap();
    assert_eq!(
        next[1].nodes[&id].properties["value"],
        PropertyValue::Int(7)
    );
    assert_eq!(next[1].nodes[&id].position, [8000.0, 3100.0]);
}

#[test]
fn failed_import_never_changes_the_previous_graphs() {
    let previous = import_script(&rvn_parser::parse("label start\nreturn\n").unwrap()).unwrap();
    let before = previous.clone();
    let bad = rvn_parser::parse("use \"unresolved.rvn\"\n").unwrap();
    assert!(reimport_script(&bad, &previous).is_err());
    assert_eq!(previous, before);
}

#[test]
fn newly_imported_branch_owners_follow_reconciled_node_identities() {
    let initial =
        "init{character.create(\"narrator\",\"\")}\nlabel start\nnarrator \"Finished.\"\n";
    let mut previous = import_script(&rvn_parser::parse(initial).unwrap()).unwrap();
    for node in previous[1].nodes.values_mut() {
        node.position = [10000.0 + node.id.get() as f64, -3000.0];
    }
    let source = r#"init{character.create("narrator","")}
label start
imagemap {
 background:"stage.png"
 hotspot{name:"door" area:(20,20,200,180)}=>{narrator "Clicked."}
}
if true {narrator "Inside."} else {narrator "Outside."}
choice {"Continue" => {narrator "Choice."}}
while false {narrator "Loop."}
for item in [] {narrator "For."}
narrator "Finished."
"#;
    let next = reimport_script(&rvn_parser::parse(source).unwrap(), &previous).unwrap();
    let generated = transpile_project(&next).unwrap();
    assert_eq!(
        generated.ast,
        transpile_project(&import_script(&rvn_parser::parse(source).unwrap()).unwrap())
            .unwrap()
            .ast
    );
    for graph in &next {
        for end in graph
            .nodes
            .values()
            .filter(|node| node.kind == NodeKind::BranchEnd)
        {
            let PropertyValue::Int(owner) = end.properties["owner"] else {
                panic!()
            };
            assert!(matches!(
                graph.nodes[&NodeId::new(owner as u64)].kind,
                NodeKind::Imagemap
                    | NodeKind::If
                    | NodeKind::Choice
                    | NodeKind::While
                    | NodeKind::ForEach
            ));
        }
    }
    let finished = previous[1]
        .nodes
        .values()
        .find(|node| node.kind == NodeKind::TextValue)
        .unwrap();
    assert_eq!(next[1].nodes[&finished.id].position, finished.position);
    assert_eq!(
        reimport_script(&rvn_parser::parse(source).unwrap(), &next).unwrap(),
        next
    );
}

#[test]
fn fresh_nodes_avoid_user_placements_without_moving_existing_nodes() {
    let source = "label start\n\"Keep\"\nreturn";
    let mut previous = import_script(&rvn_parser::parse(source).unwrap()).unwrap();
    for node in previous[1].nodes.values_mut() {
        node.position = [960.0, 0.0];
    }
    let next = reimport_script(
        &rvn_parser::parse("label start\n\"New\"\n\"Keep\"\nreturn").unwrap(),
        &previous,
    )
    .unwrap();
    for (id, node) in &previous[1].nodes {
        assert_eq!(next[1].nodes[id].position, node.position);
    }
    for (id, node) in &next[1].nodes {
        if !previous[1].nodes.contains_key(id) {
            assert!(node.position[0] != 960.0 || node.position[1] > 100.0);
        }
    }
}

#[test]
fn malformed_previous_document_is_rejected_without_panic_or_partial_changes() {
    let script = rvn_parser::parse("label start\nreturn\n").unwrap();
    let mut previous = import_script(&script).unwrap();
    previous[1].pins.clear();
    let before = previous.clone();
    assert!(reimport_script(&script, &previous).is_err());
    assert_eq!(previous, before);
}
