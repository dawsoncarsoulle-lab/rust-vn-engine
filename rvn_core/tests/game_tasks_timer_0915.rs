//! Private Timer slice qualification sources; execution belongs to Root.
//! Actual file programs, actual Game owner, no Scene or audio authority fabricated.
use rvn_core::{Engine,Renderer,renderer::SceneRenderBatch,types::SpriteState};
use rvn_parser::{Position,Transition,Hotspot};
use std::sync::{Arc,Mutex,atomic::AtomicBool};
use rvn_core::engine::Interaction;
use serde_json::Value;
use std::{path::PathBuf,sync::atomic::{AtomicUsize,Ordering}};
#[derive(Clone,Default)]
struct Host { reject:Arc<AtomicBool>, batches:Arc<Mutex<Vec<SceneRenderBatch>>> }
impl Renderer for Host {
 fn validate_scene_batch(&self,_:&SceneRenderBatch)->Result<(),String>{if self.reject.load(Ordering::SeqCst){Err("fixture rejects publication".into())}else{Ok(())}}
 fn publish_scene_batch(&mut self,b:SceneRenderBatch){self.batches.lock().unwrap().push(b);}
 fn set_background(&mut self,_:&str,_:&Transition){}
 fn show_sprite(&mut self,_:&str,_:Option<&str>,_:&Position,_:&Transition,_:Option<&SpriteState>){}
 fn hide_sprite(&mut self,_:&str,_:&Transition,_:&SpriteState){}
 fn move_sprite(&mut self,_:&str,_:&Position,_:&Transition,_:&SpriteState){}
 fn show_dialogue(&mut self,_:Option<&str>,_:&str){}
 fn show_choice(&mut self,_:&[String])->usize{panic!("not a Choice fixture")}
 fn show_imagemap(&mut self,_:&str,_:Option<&str>,_:&[Hotspot])->usize{panic!("not an Imagemap fixture")}
 fn music_play(&mut self,_:&str,_:&Transition,_:Option<&str>){}
 fn music_stop(&mut self,_:&Transition){}
 fn music_set_volume(&mut self,_:f32){}
 fn sfx_play(&mut self,_:&str,_:&Transition){}
 fn sfx_stop(&mut self,_:&str,_:&Transition){}
}
fn fixture(name:&str)->(rvn_parser::ParsedProgram,Engine<Host>){
 static NEXT:AtomicUsize=AtomicUsize::new(0);
 let root=PathBuf::from(std::env::var("RVN_GAME_TASK_CONTRACT_FIXTURES").expect("isolated writable fixture root"));
 let dir=root.join(format!("{}-{}-{}",name,std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)));
 std::fs::create_dir_all(&dir).unwrap();
 let input=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/game_tasks_timer_0915_fixtures").join(format!("{name}.rvn"));
 let file=dir.join("main.rvn"); std::fs::copy(input,file.clone()).unwrap();
 let program=rvn_parser::parse_file_program_with_uses(&file).unwrap();
 let game=Engine::from_program(program.clone(),Host::default(),8).unwrap(); (program,game)
}
fn state(game:&Engine<Host>)->Value{
 assert!(game.state.scene_execution.is_none(),"a real Game authority must not fabricate a Scene");
 assert!(game.state.game_audio.is_none(),"sprite/timer ownership must not create an audio registry");
 assert!(game.active_timer.is_none(),"Native timer must have exactly one saved owner");
 serde_json::to_value(&game.state).unwrap()
}
fn check_case(site:&str,kind:&str){
 let(_,mut game)=fixture(&format!("{site}_{kind}"));
 assert!(matches!(game.step_until_interaction().unwrap(),Some(Interaction::Dialogue{..})));
 let value=state(&game);let tasks=&value["game_tasks"];
 assert!(tasks.is_object(),"source-owned GameTasks must be present");
 match kind{
  "timer"=>{assert_eq!(tasks["tasks"]["timer"]["duration"],0.5);assert_eq!(tasks["tasks"]["timer"]["elapsed"],0.0);
   let old=game.timer_token().expect("real timer callback capability");
   game.tick_game_frame(0.25,Default::default()).unwrap();
   assert_eq!(state(&game)["game_tasks"]["tasks"]["timer"]["elapsed"],0.25);
   game.tick_game_frame(0.25,Default::default()).unwrap();
   assert!(state(&game)["game_tasks"]["tasks"]["timer"].is_null());
   assert!(!game.fire_timer(&old).unwrap(),"due callback cannot run twice");
   assert!(matches!(game.step_until_interaction().unwrap(),Some(Interaction::Dialogue{text,..})if text=="done"));},
  "timer_cancel"=>assert!(tasks["tasks"]["timer"].is_null()),
  "sprite_animate"=>{let list=tasks["resources"]["tasks"].as_array().unwrap();
   assert_eq!(list.iter().filter(|t|!t["effect"].is_null()).count(),1);
   assert!(list.iter().all(|t|t["key"]["owner"]["scope"]=="game"));
   game.tick_game_frame(0.125,Default::default()).unwrap();
   let after=state(&game);let list=after["game_tasks"]["resources"]["tasks"].as_array().unwrap();
   let effect=list.iter().find(|t|!t["effect"].is_null()).unwrap();assert_eq!(effect["effect"]["clock"]["elapsed"],0.125);},
  "sprite_stop"=>assert!(tasks["resources"]["tasks"].as_array().unwrap().iter().all(|t|t["effect"].is_null())),
  "sprite_effect"=>{assert_eq!(tasks["sprite_styles"]["hero"]["scale"],1.25);assert_eq!(tasks["sprite_styles"]["hero"]["flip_x"],true);assert_eq!(tasks["sprite_styles"]["hero"]["tint"],"#ff8800");},
  _=>panic!("unknown fixture family")
 }
}
macro_rules! case{($name:ident,$site:literal,$kind:literal)=>{#[test]fn $name(){check_case($site,$kind);}}}
case!(story_timer_owned_one_clock,"story","timer");
case!(init_timer_handoff_no_early_tick,"init","timer");
case!(story_timer_cancel_owned,"story","timer_cancel");
case!(init_timer_cancel_owned,"init","timer_cancel");
#[test]fn timer_replacement_revokes_old_callback(){
 let(_,mut game)=fixture("timer_renew");game.step_until_interaction().unwrap();let old=game.timer_token().unwrap();
 game.advance_dialogue().unwrap();game.step_until_interaction().unwrap();let new=game.timer_token().unwrap();
 assert_ne!(old.revision,new.revision);assert!(!game.fire_timer(&old).unwrap());
 assert_eq!(state(&game)["game_tasks"]["tasks"]["timer"]["duration"],0.75);
}
#[test]fn timer_cancellation_revokes_old_callback(){
 let(_,mut game)=fixture("timer_cancel_after_wait");game.step_until_interaction().unwrap();let old=game.timer_token().unwrap();
 game.advance_dialogue().unwrap();game.step_until_interaction().unwrap();assert!(game.timer_token().is_none());assert!(!game.fire_timer(&old).unwrap());
}
#[test]fn timer_save15_cold_restore_retains_elapsed_and_revokes_source_token(){
 let(program,mut game)=fixture("story_timer");game.step_until_interaction().unwrap();game.tick_game_frame(0.25,Default::default()).unwrap();
 let old=game.timer_token().unwrap();let save=game.capture_save15(&program,None,1,"timer".into()).unwrap();
 let wire=save.encode().unwrap();let decoded=rvn_core::save::consumer_checkpoint::Save15::decode(&wire).unwrap();
 let mut cold=Engine::from_program(program.clone(),Host::default(),8).unwrap();cold.restore_save15(&program,decoded).unwrap();
 assert_eq!(state(&cold)["game_tasks"]["tasks"]["timer"]["elapsed"],0.25);
 assert!(!cold.fire_timer(&old).unwrap());cold.tick_game_frame(0.25,Default::default()).unwrap();
 assert!(state(&cold)["game_tasks"]["tasks"]["timer"].is_null());
}
#[test]fn raw_legacy_timer_preserves_historical_route(){
 let(_,mut game)=fixture("legacy_only");game.step_until_interaction().unwrap();
 assert!(game.state.scene_execution.is_none());assert!(game.state.game_audio.is_none());
 assert!(serde_json::to_value(&game.state).unwrap().get("game_tasks").is_none());
 assert_eq!(game.active_timer.as_ref().unwrap().0,0.5);
}

fn refused_frame(name:&str){
 let(_,mut game)=fixture(name);game.step_until_interaction().unwrap();
 let before=serde_json::to_value(&game.state).unwrap();let epoch=game.runtime_epoch();let count=game.renderer.batches.lock().unwrap().len();
 game.renderer.reject.store(true,Ordering::SeqCst);assert!(game.tick_game_frame(0.5,Default::default()).is_err());
 assert_eq!(serde_json::to_value(&game.state).unwrap(),before);assert_eq!(game.runtime_epoch(),epoch);assert_eq!(game.renderer.batches.lock().unwrap().len(),count);
 game.renderer.reject.store(false,Ordering::SeqCst);game.tick_game_frame(0.125,Default::default()).unwrap();
}
#[test]fn timer_frame_renderer_refusal_is_atomic(){refused_frame("story_timer");}

fn source_game(name:&str,text:&str)->(rvn_parser::ParsedProgram,Engine<Host>){
 static NEXT:AtomicUsize=AtomicUsize::new(0);
 let root=PathBuf::from(std::env::var("RVN_GAME_TASK_CONTRACT_FIXTURES").unwrap()).join(format!("extra-{name}-{}-{}",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)));std::fs::create_dir_all(&root).unwrap();let file=root.join("main.rvn");std::fs::write(&file,text).unwrap();let program=rvn_parser::parse_file_program_with_uses(&file).unwrap();let game=Engine::from_program(program.clone(),Host::default(),8).unwrap();(program,game)
}
#[test]fn timer_invalid_delta_preserves_clock_state_and_publication(){
 let(_,mut game)=fixture("story_timer");game.step_until_interaction().unwrap();let before=serde_json::to_value(&game.state).unwrap();let count=game.renderer.batches.lock().unwrap().len();
 for delta in [-1.0,f64::NAN,f64::INFINITY,3600.01]{assert!(game.tick_game_frame(delta,Default::default()).is_err());assert_eq!(serde_json::to_value(&game.state).unwrap(),before);assert_eq!(game.renderer.batches.lock().unwrap().len(),count);}
}
#[test]fn timer_zero_is_consumed_once_on_real_game_frame(){
 let(_,mut game)=source_game("zero","label start\nflow.consume_v2(\"scope\",\"timer\"){timer 0 => jump done}\n\"waiting\"\nlabel done\n\"done\"\n");game.step_until_interaction().unwrap();assert!(game.tick_game_frame(0.0,Default::default()).unwrap().scene_changed);assert!(game.timer_token().is_none());assert_eq!(game.tick_game_frame(0.0,Default::default()).unwrap().callbacks,0);
}
#[test]fn timer_save15_rejects_missing_program_owned_authority(){
 let(program,mut game)=fixture("story_timer");game.step_until_interaction().unwrap();game.state.game_tasks=None;assert!(game.capture_save15(&program,None,1,"bad".into()).is_err());
}
#[test]fn timer_save15_rejects_foreign_registry_payload(){
 let(program,mut game)=fixture("story_timer");game.step_until_interaction().unwrap();game.state.game_tasks.as_mut().unwrap().resources.ensure_target(rvn_core::scene_tasks::TaskOwner::Game,rvn_core::scene_resources::TargetKind::Music,"music").unwrap();assert!(game.capture_save15(&program,None,1,"bad".into()).is_err());
}
#[test]fn timer_save15_rejects_missing_action_target(){
 let(program,mut game)=fixture("story_timer");game.step_until_interaction().unwrap();game.state.game_tasks.as_mut().unwrap().tasks.timer.as_mut().unwrap().action="jump missing".into();assert!(game.capture_save15(&program,None,1,"bad".into()).is_err());
}
#[test]fn timer_save15_relocated_source_preserves_partial_clock(){
 let(program,mut game)=fixture("story_timer");game.step_until_interaction().unwrap();game.tick_game_frame(0.125,Default::default()).unwrap();let saved=game.capture_save15(&program,None,1,"relocate".into()).unwrap();
 let(relocated,mut cold)=fixture("story_timer");assert_ne!(program.sources[0].path,relocated.sources[0].path);cold.restore_save15(&relocated,saved).unwrap();assert_eq!(state(&cold)["game_tasks"]["tasks"]["timer"]["elapsed"],0.125);
}
#[test]fn timer_historical_save_conversion_refuses_loss_of_owned_clock(){
 let(_,mut game)=fixture("story_timer");game.step_until_interaction().unwrap();assert!(rvn_core::save::SaveData::try_from_state(&game.state,1,"legacy".into(),"main".into()).is_err());let snapshot=game.snapshot_for_save().unwrap();assert!(rvn_core::save::SaveData::try_from_snapshot(&snapshot,1,"legacy".into(),"main".into()).is_err());
}
#[test]fn timer_call_returns_after_the_displayed_interaction(){
 let(_,mut game)=source_game("call","label start\nflow.consume_v2(\"scope\",\"timer\"){timer 0.5 => call task}\n\"before\"\n\"returned\"\njump done\nlabel task\n\"task\"\nreturn\nlabel done\n");game.step_until_interaction().unwrap();game.tick_game_frame(0.5,Default::default()).unwrap();assert!(matches!(game.step_until_interaction().unwrap(),Some(Interaction::Dialogue{text,..})if text=="task"));game.advance_dialogue().unwrap();assert!(matches!(game.step_until_interaction().unwrap(),Some(Interaction::Dialogue{text,..})if text=="returned"));
}
#[test]fn timer_real_rollback_receipt_restores_clock_and_revokes_old_callback(){
 let(_,mut game)=source_game("rollback","label start\nflow.consume_v2(\"scope\",\"timer\"){timer 5 => jump done}\n\"first\"\n\"second\"\nlabel done\n\"done\"\n");game.step_until_interaction().unwrap();game.tick_game_frame(0.25,Default::default()).unwrap();game.advance_dialogue().unwrap();game.step_until_interaction().unwrap();let old=game.timer_token().unwrap();let source=game.preview_game_audio_rollback_source().unwrap().unwrap();assert!(game.rollback_from_audio_source(&source).unwrap());assert_eq!(state(&game)["game_tasks"]["tasks"]["timer"]["elapsed"],0.0);assert!(!game.fire_timer(&old).unwrap());
}
#[test]fn timer_pure_frame_save15_resumes_actual_operation_and_ticks_once(){
 let(program,mut game)=fixture("story_timer");game.step_until_interaction().unwrap();let session=game.debug_begin(rvn_core::engine::DebugOperation::GameFrame{seconds:0.25,reports:Default::default(),audio:vec![]},vec![]).unwrap();
 let before=serde_json::to_value(&game.state).unwrap();let saved=game.capture_save15(&program,Some(&session),1,"real frame entry".into()).unwrap();let wire=saved.encode().unwrap();assert_eq!(serde_json::to_value(&game.state).unwrap(),before);
 let raw:serde_json::Value=serde_json::from_str(&wire).unwrap();assert!(raw["debugger"]["frame_operation"].is_object());assert!(raw["debugger"].get("trace").is_none());
 let mut cold=Engine::from_program(program.clone(),Host::default(),8).unwrap();let mut resumed=cold.restore_save15(&program,rvn_core::save::consumer_checkpoint::Save15::decode(&wire).unwrap()).unwrap().unwrap();
 let mut complete=false;for _ in 0..64{if cold.debug_step(&mut resumed,rvn_core::editor_debugger::StepKind::Continue).unwrap().0==rvn_core::engine::DebugProgress::Complete{complete=true;break;}}assert!(complete);assert_eq!(state(&cold)["game_tasks"]["tasks"]["timer"]["elapsed"],0.25);assert!(cold.debug_step(&mut resumed,rvn_core::editor_debugger::StepKind::Continue).is_err());
}
#[test]fn standalone_timer_keeps_existing_story_sprite_callbacks_admitted(){
 let(_,mut game)=source_game("coexist","label start\nhero.show(\"hero.png\") at center\nhero.animate(\"bounce\")\nhero.effect(scale:1.25)\nflow.consume_v2(\"scope\",\"timer\"){timer 0.5 => jump done}\nhero.stop_animation()\n\"waiting\"\nlabel done\n\"done\"\n");assert!(game.step_until_interaction().unwrap().is_some());assert!(game.state.game_tasks.is_some());assert!(game.state.sprites["hero"].visible);game.tick_game_frame(0.25,Default::default()).unwrap();
}
