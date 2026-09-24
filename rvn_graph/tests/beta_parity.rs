use rvn_graph::*;

#[test]
fn make_color_exports_dynamic_rgba_and_preserves_inline_inputs() {
    let (mut g, root) = graph();
    let color = node(&mut g, NodeKind::MakeColor);
    let effect = node(&mut g, NodeKind::SpriteEffect);
    g.set_property(effect, "apply_tint", PropertyValue::Bool(true))
        .unwrap();
    default(
        &mut g,
        effect,
        "character",
        PropertyValue::String("alice".into()),
    );
    default(&mut g, color, "r", PropertyValue::Float(255.0));
    default(&mut g, color, "a", PropertyValue::Float(0.5));
    connect(&mut g, root, "exec_out", effect, "exec_in");
    connect(&mut g, color, "result", effect, "tint");
    let source = transpile(&g).unwrap().source;
    assert!(
        source.contains("make_color_rgb(255.0, 0.0, 0.0, 0.5)"),
        "{source}"
    );
    let restored = GraphDocument::from_json(&g.to_pretty_json().unwrap()).unwrap();
    assert_eq!(restored, g);
    g.materialize_visible_defaults().unwrap();
    for key in ["r", "g", "b", "a"] {
        let pin = g.pin_by_key(color, key).unwrap();
        assert!(!g.edges.values().any(|edge| edge.input == pin.id));
    }
    let script = rvn_parser::parse("set c = make_color(1, 0.5, 0, 0.5)\n").unwrap();
    let rvn_parser::Statement::SetVar { value, .. } = &script[0] else {
        panic!("assignment expected")
    };
    assert_eq!(
        rvn_core::eval_expr(value, &Default::default()).unwrap(),
        rvn_parser::Value::Str("#ff800080".into())
    );
    let rgb = rvn_parser::parse("set c = make_color_rgb(255, 128, 0, 0.5)\n").unwrap();
    let rvn_parser::Statement::SetVar { value, .. } = &rgb[0] else {
        panic!()
    };
    assert_eq!(
        rvn_core::eval_expr(value, &Default::default()).unwrap(),
        rvn_parser::Value::Str("#ff800080".into())
    );
}

#[test]
fn legacy_colors_upgrade_once_without_changing_sources_or_alpha() {
    let (mut g, _) = graph();
    let color = node(&mut g, NodeKind::MakeColor);
    g.nodes
        .get_mut(&color)
        .unwrap()
        .properties
        .remove("rgb_max");
    default(&mut g, color, "r", PropertyValue::Float(0.5));
    default(&mut g, color, "a", PropertyValue::Float(0.25));
    let literal = node(&mut g, NodeKind::Literal);
    g.set_property(literal, "value", PropertyValue::Float(0.75))
        .unwrap();
    connect(&mut g, literal, "value", color, "g");
    assert_eq!(g.upgrade_color_range().unwrap(), 1);
    assert_eq!(
        g.pin_by_key(color, "r").unwrap().default_value,
        Some(PropertyValue::Float(127.5))
    );
    assert_eq!(
        g.pin_by_key(color, "a").unwrap().default_value,
        Some(PropertyValue::Float(0.25))
    );
    assert_eq!(
        g.nodes[&literal].properties["value"],
        PropertyValue::Float(0.75)
    );
    let scale = g
        .nodes
        .values()
        .find(|n| n.kind == NodeKind::MathMultiply)
        .unwrap();
    assert_eq!(
        g.pin_by_key(scale.id, "right").unwrap().default_value,
        Some(PropertyValue::Float(255.0))
    );
    let snapshot = g.clone();
    assert_eq!(g.upgrade_color_range().unwrap(), 0);
    assert_eq!(g, snapshot);
}

#[test]
fn set_inline_values_remain_inline_after_save_and_load() {
    let (mut g, _) = graph();
    for (name, kind, value) in [
        ("flag", ValueType::Bool, PropertyValue::Bool(false)),
        ("score", ValueType::Int, PropertyValue::Int(0)),
        (
            "text",
            ValueType::String,
            PropertyValue::String("texte ici".into()),
        ),
    ] {
        g.add_variable(name, kind, value.clone()).unwrap();
        let set = node(&mut g, NodeKind::SetVariable);
        g.set_property(set, "name", PropertyValue::String(name.into()))
            .unwrap();
        default(&mut g, set, "value", value);
    }
    let mut loaded = GraphDocument::from_json(&g.to_pretty_json().unwrap()).unwrap();
    assert_eq!(loaded.materialize_visible_defaults().unwrap(), 0);
    assert_eq!(loaded, g);
}

fn graph() -> (GraphDocument, NodeId) {
    let mut graph = GraphDocument::new(
        GraphId::new(1),
        GraphKind::Label {
            name: "start".into(),
        },
    );
    let root = graph.add_catalog_node(NodeKind::Label, [0.0, 0.0]).unwrap();
    (graph, root)
}
fn node(g: &mut GraphDocument, kind: NodeKind) -> NodeId {
    g.add_catalog_node(kind, [0.0, 0.0]).unwrap()
}
fn connect(g: &mut GraphDocument, a: NodeId, ak: &str, b: NodeId, bk: &str) {
    g.connect(
        g.pin_by_key(a, ak).unwrap().id,
        g.pin_by_key(b, bk).unwrap().id,
    )
    .unwrap();
}
fn default(g: &mut GraphDocument, n: NodeId, key: &str, value: PropertyValue) {
    let id = g.pin_by_key(n, key).unwrap().id;
    g.pins.get_mut(&id).unwrap().default_value = Some(value);
}
#[test]
fn named_label_can_be_reentered_with_jump() {
    let (mut g, root) = graph();
    let label = node(&mut g, NodeKind::Label);
    g.set_property(label, "label", PropertyValue::String("door".into()))
        .unwrap();
    let jump = node(&mut g, NodeKind::Jump);
    default(&mut g, jump, "target", PropertyValue::String("door".into()));
    connect(&mut g, root, "exec_out", jump, "exec_in");
    let result = transpile(&g).unwrap();
    assert!(result.source.contains("jump door"));
    assert!(result.source.contains("label door"));
    g.set_property(label, "label", PropertyValue::String("start".into()))
        .unwrap();
    assert!(transpile(&g).is_err());
}
#[test]
fn typed_function_arguments_and_mixed_lists_preserve_types() {
    let (mut g, root) = graph();
    g.add_variable(
        "values",
        ValueType::List(Box::new(ValueType::Any)),
        PropertyValue::StringList(vec![]),
    )
    .unwrap();
    let set = node(&mut g, NodeKind::SetVariable);
    g.set_property(set, "name", PropertyValue::String("values".into()))
        .unwrap();
    let array = node(&mut g, NodeKind::ListLiteral);
    g.resize_value_inputs(array, 3).unwrap();
    default(
        &mut g,
        array,
        "item_1",
        PropertyValue::String("hello".into()),
    );
    default(&mut g, array, "item_2", PropertyValue::Bool(true));
    let function = node(&mut g, NodeKind::FunctionCall);
    g.set_property(function, "function", PropertyValue::String("random".into()))
        .unwrap();
    g.resize_value_inputs(function, 2).unwrap();
    default(&mut g, function, "item_0", PropertyValue::Int(1));
    default(&mut g, function, "item_1", PropertyValue::Int(6));
    connect(&mut g, root, "exec_out", set, "exec_in");
    connect(&mut g, function, "result", array, "item_0");
    connect(&mut g, array, "list", set, "value");
    let g = GraphDocument::from_json(&g.to_pretty_json().unwrap()).unwrap();
    assert!(transpile(&g)
        .unwrap()
        .source
        .contains("[random(1, 6), \"hello\", true]"));
}
#[test]
fn upgrading_legacy_list_never_reinterprets_strings() {
    let (mut g, _) = graph();
    let list = node(&mut g, NodeKind::ListLiteral);
    g.set_property(
        list,
        "items",
        PropertyValue::StringList(vec!["1".into(), "true".into()]),
    )
    .unwrap();
    g.resize_value_inputs(list, 3).unwrap();
    assert_eq!(
        g.pin_by_key(list, "item_0").unwrap().default_value,
        Some(PropertyValue::String("1".into()))
    );
    assert_eq!(
        g.pin_by_key(list, "item_1").unwrap().default_value,
        Some(PropertyValue::String("true".into()))
    );
    g.resize_value_inputs(list, 0).unwrap();
    assert!(g.pin_by_key(list, "item_0").is_none());
}
#[test]
fn choice_conditions_are_evaluated_before_display() {
    let (mut g, root) = graph();
    let choice = node(&mut g, NodeKind::Choice);
    g.add_choice_option(choice, "Partir").unwrap();
    g.add_choice_option(choice, "Ouvrir").unwrap();
    g.add_choice_option(choice, "Secret").unwrap();
    g.set_property(
        choice,
        "option_1_condition",
        PropertyValue::String("possede_cle".into()),
    )
    .unwrap();
    g.set_property(
        choice,
        "option_2_condition",
        PropertyValue::String("confiance >= 50".into()),
    )
    .unwrap();
    connect(&mut g, root, "exec_out", choice, "exec_in");
    let source = transpile(&g).unwrap().source;
    assert!(source.contains("\"Partir\" =>"));
    assert!(source.contains("\"Ouvrir\" if possede_cle =>"));
    assert!(source.contains("\"Secret\" if confiance >= 50 =>"));
    assert!(validate_expression("true\nset injected = 1").is_err());
}
#[test]
fn effect_modifies_only_checked_properties() {
    let (mut g, root) = graph();
    let effect = node(&mut g, NodeKind::SpriteEffect);
    default(
        &mut g,
        effect,
        "character",
        PropertyValue::String("alice".into()),
    );
    default(&mut g, effect, "rotation", PropertyValue::Float(45.0));
    g.set_property(effect, "apply_rotation", PropertyValue::Bool(true))
        .unwrap();
    connect(&mut g, root, "exec_out", effect, "exec_in");
    assert!(transpile(&g)
        .unwrap()
        .source
        .contains("alice.effect(rotation: 45)"));
}
#[test]
fn imagemap_supports_independent_hover_rectangle() {
    let (mut g, root) = graph();
    let map = node(&mut g, NodeKind::Imagemap);
    default(
        &mut g,
        map,
        "background",
        PropertyValue::String("room.png".into()),
    );
    g.set_imagemap_hotspots(map, vec!["door:0:0:100:200:100:0:200:200".into()])
        .unwrap();
    connect(&mut g, root, "exec_out", map, "exec_in");
    let script = transpile(&g).unwrap();
    assert!(script.source.contains("hover_area: (100, 0, 200, 200)"));
}

#[test]
fn project_export_checks_destinations_and_duplicate_labels() {
    let (mut first, root) = graph();
    let jump = node(&mut first, NodeKind::Jump);
    default(
        &mut first,
        jump,
        "target",
        PropertyValue::String("door".into()),
    );
    connect(&mut first, root, "exec_out", jump, "exec_in");
    assert!(transpile_project(&[first.clone()]).is_err());
    let (mut second, _) = graph();
    second.kind = GraphKind::Label {
        name: "door".into(),
    };
    second.characters.insert("alice".into(), "Alice".into());
    first.characters = second.characters.clone();
    let compiled = transpile_project(&[second.clone(), first.clone()]).unwrap();
    assert!(
        compiled.source.find("label start").unwrap() < compiled.source.find("label door").unwrap()
    );
    assert_eq!(compiled.source.matches("character.create").count(), 1);
    assert!(transpile_project(&[first.clone(), first]).is_err());
}

#[test]
fn parser_keeps_first_list_element_and_allows_index_after_call() {
    let parsed = rvn_parser::parse("set a = [1, 2, true]\nset b = reverse(a)[0]\n").unwrap();
    assert!(
        matches!(&parsed[0], rvn_parser::Statement::SetVar { value: rvn_parser::Expr::ListLit(items), .. } if items.len() == 3)
    );
    assert!(matches!(
        &parsed[1],
        rvn_parser::Statement::SetVar {
            value: rvn_parser::Expr::Index { .. },
            ..
        }
    ));
}

#[test]
fn animation_typed_parameters_override_legacy_values_once() {
    let (mut g, root) = graph();
    let animation = node(&mut g, NodeKind::SpriteAnimate);
    default(
        &mut g,
        animation,
        "character",
        PropertyValue::String("alice".into()),
    );
    default(
        &mut g,
        animation,
        "animation",
        PropertyValue::String("shake".into()),
    );
    g.set_property(
        animation,
        "params",
        PropertyValue::StringList(vec!["duration=0.3".into()]),
    )
    .unwrap();
    g.set_property(
        animation,
        "animation_param_duration",
        PropertyValue::Float(0.7),
    )
    .unwrap();
    g.set_property(animation, "animation_param_loop", PropertyValue::Bool(true))
        .unwrap();
    connect(&mut g, root, "exec_out", animation, "exec_in");
    let source = transpile(&g).unwrap().source;
    assert!(source.contains("duration: 0.7"));
    assert!(source.contains("loop: true"));
    assert_eq!(source.matches("duration:").count(), 1);
}

#[test]
fn exported_choice_failure_dialogue_returns_to_same_menu_in_engine() {
    use rvn_core::{Engine, Interaction, TerminalRenderer};
    let (mut g, root) = graph();
    let door = node(&mut g, NodeKind::Label);
    g.set_property(door, "label", PropertyValue::String("door".into()))
        .unwrap();
    let first_jump = node(&mut g, NodeKind::Jump);
    default(
        &mut g,
        first_jump,
        "target",
        PropertyValue::String("door".into()),
    );
    connect(&mut g, root, "exec_out", first_jump, "exec_in");
    let choice = node(&mut g, NodeKind::Choice);
    g.add_choice_option(choice, "Ouvrir la porte").unwrap();
    g.add_choice_option(choice, "Partir").unwrap();
    connect(&mut g, door, "exec_out", choice, "exec_in");
    let condition = node(&mut g, NodeKind::If);
    default(&mut g, condition, "condition", PropertyValue::Bool(false));
    connect(&mut g, choice, "option_0", condition, "exec_in");
    let dialogue = node(&mut g, NodeKind::Dialogue);
    let text = node(&mut g, NodeKind::TextValue);
    g.set_property(
        text,
        "value",
        PropertyValue::String("Je n’ai pas la clé.".into()),
    )
    .unwrap();
    connect(&mut g, text, "value", dialogue, "text");
    connect(&mut g, condition, "else", dialogue, "exec_in");
    let retry = node(&mut g, NodeKind::Jump);
    default(
        &mut g,
        retry,
        "target",
        PropertyValue::String("door".into()),
    );
    connect(&mut g, dialogue, "exec_out", retry, "exec_in");
    let compiled = transpile_project(&[g]).unwrap();
    let mut engine = Engine::new(compiled.ast, TerminalRenderer, 20).unwrap();
    assert!(
        matches!(engine.step_until_interaction().unwrap(), Some(Interaction::Choice { options }) if options.len() == 2)
    );
    engine.submit_choice(0).unwrap();
    assert!(
        matches!(engine.step_until_interaction().unwrap(), Some(Interaction::Dialogue { text, .. }) if text == "Je n’ai pas la clé.")
    );
    engine.advance_dialogue().unwrap();
    assert!(
        matches!(engine.step_until_interaction().unwrap(), Some(Interaction::Choice { options }) if options[0] == "Ouvrir la porte")
    );
}
