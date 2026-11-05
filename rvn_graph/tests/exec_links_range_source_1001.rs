//! Actual Graph/public source preparation only. No VM, save or debug grant.
use rvn_graph::*;
use std::{
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
            "rvn-exec-source1001-{}-{nonce}-{}",
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
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn graph() -> (Vec<GraphDocument>, usize, NodeId, Vec<GraphEdge>) {
    let source = r#"init {
 set gate=false
 flow.for_loop(i,1,3) {
  if gate {flow.break()}
  if not gate {flow.break()}
 }
}
scene label start
"ready"
"#;
    let program = rvn_parser::parse_program(source).unwrap();
    let mut graphs = import_script(&program.script).unwrap();
    let index = graphs
        .iter()
        .position(|g| matches!(g.kind, GraphKind::Init))
        .unwrap();
    let owner = graphs[index]
        .nodes
        .values()
        .find(|n| n.kind == NodeKind::ForLoopWithBreak)
        .unwrap()
        .id;
    let pin = graphs[index].pin_by_key(owner, "break").unwrap().id;
    let edges = graphs[index]
        .edges
        .values()
        .filter(|e| e.input == pin)
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(edges.len(), 2);
    assert_eq!(graphs[index].pins[&pin].cardinality, PinCardinality::Many);
    // No mutable cardinality, hand-authored identity, synthetic wrapper or VM.
    for g in &mut graphs {
        g.enable_native_frame_preparation().unwrap();
    }
    graphs[index] = graphs[index]
        .prepare_native_loop_demand(owner)
        .unwrap()
        .graph;
    graphs[index].refresh_variable_usage_bindings().unwrap();
    (graphs, index, owner, edges)
}
fn enabled() -> (Vec<GraphDocument>, usize, NodeId, Vec<GraphEdge>) {
    let (mut graphs, index, owner, edges) = graph();
    graphs[index]
        .enable_exec_links_source_preparation()
        .unwrap();
    (graphs, index, owner, edges)
}

#[test]
fn two_real_break_wires_roundtrip_exact_ids_and_zero_capture_source_sites() {
    let (graphs, index, owner, edges) = enabled();
    let before = graphs.clone();
    let compiled = transpile_project(&graphs).unwrap();
    assert!(compiled.source.contains("!exec_links_v1|"));
    let pin = graphs[index].pin_by_key(owner, "break").unwrap().id;
    let spelling = format!(
        "break={}@[{}>{}+{}>{}]",
        pin, edges[0].id, edges[0].output, edges[1].id, edges[1].output
    );
    assert!(compiled.source.contains(&spelling));
    let files = Files::new();
    let program = files.program(&compiled.source);
    rvn_parser::scheduled_source::native_loop_contract::validate_authored_bindings(&program.script)
        .unwrap();
    let steps = rvn_parser::scheduled_source::scheduled_step_sources(&program).unwrap();
    assert!(steps.iter().all(|s| s
        .endpoint
        .as_ref()
        .is_none_or(|e| e.graph != graphs[index].graph_id.get() || e.exec_links_v1)));
    let sites = native_source_sites(&program).unwrap();
    assert!(sites
        .iter()
        .any(|site| site.graph == graphs[index].graph_id && site.consumer == owner));
    let restored = import_native_frame_script(&program.script).unwrap();
    let actual = restored
        .iter()
        .find(|g| g.graph_id == graphs[index].graph_id)
        .unwrap();
    assert_eq!(actual.edges, graphs[index].edges);
    assert_eq!(
        actual.pins.keys().collect::<Vec<_>>(),
        graphs[index].pins.keys().collect::<Vec<_>>()
    );
    assert_eq!(
        actual.nodes.keys().collect::<Vec<_>>(),
        graphs[index].nodes.keys().collect::<Vec<_>>()
    );
    assert_eq!(
        actual
            .edges
            .values()
            .filter(|e| e.input == pin)
            .cloned()
            .collect::<Vec<_>>(),
        edges
    );
    let again = transpile_project(&restored).unwrap();
    assert_eq!(again.ast, compiled.ast);
    assert_eq!(again.source, compiled.source);
    assert_eq!(graphs, before);
}

#[test]
fn forged_break_source_output_edge_collision_and_omission_refuse_neutrally() {
    let (graphs, index, owner, edges) = enabled();
    let before = graphs.clone();
    let compiled = transpile_project(&graphs).unwrap();
    let pin = graphs[index].pin_by_key(owner, "break").unwrap().id;
    let valid = format!(
        "break={}@[{}>{}+{}>{}]",
        pin, edges[0].id, edges[0].output, edges[1].id, edges[1].output
    );
    let absent = graphs[index]
        .pins
        .keys()
        .map(|pin| pin.get())
        .max()
        .unwrap()
        + 1;
    let changes = [
        format!(
            "break={}@[{}>{}+{}>{}]",
            pin, edges[0].id, edges[0].output, edges[1].id, absent
        ),
        format!(
            "break={}@[{}>{}+{}>{}]",
            pin, edges[0].id, edges[0].output, edges[0].id, edges[1].output
        ),
        format!("break={}@[{}>{}]", pin, edges[0].id, edges[0].output),
        format!(
            "break={}@[{}>{}+{}>{}]",
            pin, edges[0].id, edges[0].output, edges[1].id, edges[0].output
        ),
    ];
    let expected = [
        "ExecLinksV1 source output does not match one actual reconstructed execution wire",
        "EdgeOrder",
        "ExecLinksV1 actual input direction/count differs from full declaration",
        "DuplicateOutput",
    ];
    for (changed, expected) in changes.into_iter().zip(expected) {
        assert_ne!(changed, valid);
        let source = compiled.source.replace(&valid, &changed);
        assert_ne!(source, compiled.source);
        let error = match rvn_parser::parse_program(&source) {
            Ok(parsed) => import_native_frame_script(&parsed.script)
                .err()
                .expect("Forged declaration unexpectedly reconstructed"),
            Err(error) => error,
        };
        assert!(
            error.contains(expected),
            "Wrong refusal for {changed}: {error}; expected {expected}"
        );
        assert_eq!(graphs, before);
    }
}

#[test]
fn new_source_does_not_report_first_break_as_runtime_debug_traversal() {
    let (graphs, _, _, _) = enabled();
    let source = transpile_project(&graphs).unwrap().source;
    let files = Files::new();
    let program = files.program(&source);
    let before = program.clone();
    assert!(
        !native_source_sites(&program).unwrap().is_empty(),
        "Real full source graph validation precedes the debug boundary"
    );
    let result = NativeDebugSourceMap::from_program(&program);
    let error = result
        .err()
        .expect("No arbitrary first incoming edge may become debug evidence");
    assert_eq!(error,"ExecLinksV1 debug traversal requires actual predecessor/visit evidence; source preparation alone grants no debugger authority");
    assert_eq!(program, before);
}

#[test]
fn legacy_roundtrip_bytes_and_multiple_break_refusal_are_unchanged() {
    let (graphs, index, _, _) = graph();
    let before = graphs.clone();
    let error = prepare_native_frame_graph(&graphs[index])
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("Native source identity requires one incoming edge per port"),
        "{error}"
    );
    assert_eq!(graphs, before);
    let source = "init {set count=1\nset count=count+1}\nscene label start\n\"ready\"\n";
    let parsed = rvn_parser::parse_program(source).unwrap();
    let mut ordinary = import_script(&parsed.script).unwrap();
    for g in &mut ordinary {
        g.enable_native_frame_preparation().unwrap();
        g.refresh_variable_usage_bindings().unwrap();
    }
    let frozen = ordinary.clone();
    let refused = ordinary[0]
        .enable_exec_links_source_preparation()
        .unwrap_err();
    assert_eq!(refused,"Macro ExecLinks requires its actual Widget Handler graph");
    assert_eq!(ordinary, frozen);
    let compiled = transpile_project(&ordinary).unwrap();
    assert!(!compiled.source.contains("!exec_links_v1"));
    let old = rvn_parser::parse_program(&compiled.source).unwrap();
    let steps = rvn_parser::scheduled_source::scheduled_step_sources(&old).unwrap();
    assert!(
        !steps.is_empty(),
        "Actual legacy producer source DTO must be tested"
    );
    for row in steps {
        if let Some(endpoint) = row.endpoint {
            let json = serde_json::to_string(&endpoint).unwrap();
            assert!(!json.contains("exec_links_v1") && !json.contains("exec_incoming"));
        }
    }
    let restored = import_native_frame_script(&old.script).unwrap();
    assert_eq!(
        transpile_project(&restored).unwrap().source,
        compiled.source
    );
    assert_eq!(ordinary, frozen);
}
