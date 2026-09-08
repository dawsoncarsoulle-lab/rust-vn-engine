use rvn_graph::*;
use rvn_parser::{BinOpKind, Expr, Statement, Transition};

#[test]
fn declared_character_uses_connected_sprite_and_preserves_display_name() {
    let mut graph = GraphDocument::new(GraphId::new(900), GraphKind::Label { name: "start".into() });
    graph.characters.insert("alice".into(), "Alice Lemaire".into());
    let root = add(&mut graph, NodeKind::Label);
    let character = add(&mut graph, NodeKind::CharacterValue);
    let image = add(&mut graph, NodeKind::SpriteAsset);
    let show = add(&mut graph, NodeKind::SpriteShow);
    graph.set_property(character, "character", PropertyValue::String("alice".into())).unwrap();
    graph.set_property(image, "path", PropertyValue::String("assets/images/sprites/happy.png".into())).unwrap();
    connect(&mut graph, root, "exec_out", show, "exec_in");
    connect(&mut graph, image, "value", character, "sprite");
    connect(&mut graph, character, "value", show, "character");
    let graph = GraphDocument::from_json(&graph.to_pretty_json().unwrap()).unwrap();
    let output = transpile(&graph).unwrap();
    assert!(output.source.contains("character.create(\"alice\", \"Alice Lemaire\")"));
    assert!(output.source.contains("alice.show(\"assets/images/sprites/happy.png\")"));
}

#[test]
fn dialogue_sprite_connection_preserves_sprite_until_explicit_removal() {
    let mut graph=GraphDocument::new(GraphId::new(901),GraphKind::Label{name:"start".into()});
    graph.characters.insert("alice".into(),"Alice".into());
    let mut previous=add(&mut graph,NodeKind::Label);
    for (index,path) in [Some("sprites/alice/happy.png"),None,None,Some("sprites/alice/sad.png")].into_iter().enumerate() {
        if index==2 {
            // Repeated removal is harmless and targets only Alice.
            for _ in 0..2 {
                let hide=add(&mut graph,NodeKind::SpriteHide);
                graph.set_pin_default(hide,"character",PropertyValue::String("alice".into())).unwrap();
                connect(&mut graph,previous,"exec_out",hide,"exec_in");
                previous=hide;
            }
        }
        let dialogue=add(&mut graph,NodeKind::Dialogue);
        let character=add(&mut graph,NodeKind::CharacterValue);
        graph.set_property(character,"character",PropertyValue::String("alice".into())).unwrap();
        // Even a stale property must not provide an image without a wire.
        graph.set_property(character,"emotion",PropertyValue::String("hidden_fallback".into())).unwrap();
        graph.set_pin_default(dialogue,"text",PropertyValue::String("Bonjour".into())).unwrap();
        connect(&mut graph,previous,"exec_out",dialogue,"exec_in");
        connect(&mut graph,character,"value",dialogue,"character");
        if let Some(path)=path {
            let image=add(&mut graph,NodeKind::SpriteAsset);
            graph.set_property(image,"path",PropertyValue::String(path.into())).unwrap();
            connect(&mut graph,image,"value",character,"sprite");
        }
        previous=dialogue;
    }
    graph.materialize_visible_defaults().unwrap();
    let compiled=transpile(&graph).unwrap();
    assert!(!compiled.source.contains("hidden_fallback"));
    let mut engine=rvn_core::Engine::new(compiled.ast,rvn_core::TerminalRenderer,20).unwrap();
    engine.state.sprites.insert("bob".into(),rvn_core::SpriteState::new(Some("bob.png".into()),rvn_parser::Position::Left));
    assert_eq!(compiled.source.matches("alice.hide()").count(),2);
    for expected in [Some("sprites/alice/happy.png"),Some("sprites/alice/happy.png"),None,Some("sprites/alice/sad.png")] {
        assert!(matches!(engine.step_until_interaction().unwrap(),Some(rvn_core::Interaction::Dialogue{..})));
        let sprite=engine.state.sprites.get("alice").filter(|s|s.visible);
        assert_eq!(sprite.and_then(|s|s.emotion.as_deref()),expected);
        assert!(engine.state.sprites["bob"].visible);
        engine.advance_dialogue().unwrap();
    }
}

#[test]
fn an_unwired_speaker_is_valid_before_any_sprite_has_been_shown() {
    let mut graph=GraphDocument::new(GraphId::new(902),GraphKind::Label{name:"start".into()});
    let root=add(&mut graph,NodeKind::Label); let d=add(&mut graph,NodeKind::Dialogue);
    graph.set_pin_default(d,"character",PropertyValue::String("narrator".into())).unwrap();
    graph.set_pin_default(d,"text",PropertyValue::String("Texte seul".into())).unwrap();
    connect(&mut graph,root,"exec_out",d,"exec_in");
    graph.materialize_visible_defaults().unwrap();
    let mut engine=rvn_core::Engine::new(transpile(&graph).unwrap().ast,rvn_core::TerminalRenderer,20).unwrap();
    assert!(engine.step_until_interaction().unwrap().is_some());
    assert!(engine.state.sprites.is_empty());
}

#[test]
fn legacy_show_migration_is_explicit_and_disconnect_does_not_restore_it() {
    let mut graph=GraphDocument::new(GraphId::new(903),GraphKind::Label{name:"start".into()});
    let root=add(&mut graph,NodeKind::Label); let show=add(&mut graph,NodeKind::SpriteShow);
    graph.set_pin_default(show,"character",PropertyValue::String("alice".into())).unwrap();
    graph.set_pin_default(show,"emotion",PropertyValue::String("happy".into())).unwrap();
    connect(&mut graph,root,"exec_out",show,"exec_in");
    graph.materialize_visible_defaults().unwrap();
    assert!(transpile(&graph).unwrap().source.contains("sprites/alice/happy.png"));
    let input=graph.nodes.values().find(|n|n.kind==NodeKind::CharacterValue).and_then(|n|graph.pin_by_key(n.id,"sprite")).unwrap().id;
    graph.edges.retain(|_,e|e.input!=input);
    graph.materialize_visible_defaults().unwrap();
    assert!(matches!(transpile(&graph),Err(TranspileError::MissingInputValue{..})));
}

fn add(graph: &mut GraphDocument, kind: NodeKind) -> NodeId {
    graph.add_catalog_node(kind, [0.0, 0.0]).unwrap()
}

fn pin(graph: &GraphDocument, node: NodeId, key: &str) -> PinId {
    graph.pin_by_key(node, key).unwrap().id
}

fn connect(
    graph: &mut GraphDocument,
    output_node: NodeId,
    output_key: &str,
    input_node: NodeId,
    input_key: &str,
) {
    let output = pin(graph, output_node, output_key);
    let input = pin(graph, input_node, input_key);
    graph.connect(output, input).unwrap();
}

fn define(
    graph: &mut GraphDocument,
    name: &str,
    value_type: ValueType,
    default_value: PropertyValue,
) {
    graph.add_variable(name, value_type, default_value).unwrap();
}

#[test]
fn format_text_emits_runtime_interpolation_for_dialogue() {
    let mut graph = GraphDocument::new(
        GraphId::new(700),
        GraphKind::Label {
            name: "score".into(),
        },
    );
    let root = add(&mut graph, NodeKind::Label);
    let dialogue = add(&mut graph, NodeKind::Dialogue);
    let format = add(&mut graph, NodeKind::FormatText);
    let score = add(&mut graph, NodeKind::VariableGet);
    define(&mut graph, "score", ValueType::Int, PropertyValue::Int(0));
    graph
        .set_property(score, "name", PropertyValue::String("score".into()))
        .unwrap();
    graph
        .set_format_text_pattern(format, "Ton score est de : {Score}")
        .unwrap();

    connect(&mut graph, root, "exec_out", dialogue, "exec_in");
    connect(&mut graph, score, "value", format, "argument_Score");
    connect(&mut graph, format, "result", dialogue, "text");

    let generated = transpile(&graph).unwrap();
    assert_eq!(
        generated.source,
        "label score\n    \"Ton score est de : [score]\"\n"
    );
}

#[test]
fn data_reroute_is_transparent_to_codegen() {
    let mut graph = GraphDocument::new(
        GraphId::new(701),
        GraphKind::Label {
            name: "score".into(),
        },
    );
    let root = add(&mut graph, NodeKind::Label);
    let dialogue = add(&mut graph, NodeKind::Dialogue);
    let format = add(&mut graph, NodeKind::FormatText);
    let score = add(&mut graph, NodeKind::VariableGet);
    let reroute = add(&mut graph, NodeKind::Reroute);
    define(&mut graph, "score", ValueType::Int, PropertyValue::Int(0));
    graph
        .set_property(score, "name", PropertyValue::String("score".into()))
        .unwrap();
    graph
        .set_format_text_pattern(format, "Score : {Score}")
        .unwrap();
    graph.set_reroute_type(reroute, ValueType::Int).unwrap();

    connect(&mut graph, root, "exec_out", dialogue, "exec_in");
    connect(&mut graph, score, "value", reroute, "value");
    connect(&mut graph, reroute, "value_out", format, "argument_Score");
    connect(&mut graph, format, "result", dialogue, "text");

    assert_eq!(
        transpile(&graph).unwrap().source,
        "label score\n    \"Score : [score]\"\n"
    );
}

#[test]
fn execution_reroute_is_transparent_to_codegen() {
    let mut graph = GraphDocument::new(
        GraphId::new(702),
        GraphKind::Label {
            name: "start".into(),
        },
    );
    let root = add(&mut graph, NodeKind::Label);
    let reroute = add(&mut graph, NodeKind::Reroute);
    let dialogue = add(&mut graph, NodeKind::Dialogue);
    let text = add(&mut graph, NodeKind::TextValue);
    graph
        .set_reroute_type(reroute, ValueType::Execution)
        .unwrap();
    graph
        .set_property(text, "value", PropertyValue::String("Bonjour".into()))
        .unwrap();

    connect(&mut graph, root, "exec_out", reroute, "value");
    connect(&mut graph, reroute, "value_out", dialogue, "exec_in");
    connect(&mut graph, text, "value", dialogue, "text");

    assert_eq!(
        transpile(&graph).unwrap().source,
        "label start\n    \"Bonjour\"\n"
    );
}

#[test]
fn transpiles_numeric_conversion_nodes_as_typed_rvn_expressions() {
    let mut graph = GraphDocument::new(
        GraphId::new(18),
        GraphKind::Label {
            name: "conversions".into(),
        },
    );
    let root = add(&mut graph, NodeKind::Label);
    let decimal_set = add(&mut graph, NodeKind::SetVariable);
    let int_value = add(&mut graph, NodeKind::Literal);
    let int_to_float = add(&mut graph, NodeKind::ConvertIntToFloat);
    let text_set = add(&mut graph, NodeKind::SetVariable);
    let float_value = add(&mut graph, NodeKind::Literal);
    let number_to_text = add(&mut graph, NodeKind::ConvertNumberToText);
    define(
        &mut graph,
        "decimal_value",
        ValueType::Float,
        PropertyValue::Float(0.0),
    );
    define(
        &mut graph,
        "text_value",
        ValueType::String,
        PropertyValue::String(String::new()),
    );

    graph
        .set_pin_default(
            decimal_set,
            "name",
            PropertyValue::String("decimal_value".into()),
        )
        .unwrap();
    graph
        .set_property(int_value, "value", PropertyValue::Int(10))
        .unwrap();
    graph
        .set_pin_default(text_set, "name", PropertyValue::String("text_value".into()))
        .unwrap();
    graph
        .set_property(float_value, "value", PropertyValue::Float(12.5))
        .unwrap();

    connect(&mut graph, root, "exec_out", decimal_set, "exec_in");
    connect(&mut graph, decimal_set, "exec_out", text_set, "exec_in");
    connect(&mut graph, int_value, "value", int_to_float, "value");
    connect(&mut graph, int_to_float, "result", decimal_set, "value");
    connect(&mut graph, float_value, "value", number_to_text, "value");
    connect(&mut graph, number_to_text, "result", text_set, "value");

    let generated = transpile(&graph).unwrap();
    assert_eq!(
        generated.source,
        concat!(
            "label conversions\n",
            "    set decimal_value = (10 + 0.0)\n",
            "    set text_value = (\"\" + 12.5)\n",
        )
    );
}

#[test]
fn transpiles_text_to_integer_conversion_as_a_runtime_conversion() {
    let mut graph = GraphDocument::new(
        GraphId::new(20),
        GraphKind::Label {
            name: "text_to_integer".into(),
        },
    );
    let root = add(&mut graph, NodeKind::Label);
    let setter = add(&mut graph, NodeKind::SetVariable);
    let text = add(&mut graph, NodeKind::Literal);
    let converter = add(&mut graph, NodeKind::ConvertTextToInt);
    define(&mut graph, "score", ValueType::Int, PropertyValue::Int(0));

    graph
        .set_pin_default(setter, "name", PropertyValue::String("score".into()))
        .unwrap();
    graph
        .set_property(text, "value", PropertyValue::String("15".into()))
        .unwrap();

    connect(&mut graph, root, "exec_out", setter, "exec_in");
    connect(&mut graph, text, "value", converter, "value");
    connect(&mut graph, converter, "result", setter, "value");

    let generated = transpile(&graph).unwrap();
    assert_eq!(
        generated.source,
        concat!(
            "label text_to_integer\n",
            "    set score = to_int(\"15\")\n",
        )
    );
    rvn_parser::parse(&generated.source).unwrap();
}

#[test]
fn set_variable_value_output_reuses_the_assigned_expression() {
    let mut graph = GraphDocument::new(
        GraphId::new(19),
        GraphKind::Label {
            name: "set_passthrough".into(),
        },
    );
    let root = add(&mut graph, NodeKind::Label);
    let first_set = add(&mut graph, NodeKind::SetVariable);
    let second_set = add(&mut graph, NodeKind::SetVariable);
    let value = add(&mut graph, NodeKind::Literal);
    define(&mut graph, "score", ValueType::Int, PropertyValue::Int(0));
    define(
        &mut graph,
        "score_copy",
        ValueType::Int,
        PropertyValue::Int(0),
    );

    graph
        .set_pin_default(first_set, "name", PropertyValue::String("score".into()))
        .unwrap();
    graph
        .set_pin_default(
            second_set,
            "name",
            PropertyValue::String("score_copy".into()),
        )
        .unwrap();
    graph
        .set_property(value, "value", PropertyValue::Int(7))
        .unwrap();

    connect(&mut graph, root, "exec_out", first_set, "exec_in");
    connect(&mut graph, first_set, "exec_out", second_set, "exec_in");
    connect(&mut graph, value, "value", first_set, "value");
    connect(&mut graph, first_set, "value_out", second_set, "value");

    let generated = transpile(&graph).unwrap();
    assert_eq!(
        generated.source,
        concat!(
            "label set_passthrough\n",
            "    set score = 7\n",
            "    set score_copy = 7\n",
        )
    );
}

#[test]
fn dialogue_text_connected_to_variable_get_emits_interpolation() {
    let mut graph = GraphDocument::new(
        GraphId::new(1),
        GraphKind::Label {
            name: "start".into(),
        },
    );
    let root = add(&mut graph, NodeKind::Label);
    let dialogue = add(&mut graph, NodeKind::Dialogue);
    let variable = add(&mut graph, NodeKind::VariableGet);
    define(
        &mut graph,
        "testVar",
        ValueType::String,
        PropertyValue::String(String::new()),
    );

    graph
        .set_property(variable, "name", PropertyValue::String("testVar".into()))
        .unwrap();
    connect(&mut graph, root, "exec_out", dialogue, "exec_in");
    connect(&mut graph, variable, "value", dialogue, "text");

    let generated = transpile(&graph).unwrap();
    assert_eq!(generated.source, "label start\n    \"[testVar]\"\n");
}

#[test]
fn set_then_get_text_variable_drives_dialogue_like_the_editor_workflow() {
    let mut graph = GraphDocument::new(
        GraphId::new(24),
        GraphKind::Label {
            name: "start".into(),
        },
    );
    define(
        &mut graph,
        "dialogue1",
        ValueType::String,
        PropertyValue::String(String::new()),
    );
    let root = add(&mut graph, NodeKind::Label);
    let setter = add(&mut graph, NodeKind::SetVariable);
    let constant = add(&mut graph, NodeKind::TextValue);
    let getter = add(&mut graph, NodeKind::VariableGet);
    let dialogue = add(&mut graph, NodeKind::Dialogue);

    graph
        .set_property(setter, "name", PropertyValue::String("dialogue1".into()))
        .unwrap();
    graph
        .set_pin_default(setter, "name", PropertyValue::String("dialogue1".into()))
        .unwrap();
    graph
        .set_property(
            constant,
            "value",
            PropertyValue::String("salut cava".into()),
        )
        .unwrap();
    graph
        .set_property(getter, "name", PropertyValue::String("dialogue1".into()))
        .unwrap();
    let setter_value = pin(&graph, setter, "value");
    let setter_output = pin(&graph, setter, "value_out");
    let getter_output = pin(&graph, getter, "value");
    graph.pins.get_mut(&setter_value).unwrap().value_type = ValueType::String;
    graph.pins.get_mut(&setter_output).unwrap().value_type = ValueType::String;
    graph.pins.get_mut(&getter_output).unwrap().value_type = ValueType::String;

    connect(&mut graph, root, "exec_out", setter, "exec_in");
    connect(&mut graph, constant, "value", setter, "value");
    connect(&mut graph, setter, "exec_out", dialogue, "exec_in");
    connect(&mut graph, getter, "value", dialogue, "text");

    let generated = transpile(&graph).unwrap();
    assert_eq!(
        generated.source,
        concat!(
            "label start\n",
            "    set dialogue1 = \"salut cava\"\n",
            "    \"[dialogue1]\"\n",
        )
    );
}

#[test]
fn dialogue_never_uses_an_unconnected_hidden_text_default() {
    let mut graph = GraphDocument::new(
        GraphId::new(25),
        GraphKind::Label {
            name: "start".into(),
        },
    );
    let root = add(&mut graph, NodeKind::Label);
    let dialogue = add(&mut graph, NodeKind::Dialogue);
    graph
        .set_pin_default(
            dialogue,
            "text",
            PropertyValue::String("ce texte ne doit pas sortir".into()),
        )
        .unwrap();
    connect(&mut graph, root, "exec_out", dialogue, "exec_in");

    assert!(matches!(
        transpile(&graph),
        Err(TranspileError::MissingInputValue { node, key })
            if node == dialogue && key == "text"
    ));
}

#[test]
fn conversion_never_uses_its_empty_editor_placeholder_as_input() {
    let mut graph = GraphDocument::new(
        GraphId::new(26),
        GraphKind::Label {
            name: "start".into(),
        },
    );
    define(&mut graph, "score", ValueType::Int, PropertyValue::Int(0));
    let root = add(&mut graph, NodeKind::Label);
    let setter = add(&mut graph, NodeKind::SetVariable);
    let converter = add(&mut graph, NodeKind::ConvertTextToInt);
    graph
        .set_property(setter, "name", PropertyValue::String("score".into()))
        .unwrap();
    graph
        .set_pin_default(setter, "name", PropertyValue::String("score".into()))
        .unwrap();
    connect(&mut graph, root, "exec_out", setter, "exec_in");
    connect(&mut graph, converter, "result", setter, "value");

    assert!(matches!(
        transpile(&graph),
        Err(TranspileError::MissingInputValue { node, key })
            if node == converter && key == "value"
    ));
}

#[test]
fn transpiles_a_structured_visual_novel_graph_and_reparses_it() {
    let mut graph = GraphDocument::new(
        GraphId::new(1),
        GraphKind::Label {
            name: "chapter_one".into(),
        },
    );

    let root = add(&mut graph, NodeKind::Label);
    let greeting = add(&mut graph, NodeKind::Dialogue);
    let setter = add(&mut graph, NodeKind::SetVariable);
    let value = add(&mut graph, NodeKind::Literal);
    let conditional = add(&mut graph, NodeKind::If);
    let score = add(&mut graph, NodeKind::VariableGet);
    let threshold = add(&mut graph, NodeKind::Literal);
    let comparison = add(&mut graph, NodeKind::MathGreaterEqual);
    let success = add(&mut graph, NodeKind::Dialogue);
    let failure = add(&mut graph, NodeKind::Dialogue);
    let success_end = graph.add_branch_end(conditional, [0.0, 0.0]).unwrap();
    let failure_end = graph.add_branch_end(conditional, [0.0, 0.0]).unwrap();
    let choice = add(&mut graph, NodeKind::Choice);
    let accept = graph.add_choice_option(choice, "Continuer").unwrap();
    let refuse = graph.add_choice_option(choice, "Partir").unwrap();
    let accept_body = add(&mut graph, NodeKind::Dialogue);
    let refuse_body = add(&mut graph, NodeKind::Dialogue);
    let accept_end = graph.add_branch_end(choice, [0.0, 0.0]).unwrap();
    let refuse_end = graph.add_branch_end(choice, [0.0, 0.0]).unwrap();
    let option_condition = add(&mut graph, NodeKind::Literal);
    let scene = add(&mut graph, NodeKind::Scene);
    define(&mut graph, "score", ValueType::Int, PropertyValue::Int(0));

    graph
        .set_pin_default(greeting, "character", PropertyValue::String("alice".into()))
        .unwrap();
    graph
        .set_pin_default(
            greeting,
            "text",
            PropertyValue::String("Bienvenue [player_name] !".into()),
        )
        .unwrap();
    graph
        .set_pin_default(setter, "name", PropertyValue::String("score".into()))
        .unwrap();
    graph
        .set_property(value, "value", PropertyValue::Int(2))
        .unwrap();
    graph
        .set_property(score, "name", PropertyValue::String("score".into()))
        .unwrap();
    graph
        .set_property(threshold, "value", PropertyValue::Int(10))
        .unwrap();
    graph
        .set_pin_default(success, "character", PropertyValue::String("alice".into()))
        .unwrap();
    graph
        .set_pin_default(success, "text", PropertyValue::String("Bravo.".into()))
        .unwrap();
    graph
        .set_pin_default(failure, "character", PropertyValue::String("alice".into()))
        .unwrap();
    graph
        .set_pin_default(
            failure,
            "text",
            PropertyValue::String("Encore un effort.".into()),
        )
        .unwrap();
    graph
        .set_pin_default(
            accept_body,
            "text",
            PropertyValue::String("En avant !".into()),
        )
        .unwrap();
    graph
        .set_pin_default(
            refuse_body,
            "text",
            PropertyValue::String("À bientôt.".into()),
        )
        .unwrap();
    graph
        .set_property(option_condition, "value", PropertyValue::Bool(true))
        .unwrap();
    graph
        .set_pin_default(scene, "background", PropertyValue::String("station".into()))
        .unwrap();
    graph
        .set_pin_default(scene, "transition", PropertyValue::String("fade".into()))
        .unwrap();

    connect(&mut graph, root, "exec_out", greeting, "exec_in");
    connect(&mut graph, greeting, "exec_out", setter, "exec_in");
    connect(&mut graph, value, "value", setter, "value");
    connect(&mut graph, setter, "exec_out", conditional, "exec_in");
    connect(&mut graph, score, "value", comparison, "left");
    connect(&mut graph, threshold, "value", comparison, "right");
    connect(&mut graph, comparison, "value", conditional, "condition");
    connect(&mut graph, conditional, "then", success, "exec_in");
    connect(&mut graph, success, "exec_out", success_end, "exec_in");
    connect(&mut graph, conditional, "else", failure, "exec_in");
    connect(&mut graph, failure, "exec_out", failure_end, "exec_in");
    connect(&mut graph, conditional, "completed", choice, "exec_in");
    connect(&mut graph, choice, "option_0", accept_body, "exec_in");
    connect(&mut graph, accept_body, "exec_out", accept_end, "exec_in");
    connect(&mut graph, choice, "option_1", refuse_body, "exec_in");
    connect(&mut graph, refuse_body, "exec_out", refuse_end, "exec_in");
    connect(
        &mut graph,
        option_condition,
        "value",
        choice,
        "option_1_condition",
    );
    connect(&mut graph, choice, "completed", scene, "exec_in");

    assert!(graph.materialize_visible_defaults().unwrap() >= 4);
    let generated = transpile(&graph).unwrap();
    assert_eq!(
        generated.source,
        concat!(
            "label chapter_one\n",
            "    alice \"Bienvenue [player_name] !\"\n",
            "    set score = 2\n",
            "    if (score >= 10) {\n",
            "        alice \"Bravo.\"\n",
            "    } else {\n",
            "        alice \"Encore un effort.\"\n",
            "    }\n",
            "    choice {\n",
            "        \"Continuer\" => {\n",
            "            \"En avant !\"\n",
            "        }\n",
            "        \"Partir\" if true => {\n",
            "            \"À bientôt.\"\n",
            "        }\n",
            "    }\n",
            "    scene \"station\" with fade(500)\n",
        )
    );

    assert_eq!(generated.ast.len(), 6);
    assert!(matches!(generated.ast[0], Statement::Label { ref name } if name == "chapter_one"));
    assert!(matches!(
        generated.ast[2],
        Statement::SetVar {
            ref name,
            value: Expr::Int(2)
        } if name == "score"
    ));
    assert!(matches!(
        generated.ast[3],
        Statement::If {
            condition: Expr::BinOp { op: BinOpKind::Ge, .. },
            ref then_branch,
            ref else_branch,
        } if then_branch.len() == 1 && else_branch.len() == 1
    ));
    assert!(matches!(
        generated.ast[4],
        Statement::Choice { ref options }
            if options.len() == 2 && options[1].condition == Some(Expr::Bool(true))
    ));
    assert!(matches!(
        generated.ast[5],
        Statement::Scene {
            ref background,
            transition: Transition::Fade { .. }
        } if background == "station"
    ));

    // Le test couvre la sérialisation du document réellement stocké par l'éditeur.
    let restored = GraphDocument::from_json(&graph.to_pretty_json().unwrap()).unwrap();
    assert_eq!(transpile(&restored).unwrap().source, generated.source);

    // Les pins dynamiques de choix ont bien été créées à partir du catalogue.
    assert_eq!(accept.index, 0);
    assert_eq!(refuse.index, 1);
}

#[test]
fn rejects_an_invalid_label_before_emitting_source() {
    let mut graph = GraphDocument::new(
        GraphId::new(2),
        GraphKind::Label {
            name: "not a label".into(),
        },
    );
    add(&mut graph, NodeKind::Label);

    assert!(matches!(
        transpile(&graph),
        Err(TranspileError::InvalidIdentifier(value)) if value == "not a label"
    ));
}

#[test]
fn transpiles_the_runtime_instruction_catalog_and_reparses_it() {
    let mut graph = GraphDocument::new(
        GraphId::new(3),
        GraphKind::Label {
            name: "catalog".into(),
        },
    );
    let import = add(&mut graph, NodeKind::Use);
    graph
        .set_pin_default(
            import,
            "paths",
            PropertyValue::StringList(vec!["shared.rvn".into()]),
        )
        .unwrap();
    let root = add(&mut graph, NodeKind::Label);
    let kinds = [
        NodeKind::Config,
        NodeKind::CharacterCreate,
        NodeKind::CinematicShow,
        NodeKind::CinematicHide,
        NodeKind::UnlockEnding,
        NodeKind::SpriteShow,
        NodeKind::SpriteHide,
        NodeKind::SpriteMove,
        NodeKind::SpriteAnimate,
        NodeKind::SpriteStopAnimation,
        NodeKind::SpriteEffect,
        NodeKind::Timer,
        NodeKind::TimerCancel,
        NodeKind::MethodCall,
        NodeKind::MusicPlay,
        NodeKind::MusicStop,
        NodeKind::MusicVolume,
        NodeKind::SfxPlay,
        NodeKind::SfxStop,
        NodeKind::VoicePlay,
        NodeKind::VoiceStop,
        NodeKind::Imagemap,
        NodeKind::TypewriterSet,
        NodeKind::TypewriterSpeed,
    ];
    let nodes: Vec<_> = kinds
        .into_iter()
        .map(|kind| add(&mut graph, kind))
        .collect();
    for pair in std::iter::once(root)
        .chain(nodes.iter().copied())
        .collect::<Vec<_>>()
        .windows(2)
    {
        let output_key = if graph.nodes[&pair[0]].kind == NodeKind::Imagemap {
            "completed"
        } else {
            "exec_out"
        };
        connect(&mut graph, pair[0], output_key, pair[1], "exec_in");
    }
    let set_string = |graph: &mut GraphDocument, node: NodeId, key: &str, value: &str| {
        graph
            .set_pin_default(node, key, PropertyValue::String(value.into()))
            .unwrap();
    };
    set_string(&mut graph, nodes[0], "key", "window_title");
    set_string(&mut graph, nodes[0], "value", "Demo");
    set_string(&mut graph, nodes[1], "id", "alice");
    set_string(&mut graph, nodes[1], "display_name", "Alice");
    set_string(&mut graph, nodes[2], "cinematic", "intro");
    set_string(&mut graph, nodes[4], "id", "ending_one");
    for index in [5usize, 6, 7, 8, 9, 10] {
        set_string(&mut graph, nodes[index], "character", "alice");
    }
    set_string(&mut graph, nodes[5], "emotion", "happy");
    set_string(&mut graph, nodes[5], "position", "left");
    set_string(&mut graph, nodes[7], "position", "right");
    set_string(&mut graph, nodes[8], "animation", "idle");
    graph
        .set_property(
            nodes[8],
            "params",
            PropertyValue::StringList(vec!["loop=true".into(), "speed=1.5".into()]),
        )
        .unwrap();
    set_string(&mut graph, nodes[11], "target", "timeout");
    set_string(&mut graph, nodes[13], "target", "alice");
    set_string(&mut graph, nodes[13], "method", "blink");
    set_string(&mut graph, nodes[13], "arg", "fast");
    set_string(&mut graph, nodes[14], "file", "theme.ogg");
    set_string(&mut graph, nodes[17], "file", "click.ogg");
    set_string(&mut graph, nodes[18], "file", "click.ogg");
    set_string(&mut graph, nodes[19], "file", "line.ogg");
    set_string(&mut graph, nodes[21], "background", "map.png");
    graph
        .set_imagemap_hotspots(nodes[21], vec!["door:10:20:100:160".into()])
        .unwrap();

    let character=add(&mut graph,NodeKind::CharacterValue);
    let image=add(&mut graph,NodeKind::SpriteAsset);
    graph.set_property(character,"character",PropertyValue::String("alice".into())).unwrap();
    graph.set_property(image,"path",PropertyValue::String("sprites/alice/happy.png".into())).unwrap();
    connect(&mut graph,image,"value",character,"sprite");
    connect(&mut graph,character,"value",nodes[5],"character");
    let generated = transpile(&graph).unwrap();
    assert!(generated
        .source
        .starts_with("use \"shared.rvn\"\nlabel catalog\n"));
    assert!(generated.source.contains("alice.effect("));
    assert!(generated.source.contains("loop: true, speed: 1.5"));
    assert!(generated.source.contains("music.play(\"theme.ogg\")"));
    assert!(generated.source.contains("imagemap {"));
    assert!(generated
        .source
        .contains("hotspot { name: \"door\" area: (10, 20, 100, 160) } => {"));
    assert!(generated.source.contains("typewriter.speed(30)"));
}

#[test]
fn folds_variadic_boolean_operator_inputs_into_native_expressions() {
    let mut graph = GraphDocument::new(
        GraphId::new(4),
        GraphKind::Label {
            name: "operators".into(),
        },
    );
    let root = add(&mut graph, NodeKind::Label);
    let setter = add(&mut graph, NodeKind::SetVariable);
    let operator = add(&mut graph, NodeKind::LogicOr);
    let third = graph.add_operator_operand(operator).unwrap();
    let values = [true, false, true].map(|value| {
        let node = add(&mut graph, NodeKind::Literal);
        graph
            .set_property(node, "value", PropertyValue::Bool(value))
            .unwrap();
        node
    });
    define(
        &mut graph,
        "flag",
        ValueType::Bool,
        PropertyValue::Bool(false),
    );
    graph
        .set_pin_default(setter, "name", PropertyValue::String("flag".into()))
        .unwrap();
    connect(&mut graph, root, "exec_out", setter, "exec_in");
    connect(&mut graph, values[0], "value", operator, "left");
    connect(&mut graph, values[1], "value", operator, "right");
    let third_output = pin(&graph, values[2], "value");
    graph.connect(third_output, third).unwrap();
    connect(&mut graph, operator, "value", setter, "value");

    let generated = transpile(&graph).unwrap();
    assert!(generated
        .source
        .contains("set flag = ((true or false) or true)"));
}
