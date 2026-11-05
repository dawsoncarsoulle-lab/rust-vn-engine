//! Engine contracts for exact UI residency, independent of task clock owners.
//! These are CPU renderer transactions, not native playback observations.

use rvn_core::renderer::SceneRenderBatch;
use rvn_core::scene_resources::{ResourceTaskKind, ResourceToken};
use rvn_core::scene_tasks::TaskOwner;
use rvn_core::ui_residency::{interface_projection, ProjectionResidence, UiProjectionBinding};
use rvn_core::video::{Feedback, Playback, VideoView};
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
    fn supports_composable_motion(&self) -> bool {
        true
    }
    fn supports_custom_canvas(&self) -> bool {
        true
    }
    fn validate_scene_batch(&self, _: &SceneRenderBatch) -> Result<(), String> {
        if self.reject {
            Err("Rejected exact resident projection".into())
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
screen shared(marker) {return component("root","column",{},[
    component("player","text",{"text":marker},[]),
    component("renew","button",{"text":"Renew","events":{"click":"renew_display"}},[])])}
screen notice() {return component("notice","text",{"text":"Game notice"},[])}
handler setup_media(event) {
    video.play("embedded",video_clip("parent.webm",{"target":"ui:shared/player","on_end":"record_end","on_error":"record_error"}))
    motion.play("ui:shared/player",motion_pause(10))
}
handler record_end(event) {set ends=ends+1 set seen=marker ui.open("notice",[],false,0)}
handler record_error(event) {set errors=errors+1 set seen=marker}
handler renew_display(event) {ui.close("shared") ui.open("shared",["renewed"],false,0)}
init {
    set ends=0 set errors=0 set seen=0 set marker=7
    video.play("bootstrap",video_clip("bootstrap.webm",{"on_end":"setup_media"}))
}
scene label P
ui.open("shared",["parent"],false,0)
"parent"
call C
"returned"
jump finish
scene label C
local marker=99
ui.open("shared",["child"],false,0)
"child"
return
label finish
"#;

fn engine(source: &str) -> Engine<Host> {
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
fn advance(game: &mut Engine<Host>) -> String {
    game.advance_dialogue().unwrap();
    dialogue(game)
}
fn view(game: &Engine<Host>, alias: &str) -> VideoView {
    game.video_views()
        .unwrap()
        .into_iter()
        .find(|view| view.id == alias)
        .unwrap()
}
fn report(token: &ResourceToken, feedback: Feedback) -> GameVideoReport {
    GameVideoReport {
        token: token.clone(),
        feedback: GameVideoFeedback::Report(feedback),
    }
}
fn frame(
    game: &mut Engine<Host>,
    seconds: f64,
    reports: Vec<GameVideoReport>,
) -> rvn_core::GameFrameOutcome {
    game.tick_game_frame(seconds, GameFrameReports { videos: reports })
        .unwrap()
}
fn prepared(source: &str) -> Engine<Host> {
    let mut game = engine(source);
    assert_eq!(dialogue(&mut game), "parent");
    let bootstrap = view(&game, "bootstrap").resource_token.unwrap();
    let result = frame(&mut game, 0.0, vec![report(&bootstrap, Feedback::End)]);
    assert_eq!(result.callbacks, 1);
    assert_eq!(result.accepted_reports, 1);
    assert_eq!(
        view(&game, "embedded")
            .resource_token
            .as_ref()
            .unwrap()
            .task
            .key
            .owner,
        TaskOwner::Game
    );
    game
}
fn binding(game: &Engine<Host>, token: &ResourceToken) -> UiProjectionBinding {
    interface_projection(&game.state, &token.task.key)
        .unwrap()
        .unwrap()
}
fn json(game: &Engine<Host>) -> serde_json::Value {
    serde_json::to_value(&game.state).unwrap()
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
fn save(game: &Engine<Host>) -> rvn_core::save::SaveData {
    rvn_core::save::SaveData::try_from_state(&game.state, 1, "Resident".into(), "main.rvn".into())
        .unwrap()
}
fn parent(game: &Engine<Host>) -> &rvn_core::scene::SceneActivation {
    game.state
        .scene_execution
        .as_ref()
        .unwrap()
        .frames
        .iter()
        .find_map(|frame| frame.parked.as_ref())
        .unwrap()
}

#[test]
fn game_video_end_while_parent_ui_is_parked_uses_captured_instance_and_game_globals() {
    let mut game = prepared(SOURCE);
    let original = view(&game, "embedded");
    let token = original.resource_token.clone().unwrap();
    let initial = original.ui_projection.unwrap();
    assert_eq!(initial.residence, ProjectionResidence::Visible);
    assert_eq!(advance(&mut game), "child");
    let child = game
        .state
        .ui
        .screens
        .iter()
        .find(|screen| screen.name == "shared")
        .unwrap()
        .clone();
    assert_eq!(child.arguments, vec![Value::Str("child".into())]);
    let parked = binding(&game, &token);
    assert_eq!(parked.interface, initial.interface);
    assert_eq!(parked.order, initial.order);
    assert_eq!(parked.residence, ProjectionResidence::Parked);
    assert_ne!(parked.order, child.order);
    let batches = game.renderer.batches.len();
    let result = frame(
        &mut game,
        0.25,
        vec![
            report(
                &token,
                Feedback::Position {
                    seconds: 1.25,
                    duration: 3.0,
                },
            ),
            report(&token, Feedback::End),
        ],
    );
    assert_eq!(result.accepted_reports, 2);
    assert_eq!(result.callbacks, 1);
    assert_eq!(game.state.vars["ends"], Value::Int(1));
    assert_eq!(game.state.vars["seen"], Value::Int(7));
    assert_eq!(game.state.videos.tracks["embedded"].position, 3.0);
    assert_eq!(
        game.state.videos.tracks["embedded"].playback,
        Playback::Ended
    );
    assert_eq!(
        game.state
            .ui
            .screens
            .iter()
            .find(|screen| screen.name == "shared")
            .unwrap(),
        &child
    );
    assert_eq!(game.renderer.batches.len(), batches + 1);
    assert_eq!(
        game.renderer
            .batches
            .last()
            .unwrap()
            .videos
            .iter()
            .find(|view| view.id == "embedded")
            .unwrap()
            .ui_projection,
        Some(parked)
    );
    assert_eq!(advance(&mut game), "returned");
    let returned = view(&game, "embedded").ui_projection.unwrap();
    assert_eq!(returned.interface, initial.interface);
    assert_eq!(returned.order, initial.order);
    assert_eq!(returned.residence, ProjectionResidence::Visible);
    let before = json(&game);
    assert_eq!(
        frame(&mut game, 0.0, vec![report(&token, Feedback::End)]).ignored_reports,
        1
    );
    assert_eq!(json(&game), before);
}

#[test]
fn game_video_error_on_parked_parent_is_terminal_once_and_does_not_target_child_homonym() {
    let mut game = prepared(SOURCE);
    let token = view(&game, "embedded").resource_token.unwrap();
    assert_eq!(advance(&mut game), "child");
    let child = game.state.ui.clone();
    let expected = binding(&game, &token);
    let result = frame(
        &mut game,
        0.0,
        vec![report(&token, Feedback::Error("decoder error".into()))],
    );
    assert_eq!(result.accepted_reports, 1);
    assert_eq!(result.callbacks, 1);
    assert_eq!(game.state.vars["errors"], Value::Int(1));
    assert_eq!(game.state.vars["seen"], Value::Int(7));
    assert_eq!(game.state.ui, child);
    assert_eq!(view(&game, "embedded").ui_projection, Some(expected));
    let before = serde_json::to_vec(&game.state).unwrap();
    let batches = game.renderer.batches.len();
    let duplicate = frame(
        &mut game,
        0.0,
        vec![report(&token, Feedback::Error(String::new()))],
    );
    assert_eq!(duplicate.ignored_reports, 1);
    assert!(!duplicate.published);
    assert_eq!(serde_json::to_vec(&game.state).unwrap(), before);
    assert_eq!(game.renderer.batches.len(), batches);
}

#[test]
fn game_motion_clock_continues_against_exact_parked_parent_ui_and_returns_without_rebinding() {
    let mut game = prepared(SOURCE);
    let motion = game
        .scene_resource_token(ResourceTaskKind::Motion, "ui:shared/player")
        .unwrap();
    let visible = binding(&game, &motion);
    assert_eq!(motion.task.key.owner, TaskOwner::Game);
    assert_eq!(advance(&mut game), "child");
    let child = game.state.ui.clone();
    let saved_parent = parent(&game).tasks.suspended_ui.clone();
    let result = frame(&mut game, 2.25, vec![]);
    assert!(result.published);
    assert_eq!(game.state.motions.tracks["ui:shared/player"].elapsed, 2.25);
    assert_eq!(game.state.ui, child);
    assert_eq!(parent(&game).tasks.suspended_ui, saved_parent);
    let projected = game
        .renderer
        .batches
        .last()
        .unwrap()
        .motions
        .iter()
        .find(|view| view.target.key() == "ui:shared/player")
        .unwrap()
        .ui_projection
        .clone()
        .unwrap();
    assert_eq!(projected.interface, visible.interface);
    assert_eq!(projected.order, visible.order);
    assert_eq!(projected.residence, ProjectionResidence::Parked);
    assert_eq!(advance(&mut game), "returned");
    let resumed = binding(&game, &motion);
    assert_eq!(resumed.interface, visible.interface);
    assert_eq!(resumed.order, visible.order);
    assert_eq!(resumed.residence, ProjectionResidence::Visible);
    assert_eq!(game.state.motions.tracks["ui:shared/player"].elapsed, 2.25);
    assert!(game
        .state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .accepts(
            &motion,
            game.runtime_epoch(),
            game.state.scene_execution.as_ref().unwrap().owner()
        ));
}

#[test]
fn scene_owned_ui_work_remains_parked_and_parent_resumes_its_own_homonymous_snapshot() {
    let source=SOURCE.replace("    motion.play(\"ui:shared/player\",motion_pause(10))","")
        .replace("\"parent\"\ncall C","motion.play(\"ui:shared/player\",motion_pause(10))\nvideo.play(\"local\",video_clip(\"local.webm\",{\"target\":\"ui:shared/player\",\"on_end\":\"record_end\"}))\n\"parent\"\ncall C")
        .replace("\"child\"\nreturn","motion.play(\"ui:shared/player\",motion_pause(10))\n\"child\"\nreturn");
    let mut game = prepared(&source);
    frame(&mut game, 1.0, vec![]);
    let old = view(&game, "local");
    let old_token = old.resource_token.unwrap();
    let old_binding = old.ui_projection.unwrap();
    let parent_motion = game
        .scene_resource_token(ResourceTaskKind::Motion, "ui:shared/player")
        .unwrap();
    assert!(matches!(
        parent_motion.task.key.owner,
        TaskOwner::Scene { .. }
    ));
    assert_eq!(advance(&mut game), "child");
    assert!(!game.state.videos.tracks.contains_key("local"));
    let parked_elapsed = parent(&game).tasks.suspended_motions.tracks["ui:shared/player"].elapsed;
    assert_eq!(parked_elapsed, 1.0);
    let result = frame(
        &mut game,
        2.0,
        vec![
            report(
                &old_token,
                Feedback::Position {
                    seconds: f64::NAN,
                    duration: f64::NAN,
                },
            ),
            report(&old_token, Feedback::End),
        ],
    );
    assert_eq!(result.ignored_reports, 2);
    assert_eq!(game.state.vars["ends"], Value::Int(0));
    assert_eq!(
        parent(&game).tasks.suspended_motions.tracks["ui:shared/player"].elapsed,
        1.0
    );
    assert_eq!(game.state.motions.tracks["ui:shared/player"].elapsed, 2.0);
    assert_eq!(advance(&mut game), "returned");
    let restored = view(&game, "local");
    let binding = restored.ui_projection.unwrap();
    assert_eq!(binding.interface, old_binding.interface);
    assert_eq!(binding.order, old_binding.order);
    assert_eq!(binding.residence, ProjectionResidence::Visible);
    assert_eq!(game.state.motions.tracks["ui:shared/player"].elapsed, 1.0);
    assert_ne!(restored.resource_token.as_ref().unwrap(), &old_token);
}

#[test]
fn game_callback_new_command_binds_visible_child_while_existing_player_keeps_parked_parent() {
    let source=SOURCE.replace("set seen=marker ui.open(\"notice\",[],false,0)","set seen=marker video.play(\"child_bound\",video_clip(\"child.webm\",{\"target\":\"ui:shared/player\"}))");
    let mut game = prepared(&source);
    let token = view(&game, "embedded").resource_token.unwrap();
    let original = binding(&game, &token);
    assert_eq!(advance(&mut game), "child");
    let child_identity =
        rvn_core::ui_residency::visible_interface_target(&game.state, "shared").unwrap();
    let result = frame(&mut game, 0.0, vec![report(&token, Feedback::End)]);
    assert_eq!(result.callbacks, 1);
    let old_binding = view(&game, "embedded").ui_projection.unwrap();
    let created = view(&game, "child_bound");
    let new_binding = created.ui_projection.unwrap();
    assert_eq!(old_binding.interface, original.interface);
    assert_eq!(old_binding.residence, ProjectionResidence::Parked);
    assert_eq!(new_binding.interface, child_identity);
    assert_eq!(new_binding.residence, ProjectionResidence::Visible);
    assert_ne!(new_binding.interface, old_binding.interface);
    assert_eq!(
        created.resource_token.unwrap().task.key.owner,
        TaskOwner::Game
    );
}

#[test]
fn save_load_keeps_game_dependency_on_parked_parent_and_revokes_old_callback_epoch() {
    let mut game = prepared(SOURCE);
    let original = view(&game, "embedded").resource_token.unwrap();
    frame(
        &mut game,
        0.5,
        vec![report(
            &original,
            Feedback::Position {
                seconds: 0.75,
                duration: 4.0,
            },
        )],
    );
    assert_eq!(advance(&mut game), "child");
    let expected = binding(&game, &original);
    let saved = save(&game);
    let wire = serde_json::to_vec(&saved).unwrap();
    let mut restored = engine(SOURCE);
    let before_batches = restored.renderer.batches.len();
    restored
        .load_data(serde_json::from_slice(&wire).unwrap())
        .unwrap();
    assert_eq!(restored.renderer.batches.len(), before_batches + 1);
    assert_eq!(restored.state.vars["ends"], Value::Int(0));
    assert_eq!(restored.state.videos.tracks["embedded"].position, 0.75);
    assert_eq!(
        restored.state.motions.tracks["ui:shared/player"].elapsed,
        0.5
    );
    let current = view(&restored, "embedded");
    assert_eq!(current.ui_projection, Some(expected));
    let current_token = current.resource_token.unwrap();
    assert_ne!(
        current_token.task.runtime_epoch,
        original.task.runtime_epoch
    );
    let before = serde_json::to_vec(&restored.state).unwrap();
    let stale = frame(
        &mut restored,
        0.0,
        vec![
            report(
                &original,
                Feedback::Position {
                    seconds: f64::NAN,
                    duration: f64::NAN,
                },
            ),
            report(&original, Feedback::End),
        ],
    );
    assert_eq!(stale.ignored_reports, 2);
    assert!(!stale.published);
    assert_eq!(serde_json::to_vec(&restored.state).unwrap(), before);
    let accepted = frame(
        &mut restored,
        0.0,
        vec![report(&current_token, Feedback::End)],
    );
    assert_eq!(accepted.callbacks, 1);
    assert_eq!(restored.state.vars["ends"], Value::Int(1));
    assert_eq!(serde_json::to_vec(&saved).unwrap(), wire);
}

#[test]
fn abandoning_parent_ui_cancels_game_dependents_without_cancelling_standalone_game_player() {
    let source = SOURCE.replace("call C", "jump C");
    let mut game = prepared(&source);
    let embedded = view(&game, "embedded").resource_token.unwrap();
    let motion = game
        .scene_resource_token(ResourceTaskKind::Motion, "ui:shared/player")
        .unwrap();
    let parent_identity = binding(&game, &embedded).interface;
    let bootstrap = view(&game, "bootstrap");
    assert_eq!(advance(&mut game), "child");
    assert!(!game.state.videos.tracks.contains_key("embedded"));
    assert!(!game.state.motions.tracks.contains_key("ui:shared/player"));
    assert!(game
        .state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .targets
        .iter()
        .all(|target| target.identity != parent_identity));
    assert_eq!(
        view(&game, "bootstrap").resource_token,
        bootstrap.resource_token
    );
    assert_eq!(view(&game, "bootstrap").track, bootstrap.track);
    let before = serde_json::to_vec(&game.state).unwrap();
    let stale = frame(
        &mut game,
        0.0,
        vec![
            report(
                &embedded,
                Feedback::Position {
                    seconds: f64::NAN,
                    duration: f64::NAN,
                },
            ),
            report(&embedded, Feedback::End),
        ],
    );
    assert_eq!(stale.ignored_reports, 2);
    assert!(!stale.published);
    assert_eq!(serde_json::to_vec(&game.state).unwrap(), before);
    let execution = game.state.scene_execution.as_ref().unwrap();
    assert!(!execution
        .resources
        .accepts(&motion, game.runtime_epoch(), execution.owner()));
    assert_eq!(game.state.vars["ends"], Value::Int(0));
}

#[test]
fn malformed_parked_snapshot_save_is_refused_before_state_or_publication_despite_child_alias() {
    let mut game = prepared(SOURCE);
    assert_eq!(advance(&mut game), "child");
    let valid = save(&game);
    let original = serde_json::to_vec(&valid).unwrap();
    let mut corrupt = valid.clone();
    let parent = corrupt
        .scene_execution
        .as_mut()
        .unwrap()
        .frames
        .iter_mut()
        .find_map(|frame| frame.parked.as_mut())
        .unwrap();
    parent.tasks.suspended_ui.screens.clear();
    assert!(game
        .state
        .ui
        .screens
        .iter()
        .any(|screen| screen.name == "shared"));
    let before = serde_json::to_vec(&game.state).unwrap();
    let previous_history = history(&game);
    let generations = (game.runtime_epoch(), game.interface_epoch());
    let batches = game.renderer.batches.len();
    assert!(game.load_data(corrupt).is_err());
    assert_eq!(serde_json::to_vec(&game.state).unwrap(), before);
    assert_eq!(history(&game), previous_history);
    assert_eq!((game.runtime_epoch(), game.interface_epoch()), generations);
    assert_eq!(game.renderer.batches.len(), batches);
    assert_eq!(serde_json::to_vec(&valid).unwrap(), original);
    game.load_data(valid).unwrap();
    assert_eq!(
        view(&game, "embedded").ui_projection.unwrap().residence,
        ProjectionResidence::Parked
    );
}

#[test]
fn child_global_changes_do_not_rebind_hidden_parent_and_return_cancels_its_absent_component() {
    let source=SOURCE.replace("screen shared(marker)",r#"
function paint_frame(state,props,frame) {return [canvas_rect([0,0,5,5],[1,0,0,1],0)]}
function media_component(marker) {
    if marker=="parent" and hide_parent {return component("new","canvas",{"draw":"paint_frame","state":{"ticks":5}},[])}
    return component("player","text",{"text":marker},[])
}
function child_controller(marker) {
    if marker=="child" {return component("controller","canvas",{"draw":"paint_frame","state":{},"events":{"tick":"hide_parent_component"}},[])}
    return component("idle","text",{"text":"idle"},[])
}
handler hide_parent_component(event) {set hide_parent=true}
handler screen_opened(event) {set open_count=open_count+1}
screen shared(marker)
"#).replace("component(\"player\",\"text\",{\"text\":marker},[]),","media_component(marker), child_controller(marker),")
        .replace("component(\"root\",\"column\",{},[","component(\"root\",\"column\",{\"events\":{\"open\":\"screen_opened\"}},[")
        .replace("handler setup_media(event) {","handler setup_media(event) {set setup_count=setup_count+1")
        .replace("set ends=0 set errors=0 set seen=0 set marker=7","set ends=0 set errors=0 set seen=0 set marker=7 set hide_parent=false set open_count=0 set setup_count=0");
    let mut game = prepared(&source);
    let embedded = view(&game, "embedded").resource_token.unwrap();
    let original = binding(&game, &embedded);
    let parent_random = game
        .state
        .ui
        .screens
        .iter()
        .find(|screen| screen.name == "shared")
        .unwrap()
        .random;
    assert_eq!(game.state.vars["open_count"], Value::Int(1));
    assert_eq!(game.state.vars["setup_count"], Value::Int(1));
    assert_eq!(advance(&mut game), "child");
    assert_eq!(game.state.vars["open_count"], Value::Int(2));
    let child_identity =
        rvn_core::ui_residency::visible_interface_target(&game.state, "shared").unwrap();
    let batches = game.renderer.batches.len();
    let result = frame(
        &mut game,
        0.1,
        vec![
            report(
                &embedded,
                Feedback::Position {
                    seconds: 0.25,
                    duration: 2.0,
                },
            ),
            report(&embedded, Feedback::End),
        ],
    );
    assert_eq!(result.callbacks, 2);
    assert_eq!(result.accepted_reports, 2);
    assert_eq!(result.ignored_reports, 0);
    assert_eq!(game.state.vars["ends"], Value::Int(1));
    assert_eq!(game.state.vars["hide_parent"], Value::Bool(true));
    assert_eq!(
        game.state.videos.tracks["embedded"].playback,
        Playback::Ended
    );
    assert_eq!(game.state.videos.tracks["embedded"].position, 2.0);
    assert_eq!(game.state.motions.tracks["ui:shared/player"].elapsed, 0.1);
    let parked = binding(&game, &embedded);
    assert_eq!(parked.interface, original.interface);
    assert_eq!(parked.order, original.order);
    assert_eq!(parked.residence, ProjectionResidence::Parked);
    assert_eq!(
        rvn_core::ui_residency::visible_interface_target(&game.state, "shared").unwrap(),
        child_identity
    );
    assert_ne!(child_identity, original.interface);
    assert!(game
        .interface_views()
        .unwrap()
        .iter()
        .find(|view| view.name == "shared")
        .unwrap()
        .root
        .find("player")
        .is_some());
    assert_eq!(game.renderer.batches.len(), batches + 1);
    assert!(parent(&game)
        .tasks
        .suspended_ui
        .screens
        .iter()
        .find(|screen| screen.name == "shared")
        .unwrap()
        .canvas_states
        .is_empty());
    assert_eq!(advance(&mut game), "returned");
    assert!(!game.state.videos.tracks.contains_key("embedded"));
    assert!(!game.state.motions.tracks.contains_key("ui:shared/player"));
    let restored = rvn_core::ui_residency::visible_interface_target(&game.state, "shared").unwrap();
    assert_eq!(restored, original.interface);
    assert_ne!(restored, child_identity);
    let resumed = game
        .state
        .ui
        .screens
        .iter()
        .find(|screen| screen.name == "shared")
        .unwrap();
    assert_eq!(resumed.random, parent_random);
    assert_eq!(resumed.canvas_states["new"].elapsed, 0.0);
    assert_eq!(
        resumed.canvas_states["new"].state,
        Value::Dict(std::collections::BTreeMap::from([(
            "ticks".into(),
            Value::Int(5)
        )]))
    );
    let canvas = game
        .scene_resource_token(ResourceTaskKind::Canvas, "shared/new")
        .unwrap();
    assert_eq!(
        canvas.task.key.owner,
        TaskOwner::Scene {
            activation: game
                .state
                .scene_execution
                .as_ref()
                .unwrap()
                .owner()
                .unwrap()
        }
    );
    assert_eq!(game.state.vars["open_count"], Value::Int(2));
    assert_eq!(game.state.vars["setup_count"], Value::Int(1));
    assert!(game
        .interface_views()
        .unwrap()
        .iter()
        .find(|view| view.name == "shared")
        .unwrap()
        .root
        .find("player")
        .is_none());
    let before = serde_json::to_vec(&game.state).unwrap();
    let stale = frame(
        &mut game,
        0.0,
        vec![report(
            &embedded,
            Feedback::Position {
                seconds: f64::NAN,
                duration: f64::NAN,
            },
        )],
    );
    assert_eq!(stale.ignored_reports, 1);
    assert!(!stale.published);
    assert_eq!(serde_json::to_vec(&game.state).unwrap(), before);
}

#[test]
fn rejected_parked_game_terminal_frame_preserves_clocks_history_epochs_and_retry_publishes_once() {
    let mut game = prepared(SOURCE);
    let token = view(&game, "embedded").resource_token.unwrap();
    assert_eq!(advance(&mut game), "child");
    let before = serde_json::to_vec(&game.state).unwrap();
    let previous_history = history(&game);
    let generations = (game.runtime_epoch(), game.interface_epoch());
    let batches = game.renderer.batches.len();
    let reports = GameFrameReports {
        videos: vec![
            report(
                &token,
                Feedback::Position {
                    seconds: 0.5,
                    duration: 3.0,
                },
            ),
            report(&token, Feedback::End),
        ],
    };
    game.renderer.reject = true;
    assert!(game.tick_game_frame(0.25, reports.clone()).is_err());
    assert_eq!(serde_json::to_vec(&game.state).unwrap(), before);
    assert_eq!(history(&game), previous_history);
    assert_eq!((game.runtime_epoch(), game.interface_epoch()), generations);
    assert_eq!(game.renderer.batches.len(), batches);
    game.renderer.reject = false;
    let result = game.tick_game_frame(0.25, reports).unwrap();
    assert_eq!(result.callbacks, 1);
    assert_eq!(result.accepted_reports, 2);
    assert_eq!(game.state.vars["ends"], Value::Int(1));
    assert_eq!(game.state.motions.tracks["ui:shared/player"].elapsed, 0.25);
    assert_eq!(game.renderer.batches.len(), batches + 1);
    assert_eq!(history(&game), previous_history);
}
