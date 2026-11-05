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

mod name_invariant1021 {
    use super::*;
    use rvn_parser::{localized_text::{DeferredText, TextLocalization}, Expr, Name, Value};
    use rvn_core::{Engine, Interaction, TerminalRenderer};
    use std::collections::HashMap;

    fn invariant(source: &str) -> Value { Value::Text(Box::new(DeferredText::invariant(source))) }
    fn locale() -> rvn_core::locale::LocaleManager {
        use rvn_core::locale::{LocaleManager, LocaleTable};
        LocaleManager::from_frozen_catalogs("en", "fr", vec!["en".into(), "fr".into()], HashMap::from([
            ("en".into(), LocaleTable { lang: "en".into(), strings: HashMap::new() }.into()),
            ("fr".into(), LocaleTable { lang: "fr".into(), strings: HashMap::from([
                ("FoO_2 [score] Été".into(), "NE PAS TRADUIRE".into()),
            ]) }.into()),
        ])).unwrap()
    }

    #[test]
    fn name_invariant_constructor_keeps_spelling_none_literal_brackets_and_locale_independence() {
        for name in [Name::default(), Name::new("").unwrap(), Name::new("FoO_2").unwrap(),
            Name::new("FoO_2 [score] Été").unwrap(), Name::new("東京_7").unwrap()] {
            let spelling = name.as_str().to_owned();
            let args = vec![Value::Name(name.clone())];
            let actual = rvn_core::localized_text::construct("__rvn_invariant_text", &args).unwrap();
            assert_eq!(actual, invariant(&spelling));
            let Value::Text(text) = &actual else { panic!("A Name must produce an actual portable Text") };
            assert!(matches!(text.as_ref(), DeferredText::Literal { text } if text.localization == TextLocalization::CultureInvariant && text.source == spelling));
            let mut languages = locale();
            for lang in ["fr", "en", "fr"] {
                languages.set_language(lang).unwrap();
                assert_eq!(rvn_core::localized_text::render_text(text, Some(&languages), &rvn_core::localized_text::VnTextScalars).unwrap(), spelling);
            }
            let expression = Expr::Call { name: "__rvn_invariant_text".into(), args: vec![Expr::Call { name: "__rvn_literal_name".into(), args: vec![Expr::Str(name.as_str().into())] }] };
            assert_eq!(rvn_core::eval::eval_expr(&expression, &HashMap::from([("score".into(), Value::Int(99))])).unwrap(), actual);
            assert_eq!(serde_json::from_str::<Value>(&serde_json::to_string(&actual).unwrap()).unwrap(), actual);
        }
        assert_eq!(Name::new("").unwrap().as_str(), "None");
        assert_eq!(rvn_core::localized_text::construct("__rvn_invariant_text", &[Value::Str("[score]".into())]).unwrap(), invariant("[score]"));
        for bad in [vec![], vec![Value::Int(7)], vec![Value::Float(7.0)], vec![Value::Bool(true)],
            vec![Value::List(vec![])], vec![Value::Dict(Default::default())], vec![invariant("text")],
            vec![Value::Name(Name::default()), Value::Str("extra".into())]] {
            assert!(rvn_core::localized_text::construct("__rvn_invariant_text", &bad).is_err());
        }
        assert_eq!(ValueType::InterpolatedText.conversion_from(&ValueType::Name), Some(NodeKind::FunctionCall));
        assert_eq!(ValueType::InterpolatedText.conversion_path_from(&ValueType::Name), Some(vec![NodeKind::FunctionCall]));
        assert!(ValueType::List(Box::new(ValueType::InterpolatedText)).conversion_specs_from(&ValueType::List(Box::new(ValueType::Name))).is_none());
    }

    #[test]
    fn name_invariant_real_wire_is_one_configured_node_atomic_and_never_repairs_old_invalid_wires() {
        let mut graph = GraphDocument::new(GraphId(102101), GraphKind::Label { name: "start".into() });
        let name = add(&mut graph, NodeKind::Literal, 0.0);
        graph.set_property(name, "value", PropertyValue::Name(NameDefault::new("FoO_2").unwrap())).unwrap();
        let sink = add(&mut graph, NodeKind::Dialogue, 600.0);
        let output = pin(&graph, name, "value"); let input = pin(&graph, sink, "text");
        let before = graph.clone();
        assert!(graph.connect(output, input).is_err()); assert_eq!(graph, before);
        assert!(graph.connect_with_conversions(input, output, [200.0, 0.0]).is_err()); assert_eq!(graph, before);
        let connection = graph.connect_with_conversions(output, input, [200.0, 0.0]).unwrap();
        let [cast] = connection.conversions.as_slice() else { panic!("One visible Name to invariant Text step required") };
        assert_eq!(graph.nodes[cast].kind, NodeKind::FunctionCall);
        assert_eq!(graph.nodes[cast].properties.get("function"), Some(&PropertyValue::String("__rvn_invariant_text".into())));
        assert_eq!(graph.effective_pin_type(pin(&graph, *cast, "item_0")), Some(ValueType::Name));
        assert_eq!(graph.effective_pin_type(pin(&graph, *cast, "result")), Some(ValueType::InterpolatedText));
        assert!(graph.pin_by_key(*cast, "item_0").unwrap().text_localization.is_none());
        assert!(graph.text_owner.is_none());
        assert_eq!(graph.nodes[&name], before.nodes[&name]); assert_eq!(graph.nodes[&sink], before.nodes[&sink]);
        let after = graph.clone();
        assert!(graph.connect_with_conversions(output, input, [200.0, 0.0]).is_err()); assert_eq!(graph, after);
        assert_eq!(GraphDocument::from_json(&graph.to_pretty_json().unwrap()).unwrap(), after);
        graph = before.clone(); assert_eq!(graph, before); graph = after.clone(); assert_eq!(graph, after);
        // An older retained incompatible wire is diagnostic authoring data,
        // never authorization to silently insert the new conversion on load.
        let mut raw = serde_json::to_value(&before).unwrap();
        raw["edges"]["900001"] = serde_json::to_value(GraphEdge { id: EdgeId(900001), output, input }).unwrap();
        raw["next_edge_id"] = serde_json::json!(900002);
        let mut retained: GraphDocument = serde_json::from_value(raw).unwrap(); let invalid = retained.clone();
        assert_eq!(retained.materialize_implicit_conversions().unwrap(), 0); assert_eq!(retained, invalid);
        assert!(retained.validate().iter().any(|d| d.code == "incompatible_types"));
        assert_eq!(GraphDocument::from_json(&retained.to_pretty_json().unwrap()).unwrap(), invalid);
    }

    #[test]
    fn name_invariant_local_type_migration_converts_actual_explicit_set_default_and_executes_text() {
        let source = "scene label start\nlocal value=__rvn_literal_name(\"None\")\nset value=__rvn_literal_name(\"FoO_2 [score] Été\")\n\"[value]\"\nreturn\n";
        let mut graph = import_script(&rvn_parser::parse_program(source).unwrap().script).unwrap().into_iter()
            .find(|g| matches!(&g.kind, GraphKind::Label { name } if name == "start")).unwrap();
        let assignment = graph.nodes.values().find(|n| n.kind == NodeKind::SetVariable).unwrap().id;
        let input = pin(&graph, assignment, "value");
        // Exercise an authored inline default, not an already wired RHS.
        graph.disconnect_pin(input);
        graph.set_pin_default(assignment, "value", PropertyValue::Name(NameDefault::new("FoO_2 [score] Été").unwrap())).unwrap();
        let initial = graph.local_variable_initializers("value").unwrap().remove(0);
        let dialogue = graph.nodes.values().find(|n| n.kind == NodeKind::Dialogue).unwrap().id;
        let dialogue_input = pin(&graph, dialogue, "text");
        let previous_cast = graph.pins[&graph.edges.values().find(|e| e.input == dialogue_input).unwrap().output].node;
        assert_eq!(graph.nodes[&previous_cast].kind, NodeKind::FunctionCall);
        assert_eq!(graph.nodes[&previous_cast].properties.get("function"), Some(&PropertyValue::String("__rvn_invariant_text".into())));
        let previous_input = pin(&graph, previous_cast, "item_0");
        let read_output = graph.edges.values().find(|e| e.input == previous_input).unwrap().output;
        assert_eq!(graph.nodes[&graph.pins[&read_output].node].kind, NodeKind::VariableGet);
        assert_eq!(graph.effective_pin_type(read_output), Some(ValueType::Name));
        let before = graph.clone();
        let mut next = graph.prepare_local_variable_type("value", &initial, ValueType::InterpolatedText).unwrap().reconstruct(&graph).unwrap();
        assert_eq!(graph, before);
        let casts: Vec<_> = next.nodes.values().filter(|n| n.kind == NodeKind::FunctionCall && n.properties.get("function") == Some(&PropertyValue::String("__rvn_invariant_text".into()))).map(|n| n.id).collect();
        assert_eq!(casts.len(), 2);
        let assignment_cast = next.pins[&next.edges.values().find(|e| e.input == input).unwrap().output].node;
        assert_ne!(assignment_cast, previous_cast);
        assert!(casts.contains(&assignment_cast) && casts.contains(&previous_cast));
        for (consumer, cast) in [(input, assignment_cast), (dialogue_input, previous_cast)] {
            let edges: Vec<_> = next.edges.values().filter(|e| e.input == consumer).collect();
            assert_eq!(edges.len(), 1);
            assert_eq!(edges[0].output, pin(&next, cast, "result"));
        }
        assert_eq!(next.effective_pin_type(pin(&next, assignment_cast, "item_0")), Some(ValueType::Name));
        let assignment_source = next.edges.values().find(|e| e.input == pin(&next, assignment_cast, "item_0")).unwrap().output;
        assert_eq!(next.nodes[&next.pins[&assignment_source].node].properties.get("value"), Some(&PropertyValue::Name(NameDefault::new("FoO_2 [score] Été").unwrap())));
        // Literal declaration policy resets its default to the requested type.
        // It is a TextValue producer, not a second assignment converter.
        let initial_input = pin(&next, initial.node, "value");
        let initial_source = next.edges.values().find(|e| e.input == initial_input).unwrap().output;
        let initial_value = &next.nodes[&next.pins[&initial_source].node];
        assert_eq!(initial_value.kind, NodeKind::TextValue);
        assert_eq!(initial_value.properties.get("value"), Some(&PropertyValue::String(String::new())));
        assert_ne!(initial.node, assignment);
        // The original [value] import already had a typed Name-to-Text call.
        // A local type edit retains that authoring node; it does not promise
        // that every old typed consumer becomes executable. Reconnect the
        // genuine now-Text read to the same Dialogue before cold/run checks.
        assert_eq!(next.nodes[&previous_cast], before.nodes[&previous_cast]);
        assert_eq!(next.effective_pin_type(read_output), Some(ValueType::InterpolatedText));
        next.remove_node(previous_cast);
        next.connect(read_output, dialogue_input).unwrap();
        assert_eq!(next.pins[&input].default_value, Some(PropertyValue::String(String::new())));
        let cold = GraphDocument::from_json(&next.to_pretty_json().unwrap()).unwrap(); assert_eq!(cold, next);
        let mut engine = Engine::new(transpile(&cold).unwrap().ast, TerminalRenderer, 16).unwrap();
        assert_eq!(engine.step_until_interaction().unwrap(), Some(Interaction::Dialogue { character: None, text: "FoO_2 [score] Été".into() }));
        assert!(matches!(&engine.state.scene_execution.as_ref().unwrap().active.as_ref().unwrap().locals["value"], Value::Text(text) if **text == DeferredText::invariant("FoO_2 [score] Été")));
        println!();
    }

    #[test]
    fn name_invariant_native_graph_source_cold_and_save15_preserve_actual_name_and_text_without_locale_key() {
        let source = "init {set named=__rvn_literal_name(\"FoO_2 [score] Été\") set text=__rvn_invariant_text(\"\") set score=99}\nscene label start\nset text=__rvn_invariant_text(__rvn_name_to_string(named))\n\"[text]\"\nreturn\n";
        let original = rvn_parser::parse_program(source).unwrap();
        let mut graphs = import_script(&original.script).unwrap();
        let graph = graphs.iter_mut().find(|g| matches!(&g.kind, GraphKind::Label { name } if name == "start")).unwrap();
        let old = graph.nodes.values().find(|n| n.kind == NodeKind::FunctionCall && n.properties.get("function") == Some(&PropertyValue::String("__rvn_invariant_text".into()))).unwrap().id;
        let old_input = pin(graph, old, "item_0");
        let bridge = graph.pins[&graph.edges.values().find(|e| e.input == old_input).unwrap().output].node;
        assert_eq!(graph.nodes[&bridge].kind, NodeKind::ConvertNameToString);
        let name_input = pin(graph, bridge, "value");
        let name_output = graph.edges.values().find(|e| e.input == name_input).unwrap().output;
        let old_output = pin(graph, old, "result");
        let destination = graph.edges.values().find(|e| e.output == old_output).unwrap().input;
        graph.remove_node(old); graph.remove_node(bridge);
        let connection = graph.connect_with_conversions(name_output, destination, [240.0, 80.0]).unwrap();
        assert_eq!(connection.conversions.len(), 1);
        let cast = connection.conversions[0]; let graph_id = graph.graph_id;
        assert_eq!(graph.effective_pin_type(pin(graph, cast, "result")), Some(ValueType::InterpolatedText));
        let folder = std::env::temp_dir().join(format!("rvn-name-invariant1021-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir(&folder).unwrap();
        // This Label reads the genuine Init/global Name declaration. The
        // standalone-scope helper intentionally supplies no foreign context;
        // use the public whole-project writer, whose Native preflight validates
        // every scope against the complete emitted Init/Scene source instead.
        for graph in &mut graphs { graph.enable_native_frame_preparation().unwrap(); }
        let compiled = transpile_project(&graphs).unwrap();
        let path = folder.join("main.rvn"); std::fs::write(&path, &compiled.source).unwrap();
        let frozen = rvn_parser::parse_file_program_with_uses(&path).unwrap();
        assert_eq!(frozen.script, compiled.ast);
        let reconstructed = import_native_frame_script(&frozen.script).unwrap();
        for expected in &graphs {
            let actual = reconstructed.iter().find(|g| g.graph_id == expected.graph_id).unwrap();
            assert_eq!(actual.edges, expected.edges);
            assert_eq!(actual.nodes.keys().collect::<Vec<_>>(), expected.nodes.keys().collect::<Vec<_>>());
            assert_eq!(actual.pins.keys().collect::<Vec<_>>(), expected.pins.keys().collect::<Vec<_>>());
            for (id, pin) in &expected.pins {
                assert_eq!((&actual.pins[id].key, actual.pins[id].direction), (&pin.key, pin.direction));
                assert_eq!(actual.effective_pin_type(*id), expected.effective_pin_type(*id));
            }
            for (id, node) in &expected.nodes { assert_eq!(actual.nodes[id].kind, node.kind); }
        }
        assert_eq!(transpile_project(&reconstructed).unwrap().ast, compiled.ast);
        let origins = NativeDebugSourceMap::from_program(&frozen).unwrap();
        let commands: Vec<_> = origins.origins().iter().filter(|o| o.kind == NodeKind::SetVariable).collect();
        assert!(!commands.is_empty());
        for command in commands {
            assert!(command.span.start < command.span.end);
            assert_eq!(command.source, std::fs::canonicalize(&path).unwrap());
            assert!(graphs.iter().any(|g| g.graph_id == command.graph && g.nodes.contains_key(&command.node)));
        }
        let actual = reconstructed.iter().find(|g| g.graph_id == graph_id).unwrap();
        assert_eq!(actual.nodes[&cast].properties.get("function"), Some(&PropertyValue::String("__rvn_invariant_text".into())));
        assert_eq!(actual.effective_pin_type(pin(actual, cast, "item_0")), Some(ValueType::Name));
        assert!(actual.pin_by_key(cast, "item_0").unwrap().text_localization.is_none());
        assert!(actual.pin_by_key(cast, "item_0").unwrap().text_table_reference.is_none());
        let mut workspace = SourceWorkspace::link(&folder, &folder.join("main.rvn"), &reconstructed).unwrap();
        let authoring = workspace.project().graphs().to_vec(); workspace.save(&authoring).unwrap();
        let cold_workspace = SourceWorkspace::open(&folder).unwrap().unwrap();
        assert!(cold_workspace.source_error.is_none()); assert_eq!(cold_workspace.project().graphs(), authoring);
        let frozen = cold_workspace.project().resolved_program().unwrap();
        let binding = rvn_core::consumer_schedule::ConsumerSourceBinding::from_program(&frozen).unwrap();
        let mut warm = Engine::from_program_with_consumer_binding(frozen.clone(), binding, TerminalRenderer, 16).unwrap();
        warm.locale = Some(locale());
        assert_eq!(warm.step_until_interaction().unwrap(), Some(Interaction::Dialogue { character: None, text: "FoO_2 [score] Été".into() }));
        assert!(matches!(&warm.state.vars["named"], Value::Name(name) if name.as_str() == "FoO_2 [score] Été"));
        assert_eq!(warm.state.vars["text"], invariant("FoO_2 [score] Été"));
        let random = warm.state.random;
        let saved = warm.capture_save15(&frozen, None, 1, "Name invariant actual source".into()).unwrap().encode().unwrap();
        let mut cold = Engine::from_program(frozen.clone(), TerminalRenderer, 16).unwrap(); cold.locale = Some(locale());
        cold.restore_save15(&frozen, rvn_core::save::consumer_checkpoint::Save15::decode(&saved).unwrap()).unwrap();
        assert_eq!(cold.state.vars["text"], invariant("FoO_2 [score] Été"));
        assert!(matches!(&cold.state.vars["named"], Value::Name(name) if name.as_str() == "FoO_2 [score] Été"));
        let before = serde_json::to_value(&cold.state).unwrap();
        for lang in ["en", "fr"] {
            cold.locale.as_mut().unwrap().set_language(lang).unwrap();
            assert_eq!(cold.displayed_interaction().unwrap(), Some(Interaction::Dialogue { character: None, text: "FoO_2 [score] Été".into() }));
        }
        assert_eq!(serde_json::to_value(&cold.state).unwrap(), before); assert_eq!(cold.state.random, random);
        std::fs::remove_dir_all(folder).unwrap();
        println!();
    }
}
