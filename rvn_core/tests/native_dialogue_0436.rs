use rvn_core::{Engine,TerminalRenderer,Interaction};
use rvn_core::random::RandomState;
use rvn_parser::Value;
use rvn_core::engine::{DebugOperation,DebugProgress,DebugSession,DebugOutcome};
use rvn_core::editor_debugger::{StepKind,Route};
use rvn_core::save::consumer_checkpoint::Save15;
const SOURCE:&str=r#"function producer(){local n=random(1,999999) return n}
scene label start
flow.consume_v2("dialogue_scope","dialogue_consumer") {
 capture "value_from_source"=producer()
 "[__rvn_captured_v1(\"value_from_source\")]"
}
"done"
"#;
fn program()->rvn_parser::ParsedProgram {rvn_parser::parse_program(SOURCE).unwrap()}
fn engine(p:rvn_parser::ParsedProgram)->Engine<TerminalRenderer> {let mut e=Engine::from_program_with_debugger(p,TerminalRenderer,16).unwrap();e.state.random=RandomState::seeded(11);e}
fn state<R:rvn_core::Renderer>(e:&Engine<R>)->serde_json::Value {serde_json::to_value(&e.state).unwrap()}
fn finish<R:rvn_core::Renderer>(e:&mut Engine<R>,s:&mut DebugSession)->DebugOutcome {
 for _ in 0..128 {let(progress,result)=e.debug_step(s,StepKind::Continue).unwrap();if progress==DebugProgress::Complete{return result.unwrap();}}panic!("Dialogue continuation exceeded targeted drain bound")
}
#[test]
fn source_authored_dialogue_retains_real_scene_ports_without_graph_cache() {
 let mut e=engine(program());let mut expected=RandomState::seeded(11);let value=expected.integer(1,999999).unwrap();
 assert_eq!(e.step_until_interaction().unwrap(),Some(Interaction::Dialogue{character:None,text:value.to_string()}));assert_eq!(e.state.random,expected);
 let payload=e.state.scene_execution.as_ref().unwrap().display.as_ref().unwrap().consumer_dialogue.as_ref().unwrap();
 assert_eq!(payload.values,vec![Value::Int(value)]);assert_eq!(payload.pc,e.state.pc);assert!(matches!(payload.owner,rvn_core::consumer_schedule::ConsumerOwner::Scene{..}));
 let before=state(&e);e.step_until_interaction().unwrap();e.current_interaction().unwrap();e.displayed_interaction().unwrap();assert_eq!(state(&e),before);
}
#[test]
fn reached_dialogue_save15_cold_reveal_and_next_preserve_ports_rng_and_stale_input() {
 let p=program();let mut warm=engine(p.clone());let shown=warm.step_until_interaction().unwrap().unwrap();let Interaction::Dialogue{text,..}=&shown else{panic!("actual Dialogue missing")};
 let source=warm.display_reveal_source().unwrap();let input=warm.preview_input_source().unwrap();
 let reveal=rvn_core::display_reveal::RevealCheckpoint::observed(text,1,true,0.25,0.5).unwrap();assert!(warm.record_display_reveal(&source,reveal.clone()).unwrap());
 let bytes=warm.capture_save15(&p,None,3,"Reached native dialogue".into()).unwrap().encode().unwrap();
 let before=state(&warm);let mut cold=engine(p.clone());assert!(cold.restore_save15(&p,Save15::decode(&bytes).unwrap()).unwrap().is_none());
 assert_eq!(state(&cold),before);assert_eq!(cold.displayed_interaction().unwrap(),Some(shown));
 assert_eq!(cold.state.scene_execution.as_ref().unwrap().display.as_ref().unwrap().reveal.as_ref(),Some(&reveal));assert!(!cold.accepts_display_reveal_source(&source));
 assert!(cold.debug_begin(DebugOperation::Advance(input),vec![]).is_err());
 let restored=state(&cold);cold.step_until_interaction().unwrap();assert_eq!(state(&cold),restored);
 warm.advance_dialogue().unwrap();cold.advance_dialogue().unwrap();assert_eq!(state(&cold),state(&warm));
 assert_eq!(cold.step_until_interaction().unwrap(),Some(Interaction::Dialogue{character:None,text:"done".into()}));warm.step_until_interaction().unwrap();assert_eq!(cold.state.random,warm.state.random);
}
#[test]
fn paused_real_producer_save15_cold_resumes_without_replaying_random_or_effects() {
 let p=program();let mut warm=engine(p.clone());warm.enter_label("start").unwrap();let before=state(&warm);
 let mut sites=vec![];let mut offset=0;loop {let page=warm.debug_site_page(offset).unwrap();offset+=page.entries.len();sites.extend(page.entries);if offset==page.total{break;}}
 let breakpoint=sites.into_iter().find(|entry|entry.breakpoint.site.as_ref().is_some_and(|s|s.function.as_deref()==Some("producer")&&s.route==[Route::Statement(1)])).expect("return site must be derived from actual source catalog");assert!(breakpoint.origin.is_some());
 let mut session=warm.debug_begin(DebugOperation::Statement,vec![breakpoint.breakpoint]).unwrap();assert_eq!(warm.debug_step(&mut session,StepKind::Continue).unwrap().0,DebugProgress::Stopped);assert_eq!(state(&warm),before);
 let snapshot=session.snapshot();assert!(snapshot.candidate);assert!(snapshot.frames.iter().any(|f|f.function.as_deref()==Some("producer")));assert!(snapshot.origin.is_some());
 let bytes=warm.capture_save15(&p,Some(&session),4,"Paused real producer".into()).unwrap().encode().unwrap();
 let expected=finish(&mut warm,&mut session);let mut cold=engine(p.clone());let mut resumed=cold.restore_save15(&p,Save15::decode(&bytes).unwrap()).unwrap().unwrap();let actual=finish(&mut cold,&mut resumed);
 match(expected,actual){(DebugOutcome::Interaction(a),DebugOutcome::Interaction(b))=>assert_eq!(a,b),_=>panic!("wrong narrative continuation route")};assert_eq!(state(&cold),state(&warm));
 let mut expected=RandomState::seeded(11);expected.integer(1,999999).unwrap();assert_eq!(cold.state.random,expected);
}
#[test]
fn changed_frozen_source_and_forged_ports_or_owner_refuse_restore_atomically() {
 let p=program();let mut warm=engine(p.clone());warm.step_until_interaction().unwrap();let bytes=warm.capture_save15(&p,None,4,"Bound native dialogue".into()).unwrap().encode().unwrap();
 let changed=rvn_parser::parse_program(&SOURCE.replace("999999","999998")).unwrap();let mut cold=engine(changed.clone());let before=state(&cold);let epoch=cold.runtime_epoch();assert!(cold.restore_save15(&changed,Save15::decode(&bytes).unwrap()).is_err());assert_eq!(state(&cold),before);assert_eq!(cold.runtime_epoch(),epoch);
 for mode in ["ports","owner","missing","frame","binding","position"] {
  let mut data:serde_json::Value=serde_json::from_str(&bytes).unwrap();let payload=&mut data["story"]["base"]["scene_execution"]["display"]["consumer_dialogue"];assert!(payload.is_object());
  match mode {"ports"=>payload["values"]=serde_json::json!([]),"owner"=>payload["owner"]=serde_json::json!({"kind":"scene","activation":999999}),"missing"=>*payload=serde_json::Value::Null,"frame"=>data["story"]["frames"]=serde_json::json!([]),"binding"=>payload["program"]=serde_json::to_value([1u8;32]).unwrap(),"position"=>payload["pc"]=serde_json::json!(payload["pc"].as_u64().unwrap()+1),_=>unreachable!()};
  let mut cold=engine(p.clone());let before=state(&cold);let epoch=cold.runtime_epoch();let result=Save15::decode(&data.to_string()).and_then(|save|cold.restore_save15(&p,save));assert!(result.is_err(),"{mode} unexpectedly admitted");assert_eq!(state(&cold),before);assert_eq!(cold.runtime_epoch(),epoch);
 }
}
#[test]
fn legacy_dialogue_none_fields_omit_and_old_save_versions_remain_unowned() {
 let mut e=engine(rvn_parser::parse_program("scene label start\n\"legacy\"\n").unwrap());let initial=serde_json::to_string(&e.state).unwrap();let decoded:rvn_core::GameState=serde_json::from_str(&initial).unwrap();assert_eq!(serde_json::to_string(&decoded).unwrap(),initial);e.step_until_interaction().unwrap();let json=serde_json::to_string(&e.state).unwrap();assert!(!json.contains("consumer_dialogue"));let roundtrip:rvn_core::GameState=serde_json::from_str(&json).unwrap();assert_eq!(serde_json::to_value(&roundtrip).unwrap(),state(&e));
}

#[test]
fn malformed_capture_and_unsupported_lexical_dialogue_refuse_before_initialization() {
 let forward=SOURCE.replace("capture \"value_from_source\"=producer()","capture \"value_from_source\"=__rvn_captured_v1(\"not_yet_bound\")");
 assert!(Engine::from_program(rvn_parser::parse_program(&forward).unwrap(),TerminalRenderer,8).is_err());
 for prefix in ["function unsafe()", "screen unsafe()", "handler unsafe()", "init"] {
  let source=format!("{prefix} {{flow.consume_v2(\"scope\",\"consumer\"){{\"unsupported lexical display\"}}}}\nscene label start\n\"done\"");
  match rvn_parser::parse_program(&source) {Ok(parsed)=>assert!(Engine::from_program(parsed,TerminalRenderer,8).is_err(),"{prefix} cannot invent a story display owner"),Err(_)=>assert!(prefix!="init","Init parsed source must reach Core owner admission")};
 }
}
#[test]
fn native_dialogue_next_reaches_actual_native_choice_and_selected_destination_once() {
 let source=SOURCE.replace("\"done\"",r#"flow.consume_v2("choice_scope","choice_consumer"){choice {"proceed" => {jump target}}}
scene label target
"arrived""#);
 let p=rvn_parser::parse_program(&source).unwrap();let mut e=engine(p.clone());e.step_until_interaction().unwrap();let rng=e.state.random;e.advance_dialogue().unwrap();
 assert_eq!(e.step_until_interaction().unwrap(),Some(Interaction::Choice{options:vec!["proceed".into()]}));assert_eq!(e.state.random,rng);
 let bytes=e.capture_save15(&p,None,5,"Native choice after dialogue".into()).unwrap().encode().unwrap();let mut cold=engine(p.clone());cold.restore_save15(&p,Save15::decode(&bytes).unwrap()).unwrap();cold.submit_choice(0).unwrap();
 assert_eq!(cold.step_until_interaction().unwrap(),Some(Interaction::Dialogue{character:None,text:"arrived".into()}));
}
#[derive(Default)]struct GateHost {reject:bool,batches:Vec<rvn_core::renderer::SceneRenderBatch>}
impl rvn_core::Renderer for GateHost {
 fn validate_scene_batch(&self,_:&rvn_core::renderer::SceneRenderBatch)->Result<(),String>{if self.reject{Err("test real preflight refusal".into())}else{Ok(())}}
 fn publish_scene_batch(&mut self,b:rvn_core::renderer::SceneRenderBatch){self.batches.push(b)}
 fn set_background(&mut self,_:&str,_:&rvn_parser::Transition){}
 fn show_sprite(&mut self,_:&str,_:Option<&str>,_:&rvn_parser::Position,_:&rvn_parser::Transition,_:Option<&rvn_core::SpriteState>){}
 fn hide_sprite(&mut self,_:&str,_:&rvn_parser::Transition,_:&rvn_core::SpriteState){}
 fn move_sprite(&mut self,_:&str,_:&rvn_parser::Position,_:&rvn_parser::Transition,_:&rvn_core::SpriteState){}
 fn show_dialogue(&mut self,_:Option<&str>,_:&str){}
 fn show_choice(&mut self,_:&[String])->usize{0}
 fn show_imagemap(&mut self,_:&str,_:Option<&str>,_:&[rvn_parser::Hotspot])->usize{0}
 fn music_play(&mut self,_:&str,_:&rvn_parser::Transition,_:Option<&str>){}
 fn music_stop(&mut self,_:&rvn_parser::Transition){}
 fn music_set_volume(&mut self,_:f32){}
 fn sfx_play(&mut self,_:&str,_:&rvn_parser::Transition){}
 fn sfx_stop(&mut self,_:&str,_:&rvn_parser::Transition){}
}
#[test]
fn renderer_refusal_keeps_real_state_rng_history_source_and_next_atomic() {
 let p=program();let mut e=Engine::from_program_with_debugger(p,GateHost::default(),16).unwrap();e.state.random=RandomState::seeded(11);e.enter_label("start").unwrap();
 e.renderer.reject=true;let before=state(&e);let source=e.debug_state_source().unwrap();let published=e.renderer.batches.len();assert!(e.step_until_interaction().is_err());assert_eq!(state(&e),before);assert!(e.accepts_debug_state_source(&source));assert_eq!(e.renderer.batches.len(),published);
 e.renderer.reject=false;e.step_until_interaction().unwrap();e.renderer.reject=true;let before=state(&e);let source=e.debug_state_source().unwrap();let published=e.renderer.batches.len();assert!(e.advance_dialogue().is_err());assert_eq!(state(&e),before);assert!(e.accepts_debug_state_source(&source));assert_eq!(e.renderer.batches.len(),published);
}

#[test]
fn retained_dialogue_rollback_uses_original_ports_and_never_replays_random() {
 let mut e=engine(program());let shown=e.step_until_interaction().unwrap();let reached=state(&e);let stale=e.preview_input_source().unwrap();e.advance_dialogue().unwrap();assert!(e.rollback());assert_eq!(state(&e),reached);assert_eq!(e.displayed_interaction().unwrap(),shown);assert!(e.debug_begin(DebugOperation::Advance(stale),vec![]).is_err());let before=state(&e);e.step_until_interaction().unwrap();assert_eq!(state(&e),before);
}
#[test]
fn historical_save1_through14_refuse_nested_native_payload_even_without_frame() {
 let mut e=engine(program());e.step_until_interaction().unwrap();assert!(rvn_core::save::SaveData::try_from_state(&e.state,1,"wrong format".into(),"story".into()).is_err());
 let mut stripped=e.state.clone();stripped.scene_execution.as_mut().unwrap().active.as_mut().unwrap().consumer_frame=None;
 for version in 1..=14 {let mut dto=rvn_core::save::SaveData::from_state(&stripped,1,"nested payload".into(),"story".into());dto.format_version=version;assert!(dto.validate_checkpoint_shape().is_err(),"format{version} must not lose native retained values");}
}

#[test]
fn actual_game_and_prescene_game_dialogue_use_game_frame_and_cold_retained_values() {
 let game=SOURCE.replace("scene label start\n","");
 for source in [game.clone(),format!("{game}\nscene label tail\n\"tail\"\n")] {
  let p=rvn_parser::parse_program(&source).unwrap();let mut e=engine(p.clone());let shown=e.step_until_interaction().unwrap();assert!(shown.is_some());assert!(e.state.scene_execution.as_ref().is_none_or(|execution|execution.active.is_none()));assert_eq!(e.state.consumer_frame.as_ref().unwrap().owner,rvn_core::consumer_schedule::ConsumerOwner::Game);
  let bytes=e.capture_save15(&p,None,6,"Actual Game dialogue".into()).unwrap().encode().unwrap();let mut cold=engine(p.clone());cold.restore_save15(&p,Save15::decode(&bytes).unwrap()).unwrap();assert_eq!(state(&cold),state(&e));assert_eq!(cold.displayed_interaction().unwrap(),shown);let before=state(&cold);cold.step_until_interaction().unwrap();assert_eq!(state(&cold),before);
 }
}

#[test]
fn nested_native_dialogue_inside_actual_choice_has_authored_position_and_cold_next() {
 let source=r#"function producer(){local n=random(1,999999) return n}
scene label start
flow.consume_v2("outer_scope","outer_choice") {choice {"enter" => {
 flow.consume_v2("inner_scope","inner_dialogue") {capture "v"=producer() "[__rvn_captured_v1(\"v\")]"}
}}}
"after child"
"#;
 let p=rvn_parser::parse_program(source).unwrap();let mut warm=engine(p.clone());assert!(matches!(warm.step_until_interaction().unwrap(),Some(Interaction::Choice{..})));let stale=warm.preview_input_source().unwrap();warm.submit_choice(0).unwrap();let shown=warm.step_until_interaction().unwrap();assert!(matches!(warm.script[warm.state.pc],rvn_parser::Statement::ScheduledConsumer{..}));
 assert!(warm.debug_current_snapshot(&[]).unwrap().position.origin.unwrap().route.iter().any(|part|matches!(part,rvn_core::program::AstRoutePart::Choice{option:0,..})));assert!(warm.debug_begin(DebugOperation::Choice{source:stale,index:0},vec![]).is_err());
 let save=warm.capture_save15(&p,None,10,"Actual nested native dialogue".into()).unwrap();let mut cold=engine(p.clone());cold.restore_save15(&p,save).unwrap();assert_eq!(cold.displayed_interaction().unwrap(),shown);assert_eq!(cold.state.random,warm.state.random);warm.advance_dialogue().unwrap();cold.advance_dialogue().unwrap();assert_eq!(cold.step_until_interaction().unwrap(),Some(Interaction::Dialogue{character:None,text:"after child".into()}));warm.step_until_interaction().unwrap();assert_eq!(state(&cold),state(&warm));
}
