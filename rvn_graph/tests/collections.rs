use rvn_core::{Engine, Interaction, TerminalRenderer};
use rvn_graph::*;
use rvn_parser::{Statement, Value};

const INVENTORY: &str = r#"
init {
    set inventory = ["letter"]
    set score = 0
}
label start
    narrator "Before"
    set inventory = list_append(inventory, "key")
    set inventory = list_insert(inventory, 1, "map")
    set inventory = list_set(inventory, 0, "opened letter")
    set inventory = list_concat(inventory, ["coin", "torch"])
    set inventory = list_remove(inventory, 3)
    set score = max(len(inventory), abs(-2))
    if contains(inventory, "key") {
        narrator "Ready: [inventory[1]] / [score]"
    }
    return
"#;

#[test]
fn shipped_collection_example_plays_both_paths_from_code_and_graphs() {
    let script =
        rvn_parser::parse(include_str!("../../examples/collections/inventory.rvn")).unwrap();
    let exported = transpile_project(&import_script(&script).unwrap())
        .unwrap()
        .ast;
    for source in [script, exported] {
        for choice in [0, 1] {
            let mut game = Engine::new(source.clone(), TerminalRenderer, 16).unwrap();
            assert!(matches!(
                game.step_until_interaction().unwrap(),
                Some(Interaction::Dialogue { .. })
            ));
            game.advance_dialogue().unwrap();
            assert!(matches!(
                game.step_until_interaction().unwrap(),
                Some(Interaction::Choice { .. })
            ));
            game.submit_choice(choice).unwrap();
            let expected = if choice == 0 {
                "The door opens. / La porte s'ouvre."
            } else {
                "The door stays locked. / La porte reste fermée."
            };
            assert!(
                matches!(game.step_until_interaction().unwrap(), Some(Interaction::Dialogue { text, .. }) if text == expected)
            );
            game.advance_dialogue().unwrap();
            assert!(matches!(
                game.step_until_interaction().unwrap(),
                Some(Interaction::Dialogue { .. })
            ));
            game.advance_dialogue().unwrap();
            assert!(game.step_until_interaction().unwrap().is_none());
            assert!(game.is_finished());
        }
    }
}

#[test]
fn inventory_roundtrips_with_editable_calls_lists_and_indices() {
    let mut graphs = import_script(&rvn_parser::parse(INVENTORY).unwrap()).unwrap();
    assert_eq!(
        graphs[0].variables["inventory"].value_type,
        ValueType::List(Box::new(ValueType::String))
    );
    let label = &graphs[1];
    assert!(label
        .nodes
        .values()
        .any(|n| n.kind == NodeKind::FunctionCall));
    assert!(label.nodes.values().any(|n| n.kind == NodeKind::MathNegate));
    for _ in 0..3 {
        let exported = transpile_project(&graphs).unwrap();
        // Source import includes declarations; standalone labels do not carry
        // their variable registry in RVN. Avoid export-only end sentinels here.
        let mut combined = Vec::new();
        for (index, graph) in graphs.iter().enumerate() {
            let mut graph = graph.clone();
            if index > 0 {
                graph.characters.clear();
            }
            combined.extend(transpile(&graph).unwrap().ast);
        }
        graphs = import_script(&combined).unwrap();
        let mut game = Engine::new(exported.ast, TerminalRenderer, 16).unwrap();
        assert!(
            matches!(game.step_until_interaction().unwrap(), Some(Interaction::Dialogue { text, .. }) if text == "Before")
        );
        game.advance_dialogue().unwrap();
        assert!(
            matches!(game.step_until_interaction().unwrap(), Some(Interaction::Dialogue { text, .. }) if text == "Ready: map / 4")
        );
    }
}

#[test]
fn inventory_edits_survive_save_and_restore_on_rollback() {
    let mut game =
        Engine::new(rvn_parser::parse(INVENTORY).unwrap(), TerminalRenderer, 16).unwrap();
    game.step_until_interaction().unwrap();
    game.advance_dialogue().unwrap();
    game.step_until_interaction().unwrap();
    let saved = rvn_core::save::SaveData::from_state(
        &game.state,
        1,
        "start".into(),
        "inventory.rvn".into(),
    );
    let saved_json = serde_json::to_string(&saved).unwrap();
    let mut restored =
        Engine::new(rvn_parser::parse(INVENTORY).unwrap(), TerminalRenderer, 16).unwrap();
    restored
        .load_data(serde_json::from_str(&saved_json).unwrap())
        .unwrap();
    assert_eq!(
        restored.state.vars["inventory"],
        game.state.vars["inventory"]
    );
    assert!(game.rollback());
    assert!(game.rollback());
    assert_eq!(
        game.state.vars["inventory"],
        Value::List(vec![Value::Str("letter".into())])
    );
}

#[test]
fn import_index_and_zero_argument_call_use_the_correct_output_pins() {
    let source = rvn_parser::parse("init { set n = 0 }\nlabel start\nset n = [1, 2 + 3][1]\nset n = mouse_x()\nset n = \"hello\"[0]\nreturn\n").unwrap();
    let graphs = import_script(&source).unwrap();
    let exported = transpile(&graphs[1]).unwrap();
    assert!(exported.ast.iter().any(|s| matches!(
        s,
        Statement::SetVar {
            value: rvn_parser::Expr::Index { .. },
            ..
        }
    )));
    assert!(graphs[1]
        .nodes
        .values()
        .any(|n| n.kind == NodeKind::ListLiteral));
    assert!(exported.source.contains("mouse_x()"));
}

#[test]
fn index_pins_accept_list_positions_and_dictionary_keys() {
    let mut graph = GraphDocument::new(GraphId::new(1), GraphKind::Init);
    let node = graph.add_catalog_node(NodeKind::Index, [0.0, 0.0]).unwrap();
    let target = &graph.pin_by_key(node, "target").unwrap().value_type;
    for allowed in [
        ValueType::String,
        ValueType::InterpolatedText,
        ValueType::List(Box::new(ValueType::Int)),
        ValueType::Any,
    ] {
        assert!(target.accepts(&allowed));
    }
    let index = &graph.pin_by_key(node, "index").unwrap().value_type;
    assert!(index.accepts(&ValueType::Int));
    assert!(index.accepts(&ValueType::String));
    for rejected in [ValueType::Bool, ValueType::Float] {
        assert!(!index.accepts(&rejected));
    }
    for invalid in ["true[0]", "3[0]", "[1][false]"] {
        let ast = rvn_parser::parse(&format!("set x = {invalid}\n")).unwrap();
        let Statement::SetVar { value, .. } = &ast[0] else {
            panic!()
        };
        assert!(rvn_core::eval_expr(value, &Default::default()).is_err());
    }
}

#[test]
fn oversized_collection_import_fails_instead_of_truncating() {
    let items = (0..129)
        .map(|n| n.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    let script = rvn_parser::parse(&format!(
        "init {{ set n = 0 }}\nlabel start\nset n = len([{items}])\nreturn\n"
    ))
    .unwrap();
    assert!(import_script(&script).unwrap_err().contains("128"));
}

#[test]
fn list_operations_reject_invalid_arguments_without_mutating_inputs() {
    let vars = [(
        "items".into(),
        Value::List(vec![Value::Int(1), Value::Int(2)]),
    )]
    .into_iter()
    .collect();
    for expression in [
        "list_remove(items, -1)",
        "list_remove(items, 2)",
        "list_insert(items, 3, 0)",
        "list_set(items, 0.5, 0)",
        "list_slice(items, 2, 1)",
        "list_append(items)",
        "list_concat(items, 1)",
        "list_remove(1, 0)",
    ] {
        let ast = rvn_parser::parse(&format!("set x = {expression}\n")).unwrap();
        let Statement::SetVar { value, .. } = &ast[0] else {
            panic!()
        };
        assert!(rvn_core::eval_expr(value, &vars).is_err(), "{expression}");
    }
    assert_eq!(
        vars["items"],
        Value::List(vec![Value::Int(1), Value::Int(2)])
    );
    for (expression, expected) in [
        ("list_insert(items, 2, 3)", vec![1, 2, 3]),
        ("list_slice(items, 0, 2)", vec![1, 2]),
        ("list_slice(items, 2, 2)", vec![]),
        ("list_remove(items, 0)", vec![2]),
    ] {
        let ast = rvn_parser::parse(&format!("set x = {expression}\n")).unwrap();
        let Statement::SetVar { value, .. } = &ast[0] else {
            panic!()
        };
        assert_eq!(
            rvn_core::eval_expr(value, &vars).unwrap(),
            Value::List(expected.into_iter().map(Value::Int).collect())
        );
    }
}
