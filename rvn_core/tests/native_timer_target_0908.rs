//! Timer5 identities migrated explicitly to the actual saved Game owner.
//! The real fixture barrier stops before the target; invalid-target, RNG and Legacy oracles remain intact.
use rvn_core::{Engine, Renderer, renderer::SceneRenderBatch, types::SpriteState};
use rvn_parser::{Hotspot, Position, Transition, Statement};
use std::sync::{Arc, Mutex, atomic::{AtomicBool, AtomicUsize, Ordering}};
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


fn engine(action:&str,native:bool)->Engine<Host>{
 static NEXT:AtomicUsize=AtomicUsize::new(0);
 let root=std::path::PathBuf::from(std::env::var("RVN_NATIVE_TYPEWRITER_FIXTURES").expect("existing root-owned private fixture directory"));
 let dir=root.join(format!("timer-target-{}-{}",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)));
 std::fs::create_dir_all(&dir).unwrap();let file=dir.join("main.rvn");
 let command=format!("timer 0.5 => {action}");
 let statement=if native{format!("flow.consume_v2(\"actual_timer\",\"timer\"){{capture \"draw\"=random(1,9) {command}}}")}else{command};
 let source=format!("label start\ntimer 10.0 => jump start\n\"before timer command\"\n{statement}\n\"boundary\"\nlabel later\n\"later\"\n");
 std::fs::write(&file,source).unwrap();let program=rvn_parser::parse_file_program_with_uses(&file).unwrap();
 let mut engine=Engine::from_program(program,Host::default(),8).unwrap();
 // step_silent executes until an interaction, not one instruction. A real
 // authored dialogue prevents fixture setup from executing the target timer.
 engine.step_until_interaction().unwrap();
 assert!(matches!(engine.peek_statement(),Some(Statement::Dialogue{text,..}) if text.as_plain()==Some("before timer command")));
 if native {
  let game=engine.state.game_tasks.as_ref().expect("actual Game owner admitted by Native source");
  let prior=game.tasks.timer.as_ref().expect("authored prior timer");
  assert_eq!((prior.duration,prior.action.as_str(),prior.elapsed),(10.0,"jump start",0.0));
  assert!(game.historical_timer);assert!(engine.active_timer.is_none());
 }else{assert!(engine.state.game_tasks.is_none());assert_eq!(engine.active_timer,Some((10.0,"jump start".into(),0.0)));}
 engine.advance_dialogue().unwrap();
 assert!(if native{matches!(engine.peek_statement(),Some(Statement::ScheduledConsumer{..}))}else{matches!(engine.peek_statement(),Some(Statement::Timer{duration_secs,..})if *duration_secs==0.5)},"setup must stop at the actual unexecuted target command");
 engine
}
fn refuse(action:&str){
 let mut engine=engine(action,true);assert!(engine.state.scene_execution.is_none(),"genuine standalone Game path");
 engine.state.random=rvn_core::random::RandomState::seeded(17);
 let before=serde_json::to_value(&engine.state).unwrap();let timer=engine.active_timer.clone();let batches=engine.renderer.batches.lock().unwrap().len();let configs=engine.renderer.configs.lock().unwrap().clone();
 let source=engine.debug_state_source().unwrap();let session=engine.debug_begin(rvn_core::engine::DebugOperation::Statement,vec![]).unwrap();
 let error=engine.step_silent().expect_err("Native standalone Timer must resolve its target before replacing the real timer").to_string();
 assert!(error.contains("missing"),"{error}");assert_eq!(serde_json::to_value(&engine.state).unwrap(),before);assert_eq!(engine.active_timer,timer);assert_eq!(engine.renderer.batches.lock().unwrap().len(),batches);assert_eq!(*engine.renderer.configs.lock().unwrap(),configs);assert!(engine.accepts_debug_state_source(&source));assert!(engine.accepts_debug_session(&session));
}
#[test]fn standalone_native_timer_missing_jump_is_atomic(){refuse("jump missing");}
#[test]fn standalone_native_timer_missing_call_is_atomic(){refuse("call missing");}
fn valid(action:&str){
 let mut engine=engine(action,true);let mut expected=engine.state.random;expected.integer(1,9).unwrap();let pc=engine.state.pc;
 engine.step_silent().unwrap();assert_eq!(engine.state.random,expected);assert_eq!(engine.state.pc,pc+1);assert!(engine.active_timer.is_none());
 let timer=engine.state.game_tasks.as_ref().unwrap().tasks.timer.as_ref().unwrap();assert_eq!((timer.duration,timer.action.as_str(),timer.elapsed),(0.5,action,0.0));
 assert_eq!(engine.tick_timer(0.25),None);assert_eq!(engine.state.game_tasks.as_ref().unwrap().tasks.timer.as_ref().unwrap().elapsed,0.0,"legacy tick cannot advance owned Game");
 assert_eq!(engine.tick_game_frame(0.25,Default::default()).unwrap().callbacks,0);assert_eq!(engine.tick_game_frame(0.25,Default::default()).unwrap().callbacks,1);assert!(engine.state.game_tasks.as_ref().unwrap().tasks.timer.is_none());
}
#[test]fn valid_native_jump_uses_actual_timer_and_gathers_once(){valid("jump later");}
#[test]fn valid_native_call_uses_actual_timer_and_gathers_once(){valid("call later");}
#[test]fn raw_legacy_timer_target_admission_is_preserved(){
 let mut engine=engine("jump missing",false);engine.step_silent().unwrap();assert_eq!(engine.active_timer,Some((0.5,"jump missing".into(),0.0)));
}
