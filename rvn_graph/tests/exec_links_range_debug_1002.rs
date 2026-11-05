//! Real Graph -> frozen .rvn -> EngineStartup -> observed Break source -> wire.
//! Prepared only. No UI/native/pixel or client snapshot authority is asserted.
use rvn_core::{editor_debugger::StepKind, engine::EngineStartup, Engine, TerminalRenderer};
use rvn_graph::*;
use rvn_parser::Value;
use std::{
    collections::BTreeSet,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "rvn-range-debug1002-{}-{nonce}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn program(&self, source: &str) -> rvn_parser::ParsedProgram {
        let file = self.0.join("native.rvn");
        std::fs::write(&file, source).unwrap();
        rvn_parser::parse_file_program_with_uses(file).unwrap()
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let root = self.0.canonicalize().unwrap();
        let temp = std::env::temp_dir().canonicalize().unwrap();
        assert_eq!(root.parent(), Some(temp.as_path()));
        assert!(root
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("rvn-range-debug1002-"));
        let _ = std::fs::remove_dir_all(root);
    }
}
fn compile(raw: &str) -> (Vec<GraphDocument>, String) {
    let mut graphs = import_script(&rvn_parser::parse_program(raw).unwrap().script).unwrap();
    for graph in &mut graphs {
        graph.enable_native_frame_preparation().unwrap();
    }
    let init = graphs
        .iter_mut()
        .find(|g| g.kind == GraphKind::Init)
        .unwrap();
    let ranges: Vec<_> = init
        .nodes
        .values()
        .filter(|n| n.kind == NodeKind::ForLoopWithBreak)
        .map(|n| n.id)
        .collect();
    for range in ranges {
        *init = init.prepare_native_loop_demand(range).unwrap().graph;
    }
    init.enable_exec_links_source_preparation().unwrap();
    init.refresh_variable_usage_bindings().unwrap();
    let source = transpile_project(&graphs).unwrap().source;
    (graphs, source)
}
fn startup(program: rvn_parser::ParsedProgram) -> EngineStartup<TerminalRenderer> {
    Engine::prepare_debugger_startup(
        program,
        TerminalRenderer,
        16,
        Default::default(),
        Default::default(),
        None,
    )
    .unwrap()
}
fn observed(
    owner: &mut EngineStartup<TerminalRenderer>,
    map: &NativeRangeBreakSourceMap,
) -> Vec<NativeRangeBreakVisit> {
    let mut visits = Vec::new();
    for _ in 0..2048 {
        if owner.is_complete() {
            return visits;
        }
        let snapshot = owner.snapshot();
        if let Some(origin) = snapshot.origin {
            if let (Some(source), Some(span)) = (origin.source, origin.span) {
                if let Some(visit) = map.resolve_origin(owner.program(), &source, &span).unwrap() {
                    assert_eq!(
                        &owner.program().sources[visit.source_ordinal].source[span],
                        "flow.break()"
                    );
                    visits.push(visit.clone());
                }
            }
        }
        owner.step(StepKind::Into).unwrap();
    }
    panic!("actual startup did not finish within the bounded fixture");
}

#[test]
fn range_debug1002_actual_startup_selects_each_real_break_wire_without_header_regather() {
    let mut reached = BTreeSet::new();
    for gate in [true, false] {
        let raw = format!("function limit(){{local token=random(1,1) return 3}}\ninit{{set gate={gate} set visits=0 flow.for_loop(i,0,limit()){{set visits=visits+1 if gate{{flow.break()}} if not gate{{flow.break()}}}} set done=true}}\nscene label start\n\"ready\"\n");
        let (graphs, source) = compile(&raw);
        let files = Files::new();
        let program = files.program(&source);
        let map = NativeRangeBreakSourceMap::from_program(&program).unwrap();
        assert_eq!(map.visits().len(), 2);
        assert_ne!(map.visits()[0].edge, map.visits()[1].edge);
        assert_ne!(
            map.visits()[0].predecessor_output,
            map.visits()[1].predecessor_output
        );
        let before = program.clone();
        let frozen_graphs = graphs.clone();
        let mut owner = startup(program);
        let mut expected = owner.state().random;
        expected.integer(1, 1).unwrap();
        let visits = observed(&mut owner, &map);
        let unique: BTreeSet<_> = visits.iter().map(|v| v.edge).collect();
        assert_eq!(
            unique.len(),
            1,
            "only the branch actually reached by the VM has an observed wire"
        );
        let actual = visits
            .first()
            .expect("real LoopBreak debugger source observation");
        assert_eq!(actual, &map.visits()[usize::from(!gate)]);
        reached.insert(actual.edge);
        let graph = graphs.iter().find(|g| g.graph_id == actual.graph).unwrap();
        let edge = &graph.edges[&actual.edge];
        assert_eq!(
            (edge.input, edge.output),
            (actual.input, actual.predecessor_output)
        );
        assert_eq!(graph.pins[&edge.output].node, actual.predecessor);
        assert_eq!(
            owner.state().random,
            expected,
            "Break must not gather Last again"
        );
        assert_eq!(owner.state().vars["visits"], Value::Int(1));
        assert_eq!(owner.state().vars["done"], Value::Bool(true));
        assert_eq!(owner.program(), &before);
        assert_eq!(graphs, frozen_graphs);
        assert!(owner.into_engine().is_ok());
    }
    assert_eq!(
        reached.len(),
        2,
        "second ingress must not be reported as first/min EdgeId"
    );
}

#[test]
fn range_debug1002_nested_actual_ranges_keep_nearest_break_owner_and_exact_source_occurrences() {
    let raw = "init{set gate=false set seen=0 flow.for_loop(a,0,3){flow.for_loop(b,0,3){if gate{flow.break()} if not gate{flow.break()}} set seen=seen+1 if gate{flow.break()} if not gate{flow.break()}} set done=true}\nscene label start\n\"ready\"\n";
    let (graphs, source) = compile(raw);
    let files = Files::new();
    let program = files.program(&source);
    let map = NativeRangeBreakSourceMap::from_program(&program).unwrap();
    assert_eq!(map.visits().len(), 4);
    let mut owner = startup(program);
    let rng = owner.state().random;
    let actual = observed(&mut owner, &map);
    let unique: BTreeSet<_> = actual.iter().map(|v| (v.range, v.edge)).collect();
    assert_eq!(unique.len(), 2);
    assert_eq!(unique.iter().map(|v| v.0).collect::<BTreeSet<_>>().len(), 2);
    for visit in actual {
        let graph = graphs.iter().find(|g| g.graph_id == visit.graph).unwrap();
        assert_eq!(
            graph.pin_by_key(visit.range, "break").unwrap().id,
            visit.input
        );
        assert_eq!(graph.edges[&visit.edge].output, visit.predecessor_output);
    }
    assert_eq!(owner.state().vars["seen"], Value::Int(1));
    assert_eq!(owner.state().vars["done"], Value::Bool(true));
    assert_eq!(owner.state().random, rng);
}

#[test]
fn range_debug1002_frozen_source_and_exact_span_refusals_do_not_mutate_owner_or_origins() {
    let raw="init{set gate=false flow.for_loop(i,0,2){if gate{flow.break()} if not gate{flow.break()}}}\nscene label start\n\"ready\"\n";
    let (_, source) = compile(raw);
    let files = Files::new();
    let program = files.program(&source);
    let map = NativeRangeBreakSourceMap::from_program(&program).unwrap();
    let entry = &map.visits()[0];
    let owner = startup(program.clone());
    let before = serde_json::to_vec(owner.state()).unwrap();
    let rows = map.visits().to_vec();
    assert!(map
        .resolve_origin(owner.program(), &files.0.join("foreign.rvn"), &entry.span)
        .unwrap_err()
        .contains("foreign source"));
    assert!(map
        .resolve_origin(
            owner.program(),
            &entry.source,
            &(entry.span.start..entry.span.end + 1)
        )
        .unwrap()
        .is_none());
    let mut changed = program.clone();
    changed.sources[0]
        .source
        .push_str("\n// changed frozen owner\n");
    assert!(map
        .resolve_origin(&changed, &entry.source, &entry.span)
        .unwrap_err()
        .contains("frozen programme changed"));
    let mut forged = program.clone();
    forged.origins[0].span.range.end -= 1;
    assert!(NativeRangeBreakSourceMap::from_program(&forged)
        .err()
        .unwrap()
        .contains("AST/span differs"));
    assert_eq!(serde_json::to_vec(owner.state()).unwrap(), before);
    assert_eq!(map.visits(), rows);
    assert_eq!(owner.program(), &program);
    // A specialized actual Break observation does not weaken the general map.
    assert_eq!(NativeDebugSourceMap::from_program(&program).err().unwrap(),"ExecLinksV1 debug traversal requires actual predecessor/visit evidence; source preparation alone grants no debugger authority");
}
