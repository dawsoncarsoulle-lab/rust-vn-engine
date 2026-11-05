//! Targeted Core Startup route; no Bevy World, device or native UI claim.
use rvn_core::{Engine,Renderer,renderer::SceneRenderBatch,random::RandomState,types::SpriteState};
use rvn_parser::{Value,Position,Transition,Hotspot};
use std::sync::{Arc,Mutex,atomic::{AtomicBool,Ordering}};
#[derive(Clone,Default)]struct Host{batches:Arc<Mutex<Vec<SceneRenderBatch>>>,reject:Arc<AtomicBool>}
impl Renderer for Host{
 fn validate_scene_batch(&self,_:&SceneRenderBatch)->Result<(),String>{if self.reject.load(Ordering::SeqCst){Err("fixture preflight failure".into())}else{Ok(())}}
 fn publish_scene_batch(&mut self,b:SceneRenderBatch){self.batches.lock().unwrap().push(b);}
 fn supports_programmable_ui(&self)->bool{true}
 fn set_background(&mut self,_:&str,_:&Transition){}fn show_sprite(&mut self,_:&str,_:Option<&str>,_:&Position,_:&Transition,_:Option<&SpriteState>){}fn hide_sprite(&mut self,_:&str,_:&Transition,_:&SpriteState){}fn move_sprite(&mut self,_:&str,_:&Position,_:&Transition,_:&SpriteState){}fn show_dialogue(&mut self,_:Option<&str>,_:&str){}fn show_choice(&mut self,_:&[String])->usize{0}fn show_imagemap(&mut self,_:&str,_:Option<&str>,_:&[Hotspot])->usize{0}fn music_play(&mut self,_:&str,_:&Transition,_:Option<&str>){}fn music_stop(&mut self,_:&Transition){}fn music_set_volume(&mut self,_:f32){}fn sfx_play(&mut self,_:&str,_:&Transition){}fn sfx_stop(&mut self,_:&str,_:&Transition){}
}
#[path="private_pcm0370/frame_pcm_host.rs"]mod pcm;
fn startup(commands:&[&str])->(rvn_core::engine::EngineStartup<pcm::PcmHost>,pcm::PcmHost,RandomState){
 let mut source="init {\n".to_owned();
 for(i,command)in commands.iter().enumerate(){source.push_str(&format!("flow.consume_v2(\"init_audio_{i}\",\"command_{i}\"){{capture \"draw_{i}\"=random(1,999999) {command}}}\n"));}
 source.push_str("set after=7}\nscene label start\n\"Ready\"\n");
 let base=std::path::PathBuf::from(std::env::var("RVN_STARTUP_STEP_FIXTURES").unwrap());
 static NEXT:std::sync::atomic::AtomicUsize=std::sync::atomic::AtomicUsize::new(0);
 let root=base.join(format!("audio-init-{}-{}",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)));std::fs::create_dir(&root).unwrap();let file=root.join("main.rvn");std::fs::write(&file,source).unwrap();
 let program=rvn_parser::parse_file_program_with_uses(file).unwrap();let host=pcm::PcmHost::default();
 let owner=Engine::prepare_debugger_startup(program,host.clone(),8,Default::default(),Default::default(),None).unwrap();
 host.bind_actual(owner.state(),owner.snapshot().instance,owner.snapshot().epoch).unwrap();let random=owner.state().random;
 (owner,host,random)
}
fn finish(owner:&mut rvn_core::engine::EngineStartup<pcm::PcmHost>){
 for _ in 0..128{if owner.is_complete(){return;}owner.step(rvn_core::editor_debugger::StepKind::Continue).unwrap();}
 panic!("Real Init did not complete within its bounded command count")
}
#[test]
fn seven_native_audio_commands_execute_in_real_init_once_before_engine_handoff(){
 let commands=[r#"music.play("host.wav")"#,r#"music.volume("0.3")"#,r#"sfx.play("host.wav")"#,r#"voice "voice/voice.wav""#,r#"sfx.stop("host.wav")"#,"voice stop","music.stop()"];
 let(mut owner,host,mut expected)=startup(&commands);finish(&mut owner);for _ in &commands{expected.integer(1,999999).unwrap();}
 assert_eq!(owner.state().random,expected);assert_eq!(owner.state().vars["after"],Value::Int(7));
 assert!(owner.state().music.current_file.is_none());assert!(rvn_core::audio_playback::describe(owner.state(),owner.snapshot().epoch).unwrap().is_empty());
 // The final authored Set also publishes its changed state. It must not be
 // mistaken for an eighth audio command or an audio replay.
 assert_eq!(host.publication_count(),8,"Seven audio commands plus the real final Set publication");
 let engine=owner.into_engine().unwrap();assert_eq!(engine.state.random,expected);assert_eq!(engine.renderer.publication_count(),8);
}
#[test]
fn actual_init_game_audio_sources_survive_unique_engine_handoff_without_rewind(){
 let commands=[r#"music.play("host.wav")"#,r#"sfx.play("host.wav")"#,r#"voice "voice/voice.wav""#];
 let(mut owner,host,mut expected)=startup(&commands);finish(&mut owner);for _ in &commands{expected.integer(1,999999).unwrap();}
 assert_eq!(owner.state().random,expected);
 let registry=owner.state().audio_resource_registry().unwrap();assert_eq!(registry.targets.len(),3);
 assert!(registry.targets.iter().all(|t|t.owner==rvn_core::scene_tasks::TaskOwner::Game));
 let targets=registry.targets.iter().map(|t|t.identity.clone()).collect::<Vec<_>>();
 for target in &targets{assert_eq!(host.pull(target,7).unwrap().len(),7);assert_eq!(host.cursor(target).unwrap().next_sample,7);}
 let before=serde_json::to_value(owner.state()).unwrap();let engine=owner.into_engine().unwrap();
 assert_eq!(serde_json::to_value(&engine.state).unwrap(),before);assert_eq!(engine.state.random,expected);
 for target in &targets{assert_eq!(engine.renderer.cursor(target).unwrap().next_sample,7);assert_eq!(engine.renderer.pull(target,5).unwrap().len(),5);assert_eq!(engine.renderer.cursor(target).unwrap().next_sample,12);}
}

#[test]
fn stopped_real_init_audio_cold_save15_keeps_pcm_cursor_rng_and_rejects_changed_program(){
 let commands=[r#"music.play("host.wav")"#,r#"sfx.play("host.wav")"#,r#"voice "voice/voice.wav""#];
 let(mut warm,host,mut expected)=startup(&commands);
 for _ in 0..128{
  if warm.state().audio_resource_registry().is_some_and(|r|r.targets.len()==3){break;}
  assert!(!warm.is_complete());warm.step(rvn_core::editor_debugger::StepKind::Continue).unwrap();
 }
 assert!(!warm.is_complete());assert!(!warm.state().vars.contains_key("after"));
 for _ in &commands{expected.integer(1,999999).unwrap();}assert_eq!(warm.state().random,expected);
 let targets=warm.state().audio_resource_registry().unwrap().targets.iter().map(|t|t.identity.clone()).collect::<Vec<_>>();assert_eq!(targets.len(),3);
 for t in &targets{assert_eq!(host.pull(t,7).unwrap().len(),7);assert_eq!(host.cursor(t).unwrap().next_channel,1);}
 let before=serde_json::to_value(warm.state()).unwrap();let bytes=warm.capture_save15(35,"Real pending Init audio".into()).unwrap().encode().unwrap();
 assert_eq!(serde_json::to_value(warm.state()).unwrap(),before);
 let(mut cold,cold_host,_)=startup(&commands);assert_eq!(cold_host.publication_count(),0);
 cold.restore_save15(rvn_core::save::consumer_checkpoint::Save15::decode(&bytes).unwrap()).unwrap();
 assert_eq!(cold.state().random,expected);assert!(!cold.is_complete());assert!(!cold.state().vars.contains_key("after"));assert_eq!(cold_host.publication_count(),1,"Restore publishes once without replaying the three audio commands");
 for t in &targets{
  let cursor=cold_host.cursor(t).unwrap();assert_eq!(cursor.next_sample,7);assert_eq!(cursor.next_channel,1);
  assert_eq!(cold_host.pull(t,1).unwrap(),vec![0]);assert_eq!(cold_host.cursor(t).unwrap(),cursor);
  assert_eq!(host.pull(t,5).unwrap(),cold_host.pull(t,5).unwrap());
 }
 finish(&mut cold);assert_eq!(cold.state().random,expected);assert_eq!(cold.state().vars["after"],Value::Int(7));
 let engine=cold.into_engine().unwrap();assert_eq!(engine.state.random,expected);for t in &targets{assert_eq!(engine.renderer.cursor(t).unwrap().next_sample,12);}
 let changed=[r#"music.volume("0.8")"#,commands[1],commands[2]];
 let(mut refused,refused_host,_)=startup(&changed);let stopped=serde_json::to_value(refused.state()).unwrap();let source=refused.observation_source().unwrap();
 assert!(refused.restore_save15(rvn_core::save::consumer_checkpoint::Save15::decode(&bytes).unwrap()).is_err());assert_eq!(serde_json::to_value(refused.state()).unwrap(),stopped);assert!(refused.accepts_observation_source(&source));assert_eq!(refused_host.publication_count(),0);
}
