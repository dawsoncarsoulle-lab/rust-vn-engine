//! Real public FunctionLibrary admission followed by its actual pending Probe.
//! Host-resident Handler/Screen and full Engine Save15 are separate gates.
use super::*;
const SCOPE:&str="g770/n1:function_entry|exec_out=1@0";
const RANGE:&str="n2:for_loop_with_break|exec_in=2@1,first=3@2,last=4@3,break=5@0,body=6@0,index=7@0,completed=8@0";
const INDEX:&str="rvn_native_index_g770_n2";
const FIRST:&str="n4:function_call|result=9@0";
const LAST:&str="n5:function_call|result=10@0";
fn range_source(body:&str)->String{
    format!(r#"function first(){{return random(0,0)}}
function last(){{local draw=random(1,1) return 2}}
function probe(){{local trace="" local seen=-1
flow.consume_v3("{SCOPE}","{RANGE}"){{
capture "{FIRST}"=first()
capture "{LAST}"=last()
flow.for_loop({INDEX},__rvn_captured_v1("{FIRST}"),__rvn_captured_v1("{LAST}")){{{body}}}
}}
return [trace,seen]
}}"#)
}
fn expression()->Expr{
    let Statement::SetVar{value,..}=rvn_parser::parse("set result=probe()").unwrap().remove(0)else{unreachable!()};value
}
fn begin(source:&str)->(FunctionLibrary,SharedProbe,Continuation){
    let library=FunctionLibrary::from_script(&rvn_parser::parse(source).unwrap()).unwrap();
    let probe=Probe::shared();let mut debug=library.clone();debug.debug_probe=Some(probe.clone());
    let mut random=RandomState::seeded(770);let before=random;
    assert!(debug.eval_with_random(&expression(),&HashMap::new(),&mut random).is_err());assert_eq!(random,before);
    let actual=Probe::pending(&probe,&library).unwrap().unwrap();(library,probe,actual)
}
fn finish(c:&mut Continuation){for _ in 0..20_000{if c.done(){return;}c.step_with_budget(1).unwrap();}panic!("finite fixture did not finish");}
fn assert_no_projection(c:&Continuation){for f in &c.frames{assert!(!f.variables.contains_key(INDEX));}assert!(!c.state.globals.contains_key(INDEX));}
#[test]
fn lexical0770_first_once_test_per_visit_and_real_random_replay(){
    let source=range_source(&format!("set trace=trace+\"B\" set seen={INDEX}"));
    let(library,probe,mut c)=begin(&source);finish(&mut c);
    assert_eq!(c.result,Some(Value::List(vec![Value::Str("BBB".into()),Value::Int(2)])));
    let mut expected=RandomState::seeded(770);expected.integer(0,0).unwrap();for _ in 0..4{expected.integer(1,1).unwrap();}
    assert_eq!(c.state.random,expected);assert_no_projection(&c);
    Probe::finish(&probe,&c).unwrap();Probe::rewind(&probe).unwrap();let mut replay=library.clone();replay.debug_probe=Some(probe);
    let mut random=RandomState::seeded(770);assert_eq!(replay.eval_with_random(&expression(),&HashMap::new(),&mut random).unwrap(),c.result.unwrap());assert_eq!(random,expected);
}
#[test]
fn lexical0770_break_keeps_sequence_tail_and_skips_extra_last(){
    for body in ["flow.sequence(){{flow.break()}{set trace=trace+\"T\"}}","flow.sequence(){{set trace=trace+\"B\" flow.break()}{set trace=trace+\"T\"}}"]{
        let source=range_source(body);let(_,_,mut c)=begin(&source);finish(&mut c);
        let expected_trace=if body.contains("+\"B\""){"BT"}else{"T"};assert_eq!(c.result,Some(Value::List(vec![Value::Str(expected_trace.into()),Value::Int(-1)])));
        let mut rng=RandomState::seeded(770);rng.integer(0,0).unwrap();rng.integer(1,1).unwrap();assert_eq!(c.state.random,rng);assert_no_projection(&c);
    }
}
#[test]
fn lexical0770_portable_first_and_test_keep_captured_slice_and_rng(){
    let source=range_source("set trace=trace+\"B\"");
    for phase in [LoopPhase::First,LoopPhase::Test]{
        let(_,probe,mut c)=begin(&source);let mut stopped=false;
        for _ in 0..20_000{
            if c.tasks.iter().any(|t|matches!(t,Task::NativeLoopResume{state,..}if state.phase==phase))&&c.frames.len()>2{stopped=true;break;}
            c.step_with_budget(1).unwrap();
        }
        assert!(stopped,"must stop inside real requested helper in {phase:?}");
        let trace=Probe::portable_trace(&probe,&c).unwrap();let json=serde_json::to_string(&trace).unwrap();assert!(!json.contains("ScheduledConsumer"));
        let trace:PortableTrace=serde_json::from_str(&json).unwrap();let(_,_,mut cold)=begin(&source);cold.restore_portable(&trace,None).unwrap();
        finish(&mut c);finish(&mut cold);assert_eq!(cold.result,c.result);assert_eq!(cold.state.random,c.state.random);assert_eq!(cold.work(),c.work());assert_no_projection(&cold);
    }
}
#[test]
fn lexical0770_cold_latched_tail_and_cursor_phase_projection_forgery_refusal(){
    let source=range_source("flow.sequence(){{flow.break()}{set trace=trace+\"T\"}}");let(_,probe,mut c)=begin(&source);let mut stopped=false;
    for _ in 0..20_000{if c.tasks.iter().any(|t|matches!(t,Task::NativeLoopResume{state,..}if state.phase==LoopPhase::Body&&state.break_requested)){stopped=true;break;}c.step_with_budget(1).unwrap();}
    assert!(stopped);let trace=Probe::portable_trace(&probe,&c).unwrap();let encoded=serde_json::to_value(&trace).unwrap();
    let owner=encoded["pending"]["tasks"].as_array().unwrap().iter().position(|t|t["kind"]=="native_loop_resume").unwrap();
    for field in ["phase","cursor_index","projection","frame","route","latch","break_site"]{
        let mut bad=encoded.clone();match field{
            "phase"=>bad["pending"]["tasks"][owner]["data"]["phase"]=serde_json::json!("test"),
            "cursor_index"=>bad["pending"]["tasks"][owner]["data"]["cursor"]["index"]=serde_json::json!(10),
            "projection"=>{let f=bad["pending"]["tasks"][owner]["ctx"]["frame"].as_u64().unwrap() as usize;bad["pending"]["frames"][f]["variables"][INDEX]=serde_json::json!({"Int":10});},
            "frame"=>bad["pending"]["tasks"][owner]["ctx"]["frame"]=serde_json::json!(99),
            "route"=>bad["pending"]["tasks"][owner]["ctx"]["route"]=serde_json::json!([]),
            "latch"=>bad["pending"]["tasks"][owner]["data"]["break_requested"]=serde_json::json!(false),
            "break_site"=>bad["pending"]["tasks"][owner]["data"]["break_site"]["frame"]=serde_json::json!(99),_=>unreachable!(),
        }
        let bad:PortableTrace=serde_json::from_value(bad).unwrap();let(_,_,mut rejected)=begin(&source);let before=format!("{rejected:?}");assert!(rejected.restore_portable(&bad,None).is_err(),"accepted {field}");assert_eq!(format!("{rejected:?}"),before);
    }
    let(_,_,mut cold)=begin(&source);cold.restore_portable(&trace,None).unwrap();finish(&mut cold);finish(&mut c);assert_eq!(cold.result,c.result);assert_eq!(cold.state.random,c.state.random);assert_no_projection(&cold);
}
#[test]
fn lexical0770_return_and_error_cleanup_private_index_without_publishing_rng(){
    let source=range_source("flow.sequence(){{return [\"R\",0]}{set trace=trace+\"forbidden\"}}");let(_,_,mut c)=begin(&source);finish(&mut c);assert_eq!(c.result,Some(Value::List(vec![Value::Str("R".into()),Value::Int(0)])));assert_no_projection(&c);
    let source=range_source("flow.sequence(){{flow.break()}{set seen=1/0}}");let(library,_,mut c)=begin(&source);
    let mut failed=false;for _ in 0..20_000{match c.step_with_budget(1){Ok(())=>{},Err(EvalError::DivisionByZero)=>{failed=true;break;},Err(e)=>panic!("unexpected error {e:?}")}}
    assert!(failed);assert_no_projection(&c);
    let mut rng=RandomState::seeded(770);let mut frame=crate::consumer_schedule::ConsumerFrame::default();let before=serde_json::to_value(&frame).unwrap();
    assert!(library.execute_in_consumer_frame(&rvn_parser::parse("set answer=probe()").unwrap(),&HashMap::new(),&mut rng,&mut frame).is_err());assert_eq!(rng,RandomState::seeded(770));assert_eq!(serde_json::to_value(&frame).unwrap(),before);
}

#[test]
fn lexical0770_array_and_range_requery_mutated_lexical_inputs(){
    let array_scope="g771/n1:function_entry|exec_out=1@0";
    let array="n2:for_each|exec_in=2@1,collection=3@0,body=4@0,completed=5@0,array_index=6@0,array_element=7@0";
    for (initial,replacement,visits) in [("[1,2,3]","[1]",1),("[1]","[1,2,3]",3)]{
        let source=format!("function probe(){{local items={initial} local visits=0 flow.consume_v3(\"{array_scope}\",\"{array}\"){{for rvn_native_index_g771_n2 in items{{set visits=visits+1 if rvn_native_index_g771_n2==0{{set items={replacement}}}}}}} return visits}}");
        let(library,_,mut c)=begin(&source);finish(&mut c);assert_eq!(c.result,Some(Value::Int(visits)));assert_eq!(c.state.random,RandomState::seeded(770));assert!(!c.frames[0].variables.contains_key("rvn_native_index_g771_n2"));
        assert_eq!(library.eval(&expression(),&HashMap::new()).unwrap(),Value::Int(visits));
    }
    let range="n2:for_loop_with_break|exec_in=2@1,first=3@0,last=4@0,break=5@0,body=6@0,index=7@0,completed=8@0";
    for (initial,replacement,visits) in [(2,0,1),(0,2,3)]{
        let source=format!("function probe(){{local bound={initial} local visits=0 flow.consume_v3(\"{SCOPE}\",\"{range}\"){{flow.for_loop({INDEX},0,bound){{set visits=visits+1 if {INDEX}==0{{set bound={replacement}}}}}}} return visits}}");
        let(library,_,mut c)=begin(&source);finish(&mut c);assert_eq!(c.result,Some(Value::Int(visits)));assert_no_projection(&c);assert_eq!(library.eval(&expression(),&HashMap::new()).unwrap(),Value::Int(visits));
    }
}

#[test]
fn lexical0770_legacy_range_masks_native_break_and_old_wire_has_no_loop_extension(){
    let source=range_source("flow.for_loop(old,0,3){flow.sequence(){{flow.break()}{set trace=trace+\"forbidden\"}}} set trace=trace+\"L\"");
    let(_,_,mut c)=begin(&source);finish(&mut c);assert_eq!(c.result,Some(Value::List(vec![Value::Str("LLL".into()),Value::Int(-1)])));assert_no_projection(&c);
    let legacy="function probe(){local x=0 flow.for_loop(i,0,2){set x=x+1} return x}";
    let(_,probe,mut c)=begin(legacy);c.step_with_budget(1).unwrap();let trace=Probe::portable_trace(&probe,&c).unwrap();let raw=serde_json::to_string(&trace).unwrap();
    assert!(!raw.contains("native_loop_resume")&&!raw.contains("native_loop_header")&&!raw.contains("LexicalLoopState"));
    let(_,_,mut cold)=begin(legacy);cold.restore_portable(&trace,None).unwrap();finish(&mut c);finish(&mut cold);assert_eq!(c.result,Some(Value::Int(3)));assert_eq!(cold.result,c.result);assert_eq!(cold.state.random,c.state.random);
}

#[test]
fn lexical0770_exhausted_budget_and_i64_overflow_remove_body_projection(){
    let scope="g770/n1:function_entry|exec_out=1@0";
    let range="n2:for_loop_with_break|exec_in=2@1,first=3@0,last=4@0,break=5@0,body=6@0,index=7@0,completed=8@0";
    let max="9223372036854775807";
    let source=format!("function probe(){{flow.consume_v3(\"{scope}\",\"{range}\"){{flow.for_loop({INDEX},{max},{max}){{}}}} return 0}}");
    let(_,_,mut c)=begin(&source);let mut overflow=false;for _ in 0..20_000{match c.step_with_budget(1){Ok(())=>{},Err(EvalError::NumericOverflow)=>{overflow=true;break;},Err(e)=>panic!("unexpected {e:?}")}}assert!(overflow);assert_no_projection(&c);
    let source=range_source("set seen=1");let(_,_,mut c)=begin(&source);let mut stopped=false;for _ in 0..20_000{if c.tasks.iter().any(|t|matches!(t,Task::NativeLoopResume{state,..}if state.phase==LoopPhase::Body)){stopped=true;break;}c.step_with_budget(1).unwrap();}assert!(stopped);
    // Unit-only control of the existing budget: this is not a forged Probe or
    // admission, and does not stand in for an executed oversized source case.
    c.state.steps=MAX_COMPUTATION_STEPS;
    let mut exhausted=false;for _ in 0..32{match c.step_with_budget(1){Ok(())=>{},Err(EvalError::ExecutionLimit{..})=>{exhausted=true;break;},Err(e)=>panic!("unexpected {e:?}")}}assert!(exhausted);assert_no_projection(&c);
}

#[test]
fn lexical0770_called_native_helper_keeps_its_callers_latched_owner_cold(){
    let helper=range_source("flow.sequence(){{flow.break()}{set trace=trace+\"H\"}}");
    let helper=helper[helper.find("function probe()").unwrap()..].replace("function probe()","function helper()").replace("g770/","g774/").replace("g770_n","g774_n").replace("return [trace,seen]","return trace");
    let source=format!("{helper}\n{}",range_source("flow.sequence(){{flow.break()}{set trace=trace+helper() set trace=trace+\"T\"}}"));
    let(_,probe,mut c)=begin(&source);let mut stopped=false;
    for _ in 0..30_000{
        let caller=c.tasks.iter().any(|t|matches!(t,Task::NativeLoopResume{state,ctx,..}if state.break_requested&&ctx.frame==1));
        let helper=c.tasks.iter().any(|t|matches!(t,Task::NativeLoopResume{state,ctx,..}if ctx.frame==2&&state.phase==LoopPhase::Test));
        if caller&&helper{stopped=true;break;}c.step_with_budget(1).unwrap();
    }
    assert!(stopped);let trace=Probe::portable_trace(&probe,&c).unwrap();let(_,_,mut cold)=begin(&source);cold.restore_portable(&trace,None).unwrap();finish(&mut c);finish(&mut cold);
    assert_eq!(c.result,Some(Value::List(vec![Value::Str("HT".into()),Value::Int(-1)])));assert_eq!(cold.result,c.result);assert_eq!(cold.state.random,c.state.random);assert_no_projection(&cold);
    assert!(cold.frames.iter().all(|f|!f.variables.contains_key("rvn_native_index_g774_n2")));
}

#[test]
fn lexical0770_pure_function_rejects_authored_private_index_assignment_before_body(){
    let source=range_source(&format!("set {INDEX}=42 set trace=\"must not run\""));
    let library=match FunctionLibrary::from_script(&rvn_parser::parse(&source).unwrap()){
        Err(error)=>{assert!(error.to_string().contains("Authored assignment collides with an actual native loop private binding"));return;},
        Ok(library)=>library,
    };
    let probe=Probe::shared();let mut debug=library.clone();debug.debug_probe=Some(probe.clone());let mut random=RandomState::seeded(770);
    assert!(debug.eval_with_random(&expression(),&HashMap::new(),&mut random).is_err());assert_eq!(random,RandomState::seeded(770));
    let mut c=Probe::pending(&probe,&library).unwrap().unwrap();let before=c.state.random;let mut refused=false;
    for _ in 0..32{if let Err(error)=c.step_with_budget(1){assert!(error.to_string().contains("Authored assignment collides with an actual native loop private binding"));refused=true;break;}}
    assert!(refused);assert_eq!(c.state.random,before);assert_no_projection(&c);
}

#[test]
fn lexical0770_nested_same_frame_sequence_matches_measured_order_and_cold_latch(){
    let inner="n3:for_loop_with_break|exec_in=12@4,first=13@0,last=14@0,break=15@0,body=16@0,index=17@0,completed=18@0";
    let body=format!("flow.sequence(){{{{set trace=trace+\"O\" flow.consume_v3(\"{SCOPE}\",\"{inner}\"){{flow.for_loop(rvn_native_index_g770_n3,0,inner_last()){{flow.sequence(){{{{set trace=trace+\"I\" flow.break()}}{{set trace=trace+\"T\" set seen=rvn_native_index_g770_n3}}}}}}}} set trace=trace+\"CA\"}}{{set trace=trace+\"X\"}}}}");
    let source=range_source(&body).replace("return 2}","return 1}").replace("return [trace,seen]","set trace=trace+\"C\" return [trace,seen]");
    let source=format!("function inner_last(){{local draw=random(1,1) return 2}}\n{source}");
    let(_,probe,mut c)=begin(&source);let mut stopped=false;
    for _ in 0..30_000{let owners=c.tasks.iter().filter(|t|matches!(t,Task::NativeLoopResume{ctx,..}if ctx.frame==1)).count();let inner=c.tasks.iter().any(|t|matches!(t,Task::NativeLoopResume{statement,state,..}if state.break_requested&&matches!(statement,Statement::ScheduledConsumer{consumer,..}if consumer.starts_with("n3:"))));if owners==2&&inner{stopped=true;break;}c.step_with_budget(1).unwrap();}
    assert!(stopped);let trace=Probe::portable_trace(&probe,&c).unwrap();let(_,_,mut cold)=begin(&source);cold.restore_portable(&trace,None).unwrap();finish(&mut c);finish(&mut cold);
    assert_eq!(c.result,Some(Value::List(vec![Value::Str("OITCAXOITCAXC".into()),Value::Int(0)])));assert_eq!(cold.result,c.result);assert_eq!(cold.state.random,c.state.random);
    let mut expected=RandomState::seeded(770);expected.integer(0,0).unwrap();for _ in 0..5{expected.integer(1,1).unwrap();}assert_eq!(c.state.random,expected);assert_no_projection(&cold);assert!(cold.frames.iter().all(|f|!f.variables.contains_key("rvn_native_index_g770_n3")));
}
