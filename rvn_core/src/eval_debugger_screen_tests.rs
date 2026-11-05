use super::*;
use crate::{Engine,Renderer};
use rvn_parser::{Hotspot,Position,Transition};
#[derive(Default)]
struct Host{batches:usize,reject:bool}
impl Renderer for Host{
 fn validate_scene_batch(&self,_:&crate::renderer::SceneRenderBatch)->Result<(),String>{if self.reject{Err("actual Screen renderer refusal".into())}else{Ok(())}}
 fn publish_scene_batch(&mut self,_:crate::renderer::SceneRenderBatch){self.batches+=1;}
 fn supports_programmable_ui(&self)->bool{true}
 fn update_interfaces(&mut self,_:&[rvn_ui::programmable::ScreenView])->Result<(),String>{Ok(())}
 fn set_background(&mut self,_:&str,_:&Transition){}
 fn show_sprite(&mut self,_:&str,_:Option<&str>,_:&Position,_:&Transition,_:Option<&crate::types::SpriteState>){}
 fn hide_sprite(&mut self,_:&str,_:&Transition,_:&crate::types::SpriteState){}
 fn move_sprite(&mut self,_:&str,_:&Position,_:&Transition,_:&crate::types::SpriteState){}
 fn show_dialogue(&mut self,_:Option<&str>,_:&str){}
 fn show_choice(&mut self,_:&[String])->usize{0}
 fn show_imagemap(&mut self,_:&str,_:Option<&str>,_:&[Hotspot])->usize{0}
 fn music_play(&mut self,_:&str,_:&Transition,_:Option<&str>){}
 fn music_stop(&mut self,_:&Transition){}
 fn music_set_volume(&mut self,_:f32){}
 fn sfx_play(&mut self,_:&str,_:&Transition){}
 fn sfx_stop(&mut self,_:&str,_:&Transition){}
}
fn fixture()->(rvn_parser::ParsedProgram,Engine<Host>,crate::engine::DebugSession){
 let source=r#"
function draw0775(){local token=random(1,9) return token}
screen panel(marker){
 flow.consume_v2("screen:panel","gather"){
 capture "roll"=draw0775()
 return component("root","column",{},[component("act","button",{"text":marker},[])])
 }
}
scene label start
ui.open("panel",["actual"],false,1)
"ready"
return
"#;
 let mut p=rvn_parser::parse_program(source).unwrap();let path=Some(std::path::PathBuf::from("C:/private/screen-construction0775.rvn"));p.sources[0].path=path.clone();for origin in &mut p.origins{origin.source=path.clone();}
 let prepared=crate::program::PreparedNarrative::from_parsed(p.clone()).unwrap();let pc=prepared.script.iter().position(|s|matches!(s,Statement::Function{name,..}if name=="draw0775")).unwrap();
 let bp=wire::Breakpoint{position:prepared.position(pc),site:Some(Site{function:Some("draw0775".into()),route:vec![Route::Statement(1)],depth:2,statement:true}),enabled:true};
 let mut e=Engine::from_program_with_debugger(p.clone(),Host::default(),16).unwrap();e.state.random=RandomState::seeded(775);e.enter_label("start").unwrap();e.renderer.batches=0;
 let before=serde_json::to_value(&e.state).unwrap();let mut s=e.debug_begin(crate::engine::DebugOperation::Statement,vec![bp]).unwrap();
 assert_eq!(e.debug_step(&mut s,wire::StepKind::Continue).unwrap().0,crate::engine::DebugProgress::Stopped);
 assert_eq!(serde_json::to_value(&e.state).unwrap(),before);assert_eq!(e.renderer.batches,0);assert!(e.state.ui.screens.is_empty());
 let probe=s.screen_probe_for_test().lock().unwrap();let request=probe.requested.as_ref().unwrap();assert!(probe.admits_screen_request(request));assert_eq!(request.scope.as_deref(),Some("screen:panel"));drop(probe);
 (p,e,s)
}
fn finish(e:&mut Engine<Host>,s:&mut crate::engine::DebugSession){for _ in 0..256{if e.debug_step(s,wire::StepKind::Continue).unwrap().0==crate::engine::DebugProgress::Complete{return;}}panic!("actual Screen description did not finish");}
#[test]
fn actual_screen_factory_checkpoint_cold_regenerates_witness_without_second_draw(){
 let(p,mut e,mut s)=fixture();let before=serde_json::to_value(&e.state).unwrap();
 let save=e.capture_save15(&p,Some(&s),1,"actual initial Screen".into()).unwrap();let bytes=save.encode().unwrap();
 assert!(!bytes.contains("screen_operation")&&!bytes.contains("ConstructionWitness"));
 assert_eq!(serde_json::to_value(&e.state).unwrap(),before);finish(&mut e,&mut s);let expected=serde_json::to_value(&e.state).unwrap();assert_eq!(e.renderer.batches,1);
 let mut cold=Engine::from_program_with_debugger(p.clone(),Host::default(),16).unwrap();cold.state.random=RandomState::seeded(775);
 let mut restored=cold.restore_save15(&p,crate::save::consumer_checkpoint::Save15::decode(&bytes).unwrap()).unwrap().unwrap();
 assert!(cold.state.ui.screens.is_empty());let base=cold.renderer.batches;finish(&mut cold,&mut restored);
 assert_eq!(serde_json::to_value(&cold.state).unwrap(),expected);assert_eq!(cold.renderer.batches,base+1);
}
#[test]
fn absent_factory_witness_cannot_be_replaced_by_actual_scope_and_root(){
 let(_,e,s)=fixture();let shared=s.screen_probe_for_test();let mut c=e.screen_debug_pending_for_test(&s).unwrap();
 c.request.screen=None;shared.lock().unwrap().requested.as_mut().unwrap().screen=None;
 assert!(Probe::portable_trace(shared,&c).is_err());assert!(e.state.ui.screens.is_empty());assert_eq!(e.renderer.batches,0);
}
#[test]
fn foreign_actual_factory_witness_is_refused_without_rng_or_publication(){
 let(_,e,s)=fixture();let(_,foreign,other)=fixture();let shared=s.screen_probe_for_test();let mut c=e.screen_debug_pending_for_test(&s).unwrap();
 let witness=other.screen_probe_for_test().lock().unwrap().requested.as_ref().unwrap().screen.clone();
 c.request.screen=witness.clone();shared.lock().unwrap().requested.as_mut().unwrap().screen=witness;
 assert!(Probe::portable_trace(shared,&c).is_err());assert!(Probe::validate_screen_construction(other.screen_probe_for_test(),&e).is_err());
 assert_eq!(e.state.random,RandomState::seeded(775));assert_eq!(foreign.state.random,RandomState::seeded(775));assert_eq!(e.renderer.batches,0);
}
#[test]
fn stale_actual_source_and_revoked_operation_cannot_checkpoint_old_witness(){
 let(p,mut e,s)=fixture();e.state.vars.insert("changed".into(),Value::Int(1));
 assert!(Probe::validate_screen_construction(s.screen_probe_for_test(),&e).is_err());assert!(e.capture_save15(&p,Some(&s),1,"stale".into()).is_err());
 let(_,e,s)=fixture();let shared=s.screen_probe_for_test();let c=e.screen_debug_pending_for_test(&s).unwrap();Probe::revoke_screen_construction(shared);
 assert!(Probe::portable_trace(shared,&c).is_err());assert_eq!(e.renderer.batches,0);
}
#[test]
fn renderer_refusal_revokes_factory_witness_and_keeps_initial_screen_unpublished(){
 let(p,mut e,mut s)=fixture();let before=serde_json::to_value(&e.state).unwrap();e.renderer.reject=true;let mut failed=false;
 for _ in 0..256{match e.debug_step(&mut s,wire::StepKind::Continue){Err(_)=>{failed=true;break;},Ok((crate::engine::DebugProgress::Complete,_))=>panic!("real renderer refusal was ignored"),_=>{}}}
 assert!(failed);assert_eq!(serde_json::to_value(&e.state).unwrap(),before);assert_eq!(e.renderer.batches,0);
 assert!(s.screen_probe_for_test().lock().unwrap().screen_operation.is_none());assert!(e.capture_save15(&p,Some(&s),1,"failed".into()).is_err());
}
#[test]
fn forged_screen_request_scope_is_rejected_by_real_cold_preparation(){
 let(p,e,s)=fixture();let save=e.capture_save15(&p,Some(&s),1,"actual Screen".into()).unwrap();let bytes=save.encode().unwrap();
 let mut forged:serde_json::Value=serde_json::from_str(&bytes).unwrap();
 assert_eq!(forged["debugger"]["trace"]["pending"]["request"]["scope"],"screen:panel");
 forged["debugger"]["trace"]["pending"]["request"]["scope"]="screen:other".into();
 let mut cold=Engine::from_program_with_debugger(p.clone(),Host::default(),16).unwrap();let before=serde_json::to_value(&cold.state).unwrap();let batches=cold.renderer.batches;
 assert!(cold.restore_save15(&p,crate::save::consumer_checkpoint::Save15::decode(&forged.to_string()).unwrap()).is_err());
 assert_eq!(serde_json::to_value(&cold.state).unwrap(),before);assert_eq!(cold.renderer.batches,batches);
}
#[test]
fn actual_screen_restore_recapture_and_second_cold_keep_same_pending_rng(){
 let(p,mut original,mut original_session)=fixture();let initial=original.capture_save15(&p,Some(&original_session),1,"first cold".into()).unwrap().encode().unwrap();
 let first_wire:serde_json::Value=serde_json::from_str(&initial).unwrap();
 let mut once=Engine::from_program_with_debugger(p.clone(),Host::default(),16).unwrap();
 let mut once_session=once.restore_save15(&p,crate::save::consumer_checkpoint::Save15::decode(&initial).unwrap()).unwrap().unwrap();
 // No step, refresh, synthetic owner or RNG call between restore and capture.
 let recaptured=once.capture_save15(&p,Some(&once_session),2,"recaptured immediately".into()).unwrap().encode().unwrap();
 let second_wire:serde_json::Value=serde_json::from_str(&recaptured).unwrap();
 for part in ["request","state"]{
  let random=if part=="request"{&first_wire["debugger"]["trace"]["pending"][part]["state"]["random"]}else{&first_wire["debugger"]["trace"]["pending"][part]["random"]};
  let recaptured_random=if part=="request"{&second_wire["debugger"]["trace"]["pending"][part]["state"]["random"]}else{&second_wire["debugger"]["trace"]["pending"][part]["random"]};
  assert!(!random.is_null());assert!(!recaptured_random.is_null());assert_eq!(random,recaptured_random);
 }
 let mut twice=Engine::from_program_with_debugger(p.clone(),Host::default(),16).unwrap();
 let mut twice_session=twice.restore_save15(&p,crate::save::consumer_checkpoint::Save15::decode(&recaptured).unwrap()).unwrap().unwrap();
 twice.capture_save15(&p,Some(&twice_session),3,"second immediate capture".into()).unwrap();
 finish(&mut original,&mut original_session);finish(&mut once,&mut once_session);finish(&mut twice,&mut twice_session);
 assert_eq!(serde_json::to_value(&once.state).unwrap(),serde_json::to_value(&original.state).unwrap());
 assert_eq!(serde_json::to_value(&twice.state).unwrap(),serde_json::to_value(&original.state).unwrap());
}
#[test]
fn ordinary_unfrozen_legacy_ui_open_remains_an_actual_debugger_operation(){
 let source=r#"screen panel(){return component("root","column",{},[])}
label start
ui.open("panel",[],false,1)
"ready"
"#;
 let script=rvn_parser::parse(source).unwrap();
 let label_pc=script.iter().position(|statement|matches!(statement,Statement::Label{name}if name=="start")).unwrap();
 let mut e=Engine::new(script.clone(),Host::default(),16).unwrap();e.enter_label("start").unwrap();e.renderer.batches=0;
 // Legacy enter_label selects the Label itself. A real Statement executes
 // only that Label, without opening the following Screen.
 assert_eq!(e.state.pc,label_pc);let random=e.state.random;
 let label_before=serde_json::to_value(&e.state).unwrap();
 let mut label=e.debug_begin(crate::engine::DebugOperation::Statement,vec![]).unwrap();
 assert_eq!(serde_json::to_value(&e.state).unwrap(),label_before);
 assert!(label.screen_probe_for_test().lock().unwrap().screen_operation.is_none());
 finish(&mut e,&mut label);
 assert_eq!(e.state.pc,label_pc+1);assert!(e.state.ui.screens.is_empty());
 assert_eq!(e.state.random,random);assert_eq!(e.renderer.batches,0);
 assert!(label.screen_probe_for_test().lock().unwrap().screen_operation.is_none());
 // This second genuine command is the source-free Legacy UiOpen.
 let before=serde_json::to_value(&e.state).unwrap();let mut s=e.debug_begin(crate::engine::DebugOperation::Statement,vec![]).unwrap();
 assert!(s.screen_probe_for_test().lock().unwrap().screen_operation.is_none());assert_eq!(serde_json::to_value(&e.state).unwrap(),before);
 finish(&mut e,&mut s);assert_eq!(e.state.ui.screens.len(),1);assert_eq!(e.state.ui.screens[0].name,"panel");
 assert_eq!(e.state.pc,label_pc+2);
 // Ordinary non-Host UiOpen allocates one real Screen RNG stream from Story.
 // This seed is required even when the Screen description is a constant.
 let mut expected_story=random;let expected_screen=RandomState::seeded(expected_story.next_u64());
 assert_eq!(e.state.random,expected_story);assert_eq!(e.state.ui.screens[0].random,expected_screen);
 // Compare both actual public one-statement operations without a debugger.
 // Only the initial RNG seed is aligned; no state, owner or result is copied.
 let mut ordinary=Engine::new(script,Host::default(),16).unwrap();ordinary.state.random=random;
 ordinary.enter_label("start").unwrap();assert_eq!(ordinary.state.pc,label_pc);
 ordinary.preview_step_statement().unwrap();assert_eq!(ordinary.state.pc,label_pc+1);
 assert!(ordinary.state.ui.screens.is_empty());assert_eq!(ordinary.state.random,random);
 ordinary.preview_step_statement().unwrap();assert_eq!(ordinary.state.pc,label_pc+2);
 assert_eq!(ordinary.state.random,expected_story);assert_eq!(ordinary.state.ui.screens[0].random,expected_screen);
 assert_eq!(serde_json::to_value(&e.state).unwrap(),serde_json::to_value(&ordinary.state).unwrap());
 let committed=serde_json::to_value(&e.state).unwrap();
 let views=e.interface_views().unwrap();assert_eq!(views.len(),1);assert_eq!(views[0].name,"panel");
 assert_eq!(views,ordinary.interface_views().unwrap());assert_eq!(e.interface_views().unwrap(),views);
 assert_eq!(serde_json::to_value(&e.state).unwrap(),committed);
 assert!(s.screen_probe_for_test().lock().unwrap().screen_operation.is_none());
}
