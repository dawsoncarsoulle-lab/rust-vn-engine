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

use rvn_core::Interaction;
use rvn_core::audio_playback::{AudioFamily,AudioAdmission,AudioReport,AudioFeedback};
use rvn_core::scene_tasks::TaskOwner;
use rvn_core::save::consumer_checkpoint::Save15;
use rvn_core::engine::{DebugOperation,DebugSession,DebugProgress};
use rvn_core::editor_debugger::{StepKind,Route};
#[path="private_pcm0370/frame_pcm_host.rs"]mod real_pcm0370;
fn state<R:Renderer>(e:&Engine<R>)->serde_json::Value{serde_json::to_value(&e.state).unwrap()}
fn source(commands:&[&str],scene:bool)->String{
 let mut s="function draw(){local n=random(1,999999) return n}\n".to_string();if scene{s.push_str("scene label start\n");}
 for (i,command) in commands.iter().enumerate(){s.push_str(&format!("flow.consume_v2(\"audio_scope_{i}\",\"audio_consumer_{i}\"){{capture \"actual_draw_{i}\"=draw() {command}}}\n\"boundary{i}\"\n"));}s
}
fn engine<R:Renderer>(s:&str,r:R)->(ParsedProgram,Engine<R>){let p=rvn_parser::parse_program(s).unwrap();let mut e=Engine::from_program_with_debugger(p.clone(),r,24).unwrap();e.state.random=RandomState::seeded(29);(p,e)}
fn next<R:Renderer>(e:&mut Engine<R>){e.advance_dialogue().unwrap();e.step_until_interaction().unwrap();}
fn expected(draws:usize)->RandomState{let mut r=RandomState::seeded(29);for _ in 0..draws{r.integer(1,999999).unwrap();}r}
// Existing Legacy Dialogue creates its display stream from one story draw.
fn expected_reached(commands:usize)->RandomState{let mut r=RandomState::seeded(29);for _ in 0..commands{r.integer(1,999999).unwrap();r.next_u64();}r}
// Traverse actual declarations/labels through the existing debugger entry API.
fn seek_audio<R:Renderer>(e:&mut Engine<R>){for _ in 0..16{if matches!(e.script.get(e.state.pc),Some(Statement::ScheduledConsumer{..})){return;}let mut session=e.debug_begin(DebugOperation::Statement,vec![]).unwrap();finish(e,&mut session);}panic!("actual entry traversal exceeded declaration bound")}
fn finish<R:Renderer>(e:&mut Engine<R>,s:&mut DebugSession){for _ in 0..128{if e.debug_step(s,StepKind::Continue).unwrap().0==DebugProgress::Complete{return;}}panic!("actual bounded continuation did not finish")}
#[test]
fn seven_native_audio_commands_use_real_scene_and_game_frames_and_existing_paths(){
 let commands=[r#"music.play("host.wav")"#,r#"music.volume("0.3")"#,r#"sfx.play("host.wav")"#,r#"voice "voice/voice.wav""#,r#"sfx.stop("host.wav")"#,"voice stop","music.stop()"];
 for scene in [false,true]{let(_,mut e)=engine(&source(&commands,scene),Host::default());
  for i in 0..commands.len(){e.step_until_interaction().unwrap();assert_eq!(e.state.random,expected_reached(i+1));
   let r=e.state.audio_resource_registry().unwrap();assert_eq!(e.state.scene_execution.is_some(),scene);assert_eq!(e.state.game_audio.is_some(),!scene);
   if i<=5{assert_eq!(e.state.music.current_file.as_deref(),Some("music/host.wav"));}else{assert!(e.state.music.current_file.is_none());}
   if i>=1{assert_eq!(e.state.music.volume,0.3);}
   let v=e.audio_playback_views().unwrap();assert_eq!(v.iter().filter(|v|v.playback.family==AudioFamily::Sfx).count(),usize::from((2..4).contains(&i)));assert_eq!(v.iter().filter(|v|v.playback.family==AudioFamily::Voice).count(),usize::from((3..5).contains(&i)));
   for v in v {assert_eq!(v.token.task.key.owner,if scene{TaskOwner::Scene{activation:e.state.scene_execution.as_ref().unwrap().owner().unwrap()}}else{TaskOwner::Game});}
   assert!(r.targets.iter().filter(|t|t.identity.kind==rvn_core::scene_resources::TargetKind::Music).all(|t|t.owner==TaskOwner::Game));
   if i+1<commands.len(){e.advance_dialogue().unwrap();}
  }
 }
}
#[test]
fn real_wav_sources_cold_save15_keep_non_consumed_sample_and_never_relaunch_audio(){
 use real_pcm0370::PcmHost;
 for scene in [false,true]{let commands=[r#"music.play("host.wav")"#,r#"sfx.play("host.wav")"#,r#"voice "voice/voice.wav""#,"voice stop",r#"sfx.stop("host.wav")"#,"music.stop()"];
  let text=source(&commands,scene);let(p,mut warm)=engine(&text,PcmHost::default());warm.renderer.bind_actual(&warm.state,warm.instance_id(),warm.runtime_epoch()).unwrap();warm.step_until_interaction().unwrap();next(&mut warm);next(&mut warm);
  let targets=warm.state.audio_resource_registry().unwrap().targets.iter().map(|t|t.identity.clone()).collect::<Vec<_>>();assert_eq!(targets.len(),3);
  for target in &targets{assert_eq!(warm.renderer.pull(target,7).unwrap().len(),7);}
  let bytes=warm.capture_save15(&p,None,48,"Actual Native audio Sources".into()).unwrap().encode().unwrap();
  let(_,mut cold)=engine(&text,PcmHost::default());cold.renderer.bind_actual(&cold.state,cold.instance_id(),cold.runtime_epoch()).unwrap();cold.restore_save15(&p,Save15::decode(&bytes).unwrap()).unwrap();
  assert_eq!(cold.state.random,expected_reached(3));let stopped=state(&cold);cold.step_until_interaction().unwrap();assert_eq!(state(&cold),stopped);
  for target in &targets{
   let cursor=cold.renderer.cursor(target).unwrap();assert_eq!(cursor.next_sample,7);
   // Fresh output starts at channel0; exact Source cursor7 is channel1.
   let padding=usize::from(cursor.next_channel);assert_eq!(cold.renderer.pull(target,padding).unwrap(),vec![0;padding]);assert_eq!(cold.renderer.cursor(target).unwrap(),cursor);
   assert_eq!(warm.renderer.pull(target,5).unwrap(),cold.renderer.pull(target,5).unwrap());
  }
  next(&mut warm);next(&mut cold);assert_eq!(cold.audio_playback_views().unwrap().len(),1);assert_eq!(cold.state.random,expected_reached(4));
  next(&mut warm);next(&mut cold);assert!(cold.audio_playback_views().unwrap().is_empty());next(&mut warm);next(&mut cold);assert!(cold.state.music.current_file.is_none());assert_eq!(cold.state.random,warm.state.random);assert_eq!(cold.state.audio_resource_registry(),warm.state.audio_resource_registry());
 }
}
#[test]
fn paused_actual_audio_producer_cold_continuation_evaluates_once_and_publishes_once(){
 for scene in [false,true]{let text=source(&[r#"sfx.play("host.wav")"#],scene);let(p,mut warm)=engine(&text,Host::default());if scene{warm.enter_label("start").unwrap();}seek_audio(&mut warm);
  let mut sites=vec![];let mut offset=0;loop{let page=warm.debug_site_page(offset).unwrap();offset+=page.entries.len();sites.extend(page.entries);if offset==page.total{break;}}
  let point=sites.into_iter().find(|e|e.breakpoint.site.as_ref().is_some_and(|s|s.function.as_deref()==Some("draw")&&s.route==[Route::Statement(1)])).expect("actual source return");assert!(point.origin.is_some());
  let before=state(&warm);let mut session=warm.debug_begin(DebugOperation::Statement,vec![point.breakpoint]).unwrap();assert_eq!(warm.debug_step(&mut session,StepKind::Continue).unwrap().0,DebugProgress::Stopped);assert_eq!(state(&warm),before);assert!(warm.audio_playback_views().unwrap().is_empty());
  let bytes=warm.capture_save15(&p,Some(&session),49,"Stopped actual audio gather".into()).unwrap().encode().unwrap();finish(&mut warm,&mut session);
  let(_,mut cold)=engine(&text,Host::default());let mut restored=cold.restore_save15(&p,Save15::decode(&bytes).unwrap()).unwrap().unwrap();finish(&mut cold,&mut restored);assert_eq!(state(&warm),state(&cold));assert_eq!(cold.state.random,expected(1));assert_eq!(cold.audio_playback_views().unwrap().len(),1);
 }
}
#[test]
fn renderer_and_invalid_audio_refuse_frame_rng_history_and_publication_atomically(){
 for command in [r#"sfx.play("host.wav")"#,r#"sfx.play("")"#]{for scene in [false,true]{let(_,mut e)=engine(&source(&[command],scene),Host::default());if scene{e.enter_label("start").unwrap();}seek_audio(&mut e);if command.contains("host"){e.renderer.reject.store(true,Ordering::SeqCst);}
  let before=state(&e);let src=e.debug_state_source().unwrap();let epoch=e.runtime_epoch();let publications=e.renderer.batches.lock().unwrap().len();assert!(e.step_until_interaction().is_err());assert_eq!(state(&e),before);assert!(e.accepts_debug_state_source(&src));assert_eq!(e.runtime_epoch(),epoch);assert_eq!(e.renderer.batches.lock().unwrap().len(),publications);
 }}
}
#[test]
fn exact_audio_reports_reject_aba_owner_epoch_and_decoder_error_before_commit(){
 for scene in [false,true]{let commands=[r#"sfx.play("host.wav")"#,r#"sfx.stop("host.wav")"#,r#"sfx.play("host.wav")"#];let(_,mut e)=engine(&source(&commands,scene),Host::default());e.step_until_interaction().unwrap();let old=e.audio_playback_views().unwrap()[0].token.clone();next(&mut e);next(&mut e);let current=e.audio_playback_views().unwrap()[0].token.clone();assert_ne!(old.target,current.target);
  let instance=e.instance_id();let old_report=AudioReport{instance_id:instance,token:old,feedback:AudioFeedback::End};let(_,receipts)=e.tick_game_frame_with_audio(0.0,Default::default(),vec![old_report],true).unwrap();assert_eq!(receipts[0].disposition,rvn_core::engine::AudioReportDisposition::RejectedStale);assert_eq!(e.audio_report_admission(instance+1,&current),AudioAdmission::Stale);
  let before=state(&e);let err=AudioReport{instance_id:instance,token:current.clone(),feedback:AudioFeedback::Error("actual decoder failed".into())};assert!(e.tick_game_frame_with_audio(0.0,Default::default(),vec![err],true).is_err());assert_eq!(state(&e),before);
  let end=AudioReport{instance_id:instance,token:current.clone(),feedback:AudioFeedback::End};let(o,r)=e.tick_game_frame_with_audio(0.0,Default::default(),vec![end],true).unwrap();assert!(o.published);assert_eq!(r[0].disposition,rvn_core::engine::AudioReportDisposition::Consumed);assert!(e.audio_playback_views().unwrap().is_empty());assert_eq!(e.audio_report_admission(instance,&current),AudioAdmission::Stale);
 }
}
#[test]
fn native_audio_version1_and_pure_host_contexts_do_not_acquire_story_ownership(){
 let body=r#"flow.consume_v2("a","b"){sfx.play("host.wav")}"#;assert!(rvn_parser::parse_program(&body.replace("consume_v2","consume_v1")).is_err());
 for prefix in ["function unsafe()","screen unsafe()","handler unsafe()"]{let text=format!("{prefix}{{{body}}}\nscene label start\n\"end\"");match rvn_parser::parse_program(&text){Ok(p)=>assert!(Engine::from_program(p,Host::default(),8).is_err()),Err(_)=>{}}}
 let(_,legacy)=engine("\"old\"",Host::default());let json=serde_json::to_string(&legacy.state).unwrap();assert!(!json.contains("game_audio"));let decoded:rvn_core::GameState=serde_json::from_str(&json).unwrap();assert_eq!(serde_json::to_string(&decoded).unwrap(),json);
 let(_,e)=engine(&source(&[r#"sfx.play("host.wav")"#],false),Host::default());let mut stripped=e.state.clone();stripped.consumer_frame=None;for version in 1..=14{let mut dto=rvn_core::save::SaveData::from_state(&stripped,1,"old".into(),"old".into());dto.format_version=version;assert!(dto.validate_checkpoint_shape().is_err());}
}

#[test]
fn pre_scene_game_audio_uses_real_game_owner_and_frame_without_invented_activation(){
 let text=format!("{}\nscene label later\n\"later\"\n",source(&[r#"sfx.play("host.wav")"#],false));let(p,mut e)=engine(&text,real_pcm0370::PcmHost::default());e.renderer.bind_actual(&e.state,e.instance_id(),e.runtime_epoch()).unwrap();e.step_until_interaction().unwrap();let execution=e.state.scene_execution.as_ref().unwrap();assert!(execution.active.is_none());assert!(e.state.game_audio.is_none());assert_eq!(e.state.consumer_frame.as_ref().unwrap().owner,rvn_core::consumer_schedule::ConsumerOwner::Game);assert_eq!(e.audio_playback_views().unwrap()[0].token.task.key.owner,TaskOwner::Game);
 let save=e.capture_save15(&p,None,50,"Actual preSceneGame".into()).unwrap();let(_,mut cold)=engine(&text,real_pcm0370::PcmHost::default());cold.renderer.bind_actual(&cold.state,cold.instance_id(),cold.runtime_epoch()).unwrap();cold.restore_save15(&p,save).unwrap();assert_eq!(cold.state.random,e.state.random);assert!(cold.state.scene_execution.as_ref().unwrap().active.is_none());
}
#[test]
fn game_and_scene_audio_fades_advance_exact_clocks_then_remove_only_current_tasks(){
 for scene in [false,true]{let cmds=[r#"music.play("host.wav") with fade"#,r#"music.volume("0.25")"#,r#"sfx.play("host.wav") with fade"#,r#"sfx.stop("host.wav") with fade"#,"music.stop() with fade"];
 let(_,mut e)=engine(&source(&cmds,scene),Host::default());e.step_until_interaction().unwrap();assert_eq!(e.state.audio_resource_registry().unwrap().tasks.iter().filter(|t|t.effect.is_some()).count(),1);
 e.tick_game_frame(0.25,Default::default()).unwrap();assert_eq!(e.state.audio_resource_registry().unwrap().tasks.iter().find_map(|t|t.effect.as_ref()).unwrap().clock.elapsed,0.25);
 next(&mut e);assert!(e.state.audio_resource_registry().unwrap().tasks.iter().all(|t|t.effect.is_none()));next(&mut e);let v=e.audio_playback_views().unwrap()[0].clone();assert_eq!(v.playback.gain(),0.0);e.tick_game_frame(0.25,Default::default()).unwrap();let halfway=e.audio_playback_views().unwrap()[0].clone();assert!(halfway.playback.gain()>0.0&&halfway.playback.gain()<1.0);
 next(&mut e);let stopping=e.audio_playback_views().unwrap()[0].clone();assert_eq!(stopping.token.target,v.token.target);assert_ne!(stopping.token.task.revision,v.token.task.revision);assert!(stopping.playback.stopping);e.tick_game_frame(0.5,Default::default()).unwrap();assert!(e.audio_playback_views().unwrap().is_empty());next(&mut e);assert!(e.state.music.current_file.is_none());let retired=e.state.audio_resource_registry().unwrap().tasks.iter().find(|t|t.effect.as_ref().is_some_and(|e|e.presentation.is_some())).unwrap().target.clone();e.tick_game_frame(0.5,Default::default()).unwrap();assert!(!e.state.audio_resource_registry().unwrap().targets.iter().any(|t|t.identity==retired));
 }
}
#[test]
fn cold_audio_source_owner_registry_and_checkpoint_tampering_refuse_atomically(){
 let text=source(&[r#"sfx.play("host.wav")"#],false);let(p,mut warm)=engine(&text,real_pcm0370::PcmHost::default());warm.renderer.bind_actual(&warm.state,warm.instance_id(),warm.runtime_epoch()).unwrap();warm.step_until_interaction().unwrap();let target=warm.audio_playback_views().unwrap()[0].token.target.clone();warm.renderer.pull(&target,7).unwrap();
 let all=warm.renderer.capture_frame_audio_checkpoint(&warm.state,warm.instance_id(),warm.runtime_epoch()).unwrap();let checkpointed=rvn_core::frame_audio_checkpoint::checkpointed_frame_state(&warm.state,warm.instance_id(),warm.runtime_epoch(),&all).unwrap();rvn_core::frame_audio_checkpoint::require_frame_checkpoints(&checkpointed).unwrap();assert_eq!(warm.renderer.cursor(&target).unwrap().next_sample,7);
 let bytes=warm.capture_save15(&p,None,51,"Bound real Native source".into()).unwrap().encode().unwrap();
 for mode in ["owner","missing","duplicate","pcm","incarnation"]{let mut dto:serde_json::Value=serde_json::from_str(&bytes).unwrap();let r=&mut dto["story"]["base"]["game_audio"];assert!(r.is_object());match mode{
  "owner"=>r["targets"][0]["owner"]=serde_json::json!({"scope":"host","role":"pause"}),"missing"=>*r=serde_json::Value::Null,"duplicate"=>{let row=r["tasks"][0].clone();r["tasks"].as_array_mut().unwrap().push(row);},"pcm"=>r["tasks"][0]["audio"]["checkpoint"]["pcm_sha256"][0]=serde_json::json!(255),"incarnation"=>r["targets"][0]["identity"]["incarnation"]=serde_json::json!(999999),_=>unreachable!()};
  let(_,mut cold)=engine(&text,real_pcm0370::PcmHost::default());cold.renderer.bind_actual(&cold.state,cold.instance_id(),cold.runtime_epoch()).unwrap();let before=state(&cold);let src=cold.debug_state_source().unwrap();let publications=cold.renderer.publication_count();let epoch=cold.runtime_epoch();let result=Save15::decode(&dto.to_string()).and_then(|s|cold.restore_save15(&p,s));assert!(result.is_err(),"{mode} falsely admitted");assert_eq!(state(&cold),before);assert!(cold.accepts_debug_state_source(&src));assert_eq!(cold.renderer.publication_count(),publications);assert_eq!(cold.runtime_epoch(),epoch);
 }
 let changed=rvn_parser::parse_program(&text.replace("host.wav","other.wav")).unwrap();let(_,mut cold)=engine(&text,real_pcm0370::PcmHost::default());let before=state(&cold);assert!(cold.restore_save15(&changed,Save15::decode(&bytes).unwrap()).is_err());assert_eq!(state(&cold),before);
}

#[test]
fn game_audio_rollback_uses_real_fenced_visit_and_refuses_missing_source_atomically(){
 let text=source(&[r#"sfx.play("host.wav")"#,r#"music.volume("0.4")"#],false);let(_,mut e)=engine(&text,real_pcm0370::PcmHost::default());e.renderer.bind_actual(&e.state,e.instance_id(),e.runtime_epoch()).unwrap();e.step_until_interaction().unwrap();let old=e.audio_playback_views().unwrap()[0].token.clone();e.renderer.pull(&old.target,7).unwrap();
 let anchor=e.audio_publication_source().unwrap();let snap=e.renderer.capture_audio_checkpoint(&e.state,e.instance_id(),e.runtime_epoch()).unwrap();assert!(e.record_audio_publication(&anchor,snap).unwrap());let before=e.renderer.cursor(&old.target).unwrap();assert_eq!(before.next_sample,7);
 next(&mut e);e.renderer.pull(&old.target,2).unwrap();let anchor=e.audio_publication_source().unwrap();let snap=e.renderer.capture_audio_checkpoint(&e.state,e.instance_id(),e.runtime_epoch()).unwrap();assert!(e.record_audio_publication(&anchor,snap).unwrap());
 assert!(e.preview_game_audio_rollback().is_err(),"historical SaveData must not gain Native authority");let source=e.preview_game_audio_rollback_source().unwrap().expect("previous actual visit");assert!(e.rollback_from_audio_source(&source).unwrap());assert_eq!(e.renderer.cursor(&old.target).unwrap().next_sample,7);assert_eq!(e.audio_report_admission(e.instance_id(),&old),AudioAdmission::Stale);assert!(!e.accepts_audio_rollback_source(&source));assert_eq!(e.state.music.volume,1.0);
 let cursor=e.renderer.cursor(&old.target).unwrap();assert_eq!(e.renderer.pull(&old.target,usize::from(cursor.next_channel)).unwrap(),vec![0;usize::from(cursor.next_channel)]);assert_eq!(e.renderer.cursor(&old.target).unwrap(),cursor);assert_eq!(e.renderer.pull(&old.target,3).unwrap(),vec![7,8,9]);
 let(_,mut unmeasured)=engine(&text,Host::default());unmeasured.step_until_interaction().unwrap();let before=state(&unmeasured);let source=unmeasured.debug_state_source().unwrap();assert!(!unmeasured.rollback());assert_eq!(state(&unmeasured),before);assert!(unmeasured.accepts_debug_state_source(&source));
}

#[test]
fn native_game_audio_dialogue_and_choice_preflight_do_not_leak_live_description_cache(){
 for body in [r#""native text""#,r#"choice {"enter"=>{jump finish}}"#]{let text=format!("function draw(){{return 1}}\nflow.consume_v2(\"a\",\"play\"){{sfx.play(\"host.wav\")}}\nflow.consume_v2(\"display\",\"display_consumer\"){{{body}}}\nlabel finish\n\"done\"");let(_,mut e)=engine(&text,Host::default());seek_audio(&mut e);let mut session=e.debug_begin(DebugOperation::Statement,vec![]).unwrap();finish(&mut e,&mut session);assert_eq!(e.audio_playback_views().unwrap().len(),1);
  e.renderer.reject.store(true,Ordering::SeqCst);let before=state(&e);let src=e.debug_state_source().unwrap();let count=e.renderer.batches.lock().unwrap().len();assert!(e.step_until_interaction().is_err());assert_eq!(state(&e),before);assert!(e.accepts_debug_state_source(&src));assert_eq!(e.renderer.batches.lock().unwrap().len(),count);
 }
}

#[test]
fn ui_open_next_to_native_game_audio_preserves_source_and_state_on_renderer_refusal(){
 let text=r#"screen panel(){return component("root","column",{},[])}
flow.consume_v2("a","play"){sfx.play("host.wav")}
flow.consume_v2("u","open"){ui.open("panel",[],false,0)}
"boundary"
"#;
 let(_,mut e)=engine(text,Host::default());seek_audio(&mut e);let mut session=e.debug_begin(DebugOperation::Statement,vec![]).unwrap();finish(&mut e,&mut session);e.renderer.reject.store(true,Ordering::SeqCst);let before=state(&e);let src=e.debug_state_source().unwrap();let count=e.renderer.batches.lock().unwrap().len();assert!(e.step_until_interaction().is_err());assert_eq!(state(&e),before);assert!(e.accepts_debug_state_source(&src));assert_eq!(e.renderer.batches.lock().unwrap().len(),count);
 e.renderer.reject.store(false,Ordering::SeqCst);e.step_until_interaction().unwrap();assert_eq!(e.state.ui.screens.len(),1);assert_eq!(e.audio_playback_views().unwrap().len(),1);
}
