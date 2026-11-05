//! Private sprite owner integration sources; no execution claimed.
//! No new runtime API is assumed. JSON paths express the proposed wire contract.
use rvn_core::{Engine,Renderer,renderer::SceneRenderBatch,types::SpriteState};
use rvn_parser::{Position,Transition,Hotspot,AnimationParam};
use std::sync::{Arc,Mutex,atomic::AtomicBool};
use rvn_core::engine::Interaction;
use serde_json::Value;
use std::{path::PathBuf,sync::atomic::{AtomicUsize,Ordering}};
#[derive(Clone,Default)]
struct Host { reject:Arc<AtomicBool>, batches:Arc<Mutex<Vec<SceneRenderBatch>>>, commands:Arc<Mutex<Vec<String>>> }
impl Renderer for Host {
 fn validate_scene_batch(&self,_:&SceneRenderBatch)->Result<(),String>{if self.reject.load(Ordering::SeqCst){Err("fixture rejects publication".into())}else{Ok(())}}
 fn publish_scene_batch(&mut self,b:SceneRenderBatch){self.batches.lock().unwrap().push(b);}
 fn animate_sprite(&mut self,id:&str,_:&str,_:&[AnimationParam]){self.commands.lock().unwrap().push(format!("animate:{id}"));}
 fn stop_sprite_animation(&mut self,id:&str){self.commands.lock().unwrap().push(format!("stop:{id}"));}
 fn set_sprite_effect(&mut self,id:&str,_:Option<bool>,_:Option<bool>,_:Option<f32>,_:Option<f32>,_:Option<&str>){self.commands.lock().unwrap().push(format!("effect:{id}"));}
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
 let input=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/game_tasks_sprites_0917_fixtures").join(format!("{name}.rvn"));
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
case!(story_sprite_animate_saved_clock,"story","sprite_animate");
case!(init_sprite_animate_handoff,"init","sprite_animate");
case!(story_sprite_stop_removes_only_animation,"story","sprite_stop");
case!(init_sprite_stop_retains_target,"init","sprite_stop");
case!(story_sprite_effect_retained_style,"story","sprite_effect");
case!(init_sprite_effect_retained_style,"init","sprite_effect");
fn refused_frame(name:&str){
 let(_,mut game)=fixture(name);game.step_until_interaction().unwrap();
 let before=serde_json::to_value(&game.state).unwrap();let epoch=game.runtime_epoch();let count=game.renderer.batches.lock().unwrap().len();
 game.renderer.reject.store(true,Ordering::SeqCst);assert!(game.tick_game_frame(0.5,Default::default()).is_err());
 assert_eq!(serde_json::to_value(&game.state).unwrap(),before);assert_eq!(game.runtime_epoch(),epoch);assert_eq!(game.renderer.batches.lock().unwrap().len(),count);
 game.renderer.reject.store(false,Ordering::SeqCst);game.tick_game_frame(0.125,Default::default()).unwrap();
}
#[test]fn sprite_frame_renderer_refusal_is_atomic(){refused_frame("story_sprite_animate");}

fn program_text(name:&str,source:&str)->rvn_parser::ParsedProgram{
 static NEXT:AtomicUsize=AtomicUsize::new(0);
 let root=PathBuf::from(std::env::var("RVN_GAME_TASK_CONTRACT_FIXTURES").expect("isolated writable fixture root"));
 let dir=root.join(format!("{name}-{}-{}",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)));
 std::fs::create_dir_all(&dir).unwrap();let file=dir.join("main.rvn");std::fs::write(&file,source).unwrap();
 rvn_parser::parse_file_program_with_uses(&file).unwrap()
}
fn owned_game(name:&str,source:&str)->(rvn_parser::ParsedProgram,Engine<Host>){
 let program=program_text(name,source);let game=Engine::from_program(program.clone(),Host::default(),8).unwrap();(program,game)
}
fn token<R:Renderer>(game:&Engine<R>)->rvn_core::scene_resources::ResourceToken{
 let task=game.state.game_tasks.as_ref().unwrap().resources.tasks.iter().find(|task|task.effect.is_some()).unwrap();
 rvn_core::scene_resources::ResourceToken{task:rvn_core::scene_tasks::TaskToken{runtime_epoch:game.runtime_epoch(),key:task.key.clone(),revision:task.revision},target:task.target.clone()}
}
const PORTABLE_SPRITE:&str="label start\nhero.show(\"hero.png\") at center\nflow.consume_v2(\"animation\",\"command\"){hero.animate(\"bounce\", duration:2.0, loop:true)}\nflow.consume_v2(\"style\",\"command\"){hero.effect(scale:1.25)}\nflow.consume_v2(\"timer\",\"command\"){timer 5 => jump done}\n\"first\"\n\"second\"\nlabel done\n\"done\"\n";
#[test]
fn sprite_timer_save15_cold_restore_keeps_one_saved_clock_and_revokes_epoch(){
 let(program,mut game)=owned_game("sprite-save",PORTABLE_SPRITE);game.step_until_interaction().unwrap();
 game.tick_game_frame(0.25,Default::default()).unwrap();let old=token(&game);let old_timer=game.timer_token().unwrap();
 let save=game.capture_save15(&program,None,1,"sprite".into()).unwrap();let wire=save.encode().unwrap();
 let decoded=rvn_core::save::consumer_checkpoint::Save15::decode(&wire).unwrap();
 let mut cold=Engine::from_program(program.clone(),Host::default(),8).unwrap();cold.restore_save15(&program,decoded).unwrap();
 assert_eq!(cold.state.game_tasks,game.state.game_tasks);assert_eq!(cold.state.sprites,game.state.sprites);
 assert!(!cold.complete_scene_effect(&old).unwrap());assert!(!cold.fire_timer(&old_timer).unwrap());
 cold.tick_game_frame(0.125,Default::default()).unwrap();let value=state(&cold);
 assert_eq!(value["game_tasks"]["tasks"]["timer"]["elapsed"],0.375);
 let resources=cold.state.game_tasks.as_ref().unwrap();assert_eq!(resources.resources.tasks.iter().find_map(|t|t.effect.as_ref()).unwrap().clock.elapsed,0.375);
 assert_eq!(resources.sprite_styles["hero"].scale,1.25);
}
#[test]
fn hide_and_show_renew_actual_target_and_revoke_old_effect(){
 let source="label start\nhero.show(\"hero.png\") at center\nflow.consume_v2(\"a\",\"command\"){hero.animate(\"bounce\")}\nflow.consume_v2(\"style\",\"command\"){hero.effect(scale:1.25)}\n\"first\"\nhero.hide()\nhero.show(\"replacement.png\") at center\nflow.consume_v2(\"b\",\"command\"){hero.animate(\"shake\")}\n\"second\"\n";
 let(_,mut game)=owned_game("sprite-recreate",source);game.step_until_interaction().unwrap();let old=token(&game);
 game.advance_dialogue().unwrap();game.step_until_interaction().unwrap();let fresh=token(&game);
 assert_ne!(old.target.incarnation,fresh.target.incarnation);assert!(!game.complete_scene_effect(&old).unwrap());
 assert!(game.state.game_tasks.as_ref().unwrap().sprite_styles.is_empty());assert_eq!(game.state.game_tasks.as_ref().unwrap().resources.targets.len(),1);
}
#[test]
fn visible_show_preserves_object_but_supersedes_its_animation_channel(){
 let source="label start\nhero.show(\"hero.png\") at center\nflow.consume_v2(\"a\",\"command\"){hero.animate(\"bounce\")}\n\"first\"\nhero.show(\"replacement.png\") at center\nflow.consume_v2(\"b\",\"command\"){hero.animate(\"shake\")}\n\"second\"\n";
 let(_,mut game)=owned_game("sprite-show",source);game.step_until_interaction().unwrap();let old=token(&game);
 game.advance_dialogue().unwrap();game.step_until_interaction().unwrap();let fresh=token(&game);
 assert_eq!(old.target,fresh.target);assert_ne!(old.task.revision,fresh.task.revision);assert!(!game.complete_scene_effect(&old).unwrap());
 assert_eq!(game.state.game_tasks.as_ref().unwrap().resources.tasks.len(),1);
}
#[test]
fn sprite_timer_real_rollback_restores_clocks_and_revokes_old_capabilities(){
 let(_,mut game)=owned_game("sprite-rollback",PORTABLE_SPRITE);game.step_until_interaction().unwrap();game.tick_game_frame(0.25,Default::default()).unwrap();
 game.advance_dialogue().unwrap();game.step_until_interaction().unwrap();let old=token(&game);let old_timer=game.timer_token().unwrap();
 let source=game.preview_game_audio_rollback_source().unwrap().unwrap();assert!(game.rollback_from_audio_source(&source).unwrap());
 assert!(!game.complete_scene_effect(&old).unwrap());assert!(!game.fire_timer(&old_timer).unwrap());
 let value=state(&game);assert_eq!(value["game_tasks"]["tasks"]["timer"]["elapsed"],0.0);
 assert_eq!(game.state.game_tasks.as_ref().unwrap().resources.tasks.iter().find_map(|t|t.effect.as_ref()).unwrap().clock.elapsed,0.0);
}
#[test]
fn pure_legacy_sprite_callbacks_keep_their_existing_route(){
 let(_,mut game)=owned_game("legacy-sprites","label start\nhero.show(\"hero.png\") at center\nhero.animate(\"bounce\")\nhero.effect(scale:1.25)\nhero.stop_animation()\ntimer 0.5 => jump done\n\"hold\"\nlabel done\n\"done\"\n");
 game.step_until_interaction().unwrap();assert!(game.state.game_tasks.is_none());assert!(game.state.scene_execution.is_none());assert!(game.state.game_audio.is_none());
 assert_eq!(*game.renderer.commands.lock().unwrap(),vec!["animate:hero","effect:hero","stop:hero"]);assert_eq!(game.active_timer.as_ref().unwrap().0,0.5);
}
#[test]
fn actual_startup_drop_rejection_commit_and_handoff_keep_sprite_timer_authority(){
 let program=program_text("startup-sprites","init{\nhero.show(\"hero.png\") at center\nflow.consume_v2(\"a\",\"command\"){hero.animate(\"bounce\",duration:2.0,loop:true)}\nflow.consume_v2(\"style\",\"command\"){hero.effect(scale:1.25)}\nflow.consume_v2(\"timer\",\"command\"){timer 5 => jump done}\n}\nlabel start\n\"hold\"\nlabel done\n\"done\"\n");
 let host=Host::default();let mut startup=Engine::prepare_debugger_startup(program.clone(),host.clone(),8,Default::default(),Default::default(),None).unwrap();
 assert_eq!(startup.program().sources,program.sources);let initial=serde_json::to_value(startup.state()).unwrap();
 let prepared=startup.prepare_step(rvn_core::editor_debugger::StepKind::Continue).unwrap();assert_eq!(serde_json::to_value(startup.state()).unwrap(),initial);drop(prepared);
 assert_eq!(serde_json::to_value(startup.state()).unwrap(),initial);assert!(host.batches.lock().unwrap().is_empty());
 let prepared=startup.prepare_step(rvn_core::editor_debugger::StepKind::Continue).unwrap();assert!(prepared.render_batch().is_some());host.reject.store(true,Ordering::SeqCst);
 assert!(startup.begin_prepared_step_commit(prepared).is_err());assert_eq!(serde_json::to_value(startup.state()).unwrap(),initial);assert!(host.batches.lock().unwrap().is_empty());
 host.reject.store(false,Ordering::SeqCst);
 for _ in 0..32{if startup.is_complete(){break;}let p=startup.prepare_step(rvn_core::editor_debugger::StepKind::Continue).unwrap();startup.begin_prepared_step_commit(p).unwrap().commit();}
 assert!(startup.is_complete());assert!(startup.state().scene_execution.is_none());assert!(startup.state().game_audio.is_none());
 let before=serde_json::to_value(startup.state()).unwrap();assert_eq!(before["game_tasks"]["tasks"]["timer"]["elapsed"],0.0);
 assert_eq!(before["game_tasks"]["sprite_styles"]["hero"]["scale"],1.25);
 let mut game=startup.into_engine().unwrap();assert_eq!(serde_json::to_value(&game.state).unwrap(),before);
 game.tick_game_frame(0.125,Default::default()).unwrap();assert_eq!(state(&game)["game_tasks"]["tasks"]["timer"]["elapsed"],0.125);
 assert_eq!(game.state.game_tasks.as_ref().unwrap().resources.tasks.iter().find_map(|t|t.effect.as_ref()).unwrap().clock.elapsed,0.125);
}

// Reuse the existing real WAV/Rodio Source adapter, shared gate and cursor
// verifier. This CPU boundary does not claim device playback or audible parity.
#[path="private_pcm0370/frame_pcm_host.rs"]
mod actual_pcm;
#[test]
fn sprite_timer_music_save15_uses_real_pcm_cursor_and_stop_preserves_audio(){
 let source="label start\nhero.show(\"hero.png\") at center\nflow.consume_v2(\"music\",\"command\"){music.play(\"host.wav\") with fade(1000)}\nflow.consume_v2(\"animation\",\"command\"){hero.animate(\"bounce\",duration:2.0,loop:true)}\nflow.consume_v2(\"style\",\"command\"){hero.effect(scale:1.25)}\nflow.consume_v2(\"timer\",\"command\"){timer 5 => jump done}\n\"first\"\nflow.consume_v2(\"stop\",\"command\"){hero.stop_animation()}\n\"second\"\nlabel done\n\"done\"\n";
 let program=program_text("sprite-actual-pcm",source);let mut game=Engine::from_program(program.clone(),actual_pcm::PcmHost::default(),8).unwrap();game.step_until_interaction().unwrap();
 assert!(game.state.scene_execution.is_none());assert!(game.state.game_audio.is_some());assert!(game.state.game_tasks.is_some());assert!(game.active_timer.is_none());
 game.renderer.bind_actual(&game.state,game.instance_id(),game.runtime_epoch()).unwrap();
 let music=rvn_core::audio_checkpoint::music_views(&game.state,game.runtime_epoch()).unwrap();assert_eq!(music.len(),1);let music_target=music[0].authority.target.clone();
 assert_eq!(game.renderer.pull(&music_target,7).unwrap().len(),7);assert_eq!(game.renderer.cursor(&music_target).unwrap().next_sample,7);
 game.tick_game_frame(0.25,Default::default()).unwrap();assert_eq!(game.renderer.cursor(&music_target).unwrap().next_sample,7,"A logical timer/sprite frame cannot consume a PCM Source");
 let old_sprite=token(&game);let old_timer=game.timer_token().unwrap();let save=game.capture_save15(&program,None,1,"actual-pcm".into()).unwrap();
 let decoded=rvn_core::save::consumer_checkpoint::Save15::decode(&save.encode().unwrap()).unwrap();
 let mut cold=Engine::from_program(program.clone(),actual_pcm::PcmHost::default(),8).unwrap();cold.restore_save15(&program,decoded).unwrap();
 cold.renderer.bind_actual(&cold.state,cold.instance_id(),cold.runtime_epoch()).unwrap();
 assert_eq!(cold.renderer.cursor(&music_target).unwrap().next_sample,7);assert_eq!(cold.state.game_tasks,game.state.game_tasks);
 assert!(!cold.complete_scene_effect(&old_sprite).unwrap());assert!(!cold.fire_timer(&old_timer).unwrap());
 let audio_before=serde_json::to_value(&cold.state.game_audio).unwrap();let styles=cold.state.game_tasks.as_ref().unwrap().sprite_styles.clone();
 cold.advance_dialogue().unwrap();cold.step_until_interaction().unwrap();
 assert_eq!(serde_json::to_value(&cold.state.game_audio).unwrap(),audio_before);assert_eq!(cold.renderer.cursor(&music_target).unwrap().next_sample,7);
 let tasks=cold.state.game_tasks.as_ref().unwrap();assert_eq!(tasks.sprite_styles,styles);assert_eq!(tasks.resources.targets.len(),1);assert!(tasks.resources.tasks.is_empty());
 assert_eq!(tasks.tasks.timer.as_ref().unwrap().elapsed,0.25);
 assert!(cold.state.game_audio.as_ref().unwrap().targets.iter().all(|target|target.identity.kind!=rvn_core::scene_resources::TargetKind::Sprite));
}

#[test]
fn native_timer_does_not_reject_or_adopt_historical_custom_renderer_animation(){
 let(_,mut game)=owned_game("historical-custom","label start\nhero.show(\"hero.png\") at center\nhero.animate(\"custom_renderer_animation\")\nflow.consume_v2(\"timer\",\"command\"){timer 5 => jump done}\n\"hold\"\nlabel done\n\"done\"\n");
 game.step_until_interaction().unwrap();assert_eq!(*game.renderer.commands.lock().unwrap(),vec!["animate:hero"]);
 assert!(game.state.game_tasks.as_ref().unwrap().resources.tasks.is_empty());game.tick_game_frame(0.125,Default::default()).unwrap();
 assert_eq!(game.state.game_tasks.as_ref().unwrap().tasks.timer.as_ref().unwrap().elapsed,0.125);
}
#[test]
fn native_sprite_descriptor_still_refuses_an_unknown_animation_without_callback(){
 let(_,mut game)=owned_game("native-custom-refusal","label start\nhero.show(\"hero.png\") at center\nflow.consume_v2(\"animation\",\"command\"){hero.animate(\"custom_renderer_animation\")}\n\"hold\"\n");
 let error=game.step_until_interaction().unwrap_err().to_string();assert!(error.contains("Unknown sprite animation"),"{error}");
 assert!(game.renderer.commands.lock().unwrap().is_empty());assert!(game.state.game_tasks.as_ref().unwrap().resources.tasks.is_empty());
}
#[test]
fn historical_sprite_effect_keeps_callback_and_bevy_tint_fallback_with_native_timer(){
 let(_,mut game)=owned_game("historical-tint","label start\nhero.show(\"hero.png\") at center\nhero.animate(\"bounce\")\nhero.effect(scale:1.25,tint:\"not-a-color\")\nhero.stop_animation()\nflow.consume_v2(\"timer\",\"command\"){timer 5 => jump done}\n\"hold\"\nlabel done\n\"done\"\n");
 game.step_until_interaction().unwrap();assert_eq!(*game.renderer.commands.lock().unwrap(),vec!["animate:hero","effect:hero","stop:hero"]);
 let owned=game.state.game_tasks.as_ref().unwrap();assert_eq!(owned.sprite_styles["hero"].scale,1.25);assert!(owned.sprite_styles["hero"].tint.is_none());assert!(owned.resources.tasks.is_empty());
}

#[test]
fn historical_timer_missing_target_is_delayed_portable_and_fails_atomically_at_callback(){
 let(program,mut game)=owned_game("raw-missing-target","label start\nhero.show(\"hero.png\") at center\nflow.consume_v2(\"animation\",\"command\"){hero.animate(\"bounce\",duration:2.0,loop:true)}\ntimer 5 => jump missing\n\"hold\"\n");
 game.step_until_interaction().unwrap();assert!(game.active_timer.is_none());assert!(game.state.game_tasks.as_ref().unwrap().historical_timer);
 game.tick_game_frame(0.125,Default::default()).unwrap();let save=game.capture_save15(&program,None,1,"historical".into()).unwrap();
 let decoded=rvn_core::save::consumer_checkpoint::Save15::decode(&save.encode().unwrap()).unwrap();let mut cold=Engine::from_program(program.clone(),Host::default(),8).unwrap();cold.restore_save15(&program,decoded).unwrap();
 assert!(cold.state.game_tasks.as_ref().unwrap().historical_timer);let before=serde_json::to_value(&cold.state).unwrap();let publications=cold.renderer.batches.lock().unwrap().len();
 let error=cold.tick_game_frame(5.0,Default::default()).unwrap_err().to_string();assert!(error.contains("missing"),"{error}");
 assert_eq!(serde_json::to_value(&cold.state).unwrap(),before);assert_eq!(cold.renderer.batches.lock().unwrap().len(),publications);
}
#[test]
fn historical_timer_origin_cannot_be_forged_without_a_real_raw_declaration(){
 let(program,mut game)=owned_game("forged-timer-origin",PORTABLE_SPRITE);game.step_until_interaction().unwrap();game.state.game_tasks.as_mut().unwrap().historical_timer=true;
 let error=game.capture_save15(&program,None,1,"forged".into()).unwrap_err().to_string();assert!(error.contains("Historical timer provenance"),"{error}");
}
#[test]
fn native_timer_renewal_and_cancellation_replace_historical_provenance_on_the_same_slot(){
 let(_,mut game)=owned_game("raw-native-renewal","label start\nhero.show(\"hero.png\") at center\nflow.consume_v2(\"animation\",\"command\"){hero.animate(\"bounce\",duration:2.0,loop:true)}\ntimer 5 => jump missing\n\"first\"\nflow.consume_v2(\"timer\",\"command\"){timer 0.5 => jump done}\n\"second\"\nflow.consume_v2(\"cancel\",\"command\"){timer cancel}\n\"third\"\nlabel done\n\"done\"\n");
 game.step_until_interaction().unwrap();let old=game.timer_token().unwrap();assert!(game.state.game_tasks.as_ref().unwrap().historical_timer);
 game.advance_dialogue().unwrap();game.step_until_interaction().unwrap();let fresh=game.timer_token().unwrap();assert_ne!(fresh.revision,old.revision);assert!(!game.state.game_tasks.as_ref().unwrap().historical_timer);assert!(!game.fire_timer(&old).unwrap());
 game.advance_dialogue().unwrap();game.step_until_interaction().unwrap();assert!(game.timer_token().is_none());assert!(!game.state.game_tasks.as_ref().unwrap().historical_timer);assert!(!game.fire_timer(&fresh).unwrap());
}

#[test]
fn historical_custom_callback_blocks_save15_only_until_actual_stop(){
 let source="label start\nhero.show(\"hero.png\") at center\nhero.animate(\"custom_renderer_animation\")\nflow.consume_v2(\"timer\",\"command\"){timer 5 => jump done}\n\"first\"\nhero.stop_animation()\n\"second\"\nlabel done\n\"done\"\n";
 let(program,mut game)=owned_game("custom-save-boundary",source);game.step_until_interaction().unwrap();
 assert_eq!(*game.renderer.commands.lock().unwrap(),vec!["animate:hero"]);
 assert!(game.state.game_tasks.as_ref().unwrap().legacy_sprite_effects["hero"].animation);
 assert!(game.capture_save15(&program,None,1,"unportable".into()).unwrap_err().to_string().contains("Historical sprite callback"));
 game.advance_dialogue().unwrap();game.step_until_interaction().unwrap();
 assert!(game.state.game_tasks.as_ref().unwrap().legacy_sprite_effects.is_empty());
 assert_eq!(*game.renderer.commands.lock().unwrap(),vec!["animate:hero","stop:hero"]);
 let save=game.capture_save15(&program,None,1,"stopped".into()).unwrap();
 let mut cold=Engine::from_program(program.clone(),Host::default(),8).unwrap();
 cold.restore_save15(&program,rvn_core::save::consumer_checkpoint::Save15::decode(&save.encode().unwrap()).unwrap()).unwrap();
 assert_eq!(cold.state.game_tasks,game.state.game_tasks);
}

#[test]
fn historical_timer_outside_portable_action_bound_keeps_one_mutually_exclusive_slot(){
 let label="T".repeat(4100);
 let source=format!("label start\nhero.show(\"hero.png\") at center\nflow.consume_v2(\"animation\",\"command\"){{hero.animate(\"bounce\",duration:2.0,loop:true)}}\nflow.consume_v2(\"first-timer\",\"command\"){{timer 5 => jump done}}\n\"native\"\ntimer 0.5 => jump {label}\n\"raw\"\nflow.consume_v2(\"renew-timer\",\"command\"){{timer 1 => jump done}}\n\"renewed\"\ntimer 0.5 => jump {label}\n\"raw for cancel\"\nflow.consume_v2(\"cancel-timer\",\"command\"){{timer cancel}}\n\"cancelled\"\nlabel {label}\n\"long done\"\nlabel done\n\"done\"\n");
 let(program,mut game)=owned_game("long-raw-timer",&source);game.step_until_interaction().unwrap();let old=game.timer_token().unwrap();
 let refused_publication=|game:&mut Engine<Host>|{
  let before=serde_json::to_value(&game.state).unwrap();let legacy=game.active_timer.clone();
  let source=game.debug_state_source().unwrap();let batches=game.renderer.batches.lock().unwrap().len();
  game.renderer.reject.store(true,Ordering::SeqCst);
  assert!(game.step_silent().is_err(),"actual renderer must refuse the prospective timer transaction");
  game.renderer.reject.store(false,Ordering::SeqCst);
  assert_eq!(serde_json::to_value(&game.state).unwrap(),before);assert_eq!(game.active_timer,legacy);
  assert_eq!(game.renderer.batches.lock().unwrap().len(),batches);assert!(game.accepts_debug_state_source(&source));
 };
 game.advance_dialogue().unwrap();game.step_until_interaction().unwrap();
 assert!(game.state.game_tasks.as_ref().unwrap().tasks.timer.is_none());assert_eq!(game.active_timer.as_ref().unwrap().0,0.5);assert!(!game.fire_timer(&old).unwrap());
 game.tick_game_frame(0.125,Default::default()).unwrap();assert_eq!(game.active_timer.as_ref().unwrap().2,0.125);
 assert!(game.capture_save15(&program,None,1,"raw active".into()).unwrap_err().to_string().contains("Historical timer"));
 game.advance_dialogue().unwrap();refused_publication(&mut game);game.step_until_interaction().unwrap();
 assert!(game.active_timer.is_none());assert!(!game.state.game_tasks.as_ref().unwrap().historical_timer);
 let renewed=game.timer_token().unwrap();assert_ne!(renewed.revision,old.revision);
 let save=game.capture_save15(&program,None,1,"renewed".into()).unwrap();
 let mut cold=Engine::from_program(program.clone(),Host::default(),8).unwrap();cold.restore_save15(&program,rvn_core::save::consumer_checkpoint::Save15::decode(&save.encode().unwrap()).unwrap()).unwrap();assert_eq!(cold.state.game_tasks,game.state.game_tasks);
 game.advance_dialogue().unwrap();game.step_until_interaction().unwrap();
 assert!(game.state.game_tasks.as_ref().unwrap().tasks.timer.is_none());assert_eq!(game.active_timer.as_ref().unwrap().0,0.5);assert!(!game.fire_timer(&renewed).unwrap());
 game.advance_dialogue().unwrap();refused_publication(&mut game);game.step_until_interaction().unwrap();
 assert!(game.active_timer.is_none());assert!(game.timer_token().is_none());assert!(!game.fire_timer(&renewed).unwrap());
}

#[test]
fn historical_timer_callback_in_compatibility_slot_is_one_atomic_game_frame(){
 let label="T".repeat(4100);
 let source=format!("label start\nhero.show(\"hero.png\") at center\nflow.consume_v2(\"animation\",\"command\"){{hero.animate(\"bounce\",duration:2.0,loop:true)}}\ntimer 0.5 => jump {label}\n\"hold\"\nlabel {label}\n\"done\"\n");
 let(_,mut game)=owned_game("raw-timer-atomic-frame",&source);game.step_until_interaction().unwrap();
 let before=serde_json::to_value(&game.state).unwrap();let legacy=game.active_timer.clone();let count=game.renderer.batches.lock().unwrap().len();
 game.renderer.reject.store(true,Ordering::SeqCst);assert!(game.tick_game_frame(0.5,Default::default()).is_err());
 assert_eq!(serde_json::to_value(&game.state).unwrap(),before);assert_eq!(game.active_timer,legacy);assert_eq!(game.renderer.batches.lock().unwrap().len(),count);
 game.renderer.reject.store(false,Ordering::SeqCst);let outcome=game.tick_game_frame(0.5,Default::default()).unwrap();assert_eq!(outcome.callbacks,1);assert!(outcome.published);assert!(game.active_timer.is_none());assert!(game.timer_token().is_none());
 assert!(matches!(game.step_until_interaction().unwrap(),Some(Interaction::Dialogue{text,..})if text=="done"));
}

#[test]
fn historical_decimal_overflow_is_runtime_compatible_but_not_a_false_cold_save(){
 let huge="9".repeat(50)+".0";
 let source=format!("label start\nhero.show(\"hero.png\") at center\nflow.consume_v2(\"animation\",\"command\"){{hero.animate(\"bounce\",duration:2.0,loop:true)}}\ntimer {huge} => jump done\n\"hold\"\nlabel done\n\"done\"\n");
 let(program,mut game)=owned_game("raw-timer-overflow",&source);game.step_until_interaction().unwrap();
 assert!(game.active_timer.as_ref().unwrap().0.is_infinite());assert!(game.state.game_tasks.as_ref().unwrap().tasks.timer.is_none());
 game.tick_game_frame(0.125,Default::default()).unwrap();assert_eq!(game.active_timer.as_ref().unwrap().2,0.125);
 assert!(game.capture_save15(&program,None,1,"overflow source".into()).is_err());
}

#[test]
fn historical_nonfinite_sprite_callback_does_not_fail_a_native_timer_program(){
 let huge="9".repeat(50)+".0";
 let source=format!("label start\nhero.show(\"hero.png\") at center\nhero.effect(scale:{huge},rotation:{huge})\nflow.consume_v2(\"timer\",\"command\"){{timer 5 => jump done}}\n\"hold\"\nlabel done\n\"done\"\n");
 let(program,mut game)=owned_game("raw-effect-overflow",&source);game.step_until_interaction().unwrap();
 assert_eq!(*game.renderer.commands.lock().unwrap(),vec!["effect:hero"]);
 let tasks=game.state.game_tasks.as_ref().unwrap();assert!(tasks.legacy_sprite_effects["hero"].scale);assert!(tasks.legacy_sprite_effects["hero"].rotation);assert!(tasks.sprite_styles["hero"].scale.is_finite());
 game.tick_game_frame(0.125,Default::default()).unwrap();assert!(game.capture_save15(&program,None,1,"nonportable renderer state".into()).is_err());
}

#[test]
fn cold_save_cannot_smuggle_an_unowned_historical_sprite_callback(){
 let(program,mut game)=owned_game("forged-legacy-sprite",PORTABLE_SPRITE);game.step_until_interaction().unwrap();
 let save=game.capture_save15(&program,None,1,"portable".into()).unwrap();let mut raw:serde_json::Value=serde_json::from_str(&save.encode().unwrap()).unwrap();
 raw["story"]["base"]["game_tasks"]["legacy_sprite_effects"]=serde_json::json!({"hero":{"animation":true,"scale":false,"rotation":false,"untracked_style":false}});
 let forged=rvn_core::save::consumer_checkpoint::Save15::decode(&serde_json::to_string(&raw).unwrap()).unwrap();let before=serde_json::to_value(&game.state).unwrap();
 assert!(game.restore_save15(&program,forged).err().expect("Forged callback must refuse").to_string().contains("Historical sprite callback"));assert_eq!(serde_json::to_value(&game.state).unwrap(),before);
}

#[test]
fn unrelated_raw_sprite_identity_is_not_rejected_by_a_native_timer(){
 let id="H".repeat(129);
 let source=format!("label start\n{id}.show(\"hero.png\") at center\nflow.consume_v2(\"timer\",\"command\"){{timer 5 => jump done}}\n\"first\"\nlabel done\n\"done\"\n");
 let(program,mut game)=owned_game("unrelated-raw-sprite-id",&source);game.step_until_interaction().unwrap();
 assert!(game.state.sprites[&id].visible);assert!(game.state.game_tasks.as_ref().unwrap().resources.targets.is_empty());
 game.tick_game_frame(0.125,Default::default()).unwrap();
 let save=game.capture_save15(&program,None,1,"unrelated sprite".into()).unwrap();let mut cold=Engine::from_program(program.clone(),Host::default(),8).unwrap();cold.restore_save15(&program,rvn_core::save::consumer_checkpoint::Save15::decode(&save.encode().unwrap()).unwrap()).unwrap();
 assert_eq!(cold.state.sprites,game.state.sprites);assert_eq!(cold.state.game_tasks,game.state.game_tasks);
}

#[test]
fn raw_sprite_callbacks_outside_native_alias_bound_are_preserved_and_bounded_at_save(){
 let id="H".repeat(129);
 let source=format!("label start\n{id}.show(\"hero.png\") at center\n{id}.animate(\"bounce\")\nflow.consume_v2(\"timer\",\"command\"){{timer 5 => jump done}}\n\"first\"\n{id}.stop_animation()\n{id}.hide()\n\"second\"\nlabel done\n\"done\"\n");
 let(program,mut game)=owned_game("raw-sprite-id-callback",&source);game.step_until_interaction().unwrap();
 assert_eq!(*game.renderer.commands.lock().unwrap(),vec![format!("animate:{id}")]);assert!(game.state.game_tasks.as_ref().unwrap().resources.targets.is_empty());
 assert!(game.capture_save15(&program,None,1,"active raw callback".into()).unwrap_err().to_string().contains("Historical sprite callback"));
 game.advance_dialogue().unwrap();game.step_until_interaction().unwrap();assert_eq!(*game.renderer.commands.lock().unwrap(),vec![format!("animate:{id}"),format!("stop:{id}")]);
 assert!(game.state.game_tasks.as_ref().unwrap().legacy_sprite_effects.is_empty());game.capture_save15(&program,None,1,"stopped".into()).unwrap();
}

#[test]
fn actual_native_sprite_target_still_requires_a_valid_resource_identity(){
 let id="H".repeat(129);
 let source=format!("label start\n{id}.show(\"hero.png\") at center\nflow.consume_v2(\"animation\",\"command\"){{{id}.animate(\"bounce\")}}\n\"hold\"\n");
 let(_,mut game)=owned_game("native-sprite-id-guard",&source);let error=game.step_until_interaction().unwrap_err().to_string();assert!(error.contains("Invalid logical resource alias"),"{error}");assert!(game.renderer.commands.lock().unwrap().is_empty());
}

fn timer_call_before_next_statement(historical:bool){
 let timer=if historical{"timer 0.5 => call task"}else{"flow.consume_v2(\"timer\",\"command\"){timer 0.5 => call task}"};
 let source=format!("label start\nhero.show(\"hero.png\") at center\nflow.consume_v2(\"animation\",\"command\"){{hero.animate(\"bounce\")}}\n{timer}\n\"first\"\nset between=7\n\"returned\"\njump done\nlabel task\n\"task\"\nreturn\nlabel done\n\"done\"\n");
 let(_,mut game)=owned_game(if historical{"raw-call-before-step"}else{"native-call-before-step"},&source);game.step_until_interaction().unwrap();game.advance_dialogue().unwrap();
 assert!(matches!(game.peek_statement(),Some(rvn_parser::Statement::SetVar{name,..})if name=="between"));
 game.tick_game_frame(0.5,Default::default()).unwrap();assert!(matches!(game.step_until_interaction().unwrap(),Some(Interaction::Dialogue{text,..})if text=="task"));
 game.advance_dialogue().unwrap();assert!(matches!(game.step_until_interaction().unwrap(),Some(Interaction::Dialogue{text,..})if text=="returned"));
 let expected=(!historical).then_some(rvn_parser::Value::Int(7));assert_eq!(game.state.vars.get("between"),expected.as_ref());
}
#[test]fn historical_timer_call_retains_its_existing_next_pc_rule(){timer_call_before_next_statement(true);}
#[test]fn native_timer_call_retains_an_unexecuted_statement_after_return(){timer_call_before_next_statement(false);}

#[test]
fn warm_save15_load_replaces_the_historical_slot_only_after_real_preflight(){
 let label="T".repeat(4100);
 let source=format!("label start\nflow.consume_v2(\"timer\",\"command\"){{timer 5 => jump done}}\n\"native\"\ntimer 0.5 => jump {label}\n\"raw\"\nlabel {label}\n\"long done\"\nlabel done\n\"done\"\n");
 let(program,mut game)=owned_game("warm-replace-raw-timer",&source);game.step_until_interaction().unwrap();let old=game.timer_token().unwrap();let saved=game.capture_save15(&program,None,1,"native timer".into()).unwrap();let wire=saved.encode().unwrap();
 game.advance_dialogue().unwrap();game.step_until_interaction().unwrap();game.tick_game_frame(0.125,Default::default()).unwrap();assert!(game.active_timer.is_some());
 let before=serde_json::to_value(&game.state).unwrap();let legacy=game.active_timer.clone();
 game.renderer.reject.store(true,Ordering::SeqCst);assert!(game.restore_save15(&program,rvn_core::save::consumer_checkpoint::Save15::decode(&wire).unwrap()).is_err());assert_eq!(serde_json::to_value(&game.state).unwrap(),before);assert_eq!(game.active_timer,legacy);
 game.renderer.reject.store(false,Ordering::SeqCst);game.restore_save15(&program,rvn_core::save::consumer_checkpoint::Save15::decode(&wire).unwrap()).unwrap();
 assert!(game.active_timer.is_none());assert_eq!(game.state.game_tasks.as_ref().unwrap().tasks.timer.as_ref().unwrap().elapsed,0.0);assert!(!game.fire_timer(&old).unwrap());
}
