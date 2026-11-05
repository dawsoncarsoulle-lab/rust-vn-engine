// PRIVATE NOT_RUN: real graph/compiler/Engine tests, no mocked evaluator.
use rvn_core::{Engine, Interaction, TerminalRenderer};
use rvn_graph::*;
use rvn_parser::{parse_program, Value};

fn story() -> (GraphDocument, NodeId, NodeId, NodeId, NodeId) {
    let mut graph = import_script(&parse_program(
        "scene label start\nset hits = 0\nset hits = hits + 1\n\"[hits]\"\n",
    ).unwrap().script).unwrap().into_iter()
        .find(|graph| matches!(&graph.kind, GraphKind::Label { name } if name == "start"))
        .unwrap();
    let root = graph.nodes.values().find(|node| node.kind == NodeKind::Label).unwrap().id;
    let sets: Vec<_> = graph.nodes.values().filter(|node| node.kind == NodeKind::SetVariable)
        .map(|node| node.id).collect();
    assert_eq!(sets.len(), 2);
    let root_output = graph.pin_by_key(root, "exec_out").unwrap().id;
    let zero = graph.edges.values().find(|edge| edge.output == root_output)
        .map(|edge| graph.pins[&edge.input].node).unwrap();
    let increment = *sets.iter().find(|node| **node != zero).unwrap();
    let dialogue = graph.nodes.values().find(|node| node.kind == NodeKind::Dialogue).unwrap().id;
    let exec_edges: Vec<_> = graph.edges.values()
        .filter(|edge| graph.pins[&edge.output].value_type.is_execution())
        .map(|edge| edge.id).collect();
    for edge in exec_edges { graph.remove_edge(edge); }
    (graph, root, zero, increment, dialogue)
}

fn wire(graph: &mut GraphDocument, from: NodeId, output: &str, to: NodeId, input: &str) {
    let output = graph.pin_by_key(from, output).unwrap().id;
    let input = graph.pin_by_key(to, input).unwrap().id;
    graph.connect(output, input).unwrap();
}

// The model supports this multiplicity. Catalog/canvas parity is a separate
// migration; tests explicitly author it and never silently rewrite old files.
fn multiple_exec_input(graph: &mut GraphDocument, node: NodeId) {
    let input = graph.pin_by_key(node, "exec_in").unwrap().id;
    graph.pins.get_mut(&input).unwrap().cardinality = PinCardinality::Many;
}

fn expect_dialogue(game: &mut Engine<TerminalRenderer>, text: &str, hits: i64) {
    assert!(matches!(game.step_until_interaction().unwrap(),
        Some(Interaction::Dialogue { text: actual, .. }) if actual == text));
    assert_eq!(game.state.vars["hits"], Value::Int(hits));
}

fn flatten<'a>(origins: &'a [GraphStatementOrigin], node: NodeId, into: &mut Vec<&'a GraphStatementOrigin>) {
    for origin in origins {
        if origin.node == Some(node) { into.push(origin); }
        flatten(&origin.children, node, into);
    }
}

#[test]
fn convergent_if_runs_shared_node_once_for_each_selected_branch_and_keeps_two_origins() {
    for condition in [true, false] {
        let (mut graph, root, zero, increment, dialogue) = story();
        let branch = graph.add_catalog_node(NodeKind::If, [200.0, 0.0]).unwrap();
        graph.set_pin_default(branch, "condition", PropertyValue::Bool(condition)).unwrap();
        multiple_exec_input(&mut graph, increment);
        wire(&mut graph, root, "exec_out", zero, "exec_in");
        wire(&mut graph, zero, "exec_out", branch, "exec_in");
        wire(&mut graph, branch, "then", increment, "exec_in");
        wire(&mut graph, branch, "else", increment, "exec_in");
        wire(&mut graph, increment, "exec_out", dialogue, "exec_in");
        let bytes = graph.to_pretty_json().unwrap();
        let restored = GraphDocument::from_json(&bytes).unwrap();
        assert_eq!(restored, graph);
        let compiled = transpile_with_origins(&restored).unwrap();
        assert_eq!(graph.to_pretty_json().unwrap(), bytes);
        let mut sites = Vec::new();
        flatten(&compiled.origins, increment, &mut sites);
        assert_eq!(sites.len(), 2);
        assert!(sites.iter().all(|site| site.graph == Some(graph.graph_id)));
        assert_ne!(sites[0].span.range, sites[1].span.range);
        let mut game = Engine::new(compiled.ast, TerminalRenderer, 16).unwrap();
        expect_dialogue(&mut game, "1", 1);
        game.advance_dialogue().unwrap();
        assert!(game.step_until_interaction().unwrap().is_none());
    }
}

#[test]
fn convergent_sequence_runs_shared_node_twice_and_save_resumes_the_correct_visit() {
    let (mut graph, root, zero, increment, dialogue) = story();
    let sequence = graph.add_catalog_node(NodeKind::Sequence, [200.0, 0.0]).unwrap();
    multiple_exec_input(&mut graph, increment);
    wire(&mut graph, root, "exec_out", zero, "exec_in");
    wire(&mut graph, zero, "exec_out", sequence, "exec_in");
    wire(&mut graph, sequence, "then_0", increment, "exec_in");
    wire(&mut graph, sequence, "then_1", increment, "exec_in");
    wire(&mut graph, increment, "exec_out", dialogue, "exec_in");
    let before = graph.clone();
    let compiled = transpile_with_origins(&graph).unwrap();
    assert_eq!(graph, before);
    let mut sites = Vec::new();
    flatten(&compiled.origins, increment, &mut sites);
    assert_eq!(sites.len(), 2);
    assert_ne!(sites[0].span.range, sites[1].span.range);
    let mut game = Engine::new(compiled.ast, TerminalRenderer, 16).unwrap();
    expect_dialogue(&mut game, "1", 1);
    let saved = rvn_core::save::SaveData::from_state(&game.state, 1, "start".into(), "convergence.rvn".into());
    let serialized = serde_json::to_vec(&saved).unwrap();
    let mut fresh = game.fresh(TerminalRenderer, 16).unwrap();
    fresh.load_data(serde_json::from_slice(&serialized).unwrap()).unwrap();
    for runtime in [&mut game, &mut fresh] {
        runtime.advance_dialogue().unwrap();
        expect_dialogue(runtime, "2", 2);
        runtime.advance_dialogue().unwrap();
        assert!(runtime.step_until_interaction().unwrap().is_none());
    }
}

#[test]
fn branch_and_parent_continuation_share_node_without_suppressing_either_visit() {
    let (mut graph, root, zero, increment, dialogue) = story();
    let branch = graph.add_catalog_node(NodeKind::If, [200.0, 0.0]).unwrap();
    graph.set_pin_default(branch, "condition", PropertyValue::Bool(true)).unwrap();
    multiple_exec_input(&mut graph, increment);
    wire(&mut graph, root, "exec_out", zero, "exec_in");
    wire(&mut graph, zero, "exec_out", branch, "exec_in");
    wire(&mut graph, branch, "then", increment, "exec_in");
    wire(&mut graph, branch, "completed", increment, "exec_in");
    wire(&mut graph, increment, "exec_out", dialogue, "exec_in");
    let mut game = Engine::new(transpile(&graph).unwrap().ast, TerminalRenderer, 16).unwrap();
    expect_dialogue(&mut game, "1", 1);
    game.advance_dialogue().unwrap();
    expect_dialogue(&mut game, "2", 2);
}

#[test]
fn actual_ancestor_cycle_retains_diagnostic_and_exact_graph_until_cycle_runtime_is_implemented() {
    let (mut graph, root, zero, increment, _) = story();
    multiple_exec_input(&mut graph, zero);
    wire(&mut graph, root, "exec_out", zero, "exec_in");
    wire(&mut graph, zero, "exec_out", increment, "exec_in");
    wire(&mut graph, increment, "exec_out", zero, "exec_in");
    let before = graph.to_pretty_json().unwrap();
    let error = transpile(&graph).unwrap_err();
    assert!(matches!(error, TranspileError::ExecutionCycle(node) if node == zero));
    assert_eq!(graph.to_pretty_json().unwrap(), before);
}

#[test]
fn compact_acyclic_diamonds_refuse_exponential_expansion_without_modifying_graph() {
    let (mut graph, root, zero, increment, _) = story();
    let mut previous = None;
    for _ in 0..18 {
        let sequence = graph.add_catalog_node(NodeKind::If, [200.0, 0.0]).unwrap();
        graph.set_pin_default(sequence, "condition", PropertyValue::Bool(true)).unwrap();
        multiple_exec_input(&mut graph, sequence);
        if let Some(previous) = previous {
            wire(&mut graph, previous, "then", sequence, "exec_in");
            wire(&mut graph, previous, "else", sequence, "exec_in");
        } else {
            wire(&mut graph, root, "exec_out", zero, "exec_in");
            wire(&mut graph, zero, "exec_out", sequence, "exec_in");
        }
        previous = Some(sequence);
    }
    multiple_exec_input(&mut graph, increment);
    let previous = previous.unwrap();
    wire(&mut graph, previous, "then", increment, "exec_in");
    wire(&mut graph, previous, "else", increment, "exec_in");
    let before = graph.clone();
    let error = transpile(&graph).unwrap_err();
    assert!(matches!(error, TranspileError::ExecutionExpansionLimit { resource: "execution_visits", limit: 65_536, .. }), "{error:?}");
    assert_eq!(error.code(), "execution_expansion_limit");
    assert!(error.node().is_some());
    assert_eq!(graph, before);
}

#[test]
fn deep_acyclic_branch_nesting_returns_a_targeted_error_without_stack_overflow() {
    let (mut graph, root, zero, _, _) = story();
    wire(&mut graph, root, "exec_out", zero, "exec_in");
    let mut previous = zero;
    let mut output = "exec_out";
    for _ in 0..130 {
        let branch = graph.add_catalog_node(NodeKind::If, [200.0, 0.0]).unwrap();
        graph.set_pin_default(branch, "condition", PropertyValue::Bool(true)).unwrap();
        wire(&mut graph, previous, output, branch, "exec_in");
        previous = branch;
        output = "then";
    }
    let before = graph.clone();
    let error = transpile(&graph).unwrap_err();
    assert!(matches!(error, TranspileError::ExecutionExpansionLimit { resource: "nesting", limit: 128, .. }));
    assert_eq!(graph, before);
}

#[test]
fn repeated_long_dialogue_refuses_source_expansion_without_changing_authored_text() {
    let (mut graph, root, zero, _, dialogue) = story();
    let sequence = graph.add_catalog_node(NodeKind::Sequence, [200.0, 0.0]).unwrap();
    graph.resize_sequence_outputs(sequence, 32).unwrap();
    let text_input = graph.pin_by_key(dialogue, "text").unwrap().id;
    let text_edge = graph.edges.values().find(|edge| edge.input == text_input).unwrap().id;
    graph.remove_edge(text_edge);
    let text_source = graph.add_catalog_node(NodeKind::TextValue, [400.0, 200.0]).unwrap();
    graph.set_property(text_source, "value", PropertyValue::String("x".repeat(600_000))).unwrap();
    wire(&mut graph, text_source, "value", dialogue, "text");
    multiple_exec_input(&mut graph, dialogue);
    wire(&mut graph, root, "exec_out", zero, "exec_in");
    wire(&mut graph, zero, "exec_out", sequence, "exec_in");
    for index in 0..32 {
        wire(&mut graph, sequence, &format!("then_{index}"), dialogue, "exec_in");
    }
    let before = graph.clone();
    let error = transpile(&graph).unwrap_err();
    assert!(matches!(error, TranspileError::ExecutionExpansionLimit { resource: "source_bytes", limit, .. } if limit == 16 * 1024 * 1024), "{error:?}");
    assert_eq!(graph, before);
}
