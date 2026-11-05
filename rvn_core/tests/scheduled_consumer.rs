use rvn_core::{consumer_schedule::ConsumerFrame, eval::FunctionLibrary, random::RandomState};
use rvn_parser::{parse, ConsumerStep, Expr, Name, Statement, Value};
use std::collections::HashMap;

fn run(source: &str, vars: &HashMap<String,Value>, random: &mut RandomState, frame: &mut ConsumerFrame) -> HashMap<String,Value> {
    let script = parse(source).unwrap();
    let library = FunctionLibrary::from_script(&script).unwrap();
    let body: Vec<_> = script.into_iter().filter(|stmt| !matches!(stmt,Statement::Function{..})).collect();
    library.execute_in_consumer_frame(&body,vars,random,frame).unwrap()
}
fn inspector(consumer: &str, input: &str, target: &str) -> String {
    format!(r#"flow.consume_v1("fixture-source:scope", "{consumer}") {{
        inspect "inspector-node" = {input}
        set {target} = [__rvn_terminal_v1("inspector-node", "valid"), __rvn_terminal_v1("inspector-node", "table_id"), __rvn_terminal_v1("inspector-node", "key")]
    }}"#)
}
const TABLE: &str = r#"__rvn_string_table_text_name(__rvn_literal_name("asset:TableCase"), "hello")"#;

#[test]
fn typed_out_terminals_retain_false_and_start_fresh_per_frame() {
    let mut frame = ConsumerFrame::default(); let mut rng = RandomState::seeded(4);
    let vars = run(&inspector("first",TABLE,"before"),&HashMap::new(),&mut rng,&mut frame);
    let vars = run(&inspector("second",r#"__rvn_invariant_text("literal")"#,"after"),&vars,&mut rng,&mut frame);
    assert_eq!(vars["before"],Value::List(vec![Value::Bool(true),Value::Name(Name::new("asset:TableCase").unwrap()),Value::Str("hello".into())]));
    assert_eq!(vars["after"],Value::List(vec![Value::Bool(false),Value::Name(Name::new("asset:TableCase").unwrap()),Value::Str("hello".into())]));
    let fresh = run(&inspector("fresh",r#"__rvn_invariant_text("literal")"#,"after"),&vars,&mut rng,&mut ConsumerFrame::default());
    assert_eq!(fresh["after"],Value::List(vec![Value::Bool(false),Value::Name(Name::default()),Value::Str(String::new())]));
}

#[test]
fn one_shared_user_producer_consumes_exactly_one_random_draw() {
    let source = r#"
    function make_text() {
        set draw = random(1, 1)
        return __rvn_string_table_text_name(__rvn_literal_name("asset:TableCase"), "hello")
    }
    flow.consume_v1("source:scope", "consumer") {
        capture "effectful-producer" = make_text()
        inspect "inspector" = __rvn_captured_v1("effectful-producer")
        set outputs = [__rvn_terminal_v1("inspector", "valid"), __rvn_terminal_v1("inspector", "table_id"), __rvn_terminal_v1("inspector", "key"), __rvn_captured_v1("effectful-producer")]
    }
    "#;
    let mut rng = RandomState::seeded(88); let mut expected = rng;
    expected.integer(1,1).unwrap();
    let vars = run(source,&HashMap::new(),&mut rng,&mut ConsumerFrame::default());
    assert_eq!(rng,expected);
    assert!(!vars.contains_key("effectful-producer"));
    assert!(!vars.contains_key("draw"));
    assert!(matches!(vars["outputs"],Value::List(ref values) if values.len()==4));
}

#[test]
fn two_executed_consumers_and_loop_iterations_each_regather_once() {
    let source = r#"
    set index = 0
    while index < 3 {
        flow.consume_v1("source:scope", "same-consumer") {
            capture "draw" = random(1, 1)
            set index = index + __rvn_captured_v1("draw")
        }
    }
    flow.consume_v1("source:scope", "second-consumer") {
        capture "draw" = random(1, 1)
        set end = __rvn_captured_v1("draw")
    }
    "#;
    let mut rng = RandomState::seeded(33); let mut expected = rng;
    for _ in 0..4 { expected.integer(1,1).unwrap(); }
    let vars = run(source,&HashMap::new(),&mut rng,&mut ConsumerFrame::default());
    assert_eq!(rng,expected); assert_eq!(vars["index"],Value::Int(3));
}

#[test]
fn skipped_branch_never_evaluates_scheduled_dependencies() {
    let source = r#"if false { flow.consume_v1("scope", "unused") {
        capture "draw" = random(1, 1)
        set bad = __rvn_captured_v1("draw")
    }} set good = 4"#;
    let mut rng = RandomState::seeded(3); let expected=rng;
    let vars=run(source,&HashMap::new(),&mut rng,&mut ConsumerFrame::default());
    assert_eq!(rng,expected); assert!(!vars.contains_key("bad"));
}

#[test]
fn recursive_calls_have_fresh_terminals_and_restore_caller_capture() {
    let source = r#"
    function recurse(depth) {
        if depth == 0 {
            flow.consume_v1("function:recurse", "leaf") {
                inspect "inspect" = __rvn_invariant_text("literal")
                return __rvn_terminal_v1("inspect", "table_id")
            }
        }
        flow.consume_v1("function:recurse", "table") {
            inspect "inspect" = __rvn_string_table_text_name(__rvn_literal_name("asset:Table"), "hello")
            set before = __rvn_terminal_v1("inspect", "table_id")
        }
        flow.consume_v1("function:recurse", "recursive") {
            capture "recursive-output" = recurse(depth - 1)
            inspect "inspect" = __rvn_invariant_text("literal")
            return [__rvn_captured_v1("recursive-output"), __rvn_terminal_v1("inspect", "table_id")]
        }
    }
    flow.consume_v1("source:caller", "caller") {
        capture "result" = recurse(2)
        set result = __rvn_captured_v1("result")
    }
    "#;
    let vars=run(source,&HashMap::new(),&mut RandomState::default(),&mut ConsumerFrame::default());
    let name=Value::Name(Name::new("asset:Table").unwrap());
    assert_eq!(vars["result"],Value::List(vec![Value::List(vec![Value::Name(Name::default()),name.clone()]),name]));
}

#[test]
fn failed_consumer_rolls_back_random_frame_and_user_variables() {
    let mut frame=ConsumerFrame::default(); let mut rng=RandomState::seeded(9);
    let vars=run(&inspector("first",TABLE,"before"),&HashMap::new(),&mut rng,&mut frame);
    let before=frame.clone(); let random_before=rng; let variables_before=vars.clone();
    let source=r#"flow.consume_v1("fixture-source:scope", "fails") {
        capture "draw" = random(1, 1)
        inspect "inspector-node" = __rvn_string_table_text_name(__rvn_literal_name("asset:Changed"), "newkey")
        set result = 1 / 0
    }"#;
    let result=FunctionLibrary::default().execute_in_consumer_frame(&parse(source).unwrap(),&vars,&mut rng,&mut frame);
    assert!(result.is_err()); assert_eq!(frame,before); assert_eq!(rng,random_before); assert_eq!(vars,variables_before);
}

#[test]
fn plan_version_order_duplicate_and_dynamic_ports_reject_before_effects() {
    let mut statement=parse(&inspector("first",TABLE,"result")).unwrap().remove(0);
    let Statement::ScheduledConsumer {version,..}=&mut statement else {panic!()}; *version=3;
    let mut rng=RandomState::seeded(99); let before=rng; let mut frame=ConsumerFrame::default();
    assert!(FunctionLibrary::default().execute_in_consumer_frame(&[statement],&HashMap::new(),&mut rng,&mut frame).is_err());
    for source in [
        r#"flow.consume_v1("scope", "consumer") { capture "a" = __rvn_captured_v1("future") set x = 1 }"#,
        r#"flow.consume_v1("scope", "consumer") { inspect "a" = __rvn_invariant_text("x") set x = __rvn_terminal_v1("a", random(1,1)) }"#,
    ] {
        assert!(FunctionLibrary::default().execute_in_consumer_frame(&parse(source).unwrap(),&HashMap::new(),&mut rng,&mut frame).is_err());
    }
    assert!(parse(r#"flow.consume_v1("scope", "consumer") { capture "a" = 1 inspect "a" = "x" set x = 1 }"#).is_err());
    assert_eq!(rng,before);
}

#[test]
fn frame_cold_roundtrip_is_typed_bounded_and_not_a_save_claim() {
    let mut frame=ConsumerFrame::default();
    run(&inspector("first",TABLE,"result"),&HashMap::new(),&mut RandomState::default(),&mut frame);
    let bytes=serde_json::to_vec(&frame).unwrap();
    let restored:ConsumerFrame=serde_json::from_slice(&bytes).unwrap(); restored.validate().unwrap(); assert_eq!(restored,frame);
    let mut forged=serde_json::to_value(&frame).unwrap(); forged["version"]=serde_json::json!(2);
    let invalid:ConsumerFrame=serde_json::from_value(forged).unwrap(); assert!(invalid.validate().is_err());
    assert!(serde_json::from_str::<ConsumerFrame>(r#"{"version":1,"scopes":{},"globals":{}}"#).is_err());
}

#[test]
fn source_ast_three_cold_cycles_preserve_contract_and_pure_legacy_calls() {
    let source=inspector("cold",TABLE,"result");
    let mut ast=parse(&source).unwrap();
    for _ in 0..3 {
        let bytes=serde_json::to_vec(&ast).unwrap(); ast=serde_json::from_slice(&bytes).unwrap();
        assert_eq!(ast,parse(&source).unwrap());
    }
    assert!(rvn_parser::validate_logic(&ast,false).is_empty());
    let legacy=parse(r#"set id = __rvn_text_table_id(__rvn_invariant_text("literal"))"#).unwrap();
    assert_eq!(FunctionLibrary::default().execute(&legacy,&HashMap::new()).unwrap()["id"],Value::Name(Name::default()));
    let forbidden=parse(r#"set x = __rvn_captured_v1("outside")"#).unwrap();
    assert!(FunctionLibrary::default().execute(&forbidden,&HashMap::new()).is_err());
}

#[test]
fn direct_ast_rejects_hidden_second_consumer_or_forward_capture() {
    let stmt=Statement::ScheduledConsumer {version:1,scope:"scope".into(),consumer:"consumer".into(),
        steps:vec![ConsumerStep::Capture{producer:"a".into(),value:Expr::Int(1)}],
        body:vec![Statement::SetVar{name:"x".into(),value:Expr::Int(1)},Statement::SetVar{name:"y".into(),value:Expr::Int(2)}]};
    assert!(FunctionLibrary::default().execute_in_consumer_frame(&[stmt],&HashMap::new(),&mut RandomState::default(),&mut ConsumerFrame::default()).is_err());
}

#[test]
fn legacy_root_entry_refuses_nested_schedule_without_guessing_owner() {
    let source=format!("if true {{ {} }}",inspector("inside",TABLE,"result"));
    let block=parse(&source).unwrap(); let mut rng=RandomState::seeded(4); let before=rng;
    let error=FunctionLibrary::default().execute_with_random(&block,&HashMap::new(),&mut rng).unwrap_err();
    assert!(error.to_string().contains("explicitly owned")); assert_eq!(rng,before);
}

#[test]
fn old_lazy_boolean_rhs_does_not_become_eager() {
    let mut rng=RandomState::seeded(4); let before=rng;
    let vars=FunctionLibrary::default().execute_with_random(&parse("set x = false and random(1, 1)\nset y = true or random(1, 1)").unwrap(),&HashMap::new(),&mut rng).unwrap();
    assert_eq!(rng,before); assert_eq!(vars["x"],Value::Bool(false)); assert_eq!(vars["y"],Value::Bool(true));
}

#[test]
fn callee_cannot_observe_caller_capture_and_failure_rolls_back_draw() {
    let source=r#"
    function steal() { return __rvn_captured_v1("secret") }
    flow.consume_v1("scope", "consumer") {
        capture "secret" = random(1, 1)
        capture "attempt" = steal()
        set result = __rvn_captured_v1("attempt")
    }"#;
    let script=parse(source).unwrap(); let library=FunctionLibrary::from_script(&script).unwrap();
    let mut rng=RandomState::seeded(99); let before=rng; let mut frame=ConsumerFrame::default(); let frame_before=frame.clone();
    let error=library.execute_in_consumer_frame(&script[1..],&HashMap::new(),&mut rng,&mut frame).unwrap_err();
    assert!(error.to_string().contains("outside its consumer")); assert_eq!(rng,before); assert_eq!(frame,frame_before);
}

#[test]
fn forged_or_overbudget_frame_is_rejected_before_any_consumer_effect() {
    let terminal=serde_json::json!({"valid":false,"table_id":"None","key":"x".repeat(4000)});
    let terminals:serde_json::Map<String,serde_json::Value>=(0..300).map(|index|(format!("producer-{index}"),terminal.clone())).collect();
    let frame:ConsumerFrame=serde_json::from_value(serde_json::json!({"version":1,"scopes":{"scope":terminals}})).unwrap();
    assert!(frame.validate().is_err());
    let mut invalid=frame; let mut rng=RandomState::seeded(99); let before=rng;
    let block=parse(r#"flow.consume_v1("scope", "consumer") { capture "draw" = random(1, 1) set x = __rvn_captured_v1("draw") }"#).unwrap();
    assert!(FunctionLibrary::default().execute_in_consumer_frame(&block,&HashMap::new(),&mut rng,&mut invalid).is_err());
    assert_eq!(rng,before);
    for badkey in ["x".repeat(4097),"nul\0key".into()] {
        let frame:ConsumerFrame=serde_json::from_value(serde_json::json!({"version":1,"scopes":{"scope":{"producer":{"valid":false,"table_id":"None","key":badkey}}}})).unwrap();
        assert!(frame.validate().is_err());
    }
}

#[test]
fn exact_consumer_child_origin_survives_prepare_and_engine_executes_owned_plan() {
    let source=format!("scene label Start\n{}\n\"Next\"",inspector("real-origin",TABLE,"result"));
    let parsed=rvn_parser::parse_program(&source).unwrap();
    assert_eq!(parsed.origins[1].span.children.len(),1);
    let child=&parsed.origins[1].span.children[0];
    assert!(source[child.range.clone()].starts_with("set result"));
    let prepared=rvn_core::program::PreparedNarrative::from_parsed(parsed).unwrap();
    let mut engine=rvn_core::Engine::from_narrative(prepared,rvn_core::TerminalRenderer,8).unwrap();
    let before=engine.state.random;
    engine.step_silent().unwrap();
    assert!(matches!(engine.peek_statement(),Some(Statement::Dialogue {..})));
    assert_eq!(engine.state.random,before); assert!(engine.state.vars.contains_key("result"));
    assert!(engine.state.scene_execution.as_ref().unwrap().active.as_ref().unwrap().consumer_frame.is_some());
}
