use rvn_graph::*;

#[test]
#[ignore = "Generates an isolated fixture for the real native editor QA"]
fn native_editor_accessibility_fixture() {
    let root = std::path::PathBuf::from(
        std::env::var_os("RVN_ACCESSIBILITY_EDITOR_QA")
            .expect("Choose an existing, empty test directory"),
    );
    assert!(root.is_dir() && std::fs::read_dir(&root).unwrap().next().is_none());
    let example =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples/accessibility");
    for directory in ["assets", "locales", "graphs"] {
        std::fs::create_dir(root.join(directory)).unwrap();
    }
    for file in [
        "main.rvn",
        "rvn.toml",
        "theme.toml",
        "assets/config.toml",
        "locales/en.toml",
        "locales/fr.toml",
    ] {
        std::fs::copy(example.join(file), root.join(file)).unwrap();
    }
    let workspace = SourceWorkspace::link(&root, &root.join("main.rvn"), &[]).unwrap();
    for graph in workspace.project().graphs() {
        let name = match &graph.kind {
            GraphKind::Label { name }
            | GraphKind::Function { name }
            | GraphKind::Screen { name }
            | GraphKind::Handler { name } => name.clone(),
            GraphKind::Init => "init".into(),
            other => panic!("{other:?}"),
        };
        std::fs::write(
            root.join("graphs").join(format!("{name}.rvngraph")),
            graph.to_pretty_json().unwrap(),
        )
        .unwrap();
    }
}

#[test]
fn bilingual_example_is_editable_in_both_modes_without_erasing_source() {
    let source = include_str!("../../examples/accessibility/main.rvn");
    let mut project = SourceProject::open(source, &[]).unwrap();
    let mut presentation = project.graphs().to_vec();
    assert!(presentation
        .iter()
        .any(|graph| matches!(graph.kind, GraphKind::Screen { .. })));
    assert!(presentation
        .iter()
        .any(|graph| matches!(graph.kind, GraphKind::Handler { .. })));
    for graph in &mut presentation {
        for node in graph.nodes.values_mut() {
            node.position = [520.0, 130.0];
        }
    }
    project.apply_visual(source, &presentation).unwrap();
    assert_eq!(project.source(), source);
    project.refresh(source).unwrap();
    assert_eq!(project.graphs(), presentation);
    for source in [
        include_str!("../../examples/accessibility/locales/en.toml"),
        include_str!("../../examples/accessibility/locales/fr.toml"),
    ] {
        let locale = rvn_core::locale::LocaleTable::parse("test", source).unwrap();
        for key in [
            "a11y.welcome",
            "a11y.title",
            "a11y.letter",
            "a11y.controls",
            "a11y.done [selected]",
        ] {
            assert!(locale.strings.contains_key(key), "Missing {key}");
        }
    }
}

#[test]
fn accessibility_roundtrips_preserve_comments_positions_and_identifiers() {
    let source="// Player comfort, not a colour-only cue\nlabel start\naccessibility.configure({\"text_scale\":1.5,\"reduced_motion\":true})\naccessibility.speak(\"Welcome\")\naccessibility.stop()\n\"Continue\"\n";
    let mut project = SourceProject::open(source, &[]).unwrap();
    let mut graphs = project.graphs().to_vec();
    let original = rvn_parser::parse(source).unwrap();
    for kind in [
        NodeKind::AccessibilityConfigure,
        NodeKind::AccessibilitySpeak,
        NodeKind::AccessibilityStop,
    ] {
        assert!(graphs
            .iter()
            .any(|graph| graph.nodes.values().any(|node| node.kind == kind)));
        assert!(node_definition(kind)
            .pins
            .iter()
            .any(|pin| pin.key == "exec_out"));
    }
    for _ in 0..3 {
        let generated = transpile_project(&graphs).unwrap();
        assert!(rvn_parser::parse(&generated.source)
            .unwrap()
            .windows(original.len())
            .any(|window| window == original));
        let refreshed = reimport_script(&original, &graphs).unwrap();
        for (before, after) in graphs.iter().zip(&refreshed) {
            for (id, node) in &before.nodes {
                assert_eq!(after.nodes[id].position, node.position);
            }
        }
        graphs = refreshed;
    }
    let graph = graphs
        .iter_mut()
        .find(|graph| {
            graph
                .nodes
                .values()
                .any(|node| node.kind == NodeKind::AccessibilityConfigure)
        })
        .unwrap();
    let node = graph
        .nodes
        .values()
        .find(|node| node.kind == NodeKind::AccessibilityConfigure)
        .unwrap()
        .id;
    graph
        .set_dictionary_property(
            node,
            "settings",
            &["text_scale"],
            PropertyValue::Float(1.75),
        )
        .unwrap();
    project.apply_visual(source, &graphs).unwrap();
    assert_eq!(project.source(), source.replace("1.5", "1.75"));
}
