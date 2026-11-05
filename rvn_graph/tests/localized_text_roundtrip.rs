use rvn_graph::*;
use rvn_core::{Engine, Interaction, TerminalRenderer};

const SOURCE: &str = r#"// Authored text identity must survive visual compilation.
init {
set pattern = __rvn_localizable_text("Hello {Name}! Count {Count}", "asset:1", "VN", "welcome", "Native typed text fixture")
set person = __rvn_localizable_text("Visitor", "asset:2", "Names", "visitor")
set message = __rvn_format_text(pattern, {"Name": person, "Count": 3})
set invariant = __rvn_invariant_text("Exact")
}
scene label start
"[message]"
"Invariant: [invariant]"
"#;

fn dialogue(engine: &mut Engine<TerminalRenderer>) -> String {
    match engine.step_until_interaction().unwrap().unwrap() {
        Interaction::Dialogue { text, .. } => text,
        other => panic!("Unexpected interaction {other:?}"),
    }
}

#[test]
fn typed_text_source_graph_codegen_reimport_preserves_structure_and_runtime() {
    let script = rvn_parser::parse(SOURCE).unwrap();
    let graphs = import_script(&script).unwrap();
    assert!(graphs.iter().all(|g| g.schema_version == if g.narrative.is_some() { 6 } else { 5 }));
    let generated = transpile_project(&graphs).unwrap();
    assert_eq!(&generated.ast[..script.len()], script.as_slice());
    let mut game = Engine::new(generated.ast, TerminalRenderer, 8).unwrap();
    assert_eq!(dialogue(&mut game), "Hello Visitor! Count 3");
    let saved = rvn_core::save::SaveData::try_from_state(&game.state, 1, "Text".into(), "main.rvn".into()).unwrap();
    assert_eq!(saved.format_version, 12);
    assert!(matches!(game.state.vars["message"], rvn_parser::Value::Text(_)));
    game.advance_dialogue().unwrap();
    assert_eq!(dialogue(&mut game), "Invariant: Exact");
    let cold: Vec<_> = graphs.iter().map(|g| GraphDocument::from_json(&serde_json::to_string(g).unwrap()).unwrap()).collect();
    assert_eq!(cold, graphs);
    let reimported = reimport_script(&script, &cold).unwrap();
    assert_eq!(reimported, cold);
    assert_eq!(&transpile_project(&reimported).unwrap().ast[..script.len()], script.as_slice());
}

#[test]
fn typed_text_visual_edit_and_cold_workspace_keep_metadata_and_authored_trivia() {
    let root = std::env::temp_dir().join(format!("rvn-graph-text-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir(&root).unwrap();
    struct Fixture(std::path::PathBuf);
    impl Drop for Fixture { fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); } }
    let _fixture = Fixture(root.clone());
    let source_path = root.join("main.rvn");
    std::fs::write(&source_path, SOURCE).unwrap();
    let mut workspace = SourceWorkspace::link(&root, &source_path, &[]).unwrap();
    let before = workspace.project().graphs().to_vec();
    let mut edited = before.clone();
    let owner = edited.iter_mut().find(|g| matches!(g.kind, GraphKind::Init)).unwrap();
    let source_node = owner.nodes.values().find(|n| n.properties.get("function") == Some(&PropertyValue::String("__rvn_localizable_text".into()))
        && owner.pin_by_key(n.id, "item_0").unwrap().default_value == Some(PropertyValue::String("Visitor".into()))).unwrap().id;
    owner.set_pin_default(source_node, "item_0", PropertyValue::String("Guest".into())).unwrap();
    workspace.write_recovery(&edited).unwrap();
    assert_eq!(workspace.recovery().unwrap().unwrap(), edited);
    workspace.save(&edited).unwrap();
    let published = workspace.project().graphs().to_vec();
    assert!(workspace.recovery().unwrap().is_none());
    let saved = std::fs::read_to_string(&source_path).unwrap();
    assert!(saved.starts_with("// Authored text identity must survive visual compilation."));
    assert!(saved.contains("Native typed text fixture"));
    assert!(saved.contains("\"asset:2\", \"Names\", \"visitor\""));
    drop(workspace);
    let cold = SourceWorkspace::open(&root).unwrap().unwrap();
    assert_eq!(cold.project().graphs(), published);
    assert_eq!(cold.project().source(), saved);
    let mut game = Engine::from_program(cold.project().resolved_program().unwrap(), TerminalRenderer, 8).unwrap();
    assert_eq!(dialogue(&mut game), "Hello Guest! Count 3");
    let rvn_parser::Value::Text(text) = &game.state.vars["person"] else { panic!("Lost Text type") };
    let rvn_parser::localized_text::DeferredText::Literal { text } = text.as_ref() else { panic!("Lost literal") };
    assert_eq!(text.localization.identity().unwrap().key, "visitor");
}
