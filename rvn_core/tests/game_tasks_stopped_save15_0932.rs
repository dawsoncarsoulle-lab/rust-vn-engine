//! Actual stopped operations and cold initializer ownership; no fake evaluator root.
use rvn_core::{Engine, Renderer, renderer::SceneRenderBatch, types::SpriteState};
use rvn_core::engine::{DebugOperation, DebugProgress, DebugOutcome, DebugSession, EngineStartup};
use rvn_core::editor_debugger::StepKind;
use rvn_core::save::consumer_checkpoint::Save15;
use rvn_parser::{Position, Transition, Hotspot, Value, ParsedProgram};
use std::sync::{Arc, Mutex, atomic::{AtomicBool, AtomicUsize, Ordering}};

#[derive(Clone, Default)]
struct Host { reject: Arc<AtomicBool>, batches: Arc<Mutex<Vec<SceneRenderBatch>>> }
impl Renderer for Host {
    fn validate_scene_batch(&self, _: &SceneRenderBatch) -> Result<(), String> {
        if self.reject.load(Ordering::SeqCst) { Err("actual renderer refuses publication".into()) } else { Ok(()) }
    }
    fn publish_scene_batch(&mut self, b: SceneRenderBatch) { self.batches.lock().unwrap().push(b); }
    fn set_background(&mut self, _: &str, _: &Transition) {}
    fn show_sprite(&mut self, _: &str, _: Option<&str>, _: &Position, _: &Transition, _: Option<&SpriteState>) {}
    fn hide_sprite(&mut self, _: &str, _: &Transition, _: &SpriteState) {}
    fn move_sprite(&mut self, _: &str, _: &Position, _: &Transition, _: &SpriteState) {}
    fn show_dialogue(&mut self, _: Option<&str>, _: &str) {}
    fn show_choice(&mut self, _: &[String]) -> usize { panic!("not a choice fixture") }
    fn show_imagemap(&mut self, _: &str, _: Option<&str>, _: &[Hotspot]) -> usize { panic!("not an imagemap fixture") }
    fn music_play(&mut self, _: &str, _: &Transition, _: Option<&str>) {}
    fn music_stop(&mut self, _: &Transition) {}
    fn music_set_volume(&mut self, _: f32) {}
    fn sfx_play(&mut self, _: &str, _: &Transition) {}
    fn sfx_stop(&mut self, _: &str, _: &Transition) {}
}
#[path = "private_pcm0370/frame_pcm_host.rs"]
mod actual_pcm;

const STORY: &str = r#"
label start
hero.show("hero.png") at center
flow.consume_v2("animation","command"){hero.animate("bounce",duration:2.0,loop:true)}
flow.consume_v2("style","command"){hero.effect(scale:1.25)}
flow.consume_v2("timer","command"){timer 0.5 => jump done}
"waiting"
label done
"done"
"#;
const INIT: &str = r#"
init {
 hero.show("hero.png") at center
 flow.consume_v2("animation","command"){hero.animate("bounce",duration:2.0,loop:true)}
 flow.consume_v2("style","command"){hero.effect(scale:1.25)}
 flow.consume_v2("timer","command"){timer 5 => jump done}
 set setup=1
 set after=random(1,9)+1
}
label start
"waiting"
label done
"done"
"#;

fn program(text: &str) -> ParsedProgram {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let root = std::env::temp_dir().join(format!("rvn-save0932-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    std::fs::create_dir(&root).unwrap();
    let main = root.join("main.rvn");
    std::fs::write(&main, text).unwrap();
    rvn_parser::parse_file_program_with_uses(main).unwrap()
}
fn game<R: Renderer>(p: &ParsedProgram, host: R) -> Engine<R> {
    Engine::from_program_with_debugger(p.clone(), host, 8).unwrap()
}
fn state<R: Renderer>(e: &Engine<R>) -> serde_json::Value { serde_json::to_value(&e.state).unwrap() }
fn begin<R: Renderer>(e: &Engine<R>, delta: f64) -> DebugSession {
    e.debug_begin(DebugOperation::GameFrame { seconds: delta, reports: Default::default(), audio: vec![] }, vec![]).unwrap()
}
fn finish<R: Renderer>(e: &mut Engine<R>, s: &mut DebugSession) -> DebugOutcome {
    for _ in 0..128 {
        let (progress, outcome) = e.debug_step(s, StepKind::Continue).unwrap();
        if progress == DebugProgress::Complete { return outcome.unwrap(); }
    }
    panic!("actual saved operation did not finish");
}
fn timer_elapsed<R: Renderer>(e: &Engine<R>) -> f64 {
    e.state.game_tasks.as_ref().unwrap().tasks.timer.as_ref().unwrap().elapsed
}
fn sprite_elapsed<R: Renderer>(e: &Engine<R>) -> f64 {
    e.state.game_tasks.as_ref().unwrap().resources.tasks.iter()
        .find_map(|t| t.effect.as_ref()).unwrap().clock.elapsed
}
fn frame_save<R: Renderer>(e: &Engine<R>, p: &ParsedProgram, s: &DebugSession) -> String {
    let bytes = e.capture_save15(p, Some(s), 15, "real pending scalar Game frame".into()).unwrap().encode().unwrap();
    let wire: serde_json::Value = serde_json::from_str(&bytes).unwrap();
    assert!(wire["debugger"]["frame_operation"].is_object());
    assert!(wire["debugger"].get("trace").is_none(), "A scalar clock frame has no evaluator trace to invent");
    assert_eq!(wire["debugger"]["entry_stop"], true);
    bytes
}

#[test]
fn stopped_scalar_game_frame_cold_resave_advances_native_timer_and_sprite_once() {
    let p = program(STORY);
    let mut warm = game(&p, Host::default()); warm.step_until_interaction().unwrap();
    let old_timer = warm.timer_token().unwrap();
    let before = state(&warm);
    let mut pending = begin(&warm, 0.25);
    assert_eq!(state(&warm), before);
    let bytes = frame_save(&warm, &p, &pending);
    assert_eq!(state(&warm), before);
    finish(&mut warm, &mut pending);
    assert_eq!(timer_elapsed(&warm), 0.25); assert_eq!(sprite_elapsed(&warm), 0.25);
    let mut cold = game(&p, Host::default());
    let mut restored = cold.restore_save15(&p, Save15::decode(&bytes).unwrap()).unwrap().unwrap();
    assert_eq!(timer_elapsed(&cold), 0.0); assert_eq!(sprite_elapsed(&cold), 0.0);
    assert!(!cold.fire_timer(&old_timer).unwrap());
    let bytes2 = frame_save(&cold, &p, &restored);
    let mut colder = game(&p, Host::default());
    let mut restored2 = colder.restore_save15(&p, Save15::decode(&bytes2).unwrap()).unwrap().unwrap();
    finish(&mut cold, &mut restored); finish(&mut colder, &mut restored2);
    for actual in [&cold, &colder] {
        assert_eq!(timer_elapsed(actual), 0.25); assert_eq!(sprite_elapsed(actual), 0.25);
        assert_eq!(actual.state.pc, warm.state.pc); assert_eq!(actual.state.random, warm.state.random);
        assert!(actual.state.scene_execution.is_none()); assert!(actual.state.game_audio.is_none());
    }
    assert!(cold.debug_step(&mut restored, StepKind::Continue).is_err());
    assert_eq!(timer_elapsed(&cold), 0.25);
}

#[test]
fn stopped_due_timer_frame_cold_restore_transfers_pc_and_consumes_callback_once() {
    let p = program(STORY); let mut warm = game(&p, Host::default()); warm.step_until_interaction().unwrap();
    let old = warm.timer_token().unwrap(); let pending = begin(&warm, 0.5);
    let bytes = frame_save(&warm, &p, &pending);
    let mut cold = game(&p, Host::default());
    let mut restored = cold.restore_save15(&p, Save15::decode(&bytes).unwrap()).unwrap().unwrap();
    let outcome = finish(&mut cold, &mut restored);
    match outcome { DebugOutcome::Game((frame, _)) => assert_eq!(frame.frame.callbacks, 1), _ => panic!("wrong operation") }
    assert!(cold.timer_token().is_none()); assert!(!cold.fire_timer(&old).unwrap());
    assert_eq!(sprite_elapsed(&cold), 0.5);
    assert!(matches!(cold.step_until_interaction().unwrap(), Some(rvn_core::Interaction::Dialogue { text, .. }) if text == "done"));
}

#[test]
fn stopped_frame_save_restore_rejection_preserves_live_owner_epoch_and_publication() {
    let p = program(STORY); let mut warm = game(&p, Host::default()); warm.step_until_interaction().unwrap();
    let pending = begin(&warm, 0.25); let bytes = frame_save(&warm, &p, &pending);
    let mut live = game(&p, Host::default()); live.step_until_interaction().unwrap();
    let before = state(&live); let epoch = live.runtime_epoch(); let owner = live.debug_state_source().unwrap();
    let count = live.renderer.batches.lock().unwrap().len();
    let prepared = live.prepare_save15_with_locale(&p, Save15::decode(&bytes).unwrap(), None).unwrap();
    assert_eq!(state(&live), before); drop(prepared);
    let prepared = live.prepare_save15_with_locale(&p, Save15::decode(&bytes).unwrap(), None).unwrap();
    live.renderer.reject.store(true, Ordering::SeqCst);
    assert!(live.begin_prepared_save15_commit(prepared).is_err());
    assert_eq!(state(&live), before); assert_eq!(live.runtime_epoch(), epoch);
    assert!(live.accepts_debug_state_source(&owner)); assert_eq!(live.renderer.batches.lock().unwrap().len(), count);
}

#[test]
fn stopped_frame_source_binding_change_rejects_without_publishing_or_advancing_clocks() {
    let p = program(STORY); let mut warm = game(&p, Host::default()); warm.step_until_interaction().unwrap();
    let pending = begin(&warm, 0.25); let bytes = frame_save(&warm, &p, &pending);
    let changed = program(&STORY.replace("scale:1.25", "scale:1.5"));
    let mut cold = game(&changed, Host::default()); let before = state(&cold); let epoch = cold.runtime_epoch();
    assert!(cold.restore_save15(&changed, Save15::decode(&bytes).unwrap()).is_err());
    assert_eq!(state(&cold), before); assert_eq!(cold.runtime_epoch(), epoch);
    assert!(cold.renderer.batches.lock().unwrap().is_empty());
}

#[test]
fn trace_free_frame_wire_rejects_forged_phase_and_missing_program_owned_authority() {
    let p = program(STORY); let mut warm = game(&p, Host::default()); warm.step_until_interaction().unwrap();
    let pending = begin(&warm, 0.25); let bytes = frame_save(&warm, &p, &pending);
    for field in ["entry_stop", "work", "last_site", "game_tasks", "clock"] {
        let mut wire: serde_json::Value = serde_json::from_str(&bytes).unwrap();
        match field {
            "entry_stop" => wire["debugger"]["entry_stop"] = false.into(),
            "work" => wire["debugger"]["work"] = serde_json::json!([1, 0]),
            "last_site" => wire["debugger"]["last_site"] = serde_json::json!({"function":null,"route":[],"depth":0,"statement":true}),
            "game_tasks" => wire["story"]["base"]["game_tasks"] = serde_json::Value::Null,
            "clock" => wire["debugger"]["frame_operation"]["clock"] = "host".into(),
            _ => unreachable!(),
        }
        assert!(Save15::decode(&wire.to_string()).is_err(), "forged phase {field} passed wire admission");
    }
}

fn pcm_program() -> ParsedProgram {
    program(&STORY.replace("label start\n", "label start\nflow.consume_v2(\"audio\",\"command\"){sfx.play(\"host.wav\")}\n"))
}

#[test]
fn stopped_game_frame_pcm_identity_and_cursor_survive_cold_restore_and_old_epoch_is_stale() {
    let p = pcm_program(); let mut warm = game(&p, actual_pcm::PcmHost::default()); warm.step_until_interaction().unwrap();
    warm.renderer.bind_actual(&warm.state, warm.instance_id(), warm.runtime_epoch()).unwrap();
    let old = warm.audio_playback_views().unwrap().remove(0); let target = old.token.target.clone();
    assert_eq!(warm.renderer.pull(&target, 7).unwrap().len(), 7);
    let pending = begin(&warm, 0.25); let bytes = frame_save(&warm, &p, &pending);
    assert_eq!(warm.renderer.cursor(&target).unwrap().next_sample, 7);
    let mut cold = game(&p, actual_pcm::PcmHost::default());
    cold.renderer.bind_actual(&cold.state, cold.instance_id(), cold.runtime_epoch()).unwrap();
    let mut restored = cold.restore_save15(&p, Save15::decode(&bytes).unwrap()).unwrap().unwrap();
    cold.renderer.bind_actual(&cold.state, cold.instance_id(), cold.runtime_epoch()).unwrap();
    assert_eq!(cold.renderer.cursor(&target).unwrap().next_sample, 7);
    assert_eq!(cold.audio_report_admission(warm.instance_id(), &old.token), rvn_core::audio_playback::AudioAdmission::Stale);
    finish(&mut cold, &mut restored);
    assert_eq!(timer_elapsed(&cold), 0.25); assert_eq!(sprite_elapsed(&cold), 0.25);
    assert_eq!(cold.renderer.cursor(&target).unwrap().next_sample, 7, "logical clocks cannot consume PCM samples");
    let restored_cursor = cold.renderer.cursor(&target).unwrap();
    // A restored interleaved source pads its output to the retained channel.
    // Padding consumes no PCM sample and cannot advance the saved cursor.
    let padding = usize::from(restored_cursor.next_channel);
    assert_eq!(cold.renderer.pull(&target, padding).unwrap(), vec![0; padding]);
    assert_eq!(cold.renderer.cursor(&target).unwrap(), restored_cursor);
    assert_eq!(cold.renderer.pull(&target, 3).unwrap(), vec![7, 8, 9]);
}

#[test]
fn stopped_audio_only_game_frame_cold_restore_does_not_fabricate_a_sprite_task_owner() {
    let p = program("label start\nflow.consume_v2(\"audio\",\"command\"){sfx.play(\"host.wav\")}\n\"waiting\"\n");
    let mut warm = game(&p, actual_pcm::PcmHost::default()); warm.step_until_interaction().unwrap();
    assert!(warm.state.game_tasks.is_none()); assert!(warm.state.game_audio.is_some());
    warm.renderer.bind_actual(&warm.state, warm.instance_id(), warm.runtime_epoch()).unwrap();
    let target = warm.audio_playback_views().unwrap().remove(0).token.target;
    warm.renderer.pull(&target, 7).unwrap();
    let pending = begin(&warm, 0.25); let bytes = frame_save(&warm, &p, &pending);
    let mut cold = game(&p, actual_pcm::PcmHost::default());
    cold.renderer.bind_actual(&cold.state, cold.instance_id(), cold.runtime_epoch()).unwrap();
    let mut restored = cold.restore_save15(&p, Save15::decode(&bytes).unwrap()).unwrap().unwrap();
    assert!(cold.state.game_tasks.is_none()); assert!(cold.state.scene_execution.is_none());
    finish(&mut cold, &mut restored);
    assert!(cold.state.game_tasks.is_none());
    assert_eq!(cold.renderer.cursor(&target).unwrap().next_sample, 7);
}

#[test]
fn stopped_game_frame_audio_report_ordinals_rehydrate_only_real_current_game_audio_source() {
    use rvn_core::audio_playback::{AudioReport, AudioFeedback};
    let p = pcm_program(); let mut warm = game(&p, actual_pcm::PcmHost::default()); warm.step_until_interaction().unwrap();
    warm.renderer.bind_actual(&warm.state, warm.instance_id(), warm.runtime_epoch()).unwrap();
    let token = warm.audio_playback_views().unwrap().remove(0).token;
    warm.renderer.pull(&token.target, 7).unwrap();
    let real = AudioReport { instance_id: warm.instance_id(), token, feedback: AudioFeedback::End };
    let mut stale = real.clone(); stale.instance_id += 1;
    let mut pending = warm.debug_begin(DebugOperation::GameFrame { seconds: 0.25, reports: Default::default(), audio: vec![stale, real.clone(), real] }, vec![]).unwrap();
    let bytes = frame_save(&warm, &p, &pending);
    let expected = finish(&mut warm, &mut pending);
    let mut cold = game(&p, actual_pcm::PcmHost::default());
    cold.renderer.bind_actual(&cold.state, cold.instance_id(), cold.runtime_epoch()).unwrap();
    let mut restored = cold.restore_save15(&p, Save15::decode(&bytes).unwrap()).unwrap().unwrap();
    let actual = finish(&mut cold, &mut restored);
    match (actual, expected) {
        (DebugOutcome::Game((_, a)), DebugOutcome::Game((_, b))) => {
            assert_eq!(a, b); assert_eq!(a.len(), 3);
            assert_eq!(a[0].disposition, rvn_core::engine::AudioReportDisposition::RejectedStale);
            assert_eq!(a[1].disposition, rvn_core::engine::AudioReportDisposition::Consumed);
        }, _ => panic!("wrong restored frame route"),
    }
    assert!(cold.audio_playback_views().unwrap().is_empty()); assert_eq!(timer_elapsed(&cold), 0.25);
}

#[test]
fn stopped_frame_pcm_fence_rejects_foreign_instance_missing_reader_and_changed_pcm_identity() {
    let p = pcm_program(); let mut warm = game(&p, actual_pcm::PcmHost::default()); warm.step_until_interaction().unwrap();
    let pending = begin(&warm, 0.25); let before = state(&warm);
    assert!(warm.capture_save15(&p, Some(&pending), 1, "missing actual Source".into()).is_err());
    warm.renderer.bind_actual(&warm.state, warm.instance_id(), warm.runtime_epoch()).unwrap();
    let mut rows = warm.renderer.capture_frame_audio_checkpoint(&warm.state, warm.instance_id(), warm.runtime_epoch()).unwrap();
    rows.instance_id += 1;
    assert!(warm.capture_save15_with_fenced_media(&p, Some(&pending), 1, "foreign fence".into(), rows, vec![]).is_err());
    assert_eq!(state(&warm), before);
    let bytes = frame_save(&warm, &p, &pending);
    let mut wire: serde_json::Value = serde_json::from_str(&bytes).unwrap();
    let byte = &mut wire["story"]["base"]["game_audio"]["tasks"][0]["audio"]["checkpoint"]["pcm_sha256"][0];
    *byte = ((byte.as_u64().unwrap() + 1) % 256).into();
    let mut cold = game(&p, actual_pcm::PcmHost::default()); let initial = state(&cold);
    cold.renderer.bind_actual(&cold.state, cold.instance_id(), cold.runtime_epoch()).unwrap();
    let forged = Save15::decode(&wire.to_string()).unwrap();
    assert!(cold.restore_save15(&p, forged).is_err()); assert_eq!(state(&cold), initial);
}

fn startup(p: &ParsedProgram, host: Host) -> EngineStartup<Host> {
    Engine::prepare_debugger_startup(p.clone(), host, 8, Default::default(), Default::default(), None).unwrap()
}
fn paused_init(p: &ParsedProgram, host: Host) -> EngineStartup<Host> {
    let mut actual = startup(p, host);
    for _ in 0..512 {
        if actual.state().vars.get("setup") == Some(&Value::Int(1)) && !actual.state().vars.contains_key("after") {
            assert!(!actual.is_complete()); return actual;
        }
        let prepared = actual.prepare_step(StepKind::Into).unwrap();
        actual.begin_prepared_step_commit(prepared).unwrap().commit();
    }
    panic!("real Init did not pause after committed tasks and before its final expression");
}
fn finish_init(actual: &mut EngineStartup<Host>) {
    for _ in 0..128 {
        if actual.is_complete() { return; }
        let prepared = actual.prepare_step(StepKind::Continue).unwrap();
        actual.begin_prepared_step_commit(prepared).unwrap().commit();
    }
    panic!("real restored Init did not complete");
}

#[test]
fn stopped_native_init_cold_restore_keeps_owned_clocks_and_rng_before_unique_game_handoff() {
    let p = program(INIT); let mut warm = paused_init(&p, Host::default());
    let old = warm.observation_source().unwrap(); let saved_state = serde_json::to_value(warm.state()).unwrap();
    let saved_random = warm.state().random;
    let bytes = warm.capture_save15(15, "actual stopped Init with tasks".into()).unwrap().encode().unwrap();
    assert_eq!(serde_json::to_value(warm.state()).unwrap(), saved_state);
    let wire: serde_json::Value = serde_json::from_str(&bytes).unwrap();
    assert!(wire["debugger"]["startup"].is_object());
    assert!(wire["debugger"]["trace"].is_object(), "Init still owns its genuine pending evaluator computation");
    finish_init(&mut warm); let expected = warm.state().vars["after"].clone(); let expected_random = warm.state().random;
    let mut cold = startup(&p, Host::default()); let uninitialized = cold.observation_source().unwrap();
    assert!(!cold.state().vars.contains_key("setup"));
    cold.restore_save15(Save15::decode(&bytes).unwrap()).unwrap();
    assert!(!cold.accepts_observation_source(&old)); assert!(!cold.accepts_observation_source(&uninitialized));
    assert_eq!(cold.state().random, saved_random); assert!(!cold.state().vars.contains_key("after"));
    assert_eq!(cold.state().game_tasks.as_ref().unwrap().tasks.timer.as_ref().unwrap().elapsed, 0.0);
    finish_init(&mut cold); assert_eq!(cold.state().vars["after"], expected); assert_eq!(cold.state().random, expected_random);
    let mut actual = cold.into_engine().unwrap();
    assert!(actual.state.scene_execution.is_none()); assert!(actual.state.game_audio.is_none());
    assert_eq!(timer_elapsed(&actual), 0.0); assert_eq!(sprite_elapsed(&actual), 0.0);
    actual.tick_game_frame(0.25, Default::default()).unwrap();
    assert_eq!(timer_elapsed(&actual), 0.25); assert_eq!(sprite_elapsed(&actual), 0.25);
}

#[test]
fn stopped_native_init_renderer_and_source_rejections_leave_pending_owner_unchanged() {
    let p = program(INIT); let warm = paused_init(&p, Host::default());
    let bytes = warm.capture_save15(15, "actual pending initializer".into()).unwrap().encode().unwrap();
    let host = Host::default(); let mut cold = startup(&p, host.clone());
    let before = serde_json::to_value(cold.state()).unwrap(); let source = cold.observation_source().unwrap();
    let prepared = cold.prepare_save15_with_locale(Save15::decode(&bytes).unwrap(), None).unwrap();
    host.reject.store(true, Ordering::SeqCst);
    assert!(cold.begin_prepared_save15_commit(prepared).is_err());
    assert_eq!(serde_json::to_value(cold.state()).unwrap(), before); assert!(cold.accepts_observation_source(&source));
    assert!(host.batches.lock().unwrap().is_empty());
    let changed = program(&INIT.replace("timer 5", "timer 6")); let mut foreign = startup(&changed, Host::default());
    let before = serde_json::to_value(foreign.state()).unwrap(); let source = foreign.observation_source().unwrap();
    assert!(foreign.restore_save15(Save15::decode(&bytes).unwrap()).is_err());
    assert_eq!(serde_json::to_value(foreign.state()).unwrap(), before); assert!(foreign.accepts_observation_source(&source));
}
