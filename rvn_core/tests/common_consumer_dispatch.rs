//! Private targeted tests: genuine graph -> strict preparation -> file program
//! -> source-bound constructor. No fabricated owner or publication token.
use rvn_core::{Engine, Renderer, renderer::SceneRenderBatch, types::SpriteState};
use rvn_graph::*;
use rvn_parser::{Hotspot, ParsedProgram, Position, Transition, Value};
use std::{path::PathBuf, sync::{Arc, Mutex, atomic::{AtomicBool, AtomicUsize, Ordering}}};

#[derive(Clone, Default)]
struct Host {
    batches: Arc<Mutex<Vec<SceneRenderBatch>>>,
    configs: Arc<Mutex<Vec<f32>>>,
    reject: Arc<AtomicBool>,
    events: Arc<Mutex<Vec<String>>>,
}
impl Renderer for Host {
    fn validate_scene_batch(&self, _: &SceneRenderBatch) -> Result<(),String> {
        if self.reject.load(Ordering::SeqCst) {Err("typewriter fixture rejects candidate".into())} else {Ok(())}
    }
    fn publish_scene_batch(&mut self, batch: SceneRenderBatch) {self.batches.lock().unwrap().push(batch);}
    fn unlock_ending(&mut self,id:&str){self.events.lock().unwrap().push(format!("ending:{id}"));}
    fn set_typewriter_config(&mut self, speed: f32) {self.configs.lock().unwrap().push(speed);}
    fn set_background(&mut self, path: &str, _: &Transition) {self.events.lock().unwrap().push(format!("bg:{path}"));}
    fn show_cinematic(&mut self,id:&str,_:Option<&str>){self.events.lock().unwrap().push(format!("cg:{id}"));}
    fn hide_cinematic(&mut self,_:Option<&str>){self.events.lock().unwrap().push("cg-hide".into());}
    fn animate_sprite(&mut self,id:&str,_:&str,_:&[rvn_parser::AnimationParam]){self.events.lock().unwrap().push(format!("animate:{id}"));}
    fn stop_sprite_animation(&mut self,id:&str){self.events.lock().unwrap().push(format!("stop:{id}"));}
    fn set_sprite_effect(&mut self,id:&str,_:Option<bool>,_:Option<bool>,_:Option<f32>,_:Option<f32>,_:Option<&str>){self.events.lock().unwrap().push(format!("effect:{id}"));}
    fn show_sprite(&mut self, id: &str, _: Option<&str>, _: &Position, _: &Transition, _: Option<&SpriteState>) {self.events.lock().unwrap().push(format!("show:{id}"));}
    fn hide_sprite(&mut self, id: &str, _: &Transition, _: &SpriteState) {self.events.lock().unwrap().push(format!("hide:{id}"));}
    fn move_sprite(&mut self, id: &str, _: &Position, _: &Transition, _: &SpriteState) {self.events.lock().unwrap().push(format!("move:{id}"));}
    fn show_dialogue(&mut self, _: Option<&str>, _: &str) {}
    fn show_choice(&mut self, _: &[String]) -> usize {0}
    fn show_imagemap(&mut self, _: &str, _: Option<&str>, _: &[Hotspot]) -> usize {panic!("Scheduled preparation cannot invent a hotspot response")}
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

#[test]
fn native_speed_file_program_reaches_real_dispatch() {
 let dir=fixture_dir("classic-speed");let path=dir.join("main.rvn");
 std::fs::write(&path,"label start\nflow.consume_v2(\"scope\",\"speed\"){typewriter.speed(23)}\n\"boundary\"\n").unwrap();
 let p=rvn_parser::parse_file_program_with_uses(&path).unwrap();
 let mut e=Engine::from_program(p,Host::default(),8).expect("shared grammar must have an actual Core adapter");
 e.step_until_interaction().unwrap();assert_eq!(e.state.typewriter.chars_per_sec,23);assert!(e.state.typewriter.enabled);
 assert_eq!(*e.renderer.configs.lock().unwrap(),vec![23.]);
}

fn file_program(name:&str,source:&str)->ParsedProgram {
 let path=fixture_dir(name).join("main.rvn");std::fs::write(&path,source).unwrap();
 rvn_parser::parse_file_program_with_uses(path).unwrap()
}
const SETUP:&str="hero.show(\"hero.png\") at center\nhero.animate(\"bounce\")\nhero.effect(scale:1.1)\ntimer 0.5 => jump start\ncinematic \"previous.png\"\n";
fn wrap(body:&str)->String{format!("flow.consume_v2(\"scope\",\"consumer\"){{capture \"draw\"=random(1,9)\n{body}}}")}
fn check_effect(e:&Engine<Host>,kind:&str){match kind {
 "config"|"background"=>assert_eq!(e.state.background_image,"garden.png"),
 "cinematic_show"=>assert_eq!(e.state.cinematic.current.as_deref(),Some("cg.png")),
 "cinematic_hide"=>assert!(e.state.cinematic.current.is_none()),
 "ending"=>assert_eq!(*e.renderer.events.lock().unwrap(),vec!["ending:ending"]),
 "sprite_show"=>assert!(e.state.sprites["hero"].visible),
 "sprite_hide"=>assert!(!e.state.sprites["hero"].visible),
 "sprite_move"=>assert_eq!(e.state.sprites["hero"].position,Position::Left),
 "sprite_effect"=>assert_eq!(e.state.scene_execution.as_ref().unwrap().sprite_styles["hero"].scale,1.25),
 "timer"=>assert_eq!(e.state.scene_execution.as_ref().unwrap().active.as_ref().map(|a|&a.tasks).unwrap_or(&e.state.scene_execution.as_ref().unwrap().game_tasks).timer.as_ref().unwrap().duration,0.25),
 "timer_cancel"=>assert!(e.state.scene_execution.as_ref().unwrap().active.as_ref().map(|a|&a.tasks).unwrap_or(&e.state.scene_execution.as_ref().unwrap().game_tasks).timer.is_none()),
 "set"=>assert!(e.state.typewriter.enabled),
 "speed"=>assert_eq!(e.state.typewriter.chars_per_sec,23),
 "sprite_animate"|"sprite_stop"=>{let execution=e.state.scene_execution.as_ref().unwrap();let count=execution.resources.active_effects(execution.owner()).into_iter().filter(|task|task.effect.as_ref().is_some_and(|effect|matches!(effect.parameters,rvn_core::scene_resources::EffectParameters::SpriteAnimation{..}))).count();assert_eq!(count,usize::from(kind=="sprite_animate"));},
 _=>panic!("unknown actual effect oracle {kind}"),
}}
fn scene_command(name:&str,body:&str){
 let p=file_program(name,&format!("scene label start\n{SETUP}\"before\"\n{}\n\"after\"\n",wrap(body)));
 let mut e=Engine::from_program(p,Host::default(),8).unwrap();e.step_until_interaction().unwrap();e.advance_dialogue().unwrap();
 e.state.random=rvn_core::random::RandomState::seeded(17);let mut expected=e.state.random;expected.integer(1,9).unwrap();
 let before=serde_json::to_value(&e.state).unwrap();let count=e.renderer.batches.lock().unwrap().len();
 let session=e.debug_begin(rvn_core::engine::DebugOperation::Statement,vec![]).unwrap();
 e.renderer.reject.store(true,Ordering::SeqCst);assert!(e.step_silent().is_err());
 assert_eq!(serde_json::to_value(&e.state).unwrap(),before);assert_eq!(e.renderer.batches.lock().unwrap().len(),count);
 assert!(e.renderer.events.lock().unwrap().is_empty());assert!(e.renderer.configs.lock().unwrap().is_empty());assert!(e.accepts_debug_session(&session));
 e.renderer.reject.store(false,Ordering::SeqCst);e.step_silent().unwrap();
 assert_eq!(e.state.random,expected);assert_eq!(e.state.pc,before["pc"].as_u64().unwrap()as usize+1);
 assert_eq!(e.renderer.batches.lock().unwrap().len(),count+1);check_effect(&e,name);
 let after=serde_json::to_value(&e.state).unwrap();e.step_silent().unwrap();assert_eq!(serde_json::to_value(&e.state).unwrap(),after);
 assert_eq!(e.renderer.batches.lock().unwrap().len(),count+1);
}
macro_rules! scene_case{($name:ident,$source:expr)=>{#[test]fn $name(){scene_command(stringify!($name),$source);}}}
scene_case!(config,"bg: \"garden.png\"");
scene_case!(background,"scene \"garden.png\"");
scene_case!(cinematic_show,"cinematic \"cg.png\"");
scene_case!(cinematic_hide,"cinematic hide");
scene_case!(ending,"unlock_ending \"ending\"");
scene_case!(sprite_show,"hero.show(\"hero.png\") at center");
scene_case!(sprite_hide,"hero.hide()");
scene_case!(sprite_move,"hero.move() at left");
scene_case!(sprite_animate,"hero.animate(\"bounce\")");
scene_case!(sprite_stop,"hero.stop_animation()");
scene_case!(sprite_effect,"hero.effect(scale:1.25,flip_x:true)");
scene_case!(timer,"timer 0.25 => jump start");
scene_case!(timer_cancel,"timer cancel");
scene_case!(set,"typewriter=true");
scene_case!(speed,"typewriter.speed(23)");

fn refused(name:&str,body:&str,diagnostic:&str){
 let p=file_program(name,&format!("init{{set sentinel=1}}\nscene label start\n{}\n\"after\"\n",wrap(body)));
 let host=Host::default();let error=Engine::from_program(p,host.clone(),8).err().expect("unsupported adapter refuses before Init").to_string();
 assert!(error.contains(diagnostic),"{error}");assert!(host.batches.lock().unwrap().is_empty());assert!(host.events.lock().unwrap().is_empty());
}
#[test]fn character_create_uses_saved_state_instead_of_print_only(){let p=file_program("character-create","scene label start\nflow.consume_v2(\"scope\",\"consumer\"){character.create(\"hero\",\"Hero\")}\n\"after\"\n");let mut e=Engine::from_program(p,Host::default(),8).unwrap();e.step_until_interaction().unwrap();assert_eq!(e.state.layered.display_names["hero"],"Hero");assert!(e.renderer.batches.lock().unwrap().iter().any(|b|b.state.layered.display_names.get("hero").is_some_and(|name|name=="Hero")));}
#[test]fn custom_method_is_not_admitted_as_print_only(){refused("method","hero.custom(\"value\")","diagnostic-only");}
#[test]fn imagemap_requires_real_interactive_continuation(){
 let p=file_program("imagemap",&format!("init{{set sentinel=1}}\nscene label start\n{}\n\"after\"\n",wrap("imagemap {background: \"garden.png\" hotspot {name: \"act\" area: (0,0,100,100)} => {set chosen=1}}")));
 let mut e=Engine::from_program(p,Host::default(),8).unwrap();
 assert_eq!(e.state.vars["sentinel"],Value::Int(1));
 assert!(matches!(e.step_until_interaction().unwrap(),Some(rvn_core::engine::Interaction::Imagemap{hotspots,..})if hotspots.len()==1));
 assert!(!e.state.vars.contains_key("chosen"));let input=e.preview_input_source().unwrap();
 let before=serde_json::to_value(&e.state).unwrap();e.step_silent().unwrap();assert_eq!(serde_json::to_value(&e.state).unwrap(),before);
 e.submit_hotspot(0).unwrap();assert!(matches!(e.step_until_interaction().unwrap(),Some(rvn_core::engine::Interaction::Dialogue{text,..})if text=="after"));
 assert_eq!(e.state.vars["chosen"],Value::Int(1));assert!(!e.accepts_preview_input_source(&input));
}
#[test]fn init_ending_uses_actual_owned_publication_before_gameplay(){
 let p=file_program("init-ending",&format!("init{{{}}}\nscene label start\n\"after\"\n",wrap("unlock_ending \"ending\"")));
 let h=Host::default();let mut e=Engine::from_program(p,h.clone(),8).unwrap();assert!(e.has_frozen_startup_program());assert_eq!(*h.events.lock().unwrap(),vec!["ending:ending"]);e.step_until_interaction().unwrap();assert_eq!(*h.events.lock().unwrap(),vec!["ending:ending"]);
}
#[test]fn invalid_gather_does_not_publish_or_consume_rng(){
 let p=file_program("bad-gather","scene label start\n\"before\"\nflow.consume_v2(\"scope\",\"consumer\"){capture \"bad\"=1/0\nscene \"garden.png\"}\n\"after\"\n");
 let mut e=Engine::from_program(p,Host::default(),8).unwrap();e.step_until_interaction().unwrap();e.advance_dialogue().unwrap();
 let before=serde_json::to_value(&e.state).unwrap();let count=e.renderer.batches.lock().unwrap().len();assert!(e.step_silent().is_err());assert_eq!(serde_json::to_value(&e.state).unwrap(),before);assert_eq!(e.renderer.batches.lock().unwrap().len(),count);assert!(e.renderer.events.lock().unwrap().is_empty());
}

const FAMILIES:&[(&str,&str)]=&[
 ("config","bg: \"garden.png\""),("background","scene \"garden.png\""),("cinematic_show","cinematic \"cg.png\""),("cinematic_hide","cinematic hide"),("ending","unlock_ending \"ending\""),
 ("sprite_show","hero.show(\"hero.png\") at center"),("sprite_hide","hero.hide()"),("sprite_move","hero.move() at left"),("sprite_animate","hero.animate(\"bounce\")"),("sprite_stop","hero.stop_animation()"),("sprite_effect","hero.effect(scale:1.25,flip_x:true)"),
 ("timer","timer 0.25 => jump start"),("timer_cancel","timer cancel"),("set","typewriter=true"),("speed","typewriter.speed(23)")];
#[test]fn all_executable_families_use_actual_game_owner(){
 for (kind,body) in FAMILIES {
  let p=file_program(kind,&format!("label start\n{SETUP}\"before\"\n{}\n\"after\"\nscene label later\n\"later\"\n",wrap(body)));
  let mut e=Engine::from_program(p,Host::default(),8).unwrap();e.step_until_interaction().unwrap();e.advance_dialogue().unwrap();
  assert!(e.state.scene_execution.as_ref().unwrap().active.is_none());
  e.state.random=rvn_core::random::RandomState::seeded(17);let mut expected=e.state.random;expected.integer(1,9).unwrap();
  e.step_silent().unwrap();assert_eq!(e.state.random,expected);check_effect(&e,kind);
  let frame=e.state.consumer_frame.as_ref().expect("real Game frame");assert_eq!(frame.owner,rvn_core::consumer_schedule::ConsumerOwner::Game);
 }
}
#[test]fn legal_init_families_reuse_actual_game_resources(){
 for (kind,body) in FAMILIES.iter().filter(|(k,_)|*k!="ending") {
  let p=file_program(kind,&format!("init{{{SETUP}{}}}\nscene label start\n\"after\"\n",wrap(body)));
  let e=Engine::from_program(p,Host::default(),8).unwrap();check_effect(&e,kind);
  assert!(e.state.scene_execution.as_ref().unwrap().active.is_none());assert!(e.state.consumer_frame.is_some());
 }
}
#[test]fn cold_save15_all_executable_families_preserves_effect_without_replay(){
 for (kind,body) in FAMILIES {
  let p=file_program(kind,&format!("scene label start\n{SETUP}{}\n\"after\"\n",wrap(body)));
  let mut e=Engine::from_program(p.clone(),Host::default(),8).unwrap();e.step_until_interaction().unwrap();check_effect(&e,kind);
  let save=e.capture_save15(&p,None,1,kind.to_string()).unwrap();let bytes=save.encode().unwrap();let rng=e.state.random;let pc=e.state.pc;
  let mut cold=Engine::from_program(p.clone(),Host::default(),8).unwrap();let decoded=rvn_core::save::consumer_checkpoint::Save15::decode(&bytes).unwrap();
  cold.restore_save15(&p,decoded).unwrap();assert_eq!(cold.state.random,rng);assert_eq!(cold.state.pc,pc);
  assert!(cold.renderer.events.lock().unwrap().is_empty(),"restore must not replay ending or other legacy intent");
  if *kind!="ending"{check_effect(&cold,kind);}
  let count=cold.renderer.batches.lock().unwrap().len();cold.step_silent().unwrap();assert_eq!(cold.renderer.batches.lock().unwrap().len(),count);assert_eq!(cold.state.random,rng);
 }
}
#[test]fn rollback_all_executable_families_keeps_rejection_atomic(){
 for (kind,body) in FAMILIES {
  let p=file_program(kind,&format!("scene label start\n{SETUP}\"before\"\n{}\n\"after\"\n",wrap(body)));
  let mut e=Engine::from_program(p,Host::default(),8).unwrap();e.step_until_interaction().unwrap();let before=e.state.clone();e.advance_dialogue().unwrap();e.step_silent().unwrap();
  let after=serde_json::to_value(&e.state).unwrap();let count=e.renderer.batches.lock().unwrap().len();e.renderer.reject.store(true,Ordering::SeqCst);assert!(!e.rollback());assert_eq!(serde_json::to_value(&e.state).unwrap(),after);assert_eq!(e.renderer.batches.lock().unwrap().len(),count);
  e.renderer.reject.store(false,Ordering::SeqCst);assert!(e.rollback());assert_eq!(e.state.random,before.random);assert_eq!(e.state.pc,before.pc);assert_eq!(e.state.typewriter,before.typewriter);assert_eq!(e.state.background_image,before.background_image);
 }
}
#[test]fn prepared_init_speed_is_candidate_only_and_stale_guarded(){
 let p=file_program("init-speed",&format!("init{{{}}}\nscene label start\n\"after\"\n",wrap("typewriter.speed(23)")));
 let h=Host::default();let mut owner=Engine::prepare_debugger_startup(p,h.clone(),8,Default::default(),Default::default(),None).unwrap();let initial=owner.state().random;let mut expected=initial;expected.integer(1,9).unwrap();
 for _ in 0..32 {
  if owner.is_complete(){break;}
  let before=serde_json::to_value(owner.state()).unwrap();let count=h.batches.lock().unwrap().len();let plan=owner.prepare_step(rvn_core::editor_debugger::StepKind::Continue).unwrap();let stale=owner.prepare_step(rvn_core::editor_debugger::StepKind::Continue).unwrap();
  assert_eq!(serde_json::to_value(owner.state()).unwrap(),before);assert_eq!(h.batches.lock().unwrap().len(),count);assert!(h.configs.lock().unwrap().is_empty());
  owner.begin_prepared_step_commit(plan).unwrap().commit();let after=serde_json::to_value(owner.state()).unwrap();assert!(owner.begin_prepared_step_commit(stale).is_err());assert_eq!(serde_json::to_value(owner.state()).unwrap(),after);
 }
 assert!(owner.is_complete());assert_eq!(owner.state().typewriter.chars_per_sec,23);assert_eq!(owner.state().random,expected);assert_eq!(h.batches.lock().unwrap().len(),1);
}

#[test]fn legacy_callbacks_are_ordered_once_after_actual_preflight(){
 let commands=["scene \"garden.png\"","cinematic \"cg.png\"","cinematic hide","hero.show(\"hero.png\") at center","hero.move() at left","hero.animate(\"bounce\")","hero.stop_animation()","hero.effect(scale:1.25)","hero.hide()","unlock_ending \"ending\"","typewriter=true","typewriter.speed(23)"];
 // Raw authored commands retain the exact historical Renderer extension
 // points. Wrapping these commands in consume_v2 changes their real origin
 // to Native and therefore cannot serve as the legacy callback oracle.
 let raw=format!("label start\n{}\n\"after\"\n",commands.join("\n"));
 let mut legacy=Engine::from_program(file_program("legacy-order",&raw),Host::default(),8).unwrap();
 assert!(legacy.state.scene_execution.is_none());assert!(legacy.state.game_tasks.is_none());
 legacy.step_until_interaction().unwrap();
 assert_eq!(*legacy.renderer.events.lock().unwrap(),vec!["bg:garden.png","cg:cg.png","cg-hide","show:hero","move:hero","animate:hero","stop:hero","effect:hero","hide:hero","ending:ending"]);
 assert_eq!(*legacy.renderer.configs.lock().unwrap(),vec![40.,23.]);

 // Native commands instead publish the portable owned batch once. Exercise
 // every original command, with a real rejected renderer preflight before
 // its successful commit, so the corrected origin oracle cannot hide a
 // missing sprite effect or replace the Native path with raw execution.
 let source=format!("label start\n\"ready\"\n{}\n",commands.iter().enumerate().map(|(i,c)|format!("flow.consume_v2(\"scope\",\"c{i}\"){{{c}}}\n\"after-{i}\"")).collect::<Vec<_>>().join("\n"));
 let mut e=Engine::from_program(file_program("native-order",&source),Host::default(),8).unwrap();
 assert!(e.state.scene_execution.is_none());assert!(e.state.game_tasks.is_some());
 assert!(matches!(e.step_until_interaction().unwrap(),Some(rvn_core::engine::Interaction::Dialogue{text,..})if text=="ready"));
 for i in 0..commands.len(){
  e.advance_dialogue().unwrap();
  let before=serde_json::to_value(&e.state).unwrap();let rng=e.state.random;let pc=e.state.pc;let epoch=e.runtime_epoch();
  let count=e.renderer.batches.lock().unwrap().len();let events=e.renderer.events.lock().unwrap().clone();let configs=e.renderer.configs.lock().unwrap().clone();
  e.renderer.reject.store(true,Ordering::SeqCst);
  let error=e.step_silent().unwrap_err().to_string();assert!(error.contains("typewriter fixture rejects candidate"),"{error}");
  assert_eq!(serde_json::to_value(&e.state).unwrap(),before);assert_eq!(e.runtime_epoch(),epoch);
  assert_eq!(e.renderer.batches.lock().unwrap().len(),count);assert_eq!(*e.renderer.events.lock().unwrap(),events);assert_eq!(*e.renderer.configs.lock().unwrap(),configs);
  e.renderer.reject.store(false,Ordering::SeqCst);e.step_silent().unwrap();
  assert_eq!(e.state.pc,pc+1);assert_eq!(e.state.random,rng);
  assert_eq!(e.renderer.batches.lock().unwrap().len(),count+1,"command {i} publishes exactly one owned batch");
  {
   let batches=e.renderer.batches.lock().unwrap();let batch=batches.last().unwrap();
   assert_eq!(serde_json::to_value(&batch.state).unwrap(),serde_json::to_value(&e.state).unwrap());
   match i{
    0=>assert_eq!(batch.state.background_image,"garden.png"),
    1=>assert_eq!(batch.state.cinematic.current.as_deref(),Some("cg.png")),
    2=>assert!(batch.state.cinematic.current.is_none()),
    3=>assert!(batch.state.sprites["hero"].visible),
    4=>assert_eq!(batch.state.sprites["hero"].position,Position::Left),
    5=>{
     assert!(batch.state.sprites["hero"].visible,"animation requires the real visible sprite");
     let game=batch.state.game_tasks.as_ref().unwrap();assert_eq!(game.resources.tasks.len(),1);
     let task=&game.resources.tasks[0];assert_eq!(task.key.owner,rvn_core::scene_tasks::TaskOwner::Game);assert_eq!(task.target.alias,"hero");
     assert!(task.effect.as_ref().is_some_and(|effect|matches!(effect.parameters,rvn_core::scene_resources::EffectParameters::SpriteAnimation{..})));
     assert_eq!(batch.effects.len(),1);
    },
    6=>{assert!(batch.state.game_tasks.as_ref().unwrap().resources.tasks.is_empty());assert!(batch.effects.is_empty());},
    7=>{assert!(batch.state.sprites["hero"].visible);assert_eq!(batch.state.game_tasks.as_ref().unwrap().sprite_styles["hero"].scale,1.25);},
    8=>assert!(!batch.state.sprites["hero"].visible),
    10=>assert!(batch.state.typewriter.enabled),
    11=>assert_eq!(batch.state.typewriter.chars_per_sec,23),
    _=>{},
   }
  }
  assert!(matches!(e.step_until_interaction().unwrap(),Some(rvn_core::engine::Interaction::Dialogue{text,..})if text==format!("after-{i}")));
  let state=serde_json::to_value(&e.state).unwrap();let count=e.renderer.batches.lock().unwrap().len();let events=e.renderer.events.lock().unwrap().clone();let configs=e.renderer.configs.lock().unwrap().clone();
  e.step_silent().unwrap();assert_eq!(serde_json::to_value(&e.state).unwrap(),state);assert_eq!(e.renderer.batches.lock().unwrap().len(),count);assert_eq!(*e.renderer.events.lock().unwrap(),events);assert_eq!(*e.renderer.configs.lock().unwrap(),configs);
 }
 assert_eq!(*e.renderer.events.lock().unwrap(),vec!["bg:garden.png","cg:cg.png","cg-hide","show:hero","move:hero","hide:hero","ending:ending"]);
 assert_eq!(*e.renderer.configs.lock().unwrap(),vec![40.,23.]);
}
#[test]fn bounded_intents_refuse_before_live_allocation_or_publication(){
 let long="x".repeat(70000);let p=file_program("intent-budget",&format!("label start\n\"before\"\nflow.consume_v2(\"scope\",\"ending\"){{unlock_ending \"{long}\"}}\n\"after\"\n"));
 let mut e=Engine::from_program(p,Host::default(),8).unwrap();e.step_until_interaction().unwrap();e.advance_dialogue().unwrap();let before=serde_json::to_value(&e.state).unwrap();
 let error=e.step_silent().unwrap_err().to_string();assert!(error.contains("128 commands or 64 KiB"),"{error}");assert_eq!(serde_json::to_value(&e.state).unwrap(),before);assert!(e.renderer.events.lock().unwrap().is_empty());
}
