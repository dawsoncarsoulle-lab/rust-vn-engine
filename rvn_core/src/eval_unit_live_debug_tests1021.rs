//! Semantics of the real continuation versus the existing synchronous executor.
use super::*;

fn library(source:&str)->FunctionLibrary{
    FunctionLibrary::from_script(&rvn_parser::parse(source).unwrap()).unwrap()
}
fn expr(source:&str)->Expr{
    let statements=rvn_parser::parse(&format!("set scratch={source}")).unwrap();
    let Statement::SetVar{value,..}=&statements[0]else{panic!("expression fixture")};value.clone()
}
fn complete(continuation:&mut Continuation){
    for _ in 0..200_000{if continuation.done(){return;}if continuation.step().is_err(){return;}}
    panic!("Unit continuation exceeded its shared bounded calculation")
}

#[test]
fn unit_live_eval1021_nested_lexical_masks_typed_returns_shared_work_and_replay_match_executor(){
    let library=library("function value(n){local private=n+1 return private}\nfunction inner(`quoted parameter`)->(){set result=value(`quoted parameter`)+random(1,9) set masked=17 return set forbidden=1}\nfunction outer(masked)->(){local own=99 for item in [2,3] {call inner(item)} set result=result+masked flow.sequence() {{return} {set forbidden=2}}}\nfunction empty()->(){}\n");
    let globals=HashMap::from([("result".into(),Value::Int(0)),("masked".into(),Value::Int(0))]);
    let environment=HashMap::from([("caller".into(),Value::Int(4))]);
    for (name,arguments,expected_exit) in [("outer",vec![expr("caller")],BlockExit::Unit),("empty",vec![],BlockExit::Continue)]{
        let mut normal=Computation::new(&library,&globals);normal.random=RandomState::seeded(775);
        normal.invoke_unit_expressions(name,&arguments,&environment).unwrap();
        let probe=Probe::shared();let mut debug=library.clone();debug.debug_probe=Some(probe.clone());
        let mut random=RandomState::seeded(775);
        assert!(debug.invoke_unit_scoped_with_locale(name,&arguments,&environment,&globals,&mut random,None).is_err());
        assert_eq!(random,RandomState::seeded(775));
        let mut pending=Probe::pending(&probe,&library).unwrap().unwrap();complete(&mut pending);
        assert_eq!(pending.error,None);assert_eq!(pending.unit_exit,Some(expected_exit));assert_eq!(pending.result,None);
        assert_eq!(pending.state.globals,normal.globals);assert_eq!(pending.state.unit_writes,normal.unit_global_writes);
        assert_eq!(pending.state.random,normal.random);assert_eq!(pending.work(),(normal.steps,normal.value_work));
        assert_eq!(pending.frames.len(),1);assert_eq!(pending.frames[0].variables,environment);
        assert!(!pending.state.globals.contains_key("own"));assert!(!pending.state.globals.contains_key("item"));assert!(!pending.state.globals.contains_key("forbidden"));
        Probe::finish(&probe,&pending).unwrap();Probe::rewind(&probe).unwrap();
        let (actual,written)=debug.invoke_unit_scoped_with_locale(name,&arguments,&environment,&globals,&mut random,None).unwrap();
        assert_eq!(actual,normal.globals);assert_eq!(written,normal.unit_global_writes);assert_eq!(random,normal.random);
        assert!(Probe::complete(&probe));
    }
}

#[test]
fn unit_live_eval1021_shared_depth_steps_value_work_and_failed_arguments_never_publish(){
    for source in [
        "function effect()->(){while true {set result=1}}",
        "function effect()->(){call effect()}",
        "function effect()->(){while true {set result=payload}}",
        "function effect()->(){set result=random(1,9)+absent}",
    ]{
        let library=library(source);
        let mut globals=HashMap::from([("result".into(),Value::Int(0))]);
        if source.contains("payload"){globals.insert("payload".into(),Value::Str("x".repeat(600_000)));}
        let mut normal=Computation::new(&library,&globals);normal.random=RandomState::seeded(42);
        let expected=normal.invoke_unit_expressions("effect",&[],&globals).unwrap_err();
        let probe=Probe::shared();let mut debug=library.clone();debug.debug_probe=Some(probe.clone());
        let mut random=RandomState::seeded(42);
        assert!(debug.invoke_unit_scoped_with_locale("effect",&[],&globals,&globals,&mut random,None).is_err());
        let mut pending=Probe::pending(&probe,&library).unwrap().unwrap();complete(&mut pending);
        assert_eq!(pending.error,Some(expected));assert_eq!(pending.work(),(normal.steps,normal.value_work));
        assert_eq!(pending.state.random,normal.random);assert_eq!(random,RandomState::seeded(42));
        Probe::finish(&probe,&pending).unwrap();Probe::rewind(&probe).unwrap();
        assert!(debug.invoke_unit_scoped_with_locale("effect",&[],&globals,&globals,&mut random,None).is_err());
        assert_eq!(random,RandomState::seeded(42));assert_eq!(globals["result"],Value::Int(0));
    }
    let library=library("function effect()->(){}");let probe=Probe::shared();let mut debug=library.clone();debug.debug_probe=Some(probe.clone());
    let mut random=RandomState::seeded(42);
    assert!(debug.invoke_unit_scoped_with_locale("effect",&[expr("random(1,9)")],&HashMap::new(),&HashMap::new(),&mut random,None).is_err());
    assert_eq!(random,RandomState::seeded(42));assert!(Probe::pending(&probe,&library).unwrap().is_none());
}

#[test]
fn unit_live_eval1021_portable_export_and_foreign_import_refuse_before_candidate_mutation(){
    let legacy=library("function value(){return random(1,9)}");let legacy_probe=Probe::shared();let mut legacy_debug=legacy.clone();legacy_debug.debug_probe=Some(legacy_probe.clone());
    assert!(legacy_debug.eval_with_random(&expr("value()"),&HashMap::new(),&mut RandomState::seeded(42)).is_err());
    let legacy_pending=Probe::pending(&legacy_probe,&legacy).unwrap().unwrap();
    let trace=Probe::portable_trace(&legacy_probe,&legacy_pending).unwrap();
    let trace:PortableTrace=serde_json::from_slice(&serde_json::to_vec(&trace).unwrap()).unwrap();
    let library=library("function effect()->(){set result=random(1,9)}");let probe=Probe::shared();let mut debug=library.clone();debug.debug_probe=Some(probe.clone());
    let globals=HashMap::from([("result".into(),Value::Int(0))]);let mut random=RandomState::seeded(42);
    assert!(debug.invoke_unit_scoped_with_locale("effect",&[],&globals,&globals,&mut random,None).is_err());
    let mut pending=Probe::pending(&probe,&library).unwrap().unwrap();
    assert!(Probe::portable_trace(&probe,&pending).is_err());
    let before=format!("{pending:?}");
    assert!(pending.restore_portable(&trace,None).is_err());assert_eq!(format!("{pending:?}"),before);
    pending.step().unwrap();assert!(Probe::portable_trace(&probe,&pending).is_err());
    assert_eq!(random,RandomState::seeded(42));assert_eq!(globals["result"],Value::Int(0));
}


#[test]
fn unit_live_eval1021_heap_depth64_and_refused65_keep_exact_debugger_work_and_candidate_order() {
    let library = library("function recur(n)->(){local own=n flow.for_loop(index,1,3){if index==2{flow.break()} set result=random(1,9)} if n>0{call recur(n-1)}else{return} set result=result+own}");
    let globals = HashMap::from([("result".into(), Value::Int(0)), ("index".into(), Value::Int(99))]);
    for depth in [63, 64] {
        let arguments = [Expr::Int(depth)];
        let mut normal = Computation::new(&library, &globals);
        normal.random = RandomState::seeded(775);
        let outcome = normal.invoke_unit_expressions("recur", &arguments, &globals);
        assert_eq!(outcome.is_ok(), depth == 63);
        let probe = Probe::shared(); let mut debug = library.clone(); debug.debug_probe = Some(probe.clone());
        let seed = RandomState::seeded(775); let mut random = seed;
        assert!(debug.invoke_unit_scoped_with_locale("recur", &arguments, &globals, &globals, &mut random, None).is_err());
        let mut pending = Probe::pending(&probe, &library).unwrap().unwrap();
        complete(&mut pending);
        assert_eq!(pending.error, outcome.err());
        assert_eq!(pending.work(), (normal.steps, normal.value_work));
        assert_eq!(pending.state.random, normal.random);
        assert_eq!(pending.state.globals, normal.globals);
        assert_eq!(pending.state.unit_writes, normal.unit_global_writes);
        assert_eq!(random, seed);
        if depth == 63 {
            assert_eq!(pending.frames.len(), 1);
            assert_eq!(normal.globals["index"], Value::Int(99));
            assert!(!normal.globals.contains_key("own"));
            Probe::finish(&probe, &pending).unwrap(); Probe::rewind(&probe).unwrap();
            let (actual, written) = debug.invoke_unit_scoped_with_locale("recur", &arguments, &globals, &globals, &mut random, None).unwrap();
            assert_eq!(actual, normal.globals); assert_eq!(written, normal.unit_global_writes);
            assert_eq!(random, normal.random);
        }
    }
}
