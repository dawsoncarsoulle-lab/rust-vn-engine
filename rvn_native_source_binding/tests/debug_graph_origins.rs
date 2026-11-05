use rvn_core::{editor_debugger::Route, Engine, TerminalRenderer};
use rvn_graph::*;
use rvn_native_source_binding::FrozenNativeSource;
use std::{fs, path::PathBuf};
fn folder(name: &str) -> PathBuf {
    let base = PathBuf::from(
        std::env::var("RVN_NATIVE_SOURCE_TEST_DIR").expect("private fixture directory required"),
    );
    let dir = base.join(format!(
        "debug-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}
fn frozen(name: &str) -> (PathBuf, SourceWorkspace, FrozenNativeSource) {
    let root = folder(name);
    let raw = include_str!("../../rvn_graph/tests/fixtures/native_source_single.rvn");
    fs::write(
        root.join("main.rvn"),
        format!("{raw}\nlabel start\nset outputs=probe()\n\"done\"\n"),
    )
    .unwrap();
    let workspace =
        SourceWorkspace::link_native_preparation(&root, &root.join("main.rvn"), &[]).unwrap();
    let frozen = FrozenNativeSource::from_workspace(&workspace).unwrap();
    (root, workspace, frozen)
}
#[test]
fn actual_engine_catalogue_uses_verified_consumer_and_declaration_node_mapping() {
    let (_, _, frozen) = frozen("admitted");
    let site = &frozen.sites()[0];
    let endpoint =
        frozen.compiler_endpoint(&site.source, site.graph, site.consumer, PinId::new(13));
    assert!(!endpoint.is_empty());
    assert!(endpoint.iter().any(|entry| !entry.nested_sites.is_empty()));
    let (engine, attachment) = frozen
        .into_debugger_engine(TerminalRenderer, 4, 350)
        .unwrap();
    let breakpoints = frozen
        .breakpoints_for_endpoint(
            &engine,
            &attachment,
            &site.source,
            site.graph,
            site.consumer,
            PinId::new(13),
        )
        .unwrap();
    assert!(!breakpoints.is_empty());
    engine.validate_debug_breakpoints(&breakpoints).unwrap();
    let mapped = engine.debug_site_page(0).unwrap();
    assert!(mapped
        .entries
        .iter()
        .any(|entry| entry.origin.as_ref().is_some_and(|origin| origin
            .graph
            .as_ref()
            .is_some_and(|graph| graph.node == Some(5) && graph.graph == 900))));
    assert!(frozen
        .breakpoints_for_endpoint(
            &engine,
            &attachment,
            &site.source,
            site.graph,
            site.consumer,
            PinId::new(9999)
        )
        .unwrap()
        .is_empty());
}

#[test]
fn actual_staged_attachment_selects_producer_pins_and_snapshot_retains_current_source_path(){
    let(root,_,frozen)=frozen("staged-producer");let staged_root=folder("staged-producer-cold");fs::write(staged_root.join("main.rvn"),fs::read(root.join("main.rvn")).unwrap()).unwrap();
    let staged=rvn_parser::parse_file_program_with_uses(staged_root.join("main.rvn")).unwrap();let mut engine=Engine::from_program_with_debugger(staged.clone(),TerminalRenderer,4).unwrap();let attachment=frozen.attach_staged_debugger(&staged,&mut engine,368).unwrap();assert_eq!(attachment.staged_binding(),&rvn_core::consumer_schedule::ConsumerSourceBinding::from_program(&staged).unwrap());
    let origin=frozen.debug_origins().origins().iter().find(|origin|matches!(origin.role,NativeOriginRole::Producer{inspector:true,..})).unwrap();
    let mut selected=None;
    for pin in [6,7,8]{let bps=frozen.breakpoints_for_endpoint(&engine,&attachment,&origin.source,origin.graph,origin.node,PinId::new(pin)).unwrap();assert_eq!(bps.len(),1);assert!(!bps[0].site.as_ref().unwrap().statement);if let Some(previous)=&selected{assert_eq!(previous,&bps[0]);}else{selected=Some(bps[0].clone());}}
    engine.enter_label("start").unwrap();engine.preview_step_statement().unwrap();let before=serde_json::to_value(&engine.state).unwrap();let mut session=engine.debug_begin(rvn_core::engine::DebugOperation::Statement,vec![selected.unwrap()]).unwrap();assert_eq!(engine.debug_step(&mut session,rvn_core::editor_debugger::StepKind::Continue).unwrap().0,rvn_core::engine::DebugProgress::Stopped);assert_eq!(serde_json::to_value(&engine.state).unwrap(),before);
    let snapshot=session.snapshot();let actual=snapshot.origin.as_ref().unwrap();assert_eq!(actual.source.as_ref().unwrap(),&staged_root.join("main.rvn").canonicalize().unwrap());assert_eq!(actual.graph.as_ref().unwrap().source.as_ref(),Some(&origin.source));assert_eq!(actual.scheduled_step.as_ref().unwrap().index,1);let raw=&staged.sources[0].source;assert!(raw[actual.span.clone().unwrap()].starts_with("inspect \"n3:"));
    let payload=snapshot.encode().unwrap();let decoded=rvn_core::editor_debugger::Snapshot::decode(&payload,engine.instance_id(),engine.runtime_epoch()).unwrap();assert_eq!(decoded.site,session.snapshot().site);
}
#[test]
fn real_producer_pin_diagnostics_are_exact_and_no_fake_core_breakpoint_is_created() {
    let (_, _, frozen) = frozen("producer");
    let origin = frozen
        .debug_origins()
        .origins()
        .iter()
        .find(|origin| {
            matches!(
                origin.role,
                NativeOriginRole::Producer {
                    inspector: true,
                    ..
                }
            )
        })
        .unwrap();
    let endpoint =
        frozen.compiler_endpoint(&origin.source, origin.graph, origin.node, PinId::new(7));
    assert_eq!(endpoint.len(), 1);
    assert!(!endpoint[0].requires_producer_admission);
    assert_eq!(endpoint[0].nested_sites.len(),1);
    assert!(endpoint[0].emitted_instructions.is_empty());
    let raw = &frozen.program().sources[origin.source_ordinal].source;
    assert!(raw[origin.span.clone()].starts_with("inspect "));
    assert!(raw[origin.expression.clone().unwrap()].starts_with("__rvn_captured_v1("));
    let (engine, attachment) = frozen
        .into_debugger_engine(TerminalRenderer, 4, 351)
        .unwrap();
    assert!(!frozen
        .breakpoints_for_endpoint(
            &engine,
            &attachment,
            &origin.source,
            origin.graph,
            origin.node,
            PinId::new(7)
        )
        .unwrap()
        .is_empty());
    let mut actual = engine
        .debug_site_page(0)
        .unwrap()
        .entries
        .into_iter()
        .find(|entry| {
            entry
                .breakpoint
                .site
                .as_ref()
                .is_some_and(|site| site.function.as_deref() == Some("probe"))
        })
        .unwrap()
        .breakpoint;
    actual.site.as_mut().unwrap().route.push(Route::Producer(999));
    actual.site.as_mut().unwrap().statement = false;
    assert!(engine.validate_debug_breakpoints(&[actual]).is_err());
}
#[test]
fn overlay_cold_relocation_maps_actual_sites_and_changed_source_rejected_before_install() {
    let (root, _, frozen) = frozen("relocate");
    let moved = folder("moved");
    fs::write(
        moved.join("main.rvn"),
        fs::read(root.join("main.rvn")).unwrap(),
    )
    .unwrap();
    let cold =
        SourceWorkspace::link_native_preparation(&moved, &moved.join("main.rvn"), &[]).unwrap();
    let staged = cold.freeze_native_program().unwrap();
    let overlay = frozen.debugger_overlay(&staged, 352).unwrap();
    let bytes = overlay.encode().unwrap();
    let overlay = rvn_core::debugger_provenance::Overlay::decode(bytes.as_bytes()).unwrap();
    let mut engine = Engine::from_program_with_debugger(staged, TerminalRenderer, 4).unwrap();
    engine.install_debug_source_map(overlay, 352).unwrap();
    assert!(engine
        .debug_site_page(0)
        .unwrap()
        .entries
        .iter()
        .any(|entry| entry.origin.as_ref().is_some_and(|origin| origin
            .graph
            .as_ref()
            .is_some_and(|g| g.graph == 900 && g.node == Some(5)))));
    let raw = fs::read_to_string(moved.join("main.rvn")).unwrap();
    fs::write(
        moved.join("main.rvn"),
        format!("// changed source same AST\n{raw}"),
    )
    .unwrap();
    let changed = SourceWorkspace::open_native_preparation(&moved)
        .unwrap()
        .unwrap()
        .freeze_native_program()
        .unwrap();
    assert!(frozen.debugger_overlay(&changed, 352).is_err());
    assert!(frozen.debugger_overlay(frozen.program(), 0).is_err());
}
#[test]
fn attachment_rejects_other_engine_edited_program_and_wrong_graph_endpoint() {
    let (_, _, frozen) = frozen("owner");
    let (mut engine, attachment) = frozen
        .into_debugger_engine(TerminalRenderer, 4, 353)
        .unwrap();
    let (other, _) = frozen
        .into_debugger_engine(TerminalRenderer, 4, 354)
        .unwrap();
    let site = &frozen.sites()[0];
    assert!(frozen
        .breakpoints_for_endpoint(
            &other,
            &attachment,
            &site.source,
            site.graph,
            site.consumer,
            PinId::new(13)
        )
        .is_err());
    assert!(frozen
        .breakpoints_for_endpoint(
            &engine,
            &attachment,
            &site.source,
            GraphId::new(901),
            site.consumer,
            PinId::new(13)
        )
        .unwrap()
        .is_empty());
    engine.script.push(rvn_parser::Statement::Return);
    assert!(frozen
        .breakpoints_for_endpoint(
            &engine,
            &attachment,
            &site.source,
            site.graph,
            site.consumer,
            PinId::new(13)
        )
        .is_err());
}
#[test]
fn real_writer_edit_refuses_old_overlay_and_uses_new_ranges_in_cold_engine() {
    let (_, mut workspace, old) = frozen("writer");
    let mut graphs = workspace.project().graphs().to_vec();
    graphs
        .iter_mut()
        .find(|g| g.graph_id == GraphId::new(900))
        .unwrap()
        .set_pin_default(
            NodeId::new(2),
            "key",
            PropertyValue::String("longer 🌍\nnew key".into()),
        )
        .unwrap();
    workspace.save(&graphs).unwrap();
    let current = FrozenNativeSource::from_workspace(&workspace).unwrap();
    assert!(old.debugger_overlay(current.program(), 355).is_err());
    let (engine, attachment) = current
        .into_debugger_engine(TerminalRenderer, 4, 355)
        .unwrap();
    let site = &current.sites()[0];
    assert!(!current
        .breakpoints_for_endpoint(
            &engine,
            &attachment,
            &site.source,
            site.graph,
            site.consumer,
            PinId::new(13)
        )
        .unwrap()
        .is_empty());
    let mut foreign =
        Engine::from_program_with_debugger(old.program().clone(), TerminalRenderer, 4).unwrap();
    let before = serde_json::to_value(&foreign.state).unwrap();
    assert!(current.attach_debugger(&mut foreign, 356).is_err());
    assert_eq!(serde_json::to_value(&foreign.state).unwrap(), before);
}

#[test]
fn genuine_selected_breakpoint_stops_actual_native_consumer_with_graph_origin_before_publication() {
    let (_, _, frozen) = frozen("execution");
    let (mut engine, attachment) = frozen
        .into_debugger_engine(TerminalRenderer, 4, 357)
        .unwrap();
    let site = &frozen.sites()[0];
    let breakpoints = frozen
        .breakpoints_for_endpoint(
            &engine,
            &attachment,
            &site.source,
            site.graph,
            site.consumer,
            PinId::new(13),
        )
        .unwrap();
    engine.enter_label("start").unwrap();
    engine.preview_step_statement().unwrap();
    assert!(matches!(
        engine.script[engine.state.pc],
        rvn_parser::Statement::SetVar { .. }
    ));
    let before = serde_json::to_value(&engine.state).unwrap();
    let mut session = engine
        .debug_begin(rvn_core::engine::DebugOperation::Statement, breakpoints)
        .unwrap();
    assert_eq!(
        engine
            .debug_step(&mut session, rvn_core::editor_debugger::StepKind::Continue)
            .unwrap()
            .0,
        rvn_core::engine::DebugProgress::Stopped
    );
    let snapshot = engine.debug_session_snapshot(&session);
    let actual = snapshot
        .origin
        .as_ref()
        .and_then(|origin| origin.graph.as_ref())
        .unwrap();
    assert_eq!((actual.graph, actual.node), (900, Some(5)));
    assert_eq!(
        snapshot.origin.as_ref().unwrap().span.as_ref(),
        Some(&site.span.range)
    );
    assert_eq!(serde_json::to_value(&engine.state).unwrap(), before);
}

#[test]
fn narrative_branch_command_and_empty_end_annotate_only_real_lowered_sites() {
    fn wire(g: &mut GraphDocument, a: NodeId, ak: &str, b: NodeId, bk: &str) {
        g.connect(
            g.pin_by_key(a, ak).unwrap().id,
            g.pin_by_key(b, bk).unwrap().id,
        )
        .unwrap();
    }
    let mut graph = GraphDocument::new(
        GraphId::new(975),
        GraphKind::Label {
            name: "start".into(),
        },
    );
    let root = graph.add_catalog_node(NodeKind::Label, [0., 0.]).unwrap();
    let inspector = graph
        .add_catalog_node(NodeKind::GetStringTableIdAndKey, [100., 0.])
        .unwrap();
    let branch = graph.add_catalog_node(NodeKind::If, [300., 0.]).unwrap();
    wire(&mut graph, root, "exec_out", branch, "exec_in");
    wire(&mut graph, inspector, "valid", branch, "condition");
    let command = graph
        .add_catalog_node(NodeKind::AccessibilitySpeak, [500., 0.])
        .unwrap();
    wire(&mut graph, branch, "then", command, "exec_in");
    wire(&mut graph, inspector, "key", command, "text");
    let end = graph.add_branch_end(branch, [700., 0.]).unwrap();
    wire(&mut graph, command, "exec_out", end, "exec_in");
    let dir = folder("branch");
    fs::write(
        dir.join("main.rvn"),
        prepare_native_frame_graph(&graph).unwrap().source,
    )
    .unwrap();
    let workspace =
        SourceWorkspace::link_native_preparation(&dir, &dir.join("main.rvn"), &[]).unwrap();
    let frozen = FrozenNativeSource::from_workspace(&workspace).unwrap();
    for node in [branch, command] {
        let site = frozen
            .sites()
            .iter()
            .find(|site| site.consumer == node)
            .unwrap();
        let actual = frozen
            .compiler_map()
            .origins
            .iter()
            .chain(frozen.compiler_map().sites.iter().map(|site| &site.origin))
            .find(|origin| {
                origin.span.as_ref() == Some(&site.span.range)
                    && origin
                        .graph
                        .as_ref()
                        .is_some_and(|id| id.graph == 975 && id.node == Some(node.get()))
            });
        assert!(
            actual.is_some(),
            "actual source consumer {node} lacks lowered mapping"
        );
    }
    assert!(frozen
        .unmapped_compiler_sites()
        .iter()
        .all(|site| site.consumer == end));
    let (engine, attachment) = frozen
        .into_debugger_engine(TerminalRenderer, 4, 358)
        .unwrap();
    let main = frozen.sites()[0].source.clone();
    assert!(!frozen
        .breakpoints_for_endpoint(
            &engine,
            &attachment,
            &main,
            graph.graph_id,
            branch,
            graph.pin_by_key(branch, "exec_in").unwrap().id
        )
        .unwrap()
        .is_empty());
    let endpoint = frozen.compiler_endpoint(&main, graph.graph_id, end, graph.nodes[&end].pins[0]);
    assert!(endpoint
        .iter()
        .any(|entry| !entry.emitted_instructions.is_empty()));
    let actual = frozen
        .breakpoints_for_endpoint(
            &engine,
            &attachment,
            &main,
            graph.graph_id,
            end,
            graph.nodes[&end].pins[0],
        )
        .unwrap();
    assert!(!actual.is_empty());
    engine.validate_debug_breakpoints(&actual).unwrap();
}

#[test]
fn real_graph_reroute_writer_cold_reparse_authenticates_current_pin_edge_and_refuses_old_overlay() {
    let (root, mut workspace, old) = frozen("reroute");
    let mut graphs=workspace.project().graphs().to_vec();
    let graph=graphs.iter_mut().find(|g|g.graph_id==GraphId::new(900)).unwrap();
    let inspector=NodeId::new(3);let input=graph.pin_by_key(inspector,"text").unwrap().id;
    let old_edge=graph.edges.values().find(|e|e.input==input).unwrap().clone();
    graph.remove_edge(old_edge.id).unwrap();
    let reroute=graph.add_catalog_node(NodeKind::Reroute,[300.,40.]).unwrap();
    graph.set_reroute_type(reroute,graph.pins[&input].value_type.clone()).unwrap();
    graph.connect(old_edge.output,graph.pin_by_key(reroute,"value").unwrap().id).unwrap();
    graph.connect(graph.pin_by_key(reroute,"value_out").unwrap().id,input).unwrap();
    let edge=graph.edges.values().find(|e|e.input==input).unwrap().id;
    assert_ne!(old_edge.id,edge);
    workspace.save(&graphs).unwrap();
    let cold=SourceWorkspace::open(&root).unwrap().unwrap();
    let current=FrozenNativeSource::from_workspace(&cold).unwrap();
    assert!(old.debugger_overlay(current.program(),369).is_err());
    let (engine,attachment)=current.into_debugger_engine(TerminalRenderer,4,369).unwrap();
    let bps=current.breakpoints_for_endpoint(&engine,&attachment,&cold.source_path(),GraphId::new(900),inspector,input).unwrap();
    assert_eq!(bps.len(),1);engine.validate_debug_breakpoints(&bps).unwrap();
    let entry=engine.debug_site_page(0).unwrap().entries.into_iter().find(|entry|entry.breakpoint==bps[0]).unwrap();
    let origin=entry.origin.unwrap();let actual=origin.scheduled_step.unwrap();
    let port=actual.endpoint.unwrap().ports.into_iter().find(|port|port.role=="text").unwrap();
    assert_eq!(port.pin,input.get());assert_eq!(port.incoming,Some(edge.get()));
    let raw=&current.program().sources[actual.source_ordinal].source;
    assert!(raw[origin.span.unwrap()].starts_with("inspect \"n3:"));
    assert!(current.breakpoints_for_endpoint(&engine,&attachment,&cold.source_path(),GraphId::new(900),reroute,input).unwrap().is_empty());
}
