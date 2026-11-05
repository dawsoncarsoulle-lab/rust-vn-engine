use super::*;
fn library(source:&str)->FunctionLibrary{FunctionLibrary::from_script(&rvn_parser::parse(source).unwrap()).unwrap()}
fn globals()->HashMap<String,Value>{[("score",0),("calls",0),("shared",0),("result",0),("shadow",100),("x",100)].into_iter().map(|(k,v)|(k.into(),Value::Int(v))).collect()}
fn arguments(source:&str)->Vec<Expr>{let mut s=rvn_parser::parse(source).unwrap();let Statement::UnitCall{arguments,..}=s.remove(0)else{panic!()};arguments}
#[test]
fn unit_core1003_empty_fallthrough_and_early_return_are_real_control_not_values(){
    let l=library("function empty()->(){} function stop()->(){set score=1 if true{return} set score=99}");let initial=globals();let mut random=RandomState::default();
    assert_eq!(l.invoke_unit_with_random("empty",&[],&initial,&mut random).unwrap(),initial);
    let output=l.invoke_unit_with_random("stop",&[],&initial,&mut random).unwrap();assert_eq!(output["score"],Value::Int(1));assert_eq!(initial["score"],Value::Int(0));
    assert!(!l.has_function("stop",0));assert!(l.has_unit_function("stop",0));assert!(l.eval(&Expr::Call{name:"stop".into(),args:vec![]},&initial).is_err());
}
#[test]
fn unit_core1003_nested_real_calls_share_candidate_globals_but_not_locals_or_pure_writes(){
    let l=library("function pure(x){set shared=900 return x*2} function inner(x)->(){local shadow=999 set shared=shared+x} function outer(x)->(){local shadow=7 set shared=1 call inner(x) set result=pure(shared)+shadow}");let initial=globals();let mut random=RandomState::default();
    let out=l.invoke_unit_with_random("outer",&[Expr::Int(3)],&initial,&mut random).unwrap();
    assert_eq!(out["shared"],Value::Int(4));assert_eq!(out["result"],Value::Int(15));assert_eq!(out["shadow"],Value::Int(100));assert_eq!(out["x"],Value::Int(100));assert_eq!(initial["shared"],Value::Int(0));
    let second=l.invoke_unit_with_random("outer",&[Expr::Int(1)],&out,&mut random).unwrap();assert_eq!(second["result"],Value::Int(11));
}
#[test]
fn unit_core1003_unit_return_propagates_every_existing_structured_block_and_range_owner(){
    for body in ["if true {set score=1 return}","while true {set score=1 return}","for item in [1,2] {set score=1 return}","flow.for_loop(i,0,2){set score=1 return}","flow.sequence(){ {set score=1 return} {set score=99} }"]{
        let l=library(&format!("function stop()->(){{{body} set score=88}}"));let mut random=RandomState::default();let out=l.invoke_unit_with_random("stop",&[],&globals(),&mut random).unwrap();assert_eq!(out["score"],Value::Int(1),"{body}");assert!(!out.contains_key("item"));assert!(!out.contains_key("i"));
    }
}
#[test]
fn unit_core1003_arguments_and_nested_effects_commit_rng_only_on_whole_success(){
    let l=library("function accept(n)->(){set score=n} function fail(n)->(){set score=n set result=1/0}");let initial=globals();let args=arguments("call accept(random(0,10))");let mut expected=RandomState::default();let value=l.eval_with_random(&args[0],&initial,&mut expected).unwrap();let mut random=RandomState::default();
    let out=l.invoke_unit_with_random("accept",&args,&initial,&mut random).unwrap();assert_eq!(out["score"],value);assert_eq!(random,expected);
    let before=random;assert!(l.invoke_unit_with_random("fail",&args,&initial,&mut random).is_err());assert_eq!(random,before);assert_eq!(initial,globals());
}
#[test]
fn unit_core1003_missing_or_wrong_mode_and_arity_refuse_before_random_arguments(){
    let l=library("function value(n){return n} function unit()->(){}");let args=arguments("call unknown(random(0,10))");let mut random=RandomState::default();let before=random;
    for name in ["missing","value","unit"]{assert!(l.invoke_unit_with_random(name,&args,&globals(),&mut random).is_err());assert_eq!(random,before);}
    assert!(l.eval_with_random(&Expr::Call{name:"unit".into(),args},&globals(),&mut random).is_err());assert_eq!(random,before);
}
#[test]
fn unit_core1003_unit_and_value_calls_share_depth_and_work_budgets_neutrally(){
    let l=library("function value(n){if n==0{return 1} return value(n-1)} function unit(n)->(){if n>0{call unit(n-1)}else{set score=value(40)}}");let initial=globals();let mut random=RandomState::default();let before=random;
    assert!(l.invoke_unit_with_random("unit",&[Expr::Int(40)],&initial,&mut random).is_err());assert_eq!(random,before);assert_eq!(initial,globals());
    let looped=library("function unit()->(){while true{set score=score+1}}");assert!(looped.invoke_unit_with_random("unit",&[],&initial,&mut random).is_err());assert_eq!(random,before);assert_eq!(initial,globals());
    let nested=library("function inner()->(){} function outer(n)->(){call inner()}");
    // Use the actual shared value-work units: nodes + UTF-8 bytes / 32.
    // Every individual value is valid; two full copies fit, a third does not.
    let mut large=globals();
    let names=(0..11).map(|i|format!("blob_{i}")).collect::<Vec<_>>();
    for name in &names{large.insert(name.clone(),Value::Str("x".repeat(crate::value_limits::MAX_VALUE_BYTES)));}
    let copy_work=large.values().map(crate::value_limits::value_work).collect::<Result<Vec<_>,_>>().unwrap().into_iter().sum::<usize>();
    assert!(2*copy_work<1_000_000 && 3*copy_work>1_000_000);
    let unchanged=large.clone();let args=arguments("call outer(random(0,10))");
    match nested.invoke_unit_with_random("outer",&args,&large,&mut random) {
        Err(EvalError::ExecutionLimit{limit})=>assert_eq!(limit,"1 000 000 copies ou caractères de valeurs par calcul"),
        Err(other)=>panic!("unexpected budget error: {other:?}"),
        Ok(_)=>panic!("the actual third unmasked global copy must exceed shared value-work"),
    }
    assert_eq!(random,before);assert!(large==unchanged);
    // Outer and inner copy the globals once each. All large bindings are
    // caller parameters here, so returning must neither clone nor charge them.
    let masked=library(&format!("function inner()->(){{}} function outer({})->(){{call inner()}}",names.join(",")));
    let masked_args=names.iter().map(|_|Expr::Int(0)).collect::<Vec<_>>();
    assert!(masked.invoke_unit_with_random("outer",&masked_args,&large,&mut random).unwrap()==unchanged);
    assert_eq!(random,before);
}
#[test]
fn unit_core1003_old_no_return_pure_lexical_and_shortcircuit_failures_remain(){
    let l=library("function no_return(){set score=9} function value(n){return n*2} function bad(){return 1/0} function noop()->(){}");let initial=globals();let mut random=RandomState::default();
    assert!(l.eval(&Expr::Call{name:"no_return".into(),args:vec![]},&initial).is_err());assert_eq!(l.eval(&Expr::Call{name:"value".into(),args:vec![Expr::Int(3)]},&initial).unwrap(),Value::Int(6));
    let source=rvn_parser::parse("set result=false and bad()").unwrap();let out=l.execute_with_random(&source,&initial,&mut random).unwrap();assert_eq!(out["result"],Value::Bool(false));assert_eq!(initial["score"],Value::Int(0));
}
#[test]
fn unit_core1003_forged_value_return_renderer_latent_and_call_modes_refuse_admission(){
    for body in [vec![Statement::FunctionReturn{value:Expr::Int(1)}],vec![Statement::Return],vec![Statement::UiClose{name:Expr::Str("screen".into())}],vec![Statement::UnitCall{name:"value".into(),arguments:vec![]}],vec![Statement::SetVar{name:"persistent.flag".into(),value:Expr::Bool(true)}]]{
        let source=vec![Statement::Function{name:"value".into(),parameters:vec![],body:vec![Statement::FunctionReturn{value:Expr::Int(0)}]},Statement::UnitFunction{name:"unit".into(),parameters:vec![],body}];assert!(FunctionLibrary::from_script(&source).is_err());
    }
    // A caller-provided read snapshot is data, never PersistentStore authority.
    let read=library("function read()->(){set result=persistent.flag}");let mut snapshot=globals();snapshot.insert("persistent.flag".into(),Value::Bool(true));let before=snapshot.clone();let mut random=RandomState::default();
    let out=read.invoke_unit_with_random("read",&[],&snapshot,&mut random).unwrap();assert_eq!(out["result"],Value::Bool(true));assert_eq!(out["persistent.flag"],Value::Bool(true));assert_eq!(snapshot,before);
}
#[test]
fn unit_core1003_debugger_and_engine_entry_refuse_without_fabricated_source_owner(){
    let mut l=library("function unit()->(){set score=7}");l.debug_probe=Some(debugger_eval::Probe::shared());let initial=globals();let mut random=RandomState::default();let before=random;
    assert!(l.invoke_unit_with_random("unit",&[],&initial,&mut random).is_err());assert_eq!(random,before);assert_eq!(initial,globals());
    let mut source=rvn_parser::parse("init {set score=1} function unit()->(){set score=7} label start").unwrap();source.push(Statement::Handler{name:"wrong".into(),parameters:vec!["event".into()],body:vec![Statement::UnitCall{name:"unit".into(),arguments:vec![]}]});assert!(crate::Engine::new(source,crate::TerminalRenderer,4).is_err());
}


#[test]
fn unit_core1021_heap_frames_admit64_and_refuse65_before_argument_rng_publication() {
    let library = library("function recur(n)->(){local own=n set calls=calls+1 if n>0{call recur(n-1)}else{set result=random(1,9)} set result=result+own}");
    let initial = globals();
    let seed = RandomState::seeded(775);
    let mut random = seed;
    let output = library.invoke_unit_with_random("recur", &[Expr::Int(63)], &initial, &mut random).unwrap();
    assert_eq!(output["calls"], Value::Int(64));
    let mut expected_random = seed;
    let leaf = library.eval_with_random(&arguments("call recur(random(1,9))")[0], &initial, &mut expected_random).unwrap();
    let Value::Int(leaf) = leaf else { panic!("integer RNG"); };
    assert_eq!(output["result"], Value::Int(leaf + (0..64).sum::<i64>()));
    assert_eq!(random, expected_random);
    assert!(!output.contains_key("own"));
    let mut refused_random = seed;
    let error = library.invoke_unit_with_random("recur", &arguments("call recur(random(64,64))"), &initial, &mut refused_random).unwrap_err();
    assert_eq!(error, EvalError::ExecutionLimit { limit: "64 appels de fonction imbriqués" });
    assert_eq!(refused_random, seed);
    assert_eq!(initial, globals());
}

#[test]
fn unit_core1021_heap_frames_resume_nested_controls_and_their_own_range_after_child_return() {
    let library = library("function recur(n)->(){local own=n flow.for_loop(index,1,3){for item in [1,2]{while true{flow.sequence(){{if index==2{flow.break()}} {set calls=calls+1 return}}}}} set forbidden=1}\nfunction outer(n)->(){flow.for_loop(index,1,3){if index==2{call stop(n) flow.break()} call recur(n)} set result=index}\nfunction stop(n)->(){if n>0{call stop(n-1)}else{return} set score=score+1}");
    let mut initial = globals(); initial.insert("index".into(), Value::Int(100));
    let mut random = RandomState::seeded(42);
    let output = library.invoke_unit_with_random("outer", &[Expr::Int(62)], &initial, &mut random).unwrap();
    assert_eq!(output["calls"], Value::Int(1));
    assert_eq!(output["score"], Value::Int(62));
    assert_eq!(output["result"], Value::Int(2));
    assert_eq!(output["index"], Value::Int(100));
    assert!(!output.contains_key("item"));
    assert!(!output.contains_key("own"));
    assert!(!output.contains_key("forbidden"));
    assert_eq!(random, RandomState::seeded(42));
}

#[test]
fn unit_core1021_heap_frames_break_skips_enclosing_while_and_sequence_only_to_nearest_range() {
    let library = library("function inner()->(){flow.for_loop(index,1,3){if index==2{flow.break()} set calls=calls+1}}\nfunction outer()->(){flow.for_loop(index,1,3){call inner() for item in [1,2]{local run=1 while run>0{flow.sequence(){{if index==2{flow.break()}} {set score=score+1}} set run=0}} set result=result+1} set shared=index}");
    let mut initial = globals(); initial.insert("index".into(), Value::Int(100));
    let mut random = RandomState::seeded(42);
    let output = library.invoke_unit_with_random("outer", &[], &initial, &mut random).unwrap();
    assert_eq!(output["calls"], Value::Int(2));
    assert_eq!(output["score"], Value::Int(2));
    assert_eq!(output["result"], Value::Int(1));
    assert_eq!(output["shared"], Value::Int(2));
    assert_eq!(output["index"], Value::Int(100));
    assert!(!output.contains_key("item"));
}

#[test]
fn unit_core1021_heap_frames_restore_caller_scope_on_success_and_nested_failure() {
    let library = library("function child()->(){local own=1 flow.for_loop(index,1,2){set score=score+1 return}}\nfunction outer()->(){call child()}\nfunction fail()->(){call child() set result=random(1,9)+absent}");
    let initial = globals();
    for name in ["outer", "fail"] {
        let mut computation = Computation::new(&library, &initial);
        computation.calls = 3;
        computation.unit_locals = Some(["caller".into()].into_iter().collect());
        computation.range_break = true;
        computation.range_break_owners.push(ComputationRangeBreak::Native { requested: true });
        let result = computation.invoke_unit_expressions(name, &[], &initial);
        assert_eq!(result.is_ok(), name == "outer");
        assert_eq!(computation.calls, 3);
        let caller_locals: std::collections::HashSet<String> = ["caller".to_owned()].into_iter().collect();
        assert_eq!(computation.unit_locals.as_ref().unwrap(), &caller_locals);
        assert!(computation.range_break);
        assert!(matches!(computation.range_break_owners.as_slice(), [ComputationRangeBreak::Native { requested: true }]));
        assert!(computation.active_consumer.is_none());
        assert_eq!(computation.consumer_frame, crate::consumer_schedule::ConsumerFrame::default());
        assert_eq!(computation.consumer_storage, crate::consumer_schedule::FrameStorage::default());
    }
}
