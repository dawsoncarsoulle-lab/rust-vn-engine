use rvn_core::{Engine,Renderer,renderer::SceneRenderBatch,random::RandomState,types::SpriteState};
use rvn_parser::{Value,Statement,Position,Transition,Hotspot,ParsedProgram};
use std::sync::{Arc,Mutex,atomic::{AtomicBool,Ordering}};
#[derive(Clone,Default)]struct Host{batches:Arc<Mutex<Vec<SceneRenderBatch>>>,reject:Arc<AtomicBool>}
impl Renderer for Host{
 fn validate_scene_batch(&self,_:&SceneRenderBatch)->Result<(),String>{if self.reject.load(Ordering::SeqCst){Err("fixture preflight failure".into())}else{Ok(())}}
 fn publish_scene_batch(&mut self,b:SceneRenderBatch){self.batches.lock().unwrap().push(b);}
 fn supports_accessibility(&self)->bool{true}fn validate_accessibility_speech(&self,_:&rvn_ui::accessibility::SpeechRequest)->Result<(),String>{Ok(())}
 fn supports_composable_motion(&self)->bool{true}fn supports_video(&self)->bool{true}fn supports_layered_characters(&self)->bool{true}fn supports_programmable_ui(&self)->bool{true}
 fn supports_custom_canvas(&self)->bool{true}
 fn update_videos(&mut self,_:&[rvn_core::video::VideoView])->Result<(),String>{Ok(())}fn update_motions(&mut self,_:&[rvn_core::motion::MotionView])->Result<(),String>{Ok(())}fn update_interfaces(&mut self,_:&[rvn_ui::programmable::ScreenView])->Result<(),String>{Ok(())}fn update_layered_characters(&mut self,_:&[rvn_core::composition::LayeredView])->Result<(),String>{Ok(())}
 fn set_background(&mut self,_:&str,_:&Transition){}fn show_sprite(&mut self,_:&str,_:Option<&str>,_:&Position,_:&Transition,_:Option<&SpriteState>){}fn hide_sprite(&mut self,_:&str,_:&Transition,_:&SpriteState){}fn move_sprite(&mut self,_:&str,_:&Position,_:&Transition,_:&SpriteState){}fn show_dialogue(&mut self,_:Option<&str>,_:&str){}fn show_choice(&mut self,_:&[String])->usize{0}fn show_imagemap(&mut self,_:&str,_:Option<&str>,_:&[Hotspot])->usize{0}fn music_play(&mut self,_:&str,_:&Transition,_:Option<&str>){}fn music_stop(&mut self,_:&Transition){}fn music_set_volume(&mut self,_:f32){}fn sfx_play(&mut self,_:&str,_:&Transition){}fn sfx_stop(&mut self,_:&str,_:&Transition){}
}

use rvn_core::save::consumer_checkpoint::Save15;
use rvn_core::engine::{DebugOperation,DebugSession,DebugProgress};
use rvn_core::editor_debugger::{StepKind,Route};
#[path="private_pcm0370/frame_pcm_host.rs"]mod real_pcm0370;
fn state(e:&Engine<Host>)->serde_json::Value{serde_json::to_value(&e.state).unwrap()}
fn engine(s:&str)->(ParsedProgram,Engine<Host>){let p=rvn_parser::parse_program(s).unwrap();let mut e=Engine::from_program_with_debugger(p.clone(),Host::default(),24).unwrap();e.state.random=RandomState::seeded(29);(p,e)}
fn seek(e:&mut Engine<Host>){for _ in 0..32{if matches!(e.script.get(e.state.pc),Some(Statement::ScheduledConsumer{..})){return;}let mut session=e.debug_begin(DebugOperation::Statement,vec![]).unwrap();finish(e,&mut session);}panic!("actual entry bound")}
fn finish(e:&mut Engine<Host>,s:&mut DebugSession){for _ in 0..256{if e.debug_step(s,StepKind::Continue).unwrap().0==DebugProgress::Complete{return;}}panic!("continuation bound")}
fn text(scene:bool,captured:bool)->String{
 format!("function draw(){{local n=random(1,999999) return n/1000000.0}}\n{}flow.consume_v2(\"s\",\"c\"){{{} music.volume({})}}\n\"boundary\"\n",if scene{"scene label start\n"}else{""},if captured{"capture \"actual\"=draw()"}else{""},if captured{"__rvn_captured_v1(\"actual\")"}else{"draw()"})
}
#[test]fn dynamic_numeric_values_use_real_game_and_scene_owner_and_vn_clamp(){
 for scene in [false,true]{for (expr,expected) in [("level",0.25),("-level",0.0),("level+4.0",1.0),("1+zero",1.0)]{
  let source=format!("{}{} level=0.25\nset zero=0\nflow.consume_v2(\"s\",\"c\"){{music.volume({expr})}}\n\"boundary\"\n",if scene{"scene label start\n"}else{""},if scene{"local"}else{"set"});
  let(_,mut e)=engine(&source);e.step_until_interaction().unwrap();assert_eq!(e.state.music.volume,expected);assert_eq!(e.state.scene_execution.is_some(),scene);assert_eq!(e.state.game_audio.is_some(),!scene);
 }}
}
#[test]fn captured_and_direct_random_volume_evaluate_once_at_real_site(){
 for scene in [false,true]{for captured in [false,true]{let(_,mut e)=engine(&text(scene,captured));if scene{e.enter_label("start").unwrap();}seek(&mut e);let prior=e.renderer.batches.lock().unwrap().len();let mut session=e.debug_begin(DebugOperation::Statement,vec![]).unwrap();finish(&mut e,&mut session);
  let mut rng=RandomState::seeded(29);let value=rng.integer(1,999999).unwrap() as f32/1000000.0;assert_eq!(e.state.random,rng);assert_eq!(e.state.music.volume,value);assert_eq!(e.renderer.batches.lock().unwrap().len(),prior+1);
 }}
}
#[test]fn paused_actual_volume_input_cold_save15_resumes_once_without_audio_replay(){
 for scene in [false,true]{for captured in [false,true]{let source=text(scene,captured);let(p,mut warm)=engine(&source);if scene{warm.enter_label("start").unwrap();}seek(&mut warm);
  let mut sites=vec![];let mut offset=0;loop{let page=warm.debug_site_page(offset).unwrap();offset+=page.entries.len();sites.extend(page.entries);if offset==page.total{break;}}
  let point=sites.into_iter().find(|e|e.breakpoint.site.as_ref().is_some_and(|s|s.function.as_deref()==Some("draw")&&s.route==[Route::Statement(1)])).unwrap();assert!(point.origin.is_some());
  let before=state(&warm);let mut session=warm.debug_begin(DebugOperation::Statement,vec![point.breakpoint]).unwrap();assert_eq!(warm.debug_step(&mut session,StepKind::Continue).unwrap().0,DebugProgress::Stopped);assert_eq!(state(&warm),before);
  let bytes=warm.capture_save15(&p,Some(&session),57,"actual dynamic volume".into()).unwrap().encode().unwrap();finish(&mut warm,&mut session);
  let(_,mut cold)=engine(&source);let mut restored=cold.restore_save15(&p,Save15::decode(&bytes).unwrap()).unwrap().unwrap();finish(&mut cold,&mut restored);
  assert_eq!(state(&warm),state(&cold));let mut rng=RandomState::seeded(29);rng.integer(1,999999).unwrap();assert_eq!(cold.state.random,rng);assert_eq!(cold.renderer.batches.lock().unwrap().len(),2); // restore + one resumed command
 }}
}
#[test]fn dynamic_volume_renderer_and_type_refusals_preserve_frame_rng_and_publication(){
 for scene in [false,true]{for command in ["music.volume(draw())","music.volume(bad)","music.volume(1.0/zero)"]{
  let binding=if scene{"local"}else{"set"};let source=format!("function draw(){{local n=random(1,999999) return n/1000000.0}}\n{}{binding} bad=\"wrong\"\n{binding} zero=0.0\nflow.consume_v2(\"s\",\"c\"){{{command}}}\n\"boundary\"\n",if scene{"scene label start\n"}else{""});let(_,mut e)=engine(&source);if scene{e.enter_label("start").unwrap();}seek(&mut e);e.renderer.reject.store(command.contains("draw"),Ordering::SeqCst);
  let before=state(&e);let witness=e.debug_state_source().unwrap();let epoch=e.runtime_epoch();let count=e.renderer.batches.lock().unwrap().len();assert!(e.step_until_interaction().is_err());assert_eq!(state(&e),before);assert!(e.accepts_debug_state_source(&witness));assert_eq!(e.runtime_epoch(),epoch);assert_eq!(e.renderer.batches.lock().unwrap().len(),count);
 }}
}
#[test]fn nonfinite_authored_value_and_unbound_capture_refuse_before_publication(){
 let source="flow.consume_v2(\"s\",\"c\"){music.volume(__rvn_captured_v1(\"missing\"))}";
 let p=rvn_parser::parse_program(source).unwrap();assert!(Engine::from_program(p,Host::default(),8).is_err());
 let(_,mut e)=engine("music.volume(level)\n\"boundary\"");e.state.vars.insert("level".into(),Value::Float(f32::INFINITY));let before=state(&e);assert!(e.step_until_interaction().is_err());assert_eq!(state(&e),before);
}
#[test]fn changed_program_refuses_cold_continuation_atomically(){
 let source=text(false,true);let(p,mut warm)=engine(&source);seek(&mut warm);let mut session=warm.debug_begin(DebugOperation::Statement,vec![]).unwrap();assert_ne!(warm.debug_step(&mut session,StepKind::Into).unwrap().0,DebugProgress::Complete);let saved=warm.capture_save15(&p,Some(&session),57,"source refusal".into()).unwrap();
 let changed=source.replace("1000000.0","1000001.0");let(other,mut cold)=engine(&changed);let before=state(&cold);let witness=cold.debug_state_source().unwrap();assert!(cold.restore_save15(&other,saved).is_err());assert_eq!(state(&cold),before);assert!(cold.accepts_debug_state_source(&witness));assert!(cold.renderer.batches.lock().unwrap().is_empty());
}
#[test]fn legacy_literal_serde_and_unwrapped_expression_path_remain_distinct(){
 assert_eq!(serde_json::to_string(&Statement::MusicVolume{level:0.25}).unwrap(),"{\"MusicVolume\":{\"level\":0.25}}");
 let(_,mut e)=engine("set level=0.35\nmusic.volume(level)\n\"boundary\"");e.step_until_interaction().unwrap();assert_eq!(e.state.music.volume,0.35);assert!(e.state.game_audio.is_some());
}

#[test]fn actual_graph_float_variable_and_typed_producer_run_in_real_scene(){
 use rvn_graph::*;
 for (expression,expected) in [("level",0.25),("level+0.125",0.375)] {
  let p=rvn_parser::parse_program(&format!("scene label start\nlocal level=0.25\nmusic.volume({expression})\n\"boundary\"\n")).unwrap();let mut graphs=import_script(&p.script).unwrap();let g=graphs.iter_mut().find(|g|matches!(&g.kind,GraphKind::Label{name}if name=="start")).unwrap();g.enable_native_frame_preparation().unwrap();let compiled=prepare_native_frame_graph(g).unwrap();let(_,mut actual)=engine(&compiled.source);actual.step_until_interaction().unwrap();assert_eq!(actual.state.music.volume,expected);assert!(actual.state.scene_execution.is_some());assert!(actual.state.game_audio.is_none());
 }
}

#[test]fn dynamic_volume_updates_real_pcm_without_restart_and_survives_cold_save(){
 use real_pcm0370::PcmHost;
 let source="set level=0.25\nflow.consume_v2(\"play\",\"p\"){music.play(\"host.wav\") with fade(777)}\n\"before\"\nflow.consume_v2(\"volume\",\"v\"){capture \"actual-level\"=level music.volume(__rvn_captured_v1(\"actual-level\"))}\n\"after\"\n";
 let p=rvn_parser::parse_program(source).unwrap();let mut warm=Engine::from_program_with_debugger(p.clone(),PcmHost::default(),8).unwrap();warm.renderer.bind_actual(&warm.state,warm.instance_id(),warm.runtime_epoch()).unwrap();warm.step_until_interaction().unwrap();let target=warm.state.audio_resource_registry().unwrap().targets.iter().find(|t|t.identity.kind==rvn_core::scene_resources::TargetKind::Music).unwrap().identity.clone();warm.renderer.pull(&target,7).unwrap();let cursor=warm.renderer.cursor(&target).unwrap();
 warm.advance_dialogue().unwrap();warm.step_until_interaction().unwrap();assert_eq!(warm.state.music.volume,0.25);assert_eq!(warm.renderer.cursor(&target).unwrap(),cursor);assert!(warm.state.audio_resource_registry().unwrap().tasks.iter().all(|task|task.effect.is_none()||task.target!=target));
 let saved=warm.capture_save15(&p,None,57,"real dynamic PCM volume".into()).unwrap().encode().unwrap();let mut cold=Engine::from_program_with_debugger(p.clone(),PcmHost::default(),8).unwrap();cold.renderer.bind_actual(&cold.state,cold.instance_id(),cold.runtime_epoch()).unwrap();cold.restore_save15(&p,Save15::decode(&saved).unwrap()).unwrap();assert_eq!(cold.state.music.volume,0.25);assert_eq!(cold.renderer.cursor(&target).unwrap(),cursor);let padding=usize::from(cursor.next_channel);assert_eq!(cold.renderer.pull(&target,padding).unwrap(),vec![0;padding]);assert_eq!(cold.renderer.cursor(&target).unwrap(),cursor);assert_eq!(cold.renderer.pull(&target,5).unwrap(),warm.renderer.pull(&target,5).unwrap());
}
#[test]fn dynamic_audio_never_acquires_pure_or_host_owner_or_version1(){
 for source in ["function f(){flow.consume_v2(\"s\",\"c\"){music.volume(level)}}", "screen f(){flow.consume_v2(\"s\",\"c\"){music.volume(level)} return 1}"]{assert!(rvn_parser::parse_program(source).is_err(),"Pure-context grammar must refuse before runtime: {source}");}
 let source="handler f(){flow.consume_v2(\"s\",\"c\"){music.volume(level)}}";assert!(rvn_parser::parse_program(source).is_err(),"Existing Handler grammar refuses this narrative command before runtime");
 assert!(rvn_parser::parse("flow.consume_v1(\"s\",\"c\"){music.volume(level)}").is_err());
}
#[test]fn unwrapped_dynamic_input_cold_pause_uses_real_expression_and_program(){
 let source="function draw(){local n=random(1,999999) return n/1000000.0}\nmusic.volume(draw())\n\"boundary\"\n";
 let(p,mut warm)=engine(source);for _ in 0..8{if matches!(warm.script.get(warm.state.pc),Some(Statement::MusicVolumeValue{..})){break;}let mut s=warm.debug_begin(DebugOperation::Statement,vec![]).unwrap();finish(&mut warm,&mut s);}
 let mut sites=vec![];let mut offset=0;loop{let page=warm.debug_site_page(offset).unwrap();offset+=page.entries.len();sites.extend(page.entries);if offset==page.total{break;}}let point=sites.into_iter().find(|e|e.breakpoint.site.as_ref().is_some_and(|s|s.function.as_deref()==Some("draw")&&s.route==[Route::Statement(1)])).unwrap();let mut session=warm.debug_begin(DebugOperation::Statement,vec![point.breakpoint]).unwrap();assert_eq!(warm.debug_step(&mut session,StepKind::Continue).unwrap().0,DebugProgress::Stopped);let bytes=warm.capture_save15(&p,Some(&session),57,"unwrapped real expression".into()).unwrap().encode().unwrap();finish(&mut warm,&mut session);let(_,mut cold)=engine(source);let mut restored=cold.restore_save15(&p,Save15::decode(&bytes).unwrap()).unwrap().unwrap();finish(&mut cold,&mut restored);assert_eq!(state(&cold),state(&warm));
}
