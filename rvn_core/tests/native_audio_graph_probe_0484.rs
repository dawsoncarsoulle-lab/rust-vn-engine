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

#[path="private_pcm0370/frame_pcm_host.rs"]mod real_pcm0370;

use rvn_graph::*;
fn actual(command:&str,name:&str){
 let legacy=format!("scene label start\n{command}\n\"boundary\"\n");let p=rvn_parser::parse_program(&legacy).unwrap();let mut graphs=import_script(&p.script).unwrap();let g=graphs.iter_mut().find(|g|matches!(&g.kind,GraphKind::Label{name} if name=="start")).unwrap();g.enable_native_frame_preparation().unwrap();
 let dir=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap().join("graph-probes");std::fs::create_dir_all(&dir).unwrap();std::fs::write(dir.join(format!("{name}.graph.json")),serde_json::to_vec_pretty(&g).unwrap()).unwrap();
 let result=prepare_native_frame_graph(g);if let Err(e)=&result{std::fs::write(dir.join(format!("{name}.error.txt")),format!("{e:?}")).unwrap();}
 let generated=result.expect("true Graph strict preparation must preserve authored identities");std::fs::write(dir.join(format!("{name}.native.rvn")),&generated.source).unwrap();let p=rvn_parser::parse_program(&generated.source).unwrap();let mut e=rvn_core::Engine::from_program(p,real_pcm0370::PcmHost::default(),8).unwrap();e.renderer.bind_actual(&e.state,e.instance_id(),e.runtime_epoch()).unwrap();assert!(e.step_until_interaction().unwrap().is_some());
}

#[test]fn graph_music_play_keeps_actual_identity(){actual(r#"music.play("host.wav")"#,"music_play");}

#[test]fn graph_music_stop_keeps_actual_identity(){actual(r#"music.stop()"#,"music_stop");}

#[test]fn graph_music_volume_keeps_actual_identity(){actual(r#"music.volume("0.3")"#,"music_volume");}

#[test]fn graph_sfx_play_keeps_actual_identity(){actual(r#"sfx.play("host.wav")"#,"sfx_play");}

#[test]fn graph_sfx_stop_keeps_actual_identity(){actual(r#"sfx.stop("host.wav")"#,"sfx_stop");}

#[test]fn graph_voice_play_keeps_actual_identity(){actual(r#"voice "voice/voice.wav""#,"voice_play");}

#[test]fn graph_voice_stop_keeps_actual_identity(){actual(r#"voice stop"#,"voice_stop");}

#[test]fn actual_asset_reroute_static_audio_source_also_passes_engine_admission(){
 let parsed=rvn_parser::parse_program("scene label start\nsfx.play(\"host.wav\")\n\"boundary\"\n").unwrap();let mut graphs=import_script(&parsed.script).unwrap();let g=graphs.iter_mut().find(|g|matches!(&g.kind,GraphKind::Label{name}if name=="start")).unwrap();let command=g.nodes.values().find(|n|n.kind==NodeKind::SfxPlay).unwrap().id;let input=g.pin_by_key(command,"file").unwrap().id;let edge=g.edges.values().find(|e|e.input==input).unwrap().clone();let ty=g.pins[&edge.output].value_type.clone();let reroute=g.add_catalog_node(NodeKind::Reroute,[400.,300.]).unwrap();g.set_reroute_type(reroute,ty).unwrap();let middle=g.pin_by_key(reroute,"value").unwrap().id;let result=g.pin_by_key(reroute,"value_out").unwrap().id;g.remove_edge(edge.id);g.connect(edge.output,middle).unwrap();g.connect(result,input).unwrap();g.enable_native_frame_preparation().unwrap();let generated=prepare_native_frame_graph(g).unwrap();
 let dir=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap().join("graph-probes");std::fs::write(dir.join("sfx-reroute.native.rvn"),&generated.source).unwrap();let p=rvn_parser::parse_program(&generated.source).unwrap();let mut e=rvn_core::Engine::from_program_with_debugger(p,real_pcm0370::PcmHost::default(),8).unwrap();e.renderer.bind_actual(&e.state,e.instance_id(),e.runtime_epoch()).unwrap();assert!(e.step_until_interaction().unwrap().is_some());assert_eq!(e.audio_playback_views().unwrap()[0].playback.file,"sfx/host.wav");
}

#[test]fn real_graph_sfx_keeps_terminal_capability_refusal_atomic(){
 let p=rvn_parser::parse_program(include_str!("../../../fixtures/static-audio-reroute-0507.rvn")).unwrap();let mut e=rvn_core::Engine::from_program_with_debugger(p,rvn_core::TerminalRenderer,8).unwrap();e.enter_label("start").unwrap();let before=serde_json::to_value(&e.state).unwrap();let source=e.debug_state_source().unwrap();let epoch=e.runtime_epoch();let error=e.step_until_interaction().unwrap_err();assert!(error.to_string().contains("Terminal rendering does not support these scene resources"),"{error}");assert_eq!(serde_json::to_value(&e.state).unwrap(),before);assert!(e.accepts_debug_state_source(&source));assert_eq!(e.runtime_epoch(),epoch);
}
