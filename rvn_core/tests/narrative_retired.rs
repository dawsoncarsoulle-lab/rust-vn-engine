//! PRIVATE, NOT_RUN: neutral Core contracts. Engine command/Bevy projection
//! acceptance remains separate; no fake Entity or native presentation is used.
use rvn_core::program::PreparedNarrative;
use rvn_core::renderer::SceneRenderBatch;
use rvn_core::retired_presentations::*;
use rvn_core::save::SaveData;
use rvn_core::scene_resources::*;
use rvn_core::scene_tasks::TaskOwner;
use rvn_core::sprite_effects::SpriteStyle;
use rvn_core::types::GameState;
use rvn_core::{Engine, Renderer, SpriteState};
use rvn_parser::{Hotspot, Position, Transition};

const SOURCE: &str = "scene label P\n\"parent\"\nscene label C\n\"child\"\nlabel helper\nreturn\n";
const EPOCH: u64 = 7;
#[derive(Default)]
struct Host {
    batches: Vec<SceneRenderBatch>,
    reject: bool,
    music_checkpoint_fixture: bool,
}
impl Renderer for Host {
    // Explicit Core descriptor fixture, never evidence of a native/Bevy decoder cursor.
    fn capture_audio_checkpoint(&self,state:&GameState,instance:u64,epoch:u64)->Result<rvn_core::audio_checkpoint::AudioCheckpointSnapshot,String> {
        if !rvn_core::audio_playback::describe(state,epoch)?.is_empty(){return Err("Retired fixture has no one-shot reader".into());}
        let music=rvn_core::audio_checkpoint::music_views(state,epoch)?;
        let mut snapshot=rvn_core::audio_checkpoint::AudioCheckpointSnapshot{instance_id:instance,runtime_epoch:epoch,rows:Vec::new(),music:Vec::new()};
        for view in music.into_iter().filter(|view|!matches!(view.owner,TaskOwner::Host{..})) {
            if !self.music_checkpoint_fixture || view.file!="audio/parent.ogg" {return Err("Retired Core fixture has no measured Music descriptor".into());}
            snapshot.music.push(rvn_core::audio_checkpoint::MusicCheckpoint{target:view.authority.target,file:view.file,checkpoint:fixture_music_descriptor()});
        }
        Ok(snapshot)
    }
    fn validate_audio_restore(&self,state:&GameState)->Result<(),String> {
        rvn_core::audio_checkpoint::require_saved_checkpoints(state)?;
        if !rvn_core::audio_playback::describe(state,0)?.is_empty(){return Err("Retired fixture has no restored one-shot reader".into());}
        for view in rvn_core::audio_checkpoint::music_views(state,0)? {
            if !self.music_checkpoint_fixture || view.file!="audio/parent.ogg" || view.checkpoint.as_ref()!=Some(&fixture_music_descriptor()) {return Err("Undeclared restored Core Music descriptor".into());}
        }
        Ok(())
    }
    fn validate_scene_batch(&self, _: &SceneRenderBatch) -> Result<(), String> {
        if self.reject {
            Err("deliberate batch refusal".into())
        } else {
            Ok(())
        }
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
// Only the named portability contracts opt into this logical descriptor fixture.
fn fixture_music_descriptor()->rvn_core::audio_checkpoint::AudioCheckpoint {
    rvn_core::audio_checkpoint::AudioCheckpoint{version:1,asset_sha256:[0x41;32],pcm_sha256:[0x52;32],channels:2,sample_rate:48_000,total_samples:4,next_sample:2,next_channel:0,looping:true,loops_completed:0}
}
fn portable_save(state:&GameState,label:&str)->SaveData {
    let mut producer=Engine::from_program(rvn_parser::parse_program(SOURCE).unwrap(),Host{music_checkpoint_fixture:true,..Default::default()},0).unwrap();
    producer.state=state.clone();
    let snapshot=producer.snapshot_for_save().unwrap();
    SaveData::try_from_snapshot(&snapshot,0,label.into(),"story.rvn".into()).unwrap()
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
    state.sprites.insert(
        "hero".into(),
        SpriteState::new(Some("smile".into()), Position::Center),
    );
    state.music.current_file = Some("audio/parent.ogg".into());
    state.music.volume = 0.6;
    sync_current(&mut state).unwrap();
    (program, state)
}
fn owner(state: &GameState) -> TaskOwner {
    TaskOwner::Scene {
        activation: state.scene_execution.as_ref().unwrap().owner().unwrap(),
    }
}
fn bytes(state: &GameState) -> Vec<u8> {
    serde_json::to_vec(state).unwrap()
}
fn registry(state: &GameState) -> &ResourceRegistry {
    &state.scene_execution.as_ref().unwrap().resources
}
fn task<'a>(state: &'a GameState, token: &ResourceToken) -> &'a ResourceTask {
    registry(state)
        .tasks
        .iter()
        .find(|task| task.key == token.task.key)
        .unwrap()
}
fn due(state: &mut GameState, token: &ResourceToken) {
    let task = state
        .scene_execution
        .as_mut()
        .unwrap()
        .resources
        .tasks
        .iter_mut()
        .find(|task| task.key == token.task.key)
        .unwrap();
    let effect = task.effect.as_mut().unwrap();
    effect.clock = effect.clock.advanced(effect.clock.duration).unwrap();
}
fn visual(duration: f64, layered: bool) -> SavedEffect {
    SavedEffect {
        presentation: None,
        base_resolved: false,
        clock: EffectClock {
            duration,
            elapsed: 0.0,
            looping: false,
        },
        parameters: EffectParameters::VisualFade {
            from: 1.0,
            to: 0.0,
            despawn_on_finish: true,
            transition: VisualTransition::Fade,
            base: TransformSnapshot::default(),
            layered,
        },
    }
}
fn outgoing(state: &mut GameState, slot: PresentationSlot, task_owner: TaskOwner) -> ResourceToken {
    let payload = snapshot_retired_source(state, &slot).unwrap().unwrap();
    let layered = matches!(
        &payload.model,
        RetiredModel::Sprite {
            composition: Some(_),
            ..
        }
    );
    let effect = if slot == PresentationSlot::Music {
        SavedEffect {
            presentation: None,
            base_resolved: false,
            clock: EffectClock {
                duration: 4.0,
                elapsed: 0.0,
                looping: false,
            },
            parameters: EffectParameters::AudioFade {
                from: 1.0,
                to: 0.0,
                despawn_on_done: true,
            },
        }
    } else {
        visual(4.0, layered)
    };
    register_retired_effect(state, EPOCH, task_owner, payload, effect)
        .unwrap()
        .unwrap()
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
fn returned(program: &PreparedNarrative, state: &mut GameState) {
    abandon_current(state).unwrap();
    let execution = state.scene_execution.as_mut().unwrap();
    let position = execution.return_to_caller(false).unwrap().unwrap();
    program.validate_position(&position).unwrap();
    state.pc = position.pc;
    execution.live_position = position;
    state.call_stack = execution.derived_call_stack();
    resume_current(state).unwrap();
}
fn composition(state: &mut GameState) {
    let definition = rvn_ui::composition::Composition::parse(serde_json::json!({
        "size":[400,600], "defaults":{"face":"smile"},
        "layers":[{"id":"body","image":"images/body.png","opacity":0.8},
            {"id":"smile","image":"images/smile.png","when":{"face":"smile"}},
            {"id":"frown","image":"images/frown.png","when":{"face":"frown"}}]
    }))
    .unwrap();
    state.layered.compose("hero".into(), definition).unwrap();
    sync_current(state).unwrap();
}

#[test]
fn retired_descriptors_exist_before_projection_and_silent_scene_call() {
    let (program, mut state) = fixture();
    let parent = owner(&state);
    let token = outgoing(
        &mut state,
        PresentationSlot::Sprite { id: "hero".into() },
        parent,
    );
    let before = task(&state, &token).effect.clone().unwrap();
    assert!(!before.base_resolved);
    assert_eq!(before.clock.elapsed, 0.0);
    let historical = before
        .presentation
        .as_ref()
        .unwrap()
        .original_target
        .identity
        .clone();
    assert_ne!(historical, token.target);
    assert!(task(&state, &token).dependencies.is_empty());
    call(&program, &mut state);
    let parked = task(&state, &token);
    assert!(parked.parked);
    assert_eq!(parked.effect.as_ref().unwrap(), &before);
    assert!(!registry(&state).accepts(
        &token,
        EPOCH,
        state.scene_execution.as_ref().unwrap().owner()
    ));
    validate_current_resource_state(&state).unwrap();
}

#[test]
fn retired_styled_composition_samples_old_motion_and_alpha_without_child_lookup() {
    let (program, mut state) = fixture();
    composition(&mut state);
    let style = SpriteStyle {
        flip_x: true,
        flip_y: false,
        scale: 1.7,
        rotation: 27.0,
        tint: Some("#8a9bcd".into()),
    };
    state
        .scene_execution
        .as_mut()
        .unwrap()
        .sprite_styles
        .insert("hero".into(), style.clone());
    state
        .motions
        .play(
            rvn_core::motion::MotionTarget::Layer {
                id: "hero".into(),
                layer: "body".into(),
            },
            rvn_ui::motion::Motion::Pause { seconds: 8.0 },
        )
        .unwrap();
    state.motions.tick(2.0).unwrap();
    sync_current(&mut state).unwrap();
    let target = registry(&state)
        .target(&TaskOwner::Game, TargetKind::Sprite, "hero")
        .unwrap()
        .clone();
    let old = SavedEffect {
        presentation: None,
        base_resolved: true,
        clock: EffectClock {
            duration: 4.0,
            elapsed: 1.5,
            looping: false,
        },
        parameters: EffectParameters::VisualFade {
            from: 0.2,
            to: 0.9,
            despawn_on_finish: false,
            transition: VisualTransition::Dissolve,
            base: TransformSnapshot {
                translation: [4.0, 6.0, 0.0],
                ..Default::default()
            },
            layered: true,
        },
    };
    register_effect(
        &mut state,
        EPOCH,
        resource_key(TaskOwner::Game, ResourceTaskKind::Effect, "incoming"),
        target,
        old.clone(),
    )
    .unwrap();
    let frozen = snapshot_retired_source(&state, &PresentationSlot::Sprite { id: "hero".into() })
        .unwrap()
        .unwrap();
    let random = state.random;
    let source_layer_track = state.motions.tracks["layer:hero/body"].clone();
    call(&program, &mut state);
    state.sprites.get_mut("hero").unwrap().emotion = Some("child".into());
    state
        .scene_execution
        .as_mut()
        .unwrap()
        .sprite_styles
        .insert("hero".into(), SpriteStyle::default());
    state
        .layered
        .characters
        .get_mut("hero")
        .unwrap()
        .attributes
        .insert("face".into(), "frown".into());
    match &frozen.model {
        RetiredModel::Sprite {
            style: copied,
            composition: Some(composed),
            sample,
            ..
        } => {
            assert_eq!(copied, &style);
            assert_eq!(composed.attributes["face"], "smile");
            assert_eq!(sample.fade.as_ref().unwrap().clock, old.clock);
            assert_eq!(sample.fade.as_ref().unwrap().from, 0.2);
            assert!(sample.fade.as_ref().unwrap().base_resolved);
            assert!(sample.layers["body"].motion.is_some());
            let captured = sample.layers["body"].motion.as_ref().unwrap();
            assert_eq!(captured.pose, source_layer_track.pose().unwrap());
            assert_eq!(captured.reduced_pose, source_layer_track.base);
            assert!(frozen.resource_paths().contains("images/frown.png"));
        }
        _ => panic!("not the original composed appearance"),
    }
    assert_eq!(state.random, random);
    frozen.validate_structure().unwrap();
    let mut forged_layer_owner = frozen.clone();
    if let RetiredModel::Sprite { sample, .. } = &mut forged_layer_owner.model {
        sample.layers.get_mut("body").unwrap().original_target.owner = owner(&state);
    }
    assert!(forged_layer_owner.validate_structure().is_err());
    let mut finite_authored_style = frozen.clone();
    if let RetiredModel::Sprite { style, .. } = &mut finite_authored_style.model {
        style.scale = f32::MAX;
        style.rotation = f32::MAX;
    }
    // Logical styles retain their existing finite-only validity. Resolved bases
    // remain subject to the separate SavedEffect transform validator.
    finite_authored_style.validate_structure().unwrap();
}

#[test]
fn same_slot_child_replacement_cancels_parked_retired_without_restoring_decor() {
    let (program, mut state) = fixture();
    let parent = owner(&state);
    let sprite = outgoing(
        &mut state,
        PresentationSlot::Sprite { id: "hero".into() },
        parent.clone(),
    );
    let music = outgoing(&mut state, PresentationSlot::Music, parent);
    call(&program, &mut state);
    state.sprites.get_mut("hero").unwrap().emotion = Some("child".into());
    let current = state.sprites["hero"].clone();
    let music_descriptor = task(&state, &music).effect.clone();
    let transition = cancel_retired_slot(
        &mut state,
        &PresentationSlot::Sprite { id: "hero".into() },
        CancellationReason::TargetReplaced,
    )
    .unwrap();
    assert!(transition.cancelled_waits.is_empty());
    assert!(!registry(&state)
        .tasks
        .iter()
        .any(|task| task.key == sprite.task.key));
    assert!(!registry(&state)
        .targets
        .iter()
        .any(|record| record.identity == sprite.target));
    assert_eq!(task(&state, &music).effect, music_descriptor);
    returned(&program, &mut state);
    assert_eq!(state.sprites["hero"], current);
}

#[test]
fn show_visible_keeps_incarnation_motion_and_supersedes_exact_channels() {
    let (_, mut state) = fixture();
    let original = registry(&state)
        .target(&TaskOwner::Game, TargetKind::Sprite, "hero")
        .unwrap()
        .clone();
    state
        .motions
        .play(
            rvn_core::motion::MotionTarget::Sprite { id: "hero".into() },
            rvn_ui::motion::Motion::Pause { seconds: 8.0 },
        )
        .unwrap();
    sync_current(&mut state).unwrap();
    let motion = state.motions.clone();
    let incoming = SavedEffect {
        presentation: None,
        base_resolved: false,
        clock: EffectClock {
            duration: 8.0,
            elapsed: 0.0,
            looping: true,
        },
        parameters: EffectParameters::SpriteAnimation {
            animation: SpriteEffectKind::Bounce { height: 3.0 },
            base: Default::default(),
        },
    };
    let token = register_effect(
        &mut state,
        EPOCH,
        resource_key(TaskOwner::Game, ResourceTaskKind::Effect, "animation"),
        original.clone(),
        incoming,
    )
    .unwrap();
    let retired = outgoing(
        &mut state,
        PresentationSlot::Sprite { id: "hero".into() },
        TaskOwner::Game,
    );
    supersede_effect_channel(&mut state, &original, EffectChannel::SpriteAnimation).unwrap();
    assert_eq!(
        registry(&state).target(&TaskOwner::Game, TargetKind::Sprite, "hero"),
        Some(&original)
    );
    assert_eq!(state.motions, motion);
    assert!(!registry(&state).accepts(
        &token,
        EPOCH,
        state.scene_execution.as_ref().unwrap().owner()
    ));
    assert!(registry(&state).accepts(
        &retired,
        EPOCH,
        state.scene_execution.as_ref().unwrap().owner()
    ));
}

#[test]
fn repeat_hide_and_stop_preserve_existing_outgoing_clock() {
    let (_, mut state) = fixture();
    let target = registry(&state)
        .target(&TaskOwner::Game, TargetKind::Music, "music")
        .unwrap()
        .clone();
    register_effect(
        &mut state,
        EPOCH,
        resource_key(TaskOwner::Game, ResourceTaskKind::Effect, "incoming_music"),
        target,
        SavedEffect {
            presentation: None,
            base_resolved: false,
            clock: EffectClock {
                duration: 4.0,
                elapsed: 1.0,
                looping: false,
            },
            parameters: EffectParameters::AudioFade {
                from: 0.25,
                to: 0.75,
                despawn_on_done: false,
            },
        },
    )
    .unwrap();
    let sprite = outgoing(
        &mut state,
        PresentationSlot::Sprite { id: "hero".into() },
        TaskOwner::Game,
    );
    let music = outgoing(&mut state, PresentationSlot::Music, TaskOwner::Game);
    state.sprites.get_mut("hero").unwrap().visible = false;
    state.music.current_file = None;
    sync_current(&mut state).unwrap();
    for key in [&sprite.task.key, &music.task.key] {
        let effect = state
            .scene_execution
            .as_mut()
            .unwrap()
            .resources
            .tasks
            .iter_mut()
            .find(|task| task.key == *key)
            .unwrap()
            .effect
            .as_mut()
            .unwrap();
        effect.clock = effect.clock.advanced(1.25).unwrap();
    }
    let before = bytes(&state);
    assert!(
        snapshot_retired_source(&state, &PresentationSlot::Sprite { id: "hero".into() })
            .unwrap()
            .is_none()
    );
    assert!(snapshot_retired_source(&state, &PresentationSlot::Music)
        .unwrap()
        .is_none());
    assert_eq!(bytes(&state), before);
    assert_eq!(
        task(&state, &sprite).effect.as_ref().unwrap().clock.elapsed,
        1.25
    );
    assert_eq!(
        task(&state, &music).effect.as_ref().unwrap().clock.elapsed,
        1.25
    );
    let effect = task(&state, &music).effect.as_ref().unwrap();
    assert_eq!(
        effect
            .presentation
            .as_ref()
            .unwrap()
            .outgoing_music_gain(effect)
            .unwrap(),
        Some(0.2578125)
    );
}

#[test]
fn retired_capture_rejects_authored_payload_or_family_mutation_atomically() {
    let (_, mut state) = fixture();
    let token = outgoing(
        &mut state,
        PresentationSlot::Sprite { id: "hero".into() },
        TaskOwner::Game,
    );
    let original = task(&state, &token).effect.clone().unwrap();
    let mut changed_path = original.clone();
    if let RetiredModel::Sprite { state, .. } =
        &mut changed_path.presentation.as_mut().unwrap().model
    {
        state.emotion = Some("replacement".into());
    }
    let mut changed_style = original.clone();
    if let RetiredModel::Sprite { style, .. } =
        &mut changed_style.presentation.as_mut().unwrap().model
    {
        style.rotation = -0.0;
    }
    let mut changed_family = original.clone();
    changed_family.parameters = EffectParameters::AudioFade {
        from: 1.0,
        to: 0.0,
        despawn_on_done: true,
    };
    let active = state.scene_execution.as_ref().unwrap().owner();
    let before = bytes(&state);
    for changed in [changed_path, changed_style, changed_family] {
        assert!(state
            .scene_execution
            .as_mut()
            .unwrap()
            .resources
            .capture_effect(&token, EPOCH, active, changed)
            .is_err());
        assert_eq!(bytes(&state), before);
    }
    let mut stale = token.clone();
    stale.task.revision += 1;
    let mut nan = original.clone();
    nan.clock.elapsed = f64::NAN;
    assert!(!state
        .scene_execution
        .as_mut()
        .unwrap()
        .resources
        .capture_effect(&stale, EPOCH, active, nan)
        .unwrap());
    assert_eq!(bytes(&state), before);
    assert!(state
        .scene_execution
        .as_mut()
        .unwrap()
        .resources
        .capture_effect_base(
            &token,
            EPOCH,
            active,
            EffectProjectionBase::VisualTransform(Default::default())
        )
        .unwrap());
    assert!(task(&state, &token).effect.as_ref().unwrap().base_resolved);
    assert_eq!(
        task(&state, &token).effect.as_ref().unwrap().presentation,
        original.presentation
    );
}

#[test]
fn retired_clock_save_load_and_return_uses_one_core_authority() {
    let (program, mut state) = fixture();
    let parent = owner(&state);
    let token = outgoing(&mut state, PresentationSlot::Background, parent);
    let effect = state
        .scene_execution
        .as_mut()
        .unwrap()
        .resources
        .tasks
        .iter_mut()
        .find(|task| task.key == token.task.key)
        .unwrap()
        .effect
        .as_mut()
        .unwrap();
    effect.clock = effect.clock.advanced(1.125).unwrap();
    call(&program, &mut state);
    let before = bytes(&state);
    let save = portable_save(&state,"retired");
    let encoded = serde_json::to_vec(&save).unwrap();
    let decoded: SaveData = serde_json::from_slice(&encoded).unwrap();
    let mut engine = Engine::from_program(
        rvn_parser::parse_program(SOURCE).unwrap(),
        Host {music_checkpoint_fixture:true,..Default::default()},
        0,
    )
    .unwrap();
    engine.load_data(decoded).unwrap();
    assert_eq!(engine.renderer.batches.len(), 1);
    assert_eq!(
        task(&engine.state, &token).effect,
        task(&state, &token).effect
    );
    assert_eq!(bytes(&state), before);
    returned(&program, &mut engine.state);
    let restored = task(&engine.state, &token);
    assert!(!restored.parked);
    assert_eq!(restored.effect.as_ref().unwrap().clock.elapsed, 1.125);
    assert!(restored.revision > token.task.revision);
    assert!(!registry(&engine.state).accepts(
        &token,
        EPOCH,
        engine.state.scene_execution.as_ref().unwrap().owner()
    ));
}

#[test]
fn retired_completion_is_due_exact_once_and_cannot_delete_current() {
    let (_, mut state) = fixture();
    let token = outgoing(&mut state, PresentationSlot::Background, TaskOwner::Game);
    let current = state.background_image.clone();
    let current_target = registry(&state)
        .target(&TaskOwner::Game, TargetKind::Background, "background")
        .unwrap()
        .clone();
    let before = bytes(&state);
    assert!(complete_due_effect(&mut state, &token, EPOCH)
        .unwrap()
        .is_none());
    assert_eq!(bytes(&state), before);
    due(&mut state, &token);
    let done = complete_due_effect(&mut state, &token, EPOCH)
        .unwrap()
        .unwrap();
    assert_eq!(done.retired_target, Some(token.target.clone()));
    assert_eq!(state.background_image, current);
    assert_eq!(
        registry(&state).target(&TaskOwner::Game, TargetKind::Background, "background"),
        Some(&current_target)
    );
    assert!(!registry(&state)
        .targets
        .iter()
        .any(|record| record.identity == token.target));
    let after = bytes(&state);
    assert!(complete_due_effect(&mut state, &token, EPOCH)
        .unwrap()
        .is_none());
    assert_eq!(bytes(&state), after);
}

#[test]
fn frame_callback_replacement_invalidates_captured_due_retired() {
    let (_, mut state) = fixture();
    let old = outgoing(&mut state, PresentationSlot::Background, TaskOwner::Game);
    due(&mut state, &old);
    cancel_retired_slot(
        &mut state,
        &PresentationSlot::Background,
        CancellationReason::TargetReplaced,
    )
    .unwrap();
    state.background_image = "images/new.png".into();
    replace_target(
        &mut state,
        TaskOwner::Game,
        TargetKind::Background,
        "background",
    )
    .unwrap();
    let new = outgoing(&mut state, PresentationSlot::Background, TaskOwner::Game);
    let before = bytes(&state);
    assert!(complete_due_effect(&mut state, &old, EPOCH)
        .unwrap()
        .is_none());
    assert_eq!(bytes(&state), before);
    assert!(registry(&state).accepts(&new, EPOCH, state.scene_execution.as_ref().unwrap().owner()));
    assert_ne!(new.target, old.target);
}

#[test]
fn retired_abandon_and_portability_remove_only_exact_owner() {
    let (program, mut state) = fixture();
    let parent = owner(&state);
    let parent_token = outgoing(&mut state, PresentationSlot::Background, parent);
    let game = outgoing(&mut state, PresentationSlot::Music, TaskOwner::Game);
    let host = outgoing(
        &mut state,
        PresentationSlot::Music,
        TaskOwner::Host {
            role: "pause".into(),
        },
    );
    call(&program, &mut state);
    let child = owner(&state);
    let child_token = outgoing(&mut state, PresentationSlot::Background, child);
    let save = portable_save(&state,"portable");
    let portable = save.scene_execution.as_ref().unwrap();
    assert!(portable
        .resources
        .tasks
        .iter()
        .any(|task| task.key == parent_token.task.key));
    assert!(portable
        .resources
        .tasks
        .iter()
        .any(|task| task.key == game.task.key));
    assert!(!portable
        .resources
        .targets
        .iter()
        .any(|record| record.identity == host.target));
    assert!(registry(&state)
        .targets
        .iter()
        .any(|record| record.identity == host.target));
    abandon_current(&mut state).unwrap();
    assert!(!registry(&state)
        .targets
        .iter()
        .any(|record| record.identity == child_token.target));
    assert!(registry(&state)
        .targets
        .iter()
        .any(|record| record.identity == parent_token.target));
    assert!(registry(&state)
        .targets
        .iter()
        .any(|record| record.identity == game.target));
    assert!(registry(&state)
        .targets
        .iter()
        .any(|record| record.identity == host.target));
}

#[test]
fn retired_caps_invalid_payload_and_renderer_refusal_leave_authority_exact() {
    let (_, mut state) = fixture();
    let source = snapshot_retired_source(&state, &PresentationSlot::Background)
        .unwrap()
        .unwrap();
    for _ in 0..MAX_MOTIONS {
        register_retired_effect(
            &mut state,
            EPOCH,
            TaskOwner::Game,
            source.clone(),
            visual(3.0, false),
        )
        .unwrap()
        .unwrap();
    }
    let before = bytes(&state);
    assert!(register_retired_effect(
        &mut state,
        EPOCH,
        TaskOwner::Game,
        source.clone(),
        visual(3.0, false)
    )
    .is_err());
    assert_eq!(bytes(&state), before);
    let (_, mut invalid) = fixture();
    let initial = bytes(&invalid);
    let mut forged_owner = source.clone();
    forged_owner.original_target.owner = owner(&invalid);
    assert!(register_retired_effect(
        &mut invalid,
        EPOCH,
        TaskOwner::Game,
        forged_owner,
        visual(3.0, false)
    )
    .is_err());
    assert_eq!(bytes(&invalid), initial);
    let mut bad = source.clone();
    if let RetiredModel::Background { path, .. } = &mut bad.model {
        *path = "../outside.png".into();
    }
    assert!(register_retired_effect(
        &mut invalid,
        EPOCH,
        TaskOwner::Game,
        bad,
        visual(3.0, false)
    )
    .is_err());
    assert_eq!(bytes(&invalid), initial);
    invalid
        .scene_execution
        .as_mut()
        .unwrap()
        .resources
        .next_incarnation = u64::MAX;
    let before = bytes(&invalid);
    assert!(register_retired_effect(
        &mut invalid,
        EPOCH,
        TaskOwner::Game,
        source.clone(),
        visual(3.0, false)
    )
    .is_err());
    assert_eq!(bytes(&invalid), before);
    let (_, mut zero) = fixture();
    let before = bytes(&zero);
    assert!(register_retired_effect(
        &mut zero,
        EPOCH,
        TaskOwner::Game,
        source,
        visual(0.0, false)
    )
    .unwrap()
    .is_none());
    assert_eq!(bytes(&zero), before);
    let (_, mut asset_budget) = fixture();
    // Invisible authored compositions still own asset references. Their 4093
    // paths plus current BG/Music leave one slot for an outgoing simple sprite.
    for character in 0..32 {
        let count = if character == 31 { 125 } else { 128 };
        let layers: Vec<_> = (0..count).map(|layer| serde_json::json!({
            "id":format!("layer_{layer}"), "image":format!("images/unused_{character}_{layer}.png")
        })).collect();
        let definition = rvn_ui::composition::Composition::parse(serde_json::json!({
            "size":[64,64],"defaults":{},"layers":layers
        }))
        .unwrap();
        asset_budget
            .layered
            .compose(format!("inactive_{character}"), definition)
            .unwrap();
    }
    sync_current(&mut asset_budget).unwrap();
    outgoing(
        &mut asset_budget,
        PresentationSlot::Sprite { id: "hero".into() },
        TaskOwner::Game,
    );
    asset_budget.sprites.get_mut("hero").unwrap().emotion = Some("another".into());
    let source = snapshot_retired_source(
        &asset_budget,
        &PresentationSlot::Sprite { id: "hero".into() },
    )
    .unwrap()
    .unwrap();
    let before = bytes(&asset_budget);
    let error = register_retired_effect(
        &mut asset_budget,
        EPOCH,
        TaskOwner::Game,
        source,
        visual(3.0, false),
    )
    .unwrap_err();
    assert!(error.contains("Aggregate saved resources"), "{error}");
    assert_eq!(bytes(&asset_budget), before);
    let save = portable_save(&state,"refused");
    let mut engine = Engine::from_program(
        rvn_parser::parse_program(SOURCE).unwrap(),
        Host {
            reject: true,
            music_checkpoint_fixture:true,
            ..Default::default()
        },
        0,
    )
    .unwrap();
    let before = bytes(&engine.state);
    assert!(engine.load_data(save).is_err());
    assert_eq!(bytes(&engine.state), before);
    assert!(engine.renderer.batches.is_empty());
}

#[test]
fn retired_projection_descriptor_never_rebases_from_live_homonym() {
    let (_, mut state) = fixture();
    let token = outgoing(
        &mut state,
        PresentationSlot::Sprite { id: "hero".into() },
        TaskOwner::Game,
    );
    let frozen = task(&state, &token)
        .effect
        .as_ref()
        .unwrap()
        .presentation
        .clone()
        .unwrap();
    state.sprites.get_mut("hero").unwrap().emotion = Some("child".into());
    replace_target(&mut state, TaskOwner::Game, TargetKind::Sprite, "hero").unwrap();
    let new_target = registry(&state)
        .target(&TaskOwner::Game, TargetKind::Sprite, "hero")
        .unwrap();
    assert_ne!(new_target, &frozen.original_target.identity);
    assert!(!registry(&state)
        .targets
        .iter()
        .any(|record| record.identity == frozen.original_target.identity));
    let mut two_modes = frozen.clone();
    if let RetiredModel::Sprite { sample, .. } = &mut two_modes.model {
        sample.root_motion = Some(FrozenMotionSample {
            pose: rvn_ui::motion::Pose {
                x: 21.0,
                frame: Some("images/normal.png".into()),
                ..Default::default()
            },
            reduced_pose: rvn_ui::motion::Pose {
                x: 7.0,
                frame: Some("images/reduced.png".into()),
                ..Default::default()
            },
        });
    }
    two_modes.validate_structure().unwrap();
    assert!(two_modes.resource_paths().contains("images/normal.png"));
    assert!(two_modes.resource_paths().contains("images/reduced.png"));
    let wire = serde_json::to_vec(&two_modes).unwrap();
    let decoded: RetiredPresentation = serde_json::from_slice(&wire).unwrap();
    assert_eq!(decoded, two_modes);
    if let RetiredModel::Sprite { sample, .. } = &mut two_modes.model {
        let motion = sample.root_motion.as_mut().unwrap();
        assert_eq!(motion.selected(false).x, 21.0);
        assert_eq!(motion.selected(true).x, 7.0);
        motion.reduced_pose.x = f32::NAN;
    }
    assert!(two_modes.validate_structure().is_err());
    assert_eq!(
        task(&state, &token)
            .effect
            .as_ref()
            .unwrap()
            .presentation
            .as_ref(),
        Some(&frozen)
    );
    validate_current_resource_state(&state).unwrap();
    let wire = serde_json::to_value(task(&state, &token).effect.as_ref().unwrap()).unwrap();
    assert_eq!(
        wire["presentation"]["original_target"]["identity"]["kind"],
        "sprite"
    );
    assert_eq!(token.target.kind, TargetKind::PresentationRetired);
    assert!(task(&state, &token).dependencies.is_empty());
    let ordinary = serde_json::to_value(visual(3.0, false)).unwrap();
    assert!(ordinary.get("presentation").is_none());
}

#[test]
fn retired_music_raw_save_and_unmeasured_renderer_remain_refused_without_state_or_history_mutation() {
    let (_,state)=fixture();
    let mut engine=Engine::from_program(rvn_parser::parse_program(SOURCE).unwrap(),Host::default(),8).unwrap();
    engine.state=state;
    let before=bytes(&engine.state);
    let history=engine.history.entries().iter().map(|entry|format!("{entry:?}")).collect::<Vec<_>>();
    assert!(engine.snapshot_for_save().is_err());
    assert!(SaveData::try_from_state(&engine.state,0,"unmeasured".into(),"story.rvn".into()).is_err());
    assert_eq!(bytes(&engine.state),before);
    assert_eq!(engine.history.entries().iter().map(|entry|format!("{entry:?}")).collect::<Vec<_>>(),history);
    assert!(engine.renderer.batches.is_empty());
}
