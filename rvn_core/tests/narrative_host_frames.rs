//! Host frame contracts: one logical Host instant and one checked publication.
//! No device, browser, GPU, native Unreal, or production Bevy schedule is exercised.

use rvn_core::renderer::SceneRenderBatch;
use rvn_core::scene_resources::{
    resource_key, EffectClock, EffectParameters, EffectProjectionBase, ResourceTaskKind,
    ResourceToken, SavedEffect, SpriteEffectKind, TargetKind, TransformSnapshot, VisualTransition,
};
use rvn_core::scene_tasks::TaskOwner;
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

// PRIVATE NOT_RUN. New Core contracts; all existing frame tests stay intact.
const SOURCE: &str = r#"
function paint(state,props,frame) {return [canvas_rect([0,0,10,10],[1,0,0,1],0)]}
function widget(id) {return component(id,"canvas",{"draw":"paint","state":{"ticks":0},"events":{"tick":"advance"}},[])}
screen host(ctx) {return component("root","column",{},[widget("one"),widget("two")])}
screen story() {return component("story","canvas",{"draw":"paint","state":{"ticks":0}},[])}
screen cover() {return component("cover","text",{"text":"cover"},[])}
handler advance(event) {set calls=calls+1}
init {set calls=0}
scene "parent.png"
scene label P
hero.show("smile") at left
ui.open("story",[],false,0)
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
    game
}
fn open_host(game: &mut Engine<Host>) {
    let context = rvn_core::ui::value_from_json(&rvn_ui::source_menus::preview_context(
        rvn_ui::PageRole::Pause,
        true,
    ))
    .unwrap();
    game.synchronize_source_menu(rvn_ui::PageRole::Pause, Some("host"), context, true, 500)
        .unwrap();
}
fn host(role: rvn_ui::PageRole) -> TaskOwner {
    TaskOwner::Host {
        role: role.id().into(),
    }
}
fn snapshot(game: &Engine<Host>) -> serde_json::Value {
    serde_json::json!({"state": game.state,
        "history": game.history.entries().iter().map(|entry| (&entry.state, &entry.display)).collect::<Vec<_>>(),
        "runtime_epoch": game.runtime_epoch(), "interface_epoch": game.interface_epoch()})
}
fn canvas(game: &Engine<Host>, name: &str, element: &str) -> f64 {
    game.state
        .ui
        .screens
        .iter()
        .find(|screen| screen.name == name)
        .unwrap()
        .canvas_states[element]
        .elapsed
}
fn register(
    game: &mut Engine<Host>,
    owner: TaskOwner,
    name: &str,
    duration: f64,
    elapsed: f64,
    looping: bool,
) -> ResourceToken {
    let (kind, alias) = if looping {
        (TargetKind::Sprite, "hero")
    } else {
        (TargetKind::Background, "background")
    };
    let target = game
        .state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .target(&TaskOwner::Game, kind, alias)
        .unwrap()
        .clone();
    game.register_scene_effect(
        resource_key(owner, ResourceTaskKind::Effect, name),
        target,
        SavedEffect {
            presentation: None,
            base_resolved: false,
            clock: EffectClock {
                duration,
                elapsed,
                looping,
            },
            parameters: if looping {
                EffectParameters::SpriteAnimation {
                    animation: SpriteEffectKind::Shake { intensity: 3.0 },
                    base: TransformSnapshot::default(),
                }
            } else {
                EffectParameters::VisualFade {
                    from: 0.0,
                    to: 1.0,
                    despawn_on_finish: false,
                    transition: VisualTransition::Fade,
                    base: TransformSnapshot::default(),
                    layered: false,
                }
            },
        },
    )
    .unwrap()
}
fn descriptor<'a>(game: &'a Engine<Host>, token: &ResourceToken) -> &'a SavedEffect {
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
}

#[test]
fn host_frame_advances_two_roles_once_without_advancing_story_or_game_clocks() {
    let mut game = prepared(SOURCE);
    let activation = game
        .state
        .scene_execution
        .as_ref()
        .unwrap()
        .owner()
        .unwrap();
    let story = register(
        &mut game,
        TaskOwner::Scene { activation },
        "story-fade",
        4.0,
        0.5,
        false,
    );
    let global = register(&mut game, TaskOwner::Game, "game-fade", 4.0, 0.5, false);
    let pause = register(
        &mut game,
        host(rvn_ui::PageRole::Pause),
        "pause-fade",
        4.0,
        0.5,
        false,
    );
    let confirm = register(
        &mut game,
        host(rvn_ui::PageRole::Confirm),
        "confirm-fade",
        4.0,
        0.5,
        false,
    );
    open_host(&mut game);
    let pc = game.state.pc;
    let history = serde_json::to_value(
        game.history
            .entries()
            .iter()
            .map(|entry| (&entry.state, &entry.display))
            .collect::<Vec<_>>(),
    )
    .unwrap();
    let batches = game.renderer.batches.len();
    let result = game.tick_host_frame(0.125).unwrap();
    assert!(result.progressed && result.published);
    assert_eq!(result.callbacks, 2);
    assert_eq!(descriptor(&game, &pause).clock.elapsed, 0.625);
    assert_eq!(descriptor(&game, &confirm).clock.elapsed, 0.625);
    assert_eq!(descriptor(&game, &story).clock.elapsed, 0.5);
    assert_eq!(descriptor(&game, &global).clock.elapsed, 0.5);
    assert!(!descriptor(&game, &pause).base_resolved);
    assert_eq!(canvas(&game, "host", "one"), 0.125);
    assert_eq!(canvas(&game, "host", "two"), 0.125);
    assert_eq!(canvas(&game, "story", "story"), 0.0);
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
    assert_eq!(game.state.pc, pc);
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
    assert_eq!(game.renderer.batches.len(), batches + 1);
}

#[test]
fn host_frame_prepares_sibling_clocks_before_a_handler_opens_a_modal() {
    let source = SOURCE.replace(
        "handler advance(event) {set calls=calls+1}",
        "handler advance(event) {set calls=calls+1 ui.open(\"cover\",[],true,1000)}",
    );
    let mut game = prepared(&source);
    open_host(&mut game);
    let fade = register(
        &mut game,
        host(rvn_ui::PageRole::Pause),
        "clock-before-modal",
        4.0,
        0.0,
        false,
    );
    let batches = game.renderer.batches.len();
    let result = game.tick_host_frame(0.125).unwrap();
    assert_eq!(game.state.vars["calls"], Value::Int(1));
    assert_eq!(result.callbacks, 1);
    assert_eq!(canvas(&game, "host", "one"), 0.125);
    assert_eq!(canvas(&game, "host", "two"), 0.125);
    assert_eq!(descriptor(&game, &fade).clock.elapsed, 0.125);
    assert_eq!(game.renderer.batches.len(), batches + 1);
}

#[test]
fn host_effect_without_canvas_completes_at_exact_phase_without_deleting_current_object() {
    let mut game = prepared(SOURCE);
    let token = register(
        &mut game,
        host(rvn_ui::PageRole::Pause),
        "no-canvas",
        0.25,
        0.0,
        false,
    );
    let background = game.state.background_image.clone();
    let target = token.target.clone();
    let result = game.tick_host_frame(0.25).unwrap();
    assert_eq!(result.callbacks, 0);
    assert_eq!(result.completed_effects, 1);
    assert!(result.published);
    assert_eq!(game.state.background_image, background);
    assert_eq!(
        game.state
            .scene_execution
            .as_ref()
            .unwrap()
            .resources
            .target(&TaskOwner::Game, TargetKind::Background, "background"),
        Some(&target)
    );
    assert!(game
        .state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .tasks
        .iter()
        .all(|task| task.key != token.task.key));
    let before = snapshot(&game);
    assert!(!game.complete_scene_effect(&token).unwrap());
    assert_eq!(snapshot(&game), before);
    assert!(!game.tick_host_frame(0.25).unwrap().published);
}

#[test]
fn host_zero_delta_drains_due_and_looping_clock_is_cumulative_without_fake_base() {
    let mut game = prepared(SOURCE);
    let due = register(
        &mut game,
        host(rvn_ui::PageRole::Pause),
        "zero-due",
        0.0,
        0.0,
        false,
    );
    let looping = register(
        &mut game,
        host(rvn_ui::PageRole::Confirm),
        "looping",
        0.125,
        0.25,
        true,
    );
    let result = game.tick_host_frame(0.0).unwrap();
    assert_eq!(result.completed_effects, 1);
    assert_eq!(descriptor(&game, &looping).clock.elapsed, 0.25);
    assert!(game
        .state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .tasks
        .iter()
        .all(|task| task.key != due.task.key));
    game.tick_host_frame(0.25).unwrap();
    assert_eq!(descriptor(&game, &looping).clock.elapsed, 0.5);
    assert!(!descriptor(&game, &looping).base_resolved);
    let before = snapshot(&game);
    assert!(!game.complete_scene_effect(&looping).unwrap());
    assert_eq!(snapshot(&game), before);
}

#[test]
fn host_due_capability_is_revalidated_after_canvas_removes_its_exact_dependency() {
    let source = SOURCE.replace(
        "handler advance(event) {set calls=calls+1}",
        "handler advance(event) {set calls=calls+1 ui.close(\"host\")}",
    );
    let mut game = prepared(&source);
    open_host(&mut game);
    let token = register(
        &mut game,
        host(rvn_ui::PageRole::Pause),
        "bound-host",
        0.125,
        0.0,
        false,
    );
    // This real UI instance is an explicit dependency, not an alias lookup.
    let ui = game
        .state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .target(
            &host(rvn_ui::PageRole::Pause),
            TargetKind::Interface,
            "host",
        )
        .unwrap()
        .clone();
    game.state
        .scene_execution
        .as_mut()
        .unwrap()
        .resources
        .tasks
        .iter_mut()
        .find(|task| task.key == token.task.key)
        .unwrap()
        .dependencies
        .push(ui);
    rvn_core::scene_resources::validate_current_resource_state(&game.state).unwrap();
    let result = game.tick_host_frame(0.125).unwrap();
    assert_eq!(result.callbacks, 1);
    assert_eq!(result.completed_effects, 0);
    assert_eq!(game.state.vars["calls"], Value::Int(1));
    assert!(game
        .state
        .ui
        .screens
        .iter()
        .all(|screen| screen.name != "host"));
    assert!(game
        .state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .tasks
        .iter()
        .all(|task| task.key != token.task.key));
    let before = snapshot(&game);
    assert!(!game.complete_scene_effect(&token).unwrap());
    assert_eq!(snapshot(&game), before);
}

#[test]
fn host_renderer_refusal_preserves_effect_canvas_rng_history_and_retry_uses_one_delta() {
    let source = SOURCE.replace(
        "handler advance(event) {set calls=calls+1}",
        "handler advance(event) {set calls=calls+1 set roll=random(1,100)}",
    );
    let mut game = prepared(&source);
    open_host(&mut game);
    let token = register(
        &mut game,
        host(rvn_ui::PageRole::Pause),
        "refusal",
        4.0,
        0.0,
        false,
    );
    let before = snapshot(&game);
    let batches = game.renderer.batches.len();
    game.renderer.reject = true;
    assert!(game.tick_host_frame(0.125).is_err());
    assert_eq!(snapshot(&game), before);
    assert_eq!(game.renderer.batches.len(), batches);
    game.renderer.reject = false;
    game.tick_host_frame(0.125).unwrap();
    assert_eq!(descriptor(&game, &token).clock.elapsed, 0.125);
    assert_eq!(canvas(&game, "host", "one"), 0.125);
    assert_eq!(canvas(&game, "host", "two"), 0.125);
    assert_eq!(game.state.vars["calls"], Value::Int(2));
    assert_eq!(game.renderer.batches.len(), batches + 1);
}

#[test]
fn host_handler_failure_and_clock_overflow_are_atomic_before_renderer_publication() {
    let source = SOURCE.replace(
        "handler advance(event) {set calls=calls+1}",
        "handler advance(event) {set calls=calls+1 set roll=random(1,100) set bad=1/0}",
    );
    let mut game = prepared(&source);
    open_host(&mut game);
    register(
        &mut game,
        host(rvn_ui::PageRole::Pause),
        "valid-before-failure",
        4.0,
        0.0,
        false,
    );
    let before = snapshot(&game);
    let batches = game.renderer.batches.len();
    assert!(game.tick_host_frame(0.125).is_err());
    assert_eq!(snapshot(&game), before);
    assert_eq!(game.renderer.batches.len(), batches);
    let mut game = prepared(SOURCE);
    register(
        &mut game,
        host(rvn_ui::PageRole::Pause),
        "a-advanced-first",
        4.0,
        0.0,
        false,
    );
    register(
        &mut game,
        host(rvn_ui::PageRole::Pause),
        "z-overflow",
        0.125,
        1.0e12,
        true,
    );
    let before = snapshot(&game);
    let batches = game.renderer.batches.len();
    assert!(game.tick_host_frame(0.25).is_err());
    assert_eq!(snapshot(&game), before);
    assert_eq!(game.renderer.batches.len(), batches);
}

#[test]
fn host_invalid_delta_is_neutral_and_portable_load_cannot_restore_host_clock_or_token() {
    let mut game = prepared(SOURCE);
    let token = register(
        &mut game,
        host(rvn_ui::PageRole::Pause),
        "portable-host",
        4.0,
        0.0,
        false,
    );
    for delta in [-0.1, 0.250001, f64::NAN, f64::INFINITY] {
        let before = snapshot(&game);
        let batches = game.renderer.batches.len();
        assert!(game.tick_host_frame(delta).is_err());
        assert_eq!(snapshot(&game), before);
        assert_eq!(game.renderer.batches.len(), batches);
    }
    game.interface_tick_host(0.125).unwrap();
    assert_eq!(descriptor(&game, &token).clock.elapsed, 0.125);
    let save = rvn_core::save::SaveData::try_from_state(
        &game.state,
        1,
        "portable".into(),
        "main.rvn".into(),
    )
    .unwrap();
    assert!(save
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .tasks
        .iter()
        .all(|task| !matches!(&task.key.owner, TaskOwner::Host { .. })));
    let epoch = game.runtime_epoch();
    game.load_data(save).unwrap();
    assert_ne!(game.runtime_epoch(), epoch);
    let before = snapshot(&game);
    assert!(!game
        .capture_scene_effect_base(
            &token,
            EffectProjectionBase::VisualTransform(TransformSnapshot::default())
        )
        .unwrap());
    assert!(!game.complete_scene_effect(&token).unwrap());
    assert_eq!(snapshot(&game), before);
    assert!(!game.tick_host_frame(0.125).unwrap().published);
}

#[test]
fn host_effect_keeps_its_exact_clock_across_call_and_return_while_parent_is_parked() {
    let mut game = prepared(SOURCE);
    let parent = game
        .state
        .scene_execution
        .as_ref()
        .unwrap()
        .owner()
        .unwrap();
    let story = register(
        &mut game,
        TaskOwner::Scene { activation: parent },
        "parked-story",
        4.0,
        0.5,
        false,
    );
    let token = register(
        &mut game,
        host(rvn_ui::PageRole::Pause),
        "host-across-call",
        4.0,
        0.5,
        false,
    );
    game.advance_dialogue().unwrap();
    assert!(
        matches!(game.step_until_interaction().unwrap(), Some(Interaction::Dialogue { text, .. }) if text == "child")
    );
    let before = serde_json::json!({"frames":game.state.scene_execution.as_ref().unwrap().frames,
        "active":game.state.scene_execution.as_ref().unwrap().active,"vars":game.state.vars,
        "pc":game.state.pc,"stack":game.state.call_stack,"random":game.state.random});
    game.tick_host_frame(0.125).unwrap();
    assert_eq!(descriptor(&game, &token).clock.elapsed, 0.625);
    assert_eq!(descriptor(&game, &story).clock.elapsed, 0.5);
    assert_eq!(
        serde_json::json!({"frames":game.state.scene_execution.as_ref().unwrap().frames,
        "active":game.state.scene_execution.as_ref().unwrap().active,"vars":game.state.vars,
        "pc":game.state.pc,"stack":game.state.call_stack,"random":game.state.random}),
        before
    );
    game.advance_dialogue().unwrap();
    assert!(
        matches!(game.step_until_interaction().unwrap(), Some(Interaction::Dialogue { text, .. }) if text == "returned")
    );
    assert_eq!(
        game.state.scene_execution.as_ref().unwrap().owner(),
        Some(parent)
    );
    assert_eq!(descriptor(&game, &token).clock.elapsed, 0.625);
    assert_eq!(descriptor(&game, &story).clock.elapsed, 0.5);
    let execution = game.state.scene_execution.as_ref().unwrap();
    assert!(execution
        .resources
        .accepts(&token, game.runtime_epoch(), execution.owner()));
}

#[test]
fn autonomous_host_retired_completes_after_its_original_current_was_replaced_without_ui() {
    use rvn_core::retired_presentations::{snapshot_retired_source, PresentationSlot};
    let source = SOURCE.replace("\"child\"", "scene \"replacement.png\"\n\"child\"");
    let mut game = prepared(&source);
    let payload = snapshot_retired_source(&game.state, &PresentationSlot::Background)
        .unwrap()
        .unwrap();
    let original = payload.original_target.identity.clone();
    let epoch = game.runtime_epoch();
    let token = rvn_core::scene_resources::register_retired_effect(
        &mut game.state,
        epoch,
        host(rvn_ui::PageRole::Pause),
        payload,
        SavedEffect {
            presentation: None,
            base_resolved: false,
            clock: EffectClock {
                duration: 0.25,
                elapsed: 0.0,
                looping: false,
            },
            parameters: EffectParameters::VisualFade {
                from: 1.0,
                to: 0.0,
                despawn_on_finish: true,
                transition: VisualTransition::Fade,
                base: TransformSnapshot::default(),
                layered: false,
            },
        },
    )
    .unwrap()
    .unwrap();
    game.advance_dialogue().unwrap();
    assert!(
        matches!(game.step_until_interaction().unwrap(), Some(Interaction::Dialogue { text, .. }) if text == "child")
    );
    let replacement = game
        .state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .target(&TaskOwner::Game, TargetKind::Background, "background")
        .unwrap()
        .clone();
    assert_ne!(replacement, original);
    assert!(game
        .state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .targets
        .iter()
        .all(|record| record.identity != original));
    let background = game.state.background_image.clone();
    let result = game.tick_host_frame(0.25).unwrap();
    assert_eq!(result.callbacks, 0);
    assert_eq!(result.completed_effects, 1);
    assert_eq!(game.state.background_image, background);
    let execution = game.state.scene_execution.as_ref().unwrap();
    assert!(execution
        .resources
        .tasks
        .iter()
        .all(|task| task.key != token.task.key));
    assert!(execution
        .resources
        .targets
        .iter()
        .all(|record| record.identity != token.target));
    assert_eq!(
        execution
            .resources
            .target(&TaskOwner::Game, TargetKind::Background, "background"),
        Some(&replacement)
    );
}


#[test]
fn story_music_and_sprite_replacements_preserve_autonomous_host_retired_until_its_own_end() {
    use rvn_core::retired_presentations::{snapshot_retired_source, PresentationSlot};
    for (slot, setup, replacement) in [
        (PresentationSlot::Music, "music.play(\"old.ogg\")\n", "music.play(\"new.ogg\")"),
        (PresentationSlot::Sprite { id: "hero".into() }, "", "hero.show(\"angry\") at right"),
    ] {
        let source = SOURCE.replace("\"parent\"", &format!("{setup}\"parent\""))
            .replace("\"child\"", &format!("{replacement}\n\"child\""));
        let mut game = prepared(&source);
        let payload = snapshot_retired_source(&game.state, &slot).unwrap().unwrap();
        let original = payload.clone();
        let parameters = if matches!(slot, PresentationSlot::Music) {
            EffectParameters::AudioFade { from: 1.0, to: 0.0, despawn_on_done: true }
        } else {
            EffectParameters::VisualFade { from: 1.0, to: 0.0, despawn_on_finish: true,
                transition: VisualTransition::Fade, base: TransformSnapshot::default(), layered: false }
        };
        let epoch = game.runtime_epoch();
        let token = rvn_core::scene_resources::register_retired_effect(
            &mut game.state, epoch, host(rvn_ui::PageRole::Pause), payload,
            SavedEffect { presentation: None, base_resolved: false,
                clock: EffectClock { duration: 0.25, elapsed: 0.0, looping: false }, parameters },
        ).unwrap().unwrap();
        game.advance_dialogue().unwrap();
        assert!(matches!(game.step_until_interaction().unwrap(), Some(Interaction::Dialogue { text, .. }) if text == "child"));
        let kept = game.state.scene_execution.as_ref().unwrap().resources.tasks.iter()
            .find(|task| task.key == token.task.key).unwrap();
        assert_eq!(kept.effect.as_ref().unwrap().presentation.as_ref(), Some(&original));
        assert_eq!(kept.effect.as_ref().unwrap().clock.elapsed.to_bits(), 0.0f64.to_bits());
        let before = (game.state.background_image.clone(), game.state.sprites.clone(), game.state.music.clone(), game.state.random);
        let result = game.tick_host_frame(0.25).unwrap();
        assert_eq!(result.callbacks, 0);
        assert_eq!(result.completed_effects, 1);
        assert_eq!((game.state.background_image.clone(), game.state.sprites.clone(), game.state.music.clone(), game.state.random), before);
        let resources = &game.state.scene_execution.as_ref().unwrap().resources;
        assert!(resources.tasks.iter().all(|task| task.key != token.task.key));
        assert!(resources.targets.iter().all(|record| record.identity != token.target));
    }
}
