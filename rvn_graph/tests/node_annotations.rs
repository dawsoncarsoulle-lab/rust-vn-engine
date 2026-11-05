use rvn_graph::*;

const SOURCE: &str = "// Keep source comment\nlabel start\nset number = 1 + 2\nreturn\n";

fn annotation(text: &str, visible: bool) -> NodeAnnotation {
    NodeAnnotation { text: text.into(), visible, pinned: true }
}

#[test]
fn annotation_legacy_json_opens_without_automatic_rewrite() {
    let mut graph = import_script(&rvn_parser::parse(SOURCE).unwrap()).unwrap().remove(1);
    let id = graph.nodes.values().find(|node| node.kind == NodeKind::SetVariable).unwrap().id;
    graph.nodes.get_mut(&id).unwrap().properties.insert("comment".into(), PropertyValue::String("Legacy".into()));
    let json = graph.to_pretty_json().unwrap();
    assert!(!json.contains("\"annotation\""));
    let reopened = GraphDocument::from_json(&json).unwrap();
    assert_eq!(reopened, graph);
    assert_eq!(reopened.to_pretty_json().unwrap(), json);
    assert_eq!(reopened.nodes[&id].effective_annotation(), Some(NodeAnnotation {
        text: "Legacy".into(), visible: true, pinned: false,
    }));
}

#[test]
fn annotation_hidden_text_and_pin_roundtrip_without_affecting_runtime() {
    let mut graphs = import_script(&rvn_parser::parse(SOURCE).unwrap()).unwrap();
    let before = transpile_project(&graphs).unwrap();
    let id = graphs[1].nodes.values().find(|node| node.kind == NodeKind::SetVariable).unwrap().id;
    let node = graphs[1].nodes.get_mut(&id).unwrap();
    node.properties.insert("comment".into(), PropertyValue::String("Old".into()));
    node.annotation = Some(annotation("Hidden retained\nUnicode é", false));
    let expected = graphs[1].clone();
    let next = GraphDocument::from_json(&expected.to_pretty_json().unwrap()).unwrap();
    assert_eq!(next, expected);
    assert_eq!(next.nodes[&id].effective_annotation(), Some(annotation("Hidden retained\nUnicode é", false)));
    assert_eq!(transpile_project(&graphs).unwrap().ast, before.ast);
    assert_eq!(transpile_project(&graphs).unwrap().source, before.source);
}

#[test]
fn annotation_reimport_retains_legacy_and_typed_metadata_for_changed_connected_value() {
    let mut previous = import_script(&rvn_parser::parse(SOURCE).unwrap()).unwrap();
    let literal = previous[1].nodes.values().find(|node| node.properties.get("value") == Some(&PropertyValue::Int(2))).unwrap().id;
    let set = previous[1].nodes.values().find(|node| node.kind == NodeKind::SetVariable).unwrap().id;
    previous[1].nodes.get_mut(&literal).unwrap().annotation = Some(annotation("Literal", false));
    previous[1].nodes.get_mut(&set).unwrap().properties.insert("comment".into(), PropertyValue::String("Legacy set".into()));
    let before = previous.clone();
    let edited = rvn_parser::parse(&SOURCE.replace("1 + 2", "1 + 7")).unwrap();
    let next = reimport_script(&edited, &previous).unwrap();
    assert_eq!(previous, before);
    assert_eq!(next[1].nodes[&literal].properties["value"], PropertyValue::Int(7));
    assert_eq!(next[1].nodes[&literal].annotation, Some(annotation("Literal", false)));
    assert_eq!(next[1].nodes[&set].properties["comment"], PropertyValue::String("Legacy set".into()));
    assert_eq!(transpile_project(&next).unwrap().ast, transpile_project(&import_script(&edited).unwrap()).unwrap().ast);
}

#[test]
fn annotation_duplicate_semantic_nodes_match_by_existing_identity_order() {
    let source = "label start\n\"Same\"\n\"Same\"\nreturn\n";
    let mut previous = import_script(&rvn_parser::parse(source).unwrap()).unwrap();
    let ids: Vec<_> = previous[1].nodes.values().filter(|node| node.kind == NodeKind::TextValue).map(|node| node.id).collect();
    assert_eq!(ids.len(), 2);
    for (index, id) in ids.iter().enumerate() {
        previous[1].nodes.get_mut(id).unwrap().properties.insert("comment".into(), PropertyValue::String(format!("Comment {index}")));
    }
    let edited = rvn_parser::parse(&source.replace("return", "\"New\"\nreturn")).unwrap();
    let next = reimport_script(&edited, &previous).unwrap();
    for (index, id) in ids.iter().enumerate() {
        assert_eq!(next[1].nodes[id].properties["comment"], PropertyValue::String(format!("Comment {index}")));
    }
    let new = next[1].nodes.values().find(|node| node.properties.get("value") == Some(&PropertyValue::String("New".into()))).unwrap();
    assert!(new.effective_annotation().is_none());
}

#[test]
fn annotation_source_visual_edit_refresh_and_cold_open_preserve_text_and_visibility() {
    let mut project = SourceProject::open(SOURCE, &[]).unwrap();
    let mut edited = project.graphs().to_vec();
    let id = edited[1].nodes.values().find(|node| node.kind == NodeKind::SetVariable).unwrap().id;
    edited[1].nodes.get_mut(&id).unwrap().annotation = Some(annotation("Retained", false));
    project.apply_visual(SOURCE, &edited).unwrap();
    assert_eq!(project.source(), SOURCE);
    project.refresh(SOURCE.replace("1 + 2", "1 + 7")).unwrap();
    assert_eq!(project.graphs()[1].nodes[&id].annotation, Some(annotation("Retained", false)));
    let frozen: Vec<_> = project.graphs().iter().map(|graph| GraphDocument::from_json(&graph.to_pretty_json().unwrap()).unwrap()).collect();
    let reopened = SourceProject::open(project.source(), &frozen).unwrap();
    assert_eq!(reopened.source(), project.source());
    assert_eq!(reopened.graphs(), project.graphs());
    let before = project.graphs().to_vec();
    let source_before = project.source().to_owned();
    assert!(project.refresh("label start\nset number =").is_err());
    assert_eq!(project.graphs(), before);
    assert_eq!(project.source(), source_before);
}

#[test]
fn annotation_shared_signed_initializer_copy_preserves_foreign_metadata_and_runtime() {
    let source = "scene label start\nlocal first=-12\nlocal second=5\n\"[first]/[second]\"\nreturn\n";
    let mut graph = import_script(&rvn_parser::parse_program(source).unwrap().script).unwrap().into_iter()
        .find(|graph| matches!(&graph.kind,GraphKind::Label{name} if name=="start")).unwrap();
    let first = graph.local_variable_initializers("first").unwrap().remove(0);
    let second = graph.local_variable_initializers("second").unwrap().remove(0);
    let first_input = graph.pin_by_key(first.node,"value").unwrap().id;
    let second_input = graph.pin_by_key(second.node,"value").unwrap().id;
    let output = graph.edges.values().find(|edge|edge.input==first_input).unwrap().output;
    graph.edges.retain(|_,edge|edge.input!=second_input);
    graph.connect(output,second_input).unwrap();
    for node in graph.nodes.values_mut() { node.annotation = Some(annotation("Shared metadata",false)); }
    let expected = graph.local_variable_initializers("first").unwrap().remove(0);
    let before = graph.clone();
    assert!(graph.replace_local_initializer("first",&expected,PropertyValue::Int(3)).unwrap());
    for (id,node) in &before.nodes { assert_eq!(&graph.nodes[id],node); }
    let copied: Vec<_> = graph.nodes.values().filter(|node|!before.nodes.contains_key(&node.id)).collect();
    assert!(!copied.is_empty());
    for node in copied { assert_eq!(node.annotation,Some(annotation("Shared metadata",false))); }
    assert_eq!(graph.local_variable_initializers("second").unwrap()[0].literal_value,Some(PropertyValue::Int(-12)));
    let mut engine = rvn_core::Engine::new(transpile(&graph).unwrap().ast,rvn_core::TerminalRenderer,20).unwrap();
    assert!(matches!(engine.step_until_interaction().unwrap(),Some(rvn_core::Interaction::Dialogue{text,..}) if text=="3/-12"));
}
