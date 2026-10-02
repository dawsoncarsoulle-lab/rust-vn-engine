//! New Blueprint contracts do not redefine older dynamic RVN expressions.
use rvn_core::{Engine, TerminalRenderer};
use rvn_graph::*;
use rvn_parser::Value;

fn graph() -> GraphDocument {
    GraphDocument::new(GraphId::new(97001), GraphKind::Init)
}
fn add(g: &mut GraphDocument, kind: NodeKind) -> NodeId {
    g.add_catalog_node(kind, [0.0, 0.0]).unwrap()
}
fn strict(g: &mut GraphDocument, kind: NodeKind) -> NodeId {
    let id = add(g, kind);
    g.enable_blueprint_operator_policy(id).unwrap();
    id
}
fn pin(g: &GraphDocument, node: NodeId, key: &str) -> PinId {
    g.pin_by_key(node, key).unwrap().id
}
fn literal(g: &mut GraphDocument, value: PropertyValue) -> NodeId {
    let node = add(g, NodeKind::Literal);
    g.set_property(node, "value", value).unwrap();
    node
}
fn wire(
    g: &mut GraphDocument,
    source: NodeId,
    output: &str,
    target: NodeId,
    input: &str,
) -> EdgeId {
    g.connect(pin(g, source, output), pin(g, target, input))
        .unwrap()
}
fn ty(g: &GraphDocument, node: NodeId, key: &str) -> ValueType {
    g.effective_pin_type(pin(g, node, key)).unwrap()
}
fn evaluate(g: &GraphDocument, node: NodeId, key: &str) -> Value {
    let expression = transpile_value_output(g, node, key).unwrap();
    let ast = rvn_parser::parse(&format!("init {{ set result = {expression} }}")).unwrap();
    Engine::new(ast, TerminalRenderer, 8).unwrap().state.vars["result"].clone()
}

#[test]
fn new_blueprint_palette_domains_are_numeric_and_string_append_is_distinct() {
    for kind in [
        NodeKind::MathAdd,
        NodeKind::MathSubtract,
        NodeKind::MathMultiply,
        NodeKind::MathDivide,
        NodeKind::MathNegate,
        NodeKind::MathLess,
        NodeKind::MathLessEqual,
        NodeKind::MathGreater,
        NodeKind::MathGreaterEqual,
    ] {
        for value in [ValueType::Int, ValueType::Float, ValueType::Any] {
            assert!(kind.accepts_blueprint_data_source(&value));
        }
        for value in [
            ValueType::Bool,
            ValueType::String,
            ValueType::InterpolatedText,
            ValueType::List(Box::new(ValueType::Int)),
        ] {
            assert!(
                !kind.accepts_blueprint_data_source(&value),
                "{kind:?} {value:?}"
            );
        }
    }
    assert!(NodeKind::StringAppend.accepts_blueprint_data_source(&ValueType::String));
    assert!(!NodeKind::StringAppend.accepts_blueprint_data_source(&ValueType::InterpolatedText));
    assert!(!NodeKind::StringAppend.accepts_blueprint_data_source(&ValueType::Int));
    assert!(NodeKind::BinaryOperator.accepts_blueprint_data_source(&ValueType::Int));
    assert!(!NodeKind::BinaryOperator.accepts_blueprint_data_source(&ValueType::String));
    assert!(NodeKind::UnaryOperator.accepts_blueprint_data_source(&ValueType::Bool));
    assert!(!NodeKind::UnaryOperator.accepts_blueprint_data_source(&ValueType::Int));
    assert!(ALL_NODE_KINDS.contains(&NodeKind::StringAppend));
    let definition = node_definition(NodeKind::StringAppend);
    assert_eq!(definition.title, "Append");
    assert!(definition.has_dynamic_pins);
}

#[test]
fn policy_is_explicit_serialized_and_never_enabled_by_generic_creation() {
    let mut g = graph();
    let old = add(&mut g, NodeKind::MathAdd);
    assert!(!g.has_blueprint_operator_policy(old));
    let new = strict(&mut g, NodeKind::MathAdd);
    assert!(g.has_blueprint_operator_policy(new));
    let restored = GraphDocument::from_json(&g.to_pretty_json().unwrap()).unwrap();
    assert!(!restored.has_blueprint_operator_policy(old));
    assert!(restored.has_blueprint_operator_policy(new));
    assert_eq!(restored, g);
    let unsupported = add(&mut g, NodeKind::Literal);
    let before = g.clone();
    assert!(matches!(
        g.enable_blueprint_operator_policy(unsupported),
        Err(GraphEditError::UnsupportedBlueprintOperator(_))
    ));
    assert_eq!(g, before);
}

#[test]
fn every_strict_numeric_operator_rejects_strings_booleans_and_default_values_atomically() {
    for kind in [
        NodeKind::MathAdd,
        NodeKind::MathSubtract,
        NodeKind::MathMultiply,
        NodeKind::MathDivide,
        NodeKind::MathNegate,
        NodeKind::MathLess,
        NodeKind::MathLessEqual,
        NodeKind::MathGreater,
        NodeKind::MathGreaterEqual,
    ] {
        let mut g = graph();
        let operator = strict(&mut g, kind);
        let key = if kind == NodeKind::MathNegate {
            "value"
        } else {
            "left"
        };
        for invalid in [
            PropertyValue::Bool(true),
            PropertyValue::String("42".into()),
        ] {
            let source = literal(&mut g, invalid.clone());
            let before = g.clone();
            assert!(
                g.connect(pin(&g, source, "value"), pin(&g, operator, key))
                    .is_err(),
                "{kind:?}"
            );
            assert_eq!(g, before);
            assert!(g
                .connect_with_conversions(
                    pin(&g, source, "value"),
                    pin(&g, operator, key),
                    [90.0, 0.0]
                )
                .is_err());
            assert_eq!(g, before);
            assert!(g.set_pin_default(operator, key, invalid).is_err());
            assert_eq!(g, before);
        }
    }
}

#[test]
fn strict_numeric_specialization_stays_true_to_runtime_and_resets_on_disconnect() {
    let mut g = graph();
    let a = strict(&mut g, NodeKind::MathAdd);
    let b = strict(&mut g, NodeKind::MathMultiply);
    assert_eq!(ty(&g, a, "value"), ValueType::Any);
    let int = literal(&mut g, PropertyValue::Int(8));
    let float = literal(&mut g, PropertyValue::Float(2.5));
    let edge = wire(&mut g, int, "value", a, "left");
    wire(&mut g, a, "value", b, "left");
    assert_eq!(ty(&g, a, "right"), ValueType::Int);
    assert_eq!(transpile_value_input(&g, a, "right").unwrap(), "0");
    let float_edge = wire(&mut g, float, "value", a, "right");
    assert_eq!(ty(&g, a, "left"), ValueType::Int);
    assert_eq!(ty(&g, a, "right"), ValueType::Float);
    assert_eq!(ty(&g, a, "value"), ValueType::Float);
    assert_eq!(ty(&g, b, "right"), ValueType::Float);
    assert_eq!(evaluate(&g, a, "value"), Value::Float(10.5));
    g.remove_edge(edge);
    g.remove_edge(float_edge);
    assert_eq!(ty(&g, a, "value"), ValueType::Any);
    assert_eq!(ty(&g, b, "value"), ValueType::Any);
    assert_eq!(g.effective_pin_default_value(pin(&g, a, "right")), None);
    wire(&mut g, int, "value", a, "right");
    assert_eq!(ty(&g, b, "value"), ValueType::Int);
}

#[test]
fn strict_equality_rejects_unrelated_families_in_both_orders_but_accepts_mixed_numbers() {
    for kind in [NodeKind::MathEqual, NodeKind::MathNotEqual] {
        for (first, second) in [
            (
                PropertyValue::Bool(true),
                PropertyValue::String("true".into()),
            ),
            (
                PropertyValue::String("true".into()),
                PropertyValue::Bool(true),
            ),
            (PropertyValue::Int(1), PropertyValue::String("1".into())),
        ] {
            let mut g = graph();
            let cmp = strict(&mut g, kind);
            let a = literal(&mut g, first);
            let b = literal(&mut g, second);
            wire(&mut g, a, "value", cmp, "left");
            let before = g.clone();
            let source_type = ty(&g, b, "value");
            let target = pin(&g, cmp, "right");
            assert!(!g.accepts_pin_source(target, &source_type));
            assert!(!g.accepts_pin_source_with_types(
                target,
                &source_type,
                &g.effective_pin_types()
            ));
            assert!(g.connect(pin(&g, b, "value"), target).is_err());
            assert!(g
                .connect_with_conversions(pin(&g, b, "value"), target, [0.0, 0.0])
                .is_err());
            assert_eq!(g, before);
        }
        let mut g = graph();
        let cmp = strict(&mut g, kind);
        let a = literal(&mut g, PropertyValue::Int(3));
        let b = literal(&mut g, PropertyValue::Float(3.0));
        wire(&mut g, a, "value", cmp, "left");
        wire(&mut g, b, "value", cmp, "right");
        assert_eq!(ty(&g, cmp, "value"), ValueType::Bool);
        assert_eq!(
            evaluate(&g, cmp, "value"),
            Value::Bool(kind == NodeKind::MathEqual)
        );
    }
}

#[test]
fn equality_reconnection_ignores_its_old_target_but_respects_authored_other_values() {
    let mut g = graph();
    let cmp = strict(&mut g, NodeKind::MathEqual);
    let boolean = literal(&mut g, PropertyValue::Bool(false));
    let number = literal(&mut g, PropertyValue::Int(0));
    let edge = wire(&mut g, boolean, "value", cmp, "left");
    assert!(
        g.accepts_pin_source(pin(&g, cmp, "left"), &ValueType::Int),
        "implicit sibling default is not authored evidence"
    );
    assert!(!g.accepts_pin_source(pin(&g, cmp, "right"), &ValueType::Int));
    g.remove_edge(edge);
    assert_eq!(ty(&g, cmp, "left"), ValueType::Any);
    assert_eq!(ty(&g, cmp, "right"), ValueType::Any);
    wire(&mut g, number, "value", cmp, "right");
    assert_eq!(ty(&g, cmp, "left"), ValueType::Int);
    g.set_pin_default(cmp, "left", PropertyValue::Int(7))
        .unwrap();
    assert!(!g.accepts_pin_source(pin(&g, cmp, "right"), &ValueType::Bool));
    assert!(g.accepts_pin_source(pin(&g, cmp, "right"), &ValueType::Float));
}

#[test]
fn text_equality_keeps_text_defaults_real_distinct_and_persistent() {
    let mut g = graph();
    let cmp = strict(&mut g, NodeKind::MathEqual);
    let text = add(&mut g, NodeKind::TextValue);
    g.set_property(text, "value", PropertyValue::String("hello".into()))
        .unwrap();
    let string = literal(&mut g, PropertyValue::String("hello".into()));
    let left_edge = wire(&mut g, text, "value", cmp, "left");
    let right = pin(&g, cmp, "right");
    let before = g.to_pretty_json().unwrap();
    assert_eq!(ty(&g, cmp, "right"), ValueType::InterpolatedText);
    assert_eq!(
        g.effective_pin_default_value(right),
        Some(PropertyValue::String(String::new()))
    );
    assert_eq!(
        transpile_value_input(&g, cmp, "right").unwrap(),
        "string_to_text(\"\")"
    );
    assert_eq!(g.to_pretty_json().unwrap(), before);
    assert!(!g.accepts_pin_source(right, &ValueType::String));
    assert!(g.connect(pin(&g, string, "value"), right).is_err());
    g.set_blueprint_pin_default(
        right,
        PropertyValue::String("hello".into()),
        ValueType::InterpolatedText,
    )
    .unwrap();
    assert_eq!(evaluate(&g, cmp, "value"), Value::Bool(true));
    assert_eq!(
        transpile_value_input(&g, cmp, "right").unwrap(),
        "string_to_text(\"hello\")"
    );
    let right_edge = wire(&mut g, text, "value", cmp, "right");
    g.remove_edge(right_edge);
    let mut restored = GraphDocument::from_json(&g.to_pretty_json().unwrap()).unwrap();
    assert_eq!(restored, g);
    assert!(restored.validate().is_empty());
    assert_eq!(ty(&restored, cmp, "right"), ValueType::InterpolatedText);
    restored.remove_edge(left_edge);
    restored.pins.get_mut(&right).unwrap().default_value = None;
    // A stale marker without a value is never evidence and never colors a pin.
    assert_eq!(ty(&restored, cmp, "right"), ValueType::Any);
    restored
        .set_blueprint_pin_default(
            right,
            PropertyValue::String("raw".into()),
            ValueType::String,
        )
        .unwrap();
    assert!(!restored.nodes[&cmp]
        .properties
        .contains_key("@blueprint_default_type:right"));
    assert_eq!(ty(&restored, cmp, "right"), ValueType::String);
    assert_eq!(
        transpile_value_input(&restored, cmp, "right").unwrap(),
        "\"raw\""
    );
}

#[test]
fn explicit_string_to_text_conversion_is_valid_without_coercing_equality_silently() {
    let mut g = graph();
    let cmp = strict(&mut g, NodeKind::MathEqual);
    let text = add(&mut g, NodeKind::TextValue);
    g.set_property(text, "value", PropertyValue::String("x".into()))
        .unwrap();
    let raw = literal(&mut g, PropertyValue::String("x".into()));
    wire(&mut g, text, "value", cmp, "left");
    let conversion = add(&mut g, NodeKind::ConvertStringToText);
    wire(&mut g, raw, "value", conversion, "value");
    wire(&mut g, conversion, "result", cmp, "right");
    assert_eq!(evaluate(&g, cmp, "value"), Value::Bool(true));
    assert!(g.validate().is_empty());
}

#[test]
fn append_is_variadic_ordered_string_and_empty_defaults_are_real_values() {
    let mut g = graph();
    let append = add(&mut g, NodeKind::StringAppend);
    assert!(g.has_blueprint_operator_policy(append));
    assert_eq!(ty(&g, append, "value"), ValueType::String);
    assert_eq!(evaluate(&g, append, "value"), Value::Str(String::new()));
    let extra = g.add_operator_operand(append).unwrap();
    assert_eq!(g.pins[&extra].key, "operand_2");
    assert_eq!(g.pins[&extra].label, "C");
    assert_eq!(
        g.pins[&extra].default_value,
        Some(PropertyValue::String(String::new()))
    );
    g.set_pin_default(append, "left", PropertyValue::String("first/".into()))
        .unwrap();
    g.set_pin_default(append, "right", PropertyValue::String("second/".into()))
        .unwrap();
    g.set_pin_default(append, "operand_2", PropertyValue::String("third".into()))
        .unwrap();
    assert_eq!(
        evaluate(&g, append, "value"),
        Value::Str("first/second/third".into())
    );
    assert_eq!(
        transpile_value_output(&g, append, "value").unwrap(),
        "((\"first/\" + \"second/\") + \"third\")"
    );
    let before = g.clone();
    assert_eq!(g.materialize_visible_defaults().unwrap(), 0);
    assert_eq!(g, before);
    let restored = GraphDocument::from_json(&g.to_pretty_json().unwrap()).unwrap();
    assert_eq!(restored, g);
    let numeric = literal(&mut g, PropertyValue::Int(42));
    let before = g.clone();
    assert!(g
        .connect(pin(&g, numeric, "value"), pin(&g, append, "left"))
        .is_err());
    assert_eq!(g, before);
    let cast = g
        .connect_with_conversions(
            pin(&g, numeric, "value"),
            pin(&g, append, "left"),
            [180.0, 0.0],
        )
        .unwrap();
    assert_eq!(cast.conversions.len(), 1);
    assert_eq!(
        g.nodes[&cast.conversions[0]].kind,
        NodeKind::ConvertNumberToText
    );
    assert_eq!(
        evaluate(&g, append, "value"),
        Value::Str("42second/third".into())
    );
    g.remove_edge(cast.edge);
    assert_eq!(
        transpile_value_input(&g, append, "left").unwrap(),
        "\"first/\""
    );
}

#[test]
fn malformed_strict_wires_and_defaults_are_reported_but_legacy_remains_valid() {
    let mut g = graph();
    let cmp = add(&mut g, NodeKind::MathEqual);
    let add_node = add(&mut g, NodeKind::MathAdd);
    let text = literal(&mut g, PropertyValue::String("true".into()));
    let boolean = literal(&mut g, PropertyValue::Bool(true));
    wire(&mut g, text, "value", cmp, "left");
    wire(&mut g, boolean, "value", cmp, "right");
    wire(&mut g, text, "value", add_node, "left");
    wire(&mut g, boolean, "value", add_node, "right");
    assert!(g.validate().is_empty());
    assert_eq!(evaluate(&g, cmp, "value"), Value::Bool(false));
    assert_eq!(
        evaluate(&g, add_node, "value"),
        Value::Str("truetrue".into())
    );
    g.enable_blueprint_operator_policy(cmp).unwrap();
    g.enable_blueprint_operator_policy(add_node).unwrap();
    let errors = g.validate();
    assert!(errors
        .iter()
        .any(|d| d.code == "incompatible_types" && d.node == Some(cmp)));
    assert!(errors
        .iter()
        .any(|d| d.code == "incompatible_types" && d.node == Some(add_node)));
    assert_eq!(
        ty(&g, add_node, "value"),
        ValueType::Any,
        "invalid numeric graph must not pretend to return String"
    );
    let numeric = strict(&mut g, NodeKind::MathMultiply);
    let input = pin(&g, numeric, "left");
    g.pins.get_mut(&input).unwrap().default_value = Some(PropertyValue::Bool(true));
    assert!(g
        .validate()
        .iter()
        .any(|d| d.code == "incompatible_types" && d.pin == Some(input)));
}

#[test]
fn strict_unknowns_and_cycles_never_invent_a_producer_type() {
    let mut g = graph();
    g.add_variable("unknown", ValueType::Any, PropertyValue::Int(0))
        .unwrap();
    let unknown = add(&mut g, NodeKind::VariableGet);
    g.set_property(unknown, "name", PropertyValue::String("unknown".into()))
        .unwrap();
    let a = strict(&mut g, NodeKind::MathAdd);
    let b = strict(&mut g, NodeKind::MathMultiply);
    let number = literal(&mut g, PropertyValue::Int(3));
    wire(&mut g, unknown, "value", a, "left");
    wire(&mut g, number, "value", a, "right");
    wire(&mut g, a, "value", b, "left");
    assert_eq!(ty(&g, a, "left"), ValueType::Any);
    assert_eq!(ty(&g, a, "value"), ValueType::Any);
    assert_eq!(ty(&g, unknown, "value"), ValueType::Any);
    assert_eq!(g.effective_pin_default_value(pin(&g, b, "right")), None);
    let cmp = strict(&mut g, NodeKind::MathEqual);
    wire(&mut g, unknown, "value", cmp, "left");
    g.set_pin_default(cmp, "right", PropertyValue::Bool(false))
        .unwrap();
    assert_eq!(ty(&g, cmp, "left"), ValueType::Any);
    let output = pin(&g, b, "value");
    let input = pin(&g, a, "right");
    let current = g
        .edges
        .values()
        .find(|edge| edge.input == input)
        .unwrap()
        .id;
    g.remove_edge(current);
    let before = g.clone();
    assert!(matches!(
        g.connect(output, input),
        Err(GraphEditError::DataCycle { .. })
    ));
    assert_eq!(g, before);
    g.edges.insert(
        EdgeId::new(999_999),
        GraphEdge {
            id: EdgeId::new(999_999),
            output,
            input,
        },
    );
    let serialized = g.to_pretty_json().unwrap();
    let _ = g.effective_pin_types();
    let _ = g.effective_pin_default_values();
    let _ = g.validate();
    assert_eq!(g.to_pretty_json().unwrap(), serialized);
    assert_eq!(ty(&g, unknown, "value"), ValueType::Any);
}

#[test]
fn explicitly_opted_in_generic_operator_respects_its_actual_operator_property() {
    let mut g = graph();
    let binary = strict(&mut g, NodeKind::BinaryOperator);
    assert!(!g.accepts_pin_source(pin(&g, binary, "left"), &ValueType::String));
    g.set_property(binary, "operator", PropertyValue::String("==".into()))
        .unwrap();
    g.set_pin_default(binary, "left", PropertyValue::Bool(true))
        .unwrap();
    assert!(!g.accepts_pin_source(pin(&g, binary, "right"), &ValueType::String));
    let unary = strict(&mut g, NodeKind::UnaryOperator);
    assert!(g.accepts_pin_source(pin(&g, unary, "value"), &ValueType::Bool));
    assert!(!g.accepts_pin_source(pin(&g, unary, "value"), &ValueType::Int));
    g.set_property(unary, "operator", PropertyValue::String("-".into()))
        .unwrap();
    assert!(g.accepts_pin_source(pin(&g, unary, "value"), &ValueType::Int));
    assert!(!g.accepts_pin_source(pin(&g, unary, "value"), &ValueType::Bool));
}
