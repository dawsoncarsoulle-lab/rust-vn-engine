use rvn_graph::*;
const SOURCE: &str = r#"
// A source-linked custom component, not an editor-only picture.
function sketch(state, props, frame) {
    // Rounded card and independently transformed artwork.
    return [canvas_rect([0,0,320,180],[0.08,0.14,0.19,1],12),
        canvas_ellipse([20,20,40,40],[0.2,0.7,0.9,1]),
        canvas_line([[0,0],[100,100]],[1,1,1,1],2),
        canvas_polygon([[20,0],[40,40],[0,40]],[0.8,0.4,0.2,1]),
        canvas_text("Puzzle",[24,80],[1,1,1,1],24),
        canvas_image("card.png",[240,20,40,60]),
        canvas_group([50,50,1,1,15,0.8],[0,0,100,100],[canvas_rect([0,0,60,60],[0,1,0,1],4)]),
        canvas_hit("card",[0,0,320,180])]
}
handler select_card(event) {
    ui.set_state(event["screen"], event["element"], dict_set(event["state"], "selected", true))
}
screen puzzle(){return component("board","canvas",{"draw":"sketch","state":{"selected":false},"props":{"name":"Demo"},"events":{"pointer_down":"select_card"}},[])}
label start
ui.open("puzzle",[],true,5)
"Test"
return
"#;
#[test]
fn every_primitive_and_state_command_has_a_real_node_and_roundtrips() {
    let graphs = import_script(&rvn_parser::parse(SOURCE).unwrap()).unwrap();
    for kind in [
        NodeKind::CanvasRect,
        NodeKind::CanvasEllipse,
        NodeKind::CanvasLine,
        NodeKind::CanvasPolygon,
        NodeKind::CanvasText,
        NodeKind::CanvasImage,
        NodeKind::CanvasGroup,
        NodeKind::CanvasHit,
        NodeKind::UiSetState,
    ] {
        assert!(
            graphs
                .iter()
                .any(|graph| graph.nodes.values().any(|node| node.kind == kind)),
            "Missing {kind:?}"
        );
    }
    let compiled = transpile_project(&graphs).unwrap();
    let second = import_script(&rvn_parser::parse(&compiled.source).unwrap()).unwrap();
    assert_eq!(transpile_project(&second).unwrap().ast, compiled.ast);
    for graph in graphs {
        assert_eq!(
            GraphDocument::from_json(&graph.to_pretty_json().unwrap()).unwrap(),
            graph
        );
    }
}
#[test]
fn repeated_visual_edits_keep_comments_ids_placements_and_other_scopes() {
    let mut project = SourceProject::open(SOURCE, &[]).unwrap();
    for radius in [16.0, 20.0, 12.0] {
        let mut graphs = project.graphs().to_vec();
        let graph = graphs
            .iter_mut()
            .find(|graph| matches!(&graph.kind,GraphKind::Function{name}if name=="sketch"))
            .unwrap();
        let rect = graph
            .nodes
            .values()
            .find(|node| node.kind == NodeKind::CanvasRect)
            .unwrap()
            .id;
        graph.nodes.get_mut(&rect).unwrap().position = [423.5, -61.0];
        graph
            .set_literal_input(rect, "radius", PropertyValue::Float(radius))
            .unwrap();
        let source = project.source().to_owned();
        project.apply_visual(&source, &graphs).unwrap();
        assert!(project
            .source()
            .contains("// Rounded card and independently transformed artwork."));
        let source = project.source().to_owned();
        project.refresh(&source).unwrap();
        let graph = project
            .graphs()
            .iter()
            .find(|graph| matches!(&graph.kind,GraphKind::Function{name}if name=="sketch"))
            .unwrap();
        assert_eq!(graph.nodes[&rect].position, [423.5, -61.0]);
        assert_eq!(
            graph.literal_input_value(rect, "radius").unwrap(),
            Some(PropertyValue::Float(radius))
        );
        assert_eq!(
            project
                .source()
                .matches("// A source-linked custom component")
                .count(),
            1
        );
        assert!(project.source().contains("handler select_card(event)"));
    }
}
#[test]
fn v4_migration_adds_no_nodes_and_keeps_logic_exact() {
    let graph =
        import_script(&rvn_parser::parse("label start\n\"Existing project\"\nreturn").unwrap())
            .unwrap()
            .pop()
            .unwrap();
    let mut raw = serde_json::to_value(&graph).unwrap();
    raw["schema_version"] = serde_json::json!(4);
    let (migrated, report) = GraphDocument::from_json_with_report(&raw.to_string()).unwrap();
    assert_eq!(report.steps, ["v4_to_v5_programmable_canvas"]);
    assert_eq!(migrated.nodes, graph.nodes);
    assert_eq!(migrated.pins, graph.pins);
    assert_eq!(migrated.edges, graph.edges);
    assert_eq!(
        transpile(&migrated).unwrap().source,
        transpile(&graph).unwrap().source
    );
}
#[test]
fn designer_can_navigate_local_state_without_flattening_dynamic_parameters() {
    let mut graph = import_script(&rvn_parser::parse(SOURCE).unwrap())
        .unwrap()
        .into_iter()
        .find(|graph| matches!(graph.kind, GraphKind::Screen { .. }))
        .unwrap();
    let component = graph
        .nodes
        .values()
        .find(|node| node.kind == NodeKind::UiComponent)
        .unwrap()
        .id;
    let state = graph
        .dictionary_property_source(component, "properties", &["state"])
        .unwrap()
        .unwrap();
    assert_eq!(graph.nodes[&state].kind, NodeKind::FunctionCall);
    let original = graph.clone();
    assert_eq!(
        graph
            .ensure_dictionary_property(component, "properties", &["state"])
            .unwrap(),
        state
    );
    assert_eq!(graph, original);
    let data = graph
        .ensure_dictionary_property(component, "properties", &["event_data"])
        .unwrap();
    assert!(graph.nodes.contains_key(&data));
    assert!(transpile(&graph).unwrap().source.contains("event_data"));
}
