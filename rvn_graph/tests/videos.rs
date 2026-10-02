use rvn_graph::*;
#[test]
fn link_isolated_video_authoring_fixture_when_requested() {
    if let Some(root) = std::env::var_os("RVN_VIDEO_EDITOR_QA_DIR") {
        let root = std::path::PathBuf::from(root);
        assert!(root.starts_with(std::env::temp_dir()) && root.join("rvn.toml").is_file());
        SourceWorkspace::link(&root, &root.join("main.rvn"), &[]).unwrap();
    }
}
#[test]
fn all_video_operations_are_typed_and_survive_source_roundtrips() {
    let source="// Keep the original video note\nlabel start\nvideo.play(\"intro\",video_clip(\"movies/clip.webm\",{\"volume\":0.5}))\nvideo.pause(\"intro\")\nvideo.seek(\"intro\",0.5)\nvideo.volume(\"intro\",0.4)\nvideo.resume(\"intro\")\nvideo.wait(\"intro\")\nvideo.skip(\"intro\")\nvideo.stop(\"intro\")\n\"Finished\"\n";
    let mut project = SourceProject::open(source, &[]).unwrap();
    let mut graphs = project.graphs().to_vec();
    for kind in [
        NodeKind::VideoPlay,
        NodeKind::VideoPause,
        NodeKind::VideoSeek,
        NodeKind::VideoVolume,
        NodeKind::VideoResume,
        NodeKind::VideoWait,
        NodeKind::VideoSkip,
        NodeKind::VideoStop,
        NodeKind::VideoClip,
    ] {
        assert!(graphs
            .iter()
            .any(|graph| graph.nodes.values().any(|node| node.kind == kind)));
    }
    let original = rvn_parser::parse(source).unwrap();
    for _ in 0..3 {
        let generated = transpile_project(&graphs).unwrap();
        let parsed = rvn_parser::parse(&generated.source).unwrap();
        assert!(parsed
            .windows(original.len())
            .any(|window| window == original));
        graphs = reimport_script(&original, &graphs).unwrap();
    }
    let graph = graphs
        .iter_mut()
        .find(|graph| {
            graph
                .nodes
                .values()
                .any(|node| node.kind == NodeKind::VideoClip)
        })
        .unwrap();
    let node = graph
        .nodes
        .values()
        .find(|node| node.kind == NodeKind::VideoClip)
        .unwrap()
        .id;
    graph
        .set_dictionary_property(node, "properties", &["volume"], PropertyValue::Float(0.75))
        .unwrap();
    project.apply_visual(source, &graphs).unwrap();
    assert_eq!(
        project.source(),
        source.replacen("\"volume\":0.5", "\"volume\":0.75", 1)
    );
    assert_eq!(
        node_definition(NodeKind::VideoPlay)
            .pins
            .iter()
            .find(|pin| pin.key == "definition")
            .unwrap()
            .value_type,
        ValueType::VideoClip
    );
    assert!(!ValueType::VideoClip.accepts(&ValueType::Motion));
}
#[test]
fn subtitle_authoring_preserves_other_cues_shared_consumers_and_source_comments() {
    let source="// Caption timing is authored here\nfunction clip(){return video_clip(\"clip.webm\",{\"volume\":0.5,\"subtitles\":[{\"start\":0,\"end\":1,\"text\":\"first\"},{\"start\":1,\"end\":2,\"text\":\"second\"}]})}\nlabel start\nvideo.play(\"intro\",clip())\n";
    let mut project = SourceProject::open(source, &[]).unwrap();
    let mut graphs = project.graphs().to_vec();
    let graph = graphs
        .iter_mut()
        .find(|graph| matches!(graph.kind, GraphKind::Function { .. }))
        .unwrap();
    let node = graph
        .nodes
        .values()
        .find(|node| node.kind == NodeKind::VideoClip)
        .unwrap()
        .id;
    graph
        .set_dictionary_list_item_property(
            node,
            "properties",
            "subtitles",
            1,
            "text",
            PropertyValue::String("revised".into()),
        )
        .unwrap();
    project.apply_visual(source, &graphs).unwrap();
    assert_eq!(
        project.source(),
        source.replace("\"second\"", "\"revised\"")
    );
    let graph = graphs
        .iter_mut()
        .find(|graph| matches!(graph.kind, GraphKind::Function { .. }))
        .unwrap();
    let other = graph
        .add_catalog_node(NodeKind::VideoClip, [600.0, 600.0])
        .unwrap();
    let output = graph
        .edges
        .values()
        .find(|edge| edge.input == graph.pin_by_key(node, "properties").unwrap().id)
        .unwrap()
        .output;
    graph
        .connect(output, graph.pin_by_key(other, "properties").unwrap().id)
        .unwrap();
    let original = graph
        .dictionary_list_item_properties(
            other,
            "properties",
            "subtitles",
            &["start", "end", "text"],
        )
        .unwrap();
    graph
        .set_dictionary_list_item_property(
            node,
            "properties",
            "subtitles",
            0,
            "text",
            PropertyValue::String("changed first".into()),
        )
        .unwrap();
    assert_eq!(
        graph
            .dictionary_list_item_properties(
                other,
                "properties",
                "subtitles",
                &["start", "end", "text"]
            )
            .unwrap(),
        original
    );
    graph
        .append_dictionary_list_item(
            node,
            "properties",
            "subtitles",
            &[
                ("start", PropertyValue::Int(2)),
                ("end", PropertyValue::Int(3)),
                ("text", PropertyValue::String("third".into())),
            ],
        )
        .unwrap();
    assert_eq!(
        graph
            .dictionary_list_item_properties(node, "properties", "subtitles", &["text"])
            .unwrap()
            .len(),
        3
    );
    graph
        .remove_dictionary_list_item(node, "properties", "subtitles", 1)
        .unwrap();
    assert_eq!(
        graph
            .dictionary_list_item_properties(
                other,
                "properties",
                "subtitles",
                &["start", "end", "text"]
            )
            .unwrap(),
        original
    );
    assert_eq!(
        graph
            .dictionary_list_item_properties(node, "properties", "subtitles", &["text"])
            .unwrap(),
        vec![
            vec![Some(PropertyValue::String("changed first".into()))],
            vec![Some(PropertyValue::String("third".into()))]
        ]
    );
    let before = graph.clone();
    assert!(graph
        .set_dictionary_list_item_property(
            node,
            "properties",
            "subtitles",
            9,
            "text",
            PropertyValue::String("bad".into())
        )
        .is_err());
    assert_eq!(graph, &before);
}
