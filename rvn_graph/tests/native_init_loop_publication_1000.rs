//! The three existing D-CATALOG-INIT-* criteria through actual Graph Init owners.
//! StandardMacros UI behavior is still an explicit external observation obligation.
use rvn_core::{Engine, Renderer, renderer::SceneRenderBatch, types::SpriteState};
use rvn_core::engine::EngineStartup;
use rvn_core::editor_debugger::StepKind;
use rvn_graph::*;
use rvn_parser::{Hotspot, ParsedProgram, Position, Transition, Value};
use std::{collections::HashMap,path::PathBuf,sync::{Arc,Mutex,atomic::{AtomicBool,AtomicUsize,Ordering}}};
#[path="../../rvn_core/tests/support/strict_native_consumer.rs"] mod strict_native_consumer;
#[derive(Clone,Default)] struct Host { batches:Arc<Mutex<Vec<SceneRenderBatch>>>,reject:Arc<AtomicBool> }
impl Renderer for Host {
 fn validate_scene_batch(&self,_:&SceneRenderBatch)->Result<(),String>{if self.reject.load(Ordering::SeqCst){Err("Actual Init loop renderer refusal".into())}else{Ok(())}}
 fn publish_scene_batch(&mut self,b:SceneRenderBatch){self.batches.lock().unwrap().push(b);}
 fn set_background(&mut self,_:&str,_:&Transition){}
 fn show_sprite(&mut self,_:&str,_:Option<&str>,_:&Position,_:&Transition,_:Option<&SpriteState>){}
 fn hide_sprite(&mut self,_:&str,_:&Transition,_:&SpriteState){}
 fn move_sprite(&mut self,_:&str,_:&Position,_:&Transition,_:&SpriteState){}
 fn show_dialogue(&mut self,_:Option<&str>,_:&str){}
 fn show_choice(&mut self,_:&[String])->usize{panic!("No fabricated choice")}
 fn show_imagemap(&mut self,_:&str,_:Option<&str>,_:&[Hotspot])->usize{panic!("No fabricated input")}
 fn music_play(&mut self,_:&str,_:&Transition,_:Option<&str>){}
 fn music_stop(&mut self,_:&Transition){}
 fn music_set_volume(&mut self,_:f32){}
 fn sfx_play(&mut self,_:&str,_:&Transition){}
 fn sfx_stop(&mut self,_:&str,_:&Transition){}
}
fn directory(name:&str)->PathBuf {
 static NEXT:AtomicUsize=AtomicUsize::new(0);
 let p=PathBuf::from(std::env::var("RVN_INIT_LOOP_FIXTURES").expect("private actual Init source directory"))
  .join(format!("graph-init-publication1000-{name}-{}-{}",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)));
 std::fs::create_dir_all(&p).unwrap();p
}
fn program(name:&str,body:&str,kinds:&[NodeKind])->ParsedProgram {
 let dir=directory(name);let file=dir.join("authored.rvn");
 std::fs::write(&file,format!("init {{set seen=0\n{body}\nset done=true\n}}\nscene label start\n\"ready\"\n")).unwrap();
 let original=rvn_parser::parse_file_program_with_uses(file).unwrap();let graphs=import_script(&original.script).unwrap();
 let init=graphs.iter().find(|g|matches!(g.kind,GraphKind::Init)).unwrap();
 for kind in kinds {assert!(init.nodes.values().any(|n|n.kind==*kind),"Actual Init lacks {kind:?}");}
 strict_native_consumer::finish_program(dir,original,graphs,kinds)
}
fn startup(p:ParsedProgram,host:Host)->EngineStartup<Host>{
 Engine::prepare_debugger_startup(p,host,16,HashMap::new(),Default::default(),None).unwrap()
}
fn state(owner:&EngineStartup<Host>)->serde_json::Value{serde_json::to_value(owner.state()).unwrap()}
// Every witnessed publishing step is rejected at preparation and final commit,
// then retried. Candidate computation/drop never changes the actual owner.
fn checked_step(owner:&mut EngineStartup<Host>,host:&Host)->bool {
 let before=state(owner);let snapshot=owner.snapshot();let source=owner.observation_source().unwrap();let count=host.batches.lock().unwrap().len();
 let prepared=owner.prepare_step(StepKind::Continue).unwrap();let publishes=prepared.render_batch().is_some();
 assert_eq!(state(owner),before);assert_eq!(owner.snapshot(),snapshot);assert!(owner.accepts_observation_source(&source));assert_eq!(host.batches.lock().unwrap().len(),count);
 drop(prepared);assert_eq!(state(owner),before);assert!(owner.accepts_observation_source(&source));
 let prepared=owner.prepare_step(StepKind::Continue).unwrap();
 if publishes {
  host.reject.store(true,Ordering::SeqCst);
  let error=owner.prepare_step(StepKind::Continue).err().expect("Actual publishing step must hit renderer refusal").to_string();assert!(error.contains("Actual Init loop renderer refusal"),"Preparation refused for wrong reason: {error}");
  let error=owner.begin_prepared_step_commit(prepared).err().expect("Previously admitted batch must be revalidated").to_string();assert!(error.contains("Actual Init loop renderer refusal"),"Commit refused for wrong reason: {error}");
  assert_eq!(state(owner),before);assert_eq!(owner.snapshot(),snapshot);assert!(owner.accepts_observation_source(&source));assert_eq!(host.batches.lock().unwrap().len(),count);
  host.reject.store(false,Ordering::SeqCst);
  let prepared=owner.prepare_step(StepKind::Continue).unwrap();owner.begin_prepared_step_commit(prepared).unwrap().commit();
  assert_eq!(host.batches.lock().unwrap().len(),count+1);
 }else {owner.begin_prepared_step_commit(prepared).unwrap().commit();assert_eq!(host.batches.lock().unwrap().len(),count);}
 publishes
}
fn finish(owner:&mut EngineStartup<Host>,host:&Host)->usize {
 let mut publications=0;for _ in 0..512{if owner.is_complete(){return publications;}publications+=usize::from(checked_step(owner,host));}panic!("Bounded finite real Init failed to complete")
}
fn run(name:&str,body:&str,kinds:&[NodeKind],seen:i64,draws:usize){
 let p=program(name,body,kinds);let host=Host::default();let mut warm=startup(p.clone(),host.clone());
 assert!(!warm.is_complete());assert!(!warm.state().vars.contains_key("done"));assert!(warm.prepare_gameplay_entry(Some("start")).is_err(),"Pending Init has no gameplay handoff authority");assert!(host.batches.lock().unwrap().is_empty());
 let initial_random=warm.state().random;let mut expected_random=initial_random;
 // The fixtures draw solely in the authored headers, all in the same integer
 // domain. Bounds below deliberately use distinct fixed ranges so no draw can
 // be lost or replayed unnoticed.
 match name {"while"=>{for _ in 0..draws{expected_random.integer(1,9).unwrap();}},"each"=>{for _ in 0..draws{expected_random.integer(1,9).unwrap();}},"range"=>{expected_random.integer(1,1).unwrap();expected_random.integer(3,3).unwrap();},_=>{assert_eq!(draws,0);}}
 let mut cold=None;let mut publications=0;
 for _ in 0..512 {
  if warm.is_complete(){break;}
  publications+=usize::from(checked_step(&mut warm,&host));
  if cold.is_none()&&!warm.is_complete()&&matches!(warm.state().vars.get("seen"),Some(Value::Int(n))if *n>=1){
   let before=state(&warm);let observation=warm.observation_source().unwrap();let count=host.batches.lock().unwrap().len();
   let bytes=warm.capture_save15(1,"actual Graph Init pending child".into()).unwrap().encode().unwrap();assert_eq!(state(&warm),before);assert!(warm.accepts_observation_source(&observation));assert_eq!(host.batches.lock().unwrap().len(),count);
   let file=p.sources[0].path.as_ref().unwrap();let relocated=file.parent().unwrap().join("cold-relocated.rvn");std::fs::copy(file,&relocated).unwrap();let cp=rvn_parser::parse_file_program_with_uses(&relocated).unwrap();assert_ne!(cp.sources[0].path,p.sources[0].path);let ch=Host::default();let mut co=startup(cp,ch.clone());
   co.restore_save15(rvn_core::save::consumer_checkpoint::Save15::decode(&bytes).unwrap()).unwrap();assert_eq!(co.state().vars,warm.state().vars);assert_eq!(co.state().random,warm.state().random);assert!(!co.is_complete());
   // Change actual source bytes through a real cold file parse. The save may
   // not silently rebind to that other source even when executable AST matches.
   let changed=relocated.with_file_name("changed.rvn");let mut text=std::fs::read_to_string(&relocated).unwrap();text.push_str("\n// different actual frozen source\n");std::fs::write(&changed,text).unwrap();let other=rvn_parser::parse_file_program_with_uses(changed).unwrap();let oh=Host::default();let mut refused=startup(other,oh.clone());let old=state(&refused);let old_source=refused.observation_source().unwrap();let old_count=oh.batches.lock().unwrap().len();let error=refused.restore_save15(rvn_core::save::consumer_checkpoint::Save15::decode(&bytes).unwrap()).unwrap_err().to_string();assert!(error.contains("Portable consumer programme differs from actual frozen bytes/order/AST/source ordinals"),"Real changed-source refusal: {error}");assert_eq!(state(&refused),old);assert!(refused.accepts_observation_source(&old_source));assert_eq!(oh.batches.lock().unwrap().len(),old_count);
   cold=Some((co,ch));
  }
 }
 assert!(warm.is_complete());assert!(publications>0);assert_eq!(warm.state().vars["seen"],Value::Int(seen));assert_eq!(warm.state().vars["done"],Value::Bool(true));assert!(!warm.state().vars.contains_key("unreachable"));assert_eq!(warm.state().random,expected_random,"Each real header draw must occur once at its actual visit");
 let(mut cold,cold_host)=cold.expect("A genuine pending child after first publication must be saveable");finish(&mut cold,&cold_host);assert!(cold.is_complete());assert_eq!(cold.state().vars,warm.state().vars);assert_eq!(cold.state().random,warm.state().random);
 if name=="nested" {for owner in [&warm,&cold] {assert_eq!(owner.state().vars["i"],Value::Int(2),"The outer Range reaches its own authored break");assert_eq!(owner.state().vars["j"],Value::Int(1),"The inner break must retire only its nearest real Range");}}
 for (mut complete,backend) in [(warm,host),(cold,cold_host)] {
  let before=state(&complete);let rng=complete.state().random;let count=backend.batches.lock().unwrap().len();let source=complete.observation_source().unwrap();
  let entry=complete.prepare_gameplay_entry(Some("start")).unwrap();drop(entry);assert_eq!(state(&complete),before);assert!(complete.accepts_observation_source(&source));assert_eq!(backend.batches.lock().unwrap().len(),count);
  let entry=complete.prepare_gameplay_entry(Some("start")).unwrap();complete.begin_prepared_step_commit(entry).unwrap().commit();assert_eq!(backend.batches.lock().unwrap().len(),count+1);
  let mut engine=complete.into_engine().unwrap();assert!(engine.has_frozen_startup_program());engine.step_until_interaction().unwrap();assert_eq!(engine.state.vars["seen"],Value::Int(seen));assert_eq!(engine.state.vars["done"],Value::Bool(true));assert_eq!(engine.state.random,rng);assert!(matches!(engine.displayed_interaction().unwrap(),Some(rvn_core::Interaction::Dialogue{text,..})if text=="ready"));
 }
}
#[test]
fn strict_graph_three_init_loops_rejection_cold_and_unique_handoff(){
 run("while","while random(1,9)>0 and seen<3 {set seen=seen+1}",&[NodeKind::While],3,4);
 run("each","for item in [random(1,9),random(1,9)] {set seen=seen+1}",&[NodeKind::ForEach],2,2);
 run("range","flow.for_loop(i,random(1,1),random(3,3)) {set seen=seen+1}",&[NodeKind::ForLoopWithBreak],3,2);
}
#[test]
fn strict_graph_nested_init_loop_break_and_changed_source_are_atomic(){
 // flow.break targets the nearest real ForLoopWithBreak, not While or
 // ForEach. One authored break belongs to each of the two actual Ranges;
 // their genuine single-incoming source identities remain representable.
 // The separate multi-Break-to-one-Range source-format gap remains open.
 run("nested","flow.for_loop(i,1,3) {for item in [1,2] {while seen<2 {flow.for_loop(j,1,3) {set seen=seen+1 if seen>0 {flow.break()} set unreachable=true}}} if i==2 {flow.break()}}",&[NodeKind::While,NodeKind::ForEach,NodeKind::ForLoopWithBreak],2,0);
}
