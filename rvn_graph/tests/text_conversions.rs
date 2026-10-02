use rvn_graph::*;

fn pin(graph: &GraphDocument, node: NodeId, key: &str) -> PinId {
    graph.pin_by_key(node, key).unwrap().id
}
fn add(graph: &mut GraphDocument, kind: NodeKind, x: f64) -> NodeId {
    graph.add_catalog_node(kind, [x, 80.0]).unwrap()
}
fn connect(
    graph: &mut GraphDocument,
    from: NodeId,
    key: &str,
    to: NodeId,
    input: &str,
) -> ConvertedConnection {
    graph
        .connect_with_conversions(pin(graph, from, key), pin(graph, to, input), [200.0, 160.0])
        .unwrap()
}

#[test]
fn string_and_text_are_distinct_and_each_cast_has_matching_wire_types() {
    assert!(!ValueType::String.accepts(&ValueType::InterpolatedText));
    assert!(!ValueType::InterpolatedText.accepts(&ValueType::String));
    for (kind, from, to) in [
        (
            NodeKind::ConvertStringToText,
            ValueType::String,
            ValueType::InterpolatedText,
        ),
        (
            NodeKind::ConvertTextToString,
            ValueType::InterpolatedText,
            ValueType::String,
        ),
    ] {
        let mut graph = GraphDocument::new(GraphId(1), GraphKind::Init);
        let cast = add(&mut graph, kind, 200.0);
        assert_eq!(graph.pin_by_key(cast, "value").unwrap().value_type, from);
        assert_eq!(graph.pin_by_key(cast, "result").unwrap().value_type, to);
        assert_eq!(to.conversion_from(&from), Some(kind));
    }
}

#[test]
fn casts_are_atomic_and_graph_snapshots_restore_undo_redo_without_identity_loss() {
    let mut graph = GraphDocument::new(
        GraphId(2),
        GraphKind::Label {
            name: "start".into(),
        },
    );
    let value = add(&mut graph, NodeKind::Literal, 0.0);
    let dialogue = add(&mut graph, NodeKind::Dialogue, 400.0);
    let before = graph.clone();
    let converted = connect(&mut graph, value, "value", dialogue, "text");
    assert_eq!(converted.conversions.len(), 1);
    assert_eq!(graph.nodes[&value], before.nodes[&value]);
    assert_eq!(graph.nodes[&dialogue], before.nodes[&dialogue]);
    let after = graph.clone();
    assert!(graph
        .connect_with_conversions(
            pin(&graph, value, "value"),
            pin(&graph, dialogue, "text"),
            [10.0, 20.0]
        )
        .is_err());
    assert_eq!(graph, after);
    graph = before.clone(); // the editor's ordinary graph snapshot Undo
    assert_eq!(graph, before);
    graph = after.clone(); // Redo includes the cast and both wires as one edit
    assert_eq!(graph, after);
    for edge in graph.edges.values() {
        assert_eq!(
            graph.pins[&edge.output].value_type,
            graph.pins[&edge.input].value_type
        );
    }
}

#[test]
fn numeric_text_conversions_use_complete_visible_chains() {
    let mut graph = GraphDocument::new(GraphId(3), GraphKind::Init);
    let number = add(&mut graph, NodeKind::Literal, 0.0);
    graph
        .set_property(number, "value", PropertyValue::Float(12.5))
        .unwrap();
    let dialogue = add(&mut graph, NodeKind::Dialogue, 500.0);
    let chain = connect(&mut graph, number, "value", dialogue, "text");
    assert_eq!(
        chain
            .conversions
            .iter()
            .map(|id| graph.nodes[id].kind)
            .collect::<Vec<_>>(),
        vec![NodeKind::ConvertNumberToText, NodeKind::ConvertStringToText]
    );
    let text = add(&mut graph, NodeKind::TextValue, 0.0);
    let int = add(&mut graph, NodeKind::ConvertIntToFloat, 500.0);
    let chain = connect(&mut graph, text, "value", int, "value");
    assert_eq!(
        chain
            .conversions
            .iter()
            .map(|id| graph.nodes[id].kind)
            .collect::<Vec<_>>(),
        vec![NodeKind::ConvertTextToString, NodeKind::ConvertTextToInt]
    );
    assert!(!graph
        .validate()
        .iter()
        .any(|d| d.code == "incompatible_types"));
    assert_eq!(graph.materialize_implicit_conversions().unwrap(), 0);
}

#[test]
fn v2_direct_string_wire_upgrades_in_memory_preserving_ids_layout_and_interpolation() {
    let mut graph = GraphDocument::new(
        GraphId(4),
        GraphKind::Label {
            name: "start".into(),
        },
    );
    let root = add(&mut graph, NodeKind::Label, 0.0);
    let text = add(&mut graph, NodeKind::Literal, 100.0);
    graph
        .set_property(
            text,
            "value",
            PropertyValue::String("Été [score + 1]".into()),
        )
        .unwrap();
    let dialogue = add(&mut graph, NodeKind::Dialogue, 500.0);
    graph
        .connect(
            pin(&graph, root, "exec_out"),
            pin(&graph, dialogue, "exec_in"),
        )
        .unwrap();
    let output = pin(&graph, text, "value");
    let input = pin(&graph, dialogue, "text");
    let id = EdgeId(99);
    let mut raw = serde_json::to_value(&graph).unwrap();
    raw["schema_version"] = serde_json::json!(2);
    raw["edges"]["99"] = serde_json::to_value(GraphEdge { id, output, input }).unwrap();
    let source = serde_json::to_string(&raw).unwrap();
    let (mut restored, report) = GraphDocument::from_json_with_report(&source).unwrap();
    assert_eq!(report.from, 2);
    assert_eq!(report.to, GRAPH_SCHEMA_VERSION);
    assert!(report.steps.contains(&"v2_to_v3_explicit_text_conversions"));
    assert_eq!(restored.nodes[&text], graph.nodes[&text]);
    assert_eq!(restored.nodes[&dialogue], graph.nodes[&dialogue]);
    assert_eq!(restored.edges[&id].input, input);
    assert!(restored
        .nodes
        .values()
        .any(|node| node.kind == NodeKind::ConvertStringToText));
    let generated = transpile(&restored).unwrap();
    assert_eq!(generated.source, "label start\n    \"Été [score + 1]\"\n");
    let mut vars = std::collections::HashMap::new();
    vars.insert("score".into(), rvn_parser::Value::Int(5));
    let rvn_parser::Statement::Dialogue { text, .. } = &generated.ast[1] else {
        panic!()
    };
    assert_eq!(
        rvn_core::eval::eval_interpolated(text, &vars).unwrap(),
        "Été 6"
    );
    assert_eq!(restored.materialize_implicit_conversions().unwrap(), 0);
    assert_eq!(
        GraphDocument::from_json(&restored.to_pretty_json().unwrap()).unwrap(),
        restored
    );
    assert_eq!(source, serde_json::to_string(&raw).unwrap());
}

#[test]
fn dialogue_casts_survive_source_reopen_without_changing_locale_keys_or_placement() {
    let source = "// Keep this comment\nlabel start\n    \"Été [score + 1]\"\n";
    let mut project = SourceProject::open(source, &[]).unwrap();
    let mut graphs = project.graphs().to_vec();
    let graph = graphs
        .iter_mut()
        .find(|g| matches!(g.kind, GraphKind::Label { .. }))
        .unwrap();
    let text = graph
        .nodes
        .values()
        .find(|n| n.kind == NodeKind::TextValue)
        .unwrap()
        .id;
    let outgoing = graph
        .edges
        .values()
        .find(|e| graph.pins[&e.output].node == text)
        .unwrap()
        .clone();
    graph.remove_edge(outgoing.id);
    let to_string = add(graph, NodeKind::ConvertTextToString, 80.0);
    graph
        .connect(outgoing.output, pin(graph, to_string, "value"))
        .unwrap();
    let cast = graph
        .connect_with_conversions(
            pin(graph, to_string, "result"),
            outgoing.input,
            [360.0, 280.0],
        )
        .unwrap();
    graph
        .set_property(to_string, "legacy_passthrough", PropertyValue::Bool(true))
        .unwrap();
    for id in cast.conversions {
        graph
            .set_property(id, "legacy_passthrough", PropertyValue::Bool(true))
            .unwrap();
    }
    project.apply_visual(source, &graphs).unwrap();
    assert_eq!(project.source(), source);
    let reopened = SourceProject::open(project.source(), project.graphs()).unwrap();
    assert_eq!(reopened.graphs(), project.graphs());
}

#[test]
fn explicit_rvn_functions_roundtrip_and_reject_non_string_values() {
    let source="// Exact casts\ninit { set a = text_to_string(\"Été [score]\") set b = string_to_text(\"\") }\nlabel start\n    return\n";
    let original = rvn_parser::parse(source).unwrap();
    assert!(rvn_parser::validate_logic(&original, false).is_empty());
    let graphs = import_script(&original).unwrap();
    assert!(graphs
        .iter()
        .flat_map(|g| g.nodes.values())
        .any(|n| n.kind == NodeKind::ConvertTextToString));
    assert!(graphs
        .iter()
        .flat_map(|g| g.nodes.values())
        .any(|n| n.kind == NodeKind::ConvertStringToText));
    let generated = transpile_project(&graphs).unwrap();
    assert!(generated.source.contains("text_to_string(\"Été [score]\")"));
    assert!(generated.source.contains("string_to_text(\"\")"));
    let reopened = SourceProject::open(source, &graphs).unwrap();
    assert_eq!(reopened.graphs(), graphs);
    for name in ["string_to_text", "text_to_string"] {
        let call = |value| rvn_parser::Expr::Call {
            name: name.into(),
            args: vec![value],
        };
        let vars = std::collections::HashMap::new();
        assert_eq!(
            rvn_core::eval::eval_expr(&call(rvn_parser::Expr::Str("Été [score]".into())), &vars)
                .unwrap(),
            rvn_parser::Value::Str("Été [score]".into())
        );
        assert!(rvn_core::eval::eval_expr(&call(rvn_parser::Expr::Int(7)), &vars).is_err());
        assert!(!rvn_parser::validate_logic(
            &rvn_parser::parse(&format!(
                "init {{ set a = {name}() }}\nlabel start\nreturn\n"
            ))
            .unwrap(),
            false
        )
        .is_empty());
    }
}

#[test]
fn new_string_to_text_dialogue_cast_is_strict_and_roundtrips_in_rvn() {
    let mut graph = GraphDocument::new(
        GraphId(8),
        GraphKind::Label {
            name: "start".into(),
        },
    );
    let root = add(&mut graph, NodeKind::Label, 0.0);
    let string = add(&mut graph, NodeKind::Literal, 100.0);
    graph
        .set_property(
            string,
            "value",
            PropertyValue::String("Hello [score]".into()),
        )
        .unwrap();
    let dialogue = add(&mut graph, NodeKind::Dialogue, 500.0);
    graph
        .connect(
            pin(&graph, root, "exec_out"),
            pin(&graph, dialogue, "exec_in"),
        )
        .unwrap();
    let conversion = connect(&mut graph, string, "value", dialogue, "text");
    assert_eq!(
        graph.nodes[&conversion.conversions[0]]
            .properties
            .get("legacy_passthrough"),
        None
    );
    let generated = transpile(&graph).unwrap();
    let rvn_parser::Statement::Dialogue { text, .. } = &generated.ast[1] else {
        panic!()
    };
    assert_eq!(
        rvn_core::eval::eval_interpolated(text, &std::collections::HashMap::new()).unwrap(),
        "Hello [score]"
    );
    let mut vars = std::collections::HashMap::new();
    vars.insert("score".into(), rvn_parser::Value::Int(5));
    assert_eq!(
        rvn_core::eval::eval_interpolated(text, &vars).unwrap(),
        "Hello [score]"
    );
    let imported = import_script(&generated.ast).unwrap();
    assert!(imported
        .iter()
        .flat_map(|g| g.nodes.values())
        .any(|n| n.kind == NodeKind::ConvertStringToText));
    assert_eq!(transpile(&imported[1]).unwrap().ast, generated.ast);
}

#[test]
fn imported_cast_dialogue_preserves_execution_continuation_and_choice_bodies() {
    for source in [
        "label start\n\"[string_to_text(\\\"Hello\\\")]\"\n\"Next\"\nreturn\n",
        "label start\nchoice { \"Option\" => { \"[string_to_text(\\\"Hello\\\")]\" \"Next\" return } }\nreturn\n",
    ] {
        let parsed=rvn_parser::parse(source).unwrap();
        let graphs=import_script(&parsed).unwrap();
        let compiled=transpile(&graphs[1]).unwrap();
        assert_eq!(compiled.ast,parsed);
        for graph in &graphs {assert!(!graph.validate().iter().any(|diagnostic| diagnostic.severity==DiagnosticSeverity::Error));}
    }
}

#[test]
fn setter_cast_reads_the_assigned_random_value_without_evaluating_it_twice() {
    let parsed = rvn_parser::parse(
        "init { set score = 0 }\nlabel start\nset score = random(1, 10)\nreturn\n",
    )
    .unwrap();
    let mut graphs = import_script(&parsed).unwrap();
    let graph = &mut graphs[1];
    let setter = graph
        .nodes
        .values()
        .find(|node| node.kind == NodeKind::SetVariable)
        .unwrap()
        .id;
    let return_node = graph
        .nodes
        .values()
        .find(|node| node.kind == NodeKind::Return)
        .unwrap()
        .id;
    let previous = graph
        .edges
        .values()
        .find(|edge| edge.input == pin(graph, return_node, "exec_in"))
        .unwrap()
        .id;
    graph.remove_edge(previous);
    let dialogue = add(graph, NodeKind::Dialogue, 900.0);
    graph
        .connect(
            pin(graph, setter, "exec_out"),
            pin(graph, dialogue, "exec_in"),
        )
        .unwrap();
    graph
        .connect(
            pin(graph, dialogue, "exec_out"),
            pin(graph, return_node, "exec_in"),
        )
        .unwrap();
    connect(graph, setter, "value_out", dialogue, "text");
    let generated = transpile(graph).unwrap();
    assert_eq!(generated.source.matches("random(").count(), 1);
    let rvn_parser::Statement::Dialogue { text, .. } = &generated.ast[2] else {
        panic!()
    };
    let mut vars = std::collections::HashMap::new();
    vars.insert("score".into(), rvn_parser::Value::Int(7));
    assert_eq!(rvn_core::eval::eval_interpolated(text, &vars).unwrap(), "7");
}

#[test]
fn source_refresh_preserves_text_annotations_and_getter_layout_but_refreshes_defaults() {
    let source = "init { set greeting = \"Hello\" }\nlabel start\n\"[greeting]\"\nreturn\n";
    let mut project = SourceProject::open(source, &[]).unwrap();
    let mut graphs = project.graphs().to_vec();
    for graph in &mut graphs {
        graph.variables.get_mut("greeting").unwrap().value_type = ValueType::InterpolatedText;
        for node in graph.nodes.values() {
            if matches!(node.kind, NodeKind::VariableGet | NodeKind::SetVariable) {
                for id in &node.pins {
                    let pin = graph.pins.get_mut(id).unwrap();
                    if matches!(pin.key.as_str(), "value" | "value_out") {
                        pin.value_type = ValueType::InterpolatedText;
                    }
                }
            }
        }
        graph.materialize_implicit_conversions().unwrap();
    }
    let label = graphs
        .iter_mut()
        .find(|graph| matches!(graph.kind, GraphKind::Label { .. }))
        .unwrap();
    let getter = label
        .nodes
        .values()
        .find(|node| node.kind == NodeKind::VariableGet)
        .unwrap()
        .id;
    label.nodes.get_mut(&getter).unwrap().position = [8172.0, -1337.0];
    project.apply_visual(source, &graphs).unwrap();
    project
        .refresh(
            "init { set greeting = \"Bonjour\" }\nlabel start\n\"[greeting]\"\n\"New\"\nreturn\n",
        )
        .unwrap();
    assert!(project
        .graphs()
        .iter()
        .all(|graph| graph.variables["greeting"].value_type == ValueType::InterpolatedText));
    assert!(project
        .graphs()
        .iter()
        .all(|graph| graph.variables["greeting"].default_value
            == PropertyValue::String("Bonjour".into())));
    let label = project
        .graphs()
        .iter()
        .find(|graph| matches!(graph.kind, GraphKind::Label { .. }))
        .unwrap();
    assert_eq!(label.nodes[&getter].kind, NodeKind::VariableGet);
    assert_eq!(label.nodes[&getter].position, [8172.0, -1337.0]);
    project
        .refresh("init { set greeting = 7 }\nlabel start\n\"[greeting]\"\n\"New\"\nreturn\n")
        .unwrap();
    assert!(project
        .graphs()
        .iter()
        .all(|graph| graph.variables["greeting"].value_type == ValueType::Int));
    assert!(project
        .graphs()
        .iter()
        .all(|graph| graph.variables["greeting"].default_value == PropertyValue::Int(7)));
    let label = project
        .graphs()
        .iter()
        .find(|graph| matches!(graph.kind, GraphKind::Label { .. }))
        .unwrap();
    let getter = label
        .nodes
        .values()
        .find(|node| node.kind == NodeKind::VariableGet)
        .unwrap()
        .id;
    assert_eq!(
        label.pin_by_key(getter, "value").unwrap().value_type,
        ValueType::Int
    );
}

#[test]
fn explicit_cast_source_declares_text_and_string_and_does_not_coerce_numbers() {
    let parsed=rvn_parser::parse("init { set text = string_to_text(\"Hello\") set string = text_to_string(\"Hello\") set dynamic_text = string_to_text(string) set dynamic_string = text_to_string(text) }\nlabel start\nreturn\n").unwrap();
    let graphs = import_script(&parsed).unwrap();
    assert!(graphs
        .iter()
        .all(|graph| graph.variables["text"].value_type == ValueType::InterpolatedText));
    assert!(graphs
        .iter()
        .all(|graph| graph.variables["string"].value_type == ValueType::String));
    assert!(graphs
        .iter()
        .all(
            |graph| graph.variables["dynamic_text"].value_type == ValueType::InterpolatedText
                && graph.variables["dynamic_text"].default_value
                    == PropertyValue::String(String::new())
        ));
    assert!(graphs
        .iter()
        .all(
            |graph| graph.variables["dynamic_string"].value_type == ValueType::String
                && graph.variables["dynamic_string"].default_value
                    == PropertyValue::String(String::new())
        ));
    for source in [
        "init { set n = 7 set bad = string_to_text(n) }\nlabel start\nreturn\n",
        "init { set bad = text_to_string(7) }\nlabel start\nreturn\n",
    ] {
        assert!(import_script(&rvn_parser::parse(source).unwrap()).is_err());
    }
}
