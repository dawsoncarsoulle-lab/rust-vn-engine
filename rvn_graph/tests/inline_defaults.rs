//! Saved inline values are editor state while wired, never a second runtime
//! input. These headless graphs do not open or alter any user project.
use rvn_core::{Engine, TerminalRenderer};
use rvn_graph::*;
use rvn_parser::Value;

fn add(graph: &mut GraphDocument, kind: NodeKind, x: f64) -> NodeId {
    graph.add_catalog_node(kind, [x, 80.0]).unwrap()
}

fn pin(graph: &GraphDocument, node: NodeId, key: &str) -> PinId {
    graph.pin_by_key(node, key).unwrap().id
}

fn connect(
    graph: &mut GraphDocument,
    from: NodeId,
    output: &str,
    to: NodeId,
    input: &str,
) -> EdgeId {
    graph
        .connect(pin(graph, from, output), pin(graph, to, input))
        .unwrap()
}

fn source(graph: &mut GraphDocument, value_type: &ValueType, value: PropertyValue) -> NodeId {
    let kind = if value_type == &ValueType::InterpolatedText {
        NodeKind::TextValue
    } else {
        NodeKind::Literal
    };
    let node = add(graph, kind, 0.0);
    graph.set_property(node, "value", value).unwrap();
    node
}

fn cast_cases() -> Vec<(NodeKind, PropertyValue, PropertyValue, Value, Value)> {
    vec![
        (
            NodeKind::ConvertIntToFloat,
            PropertyValue::Int(7),
            PropertyValue::Int(19),
            Value::Float(7.0),
            Value::Float(19.0),
        ),
        (
            NodeKind::ConvertNumberToText,
            PropertyValue::Float(1.25),
            PropertyValue::Float(2.75),
            Value::Str("1.25".into()),
            Value::Str("2.75".into()),
        ),
        (
            NodeKind::ConvertTextToInt,
            PropertyValue::String("42".into()),
            PropertyValue::String("99".into()),
            Value::Int(42),
            Value::Int(99),
        ),
        (
            NodeKind::ConvertStringToText,
            PropertyValue::String("Source [missing]".into()),
            PropertyValue::String("Saved [missing]".into()),
            Value::Str("Source [missing]".into()),
            Value::Str("Saved [missing]".into()),
        ),
        (
            NodeKind::ConvertTextToString,
            PropertyValue::String("Source [missing]".into()),
            PropertyValue::String("Saved [missing]".into()),
            Value::Str("Source [missing]".into()),
            Value::Str("Saved [missing]".into()),
        ),
    ]
}

#[test]
fn inline_defaults_survive_connections_removal_and_json_without_changing_nodes() {
    let mut cases: Vec<_> = cast_cases()
        .into_iter()
        .map(|(kind, incoming, saved, _, _)| (kind, "value", incoming, saved))
        .collect();
    for (kind, key) in [
        (NodeKind::LogicAnd, "left"),
        (NodeKind::LogicOr, "left"),
        (NodeKind::LogicNot, "value"),
        (NodeKind::If, "condition"),
        (NodeKind::While, "condition"),
    ] {
        cases.push((
            kind,
            key,
            PropertyValue::Bool(true),
            PropertyValue::Bool(false),
        ));
    }
    for kind in [NodeKind::SetVariable, NodeKind::LocalVariable] {
        cases.push((kind, "value", PropertyValue::Int(7), PropertyValue::Int(19)));
    }
    for (index, (kind, key, incoming, saved)) in cases.into_iter().enumerate() {
        let mut graph = GraphDocument::new(GraphId::new(94000 + index as u64), GraphKind::Init);
        let target = add(&mut graph, kind, 320.0);
        graph.set_pin_default(target, key, saved.clone()).unwrap();
        let input = pin(&graph, target, key);
        let value_type = graph.pins[&input].value_type.clone();
        let value = source(&mut graph, &value_type, incoming);
        let before = graph.clone();
        let edge = connect(&mut graph, value, "value", target, key);
        assert_eq!(
            graph.pins[&input].default_value,
            Some(saved.clone()),
            "{kind:?} must hide rather than delete its inline value"
        );
        assert_eq!(graph.nodes, before.nodes);
        assert_eq!(
            GraphDocument::from_json(&graph.to_pretty_json().unwrap()).unwrap(),
            graph
        );
        let wired = graph.clone();
        assert!(graph.remove_edge(edge).is_some());
        assert_eq!(graph.pins[&input].default_value, Some(saved));
        assert_eq!(graph.nodes, before.nodes);
        assert!(graph.edges.is_empty());
        assert_eq!(
            GraphDocument::from_json(&graph.to_pretty_json().unwrap()).unwrap(),
            graph
        );
        // Graph snapshots are also the editor's ordinary one-step Undo/Redo.
        graph = before.clone();
        assert_eq!(graph, before);
        graph = wired.clone();
        assert_eq!(graph, wired);
    }
}

#[test]
fn unrelated_consumers_keep_the_previous_clear_hidden_default_semantics() {
    let mut graph = GraphDocument::new(GraphId::new(94100), GraphKind::Init);
    let target = add(&mut graph, NodeKind::Config, 320.0);
    graph
        .set_pin_default(target, "value", PropertyValue::String("previous".into()))
        .unwrap();
    let value = source(
        &mut graph,
        &ValueType::String,
        PropertyValue::String("incoming".into()),
    );
    connect(&mut graph, value, "value", target, "value");
    assert_eq!(
        graph.pin_by_key(target, "value").unwrap().default_value,
        None
    );
}

fn setter(
    graph: &mut GraphDocument,
    name: &str,
    value_type: ValueType,
    default: PropertyValue,
    x: f64,
) -> NodeId {
    graph
        .add_variable(name, value_type.clone(), default.clone())
        .unwrap();
    let node = add(graph, NodeKind::SetVariable, x);
    graph
        .set_property(node, "name", PropertyValue::String(name.into()))
        .unwrap();
    graph
        .set_pin_default(node, "name", PropertyValue::String(name.into()))
        .unwrap();
    graph.set_pin_default(node, "value", default).unwrap();
    for key in ["value", "value_out"] {
        let id = pin(graph, node, key);
        graph.pins.get_mut(&id).unwrap().value_type = value_type.clone();
    }
    node
}

fn compiled_value(graph: &GraphDocument, name: &str) -> Value {
    let generated = transpile(graph).unwrap();
    assert_eq!(
        generated.source.matches(&format!("set {name} =")).count(),
        1,
        "A retained editor default must not emit a second assignment"
    );
    let game = Engine::new(generated.ast, TerminalRenderer, 8).unwrap();
    game.state.vars[name].clone()
}

#[test]
fn all_five_casts_compile_only_the_wire_then_restore_the_saved_inline_value() {
    for (index, (kind, incoming, saved, wired_value, saved_value)) in
        cast_cases().into_iter().enumerate()
    {
        let mut graph = GraphDocument::new(GraphId::new(94200 + index as u64), GraphKind::Init);
        let root = add(&mut graph, NodeKind::Init, 0.0);
        let cast = add(&mut graph, kind, 260.0);
        graph.set_pin_default(cast, "value", saved.clone()).unwrap();
        let input_type = graph.pin_by_key(cast, "value").unwrap().value_type.clone();
        let output_type = graph.pin_by_key(cast, "result").unwrap().value_type.clone();
        let initial = match output_type {
            ValueType::Int => PropertyValue::Int(999),
            ValueType::Float => PropertyValue::Float(999.0),
            _ => PropertyValue::String("setter default must not win".into()),
        };
        let assign = setter(&mut graph, "result", output_type, initial, 520.0);
        let value = source(&mut graph, &input_type, incoming);
        connect(&mut graph, root, "exec_out", assign, "exec_in");
        connect(&mut graph, cast, "result", assign, "value");
        let edge = connect(&mut graph, value, "value", cast, "value");
        let before_materialize = graph.clone();
        assert_eq!(graph.materialize_visible_defaults().unwrap(), 0);
        assert_eq!(graph, before_materialize);
        assert_eq!(
            compiled_value(&graph, "result"),
            wired_value,
            "{kind:?} must prefer its actual wire"
        );
        assert_eq!(
            graph.pin_by_key(cast, "value").unwrap().default_value,
            Some(saved.clone())
        );
        assert!(graph.remove_edge(edge).is_some());
        let restored = GraphDocument::from_json(&graph.to_pretty_json().unwrap()).unwrap();
        assert_eq!(
            compiled_value(&restored, "result"),
            saved_value,
            "{kind:?} must restore its saved inline value"
        );
    }
}

#[test]
fn connected_boolean_branch_and_loop_conditions_ignore_saved_defaults() {
    let mut graph = GraphDocument::new(GraphId::new(94300), GraphKind::Init);
    let root = add(&mut graph, NodeKind::Init, 0.0);
    let branch = add(&mut graph, NodeKind::If, 260.0);
    graph
        .set_pin_default(branch, "condition", PropertyValue::Bool(false))
        .unwrap();
    let conjunction = add(&mut graph, NodeKind::LogicAnd, 260.0);
    graph
        .set_pin_default(conjunction, "left", PropertyValue::Bool(false))
        .unwrap();
    graph
        .set_pin_default(conjunction, "right", PropertyValue::Bool(true))
        .unwrap();
    let yes = source(&mut graph, &ValueType::Bool, PropertyValue::Bool(true));
    connect(&mut graph, yes, "value", conjunction, "left");
    connect(&mut graph, conjunction, "value", branch, "condition");
    let then = setter(
        &mut graph,
        "selected",
        ValueType::Int,
        PropertyValue::Int(1),
        520.0,
    );
    let otherwise = add(&mut graph, NodeKind::SetVariable, 520.0);
    graph
        .set_property(otherwise, "name", PropertyValue::String("selected".into()))
        .unwrap();
    graph
        .set_pin_default(otherwise, "value", PropertyValue::Int(2))
        .unwrap();
    connect(&mut graph, root, "exec_out", branch, "exec_in");
    connect(&mut graph, branch, "then", then, "exec_in");
    connect(&mut graph, branch, "else", otherwise, "exec_in");
    let repeated = add(&mut graph, NodeKind::While, 780.0);
    graph
        .set_pin_default(repeated, "condition", PropertyValue::Bool(true))
        .unwrap();
    let no = source(&mut graph, &ValueType::Bool, PropertyValue::Bool(false));
    connect(&mut graph, no, "value", repeated, "condition");
    connect(&mut graph, branch, "completed", repeated, "exec_in");
    let body = setter(
        &mut graph,
        "loop_entered",
        ValueType::Int,
        PropertyValue::Int(1),
        1040.0,
    );
    connect(&mut graph, repeated, "body", body, "exec_in");
    let generated = transpile(&graph).unwrap();
    let game = Engine::new(generated.ast, TerminalRenderer, 8).unwrap();
    assert_eq!(game.state.vars["selected"], Value::Int(1));
    assert!(!game.state.vars.contains_key("loop_entered"));
    assert_eq!(
        graph.pin_by_key(branch, "condition").unwrap().default_value,
        Some(PropertyValue::Bool(false))
    );
    assert_eq!(
        graph
            .pin_by_key(repeated, "condition")
            .unwrap()
            .default_value,
        Some(PropertyValue::Bool(true))
    );
}
