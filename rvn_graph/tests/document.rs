use rvn_graph::*;

fn pin(
    graph: &mut GraphDocument,
    node: NodeId,
    key: &str,
    direction: PinDirection,
    value_type: ValueType,
) -> PinId {
    graph
        .add_pin(node, key, key, direction, value_type, PinCardinality::One)
        .unwrap()
}

#[test]
fn document_round_trips_with_stable_ids() {
    let mut graph = GraphDocument::new(
        GraphId::new(7),
        GraphKind::Label {
            name: "start".into(),
        },
    );
    let literal = graph.add_node(NodeKind::Literal, [20.0, 30.0]);
    let setter = graph.add_node(NodeKind::SetVariable, [200.0, 30.0]);
    graph
        .add_variable("score", ValueType::Int, PropertyValue::Int(0))
        .unwrap();
    graph
        .set_property(setter, "name", PropertyValue::String("score".into()))
        .unwrap();
    let output = pin(
        &mut graph,
        literal,
        "value",
        PinDirection::Output,
        ValueType::Int,
    );
    let input = pin(
        &mut graph,
        setter,
        "value",
        PinDirection::Input,
        ValueType::Int,
    );
    graph.connect(output, input).unwrap();

    let json = graph.to_pretty_json().unwrap();
    let restored = GraphDocument::from_json(&json).unwrap();

    assert_eq!(restored, graph);
    assert!(restored.validate().is_empty());
    assert!(json.contains(&format!("\"schema_version\": {GRAPH_SCHEMA_VERSION}")));
}

#[test]
fn typed_variables_round_trip_and_rename_their_usages() {
    let mut graph = GraphDocument::new(GraphId::new(15), GraphKind::Init);
    graph
        .add_variable(
            "message",
            ValueType::String,
            PropertyValue::String("Bonjour".into()),
        )
        .unwrap();
    let getter = graph
        .add_catalog_node(NodeKind::VariableGet, [0.0, 0.0])
        .unwrap();
    graph
        .nodes
        .get_mut(&getter)
        .unwrap()
        .properties
        .insert("name".into(), PropertyValue::String("message".into()));

    graph.rename_variable("message", "salutation").unwrap();
    let restored = GraphDocument::from_json(&graph.to_pretty_json().unwrap()).unwrap();

    assert_eq!(
        restored.variables["salutation"].value_type,
        ValueType::String
    );
    assert_eq!(
        restored.nodes[&getter].properties["name"],
        PropertyValue::String("salutation".into())
    );
}

#[test]
fn loading_repairs_id_counters() {
    let mut graph = GraphDocument::new(GraphId::new(2), GraphKind::Init);
    assert_eq!(graph.add_node(NodeKind::Init, [0.0, 0.0]), NodeId::new(1));
    let mut json = serde_json::to_value(&graph).unwrap();
    json["next_node_id"] = serde_json::json!(0);

    let mut restored = GraphDocument::from_json(&serde_json::to_string(&json).unwrap()).unwrap();
    assert_eq!(
        restored.add_node(NodeKind::Config, [1.0, 0.0]),
        NodeId::new(2)
    );
}

#[test]
fn rejects_wrong_direction_and_incompatible_types() {
    let mut graph = GraphDocument::new(GraphId::new(1), GraphKind::Init);
    let a = graph.add_node(NodeKind::Literal, [0.0, 0.0]);
    let b = graph.add_node(NodeKind::SetVariable, [1.0, 0.0]);
    let input_a = pin(&mut graph, a, "in", PinDirection::Input, ValueType::Int);
    let input_b = pin(&mut graph, b, "in", PinDirection::Input, ValueType::String);
    assert!(matches!(
        graph.connect(input_a, input_b),
        Err(GraphEditError::InvalidDirection { .. })
    ));

    let output = pin(&mut graph, a, "out", PinDirection::Output, ValueType::Int);
    assert!(matches!(
        graph.connect(output, input_b),
        Err(GraphEditError::IncompatibleTypes { .. })
    ));
}

#[test]
fn text_and_interpolated_text_require_a_visible_conversion() {
    let mut graph = GraphDocument::new(GraphId::new(1), GraphKind::Init);
    let source_node = graph.add_node(NodeKind::VariableGet, [0.0, 0.0]);
    graph
        .add_variable(
            "source",
            ValueType::String,
            PropertyValue::String(String::new()),
        )
        .unwrap();
    graph
        .set_property(source_node, "name", PropertyValue::String("source".into()))
        .unwrap();
    let dialogue_node = graph.add_node(NodeKind::Dialogue, [100.0, 0.0]);
    let string_output = pin(
        &mut graph,
        source_node,
        "value",
        PinDirection::Output,
        ValueType::String,
    );
    let dialogue_text = pin(
        &mut graph,
        dialogue_node,
        "text",
        PinDirection::Input,
        ValueType::InterpolatedText,
    );

    assert!(matches!(
        graph.connect(string_output, dialogue_text),
        Err(GraphEditError::IncompatibleTypes { .. })
    ));
    let connection = graph
        .connect_with_conversions(string_output, dialogue_text, [50.0, 50.0])
        .unwrap();
    assert_eq!(connection.conversions.len(), 1);
    assert_eq!(
        graph.nodes[&connection.conversions[0]].kind,
        NodeKind::ConvertStringToText
    );
}

#[test]
fn format_text_pattern_creates_named_pins_and_preserves_existing_wires() {
    let mut graph = GraphDocument::new(
        GraphId::new(701),
        GraphKind::Label {
            name: "format".into(),
        },
    );
    let format = graph
        .add_catalog_node(NodeKind::FormatText, [0.0, 0.0])
        .unwrap();
    let score = graph
        .add_catalog_node(NodeKind::Literal, [-100.0, 80.0])
        .unwrap();
    graph
        .set_property(score, "value", PropertyValue::Int(12))
        .unwrap();

    graph
        .set_format_text_pattern(format, "Ton score est de : {Score}")
        .unwrap();
    let score_pin = graph.pin_by_key(format, "argument_Score").unwrap().id;
    graph
        .connect(graph.pin_by_key(score, "value").unwrap().id, score_pin)
        .unwrap();

    graph
        .set_format_text_pattern(format, "Score actuel : {Score} / {Maximum}")
        .unwrap();
    assert_eq!(
        graph.pin_by_key(format, "argument_Score").unwrap().id,
        score_pin
    );
    assert!(graph.pin_by_key(format, "argument_Maximum").is_some());
    assert!(graph.edges.values().any(|edge| edge.input == score_pin));

    graph
        .set_format_text_pattern(format, "Maximum : {Maximum}")
        .unwrap();
    assert!(graph.pin_by_key(format, "argument_Score").is_none());
    assert!(graph.edges.values().all(|edge| edge.input != score_pin));
}

#[test]
fn reroute_pins_keep_the_cable_type_after_round_trip() {
    let mut graph = GraphDocument::new(GraphId::new(703), GraphKind::Init);
    let reroute = graph
        .add_catalog_node(NodeKind::Reroute, [32.0, 48.0])
        .unwrap();
    graph
        .set_reroute_type(reroute, ValueType::InterpolatedText)
        .unwrap();

    let restored = GraphDocument::from_json(&graph.to_pretty_json().unwrap()).unwrap();
    assert_eq!(
        restored.pin_by_key(reroute, "value").unwrap().value_type,
        ValueType::InterpolatedText
    );
    assert_eq!(
        restored
            .pin_by_key(reroute, "value_out")
            .unwrap()
            .value_type,
        ValueType::InterpolatedText
    );
}

#[test]
fn rejects_data_cycles_and_second_input_connection() {
    let mut graph = GraphDocument::new(
        GraphId::new(1),
        GraphKind::Label {
            name: "start".into(),
        },
    );
    let a = graph.add_node(NodeKind::BinaryOperator, [0.0, 0.0]);
    let b = graph.add_node(NodeKind::BinaryOperator, [100.0, 0.0]);
    let c = graph.add_node(NodeKind::Literal, [-100.0, 0.0]);
    let a_in = pin(&mut graph, a, "in", PinDirection::Input, ValueType::Int);
    let a_out = pin(&mut graph, a, "out", PinDirection::Output, ValueType::Int);
    let b_in = pin(&mut graph, b, "in", PinDirection::Input, ValueType::Int);
    let b_out = pin(&mut graph, b, "out", PinDirection::Output, ValueType::Int);
    let c_out = pin(&mut graph, c, "out", PinDirection::Output, ValueType::Int);

    graph.connect(a_out, b_in).unwrap();
    assert!(matches!(
        graph.connect(b_out, a_in),
        Err(GraphEditError::DataCycle { .. })
    ));
    graph.connect(c_out, a_in).unwrap();
    assert!(matches!(
        graph.connect(b_out, a_in),
        Err(GraphEditError::InputAlreadyConnected(_))
    ));
}

#[test]
fn validation_finds_a_cycle_in_a_manually_edited_document() {
    let mut graph = GraphDocument::new(GraphId::new(1), GraphKind::Init);
    let a = graph.add_node(NodeKind::BinaryOperator, [0.0, 0.0]);
    let b = graph.add_node(NodeKind::BinaryOperator, [1.0, 0.0]);
    let a_in = pin(&mut graph, a, "in", PinDirection::Input, ValueType::Int);
    let a_out = pin(&mut graph, a, "out", PinDirection::Output, ValueType::Int);
    let b_in = pin(&mut graph, b, "in", PinDirection::Input, ValueType::Int);
    let b_out = pin(&mut graph, b, "out", PinDirection::Output, ValueType::Int);
    graph.connect(a_out, b_in).unwrap();
    graph.edges.insert(
        EdgeId::new(99),
        GraphEdge {
            id: EdgeId::new(99),
            output: b_out,
            input: a_in,
        },
    );

    assert!(graph
        .validate()
        .iter()
        .any(|diagnostic| diagnostic.code == "data_cycle"));
}

#[test]
fn removing_a_node_removes_owned_pins_and_edges() {
    let mut graph = GraphDocument::new(GraphId::new(1), GraphKind::Init);
    let a = graph.add_node(NodeKind::Literal, [0.0, 0.0]);
    let b = graph.add_node(NodeKind::SetVariable, [1.0, 0.0]);
    graph
        .add_variable("flag", ValueType::Bool, PropertyValue::Bool(false))
        .unwrap();
    graph
        .set_property(b, "name", PropertyValue::String("flag".into()))
        .unwrap();
    let output = pin(&mut graph, a, "out", PinDirection::Output, ValueType::Bool);
    let input = pin(&mut graph, b, "in", PinDirection::Input, ValueType::Bool);
    graph.connect(output, input).unwrap();

    graph.remove_node(a).unwrap();
    assert!(!graph.pins.contains_key(&output));
    assert!(graph.edges.is_empty());
    assert!(graph.validate().is_empty());
}

#[test]
fn literal_output_tracks_the_property_type() {
    let mut graph = GraphDocument::new(GraphId::new(21), GraphKind::Init);
    let literal = graph
        .add_catalog_node(NodeKind::Literal, [0.0, 0.0])
        .unwrap();

    assert_eq!(
        graph.pin_by_key(literal, "value").unwrap().value_type,
        ValueType::String
    );
    graph
        .set_property(literal, "value", PropertyValue::Int(15))
        .unwrap();
    assert_eq!(
        graph.pin_by_key(literal, "value").unwrap().value_type,
        ValueType::Int
    );
}

#[test]
fn upgrades_a_legacy_text_to_integer_edge_with_a_converter() {
    let mut graph = GraphDocument::new(GraphId::new(22), GraphKind::Init);
    let literal = graph
        .add_catalog_node(NodeKind::Literal, [0.0, 0.0])
        .unwrap();
    let setter = graph
        .add_catalog_node(NodeKind::SetVariable, [200.0, 0.0])
        .unwrap();
    graph
        .add_variable("score", ValueType::Int, PropertyValue::Int(0))
        .unwrap();
    graph
        .set_property(setter, "name", PropertyValue::String("score".into()))
        .unwrap();
    let output = graph.pin_by_key(literal, "value").unwrap().id;
    let input = graph.pin_by_key(setter, "value").unwrap().id;
    graph.pins.get_mut(&output).unwrap().value_type = ValueType::Any;
    graph.pins.get_mut(&input).unwrap().value_type = ValueType::Int;
    // This is an old serialized edge, not a newly authored connection (which
    // correctly refuses known String→Int even with a stale Any pin cache).
    let mut raw = serde_json::to_value(&graph).unwrap();
    raw["edges"]["99"] = serde_json::to_value(GraphEdge {
        id: EdgeId(99),
        output,
        input,
    })
    .unwrap();
    raw["next_edge_id"] = serde_json::json!(100);
    graph = serde_json::from_value(raw).unwrap();

    assert_eq!(graph.materialize_implicit_conversions().unwrap(), 2);
    let converter = graph
        .nodes
        .values()
        .find(|node| node.kind == NodeKind::ConvertTextToInt)
        .expect("text-to-int converter");
    assert_eq!(graph.edges.len(), 2);
    assert_eq!(
        graph.pin_by_key(converter.id, "result").unwrap().value_type,
        ValueType::Int
    );
    assert!(graph.validate().is_empty());
}

#[test]
fn variable_nodes_require_an_existing_definition() {
    let mut graph = GraphDocument::new(GraphId::new(23), GraphKind::Init);
    let getter = graph
        .add_catalog_node(NodeKind::VariableGet, [0.0, 0.0])
        .unwrap();
    graph
        .set_property(getter, "name", PropertyValue::String("score".into()))
        .unwrap();

    assert!(graph
        .validate()
        .iter()
        .any(|diagnostic| diagnostic.code == "unknown_variable"));
    graph
        .add_variable("score", ValueType::Int, PropertyValue::Int(0))
        .unwrap();
    assert!(!graph
        .validate()
        .iter()
        .any(|diagnostic| diagnostic.code == "unknown_variable"));
}

#[test]
fn preserves_connected_constants_and_replaces_legacy_references_with_gets() {
    let mut graph = GraphDocument::new(GraphId::new(24), GraphKind::Init);
    graph
        .add_variable(
            "message",
            ValueType::String,
            PropertyValue::String("Bonjour".into()),
        )
        .unwrap();
    let dialogue = graph
        .add_catalog_node(NodeKind::Dialogue, [200.0, 0.0])
        .unwrap();
    let text = graph
        .add_catalog_node(NodeKind::TextValue, [0.0, 0.0])
        .unwrap();
    graph
        .set_property(text, "value", PropertyValue::String("Bienvenue".into()))
        .unwrap();
    let text_output = graph.pin_by_key(text, "value").unwrap().id;
    let text_input = graph.pin_by_key(dialogue, "text").unwrap().id;
    graph.connect(text_output, text_input).unwrap();

    let setter = graph
        .add_catalog_node(NodeKind::SetVariable, [200.0, 100.0])
        .unwrap();
    let set_reference = graph
        .add_catalog_node(NodeKind::VariableReference, [0.0, 100.0])
        .unwrap();
    graph
        .set_property(
            set_reference,
            "name",
            PropertyValue::String("message".into()),
        )
        .unwrap();
    let reference_output = graph.pin_by_key(set_reference, "value").unwrap().id;
    let name_input = graph.pin_by_key(setter, "name").unwrap().id;
    graph.connect(reference_output, name_input).unwrap();

    let legacy_get = graph
        .add_catalog_node(NodeKind::VariableReference, [0.0, 200.0])
        .unwrap();
    graph
        .set_property(legacy_get, "name", PropertyValue::String("message".into()))
        .unwrap();

    assert_eq!(graph.normalize_legacy_variable_references(), 2);
    assert_eq!(graph.clear_shadowed_input_defaults(), 0);
    assert!(graph.nodes.contains_key(&text));
    assert!(!graph.nodes.contains_key(&set_reference));
    assert_eq!(graph.nodes[&legacy_get].kind, NodeKind::VariableGet);
    assert!(graph
        .pin_by_key(dialogue, "text")
        .unwrap()
        .default_value
        .is_none());
    assert!(graph
        .edges
        .values()
        .any(|edge| { edge.output == text_output && edge.input == text_input }));
    assert_eq!(
        graph.nodes[&setter].properties["name"],
        PropertyValue::String("message".into())
    );
    assert!(graph.validate().is_empty());
}

#[test]
fn clears_a_hidden_default_when_the_input_is_already_connected() {
    let mut graph = GraphDocument::new(GraphId::new(25), GraphKind::Init);
    graph
        .add_variable(
            "dialogue1",
            ValueType::String,
            PropertyValue::String(String::new()),
        )
        .unwrap();
    let dialogue = graph
        .add_catalog_node(NodeKind::Dialogue, [200.0, 0.0])
        .unwrap();
    let getter = graph
        .add_catalog_node(NodeKind::VariableGet, [0.0, 0.0])
        .unwrap();
    graph
        .set_property(getter, "name", PropertyValue::String("dialogue1".into()))
        .unwrap();
    let output = graph.pin_by_key(getter, "value").unwrap().id;
    let input = graph.pin_by_key(dialogue, "text").unwrap().id;
    graph
        .connect_with_conversions(output, input, [100.0, 0.0])
        .unwrap();
    // Simule un ancien document sérialisé avant que `connect` ne nettoie la
    // valeur masquée automatiquement.
    graph
        .set_pin_default(
            dialogue,
            "text",
            PropertyValue::String("ancienne valeur cachée".into()),
        )
        .unwrap();

    assert_eq!(graph.clear_shadowed_input_defaults(), 1);
    assert!(graph.pins[&input].default_value.is_none());
    assert_eq!(graph.clear_shadowed_input_defaults(), 0);
}

#[test]
fn removing_an_edge_preserves_its_nodes_and_pins() {
    let mut graph = GraphDocument::new(GraphId::new(1), GraphKind::Init);
    let a = graph.add_node(NodeKind::Literal, [0.0, 0.0]);
    let b = graph.add_node(NodeKind::SetVariable, [1.0, 0.0]);
    let output = pin(&mut graph, a, "out", PinDirection::Output, ValueType::Int);
    let input = pin(&mut graph, b, "in", PinDirection::Input, ValueType::Int);
    let edge = graph.connect(output, input).unwrap();

    assert!(graph.remove_edge(edge).is_some());
    assert!(graph.nodes.contains_key(&a));
    assert!(graph.nodes.contains_key(&b));
    assert!(graph.pins.contains_key(&output));
    assert!(graph.pins.contains_key(&input));
    assert!(graph.edges.is_empty());
}

#[test]
fn choice_labels_can_be_edited_without_breaking_preserved_connections() {
    let mut graph = GraphDocument::new(
        GraphId::new(8),
        GraphKind::Label {
            name: "start".into(),
        },
    );
    let choice = graph
        .add_catalog_node(NodeKind::Choice, [0.0, 0.0])
        .unwrap();
    let first = graph.add_choice_option(choice, "Avant").unwrap();
    let dialogue = graph
        .add_catalog_node(NodeKind::Dialogue, [200.0, 0.0])
        .unwrap();
    let input = graph.pin_by_key(dialogue, "exec_in").unwrap().id;
    let edge = graph.connect(first.branch, input).unwrap();

    graph
        .set_choice_options(choice, vec!["Après".into(), "Nouveau".into()])
        .unwrap();

    assert_eq!(
        graph.pin_by_key(choice, "option_0").unwrap().id,
        first.branch
    );
    assert_eq!(graph.pin_by_key(choice, "option_0").unwrap().label, "Après");
    assert!(graph.edges.contains_key(&edge));
    assert!(graph.pin_by_key(choice, "option_1").is_some());

    graph
        .set_choice_options(choice, vec!["Seul".into()])
        .unwrap();
    assert!(graph.pin_by_key(choice, "option_1").is_none());
}

#[test]
fn catalog_nodes_expose_editable_expression_defaults() {
    let mut graph = GraphDocument::new(
        GraphId::new(9),
        GraphKind::Label {
            name: "start".into(),
        },
    );
    let binary = graph
        .add_catalog_node(NodeKind::BinaryOperator, [0.0, 0.0])
        .unwrap();
    let function = graph
        .add_catalog_node(NodeKind::FunctionCall, [0.0, 0.0])
        .unwrap();
    assert_eq!(
        graph.nodes[&binary].properties["operator"],
        PropertyValue::String("+".into())
    );
    assert_eq!(
        graph.nodes[&function].properties["args"],
        PropertyValue::StringList(Vec::new())
    );
}

#[test]
fn migrates_an_unversioned_graph_without_changing_stable_ids() {
    let mut graph = GraphDocument::new(
        GraphId::new(10),
        GraphKind::Label {
            name: "legacy".into(),
        },
    );
    let root = graph
        .add_catalog_node(NodeKind::Label, [12.0, 34.0])
        .unwrap();
    let mut value = serde_json::to_value(&graph).unwrap();
    let object = value.as_object_mut().unwrap();
    object.remove("schema_version");
    object.remove("next_node_id");
    object.remove("next_pin_id");
    object.remove("next_edge_id");
    for node in object["nodes"].as_object_mut().unwrap().values_mut() {
        node.as_object_mut().unwrap().remove("title_override");
        node.as_object_mut().unwrap().remove("properties");
    }

    let (mut restored, report) =
        GraphDocument::from_json_with_report(&serde_json::to_string(&value).unwrap()).unwrap();
    assert_eq!(report.from, 0);
    assert_eq!(report.to, GRAPH_SCHEMA_VERSION);
    assert_eq!(
        report.steps,
        vec![
            "v0_to_v1_stable_ids_and_defaults",
            "v1_to_v2_typed_variables",
            "v2_to_v3_explicit_text_conversions",
            "v3_to_v4_advanced_authoring",
            "v4_to_v5_programmable_canvas"
        ]
    );
    assert_eq!(restored.schema_version, GRAPH_SCHEMA_VERSION);
    assert!(restored.nodes.contains_key(&root));
    assert!(restored.nodes[&root].properties.is_empty());
    assert!(
        restored
            .add_catalog_node(NodeKind::Dialogue, [0.0, 0.0])
            .unwrap()
            .get()
            > root.get()
    );
}

#[test]
fn materializes_hidden_defaults_as_typed_connected_nodes() {
    let mut graph = GraphDocument::new(
        GraphId::new(11),
        GraphKind::Label {
            name: "visible".into(),
        },
    );
    let dialogue = graph
        .add_catalog_node(NodeKind::Dialogue, [300.0, 0.0])
        .unwrap();
    graph
        .set_pin_default(dialogue, "character", PropertyValue::String("alice".into()))
        .unwrap();
    graph
        .set_pin_default(dialogue, "text", PropertyValue::String("Bonjour".into()))
        .unwrap();
    let scene = graph
        .add_catalog_node(NodeKind::Scene, [600.0, 0.0])
        .unwrap();
    graph
        .set_pin_default(
            scene,
            "background",
            PropertyValue::String("station.png".into()),
        )
        .unwrap();
    graph
        .set_pin_default(
            scene,
            "transition",
            PropertyValue::String("fade(750)".into()),
        )
        .unwrap();

    assert_eq!(graph.materialize_visible_defaults().unwrap(), 4);
    for (node, key) in [
        (dialogue, "character"),
        (dialogue, "text"),
        (scene, "background"),
        (scene, "transition"),
    ] {
        let input = graph.pin_by_key(node, key).unwrap();
        assert!(input.default_value.is_none());
        assert_eq!(
            graph
                .edges
                .values()
                .filter(|edge| edge.input == input.id)
                .count(),
            1
        );
    }
    assert!(graph
        .nodes
        .values()
        .any(|node| node.kind == NodeKind::CharacterValue));
    assert!(graph
        .nodes
        .values()
        .any(|node| node.kind == NodeKind::TextValue));
    assert!(graph
        .nodes
        .values()
        .any(|node| node.kind == NodeKind::SceneAsset));
    assert!(graph.nodes.values().any(|node| {
        node.kind == NodeKind::TransitionFade
            && node.properties.get("duration_ms") == Some(&PropertyValue::Int(750))
    }));
}

#[test]
fn removes_legacy_false_choice_defaults_without_creating_a_literal() {
    let mut graph = GraphDocument::new(
        GraphId::new(12),
        GraphKind::Label {
            name: "choice-default".into(),
        },
    );
    let choice = graph
        .add_catalog_node(NodeKind::Choice, [0.0, 0.0])
        .unwrap();
    let option = graph.add_choice_option(choice, "Continuer").unwrap();
    graph.pins.get_mut(&option.condition).unwrap().default_value = Some(PropertyValue::Bool(false));

    assert_eq!(graph.materialize_visible_defaults().unwrap(), 1);
    assert!(graph.pins[&option.condition].default_value.is_none());
    assert_eq!(graph.nodes.len(), 1);
}

#[test]
fn normalizes_legacy_comparisons_to_typed_operator_nodes() {
    let mut graph = GraphDocument::new(
        GraphId::new(13),
        GraphKind::Label {
            name: "legacy-operator".into(),
        },
    );
    let comparison = graph
        .add_catalog_node(NodeKind::BinaryOperator, [0.0, 0.0])
        .unwrap();
    graph
        .set_property(comparison, "operator", PropertyValue::String(">=".into()))
        .unwrap();

    assert_eq!(graph.materialize_visible_defaults().unwrap(), 1);
    assert_eq!(graph.nodes[&comparison].kind, NodeKind::MathGreaterEqual);
    assert_eq!(
        graph.pin_by_key(comparison, "value").unwrap().value_type,
        ValueType::Bool
    );
    assert!(!graph.nodes[&comparison].properties.contains_key("operator"));
}

#[test]
fn variadic_boolean_operator_adds_a_visible_defaulted_operand() {
    let mut graph = GraphDocument::new(
        GraphId::new(14),
        GraphKind::Label {
            name: "variadic".into(),
        },
    );
    let operator = graph
        .add_catalog_node(NodeKind::LogicOr, [0.0, 0.0])
        .unwrap();
    let operand = graph.add_operator_operand(operator).unwrap();

    assert_eq!(graph.pins[&operand].key, "operand_2");
    assert_eq!(graph.pins[&operand].value_type, ValueType::Bool);
    assert_eq!(
        graph.pins[&operand].default_value,
        Some(PropertyValue::Bool(false))
    );
    assert_eq!(graph.materialize_visible_defaults().unwrap(), 0);
}


#[test]
fn connected_inline_fallback_survives_load_cleanup_and_disconnect() {
    for (kind,key,value) in [
        (NodeKind::SetVariable,"value",PropertyValue::Int(2)),
        (NodeKind::LocalVariable,"value",PropertyValue::Int(3)),
        (NodeKind::MathAdd,"left",PropertyValue::Int(4)),
        (NodeKind::If,"condition",PropertyValue::Bool(false)),
    ] {
        let mut graph=GraphDocument::new(GraphId::new(42),GraphKind::Label{name:"start".into()});
        graph.add_variable("score",ValueType::Int,PropertyValue::Int(0)).unwrap();
        let owner=graph.add_catalog_node(kind,[224.0,16.0]).unwrap();
        if matches!(kind,NodeKind::SetVariable|NodeKind::LocalVariable) {
            graph.set_property(owner,"name",PropertyValue::String("score".into())).unwrap();
        }
        graph.set_pin_default(owner,key,value.clone()).unwrap();
        let literal=graph.add_catalog_node(NodeKind::Literal,[32.0,80.0]).unwrap();
        graph.set_property(literal,"value",match value {PropertyValue::Bool(_)=>PropertyValue::Bool(true),_=>PropertyValue::Int(99)}).unwrap();
        let output=graph.pin_by_key(literal,"value").unwrap().id;
        let input=graph.pin_by_key(owner,key).unwrap().id;
        let edge=graph.connect(output,input).unwrap();
        assert_eq!(graph.pins[&input].default_value,Some(value.clone()),"connect already preserves the inline fallback");
        let bytes=graph.to_pretty_json().unwrap();
        let mut reopened=GraphDocument::from_json(&bytes).unwrap();
        assert_eq!(reopened.clear_shadowed_input_defaults(),0,"load cleanup must retain authored fallback for {kind:?}");
        assert_eq!(reopened,graph,"opening must preserve all IDs, positions, edges and hidden defaults");
        reopened.remove_edge(edge);
        assert_eq!(reopened.pins[&input].default_value,Some(value));
        assert_eq!(graph.to_pretty_json().unwrap(),bytes);
    }
}


#[test]
fn asset_defaults1021_all_catalogue_asset_inputs_retain_selection_behind_wire() {
    for (kind, key, source_kind) in [
        (NodeKind::Scene, "background", NodeKind::SceneAsset),
        (NodeKind::CinematicShow, "cinematic", NodeKind::CinematicAsset),
        (NodeKind::MusicPlay, "file", NodeKind::MusicAsset),
        (NodeKind::SfxPlay, "file", NodeKind::SoundEffectAsset),
        (NodeKind::SfxStop, "file", NodeKind::SoundEffectAsset),
        (NodeKind::VoicePlay, "file", NodeKind::VoiceAsset),
        (NodeKind::Imagemap, "background", NodeKind::SceneAsset),
        (NodeKind::Imagemap, "hover", NodeKind::HoverImageAsset),
        (NodeKind::CharacterValue, "sprite", NodeKind::SpriteAsset),
        (NodeKind::StringTableId, "asset", NodeKind::StringTableAsset),
    ] {
        let mut graph = GraphDocument::new(GraphId::new(1021), GraphKind::Label { name: "start".into() });
        let owner = graph.add_catalog_node(kind, [200.0, 0.0]).unwrap();
        let fallback = PropertyValue::String("authored/fallback".into());
        graph.set_pin_default(owner, key, fallback.clone()).unwrap();
        let producer = graph.add_catalog_node(source_kind, [0.0, 0.0]).unwrap();
        graph.set_property(producer, if source_kind == NodeKind::StringTableAsset { "table" } else { "path" }, PropertyValue::String("wired/selection".into())).unwrap();
        let input = graph.pin_by_key(owner, key).unwrap().id;
        let output = graph.pin_by_key(producer, "value").unwrap().id;
        assert!(matches!(graph.pins[&input].value_type, ValueType::Asset(_)));
        let edge = graph.connect(output, input).unwrap();
        assert_eq!(graph.pins[&input].default_value, Some(fallback.clone()), "{kind:?}.{key}");
        assert_eq!(graph.clear_shadowed_input_defaults(), 0);
        let bytes = graph.to_pretty_json().unwrap();
        let mut reopened = GraphDocument::from_json(&bytes).unwrap();
        assert_eq!(reopened, graph, "JSON keeps the exact graph, pin and edge identities");
        assert_eq!(reopened.clear_shadowed_input_defaults(), 0);
        reopened.remove_edge(edge);
        assert_eq!(reopened.pins[&input].default_value, Some(fallback));
        assert!(reopened.nodes.contains_key(&producer));
        assert_eq!(graph.to_pretty_json().unwrap(), bytes, "disconnecting the candidate does not mutate its source snapshot");
    }
}

#[test]
fn asset_defaults1021_wrong_asset_connection_refuses_before_selection_or_graph_mutation() {
    let mut graph = GraphDocument::new(GraphId::new(1022), GraphKind::Label { name: "start".into() });
    let effect = graph.add_catalog_node(NodeKind::SfxPlay, [200.0, 0.0]).unwrap();
    graph.set_pin_default(effect, "file", PropertyValue::String("fallback.wav".into())).unwrap();
    let music = graph.add_catalog_node(NodeKind::MusicAsset, [0.0, 0.0]).unwrap();
    graph.set_property(music, "path", PropertyValue::String("wrong.ogg".into())).unwrap();
    let output = graph.pin_by_key(music, "value").unwrap().id;
    let input = graph.pin_by_key(effect, "file").unwrap().id;
    let before = graph.clone();
    assert!(matches!(graph.connect(output, input), Err(GraphEditError::IncompatibleTypes { .. })));
    assert_eq!(graph, before);
}


#[test]
fn asset_defaults1021_legacy_ambiguous_real_neighbor_mapping_refuses_before_graph_mutation() {
    // Two editable real consumers share one actual Asset output and have no
    // distinguishing execution neighbor. Numeric allocation order is not proof
    // that either freshly imported consumer owns fallback A or fallback D.
    let mut prior = GraphDocument::new(GraphId::new(1023), GraphKind::Label { name: "start".into() });
    prior.add_catalog_node(NodeKind::Label, [0.0, 0.0]).unwrap();
    let producer = prior.add_catalog_node(NodeKind::SoundEffectAsset, [-200.0, 0.0]).unwrap();
    prior.set_property(producer, "path", PropertyValue::String("B.wav".into())).unwrap();
    let output = prior.pin_by_key(producer, "value").unwrap().id;
    for (index, value) in ["A.wav", "D.wav"].into_iter().enumerate() {
        let consumer = prior.add_catalog_node(NodeKind::SfxPlay, [200.0, index as f64 * 180.0]).unwrap();
        prior.set_pin_default(consumer, "file", PropertyValue::String(value.into())).unwrap();
        let input = prior.pin_by_key(consumer, "file").unwrap().id;
        prior.connect(output, input).unwrap();
    }
    let mut imported = GraphDocument::from_json(&prior.to_pretty_json().unwrap()).unwrap();
    for pin in imported.pins.values_mut().filter(|pin| pin.key == "file") { pin.default_value = None; }
    let previous_bytes = prior.to_pretty_json().unwrap();
    let imported_bytes = imported.to_pretty_json().unwrap();
    let error = imported.reconcile_import(&prior).unwrap_err();
    assert!(error.contains("Ambiguous retained Asset slot"));
    assert_eq!(imported.to_pretty_json().unwrap(), imported_bytes, "no IDs, allocators, links or presentation commit on ambiguity");
    assert_eq!(prior.to_pretty_json().unwrap(), previous_bytes);
}

// All three real scalar consumer kinds share the same authoring-slot policy.
fn scalar_fallback1021_document(kind: NodeKind, key: &str, a: PropertyValue, b: PropertyValue, c: PropertyValue) {
    let mut graph=GraphDocument::new(GraphId::new(102103),GraphKind::Label{name:"start".into()});
    let command=graph.add_catalog_node(kind,[300.0,20.0]).unwrap();
    graph.set_pin_default(command,key,a.clone()).unwrap();
    let input=graph.pin_by_key(command,key).unwrap().id;
    let producer=graph.add_catalog_node(NodeKind::Literal,[0.0,20.0]).unwrap();
    graph.set_property(producer,"value",b).unwrap();
    let output=graph.pin_by_key(producer,"value").unwrap().id;
    let first=graph.connect(output,input).unwrap();
    assert_eq!(graph.pins[&input].default_value,Some(a.clone()),"a real cable must retain this consumer's authored scalar A");
    let bytes=graph.to_pretty_json().unwrap();
    let mut cold=GraphDocument::from_json(&bytes).unwrap();
    assert_eq!(cold.clear_shadowed_input_defaults(),0);
    assert_eq!(cold,graph);
    let replacement=cold.add_catalog_node(NodeKind::Literal,[0.0,100.0]).unwrap();
    cold.set_property(replacement,"value",c).unwrap();
    let next=cold.pin_by_key(replacement,"value").unwrap().id;
    let before=cold.clone();
    assert!(cold.connect(next,input).is_err(),"occupied input must still refuse without replacement authority");
    assert_eq!(cold,before);
    assert!(cold.connect(input,next).is_err(),"reverse direction cannot edit any default");
    assert_eq!(cold,before);
    cold.remove_edge(first).unwrap();
    cold.remove_node(producer).unwrap();
    let second=cold.connect(next,input).unwrap();
    assert_eq!(cold.pins[&input].default_value,Some(a.clone()));
    let bad=cold.add_catalog_node(NodeKind::Literal,[0.0,180.0]).unwrap();
    cold.set_property(bad,"value",PropertyValue::String("wrong scalar type".into())).unwrap();
    let wrong=cold.pin_by_key(bad,"value").unwrap().id;
    cold.remove_edge(second).unwrap();
    let before=cold.clone();
    assert!(cold.connect(wrong,input).is_err());assert_eq!(cold,before);
    assert_eq!(cold.pins[&input].default_value,Some(a.clone()),"disconnect exposes the original scalar A, never producer C");
    assert_eq!(cold.pin_by_key(command,key).unwrap().id,input);
    assert_eq!(GraphDocument::from_json(&cold.to_pretty_json().unwrap()).unwrap(),cold);
}
#[test]
fn scalar_fallback1021_bool_connect_rewire_disconnect_json_and_refusals() {
    scalar_fallback1021_document(NodeKind::TypewriterSet,"enabled",PropertyValue::Bool(false),PropertyValue::Bool(true),PropertyValue::Bool(false));
}
#[test]
fn scalar_fallback1021_int_connect_rewire_disconnect_json_and_refusals() {
    scalar_fallback1021_document(NodeKind::TypewriterSpeed,"speed",PropertyValue::Int(11),PropertyValue::Int(23),PropertyValue::Int(47));
}
#[test]
fn scalar_fallback1021_float_connect_rewire_disconnect_json_and_refusals() {
    scalar_fallback1021_document(NodeKind::MusicVolume,"level",PropertyValue::Float(0.125),PropertyValue::Float(0.5),PropertyValue::Float(0.75));
}

