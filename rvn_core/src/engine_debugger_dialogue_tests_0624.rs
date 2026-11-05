//! Internal transfer-phase portable cursor from actual Graph source.
//! Public Save15 correctly requires a stopped visible operation; this is not one.
use super::*;
const SOURCE:&str=include_str!("../tests/fixtures/dialogue-transfer-0624.rvn");
fn engine(program:rvn_parser::ParsedProgram)->Engine<crate::TerminalRenderer>{
    let mut e=Engine::from_program_with_debugger(program,crate::TerminalRenderer,16).unwrap();e.state.random=crate::random::RandomState::seeded(11);e
}
fn finish(e:&mut Engine<crate::TerminalRenderer>,s:&mut DebugSession){for _ in 0..128{if e.debug_step(s,StepKind::Continue).unwrap().0==DebugProgress::Complete{return;}}panic!("actual Dialogue did not finish")}
fn stopped_producer()->(rvn_parser::ParsedProgram,Engine<crate::TerminalRenderer>,DebugSession){
    let program=rvn_parser::parse_program(SOURCE).unwrap();let mut e=engine(program.clone());e.enter_label("start").unwrap();
    let mut offset=0;let breakpoint=loop{let page=e.debug_site_page(offset).unwrap();
        if let Some(entry)=page.entries.iter().find(|entry|entry.origin.as_ref().and_then(|o|o.scheduled_step.as_ref()).and_then(|s|s.endpoint.as_ref()).is_some_and(|p|p.kind=="set_variable"&&p.ports.iter().any(|port|port.role=="value_out"))){break entry.breakpoint.clone();}
        offset+=page.entries.len();assert!(offset<page.total,"actual Graph assignment output producer required");
    };
    let mut session=e.debug_begin(DebugOperation::Statement,vec![breakpoint]).unwrap();assert_eq!(e.debug_step(&mut session,StepKind::Continue).unwrap().0,DebugProgress::Stopped);
    (program,e,session)
}
fn stopped_transfer()->(rvn_parser::ParsedProgram,Engine<crate::TerminalRenderer>,DebugSession){
    let (program,mut e,mut session)=stopped_producer();
    let before=serde_json::to_value(&e.state).unwrap();
    for _ in 0..128{
        if session.continuation.as_ref().is_some_and(|c|c.dialogue_transfer_pending()){break;}
        let result=e.debug_step_inner_budget(&mut session,StepKind::Continue,1,1).unwrap();assert_ne!(result.0,DebugProgress::Complete,"the exact transfer must be reached before publication");
    }
    assert!(session.continuation.as_ref().is_some_and(|c|c.dialogue_transfer_pending()),"pause after the real captures, immediately before their retained transfer");
    assert_eq!(serde_json::to_value(&e.state).unwrap(),before,"uncommitted candidate must not publish state");
    (program,e,session)
}
#[test]
fn exact_dialogue_transfer_cursor_cold_uses_the_already_captured_real_source_value(){
    let (program,mut warm,mut session)=stopped_transfer();
    assert!(warm.capture_save15(&program,Some(&session),1,"not a visible stop".into()).is_err());
    let trace=Probe::portable_trace(&session.probe,session.continuation.as_ref().unwrap()).unwrap();let json=serde_json::to_string(&trace).unwrap();assert!(json.contains("consumer_dialogue_input"));
    assert!(serde_json::to_value(&trace).unwrap()["records"].as_array().unwrap().is_empty());
    finish(&mut warm,&mut session);
    let (_,mut cold,mut restored)=stopped_producer();
    restored.continuation.as_mut().unwrap().restore_portable(&serde_json::from_str(&json).unwrap(),None).unwrap();finish(&mut cold,&mut restored);
    assert_eq!(serde_json::to_value(&cold.state).unwrap(),serde_json::to_value(&warm.state).unwrap());
    assert_eq!(cold.displayed_interaction().unwrap(),Some(Interaction::Dialogue{character:None,text:"before".into()}));assert_eq!(cold.state.random,crate::random::RandomState::seeded(11));
}
#[test]
fn forged_dialogue_transfer_cannot_become_an_authored_re_evaluation_or_another_port(){
    let (_program,_warm,session)=stopped_transfer();let trace=Probe::portable_trace(&session.probe,session.continuation.as_ref().unwrap()).unwrap();let wire=serde_json::to_value(&trace).unwrap();
    for variant in 0..3{
        let mut changed=wire.clone();let tasks=changed["pending"]["tasks"].as_array_mut().unwrap();
        let transfer=tasks.iter_mut().find(|task|task["kind"]=="consumer_dialogue_input").expect("real port transfer cursor in save");
        match variant{
            0=>transfer["kind"]=serde_json::json!("consumer_inputs"),
            1=>{transfer["kind"]=serde_json::json!("expr");transfer["ctx"]["route"]=serde_json::to_value(vec![wire::Route::Body,wire::Route::Statement(0),wire::Route::Argument(0)]).unwrap();},
            _=>transfer["data"]=serde_json::json!({"producer":"unbound_equal_value"}),
        }
        let (_,cold,mut restored)=stopped_producer();let before=serde_json::to_value(&cold.state).unwrap();let epoch=cold.runtime_epoch();
        let original=Probe::portable_trace(&restored.probe,restored.continuation.as_ref().unwrap()).unwrap();
        assert!(restored.continuation.as_mut().unwrap().restore_portable(&serde_json::from_value(changed).unwrap(),None).is_err());
        assert_eq!(serde_json::to_value(Probe::portable_trace(&restored.probe,restored.continuation.as_ref().unwrap()).unwrap()).unwrap(),serde_json::to_value(original).unwrap());
        assert_eq!(serde_json::to_value(&cold.state).unwrap(),before);assert_eq!(cold.runtime_epoch(),epoch);
    }
}


#[test]
fn fabricated_transfer_stack_and_context_are_refused_even_with_identical_real_capture_values(){
    let (_,_,session)=stopped_transfer();let trace=Probe::portable_trace(&session.probe,session.continuation.as_ref().unwrap()).unwrap();let wire=serde_json::to_value(&trace).unwrap();
    let mut accepted=vec![];
    for variant in 0..5{
        let mut changed=wire.clone();
        let tasks=changed["pending"]["tasks"].as_array_mut().unwrap();let index=tasks.iter().position(|t|t["kind"]=="consumer_dialogue_input").unwrap();
        match variant{
            0=>tasks[index]["ctx"]["depth"]=serde_json::json!(1),
            1=>{let duplicate=tasks[index].clone();tasks.push(duplicate);},
            2=>tasks.swap(0,index),
            3=>changed["pending"]["values"]=serde_json::to_value(vec![Value::Str("before".into())]).unwrap(),
            _=>tasks[index]["ctx"]["route"]=serde_json::to_value(vec![wire::Route::Producer(0)]).unwrap(),
        }
        // Identical captures/ordinals alone do not establish a real phase owner.
        assert_eq!(changed["pending"]["frames"],wire["pending"]["frames"]);
        let (_,_,mut restored)=stopped_producer();let before=serde_json::to_value(Probe::portable_trace(&restored.probe,restored.continuation.as_ref().unwrap()).unwrap()).unwrap();
        if restored.continuation.as_mut().unwrap().restore_portable(&serde_json::from_value(changed).unwrap(),None).is_ok(){accepted.push(variant);}else{
            assert_eq!(serde_json::to_value(Probe::portable_trace(&restored.probe,restored.continuation.as_ref().unwrap()).unwrap()).unwrap(),before);
        }
    }
    assert!(accepted.is_empty(),"fabricated phase/context variants accepted: {accepted:?}");
}
