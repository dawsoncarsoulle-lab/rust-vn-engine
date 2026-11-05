use rvn_core::{Engine,Renderer,renderer::SceneRenderBatch,random::RandomState,types::SpriteState};
use rvn_parser::{Value,Name,Statement,Position,Transition,Hotspot};
use std::sync::{Arc,Mutex,atomic::{AtomicBool,Ordering}};
#[derive(Clone,Default)]struct Host{batches:Arc<Mutex<Vec<SceneRenderBatch>>>,reject:Arc<AtomicBool>}
impl Renderer for Host{
 fn validate_scene_batch(&self,_:&SceneRenderBatch)->Result<(),String>{if self.reject.load(Ordering::SeqCst){Err("fixture preflight failure".into())}else{Ok(())}}
 fn publish_scene_batch(&mut self,b:SceneRenderBatch){self.batches.lock().unwrap().push(b);}
 fn supports_accessibility(&self)->bool{true} fn validate_accessibility_speech(&self,_:&rvn_ui::accessibility::SpeechRequest)->Result<(),String>{Ok(())}
 fn supports_composable_motion(&self)->bool{true}fn supports_video(&self)->bool{true}
 fn set_background(&mut self,_:&str,_:&Transition){}fn show_sprite(&mut self,_:&str,_:Option<&str>,_:&Position,_:&Transition,_:Option<&SpriteState>){}fn hide_sprite(&mut self,_:&str,_:&Transition,_:&SpriteState){}fn move_sprite(&mut self,_:&str,_:&Position,_:&Transition,_:&SpriteState){}fn show_dialogue(&mut self,_:Option<&str>,_:&str){}fn show_choice(&mut self,_:&[String])->usize{0}fn show_imagemap(&mut self,_:&str,_:Option<&str>,_:&[Hotspot])->usize{0}fn music_play(&mut self,_:&str,_:&Transition,_:Option<&str>){}fn music_stop(&mut self,_:&Transition){}fn music_set_volume(&mut self,_:f32){}fn sfx_play(&mut self,_:&str,_:&Transition){}fn sfx_stop(&mut self,_:&str,_:&Transition){}
}
const TABLE:&str=r#"__rvn_string_table_text_name(__rvn_literal_name("asset:Table"),"hello")"#;
const OUTS:&str=r#"[__rvn_terminal_v1("i","valid"),__rvn_terminal_v1("i","table_id"),__rvn_terminal_v1("i","key")]"#;
fn engine(source:&str)->Engine<Host>{let mut e=Engine::from_program(rvn_parser::parse_program(source).unwrap(),Host::default(),8).unwrap();e.state.random=RandomState::seeded(4);e}
fn expected(draws:usize)->RandomState{let mut r=RandomState::seeded(4);for _ in 0..draws{r.integer(1,1).unwrap();}r}
fn state(e:&Engine<Host>)->serde_json::Value{serde_json::to_value(&e.state).unwrap()}
#[test]fn real_scene_condition_is_evaluated_once_with_nested_sequence_set_and_local(){
 let source=include_str!("fixtures/consume_v2_scene_condition.rvn");let mut e=engine(source);e.step_silent().unwrap();assert_eq!(e.state.vars["result"],Value::Int(12));assert_eq!(e.state.scene_execution.as_ref().unwrap().active.as_ref().unwrap().locals["order"],Value::Int(12));assert_eq!(e.state.random,expected(1));assert!(!e.state.vars.contains_key("bad"));
}
#[test]fn actual_init_and_fresh_constructor_bind_source_before_nested_consumers_execute(){
 let source=include_str!("fixtures/consume_v2_init_source.rvn");let mut e=engine(source);assert_eq!(e.state.vars["order"],Value::Int(1));assert_eq!(e.state.vars["retained"],Value::Name(Name::new("asset:Table").unwrap()));let frame=e.state.consumer_frame.clone().unwrap();
 let fresh=e.fresh(Host::default(),8).unwrap();assert_eq!(fresh.state.consumer_frame.as_ref().unwrap().program,frame.program);assert_eq!(fresh.state.vars["retained"],e.state.vars["retained"]);e.step_silent().unwrap();assert_eq!(e.state.scene_execution.as_ref().unwrap().active.as_ref().unwrap().locals["result"],Value::Int(1));
}
#[test]fn ast_only_host_and_unadapted_menu_reject_before_any_init_or_publication(){
 for (source,reason) in [(include_str!("fixtures/consume_v2_unowned_screen.rvn"),"TargetIdentity"),(include_str!("fixtures/consume_v2_unowned_handler.rvn"),"TargetIdentity"),(include_str!("fixtures/consume_v2_unowned_menu.rvn"),"trusted interface event source")]{
  let host=Host::default();let receipt=host.clone();
  // Screen/Handler from a real ParsedProgram now have a verified source
  // adapter. A hand-built/AST-only declaration still supplies no authority.
  let result=if reason=="TargetIdentity" {Engine::new(rvn_parser::parse(source).unwrap(),host,8)} else {Engine::from_program(rvn_parser::parse_program(source).unwrap(),host,8)};
  let error=match result{Err(e)=>e,Ok(_)=>panic!("declaration must never authenticate an owner")};assert!(error.to_string().contains(reason),"must reject owner before invalid Init: {error}");assert!(receipt.batches.lock().unwrap().is_empty());
 }
}
#[test]fn selected_scene_if_gathers_only_selected_child_and_keeps_same_activation_terminals(){
 for selected in [true,false]{let source=r#"scene label start
flow.consume_v2("root","choose") {capture "p"=random(1,1) inspect "i"=TABLE_INPUT if SELECTED {flow.consume_v2("root","yes") {capture "p"=random(1,1) inspect "i"=__rvn_invariant_text("literal") set result=OUTS_INPUT}} else {flow.consume_v2("root","no") {capture "p"=random(1,1) inspect "i"=__rvn_invariant_text("literal") set result=OUTS_INPUT}}}
"done""#.replace("TABLE_INPUT",TABLE).replace("SELECTED",if selected{"true"}else{"false"}).replace("OUTS_INPUT",OUTS);let mut e=engine(&source);e.step_silent().unwrap();assert_eq!(e.state.random,expected(2));assert_eq!(e.state.vars["result"],Value::List(vec![Value::Bool(false),Value::Name(Name::new("asset:Table").unwrap()),Value::Str("hello".into())]));assert!(e.state.scene_execution.as_ref().unwrap().active.as_ref().unwrap().consumer_frame.is_some());}
 let source=r#"scene label start
flow.consume_v2("root","choose") {if true {flow.consume_v2("root","yes") {set result=7}} else {flow.consume_v2("root","no") {capture "p"=random(1,1) set result=1/0}}}
"done""#;let mut e=engine(source);e.step_silent().unwrap();assert_eq!(e.state.vars["result"],Value::Int(7));assert_eq!(e.state.random,expected(0));
}
#[test]fn sequence_children_are_real_instructions_in_authored_order_with_true_source_ranges(){
 let source=format!(r#"scene label start
set order=0
flow.consume_v2("root","sequence") {{flow.sequence() {{{{flow.consume_v2("root","first") {{capture "p"=random(1,1) inspect "i"={TABLE} set order=order*10+1}}}} {{flow.consume_v2("root","second") {{capture "p"=random(1,1) inspect "i"=__rvn_invariant_text("literal") set order=order*10+2}}}}}}}}
flow.consume_v2("root","last") {{inspect "i"=__rvn_invariant_text("literal") set result={OUTS}}}
"done""#);let program=rvn_parser::parse_program(&source).unwrap();let mut e=engine(&source);let prepared=rvn_core::program::PreparedNarrative::from_parsed(program.clone()).unwrap();let consumers=prepared.script.iter().enumerate().filter_map(|(pc,s)|if let Statement::ScheduledConsumer{consumer,..}=s{Some((pc,consumer))}else{None}).collect::<Vec<_>>();assert_eq!(consumers.iter().map(|(_,c)|c.as_str()).collect::<Vec<_>>(),vec!["sequence","first","second","last"]);for(pc,_)in consumers{let o=&prepared.origins[pc];assert!(o.span.is_some());assert!(o.generated.is_none());assert!(matches!(o.route.first(),Some(rvn_core::program::AstRoutePart::Root{..})));}
 e.step_silent().unwrap();assert_eq!(e.state.vars["order"],Value::Int(12));assert_eq!(e.state.random,expected(2));assert!(matches!(&e.state.vars["result"],Value::List(v)if v[1]==Value::Name(Name::new("asset:Table").unwrap())));
}
#[test]fn lexical_if_and_sequence_drop_parent_context_before_child_gather(){
 let source=format!(r#"function choose() {{local order=0 flow.consume_v2("root","choose") {{capture "parent"=true inspect "i"={TABLE} if __rvn_captured_v1("parent") {{flow.consume_v2("root","sequence") {{flow.sequence() {{{{flow.consume_v2("root","a") {{capture "p"=random(1,1) local order=1}}}} {{flow.consume_v2("root","b") {{capture "p"=random(1,1) inspect "i"=__rvn_invariant_text("literal") local order=order*10+2}}}}}}}}}} else {{return 1/0}}}} flow.consume_v2("root","return") {{return [order,__rvn_invariant_text("untouched")]}}}}
scene label start
set result=choose()
"done""#);let mut normal=engine(&source);normal.step_silent().unwrap();assert!(matches!(&normal.state.vars["result"],Value::List(v)if v[0]==Value::Int(12)));assert_eq!(normal.state.random,expected(2));let mut debug=engine(&source);debug.enter_label("start").unwrap();let mut session=debug.debug_begin(rvn_core::engine::DebugOperation::Statement,vec![]).unwrap();let mut done=false;for _ in 0..100{if debug.debug_step(&mut session,rvn_core::editor_debugger::StepKind::Continue).unwrap().0==rvn_core::engine::DebugProgress::Complete{done=true;break;}}assert!(done);assert_eq!(debug.state.vars["result"],normal.state.vars["result"]);assert_eq!(debug.state.random,normal.state.random);
}
#[test]fn nested_forgery_and_parent_capture_escape_reject_before_init_effects(){
 for body in [r#"if true {flow.consume_v2("root","child") {set result=__rvn_captured_v1("parent")}}"#,r#"if true {set result=__rvn_captured_v1("parent")}"#]{let source=format!("init {{set prior=99}}\nscene label start\nflow.consume_v2(\"root\",\"parent\") {{capture \"parent\"=7 {body}}}\n\"done\"");assert!(Engine::from_program(rvn_parser::parse_program(&source).unwrap(),Host::default(),8).is_err());}
 assert!(rvn_parser::parse(r#"flow.consume_v1("root","old"){if true {set x=1}}"#).is_err());assert!(rvn_parser::parse(r#"flow.consume_v2("root","loop"){flow.break()}"#).is_err());
}
#[test]fn nested_handbuilt_version_forgery_rejects_before_any_prior_assignment_or_draw(){
 use rvn_core::{consumer_schedule::ConsumerFrame,eval::FunctionLibrary};use std::collections::HashMap;
 let mut script=rvn_parser::parse(r#"set prior=99 flow.consume_v2("root","parent"){if true {flow.consume_v2("root","child"){capture "draw"=random(1,1) set result=__rvn_captured_v1("draw")}}}"#).unwrap();
 let Statement::ScheduledConsumer{body,..}=&mut script[1]else{panic!()};let Statement::If{then_branch,..}=&mut body[0]else{panic!()};let Statement::ScheduledConsumer{version,..}=&mut then_branch[0]else{panic!()};*version=99;
 let mut random=RandomState::seeded(4);let before=random;let mut frame=ConsumerFrame::default();let frame_before=frame.clone();let vars=HashMap::from([("prior".into(),Value::Int(7))]);let result=FunctionLibrary::default().execute_in_consumer_frame(&script,&vars,&mut random,&mut frame);assert!(result.is_err());assert_eq!(random,before);assert_eq!(frame,frame_before);assert_eq!(vars["prior"],Value::Int(7));
}
#[test]fn if_renderer_failure_preserves_frame_rng_variables_and_pc_before_branch_entry(){
 let source=format!(r#"scene label start
"before"
flow.consume_v2("root","choose") {{capture "p"=random(1,1) inspect "i"={TABLE} if true {{flow.consume_v2("root","child") {{set result=7}}}}}}
"done""#);let mut e=engine(&source);e.step_until_interaction().unwrap();e.advance_dialogue().unwrap();let before=state(&e);let batches=e.renderer.batches.lock().unwrap().len();e.renderer.reject.store(true,Ordering::SeqCst);assert!(e.step_until_interaction().is_err());assert_eq!(state(&e),before);assert_eq!(e.renderer.batches.lock().unwrap().len(),batches);e.renderer.reject.store(false,Ordering::SeqCst);e.step_until_interaction().unwrap();assert_eq!(e.state.random,expected(3));assert_eq!(e.state.vars["result"],Value::Int(7));
}
#[test]fn legacy_native_if_and_sequence_have_same_behavior_and_recomputable_source_binding(){
 let source=r#"label start
set order=0
flow.consume_v2("root","sequence") {flow.sequence() {{flow.consume_v2("root","a"){set order=1}} {flow.consume_v2("root","branch"){capture "p"=true if __rvn_captured_v1("p"){flow.consume_v2("root","b"){set order=order*10+2}} else {flow.consume_v2("root","bad"){set order=1/0}}}}}}
"done""#;let mut e=engine(source);e.step_until_interaction().unwrap();assert_eq!(e.state.vars["order"],Value::Int(12));
}
