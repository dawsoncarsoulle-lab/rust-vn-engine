//! Existing D-CATALOG-DYNAMIC-VOLUME: genuine Graph/source binding through real PCM.
//! CPU consumed-sample cursor only; no device, audible, native or pixel claim.
use rvn_core::{Engine, Renderer, renderer::SceneRenderBatch, types::SpriteState};
use rvn_graph::*;
use rvn_parser::{Hotspot, ParsedProgram, Position, Transition};
use std::{path::PathBuf, sync::{Arc, Mutex, atomic::{AtomicBool, AtomicUsize, Ordering}}};
#[path="support/strict_native_consumer.rs"] mod strict_native_consumer;
#[path="private_pcm0370/frame_pcm_host.rs"] mod real_pcm0370;
#[derive(Clone,Default)] struct Host { batches:Arc<Mutex<Vec<SceneRenderBatch>>> }
impl Renderer for Host {
 fn validate_scene_batch(&self,_:&SceneRenderBatch)->Result<(),String>{Ok(())}
 fn publish_scene_batch(&mut self,b:SceneRenderBatch){self.batches.lock().unwrap().push(b);}
 fn set_background(&mut self,_:&str,_:&Transition){}
 fn show_sprite(&mut self,_:&str,_:Option<&str>,_:&Position,_:&Transition,_:Option<&SpriteState>){}
 fn hide_sprite(&mut self,_:&str,_:&Transition,_:&SpriteState){}
 fn move_sprite(&mut self,_:&str,_:&Position,_:&Transition,_:&SpriteState){}
 fn show_dialogue(&mut self,_:Option<&str>,_:&str){}
 fn show_choice(&mut self,_:&[String])->usize{panic!("No choice in the real volume fixture")}
 fn show_imagemap(&mut self,_:&str,_:Option<&str>,_:&[Hotspot])->usize{panic!("No synthetic input")}
 fn music_play(&mut self,_:&str,_:&Transition,_:Option<&str>){}
 fn music_stop(&mut self,_:&Transition){}
 fn music_set_volume(&mut self,_:f32){}
 fn sfx_play(&mut self,_:&str,_:&Transition){}
 fn sfx_stop(&mut self,_:&str,_:&Transition){}
}
// The existing real PCM host remains the only decoder/reader. This wrapper adds
// a preflight refusal and a publication witness, never a second audio machine.
#[derive(Clone,Default)] struct Pcm {
 inner:real_pcm0370::PcmHost, reject:Arc<AtomicBool>, batches:Arc<Mutex<Vec<SceneRenderBatch>>>,
}
impl Renderer for Pcm {
 fn validate_scene_batch(&self,b:&SceneRenderBatch)->Result<(),String>{
  if self.reject.load(Ordering::SeqCst){return Err("Actual Graph PCM renderer refusal".into());}
  self.inner.validate_scene_batch(b)
 }
 fn publish_scene_batch(&mut self,b:SceneRenderBatch){self.batches.lock().unwrap().push(b.clone());self.inner.publish_scene_batch(b);}
 fn capture_audio_checkpoint(&self,s:&rvn_core::GameState,i:u64,e:u64)->Result<rvn_core::audio_checkpoint::AudioCheckpointSnapshot,String>{self.inner.capture_audio_checkpoint(s,i,e)}
 fn capture_frame_audio_checkpoint(&self,s:&rvn_core::GameState,i:u64,e:u64)->Result<rvn_core::audio_checkpoint::AudioCheckpointSnapshot,String>{self.inner.capture_frame_audio_checkpoint(s,i,e)}
 fn validate_audio_restore(&self,s:&rvn_core::GameState)->Result<(),String>{self.inner.validate_audio_restore(s)}
 fn validate_frame_audio_restore(&self,s:&rvn_core::GameState)->Result<(),String>{self.inner.validate_frame_audio_restore(s)}
 fn set_background(&mut self,a:&str,b:&Transition){self.inner.set_background(a,b)}
 fn show_sprite(&mut self,a:&str,b:Option<&str>,c:&Position,d:&Transition,e:Option<&SpriteState>){self.inner.show_sprite(a,b,c,d,e)}
 fn hide_sprite(&mut self,a:&str,b:&Transition,c:&SpriteState){self.inner.hide_sprite(a,b,c)}
 fn move_sprite(&mut self,a:&str,b:&Position,c:&Transition,d:&SpriteState){self.inner.move_sprite(a,b,c,d)}
 fn show_dialogue(&mut self,a:Option<&str>,b:&str){self.inner.show_dialogue(a,b)}
 fn show_choice(&mut self,a:&[String])->usize{self.inner.show_choice(a)}
 fn show_imagemap(&mut self,a:&str,b:Option<&str>,c:&[Hotspot])->usize{self.inner.show_imagemap(a,b,c)}
 fn music_play(&mut self,a:&str,b:&Transition,c:Option<&str>){self.inner.music_play(a,b,c)}
 fn music_stop(&mut self,a:&Transition){self.inner.music_stop(a)}
 fn music_set_volume(&mut self,a:f32){self.inner.music_set_volume(a)}
 fn sfx_play(&mut self,a:&str,b:&Transition){self.inner.sfx_play(a,b)}
 fn sfx_stop(&mut self,a:&str,b:&Transition){self.inner.sfx_stop(a,b)}
}
fn directory(name:&str)->PathBuf {
 static NEXT:AtomicUsize=AtomicUsize::new(0);
 let p=PathBuf::from(std::env::var("RVN_NATIVE_TYPEWRITER_FIXTURES").expect("private source fixture directory"))
  .join(format!("graph-volume1000-{name}-{}-{}",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)));
 std::fs::create_dir_all(&p).unwrap();p
}
fn program(game:bool,reroute:bool)->ParsedProgram {
 let dir=directory(if game{"game"}else{"scene"});
 // A plain label inside an explicit-Scene project is an inheritable Scene
 // fragment: its GET can read an unknown caller local, so Any is deliberate.
 // Author a genuine standalone Game here; the Scene case retains its actual
 // activation boundary and later Scene. Neither case fabricates a type grant.
 let text=format!("{}label start\n{} level=0.25\nset sampled=random(1,9)\nmusic.play(\"host.wav\") with fade(777)\n\"before\"\nmusic.volume(level+0.125)\n\"after\"\n{}",if game{""}else{"scene "},if game{"set"}else{"local"},if game{""}else{"scene label later\n\"later\"\n"});
 let file=dir.join("authored.rvn");std::fs::write(&file,text).unwrap();
 let original=rvn_parser::parse_file_program_with_uses(file).unwrap();let mut graphs=import_script(&original.script).unwrap();
 let graph=graphs.iter_mut().find(|g|matches!(&g.kind,GraphKind::Label{name}if name=="start")).unwrap();
 assert!(graph.nodes.values().any(|n|n.kind==NodeKind::VariableGet),"The actual Float variable remains a real Graph producer");
 let node=graph.nodes.values().find(|n|n.kind==NodeKind::MusicVolume).unwrap().id;
 let input=graph.pin_by_key(node,"level").unwrap().id;let edge=graph.edges.values().find(|e|e.input==input).unwrap().clone();
 graph.enable_native_frame_preparation().unwrap();
 graph.refresh_variable_usage_bindings().unwrap();
 let read=graph.nodes.values().find(|n|n.kind==NodeKind::VariableGet&&graph.variable_node_name(n.id).as_deref()==Some("level")).unwrap().id;
 assert_eq!(graph.effective_pin_type(graph.pin_by_key(read,"value").unwrap().id),Some(ValueType::Float),"The actual source binding must prove the GET Float before arithmetic");
 assert_eq!(graph.effective_pin_type(edge.output),Some(ValueType::Float));
 assert_eq!(graph.nodes[&graph.pins[&edge.output].node].kind,NodeKind::MathAdd);
 if reroute {let route=graph.add_catalog_node(NodeKind::Reroute,[600.,300.]).unwrap();graph.set_reroute_type(route,ValueType::Float).unwrap();graph.remove_edge(edge.id);graph.connect(edge.output,graph.pin_by_key(route,"value").unwrap().id).unwrap();graph.connect(graph.pin_by_key(route,"value_out").unwrap().id,input).unwrap();}
 strict_native_consumer::finish_program(dir,original,graphs,&[NodeKind::MusicPlay,NodeKind::MusicVolume])
}
fn open(p:&ParsedProgram)->Engine<Pcm>{
 let binding=rvn_core::consumer_schedule::ConsumerSourceBinding::from_program(p).unwrap();
 let e=Engine::from_program_with_consumer_binding(p.clone(),binding,Pcm::default(),16).unwrap();
 e.renderer.inner.bind_actual(&e.state,e.instance_id(),e.runtime_epoch()).unwrap();e
}
fn target(e:&Engine<Pcm>)->rvn_core::scene_resources::TargetIdentity {
 e.state.audio_resource_registry().unwrap().targets.iter().find(|t|t.identity.kind==rvn_core::scene_resources::TargetKind::Music).unwrap().identity.clone()
}
// A real live Float differs from its authored initial value. This changes only
// the existing variable slot before the consumer starts, never its owner/frame.
fn live_level(e:&mut Engine<Pcm>,game:bool){
 assert_eq!(e.get_var("level"),Some(&rvn_parser::Value::Float(0.25)));
 if game{e.state.vars.insert("level".into(),rvn_parser::Value::Float(0.5));}
 else{e.state.scene_execution.as_mut().unwrap().active.as_mut().unwrap().locals.insert("level".into(),rvn_parser::Value::Float(0.5));}
 assert_eq!(e.get_var("level"),Some(&rvn_parser::Value::Float(0.5)));
}
fn owner(e:&Engine<Pcm>,game:bool){
 if game {assert!(e.state.scene_execution.is_none(),"Actual standalone Game must not fabricate Scene execution");assert_eq!(e.state.consumer_frame.as_ref().unwrap().owner,rvn_core::consumer_schedule::ConsumerOwner::Game);}
 else {let execution=e.state.scene_execution.as_ref().expect("real Scene project contains a later Scene");let active=execution.active.as_ref().unwrap();assert_eq!(active.scene,"start");assert_eq!(active.consumer_frame.as_ref().unwrap().owner,rvn_core::consumer_schedule::ConsumerOwner::Scene{activation:active.id});}
}
#[test]
fn strict_graph_volume_actual_pcm_cursor_cold_and_source_binding(){
 for game in [false,true]{for reroute in [false,true]{
  let p=program(game,reroute);let mut warm=open(&p);warm.step_until_interaction().unwrap();owner(&warm,game);
  let rng=warm.state.random;let sampled=warm.state.vars["sampled"].clone();let music=target(&warm);
  warm.renderer.inner.pull(&music,7).unwrap();let cursor=warm.renderer.inner.cursor(&music).unwrap();
  warm.advance_dialogue().unwrap();live_level(&mut warm,game);assert!(matches!(warm.peek_statement(),Some(rvn_parser::Statement::ScheduledConsumer{consumer,..})if consumer.contains(":music_volume|")));
  let count=warm.renderer.inner.publication_count();warm.step_silent().unwrap();owner(&warm,game);
  assert_eq!(warm.state.music.volume,0.625);assert_eq!(target(&warm),music);assert_eq!(warm.renderer.inner.cursor(&music).unwrap(),cursor);
  assert_eq!(warm.state.random,rng);assert_eq!(warm.state.vars["sampled"],sampled);assert_eq!(warm.renderer.inner.publication_count(),count+1);
  assert_eq!(warm.renderer.batches.lock().unwrap().last().unwrap().state.music.volume,0.625,"Published state must contain the same actual dynamic Float result");
  assert!(warm.state.audio_resource_registry().unwrap().tasks.iter().all(|t|t.effect.is_none()||t.target!=music),"Volume supersedes the original fade on the existing target");
  warm.step_until_interaction().unwrap();let save=warm.capture_save15(&p,None,1,"strict Graph PCM".into()).unwrap().encode().unwrap();
  let file=p.sources[0].path.as_ref().unwrap();let relocated=file.parent().unwrap().join("cold-relocated.rvn");std::fs::copy(file,&relocated).unwrap();let cold_program=rvn_parser::parse_file_program_with_uses(relocated).unwrap();assert_ne!(cold_program.sources[0].path,p.sources[0].path);
  let mut cold=open(&cold_program);cold.restore_save15(&cold_program,rvn_core::save::consumer_checkpoint::Save15::decode(&save).unwrap()).unwrap();owner(&cold,game);
  assert_eq!(cold.state.music.volume,0.625);assert_eq!(target(&cold),music);assert_eq!(cold.renderer.inner.cursor(&music).unwrap(),cursor);assert_eq!(cold.state.random,rng);
  let count=cold.renderer.inner.publication_count();let before=serde_json::to_value(&cold.state).unwrap();cold.step_silent().unwrap();assert_eq!(serde_json::to_value(&cold.state).unwrap(),before);assert_eq!(cold.renderer.inner.publication_count(),count);
  let padding=usize::from(cursor.next_channel);assert_eq!(cold.renderer.inner.pull(&music,padding).unwrap(),vec![0;padding]);assert_eq!(cold.renderer.inner.cursor(&music).unwrap(),cursor);
  assert_eq!(cold.renderer.inner.pull(&music,5).unwrap(),warm.renderer.inner.pull(&music,5).unwrap(),"Decoded PCM resumes from the same consumed sample, never re-Play");
 }}
}
#[test]
fn strict_graph_volume_renderer_and_source_refusal_do_not_publish_or_advance_rng(){
 for game in [false,true]{
  let p=program(game,true);let binding=rvn_core::consumer_schedule::ConsumerSourceBinding::from_program(&p).unwrap();
  for mode in 0..3 {let mut forged=p.clone();match mode{0=>forged.sources[0].source.push_str("\n// changed frozen bytes\n"),1=>forged.sources.clear(),_=>forged.origins.clear()};let host=Pcm::default();assert!(Engine::from_program_with_consumer_binding(forged,binding.clone(),host.clone(),16).is_err());assert_eq!(host.inner.publication_count(),0);assert!(host.batches.lock().unwrap().is_empty());}
  let mut e=open(&p);e.step_until_interaction().unwrap();let music=target(&e);e.renderer.inner.pull(&music,7).unwrap();let cursor=e.renderer.inner.cursor(&music).unwrap();e.advance_dialogue().unwrap();live_level(&mut e,game);
  assert!(matches!(e.peek_statement(),Some(rvn_parser::Statement::ScheduledConsumer{consumer,..})if consumer.contains(":music_volume|")));
  let state=serde_json::to_value(&e.state).unwrap();let count=e.renderer.inner.publication_count();let witness=e.debug_state_source().unwrap();let epoch=e.runtime_epoch();let session=e.debug_begin(rvn_core::engine::DebugOperation::Statement,vec![]).unwrap();
  e.renderer.reject.store(true,Ordering::SeqCst);let error=e.step_silent().unwrap_err().to_string();assert!(error.contains("Actual Graph PCM renderer refusal"),"Actual refusal: {error}");
  assert_eq!(serde_json::to_value(&e.state).unwrap(),state);assert!(e.accepts_debug_state_source(&witness));assert!(e.accepts_debug_session(&session));assert_eq!(e.runtime_epoch(),epoch);assert_eq!(e.renderer.inner.publication_count(),count);assert_eq!(e.renderer.inner.cursor(&music).unwrap(),cursor);
  e.renderer.reject.store(false,Ordering::SeqCst);e.step_silent().unwrap();owner(&e,game);assert_eq!(e.state.music.volume,0.625);assert_eq!(e.renderer.inner.publication_count(),count+1);assert_eq!(e.renderer.inner.cursor(&music).unwrap(),cursor);assert_eq!(target(&e),music);
 }
}
