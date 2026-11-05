use rvn_core::program::PreparedNarrative;
use rvn_core::scene::{ActivationId, FrameKind};
use rvn_core::scene_resources::*;
use rvn_core::scene_tasks::TaskOwner;
use rvn_core::types::GameState;
use rvn_core::{Engine, Renderer, SpriteState};
use rvn_parser::{Hotspot, Position, Transition, Value};
use std::collections::BTreeMap;

#[derive(Default)]
struct Host;
impl Renderer for Host {
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
    let parsed = rvn_parser::parse_program(
        "scene label P\n\"parent\"\nscene label C\n\"child\"\nlabel helper\nreturn\n",
    )
    .unwrap();
    let program = PreparedNarrative::from_parsed(parsed.clone()).unwrap();
    let mut state = Engine::from_program(parsed, Host, 0).unwrap().state;
    let pc = program.label_table["P"];
    let execution = state.scene_execution.as_mut().unwrap();
    execution.enter("P", pc).unwrap();
    execution.live_position = program.position(pc + 1);
    state.pc = pc + 1;
    state
        .sprites
        .insert("hero".into(), SpriteState::new(None, Position::Center));
    state.background_image = "images/parent.png".into();
    sync_current(&mut state).unwrap();
    (program, state)
}
fn owner(state: &GameState) -> TaskOwner {
    TaskOwner::Scene {
        activation: state.scene_execution.as_ref().unwrap().owner().unwrap(),
    }
}
fn json(state: &GameState) -> serde_json::Value {
    serde_json::to_value(state).unwrap()
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
fn return_scene(program: &PreparedNarrative, state: &mut GameState) -> ResourceTransition {
    abandon_current(state).unwrap();
    let execution = state.scene_execution.as_mut().unwrap();
    assert_eq!(
        execution.peek_return_kind(false),
        Some(FrameKind::SceneCall)
    );
    let position = execution.return_to_caller(false).unwrap().unwrap();
    program.validate_position(&position).unwrap();
    state.pc = position.pc;
    execution.live_position = position;
    state.call_stack = execution.derived_call_stack();
    resume_current(state).unwrap()
}
fn motion(state: &mut GameState, target: rvn_core::motion::MotionTarget) {
    state
        .motions
        .play(target, rvn_ui::motion::Motion::Pause { seconds: 10.0 })
        .unwrap();
    state.motions.tick(2.25).unwrap();
    sync_current(state).unwrap();
}
fn screen(name: &str, order: u64, elapsed: f64) -> rvn_core::ui::ScreenInstance {
    rvn_core::ui::ScreenInstance {
        widget_class:None,
        name: name.into(),
        host_role: None,
        host_root: false,
        arguments: vec![Value::Int(7)],
        modal: false,
        layer: 0,
        order,
        focus: Some("field".into()),
        owner_properties:Default::default(),owner_focus_request:None,
        values: BTreeMap::from([("field".into(), Value::Str("parent".into()))]),
        random: rvn_core::random::RandomState::seeded(13),
        canvas_states: BTreeMap::from([(
            "canvas".into(),
            rvn_core::ui::CanvasState {
                state: Value::Dict(BTreeMap::from([("count".into(), Value::Int(11))])),
                elapsed,
            },
        )]),
        consumer_frames: BTreeMap::new(),
        consumer_projection: None,
        standard_macro_frame: None,
    }
}
fn video(state: &mut GameState, id: &str, position: f64) {
    let clip = rvn_ui::video::VideoClip::parse(
        serde_json::json!({"source":"movies/clip.webm","on_end":"finished"}),
    )
    .unwrap();
    state.videos.play(id.into(), clip).unwrap();
    state
        .videos
        .feedback(
            id,
            rvn_core::video::Feedback::Position {
                seconds: position,
                duration: 20.0,
            },
        )
        .unwrap();
    sync_current(state).unwrap();
}
fn effect(elapsed: f64) -> SavedEffect {
    SavedEffect {
        presentation: None,
        base_resolved: true,
        clock: EffectClock {
            duration: 4.0,
            elapsed,
            looping: false,
        },
        parameters: EffectParameters::SpriteAnimation {
            animation: SpriteEffectKind::Bounce { height: 17.0 },
            base: TransformSnapshot {
                translation: [3.0, 4.0, 5.0],
                scale: [-1.0, 2.0, 1.0],
                rotation: [0.0, 0.0, 0.6, 0.8],
            },
        },
    }
}

fn projection_effect(
    state: &mut GameState,
    task_owner: TaskOwner,
    alias: &str,
    kind: TargetKind,
    target_alias: &str,
    saved: SavedEffect,
) -> ResourceToken {
    let target = state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .target(&TaskOwner::Game, kind, target_alias)
        .unwrap()
        .clone();
    register_effect(
        state,
        7,
        resource_key(task_owner, ResourceTaskKind::Effect, alias),
        target,
        saved,
    )
    .unwrap()
}

#[test]
fn projection_capture_refuses_authored_parameters_family_and_ecs_clock_changes_atomically() {
    let (_, mut state) = fixture();
    let original = effect(1.0);
    let token = projection_effect(
        &mut state,
        TaskOwner::Game,
        "bounce",
        TargetKind::Sprite,
        "hero",
        original.clone(),
    );
    let active = state.scene_execution.as_ref().unwrap().owner();
    let mut mutations = Vec::new();
    let mut duration = original.clone();
    duration.clock.duration = 5.0;
    mutations.push(duration);
    let mut looping = original.clone();
    looping.clock.looping = true;
    mutations.push(looping);
    let mut elapsed = original.clone();
    elapsed.clock.elapsed = 0.5;
    mutations.push(elapsed);
    let mut elapsed = original.clone();
    elapsed.clock.elapsed = 1.5;
    mutations.push(elapsed);
    let mut height = original.clone();
    if let EffectParameters::SpriteAnimation { animation, .. } = &mut height.parameters {
        *animation = SpriteEffectKind::Bounce { height: 19.0 };
    }
    mutations.push(height);
    let mut family = original.clone();
    family.parameters = EffectParameters::VisualFade {
        from: 0.0,
        to: 1.0,
        despawn_on_finish: false,
        transition: VisualTransition::Fade,
        base: Default::default(),
        layered: false,
    };
    mutations.push(family);
    let before = serde_json::to_vec(&state).unwrap();
    for changed in mutations {
        assert!(state
            .scene_execution
            .as_mut()
            .unwrap()
            .resources
            .capture_effect(&token, 7, active, changed)
            .is_err());
        assert_eq!(serde_json::to_vec(&state).unwrap(), before);
    }

    let visual = SavedEffect {
        presentation: None,
        base_resolved: false,
        clock: original.clock.clone(),
        parameters: EffectParameters::VisualFade {
            from: -0.0,
            to: 1.0,
            despawn_on_finish: false,
            transition: VisualTransition::Fade,
            base: Default::default(),
            layered: false,
        },
    };
    let visual_token = projection_effect(
        &mut state,
        TaskOwner::Game,
        "visual",
        TargetKind::Sprite,
        "hero",
        visual.clone(),
    );
    for index in 0..5 {
        let mut changed = visual.clone();
        if let EffectParameters::VisualFade {
            from,
            to,
            despawn_on_finish,
            transition,
            layered,
            ..
        } = &mut changed.parameters
        {
            match index {
                0 => *from = 0.0,
                1 => *to = 0.5,
                2 => *despawn_on_finish = true,
                3 => *transition = VisualTransition::Dissolve,
                _ => *layered = true,
            }
        }
        let before = serde_json::to_vec(&state).unwrap();
        assert!(state
            .scene_execution
            .as_mut()
            .unwrap()
            .resources
            .capture_effect(&visual_token, 7, active, changed)
            .is_err());
        assert_eq!(serde_json::to_vec(&state).unwrap(), before);
    }
    state.music.current_file = Some("music/theme.ogg".into());
    sync_current(&mut state).unwrap();
    let audio = SavedEffect {
        presentation: None,
        base_resolved: false,
        clock: original.clock,
        parameters: EffectParameters::AudioFade {
            from: 0.25,
            to: 0.75,
            despawn_on_done: false,
        },
    };
    let audio_token = projection_effect(
        &mut state,
        TaskOwner::Game,
        "audio",
        TargetKind::Music,
        "music",
        audio.clone(),
    );
    for index in 0..3 {
        let mut changed = audio.clone();
        if let EffectParameters::AudioFade {
            from,
            to,
            despawn_on_done,
        } = &mut changed.parameters
        {
            match index {
                0 => *from = 0.0,
                1 => *to = 1.0,
                _ => *despawn_on_done = true,
            }
        }
        let before = serde_json::to_vec(&state).unwrap();
        assert!(state
            .scene_execution
            .as_mut()
            .unwrap()
            .resources
            .capture_effect(&audio_token, 7, active, changed)
            .is_err());
        assert_eq!(serde_json::to_vec(&state).unwrap(), before);
    }
}

#[test]
fn typed_projection_base_preserves_authored_clock_and_rebases_only_finite_matching_family() {
    let (_, mut state) = fixture();
    let mut authored = effect(1.0);
    authored.base_resolved = false;
    authored.clock.duration = 1.0000000000000002;
    let token = projection_effect(
        &mut state,
        TaskOwner::Game,
        "projection",
        TargetKind::Sprite,
        "hero",
        authored.clone(),
    );
    let active = state.scene_execution.as_ref().unwrap().owner();
    let registry = &mut state.scene_execution.as_mut().unwrap().resources;
    let base = TransformSnapshot {
        translation: [9.0, -2.0, 5.0],
        ..Default::default()
    };
    assert!(registry
        .capture_effect_base(
            &token,
            7,
            active,
            EffectProjectionBase::SpriteTransform(base.clone())
        )
        .unwrap());
    let installed = registry
        .tasks
        .iter()
        .find(|task| task.key == token.task.key)
        .unwrap()
        .effect
        .as_ref()
        .unwrap();
    assert!(installed.base_resolved);
    assert_eq!(
        installed.clock.duration.to_bits(),
        authored.clock.duration.to_bits()
    );
    assert_eq!(
        installed.clock.elapsed.to_bits(),
        authored.clock.elapsed.to_bits()
    );
    assert_eq!(
        installed.parameters,
        EffectParameters::SpriteAnimation {
            animation: SpriteEffectKind::Bounce { height: 17.0 },
            base: base.clone(),
        }
    );
    let mut rebase = base.clone();
    rebase.translation[0] = 3.0;
    assert!(registry
        .capture_effect_base(
            &token,
            7,
            active,
            EffectProjectionBase::SpriteTransform(rebase.clone())
        )
        .unwrap());
    let before = serde_json::to_vec(registry).unwrap();
    let mut invalid = rebase.clone();
    invalid.translation[0] = f32::NAN;
    let mut oversized = rebase.clone();
    oversized.scale[0] = 1_000_001.0;
    let mut rotation = rebase.clone();
    rotation.rotation = [0.0; 4];
    for rejected in [
        EffectProjectionBase::SpriteTransform(invalid),
        EffectProjectionBase::SpriteTransform(oversized),
        EffectProjectionBase::SpriteTransform(rotation),
        EffectProjectionBase::VisualTransform(rebase),
        EffectProjectionBase::AudioResolved,
    ] {
        assert!(registry
            .capture_effect_base(&token, 7, active, rejected)
            .is_err());
        assert_eq!(serde_json::to_vec(registry).unwrap(), before);
    }
    let mut unresolved = registry
        .tasks
        .iter()
        .find(|task| task.key == token.task.key)
        .unwrap()
        .effect
        .clone()
        .unwrap();
    unresolved.base_resolved = false;
    assert!(registry
        .capture_effect(&token, 7, active, unresolved)
        .is_err());
    assert_eq!(serde_json::to_vec(registry).unwrap(), before);
    validate_current_resource_state(&state).unwrap();
}

#[test]
fn stale_projection_base_is_neutral_before_payload_for_exact_epoch_revision_target_and_owner() {
    let (_, mut state) = fixture();
    let token = projection_effect(
        &mut state,
        TaskOwner::Game,
        "projection",
        TargetKind::Sprite,
        "hero",
        effect(1.0),
    );
    let active = state.scene_execution.as_ref().unwrap().owner();
    let mut tokens = Vec::new();
    let mut changed = token.clone();
    changed.task.runtime_epoch += 1;
    tokens.push(changed);
    let mut changed = token.clone();
    changed.task.revision += 1;
    tokens.push(changed);
    let mut changed = token.clone();
    changed.target.incarnation += 1;
    tokens.push(changed);
    let mut changed = token.clone();
    changed.task.key.owner = owner(&state);
    tokens.push(changed);
    let before = serde_json::to_vec(&state).unwrap();
    for stale in tokens {
        let invalid = TransformSnapshot {
            translation: [f32::NAN, 0.0, 0.0],
            ..Default::default()
        };
        assert!(!state
            .scene_execution
            .as_mut()
            .unwrap()
            .resources
            .capture_effect_base(
                &stale,
                7,
                active,
                EffectProjectionBase::SpriteTransform(invalid)
            )
            .unwrap());
        assert_eq!(serde_json::to_vec(&state).unwrap(), before);
        assert!(complete_due_effect(&mut state, &stale, 7)
            .unwrap()
            .is_none());
        assert_eq!(serde_json::to_vec(&state).unwrap(), before);
    }
}

#[test]
fn core_effect_clock_clamps_finite_time_preserves_duration_and_never_wraps_saved_loops() {
    let clock = EffectClock {
        duration: 1.0000000000000002,
        elapsed: 1.0,
        looping: false,
    };
    let advanced = clock.advanced(f64::MAX).unwrap();
    assert_eq!(advanced.duration.to_bits(), clock.duration.to_bits());
    assert_eq!(advanced.elapsed.to_bits(), clock.duration.to_bits());
    assert!(advanced.is_due());
    assert_eq!(clock.elapsed, 1.0);
    assert_eq!(clock.advanced(0.0).unwrap(), clock);
    let zero = EffectClock {
        duration: 0.0,
        elapsed: 0.0,
        looping: false,
    };
    assert_eq!(zero.advanced(10.0).unwrap(), zero);
    assert!(zero.is_due());
    let looping = EffectClock {
        duration: 2.0,
        elapsed: 2.0,
        looping: true,
    };
    let advanced = looping.advanced(5.0).unwrap();
    assert_eq!(advanced.elapsed, 7.0);
    assert_eq!(advanced.duration, 2.0);
    assert!(!advanced.is_due());
    assert!(looping.advanced(f64::MAX).is_err());
    let capped = EffectClock {
        elapsed: 1.0e12,
        ..looping.clone()
    };
    assert!(capped.advanced(1.0).is_err());
    assert_eq!(capped.elapsed, 1.0e12);
    for invalid in [f64::NAN, f64::INFINITY, -1.0] {
        assert!(clock.advanced(invalid).is_err());
    }
    assert!(EffectClock {
        duration: 0.0,
        elapsed: 0.0,
        looping: true
    }
    .advanced(0.0)
    .is_err());
}

#[test]
fn due_effect_completion_is_exact_once_preserves_global_models_and_refuses_early_or_looping() {
    let (_, mut state) = fixture();
    let token = projection_effect(
        &mut state,
        TaskOwner::Game,
        "due",
        TargetKind::Sprite,
        "hero",
        effect(3.0),
    );
    let active = state.scene_execution.as_ref().unwrap().owner();
    let before = serde_json::to_vec(&state).unwrap();
    assert!(complete_due_effect(&mut state, &token, 7)
        .unwrap()
        .is_none());
    assert!(!state
        .scene_execution
        .as_mut()
        .unwrap()
        .resources
        .complete_effect(&token, 7, active));
    assert_eq!(serde_json::to_vec(&state).unwrap(), before);
    let mut looping = effect(4.0);
    looping.clock.looping = true;
    let loop_token = projection_effect(
        &mut state,
        TaskOwner::Game,
        "loop",
        TargetKind::Sprite,
        "hero",
        looping,
    );
    let before = serde_json::to_vec(&state).unwrap();
    assert!(complete_due_effect(&mut state, &loop_token, 7)
        .unwrap()
        .is_none());
    assert_eq!(serde_json::to_vec(&state).unwrap(), before);
    let registered = state
        .scene_execution
        .as_mut()
        .unwrap()
        .resources
        .tasks
        .iter_mut()
        .find(|task| task.key == token.task.key)
        .unwrap();
    registered.effect.as_mut().unwrap().clock.elapsed = 4.0;
    registered.effect.as_mut().unwrap().base_resolved = false;
    let expected = registered.clone();
    let pc = state.pc;
    let random = state.random.clone();
    let sprites = state.sprites.clone();
    let background = state.background_image.clone();
    let music = state.music.clone();
    let variables = state.vars.clone();
    let other = state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .tasks
        .iter()
        .find(|task| task.key == loop_token.task.key)
        .unwrap()
        .clone();
    let result = complete_due_effect(&mut state, &token, 7).unwrap().unwrap();
    assert_eq!(result.task, expected);
    assert!(result.retired_target.is_none());
    assert_eq!(state.pc, pc);
    assert_eq!(state.random, random);
    assert_eq!(state.sprites, sprites);
    assert_eq!(state.background_image, background);
    assert_eq!(state.music, music);
    assert_eq!(state.vars, variables);
    let registry = &state.scene_execution.as_ref().unwrap().resources;
    assert_eq!(
        registry.target(&TaskOwner::Game, TargetKind::Sprite, "hero"),
        Some(&token.target)
    );
    assert!(!registry.tasks.iter().any(|task| task.key == token.task.key));
    assert_eq!(
        registry
            .tasks
            .iter()
            .find(|task| task.key == loop_token.task.key),
        Some(&other)
    );
    let before = serde_json::to_vec(&state).unwrap();
    assert!(complete_due_effect(&mut state, &token, 7)
        .unwrap()
        .is_none());
    assert_eq!(serde_json::to_vec(&state).unwrap(), before);
    validate_current_resource_state(&state).unwrap();
}

#[test]
fn terminal_scene_effect_keeps_child_homonym_and_requires_new_token_after_park_return() {
    let (program, mut state) = fixture();
    let parent_owner = owner(&state);
    let parent_token = projection_effect(
        &mut state,
        parent_owner.clone(),
        "same",
        TargetKind::Sprite,
        "hero",
        effect(4.0),
    );
    call(&program, &mut state);
    let child_owner = owner(&state);
    let child_token = projection_effect(
        &mut state,
        child_owner,
        "same",
        TargetKind::Sprite,
        "hero",
        effect(2.0),
    );
    state.sprites.get_mut("hero").unwrap().position = Position::Right;
    let before = serde_json::to_vec(&state).unwrap();
    assert!(complete_due_effect(&mut state, &parent_token, 7)
        .unwrap()
        .is_none());
    assert!(complete_due_effect(&mut state, &child_token, 7)
        .unwrap()
        .is_none());
    assert_eq!(serde_json::to_vec(&state).unwrap(), before);
    return_scene(&program, &mut state);
    let active = state.scene_execution.as_ref().unwrap().owner();
    let current = state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .effect_token(7, &parent_token.task.key, active)
        .unwrap();
    assert_ne!(current.task.revision, parent_token.task.revision);
    let before = serde_json::to_vec(&state).unwrap();
    assert!(complete_due_effect(&mut state, &parent_token, 7)
        .unwrap()
        .is_none());
    assert_eq!(serde_json::to_vec(&state).unwrap(), before);
    assert!(complete_due_effect(&mut state, &current, 7)
        .unwrap()
        .is_some());
    assert_eq!(state.sprites["hero"].position, Position::Right);
    assert_eq!(owner(&state), parent_owner);
    validate_current_resource_state(&state).unwrap();
}

#[test]
fn due_effect_validation_refuses_missing_dependency_before_any_task_cleanup() {
    let (_, mut state) = fixture();
    let token = projection_effect(
        &mut state,
        TaskOwner::Game,
        "due",
        TargetKind::Sprite,
        "hero",
        effect(4.0),
    );
    let execution = state.scene_execution.as_mut().unwrap();
    let unknown = TargetIdentity {
        kind: TargetKind::Interface,
        alias: "ghost".into(),
        incarnation: execution.resources.next_incarnation + 1,
    };
    execution
        .resources
        .tasks
        .iter_mut()
        .find(|task| task.key == token.task.key)
        .unwrap()
        .dependencies
        .push(unknown);
    let before = serde_json::to_vec(&state).unwrap();
    assert!(complete_due_effect(&mut state, &token, 7).is_err());
    assert_eq!(serde_json::to_vec(&state).unwrap(), before);
    assert!(state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .tasks
        .iter()
        .any(|task| task.key == token.task.key));
}

#[test]
fn unresolved_compat_capture_is_byte_exact_noop_and_only_measured_base_can_resolve() {
    let (_, mut state) = fixture();
    state.music.current_file = Some("music/theme.ogg".into());
    sync_current(&mut state).unwrap();
    for family in 0..3 {
        let mut authored = effect(1.25);
        authored.base_resolved = false;
        authored.clock.duration = 2.0000000000000004;
        let (kind, alias, measured) = match family {
            0 => {
                if let EffectParameters::SpriteAnimation { base, .. } = &mut authored.parameters {
                    base.translation[0] = -0.0;
                }
                (
                    TargetKind::Sprite,
                    "hero",
                    EffectProjectionBase::SpriteTransform(Default::default()),
                )
            }
            1 => {
                authored.parameters = EffectParameters::VisualFade {
                    from: 0.25,
                    to: 0.75,
                    despawn_on_finish: false,
                    transition: VisualTransition::Fade,
                    base: TransformSnapshot {
                        translation: [-0.0, 0.0, 0.0],
                        ..Default::default()
                    },
                    layered: false,
                };
                (
                    TargetKind::Background,
                    "background",
                    EffectProjectionBase::VisualTransform(Default::default()),
                )
            }
            _ => {
                authored.parameters = EffectParameters::AudioFade {
                    from: 0.25,
                    to: 0.75,
                    despawn_on_done: false,
                };
                (
                    TargetKind::Music,
                    "music",
                    EffectProjectionBase::AudioResolved,
                )
            }
        };
        let token = projection_effect(
            &mut state,
            TaskOwner::Game,
            &format!("unmeasured{family}"),
            kind,
            alias,
            authored.clone(),
        );
        let active = state.scene_execution.as_ref().unwrap().owner();
        let registry = &mut state.scene_execution.as_mut().unwrap().resources;
        let before = serde_json::to_vec(registry).unwrap();
        assert!(!registry
            .capture_effect(&token, 7, active, authored.clone())
            .unwrap());
        assert_eq!(serde_json::to_vec(registry).unwrap(), before);
        assert!(
            !registry
                .tasks
                .iter()
                .find(|task| task.key == token.task.key)
                .unwrap()
                .effect
                .as_ref()
                .unwrap()
                .base_resolved
        );
        if family < 2 {
            for changed_value in [0.0, 9.0] {
                let mut changed = authored.clone();
                match &mut changed.parameters {
                    EffectParameters::SpriteAnimation { base, .. }
                    | EffectParameters::VisualFade { base, .. } => {
                        base.translation[0] = changed_value
                    }
                    _ => unreachable!(),
                }
                assert!(registry.capture_effect(&token, 7, active, changed).is_err());
                assert_eq!(serde_json::to_vec(registry).unwrap(), before);
            }
        }
        let mut stale = token.clone();
        stale.task.revision += 1;
        let mut invalid = authored.clone();
        invalid.clock.elapsed = f64::NAN;
        assert!(!registry
            .capture_effect(&stale, 7, active, invalid.clone())
            .unwrap());
        assert_eq!(serde_json::to_vec(registry).unwrap(), before);
        assert!(registry.capture_effect(&token, 7, active, invalid).is_err());
        assert_eq!(serde_json::to_vec(registry).unwrap(), before);
        assert!(registry
            .capture_effect_base(&token, 7, active, measured)
            .unwrap());
        let resolved = registry
            .tasks
            .iter()
            .find(|task| task.key == token.task.key)
            .unwrap()
            .effect
            .as_ref()
            .unwrap();
        assert!(resolved.base_resolved);
        assert_eq!(
            resolved.clock.duration.to_bits(),
            authored.clock.duration.to_bits()
        );
        assert_eq!(
            resolved.clock.elapsed.to_bits(),
            authored.clock.elapsed.to_bits()
        );
        let after = serde_json::to_vec(registry).unwrap();
        assert!(registry
            .capture_effect(&token, 7, active, authored)
            .is_err());
        assert_eq!(serde_json::to_vec(registry).unwrap(), after);
    }
    validate_current_resource_state(&state).unwrap();
}

#[test]
fn scene_park_return_keeps_exact_clocks_ui_values_canvas_rng_and_game_edits() {
    let (program, mut state) = fixture();
    motion(
        &mut state,
        rvn_core::motion::MotionTarget::Sprite { id: "hero".into() },
    );
    video(&mut state, "movie", 3.75);
    state.ui.screens.push(screen("panel", 0, 12.5));
    state.ui.next_order = 1;
    state
        .scene_execution
        .as_mut()
        .unwrap()
        .active
        .as_mut()
        .unwrap()
        .tasks
        .set_timer(3.0, "call C".into())
        .unwrap();
    sync_current(&mut state).unwrap();
    let original_ui = state.ui.screens.clone();
    let original_motion = state.motions.clone();
    let original_video = state.videos.clone();
    let parent = owner(&state);
    call(&program, &mut state);
    assert!(
        state.ui.screens.is_empty()
            && state.videos.tracks.is_empty()
            && state.motions.tracks.is_empty()
    );
    state.vars.insert("global".into(), Value::Int(9));
    state.sprites.get_mut("hero").unwrap().position = Position::Right;
    state.background_image = "images/child.png".into();
    state.music.current_file = Some("music/child.ogg".into());
    return_scene(&program, &mut state);
    assert_eq!(owner(&state), parent);
    assert_eq!(state.ui.screens, original_ui);
    assert_eq!(state.motions, original_motion);
    assert_eq!(state.videos, original_video);
    assert_eq!(state.vars["global"], Value::Int(9));
    assert_eq!(state.background_image, "images/child.png");
    assert_eq!(state.music.current_file.as_deref(), Some("music/child.ogg"));
    assert_eq!(state.sprites["hero"].position, Position::Right);
    let execution = state.scene_execution.as_ref().unwrap();
    assert_eq!(
        execution
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
    execution
        .validate(&program, &state.vars, &state.call_stack)
        .unwrap();
}

#[test]
fn parent_and_child_homonym_video_interface_canvas_are_distinct_incarnations() {
    let (program, mut state) = fixture();
    video(&mut state, "same", 2.0);
    state.ui.screens.push(screen("same", 0, 4.0));
    state.ui.next_order = 1;
    sync_current(&mut state).unwrap();
    let parent = owner(&state);
    let parent_video = state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .target(&parent, TargetKind::Video, "same")
        .unwrap()
        .clone();
    let parent_ui = state.ui.screens[0].clone();
    call(&program, &mut state);
    video(&mut state, "same", 8.0);
    state.ui.screens.push(screen("same", 1, 19.0));
    state.ui.next_order = 2;
    sync_current(&mut state).unwrap();
    let child = owner(&state);
    let child_video = state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .target(&child, TargetKind::Video, "same")
        .unwrap()
        .clone();
    assert_ne!(parent_video.incarnation, child_video.incarnation);
    return_scene(&program, &mut state);
    assert_eq!(state.videos.tracks["same"].position, 2.0);
    assert_eq!(state.ui.screens, vec![parent_ui]);
    assert!(state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .target(&child, TargetKind::Video, "same")
        .is_none());
}

#[test]
fn cancelling_parked_parent_targets_preserves_live_child_homonyms_and_exact_wait() {
    let (program, mut state) = fixture();
    video(&mut state, "same", 2.0);
    state.videos.wait("same").unwrap();
    state.ui.screens.push(screen("same", 0, 4.0));
    state.ui.next_order = 1;
    sync_current(&mut state).unwrap();
    let parent = owner(&state);
    let position = state
        .scene_execution
        .as_ref()
        .unwrap()
        .live_position
        .clone();
    let parent_target = state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .target(&parent, TargetKind::Video, "same")
        .unwrap()
        .clone();
    call(&program, &mut state);
    video(&mut state, "same", 8.0);
    state.ui.screens.push(screen("same", 1, 19.0));
    state.ui.next_order = 2;
    sync_current(&mut state).unwrap();
    let child = owner(&state);
    let child_videos = state.videos.clone();
    let child_ui = state.ui.clone();
    let registry = &state.scene_execution.as_ref().unwrap().resources;
    let child_tasks: Vec<_> = registry
        .tasks
        .iter()
        .filter(|task| task.key.owner == child)
        .cloned()
        .collect();
    let child_targets: Vec<_> = registry
        .targets
        .iter()
        .filter(|target| target.owner == child)
        .cloned()
        .collect();
    assert!(
        remove_target(&mut state, &parent, TargetKind::Video, "same")
            .unwrap()
            .cancelled_waits
            .is_empty()
    );
    remove_target(&mut state, &parent, TargetKind::Interface, "same").unwrap();
    assert_eq!(state.videos, child_videos);
    assert_eq!(state.ui, child_ui);
    let execution = state.scene_execution.as_ref().unwrap();
    assert_eq!(
        execution
            .resources
            .tasks
            .iter()
            .filter(|task| task.key.owner == child)
            .cloned()
            .collect::<Vec<_>>(),
        child_tasks
    );
    assert_eq!(
        execution
            .resources
            .targets
            .iter()
            .filter(|target| target.owner == child)
            .cloned()
            .collect::<Vec<_>>(),
        child_targets
    );
    let parent_tasks = &execution.frames[0].parked.as_ref().unwrap().tasks;
    assert!(parent_tasks.suspended_videos.tracks.is_empty());
    assert!(parent_tasks.suspended_videos.waiting.is_none());
    assert!(parent_tasks.suspended_ui.screens.is_empty());
    assert_eq!(execution.resources.cancelled_waits.len(), 1);
    let returned = return_scene(&program, &mut state);
    assert_eq!(returned.cancelled_waits.len(), 1);
    let receipt = &returned.cancelled_waits[0];
    assert_eq!(receipt.key.owner, parent);
    assert_eq!(receipt.target, parent_target);
    assert_eq!(receipt.position, position);
    assert_eq!(receipt.reason, CancellationReason::TargetRemoved);
    assert_eq!(state.pc, position.pc);
    assert!(state.videos.tracks.is_empty() && state.videos.waiting.is_none());
    assert!(state.ui.screens.is_empty());
    assert!(state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .cancelled_waits
        .is_empty());
}

#[test]
fn renewal_preserves_game_clock_and_updates_every_suspended_caller_lease_atomically() {
    let (program, mut state) = fixture();
    let parent = state
        .scene_execution
        .as_mut()
        .unwrap()
        .active
        .take()
        .unwrap();
    video(&mut state, "shared", 3.5);
    state.scene_execution.as_mut().unwrap().active = Some(parent);
    state.videos.wait("shared").unwrap();
    sync_current(&mut state).unwrap();
    call(&program, &mut state);
    state.videos.wait("shared").unwrap();
    sync_current(&mut state).unwrap();
    call(&program, &mut state);
    let key = resource_key(TaskOwner::Game, ResourceTaskKind::Video, "shared");
    let before = state.videos.clone();
    let execution = state.scene_execution.as_mut().unwrap();
    let old_task = execution
        .resources
        .tasks
        .iter()
        .find(|task| task.key == key)
        .unwrap()
        .clone();
    let old_token = execution
        .resources
        .effect_token(11, &key, execution.owner())
        .unwrap();
    let old_leases = execution.resources.waits.clone();
    assert_eq!(old_leases.len(), 2);
    assert!(execution.resources.renew_task(&key).unwrap());
    let task = execution
        .resources
        .tasks
        .iter()
        .find(|task| task.key == key)
        .unwrap();
    assert!(task.revision > old_task.revision);
    let mut expected = old_task;
    expected.revision = task.revision;
    assert_eq!(task, &expected);
    assert!(!execution
        .resources
        .accepts(&old_token, 11, execution.owner()));
    let new_token = execution
        .resources
        .effect_token(11, &key, execution.owner())
        .unwrap();
    assert!(execution
        .resources
        .accepts(&new_token, 11, execution.owner()));
    for (old, new) in old_leases.iter().zip(&execution.resources.waits) {
        let mut expected = old.clone();
        expected.revision = new_token.task.revision;
        assert_eq!(new, &expected);
    }
    assert!(execution.resources.cancelled_waits.is_empty());
    assert_eq!(state.videos, before);
    return_scene(&program, &mut state);
    return_scene(&program, &mut state);
    assert_eq!(state.videos.tracks, before.tracks);
    assert_eq!(state.videos.waiting.as_deref(), Some("shared"));
    let registry = &mut state.scene_execution.as_mut().unwrap().resources;
    registry.next_revision = u64::MAX;
    let snapshot = registry.clone();
    assert!(registry.renew_task(&key).is_err());
    assert_eq!(*registry, snapshot);
    let absent = resource_key(TaskOwner::Game, ResourceTaskKind::Video, "absent");
    assert!(!registry.renew_task(&absent).unwrap());
    assert_eq!(*registry, snapshot);
}

#[test]
fn renewal_of_parked_scene_operation_keeps_snapshot_and_wait_until_real_return() {
    let (program, mut state) = fixture();
    video(&mut state, "movie", 6.25);
    state.videos.wait("movie").unwrap();
    sync_current(&mut state).unwrap();
    let parent = owner(&state);
    let key = resource_key(parent, ResourceTaskKind::Video, "movie");
    call(&program, &mut state);
    let execution = state.scene_execution.as_mut().unwrap();
    let snapshot = execution.frames[0].parked.as_ref().unwrap().tasks.clone();
    let target = execution
        .resources
        .tasks
        .iter()
        .find(|task| task.key == key)
        .unwrap()
        .target
        .clone();
    assert!(execution.resources.renew_task(&key).unwrap());
    let task = execution
        .resources
        .tasks
        .iter()
        .find(|task| task.key == key)
        .unwrap();
    assert!(task.parked);
    assert_eq!(task.target, target);
    assert_eq!(execution.resources.waits[0].revision, task.revision);
    assert!(execution.resources.waits[0].parked);
    assert!(execution
        .resources
        .effect_token(11, &key, execution.owner())
        .is_none());
    assert_eq!(execution.frames[0].parked.as_ref().unwrap().tasks, snapshot);
    assert!(execution.resources.cancelled_waits.is_empty());
    return_scene(&program, &mut state);
    assert_eq!(state.videos.tracks["movie"].position, 6.25);
    assert_eq!(state.videos.waiting.as_deref(), Some("movie"));
    validate_current_resource_state(&state).unwrap();
}

#[test]
fn game_video_terminal_feedback_releases_all_parked_waiters_without_cancellation_or_replay() {
    for failed in [false, true] {
        let (program, mut state) = fixture();
        let parent = state
            .scene_execution
            .as_mut()
            .unwrap()
            .active
            .take()
            .unwrap();
        video(&mut state, "shared", 3.5);
        state.scene_execution.as_mut().unwrap().active = Some(parent);
        state.videos.wait("shared").unwrap();
        sync_current(&mut state).unwrap();
        call(&program, &mut state);
        state.videos.wait("shared").unwrap();
        sync_current(&mut state).unwrap();
        call(&program, &mut state);
        let execution = state.scene_execution.as_ref().unwrap();
        let key = resource_key(TaskOwner::Game, ResourceTaskKind::Video, "shared");
        let token = execution
            .resources
            .effect_token(11, &key, execution.owner())
            .unwrap();
        assert_eq!(execution.resources.waits.len(), 2);
        let pc = state.pc;
        let call_stack = state.call_stack.clone();
        let variables = state.vars.clone();
        state
            .videos
            .feedback(
                "shared",
                if failed {
                    rvn_core::video::Feedback::Error("decoder stopped".into())
                } else {
                    rvn_core::video::Feedback::End
                },
            )
            .unwrap();
        let terminal = state.videos.tracks["shared"].clone();
        assert!(terminal.finished());
        assert!(release_waiters(&mut state, &token, 11).unwrap());
        assert_eq!(state.pc, pc);
        assert_eq!(state.call_stack, call_stack);
        assert_eq!(state.vars, variables);
        let execution = state.scene_execution.as_ref().unwrap();
        assert!(
            execution.resources.waits.is_empty() && execution.resources.cancelled_waits.is_empty()
        );
        assert!(execution.resources.accepts(&token, 11, execution.owner()));
        assert!(execution
            .resources
            .tasks
            .iter()
            .find(|task| task.key == key)
            .unwrap()
            .wait
            .is_none());
        for frame in &execution.frames {
            assert!(frame
                .parked
                .as_ref()
                .unwrap()
                .tasks
                .suspended_videos
                .waiting
                .is_none());
        }
        return_scene(&program, &mut state);
        return_scene(&program, &mut state);
        assert_eq!(state.videos.tracks["shared"], terminal);
        assert!(state.videos.waiting.is_none());
        assert_eq!(state.vars, variables);
        let saved = json(&state);
        assert!(release_waiters(&mut state, &token, 11).unwrap());
        assert_eq!(json(&state), saved);
        validate_current_resource_state(&state).unwrap();
    }
}

#[test]
fn game_motion_completion_releases_previsit_and_scene_waiters_with_stale_token_neutral() {
    let (program, mut state) = fixture();
    state.scene_execution.as_mut().unwrap().active = None;
    motion(
        &mut state,
        rvn_core::motion::MotionTarget::Sprite { id: "hero".into() },
    );
    state.motions.waiting = Some("sprite:hero".into());
    sync_current(&mut state).unwrap();
    let key = resource_key(TaskOwner::Game, ResourceTaskKind::Motion, "sprite:hero");
    let execution = state.scene_execution.as_ref().unwrap();
    let old = execution
        .resources
        .effect_token(11, &key, execution.owner())
        .unwrap();
    let before = json(&state);
    assert!(release_waiters(&mut state, &old, 11).is_err());
    assert_eq!(json(&state), before);
    call(&program, &mut state);
    state.motions.waiting = Some("sprite:hero".into());
    sync_current(&mut state).unwrap();
    call(&program, &mut state);
    state.motions.tick(7.75).unwrap();
    assert!(!state.motions.tracks["sprite:hero"].running);
    let execution = state.scene_execution.as_mut().unwrap();
    assert_eq!(execution.resources.waits.len(), 2);
    assert!(execution.resources.renew_task(&key).unwrap());
    let current = execution
        .resources
        .effect_token(11, &key, execution.owner())
        .unwrap();
    let before = json(&state);
    assert!(!release_waiters(&mut state, &old, 11).unwrap());
    assert!(!release_waiters(&mut state, &current, 12).unwrap());
    assert_eq!(json(&state), before);
    let clock = state.motions.tracks["sprite:hero"].clone();
    assert!(release_waiters(&mut state, &current, 11).unwrap());
    let execution = state.scene_execution.as_ref().unwrap();
    assert!(execution.resources.waits.is_empty() && execution.resources.cancelled_waits.is_empty());
    assert!(execution.frames[0]
        .pre_scene_resources
        .as_ref()
        .unwrap()
        .suspended_motions
        .waiting
        .is_none());
    assert!(execution.frames[1]
        .parked
        .as_ref()
        .unwrap()
        .tasks
        .suspended_motions
        .waiting
        .is_none());
    return_scene(&program, &mut state);
    return_scene(&program, &mut state);
    assert_eq!(state.scene_execution.as_ref().unwrap().owner(), None);
    assert_eq!(state.motions.tracks["sprite:hero"], clock);
    assert!(state.motions.waiting.is_none());
    validate_current_resource_state(&state).unwrap();
}

#[test]
fn hide_show_replacement_cancels_parked_motion_and_exact_wait_without_completion() {
    let (program, mut state) = fixture();
    motion(
        &mut state,
        rvn_core::motion::MotionTarget::Sprite { id: "hero".into() },
    );
    state.motions.waiting = Some("sprite:hero".into());
    sync_current(&mut state).unwrap();
    let parent = owner(&state);
    let position = state
        .scene_execution
        .as_ref()
        .unwrap()
        .live_position
        .clone();
    let old = state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .target(&TaskOwner::Game, TargetKind::Sprite, "hero")
        .unwrap()
        .clone();
    call(&program, &mut state);
    remove_target(&mut state, &TaskOwner::Game, TargetKind::Sprite, "hero").unwrap();
    state.sprites.get_mut("hero").unwrap().visible = false;
    state.sprites.get_mut("hero").unwrap().visible = true;
    sync_current(&mut state).unwrap();
    let new = state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .target(&TaskOwner::Game, TargetKind::Sprite, "hero")
        .unwrap()
        .clone();
    assert_ne!(old, new);
    let encoded = serde_json::to_vec(&state).unwrap();
    state = serde_json::from_slice(&encoded).unwrap();
    let transition = return_scene(&program, &mut state);
    assert_eq!(transition.cancelled_waits.len(), 1);
    let wait = &transition.cancelled_waits[0];
    assert_eq!(wait.key.owner, parent);
    assert_eq!(wait.target, old);
    assert_eq!(wait.position, position);
    assert_eq!(wait.reason, CancellationReason::TargetRemoved);
    assert!(state.motions.tracks.is_empty() && state.motions.waiting.is_none());
    assert_eq!(state.pc, position.pc);
    assert!(state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .cancelled_waits
        .is_empty());
}

#[test]
fn identical_background_and_music_replacement_revokes_old_incarnations_not_aliases() {
    let (program, mut state) = fixture();
    state.music.current_file = Some("music/same.ogg".into());
    sync_current(&mut state).unwrap();
    motion(&mut state, rvn_core::motion::MotionTarget::Background);
    let old_background = state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .target(&TaskOwner::Game, TargetKind::Background, "background")
        .unwrap()
        .clone();
    let old_music = state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .target(&TaskOwner::Game, TargetKind::Music, "music")
        .unwrap()
        .clone();
    call(&program, &mut state);
    replace_target(
        &mut state,
        TaskOwner::Game,
        TargetKind::Background,
        "background",
    )
    .unwrap();
    replace_target(&mut state, TaskOwner::Game, TargetKind::Music, "music").unwrap();
    return_scene(&program, &mut state);
    let registry = &state.scene_execution.as_ref().unwrap().resources;
    assert_ne!(
        registry.target(&TaskOwner::Game, TargetKind::Background, "background"),
        Some(&old_background)
    );
    assert_ne!(
        registry.target(&TaskOwner::Game, TargetKind::Music, "music"),
        Some(&old_music)
    );
    assert!(state.motions.tracks.is_empty());
    assert_eq!(state.background_image, "images/parent.png");
    assert_eq!(state.music.current_file.as_deref(), Some("music/same.ogg"));
}

#[test]
fn game_effect_token_continues_call_while_scene_effect_token_is_revoked_on_park_return() {
    let (program, mut state) = fixture();
    let parent = owner(&state);
    let target = state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .target(&TaskOwner::Game, TargetKind::Sprite, "hero")
        .unwrap()
        .clone();
    let scene_key = resource_key(parent.clone(), ResourceTaskKind::Effect, "bounce");
    let game_key = resource_key(TaskOwner::Game, ResourceTaskKind::Effect, "global_bounce");
    let scene_token = register_effect(
        &mut state,
        7,
        scene_key.clone(),
        target.clone(),
        effect(1.25),
    )
    .unwrap();
    let game_token = register_effect(&mut state, 7, game_key, target, effect(0.5)).unwrap();
    call(&program, &mut state);
    let execution = state.scene_execution.as_ref().unwrap();
    assert!(!execution
        .resources
        .accepts(&scene_token, 7, execution.owner()));
    assert!(execution
        .resources
        .accepts(&game_token, 7, execution.owner()));
    return_scene(&program, &mut state);
    let execution = state.scene_execution.as_ref().unwrap();
    assert!(!execution
        .resources
        .accepts(&scene_token, 7, execution.owner()));
    assert!(execution
        .resources
        .accepts(&game_token, 7, execution.owner()));
    let renewed = execution
        .resources
        .effect_token(7, &scene_key, execution.owner())
        .unwrap();
    assert_ne!(renewed.task.revision, scene_token.task.revision);
    let saved = &execution
        .resources
        .tasks
        .iter()
        .find(|task| task.key == scene_key)
        .unwrap()
        .effect;
    assert_eq!(saved, &Some(effect(1.25)));
    assert!(!execution.resources.accepts(&renewed, 8, execution.owner()));
}

#[test]
fn explicit_sprite_animation_supersession_keeps_composable_motion_and_game_position() {
    let (program, mut state) = fixture();
    motion(
        &mut state,
        rvn_core::motion::MotionTarget::Sprite { id: "hero".into() },
    );
    let parent = owner(&state);
    let target = state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .target(&TaskOwner::Game, TargetKind::Sprite, "hero")
        .unwrap()
        .clone();
    let key = resource_key(parent, ResourceTaskKind::Effect, "bounce");
    register_effect(&mut state, 0, key.clone(), target.clone(), effect(1.0)).unwrap();
    call(&program, &mut state);
    state.sprites.get_mut("hero").unwrap().position = Position::Left;
    supersede_task(&mut state, &key).unwrap();
    return_scene(&program, &mut state);
    assert_eq!(state.motions.tracks["sprite:hero"].elapsed, 2.25);
    assert_eq!(state.sprites["hero"].position, Position::Left);
    let registry = &state.scene_execution.as_ref().unwrap().resources;
    assert_eq!(
        registry.target(&TaskOwner::Game, TargetKind::Sprite, "hero"),
        Some(&target)
    );
    assert!(!registry.tasks.iter().any(|task| task.key == key));
}

#[test]
fn abandoned_child_resources_do_not_delete_live_game_or_host_resources() {
    let (_, mut state) = fixture();
    let mut hud = screen("hud", 0, 5.0);
    hud.host_role = Some(rvn_ui::PageRole::Pause);
    hud.host_root = true;
    state.ui.screens.push(hud.clone());
    state.ui.host_next_order = 1;
    sync_current(&mut state).unwrap();
    let game_target = state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .target(&TaskOwner::Game, TargetKind::Sprite, "hero")
        .unwrap()
        .clone();
    let key = resource_key(TaskOwner::Game, ResourceTaskKind::Effect, "game_effect");
    let token = register_effect(&mut state, 3, key, game_target.clone(), effect(1.0)).unwrap();
    video(&mut state, "scene_video", 3.0);
    abandon_current(&mut state).unwrap();
    assert!(state.videos.tracks.is_empty());
    assert_eq!(state.ui.screens, vec![hud]);
    let execution = state.scene_execution.as_ref().unwrap();
    assert!(execution.resources.accepts(&token, 3, execution.owner()));
    assert_eq!(
        execution
            .resources
            .target(&TaskOwner::Game, TargetKind::Sprite, "hero"),
        Some(&game_target)
    );
    let saved = execution.resources.story_only();
    assert!(!saved
        .targets
        .iter()
        .any(|target| matches!(target.owner, TaskOwner::Host { .. })));
    assert!(!saved
        .tasks
        .iter()
        .any(|task| matches!(task.key.owner, TaskOwner::Host { .. })));
}

#[test]
fn failed_return_collision_keeps_candidate_and_parked_snapshot_byte_exact() {
    let (program, mut state) = fixture();
    video(&mut state, "movie", 1.5);
    call(&program, &mut state);
    let execution = state.scene_execution.as_mut().unwrap();
    let position = execution.return_to_caller(false).unwrap().unwrap();
    execution.live_position = position;
    let clip =
        rvn_ui::video::VideoClip::parse(serde_json::json!({"source":"movies/other.webm"})).unwrap();
    state.videos.play("movie".into(), clip).unwrap();
    let before = json(&state);
    assert!(resume_current(&mut state)
        .unwrap_err()
        .contains("overwritten"));
    assert_eq!(json(&state), before);
}

#[test]
fn aggregate_video_and_canvas_caps_include_hidden_callers_without_partial_park() {
    let (program, mut state) = fixture();
    for index in 0..8 {
        video(&mut state, &format!("v{index}"), 1.0);
    }
    call(&program, &mut state);
    let clip =
        rvn_ui::video::VideoClip::parse(serde_json::json!({"source":"movies/child.webm"})).unwrap();
    state.videos.play("extra".into(), clip).unwrap();
    let before = json(&state);
    assert!(sync_current(&mut state).is_err());
    assert_eq!(json(&state), before);
    let (program, mut state) = fixture();
    let mut parent = screen("parent", 0, 0.0);
    parent.canvas_states.clear();
    for index in 0..32 {
        parent.canvas_states.insert(
            format!("c{index}"),
            rvn_core::ui::CanvasState {
                state: Value::Dict(BTreeMap::new()),
                elapsed: 1.0,
            },
        );
    }
    state.ui.screens.push(parent);
    state.ui.next_order = 1;
    sync_current(&mut state).unwrap();
    call(&program, &mut state);
    state.ui.screens.push(screen("child", 1, 0.0));
    state.ui.next_order = 2;
    let before = json(&state);
    assert!(sync_current(&mut state).unwrap_err().contains("Aggregate"));
    assert_eq!(json(&state), before);
}

#[test]
fn aggregate_bindings_include_parked_visits_and_refuse_invalid_canvas_clock() {
    let (program, mut state) = fixture();
    call(&program, &mut state);
    let parked = state
        .scene_execution
        .as_mut()
        .unwrap()
        .frames
        .last_mut()
        .unwrap()
        .parked
        .as_mut()
        .unwrap();
    parked.locals = (0..4096)
        .map(|index| (format!("v{index}"), Value::Int(1)))
        .collect();
    state.vars.insert("extra".into(), Value::Int(1));
    assert!(validate_current_resource_state(&state)
        .unwrap_err()
        .contains("4096"));
    state.vars.clear();
    state
        .scene_execution
        .as_mut()
        .unwrap()
        .frames
        .last_mut()
        .unwrap()
        .parked
        .as_mut()
        .unwrap()
        .locals
        .clear();
    state.ui.screens.push(screen("bad", 0, f64::INFINITY));
    state.ui.next_order = 1;
    assert!(validate_current_resource_state(&state)
        .unwrap_err()
        .contains("elapsed"));
}

#[test]
fn identity_revision_overflow_and_forged_effect_snapshot_refuse_without_mutation() {
    let (_, mut state) = fixture();
    let before = json(&state);
    state
        .scene_execution
        .as_mut()
        .unwrap()
        .resources
        .next_incarnation = u64::MAX;
    let overflow_before = json(&state);
    assert!(replace_target(&mut state, TaskOwner::Game, TargetKind::Sprite, "hero").is_err());
    assert_eq!(json(&state), overflow_before);
    state
        .scene_execution
        .as_mut()
        .unwrap()
        .resources
        .next_incarnation = before["scene_execution"]["resources"]["next_incarnation"]
        .as_u64()
        .unwrap();
    let target = state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .target(&TaskOwner::Game, TargetKind::Sprite, "hero")
        .unwrap()
        .clone();
    let key = resource_key(owner(&state), ResourceTaskKind::Effect, "bounce");
    let token = register_effect(&mut state, 1, key, target, effect(1.0)).unwrap();
    let execution = state.scene_execution.as_mut().unwrap();
    let owner = execution.owner();
    let mut bad = effect(1.0);
    if let EffectParameters::SpriteAnimation { base, .. } = &mut bad.parameters {
        base.rotation = [0.0; 4];
    }
    let registry_before = execution.resources.clone();
    assert!(execution
        .resources
        .capture_effect(&token, 1, owner, bad)
        .is_err());
    assert_eq!(execution.resources, registry_before);
    let mut stale = token.clone();
    stale.target.incarnation += 1;
    assert!(!execution
        .resources
        .capture_effect(&stale, 1, owner, effect(2.0))
        .unwrap());
    assert_eq!(execution.resources, registry_before);
}

#[test]
fn initial_scene_entry_abandons_previsit_temporaries_but_call_from_none_restores_them() {
    let (program, state) = fixture();
    let mut execution = rvn_core::scene::SceneExecution::new(&program);
    execution
        .pre_scene_temporaries
        .insert("iteration".into(), Value::Int(4));
    execution.enter("P", program.label_table["P"]).unwrap();
    assert!(execution.pre_scene_temporaries.is_empty());
    execution = rvn_core::scene::SceneExecution::new(&program);
    execution
        .pre_scene_temporaries
        .insert("iteration".into(), Value::Int(4));
    execution
        .call_scene("C", program.label_table["C"], program.position(1))
        .unwrap();
    assert!(execution.pre_scene_temporaries.is_empty());
    execution.return_to_caller(false).unwrap();
    assert_eq!(execution.pre_scene_temporaries["iteration"], Value::Int(4));
    assert_eq!(execution.owner(), None);
    assert_eq!(
        state.scene_execution.unwrap().owner(),
        Some(ActivationId(1))
    );
}

#[test]
fn scene_wait_on_game_clock_parks_only_its_continuation_and_restores_exact_flag() {
    let (program, mut state) = fixture();
    let parent = state
        .scene_execution
        .as_mut()
        .unwrap()
        .active
        .take()
        .unwrap();
    motion(
        &mut state,
        rvn_core::motion::MotionTarget::Sprite { id: "hero".into() },
    );
    state.scene_execution.as_mut().unwrap().active = Some(parent);
    state.motions.waiting = Some("sprite:hero".into());
    sync_current(&mut state).unwrap();
    let key = resource_key(TaskOwner::Game, ResourceTaskKind::Motion, "sprite:hero");
    let token = state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .effect_token(4, &key, state.scene_execution.as_ref().unwrap().owner())
        .unwrap();
    call(&program, &mut state);
    assert_eq!(state.motions.waiting, None);
    assert_eq!(state.motions.tracks["sprite:hero"].elapsed, 2.25);
    let execution = state.scene_execution.as_ref().unwrap();
    assert!(execution.resources.accepts(&token, 4, execution.owner()));
    assert!(execution.frames[0]
        .parked
        .as_ref()
        .unwrap()
        .tasks
        .suspended_motions
        .tracks
        .is_empty());
    assert_eq!(
        execution.frames[0]
            .parked
            .as_ref()
            .unwrap()
            .tasks
            .suspended_motions
            .waiting
            .as_deref(),
        Some("sprite:hero")
    );
    state.motions.tick(1.5).unwrap();
    return_scene(&program, &mut state);
    assert_eq!(state.motions.tracks["sprite:hero"].elapsed, 3.75);
    assert_eq!(state.motions.waiting.as_deref(), Some("sprite:hero"));
    let execution = state.scene_execution.as_ref().unwrap();
    assert!(execution.resources.accepts(&token, 4, execution.owner()));
    assert_eq!(execution.resources.waits.len(), 1);
    assert!(!execution.resources.waits[0].parked);
}

#[test]
fn two_suspended_callers_waiting_one_game_operation_receive_only_their_exact_cancellation() {
    let (program, mut state) = fixture();
    let parent = state
        .scene_execution
        .as_mut()
        .unwrap()
        .active
        .take()
        .unwrap();
    motion(
        &mut state,
        rvn_core::motion::MotionTarget::Sprite { id: "hero".into() },
    );
    state.scene_execution.as_mut().unwrap().active = Some(parent);
    let first = state.scene_execution.as_ref().unwrap().owner();
    state.motions.waiting = Some("sprite:hero".into());
    sync_current(&mut state).unwrap();
    call(&program, &mut state);
    let second = state.scene_execution.as_ref().unwrap().owner();
    state.motions.waiting = Some("sprite:hero".into());
    sync_current(&mut state).unwrap();
    call(&program, &mut state);
    assert_eq!(
        state
            .scene_execution
            .as_ref()
            .unwrap()
            .resources
            .waits
            .len(),
        2
    );
    remove_target(&mut state, &TaskOwner::Game, TargetKind::Sprite, "hero").unwrap();
    state.sprites.get_mut("hero").unwrap().visible = false;
    assert_eq!(
        state
            .scene_execution
            .as_ref()
            .unwrap()
            .resources
            .cancelled_waits
            .len(),
        2
    );
    let second_receipt = return_scene(&program, &mut state);
    assert_eq!(second_receipt.cancelled_waits.len(), 1);
    assert_eq!(second_receipt.cancelled_waits[0].owner, second);
    let first_receipt = return_scene(&program, &mut state);
    assert_eq!(first_receipt.cancelled_waits.len(), 1);
    assert_eq!(first_receipt.cancelled_waits[0].owner, first);
    assert_eq!(first_receipt.cancelled_waits[0].key.owner, TaskOwner::Game);
    assert!(state.motions.waiting.is_none());
    assert!(state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .cancelled_waits
        .is_empty());
}

#[test]
fn previsit_wait_survives_scene_call_without_parking_game_motion_or_timer() {
    let (program, mut state) = fixture();
    state.scene_execution.as_mut().unwrap().active = None;
    motion(
        &mut state,
        rvn_core::motion::MotionTarget::Sprite { id: "hero".into() },
    );
    state.motions.waiting = Some("sprite:hero".into());
    sync_current(&mut state).unwrap();
    state
        .scene_execution
        .as_mut()
        .unwrap()
        .game_tasks
        .set_timer(10.0, "call C".into())
        .unwrap();
    let timer = state
        .scene_execution
        .as_ref()
        .unwrap()
        .game_tasks
        .timer
        .clone();
    call(&program, &mut state);
    assert!(state.motions.waiting.is_none());
    assert_eq!(state.motions.tracks["sprite:hero"].elapsed, 2.25);
    assert_eq!(
        state.scene_execution.as_ref().unwrap().game_tasks.timer,
        timer
    );
    assert!(state.scene_execution.as_ref().unwrap().frames[0]
        .pre_scene_resources
        .as_ref()
        .unwrap()
        .suspended_motions
        .tracks
        .is_empty());
    state.motions.tick(0.5).unwrap();
    return_scene(&program, &mut state);
    assert_eq!(state.scene_execution.as_ref().unwrap().owner(), None);
    assert_eq!(state.motions.tracks["sprite:hero"].elapsed, 2.75);
    assert_eq!(state.motions.waiting.as_deref(), Some("sprite:hero"));
    assert_eq!(
        state.scene_execution.as_ref().unwrap().game_tasks.timer,
        timer
    );
}

#[test]
fn suspended_resource_without_matching_registry_task_is_rejected_before_restore() {
    let (program, mut state) = fixture();
    video(&mut state, "movie", 1.5);
    call(&program, &mut state);
    let task = state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .tasks
        .iter()
        .find(|task| task.kind == ResourceTaskKind::Video)
        .unwrap()
        .key
        .clone();
    state
        .scene_execution
        .as_mut()
        .unwrap()
        .resources
        .tasks
        .retain(|entry| entry.key != task);
    let before = json(&state);
    assert!(validate_current_resource_state(&state)
        .unwrap_err()
        .contains("exact player"));
    assert_eq!(json(&state), before);
}

#[test]
fn abandoning_previsit_revokes_only_its_wait_not_the_live_game_clock_or_timer() {
    let (program, mut state) = fixture();
    state.scene_execution.as_mut().unwrap().active = None;
    motion(
        &mut state,
        rvn_core::motion::MotionTarget::Sprite { id: "hero".into() },
    );
    state.motions.waiting = Some("sprite:hero".into());
    sync_current(&mut state).unwrap();
    state
        .scene_execution
        .as_mut()
        .unwrap()
        .game_tasks
        .set_timer(10.0, "call C".into())
        .unwrap();
    let track = state.motions.tracks["sprite:hero"].clone();
    let timer = state
        .scene_execution
        .as_ref()
        .unwrap()
        .game_tasks
        .timer
        .clone();
    let result = abandon_current(&mut state).unwrap();
    assert_eq!(result.cancelled_waits.len(), 1);
    assert_eq!(result.cancelled_waits[0].owner, None);
    assert_eq!(
        result.cancelled_waits[0].reason,
        CancellationReason::OwnerAbandoned
    );
    assert_eq!(state.motions.waiting, None);
    assert_eq!(state.motions.tracks["sprite:hero"], track);
    assert_eq!(
        state.scene_execution.as_ref().unwrap().game_tasks.timer,
        timer
    );
    let pc = program.label_table["C"];
    state
        .scene_execution
        .as_mut()
        .unwrap()
        .enter("C", pc)
        .unwrap();
    state.scene_execution.as_mut().unwrap().live_position = program.position(pc + 1);
    state.pc = pc + 1;
    sync_current(&mut state).unwrap();
    assert!(state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .waits
        .is_empty());
    assert_eq!(state.motions.tracks["sprite:hero"], track);
}

#[test]
fn aggregate_motion_cap_counts_live_game_and_parked_scene_clocks_atomically() {
    let (program, mut state) = fixture();
    for index in 0..64 {
        let id = format!("s{index}");
        state
            .sprites
            .insert(id.clone(), SpriteState::new(None, Position::Center));
        state
            .motions
            .play(
                rvn_core::motion::MotionTarget::Sprite { id },
                rvn_ui::motion::Motion::Pause { seconds: 10.0 },
            )
            .unwrap();
    }
    sync_current(&mut state).unwrap();
    call(&program, &mut state);
    state
        .motions
        .play(
            rvn_core::motion::MotionTarget::Sprite { id: "hero".into() },
            rvn_ui::motion::Motion::Pause { seconds: 10.0 },
        )
        .unwrap();
    let before = json(&state);
    assert!(sync_current(&mut state).is_err());
    assert_eq!(json(&state), before);
    assert_eq!(
        state.scene_execution.as_ref().unwrap().frames[0]
            .parked
            .as_ref()
            .unwrap()
            .tasks
            .suspended_motions
            .tracks
            .len(),
        64
    );
}

fn resident_game_video(state: &mut GameState, alias: &str, screen: &str) -> ResourceToken {
    let clip = rvn_ui::video::VideoClip::parse(serde_json::json!({
        "source":"movies/resident.webm", "target":format!("ui:{screen}/player")
    }))
    .unwrap();
    state.videos.play(alias.into(), clip).unwrap();
    let key = resource_key(TaskOwner::Game, ResourceTaskKind::Video, alias);
    let execution = state.scene_execution.as_mut().unwrap();
    let target = execution
        .resources
        .ensure_target(TaskOwner::Game, TargetKind::Video, alias)
        .unwrap();
    execution
        .resources
        .register(key.clone(), target, ResourceTaskKind::Video, None)
        .unwrap();
    rvn_core::ui_residency::bind_video_interface(state, &key).unwrap();
    state
        .videos
        .feedback(
            alias,
            rvn_core::video::Feedback::Position {
                seconds: 1.75,
                duration: 20.0,
            },
        )
        .unwrap();
    sync_current(state).unwrap();
    let execution = state.scene_execution.as_ref().unwrap();
    execution
        .resources
        .effect_token(17, &key, execution.owner())
        .unwrap()
}

fn resident_game_motion(state: &mut GameState, screen: &str) -> ResourceToken {
    let interface = rvn_core::ui_residency::visible_interface_target(state, screen).unwrap();
    let target = rvn_core::motion::MotionTarget::Interface {
        screen: screen.into(),
        element: "player".into(),
    };
    let alias = target.key();
    state
        .motions
        .play(target, rvn_ui::motion::Motion::Pause { seconds: 10.0 })
        .unwrap();
    let key = resource_key(TaskOwner::Game, ResourceTaskKind::Motion, &alias);
    let execution = state.scene_execution.as_mut().unwrap();
    execution
        .resources
        .register(key.clone(), interface, ResourceTaskKind::Motion, None)
        .unwrap();
    state.motions.tick(2.25).unwrap();
    sync_current(state).unwrap();
    let execution = state.scene_execution.as_ref().unwrap();
    execution
        .resources
        .effect_token(17, &key, execution.owner())
        .unwrap()
}

fn resident_fixture() -> (PreparedNarrative, GameState, ResourceToken, ResourceToken) {
    let (program, mut state) = fixture();
    state.ui.screens.push(screen("form", 0, 9.5));
    state.ui.next_order = 1;
    sync_current(&mut state).unwrap();
    let video = resident_game_video(&mut state, "resident", "form");
    let motion = resident_game_motion(&mut state, "form");
    (program, state, video, motion)
}

#[test]
fn game_interface_projection_keeps_captured_parent_incarnation_through_child_homonym() {
    use rvn_core::ui_residency::{interface_projection, resident_interface, ProjectionResidence};
    let (program, mut state, video, motion) = resident_fixture();
    let original = state.ui.screens[0].clone();
    let binding = interface_projection(&state, &video.task.key)
        .unwrap()
        .unwrap();
    assert_eq!(binding.residence, ProjectionResidence::Visible);
    assert_eq!(binding.interface, motion.target);
    let initial_video = state.videos.tracks["resident"].clone();
    call(&program, &mut state);
    sync_current(&mut state).unwrap();
    state.ui.screens.push(screen("form", 1, 0.25));
    state.ui.next_order = 2;
    sync_current(&mut state).unwrap();
    let child = rvn_core::ui_residency::visible_interface_target(&state, "form").unwrap();
    assert_ne!(child, binding.interface);
    assert_eq!(
        resident_interface(&state, &child).unwrap().residence,
        ProjectionResidence::Visible
    );
    for token in [&video, &motion] {
        let hidden = interface_projection(&state, &token.task.key)
            .unwrap()
            .unwrap();
        assert_eq!(hidden.interface, binding.interface);
        assert_eq!(hidden.order, original.order);
        assert_eq!(hidden.residence, ProjectionResidence::Parked);
        let execution = state.scene_execution.as_ref().unwrap();
        assert!(execution.resources.accepts(token, 17, execution.owner()));
    }
    assert_eq!(
        resident_interface(&state, &binding.interface)
            .unwrap()
            .instance,
        &original
    );
    assert_eq!(state.videos.tracks["resident"], initial_video);
    state.motions.tick(1.0).unwrap();
    sync_current(&mut state).unwrap();
    assert_eq!(state.motions.tracks["ui:form/player"].elapsed, 3.25);
    return_scene(&program, &mut state);
    assert_eq!(state.ui.screens, vec![original]);
    assert_eq!(state.videos.tracks["resident"], initial_video);
    assert_eq!(state.motions.tracks["ui:form/player"].elapsed, 3.25);
    let returned = interface_projection(&state, &video.task.key)
        .unwrap()
        .unwrap();
    assert_eq!(returned, binding);
    assert!(state.scene_execution.as_ref().unwrap().resources.accepts(
        &video,
        17,
        state.scene_execution.as_ref().unwrap().owner()
    ));
}

#[test]
fn new_interface_binding_refuses_parked_or_foreign_instance_without_registry_mutation() {
    let (program, mut state, video, _) = resident_fixture();
    let identity = rvn_core::ui_residency::visible_interface_target(&state, "form").unwrap();
    call(&program, &mut state);
    let before = serde_json::to_vec(&state).unwrap();
    assert!(rvn_core::ui_residency::visible_interface_target(&state, "form").is_err());
    assert!(rvn_core::ui_residency::bind_video_interface(&mut state, &video.task.key).is_err());
    let mut foreign = identity.clone();
    foreign.incarnation += 100;
    assert!(rvn_core::ui_residency::resident_interface(&state, &foreign).is_err());
    foreign = identity.clone();
    foreign.kind = TargetKind::Sprite;
    assert!(rvn_core::ui_residency::resident_interface(&state, &foreign).is_err());
    assert_eq!(serde_json::to_vec(&state).unwrap(), before);
    assert_eq!(
        rvn_core::ui_residency::resident_interface(&state, &identity)
            .unwrap()
            .residence,
        rvn_core::ui_residency::ProjectionResidence::Parked
    );
}

#[test]
fn registered_embedded_video_missing_or_wrong_dependency_never_adopts_a_live_homonym() {
    let (_, state, video, _) = resident_fixture();
    for variant in 0..4 {
        let mut forged = state.clone();
        let execution = forged.scene_execution.as_mut().unwrap();
        let wrong = execution
            .resources
            .target(&TaskOwner::Game, TargetKind::Sprite, "hero")
            .unwrap()
            .clone();
        let task = execution
            .resources
            .tasks
            .iter_mut()
            .find(|task| task.key == video.task.key)
            .unwrap();
        match variant {
            0 => task.dependencies.clear(),
            1 => task.dependencies = vec![wrong],
            2 => task.dependencies.push(task.dependencies[0].clone()),
            _ => task.dependencies[0].incarnation += 100,
        }
        let before = serde_json::to_vec(&forged).unwrap();
        assert!(validate_current_resource_state(&forged).is_err());
        assert!(sync_current(&mut forged).is_err());
        assert_eq!(serde_json::to_vec(&forged).unwrap(), before);
    }
    let mut missing = state.clone();
    missing.ui.screens.clear();
    let before = serde_json::to_vec(&missing).unwrap();
    assert!(validate_current_resource_state(&missing).is_err());
    assert!(park_current(&mut missing).is_err());
    assert_eq!(serde_json::to_vec(&missing).unwrap(), before);
}

#[test]
fn abandon_cancels_game_dependencies_of_child_ui_and_preserves_parent_targets_and_globals() {
    let (program, mut state, parent_video, parent_motion) = resident_fixture();
    call(&program, &mut state);
    let parent = state.scene_execution.as_ref().unwrap().frames[0]
        .parked
        .as_ref()
        .unwrap()
        .clone();
    state.ui.screens.push(screen("childform", 1, 0.5));
    state.ui.next_order = 2;
    sync_current(&mut state).unwrap();
    let child_video = resident_game_video(&mut state, "childplayer", "childform");
    let child_motion = resident_game_motion(&mut state, "childform");
    state.videos.wait("childplayer").unwrap();
    sync_current(&mut state).unwrap();
    let globals = (
        state.vars.clone(),
        state.background_image.clone(),
        state.sprites.clone(),
    );
    let parent_track = state.videos.tracks["resident"].clone();
    let pc = state.pc;
    let transition = abandon_current(&mut state).unwrap();
    assert_eq!(state.pc, pc);
    assert_eq!(
        (
            state.vars.clone(),
            state.background_image.clone(),
            state.sprites.clone()
        ),
        globals
    );
    assert_eq!(state.videos.tracks["resident"], parent_track);
    assert!(!state.videos.tracks.contains_key("childplayer"));
    assert!(!state.motions.tracks.contains_key("ui:childform/player"));
    assert!(state.videos.waiting.is_none());
    assert_eq!(transition.cancelled_waits.len(), 1);
    assert_eq!(transition.cancelled_waits[0].key, child_video.task.key);
    assert_eq!(transition.cancelled_waits[0].target, child_video.target);
    assert_eq!(
        transition.cancelled_waits[0].reason,
        CancellationReason::OwnerAbandoned
    );
    let execution = state.scene_execution.as_ref().unwrap();
    assert!(execution
        .resources
        .accepts(&parent_video, 17, execution.owner()));
    assert!(execution
        .resources
        .accepts(&parent_motion, 17, execution.owner()));
    assert!(!execution
        .resources
        .accepts(&child_video, 17, execution.owner()));
    assert!(!execution
        .resources
        .accepts(&child_motion, 17, execution.owner()));
    assert!(!execution
        .resources
        .targets
        .iter()
        .any(|record| record.identity == child_video.target));
    assert_eq!(execution.frames[0].parked.as_ref().unwrap(), &parent);
    validate_current_resource_state(&state).unwrap();
}

#[test]
fn removing_parked_interface_cancels_game_dependents_and_only_exact_caller_waits() {
    let (program, mut state, video, motion) = resident_fixture();
    let parent_id = state.scene_execution.as_ref().unwrap().owner();
    let parent_owner = owner(&state);
    let identity = rvn_core::ui_residency::visible_interface_target(&state, "form").unwrap();
    state.videos.wait("resident").unwrap();
    sync_current(&mut state).unwrap();
    call(&program, &mut state);
    let child_id = state.scene_execution.as_ref().unwrap().owner();
    state.ui.screens.push(screen("form", 1, 1.25));
    state.ui.next_order = 2;
    sync_current(&mut state).unwrap();
    let child_identity = rvn_core::ui_residency::visible_interface_target(&state, "form").unwrap();
    let child_ui = state.ui.clone();
    state.videos.wait("resident").unwrap();
    sync_current(&mut state).unwrap();
    let pc = state.pc;
    let transition =
        remove_target(&mut state, &parent_owner, TargetKind::Interface, "form").unwrap();
    assert_eq!(state.pc, pc);
    assert_eq!(state.ui, child_ui);
    assert!(!state.videos.tracks.contains_key("resident"));
    assert!(!state.motions.tracks.contains_key("ui:form/player"));
    assert_eq!(transition.cancelled_waits.len(), 1);
    assert_eq!(transition.cancelled_waits[0].owner, child_id);
    assert_eq!(transition.cancelled_waits[0].key, video.task.key);
    assert_eq!(transition.cancelled_waits[0].revision, video.task.revision);
    assert_eq!(transition.cancelled_waits[0].target, video.target);
    assert_eq!(
        transition.cancelled_waits[0].reason,
        CancellationReason::TargetRemoved
    );
    let execution = state.scene_execution.as_ref().unwrap();
    assert!(!execution
        .resources
        .targets
        .iter()
        .any(|record| record.identity == identity));
    assert!(execution
        .resources
        .targets
        .iter()
        .any(|record| record.identity == child_identity));
    assert!(!execution.resources.accepts(&video, 17, execution.owner()));
    assert!(!execution.resources.accepts(&motion, 17, execution.owner()));
    assert_eq!(execution.resources.cancelled_waits.len(), 1);
    assert_eq!(execution.resources.cancelled_waits[0].owner, parent_id);
    assert!(execution.frames[0]
        .parked
        .as_ref()
        .unwrap()
        .tasks
        .suspended_videos
        .waiting
        .is_none());
    let returned = return_scene(&program, &mut state);
    assert_eq!(returned.cancelled_waits.len(), 1);
    assert_eq!(returned.cancelled_waits[0].owner, parent_id);
    assert_eq!(returned.cancelled_waits[0].key, video.task.key);
    assert!(state.videos.waiting.is_none());
    assert!(state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .cancelled_waits
        .is_empty());
}

#[test]
fn portable_story_snapshot_keeps_game_ui_dependency_and_resident_parent_state_byte_exact() {
    let (program, mut state, video, motion) = resident_fixture();
    state.videos.wait("resident").unwrap();
    sync_current(&mut state).unwrap();
    call(&program, &mut state);
    let before = serde_json::to_vec(&state).unwrap();
    let save =
        rvn_core::save::SaveData::try_from_state(&state, 4, "resident".into(), "story.rvn".into())
            .unwrap();
    let roundtrip: rvn_core::save::SaveData =
        serde_json::from_slice(&serde_json::to_vec(&save).unwrap()).unwrap();
    let saved_execution = roundtrip.scene_execution.as_ref().unwrap();
    let live_execution = state.scene_execution.as_ref().unwrap();
    assert_eq!(saved_execution.resources, live_execution.resources);
    assert_eq!(saved_execution.frames, live_execution.frames);
    assert_eq!(roundtrip.videos, state.videos);
    assert_eq!(roundtrip.motions, state.motions);
    assert_eq!(serde_json::to_vec(&state).unwrap(), before);
    let mut restored = state.clone();
    restored.scene_execution = roundtrip.scene_execution;
    restored.videos = roundtrip.videos;
    restored.motions = roundtrip.motions;
    validate_current_resource_state(&restored).unwrap();
    for token in [&video, &motion] {
        assert_eq!(
            rvn_core::ui_residency::interface_projection(&restored, &token.task.key)
                .unwrap()
                .unwrap()
                .residence,
            rvn_core::ui_residency::ProjectionResidence::Parked
        );
    }
    return_scene(&program, &mut restored);
    assert_eq!(restored.videos.tracks["resident"].position, 1.75);
    assert_eq!(restored.motions.tracks["ui:form/player"].elapsed, 2.25);
    assert_eq!(restored.ui.screens[0].canvas_states["canvas"].elapsed, 9.5);
    assert_eq!(restored.videos.waiting.as_deref(), Some("resident"));
}

fn effect_operation_engine() -> Engine<Host> {
    let (_, state) = fixture();
    let parsed = rvn_parser::parse_program(
        "scene label P\n\"parent\"\nscene label C\n\"child\"\nlabel helper\nreturn\n",
    )
    .unwrap();
    let mut engine = Engine::from_program(parsed, Host, 0).unwrap();
    engine.state = state;
    validate_current_resource_state(&engine.state).unwrap();
    engine
}

fn effect_operation_target(engine: &Engine<Host>) -> TargetIdentity {
    engine
        .state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .target(&TaskOwner::Game, TargetKind::Sprite, "hero")
        .unwrap()
        .clone()
}

fn effect_operation_descriptor(
    engine: &Engine<Host>,
    key: &rvn_core::scene_tasks::TaskKey,
) -> SavedEffect {
    engine
        .state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .tasks
        .iter()
        .find(|task| task.key == *key)
        .unwrap()
        .effect
        .clone()
        .unwrap()
}

#[test]
fn engine_effect_replacement_changes_revision_and_authored_parameters_without_retagging_target() {
    let mut engine = effect_operation_engine();
    let target = effect_operation_target(&engine);
    let key = resource_key(
        owner(&engine.state),
        ResourceTaskKind::Effect,
        "hero/bounce",
    );
    let first = engine
        .register_scene_effect(key.clone(), target.clone(), effect(1.0))
        .unwrap();
    let before = engine
        .state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .clone();
    let mut authored = effect(0.0);
    authored.clock.duration = 8.000000000000002;
    if let EffectParameters::SpriteAnimation { animation, .. } = &mut authored.parameters {
        *animation = SpriteEffectKind::Bounce { height: 31.0 };
    }
    let second = engine
        .register_scene_effect(key.clone(), target.clone(), authored.clone())
        .unwrap();
    assert!(second.task.revision > first.task.revision);
    assert_eq!(second.task.runtime_epoch, first.task.runtime_epoch);
    assert_eq!(second.task.key, first.task.key);
    assert_eq!(second.target, first.target);
    let registry = &engine.state.scene_execution.as_ref().unwrap().resources;
    assert_eq!(registry.tasks.len(), before.tasks.len());
    assert_eq!(registry.targets, before.targets);
    assert_eq!(registry.waits, before.waits);
    assert_eq!(registry.cancelled_waits, before.cancelled_waits);
    assert_eq!(effect_operation_descriptor(&engine, &key), authored);
    assert!(!registry.accepts(
        &first,
        engine.runtime_epoch(),
        engine.state.scene_execution.as_ref().unwrap().owner()
    ));
    assert!(registry.accepts(
        &second,
        engine.runtime_epoch(),
        engine.state.scene_execution.as_ref().unwrap().owner()
    ));
    validate_current_resource_state(&engine.state).unwrap();
}

#[test]
fn engine_effect_identical_payload_still_revokes_stale_invalid_capture_before_validation() {
    let mut engine = effect_operation_engine();
    let target = effect_operation_target(&engine);
    let key = resource_key(
        owner(&engine.state),
        ResourceTaskKind::Effect,
        "hero/bounce",
    );
    let authored = effect(1.0);
    let first = engine
        .register_scene_effect(key.clone(), target.clone(), authored.clone())
        .unwrap();
    let second = engine
        .register_scene_effect(key.clone(), target, authored.clone())
        .unwrap();
    assert!(second.task.revision > first.task.revision);
    assert_eq!(effect_operation_descriptor(&engine, &key), authored);
    let before = serde_json::to_vec(&engine.state).unwrap();
    let mut invalid = authored.clone();
    invalid.clock.elapsed = f64::NAN;
    assert!(!engine
        .capture_scene_effect(&first, invalid.clone())
        .unwrap());
    assert!(!engine
        .capture_scene_effect_base(
            &first,
            EffectProjectionBase::SpriteTransform(TransformSnapshot {
                translation: [f32::NAN, 0.0, 0.0],
                ..Default::default()
            }),
        )
        .unwrap());
    assert!(!engine.complete_scene_effect(&first).unwrap());
    assert_eq!(serde_json::to_vec(&engine.state).unwrap(), before);
    assert!(engine.capture_scene_effect(&second, invalid).is_err());
    assert_eq!(serde_json::to_vec(&engine.state).unwrap(), before);
    let measured = TransformSnapshot {
        translation: [8.0, 9.0, 10.0],
        ..Default::default()
    };
    assert!(engine
        .capture_scene_effect_base(
            &second,
            EffectProjectionBase::SpriteTransform(measured.clone())
        )
        .unwrap());
    let resolved = effect_operation_descriptor(&engine, &key);
    assert_eq!(resolved.clock, authored.clock);
    assert!(
        matches!(resolved.parameters, EffectParameters::SpriteAnimation { animation: SpriteEffectKind::Bounce { height: 17.0 }, base } if base == measured)
    );
}

#[test]
fn engine_effect_invalid_replacement_and_revision_exhaustion_leave_old_operation_byte_exact() {
    let mut engine = effect_operation_engine();
    let target = effect_operation_target(&engine);
    let key = resource_key(
        owner(&engine.state),
        ResourceTaskKind::Effect,
        "hero/bounce",
    );
    let token = engine
        .register_scene_effect(key.clone(), target.clone(), effect(1.0))
        .unwrap();
    let before = serde_json::to_vec(&engine.state).unwrap();
    let mut bad = effect(0.0);
    bad.clock.duration = f64::INFINITY;
    assert!(engine
        .register_scene_effect(key.clone(), target.clone(), bad)
        .is_err());
    assert_eq!(serde_json::to_vec(&engine.state).unwrap(), before);
    engine
        .state
        .scene_execution
        .as_mut()
        .unwrap()
        .resources
        .next_revision = u64::MAX;
    validate_current_resource_state(&engine.state).unwrap();
    let exhausted = serde_json::to_vec(&engine.state).unwrap();
    assert!(engine
        .register_scene_effect(key.clone(), target, effect(0.0))
        .unwrap_err()
        .to_string()
        .contains("revision exhausted"));
    assert_eq!(serde_json::to_vec(&engine.state).unwrap(), exhausted);
    let execution = engine.state.scene_execution.as_ref().unwrap();
    assert!(execution
        .resources
        .accepts(&token, engine.runtime_epoch(), execution.owner()));
    assert_eq!(effect_operation_descriptor(&engine, &key), effect(1.0));
}

#[test]
fn engine_effect_replacement_cannot_change_incarnation_or_adopt_a_parked_operation() {
    let mut engine = effect_operation_engine();
    let target = effect_operation_target(&engine);
    let key = resource_key(
        owner(&engine.state),
        ResourceTaskKind::Effect,
        "hero/bounce",
    );
    let token = engine
        .register_scene_effect(key.clone(), target.clone(), effect(1.0))
        .unwrap();
    engine
        .state
        .sprites
        .insert("other".into(), SpriteState::new(None, Position::Left));
    sync_current(&mut engine.state).unwrap();
    let other = engine
        .state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .target(&TaskOwner::Game, TargetKind::Sprite, "other")
        .unwrap()
        .clone();
    let before = serde_json::to_vec(&engine.state).unwrap();
    assert!(engine
        .register_scene_effect(key.clone(), other, effect(0.0))
        .is_err());
    assert_eq!(serde_json::to_vec(&engine.state).unwrap(), before);
    let (program, _) = fixture();
    call(&program, &mut engine.state);
    validate_current_resource_state(&engine.state).unwrap();
    let parked = serde_json::to_vec(&engine.state).unwrap();
    assert!(engine
        .register_scene_effect(key, target, effect(0.0))
        .is_err());
    assert!(!engine
        .capture_scene_effect_base(
            &token,
            EffectProjectionBase::SpriteTransform(TransformSnapshot {
                translation: [f32::NAN, 0.0, 0.0],
                ..Default::default()
            }),
        )
        .unwrap());
    assert_eq!(serde_json::to_vec(&engine.state).unwrap(), parked);
}

#[test]
fn generic_motion_registration_remains_idempotent_and_rejects_kind_or_parked_adoption() {
    let mut registry = ResourceRegistry::default();
    let target = registry
        .ensure_target(TaskOwner::Game, TargetKind::Sprite, "hero")
        .unwrap();
    let key = resource_key(TaskOwner::Game, ResourceTaskKind::Motion, "hero");
    let revision = registry
        .register(key.clone(), target.clone(), ResourceTaskKind::Motion, None)
        .unwrap();
    assert_eq!(
        registry
            .register(key.clone(), target.clone(), ResourceTaskKind::Motion, None)
            .unwrap(),
        revision
    );
    let before = registry.clone();
    assert!(registry
        .register(key.clone(), target.clone(), ResourceTaskKind::Effect, None)
        .is_err());
    assert_eq!(registry, before);
    registry.tasks[0].parked = true;
    let parked = registry.clone();
    assert!(registry
        .register(key, target, ResourceTaskKind::Motion, None)
        .is_err());
    assert_eq!(registry, parked);
}

#[test]
fn engine_effect_aggregate_budget_refuses_new_key_but_allows_fresh_operation_at_capacity() {
    let mut engine = effect_operation_engine();
    let target = effect_operation_target(&engine);
    let operation_owner = owner(&engine.state);
    let mut first = None;
    for index in 0..64 {
        let key = resource_key(
            operation_owner.clone(),
            ResourceTaskKind::Effect,
            &format!("hero/effect{index}"),
        );
        let token = engine
            .register_scene_effect(key, target.clone(), effect(1.0))
            .unwrap();
        if first.is_none() {
            first = Some(token);
        }
    }
    validate_current_resource_state(&engine.state).unwrap();
    let first = first.unwrap();
    let before = serde_json::to_vec(&engine.state).unwrap();
    let extra_key = resource_key(operation_owner, ResourceTaskKind::Effect, "hero/extra");
    assert!(engine
        .register_scene_effect(extra_key, target.clone(), effect(0.0))
        .is_err());
    assert_eq!(serde_json::to_vec(&engine.state).unwrap(), before);
    assert!(engine
        .state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .accepts(
            &first,
            engine.runtime_epoch(),
            engine.state.scene_execution.as_ref().unwrap().owner()
        ));
    let replacement = engine
        .register_scene_effect(first.task.key.clone(), target, effect(0.0))
        .unwrap();
    assert!(replacement.task.revision > first.task.revision);
    let registry = &engine.state.scene_execution.as_ref().unwrap().resources;
    assert_eq!(
        registry
            .tasks
            .iter()
            .filter(|task| task.kind == ResourceTaskKind::Effect)
            .count(),
        64
    );
    assert!(!registry.accepts(
        &first,
        engine.runtime_epoch(),
        engine.state.scene_execution.as_ref().unwrap().owner()
    ));
    assert!(registry.accepts(
        &replacement,
        engine.runtime_epoch(),
        engine.state.scene_execution.as_ref().unwrap().owner()
    ));
    validate_current_resource_state(&engine.state).unwrap();
}


#[test]
fn portable_save_roundtrips_live_and_parked_resource_clock_bits_without_drift() {
    // Includes the exact legacy Atlas canvas clock and small decimal values
    // that exercise direct typed f64/f32 decoding rather than approximate
    // significand arithmetic or an intermediate f64-to-f32 conversion.
    for elapsed in [1.9963328988524154f64, 0.10000000000000002, 2.0030397744267762e-253] {
        for parked in [false, true] {
            let (program, mut state) = fixture();
            state.ui.screens.push(screen("precision", 0, elapsed));
            state.ui.next_order = 1;
            video(&mut state, "precision", elapsed);
            state.motions.play(
                rvn_core::motion::MotionTarget::Sprite { id: "hero".into() },
                rvn_ui::motion::Motion::Pause { seconds: 10.0 },
            ).unwrap();
            state.motions.tick(elapsed).unwrap();
            sync_current(&mut state).unwrap();
            let task_owner = owner(&state);
            let token = projection_effect(
                &mut state, task_owner, "precision", TargetKind::Sprite, "hero", effect(elapsed),
            );
            state.music.volume = 7.038531e-26f32;
            sync_current(&mut state).unwrap();
            if parked {
                call(&program, &mut state);
            }
            let original = serde_json::to_value(&state).unwrap();
            let mut save = rvn_core::save::SaveData::try_from_state(
                &state, 6, "precision".into(), "story.rvn".into(),
            ).unwrap();
            let expected = save.clone().into_game_state();
            for _ in 0..8 {
                let encoded = serde_json::to_vec(&save).unwrap();
                save = serde_json::from_slice(&encoded).unwrap();
                // SceneExecution intentionally has no aggregate PartialEq.
                // Compare all typed fields, including nested parked clocks,
                // without changing the production model for a test helper.
                let actual_scene = save.scene_execution.as_ref().unwrap();
                let expected_scene = expected.scene_execution.as_ref().unwrap();
                assert_eq!(actual_scene.next_activation_id, expected_scene.next_activation_id);
                assert_eq!(actual_scene.active, expected_scene.active);
                assert_eq!(actual_scene.frames, expected_scene.frames);
                assert_eq!(actual_scene.pre_scene_temporaries, expected_scene.pre_scene_temporaries);
                assert_eq!(actual_scene.live_position, expected_scene.live_position);
                assert_eq!(actual_scene.game_tasks, expected_scene.game_tasks);
                assert_eq!(actual_scene.resources, expected_scene.resources);
                assert_eq!(actual_scene.sprite_styles, expected_scene.sprite_styles);
                assert_eq!(
                    serde_json::to_value(&actual_scene.display).unwrap(),
                    serde_json::to_value(&expected_scene.display).unwrap(),
                );
                for (actual_task, expected_task) in actual_scene.resources.tasks.iter()
                    .zip(&expected_scene.resources.tasks)
                {
                    if let Some(actual_effect) = &actual_task.effect {
                        let expected_effect = expected_task.effect.as_ref().unwrap();
                        assert_eq!(actual_effect.clock.duration.to_bits(), expected_effect.clock.duration.to_bits());
                        assert_eq!(actual_effect.clock.elapsed.to_bits(), expected_effect.clock.elapsed.to_bits());
                    }
                }
                assert_eq!(save.videos, expected.videos);
                assert_eq!(save.motions, expected.motions);
                assert_eq!(save.ui, expected.ui);
                assert_eq!(save.music.volume.to_bits(), 7.038531e-26f32.to_bits());
            }
            assert_eq!(serde_json::to_value(&state).unwrap(), original);
            let mut restored = save.into_game_state();
            validate_current_resource_state(&restored).unwrap();
            assert_eq!(restored.pc, expected.pc);
            assert_eq!(restored.call_stack, expected.call_stack);
            assert_eq!(restored.random, expected.random);
            assert_eq!(restored.display_random, expected.display_random);
            if parked {
                return_scene(&program, &mut restored);
            }
            assert_eq!(restored.ui.screens[0].canvas_states["canvas"].elapsed.to_bits(), elapsed.to_bits());
            assert_eq!(restored.videos.tracks["precision"].position.to_bits(), elapsed.to_bits());
            assert_eq!(restored.motions.tracks["sprite:hero"].elapsed.to_bits(), elapsed.to_bits());
            let task = restored.scene_execution.as_ref().unwrap().resources.tasks.iter()
                .find(|task| task.key == token.task.key).unwrap();
            assert!(!task.parked);
            assert_eq!(task.effect.as_ref().unwrap().clock.elapsed.to_bits(), elapsed.to_bits());
        }
    }
}
