//! Pure headless regression fixtures. No user project or serialized type cache
//! is rewritten to specialize the presentation of a vacant operand.
use rvn_core::{Engine, TerminalRenderer};
use rvn_graph::*;
use rvn_parser::Value;

fn graph() -> GraphDocument {
    GraphDocument::new(GraphId::new(96001), GraphKind::Init)
}
fn add(g: &mut GraphDocument, kind: NodeKind) -> NodeId {
    g.add_catalog_node(kind, [0.0, 0.0]).unwrap()
}
fn pin(g: &GraphDocument, node: NodeId, key: &str) -> PinId {
    g.pin_by_key(node, key).unwrap().id
}
fn literal(g: &mut GraphDocument, value: PropertyValue) -> NodeId {
    let node = add(g, NodeKind::Literal);
    g.set_property(node, "value", value).unwrap();
    node
}
fn wire(g: &mut GraphDocument, from: NodeId, output: &str, to: NodeId, input: &str) -> EdgeId {
    g.connect(pin(g, from, output), pin(g, to, input)).unwrap()
}
fn ty(g: &GraphDocument, node: NodeId, key: &str) -> ValueType {
    g.effective_pin_type(pin(g, node, key)).unwrap()
}
fn default(g: &GraphDocument, node: NodeId, key: &str) -> Option<PropertyValue> {
    g.effective_pin_default_value(pin(g, node, key))
}
fn evaluate(g: &GraphDocument, node: NodeId, key: &str) -> Value {
    let expression = transpile_value_output(g, node, key).unwrap();
    let script = rvn_parser::parse(&format!("init {{ set result = {expression} }}")).unwrap();
    Engine::new(script, TerminalRenderer, 8).unwrap().state.vars["result"].clone()
}

#[test]
fn new_vacant_operators_are_neutral_missing_and_read_only() {
    for kind in [
        NodeKind::MathAdd,
        NodeKind::MathSubtract,
        NodeKind::MathMultiply,
        NodeKind::MathDivide,
        NodeKind::MathEqual,
        NodeKind::MathNotEqual,
        NodeKind::MathLess,
        NodeKind::MathLessEqual,
        NodeKind::MathGreater,
        NodeKind::MathGreaterEqual,
    ] {
        let mut g = graph();
        let node = add(&mut g, kind);
        let before = g.to_pretty_json().unwrap();
        assert_eq!(ty(&g, node, "left"), ValueType::Any);
        assert_eq!(ty(&g, node, "right"), ValueType::Any);
        assert_eq!(default(&g, node, "left"), None);
        assert_eq!(default(&g, node, "right"), None);
        assert!(matches!(
            transpile_value_output(&g, node, "value"),
            Err(TranspileError::MissingInputValue { .. })
        ));
        let types = g.effective_pin_types();
        let defaults = g.effective_pin_default_values();
        assert_eq!(types.len(), g.pins.len());
        assert!(defaults.is_empty());
        assert_eq!(g.to_pretty_json().unwrap(), before);
    }
}

#[test]
fn first_known_operand_specializes_a_real_default_and_disconnect_resets_it() {
    for (property, expected_type, expected_default, result) in [
        (
            PropertyValue::Int(8),
            ValueType::Int,
            PropertyValue::Int(0),
            Value::Int(8),
        ),
        (
            PropertyValue::Float(2.5),
            ValueType::Float,
            PropertyValue::Float(0.0),
            Value::Float(2.5),
        ),
        (
            PropertyValue::String("name".into()),
            ValueType::String,
            PropertyValue::String(String::new()),
            Value::Str("name".into()),
        ),
    ] {
        let mut g = graph();
        let source = literal(&mut g, property);
        let node = add(&mut g, NodeKind::MathAdd);
        let edge = wire(&mut g, source, "value", node, "left");
        let before = g.to_pretty_json().unwrap();
        assert_eq!(ty(&g, node, "left"), expected_type);
        assert_eq!(ty(&g, node, "right"), expected_type);
        assert_eq!(ty(&g, node, "value"), expected_type);
        assert_eq!(default(&g, node, "right"), Some(expected_default.clone()));
        assert_eq!(
            g.effective_pin_default_values()[&pin(&g, node, "right")],
            expected_default
        );
        let generated_default = transpile_value_input(&g, node, "right").unwrap();
        assert_eq!(
            generated_default,
            match expected_type {
                ValueType::Int => "0",
                ValueType::Float => "0.0",
                ValueType::String => "\"\"",
                _ => unreachable!(),
            }
        );
        assert_eq!(evaluate(&g, node, "value"), result);
        assert_eq!(
            g.to_pretty_json().unwrap(),
            before,
            "draw and codegen are not edits"
        );
        assert_eq!(
            g.pin_by_key(node, "right").unwrap().value_type,
            ValueType::Any
        );
        assert_eq!(g.pin_by_key(node, "right").unwrap().default_value, None);
        g.remove_edge(edge);
        assert_eq!(ty(&g, node, "left"), ValueType::Any);
        assert_eq!(ty(&g, node, "right"), ValueType::Any);
        assert_eq!(ty(&g, node, "value"), ValueType::Any);
        assert_eq!(default(&g, node, "right"), None);
    }
}

#[test]
fn mixed_numeric_inputs_keep_their_own_types_and_real_float_promotion_through_chains() {
    let mut g = graph();
    let integer = literal(&mut g, PropertyValue::Int(8));
    let float = literal(&mut g, PropertyValue::Float(2.5));
    let a = add(&mut g, NodeKind::MathAdd);
    let b = add(&mut g, NodeKind::MathAdd);
    let c = add(&mut g, NodeKind::MathSubtract);
    let int_wire = wire(&mut g, integer, "value", a, "left");
    wire(&mut g, a, "value", b, "left");
    wire(&mut g, b, "value", c, "left");
    assert_eq!(ty(&g, c, "value"), ValueType::Int);
    assert_eq!(evaluate(&g, c, "value"), Value::Int(8));
    let float_wire = wire(&mut g, float, "value", a, "right");
    assert_eq!(ty(&g, a, "left"), ValueType::Int);
    assert_eq!(ty(&g, a, "right"), ValueType::Float);
    assert_eq!(ty(&g, a, "value"), ValueType::Float);
    assert_eq!(ty(&g, b, "right"), ValueType::Float);
    assert_eq!(ty(&g, c, "value"), ValueType::Float);
    assert_eq!(evaluate(&g, c, "value"), Value::Float(10.5));
    g.remove_edge(int_wire);
    assert_eq!(ty(&g, a, "left"), ValueType::Float);
    assert_eq!(default(&g, a, "left"), Some(PropertyValue::Float(0.0)));
    g.remove_edge(float_wire);
    assert_eq!(ty(&g, c, "value"), ValueType::Any);
    assert_eq!(default(&g, a, "left"), None);
    assert_eq!(default(&g, b, "right"), None);
    assert_eq!(default(&g, c, "right"), None);
    let replacement = wire(&mut g, float, "value", a, "left");
    assert_eq!(ty(&g, c, "value"), ValueType::Float);
    assert_eq!(evaluate(&g, c, "value"), Value::Float(2.5));
    g.remove_edge(replacement);
    wire(&mut g, integer, "value", a, "right");
    assert_eq!(ty(&g, c, "value"), ValueType::Int);
}

#[test]
fn every_arithmetic_kind_uses_typed_zero_and_never_changes_division_semantics() {
    for (kind, expected) in [
        (NodeKind::MathAdd, Value::Int(8)),
        (NodeKind::MathSubtract, Value::Int(-8)),
        (NodeKind::MathMultiply, Value::Int(0)),
        (NodeKind::MathDivide, Value::Int(0)),
    ] {
        let mut g = graph();
        let source = literal(&mut g, PropertyValue::Int(8));
        let node = add(&mut g, kind);
        wire(&mut g, source, "value", node, "right");
        assert_eq!(default(&g, node, "left"), Some(PropertyValue::Int(0)));
        assert_eq!(ty(&g, node, "value"), ValueType::Int);
        assert_eq!(evaluate(&g, node, "value"), expected);
    }
    let mut g = graph();
    let source = literal(&mut g, PropertyValue::Float(8.5));
    let node = add(&mut g, NodeKind::MathMultiply);
    wire(&mut g, source, "value", node, "right");
    assert_eq!(default(&g, node, "left"), Some(PropertyValue::Float(0.0)));
    assert_eq!(evaluate(&g, node, "value"), Value::Float(0.0));
}

#[test]
fn unknown_connected_values_never_borrow_a_known_operand_or_downstream_type() {
    let mut g = graph();
    g.add_variable("unknown", ValueType::Any, PropertyValue::Int(0))
        .unwrap();
    let unknown = add(&mut g, NodeKind::VariableGet);
    g.set_property(unknown, "name", PropertyValue::String("unknown".into()))
        .unwrap();
    let node = add(&mut g, NodeKind::MathAdd);
    let integer = literal(&mut g, PropertyValue::Int(3));
    wire(&mut g, unknown, "value", node, "left");
    assert_eq!(default(&g, node, "right"), None);
    wire(&mut g, integer, "value", node, "right");
    assert_eq!(ty(&g, node, "left"), ValueType::Any);
    assert_eq!(ty(&g, node, "right"), ValueType::Int);
    assert_eq!(ty(&g, node, "value"), ValueType::Any);
    assert_eq!(default(&g, unknown, "value"), None);
    let cast = add(&mut g, NodeKind::ConvertIntToFloat);
    wire(&mut g, node, "value", cast, "value");
    assert_eq!(ty(&g, unknown, "value"), ValueType::Any);
    assert_eq!(ty(&g, node, "value"), ValueType::Any);
}

#[test]
fn comparisons_use_their_real_domain_and_always_return_boolean() {
    for (kind, property, input_type, fallback, expected) in [
        (
            NodeKind::MathEqual,
            PropertyValue::Bool(false),
            ValueType::Bool,
            PropertyValue::Bool(false),
            Value::Bool(true),
        ),
        (
            NodeKind::MathNotEqual,
            PropertyValue::Bool(true),
            ValueType::Bool,
            PropertyValue::Bool(false),
            Value::Bool(true),
        ),
        (
            NodeKind::MathLess,
            PropertyValue::Int(2),
            ValueType::Int,
            PropertyValue::Int(0),
            Value::Bool(true),
        ),
        (
            NodeKind::MathGreaterEqual,
            PropertyValue::Float(2.5),
            ValueType::Float,
            PropertyValue::Float(0.0),
            Value::Bool(false),
        ),
        (
            NodeKind::MathLessEqual,
            PropertyValue::String("b".into()),
            ValueType::String,
            PropertyValue::String(String::new()),
            Value::Bool(true),
        ),
    ] {
        let mut g = graph();
        let source = literal(&mut g, property);
        let node = add(&mut g, kind);
        let edge = wire(&mut g, source, "value", node, "right");
        assert_eq!(ty(&g, node, "left"), input_type);
        assert_eq!(default(&g, node, "left"), Some(fallback));
        assert_eq!(ty(&g, node, "value"), ValueType::Bool);
        assert_eq!(evaluate(&g, node, "value"), expected);
        g.remove_edge(edge);
        assert_eq!(ty(&g, node, "left"), ValueType::Any);
        assert_eq!(ty(&g, node, "value"), ValueType::Bool);
    }
}

#[test]
fn authored_defaults_take_precedence_and_survive_wires_undo_and_json() {
    for kind in [
        NodeKind::MathAdd,
        NodeKind::MathSubtract,
        NodeKind::MathMultiply,
        NodeKind::MathDivide,
        NodeKind::MathEqual,
        NodeKind::MathNotEqual,
        NodeKind::MathLess,
        NodeKind::MathLessEqual,
        NodeKind::MathGreater,
        NodeKind::MathGreaterEqual,
        NodeKind::BinaryOperator,
    ] {
        let mut g = graph();
        let source = literal(&mut g, PropertyValue::Int(3));
        let node = add(&mut g, kind);
        g.set_pin_default(node, "left", PropertyValue::Float(5.5))
            .unwrap();
        let before = g.clone();
        let edge = wire(&mut g, source, "value", node, "left");
        let wired = g.clone();
        assert_eq!(default(&g, node, "left"), Some(PropertyValue::Float(5.5)));
        assert_eq!(ty(&g, node, "left"), ValueType::Int);
        assert_eq!(
            GraphDocument::from_json(&g.to_pretty_json().unwrap()).unwrap(),
            g
        );
        g.remove_edge(edge);
        assert_eq!(default(&g, node, "left"), Some(PropertyValue::Float(5.5)));
        assert_eq!(ty(&g, node, "left"), ValueType::Float);
        g = before.clone();
        assert_eq!(g, before);
        g = wired.clone();
        assert_eq!(g, wired);
    }
}

#[test]
fn generic_operators_share_defaults_but_empty_negate_never_invents_an_input() {
    for (operator, expected_type, expected) in [
        ("+", ValueType::Int, Value::Int(3)),
        ("-", ValueType::Int, Value::Int(-3)),
        ("*", ValueType::Int, Value::Int(0)),
        ("/", ValueType::Int, Value::Int(0)),
        ("<", ValueType::Bool, Value::Bool(true)),
        ("==", ValueType::Bool, Value::Bool(false)),
    ] {
        let mut g = graph();
        let source = literal(&mut g, PropertyValue::Int(3));
        let node = add(&mut g, NodeKind::BinaryOperator);
        g.set_property(node, "operator", PropertyValue::String(operator.into()))
            .unwrap();
        wire(&mut g, source, "value", node, "right");
        assert_eq!(default(&g, node, "left"), Some(PropertyValue::Int(0)));
        assert_eq!(ty(&g, node, "value"), expected_type);
        assert_eq!(evaluate(&g, node, "value"), expected);
    }
    for kind in [NodeKind::MathNegate, NodeKind::UnaryOperator] {
        let mut g = graph();
        let node = add(&mut g, kind);
        g.set_property(node, "operator", PropertyValue::String("-".into()))
            .unwrap();
        assert_eq!(default(&g, node, "value"), None);
        assert_eq!(ty(&g, node, "result"), ValueType::Any);
        let source = literal(&mut g, PropertyValue::Float(2.5));
        let edge = wire(&mut g, source, "value", node, "value");
        assert_eq!(ty(&g, node, "result"), ValueType::Float);
        assert_eq!(evaluate(&g, node, "result"), Value::Float(-2.5));
        g.remove_edge(edge);
        assert_eq!(default(&g, node, "value"), None);
    }
}

#[test]
fn malformed_cycles_and_incompatible_domains_never_create_implicit_values() {
    let mut g = graph();
    let a = add(&mut g, NodeKind::MathAdd);
    let b = add(&mut g, NodeKind::MathAdd);
    wire(&mut g, a, "value", b, "left");
    let cycle = EdgeId::new(999);
    g.edges.insert(
        cycle,
        GraphEdge {
            id: cycle,
            output: pin(&g, b, "value"),
            input: pin(&g, a, "left"),
        },
    );
    assert_eq!(ty(&g, a, "value"), ValueType::Any);
    assert_eq!(ty(&g, b, "value"), ValueType::Any);
    assert!(g.effective_pin_default_values().is_empty());
    assert!(g.validate().iter().any(|d| d.code == "data_cycle"));
    let mut invalid = graph();
    let bool_source = literal(&mut invalid, PropertyValue::Bool(true));
    let arithmetic = add(&mut invalid, NodeKind::MathAdd);
    wire(&mut invalid, bool_source, "value", arithmetic, "left");
    assert_eq!(default(&invalid, arithmetic, "right"), None);
    assert_eq!(ty(&invalid, arithmetic, "value"), ValueType::Any);
}

#[test]
fn high_arity_defaults_share_one_context_without_rewriting_a_single_operand() {
    let mut g = graph();
    let node = add(&mut g, NodeKind::MathAdd);
    g.set_pin_default(node, "left", PropertyValue::Float(2.5))
        .unwrap();
    for _ in 0..1000 {
        g.add_operator_operand(node).unwrap();
    }
    let before = g.to_pretty_json().unwrap();
    let defaults = g.effective_pin_default_values();
    let types = g.effective_pin_types();
    assert_eq!(defaults.len(), 1002);
    for operand in &g.nodes[&node].pins {
        assert_eq!(types[operand], ValueType::Float);
        if g.pins[operand].direction == PinDirection::Input {
            assert_eq!(
                defaults[operand],
                if g.pins[operand].key == "left" {
                    PropertyValue::Float(2.5)
                } else {
                    PropertyValue::Float(0.0)
                }
            );
        }
    }
    assert_eq!(g.to_pretty_json().unwrap(), before);
}

#[test]
fn opening_operator_defaults_keeps_inline_fields_without_extracting_new_literals() {
    for kind in [
        NodeKind::MathAdd,
        NodeKind::MathSubtract,
        NodeKind::MathMultiply,
        NodeKind::MathDivide,
        NodeKind::MathEqual,
        NodeKind::MathNotEqual,
        NodeKind::MathLess,
        NodeKind::MathLessEqual,
        NodeKind::MathGreater,
        NodeKind::MathGreaterEqual,
        NodeKind::MathNegate,
    ] {
        let mut g = graph();
        let node = add(&mut g, kind);
        let key = if kind == NodeKind::MathNegate {
            "value"
        } else {
            "left"
        };
        g.set_pin_default(node, key, PropertyValue::Float(2.5))
            .unwrap();
        let before = g.to_pretty_json().unwrap();
        assert_eq!(g.materialize_visible_defaults().unwrap(), 0);
        assert_eq!(g.to_pretty_json().unwrap(), before);
        let mut reopened = GraphDocument::from_json(&before).unwrap();
        assert_eq!(reopened.materialize_visible_defaults().unwrap(), 0);
        assert_eq!(reopened.to_pretty_json().unwrap(), before);
    }
    let mut legacy = graph();
    let node = add(&mut legacy, NodeKind::BinaryOperator);
    legacy
        .set_property(node, "operator", PropertyValue::String("+".into()))
        .unwrap();
    legacy
        .set_pin_default(node, "left", PropertyValue::Int(3))
        .unwrap();
    legacy.materialize_visible_defaults().unwrap();
    assert_eq!(legacy.nodes[&node].kind, NodeKind::MathAdd);
    assert_eq!(legacy.nodes.len(), 1);
    assert_eq!(default(&legacy, node, "left"), Some(PropertyValue::Int(3)));
}

#[test]
fn source_reopen_preserves_implicit_and_authored_inline_defaults_with_comments_and_identities() {
    let source = "// Kept exactly\nfunction value() { return 2.5 + 0.0 }\nlabel start\nreturn\n";
    let mut project = SourceProject::open(source, &[]).unwrap();
    let mut edited = project.graphs().to_vec();
    let function = edited
        .iter_mut()
        .find(|g| matches!(g.kind, GraphKind::Function { .. }))
        .unwrap();
    let op = function
        .nodes
        .values()
        .find(|n| n.kind == NodeKind::MathAdd)
        .unwrap()
        .id;
    // This is an explicit UI edit: the authored literals are converted into
    // inline fields. A read or re-open must preserve that exact representation.
    for key in ["left", "right"] {
        let input = pin(function, op, key);
        let edge = function
            .edges
            .values()
            .find(|edge| edge.input == input)
            .unwrap()
            .clone();
        let producer = function.pins[&edge.output].node;
        function.remove_node(producer);
    }
    function
        .set_pin_default(op, "left", PropertyValue::Float(2.5))
        .unwrap();
    let before = function.clone();
    assert_eq!(
        default(function, op, "right"),
        Some(PropertyValue::Float(0.0))
    );
    project.apply_visual(source, &edited).unwrap();
    assert_eq!(project.source(), source);
    let mut reopened = SourceProject::open(project.source(), project.graphs()).unwrap();
    let function = reopened
        .graphs()
        .iter()
        .find(|g| matches!(g.kind, GraphKind::Function { .. }))
        .unwrap();
    assert_eq!(function, &before);
    assert_eq!(
        function.pin_by_key(op, "right").unwrap().default_value,
        None
    );
    let current = reopened.source().to_owned();
    let untouched = reopened.graphs().to_vec();
    reopened.apply_visual(&current, &untouched).unwrap();
    assert_eq!(reopened.source(), source);
}

#[test]
fn string_concatenation_and_integer_division_match_the_actual_rvn_runtime() {
    for (left, right, left_type, right_type, expected) in [
        (
            PropertyValue::Bool(true),
            PropertyValue::String("name".into()),
            ValueType::Bool,
            ValueType::String,
            Value::Str("truename".into()),
        ),
        (
            PropertyValue::String("name".into()),
            PropertyValue::Bool(true),
            ValueType::String,
            ValueType::Bool,
            Value::Str("nametrue".into()),
        ),
    ] {
        let mut g = graph();
        let a = literal(&mut g, left);
        let b = literal(&mut g, right);
        let op = add(&mut g, NodeKind::MathAdd);
        wire(&mut g, a, "value", op, "left");
        wire(&mut g, b, "value", op, "right");
        assert_eq!(ty(&g, op, "left"), left_type);
        assert_eq!(ty(&g, op, "right"), right_type);
        assert_eq!(ty(&g, op, "value"), ValueType::String);
        assert_eq!(evaluate(&g, op, "value"), expected);
    }
    for (right, right_type, expected) in [
        (PropertyValue::Int(2), ValueType::Int, Value::Int(4)),
        (
            PropertyValue::Float(2.0),
            ValueType::Float,
            Value::Float(4.5),
        ),
    ] {
        let mut g = graph();
        let a = literal(&mut g, PropertyValue::Int(9));
        let b = literal(&mut g, right);
        let op = add(&mut g, NodeKind::MathDivide);
        wire(&mut g, a, "value", op, "left");
        wire(&mut g, b, "value", op, "right");
        assert_eq!(ty(&g, op, "value"), right_type);
        assert_eq!(evaluate(&g, op, "value"), expected);
    }
}

#[test]
#[ignore = "Opt-in read-only benchmark of an explicitly selected Atlas snapshot"]
fn atlas_type_and_default_projection_benchmark() {
    use std::time::Instant;
    let path = std::path::PathBuf::from(
        std::env::var_os("RVN_ADAPTIVE_ATLAS_SNAPSHOT")
            .expect("Choose the snapshot to inspect read-only"),
    );
    let bytes = std::fs::read(&path).unwrap();
    let snapshot: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let graphs: Vec<GraphDocument> = serde_json::from_value(snapshot["graphs"].clone()).unwrap();
    let node_count: usize = graphs.iter().map(|g| g.nodes.len()).sum();
    let pin_count: usize = graphs.iter().map(|g| g.pins.len()).sum();
    let before: Vec<_> = graphs.iter().map(|g| g.to_pretty_json().unwrap()).collect();
    let mut durations = Vec::new();
    for pass in 0..6 {
        let start = Instant::now();
        let mut resolved = 0;
        let mut defaults = 0;
        for g in &graphs {
            resolved += g.effective_pin_types().len();
            defaults += g.effective_pin_default_values().len();
        }
        assert_eq!(resolved, pin_count);
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;
        durations.push(elapsed);
        println!("Adaptive Atlas projection pass {pass}: {elapsed:.3} ms / {} graphs / {node_count} nodes / {pin_count} pins / {defaults} defaults", graphs.len());
    }
    assert_eq!(
        graphs
            .iter()
            .map(|g| g.to_pretty_json().unwrap())
            .collect::<Vec<_>>(),
        before
    );
    assert_eq!(
        std::fs::read(&path).unwrap(),
        bytes,
        "The inspected snapshot must remain byte-for-byte unchanged"
    );
    println!(
        "Adaptive Atlas projection average: {:.3} ms",
        durations.iter().sum::<f64>() / durations.len() as f64
    );
}
