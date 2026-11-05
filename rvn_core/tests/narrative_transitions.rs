//! Private C11 Engine contracts. NOT_RUN, no native/rendering claim.
use rvn_core::renderer::SceneRenderBatch;
use rvn_core::retired_presentations::{PresentationSlot, RetiredModel};
use rvn_core::scene_resources::{
    resource_key, EffectClock, EffectParameters, ResourceTask, ResourceTaskKind, ResourceToken,
    SavedEffect, SpriteEffectKind, TargetKind, VisualTransition,
};
use rvn_core::scene_tasks::TaskOwner;
use rvn_core::{Engine, GameFrameReports, Interaction, Renderer, SpriteState};
use rvn_parser::{Hotspot, Position, Transition};

#[derive(Default)]
struct Host {
    batches: Vec<SceneRenderBatch>,
    reject: bool,
    legacy: Vec<String>,
    music_checkpoint_fixture:bool,
}
impl Renderer for Host {
    fn capture_audio_checkpoint(&self,state:&rvn_core::GameState,instance:u64,epoch:u64)->Result<rvn_core::audio_checkpoint::AudioCheckpointSnapshot,String> {
        let shots=rvn_core::audio_playback::describe(state,epoch)?;
        let music=rvn_core::audio_checkpoint::music_views(state,epoch)?;
        if !shots.is_empty() || (!music.is_empty() && !self.music_checkpoint_fixture) {return Err("This Core fixture has no measured music reader".into());}
        let mut snapshot=rvn_core::audio_checkpoint::AudioCheckpointSnapshot{instance_id:instance,runtime_epoch:epoch,rows:Vec::new(),music:Vec::new()};
        for view in music {
            if view.file!="music/theme.ogg" {return Err("Music descriptor fixture only declares its exact test asset".into());}
            snapshot.music.push(rvn_core::audio_checkpoint::MusicCheckpoint{target:view.authority.target,file:view.file,checkpoint:fixture_music_descriptor()});
        }
        Ok(snapshot)
    }
    fn validate_audio_restore(&self,state:&rvn_core::GameState)->Result<(),String> {
        rvn_core::audio_checkpoint::require_saved_checkpoints(state)?;
        if !rvn_core::audio_playback::describe(state,0)?.is_empty() {return Err("This Core fixture has no one-shot reader".into());}
        for view in rvn_core::audio_checkpoint::music_views(state,0)? {
            if !self.music_checkpoint_fixture || view.file!="music/theme.ogg" || view.checkpoint.as_ref()!=Some(&fixture_music_descriptor()) {return Err("Undeclared Core music descriptor fixture".into());}
        }
        Ok(())
    }
    fn supports_composable_motion(&self) -> bool {
        true
    }
    fn supports_layered_characters(&self) -> bool {
        true
    }
    fn validate_scene_batch(&self, _: &SceneRenderBatch) -> Result<(), String> {
        if self.reject {
            Err("C11 test rejected final candidate".into())
        } else {
            Ok(())
        }
    }
    fn publish_scene_batch(&mut self, batch: SceneRenderBatch) {
        self.batches.push(batch);
    }
    fn set_background(&mut self, path: &str, _: &Transition) {
        self.legacy.push(format!("bg:{path}"));
    }
    fn show_sprite(
        &mut self,
        id: &str,
        _: Option<&str>,
        _: &Position,
        _: &Transition,
        _: Option<&SpriteState>,
    ) {
        self.legacy.push(format!("show:{id}"));
    }
    fn hide_sprite(&mut self, id: &str, _: &Transition, _: &SpriteState) {
        self.legacy.push(format!("hide:{id}"));
    }
    fn move_sprite(&mut self, id: &str, _: &Position, _: &Transition, _: &SpriteState) {
        self.legacy.push(format!("move:{id}"));
    }
    fn show_dialogue(&mut self, _: Option<&str>, _: &str) {}
    fn show_choice(&mut self, _: &[String]) -> usize {
        0
    }
    fn show_imagemap(&mut self, _: &str, _: Option<&str>, _: &[Hotspot]) -> usize {
        0
    }
    fn music_play(&mut self, file: &str, _: &Transition, _: Option<&str>) {
        self.legacy.push(format!("music:{file}"));
    }
    fn music_stop(&mut self, _: &Transition) {
        self.legacy.push("stop".into());
    }
    fn set_typewriter_config(&mut self, speed_cps: f32) {
        self.legacy.push(format!("typewriter:{speed_cps}"));
    }
    fn music_set_volume(&mut self, _: f32) {}
    fn sfx_play(&mut self, _: &str, _: &Transition) {}
    fn sfx_stop(&mut self, _: &str, _: &Transition) {}
}
// Deliberate Core descriptor input, never a synthetic success for a real reader.
// Only the named save/load fade contract explicitly opts into this boundary.
fn fixture_music_descriptor()->rvn_core::audio_checkpoint::AudioCheckpoint {
    rvn_core::audio_checkpoint::AudioCheckpoint{version:1,asset_sha256:[0x41;32],pcm_sha256:[0x52;32],channels:2,sample_rate:48_000,total_samples:4,next_sample:2,next_channel:0,looping:true,loops_completed:0}
}
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
fn at_first(source: &str) -> Engine<Host> {
    let mut game = engine(source);
    assert_eq!(dialogue(&mut game), "before");
    game
}
fn ready(game: &mut Engine<Host>) {
    game.advance_dialogue().unwrap();
    game.renderer.batches.clear();
    game.renderer.legacy.clear();
}
fn command(game: &mut Engine<Host>) {
    ready(game);
    game.step_silent().unwrap();
    assert_eq!(game.renderer.batches.len(), 1);
    assert!(game.renderer.legacy.is_empty());
    let batch = game.renderer.batches.last().unwrap();
    assert_eq!(
        serde_json::to_value(&batch.state).unwrap(),
        serde_json::to_value(&game.state).unwrap()
    );
}
fn scene_owner(game: &Engine<Host>) -> TaskOwner {
    TaskOwner::Scene {
        activation: game
            .state
            .scene_execution
            .as_ref()
            .unwrap()
            .owner()
            .unwrap(),
    }
}
fn target(
    game: &Engine<Host>,
    kind: TargetKind,
    alias: &str,
) -> rvn_core::scene_resources::TargetIdentity {
    game.state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .target(&TaskOwner::Game, kind, alias)
        .unwrap()
        .clone()
}
fn outgoing(game: &Engine<Host>, slot: &PresentationSlot) -> ResourceTask {
    let tasks: Vec<_> = game
        .state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .tasks
        .iter()
        .filter(|task| {
            task.effect
                .as_ref()
                .and_then(|effect| effect.presentation.as_ref())
                .is_some_and(|payload| payload.original_slot == *slot)
        })
        .cloned()
        .collect();
    assert_eq!(tasks.len(), 1);
    tasks[0].clone()
}
fn incoming(game: &Engine<Host>, kind: TargetKind, alias: &str) -> ResourceTask {
    let current = target(game, kind, alias);
    let tasks: Vec<_> = game
        .state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .tasks
        .iter()
        .filter(|task| {
            task.target == current
                && task.effect.as_ref().is_some_and(|effect| {
                    effect.presentation.is_none()
                        && matches!(
                            effect.parameters,
                            EffectParameters::VisualFade { .. }
                                | EffectParameters::AudioFade { .. }
                        )
                })
        })
        .cloned()
        .collect();
    assert_eq!(tasks.len(), 1);
    tasks[0].clone()
}
fn token(game: &Engine<Host>, task: &ResourceTask) -> ResourceToken {
    game.state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .effect_token(
            game.runtime_epoch(),
            &task.key,
            game.state.scene_execution.as_ref().unwrap().owner(),
        )
        .unwrap()
}
fn frame(game: &mut Engine<Host>, seconds: f64) {
    game.renderer.batches.clear();
    game.tick_game_frame(seconds, GameFrameReports::default())
        .unwrap();
}
fn wire(game: &Engine<Host>) -> serde_json::Value {
    serde_json::json!({"state":&game.state,"history":game.history.entries().iter().map(|entry|(&entry.state,&entry.display)).collect::<Vec<_>>(),
        "runtime_epoch":game.runtime_epoch(),"interface_epoch":game.interface_epoch(),"legacy":&game.renderer.legacy,
        "batches":game.renderer.batches.iter().map(|batch|(&batch.state,&batch.previous,batch.runtime_epoch,batch.interface_epoch,batch.video_epoch)).collect::<Vec<_>>()})
}
fn visual(task: &ResourceTask, from: f32, to: f32, despawn: bool) {
    let effect = task.effect.as_ref().unwrap();
    assert!(!effect.clock.looping);
    match &effect.parameters {
        EffectParameters::VisualFade {
            from: a,
            to: b,
            despawn_on_finish,
            ..
        } => {
            assert_eq!(a.to_bits(), from.to_bits());
            assert_eq!(b.to_bits(), to.to_bits());
            assert_eq!(*despawn_on_finish, despawn);
        }
        other => panic!("Expected visual fade, got {other:?}"),
    }
}

#[test]
fn first_show_creates_only_exact_incoming_and_one_complete_command_batch() {
    let mut game = at_first(
        "scene label P\n\"before\"\neileen.show(\"smile\") at left with fade(2000)\n\"after\"\n",
    );
    command(&mut game);
    let current = target(&game, TargetKind::Sprite, "eileen");
    let effect = incoming(&game, TargetKind::Sprite, "eileen");
    visual(&effect, 0.0, 1.0, false);
    assert_eq!(effect.key.owner, scene_owner(&game));
    assert_eq!(effect.target, current);
    assert_eq!(effect.effect.as_ref().unwrap().clock.duration, 2.0);
    assert_eq!(effect.effect.as_ref().unwrap().clock.elapsed, 0.0);
    assert!(effect.dependencies.is_empty());
    assert!(game
        .state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .tasks
        .iter()
        .all(|task| task.target.kind != TargetKind::PresentationRetired));
}

#[test]
fn visible_show_freezes_old_style_and_pose_keeps_incarnation_and_motion_but_resets_current_style() {
    let mut game = at_first("scene label P\neileen.show(\"old\") at left\neileen.effect(scale:0.7,tint:\"#ffffff66\")\neileen.animate(\"shake\",duration:10)\nmotion.play(\"sprite:eileen\",motion_pause(10))\n\"before\"\neileen.show(\"new\") at right with fade(2000)\n\"after\"\n");
    frame(&mut game, 0.5);
    let current = target(&game, TargetKind::Sprite, "eileen");
    let motion_before = game
        .scene_resource_token(ResourceTaskKind::Motion, "sprite:eileen")
        .unwrap();
    let old_style = game.state.scene_execution.as_ref().unwrap().sprite_styles["eileen"].clone();
    let old_animation = game
        .scene_resource_token(ResourceTaskKind::Effect, "sprite/eileen/animation")
        .unwrap();
    command(&mut game);
    assert_eq!(target(&game, TargetKind::Sprite, "eileen"), current);
    assert_eq!(
        game.scene_resource_token(ResourceTaskKind::Motion, "sprite:eileen")
            .unwrap(),
        motion_before
    );
    assert_eq!(game.state.motions.tracks["sprite:eileen"].elapsed, 0.5);
    assert!(!game
        .state
        .scene_execution
        .as_ref()
        .unwrap()
        .sprite_styles
        .contains_key("eileen"));
    assert!(game
        .scene_resource_token(ResourceTaskKind::Effect, "sprite/eileen/animation")
        .is_none());
    let old = outgoing(
        &game,
        &PresentationSlot::Sprite {
            id: "eileen".into(),
        },
    );
    visual(&old, 1.0, 0.0, true);
    let payload = old.effect.as_ref().unwrap().presentation.as_ref().unwrap();
    assert_eq!(payload.original_target.identity, current);
    assert!(old.dependencies.is_empty());
    match &payload.model {
        RetiredModel::Sprite {
            state,
            style,
            sample,
            ..
        } => {
            assert_eq!(state.emotion.as_deref(), Some("old"));
            assert_eq!(state.position, Position::Left);
            assert_eq!(style, &old_style);
            assert_eq!(sample.sprite.as_ref().unwrap().clock.elapsed, 0.5);
            assert!(sample.root_motion.is_some());
        }
        other => panic!("Expected frozen sprite, got {other:?}"),
    }
    visual(
        &incoming(&game, TargetKind::Sprite, "eileen"),
        0.0,
        1.0,
        false,
    );
    assert!(!game
        .capture_scene_effect_base(
            &old_animation,
            rvn_core::scene_resources::EffectProjectionBase::SpriteTransform(Default::default())
        )
        .unwrap());
}

#[test]
fn hide_retires_the_visible_source_without_a_current_dependency_or_deleting_its_payload() {
    let mut game = at_first("scene label P\neileen.show(\"smile\") at left\neileen.effect(tint:\"#ffffff66\")\n\"before\"\neileen.hide() with slideleft(2000)\n\"after\"\n");
    let before = target(&game, TargetKind::Sprite, "eileen");
    command(&mut game);
    assert!(!game.state.sprites["eileen"].visible);
    assert!(game
        .state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .target(&TaskOwner::Game, TargetKind::Sprite, "eileen")
        .is_none());
    let retired = outgoing(
        &game,
        &PresentationSlot::Sprite {
            id: "eileen".into(),
        },
    );
    visual(&retired, 1.0, 0.0, true);
    assert_eq!(
        retired
            .effect
            .as_ref()
            .unwrap()
            .presentation
            .as_ref()
            .unwrap()
            .original_target
            .identity,
        before
    );
    assert!(retired.dependencies.is_empty());
    assert_ne!(retired.target, before);
    let batch = game.renderer.batches.last().unwrap();
    assert!(batch.previous.sprites["eileen"].visible);
    assert!(!batch.state.sprites["eileen"].visible);
    frame(&mut game, 0.75);
    assert_eq!(
        outgoing(
            &game,
            &PresentationSlot::Sprite {
                id: "eileen".into()
            }
        )
        .effect
        .unwrap()
        .clock
        .elapsed,
        0.75
    );
}

#[test]
fn repeated_absent_hide_preserves_the_existing_retired_clock_revision_and_model() {
    let mut game = at_first("scene label P\neileen.show(\"smile\")\n\"before\"\neileen.hide() with fade(2000)\n\"after\"\neileen.hide() with fade(5000)\n\"done\"\n");
    command(&mut game);
    frame(&mut game, 0.5);
    let before = outgoing(
        &game,
        &PresentationSlot::Sprite {
            id: "eileen".into(),
        },
    );
    dialogue(&mut game);
    command(&mut game);
    assert_eq!(
        outgoing(
            &game,
            &PresentationSlot::Sprite {
                id: "eileen".into()
            }
        ),
        before
    );
}

#[test]
fn same_background_replacement_renews_current_and_retired_targets_and_supersedes_prior_exit() {
    let mut game = at_first("scene label P\nscene \"room.png\"\n\"before\"\nscene \"room.png\" with dissolve(2000)\n\"after\"\nscene \"room.png\" with dissolve(3000)\n\"done\"\n");
    let original = target(&game, TargetKind::Background, "background");
    command(&mut game);
    let current = target(&game, TargetKind::Background, "background");
    assert_ne!(current, original);
    let retired = outgoing(&game, &PresentationSlot::Background);
    assert_eq!(
        retired
            .effect
            .as_ref()
            .unwrap()
            .presentation
            .as_ref()
            .unwrap()
            .original_target
            .identity,
        original
    );
    let old_token = token(&game, &retired);
    frame(&mut game, 0.5);
    dialogue(&mut game);
    command(&mut game);
    assert_ne!(target(&game, TargetKind::Background, "background"), current);
    assert_ne!(
        outgoing(&game, &PresentationSlot::Background).target,
        retired.target
    );
    assert!(!game.complete_scene_effect(&old_token).unwrap());
    visual(
        &incoming(&game, TargetKind::Background, "background"),
        0.0,
        1.0,
        false,
    );
}

#[test]
fn config_bg_is_an_instant_transaction_that_cancels_only_the_background_slot() {
    let mut game = at_first("scene label P\nscene \"old.png\"\neileen.show(\"smile\")\neileen.hide() with fade(5000)\n\"before\"\nscene \"new.png\" with fade(4000)\n\"after\"\nbg: \"configured.png\"\n\"done\"\n");
    command(&mut game);
    let unrelated = outgoing(
        &game,
        &PresentationSlot::Sprite {
            id: "eileen".into(),
        },
    );
    let previous = target(&game, TargetKind::Background, "background");
    dialogue(&mut game);
    command(&mut game);
    assert_eq!(game.state.background_image, "configured.png");
    assert_eq!(game.state.last_transition, Transition::None);
    assert_ne!(
        target(&game, TargetKind::Background, "background"),
        previous
    );
    assert_eq!(
        outgoing(
            &game,
            &PresentationSlot::Sprite {
                id: "eileen".into()
            }
        ),
        unrelated
    );
    assert!(game
        .state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .tasks
        .iter()
        .all(|task| task
            .effect
            .as_ref()
            .and_then(|effect| effect.presentation.as_ref())
            .is_none_or(|payload| payload.original_slot != PresentationSlot::Background)));
    assert!(game
        .state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .tasks
        .iter()
        .all(
            |task| task.target != target(&game, TargetKind::Background, "background")
                || task.effect.is_none()
        ));
}

#[test]
fn same_music_replacement_keeps_absolute_incoming_volume_and_frozen_outgoing_gain_once() {
    let mut game = at_first("scene label P\nmusic.volume(0.4)\nmusic.play(\"theme.ogg\") with fade(4000)\n\"before\"\nmusic.play(\"theme.ogg\") with fade(2000)\n\"after\"\n");
    frame(&mut game, 2.0);
    let old = target(&game, TargetKind::Music, "music");
    command(&mut game);
    assert_ne!(target(&game, TargetKind::Music, "music"), old);
    assert_eq!(game.state.music.volume, 0.4);
    let retired = outgoing(&game, &PresentationSlot::Music);
    let effect = retired.effect.as_ref().unwrap();
    let payload = effect.presentation.as_ref().unwrap();
    assert_eq!(payload.original_target.identity, old);
    assert_eq!(
        payload
            .outgoing_music_gain(effect)
            .unwrap()
            .unwrap()
            .to_bits(),
        0.2f32.to_bits()
    );
    match &effect.parameters {
        EffectParameters::AudioFade {
            from,
            to,
            despawn_on_done,
        } => {
            assert_eq!((*from, *to), (1.0, 0.0));
            assert!(*despawn_on_done);
        }
        other => panic!("Expected outgoing factor, got {other:?}"),
    }
    match &incoming(&game, TargetKind::Music, "music")
        .effect
        .unwrap()
        .parameters
    {
        EffectParameters::AudioFade {
            from,
            to,
            despawn_on_done,
        } => {
            assert_eq!((*from, *to), (0.0, 0.4));
            assert!(!*despawn_on_done);
        }
        other => panic!("Expected absolute incoming audio, got {other:?}"),
    }
    frame(&mut game, 1.0);
    let retired_after = outgoing(&game, &PresentationSlot::Music);
    let effect = retired_after.effect.as_ref().unwrap();
    assert_eq!(
        effect
            .presentation
            .as_ref()
            .unwrap()
            .outgoing_music_gain(effect)
            .unwrap()
            .unwrap()
            .to_bits(),
        0.1f32.to_bits()
    );
}

#[test]
fn music_stop_freezes_the_source_gain_and_later_volume_does_not_rebase_that_exit() {
    let mut game = at_first("scene label P\nmusic.volume(0.4)\nmusic.play(\"theme.ogg\") with fade(4000)\n\"before\"\nmusic.stop() with fade(2000)\n\"after\"\nmusic.volume(0.9)\n\"done\"\n");
    frame(&mut game, 2.0);
    command(&mut game);
    let retired = outgoing(&game, &PresentationSlot::Music);
    assert!(game.state.music.current_file.is_none());
    assert!(retired.dependencies.is_empty());
    let effect = retired.effect.as_ref().unwrap();
    assert_eq!(
        effect
            .presentation
            .as_ref()
            .unwrap()
            .outgoing_music_gain(effect)
            .unwrap()
            .unwrap()
            .to_bits(),
        0.2f32.to_bits()
    );
    dialogue(&mut game);
    command(&mut game);
    assert_eq!(outgoing(&game, &PresentationSlot::Music), retired);
    assert_eq!(game.state.music.volume, 0.9);
    frame(&mut game, 1.0);
    let task = outgoing(&game, &PresentationSlot::Music);
    let effect = task.effect.as_ref().unwrap();
    assert_eq!(
        effect
            .presentation
            .as_ref()
            .unwrap()
            .outgoing_music_gain(effect)
            .unwrap()
            .unwrap()
            .to_bits(),
        0.1f32.to_bits()
    );
}

#[test]
fn repeated_absent_stop_does_not_replace_its_source_payload_or_restart_the_fade() {
    let mut game = at_first("scene label P\nmusic.play(\"theme.ogg\")\n\"before\"\nmusic.stop() with fade(2000)\n\"after\"\nmusic.stop() with fade(9000)\n\"done\"\n");
    command(&mut game);
    frame(&mut game, 0.75);
    let retired = outgoing(&game, &PresentationSlot::Music);
    dialogue(&mut game);
    command(&mut game);
    assert_eq!(outgoing(&game, &PresentationSlot::Music), retired);
}

#[test]
fn authored_story_stop_preserves_autonomous_host_exit_but_revokes_host_current_dependency() {
    use rvn_core::retired_presentations::snapshot_retired_source;
    let mut game = at_first("scene label P\nmusic.play(\"theme.ogg\")\n\"before\"\nmusic.stop() with fade(2000)\n\"after\"\n");
    let host = TaskOwner::Host {
        role: rvn_ui::PageRole::Pause.id().into(),
    };
    let payload = snapshot_retired_source(&game.state, &PresentationSlot::Music)
        .unwrap()
        .unwrap();
    let detached = SavedEffect {
        presentation: None,
        base_resolved: false,
        clock: EffectClock {
            duration: 3.0,
            elapsed: 0.0,
            looping: false,
        },
        parameters: EffectParameters::AudioFade {
            from: 1.0,
            to: 0.0,
            despawn_on_done: true,
        },
    };
    let epoch = game.runtime_epoch();
    let host_exit = rvn_core::scene_resources::register_retired_effect(
        &mut game.state,
        epoch,
        host.clone(),
        payload,
        detached,
    )
    .unwrap()
    .unwrap();
    let host_before = game
        .state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .tasks
        .iter()
        .find(|task| task.key == host_exit.task.key)
        .unwrap()
        .clone();
    let current = target(&game, TargetKind::Music, "music");
    let physical = SavedEffect {
        presentation: None,
        base_resolved: false,
        clock: EffectClock {
            duration: 3.0,
            elapsed: 0.0,
            looping: false,
        },
        parameters: EffectParameters::AudioFade {
            from: 0.2,
            to: 0.8,
            despawn_on_done: false,
        },
    };
    let host_current = game
        .register_scene_effect(
            resource_key(host, ResourceTaskKind::Effect, "host/current/music"),
            current.clone(),
            physical,
        )
        .unwrap();
    command(&mut game);
    let registry = &game.state.scene_execution.as_ref().unwrap().resources;
    assert_eq!(
        registry
            .tasks
            .iter()
            .find(|task| task.key == host_exit.task.key),
        Some(&host_before)
    );
    assert!(registry.tasks.iter().all(|task| task.target != current));
    let story: Vec<_> = registry
        .tasks
        .iter()
        .filter(|task| {
            !matches!(task.key.owner, TaskOwner::Host { .. })
                && task
                    .effect
                    .as_ref()
                    .and_then(|effect| effect.presentation.as_ref())
                    .is_some_and(|payload| payload.original_slot == PresentationSlot::Music)
        })
        .collect();
    assert_eq!(story.len(), 1);
    assert_eq!(story[0].key.owner, scene_owner(&game));
    assert!(story[0].dependencies.is_empty());
    assert!(!game
        .capture_scene_effect_base(
            &host_current,
            rvn_core::scene_resources::EffectProjectionBase::AudioResolved
        )
        .unwrap());
}

#[test]
fn scene_owned_retired_clock_parks_on_call_and_resumes_only_after_exact_return() {
    let mut game = at_first("scene label P\neileen.show(\"smile\")\n\"before\"\neileen.hide() with fade(2000)\n\"after\"\ncall C\n\"returned\"\njump done\nscene label C\n\"child\"\nreturn\nlabel done\n");
    command(&mut game);
    frame(&mut game, 0.5);
    let task = outgoing(
        &game,
        &PresentationSlot::Sprite {
            id: "eileen".into(),
        },
    );
    let parent = task.key.owner.clone();
    dialogue(&mut game);
    game.advance_dialogue().unwrap();
    assert_eq!(dialogue(&mut game), "child");
    let parked = outgoing(
        &game,
        &PresentationSlot::Sprite {
            id: "eileen".into(),
        },
    );
    assert!(parked.parked);
    assert_eq!(parked.key.owner, parent);
    frame(&mut game, 1.0);
    assert_eq!(
        outgoing(
            &game,
            &PresentationSlot::Sprite {
                id: "eileen".into()
            }
        )
        .effect
        .unwrap()
        .clock
        .elapsed,
        0.5
    );
    game.advance_dialogue().unwrap();
    assert_eq!(dialogue(&mut game), "returned");
    assert!(
        !outgoing(
            &game,
            &PresentationSlot::Sprite {
                id: "eileen".into()
            }
        )
        .parked
    );
    frame(&mut game, 0.25);
    assert_eq!(
        outgoing(
            &game,
            &PresentationSlot::Sprite {
                id: "eileen".into()
            }
        )
        .effect
        .unwrap()
        .clock
        .elapsed,
        0.75
    );
}

#[test]
fn game_owned_previsit_exit_survives_scene_jump_while_scene_exit_is_abandoned() {
    let mut game = engine("init {scene \"initial.png\" scene \"game.png\" with fade(5000)}\nscene label P\neileen.show(\"smile\")\n\"before\"\neileen.hide() with fade(5000)\n\"after\"\njump C\nscene label C\n\"child\"\n");
    assert_eq!(dialogue(&mut game), "before");
    let background = outgoing(&game, &PresentationSlot::Background);
    assert_eq!(background.key.owner, TaskOwner::Game);
    command(&mut game);
    let scene_exit = outgoing(
        &game,
        &PresentationSlot::Sprite {
            id: "eileen".into(),
        },
    );
    let old = token(&game, &scene_exit);
    dialogue(&mut game);
    game.advance_dialogue().unwrap();
    assert_eq!(dialogue(&mut game), "child");
    frame(&mut game, 1.0);
    assert_eq!(
        outgoing(&game, &PresentationSlot::Background)
            .effect
            .unwrap()
            .clock
            .elapsed,
        1.0
    );
    assert!(!game.complete_scene_effect(&old).unwrap());
    assert!(game
        .state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .tasks
        .iter()
        .all(|task| task.target != scene_exit.target));
}

#[test]
fn save_load_preserves_partial_outgoing_without_current_replay_and_revokes_old_token() {
    let source = "scene label P\neileen.show(\"smile\")\n\"before\"\neileen.hide() with fade(2000)\n\"after\"\n";
    let mut game = at_first(source);
    command(&mut game);
    frame(&mut game, 0.75);
    let task = outgoing(
        &game,
        &PresentationSlot::Sprite {
            id: "eileen".into(),
        },
    );
    let old = token(&game, &task);
    let save = rvn_core::save::SaveData::try_from_state(
        &game.state,
        1,
        "retired".into(),
        "main.rvn".into(),
    )
    .unwrap();
    let serialized = serde_json::to_vec(&save).unwrap();
    let decoded = serde_json::from_slice(&serialized).unwrap();
    game.renderer.batches.clear();
    game.load_data(decoded).unwrap();
    assert_eq!(game.renderer.batches.len(), 1);
    assert!(game.renderer.legacy.is_empty());
    assert!(!game.state.sprites["eileen"].visible);
    assert_eq!(
        outgoing(
            &game,
            &PresentationSlot::Sprite {
                id: "eileen".into()
            }
        ),
        task
    );
    let fresh = token(&game, &task);
    assert_ne!(fresh, old);
    let before = wire(&game);
    assert!(!game
        .capture_scene_effect_base(
            &old,
            rvn_core::scene_resources::EffectProjectionBase::VisualTransform(Default::default())
        )
        .unwrap());
    assert_eq!(wire(&game), before);
    frame(&mut game, 0.25);
    assert_eq!(
        outgoing(
            &game,
            &PresentationSlot::Sprite {
                id: "eileen".into()
            }
        )
        .effect
        .unwrap()
        .clock
        .elapsed,
        1.0
    );
}

#[test]
fn renderer_refusal_leaves_current_retired_history_epochs_and_publication_byte_exact() {
    let mut game = at_first("scene label P\nscene \"old.png\"\n\"before\"\nscene \"new.png\" with fade(2000)\n\"after\"\n");
    ready(&mut game);
    game.renderer.reject = true;
    let before = wire(&game);
    assert!(game.step_silent().is_err());
    assert_eq!(wire(&game), before);
    assert!(game.renderer.batches.is_empty());
    game.renderer.reject = false;
    game.step_silent().unwrap();
    assert_eq!(game.renderer.batches.len(), 1);
    assert_eq!(game.state.background_image, "new.png");
}

#[test]
fn transition_clock_or_incarnation_exhaustion_cannot_publish_a_half_replacement() {
    let mut invalid = at_first("scene label P\nscene \"old.png\"\n\"before\"\nscene \"new.png\" with fade(4294967295)\n\"after\"\n");
    ready(&mut invalid);
    let before = wire(&invalid);
    assert!(invalid.step_silent().is_err());
    assert_eq!(wire(&invalid), before);
    let mut exhausted = at_first("scene label P\nscene \"old.png\"\n\"before\"\nscene \"new.png\" with fade(2000)\n\"after\"\n");
    ready(&mut exhausted);
    exhausted
        .state
        .scene_execution
        .as_mut()
        .unwrap()
        .resources
        .next_incarnation = u64::MAX;
    let before = wire(&exhausted);
    assert!(exhausted.step_silent().is_err());
    assert_eq!(wire(&exhausted), before);
}

#[test]
fn retired_plus_incoming_budget_failure_keeps_all_prior_effects_and_current_model_exact() {
    let mut game = at_first("scene label P\neileen.show(\"smile\")\n\"before\"\neileen.show(\"new\") with fade(2000)\n\"after\"\n");
    let owner = scene_owner(&game);
    for index in 0..63 {
        let id = format!("extra_{index}");
        game.state
            .sprites
            .insert(id.clone(), SpriteState::new(None, Position::Center));
        rvn_core::scene_resources::sync_current(&mut game.state).unwrap();
        let target = target(&game, TargetKind::Sprite, &id);
        let effect = SavedEffect {
            presentation: None,
            base_resolved: false,
            clock: EffectClock {
                duration: 10.0,
                elapsed: 0.0,
                looping: false,
            },
            parameters: EffectParameters::SpriteAnimation {
                animation: SpriteEffectKind::Shake { intensity: 1.0 },
                base: Default::default(),
            },
        };
        game.register_scene_effect(
            resource_key(
                owner.clone(),
                ResourceTaskKind::Effect,
                &format!("sprite/{id}/animation"),
            ),
            target,
            effect,
        )
        .unwrap();
    }
    ready(&mut game);
    let before = wire(&game);
    assert!(game.step_silent().is_err());
    assert_eq!(wire(&game), before);
    assert_eq!(
        game.state
            .scene_execution
            .as_ref()
            .unwrap()
            .resources
            .tasks
            .iter()
            .filter(|task| task.effect.is_some())
            .count(),
        63
    );
}

#[test]
fn legacy_commands_keep_immediate_renderer_calls_and_do_not_allocate_narrative_effects() {
    let mut game = engine("label start\nscene \"room.png\" with fade(2000)\neileen.show(\"smile\") with fade(2000)\nmusic.play(\"theme.ogg\") with fade(2000)\nmusic.stop() with fade(2000)\neileen.hide() with fade(2000)\n\"legacy\"\n");
    game.step_silent().unwrap();
    assert!(game.state.scene_execution.is_none());
    assert!(game.renderer.batches.is_empty());
    assert_eq!(
        game.renderer.legacy,
        vec![
            "bg:room.png",
            "show:eileen",
            "music:music/theme.ogg",
            "stop",
            "hide:eileen"
        ]
    );
}

#[test]
fn move_remains_immediate_and_keeps_current_style_without_creating_a_transition_task() {
    let mut game = at_first("scene label P\neileen.show(\"smile\") at left\neileen.effect(scale:0.7,tint:\"#ffffff66\")\n\"before\"\neileen.move() at right with slideleft(2000)\n\"after\"\n");
    let current = target(&game, TargetKind::Sprite, "eileen");
    let style = game.state.scene_execution.as_ref().unwrap().sprite_styles["eileen"].clone();
    command(&mut game);
    assert_eq!(game.state.sprites["eileen"].position, Position::Right);
    assert_eq!(target(&game, TargetKind::Sprite, "eileen"), current);
    assert_eq!(
        game.state.scene_execution.as_ref().unwrap().sprite_styles["eileen"],
        style
    );
    assert!(game
        .state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .tasks
        .iter()
        .all(|task| task.effect.is_none()));
    assert!(game.state.motions.tracks.is_empty());
}

// Additive C12 immediate MusicVolume contracts. Existing C11 tests are intact.
fn volume_current_token_is_rejected(game: &mut Engine<Host>, old: &ResourceToken) {
    let execution = game.state.scene_execution.as_ref().unwrap();
    assert!(!execution
        .resources
        .accepts(old, game.runtime_epoch(), execution.owner()));
    assert!(execution
        .resources
        .tasks
        .iter()
        .all(|task| task.key != old.task.key));
    let before = wire(game);
    assert!(!game
        .capture_scene_effect_base(
            old,
            rvn_core::scene_resources::EffectProjectionBase::AudioResolved
        )
        .unwrap());
    assert!(!game.complete_scene_effect(old).unwrap());
    assert_eq!(wire(game), before);
}

#[test]
fn music_volume_live_supersession_preserves_current_identity_and_retired_source() {
    let mut game = at_first("scene label A\nmusic.volume(0.4)\nmusic.play(\"old.ogg\")\n\"before\"\nmusic.play(\"new.ogg\") with fade(4000)\n\"fade\"\nmusic.volume(0.8)\n\"after\"\n");
    command(&mut game);
    frame(&mut game, 1.0);
    let current = target(&game, TargetKind::Music, "music");
    let fade = incoming(&game, TargetKind::Music, "music");
    assert_eq!(fade.effect.as_ref().unwrap().clock.elapsed, 1.0);
    let old = token(&game, &fade);
    let retired = outgoing(&game, &PresentationSlot::Music);
    // A newly displayed dialogue intentionally allocates one display seed.
    // Bound the authored Volume command after that existing snapshot contract.
    let mut expected_random = game.state.random;
    let expected_display_random = rvn_core::random::RandomState::seeded(expected_random.next_u64());
    assert_eq!(dialogue(&mut game), "fade");
    assert_eq!(game.state.random, expected_random);
    assert_eq!(game.state.display_random, expected_display_random);
    let random = game.state.random.clone();
    assert_eq!(dialogue(&mut game), "fade");
    assert_eq!(game.state.random, random);
    assert_eq!(game.state.display_random, expected_display_random);
    command(&mut game);
    assert_eq!(game.state.music.volume, 0.8f32);
    assert_eq!(target(&game, TargetKind::Music, "music"), current);
    assert_eq!(
        game.state.music.current_file.as_deref(),
        Some("music/new.ogg")
    );
    assert_eq!(outgoing(&game, &PresentationSlot::Music), retired);
    assert_eq!(game.state.random, random);
    assert!(game
        .renderer
        .batches
        .last()
        .unwrap()
        .effects
        .iter()
        .all(|task| task.target != current));
    volume_current_token_is_rejected(&mut game, &old);
    assert_eq!(dialogue(&mut game), "after");
    frame(&mut game, 0.25);
    assert_eq!(game.state.music.volume, 0.8f32);
    assert!(game
        .state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .tasks
        .iter()
        .all(|task| task.target != current));
}

#[test]
fn music_volume_revokes_current_fades_for_scene_game_and_host_including_parked_parent() {
    for ownership in ["scene", "game", "host"] {
        let mut game = at_first("scene label P\nmusic.volume(0.4)\nmusic.play(\"theme.ogg\")\n\"before\"\ncall C\n\"returned\"\njump done\nscene label C\n\"child before\"\nmusic.volume(0.8)\n\"child after\"\nreturn\nlabel done\n");
        let owner = match ownership {
            "scene" => scene_owner(&game),
            "game" => TaskOwner::Game,
            "host" => TaskOwner::Host {
                role: rvn_ui::PageRole::Pause.id().into(),
            },
            _ => unreachable!(),
        };
        let current = target(&game, TargetKind::Music, "music");
        let epoch = game.runtime_epoch();
        let old = game
            .register_scene_effect(
                resource_key(owner, ResourceTaskKind::Effect, "volume/current"),
                current.clone(),
                SavedEffect {
                    presentation: None,
                    base_resolved: false,
                    clock: EffectClock {
                        duration: 4.0,
                        elapsed: 1.0,
                        looping: false,
                    },
                    parameters: EffectParameters::AudioFade {
                        from: 0.0,
                        to: 0.4,
                        despawn_on_done: false,
                    },
                },
            )
            .unwrap();
        let payload = rvn_core::retired_presentations::snapshot_retired_source(
            &game.state,
            &PresentationSlot::Music,
        )
        .unwrap()
        .unwrap();
        let host_exit = rvn_core::scene_resources::register_retired_effect(
            &mut game.state,
            epoch,
            TaskOwner::Host {
                role: rvn_ui::PageRole::Pause.id().into(),
            },
            payload,
            SavedEffect {
                presentation: None,
                base_resolved: false,
                clock: EffectClock {
                    duration: 3.0,
                    elapsed: 0.0,
                    looping: false,
                },
                parameters: EffectParameters::AudioFade {
                    from: 1.0,
                    to: 0.0,
                    despawn_on_done: true,
                },
            },
        )
        .unwrap()
        .unwrap();
        let detached = outgoing(&game, &PresentationSlot::Music);
        game.advance_dialogue().unwrap();
        assert_eq!(dialogue(&mut game), "child before");
        let execution = game.state.scene_execution.as_ref().unwrap();
        let task = execution
            .resources
            .tasks
            .iter()
            .find(|task| task.key == old.task.key)
            .unwrap();
        assert_eq!(task.parked, ownership == "scene");
        assert_eq!(task.effect.as_ref().unwrap().clock.elapsed, 1.0);
        command(&mut game);
        assert_eq!(game.state.music.volume, 0.8f32);
        assert_eq!(target(&game, TargetKind::Music, "music"), current);
        assert_eq!(outgoing(&game, &PresentationSlot::Music), detached);
        assert!(game
            .state
            .scene_execution
            .as_ref()
            .unwrap()
            .resources
            .accepts(
                &host_exit,
                game.runtime_epoch(),
                game.state.scene_execution.as_ref().unwrap().owner()
            ));
        volume_current_token_is_rejected(&mut game, &old);
        assert_eq!(dialogue(&mut game), "child after");
        game.advance_dialogue().unwrap();
        assert_eq!(dialogue(&mut game), "returned");
        assert_eq!(game.state.music.volume, 0.8f32);
        assert_eq!(target(&game, TargetKind::Music, "music"), current);
        assert_eq!(outgoing(&game, &PresentationSlot::Music), detached);
        volume_current_token_is_rejected(&mut game, &old);
    }
}

#[test]
fn music_volume_save_load_return_cannot_resurrect_superseded_parent_fade() {
    let source = "scene label P\nmusic.volume(0.4)\nmusic.play(\"theme.ogg\") with fade(4000)\n\"before\"\ncall C\n\"returned\"\njump done\nscene label C\n\"child before\"\nmusic.volume(0.8)\n\"child after\"\nreturn\nlabel done\n";
    let mut game = at_first(source);
    frame(&mut game, 1.0);
    let fade = incoming(&game, TargetKind::Music, "music");
    let old = token(&game, &fade);
    let current = target(&game, TargetKind::Music, "music");
    game.advance_dialogue().unwrap();
    assert_eq!(dialogue(&mut game), "child before");
    command(&mut game);
    assert_eq!(dialogue(&mut game), "child after");
    // Opt-in Core descriptor fixture for this fade/target contract only.
    // No Rodio/native cursor or actual audible playback is claimed here.
    game.renderer.music_checkpoint_fixture=true;
    let save = rvn_core::save::SaveData::try_from_snapshot(
        &game.snapshot_for_save().unwrap(),
        1,
        "volume".into(),
        "main.rvn".into(),
    )
    .unwrap();
    let saved = serde_json::to_vec(&save).unwrap();
    let restored: rvn_core::save::SaveData = serde_json::from_slice(&saved).unwrap();
    let epoch = game.runtime_epoch();
    game.renderer.batches.clear();
    game.load_data(restored).unwrap();
    assert_eq!(game.renderer.batches.len(), 1);
    assert_ne!(game.runtime_epoch(), epoch);
    assert_eq!(game.state.music.volume, 0.8f32);
    assert_eq!(target(&game, TargetKind::Music, "music"), current);
    volume_current_token_is_rejected(&mut game, &old);
    game.advance_dialogue().unwrap();
    assert_eq!(dialogue(&mut game), "returned");
    assert_eq!(game.state.music.volume, 0.8f32);
    assert_eq!(target(&game, TargetKind::Music, "music"), current);
    assert!(game
        .state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .tasks
        .iter()
        .all(|task| task.target != current));
    volume_current_token_is_rejected(&mut game, &old);
}

#[test]
fn music_volume_only_supersedes_new_current_after_same_file_child_replacement() {
    let mut game = at_first("scene label P\nmusic.volume(0.4)\nmusic.play(\"theme.ogg\") with fade(4000)\n\"before\"\ncall C\n\"returned\"\njump done\nscene label C\nmusic.play(\"theme.ogg\") with fade(2000)\n\"child before\"\nmusic.volume(0.8)\n\"child after\"\nreturn\nlabel done\n");
    frame(&mut game, 1.0);
    let parent_target = target(&game, TargetKind::Music, "music");
    let parent_old = token(&game, &incoming(&game, TargetKind::Music, "music"));
    game.advance_dialogue().unwrap();
    assert_eq!(dialogue(&mut game), "child before");
    let child_target = target(&game, TargetKind::Music, "music");
    assert_ne!(child_target, parent_target);
    let child_old = token(&game, &incoming(&game, TargetKind::Music, "music"));
    let retired = outgoing(&game, &PresentationSlot::Music);
    command(&mut game);
    assert_eq!(target(&game, TargetKind::Music, "music"), child_target);
    assert_eq!(game.state.music.volume, 0.8f32);
    assert_eq!(outgoing(&game, &PresentationSlot::Music), retired);
    volume_current_token_is_rejected(&mut game, &parent_old);
    volume_current_token_is_rejected(&mut game, &child_old);
    assert_eq!(dialogue(&mut game), "child after");
    game.advance_dialogue().unwrap();
    assert_eq!(dialogue(&mut game), "returned");
    assert_eq!(target(&game, TargetKind::Music, "music"), child_target);
    // The setter preserved this independent child presentation. Returning
    // abandons the child owner, so its outgoing task is now removed normally.
    assert!(game
        .state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .tasks
        .iter()
        .all(|task| task.key != retired.key));
}

#[test]
fn music_volume_renderer_refusal_preserves_fade_state_tokens_and_publication() {
    let mut game = at_first("scene label A\nmusic.volume(0.4)\nmusic.play(\"theme.ogg\") with fade(4000)\n\"before\"\nmusic.volume(0.8)\n\"after\"\n");
    frame(&mut game, 1.0);
    let old = token(&game, &incoming(&game, TargetKind::Music, "music"));
    ready(&mut game);
    game.renderer.reject = true;
    let before = wire(&game);
    assert!(game.step_silent().is_err());
    assert_eq!(wire(&game), before);
    let execution = game.state.scene_execution.as_ref().unwrap();
    assert!(execution
        .resources
        .accepts(&old, game.runtime_epoch(), execution.owner()));
    assert_eq!(game.state.music.volume, 0.4f32);
    assert!(game.renderer.batches.is_empty());
}

#[test]
fn music_volume_before_play_is_immediate_without_phantom_target_or_fade() {
    let mut game = at_first("scene label A\n\"before\"\nmusic.volume(0.8)\n\"after\"\n");
    command(&mut game);
    assert_eq!(game.state.music.volume, 0.8f32);
    assert!(game.state.music.current_file.is_none());
    let execution = game.state.scene_execution.as_ref().unwrap();
    assert!(execution
        .resources
        .target(&TaskOwner::Game, TargetKind::Music, "music")
        .is_none());
    assert!(execution.resources.tasks.iter().all(|task| !matches!(
        task.effect.as_ref().map(|effect| &effect.parameters),
        Some(EffectParameters::AudioFade { .. })
    )));
    assert!(game.renderer.batches.last().unwrap().effects.is_empty());
}


#[test]
fn authored_typewriter_parameters_are_atomic_on_renderer_refusal_and_retry() {
    for (statement, enabled, speed, effective) in [
        ("typewriter = true", true, 40, 40.0f32),
        ("typewriter = false", false, 40, 0.0),
        ("typewriter.speed(17)", true, 17, 17.0),
        ("typewriter.speed(0)", false, 0, 0.0),
    ] {
        let mut game = at_first(&format!("scene label P\n\"before\"\n{statement}\n\"after\"\n"));
        ready(&mut game);
        game.renderer.reject = true;
        let before = wire(&game);
        assert!(game.step_silent().is_err(), "{statement}");
        assert_eq!(wire(&game), before, "{statement}");
        assert!(game.renderer.batches.is_empty());
        assert!(game.renderer.legacy.is_empty());
        game.renderer.reject = false;
        game.step_silent().unwrap();
        assert_eq!(game.renderer.batches.len(), 1, "{statement}");
        assert!(game.renderer.legacy.is_empty());
        assert_eq!(game.state.typewriter.enabled, enabled);
        assert_eq!(game.state.typewriter.chars_per_sec, speed);
        assert_eq!(game.state.typewriter.effective_speed().to_bits(), effective.to_bits());
        assert_eq!(game.renderer.batches[0].state.typewriter, game.state.typewriter);
        assert_eq!(dialogue(&mut game), "after");
    }
}

#[test]
fn authored_typewriter_configuration_survives_scene_call_portable_load_and_return() {
    let mut game = at_first("scene label P\n\"before\"\ncall C\n\"returned\"\njump done\nscene label C\ntypewriter.speed(17)\n\"child\"\nreturn\nlabel done\n");
    game.advance_dialogue().unwrap();
    assert_eq!(dialogue(&mut game), "child");
    assert_eq!(game.state.typewriter.chars_per_sec, 17);
    assert!(game.state.typewriter.enabled);
    let data = rvn_core::save::SaveData::try_from_state(&game.state, 1, "typing".into(), "main.rvn".into()).unwrap();
    let encoded = serde_json::to_vec(&data).unwrap();
    let restored = serde_json::from_slice(&encoded).unwrap();
    game.renderer.batches.clear();
    game.renderer.legacy.clear();
    let epoch = game.runtime_epoch();
    game.load_data(restored).unwrap();
    assert_ne!(game.runtime_epoch(), epoch);
    assert_eq!(game.renderer.batches.len(), 1);
    assert!(game.renderer.legacy.is_empty());
    assert_eq!(game.renderer.batches[0].state.typewriter.chars_per_sec, 17);
    game.advance_dialogue().unwrap();
    assert_eq!(dialogue(&mut game), "returned");
    assert_eq!(game.state.typewriter.chars_per_sec, 17);
    assert!(game.state.typewriter.enabled);
}

#[test]
fn legacy_typewriter_commands_keep_the_existing_renderer_hook() {
    let mut game = Engine::new(rvn_parser::parse("\"before\"\ntypewriter.speed(17)\n\"after\"\n").unwrap(), Host::default(), 32).unwrap();
    assert_eq!(dialogue(&mut game), "before");
    ready(&mut game);
    game.step_silent().unwrap();
    assert_eq!(game.renderer.legacy, ["typewriter:17"]);
    assert!(game.renderer.batches.is_empty());
    assert_eq!(game.state.typewriter.chars_per_sec, 17);
    assert_eq!(dialogue(&mut game), "after");
}

#[test]
fn missing_actual_music_checkpoint_still_refuses_writer_and_load_without_changing_prior_file_or_core_history() {
    let mut game=at_first("scene label P\nmusic.play(\"theme.ogg\")\n\"before\"");
    assert!(!game.renderer.music_checkpoint_fixture);
    let directory=std::env::temp_dir().join(format!("rvn-unmeasured-music-{}-{}",std::process::id(),game.instance_id()));
    let manager=rvn_core::save::SaveManager::new(&directory,2).unwrap();
    let legacy=Engine::new(rvn_parser::parse("label P\n\"legacy\"").unwrap(),Host::default(),8).unwrap();
    manager.save(&legacy.state,1,"prior".into(),"legacy.rvn".into()).unwrap();
    let path=directory.join("slot_01.json");let prior=std::fs::read(&path).unwrap();let before=wire(&game);
    assert!(game.snapshot_for_save().is_err());assert!(game.save(&manager,1,"must refuse".into(),"main.rvn".into()).is_err());
    assert_eq!(std::fs::read(&path).unwrap(),prior);assert_eq!(wire(&game),before);
    let unmeasured=rvn_core::save::SaveData::from_state(&game.state,1,"negative DTO".into(),"main.rvn".into());
    assert!(game.load_data(unmeasured).is_err());assert_eq!(wire(&game),before);assert_eq!(std::fs::read(&path).unwrap(),prior);
    std::fs::remove_dir_all(directory).unwrap();
}
