//! Real source/parser/Graph/writer contracts; no runtime/native/pixel credit.
use rvn_graph::*;
use rvn_parser::{NarrativeContext, Statement};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

fn empty(id: u64) -> GraphDocument {
    let mut graph = GraphDocument::new(GraphId::new(id), GraphKind::Init);
    for _ in 0..2 {
        let n = graph.add_catalog_node(NodeKind::Init, [0., 0.]).unwrap();
        graph.remove_node(n);
    }
    graph.add_catalog_node(NodeKind::Init, [13., 29.]).unwrap();
    graph.enable_native_frame_preparation().unwrap();
    graph
}
fn folder(name: &str) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let base = PathBuf::from(
        std::env::var("RVN_EMPTY_NATIVE_INIT_FIXTURES")
            .expect("fresh private fixture root required"),
    );
    let path = base.join(format!(
        "{name}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&path).unwrap();
    std::fs::canonicalize(path).unwrap()
}
fn same_identity(actual: &GraphDocument, expected: &GraphDocument) {
    assert_eq!(actual.kind, GraphKind::Init);
    assert_eq!(actual.graph_id, expected.graph_id);
    assert_eq!(actual.schema_version, expected.schema_version);
    assert_eq!(actual.inspector_evaluation, expected.inspector_evaluation);
    assert_eq!(
        actual.nodes.keys().collect::<Vec<_>>(),
        expected.nodes.keys().collect::<Vec<_>>()
    );
    assert_eq!(actual.pins, expected.pins);
    assert_eq!(actual.edges, expected.edges);
}
#[test]
fn empty0984_real_factory_export_parser_and_source_import_keep_ids_without_an_executable_child() {
    let graph = empty(74013);
    let before = graph.clone();
    let generated = prepare_native_frame_graph(&graph).unwrap();
    let mut legacy = graph.clone();
    legacy.inspector_evaluation = InspectorEvaluationContract::LegacyStateless;
    legacy.schema_version = GRAPH_SCHEMA_VERSION;
    let ordinary = transpile_with_origins(&legacy).unwrap();
    assert_eq!(generated.ast, ordinary.ast);
    assert!(matches!(generated.ast.as_slice(),[Statement::Init{body}]if body.is_empty()));
    assert!(!generated.source.contains("flow.consume_v"));
    assert_eq!(rvn_parser::parse(&generated.source).unwrap(), generated.ast);
    assert_eq!(generated.origins.len(), 1);
    assert!(generated.origins[0].children.is_empty());
    assert_eq!(generated.origins[0].graph, Some(graph.graph_id));
    assert_eq!(
        generated.origins[0].node,
        graph.nodes.keys().next().copied()
    );
    let restored = import_native_frame_source(&generated.source, generated.context).unwrap();
    same_identity(
        restored.iter().find(|g| g.kind == GraphKind::Init).unwrap(),
        &graph,
    );
    assert_eq!(graph, before);

    // Real initial linking supplies Unit IDs once. Opening the frozen source
    // validates its Empty Init without requesting another parameter allocator.
    let mixed = format!("{}function scoped(alpha,beta) -> () {{return}}\nscene label start\ncall scoped(10,11)\nreturn\n", generated.source);
    let mut allocated = 0u64;
    let owners = prepare_initial_unit_link_graphs(&rvn_parser::parse(&mixed).unwrap(), &[], || {
        allocated += 1;
        Ok(format!("{:032x}", 0x74013 + allocated))
    }).unwrap();
    assert_eq!(allocated, 2);
    let unit = owners.iter().find(|g| matches!(g.kind, GraphKind::UnitFunction { .. })).unwrap();
    let unit_id = unit.graph_id;
    let parameter_ids = unit.pins.iter().filter_map(|(id, pin)|
        pin.parameter_id.as_ref().map(|parameter| (*id, parameter.clone()))).collect::<Vec<_>>();
    assert_eq!(parameter_ids.len(), 2);
    let root = folder("owned-unit-validator");
    let file = root.join("main.rvn");
    std::fs::write(&file, &mixed).unwrap();
    let project = SourceProject::open_native_preparation_at(&file, &mixed, &owners).unwrap();
    same_identity(project.graphs().iter().find(|g| g.kind == GraphKind::Init).unwrap(), &graph);
    let actual = project.graphs().iter().find(|g| g.graph_id == unit_id).unwrap();
    assert_eq!(actual.schema_version, GRAPH_UNIT_SIGNATURE_SCHEMA_VERSION);
    assert!(actual.inspector_evaluation.is_legacy());
    assert_eq!(actual.pins.iter().filter_map(|(id, pin)|
        pin.parameter_id.as_ref().map(|parameter| (*id, parameter.clone()))).collect::<Vec<_>>(), parameter_ids);
    assert_eq!(allocated, 2);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), mixed);
    assert!(SourceProject::open_native_preparation_at(&file, &mixed, &[])
        .unwrap_err().contains("allocator"), "declaration validation does not grant Unit ownership");
}
#[test]
fn empty0984_ast_only_and_unmarked_legacy_cannot_recover_a_native_root_from_cache() {
    let graph = empty(74014);
    let generated = prepare_native_frame_graph(&graph).unwrap();
    assert!(import_native_frame_script(&generated.ast).is_err());
    let legacy = SourceProject::open("init {}\n", &[graph]).unwrap();
    assert!(legacy
        .graphs()
        .iter()
        .all(|g| g.inspector_evaluation.is_legacy()));
    assert!(legacy.graphs().iter().all(|g|
        g.schema_version <= GRAPH_NAME_TEXT_SCHEMA_VERSION
        && !g.validate().iter().any(|d| d.severity == DiagnosticSeverity::Error)),
        "an unmarked source must replace cached Native schemas with valid Legacy presentation");
    assert!(!legacy.source().contains("@rvn-native-empty-init"));
}
#[test]
fn empty0984_reserved_versions_payloads_root_kinds_ports_and_duplicates_refuse_before_projection() {
    let generated = prepare_native_frame_graph(&empty(74015)).unwrap();
    let source = &generated.source;
    let root = folder("invalid-program-declaration");
    let file = root.join("main.rvn");
    for bad in [
        source.replace(":v1 ", ":v99 "),
        source.replace(":v1 ", " "),
        source.replace("\"schema\":12", "\"schema\":1"),
        source.replace("\"scope\":", "\"unexpected\":true,\"scope\":"),
        source.replace("g74015/", "g0/"),
        source.replace(":init|", ":function_entry|"),
        source.replace(":init|", ":init!exec_links_v1|"),
        source.replacen("/n", "/n0", 1),
        source.replace("exec_out=", "foreign_role="),
        source.replace("@0", "@1"),
        source.replace(
            "    // @rvn-native-empty-init",
            "set effect=1\n    // @rvn-native-empty-init",
        ),
        format!("{source}\n{source}"),
        format!("{source}\ninit{{}}\n"),
        source.replace("init {", "function fake() {"),
    ] {
        assert!(
            import_native_frame_source(&bad, NarrativeContext::Legacy).is_err(),
            "malformed declaration admitted: {bad}"
        );
        assert!(
            SourceProject::open(&bad, &[]).is_err(),
            "editor open must not ignore a reserved malformed declaration"
        );
        std::fs::write(&file, &bad).unwrap();
        assert!(
            SourceProject::open_native_preparation_at(&file, &bad, &[]).is_err(),
            "frozen Program validation must reject the actual malformed declaration: {bad}"
        );
        assert_eq!(std::fs::read_to_string(&file).unwrap(), bad);
    }
}
#[test]
fn empty0984_string_tokens_and_ordinary_comments_never_masquerade_as_metadata() {
    let raw = "init {}\nlabel start\n\"// @rvn-native-empty-init:v99 {}\"\nreturn\n";
    let project = SourceProject::open(raw, &[]).unwrap();
    assert!(project
        .graphs()
        .iter()
        .all(|g| g.inspector_evaluation.is_legacy()));
    let inline = "init {// ordinary prose @rvn-native-empty-init:v1 {}\n}\n";
    assert!(SourceProject::open(inline, &[])
        .unwrap()
        .graphs()
        .iter()
        .all(|g| g.inspector_evaluation.is_legacy()));
    let generated = prepare_native_frame_graph(&empty(74016)).unwrap();
    let moved = generated
        .source
        .replace("init {\n", "")
        .replacen("}\n", "", 1);
    assert!(
        SourceProject::open(moved, &[]).is_err(),
        "reserved actual comment outside Init is rejected"
    );
}
#[test]
fn empty0984_mixed_native_definition_collision_and_executable_siblings_refuse() {
    let graph = empty(74017);
    let mut docs = import_script(&rvn_parser::parse("label start\nreturn\n").unwrap()).unwrap();
    let label = docs
        .iter_mut()
        .find(|g| matches!(g.kind, GraphKind::Label { .. }))
        .unwrap();
    label.graph_id = graph.graph_id;
    label.enable_native_frame_preparation().unwrap();
    let label = prepare_native_frame_graph(label).unwrap();
    let init = prepare_native_frame_graph(&graph).unwrap();
    assert!(
        import_native_frame_source(&(init.source + &label.source), NarrativeContext::Legacy)
            .is_err()
    );
    let nested = format!(
        "init {{ if true {{\n{}\n}} }}",
        prepare_native_frame_graph(&graph).unwrap().source
    );
    assert!(SourceProject::open(nested, &[]).is_err());
}
#[test]
fn empty0984_actual_workspace_freeze_debugger_save_reopen_and_stale_bytes_use_the_same_source() {
    let graph = empty(74018);
    let generated = prepare_native_frame_graph(&graph).unwrap();
    let root = folder("writer");
    let file = root.join("main.rvn");
    std::fs::write(&file, &generated.source).unwrap();
    let mut workspace =
        SourceWorkspace::link_native_preparation(&root, &file, &[graph.clone()]).unwrap();
    let program = workspace.freeze_native_program().unwrap();
    assert_eq!(program.script, generated.ast);
    assert!(native_source_sites(&program).unwrap().is_empty());
    let map = NativeDebugSourceMap::from_workspace(&workspace).unwrap();
    assert_eq!(map.origins().len(), 1);
    let origin = &map.origins()[0];
    assert_eq!(origin.kind, NodeKind::Init);
    assert_eq!(origin.role, NativeOriginRole::Declaration);
    assert_eq!(origin.graph, graph.graph_id);
    assert!(origin.expression.is_none());
    assert_eq!(origin.route, vec![0]);
    assert_eq!(origin.span, program.origins[0].span.range);
    assert_eq!(origin.source, file);
    let edits = workspace.project().graphs().to_vec();
    workspace.save(&edits).unwrap();
    let saved = std::fs::read_to_string(&file).unwrap();
    assert_eq!(rvn_parser::parse(&saved).unwrap(), generated.ast);
    drop(workspace);
    let reopened = SourceWorkspace::open(&root).unwrap().unwrap();
    same_identity(
        reopened
            .project()
            .graphs()
            .iter()
            .find(|g| g.kind == GraphKind::Init)
            .unwrap(),
        &graph,
    );
    assert_eq!(
        reopened.freeze_native_program().unwrap().sources[0].source,
        saved
    );
    std::fs::write(&file, saved.replace("g74018/", "g74019/")).unwrap();
    assert!(reopened.freeze_native_program().is_err());
}
#[test]
fn empty0984_imported_source_origins_can_share_local_ids_without_aliasing_their_roots() {
    let graph = empty(74020);
    let generated = prepare_native_frame_graph(&graph).unwrap();
    let root = folder("imports");
    let file = root.join("main.rvn");
    let imported = root.join("helper.rvn");
    std::fs::write(&imported, &generated.source).unwrap();
    std::fs::write(&file, format!("use \"helper.rvn\"\n{}", generated.source)).unwrap();
    let workspace =
        SourceWorkspace::link_native_preparation(&root, &file, &[graph.clone()]).unwrap();
    let program = workspace.freeze_native_program().unwrap();
    assert_eq!(program.sources.len(), 2);
    let map = NativeDebugSourceMap::from_program(&program).unwrap();
    assert_eq!(map.origins().len(), 2);
    assert_ne!(map.origins()[0].source, map.origins()[1].source);
    assert_ne!(
        map.origins()[0].definition_root_ordinal,
        map.origins()[1].definition_root_ordinal
    );
    assert!(map.origins().iter().all(|o| o.graph == graph.graph_id
        && o.role == NativeOriginRole::Declaration
        && o.expression.is_none()));
}
#[test]
fn empty0984_real_visual_publish_can_add_and_remove_the_last_command_without_duplicate_comments() {
    let graph = empty(74021);
    let generated = prepare_native_frame_graph(&graph).unwrap();
    let root = folder("edits");
    let file = root.join("main.rvn");
    std::fs::write(&file, &generated.source).unwrap();
    let mut workspace = SourceWorkspace::link_native_preparation(&root, &file, &[graph]).unwrap();
    let mut docs = workspace.project().graphs().to_vec();
    let init = docs.iter_mut().find(|g| g.kind == GraphKind::Init).unwrap();
    init.add_variable("seen", ValueType::Int, PropertyValue::Int(0))
        .unwrap();
    let set = init
        .add_catalog_node(NodeKind::SetVariable, [150., 150.])
        .unwrap();
    init.nodes
        .get_mut(&set)
        .unwrap()
        .properties
        .insert("name".into(), PropertyValue::String("seen".into()));
    // A variable declaration supplies its registry default, not a SET RHS.
    // Author the actual command operand before asking the writer to save it.
    init.set_pin_default(set, "value", PropertyValue::Int(1)).unwrap();
    let root = *init
        .nodes
        .keys()
        .find(|id| init.nodes[*id].kind == NodeKind::Init)
        .unwrap();
    init.connect(
        init.pin_by_key(root, "exec_out").unwrap().id,
        init.pin_by_key(set, "exec_in").unwrap().id,
    )
    .unwrap();
    workspace.save(&docs).unwrap();
    assert!(!workspace
        .project()
        .source()
        .contains("@rvn-native-empty-init"));
    let mut docs = workspace.project().graphs().to_vec();
    let init = docs.iter_mut().find(|g| g.kind == GraphKind::Init).unwrap();
    let set = *init
        .nodes
        .keys()
        .find(|id| init.nodes[*id].kind == NodeKind::SetVariable)
        .unwrap();
    init.remove_node(set);
    init.variables.remove("seen");
    workspace.save(&docs).unwrap();
    assert_eq!(
        workspace
            .project()
            .source()
            .matches("// @rvn-native-empty-init:v1 ")
            .count(),
        1
    );
    assert!(
        matches!(workspace.freeze_native_program().unwrap().script.as_slice(),[Statement::Init{body}]if body.is_empty())
    );
}
#[test]
fn empty0984_mixed_project_export_reopens_both_real_native_roots_and_only_declares_the_empty_init()
{
    let graph = empty(74022);
    let docs = import_script(&rvn_parser::parse("label start\nreturn\n").unwrap()).unwrap();
    let mut label = docs
        .into_iter()
        .find(|g| matches!(g.kind, GraphKind::Label { .. }))
        .unwrap();
    label.graph_id = GraphId::new(74023);
    label.enable_native_frame_preparation().unwrap();
    let compiled = transpile_project(&[graph.clone(), label.clone()]).unwrap();
    let parsed = rvn_parser::parse_program(&compiled.source).unwrap();
    assert_eq!(parsed.script, compiled.ast);
    let project = SourceProject::open(&compiled.source, &[]).unwrap();
    same_identity(
        project
            .graphs()
            .iter()
            .find(|g| g.kind == GraphKind::Init)
            .unwrap(),
        &graph,
    );
    assert!(project
        .graphs()
        .iter()
        .any(|g| g.graph_id == label.graph_id && !g.inspector_evaluation.is_legacy()));
    let ast_only = import_native_frame_script(&compiled.ast).unwrap();
    assert!(ast_only
        .iter()
        .find(|g| g.kind == GraphKind::Init)
        .unwrap()
        .inspector_evaluation
        .is_legacy());
    let root = folder("mixed");
    let file = root.join("main.rvn");
    std::fs::write(&file, &compiled.source).unwrap();
    let workspace =
        SourceWorkspace::link_native_preparation(&root, &file, project.graphs()).unwrap();
    let program = workspace.freeze_native_program().unwrap();
    assert!(native_source_sites(&program)
        .unwrap()
        .iter()
        .all(|site| site.graph != graph.graph_id));
    let map = NativeDebugSourceMap::from_program(&program).unwrap();
    let empty_origins = map
        .origins()
        .iter()
        .filter(|o| o.graph == graph.graph_id)
        .collect::<Vec<_>>();
    assert_eq!(empty_origins.len(), 1);
    assert_eq!(empty_origins[0].role, NativeOriginRole::Declaration);
    assert!(empty_origins[0].expression.is_none());
}
#[test]
fn empty0984_existing_character_registry_prelude_keeps_both_actual_init_origins_without_borrowing_its_effects(){
    let mut graph=empty(74024);graph.characters.insert("hero".into(),"Hero".into());let generated=prepare_native_frame_graph(&graph).unwrap();
    let mut legacy=graph.clone();legacy.inspector_evaluation=InspectorEvaluationContract::LegacyStateless;legacy.schema_version=GRAPH_SCHEMA_VERSION;let ordinary=transpile_with_origins(&legacy).unwrap();assert_eq!(generated.ast,ordinary.ast);assert_eq!(generated.ast.len(),2);assert!(matches!(&generated.ast[0],Statement::Init{body}if matches!(body.as_slice(),[Statement::CharacterCreate{..}])));assert!(matches!(&generated.ast[1],Statement::Init{body}if body.is_empty()));
    let restored=import_native_frame_source(&generated.source,generated.context).unwrap();let init=restored.iter().find(|g|g.kind==GraphKind::Init).unwrap();same_identity(init,&graph);assert_eq!(init.characters,graph.characters);
    let root=folder("characters");let file=root.join("main.rvn");std::fs::write(&file,&generated.source).unwrap();let workspace=SourceWorkspace::link_native_preparation(&root,&file,&[graph]).unwrap();let program=workspace.freeze_native_program().unwrap();assert_eq!(program.script,generated.ast);assert_eq!(program.origins.len(),2);
    let map=NativeDebugSourceMap::from_program(&program).unwrap();assert_eq!(map.origins().len(),1);let declaration=&map.origins()[0];assert_eq!(declaration.role,NativeOriginRole::Declaration);assert_eq!(declaration.definition_root_ordinal,1);assert_eq!(declaration.span,program.origins[1].span.range);assert_ne!(declaration.span,program.origins[0].span.range);assert!(native_source_sites(&program).unwrap().is_empty());
}

#[test]
fn empty0984_visual_optin_of_actual_or_source_less_legacy_empty_init_publishes_and_explicit_optout_reopens_legacy() {
    for (name, source) in [("optin-block", "init {}\n"), ("optin-derived", "label start\nreturn\n")] {
        let root = folder(name);
        let file = root.join("main.rvn");
        std::fs::write(&file, source).unwrap();
        let mut workspace = SourceWorkspace::link(&root, &file, &[]).unwrap();
        let before = rvn_parser::parse(source).unwrap();
        let mut docs = workspace.project().graphs().to_vec();
        let graph = docs.iter_mut().find(|g| g.kind == GraphKind::Init).unwrap();
        assert!(graph.inspector_evaluation.is_legacy());
        graph.enable_native_frame_preparation().unwrap();
        let expected = graph.clone();
        workspace.save(&docs).unwrap();
        let saved = std::fs::read_to_string(&file).unwrap();
        assert_eq!(saved.matches("// @rvn-native-empty-init:v1 ").count(), 1);
        let program = workspace.freeze_native_program().unwrap();
        if name == "optin-block" { assert_eq!(program.script, before); }
        else {
            assert_eq!(program.script.iter().filter(|s| matches!(s, Statement::Init { .. })).count(), 1);
            assert_eq!(program.script.iter().filter(|s| !matches!(s, Statement::Init { .. })).cloned().collect::<Vec<_>>(), before);
        }
        assert!(native_source_sites(&program).unwrap().is_empty());
        same_identity(workspace.project().graphs().iter().find(|g| g.kind == GraphKind::Init).unwrap(), &expected);
        drop(workspace);
        let mut reopened = SourceWorkspace::open(&root).unwrap().unwrap();
        same_identity(reopened.project().graphs().iter().find(|g| g.kind == GraphKind::Init).unwrap(), &expected);
        assert_eq!(NativeDebugSourceMap::from_workspace(&reopened).unwrap().origins().iter().filter(|o| o.graph == expected.graph_id).count(), 1);
        let native_ast = reopened.freeze_native_program().unwrap().script;
        let mut docs = reopened.project().graphs().to_vec();
        let graph = docs.iter_mut().find(|g| g.kind == GraphKind::Init).unwrap();
        graph.inspector_evaluation = InspectorEvaluationContract::LegacyStateless;
        graph.schema_version = GRAPH_SCHEMA_VERSION;
        reopened.save(&docs).unwrap();
        let legacy_source = std::fs::read_to_string(&file).unwrap();
        assert!(!legacy_source.contains("@rvn-native-empty-init"));
        assert_eq!(rvn_parser::parse(&legacy_source).unwrap(), native_ast);
        assert!(reopened.project().graphs().iter().find(|g| g.kind == GraphKind::Init).unwrap().inspector_evaluation.is_legacy());
        drop(reopened);
        let legacy = SourceWorkspace::open(&root).unwrap().unwrap();
        assert!(legacy.project().graphs().iter().find(|g| g.kind == GraphKind::Init).unwrap().inspector_evaluation.is_legacy());
    }
}

#[test]
fn empty0984_visual_character_prelude_and_last_command_edits_keep_the_exact_root_separate() {
    let expected = empty(74025);
    let generated = prepare_native_frame_graph(&expected).unwrap();
    let root = folder("character-writer"); let file = root.join("main.rvn");
    std::fs::write(&file, &generated.source).unwrap();
    let mut workspace = SourceWorkspace::link(&root, &file, &[expected.clone()]).unwrap();
    let mut docs = workspace.project().graphs().to_vec();
    docs.iter_mut().find(|g| g.kind == GraphKind::Init).unwrap().characters.insert("hero".into(), "Hero".into());
    workspace.save(&docs).unwrap();
    let program = workspace.freeze_native_program().unwrap();
    assert!(matches!(program.script.as_slice(), [Statement::Init { body: prelude }, Statement::Init { body }] if matches!(prelude.as_slice(), [Statement::CharacterCreate { id, display_name }] if id == "hero" && display_name == "Hero") && body.is_empty()));
    same_identity(workspace.project().graphs().iter().find(|g| g.kind == GraphKind::Init).unwrap(), &expected);
    let map = NativeDebugSourceMap::from_workspace(&workspace).unwrap();
    assert_eq!(map.origins().len(), 1); assert_eq!(map.origins()[0].definition_root_ordinal, 1);
    let mut docs = workspace.project().graphs().to_vec();
    let graph = docs.iter_mut().find(|g| g.kind == GraphKind::Init).unwrap();
    graph.add_variable("seen", ValueType::Int, PropertyValue::Int(0)).unwrap();
    let set = graph.add_catalog_node(NodeKind::SetVariable, [150., 150.]).unwrap();
    graph.nodes.get_mut(&set).unwrap().properties.insert("name".into(), PropertyValue::String("seen".into()));
    graph.set_pin_default(set, "value", PropertyValue::Int(1)).unwrap();
    let entry = *graph.nodes.keys().find(|id| graph.nodes[*id].kind == NodeKind::Init).unwrap();
    graph.connect(graph.pin_by_key(entry, "exec_out").unwrap().id, graph.pin_by_key(set, "exec_in").unwrap().id).unwrap();
    workspace.save(&docs).unwrap();
    assert!(!workspace.project().source().contains("@rvn-native-empty-init"));
    assert_eq!(workspace.freeze_native_program().unwrap().script.iter().filter(|s| matches!(s, Statement::Init { .. })).count(), 2);
    let mut docs = workspace.project().graphs().to_vec();
    let graph = docs.iter_mut().find(|g| g.kind == GraphKind::Init).unwrap();
    graph.remove_node(set); graph.variables.remove("seen");
    workspace.save(&docs).unwrap();
    same_identity(workspace.project().graphs().iter().find(|g| g.kind == GraphKind::Init).unwrap(), &expected);
    assert_eq!(workspace.project().source().matches("// @rvn-native-empty-init:v1 ").count(), 1);
    let mut docs = workspace.project().graphs().to_vec();
    docs.iter_mut().find(|g| g.kind == GraphKind::Init).unwrap().characters.remove("hero");
    workspace.save(&docs).unwrap();
    let program = workspace.freeze_native_program().unwrap();
    assert!(matches!(program.script.as_slice(), [Statement::Init { body }] if body.is_empty()));
    drop(workspace);
    let reopened = SourceWorkspace::open(&root).unwrap().unwrap();
    same_identity(reopened.project().graphs().iter().find(|g| g.kind == GraphKind::Init).unwrap(), &expected);
    assert!(reopened.project().graphs().iter().all(|g| g.characters.is_empty()));
}

#[test]
fn empty0984_actual_recovery_cache_restores_native_optin_then_save_freeze_and_reopen() {
    let root = folder("recover-optin"); let file = root.join("main.rvn");
    std::fs::write(&file, "init {}\n").unwrap();
    let mut workspace = SourceWorkspace::link(&root, &file, &[]).unwrap();
    let baseline = std::fs::read_to_string(&file).unwrap();
    let mut docs = workspace.project().graphs().to_vec();
    let graph = docs.iter_mut().find(|g| g.kind == GraphKind::Init).unwrap();
    graph.enable_native_frame_preparation().unwrap(); let expected = graph.clone();
    workspace.write_recovery(&docs).unwrap();
    let cached = std::fs::read(root.join(".rvn-authoring.recovery.json")).unwrap();
    assert_eq!(std::fs::read_to_string(&file).unwrap(), baseline);
    drop(workspace);
    let mut cold = SourceWorkspace::open(&root).unwrap().unwrap();
    assert!(cold.project().graphs().iter().find(|g|g.kind == GraphKind::Init).unwrap().inspector_evaluation.is_legacy());
    let restored = cold.recovery().unwrap().unwrap();
    same_identity(restored.iter().find(|g|g.kind == GraphKind::Init).unwrap(), &expected);
    assert_eq!(std::fs::read(root.join(".rvn-authoring.recovery.json")).unwrap(), cached);
    cold.save(&restored).unwrap();
    assert!(!root.join(".rvn-authoring.recovery.json").exists());
    assert_eq!(NativeDebugSourceMap::from_workspace(&cold).unwrap().origins().len(), 1);
    assert!(native_source_sites(&cold.freeze_native_program().unwrap()).unwrap().is_empty());
    drop(cold);
    let reopened = SourceWorkspace::open(&root).unwrap().unwrap();
    same_identity(reopened.project().graphs().iter().find(|g| g.kind == GraphKind::Init).unwrap(), &expected);
}

#[test]
fn empty0984_interrupted_actual_save_journal_recovers_all_prefixes_and_preserves_conflicting_files() {
    for prefix in 0..4 {
        let root = folder(&format!("journal-{prefix}")); let file = root.join("main.rvn");
        let sidecar = root.join(".rvn-authoring.json"); let journal = root.join(".rvn-authoring.transaction.json");
        std::fs::write(&file, "init {}\n").unwrap();
        let mut workspace = SourceWorkspace::link(&root, &file, &[]).unwrap();
        let before_source = std::fs::read_to_string(&file).unwrap();
        let before_sidecar = std::fs::read_to_string(&sidecar).unwrap();
        let mut docs = workspace.project().graphs().to_vec();
        let graph = docs.iter_mut().find(|g|g.kind == GraphKind::Init).unwrap();
        graph.enable_native_frame_preparation().unwrap(); let expected = graph.clone();
        workspace.save(&docs).unwrap();
        let after_source = std::fs::read_to_string(&file).unwrap();
        let after_sidecar = std::fs::read_to_string(&sidecar).unwrap();
        drop(workspace);
        // Use byte-exact before/after produced by the real Save publisher and
        // the existing durable journal schema, without a product failpoint.
        let transaction = serde_json::json!({"source":"main.rvn", "before_source":before_source,
            "after_source":after_source, "before_sidecar":before_sidecar, "after_sidecar":after_sidecar});
        let journal_bytes = serde_json::to_vec(&transaction).unwrap();
        std::fs::write(&journal, &journal_bytes).unwrap();
        std::fs::write(&file, if prefix & 1 == 0 { &before_source } else { &after_source }).unwrap();
        std::fs::write(&sidecar, if prefix & 2 == 0 { &before_sidecar } else { &after_sidecar }).unwrap();
        let recovered = SourceWorkspace::open(&root).unwrap().unwrap();
        assert!(!journal.exists());
        assert_eq!(std::fs::read_to_string(&file).unwrap(), after_source);
        assert_eq!(std::fs::read_to_string(&sidecar).unwrap(), after_sidecar);
        same_identity(recovered.project().graphs().iter().find(|g|g.kind == GraphKind::Init).unwrap(), &expected);
        assert_eq!(NativeDebugSourceMap::from_workspace(&recovered).unwrap().origins().len(), 1);
        assert!(native_source_sites(&recovered.freeze_native_program().unwrap()).unwrap().is_empty());
        drop(recovered);
        std::fs::write(&journal, &journal_bytes).unwrap();
        std::fs::write(&file, "init {}\n// third party edit\n").unwrap();
        let foreign_source = std::fs::read(&file).unwrap();
        let actual_sidecar = std::fs::read(&sidecar).unwrap();
        assert!(SourceWorkspace::open(&root).is_err());
        assert_eq!(std::fs::read(&journal).unwrap(), journal_bytes);
        assert_eq!(std::fs::read(&file).unwrap(), foreign_source);
        assert_eq!(std::fs::read(&sidecar).unwrap(), actual_sidecar);
    }
}

#[test]
fn empty0998_character_registry_prelude_roundtrips_a_distinct_executable_init_and_refuses_foreign_prefixes() {
    let mut graph=empty(74031);
    graph.characters.insert("hero".into(),"Hero".into());
    graph.add_variable("seen",ValueType::Int,PropertyValue::Int(0)).unwrap();
    let set=graph.add_catalog_node(NodeKind::SetVariable,[150.,150.]).unwrap();
    graph.set_property(set,"name",PropertyValue::String("seen".into())).unwrap();
    graph.set_pin_default(set,"value",PropertyValue::Int(1)).unwrap();
    let root=graph.nodes.values().find(|node|node.kind==NodeKind::Init).unwrap().id;
    graph.connect(graph.pin_by_key(root,"exec_out").unwrap().id,graph.pin_by_key(set,"exec_in").unwrap().id).unwrap();
    let before=graph.clone();let generated=prepare_native_frame_graph(&graph).unwrap();assert_eq!(graph,before);
    assert!(matches!(generated.ast.as_slice(),[Statement::Init{body:prelude},Statement::Init{body}]if matches!(prelude.as_slice(),[Statement::CharacterCreate{id,display_name}]if id=="hero"&&display_name=="Hero")&&!body.is_empty()));
    assert_eq!(generated.origins.len(),2);assert_eq!(generated.origins[0].graph,None);assert_eq!(generated.origins[0].node,None);assert_eq!(generated.origins[1].graph,Some(graph.graph_id));assert_eq!(generated.origins[1].node,Some(root));
    let actual=import_native_frame_source(&generated.source,generated.context).unwrap();
    let actual=actual.iter().find(|candidate|candidate.graph_id==graph.graph_id).unwrap();
    assert_eq!(actual.characters,graph.characters);assert_eq!(actual.edges,graph.edges);assert_eq!(actual.pins.keys().collect::<Vec<_>>(),graph.pins.keys().collect::<Vec<_>>());assert_eq!(actual.nodes.keys().collect::<Vec<_>>(),graph.nodes.keys().collect::<Vec<_>>());
    for (id,pin) in &graph.pins {let restored=&actual.pins[id];assert_eq!((&restored.node,&restored.key,restored.direction,restored.cardinality),(&pin.node,&pin.key,pin.direction,pin.cardinality));assert_eq!(actual.effective_pin_type(*id),graph.effective_pin_type(*id));}
    for prefix in ["init{set injected=7}\n","init{character.create(\"hero\",\"Hero\")}\n"] {
        let changed=format!("{prefix}{}",generated.source);
        assert!(import_native_frame_source(&changed,generated.context).is_err(),"an unrepresented executable or duplicate registry prefix must not borrow Native root identity");
        assert!(SourceProject::open(changed,&[graph.clone()]).is_err());
    }
}
