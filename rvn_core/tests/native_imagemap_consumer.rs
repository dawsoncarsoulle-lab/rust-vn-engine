//! Private targeted tests: genuine graph -> strict preparation -> file program
//! -> source-bound constructor. No fabricated owner or publication token.
use rvn_core::{Engine, Renderer, renderer::SceneRenderBatch, types::SpriteState};
use rvn_graph::*;
use rvn_parser::{Hotspot, ParsedProgram, Position, Transition};
use std::{path::PathBuf, sync::{Arc, Mutex, atomic::{AtomicBool, AtomicUsize, Ordering}}};

#[derive(Clone, Default)]
struct Host {
    batches: Arc<Mutex<Vec<SceneRenderBatch>>>,
    configs: Arc<Mutex<Vec<f32>>>,
    reject: Arc<AtomicBool>,
}
impl Renderer for Host {
    fn validate_scene_batch(&self, _: &SceneRenderBatch) -> Result<(),String> {
        if self.reject.load(Ordering::SeqCst) {Err("imagemap fixture rejects candidate".into())} else {Ok(())}
    }
    fn publish_scene_batch(&mut self, batch: SceneRenderBatch) {self.batches.lock().unwrap().push(batch);}
    fn set_typewriter_config(&mut self, speed: f32) {self.configs.lock().unwrap().push(speed);}
    fn set_background(&mut self, _: &str, _: &Transition) {}
    fn show_sprite(&mut self, _: &str, _: Option<&str>, _: &Position, _: &Transition, _: Option<&SpriteState>) {}
    fn hide_sprite(&mut self, _: &str, _: &Transition, _: &SpriteState) {}
    fn move_sprite(&mut self, _: &str, _: &Position, _: &Transition, _: &SpriteState) {}
    fn show_dialogue(&mut self, _: Option<&str>, _: &str) {}
    fn show_choice(&mut self, _: &[String]) -> usize {panic!("Preparation cannot invent a Choice response")}
    fn show_imagemap(&mut self, _: &str, _: Option<&str>, _: &[Hotspot]) -> usize {panic!("No interactive response may be fabricated during preparation") }
    fn music_play(&mut self, _: &str, _: &Transition, _: Option<&str>) {}
    fn music_stop(&mut self, _: &Transition) {}
    fn music_set_volume(&mut self, _: f32) {}
    fn sfx_play(&mut self, _: &str, _: &Transition) {}
    fn sfx_stop(&mut self, _: &str, _: &Transition) {}
}

fn fixture_dir(name: &str) -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let root = PathBuf::from(std::env::var("RVN_NATIVE_IMAGEMAP_FIXTURES").expect("private fixture directory"));
    let dir = root.join(format!("{name}-{}-{}",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)));
    std::fs::create_dir_all(&dir).unwrap(); dir
}
fn program(name: &str, source: &str) -> ParsedProgram {
    let dir=fixture_dir(name); let input=dir.join("authored.rvn");
    std::fs::write(&input,source).unwrap();
    let original=rvn_parser::parse_file_program_with_uses(&input).unwrap();
    let graphs=import_script(&original.script).unwrap();
    finish_program(dir,original,graphs)
}
fn finish_program(dir:PathBuf,original:ParsedProgram,mut graphs:Vec<GraphDocument>)->ParsedProgram {
    let mut source=String::new();let mut ast=Vec::new();
    for graph in &mut graphs {
        // The importer supplies an empty Init graph even when no Init was
        // authored. It has no consumer carrying a native graph identity.
        // Keep that historical empty graph without inventing an owned wrapper.
        if matches!(graph.kind,GraphKind::Init) && graph.nodes.len()==1 && graph.edges.is_empty() {
            assert!(!original.script.iter().any(|s|matches!(s,rvn_parser::Statement::Init{body}if !body.is_empty())));
            continue;
        }
        graph.enable_native_frame_preparation().unwrap();
        std::fs::write(dir.join(format!("graph-{}-before-strict.json",graph.graph_id)),graph.to_pretty_json().unwrap()).unwrap();
        let strict=prepare_native_frame_graph(graph).unwrap_or_else(|error|panic!("strict graph preparation {:?} {}: {error:?}",graph.kind,graph.graph_id));
        let restored=import_native_frame_script(&strict.ast).unwrap();
        let restored=restored.iter().find(|actual|actual.graph_id==graph.graph_id).unwrap();
        assert_eq!(restored.edges,graph.edges);
        assert_eq!(restored.nodes.keys().collect::<Vec<_>>(),graph.nodes.keys().collect::<Vec<_>>());
        assert_eq!(restored.pins.keys().collect::<Vec<_>>(),graph.pins.keys().collect::<Vec<_>>());
        std::fs::write(dir.join(format!("graph-{}.json",graph.graph_id)),graph.to_pretty_json().unwrap()).unwrap();
        source.push_str(&strict.source);source.push('\n');ast.extend(strict.ast);
    }
    // Each actual graph was strictly prepared above. Concatenate its source
    // directly: project export adds a synthetic end Label/Jump which has no
    // authoring identity and is irrelevant to these targeted real programs.
    let file=dir.join("main.rvn"); std::fs::write(&file,&source).unwrap();
    let parsed=rvn_parser::parse_file_program_with_uses(&file).unwrap();
    assert_eq!(parsed.script,ast);
    let origins=NativeDebugSourceMap::from_program(&parsed).unwrap();
    let commands:Vec<_>=origins.origins().iter().filter(|o|o.kind==NodeKind::Imagemap).collect();
    assert!(!commands.is_empty());
    for command in commands {
        assert!(command.span.start<command.span.end);
        assert_eq!(command.source,std::fs::canonicalize(&file).unwrap());
        assert!(graphs.iter().any(|g|g.graph_id==command.graph&&g.nodes.contains_key(&command.node)));
    }
    parsed
}


#[test]
fn genuine_graph_imagemap_requires_actual_reached_input_and_selected_branch(){
 let p=program("imagemap-scene",include_str!("../../../fixtures/scene.rvn"));
 let mut e=Engine::from_program(p,Host::default(),8).expect("Imagemap needs a real input continuation adapter");
 assert!(matches!(e.step_until_interaction().unwrap(),Some(rvn_core::engine::Interaction::Imagemap{hotspots,..})if hotspots.len()==2));
 assert!(!e.state.vars.contains_key("chosen"));e.submit_hotspot(1).unwrap();
 assert!(matches!(e.step_until_interaction().unwrap(),Some(rvn_core::engine::Interaction::Dialogue{text,..})if text=="right branch"));
 assert_eq!(e.state.vars["chosen"],rvn_parser::Value::Int(2));
}

fn file_program(name:&str,source:&str)->ParsedProgram {
 let file=fixture_dir(name).join("main.rvn");std::fs::write(&file,source).unwrap();rvn_parser::parse_file_program_with_uses(file).unwrap()
}
fn frozen(e:&Engine<Host>)->serde_json::Value{serde_json::to_value(&e.state).unwrap()}
// Stop before the next display: a displayed dialogue adds its own historical
// RNG draw and history entry. This observes the selected child transaction.
fn reached_variable(e:&mut Engine<Host>,name:&str){
 for _ in 0..64 {if e.state.vars.contains_key(name){return;} e.step_silent().unwrap();}
 panic!("selected route did not reach variable {name}");
}
#[test]fn direct_source_imagemap_reaches_constructor_and_waits_without_an_answer(){
 let p=file_program("direct-imagemap",include_str!("../../../fixtures/header-capture.rvn"));
 let mut e=Engine::from_program(p,Host::default(),8).expect("Real source-bound Imagemap must execute instead of constructor whitelist-only");
 assert!(matches!(e.step_until_interaction().unwrap(),Some(rvn_core::engine::Interaction::Imagemap{hotspots,..})if hotspots.len()==2));
 assert!(!e.state.vars.contains_key("chosen"));let source=e.preview_input_source().unwrap();assert_eq!(source.choice_indices,vec![0,1]);
 let before=frozen(&e);let history=e.history.len();let batches=e.renderer.batches.lock().unwrap().len();
 assert!(e.submit_hotspot(99).is_err());assert_eq!(frozen(&e),before);assert_eq!(e.history.len(),history);assert_eq!(e.renderer.batches.lock().unwrap().len(),batches);
 e.submit_hotspot(0).unwrap();assert!(matches!(e.step_until_interaction().unwrap(),Some(rvn_core::engine::Interaction::Dialogue{text,..})if text=="selected child"));assert_eq!(e.state.vars["chosen"],rvn_parser::Value::Int(2));assert!(!e.state.vars.contains_key("bad"));assert!(!e.accepts_preview_input_source(&source));
}
#[test]fn choice_renderer_refusal_preserves_complete_source_cache_frame_random_and_history(){
 let p=file_program("choice-cache","scene label start\nset cache_probe=1\n\"before\"\nflow.consume_v2(\"actual\",\"choice\"){capture \"header\"=random(1,9)\nchoice {\"A\"=>{set selected=1} \"B\"=>{set selected=2}}}\n\"completed\"\n");
 let mut e=Engine::from_program(p,Host::default(),8).unwrap();e.step_until_interaction().unwrap();e.advance_dialogue().unwrap();
 // Force a real description-cache miss at the next candidate. The old live
 // cache describes cache_probe=1; the actual current binding is now99.
 e.state.vars.insert("cache_probe".into(),rvn_parser::Value::Int(99));e.state.random=rvn_core::random::RandomState::seeded(17);let mut expected=e.state.random;expected.integer(1,9).unwrap();
 let state=frozen(&e);let source=e.debug_state_source().unwrap();let history=e.history.len();let batches=e.renderer.batches.lock().unwrap().len();let session=e.debug_begin(rvn_core::engine::DebugOperation::Statement,vec![]).unwrap();
 e.renderer.reject.store(true,Ordering::SeqCst);assert!(e.step_until_interaction().is_err());assert_eq!(frozen(&e),state);assert!(e.accepts_debug_state_source(&source));assert!(e.accepts_debug_session(&session));assert_eq!(e.history.len(),history);assert_eq!(e.renderer.batches.lock().unwrap().len(),batches);
 e.renderer.reject.store(false,Ordering::SeqCst);assert!(matches!(e.step_until_interaction().unwrap(),Some(rvn_core::engine::Interaction::Choice{options})if options==vec!["A","B"]));assert_eq!(e.state.random,expected);assert_eq!(e.history.len(),history+1);assert_eq!(e.renderer.batches.lock().unwrap().len(),batches+1);
 let accepted=frozen(&e);e.step_silent().unwrap();assert_eq!(frozen(&e),accepted);assert_eq!(e.history.len(),history+1);assert_eq!(e.renderer.batches.lock().unwrap().len(),batches+1);
}
#[test]fn native_init_imagemap_remains_an_explicit_missing_input_owner(){
 let p=file_program("init-refusal",include_str!("../../../fixtures/init-refusal.rvn"));let h=Host::default();let error=Engine::from_program(p,h.clone(),8).err().expect("Init has no actual input owner yet").to_string();assert!(error.contains("Startup input owner"),"{error}");assert!(h.batches.lock().unwrap().is_empty());
}


#[test]fn genuine_game_graph_imagemap_has_real_game_owner_and_completion(){
 let p=program("imagemap-game",include_str!("../../../fixtures/game.rvn"));
 let mut e=Engine::from_program(p,Host::default(),8).unwrap();
 assert!(matches!(e.step_until_interaction().unwrap(),Some(rvn_core::engine::Interaction::Imagemap{..})));
 assert!(e.state.scene_execution.is_none());let input=e.preview_input_source().unwrap();assert_eq!(input.choice_indices,vec![0,1]);
 e.submit_hotspot(0).unwrap();assert!(matches!(e.step_until_interaction().unwrap(),Some(rvn_core::engine::Interaction::Dialogue{text,..})if text=="left branch"));
 e.advance_dialogue().unwrap();assert!(matches!(e.step_until_interaction().unwrap(),Some(rvn_core::engine::Interaction::Dialogue{text,..})if text=="completed"));
 assert_eq!(e.state.vars["chosen"],rvn_parser::Value::Int(1));assert!(!e.accepts_preview_input_source(&input));
}

#[test]fn imagemap_fenced_model_refusal_keeps_header_random_source_frame_and_history(){
 let p=file_program("imagemap-refused",include_str!("../../../fixtures/header-capture.rvn"));
 let mut e=Engine::from_program(p,Host::default(),8).unwrap();e.step_silent().unwrap();assert!(matches!(e.peek_statement(),Some(rvn_parser::Statement::ScheduledConsumer{..})));e.state.random=rvn_core::random::RandomState::seeded(17);
 let mut expected=e.state.random;expected.integer(1,9).unwrap();
 let before=frozen(&e);let source=e.debug_state_source().unwrap();let count=e.renderer.batches.lock().unwrap().len();let history=e.history.len();
 e.renderer.reject.store(true,Ordering::SeqCst);assert!(e.step_until_interaction().is_err());
 assert_eq!(frozen(&e),before);assert!(e.accepts_debug_state_source(&source));assert_eq!(e.history.len(),history);assert_eq!(e.renderer.batches.lock().unwrap().len(),count);
 e.renderer.reject.store(false,Ordering::SeqCst);e.step_until_interaction().unwrap();assert_eq!(e.state.random,expected);
 let reached=frozen(&e);let count=e.renderer.batches.lock().unwrap().len();let history=e.history.len();e.step_silent().unwrap();
 assert_eq!(frozen(&e),reached);assert_eq!(e.renderer.batches.lock().unwrap().len(),count);assert_eq!(e.history.len(),history);
}

#[test]fn cold_save15_waiting_imagemap_rebinds_real_visit_without_header_or_child_replay(){
 let p=file_program("imagemap-cold",include_str!("../../../fixtures/header-capture.rvn"));
 let mut live=Engine::from_program(p.clone(),Host::default(),8).unwrap();live.state.random=rvn_core::random::RandomState::seeded(17);live.step_until_interaction().unwrap();
 let mut expected=live.state.random;expected.integer(1,9).unwrap();
 let input=live.preview_input_source().unwrap();let random=live.state.random;
 let wire=live.capture_save15(&p,None,1,"imagemap".into()).unwrap().encode().unwrap();
 let save=rvn_core::save::consumer_checkpoint::Save15::decode(&wire).unwrap();
 let mut cold=Engine::from_program(p.clone(),Host::default(),8).unwrap();cold.restore_save15(&p,save).unwrap();
 assert_eq!(cold.state.random,random);assert!(matches!(cold.current_interaction().unwrap(),Some(rvn_core::engine::Interaction::Imagemap{hotspots,..})if hotspots.len()==2));
 assert!(!cold.accepts_preview_input_source(&input));assert_eq!(cold.preview_input_source().unwrap().choice_indices,vec![0,1]);
 let count=cold.renderer.batches.lock().unwrap().len();cold.step_silent().unwrap();assert_eq!(cold.state.random,random);assert_eq!(cold.renderer.batches.lock().unwrap().len(),count);
 cold.submit_hotspot(0).unwrap();reached_variable(&mut cold,"chosen");assert_eq!(cold.state.random,expected);assert_eq!(cold.state.vars["chosen"],rvn_parser::Value::Int(2));assert!(!cold.state.vars.contains_key("bad"));
 live.submit_hotspot(0).unwrap();reached_variable(&mut live,"chosen");assert_eq!(cold.state.random,live.state.random);assert_eq!(cold.state.vars,live.state.vars);
 let mut displayed=expected;displayed.next_u64();
 for engine in [&mut live,&mut cold]{assert!(matches!(engine.step_until_interaction().unwrap(),Some(rvn_core::engine::Interaction::Dialogue{text,..})if text=="selected child"));assert_eq!(engine.state.random,displayed);}
 assert_eq!(cold.state.vars,live.state.vars);
}

#[test]fn refused_restore_and_stale_prepared_restore_publish_no_imagemap_candidate(){
 let p=file_program("imagemap-restore",include_str!("../../../fixtures/header-capture.rvn"));
 let mut live=Engine::from_program(p.clone(),Host::default(),8).unwrap();live.step_until_interaction().unwrap();let save=live.capture_save15(&p,None,1,"input".into()).unwrap();
 let mut cold=Engine::from_program(p.clone(),Host::default(),8).unwrap();let before=frozen(&cold);let source=cold.debug_state_source().unwrap();let count=cold.renderer.batches.lock().unwrap().len();
 cold.renderer.reject.store(true,Ordering::SeqCst);assert!(cold.restore_save15(&p,save.clone()).is_err());assert_eq!(frozen(&cold),before);assert!(cold.accepts_debug_state_source(&source));assert_eq!(cold.renderer.batches.lock().unwrap().len(),count);
 cold.renderer.reject.store(false,Ordering::SeqCst);let prepared=cold.prepare_save15_with_locale(&p,save,None).unwrap();cold.state.vars.insert("actual_mutation".into(),rvn_parser::Value::Int(9));let changed=frozen(&cold);assert!(cold.begin_prepared_save15_commit(prepared).is_err());assert_eq!(frozen(&cold),changed);assert_eq!(cold.renderer.batches.lock().unwrap().len(),count);
}

#[test]fn anonymous_empty_colon_and_unicode_names_keep_exact_graph_identity_and_hotspot_values(){
 for (index,name) in [None,Some(""),Some("a:b"),Some(" Héroïne😀 ")].into_iter().enumerate(){
  let field=name.map(|name|format!("name: {} ",serde_json::to_string(name).unwrap())).unwrap_or_default();
  let source=format!("scene label start\nimagemap {{background: \"garden.png\" hotspot {{{field}area: (0,0,50,100)}} => {{\"selected\"}}}}\n");
  let p=program(&format!("imagemap-name-{index}"),&source);let mut e=Engine::from_program(p,Host::default(),8).unwrap();
  let Some(rvn_core::engine::Interaction::Imagemap{hotspots,..})=e.step_until_interaction().unwrap()else{panic!("actual input")};assert_eq!(hotspots[0].name.as_deref(),name);
 }
}


#[test]fn modified_portable_visit_owner_site_indices_or_definition_refuse_before_publication(){
 let p=file_program("imagemap-forgery",include_str!("../../../fixtures/header-capture.rvn"));let mut live=Engine::from_program(p.clone(),Host::default(),8).unwrap();live.step_until_interaction().unwrap();
 let original=serde_json::to_value(live.capture_save15(&p,None,1,"map".into()).unwrap()).unwrap();
 let data="/story/base/scene_execution/display/consumer_imagemap";
 assert!(original.pointer(data).is_some(),"portable payload must retain its actual visit");
 for kind in ["program","owner","pc","indices","definition","omitted"] {
  let mut value=original.clone();
  match kind {
   "program"=>value.pointer_mut(data).unwrap()["program"]=serde_json::json!(vec![1u8;32]),
   "owner"=>value.pointer_mut(data).unwrap()["owner"]=serde_json::json!("Game"),
   "pc"=>value.pointer_mut(data).unwrap()["pc"]=serde_json::json!(usize::MAX),
   "indices"=>*value.pointer_mut("/story/base/scene_execution/display/choice_indices").unwrap()=serde_json::json!([1,0]),
   "definition"=>{let display=value.pointer_mut("/story/base/scene_execution/display/display").unwrap();display["Imagemap"]["background"]=serde_json::json!("foreign.png");},
   "omitted"=>{value.pointer_mut("/story/base/scene_execution/display").unwrap().as_object_mut().unwrap().remove("consumer_imagemap");},
   _=>unreachable!()
  }
  let mut cold=Engine::from_program(p.clone(),Host::default(),8).unwrap();let before=frozen(&cold);let source=cold.debug_state_source().unwrap();let count=cold.renderer.batches.lock().unwrap().len();
  let result=rvn_core::save::consumer_checkpoint::Save15::decode(&serde_json::to_string(&value).unwrap()).and_then(|save|cold.restore_save15(&p,save).map(|_|()));
  assert!(result.is_err(),"changed {kind} cannot authorize a restored input");assert_eq!(frozen(&cold),before,"{kind}");assert!(cold.accepts_debug_state_source(&source),"{kind}");assert_eq!(cold.renderer.batches.lock().unwrap().len(),count,"{kind}");
 }
}

#[test]fn rollback_restores_reached_input_and_renderer_refusal_keeps_selected_branch(){
 let p=file_program("imagemap-rollback",include_str!("../../../fixtures/header-capture.rvn"));let mut e=Engine::from_program(p,Host::default(),8).unwrap();e.state.random=rvn_core::random::RandomState::seeded(17);e.step_until_interaction().unwrap();let random=e.state.random;
 let input=e.preview_input_source().unwrap();e.submit_hotspot(0).unwrap();reached_variable(&mut e,"chosen");let before=frozen(&e);let count=e.renderer.batches.lock().unwrap().len();let source=e.debug_state_source().unwrap();
 e.renderer.reject.store(true,Ordering::SeqCst);assert!(!e.rollback());assert_eq!(frozen(&e),before);assert!(e.accepts_debug_state_source(&source));assert_eq!(e.renderer.batches.lock().unwrap().len(),count);
 e.renderer.reject.store(false,Ordering::SeqCst);assert!(e.rollback());assert!(matches!(e.current_interaction().unwrap(),Some(rvn_core::engine::Interaction::Imagemap{..})));assert_eq!(e.state.random,random);assert!(!e.state.vars.contains_key("chosen"));assert!(!e.accepts_preview_input_source(&input));
 e.submit_hotspot(0).unwrap();e.step_until_interaction().unwrap();assert_eq!(e.state.vars["chosen"],rvn_parser::Value::Int(2));assert!(!e.state.vars.contains_key("bad"));
}

#[test]fn real_debugger_hotspot_source_transfers_only_the_actual_selected_route(){
 let p=file_program("imagemap-debugger",include_str!("../../../fixtures/header-capture.rvn"));let mut e=Engine::from_program_with_debugger(p,Host::default(),8).unwrap();e.step_until_interaction().unwrap();let input=e.preview_input_source().unwrap();
 let mut session=e.debug_begin(rvn_core::engine::DebugOperation::Hotspot{source:input.clone(),index:0},vec![]).unwrap();let snap=session.snapshot();assert!(snap.origin.as_ref().is_some_and(|origin|origin.span.is_some()));
 for _ in 0..64 {if e.debug_step(&mut session,rvn_core::editor_debugger::StepKind::Continue).unwrap().0==rvn_core::engine::DebugProgress::Complete{break;}}
 assert!(session.is_complete());e.step_until_interaction().unwrap();assert_eq!(e.state.vars["chosen"],rvn_parser::Value::Int(2));assert!(!e.state.vars.contains_key("bad"));
 assert!(e.debug_begin(rvn_core::engine::DebugOperation::Hotspot{source:input,index:0},vec![]).is_err());
}


#[test]fn gather_failure_and_real_source_edit_cannot_publish_an_imagemap(){
 let source="scene label start\n\"before\"\nflow.consume_v2(\"actual\",\"map\"){capture \"rng\"=random(1,9)\ncapture \"bad\"=1/0\nimagemap {background: \"garden.png\" hotspot {area: (0,0,50,100)} => {\"selected\"}}}\n";
 let p=file_program("imagemap-gather",source);let mut e=Engine::from_program(p,Host::default(),8).unwrap();e.step_until_interaction().unwrap();e.advance_dialogue().unwrap();let before=frozen(&e);let actual=e.debug_state_source().unwrap();let history=e.history.len();let batches=e.renderer.batches.lock().unwrap().len();
 assert!(e.step_until_interaction().is_err());assert_eq!(frozen(&e),before);assert!(e.accepts_debug_state_source(&actual));assert_eq!(e.history.len(),history);assert_eq!(e.renderer.batches.lock().unwrap().len(),batches);
 let mut p=file_program("imagemap-source-forged",include_str!("../../../fixtures/header-capture.rvn"));let old=p.sources[0].source.clone();p.sources[0].source=old.replace("garden.png","other.png");assert_ne!(p.sources[0].source,old);let h=Host::default();assert!(Engine::from_program(p,h.clone(),8).is_err());assert!(h.batches.lock().unwrap().is_empty());
}

#[test]fn graph_asset_reroute_preserves_exact_source_edges_pins_and_selected_route(){
 let dir=fixture_dir("imagemap-reroute");let input=dir.join("authored.rvn");std::fs::write(&input,include_str!("../../../fixtures/scene.rvn")).unwrap();let original=rvn_parser::parse_file_program_with_uses(&input).unwrap();let mut graphs=import_script(&original.script).unwrap();let graph=graphs.iter_mut().find(|g|matches!(g.kind,GraphKind::Label{..})).unwrap();
 let map=graph.nodes.values().find(|n|n.kind==NodeKind::Imagemap).unwrap().id;let port=graph.pin_by_key(map,"background").unwrap().id;let edge=graph.edges.values().find(|e|e.input==port).unwrap().clone();graph.remove_edge(edge.id);
 let reroute=graph.add_catalog_node(NodeKind::Reroute,[500.,500.]).unwrap();let ri=graph.pin_by_key(reroute,"value").unwrap().id;let ro=graph.pin_by_key(reroute,"value_out").unwrap().id;graph.connect(edge.output,ri).unwrap();graph.connect(ro,port).unwrap();
 let p=finish_program(dir,original,graphs);let mut e=Engine::from_program(p,Host::default(),8).unwrap();assert!(matches!(e.step_until_interaction().unwrap(),Some(rvn_core::engine::Interaction::Imagemap{background,..})if background=="garden.png"));e.submit_hotspot(1).unwrap();e.step_until_interaction().unwrap();assert_eq!(e.state.vars["chosen"],rvn_parser::Value::Int(2));
}
