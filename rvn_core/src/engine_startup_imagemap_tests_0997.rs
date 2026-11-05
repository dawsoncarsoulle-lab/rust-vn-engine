//! Actual Core Startup owner tests. Backend image/input tests are independent.
use super::*;
use std::sync::{Arc,Mutex,atomic::{AtomicBool,AtomicUsize,Ordering}};
use rvn_parser::{Hotspot,Position,Transition};

struct Host{published:Arc<Mutex<Vec<crate::renderer::SceneRenderBatch>>>,reject:Arc<AtomicBool>}
impl Renderer for Host{
 fn supports_programmable_ui(&self)->bool{true}
 fn validate_scene_batch(&self,_:&crate::renderer::SceneRenderBatch)->Result<(),String>{if self.reject.load(Ordering::Relaxed){Err("actual test host refuses publication".into())}else{Ok(())}}
 fn publish_scene_batch(&mut self,b:crate::renderer::SceneRenderBatch){self.published.lock().unwrap().push(b);}
 fn set_background(&mut self,_:&str,_:&Transition){}
 fn show_sprite(&mut self,_:&str,_:Option<&str>,_:&Position,_:&Transition,_:Option<&crate::SpriteState>){}
 fn hide_sprite(&mut self,_:&str,_:&Transition,_:&crate::SpriteState){}
 fn move_sprite(&mut self,_:&str,_:&Position,_:&Transition,_:&crate::SpriteState){}
 fn show_dialogue(&mut self,_:Option<&str>,_:&str){}
 fn show_choice(&mut self,_:&[String])->usize{panic!("Startup never asks an ordinary renderer for a default input answer")}
 fn show_imagemap(&mut self,_:&str,_:Option<&str>,_:&[Hotspot])->usize{panic!("Startup never invents an input selection")}
 fn music_play(&mut self,_:&str,_:&Transition,_:Option<&str>){}
 fn music_stop(&mut self,_:&Transition){}
 fn music_set_volume(&mut self,_:f32){}
 fn sfx_play(&mut self,_:&str,_:&Transition){}
 fn sfx_stop(&mut self,_:&str,_:&Transition){}
}
impl StartupBatchSink for Host{
 type PreparedPublication=StartupPublicationSource;
 type Publication=crate::renderer::SceneRenderBatch;
 fn prepare_startup_publication(&self,s:&StartupPublicationSource,b:&crate::renderer::SceneRenderBatch)->Result<Self::PreparedPublication,String>{self.validate_scene_batch(b)?;Ok(s.clone())}
 fn publish_startup_publication(&mut self,s:StartupPublicationSource,b:crate::renderer::SceneRenderBatch,p:Self::PreparedPublication)->Self::Publication{assert_eq!(s,p);self.publish_scene_batch(b.clone());b}
}
impl StartupInputBatchSink for Host{fn validate_startup_input_host(&self)->Result<(),String>{Ok(())}}
fn programme_source(name:&str,source:&str)->ParsedProgram{
 static NEXT:AtomicUsize=AtomicUsize::new(0);
 let root=std::path::PathBuf::from(std::env::var("RVN_NATIVE_IMAGEMAP_FIXTURES").expect("isolated private D fixtures"));
 let directory=root.join(format!("startup-input-{name}-{}-{}",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)));std::fs::create_dir_all(&directory).unwrap();
 std::fs::write(directory.join("main.rvn"),source).unwrap();rvn_parser::parse_file_program_with_uses(directory.join("main.rvn")).unwrap()
}
fn programme(name:&str)->ParsedProgram{
 let source=r#"init{set before=1 flow.consume_v2("actual_map","map"){capture "draw"=random(1,9) imagemap{background:"garden.png" hotspot{name:"left" area:(0,0,50,100)}=>{set selected=1} hotspot{name:"empty" area:(50,0,100,100)}=>{}}} set after=2}
scene label start
"Boundary"
"#;
 programme_source(name,source)
}
fn host()->Host{Host{published:Default::default(),reject:Default::default()}}
fn pending(program:ParsedProgram)->EngineStartup<Host>{Engine::prepare_input_startup(program,host(),8,HashMap::new(),TextRuntimeContext::default(),None).unwrap()}
fn waiting(s:&mut EngineStartup<Host>){for _ in 0..100{if s.waiting_for_imagemap(){return;}let p=s.prepare_step(StepKind::Continue).unwrap();s.begin_prepared_step_commit(p).unwrap().commit();}panic!("actual Init input was not reached")}
fn frozen(s:&EngineStartup<Host>)->serde_json::Value{serde_json::json!({"state":s.state(),"snapshot":s.snapshot(),"startup":s.engine.portable_startup()})}
fn source(s:&EngineStartup<Host>)->StartupImagemapSource{s.imagemap_input_source(Arc::new(s.observation_source().unwrap())).unwrap()}

#[test]
fn startup_imagemap0997_actual_wait_and_dropped_selection_preserve_cursor_gather_rng_and_publication(){
 let mut s=pending(programme("drop"));waiting(&mut s);assert!(s.session.is_none());assert!(matches!(s.imagemap_interaction().unwrap(),Some(Interaction::Imagemap{hotspots,..})if hotspots.len()==2));
 assert!(s.prepare_step(StepKind::Continue).is_err());assert!(s.validate_driver_run(StepKind::Into).is_err());
 let before=frozen(&s);let count=s.engine.renderer.published.lock().unwrap().len();let input=source(&s);
 assert!(s.prepare_imagemap_selection_boxed(&input,2).is_err());assert_eq!(frozen(&s),before);
 let p=s.prepare_imagemap_selection_boxed(&input,0).unwrap();assert!(p.render_batch().unwrap().display.is_none());assert!(!p.render_batch().unwrap().state.vars.contains_key("selected"));assert_eq!(frozen(&s),before);drop(p);
 assert_eq!(frozen(&s),before);assert_eq!(s.engine.renderer.published.lock().unwrap().len(),count);assert!(s.accepts_imagemap_input_source(&input));
}
#[test]
fn startup_imagemap0997_committed_selection_enters_only_the_actual_branch_once_and_revokes_old_input(){
 let mut s=pending(programme("commit"));waiting(&mut s);let random=s.state().random;let input=source(&s);let p=s.prepare_imagemap_selection_boxed(&input,0).unwrap();let duplicate=s.prepare_imagemap_selection_boxed(&input,0).unwrap();
 s.begin_prepared_step_commit(*p).unwrap().commit();assert!(!s.accepts_imagemap_input_source(&input));assert!(s.begin_prepared_step_commit(*duplicate).is_err());assert!(!s.state().vars.contains_key("selected"));assert!(!s.waiting_for_imagemap());assert_eq!(s.state().random,random);
 for _ in 0..100{if s.is_complete(){break;}let p=s.prepare_step(StepKind::Continue).unwrap();s.begin_prepared_step_commit(p).unwrap().commit();}
 assert!(s.is_complete());assert_eq!(s.state().vars["selected"],Value::Int(1));assert_eq!(s.state().vars["after"],Value::Int(2));assert_eq!(s.state().random,random);
 let mut empty=pending(programme("empty"));waiting(&mut empty);let input=source(&empty);let p=empty.prepare_imagemap_selection_boxed(&input,1).unwrap();empty.begin_prepared_step_commit(*p).unwrap().commit();
 for _ in 0..100{if empty.is_complete(){break;}let p=empty.prepare_step(StepKind::Continue).unwrap();empty.begin_prepared_step_commit(p).unwrap().commit();}
 assert!(empty.is_complete());assert!(!empty.state().vars.contains_key("selected"));assert_eq!(empty.state().vars["after"],Value::Int(2));
}
#[test]
fn startup_imagemap0997_foreign_owner_renderer_refusal_and_configuration_changes_are_atomic(){
 let mut s=pending(programme("guards"));waiting(&mut s);let mut foreign=pending(programme("foreign"));waiting(&mut foreign);let input=source(&s);assert!(foreign.prepare_imagemap_selection_boxed(&input,0).is_err());
 let before=frozen(&s);s.engine.renderer.reject.store(true,Ordering::Relaxed);assert!(s.prepare_imagemap_selection_boxed(&input,0).is_err());assert_eq!(frozen(&s),before);s.engine.renderer.reject.store(false,Ordering::Relaxed);
 let p=s.prepare_imagemap_selection_boxed(&input,0).unwrap();s.configure(vec![],vec![wire::Watch{scope:wire::WatchScope::Globals,name:"before".into()}]).unwrap();let configured=frozen(&s);assert!(s.begin_prepared_step_commit(*p).is_err());assert_eq!(frozen(&s),configured);assert!(!s.accepts_imagemap_input_source(&input));
}
#[test]
fn startup_imagemap0997_save15_restores_real_wait_without_saving_an_input_capability_or_replaying_gather(){
 let program=programme("save");let mut s=pending(program.clone());waiting(&mut s);let random=s.state().random;let old=source(&s);
 let save=s.capture_save15(1,"actual Init input".into()).unwrap();let encoded=save.encode().unwrap();assert!(!encoded.contains("StartupImagemapSource"));assert!(!encoded.contains("StartupObservationSource"));
 let mut cold=pending(program);cold.restore_save15(crate::save::consumer_checkpoint::Save15::decode(&encoded).unwrap()).unwrap();assert!(cold.waiting_for_imagemap());assert_eq!(cold.state().random,random);assert!(!cold.accepts_imagemap_input_source(&old));
 let input=source(&cold);let p=cold.prepare_imagemap_selection_boxed(&input,0).unwrap();cold.begin_prepared_step_commit(*p).unwrap().commit();assert_eq!(cold.state().random,random);
}
#[test]
fn startup_imagemap0997_ordinary_and_raw_ast_constructors_do_not_acquire_input_authority(){
 let program=programme("ordinary");assert!(Engine::from_program(program.clone(),host(),8).is_err());assert!(Engine::new(program.script.clone(),host(),8).is_err());
 assert!(Engine::prepare_debugger_startup(program,host(),8,HashMap::new(),TextRuntimeContext::default(),None).is_err());
}
#[test]
fn startup_imagemap0997_forged_wait_route_origin_and_gather_refuse_before_live_restore(){
 let program=programme("restore-guards");let mut live=pending(program.clone());waiting(&mut live);let save=live.capture_save15(2,"exact Init wait".into()).unwrap();
 for tamper in 0..4{
  let mut forged=save.clone();let saved=forged.debugger.as_mut().unwrap().startup.as_mut().unwrap();let frame=saved.imagemaps.last_mut().unwrap();
  match tamper{
   0=>frame.selected=Some(0),
   1=>frame.header=vec![wire::Route::Statement(0)],
   2=>frame.header_origin.span=Some(0..1),
   3=>frame.gather.clear(),
   _=>unreachable!(),
  }
  let mut cold=pending(program.clone());let before=frozen(&cold);assert!(cold.restore_save15(forged).is_err(),"tamper{tamper}");assert_eq!(frozen(&cold),before,"tamper{tamper}");assert!(cold.engine.renderer.published.lock().unwrap().is_empty());
 }
 let mut changed=program;changed.sources[0].source=changed.sources[0].source.replace("selected=1","selected=3");assert!(Engine::prepare_input_startup(changed,host(),8,HashMap::new(),TextRuntimeContext::default(),None).is_err());
}

fn finish_input(s:&mut EngineStartup<Host>){
 for _ in 0..1000{if s.is_complete(){return;}assert!(!s.waiting_for_imagemap(),"test must select each actual input explicitly");let p=s.prepare_step(StepKind::Continue).unwrap();s.begin_prepared_step_commit(p).unwrap().commit();}
 panic!("actual selected Init did not terminate");
}
#[test]
fn startup_imagemap0998_nested_hotspot_wait_keeps_ancestors_and_cold_child_returns_to_real_parent(){
 let program=programme_source("nested",r#"init{set selected=0 flow.consume_v2("outer","map"){capture "draw"=random(1,9) imagemap{background:"outer.png" hotspot{name:"enter" area:(0,0,100,100)}=>{set parent=1 flow.consume_v2("inner","map"){capture "draw"=random(1,9) imagemap{background:"inner.png" hotspot{name:"choose" area:(0,0,100,100)}=>{set selected=selected+1}}} set returned=1}}} set done=1}
scene label start
"Boundary"
"#);
 let mut live=pending(program.clone());let mut expected=live.state().random;expected.integer(1,9).unwrap();waiting(&mut live);assert_eq!(live.state().random,expected);
 let old=source(&live);let p=live.prepare_imagemap_selection_boxed(&old,0).unwrap();live.begin_prepared_step_commit(*p).unwrap().commit();waiting(&mut live);expected.integer(1,9).unwrap();assert_eq!(live.state().random,expected);assert_eq!(live.state().vars["parent"],Value::Int(1));assert_eq!(live.engine.startup.as_ref().unwrap().imagemaps.len(),2);assert!(!live.accepts_imagemap_input_source(&old));
 let save=live.capture_save15(3,"nested actual wait".into()).unwrap();let mut cold=pending(program);cold.restore_save15(crate::save::consumer_checkpoint::Save15::decode(&save.encode().unwrap()).unwrap()).unwrap();assert_eq!(cold.state().random,expected);assert_eq!(cold.engine.startup.as_ref().unwrap().imagemaps.len(),2);
 let input=source(&cold);let p=cold.prepare_imagemap_selection_boxed(&input,0).unwrap();cold.begin_prepared_step_commit(*p).unwrap().commit();finish_input(&mut cold);assert_eq!(cold.state().random,expected);assert_eq!(cold.state().vars["selected"],Value::Int(1));assert_eq!(cold.state().vars["returned"],Value::Int(1));assert_eq!(cold.state().vars["done"],Value::Int(1));assert!(cold.engine.startup.is_none());
}
#[test]
fn startup_imagemap0998_repeated_loop_header_and_cold_selected_child_do_not_replay_or_accept_sibling_forgery(){
 let program=programme_source("loop-selected",r#"init{set visits=0 while visits<2 {flow.consume_v2("repeat","map"){capture "draw"=random(1,9) imagemap{background:"repeat.png" hotspot{name:"left" area:(0,0,50,100)}=>{set visits=visits+1} hotspot{name:"right" area:(50,0,100,100)}=>{set visits=visits+1}}}} set done=1}
scene label start
"Boundary"
"#);
 let mut live=pending(program.clone());let mut expected=live.state().random;expected.integer(1,9).unwrap();waiting(&mut live);let header=live.engine.startup.as_ref().unwrap().route();let first=source(&live);let p=live.prepare_imagemap_selection_boxed(&first,0).unwrap();live.begin_prepared_step_commit(*p).unwrap().commit();assert_eq!(live.state().vars["visits"],Value::Int(0));
 let save=live.capture_save15(4,"actual selected child".into()).unwrap();let mut forged=save.clone();forged.debugger.as_mut().unwrap().startup.as_mut().unwrap().imagemaps.last_mut().unwrap().selected=Some(1);let mut refuse=pending(program.clone());let before=frozen(&refuse);assert!(refuse.restore_save15(forged).is_err());assert_eq!(frozen(&refuse),before);
 let mut cold=pending(program);cold.restore_save15(crate::save::consumer_checkpoint::Save15::decode(&save.encode().unwrap()).unwrap()).unwrap();assert!(!cold.waiting_for_imagemap());assert_eq!(cold.state().random,expected);waiting(&mut cold);expected.integer(1,9).unwrap();assert_eq!(cold.state().random,expected);assert_eq!(cold.state().vars["visits"],Value::Int(1));assert_eq!(cold.engine.startup.as_ref().unwrap().route(),header);assert_eq!(cold.engine.startup.as_ref().unwrap().imagemaps.len(),1);assert!(!cold.accepts_imagemap_input_source(&first));
 let next=source(&cold);let p=cold.prepare_imagemap_selection_boxed(&next,0).unwrap();cold.begin_prepared_step_commit(*p).unwrap().commit();finish_input(&mut cold);assert_eq!(cold.state().random,expected);assert_eq!(cold.state().vars["visits"],Value::Int(2));assert_eq!(cold.state().vars["done"],Value::Int(1));
}
#[test]
fn startup_imagemap0998_zero_hotspot_gathers_exactly_once_and_never_issues_an_input_source(){
 let program=programme_source("zero",r#"init{flow.consume_v2("zero","map"){capture "draw"=random(1,9) imagemap{background:"unused.png"}} set done=1}
scene label start
"Boundary"
"#);
 let mut live=pending(program);let mut expected=live.state().random;expected.integer(1,9).unwrap();
 for _ in 0..100{if live.is_complete(){break;}assert!(!live.waiting_for_imagemap());assert!(live.imagemap_interaction().unwrap().is_none());let observation=Arc::new(live.observation_source().unwrap());assert!(live.imagemap_input_source(observation).is_err());let p=live.prepare_step(StepKind::Continue).unwrap();assert!(p.imagemap_interaction().is_none());live.begin_prepared_step_commit(p).unwrap().commit();}
 assert!(live.is_complete());assert_eq!(live.state().random,expected);assert_eq!(live.state().vars["done"],Value::Int(1));assert!(live.engine.renderer.published.lock().unwrap().iter().all(|batch|batch.display.is_none()));
}
