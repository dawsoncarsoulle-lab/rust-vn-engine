//! Public narrative frame contracts: clocks first, callbacks next, one commit.
//! These fixtures retain the legacy interface_tick contract in its own suite.

use rvn_core::renderer::SceneRenderBatch;
use rvn_core::scene_resources::{
    EffectClock, EffectParameters, EffectProjectionBase, ResourceTaskKind, SavedEffect,
    SpriteEffectKind, TargetKind, TransformSnapshot,
};
use rvn_core::scene_tasks::TaskOwner;
use rvn_core::video::{Feedback, Playback};
use rvn_core::{
    Engine, GameFrameReports, GameVideoFeedback, GameVideoReport, Interaction, Renderer,
    SpriteState,
};
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
    fn supports_custom_canvas(&self) -> bool {
        true
    }
    fn supports_composable_motion(&self) -> bool {
        true
    }
    fn validate_scene_batch(&self, _: &SceneRenderBatch) -> Result<(), String> {
        if self.reject {
            Err("Rejected complete frame".into())
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

const CANVASES: &str = r#"
function paint(state,props,frame) {return [canvas_rect([0,0,10,10],[1,0,0,1],radius)]}
function widget(id) {return component(id,"canvas",{"draw":"paint","state":{"ticks":0},"events":{"tick":"advance"}},[])}
screen custom() {return component("root","column",{},[widget("one"),widget("two")])}
screen overlay() {return component("overlay","text",{"text":"Modal"},[])}
handler advance(event) {set calls=calls+1}
init {set calls=0 set radius=0}
scene label P
ui.open("custom",[],false,0)
"parent"
jump finish
scene label C
"child"
return
label finish
"#;

fn game(source: &str) -> Engine<Host> {
    Engine::from_program(
        rvn_parser::parse_program(source).unwrap(),
        Host::default(),
        32,
    )
    .unwrap()
}
fn prepared(source: &str) -> Engine<Host> {
    let mut game = game(source);
    assert!(matches!(
        game.step_until_interaction().unwrap(),
        Some(Interaction::Dialogue { .. })
    ));
    game
}
fn state(game: &Engine<Host>) -> serde_json::Value {
    serde_json::to_value(&game.state).unwrap()
}
fn canvas<'a>(
    game: &'a Engine<Host>,
    screen: &str,
    element: &str,
) -> &'a rvn_core::ui::CanvasState {
    &game
        .state
        .ui
        .screens
        .iter()
        .find(|instance| instance.name == screen)
        .unwrap()
        .canvas_states[element]
}
fn video(game: &Engine<Host>, alias: &str, feedback: GameVideoFeedback) -> GameVideoReport {
    GameVideoReport {
        token: game
            .video_views()
            .unwrap()
            .into_iter()
            .find(|view| view.id == alias)
            .unwrap()
            .resource_token
            .unwrap(),
        feedback,
    }
}
fn history(game: &Engine<Host>) -> serde_json::Value {
    serde_json::to_value(
        game.history
            .entries()
            .iter()
            .map(|entry| (&entry.state, &entry.display))
            .collect::<Vec<_>>(),
    )
    .unwrap()
}
fn effect(
    game: &mut Engine<Host>,
    owner: TaskOwner,
    alias: &str,
) -> rvn_core::scene_resources::ResourceToken {
    let target = game
        .state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .target(&TaskOwner::Game, TargetKind::Background, "background")
        .unwrap()
        .clone();
    game.register_scene_effect(
        rvn_core::scene_resources::resource_key(owner, ResourceTaskKind::Effect, alias),
        target,
        SavedEffect {
            presentation: None,
            base_resolved: true,
            clock: EffectClock {
                duration: 5.0,
                elapsed: 0.0,
                looping: false,
            },
            parameters: EffectParameters::VisualFade {
                from: 0.0,
                to: 1.0,
                despawn_on_finish: false,
                transition: rvn_core::scene_resources::VisualTransition::Fade,
                base: TransformSnapshot::default(),
                layered: false,
            },
        },
    )
    .unwrap()
}
fn elapsed(game: &Engine<Host>, token: &rvn_core::scene_resources::ResourceToken) -> f64 {
    game.state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .tasks
        .iter()
        .find(|task| task.key == token.task.key)
        .unwrap()
        .effect
        .as_ref()
        .unwrap()
        .clock
        .elapsed
}

#[test]
fn frame_advances_both_canvas_clocks_before_first_handler_opens_modal() {
    let source = CANVASES.replace(
        "handler advance(event) {set calls=calls+1}",
        "handler advance(event) {set calls=calls+1 ui.open(\"overlay\",[],true,10)}",
    );
    let mut game = prepared(&source);
    let batches = game.renderer.batches.len();
    let old_history = history(&game);
    let result = game
        .tick_game_frame(0.1, GameFrameReports::default())
        .unwrap();
    assert_eq!(canvas(&game, "custom", "one").elapsed, 0.1);
    assert_eq!(canvas(&game, "custom", "two").elapsed, 0.1);
    assert_eq!(game.state.vars["calls"], Value::Int(1));
    assert_eq!(result.callbacks, 1);
    assert!(result.published);
    assert_eq!(game.renderer.batches.len(), batches + 1);
    assert_eq!(history(&game), old_history);
}

#[test]
fn frame_does_not_deliver_old_ticks_to_reopened_homonymous_canvas_instances() {
    let source=CANVASES.replace("handler advance(event) {set calls=calls+1}",
        "handler advance(event) {set calls=calls+1 ui.close(\"custom\") ui.open(\"custom\",[],false,0)}");
    let mut game = prepared(&source);
    let old = game
        .scene_resource_token(ResourceTaskKind::Canvas, "custom/two")
        .unwrap();
    let old_order = game.state.ui.screens[0].order;
    game.tick_game_frame(0.1, GameFrameReports::default())
        .unwrap();
    assert_eq!(game.state.vars["calls"], Value::Int(1));
    assert_eq!(canvas(&game, "custom", "one").elapsed, 0.0);
    assert_eq!(canvas(&game, "custom", "two").elapsed, 0.0);
    assert_ne!(game.state.ui.screens[0].order, old_order);
    assert_ne!(
        game.scene_resource_token(ResourceTaskKind::Canvas, "custom/two")
            .unwrap()
            .target,
        old.target
    );
}

#[test]
fn frame_uses_structural_preflight_until_the_final_valid_canvas_drawing() {
    let source=CANVASES.replace("handler advance(event) {set calls=calls+1}",
        "handler advance(event) {set calls=calls+1 if event[\"element\"]==\"one\" {set radius=-1 video.play(\"embedded\",video_clip(\"clip.webm\",{\"target\":\"ui:custom/one\"}))} else {set radius=1}}");
    let mut game = prepared(&source);
    let batches = game.renderer.batches.len();
    game.tick_game_frame(0.1, GameFrameReports::default())
        .unwrap();
    assert_eq!(game.state.vars["radius"], Value::Int(1));
    assert_eq!(game.state.vars["calls"], Value::Int(2));
    assert!(game.state.videos.tracks.contains_key("embedded"));
    assert_eq!(game.renderer.batches.len(), batches + 1);
    assert_eq!(
        game.renderer.batches.last().unwrap().interfaces,
        game.interface_views().unwrap()
    );
}

#[test]
fn frame_game_timer_parks_a_due_scene_timer_after_all_parent_clocks_advance() {
    let source = CANVASES
        .replace("scene label P", "scene \"parent.png\"\nscene label P")
        .replace("\"parent\"", "timer 5 => jump done\n\"parent\"")
        .replace("label finish", "label done\n\"due parent\"\nlabel finish");
    let mut game = prepared(&source);
    game.state
        .scene_execution
        .as_mut()
        .unwrap()
        .game_tasks
        .set_timer(5.0, "call C".into())
        .unwrap();
    let parent = game
        .state
        .scene_execution
        .as_ref()
        .unwrap()
        .owner()
        .unwrap();
    let old_scene_token = game.timer_token().unwrap();
    let parent_effect = effect(&mut game, TaskOwner::Scene { activation: parent }, "fade");
    game.tick_game_frame(5.0, GameFrameReports::default())
        .unwrap();
    let execution = game.state.scene_execution.as_ref().unwrap();
    assert_ne!(execution.owner(), Some(parent));
    let parked = execution.frames.last().unwrap().parked.as_ref().unwrap();
    assert_eq!(parked.tasks.timer.as_ref().unwrap().elapsed, 5.0);
    assert_eq!(
        parked.tasks.suspended_ui.screens[0].canvas_states["one"].elapsed,
        0.25
    );
    assert_eq!(
        parked.tasks.suspended_ui.screens[0].canvas_states["two"].elapsed,
        0.25
    );
    assert!(execution.active.as_ref().unwrap().tasks.timer.is_none());
    assert!(!game.fire_timer(&old_scene_token).unwrap());
    assert_eq!(elapsed(&game, &parent_effect), 5.0);
    assert_eq!(
        game.step_until_interaction().unwrap(),
        Some(Interaction::Dialogue {
            character: None,
            text: "child".into()
        })
    );
    game.advance_dialogue().unwrap();
    game.step_until_interaction().unwrap();
    assert_eq!(
        game.state.scene_execution.as_ref().unwrap().owner(),
        Some(parent)
    );
    assert_eq!(elapsed(&game, &parent_effect), 5.0);
    // A zero-delta frame settles the exact due effect after Return, without
    // advancing any restored clocks or relying on an ECS completion callback.
    game.tick_game_frame(0.0, GameFrameReports::default())
        .unwrap();
    assert!(!game
        .state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .tasks
        .iter()
        .any(|task| task.key == parent_effect.task.key));
}

#[test]
fn frame_game_motion_completion_releases_two_parked_waiters_without_replaying_pc() {
    let source="scene \"parent.png\"\nmotion.play(\"background\",motion_pause(1))\nscene label P\nmotion.wait(\"background\")\n\"P resumed\"\njump finish\nscene label C\nmotion.wait(\"background\")\n\"C resumed\"\nreturn\nscene label D\n\"D\"\nreturn\nlabel finish\n";
    let mut game = game(source);
    assert!(game.step_until_interaction().unwrap().is_none());
    game.execute_timer_action("call C").unwrap();
    assert!(game.step_until_interaction().unwrap().is_none());
    game.execute_timer_action("call D").unwrap();
    assert!(game.step_until_interaction().unwrap().is_some());
    assert_eq!(
        game.state
            .scene_execution
            .as_ref()
            .unwrap()
            .resources
            .waits
            .len(),
        2
    );
    let pc = game.state.pc;
    let previous_history = history(&game);
    let batches = game.renderer.batches.len();
    game.tick_game_frame(1.0, GameFrameReports::default())
        .unwrap();
    assert_eq!(game.state.pc, pc);
    assert_eq!(history(&game), previous_history);
    assert!(game
        .state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .waits
        .is_empty());
    assert_eq!(game.state.motions.tracks["background"].elapsed, 1.0);
    assert!(!game.state.motions.tracks["background"].running);
    for frame in &game.state.scene_execution.as_ref().unwrap().frames {
        assert!(frame
            .parked
            .as_ref()
            .unwrap()
            .tasks
            .suspended_motions
            .waiting
            .is_none());
    }
    assert_eq!(game.renderer.batches.len(), batches + 1);
}

#[test]
fn frame_validates_all_positions_before_a_terminal_handler_removes_the_second_player() {
    let source="init {set ends=0}\nhandler done(event) {set ends=ends+1 set roll=random(1,100) video.stop(\"b\")}\nscene label P\nvideo.play(\"a\",video_clip(\"a.webm\",{\"on_end\":\"done\"}))\nvideo.play(\"b\",video_clip(\"b.webm\",{}))\n\"P\"\n";
    let mut game = prepared(source);
    let before = state(&game);
    let batches = game.renderer.batches.len();
    let end = video(&game, "a", GameVideoFeedback::Report(Feedback::End));
    let bad = video(
        &game,
        "b",
        GameVideoFeedback::Report(Feedback::Position {
            seconds: f64::NAN,
            duration: 2.0,
        }),
    );
    assert!(game
        .tick_game_frame(
            0.1,
            GameFrameReports {
                videos: vec![end.clone(), bad]
            }
        )
        .is_err());
    assert_eq!(state(&game), before);
    assert_eq!(game.renderer.batches.len(), batches);
    let position_a = video(
        &game,
        "a",
        GameVideoFeedback::Report(Feedback::Position {
            seconds: 0.4,
            duration: 2.0,
        }),
    );
    let position_b = video(
        &game,
        "b",
        GameVideoFeedback::Report(Feedback::Position {
            seconds: 0.7,
            duration: 2.0,
        }),
    );
    let outcome = game
        .tick_game_frame(
            0.1,
            GameFrameReports {
                videos: vec![end, position_b, position_a],
            },
        )
        .unwrap();
    assert_eq!(outcome.accepted_reports, 3);
    assert_eq!(game.state.vars["ends"], Value::Int(1));
    assert!(!game.state.videos.tracks.contains_key("b"));
    assert_eq!(game.state.videos.tracks["a"].position, 2.0);
    assert_eq!(game.renderer.batches.len(), batches + 1);
}

#[test]
fn frame_resume_renews_authority_and_ignores_stale_payload_and_old_terminal() {
    let mut game =
        prepared("scene label P\nvideo.play(\"clip\",video_clip(\"clip.webm\",{}))\n\"P\"\n");
    let old = video(
        &game,
        "clip",
        GameVideoFeedback::Report(Feedback::Position {
            seconds: f64::NAN,
            duration: f64::NAN,
        }),
    );
    let view = game.video_views().unwrap().pop().unwrap();
    game.video_feedback_from_view(&view, Feedback::AutoplayBlocked("Gesture needed".into()))
        .unwrap();
    // Load revokes old; the re-opened blocked task retains its saved position.
    let save = rvn_core::save::SaveData::try_from_state(
        &game.state,
        1,
        "Blocked".into(),
        "story.rvn".into(),
    )
    .unwrap();
    game.load_data(save).unwrap();
    let resumed = video(&game, "clip", GameVideoFeedback::Resume);
    let old_end = GameVideoReport {
        token: resumed.token.clone(),
        feedback: GameVideoFeedback::Report(Feedback::End),
    };
    let before = game.state.videos.tracks["clip"].position;
    let outcome = game
        .tick_game_frame(
            0.0,
            GameFrameReports {
                videos: vec![old, resumed.clone(), old_end],
            },
        )
        .unwrap();
    assert_eq!(outcome.accepted_reports, 1);
    assert_eq!(outcome.ignored_reports, 2);
    assert_eq!(game.state.videos.tracks["clip"].playback, Playback::Playing);
    assert_eq!(game.state.videos.tracks["clip"].position, before);
    assert_ne!(
        game.video_views().unwrap()[0]
            .resource_token
            .as_ref()
            .unwrap(),
        &resumed.token
    );
}

#[test]
fn frame_core_effect_clock_survives_call_load_and_old_projection_token_is_neutral() {
    let mut game = prepared("scene \"parent.png\"\nscene label P\neileen.show(\"smile\") at left\n\"P\"\nscene label C\n\"C\"\nreturn\n");
    let token = effect(&mut game, TaskOwner::Game, "fade");
    let activation = game
        .state
        .scene_execution
        .as_ref()
        .unwrap()
        .owner()
        .unwrap();
    let target = game
        .state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .target(&TaskOwner::Game, TargetKind::Sprite, "eileen")
        .unwrap()
        .clone();
    let scene_token = game
        .register_scene_effect(
            rvn_core::scene_resources::resource_key(
                TaskOwner::Scene { activation },
                ResourceTaskKind::Effect,
                "bounce",
            ),
            target,
            SavedEffect {
                presentation: None,
                base_resolved: true,
                clock: EffectClock {
                    duration: 5.0,
                    elapsed: 0.0,
                    looping: false,
                },
                parameters: EffectParameters::SpriteAnimation {
                    animation: SpriteEffectKind::Bounce { height: 10.0 },
                    base: TransformSnapshot::default(),
                },
            },
        )
        .unwrap();
    game.tick_game_frame(0.1, GameFrameReports::default())
        .unwrap();
    assert_eq!(elapsed(&game, &token), 0.1);
    assert_eq!(elapsed(&game, &scene_token), 0.1);
    game.execute_timer_action("call C").unwrap();
    game.step_until_interaction().unwrap();
    game.tick_game_frame(0.1, GameFrameReports::default())
        .unwrap();
    assert!((elapsed(&game, &token) - 0.2).abs() < 1e-12);
    assert_eq!(elapsed(&game, &scene_token), 0.1);
    let save = rvn_core::save::SaveData::try_from_state(
        &game.state,
        1,
        "Effect".into(),
        "story.rvn".into(),
    )
    .unwrap();
    game.load_data(save).unwrap();
    assert!((elapsed(&game, &token) - 0.2).abs() < 1e-12);
    assert_eq!(elapsed(&game, &scene_token), 0.1);
    let before = state(&game);
    let before_bytes = serde_json::to_vec(&game.state).unwrap();
    let invalid_base = TransformSnapshot {
        translation: [f32::NAN, 0.0, 0.0],
        ..TransformSnapshot::default()
    };
    assert!(!game
        .capture_scene_effect_base(&token, EffectProjectionBase::VisualTransform(invalid_base))
        .unwrap());
    assert!(!game.complete_scene_effect(&token).unwrap());
    assert!(!game.complete_scene_effect(&scene_token).unwrap());
    assert_eq!(state(&game), before);
    assert_eq!(serde_json::to_vec(&game.state).unwrap(), before_bytes);
    let current = game
        .scene_resource_token(ResourceTaskKind::Effect, "fade")
        .unwrap();
    let previous_effect = game
        .state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .tasks
        .iter()
        .find(|task| task.key == current.task.key)
        .unwrap()
        .effect
        .as_ref()
        .unwrap()
        .clone();
    let base = TransformSnapshot {
        translation: [12.0, 23.0, 0.0],
        ..TransformSnapshot::default()
    };
    assert!(game
        .capture_scene_effect_base(
            &current,
            EffectProjectionBase::VisualTransform(base.clone())
        )
        .unwrap());
    let mut expected = previous_effect;
    if let EffectParameters::VisualFade { base: original, .. } = &mut expected.parameters {
        *original = base;
    } else {
        panic!("Expected the authored VisualFade fixture");
    }
    let actual = game
        .state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .tasks
        .iter()
        .find(|task| task.key == current.task.key)
        .unwrap()
        .effect
        .as_ref()
        .unwrap();
    assert_eq!(actual, &expected);
    let before = serde_json::to_vec(&game.state).unwrap();
    let mut illegal_clock = expected;
    illegal_clock.clock.elapsed = 0.7;
    assert!(game.capture_scene_effect(&current, illegal_clock).is_err());
    assert_eq!(serde_json::to_vec(&game.state).unwrap(), before);
    game.tick_game_frame(0.1, GameFrameReports::default())
        .unwrap();
    assert!((elapsed(&game, &token) - 0.3).abs() < 1e-12);
    assert_eq!(elapsed(&game, &scene_token), 0.1);
}

#[test]
fn frame_rejection_preserves_all_clocks_rng_history_and_retry_advances_once() {
    let source = CANVASES
        .replace(
            "handler advance(event) {set calls=calls+1}",
            "handler advance(event) {set calls=calls+1 set roll=random(1,100)}",
        )
        .replace("scene label P", "scene \"parent.png\"\nscene label P")
        .replace(
            "\"parent\"",
            "eileen.show(\"smile\") at left\nmotion.play(\"background\",motion_pause(5))\nmotion.play(\"sprite:eileen\",motion_pause(5))\ntimer 5 => jump finish\n\"parent\"",
        );
    let mut game = prepared(&source);
    let token = effect(&mut game, TaskOwner::Game, "fade");
    let before = state(&game);
    let before_bytes = serde_json::to_vec(&game.state).unwrap();
    let old_history = history(&game);
    let batches = game.renderer.batches.len();
    let epochs = (game.runtime_epoch(), game.interface_epoch());
    game.renderer.reject = true;
    assert!(game
        .tick_game_frame(0.1, GameFrameReports::default())
        .is_err());
    assert_eq!(state(&game), before);
    assert_eq!(serde_json::to_vec(&game.state).unwrap(), before_bytes);
    assert_eq!(history(&game), old_history);
    assert_eq!(game.renderer.batches.len(), batches);
    assert_eq!((game.runtime_epoch(), game.interface_epoch()), epochs);
    game.renderer.reject = false;
    game.tick_game_frame(0.1, GameFrameReports::default())
        .unwrap();
    assert_eq!(canvas(&game, "custom", "one").elapsed, 0.1);
    assert_eq!(canvas(&game, "custom", "two").elapsed, 0.1);
    assert_eq!(game.state.motions.tracks["background"].elapsed, 0.1);
    // Registry target alias is "eileen"; the live descriptor is keyed by the
    // actual authored Motion alias "sprite:eileen".
    assert_eq!(game.state.motions.tracks["sprite:eileen"].elapsed, 0.1);
    assert_eq!(
        game.state
            .scene_execution
            .as_ref()
            .unwrap()
            .active
            .as_ref()
            .unwrap()
            .tasks
            .timer
            .as_ref()
            .unwrap()
            .elapsed,
        0.1
    );
    assert_eq!(elapsed(&game, &token), 0.1);
    assert_eq!(game.state.vars["calls"], Value::Int(2));
    assert_eq!(game.renderer.batches.len(), batches + 1);
    assert_eq!(history(&game), old_history);
}

#[test]
fn frame_host_clock_does_not_advance_portable_story_and_game_delta_bounds_are_explicit() {
    let source=CANVASES.replace("screen overlay()", "screen host(ctx) {return component(\"host_canvas\",\"canvas\",{\"draw\":\"paint\",\"state\":{\"ticks\":0}},[])}\nscreen overlay()")
        .replace("scene label P", "scene \"parent.png\"\nscene label P")
        .replace("\"parent\"", "motion.play(\"background\",motion_pause(5))\ntimer 5 => jump finish\n\"parent\"");
    let mut game = prepared(&source);
    for delta in [-0.1, 3600.001, f64::NAN, f64::INFINITY] {
        let before = state(&game);
        let batches = game.renderer.batches.len();
        assert!(game
            .tick_game_frame(delta, GameFrameReports::default())
            .is_err());
        assert_eq!(state(&game), before);
        assert_eq!(game.renderer.batches.len(), batches);
    }
    let context = rvn_core::ui::value_from_json(&rvn_ui::source_menus::preview_context(
        rvn_ui::PageRole::Pause,
        true,
    ))
    .unwrap();
    game.synchronize_source_menu(rvn_ui::PageRole::Pause, Some("host"), context, true, 500)
        .unwrap();
    game.interface_tick_host(0.1).unwrap();
    assert_eq!(canvas(&game, "host", "host_canvas").elapsed, 0.1);
    assert_eq!(canvas(&game, "custom", "one").elapsed, 0.0);
    assert_eq!(canvas(&game, "custom", "two").elapsed, 0.0);
    assert_eq!(game.state.motions.tracks["background"].elapsed, 0.0);
    assert_eq!(
        game.state
            .scene_execution
            .as_ref()
            .unwrap()
            .active
            .as_ref()
            .unwrap()
            .tasks
            .timer
            .as_ref()
            .unwrap()
            .elapsed,
        0.0
    );
    let save =
        rvn_core::save::SaveData::try_from_state(&game.state, 1, "Host".into(), "story.rvn".into())
            .unwrap();
    assert!(save.ui.screens.iter().all(|screen| screen.name != "host"));
    game.synchronize_source_menu(
        rvn_ui::PageRole::Pause,
        None,
        Value::Dict(Default::default()),
        false,
        0,
    )
    .unwrap();
    game.tick_game_frame(1.0, GameFrameReports::default())
        .unwrap();
    assert_eq!(canvas(&game, "custom", "one").elapsed, 0.25);
    assert_eq!(canvas(&game, "custom", "two").elapsed, 0.25);
    assert_eq!(game.state.motions.tracks["background"].elapsed, 1.0);
    assert_eq!(
        game.state
            .scene_execution
            .as_ref()
            .unwrap()
            .active
            .as_ref()
            .unwrap()
            .tasks
            .timer
            .as_ref()
            .unwrap()
            .elapsed,
        1.0
    );
}

#[test]
fn frame_media_and_canvas_handlers_share_one_aggregate_computation_budget() {
    let source = r#"
function paint(state,props,frame) {return [canvas_rect([0,0,10,10],[1,0,0,1],0)]}
screen panel() {return component("canvas","canvas",{"draw":"paint","state":{"ticks":0},"events":{"tick":"canvas_work"}},[])}
handler canvas_work(event) {local i=0 while i<7500 {set i=i+1} set canvas_calls=canvas_calls+1}
handler media_work(event) {local i=0 while i<7500 {set i=i+1} set media_calls=media_calls+1 set roll=random(1,100)}
init {set canvas_calls=0 set media_calls=0}
scene label P
ui.open("panel",[],false,0)
video.play("clip",video_clip("clip.webm",{"on_end":"media_work"}))
"P"
"#;
    // Each handler is independently legal (~60k operations). Rejecting the
    // combined frame must exercise the shared budget, not a broken fixture.
    let mut canvas_only = prepared(source);
    canvas_only
        .tick_game_frame(0.1, GameFrameReports::default())
        .unwrap();
    assert_eq!(canvas_only.state.vars["canvas_calls"], Value::Int(1));
    assert_eq!(canvas_only.state.vars["media_calls"], Value::Int(0));
    let mut media_only = prepared(source);
    let media_end = video(
        &media_only,
        "clip",
        GameVideoFeedback::Report(Feedback::End),
    );
    media_only
        .tick_game_frame(
            0.0,
            GameFrameReports {
                videos: vec![media_end],
            },
        )
        .unwrap();
    assert_eq!(media_only.state.vars["canvas_calls"], Value::Int(0));
    assert_eq!(media_only.state.vars["media_calls"], Value::Int(1));
    let mut combined = prepared(source);
    let before = state(&combined);
    let before_bytes = serde_json::to_vec(&combined.state).unwrap();
    let previous_history = history(&combined);
    let generations = (combined.runtime_epoch(), combined.interface_epoch());
    let batches = combined.renderer.batches.len();
    let end = video(&combined, "clip", GameVideoFeedback::Report(Feedback::End));
    let error = combined
        .tick_game_frame(0.1, GameFrameReports { videos: vec![end] })
        .unwrap_err();
    assert!(error.to_string().contains("100 000 opérations"), "{error}");
    assert_eq!(state(&combined), before);
    assert_eq!(serde_json::to_vec(&combined.state).unwrap(), before_bytes);
    assert_eq!(history(&combined), previous_history);
    assert_eq!(
        (combined.runtime_epoch(), combined.interface_epoch()),
        generations
    );
    assert_eq!(combined.renderer.batches.len(), batches);
}

#[test]
fn frame_normalizes_bound_canvas_tree_and_cancels_absent_targets_before_one_publication() {
    let source = r#"
function paint(state,props,frame) {return [canvas_rect([0,0,10,10],[1,0,0,1],0)]}
function selected() {
    if replacement {return component("new","canvas",{"draw":"paint","state":{"ticks":0}},[])}
    return component("old","canvas",{"draw":"paint","state":{"ticks":0},"events":{"tick":"old_tick"}},[])
}
screen custom() {return component("root","column",{},[
    component("controller","canvas",{"draw":"paint","state":{"ticks":0},"focus_order":-1,"events":{"tick":"replace_canvas_tree"}},[]), selected()])}
handler replace_canvas_tree(event) {set replacement=true}
handler old_tick(event) {set old_ticks=old_ticks+1}
handler terminal(event) {set old_ends=old_ends+1}
init {set replacement=false set old_ticks=0 set old_ends=0}
scene label P
ui.open("custom",[],false,0)
ui.focus("custom","old")
motion.play("ui:custom/old",motion_pause(5))
video.play("embedded",video_clip("clip.webm",{"target":"ui:custom/old","on_end":"terminal"}))
"P"
"#;
    let mut game = prepared(source);
    assert_eq!(game.state.ui.screens[0].focus.as_deref(), Some("old"));
    let old_canvas = game
        .scene_resource_token(ResourceTaskKind::Canvas, "custom/old")
        .unwrap();
    let old_motion = game
        .scene_resource_token(ResourceTaskKind::Motion, "ui:custom/old")
        .unwrap();
    let old_video = video(&game, "embedded", GameVideoFeedback::Report(Feedback::End));
    let previous_history = history(&game);
    let batches = game.renderer.batches.len();
    let outcome = game
        .tick_game_frame(
            0.1,
            GameFrameReports {
                videos: vec![old_video.clone()],
            },
        )
        .unwrap();
    assert_eq!(outcome.callbacks, 1);
    assert_eq!(outcome.accepted_reports, 0);
    assert_eq!(outcome.ignored_reports, 1);
    assert_eq!(game.state.vars["old_ticks"], Value::Int(0));
    assert_eq!(game.state.vars["old_ends"], Value::Int(0));
    assert!(game.state.motions.tracks.is_empty());
    assert!(game.state.videos.tracks.is_empty());
    assert_eq!(canvas(&game, "custom", "controller").elapsed, 0.1);
    assert_eq!(canvas(&game, "custom", "new").elapsed, 0.0);
    assert!(!game.state.ui.screens[0].canvas_states.contains_key("old"));
    assert_eq!(game.state.ui.screens[0].focus.as_deref(), Some("new"));
    let execution = game.state.scene_execution.as_ref().unwrap();
    for token in [old_canvas, old_motion, old_video.token.clone()] {
        assert!(!execution
            .resources
            .accepts(&token, game.runtime_epoch(), execution.owner()));
    }
    assert!(game
        .scene_resource_token(ResourceTaskKind::Canvas, "custom/new")
        .is_some());
    assert_eq!(history(&game), previous_history);
    assert_eq!(game.renderer.batches.len(), batches + 1);
    assert_eq!(
        game.renderer.batches.last().unwrap().interfaces,
        game.interface_views().unwrap()
    );
    let before = serde_json::to_vec(&game.state).unwrap();
    let outcome = game
        .tick_game_frame(
            0.0,
            GameFrameReports {
                videos: vec![GameVideoReport {
                    token: old_video.token,
                    feedback: GameVideoFeedback::Report(Feedback::Position {
                        seconds: f64::NAN,
                        duration: f64::NAN,
                    }),
                }],
            },
        )
        .unwrap();
    assert_eq!(outcome.ignored_reports, 1);
    assert!(!outcome.published);
    assert_eq!(serde_json::to_vec(&game.state).unwrap(), before);
    assert_eq!(game.renderer.batches.len(), batches + 1);
}

#[test]
fn frame_reopened_canvas_registration_is_atomic_on_renderer_refusal() {
    let source=CANVASES.replace("handler advance(event) {set calls=calls+1}",
        "handler advance(event) {set calls=calls+1 ui.close(\"custom\") ui.open(\"custom\",[],false,0)}");
    let mut game=prepared(&source);
    let before=serde_json::to_vec(&game.state).unwrap();
    let before_history=format!("{:?}",game.history.entries());
    let before_epoch=game.runtime_epoch();
    let before_instance=game.instance_id();
    let before_batches=game.renderer.batches.len();
    let old=game.scene_resource_token(ResourceTaskKind::Canvas,"custom/two").unwrap();
    game.renderer.reject=true;
    let error=game.tick_game_frame(0.1,GameFrameReports::default()).unwrap_err();
    assert!(format!("{error:?}").contains("Rejected complete frame"));
    assert_eq!(serde_json::to_vec(&game.state).unwrap(),before);
    assert_eq!(format!("{:?}",game.history.entries()),before_history);
    assert_eq!(game.runtime_epoch(),before_epoch);
    assert_eq!(game.instance_id(),before_instance);
    assert_eq!(game.renderer.batches.len(),before_batches);
    assert_eq!(game.scene_resource_token(ResourceTaskKind::Canvas,"custom/two").unwrap(),old);
    game.renderer.reject=false;
    let outcome=game.tick_game_frame(0.1,GameFrameReports::default()).unwrap();
    assert_eq!(outcome.callbacks,1);
    assert_eq!(game.state.vars["calls"],Value::Int(1));
    assert_eq!(canvas(&game,"custom","one").elapsed,0.0);
    assert_eq!(canvas(&game,"custom","two").elapsed,0.0);
    assert_ne!(game.scene_resource_token(ResourceTaskKind::Canvas,"custom/two").unwrap().target,old.target);
    assert_eq!(game.renderer.batches.len(),before_batches+1);
    rvn_core::scene_resources::validate_current_resource_state(&game.state).unwrap();
}
