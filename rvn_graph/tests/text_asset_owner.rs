use rvn_graph::*;

const OWNER: &str = "b9ab9b6d1c9448ad8dcd799c23b4b550";
const OTHER: &str = "797679920f7142be99d9d6fe5553ca34";
fn owner(value: &str) -> TextOwnerId { TextOwnerId::parse(value).unwrap() }
fn graphs(source: &str) -> Vec<GraphDocument> {
    import_script(&rvn_parser::parse_program(source).unwrap().script).unwrap()
}

#[test]
fn opt_in_owner_preserves_legacy_and_scene_semantics_and_rejects_bad_storage() {
    for source in ["label start\n\"Hello\"\nreturn\n", "scene label start\n\"Hello\"\nreturn\n"] {
        let mut graph = graphs(source).into_iter().find(|g| matches!(g.kind, GraphKind::Label {..})).unwrap();
        let before = graph.clone();
        let json = serde_json::to_string(&before).unwrap();
        assert!(!json.contains("text_owner"));
        assert_eq!(GraphDocument::from_json(&json).unwrap(), before);
        assert!(graph.enable_text_owner(owner(OWNER)).unwrap());
        assert_eq!(graph.schema_version, 7);
        assert_eq!((&graph.nodes, &graph.pins, &graph.edges, graph.graph_id, graph.narrative),
            (&before.nodes, &before.pins, &before.edges, before.graph_id, before.narrative));
        assert_eq!(transpile(&graph).unwrap().ast, transpile(&before).unwrap().ast);
        let json = serde_json::to_string(&graph).unwrap();
        let (loaded, report) = GraphDocument::from_json_with_report(&json).unwrap();
        assert_eq!(loaded, graph);
        assert_eq!((report.from, report.to), (7, 7));
        assert!(report.steps.is_empty());
        assert!(!graph.enable_text_owner(owner(OWNER)).unwrap());
        let exact = graph.clone();
        assert!(graph.enable_text_owner(owner(OTHER)).is_err());
        assert_eq!(graph, exact);
        graph.set_narrative_metadata(None).unwrap();
        assert_eq!(graph.schema_version, 7);
        graph.set_narrative_metadata(Some(NarrativeMetadata::SceneEntry)).unwrap();
        assert_eq!(graph.schema_version, 7);
        for invalid in ["", "asset:1", "00000000000000000000000000000000", "B9AB9B6D1C9448AD8DCD799C23B4B550", "x9ab9b6d1c9448ad8dcd799c23b4b550"] {
            let mut value = serde_json::to_value(&graph).unwrap();
            value["text_owner"] = invalid.into();
            assert!(GraphDocument::from_json(&value.to_string()).is_err());
        }
        for version in [5, 6, GRAPH_MAX_SUPPORTED_SCHEMA_VERSION + 1] {
            let mut value = serde_json::to_value(&graph).unwrap();
            value["schema_version"] = version.into();
            assert!(GraphDocument::from_json(&value.to_string()).is_err());
        }
        let mut missing = serde_json::to_value(&graph).unwrap();
        missing.as_object_mut().unwrap().remove("text_owner");
        assert!(GraphDocument::from_json(&missing.to_string()).is_err());
    }
}

#[test]
fn source_reimport_and_rename_preserve_owner_and_conflicting_edits_are_atomic() {
    let source = "// identity is independent of the scope name\nscene label start\n\"Hello\"\nreturn\nscene label next\n\"Next\"\nreturn\n";
    let mut project = SourceProject::open(source, &[]).unwrap();
    let mut edited = project.graphs().to_vec();
    let start = edited.iter().position(|g| matches!(&g.kind, GraphKind::Label {name} if name=="start")).unwrap();
    let next = edited.iter().position(|g| matches!(&g.kind, GraphKind::Label {name} if name=="next")).unwrap();
    edited[start].enable_text_owner(owner(OWNER)).unwrap();
    edited[next].enable_text_owner(owner(OTHER)).unwrap();
    project.apply_visual(source, &edited).unwrap();
    assert_eq!(project.source(), source);
    assert_eq!(project.graphs(), edited);
    // Force structural reconciliation rather than only the equal-AST fast path.
    let changed = source.replace("\"Hello\"", "set count = 1\n\"Changed\"");
    project.refresh(changed.clone()).unwrap();
    assert_eq!(project.graphs()[start].text_owner, Some(owner(OWNER)));
    assert_eq!(project.graphs()[next].text_owner, Some(owner(OTHER)));
    assert_eq!(project.graphs()[start].schema_version, 7);
    let current = project.graphs().to_vec();
    let mut renamed = current.clone();
    let id = renamed[start].graph_id;
    renamed[start].kind = GraphKind::Label { name: "renamed".into() };
    project.apply_visual(&changed, &renamed).unwrap();
    assert_eq!(project.graphs().iter().find(|g| g.graph_id == id).unwrap().text_owner, Some(owner(OWNER)));
    let baseline = project.graphs().to_vec();
    let exact_source = project.source().to_string();
    for replacement in [None, Some(owner(OTHER))] {
        let mut bad = baseline.clone();
        bad[start].text_owner = replacement;
        if bad[start].text_owner.is_none() { bad[start].schema_version = 6; }
        assert!(project.apply_visual(&exact_source, &bad).is_err());
        assert_eq!(project.graphs(), baseline);
        assert_eq!(project.source(), exact_source);
    }
    let mut duplicated = baseline.clone();
    duplicated[next].text_owner = Some(owner(OWNER));
    assert!(transpile_project(&duplicated).is_err());
    assert!(reimport_script(&rvn_parser::parse_program(&exact_source).unwrap().script, &duplicated).is_err());
    assert_eq!(project.graphs(), baseline);
}

#[test]
fn recovery_save_cold_open_and_external_source_change_keep_owner() {
    let root = std::env::temp_dir().join(format!("rvn-text-owner-{}-{}", std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir(&root).unwrap();
    struct Fixture(std::path::PathBuf);
    impl Drop for Fixture { fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); } }
    let _fixture = Fixture(root.clone());
    let path = root.join("main.rvn");
    let source = "// keep authored trivia\nscene label start\n\"Hello\"\nreturn\n";
    std::fs::write(&path, source).unwrap();
    let mut workspace = SourceWorkspace::link(&root, &path, &[]).unwrap();
    let mut edited = workspace.project().graphs().to_vec();
    let start = edited.iter().position(|g| matches!(&g.kind, GraphKind::Label {name} if name=="start")).unwrap();
    edited[start].enable_text_owner(owner(OWNER)).unwrap();
    workspace.write_recovery(&edited).unwrap();
    drop(workspace);
    let mut recovered = SourceWorkspace::open(&root).unwrap().unwrap();
    assert_eq!(recovered.recovery().unwrap().unwrap(), edited);
    recovered.save(&edited).unwrap();
    assert!(recovered.recovery().unwrap().is_none());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
    let published = recovered.project().graphs().to_vec();
    drop(recovered);
    let cold = SourceWorkspace::open(&root).unwrap().unwrap();
    assert_eq!(cold.project().graphs(), published);
    drop(cold);
    std::fs::write(&path, source.replace("\"Hello\"", "\"External\"")).unwrap();
    let refreshed = SourceWorkspace::open(&root).unwrap().unwrap();
    assert_eq!(refreshed.project().graphs()[start].text_owner, Some(owner(OWNER)));
    assert_eq!(refreshed.project().graphs()[start].schema_version, 7);
    assert_eq!(owner(OWNER).locale_owner(), format!("asset:{OWNER}"));
}
