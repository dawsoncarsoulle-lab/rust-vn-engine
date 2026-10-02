use rvn_graph::*;
#[test]
fn advanced_compositions_discovery_and_visual_presets_roundtrip_with_stable_layout() {
    let source = r#"// Advanced layered source stays readable.
function select_face(attributes){return {}}
function portrait(){
    // Preserve discovered names and the rest of this definition.
    return layered_image([600,1000],{"face":"neutral"},image_layers("iris",["iris__body.png","iris__face__neutral.png","iris__face__happy.png"]),{"variants":{"evening":{"face":"happy"}},"selector":"select_face"})
}
label start
character.compose("iris",portrait())
"Ready"
"#;
    let mut project = SourceProject::open(source, &[]).unwrap();
    let original = project.graphs().to_vec();
    let mut graphs = original.clone();
    let graph = graphs
        .iter_mut()
        .find(|graph| {
            graph
                .nodes
                .values()
                .any(|node| node.kind == NodeKind::LayeredImage)
        })
        .unwrap();
    let node = graph
        .nodes
        .values()
        .find(|node| node.kind == NodeKind::LayeredImage)
        .unwrap()
        .id;
    assert!(graph
        .nodes
        .values()
        .any(|node| node.kind == NodeKind::ImageLayers));
    graph
        .set_dictionary_property(
            node,
            "options",
            &["variants", "evening", "face"],
            PropertyValue::String("neutral".into()),
        )
        .unwrap();
    project.apply_visual(source, &graphs).unwrap();
    assert_eq!(
        project.source(),
        source.replacen(
            "\"evening\":{\"face\":\"happy\"}",
            "\"evening\":{\"face\":\"neutral\"}",
            1
        )
    );
    for _ in 0..3 {
        let generated = transpile_project(project.graphs()).unwrap();
        let reopened = SourceProject::open(&generated.source, project.graphs()).unwrap();
        assert!(reopened
            .graphs()
            .iter()
            .flat_map(|graph| graph.nodes.values())
            .any(|node| node.kind == NodeKind::ImageLayers));
    }
    for (old, new) in original.iter().zip(project.graphs()) {
        for (id, node) in &old.nodes {
            assert_eq!(new.nodes[id].position, node.position);
        }
    }
    let graph = graphs
        .iter_mut()
        .find(|graph| {
            graph
                .nodes
                .get(&node)
                .is_some_and(|node| node.kind == NodeKind::LayeredImage)
        })
        .unwrap();
    graph
        .remove_dictionary_entry(node, "options", &["variants", "evening"])
        .unwrap();
    assert!(transpile_value_output(graph, node, "value")
        .unwrap()
        .contains("select_face"));
    assert!(!transpile_value_output(graph, node, "value")
        .unwrap()
        .contains("evening"));
}
#[test]
fn layered_example_is_a_complete_source_authoring_workspace() {
    let source = include_str!("../../examples/layered-characters/main.rvn");
    let project = SourceProject::open(source, &[]).unwrap();
    let generated = transpile_project(project.graphs()).unwrap();
    assert!(rvn_parser::parse(&generated.source).is_ok());
    if let Some(path) = std::env::var_os("RVN_EDITOR_LAYER_FIXTURE") {
        let root = std::path::PathBuf::from(path);
        assert!(root.is_dir());
        std::fs::write(root.join("main.rvn"), source).unwrap();
        SourceWorkspace::link(&root, &root.join("main.rvn"), &[]).unwrap();
    }
}
#[test]
fn layered_characters_are_typed_and_roundtrip_without_touching_comments_or_layout() {
    let source = r#"// One composition, independent outfits and expressions.
function portrait() {
    // These authored comments must survive edits.
    return layered_image([600,1000],{"outfit":"coat"},[
        image_layer("body","body.png",{}),
        image_layer("coat","coat.png",{"group":"outfit","attribute":"coat","opacity":0.5})
    ])
}
label start
character.compose("iris",portrait())
iris.show()
character.attributes("iris",{"outfit":"coat"})
motion.play("layer:iris/coat",motion_tween(1,{}, {"opacity":0.5},"linear"))
"Ready"
"#;
    let mut project = SourceProject::open(source, &[]).unwrap();
    let mut graphs = project.graphs().to_vec();
    for kind in [
        NodeKind::LayeredImage,
        NodeKind::ImageLayer,
        NodeKind::CharacterCompose,
        NodeKind::CharacterAttributes,
    ] {
        assert!(graphs
            .iter()
            .any(|graph| graph.nodes.values().any(|node| node.kind == kind)));
    }
    for _ in 0..3 {
        let generated = transpile_project(&graphs).unwrap();
        graphs = reimport_script(&rvn_parser::parse(&generated.source).unwrap(), &graphs).unwrap();
    }
    graphs = project.graphs().to_vec();
    let graph = graphs
        .iter_mut()
        .find(|graph| matches!(graph.kind, GraphKind::Function { .. }))
        .unwrap();
    let layer = graph
        .nodes
        .values()
        .find(|node| {
            node.kind == NodeKind::ImageLayer
                && graph.literal_input_value(node.id, "id").unwrap()
                    == Some(PropertyValue::String("coat".into()))
        })
        .unwrap()
        .id;
    graph
        .set_dictionary_property(
            layer,
            "properties",
            &["opacity"],
            PropertyValue::Float(0.75),
        )
        .unwrap();
    project.apply_visual(source, &graphs).unwrap();
    assert_eq!(
        project.source(),
        source.replacen("\"opacity\":0.5", "\"opacity\":0.75", 1)
    );
    let pins = node_definition(NodeKind::CharacterCompose).pins;
    assert_eq!(
        pins.iter()
            .find(|pin| pin.key == "definition")
            .unwrap()
            .value_type,
        ValueType::Composition
    );
    assert!(!ValueType::Composition.accepts(&ValueType::Motion));
}
#[test]
fn composition_reordering_keeps_comments_and_is_reversible_in_source() {
    let source = "// Keep file header\nfunction portrait() {\n    // Keep composition note\n    return layered_image([600,1000], {}, [\n        // Keep body note\n        image_layer(\"body\", \"body.png\", {}),\n        // Keep coat note\n        image_layer(\"coat\", \"coat.png\", {})\n    ])\n}\n";
    let mut project = SourceProject::open(source, &[]).unwrap();
    let original = project.graphs().to_vec();
    let mut graphs = original.clone();
    let graph = graphs
        .iter_mut()
        .find(|graph| matches!(graph.kind, GraphKind::Function { .. }))
        .unwrap();
    let node = graph
        .nodes
        .values()
        .find(|node| node.kind == NodeKind::LayeredImage)
        .unwrap()
        .id;
    graph.move_list_input(node, "layers", 0, 1).unwrap();
    project.apply_visual(source, &graphs).unwrap();
    for note in [
        "Keep file header",
        "Keep composition note",
        "Keep body note",
        "Keep coat note",
    ] {
        assert!(project.source().contains(note), "Lost comment {note}");
    }
    assert!(
        project.source().find("image_layer(\"coat\"").unwrap()
            < project.source().find("image_layer(\"body\"").unwrap()
    );
    let current = project.source().to_owned();
    project.apply_visual(&current, &original).unwrap();
    let reopened = SourceProject::open(project.source(), project.graphs()).unwrap();
    assert_eq!(
        rvn_parser::parse(reopened.source()).unwrap(),
        rvn_parser::parse(source).unwrap()
    );
    for (old, new) in original.iter().zip(reopened.graphs()) {
        for (id, node) in &old.nodes {
            assert_eq!(new.nodes[id].position, node.position);
        }
    }
}
#[test]
fn adding_advanced_options_to_legacy_composition_keeps_layer_comments_and_presentation() {
    let source="// Keep file header\nfunction portrait() {\n    // Keep composition note\n    return layered_image([600,1000], {\"outfit\":\"coat\"}, [\n        // Keep body note\n        image_layer(\"body\", \"body.png\", {}),\n        // Keep coat note\n        image_layer(\"coat\", \"coat.png\", {\"group\":\"outfit\",\"attribute\":\"coat\"})\n    ])\n}\n";
    let mut project = SourceProject::open(source, &[]).unwrap();
    let mut graphs = project.graphs().to_vec();
    let graph = graphs
        .iter_mut()
        .find(|graph| matches!(graph.kind, GraphKind::Function { .. }))
        .unwrap();
    let node = graph
        .nodes
        .values()
        .find(|node| node.kind == NodeKind::LayeredImage)
        .unwrap()
        .id;
    graph
        .set_dictionary_property(
            node,
            "options",
            &["variants", "evening", "outfit"],
            PropertyValue::String("coat".into()),
        )
        .unwrap();
    project.apply_visual(source, &graphs).unwrap();
    for note in [
        "Keep file header",
        "Keep composition note",
        "Keep body note",
        "Keep coat note",
    ] {
        assert!(project.source().contains(note), "Lost {note}");
    }
    let reopened = SourceProject::open(project.source(), project.graphs()).unwrap();
    assert_eq!(reopened.graphs(), project.graphs());
}
