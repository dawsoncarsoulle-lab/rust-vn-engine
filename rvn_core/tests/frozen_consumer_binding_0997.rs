//! Actual source admission, independent of a scheduled-wrapper coincidence.
use rvn_core::{Engine, Renderer, consumer_schedule::ConsumerSourceBinding,
    renderer::SceneRenderBatch, save::consumer_checkpoint::Save15, types::SpriteState};
use rvn_parser::{Hotspot, ParsedProgram, Position, Transition};
use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};

#[derive(Clone,Default)]
struct Host(Arc<AtomicUsize>);
impl Renderer for Host {
 fn validate_scene_batch(&self,_:&SceneRenderBatch)->Result<(),String>{Ok(())}
 fn publish_scene_batch(&mut self,_:SceneRenderBatch){self.0.fetch_add(1,Ordering::SeqCst);}
 fn set_typewriter_config(&mut self,_:f32){self.0.fetch_add(1,Ordering::SeqCst);}
 fn set_background(&mut self,_:&str,_:&Transition){}
 fn show_sprite(&mut self,_:&str,_:Option<&str>,_:&Position,_:&Transition,_:Option<&SpriteState>){}
 fn hide_sprite(&mut self,_:&str,_:&Transition,_:&SpriteState){}
 fn move_sprite(&mut self,_:&str,_:&Position,_:&Transition,_:&SpriteState){}
 fn show_dialogue(&mut self,_:Option<&str>,_:&str){}
 fn show_choice(&mut self,_:&[String])->usize{0}
 fn show_imagemap(&mut self,_:&str,_:Option<&str>,_:&[Hotspot])->usize{0}
 fn music_play(&mut self,_:&str,_:&Transition,_:Option<&str>){}
 fn music_stop(&mut self,_:&Transition){}
 fn music_set_volume(&mut self,_:f32){}
 fn sfx_play(&mut self,_:&str,_:&Transition){}
 fn sfx_stop(&mut self,_:&str,_:&Transition){}
}
fn program()->ParsedProgram {
 let p=rvn_parser::parse_program("init{set marker=7}\nscene label start\ntypewriter=true\n\"boundary\"\n").unwrap();
 assert!(!p.script.iter().any(|s|matches!(s,rvn_parser::Statement::ScheduledConsumer{..})));
 p
}
fn frozen(e:&Engine<Host>)->serde_json::Value{serde_json::to_value(&e.state).unwrap()}
#[test]
fn explicit_binding_without_wrapper_is_retained_for_actual_save15_and_cold_restore(){
 let p=program();let binding=ConsumerSourceBinding::from_program(&p).unwrap();
 let mut warm=Engine::from_program_with_consumer_binding(p.clone(),binding.clone(),Host::default(),8).unwrap();
 warm.step_until_interaction().unwrap();let before=frozen(&warm);let random=warm.state.random;
 let saved=warm.capture_save15(&p,None,1,"explicit frozen source".into()).unwrap().encode().unwrap();
 assert_eq!(frozen(&warm),before);
 let mut cold=Engine::from_program_with_consumer_binding(p.clone(),binding,Host::default(),8).unwrap();
 assert!(cold.restore_save15(&p,Save15::decode(&saved).unwrap()).unwrap().is_none());
 assert_eq!(frozen(&cold),before);assert_eq!(cold.state.random,random);
 let publications=cold.renderer.0.load(Ordering::SeqCst);cold.step_silent().unwrap();
 assert_eq!(frozen(&cold),before);assert_eq!(cold.renderer.0.load(Ordering::SeqCst),publications);
}
#[test]
fn debugger_binding_without_wrapper_is_retained_and_changed_capture_is_atomic(){
 let p=program();let mut e=Engine::from_program_with_debugger(p.clone(),Host::default(),8).unwrap();
 e.step_until_interaction().unwrap();let before=frozen(&e);let publications=e.renderer.0.load(Ordering::SeqCst);
 e.capture_save15(&p,None,1,"debug frozen source".into()).unwrap();
 let changed=rvn_parser::parse_program("init{set marker=8}\nscene label start\ntypewriter=true\n\"boundary\"\n").unwrap();
 assert!(e.capture_save15(&changed,None,1,"changed actual source".into()).is_err());
 assert_eq!(frozen(&e),before);assert_eq!(e.renderer.0.load(Ordering::SeqCst),publications);
}
#[test]
fn forged_supplied_binding_and_debugger_source_fail_before_init_publication(){
 let p=rvn_parser::parse_program("init{typewriter=true}\nscene label start\n\"boundary\"\n").unwrap();
 let mut binding=ConsumerSourceBinding::from_program(&p).unwrap();binding.closure[0]^=1;
 let host=Host::default();
 assert!(Engine::from_program_with_consumer_binding(p.clone(),binding,host.clone(),8).is_err());
 assert_eq!(host.0.load(Ordering::SeqCst),0);
 let mut forged=p;forged.sources[0].source=forged.sources[0].source.replace("typewriter=true","typewriter=false");
 assert!(Engine::from_program_with_debugger(forged,host.clone(),8).is_err());
 assert_eq!(host.0.load(Ordering::SeqCst),0);
}
#[test]
fn ordinary_unwrapped_constructor_does_not_acquire_source_authority(){
 let p=program();let mut e=Engine::from_program(p.clone(),Host::default(),8).unwrap();
 e.step_until_interaction().unwrap();let before=frozen(&e);let publications=e.renderer.0.load(Ordering::SeqCst);
 assert!(e.capture_save15(&p,None,1,"ordinary no admission".into()).is_err());
 assert_eq!(frozen(&e),before);assert_eq!(e.renderer.0.load(Ordering::SeqCst),publications);
}
