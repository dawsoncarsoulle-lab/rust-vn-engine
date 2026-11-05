// PRIVATE NOT_RUN. Uses actual preparation, Engine, evaluator, Save and rollback.
use rvn_core::{Engine,Interaction,TerminalRenderer};
use rvn_parser::{parse,parse_program,Value,Statement as S};
fn text<R:rvn_core::Renderer>(engine:&mut Engine<R>)->String{
    match engine.step_until_interaction().unwrap(){Some(Interaction::Dialogue{text,..})=>text,other=>panic!("{other:?}")}
}
#[test]
fn sequence_resumes_each_ordered_vn_branch_after_real_call_and_dialogue(){
    for prefix in ["label start","scene label start"]{
        let source=format!("init {{set trace=0}}\n{prefix}\nflow.sequence() {{\n{{set trace=1\n\"first\"}}\n{{call helper\nset trace=trace+10\n\"second\"}}\n}}\n\"completed\"\njump end\nlabel helper\nset trace=trace+2\n\"helper\"\nreturn\nlabel end\n");
        let mut engine=Engine::new(parse(&source).unwrap(),TerminalRenderer,32).unwrap();
        assert_eq!(text(&mut engine),"first");assert_eq!(engine.state.vars["trace"],Value::Int(1));
        engine.advance_dialogue().unwrap();assert_eq!(text(&mut engine),"helper");assert_eq!(engine.state.vars["trace"],Value::Int(3));
        engine.advance_dialogue().unwrap();assert_eq!(text(&mut engine),"second");assert_eq!(engine.state.vars["trace"],Value::Int(13));
        engine.advance_dialogue().unwrap();assert_eq!(text(&mut engine),"completed");
    }
}
#[test]
fn inclusive_range_break_skips_following_branches_and_completes_without_crossing_call(){
    let source="init {set total=0}\nscene label start\nflow.for_loop(i,-2,3) {flow.sequence() {{set total=total+i} {if i==0 {flow.break()}\nset total=total+100}}}\n\"[total]:[i]\"\n";
    let mut engine=Engine::from_program(parse_program(source).unwrap(),TerminalRenderer,32).unwrap();
    assert_eq!(text(&mut engine),"197:0");assert_eq!(engine.state.vars["i"],Value::Int(0));
    assert!(!engine.state.vars.keys().any(|s|s.starts_with("__rvn_")));
}
#[test]
fn range_real_call_return_resumes_cursor_and_scene_jump_abandons_remaining_sequence(){
    let source="init {set seen=0}\nscene label start\nflow.for_loop(i,1,2) {call helper\nset seen=seen+i}\nflow.sequence() {{jump destination} {set seen=999}}\n\"unreachable\"\nlabel helper\n\"helper [i]\"\nreturn\nscene label destination\n\"done [seen]\"\n";
    let mut engine=Engine::from_program(parse_program(source).unwrap(),TerminalRenderer,32).unwrap();
    assert_eq!(text(&mut engine),"helper 1");engine.advance_dialogue().unwrap();
    assert_eq!(text(&mut engine),"helper 2");assert_eq!(engine.state.vars["seen"],Value::Int(1));engine.advance_dialogue().unwrap();
    assert_eq!(text(&mut engine),"done 3");assert_eq!(engine.state.vars["seen"],Value::Int(3));
    let execution=engine.state.scene_execution.as_ref().unwrap();assert_eq!(execution.active.as_ref().unwrap().scene,"destination");assert!(execution.frames.is_empty());
}
#[test]
fn range_dialogue_save_load_and_rollback_preserve_actual_cursor_and_scope(){
    let source="init {set i=99}\nscene label start\nlocal i=0\nflow.for_loop(i,1,3) {\"index [i]\"\nif i==2 {flow.break()}}\n\"completed [i]\"\n";
    let mut engine=Engine::from_program(parse_program(source).unwrap(),TerminalRenderer,32).unwrap();
    assert_eq!(text(&mut engine),"index 1");assert_eq!(engine.state.vars["i"],Value::Int(99));
    let snapshot=engine.snapshot_for_save().unwrap();let saved=rvn_core::save::SaveData::try_from_snapshot(&snapshot,1,"loop".into(),"flow.rvn".into()).unwrap();
    assert_eq!(saved.format_version,11);
    let bytes=serde_json::to_vec(&saved).unwrap();let mut fresh=engine.fresh(TerminalRenderer,32).unwrap();fresh.load_data(serde_json::from_slice(&bytes).unwrap()).unwrap();
    assert_eq!(text(&mut fresh),"index 1");fresh.advance_dialogue().unwrap();assert_eq!(text(&mut fresh),"index 2");
    fresh.advance_dialogue().unwrap();assert_eq!(text(&mut fresh),"completed 2");
    let (source,_)=fresh.preview_game_audio_rollback().unwrap().unwrap();
    assert!(fresh.rollback_from_audio_source(&source).unwrap());assert_eq!(text(&mut fresh),"index 2");
    assert_eq!(fresh.state.vars["i"],Value::Int(99));
}
#[test]
fn legacy_range_dialogue_snapshot9_resumes_the_same_scalar_cursor_after_fresh_load(){
    let source="label start\nflow.for_loop(i,1,2) {\"index [i]\"}\n\"done [i]\"\n";
    let mut engine=Engine::new(parse(source).unwrap(),TerminalRenderer,16).unwrap();assert_eq!(text(&mut engine),"index 1");
    let data=rvn_core::save::SaveData::try_from_state(&engine.state,1,"loop".into(),"legacy.rvn".into()).unwrap();assert_eq!(data.format_version,9);
    let mut restored=engine.fresh(TerminalRenderer,16).unwrap();restored.load_data(data).unwrap();assert_eq!(text(&mut restored),"index 1");
    restored.advance_dialogue().unwrap();assert_eq!(text(&mut restored),"index 2");restored.advance_dialogue().unwrap();assert_eq!(text(&mut restored),"done 2");
}
#[test]
fn modern_range_bound_evaluation_refuses_before_cursor_rng_history_or_publication_changes(){
    let source="init {set seen=7}\nscene label start\nflow.for_loop(i,rand(0,9),1.5) {set seen=999}\n\"done\"\n";
    let mut engine=Engine::from_program(parse_program(source).unwrap(),TerminalRenderer,16).unwrap();engine.enter_label("start").unwrap();
    let before=serde_json::to_vec(&engine.state).unwrap();let epoch=engine.runtime_epoch();let history=engine.history.entries().len();
    let error=engine.step_silent().unwrap_err();assert!(error.to_string().contains("ForLoopWithBreak"));
    assert_eq!(serde_json::to_vec(&engine.state).unwrap(),before);assert_eq!(engine.runtime_epoch(),epoch);assert_eq!(engine.history.entries().len(),history);
}
#[test]
fn nested_range_break_is_local_and_existing_while_foreach_keep_their_semantics(){
    let source="function count() {set total=0\nflow.for_loop(i,0,2) {flow.for_loop(j,0,3) {set total=total+1\nflow.break()}\nset total=total+10}\nfor n in [2,3] {set total=total+n}\nwhile total<40 {set total=total+1}\nreturn total}\nlabel start\n\"[count()]\"";
    let mut engine=Engine::new(parse(source).unwrap(),TerminalRenderer,16).unwrap();assert_eq!(text(&mut engine),"40");
    for name in ["i","j","n","total"]{assert!(!engine.state.vars.contains_key(name));}
}
#[test]
fn break_reaches_its_lexical_range_through_old_while_and_foreach_after_interaction(){
    for prefix in ["label start","scene label start"]{
        let source=format!("function count() {{set n=0\nflow.for_loop(i,0,3) {{while true {{for item in [1,2] {{set n=n+item\nflow.break()}}\nset n=999}}\nset n=999}}\nreturn n}}\ninit {{set total=0}}\n{prefix}\nflow.for_loop(i,0,3) {{while true {{for item in [1,2] {{set total=total+item\n\"before [i]:[item]\"\nflow.break()}}\nset total=999}}\nset total=999}}\n\"done [total]:[count()]\"\n");
        let mut engine=Engine::new(parse(&source).unwrap(),TerminalRenderer,16).unwrap();
        assert_eq!(text(&mut engine),"before 0:1");assert_eq!(engine.state.vars["total"],Value::Int(1));
        engine.advance_dialogue().unwrap();assert_eq!(text(&mut engine),"done 1:1");
        assert_eq!(engine.state.vars["item"],Value::Int(1));assert_eq!(engine.state.vars["i"],Value::Int(0));
    }
}
#[test]
fn empty_inclusive_and_integer_boundary_ranges_do_not_allocate_or_overflow(){
    for (bounds,expected) in [("3,2",0),("-2,2",5),("9223372036854775807,9223372036854775807",1),("-9223372036854775807-1,-9223372036854775807-1",1)]{
        let source=format!("function count() {{set n=0\nflow.for_loop(i,{bounds}) {{set n=n+1}}\nreturn n}}\nlabel start\n\"[count()]\"");
        let mut engine=Engine::new(parse(&source).unwrap(),TerminalRenderer,16).unwrap();assert_eq!(text(&mut engine),expected.to_string());
    }
}
#[test]
fn flow_budget_and_invalid_bounds_refuse_without_committing_handler_rng_or_globals(){
    use rvn_core::{eval::FunctionLibrary,random::RandomState};use std::collections::HashMap;
    for bounds in ["0,100000","0,1.5","0,9223372036854775807"]{
        let source=format!("handler h() {{set seen=rand(0,9)\nflow.for_loop(i,{bounds}) {{set seen=seen+i}}}}");
        let script=parse(&source).unwrap();let S::Handler{parameters,body,..}=&script[0] else{panic!()};
        let library=FunctionLibrary::from_script(&script).unwrap();let mut random=RandomState::default();let before=random.clone();let globals=HashMap::from([("seen".into(),Value::Int(7))]);
        assert!(library.handle_event(parameters,body,&[],&globals,&mut random).is_err());assert_eq!(random,before);assert_eq!(globals["seen"],Value::Int(7));
    }
    let script=parse("handler h() {flow.for_loop(i,0,99999) {set a=i}} ").unwrap();let S::Handler{parameters,body,..}=&script[0] else{panic!()};let library=FunctionLibrary::from_script(&script).unwrap();
    let mut rng=RandomState::default();assert!(library.handle_event(parameters,body,&[],&HashMap::new(),&mut rng).unwrap_err().to_string().contains("100 000"));
}
#[test]
fn flow_new_position_format10_refuses_before_restore_and_old_shapes_remain10_or9(){
    let mut engine=Engine::new(parse("scene label start\nflow.sequence() {{\"one\"} {\"two\"}}\n").unwrap(),TerminalRenderer,16).unwrap();text(&mut engine);
    let mut data=rvn_core::save::SaveData::try_from_state(&engine.state,1,"s".into(),"s.rvn".into()).unwrap();assert_eq!(data.format_version,11);data.format_version=10;
    let before=serde_json::to_vec(&engine.state).unwrap();let epoch=engine.runtime_epoch();assert!(engine.load_data(data).is_err());assert_eq!(serde_json::to_vec(&engine.state).unwrap(),before);assert_eq!(engine.runtime_epoch(),epoch);
    for (source,version) in [("scene label start\n\"old\"",10),("label start\n\"old\"",9)]{let mut engine=Engine::new(parse(source).unwrap(),TerminalRenderer,16).unwrap();text(&mut engine);assert_eq!(rvn_core::save::SaveData::try_from_state(&engine.state,1,"s".into(),"s.rvn".into()).unwrap().format_version,version);}
}
#[test]
fn forged_break_and_sequence_shape_are_refused_before_engine_construction(){
    for script in [vec![S::LoopBreak],vec![S::Sequence{branches:vec![]}],vec![S::Sequence{branches:vec![vec![];33]}]]{assert!(Engine::new(script,TerminalRenderer,16).is_err());}
}
