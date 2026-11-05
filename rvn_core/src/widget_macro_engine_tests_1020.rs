//! Child of the existing engine_widget_handler_tests_0956 fixture. The source
//! must pass real Graph import, SourceProject/class admission and Engine input.
//! These are proposed vertical tests, NOT executed in this private packet.
use super::*;
fn fixture() -> Fixture {
    Fixture::with_source_transform(false, false, false, false, |source| {
        let original = rvn_parser::parse("handler allowed(event){set hits=hits+1}").unwrap();
        let mut graph = rvn_graph::import_script(&original)
            .unwrap()
            .into_iter()
            .find(
                |graph| matches!(&graph.kind,rvn_graph::GraphKind::Handler{name}if name=="allowed"),
            )
            .unwrap();
        let root = graph
            .nodes
            .values()
            .find(|node| node.kind == rvn_graph::NodeKind::HandlerEntry)
            .unwrap()
            .id;
        let output = graph.pin_by_key(root, "exec_out").unwrap().id;
        let edge = graph
            .edges
            .values()
            .find(|edge| edge.output == output)
            .unwrap()
            .clone();
        let once = graph
            .add_catalog_node(rvn_graph::NodeKind::DoOnce, [400.0, 0.0])
            .unwrap();
        let enter = graph.pin_by_key(once, "exec_in").unwrap().id;
        let completed = graph.pin_by_key(once, "completed").unwrap().id;
        graph.remove_edge(edge.id).unwrap();
        graph.connect(output, enter).unwrap();
        graph.connect(completed, edge.input).unwrap();
        graph.enable_native_frame_preparation().unwrap();
        let generated = rvn_graph::prepare_native_frame_graph(&graph).unwrap();
        assert!(generated.source.contains("flow.macro_dispatch_v1("));
        let old="handler allowed(event){local roll=random(1,100) while false{set roll=roll+1} set hits=hits+1}\n";
        assert_eq!(
            source.matches(old).count(),
            1,
            "replace exactly one real Handler source"
        );
        source.replacen(old, &generated.source, 1)
    })
}
fn click(engine: &mut Engine<Host>, alias: &str) {
    let event = input(engine, alias, rvn_ui::programmable::ScreenEventKind::Click);
    assert!(engine.interface_event_scoped(event).unwrap());
}
#[test]
fn standard_macro1020_real_widget_button_once_is_instance_local_survives_detach_and_recreation_is_fresh(
) {
    let fixture = fixture();
    let mut engine = fixture.engine();
    let a = engine
        .create_widget(A, "macro_a", vec![Value::Str("allowed".into())])
        .unwrap();
    let b = engine
        .create_widget(A, "macro_b", vec![Value::Str("allowed".into())])
        .unwrap();
    engine.add_widget_to_viewport(&a, false, 0).unwrap();
    engine.add_widget_to_viewport(&b, false, 0).unwrap();
    click(&mut engine, "macro_a");
    click(&mut engine, "macro_a");
    assert_eq!(engine.state.vars["hits"], Value::Int(1));
    click(&mut engine, "macro_b");
    assert_eq!(engine.state.vars["hits"], Value::Int(2));
    // Warm rollback + real wrapper rematerialization is exercised by the
    // companion Bevy test, which uses its actual backend tree/reservation.
    let target = engine.widget_backend_owner(&a).unwrap().target().clone();
    engine.remove_widget_from_viewport(&a).unwrap();
    engine.add_widget_to_viewport(&a, false, 0).unwrap();
    assert_eq!(engine.widget_backend_owner(&a).unwrap().target(), &target);
    click(&mut engine, "macro_a");
    assert_eq!(engine.state.vars["hits"], Value::Int(2));
    engine.destroy_widget(a).unwrap();
    let fresh = engine
        .create_widget(A, "macro_a", vec![Value::Str("allowed".into())])
        .unwrap();
    assert_ne!(
        engine.widget_backend_owner(&fresh).unwrap().target(),
        &target
    );
    engine.add_widget_to_viewport(&fresh, false, 0).unwrap();
    click(&mut engine, "macro_a");
    assert_eq!(engine.state.vars["hits"], Value::Int(3));
    // Public state is not authority. Copying an inert frame to a different
    // same-class instance must fail against actual routed TargetIdentity.
    let wrong = engine
        .state
        .ui
        .screens
        .iter()
        .find(|i| i.name == "macro_a")
        .unwrap()
        .standard_macro_frame
        .clone();
    engine
        .state
        .ui
        .screens
        .iter_mut()
        .find(|i| i.name == "macro_b")
        .unwrap()
        .standard_macro_frame = wrong;
    let vars = engine.state.vars.clone();
    let batches = engine.renderer.batches.lock().unwrap().len();
    let forged = input(
        &engine,
        "macro_b",
        rvn_ui::programmable::ScreenEventKind::Click,
    );
    assert!(engine.interface_event_scoped(forged).is_err());
    assert_eq!(engine.state.vars, vars);
    assert_eq!(engine.renderer.batches.lock().unwrap().len(), batches);
    let mut left = fixture.engine();
    let mut right = fixture.engine();
    let left_handle = left
        .create_widget(A, "same_birth", vec![Value::Str("allowed".into())])
        .unwrap();
    let right_handle = right
        .create_widget(A, "same_birth", vec![Value::Str("allowed".into())])
        .unwrap();
    left.add_widget_to_viewport(&left_handle, false, 0).unwrap();
    right
        .add_widget_to_viewport(&right_handle, false, 0)
        .unwrap();
    assert_eq!(
        left.widget_backend_owner(&left_handle).unwrap().target(),
        right.widget_backend_owner(&right_handle).unwrap().target(),
        "same source/TargetIdentity is deliberate"
    );
    assert_ne!(left.instance_id(), right.instance_id());
    click(&mut left, "same_birth");
    let foreign = left
        .state
        .ui
        .screens
        .iter()
        .find(|i| i.name == "same_birth")
        .unwrap()
        .standard_macro_frame
        .clone();
    right
        .state
        .ui
        .screens
        .iter_mut()
        .find(|i| i.name == "same_birth")
        .unwrap()
        .standard_macro_frame = foreign;
    let before = right.state.vars.clone();
    let publications = right.renderer.batches.lock().unwrap().len();
    let event = input(
        &right,
        "same_birth",
        rvn_ui::programmable::ScreenEventKind::Click,
    );
    assert!(
        right.interface_event_scoped(event).is_err(),
        "real routing must reject the foreign Engine lineage before any mutation/publication"
    );
    assert_eq!(right.state.vars, before);
    assert_eq!(right.renderer.batches.lock().unwrap().len(), publications);
}
#[test]
fn standard_macro1020_real_publication_refusal_keeps_cell_unconsumed_and_completed_cold_save_resets_transient_cells(
) {
    let fixture = fixture();
    let mut engine = fixture.engine();
    let a = engine
        .create_widget(A, "macro_a", vec![Value::Str("allowed".into())])
        .unwrap();
    engine.add_widget_to_viewport(&a, false, 0).unwrap();
    let before = engine.state.clone();
    let publications = engine.renderer.batches.lock().unwrap().len();
    engine.renderer.reject.store(true, Ordering::SeqCst);
    let event = input(
        &engine,
        "macro_a",
        rvn_ui::programmable::ScreenEventKind::Click,
    );
    assert!(engine.interface_event_scoped(event).is_err());
    assert_eq!(
        engine
            .state
            .ui
            .screens
            .iter()
            .find(|i| i.name == "macro_a")
            .unwrap()
            .standard_macro_frame,
        before
            .ui
            .screens
            .iter()
            .find(|i| i.name == "macro_a")
            .unwrap()
            .standard_macro_frame,
        "compare live cells directly: cold JSON deliberately omits them"
    );
    assert_eq!(engine.state.vars, before.vars);
    assert_eq!(engine.renderer.batches.lock().unwrap().len(), publications);
    engine.renderer.reject.store(false, Ordering::SeqCst);
    click(&mut engine, "macro_a");
    click(&mut engine, "macro_a");
    assert_eq!(engine.state.vars["hits"], Value::Int(1));
    let live = engine.state.clone();
    assert_eq!(
        live.ui
            .screens
            .iter()
            .find(|i| i.name == "macro_a")
            .unwrap()
            .standard_macro_frame,
        engine
            .state
            .ui
            .screens
            .iter()
            .find(|i| i.name == "macro_a")
            .unwrap()
            .standard_macro_frame
    );
    let save = engine
        .capture_save15(&fixture.program, None, 1, "transient standard macro".into())
        .unwrap();
    let wire = save.encode().unwrap();
    assert!(
        !wire.contains("standard_macro_frame"),
        "old GameSave JSON has no transient macro field"
    );
    let decoded = crate::save::consumer_checkpoint::Save15::decode(&wire).unwrap();
    let raw =
        crate::save::SaveData::from_state(&engine.state, 1, "direct DTO".into(), "main.rvn".into());
    assert!(
        raw.ui
            .screens
            .iter()
            .all(|i| i.standard_macro_frame.is_none()),
        "direct SaveData capture is cold too"
    );
    assert!(raw
        .into_game_state()
        .ui
        .screens
        .iter()
        .all(|i| i.standard_macro_frame.is_none()));
    for save in [save, decoded] {
        let startup = Engine::prepare_widget_class_startup(
            fixture.program.clone(),
            fixture.classes.clone(),
            Host::default(),
            16,
            Default::default(),
            Default::default(),
        )
        .unwrap();
        let (mut cold, debugger) = startup.restore_completed_save15(save).unwrap();
        assert!(debugger.is_none());
        assert!(cold
            .state
            .ui
            .screens
            .iter()
            .all(|i| i.standard_macro_frame.is_none()));
        assert_eq!(
            cold.state.vars["hits"],
            Value::Int(1),
            "do not replay Init or alter saved authored variables"
        );
        assert!(
            cold.capture_ui_input_source("macro_a").is_none(),
            "cold logical restoration alone grants no external input"
        );
        // This Host intentionally remains logical-only. Keep the strict
        // production refusal; actual rematerialization/cold input is tested
        // through the existing Bevy backend by the companion test.
        assert!(cold.has_pending_widget_materialization());
        let vars = cold.state.vars.clone();
        let rng = cold.state.random;
        let error = cold.next_widget_materialization().unwrap_err();
        assert!(error.to_string().contains("Materialization did not prepare a real source-owned backend object"));
        assert!(cold.has_pending_widget_materialization());
        assert!(cold.capture_ui_input_source("macro_a").is_none());
        assert_eq!(cold.state.vars, vars);
        assert_eq!(cold.state.random, rng);
    }
}

#[test]
fn standard_macro1020_actual_debug_candidate_has_exact_cells_and_abandonment_does_not_consume_live_owner(
) {
    use crate::widget_class_source_0950::standard_macros::CellInspection;
    let fixture = fixture();
    let mut engine = fixture.engine();
    let a = engine
        .create_widget(A, "macro_a", vec![Value::Str("allowed".into())])
        .unwrap();
    engine.add_widget_to_viewport(&a, false, 0).unwrap();
    let site = fixture
        .program
        .script
        .iter()
        .find_map(|statement| match statement {
            Statement::Handler { name, body, .. } if name == "allowed" => {
                body.iter().find_map(|statement| match statement {
                    Statement::StandardMacroDispatch { dispatch } => {
                        Some((dispatch.site.node.graph, dispatch.site.node.node))
                    }
                    _ => None,
                })
            }
            _ => None,
        })
        .unwrap();
    assert_eq!(
        engine
            .inspect_widget_macro_cell(&a, site.0, site.1)
            .unwrap()
            .1,
        CellInspection::DoOnce {
            initialized: false,
            completed: false
        }
    );
    let event = input(
        &engine,
        "macro_a",
        rvn_ui::programmable::ScreenEventKind::Click,
    );
    let mut session = engine
        .debug_begin(DebugOperation::Input(event), vec![])
        .unwrap();
    let mut observed = false;
    for _ in 0..128 {
        if let Some((
            origin,
            CellInspection::DoOnce {
                initialized: true,
                completed: true,
            },
        )) = engine
            .inspect_debug_widget_macro_cell(&session, &a, site.0, site.1)
            .unwrap()
        {
            assert_eq!((origin.graph, origin.node), (site.0, Some(site.1)));
            observed = true;
            break;
        }
        let (progress, _) = engine
            .debug_step(&mut session, crate::editor_debugger::StepKind::Into)
            .unwrap();
        assert_ne!(
            progress,
            DebugProgress::Complete,
            "must observe the cell before the real downstream assignment commits"
        );
    }
    assert!(
        observed,
        "actual continuation sees completion written before its recipient"
    );
    let live_before_refusal = engine.state.clone();
    let epoch_before_refusal = engine.runtime_epoch();
    let publications = engine.renderer.batches.lock().unwrap().len();
    let error = engine
        .capture_save15(
            &fixture.program,
            Some(&session),
            2,
            "pending macro must refuse cold Save15".into(),
        )
        .err()
        .expect("pending hot owner has no cold-load mapping");
    assert!(
        error.to_string().contains("pending Widget macro owner"),
        "actual Save15 guard: {error}"
    );
    assert_eq!(engine.state.vars, live_before_refusal.vars);
    assert_eq!(engine.state.random, live_before_refusal.random);
    assert_eq!(engine.runtime_epoch(), epoch_before_refusal);
    assert_eq!(engine.renderer.batches.lock().unwrap().len(), publications);
    assert_eq!(
        engine
            .state
            .ui
            .screens
            .iter()
            .map(|i| &i.standard_macro_frame)
            .collect::<Vec<_>>(),
        live_before_refusal
            .ui
            .screens
            .iter()
            .map(|i| &i.standard_macro_frame)
            .collect::<Vec<_>>()
    );
    assert!(
        engine.accepts_debug_session(&session),
        "capture refusal leaves the actual live continuation admitted"
    );
    let trace = engine
        .actual_widget_macro_trace_for_test(&session, &a)
        .unwrap();
    assert!(trace.has_widget_macro_owner());
    // An external DTO may contain otherwise valid wire data. It grants no hot
    // owner: both public admission paths must refuse it before any preparation.
    let mut external = engine
        .capture_save15(
            &fixture.program,
            None,
            3,
            "actual completed cold base for external-input refusal".into(),
        )
        .unwrap();
    external.debugger = Some(
        engine
            .actual_widget_macro_debug_dto_for_test(&fixture.program, &session, &a)
            .unwrap(),
    );
    let encoded = external.encode().unwrap();
    let external = crate::save::consumer_checkpoint::Save15::decode(&encoded).unwrap();
    let warming_error = engine
        .save15_state_for_audio_warming(&fixture.program, &external)
        .err()
        .expect("external macro trace cannot warm a cold restore");
    assert!(
        warming_error
            .to_string()
            .contains("pending Widget macro owner"),
        "early warming refusal: {warming_error}"
    );
    let restore_error = engine
        .prepare_save15_with_locale(&fixture.program, external, None)
        .err()
        .expect("external macro trace cannot prepare a cold restore");
    assert!(
        restore_error
            .to_string()
            .contains("pending Widget macro owner"),
        "early restore refusal: {restore_error}"
    );
    assert_eq!(engine.state.vars, live_before_refusal.vars);
    assert_eq!(engine.state.random, live_before_refusal.random);
    assert_eq!(engine.runtime_epoch(), epoch_before_refusal);
    assert_eq!(engine.renderer.batches.lock().unwrap().len(), publications);
    assert_eq!(
        engine
            .state
            .ui
            .screens
            .iter()
            .map(|i| &i.standard_macro_frame)
            .collect::<Vec<_>>(),
        live_before_refusal
            .ui
            .screens
            .iter()
            .map(|i| &i.standard_macro_frame)
            .collect::<Vec<_>>()
    );
    assert!(
        engine.accepts_debug_session(&session),
        "external DTO refusals grant or mutate no actual session"
    );
    let foreign = fixture.engine();
    engine.functions.assert_actual_macro_trace_lineage_for_test(
        &trace,
        engine.instance_id(),
        foreign.instance_id(),
    );

    assert_eq!(engine.state.vars["hits"], Value::Int(0));
    assert_eq!(
        engine
            .inspect_widget_macro_cell(&a, site.0, site.1)
            .unwrap()
            .1,
        CellInspection::DoOnce {
            initialized: false,
            completed: false
        },
        "live owner is unchanged while candidate is stopped"
    );
    click(&mut engine, "macro_a");
    assert_eq!(engine.state.vars["hits"], Value::Int(1));
    assert!(!engine.accepts_debug_session(&session));
    assert!(
        engine
            .inspect_debug_widget_macro_cell(&session, &a, site.0, site.1)
            .is_err(),
        "stale candidate cannot silently rebind to current owner"
    );
    drop(session);
    click(&mut engine, "macro_a");
    assert_eq!(engine.state.vars["hits"], Value::Int(1));
}
// Additional proposed vertical coverage. These fixtures author real catalogue
// pins, pass the strict .rvn topology roundtrip and use the existing class/Host.
fn family_assignment(graph: &rvn_graph::GraphDocument, name: &str) -> rvn_graph::NodeId {
    let nodes = graph
        .nodes
        .values()
        .filter(|n| {
            n.kind == rvn_graph::NodeKind::SetVariable
                && n.properties.get("name") == Some(&rvn_graph::PropertyValue::String(name.into()))
        })
        .map(|n| n.id)
        .collect::<Vec<_>>();
    assert_eq!(nodes.len(), 1, "one authored assignment {name}");
    nodes[0]
}
fn family_wire(
    graph: &mut rvn_graph::GraphDocument,
    from: rvn_graph::NodeId,
    output: &str,
    to: rvn_graph::NodeId,
    input: &str,
) {
    let output = graph.pin_by_key(from, output).unwrap().id;
    let input = graph.pin_by_key(to, input).unwrap().id;
    graph.connect(output, input).unwrap();
}
fn family_replace_route(
    graph: &mut rvn_graph::GraphDocument,
    marker: &str,
    node: rvn_graph::NodeId,
    control: &str,
    output: Option<&str>,
) {
    let marker = family_assignment(graph, marker);
    let enter = graph.pin_by_key(marker, "exec_in").unwrap().id;
    let exit = graph.pin_by_key(marker, "exec_out").unwrap().id;
    let incoming = graph
        .edges
        .values()
        .filter(|e| e.input == enter)
        .cloned()
        .collect::<Vec<_>>();
    let outgoing = graph
        .edges
        .values()
        .filter(|e| e.output == exit)
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(incoming.len(), 1);
    assert_eq!(outgoing.len(), usize::from(output.is_some()));
    graph.remove_node(marker).unwrap();
    let input = graph.pin_by_key(node, control).unwrap().id;
    graph.connect(incoming[0].output, input).unwrap();
    if let Some(output) = output {
        let pin = graph.pin_by_key(node, output).unwrap().id;
        graph.connect(pin, outgoing[0].input).unwrap();
    }
}
fn family_fixture(
    kind: rvn_graph::NodeKind,
    start_closed: bool,
    reentrant_reset: bool,
) -> (Fixture, (u64, u64), rvn_graph::GraphDocument) {
    use rvn_graph::{NodeKind as K, PropertyValue as P};
    assert!(!reentrant_reset || kind == K::DoN);
    let authored = match (kind, reentrant_reset) {
        (K::DoN, false) => "handler allowed(event){if event[\"element\"]==\"reset\"{set route_reset=0}else{set route_enter=0 set hits=hits+1 set observed=0}}",
        (K::DoN, true) => "handler allowed(event){set route_enter=0 set hits=hits+1 set observed=0}",
        (K::FlipFlop, false) => "handler allowed(event){set route_enter=0 set branch_a=branch_a+1 set observed_a=false set branch_b=branch_b+1 set observed_b=false}",
        (K::Gate, false) => "handler allowed(event){if event[\"element\"]==\"open\"{set route_open=0}else{if event[\"element\"]==\"close\"{set route_close=0}else{if event[\"element\"]==\"toggle\"{set route_toggle=0}else{set route_enter=0 set hits=hits+1}}}}",
        _ => panic!("only the three additional real macro families"),
    };
    let mut graph = rvn_graph::import_script(&rvn_parser::parse(authored).unwrap())
        .unwrap()
        .into_iter()
        .find(|g| matches!(&g.kind,rvn_graph::GraphKind::Handler{name}if name=="allowed"))
        .unwrap();
    // Macro outputs replace the authored marker lanes. Keep the real ends
    // reached from a nested If's completed port: those still close its outer
    // If lane and must retain their authoritative source identities.
    let old_ends = graph
        .nodes
        .values()
        .filter(|n| {
            n.kind == K::BranchEnd
                && !graph.edges.values().any(|edge| {
                    let input = &graph.pins[&edge.input];
                    let output = &graph.pins[&edge.output];
                    input.node == n.id
                        && output.key == "completed"
                        && graph.nodes[&output.node].kind == K::If
                })
        })
        .map(|n| n.id)
        .collect::<Vec<_>>();
    for end in old_ends {
        graph.remove_node(end).unwrap();
    }
    let node = graph.add_catalog_node(kind, [400.0, 0.0]).unwrap();
    match kind {
        K::DoN => {
            graph.set_pin_default(node, "n", P::Int(2)).unwrap();
            family_replace_route(&mut graph, "route_enter", node, "exec_in", Some("exit"));
            if !reentrant_reset {
                family_replace_route(&mut graph, "route_reset", node, "reset", None);
            }
            let observed = family_assignment(&graph, "observed");
            family_wire(&mut graph, node, "counter", observed, "value");
            if reentrant_reset {
                let exit = graph.pin_by_key(node, "exit").unwrap().id;
                let edge = graph
                    .edges
                    .values()
                    .find(|e| e.output == exit)
                    .unwrap()
                    .clone();
                graph.remove_edge(edge.id).unwrap();
                let sequence = graph.add_catalog_node(K::Sequence, [700.0, 0.0]).unwrap();
                family_wire(&mut graph, node, "exit", sequence, "exec_in");
                family_wire(&mut graph, sequence, "then_0", node, "reset");
                let then = graph.pin_by_key(sequence, "then_1").unwrap().id;
                graph.connect(then, edge.input).unwrap();
                let end = graph.add_branch_end(sequence, [1000.0, 300.0]).unwrap();
                family_wire(&mut graph, observed, "exec_out", end, "exec_in");
            }
        }
        K::FlipFlop => {
            let observed_a = family_assignment(&graph, "observed_a");
            let branch_b = family_assignment(&graph, "branch_b");
            let out = graph.pin_by_key(observed_a, "exec_out").unwrap().id;
            let cut = graph
                .edges
                .values()
                .find(|e| e.output == out)
                .unwrap()
                .clone();
            assert_eq!(cut.input, graph.pin_by_key(branch_b, "exec_in").unwrap().id);
            graph.remove_edge(cut.id).unwrap();
            family_replace_route(&mut graph, "route_enter", node, "exec_in", Some("a"));
            family_wire(&mut graph, node, "b", branch_b, "exec_in");
            let observed_b = family_assignment(&graph, "observed_b");
            family_wire(&mut graph, node, "is_a", observed_a, "value");
            family_wire(&mut graph, node, "is_a", observed_b, "value");
        }
        K::Gate => {
            graph
                .set_pin_default(node, "start_closed", P::Bool(start_closed))
                .unwrap();
            family_replace_route(&mut graph, "route_enter", node, "exec_in", Some("exit"));
            for control in ["open", "close", "toggle"] {
                family_replace_route(&mut graph, &format!("route_{control}"), node, control, None);
            }
        }
        _ => unreachable!(),
    }
    graph.enable_native_frame_preparation().unwrap();
    // This performs the authoritative emitted-source reconstruction and exact
    // node/pin/edge comparison; no fallback importer or fabricated identity.
    let generated = rvn_graph::prepare_native_frame_graph(&graph).unwrap();
    assert!(generated.source.contains("flow.macro_dispatch_v1("));
    if kind == K::DoN {
        assert!(generated.source.contains("flow.macro_counter_v1("));
    }
    if kind == K::FlipFlop {
        assert!(generated.source.contains("flow.macro_is_a_v1("));
    }
    let site = (graph.graph_id.get(), node.get());
    let fixture = Fixture::with_source_transform(false, false, false, false, |source| {
        let old = "handler allowed(event){local roll=random(1,100) while false{set roll=roll+1} set hits=hits+1}\n";
        assert_eq!(source.matches(old).count(), 1);
        let source = source.replacen(old, &generated.source, 1);
        let old_panel = "screen panel(method){return component(\"go\",\"button\",{\"events\":{\"click\":method}},[])}";
        assert_eq!(source.matches(old_panel).count(), 1);
        let panel = "screen panel(method){return component(\"root\",\"column\",{},[component(\"go\",\"button\",{\"events\":{\"click\":method}},[]),component(\"reset\",\"button\",{\"events\":{\"click\":method}},[]),component(\"open\",\"button\",{\"events\":{\"click\":method}},[]),component(\"close\",\"button\",{\"events\":{\"click\":method}},[]),component(\"toggle\",\"button\",{\"events\":{\"click\":method}},[])])}";
        let source = source.replacen(old_panel, panel, 1);
        assert_eq!(source.matches("init{set hits=0}").count(), 1);
        source.replacen("init{set hits=0}", "init{set hits=0 set observed=-1 set branch_a=0 set branch_b=0 set observed_a=false set observed_b=false}", 1)
    });
    (fixture, site, graph)
}
fn family_input(
    engine: &Engine<Host>,
    alias: &str,
    element: &str,
) -> crate::ui_input::ScopedUiInput {
    crate::ui_input::ScopedUiInput {
        source: engine.capture_ui_input_source(alias).unwrap(),
        event: crate::ui::UiInput {
            screen: alias.into(),
            element: element.into(),
            kind: rvn_ui::programmable::ScreenEventKind::Click,
            value: None,
            key: None,
        },
    }
}
fn family_press_at(engine: &mut Engine<Host>, alias: &str, element: &str) {
    let event = family_input(engine, alias, element);
    assert!(engine.interface_event_scoped(event).unwrap());
}
fn family_press(engine: &mut Engine<Host>, element: &str) {
    family_press_at(engine, "macro_family", element);
}
fn family_cell(
    engine: &Engine<Host>,
    handle: &crate::engine::WidgetInstanceHandle,
    site: (u64, u64),
) -> crate::widget_class_source_0950::standard_macros::CellInspection {
    let (origin, cell) = engine
        .inspect_widget_macro_cell(handle, site.0, site.1)
        .unwrap();
    assert_eq!((origin.graph, origin.node), (site.0, Some(site.1)));
    cell
}
fn family_publication_refuses(
    engine: &mut Engine<Host>,
    handle: &crate::engine::WidgetInstanceHandle,
    site: (u64, u64),
    element: &str,
) {
    let before = engine.state.clone();
    let cell = family_cell(engine, handle, site);
    let publications = engine.renderer.batches.lock().unwrap().len();
    let epoch = engine.runtime_epoch();
    let host_random = engine.host_random.clone();
    engine.renderer.reject.store(true, Ordering::SeqCst);
    let event = family_input(engine, "macro_family", element);
    let error = engine.interface_event_scoped(event).unwrap_err();
    engine.renderer.reject.store(false, Ordering::SeqCst);
    assert!(
        error.to_string().contains("Widget renderer refusal"),
        "real renderer refusal: {error}"
    );
    assert_eq!(engine.state.vars, before.vars);
    assert_eq!(engine.state.random, before.random);
    assert_eq!(engine.host_random, host_random);
    assert_eq!(engine.runtime_epoch(), epoch);
    assert_eq!(family_cell(engine, handle, site), cell);
    assert_eq!(engine.renderer.batches.lock().unwrap().len(), publications);
}

#[test]
fn standard_macro1021_don_actual_widget_reset_counter_transaction_and_bounded_reset_reentry() {
    use crate::widget_class_source_0950::standard_macros::CellInspection as C;
    let (fixture, site, mut authored) = family_fixture(rvn_graph::NodeKind::DoN, false, false);
    let mut engine = fixture.engine();
    let handle = engine
        .create_widget(A, "macro_family", vec![Value::Str("allowed".into())])
        .unwrap();
    engine.add_widget_to_viewport(&handle, false, 0).unwrap();
    family_press(&mut engine, "reset");
    assert_eq!(
        family_cell(&engine, &handle, site),
        C::DoN {
            initialized: false,
            counter: 0
        }
    );
    family_press(&mut engine, "go");
    assert_eq!(engine.state.vars["hits"], Value::Int(1));
    assert_eq!(
        engine.state.vars["observed"],
        Value::Int(1),
        "wired Counter sees increment before recipient"
    );
    family_publication_refuses(&mut engine, &handle, site, "go");
    family_press(&mut engine, "go");
    family_press(&mut engine, "go");
    assert_eq!(engine.state.vars["hits"], Value::Int(2));
    assert_eq!(engine.state.vars["observed"], Value::Int(2));
    assert_eq!(
        family_cell(&engine, &handle, site),
        C::DoN {
            initialized: true,
            counter: 2
        }
    );
    family_press(&mut engine, "reset");
    assert_eq!(
        family_cell(&engine, &handle, site),
        C::DoN {
            initialized: true,
            counter: 0
        }
    );
    assert_eq!(
        engine.state.vars["hits"],
        Value::Int(2),
        "Reset emits no Exit"
    );
    assert_eq!(
        engine.state.vars["observed"],
        Value::Int(2),
        "Reset emits no recipient assignment"
    );
    family_press(&mut engine, "go");
    assert_eq!(engine.state.vars["hits"], Value::Int(3));
    assert_eq!(engine.state.vars["observed"], Value::Int(1));
    authored
        .set_pin_default(
            rvn_graph::NodeId::new(site.1),
            "n",
            rvn_graph::PropertyValue::Int(i64::MAX),
        )
        .unwrap();
    let rejected = rvn_graph::prepare_native_frame_graph(&authored).unwrap_err();
    assert!(
        rejected.to_string().contains("int32"),
        "out-of-domain N refused at actual Graph/source admission: {rejected}"
    );

    let (fixture, site, _) = family_fixture(rvn_graph::NodeKind::DoN, false, true);
    let mut engine = fixture.engine();
    let handle = engine
        .create_widget(A, "macro_family", vec![Value::Str("allowed".into())])
        .unwrap();
    engine.add_widget_to_viewport(&handle, false, 0).unwrap();
    for hit in 1..=2 {
        family_press(&mut engine, "go");
        assert_eq!(engine.state.vars["hits"], Value::Int(hit));
        assert_eq!(
            engine.state.vars["observed"],
            Value::Int(0),
            "Exit reentered actual Reset before later live Counter read; no frozen Exit snapshot"
        );
        assert_eq!(
            family_cell(&engine, &handle, site),
            C::DoN {
                initialized: true,
                counter: 0
            }
        );
    }
}

#[test]
fn standard_macro1021_flipflop_actual_widget_alternates_wired_is_a_and_refused_candidate_does_not_flip(
) {
    use crate::widget_class_source_0950::standard_macros::CellInspection as C;
    let (fixture, site, _) = family_fixture(rvn_graph::NodeKind::FlipFlop, false, false);
    let mut engine = fixture.engine();
    let handle = engine
        .create_widget(A, "macro_family", vec![Value::Str("allowed".into())])
        .unwrap();
    engine.add_widget_to_viewport(&handle, false, 0).unwrap();
    assert_eq!(
        family_cell(&engine, &handle, site),
        C::FlipFlop { is_a: false }
    );
    family_press(&mut engine, "go");
    assert_eq!(engine.state.vars["branch_a"], Value::Int(1));
    assert_eq!(engine.state.vars["branch_b"], Value::Int(0));
    assert_eq!(
        engine.state.vars["observed_a"],
        Value::Bool(true),
        "wired IsA reads the updated owner cell on A"
    );
    family_publication_refuses(&mut engine, &handle, site, "go");
    assert_eq!(
        family_cell(&engine, &handle, site),
        C::FlipFlop { is_a: true }
    );
    family_press(&mut engine, "go");
    assert_eq!(engine.state.vars["branch_a"], Value::Int(1));
    assert_eq!(engine.state.vars["branch_b"], Value::Int(1));
    assert_eq!(
        engine.state.vars["observed_b"],
        Value::Bool(false),
        "wired IsA reads the updated owner cell on B"
    );
    family_press(&mut engine, "go");
    assert_eq!(engine.state.vars["branch_a"], Value::Int(2));
    assert_eq!(engine.state.vars["branch_b"], Value::Int(1));
    assert_eq!(
        family_cell(&engine, &handle, site),
        C::FlipFlop { is_a: true }
    );
}

#[test]
fn standard_macro1021_gate_actual_widget_start_closed_open_close_toggle_and_refused_toggle_is_atomic(
) {
    use crate::widget_class_source_0950::standard_macros::CellInspection as C;
    for start_closed in [true, false] {
        let (fixture, site, _) = family_fixture(rvn_graph::NodeKind::Gate, start_closed, false);
        let mut engine = fixture.engine();
        let handle = engine
            .create_widget(A, "macro_family", vec![Value::Str("allowed".into())])
            .unwrap();
        engine.add_widget_to_viewport(&handle, false, 0).unwrap();
        let peer = engine
            .create_widget(A, "macro_peer", vec![Value::Str("allowed".into())])
            .unwrap();
        engine.add_widget_to_viewport(&peer, false, 0).unwrap();
        assert_eq!(
            family_cell(&engine, &handle, site),
            C::Gate {
                initialized: false,
                open: false
            }
        );
        family_press_at(&mut engine, "macro_peer", "open");
        assert_eq!(
            family_cell(&engine, &peer, site),
            C::Gate {
                initialized: true,
                open: true
            },
            "first Open shares the real lazy initializer then opens"
        );
        assert_eq!(
            engine.state.vars["hits"],
            Value::Int(0),
            "Open emits no Exit"
        );
        family_press(&mut engine, "go");
        let initial_hits = if start_closed { 0 } else { 1 };
        assert_eq!(engine.state.vars["hits"], Value::Int(initial_hits));
        family_press(&mut engine, "close");
        family_press(&mut engine, "go");
        assert_eq!(engine.state.vars["hits"], Value::Int(initial_hits));
        assert_eq!(
            family_cell(&engine, &handle, site),
            C::Gate {
                initialized: true,
                open: false
            }
        );
        family_press(&mut engine, "open");
        assert_eq!(engine.state.vars["hits"], Value::Int(initial_hits));
        family_press(&mut engine, "go");
        assert_eq!(engine.state.vars["hits"], Value::Int(initial_hits + 1));
        family_press(&mut engine, "toggle");
        assert_eq!(
            family_cell(&engine, &handle, site),
            C::Gate {
                initialized: true,
                open: false
            }
        );
        family_press(&mut engine, "go");
        assert_eq!(engine.state.vars["hits"], Value::Int(initial_hits + 1));
        family_publication_refuses(&mut engine, &handle, site, "toggle");
        family_press(&mut engine, "toggle");
        assert_eq!(
            family_cell(&engine, &handle, site),
            C::Gate {
                initialized: true,
                open: true
            }
        );
        assert_eq!(
            engine.state.vars["hits"],
            Value::Int(initial_hits + 1),
            "Toggle emits no Exit"
        );
        family_press(&mut engine, "go");
        assert_eq!(engine.state.vars["hits"], Value::Int(initial_hits + 2));
        assert_eq!(
            family_cell(&engine, &peer, site),
            C::Gate {
                initialized: true,
                open: true
            },
            "other actual instance retained its independent cell"
        );
    }
}
// Actual Sequence predecessors visit the SAME catalogue macro node twice in
// one accepted Widget Input. No synthetic Range or free source identity.
fn shared_ingress_fixture(kind: rvn_graph::NodeKind) -> (Fixture, (u64, u64)) {
    use rvn_graph::{NodeKind as K, PropertyValue as P};
    let tail = if kind == K::FlipFlop {
        "set hits=hits+1 set branch_b=branch_b+1"
    } else {
        "set hits=hits+1"
    };
    let authored = format!("handler allowed(event){{flow.sequence(){{{{set ingress_a=0}}{{set ingress_b=0}}}} {tail}}}");
    let mut graph = rvn_graph::import_script(&rvn_parser::parse(&authored).unwrap())
        .unwrap()
        .into_iter()
        .find(|g| matches!(&g.kind,rvn_graph::GraphKind::Handler{name}if name=="allowed"))
        .unwrap();
    let assignment = |graph: &rvn_graph::GraphDocument, name: &str| {
        let nodes = graph
            .nodes
            .values()
            .filter(|n| {
                n.kind == K::SetVariable
                    && n.properties.get("name") == Some(&P::String(name.into()))
            })
            .map(|n| n.id)
            .collect::<Vec<_>>();
        assert_eq!(nodes.len(), 1);
        nodes[0]
    };
    let ends = graph
        .nodes
        .values()
        .filter(|n| n.kind == K::BranchEnd)
        .map(|n| n.id)
        .collect::<Vec<_>>();
    for end in ends {
        graph.remove_node(end).unwrap();
    }
    let node = graph.add_catalog_node(kind, [500.0, 0.0]).unwrap();
    if kind == K::DoN {
        graph.set_pin_default(node, "n", P::Int(3)).unwrap();
    }
    if kind == K::DoOnce || kind == K::Gate {
        graph
            .set_pin_default(node, "start_closed", P::Bool(false))
            .unwrap();
    }
    let enter = graph.pin_by_key(node, "exec_in").unwrap().id;
    for name in ["ingress_a", "ingress_b"] {
        let marker = assignment(&graph, name);
        let input = graph.pin_by_key(marker, "exec_in").unwrap().id;
        let incoming = graph
            .edges
            .values()
            .filter(|e| e.input == input)
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(incoming.len(), 1);
        graph.remove_node(marker).unwrap();
        graph.connect(incoming[0].output, enter).unwrap();
    }
    let hits = assignment(&graph, "hits");
    let hit_input = graph.pin_by_key(hits, "exec_in").unwrap().id;
    let old = graph
        .edges
        .values()
        .find(|e| e.input == hit_input)
        .unwrap()
        .id;
    graph.remove_edge(old).unwrap();
    let output = match kind {
        K::DoOnce => "completed",
        K::FlipFlop => "a",
        K::DoN | K::Gate => "exit",
        _ => panic!("only four macro families"),
    };
    graph
        .connect(graph.pin_by_key(node, output).unwrap().id, hit_input)
        .unwrap();
    if kind == K::FlipFlop {
        let b = assignment(&graph, "branch_b");
        let b_input = graph.pin_by_key(b, "exec_in").unwrap().id;
        let old = graph
            .edges
            .values()
            .find(|e| e.input == b_input)
            .unwrap()
            .id;
        graph.remove_edge(old).unwrap();
        graph
            .connect(graph.pin_by_key(node, "b").unwrap().id, b_input)
            .unwrap();
    }
    assert!(
        graph.nodes.values().all(|n| n.kind != K::ForLoopWithBreak),
        "no unrelated Range earns the source capability"
    );
    let legacy = serde_json::to_vec(&graph).unwrap();
    assert!(graph.enable_exec_links_source_preparation().is_err());
    assert_eq!(
        serde_json::to_vec(&graph).unwrap(),
        legacy,
        "legacy refusal before mutation"
    );
    graph.enable_native_frame_preparation().unwrap();
    let schema = graph.schema_version;
    let before = graph.clone();
    assert!(
        rvn_graph::prepare_native_frame_graph(&before).is_err(),
        "scalar source cannot encode two genuine incoming edges"
    );
    // An external signature-container claim is data, not a downgrade grant.
    // Actual Unit14 roundtrip remains Unit's independent qualification scope.
    let mut external = serde_json::to_value(&before).unwrap();
    external["schema_version"] = serde_json::json!(14);
    let mut unadmitted: rvn_graph::GraphDocument = serde_json::from_value(external).unwrap();
    let blocked = serde_json::to_vec(&unadmitted).unwrap();
    assert!(unadmitted.enable_exec_links_source_preparation().is_err());
    assert_eq!(
        serde_json::to_vec(&unadmitted).unwrap(),
        blocked,
        "signature schema14 is neither admitted nor downgraded"
    );
    graph.enable_exec_links_source_preparation().unwrap();
    assert_eq!(graph.schema_version, schema);
    assert_eq!(graph.pins, before.pins);
    assert_eq!(graph.edges, before.edges);
    let generated = rvn_graph::prepare_native_frame_graph(&graph).unwrap();
    assert!(generated.source.contains("!exec_links_v1"));
    let site = (graph.graph_id.get(), node.get());
    let fixture = Fixture::with_source_transform(false, false, false, false, |source| {
        let old = "handler allowed(event){local roll=random(1,100) while false{set roll=roll+1} set hits=hits+1}\n";
        assert_eq!(source.matches(old).count(), 1);
        source.replacen(old, &generated.source, 1).replacen(
            "init{set hits=0}",
            "init{set hits=0 set branch_b=0}",
            1,
        )
    });
    (fixture, site)
}
fn shared_ingress_once(
    kind: rvn_graph::NodeKind,
) -> (
    Fixture,
    Engine<Host>,
    crate::engine::WidgetInstanceHandle,
    (u64, u64),
) {
    let (fixture, site) = shared_ingress_fixture(kind);
    let mut engine = fixture.engine();
    let handle = engine
        .create_widget(A, "macro_shared", vec![Value::Str("allowed".into())])
        .unwrap();
    engine.add_widget_to_viewport(&handle, false, 0).unwrap();
    click(&mut engine, "macro_shared");
    (fixture, engine, handle, site)
}
#[test]
fn standard_macro1021_shared_do_once_actual_sequence_two_ingresses_consume_one_cell() {
    use crate::widget_class_source_0950::standard_macros::CellInspection as C;
    let (_fixture, mut engine, handle, site) = shared_ingress_once(rvn_graph::NodeKind::DoOnce);
    assert_eq!(engine.state.vars["hits"], Value::Int(1));
    click(&mut engine, "macro_shared");
    assert_eq!(engine.state.vars["hits"], Value::Int(1));
    assert_eq!(
        engine
            .inspect_widget_macro_cell(&handle, site.0, site.1)
            .unwrap()
            .1,
        C::DoOnce {
            initialized: true,
            completed: true
        }
    );
}
#[test]
fn standard_macro1021_shared_do_n_actual_sequence_counts_two_visits_then_saturates() {
    use crate::widget_class_source_0950::standard_macros::CellInspection as C;
    let (_fixture, mut engine, handle, site) = shared_ingress_once(rvn_graph::NodeKind::DoN);
    assert_eq!(engine.state.vars["hits"], Value::Int(2));
    assert_eq!(
        engine
            .inspect_widget_macro_cell(&handle, site.0, site.1)
            .unwrap()
            .1,
        C::DoN {
            initialized: true,
            counter: 2
        }
    );
    click(&mut engine, "macro_shared");
    assert_eq!(engine.state.vars["hits"], Value::Int(3));
    assert_eq!(
        engine
            .inspect_widget_macro_cell(&handle, site.0, site.1)
            .unwrap()
            .1,
        C::DoN {
            initialized: true,
            counter: 3
        }
    );
}
#[test]
fn standard_macro1021_shared_flip_flop_actual_sequence_a_then_b_same_owner() {
    use crate::widget_class_source_0950::standard_macros::CellInspection as C;
    let (_fixture, mut engine, handle, site) = shared_ingress_once(rvn_graph::NodeKind::FlipFlop);
    assert_eq!(engine.state.vars["hits"], Value::Int(1));
    assert_eq!(engine.state.vars["branch_b"], Value::Int(1));
    assert_eq!(
        engine
            .inspect_widget_macro_cell(&handle, site.0, site.1)
            .unwrap()
            .1,
        C::FlipFlop { is_a: false }
    );
    click(&mut engine, "macro_shared");
    assert_eq!(engine.state.vars["hits"], Value::Int(2));
    assert_eq!(engine.state.vars["branch_b"], Value::Int(2));
}
#[test]
fn standard_macro1021_shared_gate_actual_sequence_open_enter_twice_same_cell() {
    use crate::widget_class_source_0950::standard_macros::CellInspection as C;
    let (_fixture, mut engine, handle, site) = shared_ingress_once(rvn_graph::NodeKind::Gate);
    assert_eq!(engine.state.vars["hits"], Value::Int(2));
    assert_eq!(
        engine
            .inspect_widget_macro_cell(&handle, site.0, site.1)
            .unwrap()
            .1,
        C::Gate {
            initialized: true,
            open: true
        }
    );
    click(&mut engine, "macro_shared");
    assert_eq!(engine.state.vars["hits"], Value::Int(4));
}

// Real Graph authoring + existing source/class fixture, appended after F12.
fn connected_fixture(kind: rvn_graph::NodeKind, expression: &str, shared: bool, share_random: bool) -> (Fixture, (u64,u64), rvn_graph::GraphDocument) {
    use rvn_graph::NodeKind as K;
    let controls = match kind {
        K::DoOnce|K::DoN => "if event[\"element\"]==\"reset\"{set route_reset=0}else{set route_enter=0 set hits=hits+1}",
        K::Gate => "if event[\"element\"]==\"open\"{set route_open=0}else{if event[\"element\"]==\"close\"{set route_close=0}else{if event[\"element\"]==\"toggle\"{set route_toggle=0}else{set route_enter=0 set hits=hits+1}}}",
        _=>panic!("connected N/StartClosed families only"),
    };
    let raw=format!("init{{set limit=1 set closed=true}} handler allowed(event){{set operand_probe={expression} if event[\"element\"]==\"increase\"{{set limit=limit+1}}else{{if event[\"element\"]==\"decrease\"{{set limit=limit-1}}else{{if event[\"element\"]==\"change\"{{set closed=not closed}}else{{{controls}}}}}}}}}");
    let mut graph=rvn_graph::import_script(&rvn_parser::parse(&raw).unwrap()).unwrap().into_iter().find(|g|matches!(&g.kind,rvn_graph::GraphKind::Handler{name}if name=="allowed")).unwrap();
    let probe=family_assignment(&graph,"operand_probe");
    let value=graph.pin_by_key(probe,"value").unwrap().id;
    let producer=if let Some(output)=graph.edges.values().find(|e|e.input==value).map(|e|e.output){output}else{
        // The importer keeps a scalar assignment inline. Author its genuine
        // Literal node before removing the probe, so this negative case still
        // exercises the connected Reroute -> DoN operand path with the same N.
        let constant=graph.pins[&value].default_value.clone().expect("actual imported scalar assignment default");
        let literal=graph.add_catalog_node(K::Literal,[0.0,100.0]).unwrap();
        graph.set_property(literal,"value",constant).unwrap();
        graph.pin_by_key(literal,"value").unwrap().id
    };
    let input=graph.pin_by_key(probe,"exec_in").unwrap().id;
    let output=graph.pin_by_key(probe,"exec_out").unwrap().id;
    let prior=graph.edges.values().find(|e|e.input==input).unwrap().output;
    let next=graph.edges.values().find(|e|e.output==output).unwrap().input;
    graph.remove_node(probe).unwrap();graph.connect(prior,next).unwrap();
    let ends=graph.nodes.values().filter(|n|n.kind==K::BranchEnd).map(|n|n.id).collect::<Vec<_>>();
    for end in ends{graph.remove_node(end).unwrap();}
    let node=graph.add_catalog_node(kind,[500.0,0.0]).unwrap();
    family_replace_route(&mut graph,"route_enter",node,"exec_in",Some(if kind==K::DoOnce{"completed"}else{"exit"}));
    if kind==K::Gate{for key in["open","close","toggle"]{family_replace_route(&mut graph,&format!("route_{key}"),node,key,None);}}
    else{family_replace_route(&mut graph,"route_reset",node,"reset",None);}
    if share_random {
        let sum=graph.pins[&producer].node;
        let left=graph.pin_by_key(sum,"left").unwrap().id;
        let right=graph.pin_by_key(sum,"right").unwrap().id;
        let random=graph.edges.values().find(|e|e.input==left).unwrap().output;
        let old=graph.edges.values().find(|e|e.input==right).unwrap().clone();let literal=graph.pins[&old.output].node;
        graph.remove_edge(old.id).unwrap();graph.remove_node(literal).unwrap();graph.connect(random,right).unwrap();
    }
    let reroute=graph.add_catalog_node(K::Reroute,[300.0,100.0]).unwrap();
    graph.connect(producer,graph.pin_by_key(reroute,"value").unwrap().id).unwrap();
    let routed=graph.pin_by_key(reroute,"value_out").unwrap().id;
    graph.connect(routed,graph.pin_by_key(node,if kind==K::DoN{"n"}else{"start_closed"}).unwrap().id).unwrap();
    if shared {
        let enter=graph.pin_by_key(node,"exec_in").unwrap().id;
        let old=graph.edges.values().find(|e|e.input==enter).unwrap().clone();graph.remove_edge(old.id).unwrap();
        let sequence=graph.add_catalog_node(K::Sequence,[200.0,0.0]).unwrap();
        graph.connect(old.output,graph.pin_by_key(sequence,"exec_in").unwrap().id).unwrap();
        family_wire(&mut graph,sequence,"then_0",node,"exec_in");family_wire(&mut graph,sequence,"then_1",node,"exec_in");
    }
    graph.enable_native_frame_preparation().unwrap();if shared{graph.enable_exec_links_source_preparation().unwrap();}
    let generated=rvn_graph::prepare_native_frame_graph(&graph).unwrap();
    assert!(generated.source.contains("operand_v1 {"));
    let port=graph.pin_by_key(node,if kind==K::DoN{"n"}else{"start_closed"}).unwrap();
    assert!(generated.source.contains(&format!("{}={}@",port.key,port.id)));
    assert!(generated.source.contains(&format!("~{routed}")),"retain actual Reroute output pin, never flatten");
    let site=(graph.graph_id.get(),node.get());
    let fixture=Fixture::with_source_transform(false,false,false,false,|source|{
        let old="handler allowed(event){local roll=random(1,100) while false{set roll=roll+1} set hits=hits+1}\n";
        assert_eq!(source.matches(old).count(),1);let source=source.replacen(old,&generated.source,1);
        let old_panel="screen panel(method){return component(\"go\",\"button\",{\"events\":{\"click\":method}},[])}";
        let buttons=["go","reset","open","close","toggle","increase","decrease","change"].iter().map(|id|format!("component(\"{id}\",\"button\",{{\"events\":{{\"click\":method}}}},[])")).collect::<Vec<_>>().join(",");
        assert_eq!(source.matches(old_panel).count(),1);
        source.replacen(old_panel,&format!("screen panel(method){{return component(\"root\",\"column\",{{}},[{buttons}])}}"),1).replacen("init{set hits=0}","init{set hits=0 set limit=1 set closed=true}",1)
    });
    (fixture,site,graph)
}
fn connected_open(f:&Fixture)->(Engine<Host>,crate::engine::WidgetInstanceHandle){
    let mut e=f.engine();let h=e.create_widget(A,"macro_family",vec![Value::Str("allowed".into())]).unwrap();e.add_widget_to_viewport(&h,false,0).unwrap();(e,h)
}
fn connected_random(e:&Engine<Host>)->(crate::random::RandomState,crate::random::RandomState){(e.state.random,e.state.ui.screens.iter().find(|i|i.name=="macro_family").unwrap().random)}
fn connected_expect_draws(before:(crate::random::RandomState,crate::random::RandomState),after:(crate::random::RandomState,crate::random::RandomState),draws:usize){
    let(mut story,mut ui)=before;for _ in 0..draws{story.integer(1,1).unwrap();ui.integer(1,1).unwrap();}
    if draws==0{assert_eq!(after,before);}else{assert!(after==(story,before.1)||after==(before.0,ui),"exactly {draws} draws in the real routed Story/UI stream: {before:?} -> {after:?}");}
}
#[test]
fn standard_macro1021_connected_don_live_n_recomputes_on_actual_enter_without_counter_reset(){
    use crate::widget_class_source_0950::standard_macros::CellInspection as C;
    let(f,site,_)=connected_fixture(rvn_graph::NodeKind::DoN,"limit",false,false);let(mut e,h)=connected_open(&f);
    family_press(&mut e,"go");family_press(&mut e,"go");assert_eq!(e.state.vars["hits"],Value::Int(1));
    family_press(&mut e,"increase");family_press(&mut e,"go");assert_eq!(e.state.vars["hits"],Value::Int(2));
    assert_eq!(family_cell(&e,&h,site),C::DoN{initialized:true,counter:2});
    family_press(&mut e,"decrease");family_press(&mut e,"go");assert_eq!(e.state.vars["hits"],Value::Int(2));
    family_press(&mut e,"reset");family_press(&mut e,"decrease");family_press(&mut e,"go");
    assert_eq!(family_cell(&e,&h,site),C::DoN{initialized:true,counter:0});assert_eq!(e.state.vars["hits"],Value::Int(2));
    for expression in["2147483648","random(2,1)"]{
        let(f,site,_)=connected_fixture(rvn_graph::NodeKind::DoN,expression,false,false);let(mut bad,h)=connected_open(&f);
        let state=bad.state.clone();let cell=family_cell(&bad,&h,site);let random=bad.host_random.clone();let epoch=bad.runtime_epoch();let batches=bad.renderer.batches.lock().unwrap().len();
        assert!(bad.interface_event_scoped(family_input(&bad,"macro_family","go")).is_err());
        assert_eq!(bad.state.vars,state.vars);assert_eq!(bad.state.random,state.random);assert_eq!(bad.host_random,random);assert_eq!(family_cell(&bad,&h,site),cell);assert_eq!(bad.runtime_epoch(),epoch);assert_eq!(bad.renderer.batches.lock().unwrap().len(),batches);
        assert_eq!(bad.state.ui.screens.iter().map(|i|i.random).collect::<Vec<_>>(),state.ui.screens.iter().map(|i|i.random).collect::<Vec<_>>());
    }
}
#[test]
fn standard_macro1021_connected_don_fan_in_random_dag_is_fresh_and_renderer_refusal_atomic(){
    let(f,site,_)=connected_fixture(rvn_graph::NodeKind::DoN,"random(1,1)+0",true,true);let(mut e,h)=connected_open(&f);
    family_publication_refuses(&mut e,&h,site,"go");
    let before=connected_random(&e);family_press(&mut e,"go");connected_expect_draws(before,connected_random(&e),2);assert_eq!(e.state.vars["hits"],Value::Int(2));
    let before=connected_random(&e);family_press(&mut e,"go");connected_expect_draws(before,connected_random(&e),2);assert_eq!(e.state.vars["hits"],Value::Int(2),"exhausted visits still demand fresh N");
}
#[test]
fn standard_macro1021_connected_do_once_skips_initialized_random_and_reset_before_first_enter(){
    let(f,site,_)=connected_fixture(rvn_graph::NodeKind::DoOnce,"random(1,1)>1",true,false);let(mut e,h)=connected_open(&f);
    family_publication_refuses(&mut e,&h,site,"go");
    let before=connected_random(&e);family_press(&mut e,"go");connected_expect_draws(before,connected_random(&e),1);assert_eq!(e.state.vars["hits"],Value::Int(1));
    let after=connected_random(&e);family_press(&mut e,"go");family_press(&mut e,"reset");family_press(&mut e,"go");assert_eq!(connected_random(&e),after);assert_eq!(e.state.vars["hits"],Value::Int(2));
    let(mut fresh,_)=connected_open(&f);let before=connected_random(&fresh);family_press(&mut fresh,"reset");family_press(&mut fresh,"go");assert_eq!(connected_random(&fresh),before);assert_eq!(fresh.state.vars["hits"],Value::Int(1));
}
#[test]
fn standard_macro1021_connected_gate_first_control_samples_once_then_ignores_changed_start_closed(){
    for first in["go","open","close","toggle"]{
        let(f,site,_)=connected_fixture(rvn_graph::NodeKind::Gate,"closed or (random(1,1)>1)",false,false);let(mut e,h)=connected_open(&f);
        family_publication_refuses(&mut e,&h,site,first);
        let before=connected_random(&e);family_press(&mut e,first);connected_expect_draws(before,connected_random(&e),1);
        let after=connected_random(&e);family_press(&mut e,"change");family_press(&mut e,"go");assert_eq!(connected_random(&e),after);
        assert_eq!(e.state.vars["hits"],Value::Int(if first=="open"||first=="toggle"{1}else{0}));
        family_press(&mut e,"open");family_press(&mut e,"go");assert_eq!(connected_random(&e),after);
    }
}
#[test]
fn standard_macro1021_connected_source_pin_forgery_and_real_data_cycle_are_refused(){
    use rvn_graph::NodeKind as K;
    let(_,site,graph)=connected_fixture(K::DoN,"limit",false,false);let compiled=rvn_graph::prepare_native_frame_graph(&graph).unwrap();
    let mut parsed=compiled.ast.clone();let mut found=false;
    fn tamper(body:&mut[Statement],found:&mut bool){for s in body{match s{Statement::StandardMacroDispatch{dispatch}=>{if dispatch.deferred_operand_v1.is_some(){let p=dispatch.site.node.ports.iter_mut().find(|p|p.role=="n").unwrap();p.source_pin=Some(dispatch.site.root.ports.iter().find(|p|p.role=="exec_out").unwrap().pin);assert!(dispatch.validate().is_err());*found=true;}},Statement::Handler{body,..}|Statement::ScheduledConsumer{body,..}=>tamper(body,found),Statement::If{then_branch,else_branch,..}=>{tamper(then_branch,found);tamper(else_branch,found);},_=>{}}}}
    tamper(&mut parsed,&mut found);assert!(found);
    let mut cycle=graph.clone();let node=rvn_graph::NodeId::new(site.1);let n=cycle.pin_by_key(node,"n").unwrap().id;
    let edge=cycle.edges.values().find(|e|e.input==n).unwrap().clone();let reroute=cycle.pins[&edge.output].node;let input=cycle.pin_by_key(reroute,"value").unwrap().id;
    let previous=cycle.edges.values().find(|e|e.input==input).unwrap().id;cycle.remove_edge(previous).unwrap();cycle.remove_edge(edge.id).unwrap();
    cycle.connect(cycle.pin_by_key(node,"counter").unwrap().id,input).unwrap();let before=serde_json::to_vec(&cycle).unwrap();
    assert!(cycle.connect(edge.output,n).is_err(),"indirect self-data cycle remains the real Graph refusal; UE indirect semantics are not admitted");assert_eq!(serde_json::to_vec(&cycle).unwrap(),before);
}
#[test]
fn standard_macro1021_connected_actual_producer_debug_replays_hot_exact_and_rejects_pending_cold_save(){
    let(f,_,_)=connected_fixture(rvn_graph::NodeKind::DoN,"random(1,1)+0",true,true);let(mut e,h)=connected_open(&f);let before=e.state.clone();
    let mut session=e.debug_begin(DebugOperation::Input(family_input(&e,"macro_family","go")),vec![]).unwrap();let mut stopped=false;
    for _ in 0..512{
        if session.snapshot().site.is_some_and(|site|matches!(site.route.into_iter().last(),Some(crate::editor_debugger::Route::Producer(index))if index>=1)){
            let trace=e.actual_widget_macro_trace_for_test(&session,&h).unwrap();
            // Other real ScheduledConsumers also expose Producer observations.
            // Wait for this actual stopped trace's deferred macro cursor.
            if trace.at_deferred_prefix_for_test(){
                session.assert_actual_deferred_replay_for_test(&trace);
                let error=e.capture_save15(&f.program,Some(&session),1,"connected pending macro refusal".into()).unwrap_err();assert!(error.to_string().contains("pending Widget macro owner"));stopped=true;break;
            }
        }
        let(progress,_)=e.debug_step(&mut session,crate::editor_debugger::StepKind::Into).unwrap();assert_ne!(progress,DebugProgress::Complete);
    }
    assert!(stopped,"a real deferred Producer source observation must be reachable");assert_eq!(e.state.vars,before.vars);assert_eq!(e.state.random,before.random);assert_eq!(e.state.ui.screens.iter().map(|i|&i.standard_macro_frame).collect::<Vec<_>>(),before.ui.screens.iter().map(|i|&i.standard_macro_frame).collect::<Vec<_>>());
    drop(session);let before=connected_random(&e);family_press(&mut e,"go");connected_expect_draws(before,connected_random(&e),2);
}
