use rvn_core::renderer::SceneRenderBatch;
use rvn_core::scene_resources::ResourceTaskKind;
use rvn_core::scene_tasks::TaskOwner;
use rvn_core::video::{Feedback, Playback, VideoView};
use rvn_core::{Engine, Interaction, Renderer, SpriteState};
use rvn_parser::{Hotspot, Position, Transition, Value};

#[derive(Default)]
struct Host {
    batches: Vec<SceneRenderBatch>,
    reject: bool,
}
impl Renderer for Host {
    fn supports_video(&self) -> bool {
        true
    }
    fn supports_programmable_ui(&self) -> bool {
        true
    }
    fn supports_composable_motion(&self) -> bool {
        true
    }
    fn validate_scene_batch(&self, _: &SceneRenderBatch) -> Result<(), String> {
        if self.reject {
            Err("Cannot prepare video projection".into())
        } else {
            Ok(())
        }
    }
    fn publish_scene_batch(&mut self, batch: SceneRenderBatch) {
        self.batches.push(batch);
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
fn game(source: &str) -> Engine<Host> {
    Engine::from_program(
        rvn_parser::parse_program(source).unwrap(),
        Host::default(),
        32,
    )
    .unwrap()
}
fn dialogue(game: &mut Engine<Host>) -> String {
    match game.step_until_interaction().unwrap().unwrap() {
        Interaction::Dialogue { text, .. } => text,
        other => panic!("Expected dialogue, got {other:?}"),
    }
}
fn view(game: &Engine<Host>, name: &str) -> VideoView {
    game.video_views()
        .unwrap()
        .into_iter()
        .find(|view| view.id == name)
        .unwrap()
}
fn json(game: &Engine<Host>) -> serde_json::Value {
    serde_json::to_value(&game.state).unwrap()
}
fn next(game: &mut Engine<Host>) -> String {
    game.advance_dialogue().unwrap();
    dialogue(game)
}

#[test]
fn scene_homonyms_reject_parent_callbacks_before_payload_and_resume_exact_position() {
    let mut game = game(
        r#"
init { set ends=0 }
handler done(event) { set ends=ends+1 }
scene label P
video.play("clip",video_clip("parent.webm",{"on_end":"done","skippable":true}))
"parent"
call C
"returned"
jump finish
scene label C
video.play("clip",video_clip("child.webm",{"on_end":"done","skippable":true}))
"child"
return
label finish
"#,
    );
    assert_eq!(dialogue(&mut game), "parent");
    let parent = view(&game, "clip");
    assert!(game
        .video_feedback_from_view(
            &parent,
            Feedback::Position {
                seconds: 0.75,
                duration: 2.0
            }
        )
        .unwrap());
    assert_eq!(next(&mut game), "child");
    let child = view(&game, "clip");
    assert_ne!(parent.resource_token, child.resource_token);
    let before = json(&game);
    assert!(!game
        .video_feedback_from_view(
            &parent,
            Feedback::Position {
                seconds: f64::NAN,
                duration: f64::NAN
            }
        )
        .unwrap());
    assert!(!game.skip_video_from_view(&parent).unwrap());
    assert!(!game.resume_video_from_view(&parent).unwrap());
    assert_eq!(json(&game), before);
    assert!(game.skip_video_from_view(&child).unwrap());
    assert!(!game
        .video_feedback_from_view(&child, Feedback::End)
        .unwrap());
    assert_eq!(game.state.vars["ends"], Value::Int(1));
    assert_eq!(next(&mut game), "returned");
    let resumed = view(&game, "clip");
    assert_eq!(resumed.track.clip.source, "parent.webm");
    assert_eq!(resumed.track.position, 0.75);
    assert_ne!(parent.resource_token, resumed.resource_token);
    assert!(!game
        .video_feedback_from_view(&child, Feedback::End)
        .unwrap());
    assert!(!game
        .video_feedback_from_view(&parent, Feedback::End)
        .unwrap());
}

#[test]
fn game_video_keeps_authority_through_call_and_its_handler_creates_game_resources() {
    let mut game = game(
        r#"
screen notice() { return {"id":"root","kind":"panel"} }
handler completed(event) {
 set result=value
 ui.open("notice",[],false,0)
 video.play("next",video_clip("next.webm",{}))
}
init {
 set value=2
 video.play("global",video_clip("game.webm",{"on_end":"completed"}))
}
scene label P
"parent"
call C
"returned"
jump finish
scene label C
local value=99
"child"
return
label finish
"#,
    );
    assert_eq!(dialogue(&mut game), "parent");
    let original = view(&game, "global");
    assert_eq!(
        original.resource_token.as_ref().unwrap().task.key.owner,
        TaskOwner::Game
    );
    assert_eq!(next(&mut game), "child");
    assert_eq!(
        view(&game, "global").resource_token,
        original.resource_token
    );
    assert!(game
        .video_feedback_from_view(&original, Feedback::End)
        .unwrap());
    assert_eq!(game.state.vars["result"], Value::Int(2));
    for (kind, name) in [
        (ResourceTaskKind::Interface, "notice"),
        (ResourceTaskKind::Video, "next"),
    ] {
        assert_eq!(
            game.scene_resource_token(kind, name)
                .unwrap()
                .task
                .key
                .owner,
            TaskOwner::Game
        );
    }
    assert_eq!(next(&mut game), "returned");
    assert!(game
        .state
        .ui
        .screens
        .iter()
        .any(|screen| screen.name == "notice"));
    assert!(game.state.videos.tracks.contains_key("next"));
}

#[test]
fn seek_pause_resume_revoke_workers_but_volume_keeps_the_same_operation() {
    let mut game = game(
        r#"
scene label P
video.play("clip",video_clip("clip.webm",{"skippable":true}))
"playing"
video.pause("clip")
"paused"
video.seek("clip",0.5)
"seeked"
video.volume("clip",0.4)
"volume"
video.resume("clip")
video.wait("clip")
"done"
"#,
    );
    assert_eq!(dialogue(&mut game), "playing");
    let playing = view(&game, "clip");
    game.video_feedback_from_view(
        &playing,
        Feedback::Position {
            seconds: 0.25,
            duration: 2.0,
        },
    )
    .unwrap();
    assert_eq!(next(&mut game), "paused");
    let paused = view(&game, "clip");
    assert_ne!(paused.resource_token, playing.resource_token);
    assert_eq!(paused.track.position, 0.25);
    let paused_state = json(&game);
    let paused_batches = game.renderer.batches.len();
    assert!(!game
        .video_feedback_from_view(&paused, Feedback::End)
        .unwrap());
    assert_eq!(json(&game), paused_state);
    assert_eq!(game.renderer.batches.len(), paused_batches);
    assert!(!game
        .video_feedback_from_view(&playing, Feedback::End)
        .unwrap());
    assert_eq!(next(&mut game), "seeked");
    let seeked = view(&game, "clip");
    assert_ne!(seeked.resource_token, paused.resource_token);
    assert_eq!(seeked.track.position, 0.5);
    assert_eq!(next(&mut game), "volume");
    assert_eq!(view(&game, "clip").resource_token, seeked.resource_token);
    game.advance_dialogue().unwrap();
    assert!(game.step_until_interaction().unwrap().is_none());
    let resumed = view(&game, "clip");
    assert_ne!(resumed.resource_token, seeked.resource_token);
    assert_eq!(game.state.videos.waiting.as_deref(), Some("clip"));
    assert_eq!(resumed.track.position, 0.5);
    assert!(!game.resume_video_from_view(&paused).unwrap());
    assert!(game
        .video_feedback_from_view(&resumed, Feedback::End)
        .unwrap());
    assert_eq!(dialogue(&mut game), "done");
}

#[test]
fn same_clip_replacement_has_a_new_incarnation_and_legacy_alias_cannot_bypass_it() {
    let mut game = game("scene label P\nvideo.play(\"clip\",video_clip(\"clip.webm\",{}))\n\"first\"\nvideo.play(\"clip\",video_clip(\"clip.webm\",{}))\n\"second\"\n");
    dialogue(&mut game);
    let old = view(&game, "clip");
    next(&mut game);
    let current = view(&game, "clip");
    assert_ne!(
        old.resource_token.as_ref().unwrap().target,
        current.resource_token.as_ref().unwrap().target
    );
    let before = json(&game);
    assert!(!game
        .video_feedback(old.epoch, "clip", Feedback::End)
        .unwrap());
    assert!(!game
        .video_feedback_from_view(&old, Feedback::Error(String::new()))
        .unwrap());
    assert_eq!(json(&game), before);
}

#[test]
fn blocked_cinematic_keeps_wait_and_resume_then_load_revoke_old_authority() {
    let mut game = game("scene label P\nvideo.play(\"clip\",video_clip(\"clip.webm\",{\"cinematic\":true}))\n\"done\"\n");
    assert!(game.step_until_interaction().unwrap().is_none());
    let original = view(&game, "clip");
    game.video_feedback_from_view(
        &original,
        Feedback::Position {
            seconds: 0.7,
            duration: 2.0,
        },
    )
    .unwrap();
    game.video_feedback_from_view(
        &original,
        Feedback::AutoplayBlocked("User gesture required".into()),
    )
    .unwrap();
    let blocked = view(&game, "clip");
    assert_eq!(blocked.track.playback, Playback::Blocked);
    assert!(game.state.videos.waiting.is_some());
    let blocked_state = json(&game);
    let blocked_batches = game.renderer.batches.len();
    assert!(!game
        .video_feedback_from_view(&blocked, Feedback::End)
        .unwrap());
    assert_eq!(json(&game), blocked_state);
    assert_eq!(game.renderer.batches.len(), blocked_batches);
    assert!(game.resume_video_from_view(&blocked).unwrap());
    let resumed = view(&game, "clip");
    assert_ne!(resumed.resource_token, blocked.resource_token);
    assert_eq!(resumed.track.position, 0.7);
    assert!(game.state.videos.waiting.is_some());
    let saved =
        rvn_core::save::SaveData::try_from_state(&game.state, 1, "Video".into(), "main.rvn".into())
            .unwrap();
    game.load_data(saved).unwrap();
    let loaded = view(&game, "clip");
    assert_ne!(loaded.resource_token, resumed.resource_token);
    assert_eq!(loaded.track.position, 0.7);
    assert!(!game
        .video_feedback_from_view(&resumed, Feedback::End)
        .unwrap());
    assert!(game
        .video_feedback_from_view(&loaded, Feedback::End)
        .unwrap());
    assert_eq!(dialogue(&mut game), "done");
}

#[test]
fn terminal_handler_and_projection_rejection_leave_the_entire_candidate_unpublished() {
    let mut game = game("init { set ends=0 }\nhandler done(event) { set ends=ends+1 set roll=random(1,100) }\nscene label P\nvideo.play(\"clip\",video_clip(\"clip.webm\",{\"on_end\":\"done\"}))\nvideo.wait(\"clip\")\n\"done\"\n");
    assert!(game.step_until_interaction().unwrap().is_none());
    let current = view(&game, "clip");
    let before = json(&game);
    let batches = game.renderer.batches.len();
    game.renderer.reject = true;
    assert!(game
        .video_feedback_from_view(&current, Feedback::End)
        .is_err());
    assert_eq!(json(&game), before);
    assert_eq!(game.renderer.batches.len(), batches);
    game.renderer.reject = false;
    assert!(game
        .video_feedback_from_view(&current, Feedback::End)
        .unwrap());
    assert_eq!(game.state.vars["ends"], Value::Int(1));
    let completed = json(&game);
    assert!(!game
        .video_feedback_from_view(&current, Feedback::End)
        .unwrap());
    assert_eq!(json(&game), completed);
}

#[test]
fn completing_a_game_video_in_the_child_releases_the_parked_parent_wait() {
    let mut game = game(
        r#"
init { video.play("global",video_clip("game.webm",{})) }
scene label P
timer 1 => call C
video.wait("global")
"parent resumed"
jump finish
scene label C
"child"
return
label finish
"#,
    );
    assert!(game.step_until_interaction().unwrap().is_none());
    let original = view(&game, "global");
    assert!(game.tick_scene_tasks(1.0).unwrap());
    assert_eq!(dialogue(&mut game), "child");
    assert_eq!(
        view(&game, "global").resource_token,
        original.resource_token
    );
    assert!(game
        .video_feedback_from_view(&original, Feedback::End)
        .unwrap());
    assert!(game
        .state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .waits
        .is_empty());
    assert_eq!(next(&mut game), "parent resumed");
    assert!(game.state.videos.waiting.is_none());
}

const UI_CINEMATIC: &str = r#"
init { set init_marker=1 set launches=0 set ends=0 set opens=0 }
screen controls() { return component("play","button",{"text":"Play","events":{"click":"launch","open":"opened"}},[]) }
handler opened(event) { set opens=opens+1 }
handler launch(event) {
 set init_marker=77
 set launches=launches+1
 set launch_roll=random(1,100)
 video.play("clip",video_clip("clip.webm",{"cinematic":true,"on_end":"done"}))
}
handler done(event) { set ends=ends+1 }
scene label P
ui.open("controls",[],false,0)
"before"
"after"
jump finish
label finish
"#;

fn launch_cinematic(game: &mut Engine<Host>) {
    assert_eq!(dialogue(game), "before");
    game.interface_event(rvn_core::ui::UiInput {
        screen: "controls".into(),
        element: "play".into(),
        kind: rvn_ui::programmable::ScreenEventKind::Click,
        value: None,
        key: None,
    })
    .unwrap();
    let current = view(game, "clip");
    assert!(game
        .video_feedback_from_view(
            &current,
            Feedback::Position {
                seconds: 0.7,
                duration: 2.0,
            }
        )
        .unwrap());
    assert!(game.current_interaction().unwrap().is_none());
}

#[test]
fn ui_created_cinematic_restores_its_dialogue_continuation_without_replaying_events() {
    let mut original = game(UI_CINEMATIC);
    launch_cinematic(&mut original);
    let old = view(&original, "clip");
    let pc = original.state.pc;
    assert!(matches!(
        original.script[pc],
        rvn_parser::Statement::Dialogue { .. }
    ));
    let execution = original.state.scene_execution.as_ref().unwrap();
    assert!(execution
        .resources
        .waits
        .iter()
        .any(|wait| wait.position == execution.live_position));
    let saved = rvn_core::save::SaveData::try_from_state(
        &original.state,
        1,
        "UI cinematic".into(),
        "main.rvn".into(),
    )
    .unwrap();
    let wire = serde_json::to_vec(&saved).unwrap();
    let mut restored = original.fresh(Host::default(), 32).unwrap();
    let batches = restored.renderer.batches.len();
    restored
        .load_data(serde_json::from_slice(&wire).unwrap())
        .unwrap();
    assert_eq!(restored.renderer.batches.len(), batches + 1);
    assert_eq!(restored.state.pc, pc);
    assert_eq!(restored.state.random, original.state.random);
    assert_eq!(
        serde_json::to_value(&restored.state.scene_execution).unwrap(),
        serde_json::to_value(&original.state.scene_execution).unwrap()
    );
    assert_eq!(restored.state.ui, original.state.ui);
    assert_eq!(restored.state.videos, original.state.videos);
    for (name, value) in [
        ("init_marker", 77),
        ("launches", 1),
        ("opens", 1),
        ("ends", 0),
    ] {
        assert_eq!(restored.state.vars[name], Value::Int(value));
    }
    let loaded = view(&restored, "clip");
    assert_ne!(old.resource_token, loaded.resource_token);
    let before = json(&restored);
    assert!(!restored
        .video_feedback_from_view(
            &old,
            Feedback::Position {
                seconds: f64::NAN,
                duration: f64::NAN,
            }
        )
        .unwrap());
    assert_eq!(json(&restored), before);
    assert!(restored
        .video_feedback_from_view(&loaded, Feedback::End)
        .unwrap());
    assert_eq!(restored.state.vars["ends"], Value::Int(1));
    assert_eq!(next(&mut restored), "after");
}

#[test]
fn valid_but_different_cinematic_continuation_is_rejected_without_mutation() {
    let mut game = game(UI_CINEMATIC);
    launch_cinematic(&mut game);
    let mut saved = rvn_core::save::SaveData::try_from_state(
        &game.state,
        1,
        "UI cinematic".into(),
        "main.rvn".into(),
    )
    .unwrap();
    let original_wire = serde_json::to_vec(&saved).unwrap();
    let prepared = rvn_core::program::PreparedNarrative::from_parsed(
        rvn_parser::parse_program(UI_CINEMATIC).unwrap(),
    )
    .unwrap();
    let different_pc = saved.pc + 1;
    assert!(matches!(
        prepared.script[different_pc],
        rvn_parser::Statement::Dialogue { .. }
    ));
    let different = prepared.position(different_pc);
    prepared.validate_position(&different).unwrap();
    let execution = saved.scene_execution.as_mut().unwrap();
    for wait in &execution.resources.waits {
        prepared.validate_position(&wait.position).unwrap();
        assert_ne!(wait.position, different);
    }
    execution.live_position = different;
    saved.pc = different_pc;
    let before = json(&game);
    let generations = (game.runtime_epoch(), game.interface_epoch());
    let videos = game.video_views().unwrap();
    let history = serde_json::to_value(
        game.history
            .entries()
            .iter()
            .map(|entry| (&entry.state, &entry.display))
            .collect::<Vec<_>>(),
    )
    .unwrap();
    let batches = game.renderer.batches.len();
    let persistent = game.persistent_vars.clone();
    let error = game.load_data(saved).unwrap_err();
    assert!(
        matches!(error, rvn_core::save::SaveError::Incompatible(ref reason)
        if reason.contains("cinematic continuation")),
        "{error}"
    );
    assert_eq!(json(&game), before);
    assert_eq!((game.runtime_epoch(), game.interface_epoch()), generations);
    assert_eq!(game.video_views().unwrap(), videos);
    assert_eq!(game.renderer.batches.len(), batches);
    assert_eq!(game.persistent_vars, persistent);
    assert_eq!(
        serde_json::to_value(
            game.history
                .entries()
                .iter()
                .map(|entry| (&entry.state, &entry.display))
                .collect::<Vec<_>>()
        )
        .unwrap(),
        history
    );
    let valid = rvn_core::save::SaveData::try_from_state(
        &game.state,
        1,
        "UI cinematic".into(),
        "main.rvn".into(),
    )
    .unwrap();
    // Timestamps are not gameplay state; the original saved bytes remain usable.
    let mut valid_wire = valid;
    valid_wire.timestamp = serde_json::from_slice::<rvn_core::save::SaveData>(&original_wire)
        .unwrap()
        .timestamp;
    assert_eq!(
        serde_json::to_value(&valid_wire).unwrap(),
        serde_json::from_slice::<serde_json::Value>(&original_wire).unwrap()
    );
}

#[test]
fn paused_current_video_can_be_explicitly_skipped_once() {
    let mut game = game("init { set ends=0 }\nhandler done(event) { set ends=ends+1 }\nscene label P\nvideo.play(\"clip\",video_clip(\"clip.webm\",{\"skippable\":true,\"on_end\":\"done\"}))\nvideo.pause(\"clip\")\n\"paused\"\n");
    assert_eq!(dialogue(&mut game), "paused");
    let paused = view(&game, "clip");
    assert_eq!(paused.track.playback, Playback::Paused);
    assert!(!game
        .video_feedback_from_view(&paused, Feedback::End)
        .unwrap());
    assert!(game.skip_video_from_view(&paused).unwrap());
    assert_eq!(game.state.vars["ends"], Value::Int(1));
    let before = json(&game);
    assert!(!game.skip_video_from_view(&paused).unwrap());
    assert_eq!(json(&game), before);
}
