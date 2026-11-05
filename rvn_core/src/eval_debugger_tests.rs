use super::*;

#[test]
fn consumer_owned_mid_producer_continuation_resumes_without_second_random_effect() {
    let source=r#"
        function producer() { set draw = random(1,1) return __rvn_string_table_text_name(__rvn_literal_name("asset:Table"),"hello") }
        function consumer() {
            flow.consume_v1("scope","first") { capture "p" = producer() inspect "i" = __rvn_captured_v1("p") set before = __rvn_terminal_v1("i","table_id") }
            flow.consume_v1("scope","second") { inspect "i" = __rvn_invariant_text("literal") return [__rvn_terminal_v1("i","valid"),__rvn_terminal_v1("i","table_id"),__rvn_terminal_v1("i","key")] }
        }
    "#;
    let library=FunctionLibrary::from_script(&rvn_parser::parse(source).unwrap()).unwrap();
    let probe=Probe::shared();let mut debug=library.clone();debug.debug_probe=Some(probe.clone());
    let globals=HashMap::new();let expr=expression("consumer()");let mut random=RandomState::seeded(4);let mut expected=random;expected.integer(1,1).unwrap();
    assert!(debug.eval_with_random(&expr,&globals,&mut random).is_err());assert_eq!(random,RandomState::seeded(4));
    let mut continuation=Probe::pending(&probe,&library).unwrap().unwrap();
    let mut stopped=false;
    for _ in 0..256 {
        if continuation.state.random==expected && continuation.site().is_some_and(|site|site.function.as_deref()==Some("producer")&&site.route==vec![Route::Statement(1)]) {stopped=true;break;}
        continuation.step().unwrap();
    }
    assert!(stopped,"stop must be the actual producer return after its random effect");
    assert!(continuation.frames.iter().any(|frame|frame.function.as_deref()==Some("consumer")&&frame.active_consumer.is_some()));
    assert_eq!(random,RandomState::seeded(4),"real caller remains uncommitted while candidate is stopped");
    let trace=Probe::portable_trace(&probe,&continuation).unwrap();
    let json=serde_json::to_string(&trace).unwrap();
    assert!(!json.contains("ScheduledConsumer") && !json.contains("InspectTextTable") && !json.contains("BinOp"),"wire must carry data/routes, never executable AST");
    let trace:PortableTrace=serde_json::from_str(&json).unwrap();
    let cold=FunctionLibrary::from_script(&rvn_parser::parse(source).unwrap()).unwrap();
    let cold_probe=Probe::shared();let mut cold_debug=cold.clone();cold_debug.debug_probe=Some(cold_probe.clone());
    assert!(cold_debug.eval_with_random(&expr,&globals,&mut RandomState::seeded(4)).is_err());
    let mut resumed=Probe::pending(&cold_probe,&cold).unwrap().unwrap();
    resumed.restore_portable(&trace,None).unwrap();
    assert_eq!(resumed.state.random,expected);
    while !resumed.done(){resumed.step().unwrap();}
    while !continuation.done(){continuation.step().unwrap();}
    assert_eq!(resumed.result,continuation.result);assert_eq!(resumed.state.random,continuation.state.random);
    let mut invalid=serde_json::to_value(&trace).unwrap();
    invalid["pending"]["tasks"][0]["kind"]=serde_json::json!("injected_call");
    let invalid:PortableTrace=serde_json::from_value(invalid).unwrap();
    let mut rejected=Probe::pending(&cold_probe,&cold).unwrap().unwrap();
    assert!(rejected.restore_portable(&invalid,None).is_err());assert_eq!(rejected.state.random,RandomState::seeded(4));
    assert_eq!(continuation.state.random,expected);
    assert_eq!(continuation.result,Some(Value::List(vec![Value::Bool(false),Value::Name(rvn_parser::Name::new("asset:Table").unwrap()),Value::Str("hello".into())])));
    Probe::finish(&probe,&continuation).unwrap();Probe::rewind(&probe).unwrap();
    let result=debug.eval_with_random(&expr,&globals,&mut random).unwrap();assert_eq!(Some(result),continuation.result);assert_eq!(random,expected);
}

#[test]
fn debugger_names_replay_typed_identity_conversions_and_errors_with_the_same_budget() {
    let library=FunctionLibrary::default();
    let globals=HashMap::new();
    for source in [
        "__rvn_string_to_name(\"RvnCase\") == __rvn_string_to_name(\"rvncase\")",
        "__rvn_string_to_name(\"É\") == __rvn_string_to_name(\"é\")",
        "__rvn_string_to_name(\"_2147483646\") == __rvn_string_to_name(\"None\")",
        "__rvn_string_to_name(\"_2147483647\") == __rvn_string_to_name(\"None\")",
        "__rvn_name_to_string(__rvn_string_to_name(\"AuthoredCase_0\"))",
        "[__rvn_string_to_name(\"none\"),__rvn_string_to_name(\"None_0\")]",
        "__rvn_name_to_string(\"legacy String\")",
        "__rvn_string_to_name(12)",
        "__rvn_string_to_name(\"Typed\") + \"String\"",
    ] {
        let expr=expression(source);
        let operation=|library:&FunctionLibrary| {
            let mut random=RandomState::seeded(219);
            let mut budget=crate::ui::CanvasBudget::default();
            let value=library.eval_scoped_with_random_budgeted(&expr,&globals,&globals,&mut random,&mut budget);
            (value,random,budget.steps,budget.value_work)
        };
        assert_eq!(resumed(&library,operation),operation(&library),"{source}");
    }
    assert_eq!(library.eval(&expression("__rvn_string_to_name(\"RvnCase\") == __rvn_string_to_name(\"rvncase\")"),&globals).unwrap(),Value::Bool(true));
    assert_eq!(library.eval(&expression("\"RvnCase\" == \"rvncase\""),&globals).unwrap(),Value::Bool(false));
    assert!(library.eval(&expression("__rvn_name_to_string(\"legacy String\")"),&globals).is_err());
    let names=Value::List(vec![Value::Name(rvn_parser::Name::new(&"x".repeat(1023)).unwrap());1024]);
    assert!(crate::value_limits::value_work(&names).is_err());
}

fn expression(text:&str)->Expr{
    let script=rvn_parser::parse(&format!("set answer = {text}")).unwrap();
    let Statement::SetVar{value,..}=&script[0]else{panic!("actual parser assignment")};value.clone()
}

#[test]
fn debugger_table_queries_replay_the_same_immutable_metadata_snapshot_and_budget() {
    use crate::string_table::{StringTableAsset,StringTableEntry};
    let mut library=FunctionLibrary::default();
    library.text_resources_mut().publish(StringTableAsset{id:"asset:query".into(),namespace:"VN".into(),entries:[("key".into(),StringTableEntry{source:"Raw".into(),developer_notes:String::new(),metadata:[("Speaker".into(),"Éloïse".into())].into()})].into()}).unwrap();
    for source in ["__rvn_string_table_metadata(\"asset:query\",\"key\",\"speaker\")","__rvn_string_table_entry_registered(\"asset:query\",\"missing\")","__rvn_string_table_keys(\"asset:query\")","__rvn_registered_string_tables()"] {
        let expr=expression(source); let globals=HashMap::new();
        let operation=|library:&FunctionLibrary|{let mut random=RandomState::seeded(19);let mut budget=crate::ui::CanvasBudget::default();let value=library.eval_scoped_with_random_budgeted(&expr,&globals,&globals,&mut random,&mut budget);(value,random,budget.steps,budget.value_work)};
        assert_eq!(resumed(&library,operation),operation(&library));
    }
    let expr=expression("__rvn_string_table_metadata(\"asset:query\",\"key\",\"speaker\")");
    let globals=HashMap::new();let probe=Probe::shared();let mut debug=library.clone();debug.debug_probe=Some(probe.clone());let mut random=RandomState::seeded(19);
    assert!(debug.eval_with_random(&expr,&globals,&mut random).is_err());
    let mut continuation=Probe::pending(&probe,&library).unwrap().unwrap();
    let mut changed=debug.text_resources().ready_table("asset:query").unwrap().clone();changed.entries.get_mut("key").unwrap().set_metadata("Speaker".into(),"Changed".into()).unwrap();debug.text_resources_mut().publish(changed).unwrap();
    while !continuation.done(){continuation.step().unwrap();}
    assert_eq!(continuation.result,Some(Value::Str("Éloïse".into())));
    Probe::finish(&probe,&continuation).unwrap();Probe::rewind(&probe).unwrap();
    assert!(debug.eval_with_random(&expr,&globals,&mut random).is_err());assert!(probe.lock().unwrap().stale);assert_eq!(random,RandomState::seeded(19));
}

#[test]
fn debugger_table_conversion_retains_snapshot_and_rejects_changed_resource_trace() {
    use crate::string_table::{StringTableAsset, StringTableEntry};
    let asset = |source: &str| StringTableAsset { id: "asset:table".into(), namespace: "VN".into(),
        entries: [("key".into(), StringTableEntry {metadata:Default::default(), source: source.into(), developer_notes: String::new() })].into() };
    let mut library = FunctionLibrary::default(); library.text_resources_mut().publish(asset("First")).unwrap();
    let expr = expression("text_to_string(__rvn_string_table_text(\"asset:table\",\"key\"))");
    let globals = HashMap::new();
    assert_eq!(resumed(&library, |library| library.eval(&expr, &globals)).unwrap(), Value::Str("First".into()));
    let probe = Probe::shared(); let mut debug = library.clone(); debug.debug_probe = Some(probe.clone());
    let mut random = RandomState::seeded(3);
    assert!(debug.eval_with_random(&expr, &globals, &mut random).is_err());
    let mut continuation = Probe::pending(&probe, &library).unwrap().unwrap();
    // A continuation owns the old immutable resource context.
    debug.text_resources_mut().publish(asset("Changed")).unwrap();
    while !continuation.done() { continuation.step().unwrap(); }
    assert_eq!(continuation.result, Some(Value::Str("First".into())));
    Probe::finish(&probe, &continuation).unwrap(); Probe::rewind(&probe).unwrap();
    assert!(debug.eval_with_random(&expr, &globals, &mut random).is_err());
    assert_eq!(random, RandomState::seeded(3));
    assert!(probe.lock().unwrap().stale);
}

#[test]
fn debugger_table_truthiness_uses_the_same_resource_and_error_contract_as_evaluation() {
    use crate::string_table::{StringTableAsset, StringTableEntry};
    let mut library = FunctionLibrary::default();
    library.text_resources_mut().publish(StringTableAsset { id: "asset:table".into(), namespace: "VN".into(),
        entries: [("empty".into(), StringTableEntry {metadata:Default::default(), source: String::new(), developer_notes: String::new() }),
            ("key".into(), StringTableEntry {metadata:Default::default(), source: "Nonempty".into(), developer_notes: String::new() })].into() }).unwrap();
    for source in ["not __rvn_string_table_text(\"asset:table\",\"empty\")",
        "__rvn_string_table_text(\"asset:table\",\"key\") and true",
        "__rvn_string_table_text(\"missing\",\"key\") or false"] {
        let expr = expression(source); let globals = HashMap::new();
        let operation = |library: &FunctionLibrary| library.eval(&expr, &globals);
        assert_eq!(resumed(&library, operation), operation(&library));
    }
}

#[test]
fn debugger_table_predicate_loop_charges_resolved_text_work_identically_before_step_limit() {
    use crate::string_table::{StringTableAsset, StringTableEntry};
    let script=rvn_parser::parse(r#"function run() {local count=0 while __rvn_string_table_text("asset:table","key") {set count=count+1} return count}"#).unwrap();
    let mut library=FunctionLibrary::from_script(&script).unwrap();
    library.text_resources_mut().publish(StringTableAsset {id:"asset:table".into(),namespace:"VN".into(),
        entries:[("key".into(),StringTableEntry{metadata:Default::default(),source:"X".repeat(900_000),developer_notes:String::new()})].into()}).unwrap();
    let expr=expression("run()");let globals=HashMap::new();
    let operation=|library:&FunctionLibrary| {
        let mut random=RandomState::seeded(9);let mut budget=crate::ui::CanvasBudget::default();
        let value=library.eval_scoped_with_random_budgeted(&expr,&globals,&globals,&mut random,&mut budget);
        (value,random,budget.steps,budget.value_work)
    };
    let normal=operation(&library);
    assert!(matches!(normal.0,Err(EvalError::ExecutionLimit{limit})if limit.starts_with("1 000 000")));
    assert!(normal.2<1000);assert!(normal.3>1_000_000);assert_eq!(normal.1,RandomState::seeded(9));
    assert_eq!(resumed(&library,operation),normal);
}

#[test]
fn debugger_table_conversion_loop_counts_hidden_arguments_and_keeps_failure_atomic() {
    use crate::string_table::{StringTableAsset, StringTableEntry};
    let script = rvn_parser::parse(r#"function run() {
local count=0
local drawn=random(1,9)
while count<1000 {
    local message=__rvn_format_text("",{"unused":__rvn_string_table_text("asset:work","payload")})
    local converted=text_to_string(message)
    set count=count+1
}
return count
}"#).unwrap();
    let mut library = FunctionLibrary::from_script(&script).unwrap();
    library.text_resources_mut().publish(StringTableAsset { id: "asset:work".into(), namespace: "VN.Work".into(),
        entries: [("payload".into(), StringTableEntry {metadata:Default::default(), source: "X".repeat(400_000), developer_notes: String::new() })].into() }).unwrap();
    let expr = expression("run()"); let globals = HashMap::new();
    let operation = |library: &FunctionLibrary| {
        let mut random = RandomState::seeded(11); let mut budget = crate::ui::CanvasBudget::default();
        let result = library.eval_scoped_with_random_budgeted(&expr, &globals, &globals, &mut random, &mut budget);
        (result, random, budget.steps, budget.value_work)
    };
    let normal = operation(&library);
    assert!(matches!(normal.0, Err(EvalError::ExecutionLimit { limit }) if limit.starts_with("1 000 000")));
    assert!(normal.2 < 1000); assert!(normal.3 > 1_000_000); assert_eq!(normal.1, RandomState::seeded(11));
    assert_eq!(resumed(&library, operation), normal); assert!(globals.is_empty());
}

#[test]
fn debugger_table_host_predicate_keeps_random_atomic_and_combines_expression_display_work() {
    use crate::string_table::{StringTableAsset, StringTableEntry};
    let script = rvn_parser::parse(r#"function unavailable() {local drew=random(1,9) return __rvn_string_table_text("missing","key")}
function bad() {local drew=random(1,9) return __rvn_string_table_text("asset:oversized","key")}
function near_limit() {
local count=0
local drew=random(1,9)
while count<39 {
local message=__rvn_format_text("",{"unused":__rvn_string_table_text("asset:work","payload")})
local converted=text_to_string(message)
set count=count+1
}
return __rvn_format_text("",{"unused":__rvn_string_table_text("asset:work","payload")})
}"#).unwrap();
    let mut library = FunctionLibrary::from_script(&script).unwrap();
    library.text_resources_mut().publish(StringTableAsset { id: "asset:work".into(), namespace: "VN.Work".into(),
        entries: [("payload".into(), StringTableEntry {metadata:Default::default(), source: "X".repeat(400_000), developer_notes: String::new() })].into() }).unwrap();
    library.text_resources_mut().publish(StringTableAsset { id: "asset:oversized".into(), namespace: "VN.Work".into(),
        entries: [("key".into(), StringTableEntry {metadata:Default::default(), source: "X".repeat(1_048_577), developer_notes: String::new() })].into() }).unwrap();
    // A valid unavailable reference projects to a nonempty marker. Both normal
    // and resumed evaluation commit the successful random draw exactly once.
    let expr = expression("unavailable()"); let globals = HashMap::new();
    let operation = |library: &FunctionLibrary| {
        let mut random = RandomState::seeded(15);
        (library.eval_bool_with_random(&expr, &globals, &mut random), random)
    };
    let normal = operation(&library); assert_eq!(normal.0, Ok(true)); assert_ne!(normal.1, RandomState::seeded(15));
    assert_eq!(resumed(&library, operation), normal);
    for call in ["bad()", "near_limit()"] {
        let expr = expression(call); let globals = HashMap::new();
        let operation = |library: &FunctionLibrary| {
            let mut random = RandomState::seeded(15);
            (library.eval_bool_with_random(&expr, &globals, &mut random), random)
        };
        let normal = operation(&library); assert!(normal.0.is_err()); assert_eq!(normal.1, RandomState::seeded(15));
        if call == "near_limit()" { assert!(matches!(normal.0, Err(EvalError::ExecutionLimit { limit }) if limit.starts_with("1 000 000"))); }
        assert_eq!(resumed(&library, operation), normal);
    }
}

#[test]
fn debugger_text_conversion_uses_retained_locale_and_rejects_a_changed_locale_trace() {
    use crate::locale::{LocaleManager, LocaleTable};
    use rvn_parser::localized_text::{DeferredText, LocalizedText, TextIdentity, TextLocalization};
    let identity = TextIdentity { owner: "asset:1".into(), namespace: "VN".into(), key: "greeting".into() };
    let text = Value::Text(Box::new(DeferredText::Literal { text: LocalizedText { source: "Hello".into(),
        localization: TextLocalization::Localizable { identity: identity.clone(), developer_notes: String::new() } } }));
    let mut locale = LocaleManager::from_tables("unused", "en", "fr", vec!["en".into(), "fr".into()], HashMap::from([
        ("fr".into(), LocaleTable { lang: "fr".into(), strings: HashMap::from([(identity.locale_key(), "Bonjour".into())]) }),
    ]));
    let library = FunctionLibrary::default();
    let globals = HashMap::from([("text".into(), text)]);
    let expr = expression("text_to_string(text)");
    let operation = |library: &FunctionLibrary| library.eval_scoped_with_locale(&expr, &globals, &globals, &mut RandomState::seeded(3), Some(&locale));
    assert_eq!(resumed(&library, operation).unwrap(), Value::Str("Bonjour".into()));
    let probe = Probe::shared(); let mut debug = library.clone(); debug.debug_probe = Some(probe.clone());
    let mut random = RandomState::seeded(3);
    assert!(debug.eval_scoped_with_locale(&expr, &globals, &globals, &mut random, Some(&locale)).is_err());
    let mut continuation = Probe::pending(&probe, &library).unwrap().unwrap();
    while !continuation.done() { continuation.step().unwrap(); }
    Probe::finish(&probe, &continuation).unwrap(); Probe::rewind(&probe).unwrap();
    locale.set_language("en").unwrap();
    assert!(debug.eval_scoped_with_locale(&expr, &globals, &globals, &mut random, Some(&locale)).is_err());
    assert_eq!(random, RandomState::seeded(3));
}
fn resumed<T>(library:&FunctionLibrary,mut operation:impl FnMut(&FunctionLibrary)->T)->T{
    let probe=Probe::shared();let mut debug=library.clone();debug.debug_probe=Some(probe.clone());
    for _ in 0..256{
        Probe::rewind(&probe).unwrap();let result=operation(&debug);
        let Some(mut continuation)=Probe::pending(&probe,library).unwrap()else{
            assert!(Probe::complete(&probe));Probe::seal(&probe).unwrap();Probe::rewind(&probe).unwrap();
            let replay=operation(&debug);assert!(Probe::complete(&probe));return replay;
        };
        // Each drain resumes the owned stack; even a budget refusal is retained
        // as the original result rather than resetting the computation budget.
        let mut drains=0;
        while !continuation.done(){let _=continuation.step();drains+=1;assert!(drains<200_000);}
        Probe::finish(&probe,&continuation).unwrap();drop(result);
    }panic!("bounded roots did not complete")
}

#[test]
fn debugger_resumable_function_lexical_scope_random_and_short_circuit_equal_normal_evaluation(){
    let script=rvn_parser::parse("function inner(p) { local v = p + score return v + random(1,9) }\nfunction outer(p) { local score = 900 return inner(p) }\n").unwrap();
    let library=FunctionLibrary::from_script(&script).unwrap();
    let globals=HashMap::from([("score".into(),Value::Int(7))]);
    let environment=HashMap::from([("score".into(),Value::Int(99)),("p".into(),Value::Int(3))]);
    for text in ["outer(p)","false and outer(p)","true or outer(p)","[outer(p), inner(p)]"]{
        let expression=expression(text);
        let operation=|library:&FunctionLibrary|{
            let mut random=RandomState::seeded(55);let mut budget=crate::ui::CanvasBudget::default();
            let result=library.eval_scoped_with_random_budgeted(&expression,&environment,&globals,&mut random,&mut budget);
            (result,random,budget.steps,budget.value_work)
        };
        let normal=operation(&library);assert_eq!(resumed(&library,operation),normal);
        if text.starts_with("false")||text.starts_with("true"){assert_eq!(normal.1,RandomState::seeded(55));}
    }
    assert_eq!(globals["score"],Value::Int(7));assert_eq!(environment["score"],Value::Int(99));
}

#[test]
fn debugger_handler_returns_exact_commands_globals_locals_and_budget_once(){
    let script=rvn_parser::parse("function choose_value(p) { return p + random(1,4) }\nhandler update(event) { local scratch = choose_value(event) set score = scratch for item in [1,2] { set score = score + item } ui.close(\"inventory\") ui.set_state(\"journal\",\"item\",{\"id\":score,\"text\":\"été 🍃\"}) }\n").unwrap();
    let library=FunctionLibrary::from_script(&script).unwrap();
    let Statement::Handler{parameters,body,..}=&script[1]else{panic!("actual handler")};
    let globals=HashMap::from([("score".into(),Value::Int(0))]);
    let operation=|library:&FunctionLibrary|{
        let mut random=RandomState::seeded(91);let mut budget=crate::ui::CanvasBudget::default();
        let result=library.debugger_scope("handler:update".into(),||library.handle_event_budgeted(parameters,body,&[Value::Int(3)],&globals,&mut random,&mut budget));
        (result,random,budget.steps,budget.value_work)
    };
    let normal=operation(&library);assert_eq!(resumed(&library,operation),normal);
    let (values,commands)=normal.0.unwrap();assert!(!values.contains_key("scratch"));assert!(!values.contains_key("item"));assert_eq!(commands.len(),2);
    assert_eq!(globals["score"],Value::Int(0));
}

#[test]
fn debugger_budget_failure_across_many_stops_preserves_original_error_random_and_accumulated_work(){
    let script=rvn_parser::parse("handler runaway(event) { set score = random(1,9) while true { set score = score + 1 } }\n").unwrap();
    let library=FunctionLibrary::from_script(&script).unwrap();
    let Statement::Handler{parameters,body,..}=&script[0]else{panic!("actual handler")};
    let globals=HashMap::from([("score".into(),Value::Int(0))]);
    let operation=|library:&FunctionLibrary|{
        let mut random=RandomState::seeded(17);let mut budget=crate::ui::CanvasBudget::default();
        let result=library.handle_event_budgeted(parameters,body,&[Value::Int(0)],&globals,&mut random,&mut budget);
        (result,random,budget.steps,budget.value_work)
    };
    let normal=operation(&library);assert!(normal.0.is_err());assert_eq!(normal.1,RandomState::seeded(17));
    assert_eq!(resumed(&library,operation),normal);assert_eq!(globals["score"],Value::Int(0));
}

#[test]
fn debugger_owned_stack_exposes_actual_callee_frame_and_truthful_missing_watches(){
    let script=rvn_parser::parse("function inner(p) { local own = p + 1 return own }\nfunction outer(p) { local caller_only = 99 return inner(p) }\n").unwrap();
    let library=FunctionLibrary::from_script(&script).unwrap();let probe=Probe::shared();let mut debug=library.clone();debug.debug_probe=Some(probe.clone());
    let globals=HashMap::from([("global".into(),Value::Int(7))]);let expression=expression("outer(4)");
    assert!(debug.eval(&expression,&globals).is_err());let mut continuation=Probe::pending(&probe,&library).unwrap().unwrap();
    let watches=vec![wire::Watch{scope:wire::WatchScope::Frame(2),name:"caller_only".into()},wire::Watch{scope:wire::WatchScope::Frame(8),name:"p".into()},wire::Watch{scope:wire::WatchScope::Globals,name:"global".into()}];
    let mut visited=false;
    for _ in 0..64{
        let snapshot=continuation.snapshot(3,4,crate::scene::ResumePosition{pc:9,origin:None},&watches);
        if snapshot.site.as_ref().is_some_and(|site|site.function.as_deref()==Some("inner")){
            assert_eq!(snapshot.frames.len(),3);assert_eq!(snapshot.frames[2].depth,2);
            assert_eq!(snapshot.watches[0].value,wire::WatchValue::NoProperty);assert_eq!(snapshot.watches[1].value,wire::WatchValue::NotInScope);
            assert_eq!(snapshot.watches[2].value,wire::WatchValue::Value(Value::Int(7)));visited=true;break;
        }
        continuation.step().unwrap();
    }assert!(visited);
    while !continuation.done(){continuation.step().unwrap();}
    assert_eq!(continuation.result,Some(Value::Int(5)));assert_eq!(continuation.frames.len(),1);
}

#[test]
fn debugger_sealed_trace_refuses_changed_expression_before_any_new_random_draw(){
    let library=FunctionLibrary::default();let probe=Probe::shared();let mut debug=library.clone();debug.debug_probe=Some(probe.clone());
    let expr=expression("random(1,9)");let globals=HashMap::new();let mut random=RandomState::seeded(3);
    assert!(debug.eval_with_random(&expr,&globals,&mut random).is_err());assert_eq!(random,RandomState::seeded(3));
    let mut continuation=Probe::pending(&probe,&library).unwrap().unwrap();while !continuation.done(){continuation.step().unwrap();}
    Probe::finish(&probe,&continuation).unwrap();Probe::rewind(&probe).unwrap();assert!(debug.eval_with_random(&expr,&globals,&mut random).is_ok());
    Probe::seal(&probe).unwrap();Probe::rewind(&probe).unwrap();let before=random;
    assert!(debug.eval_with_random(&expression("random(2,9)"),&globals,&mut random).is_err());assert_eq!(random,before);
}

#[test]
fn debugger_structured_flow_and_motion_curves_equal_original_results_rng_errors_and_cumulative_budgets(){
    let cases=[
        ("function calc() {local total=0 flow.sequence() {{flow.for_loop(i,-2,3) {flow.sequence() {{set total=total+i} {if i==0 {flow.break()} set total=total+100}}}} {for n in [2,3] {set total=total+n}} {while total<210 {set total=total+1}}} return total}","calc()"),
        ("function calc() {local total=0 flow.for_loop(i,0,2) {flow.for_loop(j,0,3) {set total=total+1 flow.break()} set total=total+10} while true {for n in [1,2] {flow.for_loop(k,0,3) {set total=total+n flow.break()}} return total} return total}","calc()"),
        ("function calc() {local total=0 flow.for_loop(i,3,2) {} flow.for_loop(j,9223372036854775807,9223372036854775807) {set total=total+1} return total}","calc()"),
        ("function calc() {flow.for_loop(i,0,1.5) {} return 1}","calc()"),
        ("function calc() {return 9223372036854775807+1}","calc()"),
        ("function calc() {return [1][4]}","calc()"),
        ("function curve(t) {return t*t}","motion_curve(\"curve\",17)"),
        ("function curve(t) {return t+random(0,1)}","motion_curve(\"curve\",17)"),
    ];
    for (source,text) in cases{
        let script=rvn_parser::parse(source).unwrap();let library=FunctionLibrary::from_script(&script).unwrap();
        let expr=expression(text);let globals=HashMap::from([("neighbor".into(),Value::Int(99))]);
        let operation=|library:&FunctionLibrary|{
            let mut random=RandomState::seeded(123);let mut budget=crate::ui::CanvasBudget::default();
            let result=library.eval_with_random_budgeted(&expr,&globals,&mut random,&mut budget);
            (result,random,budget.steps,budget.value_work)
        };
        let normal=operation(&library);assert_eq!(resumed(&library,operation),normal,"{source}");
        assert_eq!(globals["neighbor"],Value::Int(99));
    }
}

#[test]
fn debugger_source_witness_changes_invalidate_retained_locale_even_when_translations_and_language_match() {
    use crate::locale::{LocaleCatalog, LocaleManager, LocaleTable};
    use rvn_parser::localized_text::{DeferredText, LocalizedText, TextIdentity, TextLocalization};
    let identity = TextIdentity { owner: "asset:witness".into(), namespace: "VN".into(), key: "greeting".into() };
    let make_locale = |source: &str| LocaleManager::from_catalogs("unused", "en", "fr", vec!["en".into(), "fr".into()], HashMap::from([
        ("fr".into(), LocaleCatalog::with_text_sources(LocaleTable { lang: "fr".into(), strings: HashMap::from([(identity.locale_key(), "Bonjour".into())]) },
            HashMap::from([(identity.locale_key(), source.into())])).unwrap()),
    ]));
    let mut locale = make_locale("Hello");
    let text = Value::Text(Box::new(DeferredText::Literal { text: LocalizedText { source: "Hello".into(), localization:
        TextLocalization::Localizable { identity: identity.clone(), developer_notes: String::new() } } }));
    let globals = HashMap::from([("text".into(), text)]);
    let library = FunctionLibrary::default(); let expr = expression("text_to_string(text)");
    assert_eq!(resumed(&library, |library| library.eval_scoped_with_locale(&expr, &globals, &globals, &mut RandomState::seeded(17), Some(&locale))).unwrap(), Value::Str("Bonjour".into()));
    let probe = Probe::shared(); let mut debug = library.clone(); debug.debug_probe = Some(probe.clone());
    let mut random = RandomState::seeded(17);
    assert!(debug.eval_scoped_with_locale(&expr, &globals, &globals, &mut random, Some(&locale)).is_err());
    let mut continuation = Probe::pending(&probe, &library).unwrap().unwrap();
    while !continuation.done() { continuation.step().unwrap(); }
    Probe::finish(&probe, &continuation).unwrap(); Probe::rewind(&probe).unwrap();
    locale = make_locale("Changed");
    assert!(debug.eval_scoped_with_locale(&expr, &globals, &globals, &mut random, Some(&locale)).is_err());
    assert_eq!(random, RandomState::seeded(17));
    assert!(probe.lock().unwrap().stale);
}

#[test]
fn consumer_portable_cursor_reconstructs_each_phase_of_structured_calls_ranges_short_circuit_and_curve(){
    let cases=[
        ("function leaf(p) {return p+random(1,1)} function calc() {local total=0 for item in [1,2] {set total=total+leaf(item)} flow.for_loop(i,-1,1) {set total=total+i} if true {set total=total+1} else {set total=total+9} while total<10 {set total=total+1} return [total,false and leaf(9),true or leaf(9),[1,2][0],-1,not false]}","calc()"),
        ("function calc() {local total=0 flow.for_loop(i,9223372036854775807,9223372036854775807) {set total=total+1} return total}","calc()"),
        ("function curve(t) {return t*t}","motion_curve(\"curve\",3)"),
    ];
    for(source,text)in cases {
        let script=rvn_parser::parse(source).unwrap();let library=FunctionLibrary::from_script(&script).unwrap();let probe=Probe::shared();let mut debug=library.clone();debug.debug_probe=Some(probe.clone());let expr=expression(text);let globals=HashMap::new();
        assert!(debug.eval_with_random(&expr,&globals,&mut RandomState::seeded(4)).is_err());let mut continuation=Probe::pending(&probe,&library).unwrap().unwrap();let mut visits=0;
        while !continuation.done(){
            let trace=Probe::portable_trace(&probe,&continuation).unwrap();let wire=serde_json::to_string(&trace).unwrap();let trace:PortableTrace=serde_json::from_str(&wire).unwrap();
            let mut cold=Probe::pending(&probe,&library).unwrap().unwrap();cold.restore_portable(&trace,None).unwrap_or_else(|error|panic!("{source} visit{visits}: {error}"));
            cold.step().unwrap();continuation.step().unwrap();assert_eq!(cold.values,continuation.values);assert_eq!(cold.state.random,continuation.state.random);assert_eq!(cold.state.steps,continuation.state.steps);visits+=1;
        }
        assert!(visits>0);eprintln!("portable phase visits: {text} = {visits}");let mut rng=RandomState::seeded(4);assert_eq!(continuation.result,Some(library.eval_with_random(&expr,&globals,&mut rng).unwrap()));assert_eq!(continuation.state.random,rng);
    }
}
