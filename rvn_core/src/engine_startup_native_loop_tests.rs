//! CPU source/owner regressions. Prepared only: no UE/native/pixel acceptance.
use super::*;

fn native0757_source(first:&str,body:&str)->String{
    range(LAST,first,if first=="9223372036854775807"{first}else{"last()"},body)
        .replace("set bound=2","set bound=2 set tails=0 set seen=-1 set trace=\"\"")
        .replace("set done=true","set trace=trace+\"C\" set done=true")
}
fn native0757_tail_source()->String{
    native0757_source("0",&format!("flow.sequence(){{{{flow.break()}}{{set trace=trace+\"T\" set tails=tails+1 set seen={INDEX}}}}}"))
}
fn native0757_stop_tail()->(String,EngineStartup<Host>){
    let raw=native0757_tail_source();let mut s=startup(&raw);
    let current=s.engine.startup.as_ref().unwrap();let pc=current.current().unwrap().pc;
    let Statement::Init{body}=&s.engine.script[pc]else{unreachable!()};
    let ordinal=body.iter().position(|node|matches!(node,Statement::ScheduledConsumer{version:3,..})).unwrap();
    let route=vec![wire::Route::Statement(ordinal),wire::Route::Body,wire::Route::Statement(0),wire::Route::Body,wire::Route::Statement(0),wire::Route::Branch(1),wire::Route::Statement(0)];
    let bp=s.site_page(0).unwrap().entries.into_iter().find(|entry|entry.breakpoint.site.as_ref().is_some_and(|site|site.route==route)).expect("actual pending Sequence tail site").breakpoint;
    s.configure(vec![bp],vec![]).unwrap();
    for _ in 0..128{
        if s.snapshot().site.as_ref().is_some_and(|site|site.route==route)&&s.engine.startup.as_ref().is_some_and(|owner|owner.loops.last().is_some_and(|frame|frame.native_break.is_some())){return(raw,s);}
        s.step(StepKind::Continue).unwrap();
    }panic!("actual post-Break Sequence tail was not reached");
}

#[test]
fn native0757_break_preserves_sequence_tail_and_entered_max_index_without_next_bound(){
    for first in ["0","9223372036854775807"]{
        let raw=native0757_source(first,&format!("flow.sequence(){{{{flow.break()}}{{set trace=trace+\"T\" set tails=tails+1 set seen={INDEX}}}}}"));
        let mut s=startup(&raw);let expected=draws(s.state().random,usize::from(first=="0"));finish(&mut s);
        assert_eq!(s.state().vars["trace"],Value::Str("TC".into()));assert_eq!(s.state().vars["tails"],Value::Int(1));
        assert_eq!(s.state().vars["seen"],Value::Int(first.parse().unwrap()));assert_eq!(s.state().random,expected);assert_no_projections(&s);
    }
}
#[test]
fn native0757_body_break_tail_completed_keep_the_measured_order(){
    let raw=native0757_source("0","flow.sequence(){{set visits=visits+1 set trace=trace+\"B\" flow.break()}{set tails=tails+1 set trace=trace+\"T\"}}");
    let mut s=startup(&raw);let expected=draws(s.state().random,1);finish(&mut s);
    assert_eq!(s.state().vars["trace"],Value::Str("BTC".into()));assert_eq!(s.state().vars["visits"],Value::Int(1));assert_eq!(s.state().vars["tails"],Value::Int(1));assert_eq!(s.state().random,expected);
}
#[test]
fn native0757_nested_inner_break_preserves_outer_body_tail_and_continuation(){
    let inner="n3:for_loop_with_break|exec_in=12@2,first=13@0,last=14@0,break=15@0,body=16@0,index=17@0,completed=18@0";
    let defs="function outer_last(){local token=random(1,1) return 1} function inner_last(){local token=random(1,1) return 2}";
    let body=format!("flow.sequence(){{{{set visits=visits+1 set trace=trace+\"O\" flow.consume_v3(\"{SCOPE}\",\"{inner}\"){{flow.for_loop(rvn_native_index_g704_n3,0,inner_last()){{flow.sequence(){{{{set trace=trace+\"I\" flow.break()}}{{set trace=trace+\"T\" set tails=tails+1 set seen=rvn_native_index_g704_n3}}}}}}}} set trace=trace+\"CA\"}}{{set trace=trace+\"X\"}}}}");
    let raw=range(defs,"0","outer_last()",&body).replace("set bound=2","set bound=2 set tails=0 set seen=-1 set trace=\"\"").replace("set done=true","set trace=trace+\"F\" set done=true");
    let mut s=startup(&raw);let expected=draws(s.state().random,5);finish(&mut s);
    assert_eq!(s.state().vars["trace"],Value::Str("OITCAXOITCAXF".into()));assert_eq!(s.state().vars["visits"],Value::Int(2));assert_eq!(s.state().vars["tails"],Value::Int(2));assert_eq!(s.state().vars["seen"],Value::Int(0));assert_eq!(s.state().random,expected);assert_no_projections(&s);
}
#[test]
fn native0757_repeated_break_inputs_do_not_discard_later_sequence_branches(){
    let raw=native0757_source("0","flow.sequence(){{flow.break()}{flow.break()}{set trace=trace+\"T\" set tails=tails+1}}");
    let mut s=startup(&raw);let expected=draws(s.state().random,1);finish(&mut s);
    assert_eq!(s.state().vars["trace"],Value::Str("TC".into()));assert_eq!(s.state().vars["tails"],Value::Int(1));assert_eq!(s.state().random,expected);
}
#[test]
fn native0757_cold_pending_sequence_tail_keeps_break_site_without_rng_replay(){
    let(raw,mut live)=native0757_stop_tail();let random=live.state().random;
    let save=live.capture_save15(7,"Native post-Break tail".into()).unwrap();let bytes=save.encode().unwrap();
    let mut cold=startup(&raw);cold.restore_save15(crate::save::consumer_checkpoint::Save15::decode(&bytes).unwrap()).unwrap();
    assert_eq!(cold.state().random,random);
    let live_cursor=live.engine.portable_startup().unwrap();let cold_cursor=cold.engine.portable_startup().unwrap();
    let live_frame=live_cursor.loops.last().unwrap();let cold_frame=cold_cursor.loops.last().unwrap();
    assert!(live_frame.native_break.is_some()&&cold_frame.native_break.is_some());
    assert_eq!(cold_cursor.nested,live_cursor.nested);assert_eq!(cold_frame.header,live_frame.header);
    assert_eq!(cold_frame.visits,live_frame.visits);assert_eq!(cold_frame.state,live_frame.state);
    assert_eq!(cold_frame.native_break.as_ref().unwrap().route,live_frame.native_break.as_ref().unwrap().route);
    assert_eq!(cold_frame.native_break.as_ref().unwrap().visits,live_frame.native_break.as_ref().unwrap().visits);
    assert_ne!(cold_frame.header_origin.source,live_frame.header_origin.source,"cold fixture must actually relocate its source");
    assert_ne!(cold_frame.native_break.as_ref().unwrap().origin.source,live_frame.native_break.as_ref().unwrap().origin.source);
    // Live paths/namespaces must rebind; compare their actual portable exports,
    // whose typed origin/namespace mapping uses each owner's frozen program.
    let cold_save=cold.capture_save15(7,"Native post-Break tail".into()).unwrap();
    let cold_export=cold_save.debugger.as_ref().unwrap().startup.as_ref().unwrap();
    let live_export=save.debugger.as_ref().unwrap().startup.as_ref().unwrap();
    assert_eq!(serde_json::to_value(cold_export).unwrap(),serde_json::to_value(live_export).unwrap());
    finish(&mut live);finish(&mut cold);assert_eq!(cold.state().vars,live.state().vars);assert_eq!(cold.state().random,random);assert_eq!(cold.state().vars["trace"],Value::Str("TC".into()));assert_no_projections(&cold);
}
#[test]
fn native0757_forged_break_site_visit_or_source_refuses_before_publication(){
    let(raw,live)=native0757_stop_tail();let save=live.capture_save15(7,"Native break site".into()).unwrap();
    for tamper in 0..4{
        let mut forged=save.clone();let frame=&mut forged.debugger.as_mut().unwrap().startup.as_mut().unwrap().loops[0];
        match tamper{
            0=>frame.native_break.as_mut().unwrap().visits+=1,
            1=>frame.native_break.as_mut().unwrap().route=frame.header.clone(),
            2=>frame.native_break.as_mut().unwrap().origin=frame.header_origin.clone(),
            _=>frame.native_break.as_mut().unwrap().route=vec![wire::Route::Statement(usize::MAX)],
        }
        let mut cold=startup(&raw);let before=serde_json::to_value(cold.state()).unwrap();let cursor=cold.engine.portable_startup();
        assert!(cold.restore_save15(forged).is_err(),"tamper {tamper}");assert_eq!(serde_json::to_value(cold.state()).unwrap(),before);assert_eq!(cold.engine.portable_startup(),cursor);assert!(cold.engine.renderer.batches.is_empty());
    }
}
#[test]
fn native0757_renderer_refusal_preserves_latched_break_and_pending_tail(){
    let(_,mut s)=native0757_stop_tail();
    for _ in 0..16{
        let before=serde_json::to_value(s.state()).unwrap();let cursor=s.engine.portable_startup();let batches=s.engine.renderer.batches.len();
        let plan=s.prepare_step(StepKind::Continue).unwrap();
        if plan.render_batch().is_some(){
            s.engine.renderer.reject.store(true,Ordering::Relaxed);assert!(s.begin_prepared_step_commit(plan).is_err());
            assert_eq!(serde_json::to_value(s.state()).unwrap(),before);assert_eq!(s.engine.portable_startup(),cursor);assert_eq!(s.engine.renderer.batches.len(),batches);return;
        }
        s.begin_prepared_step_commit(plan).unwrap().commit();
    }panic!("actual post-Break tail publication was not prepared");
}
#[test]
fn native0757_historical_range_break_keeps_its_original_sequence_exit(){
    let raw="init{set tails=0 flow.for_loop(i,0,2){flow.sequence(){{flow.break()}{set tails=tails+1}}}set done=true}\nscene label start\n\"Done\"\n";
    let mut s=startup(raw);finish(&mut s);assert_eq!(s.state().vars["tails"],Value::Int(0));assert_eq!(s.state().vars["done"],Value::Bool(true));
    let bytes=serde_json::to_vec(&s.engine.portable_startup()).unwrap();assert!(!String::from_utf8(bytes).unwrap().contains("native_break"));
}

const SCOPE:&str="g704/n1:init|exec_out=1@0";
const EACH:&str="n2:for_each|exec_in=2@1,collection=3@0,body=4@0,completed=5@0,array_index=6@0,array_element=7@0";
const RANGE:&str="n2:for_loop_with_break|exec_in=2@1,first=3@0,last=4@0,break=5@0,body=6@0,index=7@0,completed=8@0";
const INDEX:&str="rvn_native_index_g704_n2";
const VALUES:&str="function supplier(){local token=random(1,1) return [11,22,33]}";
const LAST:&str="function last(){local token=random(1,1) return 2}";
fn each(defs:&str,test:&str,body:&str)->String{
    format!("{defs}\ninit {{set visits=0 set total=0 set items=[11,22,33] flow.consume_v3(\"{SCOPE}\",\"{EACH}\"){{for {INDEX} in {test}{{{body}}}}} set done=true}}\nscene label start\n\"Done\"\n")
}
fn range(defs:&str,first:&str,last:&str,body:&str)->String{
    format!("{defs}\ninit {{set visits=0 set total=0 set bound=2 flow.consume_v3(\"{SCOPE}\",\"{RANGE}\"){{flow.for_loop({INDEX},{first},{last}){{{body}}}}} set done=true}}\nscene label start\n\"Done\"\n")
}
fn element(collection:&str)->String{
    format!("flow.consume_v1(\"{SCOPE}\",\"n4:set_variable|exec_in=8@2,name=9@0,value=10@0,exec_out=11@0,value_out=12@0\"){{capture \"{EACH}\"=({collection})[{INDEX}] set total=total+__rvn_captured_v1(\"{EACH}\")}}")
}
fn draws(mut random:crate::random::RandomState,count:usize)->crate::random::RandomState{for _ in 0..count{random.integer(1,1).unwrap();}random}
fn assert_no_projections(s:&EngineStartup<Host>){assert!(!s.state().vars.keys().any(|k|k.starts_with("rvn_native_")));}
fn body_route()->Vec<wire::Route>{vec![wire::Route::Statement(3),wire::Route::Body,wire::Route::Statement(0),wire::Route::Body,wire::Route::Statement(0)]}
fn stop_body(s:&mut EngineStartup<Host>){
    let route=body_route();let bp=s.site_page(0).unwrap().entries.into_iter().find(|entry|entry.breakpoint.site.as_ref().is_some_and(|site|site.route==route)).expect("real Native child site").breakpoint;
    s.configure(vec![bp],vec![]).unwrap();
    for _ in 0..128{if s.snapshot().site.as_ref().is_some_and(|site|site.route==route)&&s.engine.startup.as_ref().is_some_and(|state|!state.loops.is_empty()){return;}s.step(StepKind::Continue).unwrap();}
    panic!("real Native body was not reached");
}

#[test]
fn native0704_array_index_only_has_four_real_supplier_draws(){
    let raw=each(VALUES,"supplier()",&format!("set visits=visits+1 set total=total+{INDEX}"));let mut s=startup(&raw);let expected=draws(s.state().random,4);
    let authored=rvn_parser::parse(&raw).unwrap();let Statement::Init{body}=authored.iter().find(|statement|matches!(statement,Statement::Init{..})).unwrap()else{unreachable!()};let header=body.iter().find(|statement|matches!(statement,Statement::ScheduledConsumer{version:3,..})).unwrap();
    assert!(Engine::<Host>::is_owned_startup_control(header),"actual v3 header must enter the real Startup control path");
    let binding=rvn_parser::scheduled_source::native_loop_contract::binding(header).unwrap().unwrap();assert!(rvn_parser::scheduled_source::native_loop_contract::owned_engine_phase(&binding,&std::collections::HashMap::new()).is_err(),"classifier must not supply an unowned/default phase");
    finish(&mut s);assert_eq!(s.state().random,expected);assert_eq!(s.state().vars["visits"],Value::Int(3));assert_eq!(s.state().vars["total"],Value::Int(3));assert_no_projections(&s);
}
#[test]
fn native0704_array_element_has_seven_real_supplier_draws(){
    let raw=each(VALUES,"supplier()",&format!("set visits=visits+1 {}",element("supplier()")));let mut s=startup(&raw);let expected=draws(s.state().random,7);
    finish(&mut s);assert_eq!(s.state().random,expected);assert_eq!(s.state().vars["total"],Value::Int(66));assert_no_projections(&s);
}
#[test]
fn native0704_array_length_uses_current_array_after_shrink_and_grow(){
    for (replacement,visits,total) in [("[11]",1,11),("[11,22,33,0,0]",5,66)]{
        let raw=each("","items",&format!("set visits=visits+1 {} set items={replacement}",element("items")));let mut s=startup(&raw);finish(&mut s);
        assert_eq!(s.state().vars["visits"],Value::Int(visits));assert_eq!(s.state().vars["total"],Value::Int(total));assert_no_projections(&s);
    }
}
// Same test identity; this unit exercises the actual debugger Request/Result
// pipeline, while the rest of the test executes the real Startup transaction.
fn assert_first_prefix_replays_before_retained_test(){
    use rvn_parser::scheduled_source::native_loop_contract::{self as contract,Phase};
    let raw=range(&format!("function first(){{local token=random(1,1) return 0}} {LAST}"),"first()","last()","set visits=visits+1");
    let s=startup(&raw);let startup=s.engine.startup.as_ref().unwrap();let root=startup.current().unwrap();
    let Statement::Init{body}=&s.engine.script[root.pc]else{unreachable!()};
    let ordinal=body.iter().position(|statement|matches!(statement,Statement::ScheduledConsumer{version:3,..})).unwrap();
    let header=&body[ordinal];let route=vec![wire::Route::Statement(ordinal)];
    let Statement::ScheduledConsumer{scope,..}=header else{unreachable!()};
    let scope=s.engine.consumer_program.bound_scope(scope,root.pc).unwrap();
    let binding=contract::binding(header).unwrap().unwrap();
    let library=s.engine.functions.clone();let probe=Probe::shared();let mut debug=library.clone();debug.debug_probe=Some(probe.clone());
    let initial=s.state().random;let frame=s.state().consumer_frame.as_ref().unwrap().frame.clone();
    let globals=s.engine.lexical_vars_for_eval();let variables=s.engine.vars_for_eval();
    let evaluate=|phase:Phase,random:&mut crate::random::RandomState,frame:&mut crate::consumer_schedule::ConsumerFrame|{
        let mut environment=variables.clone();environment.insert(binding.phase.clone(),Value::Int(if phase==Phase::First{0}else{1}));
        if phase==Phase::Test{environment.insert(binding.cursor.clone(),Value::Int(0));}else{environment.remove(&binding.cursor);}
        Probe::initializer_route(&probe,route.clone(),||debug.eval_engine_consumer(header,&environment,&globals,random,frame,&scope,s.engine.locale.as_ref())).unwrap()
    };
    let finish_requested=||{let mut continuation=Probe::pending(&probe,&library).unwrap().unwrap();for _ in 0..256{if continuation.done(){break;}continuation.step().unwrap();}assert!(continuation.done());Probe::finish(&probe,&continuation).unwrap();};
    let mut random=initial;let mut current=frame.clone();
    assert!(evaluate(Phase::First,&mut random,&mut current).is_err());assert_eq!(random,initial);
    assert!(Probe::completed_native_first_record(&probe,&route).is_err());
    finish_requested();Probe::rewind(&probe).unwrap();
    assert_eq!(evaluate(Phase::First,&mut random,&mut current).unwrap(),Value::Int(0));
    let first=Probe::completed_native_first_record(&probe,&route).unwrap();
    assert!(evaluate(Phase::Test,&mut random,&mut current).is_err());
    assert!(Probe::completed_native_first_record(&probe,&route).is_err());
    finish_requested();Probe::rewind(&probe).unwrap();random=initial;current=frame;
    assert!(Probe::completed_native_first_record(&probe,&route).is_err(),"future retained First has not been consumed");
    assert_eq!(evaluate(Phase::First,&mut random,&mut current).unwrap(),Value::Int(0));
    assert!(!Probe::complete(&probe),"Test is retained but has not yet been replayed");
    assert!(Probe::completed_loop_records(&probe,&route).is_err(),"full-trace completion remains strict");
    assert!(Probe::seal(&probe).is_err());
    assert_eq!(Probe::completed_native_first_record(&probe,&route).unwrap(),first);
    assert!(Probe::completed_native_first_record(&probe,&[wire::Route::Statement(ordinal+1)]).is_err());
    assert_eq!(random,draws(initial,1));
    assert_eq!(evaluate(Phase::Test,&mut random,&mut current).unwrap(),Value::Int(2));
    assert!(Probe::complete(&probe));assert_eq!(Probe::completed_loop_records(&probe,&route).unwrap().len(),2);
    assert!(Probe::completed_native_first_record(&probe,&route).is_err(),"last consumed root is Test");
    assert_eq!(random,draws(initial,2));
}

#[test]
fn native0704_range_last_has_four_real_supplier_draws(){
    assert_first_prefix_replays_before_retained_test();
    let raw=range(LAST,"0","last()",&format!("set visits=visits+1 set total=total+{INDEX}"));let mut s=startup(&raw);let expected=draws(s.state().random,4);finish(&mut s);
    assert_eq!(s.state().random,expected);assert_eq!(s.state().vars["visits"],Value::Int(3));assert_eq!(s.state().vars["total"],Value::Int(3));assert_no_projections(&s);
}
#[test]
fn native0704_range_current_bound_shrink_and_grow(){
    for (last,visits,total) in [(0,1,0),(4,5,10)]{
        let raw=range("","0","bound",&format!("set visits=visits+1 set total=total+{INDEX} set bound={last}"));let mut s=startup(&raw);finish(&mut s);
        assert_eq!(s.state().vars["visits"],Value::Int(visits));assert_eq!(s.state().vars["total"],Value::Int(total));
    }
}
#[test]
fn native0704_break_neither_advances_max_index_nor_requests_another_bound(){
    for first in ["0","9223372036854775807"]{
        let last=if first=="0"{"last()"}else{first};let raw=range(LAST,first,last,"set visits=visits+1 flow.break()");let mut s=startup(&raw);let expected=draws(s.state().random,usize::from(first=="0"));finish(&mut s);
        assert_eq!(s.state().random,expected);assert_eq!(s.state().vars["visits"],Value::Int(1));assert_eq!(s.state().vars["done"],Value::Bool(true));
    }
}
#[test]
fn native0704_cold_save_restores_body_phase_without_input_or_rng_replay(){
    for raw in [each(VALUES,"supplier()","set visits=visits+1"),range(LAST,"0","last()","set visits=visits+1")]{
        let mut live=startup(&raw);stop_body(&mut live);let random=live.state().random;let save=live.capture_save15(7,"Native body".into()).unwrap();let bytes=save.encode().unwrap();
        let mut cold=startup(&raw);cold.restore_save15(crate::save::consumer_checkpoint::Save15::decode(&bytes).unwrap()).unwrap();assert_eq!(cold.state().random,random);assert_eq!(cold.snapshot().site.as_ref().unwrap().route,body_route());
        finish(&mut live);finish(&mut cold);assert_eq!(cold.state().random,live.state().random);assert_eq!(cold.state().vars,live.state().vars);assert_no_projections(&cold);
    }
}
#[test]
fn native0704_forged_cursor_phase_source_and_global_projection_refuse_atomically(){
    let raw=range(LAST,"0","last()","set visits=visits+1");let mut live=startup(&raw);stop_body(&mut live);let save=live.capture_save15(7,"Native forged".into()).unwrap();
    for tamper in 0..6{
        let mut forged=save.clone();let frame=&mut forged.debugger.as_mut().unwrap().startup.as_mut().unwrap().loops[0];
        match tamper{
            0=>{let PortableStartupLoopState::DemandV1{cursor:crate::native_loop_cursor::DemandLoopCursor::RangeV1{index,..}}=&mut frame.state else{unreachable!()};*index+=1;},
            1=>frame.visits+=1,
            2=>frame.gather.swap(0,1),
            3=>{let mut row=serde_json::to_value(&frame.gather[1]).unwrap();row["request"]["variables"]["rvn_native_cursor_g704_n2"]=serde_json::to_value(Value::Int(99)).unwrap();frame.gather[1]=serde_json::from_value(row).unwrap();},
            4=>frame.header=vec![wire::Route::Statement(4)],
            5=>{let mut row=serde_json::to_value(&frame.gather[1]).unwrap();row["request"]["state"]["globals"][INDEX]=serde_json::to_value(Value::Int(0)).unwrap();frame.gather[1]=serde_json::from_value(row).unwrap();},_=>unreachable!()
        }
        let mut target=startup(&raw);let before=serde_json::to_vec(target.state()).unwrap();assert!(target.restore_save15(forged).is_err(),"tamper {tamper}");assert_eq!(serde_json::to_vec(target.state()).unwrap(),before);assert!(target.engine.renderer.batches.is_empty());
    }
}
#[test]
fn native0704_renderer_refusal_keeps_actual_native_cursor_and_body_unpublished(){
    let raw=each("","items","set visits=visits+1");let mut s=startup(&raw);stop_body(&mut s);
    for _ in 0..16{
        let before=serde_json::to_vec(s.state()).unwrap();let cursor=s.engine.portable_startup();let batches=s.engine.renderer.batches.len();
        let plan=s.prepare_step(StepKind::Continue).unwrap();if plan.render_batch().is_some(){
            s.engine.renderer.reject.store(true,Ordering::Relaxed);assert!(s.begin_prepared_step_commit(plan).is_err());
            assert_eq!(serde_json::to_vec(s.state()).unwrap(),before);assert_eq!(s.engine.portable_startup(),cursor);assert_eq!(s.engine.renderer.batches.len(),batches);return;
        }s.begin_prepared_step_commit(plan).unwrap().commit();
    }panic!("no actual publication was prepared");
}
#[test]
fn native0704_wrong_input_types_refuse_without_publishing_body(){
    for(raw,expected)in [(each("","7","set bad=1"),"Native ArrayLength requires its actual list"),(range("","0","2.5","set bad=1"),"Native LastIndex requires an integer"),(range("","0.0","2","set bad=1"),"Native FirstIndex requires an integer")]{
        let mut s=startup(&raw);let mut refusal=None;for _ in 0..32{match s.step(StepKind::Continue){Ok(_)=>{},Err(error)=>{refusal=Some(format!("{error:?}"));break;}}}let refusal=refusal.expect("real typed Native input must be refused");assert!(refusal.contains(expected),"wrong refusal: {refusal}");assert!(!refusal.contains("Native phase has no actual owning Engine environment"));assert!(!s.state().vars.contains_key("bad"));
    }
}
#[test]
fn native0704_historical_foreach_and_range_keep_their_original_snapshot_results(){
    for(raw,total)in [("init {set total=0 set items=[1,2,3] for item in items {set total=total+item set items=[]}}\nscene label start\n\"Done\"\n",6),("init {set total=0 set last=2 flow.for_loop(i,0,last){set total=total+i set last=0}}\nscene label start\n\"Done\"\n",3)]{
        let mut s=startup(raw);finish(&mut s);assert_eq!(s.state().vars["total"],Value::Int(total));assert_no_projections(&s);
    }
}
