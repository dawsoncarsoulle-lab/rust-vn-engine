//! Six actual Graph migration/export -> Engine owner -> stopped Save15/cold paths.
//! Prepared only. No handwritten consume_v3, source-owner, Probe, permit or outcome.
use rvn_core::{
    consumer_schedule::HostDefinitionKind,
    editor_debugger as wire,
    engine::{DebugOperation, DebugProgress, DebugSession},
    random::RandomState,
    renderer::SceneRenderBatch,
    types::SpriteState,
    Engine, Renderer,
};
use rvn_graph::*;
use rvn_parser::{Hotspot, ParsedProgram, Position, Transition, Value};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
};

const SEED: u64 = 852;

/// Real Renderer boundary used by Engine's planning/publication APIs. It does
/// not fabricate UI input or runtime authority. All input comes from Engine.
#[derive(Clone, Default)]
struct Host {
    batches: Arc<Mutex<Vec<SceneRenderBatch>>>,
    reject: Arc<AtomicBool>,
}
impl Renderer for Host {
    fn supports_programmable_ui(&self) -> bool { true }
    fn validate_scene_batch(&self, _: &SceneRenderBatch) -> Result<(), String> {
        if self.reject.load(Ordering::SeqCst) {
            Err("0852 renderer rejects this candidate".into())
        } else {
            Ok(())
        }
    }
    fn publish_scene_batch(&mut self, batch: SceneRenderBatch) {
        self.batches.lock().unwrap().push(batch);
    }
    fn update_interfaces(&mut self, _: &[rvn_ui::programmable::ScreenView]) -> Result<(), String> { Ok(()) }
    fn set_background(&mut self, _: &str, _: &Transition) {}
    fn show_sprite(&mut self, _: &str, _: Option<&str>, _: &Position, _: &Transition, _: Option<&SpriteState>) {}
    fn hide_sprite(&mut self, _: &str, _: &Transition, _: &SpriteState) {}
    fn move_sprite(&mut self, _: &str, _: &Position, _: &Transition, _: &SpriteState) {}
    fn show_dialogue(&mut self, _: Option<&str>, _: &str) {}
    fn show_choice(&mut self, _: &[String]) -> usize { panic!("0852 must not invent a choice") }
    fn show_imagemap(&mut self, _: &str, _: Option<&str>, _: &[Hotspot]) -> usize { panic!("0852 must not invent a hotspot") }
    fn music_play(&mut self, _: &str, _: &Transition, _: Option<&str>) {}
    fn music_stop(&mut self, _: &Transition) {}
    fn music_set_volume(&mut self, _: f32) {}
    fn sfx_play(&mut self, _: &str, _: &Transition) {}
    fn sfx_stop(&mut self, _: &str, _: &Transition) {}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Context { Function, Handler, Screen }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Loop { Array, Range }
impl Loop {
    fn node(self) -> NodeKind {
        match self { Self::Array => NodeKind::ForEach, Self::Range => NodeKind::ForLoopWithBreak }
    }
    fn role(self) -> &'static str {
        match self { Self::Array => "array_element", Self::Range => "index" }
    }
    fn expected(self) -> i64 { match self { Self::Array => 33, Self::Range => 2 } }
    fn demand_draws(self) -> usize {
        // Array: Test x4 + demanded ArrayElement x3. Range: First x1 + Last x4.
        match self { Self::Array => 7, Self::Range => 5 }
    }
    fn helper(self) -> &'static str {
        match self { Self::Array => "supplier0852", Self::Range => "first0852" }
    }
    fn helpers(self) -> &'static [&'static str] {
        match self { Self::Array => &["supplier0852"], Self::Range => &["first0852", "last0852"] }
    }
    fn expected_calls(self) -> BTreeMap<String, usize> {
        match self {
            Self::Array => [("supplier0852".into(), 7)].into(),
            Self::Range => [("first0852".into(), 1), ("last0852".into(), 4)].into(),
        }
    }
}
fn authored(context: Context, kind: Loop) -> &'static str {
    match (context, kind) {
        (Context::Function, Loop::Array) => include_str!("fixtures/lexical_runtime0852/function_array.rvn"),
        (Context::Function, Loop::Range) => include_str!("fixtures/lexical_runtime0852/function_range.rvn"),
        (Context::Handler, Loop::Array) => include_str!("fixtures/lexical_runtime0852/handler_array.rvn"),
        (Context::Handler, Loop::Range) => include_str!("fixtures/lexical_runtime0852/handler_range.rvn"),
        (Context::Screen, Loop::Array) => include_str!("fixtures/lexical_runtime0852/screen_array.rvn"),
        (Context::Screen, Loop::Range) => include_str!("fixtures/lexical_runtime0852/screen_range.rvn"),
    }
}
fn fixture_root(context: Context, kind: Loop) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let parent = PathBuf::from(std::env::var("RVN_GRAPH_LEXICAL0852_FIXTURES")
        .expect("Root must supply a fresh isolated D fixture-output directory"));
    let root = parent.join(format!("{context:?}-{kind:?}-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    std::fs::create_dir_all(&root).unwrap();
    root
}

fn program(context: Context, kind: Loop) -> ParsedProgram {
    let raw = authored(context, kind);
    assert!(!raw.contains("consume_v3"), "only actual Graph export may emit the Native v3 wire");
    let mut graphs = import_script(&rvn_parser::parse(raw).unwrap()).unwrap();
    for graph in &mut graphs { graph.enable_native_frame_preparation().unwrap(); }
    let index = graphs.iter().position(|g| match &g.kind {
        GraphKind::Function { name } => context == Context::Function && name == "target0852",
        GraphKind::Handler { name } => context == Context::Handler && name == "target0852",
        GraphKind::Screen { name } => context == Context::Screen && name == "target0852",
        _ => false,
    }).unwrap();
    let owner = graphs[index].nodes.values().find(|n| n.kind == kind.node()).unwrap().id;
    let before = graphs[index].to_pretty_json().unwrap();
    let migration = graphs[index].prepare_native_loop_demand(owner)
        .expect("requires the genuine lexical Graph0840 migration gate");
    assert_eq!(graphs[index].to_pretty_json().unwrap(), before);
    let [legacy_read]=migration.legacy_named_reads.as_slice() else {panic!("one actual legacy loop value read is required")};
    let legacy_read=*legacy_read;
    assert_eq!(migration.graph.nodes[&legacy_read].kind,NodeKind::VariableGet);
    assert_eq!(migration.graph.variable_node_name(legacy_read).as_deref(),Some(if kind==Loop::Array{"item"}else{"i"}));
    graphs[index] = migration.graph;
    let graph = &mut graphs[index];
    let output = graph.pin_by_key(owner, kind.role()).unwrap().id;
    let setter = graph.nodes.values().find(|n| n.kind == NodeKind::SetVariable
        && graph.variable_node_name(n.id).as_deref() == Some("seen")).unwrap().id;
    let input = graph.pin_by_key(setter, "value").unwrap().id;
    let removed: Vec<_> = graph.edges.values().filter(|e| e.input == input)
        .map(|e| (e.id, graph.pins[&e.output].node)).collect();
    assert_eq!(removed.len(),1);
    assert_eq!(removed[0].1,legacy_read);
    assert_eq!(graph.pins[&input].default_value,None,"replacing a real GET must not conceal an old inline operand");
    assert_eq!(graph.variables["seen"].value_type,if kind==Loop::Array{ValueType::Any}else{ValueType::Int});
    for (edge, _) in &removed { graph.edges.remove(edge); }
    graph.connect(output, input).unwrap();
    for (_, node) in removed {
        let n = &graph.nodes[&node];
        if matches!(n.kind, NodeKind::Literal | NodeKind::VariableGet)
            && !n.pins.iter().any(|pin| graph.edges.values().any(|e| e.input == *pin || e.output == *pin)) {
            graph.remove_node(node).unwrap();
        }
    }
    assert!(!graph.nodes.contains_key(&legacy_read));
    assert_eq!(graph.pins[&input].default_value,None);
    let graph_id = graph.graph_id;
    let nodes: Vec<_> = graph.nodes.keys().copied().collect();
    let pins = graph.pins.clone();
    let edges = graph.edges.clone();
    let frozen = graphs.clone();
    let compiled = transpile_project_with_origins(&graphs).unwrap();
    assert_eq!(graphs, frozen, "export must not rewrite the admitted graph");
    assert!(compiled.source.contains("flow.consume_v3("));
    assert_eq!(rvn_parser::parse(&compiled.source).unwrap(), compiled.ast);
    let root = fixture_root(context, kind);
    std::fs::write(root.join("authored-before-migration.rvn"), raw).unwrap();
    for graph in &graphs {
        std::fs::write(root.join(format!("graph-{}.json", graph.graph_id.get())), graph.to_pretty_json().unwrap()).unwrap();
    }
    let file = root.join("actual-export.rvn");
    std::fs::write(&file, &compiled.source).unwrap();
    let program = rvn_parser::parse_file_program_with_uses(&file).unwrap();
    let rebuilt = import_native_frame_script(&program.script).unwrap();
    let actual = rebuilt.iter().find(|g| g.graph_id == graph_id).unwrap();
    assert_eq!(actual.nodes.keys().copied().collect::<Vec<_>>(), nodes);
    assert_eq!(actual.pins, pins);
    assert_eq!(actual.edges, edges);
    assert_eq!(actual.nodes[&owner].native_loop_contract, Some(NativeLoopContract::DemandV1));
    assert_eq!(transpile_project(&rebuilt).unwrap().ast, compiled.ast);
    let map = NativeDebugSourceMap::from_program(&program).unwrap();
    assert!(!map.endpoint(&file.canonicalize().unwrap(), graph_id, owner, output).is_empty());
    program
}

fn engine(program: &ParsedProgram) -> Engine<Host> {
    let mut e = Engine::from_program_with_debugger(program.clone(), Host::default(), 16).unwrap();
    e.state.random = RandomState::seeded(SEED);
    e.enter_label("start").unwrap();
    e
}
fn state(e: &Engine<Host>) -> Vec<u8> { serde_json::to_vec(&e.state).unwrap() }
fn batches(e: &Engine<Host>) -> usize { e.renderer.batches.lock().unwrap().len() }
fn expected_random(before: RandomState, count: usize) -> RandomState {
    let mut random = before;
    for _ in 0..count { random.integer(1, 1).unwrap(); }
    random
}
fn supplier_breakpoints(e: &Engine<Host>, kind: Loop) -> Vec<wire::Breakpoint> {
    // The route/position are offered by the real SourceMap catalogue, not
    // reconstructed from a hoped-for function/statement index or saved JSON.
    let mut offset = 0;
    let mut found = BTreeMap::new();
    loop {
        let page = e.debug_site_page(offset).unwrap();
        for entry in &page.entries {
            if let Some(site) = &entry.breakpoint.site {
                if site.statement && site.function.as_deref().is_some_and(|name| kind.helpers().contains(&name)) {
                    assert!(entry.origin.is_some());
                    found.entry(site.function.clone().unwrap()).or_insert_with(|| entry.breakpoint.clone());
                }
            }
        }
        if found.len() == kind.helpers().len() { return found.into_values().collect(); }
        offset += page.entries.len();
        assert!(offset < page.total, "no actual supplier statement site was offered");
    }
}
fn finish(e: &mut Engine<Host>, session: &mut DebugSession) -> BTreeMap<String, usize> {
    let mut calls = BTreeMap::new();
    let first = session.snapshot().site.unwrap().function.unwrap();
    *calls.entry(first).or_insert(0) += 1;
    for _ in 0..128 {
        match e.debug_step(session, wire::StepKind::Continue).unwrap().0 {
            DebugProgress::Complete => return calls,
            DebugProgress::Stopped => {
                let function = session.snapshot().site.unwrap().function.unwrap();
                *calls.entry(function).or_insert(0) += 1;
            }
            DebugProgress::Running => {}
        }
    }
    panic!("0852 finite lexical fixture did not finish through the actual debugger");
}
fn input(e: &Engine<Host>) -> rvn_core::ui_input::ScopedUiInput {
    rvn_core::ui_input::ScopedUiInput {
        source: e.capture_ui_input_source("panel").unwrap(),
        event: rvn_core::ui::UiInput {
            screen: "panel".into(), element: "act".into(),
            kind: rvn_ui::programmable::ScreenEventKind::Click, value: None, key: None,
        },
    }
}
fn assert_output(e: &Engine<Host>, context: Context, kind: Loop) {
    if context == Context::Screen {
        let views = e.interface_views().unwrap();
        let view = views.iter().find(|v| v.name == "target0852").unwrap();
        assert_eq!(view.root.value, serde_json::json!(kind.expected()));
        assert_eq!(view.root.text, "actual screen");
        let published = e.renderer.batches.lock().unwrap();
        let batch = published.last().expect("UiOpen must publish through the real Renderer boundary");
        let presented = batch.interfaces.iter().find(|view| view.name == "target0852").unwrap();
        assert_eq!(presented.root.value, serde_json::json!(kind.expected()));
        let screen = e.state.ui.screens.iter().find(|s| s.name == "target0852").unwrap();
        let owner = &screen.consumer_frames["screen:target0852"].owner;
        assert_eq!(owner.kind, HostDefinitionKind::Screen);
        assert_eq!(owner.definition, "target0852");
        assert!(owner.target.incarnation > 0);
    } else {
        assert_eq!(e.state.vars["observed0852"], Value::Int(kind.expected()));
        if context == Context::Handler {
            let published = e.renderer.batches.lock().unwrap();
            assert_eq!(published.last().expect("actual handler must publish its candidate").state.vars["observed0852"], Value::Int(kind.expected()));
            let screen = e.state.ui.screens.iter().find(|s| s.name == "panel").unwrap();
            let owner = &screen.consumer_frames["handler:target0852"].owner;
            assert_eq!(owner.kind, HostDefinitionKind::Handler);
            assert_eq!(owner.definition, "target0852");
            assert!(owner.target.incarnation > 0);
        }
    }
    assert!(!e.state.vars.keys().any(|key| key.starts_with("rvn_native_")), "private output projections cannot leak into published globals");
}

fn check(context: Context, kind: Loop) {
    let program = program(context, kind);
    let mut e = engine(&program);
    // Only the Handler path opens its ordinary source-authored Screen first.
    // Stop BEFORE Dialogue so no display-seed draw is mixed into loop demand.
    if context == Context::Handler {
        e.step_silent().unwrap();
        assert_eq!(e.state.vars["observed0852"], Value::Int(0));
        assert_eq!(e.state.random, expected_random(RandomState::seeded(SEED), 1), "the genuine ui.open consumes one screen seed");
        assert!(e.state.display_random_pc.is_none());
    }
    let before = state(&e);
    let before_random = e.state.random;
    let publication_count = batches(&e);
    let old_input = (context == Context::Handler).then(|| input(&e));
    let operation = old_input.clone().map(DebugOperation::Input).unwrap_or(DebugOperation::Statement);
    let breakpoints = supplier_breakpoints(&e, kind);
    let mut session = e.debug_begin(operation, breakpoints).unwrap();
    assert_eq!(e.debug_step(&mut session, wire::StepKind::Continue).unwrap().0, DebugProgress::Stopped);
    assert_eq!(session.snapshot().site.as_ref().and_then(|site| site.function.as_deref()), Some(kind.helper()));
    assert_eq!(state(&e), before, "stopped computations cannot publish candidate state/RNG");
    assert_eq!(batches(&e), publication_count);
    let source = e.debug_state_source().unwrap();
    let bytes = e.capture_save15(&program, Some(&session), 85, "actual Graph0852 lexical supplier stop".into()).unwrap().encode().unwrap();
    assert!(e.accepts_debug_state_source(&source));
    assert!(e.accepts_debug_session(&session));
    assert_eq!(state(&e), before, "Save15 capture is observational");
    let checkpoint = rvn_core::save::consumer_checkpoint::Save15::decode(&bytes).unwrap();
    assert_eq!(finish(&mut e, &mut session), kind.expected_calls(), "count every real supplier invocation at its actual catalogue breakpoint");
    assert_output(&e, context, kind);
    let expected_draws = kind.demand_draws() + if context == Context::Screen { 1 } else { 0 };
    assert_eq!(e.state.random, expected_random(before_random, expected_draws), "actual supplier demand, plus the Screen seed only for its ui.open");
    let expected = state(&e);

    let mut cold = engine(&program);
    let mut loaded = cold.restore_save15(&program, checkpoint).unwrap().expect("actual pending continuation must survive cold restore");
    assert!(cold.accepts_debug_session(&loaded));
    assert_eq!(loaded.snapshot().site.as_ref().and_then(|site| site.function.as_deref()), Some(kind.helper()));
    assert_eq!(cold.state.random, before_random, "restoring pending supplier must not consume RNG");
    if let Some(input) = &old_input { assert!(!cold.ui_input_source_current(&input.source)); }
    assert_eq!(finish(&mut cold, &mut loaded), kind.expected_calls(), "cold continuation must not add or skip a supplier invocation");
    assert_output(&cold, context, kind);
    assert_eq!(state(&cold), expected, "cold continuation must equal the real warm publication");
    let after = state(&cold);
    for _ in 0..3 { cold.interface_views().unwrap(); }
    assert_eq!(state(&cold), after, "resident description reads cannot replay supplier RNG");

    // Synchronous public operation is a third, independent route on the SAME
    // Graph-exported program. No expected AST, permit or callback is injected.
    let mut synchronous = engine(&program);
    if context == Context::Handler {
        synchronous.step_silent().unwrap();
        assert!(synchronous.interface_event_scoped(input(&synchronous)).unwrap());
    } else {
        assert!(synchronous.preview_step_statement().unwrap().is_none());
    }
    assert_output(&synchronous, context, kind);
    assert_eq!(state(&synchronous), expected);

    if context != Context::Function {
        let mut refused = engine(&program);
        if context == Context::Handler { refused.step_silent().unwrap(); }
        let initial = state(&refused);
        let epoch = refused.runtime_epoch();
        let count = batches(&refused);
        refused.renderer.reject.store(true, Ordering::SeqCst);
        if context == Context::Handler {
            assert!(refused.interface_event_scoped(input(&refused)).is_err());
        } else {
            assert!(refused.preview_step_statement().is_err());
        }
        assert_eq!(state(&refused), initial, "renderer refusal must retain actual owner, variables, RNG and frames");
        assert_eq!(refused.runtime_epoch(), epoch);
        assert_eq!(batches(&refused), count);
        refused.renderer.reject.store(false, Ordering::SeqCst);
        if context == Context::Handler {
            assert!(refused.interface_event_scoped(input(&refused)).unwrap());
        } else {
            assert!(refused.preview_step_statement().unwrap().is_none());
        }
        assert_output(&refused, context, kind);
        assert_eq!(state(&refused), expected, "a refused candidate must not consume one of the future supplier draws");
    }
}

#[test]
fn graph0852_function_foreach_real_export_supplier_pause_and_cold_returns33() { check(Context::Function, Loop::Array); }
#[test]
fn graph0852_function_range_real_export_supplier_pause_and_cold_returns2() { check(Context::Function, Loop::Range); }
#[test]
fn graph0852_handler_foreach_real_input_publishes33_and_cold_keeps_owner() { check(Context::Handler, Loop::Array); }
#[test]
fn graph0852_handler_range_real_input_publishes2_and_cold_keeps_owner() { check(Context::Handler, Loop::Range); }
#[test]
fn graph0852_screen_foreach_real_ui_open_renders33_and_cold_keeps_owner() { check(Context::Screen, Loop::Array); }
#[test]
fn graph0852_screen_range_real_ui_open_renders2_and_cold_keeps_owner() { check(Context::Screen, Loop::Range); }
