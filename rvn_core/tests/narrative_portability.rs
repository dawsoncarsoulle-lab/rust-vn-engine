use rvn_core::program::PreparedNarrative;
use rvn_core::renderer::SceneRenderBatch;
use rvn_core::save::{
    SaveData, SaveError, SaveManager, SAVE_FORMAT_VERSION, SCENE_SAVE_FORMAT_VERSION,
};
use rvn_core::scene::FrameKind;
use rvn_core::scene_resources::*;
use rvn_core::scene_tasks::TaskOwner;
use rvn_core::types::GameState;
use rvn_core::{Engine, Renderer, SpriteState};
use rvn_parser::{Hotspot, Position, Transition, Value};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

const SOURCE: &str = "scene label P\n\"parent\"\nscene label C\n\"child\"\nlabel helper\nreturn\n";
#[derive(Default)]
struct Host {
    batches: Vec<SceneRenderBatch>,
}
impl Renderer for Host {
    fn validate_scene_batch(&self, _: &SceneRenderBatch) -> Result<(), String> {
        Ok(())
    }
    fn publish_scene_batch(&mut self, batch: SceneRenderBatch) {
        self.batches.push(batch);
    }
    fn supports_composable_motion(&self) -> bool {
        true
    }
    fn set_background(&mut self, _: &str, _: &Transition) {}
    fn show_sprite(
        &mut self,
        _: &str,
        _: Option<&str>,
        _: &Position,
        _: &Transition,
        _: Option<&SpriteState>,
    ) {
    }
    fn hide_sprite(&mut self, _: &str, _: &Transition, _: &SpriteState) {}
    fn move_sprite(&mut self, _: &str, _: &Position, _: &Transition, _: &SpriteState) {}
    fn show_dialogue(&mut self, _: Option<&str>, _: &str) {}
    fn show_choice(&mut self, _: &[String]) -> usize {
        0
    }
    fn show_imagemap(&mut self, _: &str, _: Option<&str>, _: &[Hotspot]) -> usize {
        0
    }
    fn music_play(&mut self, _: &str, _: &Transition, _: Option<&str>) {}
    fn music_stop(&mut self, _: &Transition) {}
    fn music_set_volume(&mut self, _: f32) {}
    fn sfx_play(&mut self, _: &str, _: &Transition) {}
    fn sfx_stop(&mut self, _: &str, _: &Transition) {}
}
fn fixture() -> (PreparedNarrative, GameState) {
    let parsed = rvn_parser::parse_program(SOURCE).unwrap();
    let program = PreparedNarrative::from_parsed(parsed.clone()).unwrap();
    let mut state = Engine::from_program(parsed, Host::default(), 0)
        .unwrap()
        .state;
    let pc = program.label_table["P"];
    let execution = state.scene_execution.as_mut().unwrap();
    execution.enter("P", pc).unwrap();
    execution.live_position = program.position(pc + 1);
    state.pc = pc + 1;
    state.background_image = "images/parent.png".into();
    state.vars.insert("persistent".into(), Value::Int(41));
    sync_current(&mut state).unwrap();
    (program, state)
}
fn screen(name: &str, order: u64, host: bool) -> rvn_core::ui::ScreenInstance {
    rvn_core::ui::ScreenInstance {
        widget_class:None,
        name: name.into(),
        host_role: host.then_some(rvn_ui::PageRole::Pause),
        host_root: host,
        arguments: vec![],
        modal: false,
        layer: 0,
        order,
        focus: Some("field".into()),
        owner_properties:Default::default(),owner_focus_request:None,
        values: BTreeMap::from([("field".into(), Value::Str("unchanged".into()))]),
        random: rvn_core::random::RandomState::seeded(17),
        canvas_states: BTreeMap::from([(
            "canvas".into(),
            rvn_core::ui::CanvasState {
                state: Value::Dict(BTreeMap::from([("count".into(), Value::Int(12))])),
                elapsed: 7.75,
            },
        )]),
        consumer_frames: BTreeMap::new(),
        consumer_projection: None,
        standard_macro_frame: None,
    }
}
fn host_screen(state: &mut GameState, name: &str) {
    state.ui.screens.push(screen(name, 0, true));
    state.ui.host_next_order = 1;
    sync_current(state).unwrap();
}
fn ui_motion(state: &mut GameState, name: &str, wait: bool) {
    let target = rvn_core::motion::MotionTarget::Interface {
        screen: name.into(),
        element: "field".into(),
    };
    let key = target.key();
    state
        .motions
        .play(target, rvn_ui::motion::Motion::Pause { seconds: 10.0 })
        .unwrap();
    state.motions.tick(2.5).unwrap();
    state.motions.waiting = wait.then_some(key);
    sync_current(state).unwrap();
}
fn embedded_video(state: &mut GameState, name: &str, wait: bool) {
    let clip = rvn_ui::video::VideoClip::parse(serde_json::json!({
        "source":"movies/clip.webm", "target":format!("ui:{name}/field")
    }))
    .unwrap();
    state.videos.play("embedded".into(), clip).unwrap();
    state
        .videos
        .feedback(
            "embedded",
            rvn_core::video::Feedback::Position {
                seconds: 3.25,
                duration: 20.0,
            },
        )
        .unwrap();
    if wait {
        state.videos.wait("embedded").unwrap();
    }
    sync_current(state).unwrap();
}
fn call(program: &PreparedNarrative, state: &mut GameState) {
    park_current(state).unwrap();
    let execution = state.scene_execution.as_mut().unwrap();
    execution
        .call_scene(
            "C",
            program.label_table["C"],
            execution.live_position.clone(),
        )
        .unwrap();
    execution.live_position = program.position(program.label_table["C"] + 1);
    state.pc = execution.live_position.pc;
    state.call_stack = execution.derived_call_stack();
}
fn encoded(state: &GameState) -> serde_json::Value {
    serde_json::to_value(state).unwrap()
}
fn portable(state: &GameState) -> SaveData {
    SaveData::try_from_state(state, 1, "portable".into(), "story.rvn".into()).unwrap()
}
struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let path = std::env::temp_dir().join(format!(
            "rvn-portability-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn bytes(&self) -> BTreeMap<String, Vec<u8>> {
        std::fs::read_dir(&self.0)
            .unwrap()
            .map(|entry| {
                let entry = entry.unwrap();
                assert!(entry.file_type().unwrap().is_file());
                (
                    entry.file_name().to_string_lossy().into_owned(),
                    std::fs::read(entry.path()).unwrap(),
                )
            })
            .collect()
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        // Only the three files this fixture owns; no recursive computed deletion.
        for name in ["slot_01.json", "autosave.json", "quicksave.json"] {
            let _ = std::fs::remove_file(self.0.join(name));
        }
        let _ = std::fs::remove_dir(&self.0);
    }
}
fn refuse_all_writers(state: &GameState) {
    let before = encoded(state);
    let error =
        SaveData::try_from_state(state, 1, "refused".into(), "story.rvn".into()).unwrap_err();
    assert!(
        matches!(error, SaveError::Incompatible(ref reason) if reason.contains("non-portable Host operation")),
        "{error}"
    );
    let files = Files::new();
    let manager = SaveManager::new(&files.0, 2).unwrap();
    let legacy = Engine::new(rvn_parser::parse("\"legacy\"").unwrap(), Host::default(), 0)
        .unwrap()
        .state;
    manager
        .save(&legacy, 1, "original".into(), "legacy.rvn".into())
        .unwrap();
    manager
        .save_autosave(&legacy, "original".into(), "legacy.rvn".into())
        .unwrap();
    manager
        .save_quicksave(&legacy, "original".into(), "legacy.rvn".into())
        .unwrap();
    let original = files.bytes();
    assert_eq!(original.len(), 3);
    for result in [
        manager.save(state, 1, "changed".into(), "story.rvn".into()),
        manager.save_autosave(state, "changed".into(), "story.rvn".into()),
        manager.save_quicksave(state, "changed".into(), "story.rvn".into()),
    ] {
        assert!(
            matches!(result, Err(SaveError::Incompatible(ref reason)) if reason.contains("non-portable Host operation")),
            "{result:?}"
        );
        assert_eq!(files.bytes(), original);
    }
    assert_eq!(encoded(state), before);
}

#[test]
fn active_and_previsit_waits_on_host_motion_refuse_every_writer_without_changing_bytes() {
    for previsit in [false, true] {
        let (_, mut state) = fixture();
        if previsit {
            state.scene_execution.as_mut().unwrap().active = None;
        }
        host_screen(&mut state, "system");
        ui_motion(&mut state, "system", true);
        validate_current_resource_state(&state).unwrap();
        assert_eq!(
            state.scene_execution.as_ref().unwrap().resources.waits[0]
                .owner
                .is_none(),
            previsit
        );
        refuse_all_writers(&state);
    }
}

#[test]
fn parked_video_wait_with_host_dependency_refuses_save_before_losing_parent_continuation() {
    let (program, mut state) = fixture();
    host_screen(&mut state, "system");
    embedded_video(&mut state, "system", true);
    let parent = state.scene_execution.as_ref().unwrap().owner();
    let position = state
        .scene_execution
        .as_ref()
        .unwrap()
        .live_position
        .clone();
    call(&program, &mut state);
    let execution = state.scene_execution.as_ref().unwrap();
    assert_eq!(
        execution.frames[0]
            .parked
            .as_ref()
            .unwrap()
            .tasks
            .suspended_videos
            .waiting
            .as_deref(),
        Some("embedded")
    );
    assert_eq!(execution.resources.waits[0].owner, parent);
    assert_eq!(execution.resources.waits[0].position, position);
    assert!(execution.resources.waits[0].parked);
    refuse_all_writers(&state);
}

#[test]
fn nonwaiting_host_descriptors_are_removed_while_story_clock_and_real_engine_load_survive() {
    let (_, mut state) = fixture();
    host_screen(&mut state, "system");
    ui_motion(&mut state, "system", false);
    embedded_video(&mut state, "system", false);
    state
        .motions
        .play(
            rvn_core::motion::MotionTarget::Background,
            rvn_ui::motion::Motion::Pause { seconds: 11.0 },
        )
        .unwrap();
    state.motions.tick(1.0).unwrap();
    sync_current(&mut state).unwrap();
    let story_clock = state.motions.tracks["background"].clone();
    let before = encoded(&state);
    let save = portable(&state);
    assert_eq!(save.format_version, SCENE_SAVE_FORMAT_VERSION);
    assert!(save.ui.screens.is_empty() && save.videos.tracks.is_empty());
    assert_eq!(save.motions.tracks.len(), 1);
    assert_eq!(save.motions.tracks["background"], story_clock);
    assert_eq!(save.vars["persistent"], rvn_core::save::SaveValue::Int(41));
    let saved_registry = &save.scene_execution.as_ref().unwrap().resources;
    assert!(saved_registry
        .targets
        .iter()
        .all(|record| !matches!(record.owner, TaskOwner::Host { .. })));
    assert!(saved_registry
        .tasks
        .iter()
        .all(|task| task.alias() != "embedded" && task.alias() != "ui:system/field"));
    let wire = serde_json::to_vec(&save).unwrap();
    let mut loaded = Engine::from_program(
        rvn_parser::parse_program(SOURCE).unwrap(),
        Host::default(),
        0,
    )
    .unwrap();
    assert!(loaded.renderer.batches.is_empty());
    loaded
        .load_data(serde_json::from_slice(&wire).unwrap())
        .unwrap();
    assert_eq!(loaded.renderer.batches.len(), 1);
    let published = &loaded.renderer.batches[0];
    assert_eq!(encoded(&published.state), encoded(&loaded.state));
    assert_eq!(published.state.motions.tracks["background"], story_clock);
    assert!(published.interfaces.is_empty() && published.videos.is_empty());
    assert_eq!(loaded.state.motions.tracks["background"], story_clock);
    assert!(loaded.state.ui.screens.is_empty() && loaded.state.videos.tracks.is_empty());
    validate_current_resource_state(&loaded.state).unwrap();
    assert_eq!(encoded(&state), before);
}

#[test]
fn portable_filter_keeps_parked_story_homonym_and_excludes_live_host_instance() {
    let (program, mut state) = fixture();
    state.ui.screens.push(screen("same", 0, false));
    state.ui.next_order = 1;
    ui_motion(&mut state, "same", false);
    let parent = state.scene_execution.as_ref().unwrap().owner().unwrap();
    call(&program, &mut state);
    host_screen(&mut state, "same");
    ui_motion(&mut state, "same", false);
    let parent_snapshot = state.scene_execution.as_ref().unwrap().frames[0]
        .parked
        .as_ref()
        .unwrap()
        .tasks
        .clone();
    let save = portable(&state);
    assert!(save.ui.screens.is_empty() && save.motions.tracks.is_empty());
    let execution = save.scene_execution.as_ref().unwrap();
    assert_eq!(
        execution.frames[0].parked.as_ref().unwrap().tasks,
        parent_snapshot
    );
    assert!(execution.resources.tasks.iter().any(|task| task.key.owner
        == TaskOwner::Scene { activation: parent }
        && task.alias() == "same"));
    let roundtrip = serde_json::from_slice::<SaveData>(&serde_json::to_vec(&save).unwrap())
        .unwrap()
        .into_game_state();
    validate_current_resource_state(&roundtrip).unwrap();
}

#[test]
fn cancelled_host_wait_receipt_is_portable_and_consumed_once_after_real_return() {
    let (program, mut state) = fixture();
    host_screen(&mut state, "system");
    let host_owner = TaskOwner::Host {
        role: rvn_ui::PageRole::Pause.id().into(),
    };
    let target = state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .target(&host_owner, TargetKind::Interface, "system")
        .unwrap()
        .clone();
    let key = resource_key(
        host_owner.clone(),
        ResourceTaskKind::Motion,
        "ui:system/field",
    );
    state
        .scene_execution
        .as_mut()
        .unwrap()
        .resources
        .register(key.clone(), target.clone(), ResourceTaskKind::Motion, None)
        .unwrap();
    ui_motion(&mut state, "system", true);
    let position = state
        .scene_execution
        .as_ref()
        .unwrap()
        .live_position
        .clone();
    call(&program, &mut state);
    remove_target(&mut state, &host_owner, TargetKind::Interface, "system").unwrap();
    let receipt = state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .cancelled_waits[0]
        .clone();
    assert_eq!(receipt.key, key);
    assert_eq!(receipt.position, position);
    let save = portable(&state);
    assert_eq!(
        save.scene_execution
            .as_ref()
            .unwrap()
            .resources
            .cancelled_waits,
        vec![receipt.clone()]
    );
    assert!(save.ui.screens.is_empty() && save.motions.tracks.is_empty());
    let mut restored = serde_json::from_slice::<SaveData>(&serde_json::to_vec(&save).unwrap())
        .unwrap()
        .into_game_state();
    abandon_current(&mut restored).unwrap();
    let execution = restored.scene_execution.as_mut().unwrap();
    assert_eq!(
        execution.peek_return_kind(false),
        Some(FrameKind::SceneCall)
    );
    let returned = execution.return_to_caller(false).unwrap().unwrap();
    restored.pc = returned.pc;
    execution.live_position = returned;
    restored.call_stack = execution.derived_call_stack();
    let transition = resume_current(&mut restored).unwrap();
    assert_eq!(transition.cancelled_waits, vec![receipt]);
    assert_eq!(restored.pc, position.pc);
    assert!(restored.motions.waiting.is_none());
    assert!(restored
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .cancelled_waits
        .is_empty());
    validate_current_resource_state(&restored).unwrap();
}

#[test]
fn legacy_infallible_and_checked_constructors_keep_identical_version9_wire_format() {
    let state = Engine::new(
        rvn_parser::parse("set n=7\n\"legacy\"").unwrap(),
        Host::default(),
        0,
    )
    .unwrap()
    .state;
    assert!(state.scene_execution.is_none());
    let mut old = SaveData::from_state(&state, 2, "old".into(), "legacy.rvn".into());
    let mut checked =
        SaveData::try_from_state(&state, 2, "old".into(), "legacy.rvn".into()).unwrap();
    old.timestamp = 0;
    checked.timestamp = 0;
    assert_eq!(checked.format_version, SAVE_FORMAT_VERSION);
    assert_eq!(
        serde_json::to_value(&checked).unwrap(),
        serde_json::to_value(&old).unwrap()
    );
    let wire = serde_json::to_value(&checked).unwrap();
    assert!(wire.get("scene_execution").is_none());
}
