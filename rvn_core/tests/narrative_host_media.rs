//! PRIVATE public Engine contracts; not installed or executed.
use rvn_core::renderer::SceneRenderBatch;
use rvn_core::scene_resources::ResourceTaskKind;
use rvn_core::ui::UiInput;
use rvn_core::video::{Feedback, Playback};
use rvn_core::{
    Engine, GameFrameReports, GameVideoFeedback, GameVideoReport, HostFrameReports, Interaction,
    Renderer, SpriteState, VideoReportDisposition,
};
use rvn_parser::{Hotspot, Position, Transition, Value};
use rvn_ui::programmable::ScreenEventKind;

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
    fn supports_layered_characters(&self) -> bool {
        true
    }
    fn validate_scene_batch(&self, _: &SceneRenderBatch) -> Result<(), String> {
        if self.reject {
            Err("complete Host media candidate rejected".into())
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

const SOURCE: &str = r#"
function paint(state,props,frame) {return [canvas_rect([0,0,10,10],[1,0,0,1],0)]}
screen host(ctx) {return component("root","column",{},[
 component("launch","button",{"events":{"click":"launch_host"}},[]),
 component("clock","canvas",{"draw":"paint","state":{"ticks":0},"events":{"tick":"host_tick"}},[]),
 component("player","image",{"image":"player.png"},[]),component("picture","image",{"image":"picture.png"},[])])}
handler launch_host(event) {
 set launch_roll=random(1,100)
 video.play("hostclip",video_clip("clip.webm",{"target":"ui:host/player","on_end":"host_ended","on_error":"host_failed"}))
 motion.play("ui:host/picture",motion_pause(1))
}
handler host_tick(event) {set canvas_calls=canvas_calls+1 set canvas_roll=random(1,100)}
handler host_ended(event) {set host_ends=host_ends+1 set media_roll=random(1,100)}
handler host_failed(event) {set host_errors=host_errors+1 set media_roll=random(1,100)}
init {set host_ends=0 set host_errors=0 set canvas_calls=0 set canvas_roll=0 set media_roll=0 set launch_roll=0}
scene "parent.png"
scene label P
motion.play("background",motion_pause(5))
timer 5 => jump done
"parent"
call C
"returned"
jump done
scene label C
"child"
return
label done
"#;

fn prepared(source: &str) -> Engine<Host> {
    let mut game = Engine::from_program(
        rvn_parser::parse_program(source).unwrap(),
        Host::default(),
        32,
    )
    .unwrap();
    assert!(matches!(
        game.step_until_interaction().unwrap(),
        Some(Interaction::Dialogue { .. })
    ));
    let context = rvn_core::ui::value_from_json(&rvn_ui::source_menus::preview_context(
        rvn_ui::PageRole::Pause,
        true,
    ))
    .unwrap();
    game.synchronize_source_menu(rvn_ui::PageRole::Pause, Some("host"), context, true, 500)
        .unwrap();
    game
}
fn click(game: &mut Engine<Host>) {
    game.interface_event(UiInput {
        screen: "host".into(),
        element: "launch".into(),
        kind: ScreenEventKind::Click,
        value: None,
        key: None,
    })
    .unwrap();
}
fn row(game: &Engine<Host>, feedback: GameVideoFeedback) -> GameVideoReport {
    GameVideoReport {
        token: game
            .scene_resource_token(ResourceTaskKind::Video, "hostclip")
            .unwrap(),
        feedback,
    }
}
fn story(game: &Engine<Host>) -> serde_json::Value {
    let execution = game.state.scene_execution.as_ref().unwrap();
    serde_json::json!({"pc":game.state.pc,"stack":game.state.call_stack,"random":game.state.random,
        "timer":execution.active.as_ref().unwrap().tasks.timer,"game_timer":execution.game_tasks.timer,
        "locals":execution.active.as_ref().unwrap().locals,"owner":execution.owner(),"legacy_timer":game.active_timer})
}
fn full(game: &Engine<Host>) -> serde_json::Value {
    serde_json::json!({"state":game.state,"runtime":game.runtime_epoch(),"interface":game.interface_epoch(),
        "history":game.history.entries().iter().map(|entry|(&entry.state,&entry.display)).collect::<Vec<_>>()})
}
fn host_instance(game: &Engine<Host>) -> &rvn_core::ui::ScreenInstance {
    game.state
        .ui
        .screens
        .iter()
        .find(|screen| screen.name == "host" && screen.host_role == Some(rvn_ui::PageRole::Pause))
        .unwrap()
}

#[test]
fn host_media_motion_positions_canvas_and_terminal_publish_one_instant_without_story_progress() {
    let mut game = prepared(SOURCE);
    click(&mut game);
    let before = story(&game);
    let batches = game.renderer.batches.len();
    let position = row(
        &game,
        GameVideoFeedback::Report(Feedback::Position {
            seconds: 0.5,
            duration: 2.0,
        }),
    );
    let end = row(&game, GameVideoFeedback::Report(Feedback::End));
    let result = game
        .tick_host_frame_with_reports(
            0.125,
            HostFrameReports {
                videos: vec![position, end],
            },
        )
        .unwrap();
    assert_eq!(game.state.motions.tracks["ui:host/picture"].elapsed, 0.125);
    assert_eq!(host_instance(&game).canvas_states["clock"].elapsed, 0.125);
    assert_eq!(game.state.videos.tracks["hostclip"].position, 2.0);
    assert_eq!(
        game.state.videos.tracks["hostclip"].playback,
        Playback::Ended
    );
    assert_eq!(game.state.vars["host_ends"], Value::Int(1));
    assert_eq!(story(&game), before);
    assert_eq!(game.renderer.batches.len(), batches + 1);
    assert_eq!(
        result
            .receipts
            .iter()
            .map(|receipt| (receipt.ordinal, receipt.disposition))
            .collect::<Vec<_>>(),
        vec![
            (0, VideoReportDisposition::Consumed),
            (1, VideoReportDisposition::Consumed)
        ]
    );
}

#[test]
fn host_canvas_and_media_success_use_ui_instance_random_in_the_actual_callback_order() {
    let mut game = prepared(SOURCE);
    let before = story(&game);
    let mut expected = host_instance(&game).random;
    let launch = expected.integer(1, 100).unwrap();
    click(&mut game);
    assert_eq!(game.state.vars["launch_roll"], Value::Int(launch));
    assert_eq!(host_instance(&game).random, expected);
    assert_eq!(story(&game), before);
    let canvas = expected.integer(1, 100).unwrap();
    let terminal = expected.integer(1, 100).unwrap();
    let end = row(&game, GameVideoFeedback::Report(Feedback::End));
    game.tick_host_frame_with_reports(0.125, HostFrameReports { videos: vec![end] })
        .unwrap();
    assert_eq!(game.state.vars["canvas_roll"], Value::Int(canvas));
    assert_eq!(game.state.vars["media_roll"], Value::Int(terminal));
    assert_eq!(host_instance(&game).random, expected);
    assert_eq!(story(&game), before);
}

#[test]
fn game_receipts_defer_host_rows_and_do_not_claim_their_terminal_was_processed() {
    let mut game = prepared(SOURCE);
    click(&mut game);
    let end = row(&game, GameVideoFeedback::Report(Feedback::End));
    let before = full(&game);
    let result = game
        .tick_game_frame_with_receipts(
            0.0,
            GameFrameReports {
                videos: vec![end.clone(), end.clone()],
            },
        )
        .unwrap();
    assert_eq!(
        result
            .receipts
            .iter()
            .map(|receipt| (receipt.ordinal, receipt.disposition))
            .collect::<Vec<_>>(),
        vec![
            (0, VideoReportDisposition::Deferred),
            (1, VideoReportDisposition::Deferred)
        ]
    );
    assert_eq!(game.state.vars["host_ends"], Value::Int(0));
    assert_eq!(full(&game), before);
    let result = game
        .tick_host_frame_with_reports(
            0.0,
            HostFrameReports {
                videos: vec![end.clone(), end],
            },
        )
        .unwrap();
    assert_eq!(result.receipts.len(), 2);
    assert_eq!(result.accepted_reports, 1);
    assert_eq!(game.state.vars["host_ends"], Value::Int(1));
}

#[test]
fn host_current_invalid_position_refuses_before_any_callback_and_keeps_retry_at_one_delta() {
    let mut game = prepared(SOURCE);
    click(&mut game);
    let valid = row(
        &game,
        GameVideoFeedback::Report(Feedback::Position {
            seconds: 0.5,
            duration: 2.0,
        }),
    );
    let invalid = row(
        &game,
        GameVideoFeedback::Report(Feedback::Position {
            seconds: f64::NAN,
            duration: 2.0,
        }),
    );
    let end = row(&game, GameVideoFeedback::Report(Feedback::End));
    let before = full(&game);
    let batches = game.renderer.batches.len();
    assert!(game
        .tick_host_frame_with_reports(
            0.125,
            HostFrameReports {
                videos: vec![valid.clone(), end.clone(), invalid]
            }
        )
        .is_err());
    assert_eq!(full(&game), before);
    assert_eq!(game.renderer.batches.len(), batches);
    game.tick_host_frame_with_reports(
        0.125,
        HostFrameReports {
            videos: vec![valid, end],
        },
    )
    .unwrap();
    assert_eq!(host_instance(&game).canvas_states["clock"].elapsed, 0.125);
    assert_eq!(game.state.vars["canvas_calls"], Value::Int(1));
}

#[test]
fn host_renderer_and_handler_refusal_restore_ui_rng_history_epochs_and_complete_state() {
    let mut game = prepared(SOURCE);
    click(&mut game);
    let end = row(&game, GameVideoFeedback::Report(Feedback::End));
    let before = full(&game);
    let batches = game.renderer.batches.len();
    game.renderer.reject = true;
    assert!(game
        .tick_host_frame_with_reports(
            0.125,
            HostFrameReports {
                videos: vec![end.clone()]
            }
        )
        .is_err());
    assert_eq!(full(&game), before);
    assert_eq!(game.renderer.batches.len(), batches);
    game.renderer.reject = false;
    game.tick_host_frame_with_reports(0.125, HostFrameReports { videos: vec![end] })
        .unwrap();
    assert_eq!(game.state.vars["host_ends"], Value::Int(1));
    let source = SOURCE.replace(
        "set host_ends=host_ends+1 set media_roll=random(1,100)",
        "set host_ends=host_ends+1 set media_roll=random(1,100) set bad=1/0",
    );
    let mut game = prepared(&source);
    click(&mut game);
    let end = row(&game, GameVideoFeedback::Report(Feedback::End));
    let before = full(&game);
    let batches = game.renderer.batches.len();
    assert!(game
        .tick_host_frame_with_reports(0.125, HostFrameReports { videos: vec![end] })
        .is_err());
    assert_eq!(full(&game), before);
    assert_eq!(game.renderer.batches.len(), batches);
}

#[test]
fn host_stale_payload_after_exact_replacement_is_neutral_before_validation() {
    let mut game = prepared(SOURCE);
    click(&mut game);
    let old = row(
        &game,
        GameVideoFeedback::Report(Feedback::Position {
            seconds: f64::NAN,
            duration: f64::NAN,
        }),
    );
    click(&mut game);
    let before = full(&game);
    let batches = game.renderer.batches.len();
    let result = game
        .tick_host_frame_with_reports(0.0, HostFrameReports { videos: vec![old] })
        .unwrap();
    assert_eq!(
        result.receipts[0].disposition,
        VideoReportDisposition::RejectedStale
    );
    assert_eq!(game.state.vars["host_ends"], Value::Int(0));
    assert_eq!(game.renderer.batches.len(), batches);
    assert_eq!(full(&game), before);
}

#[test]
fn host_motion_without_canvas_runs_and_completes_once_with_compatibility_wrapper() {
    let source=SOURCE.replace(" component(\"clock\",\"canvas\",{\"draw\":\"paint\",\"state\":{\"ticks\":0},\"events\":{\"tick\":\"host_tick\"}},[]),","");
    let mut game = prepared(&source);
    click(&mut game);
    let before = story(&game);
    let result = game
        .tick_host_frame_with_reports(0.25, HostFrameReports::default())
        .unwrap();
    assert_eq!(game.state.motions.tracks["ui:host/picture"].elapsed, 0.25);
    assert_eq!(result.completed_motions, 0);
    assert_eq!(story(&game), before);
    game.tick_host_frame(0.25).unwrap();
    game.interface_tick_host(0.25).unwrap();
    let result = game
        .tick_host_frame_with_reports(0.25, HostFrameReports::default())
        .unwrap();
    assert_eq!(result.completed_motions, 1);
    assert_eq!(game.state.motions.tracks["ui:host/picture"].elapsed, 1.0);
    let result = game
        .tick_host_frame_with_reports(0.0, HostFrameReports::default())
        .unwrap();
    assert_eq!(result.completed_motions, 0);
    assert!(!result.frame.published);
    assert_eq!(story(&game), before);
}

#[test]
fn host_canvas_and_video_callbacks_share_one_real_aggregate_budget() {
    let source=SOURCE.replace("set canvas_calls=canvas_calls+1 set canvas_roll=random(1,100)",
        "local i=0 while i<7500 {set i=i+1} set canvas_calls=canvas_calls+1 set canvas_roll=random(1,100)")
        .replace("set host_ends=host_ends+1 set media_roll=random(1,100)",
        "local i=0 while i<7500 {set i=i+1} set host_ends=host_ends+1 set media_roll=random(1,100)");
    let mut canvas_only = prepared(&source);
    click(&mut canvas_only);
    canvas_only
        .tick_host_frame_with_reports(0.125, HostFrameReports::default())
        .unwrap();
    assert_eq!(canvas_only.state.vars["canvas_calls"], Value::Int(1));
    let mut media_only = prepared(&source);
    click(&mut media_only);
    let end = row(&media_only, GameVideoFeedback::Report(Feedback::End));
    media_only
        .tick_host_frame_with_reports(0.0, HostFrameReports { videos: vec![end] })
        .unwrap();
    assert_eq!(media_only.state.vars["host_ends"], Value::Int(1));
    let mut combined = prepared(&source);
    click(&mut combined);
    let end = row(&combined, GameVideoFeedback::Report(Feedback::End));
    let before = full(&combined);
    let batches = combined.renderer.batches.len();
    let error = combined
        .tick_host_frame_with_reports(0.125, HostFrameReports { videos: vec![end] })
        .unwrap_err();
    assert!(error.to_string().contains("100 000 opérations"), "{error}");
    assert_eq!(full(&combined), before);
    assert_eq!(combined.renderer.batches.len(), batches);
}

#[test]
fn host_checks_all_sibling_positions_before_a_canvas_can_remove_their_instance() {
    let source=SOURCE.replace("component(\"player\",\"image\",{\"image\":\"player.png\"},[])",
        "component(\"player\",\"image\",{\"image\":\"player.png\"},[]),component(\"second\",\"image\",{\"image\":\"second.png\"},[])")
        .replace("motion.play(\"ui:host/picture\",motion_pause(1))",
        "motion.play(\"ui:host/picture\",motion_pause(1)) video.play(\"other\",video_clip(\"other.webm\",{\"target\":\"ui:host/second\"}))")
        .replace("set canvas_calls=canvas_calls+1 set canvas_roll=random(1,100)",
        "ui.close(\"host\")");
    let mut game = prepared(&source);
    click(&mut game);
    let first = row(
        &game,
        GameVideoFeedback::Report(Feedback::Position {
            seconds: 0.5,
            duration: 2.0,
        }),
    );
    let other = GameVideoReport {
        token: game
            .scene_resource_token(ResourceTaskKind::Video, "other")
            .unwrap(),
        feedback: GameVideoFeedback::Report(Feedback::Position {
            seconds: f64::NAN,
            duration: 2.0,
        }),
    };
    let before = full(&game);
    let batches = game.renderer.batches.len();
    assert!(game
        .tick_host_frame_with_reports(
            0.125,
            HostFrameReports {
                videos: vec![first, other]
            }
        )
        .is_err());
    assert_eq!(full(&game), before);
    assert_eq!(game.renderer.batches.len(), batches);
    assert!(game.state.videos.tracks.contains_key("other"));
    assert_eq!(host_instance(&game).canvas_states["clock"].elapsed, 0.0);
}

#[test]
fn host_route_defers_a_real_parked_scene_descriptor_but_rejects_its_old_revision() {
    let source = SOURCE.replace(
        "\"parent\"\ncall C",
        "video.play(\"storyclip\",video_clip(\"story.webm\",{}))\n\"parent\"\ncall C",
    );
    let mut game = prepared(&source);
    let old = game
        .scene_resource_token(ResourceTaskKind::Video, "storyclip")
        .unwrap();
    game.execute_timer_action("call C").unwrap();
    game.step_until_interaction().unwrap();
    let execution = game.state.scene_execution.as_ref().unwrap();
    let actual = execution
        .resources
        .tasks
        .iter()
        .find(|task| task.kind == ResourceTaskKind::Video && task.alias() == "storyclip")
        .unwrap();
    assert!(actual.parked);
    assert_ne!(actual.revision, old.task.revision);
    // The capability comes from the actual parked descriptor, never an invented identity.
    let parked = rvn_core::scene_resources::ResourceToken {
        task: rvn_core::scene_tasks::TaskToken {
            runtime_epoch: game.runtime_epoch(),
            key: actual.key.clone(),
            revision: actual.revision,
        },
        target: actual.target.clone(),
    };
    let before = full(&game);
    let batches = game.renderer.batches.len();
    let result = game
        .tick_host_frame_with_reports(
            0.0,
            HostFrameReports {
                videos: vec![
                    GameVideoReport {
                        token: parked,
                        feedback: GameVideoFeedback::Report(Feedback::Position {
                            seconds: f64::NAN,
                            duration: f64::NAN,
                        }),
                    },
                    GameVideoReport {
                        token: old,
                        feedback: GameVideoFeedback::Report(Feedback::Error("old".into())),
                    },
                ],
            },
        )
        .unwrap();
    assert_eq!(
        result
            .receipts
            .iter()
            .map(|receipt| receipt.disposition)
            .collect::<Vec<_>>(),
        vec![
            VideoReportDisposition::Deferred,
            VideoReportDisposition::RejectedStale
        ]
    );
    assert_eq!(full(&game), before);
    assert_eq!(game.renderer.batches.len(), batches);
}

#[test]
fn host_load_revokes_old_reports_before_payload_and_portable_save_does_not_replay_host_media() {
    let mut game = prepared(SOURCE);
    click(&mut game);
    let old = row(
        &game,
        GameVideoFeedback::Report(Feedback::Position {
            seconds: f64::NAN,
            duration: f64::NAN,
        }),
    );
    let save =
        rvn_core::save::SaveData::try_from_state(&game.state, 1, "Host".into(), "main.rvn".into())
            .unwrap();
    let original = serde_json::to_value(&save).unwrap();
    let epoch = game.runtime_epoch();
    game.load_data(serde_json::from_value(original.clone()).unwrap())
        .unwrap();
    assert_ne!(game.runtime_epoch(), epoch);
    assert!(!game.state.videos.tracks.contains_key("hostclip"));
    assert!(game
        .state
        .ui
        .screens
        .iter()
        .all(|screen| screen.host_role.is_none()));
    let before = full(&game);
    let batches = game.renderer.batches.len();
    let result = game
        .tick_host_frame_with_reports(0.0, HostFrameReports { videos: vec![old] })
        .unwrap();
    assert_eq!(
        result.receipts[0].disposition,
        VideoReportDisposition::RejectedStale
    );
    assert_eq!(full(&game), before);
    assert_eq!(game.renderer.batches.len(), batches);
    assert_eq!(serde_json::to_value(save).unwrap(), original);
}

#[test]
fn host_command_list_keeps_exact_source_random_cursor_after_closing_its_interface() {
    let declarations = r#"
function pick_face(attributes) {if random(1,100)<=50 {return {"face":"a"}} return {"face":"b"}}
function portrait() {return layered_image([64,64],{"face":"a"},image_layers("avatar",["avatar__face__a.png","avatar__face__b.png"]),{"selector":"pick_face"})}
"#;
    let source=format!("{declarations}{}",SOURCE.replace(
        "video.play(\"hostclip\",video_clip(\"clip.webm\",{\"target\":\"ui:host/player\",\"on_end\":\"host_ended\",\"on_error\":\"host_failed\"}))\n motion.play(\"ui:host/picture\",motion_pause(1))",
        "ui.close(\"host\") character.compose(\"avatar\",portrait()) character.attributes(\"avatar\",{\"face\":\"a\"})"));
    let mut game = prepared(&source);
    let before = story(&game);
    let old_order = host_instance(&game).order;
    let mut expected = host_instance(&game).random;
    let launch = expected.integer(1, 100).unwrap();
    let _compose = expected.integer(1, 100).unwrap();
    let attributes = expected.integer(1, 100).unwrap();
    click(&mut game);
    assert_eq!(game.state.vars["launch_roll"], Value::Int(launch));
    assert_eq!(
        game.state.layered.characters["avatar"].attributes["face"],
        if attributes <= 50 { "a" } else { "b" }
    );
    assert!(game.state.ui.screens.is_empty());
    assert_eq!(story(&game), before);
    let context = rvn_core::ui::value_from_json(&rvn_ui::source_menus::preview_context(
        rvn_ui::PageRole::Pause,
        true,
    ))
    .unwrap();
    game.synchronize_source_menu(rvn_ui::PageRole::Pause, Some("host"), context, true, 500)
        .unwrap();
    let new = host_instance(&game);
    assert_ne!(new.order, old_order);
    let seeded = rvn_core::random::RandomState::seeded(
        new.name
            .bytes()
            .fold(new.order ^ 0x72676e6d656e75, |seed, byte| {
                seed.wrapping_mul(1099511628211) ^ u64::from(byte)
            }),
    );
    assert_eq!(new.random, seeded);
    assert_ne!(new.random, expected);
    assert_eq!(story(&game), before);
}
