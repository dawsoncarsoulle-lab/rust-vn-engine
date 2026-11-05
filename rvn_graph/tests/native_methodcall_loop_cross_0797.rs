//! Four targeted cross-contract tests. Prepared only; no native/pixel acceptance.
use rvn_core::{Engine,Interaction,Renderer,renderer::SceneRenderBatch,types::SpriteState};
use rvn_core::{engine::DebugOperation,random::RandomState,video::{Playback,VideoView}};
use rvn_graph::*;
use rvn_parser::{Hotspot,ParsedProgram,Position,Statement,Transition,Value};
use std::{path::PathBuf,sync::{Arc,Mutex,atomic::{AtomicBool,AtomicU64,Ordering}}};

#[derive(Clone,Default)]
struct Host{batches:Arc<Mutex<Vec<SceneRenderBatch>>>,views:Arc<Mutex<Vec<VideoView>>>,reject:Arc<AtomicBool>}
impl Renderer for Host{
 fn supports_video(&self)->bool{true}
 fn validate_scene_batch(&self,_:&SceneRenderBatch)->Result<(),String>{if self.reject.load(Ordering::SeqCst){Err("cross0797 actual renderer refusal".into())}else{Ok(())}}
 fn publish_scene_batch(&mut self,b:SceneRenderBatch){*self.views.lock().unwrap()=b.videos.clone();self.batches.lock().unwrap().push(b);}
 fn update_videos(&mut self,v:&[VideoView])->Result<(),String>{if self.reject.load(Ordering::SeqCst){return Err("cross0797 actual renderer refusal".into());}*self.views.lock().unwrap()=v.to_vec();Ok(())}
 fn set_background(&mut self,_:&str,_:&Transition){}
 fn show_sprite(&mut self,_:&str,_:Option<&str>,_:&Position,_:&Transition,_:Option<&SpriteState>){}
 fn hide_sprite(&mut self,_:&str,_:&Transition,_:&SpriteState){}
 fn move_sprite(&mut self,_:&str,_:&Position,_:&Transition,_:&SpriteState){}
 fn show_dialogue(&mut self,_:Option<&str>,_:&str){}
 fn show_choice(&mut self,_:&[String])->usize{panic!("no fabricated choice")}
 fn show_imagemap(&mut self,_:&str,_:Option<&str>,_:&[Hotspot])->usize{panic!("no fabricated hotspot")}
 fn music_play(&mut self,_:&str,_:&Transition,_:Option<&str>){}
 fn music_stop(&mut self,_:&Transition){}
 fn music_set_volume(&mut self,_:f32){}
 fn sfx_play(&mut self,_:&str,_:&Transition){}
 fn sfx_stop(&mut self,_:&str,_:&Transition){}
}
fn output(name:&str)->PathBuf{
 static NEXT:AtomicU64=AtomicU64::new(0);
 let p=PathBuf::from(std::env::var("RVN_METHOD0797_FIXTURES").expect("fresh isolated D output root required"))
  .join(format!("{name}-{}-{}",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)));
 std::fs::create_dir_all(&p).unwrap();p
}
fn graphs(raw:&str,native_ranges:bool)->Vec<GraphDocument>{
 let ast=rvn_parser::parse(raw).unwrap();
 let mut docs=import_script(&ast).unwrap();
 for graph in &mut docs{
  graph.enable_native_frame_preparation().unwrap();
  if native_ranges{
   let loops:Vec<_>=graph.nodes.values().filter(|n|n.kind==NodeKind::ForLoopWithBreak).map(|n|n.id).collect();
   for node in loops{*graph=graph.prepare_native_loop_demand(node).unwrap().graph;}
  }
 }
 docs
}
fn materialize(name:&str,docs:&[GraphDocument])->ParsedProgram{
 let before=docs.to_vec();let compiled=transpile_project(docs).unwrap();assert_eq!(docs,before.as_slice());
 assert_eq!(rvn_parser::parse(&compiled.source).unwrap(),compiled.ast);
 let again=import_native_frame_script(&compiled.ast).unwrap();assert_eq!(transpile_project(&again).unwrap().ast,compiled.ast);
 for graph in docs.iter().filter(|g|!g.inspector_evaluation.is_legacy()){
  let actual=again.iter().find(|g|g.graph_id==graph.graph_id).unwrap();
  assert_eq!(actual.nodes.keys().collect::<Vec<_>>(),graph.nodes.keys().collect::<Vec<_>>());
  assert_eq!(actual.pins.keys().collect::<Vec<_>>(),graph.pins.keys().collect::<Vec<_>>());assert_eq!(actual.edges,graph.edges);
 }
 let root=output(name);for graph in docs{std::fs::write(root.join(format!("graph-{}.json",graph.graph_id.get())),graph.to_pretty_json().unwrap()).unwrap();}
 let path=root.join("actual.rvn");std::fs::write(&path,compiled.source).unwrap();rvn_parser::parse_file_program_with_uses(path).unwrap()
}
const SOURCE:&str=r#"
function bound0797(){local token=random(1,1) return 1}
function callback0797(){local token=random(1,1) return 7}
handler finished0797(event){set callback_hits=callback_hits+callback0797()}
init{set visits=0 set callback_hits=0}
scene label start
video.play("intro",video_clip("real-cross0797.webm",{"skippable":true,"on_end":"finished0797"}))
"Ready"
flow.for_loop(i,0,bound0797()){
 "enter [visits]"
 intro.pause()
 set visits=visits+1
 "paused [visits]"
}
intro.skip()
"Done"
"#;
fn program(name:&str)->ParsedProgram{
 let docs=graphs(SOURCE,true);let p=materialize(name,&docs);
 assert!(p.sources.iter().any(|s|s.source.contains("flow.consume_v3(")));
 assert!(p.sources.iter().any(|s|s.source.contains(":method_call|")));p
}
fn engine(p:&ParsedProgram)->Engine<Host>{let mut e=Engine::from_program_with_debugger(p.clone(),Host::default(),16).unwrap();e.state.random=RandomState::seeded(797);e.enter_label("start").unwrap();e}
fn text(e:&mut Engine<Host>)->String{match e.step_until_interaction().unwrap(){Some(Interaction::Dialogue{text,..})=>text,other=>panic!("real dialogue required: {other:?}")}}
fn expected(draws:usize)->RandomState{let mut r=RandomState::seeded(797);for _ in 0..draws{r.integer(1,1).unwrap();}r}
fn batches(e:&Engine<Host>)->usize{e.renderer.batches.lock().unwrap().len()}
fn frame_count(e:&Engine<Host>)->usize{
 let frames=&e.state.scene_execution.as_ref().unwrap().active.as_ref().unwrap().native_loop_frames_v1;
 serde_json::to_value(frames).unwrap().as_array().expect("actual transparent NativeLoopFramesV1 array").len()
}
fn require_pause(e:&Engine<Host>){assert!(matches!(e.peek_statement(),Some(Statement::ScheduledConsumer{version:2,body,..})if matches!(body.as_slice(),[Statement::MethodCall{target,method,arg:None,transition:Transition::None}]if target=="intro"&&method=="pause")));}
fn enter_first(e:&mut Engine<Host>){assert_eq!(text(e),"Ready");e.advance_dialogue().unwrap();assert_eq!(text(e),"enter 0");assert_eq!(e.state.random,expected(1));assert_eq!(frame_count(e),1);e.advance_dialogue().unwrap();require_pause(e);}
fn pause_once(e:&mut Engine<Host>){
 require_pause(e);let count=batches(e);let old=e.video_views().unwrap().remove(0);let rng=e.state.random;
 e.preview_step_statement().unwrap();assert_eq!(batches(e),count+1);assert_eq!(e.state.random,rng);
 let fresh=e.video_views().unwrap().remove(0);assert_eq!(fresh.track.playback,Playback::Paused);assert_ne!(fresh.resource_token,old.resource_token);
 let execution=e.state.scene_execution.as_ref().unwrap();let token=fresh.resource_token.as_ref().unwrap();assert!(execution.resources.accepts(token,e.runtime_epoch(),execution.owner()));
 let state=serde_json::to_vec(&e.state).unwrap();assert!(!e.resume_video_from_view(&old).unwrap());assert_eq!(serde_json::to_vec(&e.state).unwrap(),state);assert_eq!(batches(e),count+1);assert_eq!(frame_count(e),1);
}
fn finish_after_first_pause(e:&mut Engine<Host>){
 assert_eq!(text(e),"paused 1");assert_eq!(e.state.vars["visits"],Value::Int(1));assert_eq!(e.state.random,expected(1));
 e.advance_dialogue().unwrap();assert_eq!(text(e),"enter 1");assert_eq!(e.state.random,expected(2));e.advance_dialogue().unwrap();pause_once(e);
 assert_eq!(text(e),"paused 2");e.advance_dialogue().unwrap();assert_eq!(text(e),"Done");
 assert_eq!(e.state.vars["visits"],Value::Int(2));assert_eq!(e.state.vars["callback_hits"],Value::Int(7));
 assert_eq!(e.state.random,expected(4),"three real Last evaluations and one lexical callback function draw");assert_eq!(frame_count(e),0);
 assert_eq!(e.state.videos.tracks["intro"].playback,Playback::Ended);assert!(!e.state.vars.keys().any(|k|k.starts_with("rvn_native_")));
}
#[test]
fn cross0797_scene_loop_pause_each_invocation_and_real_lexical_skip_callback(){
 let p=program("scene-and-callback");let mut e=engine(&p);enter_first(&mut e);pause_once(&mut e);finish_after_first_pause(&mut e);
}
#[test]
fn cross0797_renderer_refusal_keeps_actual_loop_frame_rng_owner_and_debug_source(){
 let p=program("refusal");let mut e=engine(&p);enter_first(&mut e);
 let before=serde_json::to_vec(&e.state).unwrap();let count=batches(&e);let epoch=e.runtime_epoch();let views=e.video_views().unwrap();let rendered=e.renderer.views.lock().unwrap().clone();
 let session=e.debug_begin(DebugOperation::Statement,vec![]).unwrap();e.renderer.reject.store(true,Ordering::SeqCst);
 assert!(e.preview_step_statement().is_err());assert_eq!(serde_json::to_vec(&e.state).unwrap(),before);assert_eq!(batches(&e),count);assert_eq!(e.runtime_epoch(),epoch);
 assert_eq!(e.video_views().unwrap(),views);assert_eq!(*e.renderer.views.lock().unwrap(),rendered);assert!(e.accepts_debug_session(&session));
 e.renderer.reject.store(false,Ordering::SeqCst);pause_once(&mut e);finish_after_first_pause(&mut e);
}
#[test]
fn cross0797_save15_cold_body_keeps_pause_token_and_does_not_replay_method_or_suppliers(){
 let p=program("cold");let mut live=engine(&p);enter_first(&mut live);pause_once(&mut live);assert_eq!(text(&mut live),"paused 1");
 let old=live.video_views().unwrap().remove(0);let rng=live.state.random;let vars=live.state.vars.clone();
 let bytes=live.capture_save15(&p,None,79,"real loop pause body".into()).unwrap().encode().unwrap();
 let mut cold=engine(&p);cold.restore_save15(&p,rvn_core::save::consumer_checkpoint::Save15::decode(&bytes).unwrap()).unwrap();
 assert_eq!(cold.state.random,rng);assert_eq!(cold.state.vars,vars);assert_eq!(frame_count(&cold),1);
 let before=serde_json::to_vec(&cold.state).unwrap();assert!(!cold.resume_video_from_view(&old).unwrap());assert_eq!(serde_json::to_vec(&cold.state).unwrap(),before);
 let token=cold.video_views().unwrap()[0].resource_token.clone();assert_eq!(text(&mut cold),"paused 1");assert_eq!(cold.video_views().unwrap()[0].resource_token,token);assert_eq!(cold.state.random,rng);
 finish_after_first_pause(&mut live);finish_after_first_pause(&mut cold);assert_eq!(cold.state.vars,live.state.vars);assert_eq!(cold.state.random,live.state.random);
}
#[test]
fn cross0797_lexical_method_stays_refused_while_real_graph_loop_roundtrips(){
 for (name,plain,ranged) in [
  ("function","function helper(){intro.pause() return 0}","function helper(){flow.for_loop(i,0,0){set scratch=1} return 0}"),
  ("handler","handler hit(event){intro.pause()}","handler hit(event){flow.for_loop(i,0,0){set scratch=1}}"),
  ("screen","screen panel(){intro.pause() return component(\"root\",\"column\",{},[])}","screen panel(){flow.for_loop(i,0,0){set scratch=1} return component(\"root\",\"column\",{},[])}"),
 ]{
  let suffix="\ninit{set sentinel=1}\nscene label start\n\"Ready\"\n";
  // The real lexical parser is the earliest authority for this forbidden source.
  // Do not forge an AST to bypass it and then claim an Engine admission result.
  let source=format!("{plain}{suffix}");
  let error=rvn_parser::parse(&source).expect_err("a lexical track MethodCall must stay refused at its real parser boundary");
  assert!(matches!(&error.kind,rvn_parser::ParseErrorKind::UnexpectedToken{got,expected}
   if got=="Some(Ident(\"intro\"))"&&expected.contains("sans op\u{e9}ration narrative")),"{name}: {error}");
  assert_eq!(error.location.line,1);let start=error.location.col-1;
  assert_eq!(error.source_line.get(start..start+error.location.len),Some("intro"),"{name}: exact forbidden MethodCall token");
  // A real lexical loop now exports through Graph; unrelated MethodCall admission remains refused above.
  let valid=format!("{ranged}{suffix}");assert!(rvn_parser::parse(&valid).is_ok(),"{name}: valid lexical loop control");
  let docs=graphs(&valid,true);let before=docs.clone();
  let program=materialize(&format!("lexical-loop-{name}"),&docs);
  assert_eq!(docs,before,"source export must not mutate the authored graph");
  assert!(program.sources.iter().any(|s|s.source.contains("flow.consume_v3(")),"{name}: explicit loop source required");
  let declared=program.script.iter().find(|s|match s{
   Statement::Function{name:actual,..}=>name=="function"&&actual=="helper",
   Statement::Handler{name:actual,..}=>name=="handler"&&actual=="hit",
   Statement::Screen{name:actual,..}=>name=="screen"&&actual=="panel",_=>false,
  }).expect("the actual lexical declaration must survive the authoritative project roundtrip");
  let body=match declared{Statement::Function{body,..}|Statement::Handler{body,..}|Statement::Screen{body,..}=>body,_=>unreachable!()};
  assert!(body.iter().any(|s|matches!(s,Statement::ScheduledConsumer{version:3,..})),"{name}: no implicit snapshot lowering");
 }
}
