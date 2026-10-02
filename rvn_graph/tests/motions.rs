use rvn_graph::*;
#[test]
fn animation_example_is_a_complete_source_authoring_workspace() {
    let source = include_str!("../../examples/animations/main.rvn");
    let project = SourceProject::open(source, &[]).unwrap();
    let generated = transpile_project(project.graphs()).unwrap();
    assert!(rvn_parser::parse(&generated.source).is_ok());
    if let Some(path) = std::env::var_os("RVN_EDITOR_MOTION_FIXTURE") {
        let root = std::path::PathBuf::from(path);
        assert!(root.is_dir());
        std::fs::write(root.join("main.rvn"), source).unwrap();
        SourceWorkspace::link(&root, &root.join("main.rvn"), &[]).unwrap();
    }
}
#[test]
fn composable_animations_are_typed_blueprints_and_preserve_comments_and_layout() {
    let source = r#"// Authored animation factory.
function rise(seconds) {
    // Keep this explanation.
    return motion_sequence([motion_tween(seconds, {}, {"y":-100}, "ease_out"),motion_pause(0.5),motion_parallel([motion_frames(["one.png","two.png"],12),motion_repeat(2,motion_tween(0.5,{}, {"rotation":360},"linear"))])])
}
label start
scene "test.png"
motion.play("background",rise(1))
motion.wait("background")
motion.stop("background")
"Done"
"#;
    let mut project = SourceProject::open(source, &[]).unwrap();
    let mut graphs = project.graphs().to_vec();
    for kind in [
        NodeKind::MotionTween,
        NodeKind::MotionPause,
        NodeKind::MotionSequence,
        NodeKind::MotionParallel,
        NodeKind::MotionRepeat,
        NodeKind::MotionFrames,
        NodeKind::MotionPlay,
        NodeKind::MotionStop,
        NodeKind::MotionWait,
    ] {
        assert!(
            graphs
                .iter()
                .any(|graph| graph.nodes.values().any(|node| node.kind == kind)),
            "{kind:?}"
        );
    }
    for _ in 0..3 {
        let result = transpile_project(&graphs).unwrap();
        graphs = reimport_script(&rvn_parser::parse(&result.source).unwrap(), &graphs).unwrap();
    }
    // Distribution code has a synthetic end label. Source authoring retains
    // its own graphs, not those distribution-only scopes.
    graphs = project.graphs().to_vec();
    let graph = graphs
        .iter_mut()
        .find(|graph| matches!(graph.kind, GraphKind::Function { .. }))
        .unwrap();
    let pause = graph
        .nodes
        .values()
        .find(|node| node.kind == NodeKind::MotionPause)
        .unwrap()
        .id;
    graph
        .set_literal_input(pause, "seconds", PropertyValue::Float(0.75))
        .unwrap();
    project.apply_visual(source, &graphs).unwrap();
    assert!(project.source().contains("// Keep this explanation."));
    assert_eq!(
        project.source(),
        source.replace("motion_pause(0.5)", "motion_pause(0.75)")
    );
    let play = NodeKind::MotionPlay;
    let pins = node_definition(play).pins;
    assert_eq!(
        pins.iter()
            .find(|pin| pin.key == "definition")
            .unwrap()
            .value_type,
        ValueType::Motion
    );
    assert!(!ValueType::Motion.accepts(&ValueType::String));
    assert!(ValueType::Motion.accepts(&ValueType::Any));
}
