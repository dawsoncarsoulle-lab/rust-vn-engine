//! Private targeted tests: genuine graph -> strict preparation -> file program
//! -> source-bound constructor. No fabricated owner or publication token.
use rvn_core::{Engine, Renderer, renderer::SceneRenderBatch, types::SpriteState};
use rvn_graph::*;
#[path="support/strict_native_consumer.rs"]
mod strict_native_consumer;
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
        if self.reject.load(Ordering::SeqCst) {Err("character fixture rejects candidate".into())} else {Ok(())}
    }
    fn publish_scene_batch(&mut self, batch: SceneRenderBatch) {self.batches.lock().unwrap().push(batch);}
    fn set_typewriter_config(&mut self, speed: f32) {self.configs.lock().unwrap().push(speed);}
    fn set_background(&mut self, _: &str, _: &Transition) {}
    fn show_sprite(&mut self, _: &str, _: Option<&str>, _: &Position, _: &Transition, _: Option<&SpriteState>) {}
    fn hide_sprite(&mut self, _: &str, _: &Transition, _: &SpriteState) {}
    fn move_sprite(&mut self, _: &str, _: &Position, _: &Transition, _: &SpriteState) {}
    fn show_dialogue(&mut self, _: Option<&str>, _: &str) {}
    fn show_choice(&mut self, _: &[String]) -> usize {0}
    fn show_imagemap(&mut self, _: &str, _: Option<&str>, _: &[Hotspot]) -> usize {0}
    fn music_play(&mut self, _: &str, _: &Transition, _: Option<&str>) {}
    fn music_stop(&mut self, _: &Transition) {}
    fn music_set_volume(&mut self, _: f32) {}
    fn sfx_play(&mut self, _: &str, _: &Transition) {}
    fn sfx_stop(&mut self, _: &str, _: &Transition) {}
}

fn fixture_dir(name: &str) -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let root = PathBuf::from(std::env::var("RVN_NATIVE_TYPEWRITER_FIXTURES").expect("private fixture directory"));
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
fn finish_program(dir:PathBuf,original:ParsedProgram,graphs:Vec<GraphDocument>)->ParsedProgram {
    strict_native_consumer::finish_program(dir,original,graphs,&[NodeKind::CharacterCreate])
}


#[test]
fn true_graph_character_create_reaches_real_character_state(){
 let p=program("character-scene","scene label start\ncharacter.create(\"hero\",\"Héroïne😀\")\nhero \"dialogue\"\n");
 let mut e=Engine::from_program(p,Host::default(),8).expect("CharacterCreate requires actual execution");e.step_until_interaction().unwrap();
 assert_eq!(serde_json::to_value(&e.state).unwrap()["layered"]["display_names"]["hero"],"Héroïne😀");
}

#[test]
fn true_graph_character_at_narration_boundary_reaches_constructor(){
 let p=program("character-isolated","scene label start\ncharacter.create(\"hero\",\"Héroïne😀\")\n\"boundary\"\n");
 let mut e=Engine::from_program(p,Host::default(),8).expect("CharacterCreate must have a concrete owner adapter");e.step_until_interaction().unwrap();
 assert_eq!(serde_json::to_value(&e.state).unwrap()["layered"]["display_names"]["hero"],"Héroïne😀");
}

fn file_program(name:&str,source:&str)->ParsedProgram {
 let file=fixture_dir(name).join("main.rvn");std::fs::write(&file,source).unwrap();rvn_parser::parse_file_program_with_uses(file).unwrap()
}
fn init_program(name:&str)->ParsedProgram {
 let dir=fixture_dir(name);let source="scene label start\n\"after\"\n";
 let input=dir.join("authored.rvn");std::fs::write(&input,source).unwrap();let original=rvn_parser::parse_file_program_with_uses(input).unwrap();
 let mut graphs=import_script(&original.script).unwrap();let init=graphs.iter_mut().find(|g|matches!(g.kind,GraphKind::Init)).unwrap();
 let root=init.nodes.values().find(|n|n.kind==NodeKind::Init).unwrap().id;
 let node=init.add_catalog_node(NodeKind::CharacterCreate,[300.,200.]).unwrap();
 for(key,value)in[("id","hero"),("display_name","Héroïne😀")] {let pin=init.pin_by_key(node,key).unwrap().id;init.pins.get_mut(&pin).unwrap().default_value=Some(PropertyValue::String(value.into()));}
 init.connect(init.pin_by_key(root,"exec_out").unwrap().id,init.pin_by_key(node,"exec_in").unwrap().id).unwrap();
 finish_program(dir,original,graphs)
}
fn names(e:&Engine<Host>)->&std::collections::BTreeMap<String,String>{&e.state.layered.display_names}
fn frozen(e:&Engine<Host>)->serde_json::Value{serde_json::to_value(&e.state).unwrap()}
fn draw(mut random:rvn_core::random::RandomState)->rvn_core::random::RandomState{random.integer(1,9).unwrap();random}

#[test]fn real_graph_game_and_scene_keep_dialogue_id_separate_from_display_name(){
 for prefix in["label start","scene label start"] {
  let p=program("actual-owner",&format!("{prefix}\ncharacter.create(\"hero\",\"Héroïne😀\")\nhero \"bonjour\"\n"));
  let mut e=Engine::from_program(p,Host::default(),8).unwrap();let rng=e.state.random;
  e.step_until_interaction().unwrap();assert_eq!(e.state.layered.display_name("hero"),"Héroïne😀");assert_eq!(e.state.random,rng);
  assert!(matches!(e.current_interaction().unwrap(),Some(rvn_core::engine::Interaction::Dialogue{character:Some(id),text})if id=="hero"&&text=="bonjour"));
  let count=e.renderer.batches.lock().unwrap().len();let before=frozen(&e);e.step_silent().unwrap();assert_eq!(frozen(&e),before);assert_eq!(e.renderer.batches.lock().unwrap().len(),count);
 }
}
#[test]fn real_graph_character_rename_is_snapshotable_not_a_sprite_identity_change(){
 let p=program("rename","scene label start\ncharacter.create(\"hero\",\"First\")\nhero \"before\"\ncharacter.create(\"hero\",\"Deuxième😀\")\nhero \"after\"\n");
 let mut e=Engine::from_program(p,Host::default(),8).unwrap();e.step_until_interaction().unwrap();assert_eq!(names(&e)["hero"],"First");
 e.advance_dialogue().unwrap();e.step_until_interaction().unwrap();assert_eq!(names(&e).len(),1);assert_eq!(names(&e)["hero"],"Deuxième😀");
 assert!(e.state.layered.characters.is_empty());assert!(e.state.sprites.is_empty());
}
#[test]fn real_graph_renderer_rejection_keeps_full_state_session_and_published_names(){
 let p=program("refusal","scene label start\n\"before\"\ncharacter.create(\"hero\",\"Candidate\")\nhero \"after\"\n");
 let mut e=Engine::from_program(p,Host::default(),8).unwrap();e.step_until_interaction().unwrap();e.advance_dialogue().unwrap();
 let before=frozen(&e);let count=e.renderer.batches.lock().unwrap().len();let session=e.debug_begin(rvn_core::engine::DebugOperation::Statement,vec![]).unwrap();
 e.renderer.reject.store(true,Ordering::SeqCst);assert!(e.step_silent().is_err());assert_eq!(frozen(&e),before);assert!(e.accepts_debug_session(&session));assert_eq!(e.renderer.batches.lock().unwrap().len(),count);
 assert!(e.renderer.batches.lock().unwrap().iter().all(|b|b.state.layered.display_names.is_empty()));
 e.renderer.reject.store(false,Ordering::SeqCst);e.step_until_interaction().unwrap();assert_eq!(names(&e)["hero"],"Candidate");
}
#[test]fn source_bound_gather_rng_once_and_cold_save15_no_replay(){
 let p=file_program("rng-cold","scene label start\n\"before\"\nflow.consume_v2(\"actual\",\"declaration\"){capture \"rng\"=random(1,9)\ncharacter.create(\"hero\",\"Saved😀\")}\nhero \"after\"\n");
 let mut live=Engine::from_program(p.clone(),Host::default(),8).unwrap();live.step_until_interaction().unwrap();live.advance_dialogue().unwrap();live.state.random=rvn_core::random::RandomState::seeded(17);let expected=draw(live.state.random);
 live.step_silent().unwrap();assert_eq!(live.state.random,expected);live.step_until_interaction().unwrap();let saved_random=live.state.random;assert_eq!(names(&live)["hero"],"Saved😀");
 let save=live.capture_save15(&p,None,1,"character".into()).unwrap();let bytes=save.encode().unwrap();let save=rvn_core::save::consumer_checkpoint::Save15::decode(&bytes).unwrap();
 let mut cold=Engine::from_program(p.clone(),Host::default(),8).unwrap();assert!(names(&cold).is_empty());cold.restore_save15(&p,save).unwrap();assert_eq!(names(&cold),names(&live));assert_eq!(cold.state.random,saved_random);
 let before=frozen(&cold);let count=cold.renderer.batches.lock().unwrap().len();cold.step_silent().unwrap();assert_eq!(frozen(&cold),before);assert_eq!(cold.renderer.batches.lock().unwrap().len(),count);
}
#[test]fn real_graph_rollback_and_refused_rollback_restore_exact_name_map(){
 let p=program("rollback","scene label start\ncharacter.create(\"hero\",\"First\")\nhero \"before\"\ncharacter.create(\"hero\",\"Second\")\nhero \"after\"\n");
 let mut e=Engine::from_program(p,Host::default(),8).unwrap();e.step_until_interaction().unwrap();e.advance_dialogue().unwrap();e.step_silent().unwrap();assert_eq!(names(&e)["hero"],"Second");
 let before=frozen(&e);let count=e.renderer.batches.lock().unwrap().len();e.renderer.reject.store(true,Ordering::SeqCst);assert!(!e.rollback());assert_eq!(frozen(&e),before);assert_eq!(e.renderer.batches.lock().unwrap().len(),count);
 e.renderer.reject.store(false,Ordering::SeqCst);assert!(e.rollback());assert_eq!(names(&e)["hero"],"First");assert_eq!(e.renderer.batches.lock().unwrap().last().unwrap().state.layered.display_names["hero"],"First");
}
#[test]fn true_init_graph_eager_and_owned_debugger_execute_one_declaration(){
 let p=init_program("init");let e=Engine::from_program(p.clone(),Host::default(),8).unwrap();assert_eq!(names(&e)["hero"],"Héroïne😀");
 let host=Host::default();let mut owner=Engine::prepare_debugger_startup(p,host.clone(),8,Default::default(),Default::default(),None).unwrap();assert!(owner.state().layered.display_names.is_empty());
 for _ in 0..32 {if owner.is_complete(){break;}let before=serde_json::to_value(owner.state()).unwrap();let plan=owner.prepare_step(rvn_core::editor_debugger::StepKind::Continue).unwrap();assert_eq!(serde_json::to_value(owner.state()).unwrap(),before);let stale=owner.prepare_step(rvn_core::editor_debugger::StepKind::Continue).unwrap();owner.begin_prepared_step_commit(plan).unwrap().commit();let after=serde_json::to_value(owner.state()).unwrap();assert!(owner.begin_prepared_step_commit(stale).is_err());assert_eq!(serde_json::to_value(owner.state()).unwrap(),after);}
 assert!(owner.is_complete());assert_eq!(owner.state().layered.display_names["hero"],"Héroïne😀");assert_eq!(host.batches.lock().unwrap().iter().filter(|b|b.state.layered.display_names.contains_key("hero")).count(),1);
}
#[test]fn native_gather_error_or_forged_program_cannot_publish_character(){
 let p=file_program("gather-bad","scene label start\n\"before\"\nflow.consume_v2(\"actual\",\"declaration\"){capture \"rng\"=random(1,9)\ncapture \"bad\"=1/0\ncharacter.create(\"hero\",\"Rejected\")}\n\"after\"\n");
 let mut e=Engine::from_program(p,Host::default(),8).unwrap();e.step_until_interaction().unwrap();e.advance_dialogue().unwrap();let before=frozen(&e);let count=e.renderer.batches.lock().unwrap().len();assert!(e.step_silent().is_err());assert_eq!(frozen(&e),before);assert_eq!(e.renderer.batches.lock().unwrap().len(),count);
 let mut p=program("forged","scene label start\ncharacter.create(\"hero\",\"Actual\")\n\"after\"\n");p.sources[0].source=p.sources[0].source.replace("Actual","Forged");let h=Host::default();assert!(Engine::from_program(p,h.clone(),8).is_err());assert!(h.batches.lock().unwrap().is_empty());
}
#[test]fn names_are_bounded_atomic_and_old_empty_wire_is_unchanged(){
 let mut state=rvn_core::composition::LayeredState::default();let old=serde_json::json!({"characters":{}});assert_eq!(serde_json::to_value(&state).unwrap(),old);let restored:rvn_core::composition::LayeredState=serde_json::from_value(old).unwrap();assert!(restored.display_names.is_empty());assert_eq!(restored.display_name("unknown"),"unknown");
 state.declare("hero".into(),"Valid".into()).unwrap();let before=state.clone();assert!(state.declare("".into(),"bad".into()).is_err());assert_eq!(state,before);assert!(state.declare("hero".into(),"x".repeat(1_000_001)).is_err());assert_eq!(state,before);
}
#[test]fn legacy_raw_declaration_and_old_absent_save_names_use_authentic_script(){
 let p=file_program("legacy","init{character.create(\"hero\",\"Historical😀\")}\nlabel start\nhero \"after\"\n");let mut e=Engine::from_program(p.clone(),Host::default(),8).unwrap();e.step_until_interaction().unwrap();assert_eq!(names(&e)["hero"],"Historical😀");
 let mut data=rvn_core::save::SaveData::from_state(&e.state,1,"old".into(),"main.rvn".into());data.layered.display_names.clear();let wire=serde_json::to_string(&data).unwrap();assert!(!wire.contains("display_names"));let data=serde_json::from_str(&wire).unwrap();let mut cold=Engine::from_program(p,Host::default(),8).unwrap();cold.state.layered.display_names.clear();cold.load_data(data).unwrap();assert_eq!(names(&cold)["hero"],"Historical😀");
}

#[test]fn source_bound_pre_scene_game_uses_actual_game_frame_and_narrative_registry(){
 let p=file_program("actual-narrative-game","label start\nflow.consume_v2(\"actual\",\"declare\"){character.create(\"hero\",\"Game name\")}\nhero \"bonjour\"\nscene label later\n\"later\"\n");
 let mut e=Engine::from_program(p,Host::default(),8).unwrap();e.step_silent().unwrap();
 assert!(e.state.scene_execution.as_ref().unwrap().active.is_none());assert_eq!(e.state.consumer_frame.as_ref().unwrap().owner,rvn_core::consumer_schedule::ConsumerOwner::Game);assert_eq!(names(&e)["hero"],"Game name");
}
#[test]fn character_declarations_reject_computation_and_host_contexts_before_effects(){
 for declaration in ["function hidden(){flow.consume_v2(\"s\",\"c\"){character.create(\"hero\",\"No\")}}","handler hidden(event){flow.consume_v2(\"s\",\"c\"){character.create(\"hero\",\"No\")}}","screen hidden(){flow.consume_v2(\"s\",\"c\"){character.create(\"hero\",\"No\")} return component(\"r\",\"column\",{},[])}"] {
  let file=fixture_dir("illegal-owner").join("main.rvn");std::fs::write(&file,format!("init{{set sentinel=1}}\n{declaration}\nscene label start\n\"after\"\n")).unwrap();let h=Host::default();
  match rvn_parser::parse_file_program_with_uses(&file) {Ok(p)=>assert!(Engine::from_program(p,h.clone(),8).is_err()),Err(error)=>assert!(!error.is_empty())};assert!(h.batches.lock().unwrap().is_empty());
 }
}
#[test]fn static_character_proof_requires_real_kind_id_and_actual_output_pin(){
 let p=program("proof","scene label start\ncharacter.create(\"hero\",\"Name\")\nhero \"dialogue\"\n");
 let source=&p.sources[0].source;
 let marker=source.find("character=").unwrap();let end=source[marker..].find(',').unwrap()+marker;let segment=&source[marker..end];let output=segment.split('~').nth(1).unwrap().parse::<u64>().unwrap();
 let wrong_pin=source.replacen(segment,&format!("{}~{}",segment.split('~').next().unwrap(),output-1),1);
 let wrong_kind=source.replacen(":character_value|",":text_value|",1);
 let wrong_value=source.replacen(" = \"hero\"\n        capture", " = \"other\"\n        capture",1);
 assert_ne!(&wrong_pin,source);assert_ne!(&wrong_kind,source);assert_ne!(&wrong_value,source);
 for(name,raw)in[("pin",wrong_pin),("kind",wrong_kind),("value",wrong_value)] {let file=fixture_dir(name).join("main.rvn");std::fs::write(&file,raw).unwrap();let h=Host::default();match rvn_parser::parse_file_program_with_uses(&file){Ok(p)=>assert!(Engine::from_program(p,h.clone(),8).is_err(),"forged {name} admitted"),Err(error)=>assert!(!error.is_empty())};assert!(h.batches.lock().unwrap().is_empty());}
}
#[test]fn old_save_versions_default_names_and_nonempty_map_roundtrip(){
 let p=file_program("oldwire","label start\n\"after\"\n");let e=Engine::from_program(p,Host::default(),8).unwrap();let data=rvn_core::save::SaveData::from_state(&e.state,1,"legacy".into(),"main.rvn".into());let mut wire=serde_json::to_value(data).unwrap();assert!(wire["layered"].get("display_names").is_none());
 for version in 1..=14 {wire["format_version"]=version.into();let read:rvn_core::save::SaveData=serde_json::from_value(wire.clone()).unwrap();assert!(read.layered.display_names.is_empty());}
 let mut state=rvn_core::composition::LayeredState::default();state.declare("hero".into(),"Élise😀".into()).unwrap();let bytes=serde_json::to_vec(&state).unwrap();assert_eq!(serde_json::from_slice::<rvn_core::composition::LayeredState>(&bytes).unwrap(),state);
}

#[test]fn true_graph_save15_cold_preserves_names_and_random_producer_result(){
 let p=program("graph-cold","scene label start\nset sampled=random(1,9)\ncharacter.create(\"hero\",\"Graph saved😀\")\nhero \"boundary\"\n");
 let mut live=Engine::from_program(p.clone(),Host::default(),8).unwrap();live.state.random=rvn_core::random::RandomState::seeded(17);live.step_until_interaction().unwrap();let random=live.state.random;let sampled=live.state.vars["sampled"].clone();
 let save=live.capture_save15(&p,None,1,"graph".into()).unwrap();let bytes=save.encode().unwrap();let mut cold=Engine::from_program(p.clone(),Host::default(),8).unwrap();cold.restore_save15(&p,rvn_core::save::consumer_checkpoint::Save15::decode(&bytes).unwrap()).unwrap();
 assert_eq!(names(&cold),names(&live));assert_eq!(cold.state.random,random);assert_eq!(cold.state.vars["sampled"],sampled);assert!(matches!(cold.current_interaction().unwrap(),Some(rvn_core::engine::Interaction::Dialogue{character:Some(id),..})if id=="hero"));
 let count=cold.renderer.batches.lock().unwrap().len();cold.step_silent().unwrap();assert_eq!(cold.state.random,random);assert_eq!(cold.state.vars["sampled"],sampled);assert_eq!(cold.renderer.batches.lock().unwrap().len(),count);
}
#[test]fn true_character_source_reroute_keeps_exact_graph_edges_pins_and_identity(){
 let dir=fixture_dir("reroute");let input=dir.join("authored.rvn");std::fs::write(&input,"scene label start\ncharacter.create(\"hero\",\"Rerouted\")\nhero \"hello\"\n").unwrap();let original=rvn_parser::parse_file_program_with_uses(input).unwrap();let mut graphs=import_script(&original.script).unwrap();let graph=graphs.iter_mut().find(|g|matches!(g.kind,GraphKind::Label{..})).unwrap();
 let dialogue=graph.nodes.values().find(|n|n.kind==NodeKind::Dialogue).unwrap().id;let input=graph.pin_by_key(dialogue,"character").unwrap().id;let old=graph.edges.values().find(|e|e.input==input).unwrap().clone();graph.remove_edge(old.id);
 let reroute=graph.add_catalog_node(NodeKind::Reroute,[500.,500.]).unwrap();let ri=graph.pin_by_key(reroute,"value").unwrap().id;let ro=graph.pin_by_key(reroute,"value_out").unwrap().id;graph.connect(old.output,ri).unwrap();graph.connect(ro,input).unwrap();
 let p=finish_program(dir,original,graphs);let mut e=Engine::from_program(p,Host::default(),8).unwrap();e.step_until_interaction().unwrap();assert_eq!(e.state.layered.display_name("hero"),"Rerouted");assert!(matches!(e.current_interaction().unwrap(),Some(rvn_core::engine::Interaction::Dialogue{character:Some(id),..})if id=="hero"));
}
#[test]fn actual_debugger_source_and_state_mutation_revoke_prepared_save15(){
 let p=program("debugger-source","scene label start\ncharacter.create(\"hero\",\"Actual\")\nhero \"boundary\"\n");let mut e=Engine::from_program_with_debugger(p.clone(),Host::default(),8).unwrap();e.enter_label("start").unwrap();let mut session=e.debug_begin(rvn_core::engine::DebugOperation::Statement,vec![]).unwrap();let snapshot=session.snapshot();assert!(snapshot.position.origin.as_ref().unwrap().span.is_some());
 for _ in 0..32 {if e.debug_step(&mut session,rvn_core::editor_debugger::StepKind::Continue).unwrap().0==rvn_core::engine::DebugProgress::Complete{break;}}
 assert_eq!(names(&e)["hero"],"Actual");e.step_until_interaction().unwrap();let save=e.capture_save15(&p,None,1,"debug".into()).unwrap();let prepared=e.prepare_save15_with_locale(&p,save,None).unwrap();e.state.layered.declare("hero".into(),"Live changed".into()).unwrap();assert!(e.begin_prepared_save15_commit(prepared).is_err());assert_eq!(names(&e)["hero"],"Live changed");
}
