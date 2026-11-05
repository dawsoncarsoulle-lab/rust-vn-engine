//0370 owns this target. Included fixtures compile but frame0362/native0347
// tests are filtered, not recounted as new coverage.
include!("frame_checkpoint.rs");
fn saved_host0370() -> (
    ParsedProgram,
    rvn_core::save::consumer_checkpoint::Save15,
    serde_json::Value,
) {
    let p = host_frame_program();
    let mut hot = frame_engine(p.clone());
    hot.step_until_interaction().unwrap();
    open_frame_host(&mut hot);
    let mut session = hot
        .debug_begin(
            rvn_core::engine::DebugOperation::HostFrame {
                seconds: 0.1,
                reports: Default::default(),
                audio: vec![],
            },
            vec![sample_breakpoint(&p)],
        )
        .unwrap();
    assert_eq!(
        hot.debug_step(&mut session, rvn_core::editor_debugger::StepKind::Continue)
            .unwrap()
            .0,
        rvn_core::engine::DebugProgress::Stopped
    );
    let save = hot
        .capture_save15(&p, Some(&session), 20, "Host rebasing0370".into())
        .unwrap();
    frame_finish(&mut hot, &mut session);
    (p, save, state(&hot))
}
fn recreated_host0370(p: ParsedProgram) -> Engine<Host> {
    let mut e = frame_engine(p);
    e.step_until_interaction().unwrap();
    open_frame_host(&mut e);
    e.synchronize_source_menu(
        rvn_ui::PageRole::Pause,
        None,
        Value::Dict(Default::default()),
        true,
        500,
    )
    .unwrap();
    open_frame_host(&mut e);
    e
}
#[test]
fn host0370_real_lifecycle_context_rebases_incarnations_and_orders_without_alias_authority() {
    let (p, save, expected) = saved_host0370();
    let mut cold = recreated_host0370(p.clone());
    let stale = cold.capture_ui_input_source("host").unwrap();
    let old = cold
        .state
        .ui
        .screens
        .iter()
        .find(|i| i.name == "host")
        .unwrap()
        .consumer_frames["screen:host"]
        .owner
        .target
        .clone();
    let before = state(&cold);
    assert!(cold.restore_save15(&p, save.clone()).is_err());
    assert_eq!(state(&cold), before);
    let context = cold.capture_host_lifecycle_restore_context(&p).unwrap();
    let mut resumed = cold
        .restore_save15_with_host_context(&p, save, &context)
        .unwrap()
        .unwrap();
    frame_finish(&mut cold, &mut resumed);
    assert_eq!(
        serde_json::to_value(&cold.state.vars).unwrap(),
        expected["vars"]
    );
    let host = cold
        .state
        .ui
        .screens
        .iter()
        .find(|i| i.name == "host")
        .unwrap();
    assert_eq!(host.host_role, Some(rvn_ui::PageRole::Pause));
    assert_eq!(host.canvas_states["clock"].elapsed, 0.1);
    assert_eq!(
        host.consumer_frames["handler:hit"].owner.target,
        host.consumer_frames["screen:host"].owner.target
    );
    assert_ne!(host.consumer_frames["screen:host"].owner.target, old);
    assert!(!cold.ui_input_source_current(&stale));
    assert_eq!(cold.state.vars["hits"], Value::Int(1));
}
#[test]
fn host0370_foreign_stale_rng_replaced_and_renderer_refusals_keep_all_state_atomic() {
    let (p, save, _) = saved_host0370();
    let mut source = recreated_host0370(p.clone());
    let foreign = source.capture_host_lifecycle_restore_context(&p).unwrap();
    for case in 0..4 {
        let mut cold = recreated_host0370(p.clone());
        let context = if case == 0 {
            foreign.clone()
        } else {
            cold.capture_host_lifecycle_restore_context(&p).unwrap()
        };
        match case {
            1 => {
                cold.state.random.integer(1, 10).unwrap();
            }
            2 => {
                cold.synchronize_source_menu(
                    rvn_ui::PageRole::Pause,
                    None,
                    Value::Dict(Default::default()),
                    true,
                    500,
                )
                .unwrap();
                open_frame_host(&mut cold);
            }
            3 => cold.renderer.reject.store(true, Ordering::SeqCst),
            _ => {}
        }
        let before = state(&cold);
        let random = cold.state.random.clone();
        let epoch = cold.runtime_epoch();
        let count = cold.renderer.batches.lock().unwrap().len();
        assert!(
            cold.restore_save15_with_host_context(&p, save.clone(), &context)
                .is_err(),
            "case {case}"
        );
        assert_eq!(state(&cold), before);
        assert_eq!(cold.state.random, random);
        assert_eq!(cold.runtime_epoch(), epoch);
        assert_eq!(cold.renderer.batches.lock().unwrap().len(), count);
    }
    source.state.ui.screens.clear();
    assert!(source.capture_host_lifecycle_restore_context(&p).is_err());
}

#[test]
fn host0370_real_motion_payload_resumes_saved_clock_and_rng_after_recreated_lifecycle() {
    let base = host_frame_program();
    let source=base.sources[0].source.replace("screen host(context)",r#"handler start_motion(event){motion.play("ui:host/clock",motion_pause(1))}
 screen host(context)"#).replace(r#"component("clock","canvas",{"draw":"paint","state":{},"events":{"tick":"hit"}},[])])"#,r#"component("clock","canvas",{"draw":"paint","state":{},"events":{"tick":"hit"}},[]),component("start","button",{"events":{"click":"start_motion"}},[])])"#);
    let p = rvn_parser::parse_program(&source).unwrap();
    let setup = |e: &mut Engine<AudioHost>| {
        e.step_until_interaction().unwrap();
        open_frame_host(e);
        let source = e.capture_ui_input_source("host").unwrap();
        assert!(e
            .interface_event_scoped(rvn_core::ui_input::ScopedUiInput {
                source,
                event: rvn_core::ui::UiInput {
                    screen: "host".into(),
                    element: "start".into(),
                    kind: rvn_ui::programmable::ScreenEventKind::Click,
                    value: None,
                    key: None
                }
            })
            .unwrap());
    };
    let mut hot = Engine::from_program_with_debugger(p.clone(), AudioHost::default(), 16).unwrap();
    hot.state.random = RandomState::seeded(4);
    setup(&mut hot);
    hot.tick_host_frame(0.15).unwrap();
    assert_eq!(hot.state.motions.tracks["ui:host/clock"].elapsed, 0.15);
    let mut session = hot
        .debug_begin(
            rvn_core::engine::DebugOperation::HostFrame {
                seconds: 0.1,
                reports: Default::default(),
                audio: vec![],
            },
            vec![sample_breakpoint(&p)],
        )
        .unwrap();
    assert_eq!(
        hot.debug_step(&mut session, rvn_core::editor_debugger::StepKind::Continue)
            .unwrap()
            .0,
        rvn_core::engine::DebugProgress::Stopped
    );
    let save = hot
        .capture_save15(&p, Some(&session), 21, "True Host motion clock".into())
        .unwrap();
    frame_finish(&mut hot, &mut session);
    let mut cold = Engine::from_program_with_debugger(p.clone(), AudioHost::default(), 16).unwrap();
    setup(&mut cold);
    let context = cold.capture_host_lifecycle_restore_context(&p).unwrap();
    let mut resumed = cold
        .restore_save15_with_host_context(&p, save, &context)
        .unwrap()
        .unwrap();
    frame_finish(&mut cold, &mut resumed);
    assert_eq!(
        cold.state.motions.tracks["ui:host/clock"],
        hot.state.motions.tracks["ui:host/clock"]
    );
    assert_eq!(cold.state.vars, hot.state.vars);
    assert_eq!(cold.state.random, hot.state.random);
    assert_eq!(cold.state.vars["hits"], Value::Int(2));
}
#[test]
fn host0370_effect_uses_real_game_target_and_host_clock_without_replaying_progress() {
    use rvn_core::scene_resources::{
        resource_key, EffectClock, EffectParameters, ResourceTaskKind, SavedEffect, TargetKind,
        TransformSnapshot, VisualTransition,
    };
    use rvn_core::scene_tasks::TaskOwner;
    let base = host_frame_program();
    let p = rvn_parser::parse_program(&format!("scene \"bg.png\"\n{}", base.sources[0].source))
        .unwrap();
    let setup = |e: &mut Engine<Host>| {
        e.step_until_interaction().unwrap();
        open_frame_host(e);
        let instance = e
            .state
            .ui
            .screens
            .iter()
            .find(|i| i.name == "host")
            .unwrap();
        let owner = TaskOwner::Host {
            role: instance.host_role.unwrap().id().into(),
        };
        let target = e
            .state
            .scene_execution
            .as_ref()
            .unwrap()
            .resources
            .target(&TaskOwner::Game, TargetKind::Background, "background")
            .unwrap()
            .clone();
        e.register_scene_effect(
            resource_key(owner, ResourceTaskKind::Effect, "host-fade"),
            target,
            SavedEffect {
                presentation: None,
                base_resolved: false,
                clock: EffectClock {
                    duration: 1.0,
                    elapsed: 0.0,
                    looping: false,
                },
                parameters: EffectParameters::VisualFade {
                    from: 0.0,
                    to: 1.0,
                    despawn_on_finish: false,
                    transition: VisualTransition::Fade,
                    base: TransformSnapshot::default(),
                    layered: false,
                },
            },
        )
        .unwrap();
    };
    let mut hot = frame_engine(p.clone());
    setup(&mut hot);
    hot.tick_host_frame(0.15).unwrap();
    let mut session = hot
        .debug_begin(
            rvn_core::engine::DebugOperation::HostFrame {
                seconds: 0.1,
                reports: Default::default(),
                audio: vec![],
            },
            vec![sample_breakpoint(&p)],
        )
        .unwrap();
    assert_eq!(
        hot.debug_step(&mut session, rvn_core::editor_debugger::StepKind::Continue)
            .unwrap()
            .0,
        rvn_core::engine::DebugProgress::Stopped
    );
    let save = hot
        .capture_save15(&p, Some(&session), 22, "Actual Host effect".into())
        .unwrap();
    frame_finish(&mut hot, &mut session);
    let mut cold = frame_engine(p.clone());
    setup(&mut cold);
    let context = cold.capture_host_lifecycle_restore_context(&p).unwrap();
    let mut resumed = cold
        .restore_save15_with_host_context(&p, save, &context)
        .unwrap()
        .unwrap();
    frame_finish(&mut cold, &mut resumed);
    let effects = |e: &Engine<Host>| {
        e.state
            .scene_execution
            .as_ref()
            .unwrap()
            .resources
            .tasks
            .iter()
            .filter(|t| t.kind == ResourceTaskKind::Effect)
            .map(|t| (t.key.clone(), t.target.clone(), t.effect.clone()))
            .collect::<Vec<_>>()
    };
    assert_eq!(effects(&cold), effects(&hot));
    assert_eq!(cold.state.vars, hot.state.vars);
    assert_eq!(cold.state.random, hot.state.random);
    assert_eq!(effects(&cold)[0].2.as_ref().unwrap().clock.elapsed, 0.25);
}

#[path = "private_pcm0370/frame_pcm_host.rs"]
mod real_pcm0370;
fn pcm_setup0370(e: &mut Engine<real_pcm0370::PcmHost>) {
    e.step_until_interaction().unwrap();
    open_frame_host(e);
    let instance = e
        .state
        .ui
        .screens
        .iter()
        .find(|i| i.name == "host")
        .unwrap();
    let owner = rvn_core::scene_tasks::TaskOwner::Host {
        role: instance.host_role.unwrap().id().into(),
    };
    let epoch = e.runtime_epoch();
    rvn_core::audio_playback::register(
        &mut e.state,
        epoch,
        owner,
        rvn_core::audio_playback::AudioPlayback::new(
            rvn_core::audio_playback::AudioFamily::Sfx,
            "sfx/host.wav".into(),
            0.0,
        )
        .unwrap(),
    )
    .unwrap();
    e.renderer
        .bind_actual(&e.state, e.instance_id(), e.runtime_epoch())
        .unwrap();
}
#[test]
fn host0370_all_reader_rows_missing_duplicate_foreign_and_epoch_refuse_without_cursor_change() {
    use real_pcm0370::PcmHost;
    let p = host_frame_program();
    let mut e = Engine::from_program_with_debugger(p, PcmHost::default(), 16).unwrap();
    pcm_setup0370(&mut e);
    let activation = e.state.scene_execution.as_ref().unwrap().owner().unwrap();
    let epoch = e.runtime_epoch();
    rvn_core::audio_playback::register(
        &mut e.state,
        epoch,
        rvn_core::scene_tasks::TaskOwner::Scene { activation },
        rvn_core::audio_playback::AudioPlayback::new(
            rvn_core::audio_playback::AudioFamily::Voice,
            "voice/voice.wav".into(),
            0.0,
        )
        .unwrap(),
    )
    .unwrap();
    e.renderer
        .bind_actual(&e.state, e.instance_id(), epoch)
        .unwrap();
    let views = e.audio_playback_views().unwrap();
    assert_eq!(views.len(), 2);
    for (i, view) in views.iter().enumerate() {
        e.renderer.pull(&view.token.target, i + 3).unwrap();
    }
    let before = serde_json::to_value(&e.state).unwrap();
    let cursors: Vec<_> = views
        .iter()
        .map(|v| e.renderer.cursor(&v.token.target).unwrap())
        .collect();
    let snapshot = e
        .renderer
        .capture_frame_audio_checkpoint(&e.state, e.instance_id(), epoch)
        .unwrap();
    assert_eq!(snapshot.rows.len(), 2);
    for case in 0..6 {
        let mut changed = snapshot.clone();
        match case {
            0 => {
                changed.rows.pop();
            }
            1 => {
                changed.rows.push(changed.rows[0].clone());
            }
            2 => changed.instance_id += 1,
            3 => changed.runtime_epoch += 1,
            4 => changed.rows[0].token.task.runtime_epoch += 1,
            _ => changed.rows[0].token.target.incarnation += 100,
        }
        assert!(
            rvn_core::frame_audio_checkpoint::checkpointed_frame_state(
                &e.state,
                e.instance_id(),
                epoch,
                &changed
            )
            .is_err(),
            "case{case}"
        );
    }
    let mut changed = e.state.clone();
    changed
        .scene_execution
        .as_mut()
        .unwrap()
        .resources
        .tasks
        .iter_mut()
        .find(|t| t.audio.is_some())
        .unwrap()
        .revision += 1;
    assert!(e
        .renderer
        .capture_frame_audio_checkpoint(&changed, e.instance_id(), epoch)
        .is_err());
    assert_eq!(serde_json::to_value(&e.state).unwrap(), before);
    assert_eq!(
        views
            .iter()
            .map(|v| e.renderer.cursor(&v.token.target).unwrap())
            .collect::<Vec<_>>(),
        cursors
    );
}
#[test]
fn host0370_current_and_retired_host_music_share_fence_and_cold_exact_pcm() {
    use real_pcm0370::PcmHost;
    use rvn_core::retired_presentations::{PresentationSlot, RetiredModel, RetiredPresentation};
    use rvn_core::scene_resources::{EffectClock, EffectParameters, SavedEffect, TargetKind};
    let p = rvn_parser::parse_program(&host_frame_program().sources[0].source.replace(
        "scene label P\n",
        "scene label P\nmusic.play(\"music/host.wav\")\n",
    ))
    .unwrap();
    let setup = |e: &mut Engine<PcmHost>, reopen: bool| {
        e.step_until_interaction().unwrap();
        open_frame_host(e);
        if reopen {
            e.synchronize_source_menu(
                rvn_ui::PageRole::Pause,
                None,
                Value::Dict(Default::default()),
                true,
                500,
            )
            .unwrap();
            open_frame_host(e);
        }
        let owner = rvn_core::scene_tasks::TaskOwner::Host {
            role: e
                .state
                .ui
                .screens
                .iter()
                .find(|i| i.name == "host")
                .unwrap()
                .host_role
                .unwrap()
                .id()
                .into(),
        };
        let original_target = e
            .state
            .scene_execution
            .as_ref()
            .unwrap()
            .resources
            .targets
            .iter()
            .find(|r| r.identity.kind == TargetKind::Music)
            .unwrap()
            .clone();
        let epoch = e.runtime_epoch();
        rvn_core::scene_resources::register_retired_effect(
            &mut e.state,
            epoch,
            owner,
            RetiredPresentation {
                original_slot: PresentationSlot::Music,
                original_target,
                model: RetiredModel::Music {
                    file: "music/host.wav".into(),
                    volume: 1.0,
                    fade: None,
                },
            },
            SavedEffect {
                presentation: None,
                base_resolved: false,
                clock: EffectClock {
                    duration: 1.0,
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
        e.renderer
            .bind_actual(&e.state, e.instance_id(), e.runtime_epoch())
            .unwrap();
    };
    let mut hot = Engine::from_program_with_debugger(p.clone(), PcmHost::default(), 16).unwrap();
    setup(&mut hot, false);
    let music = rvn_core::audio_checkpoint::music_views(&hot.state, hot.runtime_epoch()).unwrap();
    assert_eq!(music.len(), 2);
    let current = music
        .iter()
        .find(|v| v.authority.outgoing.is_none())
        .unwrap()
        .authority
        .target
        .clone();
    let retired = music
        .iter()
        .find(|v| v.authority.outgoing.is_some())
        .unwrap()
        .authority
        .target
        .clone();
    assert_eq!(hot.renderer.pull(&current, 2).unwrap(), vec![0, 1]);
    assert_eq!(hot.renderer.pull(&retired, 5).unwrap(), vec![0, 1, 2, 3, 4]);
    hot.tick_host_frame(0.15).unwrap();
    let mut session = hot
        .debug_begin(
            rvn_core::engine::DebugOperation::HostFrame {
                seconds: 0.1,
                reports: Default::default(),
                audio: vec![],
            },
            vec![sample_breakpoint(&p)],
        )
        .unwrap();
    assert_eq!(
        hot.debug_step(&mut session, rvn_core::editor_debugger::StepKind::Continue)
            .unwrap()
            .0,
        rvn_core::engine::DebugProgress::Stopped
    );
    let save = hot
        .capture_save15(&p, Some(&session), 27, "Host retired Music real PCM".into())
        .unwrap();
    frame_finish(&mut hot, &mut session);
    let mut cold = Engine::from_program_with_debugger(p.clone(), PcmHost::default(), 16).unwrap();
    setup(&mut cold, true);
    let context = cold.capture_host_lifecycle_restore_context(&p).unwrap();
    let warming = cold
        .save15_state_for_frame_audio_warming(&p, &save, &context)
        .unwrap();
    assert_eq!(
        rvn_core::audio_checkpoint::music_views(&warming, 0)
            .unwrap()
            .len(),
        2
    );
    let mut resumed = cold
        .restore_save15_with_host_context(&p, save, &context)
        .unwrap()
        .unwrap();
    let music = rvn_core::audio_checkpoint::music_views(&cold.state, cold.runtime_epoch()).unwrap();
    let current = music
        .iter()
        .find(|v| v.authority.outgoing.is_none())
        .unwrap();
    let retired = music
        .iter()
        .find(|v| v.authority.outgoing.is_some())
        .unwrap();
    assert_eq!(
        cold.renderer
            .cursor(&current.authority.target)
            .unwrap()
            .next_sample,
        2
    );
    let cp = cold.renderer.cursor(&retired.authority.target).unwrap();
    assert_eq!((cp.next_sample, cp.next_channel, cp.looping), (5, 1, true));
    assert_eq!(
        cold.renderer.pull(&retired.authority.target, 4).unwrap(),
        vec![0, 5, 6, 7]
    );
    frame_finish(&mut cold, &mut resumed);
    assert_eq!(cold.state.vars, hot.state.vars);
    assert_eq!(cold.state.random, hot.state.random);
    let effect = cold
        .state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .tasks
        .iter()
        .find(|t| t.target.kind == TargetKind::PresentationRetired)
        .unwrap()
        .effect
        .as_ref()
        .unwrap();
    assert_eq!(effect.clock.elapsed, 0.25);
}
#[test]
fn host0370_retired_effect_is_rebased_from_real_birth_and_preserves_historical_payload() {
    use rvn_core::retired_presentations::{
        FrozenVisualSample, PresentationSlot, RetiredModel, RetiredPresentation,
    };
    use rvn_core::scene_resources::{
        EffectClock, EffectParameters, SavedEffect, TargetKind, TransformSnapshot, VisualTransition,
    };
    let p = rvn_parser::parse_program(&format!(
        "scene \"bg.png\"\n{}",
        host_frame_program().sources[0].source
    ))
    .unwrap();
    let setup = |e: &mut Engine<Host>, reopen: bool| {
        e.step_until_interaction().unwrap();
        open_frame_host(e);
        if reopen {
            e.synchronize_source_menu(
                rvn_ui::PageRole::Pause,
                None,
                Value::Dict(Default::default()),
                true,
                500,
            )
            .unwrap();
            open_frame_host(e);
        }
        let owner = rvn_core::scene_tasks::TaskOwner::Host {
            role: e
                .state
                .ui
                .screens
                .iter()
                .find(|i| i.name == "host")
                .unwrap()
                .host_role
                .unwrap()
                .id()
                .into(),
        };
        let registry = &e.state.scene_execution.as_ref().unwrap().resources;
        let original_target = registry
            .targets
            .iter()
            .find(|r| r.identity.kind == TargetKind::Background)
            .unwrap()
            .clone();
        let epoch = e.runtime_epoch();
        rvn_core::scene_resources::register_retired_effect(
            &mut e.state,
            epoch,
            owner,
            RetiredPresentation {
                original_slot: PresentationSlot::Background,
                original_target,
                model: RetiredModel::Background {
                    path: "bg.png".into(),
                    sample: FrozenVisualSample::default(),
                },
            },
            SavedEffect {
                presentation: None,
                base_resolved: false,
                clock: EffectClock {
                    duration: 1.0,
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
    };
    let mut hot = frame_engine(p.clone());
    setup(&mut hot, false);
    hot.tick_host_frame(0.15).unwrap();
    let mut session = hot
        .debug_begin(
            rvn_core::engine::DebugOperation::HostFrame {
                seconds: 0.1,
                reports: Default::default(),
                audio: vec![],
            },
            vec![sample_breakpoint(&p)],
        )
        .unwrap();
    assert_eq!(
        hot.debug_step(&mut session, rvn_core::editor_debugger::StepKind::Continue)
            .unwrap()
            .0,
        rvn_core::engine::DebugProgress::Stopped
    );
    let save = hot
        .capture_save15(&p, Some(&session), 26, "Retired Host actual birth".into())
        .unwrap();
    frame_finish(&mut hot, &mut session);
    let mut cold = frame_engine(p.clone());
    setup(&mut cold, true);
    let old = cold
        .state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .tasks
        .iter()
        .find(|t| t.target.kind == TargetKind::PresentationRetired)
        .unwrap()
        .target
        .clone();
    let context = cold.capture_host_lifecycle_restore_context(&p).unwrap();
    let mut resumed = cold
        .restore_save15_with_host_context(&p, save, &context)
        .unwrap()
        .unwrap();
    frame_finish(&mut cold, &mut resumed);
    let effect = |e: &Engine<Host>| {
        e.state
            .scene_execution
            .as_ref()
            .unwrap()
            .resources
            .tasks
            .iter()
            .find(|t| t.target.kind == TargetKind::PresentationRetired)
            .unwrap()
            .effect
            .clone()
            .unwrap()
    };
    assert_eq!(effect(&cold), effect(&hot));
    assert_eq!(effect(&cold).clock.elapsed, 0.25);
    assert_eq!(cold.state.vars, hot.state.vars);
    assert_eq!(cold.state.random, hot.state.random);
    let target = &cold
        .state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .tasks
        .iter()
        .find(|t| t.target.kind == TargetKind::PresentationRetired)
        .unwrap()
        .target;
    assert_ne!(*target, old);
    assert_eq!(target.alias, format!("retired_{}", target.incarnation));
}
#[test]
fn host0370_warming_data_has_no_commit_authority_and_pcm_mismatch_refuses_atomically() {
    use real_pcm0370::PcmHost;
    use sha2::{Digest, Sha256};
    let p = host_frame_program();
    let mut hot = Engine::from_program_with_debugger(p.clone(), PcmHost::default(), 16).unwrap();
    pcm_setup0370(&mut hot);
    let target = hot.audio_playback_views().unwrap()[0].token.target.clone();
    hot.renderer.pull(&target, 3).unwrap();
    let mut session = hot
        .debug_begin(
            rvn_core::engine::DebugOperation::HostFrame {
                seconds: 0.1,
                reports: Default::default(),
                audio: vec![],
            },
            vec![sample_breakpoint(&p)],
        )
        .unwrap();
    assert_eq!(
        hot.debug_step(&mut session, rvn_core::editor_debugger::StepKind::Continue)
            .unwrap()
            .0,
        rvn_core::engine::DebugProgress::Stopped
    );
    let save = hot
        .capture_save15(&p, Some(&session), 25, "Actual PCM refusal".into())
        .unwrap();
    for case in 0..5 {
        let mut cold =
            Engine::from_program_with_debugger(p.clone(), PcmHost::default(), 16).unwrap();
        pcm_setup0370(&mut cold);
        let context = cold.capture_host_lifecycle_restore_context(&p).unwrap();
        let before = serde_json::to_value(&cold.state).unwrap();
        let random = cold.state.random.clone();
        let epoch = cold.runtime_epoch();
        let publications = cold.renderer.publication_count();
        let target = cold.audio_playback_views().unwrap()[0].token.target.clone();
        let cursor = cold.renderer.cursor(&target).unwrap();
        let mut data = serde_json::to_value(&save).unwrap();
        let task = data["debugger"]["frame_operation"]["host_tasks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|w| !w["descriptor"]["audio"].is_null())
            .unwrap();
        let checkpoint = &mut task["descriptor"]["audio"]["checkpoint"];
        match case {
            0 => checkpoint["asset_sha256"][0] = serde_json::json!(0),
            1 => checkpoint["pcm_sha256"][0] = serde_json::json!(0),
            2 => checkpoint["next_channel"] = serde_json::json!(0),
            3 => checkpoint["next_sample"] = serde_json::json!(1000),
            _ => {}
        }
        let descriptor: rvn_core::scene_resources::ResourceTask =
            serde_json::from_value(task["descriptor"].clone()).unwrap();
        let hash: [u8; 32] = Sha256::digest(serde_json::to_vec(&descriptor).unwrap()).into();
        task["shape"] = serde_json::to_value(hash).unwrap();
        let changed =
            rvn_core::save::consumer_checkpoint::Save15::decode(&data.to_string()).unwrap();
        if case == 4 {
            let warming = cold
                .save15_state_for_frame_audio_warming(&p, &changed, &context)
                .unwrap();
            assert_eq!(serde_json::to_value(&cold.state).unwrap(), before);
            let cp = rvn_core::audio_playback::describe(&warming, 0).unwrap()[0]
                .playback
                .checkpoint
                .clone()
                .unwrap();
            assert_eq!((cp.next_sample, cp.next_channel), (3, 1));
            cold.state.random.integer(1, 3).unwrap();
            assert!(cold
                .restore_save15_with_host_context(&p, changed, &context)
                .is_err());
            assert_eq!(cold.runtime_epoch(), epoch);
        } else {
            assert!(
                cold.restore_save15_with_host_context(&p, changed, &context)
                    .is_err(),
                "case{case}"
            );
            assert_eq!(serde_json::to_value(&cold.state).unwrap(), before);
            assert_eq!(cold.state.random, random);
            assert_eq!(cold.runtime_epoch(), epoch);
            assert_eq!(cold.renderer.publication_count(), publications);
        }
        assert_eq!(cold.renderer.cursor(&target).unwrap(), cursor);
    }
}
#[test]
fn host0370_real_story_wait_keeps_host_clock_and_authentic_narrative_lease() {
    let base = host_frame_program();
    let source = base.sources[0].source
        .replace("screen host(context)", r#"handler start_motion(event){motion.play("ui:host/clock",motion_pause(1))} screen host(context)"#)
        .replace(r#"component("clock","canvas",{"draw":"paint","state":{},"events":{"tick":"hit"}},[])])"#, r#"component("clock","canvas",{"draw":"paint","state":{},"events":{"tick":"hit"}},[]),component("start","button",{"events":{"click":"start_motion"}},[])])"#)
        .replace("\"parent\"\ncall C", "\"parent\"\nmotion.wait(\"ui:host/clock\")\n\"afterwait\"\ncall C");
    let p = rvn_parser::parse_program(&source).unwrap();
    let setup = |e: &mut Engine<AudioHost>| {
        e.step_until_interaction().unwrap();
        open_frame_host(e);
        let source = e.capture_ui_input_source("host").unwrap();
        assert!(e
            .interface_event_scoped(rvn_core::ui_input::ScopedUiInput {
                source,
                event: rvn_core::ui::UiInput {
                    screen: "host".into(),
                    element: "start".into(),
                    kind: rvn_ui::programmable::ScreenEventKind::Click,
                    value: None,
                    key: None
                }
            })
            .unwrap());
        e.advance_dialogue().unwrap();
        e.step_until_interaction().unwrap();
        assert_eq!(e.state.motions.waiting.as_deref(), Some("ui:host/clock"));
        let execution = e.state.scene_execution.as_ref().unwrap();
        let lease = execution
            .resources
            .waits
            .iter()
            .find(|w| matches!(w.key.owner, rvn_core::scene_tasks::TaskOwner::Host { .. }))
            .unwrap();
        assert_eq!(lease.owner, execution.owner());
        assert_eq!(lease.position.pc, e.state.pc);
    };
    let mut hot = Engine::from_program_with_debugger(p.clone(), AudioHost::default(), 16).unwrap();
    hot.state.random = RandomState::seeded(4);
    setup(&mut hot);
    hot.tick_host_frame(0.15).unwrap();
    let mut session = hot
        .debug_begin(
            rvn_core::engine::DebugOperation::HostFrame {
                seconds: 0.1,
                reports: Default::default(),
                audio: vec![],
            },
            vec![sample_breakpoint(&p)],
        )
        .unwrap();
    assert_eq!(
        hot.debug_step(&mut session, rvn_core::editor_debugger::StepKind::Continue)
            .unwrap()
            .0,
        rvn_core::engine::DebugProgress::Stopped
    );
    let save = hot
        .capture_save15(
            &p,
            Some(&session),
            24,
            "True Story wait on Host motion".into(),
        )
        .unwrap();
    frame_finish(&mut hot, &mut session);
    for case in 0..6 {
        let mut trial =
            Engine::from_program_with_debugger(p.clone(), AudioHost::default(), 16).unwrap();
        setup(&mut trial);
        let context = trial.capture_host_lifecycle_restore_context(&p).unwrap();
        let before = serde_json::to_value(&trial.state).unwrap();
        let epoch = trial.runtime_epoch();
        let count = trial.renderer.inner.batches.lock().unwrap().len();
        let mut data = serde_json::to_value(&save).unwrap();
        let waits = data["debugger"]["frame_operation"]["host_waits"]
            .as_array_mut()
            .unwrap();
        match case {
            0 => waits[0]["owner"] = serde_json::json!(999),
            1 => waits[0]["pc"] = serde_json::json!(0),
            2 => waits[0]["revision"] = serde_json::json!(999),
            3 => waits[0]["target"]["incarnation"] = serde_json::json!(999),
            4 => waits[0]["parked"] = serde_json::json!(true),
            _ => {
                waits.push(waits[0].clone());
            }
        }
        let forged =
            rvn_core::save::consumer_checkpoint::Save15::decode(&data.to_string()).unwrap();
        assert!(
            trial
                .restore_save15_with_host_context(&p, forged, &context)
                .is_err(),
            "waitcase{case}"
        );
        assert_eq!(serde_json::to_value(&trial.state).unwrap(), before);
        assert_eq!(trial.runtime_epoch(), epoch);
        assert_eq!(trial.renderer.inner.batches.lock().unwrap().len(), count);
    }
    let mut cold = Engine::from_program_with_debugger(p.clone(), AudioHost::default(), 16).unwrap();
    setup(&mut cold);
    let context = cold.capture_host_lifecycle_restore_context(&p).unwrap();
    let mut resumed = cold
        .restore_save15_with_host_context(&p, save, &context)
        .unwrap()
        .unwrap();
    frame_finish(&mut cold, &mut resumed);
    assert_eq!(cold.state.pc, hot.state.pc);
    assert_eq!(cold.state.motions, hot.state.motions);
    assert_eq!(cold.state.random, hot.state.random);
    for _ in 0..3 {
        hot.tick_host_frame(0.25).unwrap();
        cold.tick_host_frame(0.25).unwrap();
    }
    assert!(cold.state.motions.waiting.is_none());
    assert!(cold
        .state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .waits
        .is_empty());
    assert_eq!(cold.state.vars, hot.state.vars);
    assert_eq!(cold.state.random, hot.state.random);
}
#[test]
fn host0370_sfx_voice_cold_cursor_and_reports_use_actual_wav_source_and_shared_fence() {
    use real_pcm0370::PcmHost;
    use rvn_core::audio_playback::{AudioFamily, AudioFeedback, AudioReport};
    let p = host_frame_program();
    let setup = |e: &mut Engine<PcmHost>| {
        e.step_until_interaction().unwrap();
        open_frame_host(e);
        let instance = e
            .state
            .ui
            .screens
            .iter()
            .find(|i| i.name == "host")
            .unwrap();
        let owner = rvn_core::scene_tasks::TaskOwner::Host {
            role: instance.host_role.unwrap().id().into(),
        };
        let epoch = e.runtime_epoch();
        for (family, file) in [
            (AudioFamily::Sfx, "sfx/host.wav"),
            (AudioFamily::Voice, "voice/voice.wav"),
        ] {
            rvn_core::audio_playback::register(
                &mut e.state,
                epoch,
                owner.clone(),
                rvn_core::audio_playback::AudioPlayback::new(family, file.into(), 0.0).unwrap(),
            )
            .unwrap();
        }
        e.renderer
            .bind_actual(&e.state, e.instance_id(), e.runtime_epoch())
            .unwrap();
    };
    let mut hot = Engine::from_program_with_debugger(p.clone(), PcmHost::default(), 16).unwrap();
    hot.state.random = RandomState::seeded(4);
    setup(&mut hot);
    let views = hot.audio_playback_views().unwrap();
    assert_eq!(views.len(), 2);
    let sfx = views
        .iter()
        .find(|v| v.playback.family == AudioFamily::Sfx)
        .unwrap();
    let voice = views
        .iter()
        .find(|v| v.playback.family == AudioFamily::Voice)
        .unwrap();
    assert_eq!(
        hot.renderer.pull(&sfx.token.target, 3).unwrap(),
        vec![0, 1, 2]
    );
    assert_eq!(
        hot.renderer.pull(&voice.token.target, 5).unwrap(),
        vec![100, 101, 102, 103, 104]
    );
    let report = AudioReport {
        instance_id: hot.instance_id(),
        token: sfx.token.clone(),
        feedback: AudioFeedback::End,
    };
    let mut stale = report.clone();
    stale.instance_id += 1;
    let mut session = hot
        .debug_begin(
            rvn_core::engine::DebugOperation::HostFrame {
                seconds: 0.1,
                reports: Default::default(),
                audio: vec![stale, report.clone(), report],
            },
            vec![sample_breakpoint(&p)],
        )
        .unwrap();
    assert_eq!(
        hot.debug_step(&mut session, rvn_core::editor_debugger::StepKind::Continue)
            .unwrap()
            .0,
        rvn_core::engine::DebugProgress::Stopped
    );
    let save = hot
        .capture_save15(&p, Some(&session), 23, "Actual Host Source cursor".into())
        .unwrap();
    let expected = match frame_finish(&mut hot, &mut session) {
        rvn_core::engine::DebugOutcome::Host((_, audio)) => audio,
        _ => panic!("wrong route"),
    };
    let mut cold = Engine::from_program_with_debugger(p.clone(), PcmHost::default(), 16).unwrap();
    setup(&mut cold);
    let stale_source = cold.audio_playback_views().unwrap()[0].token.clone();
    let context = cold.capture_host_lifecycle_restore_context(&p).unwrap();
    let mut resumed = cold
        .restore_save15_with_host_context(&p, save, &context)
        .unwrap()
        .unwrap();
    let views = cold.audio_playback_views().unwrap();
    let voice = views
        .iter()
        .find(|v| v.playback.family == AudioFamily::Voice)
        .unwrap();
    let cp = cold.renderer.cursor(&voice.token.target).unwrap();
    assert_eq!((cp.next_sample, cp.next_channel), (5, 1));
    assert_eq!(
        cold.renderer.pull(&voice.token.target, 4).unwrap(),
        vec![0, 105, 106, 107]
    );
    assert_ne!(views[0].token, stale_source);
    let actual = match frame_finish(&mut cold, &mut resumed) {
        rvn_core::engine::DebugOutcome::Host((_, audio)) => audio,
        _ => panic!("wrong route"),
    };
    assert_eq!(actual, expected);
    assert_eq!(cold.state.vars, hot.state.vars);
    assert_eq!(cold.state.random, hot.state.random);
    assert_eq!(
        actual[0].disposition,
        rvn_core::engine::AudioReportDisposition::RejectedStale
    );
    assert_eq!(cold.audio_playback_views().unwrap().len(), 1);
    assert_eq!(
        cold.audio_playback_views().unwrap()[0].playback.family,
        AudioFamily::Voice
    );
}
#[test]
fn host0370_cancelled_wait_is_data_once_after_actual_close_and_reopen() {
    let base = host_frame_program();
    let source = base.sources[0].source
        .replace("screen host(context)", r#"handler start_motion(event){motion.play("ui:host/clock",motion_pause(1))} screen host(context)"#)
        .replace(r#"component("clock","canvas",{"draw":"paint","state":{},"events":{"tick":"hit"}},[])])"#, r#"component("clock","canvas",{"draw":"paint","state":{},"events":{"tick":"hit"}},[]),component("start","button",{"events":{"click":"start_motion"}},[])])"#)
        .replace("\"parent\"\ncall C", "\"parent\"\nmotion.wait(\"ui:host/clock\")\n\"afterwait\"\ncall C");
    let p = rvn_parser::parse_program(&source).unwrap();
    let setup = |e: &mut Engine<AudioHost>| {
        e.step_until_interaction().unwrap();
        open_frame_host(e);
        let source = e.capture_ui_input_source("host").unwrap();
        assert!(e
            .interface_event_scoped(rvn_core::ui_input::ScopedUiInput {
                source,
                event: rvn_core::ui::UiInput {
                    screen: "host".into(),
                    element: "start".into(),
                    kind: rvn_ui::programmable::ScreenEventKind::Click,
                    value: None,
                    key: None
                }
            })
            .unwrap());
        e.advance_dialogue().unwrap();
        e.step_until_interaction().unwrap();
        assert_eq!(e.state.motions.waiting.as_deref(), Some("ui:host/clock"));
        let execution = e.state.scene_execution.as_ref().unwrap();
        let lease = execution
            .resources
            .waits
            .iter()
            .find(|w| matches!(w.key.owner, rvn_core::scene_tasks::TaskOwner::Host { .. }))
            .unwrap();
        assert_eq!(lease.owner, execution.owner());
        assert_eq!(lease.position.pc, e.state.pc);
    };
    let mut hot = Engine::from_program_with_debugger(p.clone(), AudioHost::default(), 16).unwrap();
    hot.state.random = RandomState::seeded(4);
    setup(&mut hot);
    hot.synchronize_source_menu(
        rvn_ui::PageRole::Pause,
        None,
        Value::Dict(Default::default()),
        true,
        500,
    )
    .unwrap();
    open_frame_host(&mut hot);
    assert!(hot.state.motions.waiting.is_none());
    // The actual active caller consumes cancellation inside menu reconciliation.
    assert!(hot
        .state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .cancelled_waits
        .is_empty());
    let mut session = hot
        .debug_begin(
            rvn_core::engine::DebugOperation::HostFrame {
                seconds: 0.1,
                reports: Default::default(),
                audio: vec![],
            },
            vec![sample_breakpoint(&p)],
        )
        .unwrap();
    assert_eq!(
        hot.debug_step(&mut session, rvn_core::editor_debugger::StepKind::Continue)
            .unwrap()
            .0,
        rvn_core::engine::DebugProgress::Stopped
    );
    let save = hot
        .capture_save15(
            &p,
            Some(&session),
            24,
            "True Story wait on Host motion".into(),
        )
        .unwrap();
    frame_finish(&mut hot, &mut session);
    let mut cold = Engine::from_program_with_debugger(p.clone(), AudioHost::default(), 16).unwrap();
    setup(&mut cold);
    cold.synchronize_source_menu(
        rvn_ui::PageRole::Pause,
        None,
        Value::Dict(Default::default()),
        true,
        500,
    )
    .unwrap();
    open_frame_host(&mut cold);
    let context = cold.capture_host_lifecycle_restore_context(&p).unwrap();
    let mut resumed = cold
        .restore_save15_with_host_context(&p, save, &context)
        .unwrap()
        .unwrap();
    frame_finish(&mut cold, &mut resumed);
    assert!(cold
        .state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .cancelled_waits
        .is_empty());
    cold.step_until_interaction().unwrap();
    hot.step_until_interaction().unwrap();
    assert!(cold
        .state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .cancelled_waits
        .is_empty());
    assert_eq!(cold.state.pc, hot.state.pc);
    assert_eq!(cold.state.motions, hot.state.motions);
    assert_eq!(cold.state.random, hot.state.random);
    for _ in 0..3 {
        hot.tick_host_frame(0.25).unwrap();
        cold.tick_host_frame(0.25).unwrap();
    }
    assert!(cold.state.motions.waiting.is_none());
    assert!(cold
        .state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .waits
        .is_empty());
    assert_eq!(cold.state.vars, hot.state.vars);
    assert_eq!(cold.state.random, hot.state.random);
}

#[test]
fn host0370_video_callback_rebased_actual_lifetime_once_with_stale_receipt_order() {
    let base = host_frame_program();
    let source=base.sources[0].source.replace("screen host(context)",r#"handler play_host(event){video.play("hostclip",video_clip("host.webm",{"target":"ui:host/start","on_end":"hit"}))}
 screen host(context)"#).replace(r#"component("clock","canvas",{"draw":"paint","state":{},"events":{"tick":"hit"}},[])])"#,r#"component("clock","canvas",{"draw":"paint","state":{},"events":{"tick":"hit"}},[]),component("start","button",{"events":{"click":"play_host"}},[])])"#);
    let p = rvn_parser::parse_program(&source).unwrap();
    let setup = |e: &mut Engine<Host>| {
        e.step_until_interaction().unwrap();
        open_frame_host(e);
        let source = e.capture_ui_input_source("host").unwrap();
        assert!(e
            .interface_event_scoped(rvn_core::ui_input::ScopedUiInput {
                source,
                event: rvn_core::ui::UiInput {
                    screen: "host".into(),
                    element: "start".into(),
                    kind: rvn_ui::programmable::ScreenEventKind::Click,
                    value: None,
                    key: None
                }
            })
            .unwrap());
    };
    let mut hot = frame_engine(p.clone());
    setup(&mut hot);
    let view = hot
        .renderer
        .batches
        .lock()
        .unwrap()
        .last()
        .unwrap()
        .videos
        .iter()
        .find(|v| v.id == "hostclip")
        .unwrap()
        .clone();
    let token = view.resource_token.unwrap();
    assert!(matches!(
        token.task.key.owner,
        rvn_core::scene_tasks::TaskOwner::Host { .. }
    ));
    let mut stale = token.clone();
    stale.task.runtime_epoch += 1;
    let mut session = hot
        .debug_begin(
            rvn_core::engine::DebugOperation::HostFrame {
                seconds: 0.1,
                reports: rvn_core::engine::HostFrameReports {
                    videos: vec![
                        rvn_core::engine::GameVideoReport {
                            token: stale,
                            feedback: rvn_core::engine::GameVideoFeedback::Report(
                                rvn_core::video::Feedback::Error("ignored".into()),
                            ),
                        },
                        rvn_core::engine::GameVideoReport {
                            token: token.clone(),
                            feedback: rvn_core::engine::GameVideoFeedback::Report(
                                rvn_core::video::Feedback::End,
                            ),
                        },
                        rvn_core::engine::GameVideoReport {
                            token,
                            feedback: rvn_core::engine::GameVideoFeedback::Report(
                                rvn_core::video::Feedback::End,
                            ),
                        },
                    ],
                },
                audio: vec![],
            },
            vec![sample_breakpoint(&p)],
        )
        .unwrap();
    assert_eq!(
        hot.debug_step(&mut session, rvn_core::editor_debugger::StepKind::Continue)
            .unwrap()
            .0,
        rvn_core::engine::DebugProgress::Stopped
    );
    let save = hot
        .capture_save15(&p, Some(&session), 16, "Host real video".into())
        .unwrap();
    let expected = match frame_finish(&mut hot, &mut session) {
        rvn_core::engine::DebugOutcome::Host((f, _)) => f,
        _ => panic!("wrong clock"),
    };
    let expected_state = state(&hot);
    let mut cold = frame_engine(p.clone());
    setup(&mut cold);
    cold.synchronize_source_menu(rvn_ui::PageRole::Pause,None,Value::Dict(Default::default()),true,500).unwrap();
    setup(&mut cold);
    let before=cold.state.ui.screens.iter().find(|i|i.name=="host").unwrap().consumer_frames["screen:host"].owner.target.clone();
    let context=cold.capture_host_lifecycle_restore_context(&p).unwrap();
    let mut resumed = cold.restore_save15_with_host_context(&p, save, &context).unwrap().unwrap();
    assert_ne!(cold.state.ui.screens.iter().find(|i|i.name=="host").unwrap().consumer_frames["screen:host"].owner.target,before);
    let actual = match frame_finish(&mut cold, &mut resumed) {
        rvn_core::engine::DebugOutcome::Host((f, _)) => f,
        _ => panic!("wrong clock"),
    };
    assert_eq!(actual.receipts, expected.receipts);
    assert_eq!(actual.frame, expected.frame);
    assert_eq!(serde_json::to_value(&cold.state.vars).unwrap(),expected_state["vars"]); assert_eq!(cold.state.random,hot.state.random);
    assert_eq!(cold.state.vars["hits"], Value::Int(2));
    assert_eq!(
        actual.receipts[0].disposition,
        rvn_core::engine::VideoReportDisposition::RejectedStale
    );
}


#[test]
fn host0370_finished_canonical_sfx_fade_restores_terminal_clock_without_replaying_envelope() {
    use real_pcm0370::PcmHost;
    use rvn_core::audio_playback::{AudioFamily, AudioFeedback, AudioReport};
    let p = host_frame_program();
    let setup = |e: &mut Engine<PcmHost>| {
        e.step_until_interaction().unwrap();
        open_frame_host(e);
        let instance = e
            .state
            .ui
            .screens
            .iter()
            .find(|i| i.name == "host")
            .unwrap();
        let owner = rvn_core::scene_tasks::TaskOwner::Host {
            role: instance.host_role.unwrap().id().into(),
        };
        let epoch = e.runtime_epoch();
        for (family, file) in [
            (AudioFamily::Sfx, "sfx/host.wav"),
            (AudioFamily::Voice, "voice/voice.wav"),
        ] {
            rvn_core::audio_playback::register(
                &mut e.state,
                epoch,
                owner.clone(),
                rvn_core::audio_playback::AudioPlayback::new(family, file.into(), if family == AudioFamily::Sfx {0.1}else{0.0}).unwrap(),
            )
            .unwrap();
        }
        e.renderer
            .bind_actual(&e.state, e.instance_id(), e.runtime_epoch())
            .unwrap();
    };
    let mut hot = Engine::from_program_with_debugger(p.clone(), PcmHost::default(), 16).unwrap();
    hot.state.random = RandomState::seeded(4);
    setup(&mut hot);
    hot.tick_host_frame(0.15).unwrap();
    assert!(hot.audio_playback_views().unwrap().iter().find(|v|v.playback.family==AudioFamily::Sfx).unwrap().playback.envelope.is_none());
    let views = hot.audio_playback_views().unwrap();
    assert_eq!(views.len(), 2);
    let sfx = views
        .iter()
        .find(|v| v.playback.family == AudioFamily::Sfx)
        .unwrap();
    let voice = views
        .iter()
        .find(|v| v.playback.family == AudioFamily::Voice)
        .unwrap();
    assert_eq!(
        hot.renderer.pull(&sfx.token.target, 3).unwrap(),
        vec![0, 1, 2]
    );
    assert_eq!(
        hot.renderer.pull(&voice.token.target, 5).unwrap(),
        vec![100, 101, 102, 103, 104]
    );
    let report = AudioReport {
        instance_id: hot.instance_id(),
        token: sfx.token.clone(),
        feedback: AudioFeedback::End,
    };
    let mut stale = report.clone();
    stale.instance_id += 1;
    let mut session = hot
        .debug_begin(
            rvn_core::engine::DebugOperation::HostFrame {
                seconds: 0.1,
                reports: Default::default(),
                audio: vec![stale, report.clone(), report],
            },
            vec![sample_breakpoint(&p)],
        )
        .unwrap();
    assert_eq!(
        hot.debug_step(&mut session, rvn_core::editor_debugger::StepKind::Continue)
            .unwrap()
            .0,
        rvn_core::engine::DebugProgress::Stopped
    );
    let save = hot
        .capture_save15(&p, Some(&session), 23, "Actual Host Source cursor".into())
        .unwrap();
    let expected = match frame_finish(&mut hot, &mut session) {
        rvn_core::engine::DebugOutcome::Host((_, audio)) => audio,
        _ => panic!("wrong route"),
    };
    let mut cold = Engine::from_program_with_debugger(p.clone(), PcmHost::default(), 16).unwrap();
    setup(&mut cold);
    let stale_source = cold.audio_playback_views().unwrap()[0].token.clone();
    let context = cold.capture_host_lifecycle_restore_context(&p).unwrap();
    let mut resumed = cold
        .restore_save15_with_host_context(&p, save, &context)
        .unwrap()
        .unwrap();
    let views = cold.audio_playback_views().unwrap();
    let voice = views
        .iter()
        .find(|v| v.playback.family == AudioFamily::Voice)
        .unwrap();
    let cp = cold.renderer.cursor(&voice.token.target).unwrap();
    assert_eq!((cp.next_sample, cp.next_channel), (5, 1));
    assert_eq!(
        cold.renderer.pull(&voice.token.target, 4).unwrap(),
        vec![0, 105, 106, 107]
    );
    assert_ne!(views[0].token, stale_source);
    let actual = match frame_finish(&mut cold, &mut resumed) {
        rvn_core::engine::DebugOutcome::Host((_, audio)) => audio,
        _ => panic!("wrong route"),
    };
    assert_eq!(actual, expected);
    assert_eq!(cold.state.vars, hot.state.vars);
    assert_eq!(cold.state.random, hot.state.random);
    assert_eq!(
        actual[0].disposition,
        rvn_core::engine::AudioReportDisposition::RejectedStale
    );
    assert_eq!(cold.audio_playback_views().unwrap().len(), 1);
    assert_eq!(
        cold.audio_playback_views().unwrap()[0].playback.family,
        AudioFamily::Voice
    );
}
