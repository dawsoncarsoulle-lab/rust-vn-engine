//! Live Unit integration through the actual Engine, source and renderer owner.
//! Child of existing engine_debugger_tests.rs; reuses its real Host.
use super::*;

const UNIT_SOURCE:&str="// été 🍃\r\nfunction value(n){return n+1}\r\nfunction inner(`quoted parameter`)->(){set result=value(`quoted parameter`)+random(1,9) return set forbidden=1}\r\nfunction outer(n)->(){local own=99 call inner(n) set finished=1}\r\ninit {set result=0 set finished=0}\r\nscene label start\r\nlocal n=4\r\ncall outer(n)\r\n\"After\"\r\nreturn\r\n";
fn actual_unit_engine()->Engine<Host>{
    let mut engine=Engine::from_program_with_debugger(rvn_parser::parse_program(UNIT_SOURCE).unwrap(),Host::default(),16).unwrap();
    engine.enter_label("start").unwrap();engine.preview_step_statement().unwrap();
    assert!(matches!(engine.script.get(engine.state.pc),Some(Statement::UnitCall{..})));
    engine.state.random=crate::random::RandomState::seeded(775);engine.renderer.batches.clear();engine
}

#[test]
fn unit_live_debug1021_nested_into_over_out_and_continue_publish_one_actual_call_without_result(){
    let mut normal=actual_unit_engine();normal.preview_step_statement().unwrap();
    let mut engine=actual_unit_engine();let before=engine.debugger_anchor().unwrap();
    let mut session=engine.debug_begin(DebugOperation::Statement,vec![]).unwrap();
    let mut found=false;
    for _ in 0..64{
        assert_eq!(engine.debug_step(&mut session,StepKind::Into).unwrap().0,DebugProgress::Stopped);
        assert_eq!(engine.debugger_anchor().unwrap(),before);assert!(engine.renderer.batches.is_empty());
        if session.snapshot().site.as_ref().and_then(|site|site.function.as_deref())==Some("inner"){found=true;break;}
    }
    assert!(found,"Unit callee must have a real lexical continuation frame");
    let snapshot=session.snapshot();let frame=snapshot.frames.iter().find(|frame|frame.function.as_deref()==Some("inner")).unwrap();
    assert!(frame.bindings.iter().any(|(name,_)|name=="quoted parameter"));
    assert!(frame.origin.as_ref().and_then(|origin|origin.span.as_ref()).is_some());
    engine.debug_step(&mut session,StepKind::Over).unwrap();
    assert_eq!(engine.debugger_anchor().unwrap(),before);
    let progress=engine.debug_step(&mut session,StepKind::Out).unwrap().0;
    if progress!=DebugProgress::Complete{finish(&mut engine,&mut session);}
    assert_eq!(engine.state.vars,normal.state.vars);assert_eq!(engine.state.random,normal.state.random);assert_eq!(engine.state.pc,normal.state.pc);
    assert_eq!(engine.state.scene_execution.as_ref().unwrap().active.as_ref().unwrap().locals,
        normal.state.scene_execution.as_ref().unwrap().active.as_ref().unwrap().locals);
    assert_eq!(engine.renderer.batches.len(),1);
    assert!(!engine.state.vars.contains_key("own"));assert!(!engine.state.vars.contains_key("forbidden"));
    assert!(engine.debug_step(&mut session,StepKind::Continue).is_err());assert_eq!(engine.renderer.batches.len(),1);
}

#[test]
fn unit_live_debug1021_renderer_refusal_and_stale_owner_never_publish_rng_globals_or_pc(){
    let mut engine=actual_unit_engine();let mut other=actual_unit_engine();
    let mut session=engine.debug_begin(DebugOperation::Statement,vec![]).unwrap();
    let before=other.debugger_anchor().unwrap();assert!(other.debug_step(&mut session,StepKind::Into).is_err());assert_eq!(other.debugger_anchor().unwrap(),before);
    let before=engine.debugger_anchor().unwrap();engine.renderer.reject=true;let mut refused=false;
    for _ in 0..128{
        match engine.debug_step(&mut session,StepKind::Continue){
            Err(error)=>{assert!(error.to_string().contains("real renderer refused"));refused=true;break;},Ok(_)=>{},
        }
    }
    assert!(refused);assert_eq!(engine.debugger_anchor().unwrap(),before);assert!(engine.renderer.batches.is_empty());
    // An equal-valued write still carries write intent at the real input owner.
    let source="function effect(n)->(){set __input_mouse_x=__input_mouse_x}\nscene label start\ncall effect(random(1,9))\n\"After\"\n";
    let mut protected=Engine::from_program_with_debugger(rvn_parser::parse_program(source).unwrap(),Host::default(),8).unwrap();
    protected.enter_label("start").unwrap();protected.update_input_state(None,false,2.0,3.0);
    protected.state.random=crate::random::RandomState::seeded(42);protected.renderer.batches.clear();
    let protected_before=protected.debugger_anchor().unwrap();let mut protected_session=protected.debug_begin(DebugOperation::Statement,vec![]).unwrap();
    let mut refused_input=false;
    for _ in 0..128{if let Err(error)=protected.debug_step(&mut protected_session,StepKind::Continue){assert!(error.to_string().contains("input snapshots"));refused_input=true;break;}}
    assert!(refused_input);assert_eq!(protected.debugger_anchor().unwrap(),protected_before);assert!(protected.renderer.batches.is_empty());
    let stopped=serde_json::to_vec(&session.snapshot()).unwrap();
    assert!(engine.debug_step(&mut session,StepKind::Continue).is_err());assert_eq!(serde_json::to_vec(&session.snapshot()).unwrap(),stopped);
}

#[test]
fn unit_live_debug1021_actual_declaration_breakpoints_keep_save_and_foreign_routes_refused(){
    let mut engine=actual_unit_engine();
    let pc=engine.script.iter().position(|statement|matches!(statement,Statement::UnitFunction{name,..}if name=="inner")).unwrap();
    let site=wire::Site{function:Some("inner".into()),route:vec![wire::Route::Statement(0)],depth:2,statement:true};
    let bp=wire::Breakpoint{position:engine.narrative.as_ref().unwrap().position(pc),site:Some(site.clone()),enabled:true};
    let mut foreign=bp.clone();foreign.site.as_mut().unwrap().function=Some("foreign".into());
    assert!(engine.debug_begin(DebugOperation::Statement,vec![foreign]).is_err());
    let mut session=engine.debug_begin(DebugOperation::Statement,vec![bp]).unwrap();
    assert_eq!(engine.debug_step(&mut session,StepKind::Continue).unwrap().0,DebugProgress::Stopped);
    assert_eq!(session.snapshot().site,Some(site));
    let before=engine.debugger_anchor().unwrap();assert!(engine.snapshot_for_save().is_err());assert_eq!(engine.debugger_anchor().unwrap(),before);
    finish(&mut engine,&mut session);
    let mut forged=actual_unit_engine();
    let Statement::UnitCall{arguments,..}=&mut forged.script[forged.state.pc]else{panic!("actual call")};arguments[0]=rvn_parser::Expr::Int(99);
    let unchanged=forged.debugger_anchor().unwrap();assert!(forged.debug_begin(DebugOperation::Statement,vec![]).is_err());assert_eq!(forged.debugger_anchor().unwrap(),unchanged);
    assert!(engine.snapshot_for_save().is_err(),"live Unit debug is not a portable Save activation contract");
}
