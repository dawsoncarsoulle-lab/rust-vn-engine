use rvn_graph::*;

fn pin(g: &GraphDocument, n: NodeId, key: &str) -> PinId {
    g.pin_by_key(n, key).unwrap().id
}
fn add(g: &mut GraphDocument, kind: NodeKind) -> NodeId {
    g.add_catalog_node(kind, [0.0, 0.0]).unwrap()
}
fn literal(g: &mut GraphDocument, value: PropertyValue) -> NodeId {
    let n = add(g, NodeKind::Literal);
    g.set_property(n, "value", value).unwrap();
    n
}
fn get(g: &mut GraphDocument, name: &str) -> NodeId {
    let n = add(g, NodeKind::VariableGet);
    g.set_property(n, "name", PropertyValue::String(name.into()))
        .unwrap();
    n
}
fn ty(g: &GraphDocument, n: NodeId, key: &str) -> ValueType {
    g.effective_pin_type(pin(g, n, key)).unwrap()
}
fn raw_connect(
    g: &mut GraphDocument,
    from: NodeId,
    output: &str,
    to: NodeId,
    input: &str,
) -> EdgeId {
    g.connect(pin(g, from, output), pin(g, to, input)).unwrap()
}
fn graph() -> GraphDocument {
    GraphDocument::new(GraphId(1), GraphKind::Init)
}

#[test]
fn known_globals_remain_typed_in_handlers_and_calculations() {
    let source = r#"init { set count=0 set last_event="ready" set key="" }
handler update(event) { local scratch=4 for item in [1,2] {set count=count+item} set last_event=event["kind"] set key=event["key"] }
function read(event) {return count}
screen view() {return component("root","text",{"text":last_event},[])}
label start
return
"#;
    let documents = import_script(&rvn_parser::parse(source).unwrap()).unwrap();
    for document in &documents {
        for (name, expected) in [
            ("count", ValueType::Int),
            ("last_event", ValueType::String),
            ("key", ValueType::String),
        ] {
            assert_eq!(
                document.variables[name].value_type, expected,
                "{:?} {name}",
                document.kind
            );
            assert_eq!(document.variable_scope(name), Some(VariableScope::Global));
            for node in document.nodes.values().filter(|node| {
                node.kind == NodeKind::VariableGet
                    && node.properties.get("name") == Some(&PropertyValue::String(name.into()))
            }) {
                assert_eq!(ty(document, node.id, "value"), expected);
            }
        }
    }
    let handler = documents
        .iter()
        .find(|g| matches!(g.kind, GraphKind::Handler { .. }))
        .unwrap();
    assert_eq!(
        handler.variable_scope("event"),
        Some(VariableScope::Parameter)
    );
    assert_eq!(handler.variables["event"].value_type, ValueType::Any);
    assert_eq!(
        handler.variable_scope("scratch"),
        Some(VariableScope::Local)
    );
    assert_eq!(handler.variables["scratch"].value_type, ValueType::Int);
    assert_eq!(handler.variable_scope("item"), Some(VariableScope::Local));
    assert_eq!(handler.variables["item"].value_type, ValueType::Int);
    for node in handler
        .nodes
        .values()
        .filter(|node| node.kind == NodeKind::Index)
    {
        assert_eq!(ty(handler, node.id, "value"), ValueType::Any);
    }
}

#[test]
fn parameters_and_local_shadowing_never_invent_a_global_type() {
    let source = r#"init{set count=0 set item="global" set param="global"}
function shadow(param) {set count="local" for item in [1,2] {local other=7} return count}
handler h(count) {set count=count+1 local item=1 return count}
label start
return
"#;
    let documents = import_script(&rvn_parser::parse(source).unwrap()).unwrap();
    let function = documents
        .iter()
        .find(|g| matches!(g.kind, GraphKind::Function { .. }))
        .unwrap();
    assert_eq!(function.variable_scope("count"), Some(VariableScope::Local));
    assert_eq!(
        function.variables["count"].value_type,
        ValueType::Any,
        "different local/global writes are conservatively unknown"
    );
    assert_eq!(
        function.variable_scope("param"),
        Some(VariableScope::Parameter)
    );
    assert_eq!(function.variables["param"].value_type, ValueType::Any);
    let handler = documents
        .iter()
        .find(|g| matches!(g.kind, GraphKind::Handler { .. }))
        .unwrap();
    assert_eq!(
        handler.variable_scope("count"),
        Some(VariableScope::Parameter)
    );
    assert_eq!(handler.variables["count"].value_type, ValueType::Any);
    assert_eq!(handler.variable_scope("item"), Some(VariableScope::Local));
    assert_eq!(handler.variables["item"].value_type, ValueType::Any);
    let init = documents
        .iter()
        .find(|g| g.kind == GraphKind::Init)
        .unwrap();
    assert_eq!(init.variables["count"].value_type, ValueType::Int);
    assert_eq!(init.variables["item"].value_type, ValueType::String);
}

#[test]
fn stale_variable_pin_cache_cannot_override_a_known_or_unknown_registry() {
    let mut g = graph();
    g.add_variable("count", ValueType::Int, PropertyValue::Int(0))
        .unwrap();
    g.add_variable("value", ValueType::Any, PropertyValue::Int(0))
        .unwrap();
    let count = get(&mut g, "count");
    let value = get(&mut g, "value");
    let count_pin = pin(&g, count, "value");
    let value_pin = pin(&g, value, "value");
    g.pins.get_mut(&count_pin).unwrap().value_type = ValueType::Any;
    g.pins.get_mut(&value_pin).unwrap().value_type = ValueType::String;
    let before = g.clone();
    let types = g.effective_pin_types();
    assert_eq!(types[&count_pin], ValueType::Int);
    assert_eq!(types[&value_pin], ValueType::Any);
    assert_eq!(g, before, "inference is read-only");
}

#[test]
fn index_union_matches_its_wire_without_narrowing_authoring_constraint() {
    let mut g = graph();
    let key = literal(&mut g, PropertyValue::String("width".into()));
    let index = add(&mut g, NodeKind::Index);
    let edge = raw_connect(&mut g, key, "value", index, "index");
    assert_eq!(ty(&g, index, "index"), ValueType::String);
    assert_eq!(g.effective_edge_type(edge), Some(ValueType::String));
    assert_eq!(
        g.pin_constraint_type(pin(&g, index, "index")),
        Some(ValueType::IndexKey)
    );
    g.remove_edge(edge);
    let integer = literal(&mut g, PropertyValue::Int(0));
    let connection = g
        .connect_with_conversions(
            pin(&g, integer, "value"),
            pin(&g, index, "index"),
            [0.0, 0.0],
        )
        .unwrap();
    assert!(connection.conversions.is_empty());
    assert_eq!(ty(&g, index, "index"), ValueType::Int);
    g.remove_edge(connection.edge);
    let invalid = literal(&mut g, PropertyValue::Bool(false));
    let before = g.clone();
    assert!(g
        .connect(pin(&g, invalid, "value"), pin(&g, index, "index"))
        .is_err());
    assert_eq!(g, before);
}

#[test]
fn reroutes_follow_reconnected_sources_and_keep_downstream_conflicts_visible() {
    let mut g = graph();
    let string = literal(&mut g, PropertyValue::String("width".into()));
    let route = add(&mut g, NodeKind::Reroute);
    g.set_reroute_type(route, ValueType::String).unwrap();
    let edge = raw_connect(&mut g, string, "value", route, "value");
    assert_eq!(ty(&g, route, "value"), ValueType::String);
    assert_eq!(ty(&g, route, "value_out"), ValueType::String);
    let text_cast = add(&mut g, NodeKind::ConvertStringToText);
    raw_connect(&mut g, route, "value_out", text_cast, "value");
    g.remove_edge(edge);
    let number = literal(&mut g, PropertyValue::Int(3));
    let connection = g
        .connect_with_conversions(
            pin(&g, number, "value"),
            pin(&g, route, "value"),
            [0.0, 0.0],
        )
        .unwrap();
    assert!(connection.conversions.is_empty());
    assert_eq!(ty(&g, route, "value"), ValueType::Int);
    assert_eq!(ty(&g, route, "value_out"), ValueType::Int);
    assert!(g
        .validate()
        .iter()
        .any(|diagnostic| diagnostic.code == "incompatible_types"
            && diagnostic.node == Some(text_cast)));
}

#[test]
fn operators_infer_all_operands_and_generic_operator_properties() {
    let mut g = graph();
    let add_node = add(&mut g, NodeKind::MathAdd);
    g.set_pin_default(add_node, "left", PropertyValue::Int(1))
        .unwrap();
    g.set_pin_default(add_node, "right", PropertyValue::Int(2))
        .unwrap();
    assert_eq!(ty(&g, add_node, "value"), ValueType::Int);
    let extra = g.add_operator_operand(add_node).unwrap();
    g.pins.get_mut(&extra).unwrap().default_value = Some(PropertyValue::Float(0.5));
    assert_eq!(ty(&g, add_node, "value"), ValueType::Float);
    g.pins.get_mut(&extra).unwrap().default_value = Some(PropertyValue::String("x".into()));
    assert_eq!(ty(&g, add_node, "value"), ValueType::String);
    let multiply = add(&mut g, NodeKind::MathMultiply);
    g.set_pin_default(multiply, "left", PropertyValue::Int(1))
        .unwrap();
    g.set_pin_default(multiply, "right", PropertyValue::Int(2))
        .unwrap();
    let extra = g.add_operator_operand(multiply).unwrap();
    g.pins.get_mut(&extra).unwrap().default_value = Some(PropertyValue::Float(0.5));
    assert_eq!(ty(&g, multiply, "value"), ValueType::Float);
    let generic = add(&mut g, NodeKind::BinaryOperator);
    g.set_pin_default(generic, "left", PropertyValue::Int(1))
        .unwrap();
    g.set_pin_default(generic, "right", PropertyValue::Int(2))
        .unwrap();
    for (operator, expected) in [
        ("+", ValueType::Int),
        ("==", ValueType::Bool),
        ("and", ValueType::Bool),
        ("?", ValueType::Any),
    ] {
        g.set_property(generic, "operator", PropertyValue::String(operator.into()))
            .unwrap();
        assert_eq!(ty(&g, generic, "value"), expected);
    }
    let unary = add(&mut g, NodeKind::UnaryOperator);
    g.set_pin_default(unary, "value", PropertyValue::Int(1))
        .unwrap();
    g.set_property(unary, "operator", PropertyValue::String("-".into()))
        .unwrap();
    assert_eq!(ty(&g, unary, "result"), ValueType::Int);
}

#[test]
fn indexed_lists_and_literal_dictionaries_only_narrow_when_proven() {
    let source = r#"function values(unknown){local nums=[1,2] local a=nums[0] local b={"width":300,"caption":"Title"}["width"] local c=unknown["width"] return [a,b,c]} label start return"#;
    let documents = import_script(&rvn_parser::parse(source).unwrap()).unwrap();
    let function = documents
        .iter()
        .find(|g| matches!(g.kind, GraphKind::Function { .. }))
        .unwrap();
    let indices: Vec<_> = function
        .nodes
        .values()
        .filter(|node| node.kind == NodeKind::Index)
        .map(|node| ty(function, node.id, "value"))
        .collect();
    assert_eq!(
        indices,
        vec![ValueType::Int, ValueType::Int, ValueType::Any]
    );
    let unknown = function
        .nodes
        .values()
        .find(|node| {
            node.kind == NodeKind::VariableGet
                && node.properties.get("name") == Some(&PropertyValue::String("unknown".into()))
        })
        .unwrap();
    assert_eq!(ty(function, unknown.id, "value"), ValueType::Any);
}

#[test]
fn typed_results_cannot_sneak_through_a_wildcard_cache_into_wrong_pins() {
    let mut g = graph();
    let op = add(&mut g, NodeKind::MathAdd);
    g.set_pin_default(op, "left", PropertyValue::Int(1))
        .unwrap();
    g.set_pin_default(op, "right", PropertyValue::Int(2))
        .unwrap();
    let not = add(&mut g, NodeKind::LogicNot);
    let before = g.clone();
    assert!(g
        .connect(pin(&g, op, "value"), pin(&g, not, "value"))
        .is_err());
    assert_eq!(g, before);
    let number = literal(&mut g, PropertyValue::Int(1));
    let subtract = add(&mut g, NodeKind::MathSubtract);
    raw_connect(&mut g, number, "value", subtract, "left");
    let string = literal(&mut g, PropertyValue::String("bad".into()));
    let before = g.clone();
    assert!(g
        .connect_with_conversions(
            pin(&g, string, "value"),
            pin(&g, subtract, "right"),
            [0.0, 0.0]
        )
        .is_err());
    assert_eq!(g, before);
}

#[test]
fn new_integer_float_authoring_has_a_visible_precision_cast_but_numeric_string_is_a_union() {
    let mut g = graph();
    let number = literal(&mut g, PropertyValue::Int(3));
    let pause = add(&mut g, NodeKind::MotionPause);
    let connection = g
        .connect_with_conversions(
            pin(&g, number, "value"),
            pin(&g, pause, "seconds"),
            [100.0, 100.0],
        )
        .unwrap();
    assert_eq!(connection.conversions.len(), 1);
    assert_eq!(
        g.nodes[&connection.conversions[0]].kind,
        NodeKind::ConvertIntToFloat
    );
    let text = add(&mut g, NodeKind::ConvertNumberToText);
    let connection = g
        .connect_with_conversions(
            pin(&g, number, "value"),
            pin(&g, text, "value"),
            [100.0, 100.0],
        )
        .unwrap();
    assert!(connection.conversions.is_empty());
    assert_eq!(ty(&g, text, "value"), ValueType::Int);
    assert_eq!(g.effective_edge_type(connection.edge), Some(ValueType::Int));
}

#[test]
fn accepted_legacy_numeric_and_unknown_wires_keep_both_endpoint_colors_coherent() {
    let mut g = graph();
    let number = literal(&mut g, PropertyValue::Int(3));
    let pause = add(&mut g, NodeKind::MotionPause);
    let wire = raw_connect(&mut g, number, "value", pause, "seconds");
    assert_eq!(ty(&g, pause, "seconds"), ValueType::Int);
    assert_eq!(g.effective_edge_type(wire), Some(ValueType::Int));
    assert_eq!(
        g.pin_constraint_type(pin(&g, pause, "seconds")),
        Some(ValueType::Float)
    );
    let list = add(&mut g, NodeKind::ListLiteral);
    g.resize_value_inputs(list, 4).unwrap();
    for index in 0..4 {
        g.set_pin_default(list, &format!("item_{index}"), PropertyValue::Int(100))
            .unwrap();
    }
    let rect = add(&mut g, NodeKind::CanvasRect);
    let wire = raw_connect(&mut g, list, "list", rect, "rect");
    let ints = ValueType::List(Box::new(ValueType::Int));
    assert_eq!(ty(&g, rect, "rect"), ints);
    assert_eq!(g.effective_edge_type(wire), Some(ints));
    assert_eq!(
        g.pin_constraint_type(pin(&g, rect, "rect")),
        Some(ValueType::List(Box::new(ValueType::Float)))
    );
    g.add_variable("unknown", ValueType::Any, PropertyValue::Int(0))
        .unwrap();
    let unknown = get(&mut g, "unknown");
    let text = add(&mut g, NodeKind::ConvertStringToText);
    let wire = raw_connect(&mut g, unknown, "value", text, "value");
    assert_eq!(ty(&g, text, "value"), ValueType::Any);
    assert_eq!(g.effective_edge_type(wire), Some(ValueType::Any));
    assert_eq!(
        g.pin_constraint_type(pin(&g, text, "value")),
        Some(ValueType::String)
    );
    assert!(!g
        .validate()
        .iter()
        .any(|diagnostic| diagnostic.code == "incompatible_types"));
}

#[test]
fn reopening_stale_presentation_repairs_types_without_touching_source_ids_or_placement() {
    let source="// Exact comments\ninit { set count=0 set status=\"ready\" set text=\"Text\" }\nhandler update(event) {set count=count+1 set status=event[\"kind\"] local value=event[\"value\"] return value}\nlabel start\n\"[text]\"\nreturn\n";
    let original = SourceProject::open(source, &[]).unwrap();
    let mut old = original.graphs().to_vec();
    for g in &mut old {
        if g.kind == GraphKind::Init {
            g.variables.get_mut("text").unwrap().value_type = ValueType::InterpolatedText;
        }
        for node in g.nodes.values_mut() {
            node.position = [1000.0 + node.id.get() as f64 * 7.0, -300.0];
            node.title_override = Some("Custom presentation".into());
        }
        if matches!(g.kind, GraphKind::Handler { .. }) {
            for name in ["count", "status"] {
                g.variables.get_mut(name).unwrap().value_type = ValueType::Any;
            }
            g.variables.get_mut("value").unwrap().value_type = ValueType::String;
            for pin in g.pins.values_mut() {
                if matches!(
                    g.nodes[&pin.node].kind,
                    NodeKind::VariableGet | NodeKind::SetVariable | NodeKind::LocalVariable
                ) && matches!(pin.key.as_str(), "value" | "value_out")
                {
                    pin.value_type = ValueType::Any;
                }
            }
        }
    }
    let mut reopened = SourceProject::open(source, &old).unwrap();
    assert_eq!(reopened.source(), source);
    for g in reopened.graphs() {
        let prior = old.iter().find(|old| old.graph_id == g.graph_id).unwrap();
        for (id, node) in &prior.nodes {
            assert_eq!(g.nodes[id].position, node.position);
            assert_eq!(g.nodes[id].title_override, node.title_override);
            assert_eq!(g.nodes[id].pins, node.pins);
        }
        assert_eq!(g.variables["count"].value_type, ValueType::Int);
        assert_eq!(g.variables["status"].value_type, ValueType::String);
        assert_eq!(g.variables["text"].value_type, ValueType::InterpolatedText);
        if matches!(g.kind, GraphKind::Handler { .. }) {
            assert_eq!(g.variables["value"].value_type, ValueType::Any);
        }
    }
    let edited = reopened.graphs().to_vec();
    reopened.apply_visual(source, &edited).unwrap();
    assert_eq!(reopened.source(), source);
    for _ in 0..3 {
        reopened = SourceProject::open(reopened.source(), reopened.graphs()).unwrap();
        assert_eq!(reopened.source(), source);
    }
}

#[test]
fn conflicting_global_assignment_is_reported_without_rejecting_dynamic_rvn_or_casting() {
    for expression in ["\"wrong\"", "upper(\"wrong\")"] {
        let source = format!(
            "init{{set count=0}} handler h(event){{set count={expression}}} label start return"
        );
        let documents = import_script(&rvn_parser::parse(&source).unwrap()).unwrap();
        assert!(documents
            .iter()
            .all(|g| g.variables["count"].value_type == ValueType::Any));
        let handler = documents
            .iter()
            .find(|g| matches!(g.kind, GraphKind::Handler { .. }))
            .unwrap();
        assert!(handler
            .validate()
            .iter()
            .any(|diagnostic| diagnostic.code == "variable_type_conflict"
                && diagnostic.severity == DiagnosticSeverity::Warning));
        let exported = transpile(handler).unwrap();
        assert!(!exported.source.contains("to_int("));
        assert!(exported.source.contains(expression));
    }
}

#[test]
fn unknowns_empty_collections_and_cycles_remain_wildcard() {
    let mut g = graph();
    let list = literal(&mut g, PropertyValue::StringList(Vec::new()));
    assert_eq!(
        ty(&g, list, "value"),
        ValueType::List(Box::new(ValueType::Any))
    );
    let call = add(&mut g, NodeKind::FunctionCall);
    g.set_property(
        call,
        "function",
        PropertyValue::String("unknown_user_function".into()),
    )
    .unwrap();
    assert_eq!(ty(&g, call, "result"), ValueType::Any);
    let a = add(&mut g, NodeKind::Reroute);
    let b = add(&mut g, NodeKind::Reroute);
    raw_connect(&mut g, a, "value_out", b, "value");
    let id = EdgeId(999);
    g.edges.insert(
        id,
        GraphEdge {
            id,
            output: pin(&g, b, "value_out"),
            input: pin(&g, a, "value"),
        },
    );
    assert_eq!(ty(&g, a, "value_out"), ValueType::Any);
    assert_eq!(ty(&g, b, "value_out"), ValueType::Any);
    assert!(g
        .validate()
        .iter()
        .any(|diagnostic| diagnostic.code == "data_cycle"));
}

#[test]
fn depth_and_work_limits_return_complete_neutral_maps_with_execution_preserved() {
    let mut g = graph();
    let value = literal(&mut g, PropertyValue::Int(1));
    let mut tail = pin(&g, value, "value");
    for _ in 0..150 {
        let route = add(&mut g, NodeKind::Reroute);
        let input = pin(&g, route, "value");
        g.connect(tail, input).unwrap();
        tail = pin(&g, route, "value_out");
    }
    let types = g.effective_pin_types();
    assert_eq!(types.len(), g.pins.len());
    assert_eq!(g.effective_pin_type(tail), Some(ValueType::Any));
    // Disconnected stale caches after the global work budget must not escape
    // through a missing map entry. A data wildcard remains neutral; execution
    // is an explicit invariant and needs no recursive inference.
    let mut large = graph();
    for index in 0..100_010 {
        let node = large.add_node(NodeKind::FunctionCall, [0.0, 0.0]);
        large
            .add_pin(
                node,
                "result",
                "",
                PinDirection::Output,
                ValueType::Any,
                PinCardinality::Many,
            )
            .unwrap();
        if index == 100_005 {
            large
                .add_pin(
                    node,
                    "exec",
                    "",
                    PinDirection::Output,
                    ValueType::Execution,
                    PinCardinality::Many,
                )
                .unwrap();
        }
    }
    let types = large.effective_pin_types();
    assert_eq!(types.len(), large.pins.len());
    for (id, pin) in &large.pins {
        assert_eq!(
            types[id],
            if pin.value_type.is_execution() {
                ValueType::Execution
            } else {
                ValueType::Any
            }
        );
    }
}
