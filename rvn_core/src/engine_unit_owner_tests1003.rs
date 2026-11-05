use super::*;
use rvn_parser::Expr;
use std::{cell::Cell,rc::Rc};

#[derive(Clone,Default)]
struct Receipt { fail:Rc<Cell<bool>>, publications:Rc<Cell<usize>> }
impl Renderer for Receipt {
    fn validate_scene_batch(&self,_:&crate::renderer::SceneRenderBatch)->Result<(),String>{if self.fail.get(){Err("controlled source owner refusal".into())}else{Ok(())}}
    fn publish_scene_batch(&mut self,_:crate::renderer::SceneRenderBatch){self.publications.set(self.publications.get()+1);}
    fn set_background(&mut self,_:&str,_:&Transition){}
    fn show_sprite(&mut self,_:&str,_:Option<&str>,_:&Position,_:&Transition,_:Option<&SpriteState>){}
    fn hide_sprite(&mut self,_:&str,_:&Transition,_:&SpriteState){}
    fn move_sprite(&mut self,_:&str,_:&Position,_:&Transition,_:&SpriteState){}
    fn show_dialogue(&mut self,_:Option<&str>,_:&str){}
    fn show_choice(&mut self,_:&[String])->usize{0}
    fn show_imagemap(&mut self,_:&str,_:Option<&str>,_:&[Hotspot])->usize{0}
    fn music_play(&mut self,_:&str,_:&Transition,_:Option<&str>){}
    fn music_stop(&mut self,_:&Transition){}
    fn music_set_volume(&mut self,_:f32){}
    fn sfx_play(&mut self,_:&str,_:&Transition){}
    fn sfx_stop(&mut self,_:&str,_:&Transition){}
}
fn engine(source:&str)->Engine<Receipt>{Engine::new(rvn_parser::parse_program(source).unwrap().script,Receipt::default(),8).unwrap()}
fn bytes(game:&Engine<Receipt>)->Vec<u8>{serde_json::to_vec(&game.state).unwrap()}
/// Reach the real call by executing every prior actual Engine instruction.
fn reach_call(game:&mut Engine<Receipt>){for _ in 0..1000{let statement=game.peek_statement().unwrap().clone();if matches!(statement,Statement::UnitCall{..}){return;}assert!(!Engine::<Receipt>::is_interactive(&statement));game.exec_silent(statement).unwrap();}panic!("actual call was not reached");}

#[test]
fn unit_engine1003_unused_definition_init_and_reached_call_publish_once(){
    let mut game=engine(r#"init {set calls=0 call boot() if false {call unused()}}
        function unused()->(){set forbidden=1}
        function boot()->(){set calls=calls+1}
        function reached()->(){set calls=calls+10}
        label start if false {call unused()} call reached() "After" "Next""#);
    assert_eq!(game.state.vars["calls"],Value::Int(1));assert!(!game.state.vars.contains_key("forbidden"));
    reach_call(&mut game);let pc=game.state.pc;let stack=game.state.call_stack.clone();let epochs=(game.runtime_epoch(),game.interface_epoch());
    game.exec_silent(game.peek_statement().unwrap().clone()).unwrap();
    assert_eq!(game.state.pc,pc+1);assert_eq!(game.state.vars["calls"],Value::Int(11));assert_eq!(game.state.call_stack,stack);assert_eq!((game.runtime_epoch(),game.interface_epoch()),epochs);
    game.step_until_interaction().unwrap();assert_eq!(game.state.vars["calls"],Value::Int(11));assert!(!game.state.vars.contains_key("forbidden"));
}
#[test]
fn unit_engine1003_scene_arguments_read_local_but_body_writes_only_game_globals(){
    let program=rvn_parser::parse_program(r#"init {set shared=10 set seen=0 set inner_seen=0 set read_seen=0}
        function read(){return shared}
        function inner(shared)->(){set shared=shared+1000 set inner_seen=shared}
        function outer(n)->(){local private=7 set seen=n set shared=shared+1 call inner(n) set read_seen=read()}
        scene label start local shared=99 call outer(shared) "After""#).unwrap();
    assert_eq!(program.sources.len(),1);
    assert_eq!(program.origins.len(),program.script.len());
    let unit=program.script.iter().position(|s|matches!(s,Statement::UnitFunction{name,..}if name=="outer")).unwrap();
    let Statement::UnitFunction{body,..}=&program.script[unit]else{panic!()};
    assert_eq!(program.origins[unit].span.children.len(),body.len());
    let mut game=Engine::from_program(program,Receipt::default(),8).unwrap();
    game.enter_label("start").unwrap();let activation=game.state.scene_execution.as_ref().unwrap().active.as_ref().unwrap().id;
    reach_call(&mut game);let before=game.state.scene_execution.as_ref().unwrap().clone();let pc=game.state.pc;
    game.exec_silent(game.peek_statement().unwrap().clone()).unwrap();
    let execution=game.state.scene_execution.as_ref().unwrap();let active=execution.active.as_ref().unwrap();
    assert_eq!(active.id,activation);assert_eq!(active.locals["shared"],Value::Int(99));assert_eq!(active.locals,before.active.as_ref().unwrap().locals);
    assert_eq!(active.temporaries,before.active.as_ref().unwrap().temporaries);assert_eq!(execution.frames,before.frames);
    assert_eq!(game.state.vars["shared"],Value::Int(11));assert_eq!(game.state.vars["seen"],Value::Int(99));assert_eq!(game.state.vars["inner_seen"],Value::Int(1099));assert_eq!(game.state.vars["read_seen"],Value::Int(11));assert!(!game.state.vars.contains_key("n"));assert!(!game.state.vars.contains_key("private"));assert_eq!(game.state.pc,pc+1);
}
#[test]
fn unit_engine1003_error_after_nested_rng_effects_keeps_state_pc_and_publication_neutral(){
    let mut game=engine(r#"init {set seen=0} function inner(n)->(){set seen=n} function fail(n)->(){call inner(n) set ignored=random(0,10) set seen=1/0} scene label start call fail(random(0,10)) "After""#);
    reach_call(&mut game);let before=bytes(&game);let publications=game.renderer.publications.get();let epochs=(game.runtime_epoch(),game.interface_epoch());let host=format!("{:?}",game.host_random);
    assert!(game.exec_silent(game.peek_statement().unwrap().clone()).is_err());
    assert_eq!(bytes(&game),before);assert_eq!(game.renderer.publications.get(),publications);assert_eq!((game.runtime_epoch(),game.interface_epoch()),epochs);assert_eq!(format!("{:?}",game.host_random),host);
}
#[test]
fn unit_engine1003_scene_preflight_refusal_cannot_publish_unit_rng_or_globals(){
    let mut game=engine(r#"init {set seen=0} function work(n)->(){set seen=n} scene label start call work(random(0,10)) "After""#);
    reach_call(&mut game);let before=bytes(&game);let publications=game.renderer.publications.get();game.renderer.fail.set(true);
    assert!(game.exec_silent(game.peek_statement().unwrap().clone()).is_err());assert_eq!(bytes(&game),before);assert_eq!(game.renderer.publications.get(),publications);
    let mut expected_random=game.state.random;let expected_value=game.functions.eval_with_random(&Expr::Call{name:"random".into(),args:vec![Expr::Int(0),Expr::Int(10)]},&game.lexical_vars_for_eval(),&mut expected_random).unwrap();
    game.renderer.fail.set(false);let pc=game.state.pc;game.exec_silent(game.peek_statement().unwrap().clone()).unwrap();assert_eq!(game.state.pc,pc+1);assert_eq!(game.renderer.publications.get(),publications+1);assert_eq!(game.state.vars["seen"],expected_value);assert_eq!(game.state.random,expected_random);
}
#[test]
fn unit_engine1003_real_unit_return_and_indexes_never_escape_to_caller_bindings(){
    let mut game=engine(r#"init {set index=100 set total=0} function work(n)->(){flow.for_loop(index,1,3){if index==2{return} set total=total+index} set total=999} label start call work(0) "After""#);
    reach_call(&mut game);let pc=game.state.pc;game.exec_silent(game.peek_statement().unwrap().clone()).unwrap();assert_eq!(game.state.vars["total"],Value::Int(1));assert_eq!(game.state.vars["index"],Value::Int(100));assert!(!game.state.vars.contains_key("n"));assert_eq!(game.state.pc,pc+1);assert!(game.state.call_stack.is_empty());
    let mut caller=engine(r#"init {set index=100 set total=0} function work(n)->(){set index=200 set total=total+n} scene label start local index=8 flow.for_loop(index,1,2){call work(index)} "After""#);
    caller.step_until_interaction().unwrap();assert_eq!(caller.state.vars["total"],Value::Int(3));assert_eq!(caller.state.vars["index"],Value::Int(200));assert_eq!(caller.state.scene_execution.as_ref().unwrap().active.as_ref().unwrap().locals["index"],Value::Int(2));
}
#[test]
fn unit_engine1003_unowned_scopes_persistent_and_init_controls_refuse_before_publication(){
    for source in [
        "init{set before=1} function bad()->(){set persistent.flag=true} label start",
        "init{set before=1 while true {call good()}} function good()->(){} label start",
        "init{set before=1 flow.sequence(){ {call good()} }} function good()->(){} label start",
    ] {
        let source=rvn_parser::parse_program(source).unwrap();let receipt=Receipt::default();let published=receipt.publications.clone();assert!(Engine::new(source.script,receipt,8).is_err());assert_eq!(published.get(),0);
    }
    for host in [true,false] {
        let mut source=rvn_parser::parse("init{set before=1} function good()->(){} label start").unwrap();
        let body=vec![Statement::UnitCall{name:"good".into(),arguments:vec![]}];
        source.push(if host {Statement::Handler{name:"event".into(),parameters:vec!["e".into()],body}} else {Statement::Screen{name:"ui".into(),parameters:vec![],body}});
        let receipt=Receipt::default();let published=receipt.publications.clone();assert!(Engine::new(source,receipt,8).is_err());assert_eq!(published.get(),0);
    }
    let mut game=engine("function good()->(){} label start");let before=bytes(&game);assert!(game.exec_silent(Statement::UnitReturn).is_err());assert_eq!(bytes(&game),before);
    // Reject real writes even when the candidate value equals the original
    // or a nested call later restores it. Neither is visible in a value diff.
    for (name,expression) in [("__input_mouse_clicked","__input_mouse_clicked"),("__input_mouse_x","__input_mouse_x"),("__input_mouse_y","__input_mouse_y"),("__input_key","__input_key")] {
        let source=format!(r#"init{{set changed=0}} function inner(n)->(){{set changed=n set {name}={expression}}} function work(n)->(){{call inner(n)}} scene label start local changed=88 call work(random(0,10)) "After""#);
        let mut game=engine(&source);game.update_input_state(Some("Q".into()),true,2.0,3.0);reach_call(&mut game);let before=bytes(&game);let input=format!("{:?}",game.input_state);let count=game.renderer.publications.get();
        assert!(game.exec_silent(game.peek_statement().unwrap().clone()).is_err(),"{name}");assert_eq!(bytes(&game),before);assert_eq!(format!("{:?}",game.input_state),input);assert_eq!(game.renderer.publications.get(),count);
    }
    let mut game=engine(r#"function work()->(){local old=__input_mouse_x set __input_mouse_x=99 set __input_mouse_x=old} label start call work() "After""#);game.update_input_state(None,false,2.0,3.0);reach_call(&mut game);let before=bytes(&game);assert!(game.exec_silent(game.peek_statement().unwrap().clone()).is_err());assert_eq!(bytes(&game),before);
    // Reads, unexecuted writes, ordinary globals and shadowing locals remain
    // valid. __input_key is ordinary when the actual owner injects no key.
    let mut game=engine(r#"function work(__input_mouse_y)->(){local __input_mouse_x=7 set __input_mouse_y=8 set local_seen=__input_mouse_x+__input_mouse_y set click_seen=__input_mouse_clicked if false{set __input_mouse_clicked=false} set __input_custom=4 set __input_key="ordinary"} label start call work(1) "After""#);
    game.update_input_state(None,true,2.0,3.0);reach_call(&mut game);let input=format!("{:?}",game.input_state);game.exec_silent(game.peek_statement().unwrap().clone()).unwrap();assert_eq!(game.state.vars["local_seen"],Value::Int(15));assert_eq!(game.state.vars["click_seen"],Value::Bool(true));assert_eq!(game.state.vars["__input_custom"],Value::Int(4));assert_eq!(game.state.vars["__input_key"],Value::Str("ordinary".into()));assert!(!game.state.vars.contains_key("__input_mouse_x"));assert!(!game.state.vars.contains_key("__input_mouse_y"));assert_eq!(format!("{:?}",game.input_state),input);
    let mut game=engine(r#"function work()->(){set key_seen=__input_key set y_seen=__input_mouse_y} label start call work() "After""#);game.update_input_state(Some("Q".into()),false,2.0,3.0);reach_call(&mut game);game.exec_silent(game.peek_statement().unwrap().clone()).unwrap();assert_eq!(game.state.vars["key_seen"],Value::Str("Q".into()));assert_eq!(game.state.vars["y_seen"],Value::Float(3.0));
}
#[test]
fn unit_engine1003_known_wrong_call_mode_and_source_edits_refuse_neutrally(){
    for source in ["function pure(){return 1} label start call pure(random(0,10))", "function good()->(){} label start call good(random(0,10))", "label start call absent(random(0,10))"] {assert!(Engine::new(rvn_parser::parse(source).unwrap(),Receipt::default(),8).is_err());}
    let mut game=engine(r#"function good()->(){set result=1} label start call good() "After""#);reach_call(&mut game);
    let definition=game.script.iter_mut().find_map(|s|if let Statement::UnitFunction{body,..}=s{Some(body)}else{None}).unwrap();definition.push(Statement::SetVar{name:"forged".into(),value:Expr::Int(1)});
    let before=bytes(&game);assert!(game.exec_silent(game.peek_statement().unwrap().clone()).is_err());assert_eq!(bytes(&game),before);
    let mut game=engine(r#"function good(n)->(){set result=n} label start call good(1) "After""#);reach_call(&mut game);
    assert!(!game.consumer_program.has_plans());game.consumer_program.validate_unit_emitted(&game.script).unwrap();
    if let Statement::UnitCall{arguments,..}=&mut game.script[game.state.pc]{arguments[0]=Expr::Int(2);}else{panic!();}
    let before=bytes(&game);assert!(game.exec_silent(game.peek_statement().unwrap().clone()).is_err());assert_eq!(bytes(&game),before);
    let mut game=engine(r#"function good(n)->(){} label start call good(random(0,10)) "After""#);reach_call(&mut game);
    game.state.vars.insert("unowned_class".into(),Value::WidgetClass(Box::new(None)));let before=bytes(&game);assert!(game.exec_silent(game.peek_statement().unwrap().clone()).is_err());assert_eq!(bytes(&game),before);
}
#[test]
fn unit_engine1003_units_cannot_mint_debug_save_or_deferred_init_authority(){
    let source=r#"function empty()->(){} label start "Before""#;let program=rvn_parser::parse_program(source).unwrap();let game=Engine::from_program(program.clone(),Receipt::default(),8).unwrap();let before=bytes(&game);
    assert!(game.debug_begin(super::super::DebugOperation::Statement,vec![]).is_err());assert!(game.debug_site_page(0).is_err());assert!(game.snapshot_for_save().is_err());assert!(game.capture_save15(&program,None,1,"refuse".into()).is_err());assert_eq!(bytes(&game),before);
    assert!(Engine::new_with_consumer_source(program.script,Receipt::default(),8,HashMap::new(),TextRuntimeContext::default(),None,true).is_err());
    let mut edited=Engine::new(rvn_parser::parse(source).unwrap(),Receipt::default(),8).unwrap();edited.script.retain(|statement|!matches!(statement,Statement::UnitFunction{..}));assert!(edited.snapshot_for_save().is_err());assert!(edited.debug_begin(super::super::DebugOperation::Statement,vec![]).is_err());
    let mut legacy=Engine::from_program(rvn_parser::parse_program(r#"function value(){return 7} scene label start set seen=value() "After""#).unwrap(),Receipt::default(),8).unwrap();legacy.enter_label("start").unwrap();assert!(legacy.snapshot_for_save().is_ok());assert!(legacy.debug_begin(super::super::DebugOperation::Statement,vec![]).is_ok());
}
#[test]
fn unit_engine1003_shared_budget_failure_with_caller_argument_rng_is_neutral(){
    let mut game=engine(r#"init {set seen=0} function recur(n)->(){if n>0{call recur(n-1)}else{set seen=1}} label start call recur(random(100,101)) "After""#);reach_call(&mut game);let before=bytes(&game);let error=game.exec_silent(game.peek_statement().unwrap().clone()).unwrap_err();
    assert_eq!(error.kind,crate::error::RuntimeErrorKind::EvalError(EvalError::ExecutionLimit{limit:"64 appels de fonction imbriqués"}));assert_eq!(bytes(&game),before);
    let mut admitted=engine(r#"init {set seen=0} function recur(n)->(){if n>0{call recur(n-1)}else{set seen=1}} label start call recur(63) "After""#);reach_call(&mut admitted);let pc=admitted.state.pc;admitted.exec_silent(admitted.peek_statement().unwrap().clone()).unwrap();assert_eq!(admitted.state.vars["seen"],Value::Int(1));assert_eq!(admitted.state.pc,pc+1);
}
