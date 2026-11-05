//! Proposed targeted CPU tests. All fixtures use real source/Graph/owner APIs.
//! No decoder/window acceptance, fabricated object handle or injected result.
use rvn_core::{Engine,Renderer,renderer::SceneRenderBatch,types::SpriteState};
use rvn_core::engine::{EngineStartup,StartupVideoCommandKind,DebugOperation};
use rvn_core::editor_debugger::StepKind;
use rvn_core::video::{Playback,VideoView};
use rvn_graph::*;
use rvn_parser::{Hotspot,ParsedProgram,Position,Transition,Value};
use std::{collections::HashMap,path::PathBuf,sync::{Arc,Mutex,atomic::{AtomicBool,AtomicU64,Ordering}}};

#[derive(Clone,Default)]
struct Host{batches:Arc<Mutex<Vec<SceneRenderBatch>>>,views:Arc<Mutex<Vec<VideoView>>>,reject:Arc<AtomicBool>}
impl Renderer for Host{
    fn supports_video(&self)->bool{true}
    fn validate_scene_batch(&self,_:&SceneRenderBatch)->Result<(),String>{if self.reject.load(Ordering::SeqCst){Err("real method fixture publication refused".into())}else{Ok(())}}
    fn publish_scene_batch(&mut self,b:SceneRenderBatch){*self.views.lock().unwrap()=b.videos.clone();self.batches.lock().unwrap().push(b);}
    fn update_videos(&mut self,v:&[VideoView])->Result<(),String>{if self.reject.load(Ordering::SeqCst){return Err("real method fixture publication refused".into());}*self.views.lock().unwrap()=v.to_vec();Ok(())}
    fn set_background(&mut self,_:&str,_:&Transition){}
    fn show_sprite(&mut self,_:&str,_:Option<&str>,_:&Position,_:&Transition,_:Option<&SpriteState>){}
    fn hide_sprite(&mut self,_:&str,_:&Transition,_:&SpriteState){}
    fn move_sprite(&mut self,_:&str,_:&Position,_:&Transition,_:&SpriteState){}
    fn show_dialogue(&mut self,_:Option<&str>,_:&str){}
    fn show_choice(&mut self,_:&[String])->usize{panic!("no renderer-selected input")}
    fn show_imagemap(&mut self,_:&str,_:Option<&str>,_:&[Hotspot])->usize{panic!("no renderer-selected input")}
    fn music_play(&mut self,_:&str,_:&Transition,_:Option<&str>){}
    fn music_stop(&mut self,_:&Transition){}
    fn music_set_volume(&mut self,_:f32){}
    fn sfx_play(&mut self,_:&str,_:&Transition){}
    fn sfx_stop(&mut self,_:&str,_:&Transition){}
}
fn fixture(name:&str,raw:&str,native:bool)->ParsedProgram{
    let mut graphs=import_script(&rvn_parser::parse(raw).unwrap()).unwrap();
    assert!(graphs.iter().any(|g|g.nodes.values().any(|n|n.kind==NodeKind::MethodCall)));
    for graph in &mut graphs{
        *graph=GraphDocument::from_json(&graph.to_pretty_json().unwrap()).unwrap();
        if native{graph.enable_native_frame_preparation().unwrap();}
    }
    let compiled=transpile_project(&graphs).unwrap();
    assert_eq!(compiled.source.contains("flow.consume_v2("),native);
    let imported=import_script(&compiled.ast).unwrap();
    for graph in graphs.iter().filter(|g|g.nodes.values().any(|n|n.kind==NodeKind::MethodCall)){
        if native{
            let restored=imported.iter().find(|g|g.graph_id==graph.graph_id).unwrap();
            assert_eq!(restored.nodes.keys().collect::<Vec<_>>(),graph.nodes.keys().collect::<Vec<_>>());
            assert_eq!(restored.pins.keys().collect::<Vec<_>>(),graph.pins.keys().collect::<Vec<_>>());
            assert_eq!(restored.edges,graph.edges);
        }
        for node in graph.nodes.values().filter(|n|n.kind==NodeKind::MethodCall){
            assert!(node.pins.iter().map(|id|&graph.pins[id]).filter(|p|p.direction==PinDirection::Output).all(|p|p.value_type==ValueType::Execution),"the historical statement has Unit return, no fake return pin");
        }
    }
    static NEXT:AtomicU64=AtomicU64::new(0);
    let root=PathBuf::from(std::env::var("RVN_METHOD0691_FIXTURES").expect("isolated D output root"))
        .join(format!("{name}-{}-{}",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)));
    std::fs::create_dir_all(&root).unwrap();
    for(i,g)in graphs.iter().enumerate(){std::fs::write(root.join(format!("{i}.graph.json")),g.to_pretty_json().unwrap()).unwrap();}
    let file=root.join("actual.rvn");std::fs::write(&file,compiled.source).unwrap();
    rvn_parser::parse_file_program_with_uses(file).unwrap()
}
const SETUP:&str=r#"video.play("intro",video_clip("real-story-clip.webm",{"skippable":true,"on_end":"finished"}))"#;
fn scene(name:&str,before:&str,body:&str)->(Engine<Host>,ParsedProgram){
    let raw=format!("handler finished(event) {{set ends=ends+1}}\ninit {{set ends=0}}\nscene label start\n{SETUP}\n{before}\n\"before\"\n{body}\n\"after\"\n");
    let p=fixture(name,&raw,true);let mut e=Engine::from_program(p.clone(),Host::default(),16).unwrap();
    assert!(matches!(e.step_until_interaction().unwrap(),Some(rvn_core::Interaction::Dialogue{text,..})if text=="before"));
    e.advance_dialogue().unwrap();(e,p)
}
fn state(e:&Engine<Host>)->serde_json::Value{serde_json::to_value(&e.state).unwrap()}
fn count(e:&Engine<Host>)->usize{e.renderer.batches.lock().unwrap().len()}

#[test]
fn native_method0691_pause_resolves_live_video_and_publishes_one_unit_command(){
    let(mut e,_)=scene("pause","","intro.pause()");let old=e.video_views().unwrap().remove(0);let n=count(&e);let rng=e.state.random;
    e.preview_step_statement().unwrap();
    assert_eq!(e.state.videos.tracks["intro"].playback,Playback::Paused);assert_eq!(count(&e),n+1);assert_eq!(e.state.random,rng);
    let view=e.video_views().unwrap().remove(0);let token=view.resource_token.as_ref().unwrap();let resources=&e.state.scene_execution.as_ref().unwrap().resources;
    assert!(resources.accepts(token,e.runtime_epoch(),e.state.scene_execution.as_ref().unwrap().owner()));
    assert_ne!(view.resource_token,old.resource_token);assert!(!e.resume_video_from_view(&old).unwrap());
    assert_eq!(e.renderer.views.lock().unwrap()[0].track.playback,Playback::Paused);
}
#[test]
fn native_method0691_resume_reuses_the_existing_playback_command(){
    let(mut e,_)=scene("resume","video.pause(\"intro\")","intro.resume()");assert_eq!(e.state.videos.tracks["intro"].playback,Playback::Paused);
    e.preview_step_statement().unwrap();assert_eq!(e.state.videos.tracks["intro"].playback,Playback::Playing);assert_eq!(e.renderer.views.lock().unwrap()[0].track.playback,Playback::Playing);
}
#[test]
fn native_method0691_stop_retires_the_real_target_and_projection(){
    let(mut e,_)=scene("stop","","intro.stop()");let old=e.video_views().unwrap().remove(0);
    e.preview_step_statement().unwrap();assert!(!e.state.videos.tracks.contains_key("intro"));assert!(e.video_views().unwrap().is_empty());assert!(e.renderer.views.lock().unwrap().is_empty());
    assert!(!e.resume_video_from_view(&old).unwrap());
}
#[test]
fn native_method0691_skip_runs_the_actual_callback_once(){
    let(mut e,_)=scene("skip","","intro.skip()");e.preview_step_statement().unwrap();
    assert_eq!(e.state.videos.tracks["intro"].playback,Playback::Ended);assert_eq!(e.state.vars["ends"],Value::Int(1));
    assert_eq!(e.renderer.views.lock().unwrap()[0].track.playback,Playback::Ended);
}
#[test]
fn native_method0691_missing_instance_is_atomic(){
    let(mut e,_)=scene("missing","video.stop(\"intro\")","intro.pause()");let before=state(&e);let n=count(&e);let epoch=e.runtime_epoch();
    assert!(e.preview_step_statement().is_err());assert_eq!(state(&e),before);assert_eq!(count(&e),n);assert_eq!(e.runtime_epoch(),epoch);
}
#[test]
fn native_method0691_replacement_revokes_the_old_incarnation_before_current_call(){
    let(mut e,_)=scene("replacement","","video.play(\"intro\",video_clip(\"replacement.webm\",{})) intro.pause()");
    let old=e.video_views().unwrap().remove(0);e.preview_step_statement().unwrap();let fresh=e.video_views().unwrap().remove(0);
    assert_ne!(old.resource_token.as_ref().unwrap().target.incarnation,fresh.resource_token.as_ref().unwrap().target.incarnation);
    let before=state(&e);let n=count(&e);assert!(!e.resume_video_from_view(&old).unwrap());assert_eq!(state(&e),before);assert_eq!(count(&e),n);
    e.preview_step_statement().unwrap();assert_eq!(e.state.videos.tracks["intro"].playback,Playback::Paused);assert_eq!(e.video_views().unwrap()[0].track.clip.source,"replacement.webm");
}
#[test]
fn native_method0691_forged_incarnation_never_authorizes_a_call(){
    let(mut e,_)=scene("forged-incarnation","","intro.pause()");
    let execution=e.state.scene_execution.as_mut().unwrap();
    let target=execution.resources.targets.iter_mut().find(|t|t.identity.kind==rvn_core::scene_resources::TargetKind::Video&&t.identity.alias=="intro").unwrap();
    target.identity.incarnation=target.identity.incarnation.checked_add(1).unwrap();
    let before=state(&e);let n=count(&e);assert!(e.preview_step_statement().is_err());assert_eq!(state(&e),before);assert_eq!(count(&e),n);
}
fn signature_refused(name:&str,body:&str,fragment:&str){
    let raw=format!("init{{set sentinel=1}}\nscene label start\n{body}\n\"after\"\n");let p=fixture(name,&raw,true);let host=Host::default();
    let error=Engine::from_program(p,host.clone(),16).err().expect("signature must refuse before Init").to_string();
    assert!(error.contains(fragment),"{error}");assert!(host.batches.lock().unwrap().is_empty());assert!(host.views.lock().unwrap().is_empty());
}
#[test]
fn native_method0691_nonempty_argument_does_not_become_ignored_input(){signature_refused("argument","intro.pause(\"unexpected\")","zero arguments");}
#[test]
fn native_method0691_unknown_method_does_not_become_print_only(){signature_refused("unknown","intro.custom()","diagnostic-only");}
#[test]
fn native_method0691_transition_is_rejected_without_init_effect(){signature_refused("transition","intro.pause() with fade","transition");}
#[test]
fn method0691_explicit_empty_argument_is_not_lost_in_graph_import(){
    let parsed=rvn_parser::parse("label start\nintro.pause(\"\")\n").unwrap();
    assert!(import_script(&parsed).unwrap_err().contains("explicit empty argument"));
    let json=serde_json::to_value(&parsed).unwrap();assert_eq!(serde_json::from_value::<rvn_parser::Script>(json).unwrap(),parsed);
}
#[test]
fn native_method0691_host_refusal_preserves_owner_rng_and_pending_debug_source(){
    let(mut e,_)=scene("host-refusal","","intro.pause()");let before=state(&e);let n=count(&e);let session=e.debug_begin(DebugOperation::Statement,vec![]).unwrap();
    e.renderer.reject.store(true,Ordering::SeqCst);assert!(e.preview_step_statement().is_err());assert_eq!(state(&e),before);assert_eq!(count(&e),n);assert!(e.accepts_debug_session(&session));
    e.renderer.reject.store(false,Ordering::SeqCst);e.preview_step_statement().unwrap();assert_eq!(e.state.videos.tracks["intro"].playback,Playback::Paused);assert_eq!(count(&e),n+1);
}
#[test]
fn native_method0691_unreached_valid_call_never_resolves_target_or_consumes_body_rng(){
    let(mut e,_)=scene("lazy","","if random(1,1)==0 {ghost.pause() set unused=random(1,1)}");
    let mut expected=rvn_core::random::RandomState::seeded(69);e.state.random=expected;expected.integer(1,1).unwrap();
    e.step_until_interaction().unwrap();assert_eq!(e.state.random,expected);assert!(!e.state.vars.contains_key("unused"));assert_eq!(e.state.videos.tracks["intro"].playback,Playback::Playing);
}
#[test]
fn native_method0691_source_save15_cold_restore_and_rollback_rebind_real_video_authority(){
    let(mut e,p)=scene("cold","","intro.pause()\n\"paused checkpoint\"\nintro.resume()\n\"playing checkpoint\"\nintro.pause()");
    e.preview_step_statement().unwrap();
    assert!(matches!(e.step_until_interaction().unwrap(),Some(rvn_core::Interaction::Dialogue{text,..})if text=="paused checkpoint"));
    let save=e.capture_save15(&p,None,1,"actual paused method".into()).unwrap();let bytes=save.encode().unwrap();
    let old=e.video_views().unwrap().remove(0);let mut cold=Engine::from_program(p.clone(),Host::default(),16).unwrap();
    cold.restore_save15(&p,rvn_core::save::consumer_checkpoint::Save15::decode(&bytes).unwrap()).unwrap();
    assert_eq!(cold.state.videos.tracks["intro"].playback,Playback::Paused);assert!(!cold.resume_video_from_view(&old).unwrap());
    // Save15 restores the current state, never an earlier process's rollback history.
    assert!(cold.history.is_empty());let restored=state(&cold);let n=count(&cold);let epoch=cold.runtime_epoch();
    assert!(!cold.rollback());assert_eq!(state(&cold),restored);assert_eq!(count(&cold),n);assert_eq!(cold.runtime_epoch(),epoch);
    // Establish a new real interaction checkpoint through the authored method calls.
    cold.advance_dialogue().unwrap();cold.preview_step_statement().unwrap();
    assert_eq!(cold.state.videos.tracks["intro"].playback,Playback::Playing);
    assert!(matches!(cold.step_until_interaction().unwrap(),Some(rvn_core::Interaction::Dialogue{text,..})if text=="playing checkpoint"));
    assert_eq!(cold.history.len(),1);let checkpoint=cold.state.clone();let playing=cold.video_views().unwrap().remove(0);
    cold.advance_dialogue().unwrap();cold.preview_step_statement().unwrap();
    assert_eq!(cold.state.videos.tracks["intro"].playback,Playback::Paused);let paused=cold.video_views().unwrap().remove(0);
    assert!(cold.rollback());assert_eq!(cold.state.videos.tracks["intro"].playback,Playback::Playing);
    assert_eq!(cold.state.pc,checkpoint.pc);assert_eq!(cold.state.vars,checkpoint.vars);assert_eq!(cold.state.random,checkpoint.random);
    assert!(cold.history.is_empty());let current=cold.video_views().unwrap().remove(0);let token=current.resource_token.as_ref().unwrap();
    let execution=cold.state.scene_execution.as_ref().unwrap();assert!(execution.resources.accepts(token,cold.runtime_epoch(),execution.owner()));
    assert_eq!(token.target,playing.resource_token.as_ref().unwrap().target);assert_ne!(current.resource_token,playing.resource_token);assert_ne!(current.resource_token,paused.resource_token);
    assert!(!cold.resume_video_from_view(&playing).unwrap());assert!(!cold.resume_video_from_view(&paused).unwrap());
    assert_eq!(cold.renderer.views.lock().unwrap()[0].track.playback,Playback::Playing);
}
#[test]
fn native_method0691_startup_observes_the_same_real_four_commands(){
    let raw=format!("handler finished(event){{set ends=ends+1}}\ninit{{set ends=0 {SETUP} intro.pause() intro.resume() intro.stop() {SETUP} intro.skip() set done=true}}\nscene label start\n\"Ready\"\n");
    let p=fixture("startup",&raw,true);let mut s:EngineStartup<Host>=Engine::prepare_debugger_startup(p,Host::default(),16,HashMap::new(),Default::default(),None).unwrap();
    let mut observed=Vec::new();for _ in 0..128{if s.is_complete(){break;}let step=s.prepare_step(StepKind::Continue).unwrap();observed.extend(step.video_commands().iter().map(|command|command.kind()));s.begin_prepared_step_commit(step).unwrap().commit();}
    assert!(s.is_complete());assert_eq!(s.state().vars["done"],Value::Bool(true));assert_eq!(s.state().vars["ends"],Value::Int(1));
    assert_eq!(observed,vec![StartupVideoCommandKind::Play,StartupVideoCommandKind::Pause,StartupVideoCommandKind::Resume,StartupVideoCommandKind::Stop,StartupVideoCommandKind::Play,StartupVideoCommandKind::Skip]);
}
#[test]
fn legacy_method0691_keeps_the_historical_diagnostic_and_ast_bytes(){
    let source="label start\nvideo.play(\"intro\",video_clip(\"legacy.webm\",{}))\n\"before\"\nintro.pause() with fade\n\"after\"\n";
    let old=rvn_parser::parse(source).unwrap();let wire=serde_json::to_vec(&old).unwrap();assert_eq!(serde_json::to_vec(&serde_json::from_slice::<rvn_parser::Script>(&wire).unwrap()).unwrap(),wire);
    let p=fixture("legacy",source,false);let mut e=Engine::from_program(p,Host::default(),16).unwrap();e.step_until_interaction().unwrap();e.advance_dialogue().unwrap();
    let rng=e.state.random;e.preview_step_statement().unwrap();assert_eq!(e.state.random,rng);assert_eq!(e.state.videos.tracks["intro"].playback,Playback::Playing);assert!(matches!(e.state.last_transition,Transition::Fade{..}));
}

// Additional independent MethodCall gates; no parent test is re-registered.
#[path = "support/native_method_deeper_tests_0818.rs"]
mod method_deeper_0818;
