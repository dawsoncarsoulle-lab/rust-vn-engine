use rvn_graph::*;
use rvn_parser::parse;

#[test]
fn interfaces_and_handlers_roundtrip_in_both_modes_without_losing_source_comments_or_layout() {
    let source = r#"// Shared interface component.
function heading(text) { return {"id":"heading", "kind":"text", "text":text} }
screen inventory(title) {
    // Author's inventory note.
    local children = [heading(title)]
    return {"id":"root", "kind":"column", "children":children}
}
handler selected(event) {
    local value = event["value"]
    set chosen = value
    ui.focus("inventory", "name")
    ui.close("inventory")
}
init { set chosen = "" }
label start
    ui.open("inventory", ["Inventory"], false, 1)
    "Continue"
    return
"#;
    let mut project = SourceProject::open(source, &[]).unwrap();
    let mut graphs = project.graphs().to_vec();
    assert!(graphs
        .iter()
        .any(|graph| matches!(graph.kind, GraphKind::Screen { .. })));
    assert!(graphs
        .iter()
        .any(|graph| matches!(graph.kind, GraphKind::Handler { .. })));
    for _ in 0..3 {
        let generated = transpile_project(&graphs).unwrap();
        assert_eq!(
            generated
                .ast
                .iter()
                .filter(|s| matches!(s, rvn_parser::Statement::Screen { .. }))
                .count(),
            1
        );
        let imported = reimport_script(&parse(&generated.source).unwrap(), &graphs).unwrap();
        for graph in &graphs {
            assert!(imported
                .iter()
                .any(|other| other.kind == graph.kind && other.graph_id == graph.graph_id));
        }
        graphs = imported;
    }
    let mut graphs = project.graphs().to_vec();
    let graph = graphs
        .iter_mut()
        .find(|graph| matches!(graph.kind, GraphKind::Screen { .. }))
        .unwrap();
    let node = graph.nodes.values_mut().next().unwrap();
    node.position = [321.0, 123.0];
    project.apply_visual(source, &graphs).unwrap();
    assert_eq!(project.source(), source);
    let value = graphs
        .iter_mut()
        .find(|graph| matches!(graph.kind, GraphKind::Screen { .. }))
        .unwrap()
        .pins
        .values_mut()
        .find(|pin| pin.default_value == Some(PropertyValue::String("root".into())))
        .unwrap();
    // Descriptions remain ordinary dictionaries and reusable function calls,
    // rather than an editor-only format hidden from RVN authors.
    value.default_value = Some(PropertyValue::String("main_root".into()));
    project.apply_visual(source, &graphs).unwrap();
    assert!(project.source().contains("// Author's inventory note."));
    assert!(project.source().contains("main_root"));
}
