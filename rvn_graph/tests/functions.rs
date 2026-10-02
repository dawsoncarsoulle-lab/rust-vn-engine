use rvn_core::{Engine, Interaction, TerminalRenderer};
use rvn_graph::*;
use rvn_parser::{parse, Value};

const PROGRAM: &str = r#"
function total(items, multiplier) {
    set result = 0
    for item in items {
        set result = result + item * multiplier
    }
    set i = 0
    while i < 3 {
        set result = result + 1
        set i = i + 1
    }
    return result
}
function factorial(n) {
    if n < 2 { return 1 }
    return n * factorial(n - 1)
}
init { set score = 0 }
label start
    "Before"
    set score = total([2, 3], 4) + factorial(4)
    "Score: [score] / [total([1], 2)]"
    jump finished
label finished
"#;

#[test]
fn functions_and_nested_loops_execute_identically_in_script_and_blueprints() {
    let script = parse(PROGRAM).unwrap();
    let mut documents = import_script(&script).unwrap();
    let function = documents
        .iter()
        .find(|g| matches!(&g.kind, GraphKind::Function { name } if name == "total"))
        .unwrap();
    for kind in [
        NodeKind::FunctionEntry,
        NodeKind::FunctionReturn,
        NodeKind::ForEach,
        NodeKind::While,
    ] {
        assert!(function.nodes.values().any(|node| node.kind == kind));
    }
    for _ in 0..3 {
        let exported = transpile_project(&documents).unwrap();
        for script in [&script, &exported.ast] {
            let mut game = Engine::new(script.clone(), TerminalRenderer, 16).unwrap();
            assert!(
                matches!(game.step_until_interaction().unwrap(), Some(Interaction::Dialogue { text, .. }) if text == "Before")
            );
            game.advance_dialogue().unwrap();
            assert!(
                matches!(game.step_until_interaction().unwrap(), Some(Interaction::Dialogue { text, .. }) if text == "Score: 47 / 5")
            );
            assert_eq!(game.state.vars["score"], Value::Int(47));
            for local in ["n", "items", "item", "result", "i", "multiplier"] {
                assert!(!game.state.vars.contains_key(local));
            }
            let saved = rvn_core::save::SaveData::from_state(
                &game.state,
                1,
                "start".into(),
                "functions.rvn".into(),
            );
            let mut fresh = game.fresh(TerminalRenderer, 16).unwrap();
            fresh
                .load_data(serde_json::from_str(&serde_json::to_string(&saved).unwrap()).unwrap())
                .unwrap();
            assert_eq!(fresh.state.vars["score"], Value::Int(47));
            assert!(game.rollback());
            assert!(game.rollback());
            assert_eq!(game.state.vars["score"], Value::Int(0));
        }
        let mut combined = Vec::new();
        for (index, document) in documents.iter().enumerate() {
            let mut document = document.clone();
            if index > 0 {
                document.characters.clear();
            }
            combined.extend(transpile(&document).unwrap().ast);
        }
        documents = reimport_script(&combined, &documents).unwrap();
    }
}

#[test]
fn recursion_and_loop_limits_report_errors_without_committing_partial_state() {
    for operation in ["recurse(1)", "forever()"] {
        let script = format!("function recurse(n) {{ return recurse(n) }}\nfunction forever() {{ while true {{ }} return 0 }}\ninit {{ set score = 7 }}\nlabel start\nset score = {operation}\n");
        let mut game = Engine::new(parse(&script).unwrap(), TerminalRenderer, 16).unwrap();
        let error = game.step_until_interaction().unwrap_err();
        assert!(error.to_string().contains("limite"));
        assert_eq!(game.state.vars["score"], Value::Int(7));
    }
}

#[test]
fn pure_functions_reject_narrative_operations_and_duplicate_parameters() {
    for body in [
        "call start",
        "\"dialogue\"",
        "set persistent.score = 3",
        "function inner() { return 0 }",
    ] {
        let source = format!("function bad() {{ {body} }}\nlabel start\n");
        match parse(&source) {
            Err(_) => {}
            Ok(script) => assert!(Engine::new(script, TerminalRenderer, 16).is_err()),
        }
    }
    assert!(parse("function bad(n, n) { return n }").is_err());
    assert!(parse("init { function nested() { return 0 } }").is_err());
    assert!(parse("if true { function nested() { return 0 } }").is_err());
    assert!(parse("set __rvn_for_0_items = 1").is_err());
    assert!(parse("function hidden(__rvn_value) { return 0 }").is_err());
}

#[test]
fn empty_interactive_blocks_do_not_reset_the_execution_budget() {
    for interaction in ["choice { }", "imagemap { background: \"map.png\" }"] {
        let script = parse(&format!("label start\n{interaction}\njump start")).unwrap();
        let mut game = Engine::new(script, TerminalRenderer, 8).unwrap();
        assert!(game
            .step_until_interaction()
            .unwrap_err()
            .to_string()
            .contains("limite"));
    }
}

#[test]
fn return_value_does_not_change_narrative_call_return_semantics() {
    let script = parse("function twice(n) { return n * 2 }\nlabel start\ncall child\n\"resumed [twice(3)]\"\njump finished\nlabel child\nchoice { \"continue\" => { return } }\nlabel finished\n").unwrap();
    let mut game = Engine::new(script, TerminalRenderer, 16).unwrap();
    assert!(matches!(
        game.step_until_interaction().unwrap(),
        Some(Interaction::Choice { .. })
    ));
    game.submit_choice(0).unwrap();
    assert!(
        matches!(game.step_until_interaction().unwrap(), Some(Interaction::Dialogue { text, .. }) if text == "resumed 6")
    );
}

#[test]
fn function_failures_are_explicit_and_short_circuit_is_preserved() {
    for expression in ["bad()", "no_return()", "identity()", "identity(1, 2)"] {
        let source = format!("function no_return() {{ set x = 1 }}\nfunction identity(x) {{ return x }}\nlabel start\nset result = {expression}");
        let mut game = Engine::new(parse(&source).unwrap(), TerminalRenderer, 8).unwrap();
        assert!(game.step_until_interaction().is_err());
        assert!(!game.state.vars.contains_key("result"));
    }
    let source = parse(
        "function bad() { return 1 / 0 }\nlabel start\nset result = false and bad()\n\"okay\"",
    )
    .unwrap();
    let mut game = Engine::new(source, TerminalRenderer, 8).unwrap();
    assert!(game.step_until_interaction().unwrap().is_some());
    assert_eq!(game.state.vars["result"], Value::Bool(false));
}

#[test]
fn function_scope_is_lexical_not_inherited_from_the_caller() {
    let source = parse("function read() { return secret }\nfunction caller(secret) { return read() }\ninit { set secret = 3 }\nlabel start\nset result = caller(99)\n\"done\"").unwrap();
    let mut game = Engine::new(source, TerminalRenderer, 8).unwrap();
    game.step_until_interaction().unwrap();
    assert_eq!(game.state.vars["result"], Value::Int(3));
}

#[test]
fn narrative_iteration_resumes_after_save_load_and_rollback() {
    let script = parse("init { set item = 0 }\nlabel start\nfor item in [10, 20, 30] { \"Item [item]\" }\n\"done\"\njump finished\nlabel finished\n").unwrap();
    let compiled = transpile_project(&import_script(&script).unwrap()).unwrap();
    for script in [script, compiled.ast] {
        let mut game = Engine::new(script, TerminalRenderer, 32).unwrap();
        assert!(
            matches!(game.step_until_interaction().unwrap(), Some(Interaction::Dialogue { text, .. }) if text == "Item 10")
        );
        game.advance_dialogue().unwrap();
        assert!(
            matches!(game.step_until_interaction().unwrap(), Some(Interaction::Dialogue { text, .. }) if text == "Item 20")
        );
        let saved =
            rvn_core::save::SaveData::from_state(&game.state, 1, "start".into(), "loop.rvn".into());
        let mut fresh = game.fresh(TerminalRenderer, 32).unwrap();
        fresh
            .load_data(serde_json::from_str(&serde_json::to_string(&saved).unwrap()).unwrap())
            .unwrap();
        assert!(
            matches!(fresh.step_until_interaction().unwrap(), Some(Interaction::Dialogue { text, .. }) if text == "Item 20")
        );
        fresh.advance_dialogue().unwrap();
        assert!(
            matches!(fresh.step_until_interaction().unwrap(), Some(Interaction::Dialogue { text, .. }) if text == "Item 30")
        );
        assert!(game.rollback());
        assert!(game.rollback());
        assert!(
            matches!(game.step_until_interaction().unwrap(), Some(Interaction::Dialogue { text, .. }) if text == "Item 10")
        );
        game.advance_dialogue().unwrap();
        assert!(
            matches!(game.step_until_interaction().unwrap(), Some(Interaction::Dialogue { text, .. }) if text == "Item 20")
        );
    }
}

#[test]
fn authored_return_inside_nested_loop_choice_returns_to_real_caller() {
    let script = parse("label start\ncall child\n\"resumed\"\njump finished\nlabel child\nfor item in [1, 2] { while true { choice { \"return\" => { return } } } }\nlabel finished").unwrap();
    for script in [
        script.clone(),
        transpile_project(&import_script(&script).unwrap())
            .unwrap()
            .ast,
    ] {
        let mut game = Engine::new(script, TerminalRenderer, 16).unwrap();
        assert!(matches!(
            game.step_until_interaction().unwrap(),
            Some(Interaction::Choice { .. })
        ));
        game.submit_choice(0).unwrap();
        assert!(
            matches!(game.step_until_interaction().unwrap(), Some(Interaction::Dialogue { text, .. }) if text == "resumed")
        );
        assert!(game.state.call_stack.is_empty());
    }
}

#[test]
fn non_interactive_story_cycles_and_recursive_calls_are_bounded() {
    for source in [
        "label start\njump start",
        "label start\ncall start",
        "label start\nwhile true { }",
    ] {
        let mut game = Engine::new(parse(source).unwrap(), TerminalRenderer, 4).unwrap();
        assert!(game
            .step_until_interaction()
            .unwrap_err()
            .to_string()
            .contains("limite"));
    }
}
