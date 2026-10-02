use rvn_core::{Engine, Interaction, TerminalRenderer};
use rvn_graph::*;
use rvn_parser::{parse, Value};

const SOURCE: &str = r#"
function reward(quest) {
    return dict_get(quest, "reward", 0)
}
init {
    set quest = {"name": "letter", "reward": 10, "done": false}
    set score = 0
}
label start
    "Before"
    set quest = dict_set(quest, "done", true)
    set score = reward(quest)
    if quest["done"] {
        "[quest[\"name\"]] / [score]"
    }
    jump finished
label finished
"#;

#[test]
fn dictionary_literals_and_blueprint_calls_have_the_same_saved_state() {
    let script = parse(SOURCE).unwrap();
    let graphs = import_script(&script).unwrap();
    let compiled = transpile_project(&graphs).unwrap();
    for source in [script, compiled.ast] {
        let mut game = Engine::new(source, TerminalRenderer, 16).unwrap();
        game.step_until_interaction().unwrap();
        game.advance_dialogue().unwrap();
        assert!(
            matches!(game.step_until_interaction().unwrap(), Some(Interaction::Dialogue { text, .. }) if text == "letter / 10")
        );
        let saved =
            rvn_core::save::SaveData::from_state(&game.state, 1, "start".into(), "dict.rvn".into());
        let json = serde_json::to_string(&saved).unwrap();
        let mut fresh = game.fresh(TerminalRenderer, 16).unwrap();
        fresh
            .load_data(serde_json::from_str(&json).unwrap())
            .unwrap();
        assert_eq!(fresh.state.vars["quest"], game.state.vars["quest"]);
        assert!(game.rollback());
        assert!(game.rollback());
        let Value::Dict(quest) = &game.state.vars["quest"] else {
            panic!()
        };
        assert_eq!(quest["done"], Value::Bool(false));
    }
}

#[test]
fn dictionary_operations_preserve_inputs_and_return_stable_order() {
    let statements = parse("set source = {\"z\": 3, \"a\": 1}\nset edited = dict_set(source, \"a\", 7)\nset keys = dict_keys(edited)\nset values = dict_values(edited)\nset removed = dict_remove(edited, \"z\")\nset missing = dict_get(source, \"missing\", 99)").unwrap();
    let result = rvn_core::eval::FunctionLibrary::default()
        .execute(&statements, &Default::default())
        .unwrap();
    assert_eq!(
        result["keys"],
        Value::List(vec![Value::Str("a".into()), Value::Str("z".into())])
    );
    assert_eq!(
        result["values"],
        Value::List(vec![Value::Int(7), Value::Int(3)])
    );
    assert_eq!(result["missing"], Value::Int(99));
    let Value::Dict(source) = &result["source"] else {
        panic!()
    };
    assert_eq!(source["a"], Value::Int(1));
}

#[test]
fn invalid_dictionary_keys_and_duplicates_are_never_silently_accepted() {
    assert!(parse("set x = {\"a\": 1, \"a\": 2}").is_err());
    for expression in [
        "dict(\"a\")",
        "dict(1, 2)",
        "dict(\"a\", 1, \"a\", 2)",
        "dict_at({}, \"missing\")",
        "dict_set({}, 1, 2)",
        "{\"a\": 1}[0]",
    ] {
        let statements = parse(&format!("set result = {expression}")).unwrap();
        assert!(rvn_core::eval::FunctionLibrary::default()
            .execute(&statements, &Default::default())
            .is_err());
    }
}
