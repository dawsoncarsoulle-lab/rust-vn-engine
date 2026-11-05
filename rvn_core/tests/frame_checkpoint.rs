// Private shared genuine runtime fixtures; filtered frame0362 tests do not
// execute/count the18 inherited0347 tests as new coverage.
include!("native_host_consumer.rs");
fn frame_program() -> ParsedProgram {
    rvn_parser::parse_program(&SOURCE.replace("screen panel(marker)",r#"function paint(state,props,frame){return [canvas_rect([0,0,10,10],[1,0,0,1],0)]}
screen panel(marker)"#).replace(r#"component("act","button""#,r#"component("clock","canvas",{"draw":"paint","state":{},"events":{"tick":"hit"}},[]), component("act","button""#)).unwrap()
}
fn frame_engine(p: ParsedProgram) -> Engine<Host> {
    let mut e = Engine::from_program_with_debugger(p, Host::default(), 16).unwrap();
    e.state.random = RandomState::seeded(4);
    e
}
fn sample_breakpoint(p: &ParsedProgram) -> rvn_core::editor_debugger::Breakpoint {
    use rvn_core::editor_debugger as w;
    let position =
        if let Ok(prepared) = rvn_core::program::PreparedNarrative::from_parsed(p.clone()) {
            let pc = prepared
                .script
                .iter()
                .position(|s| matches!(s,rvn_parser::Statement::Function{name,..}if name=="sample"))
                .unwrap();
            prepared.position(pc)
        } else {
            let pc = p
                .script
                .iter()
                .position(|s| matches!(s,rvn_parser::Statement::Function{name,..}if name=="sample"))
                .unwrap();
            rvn_core::scene::ResumePosition { pc, origin: None }
        };
    w::Breakpoint {
        position,
        site: Some(w::Site {
            function: Some("sample".into()),
            route: vec![w::Route::Statement(1)],
            depth: 2,
            statement: true,
        }),
        enabled: true,
    }
}
fn frame_finish<R: Renderer>(
    e: &mut Engine<R>,
    s: &mut rvn_core::engine::DebugSession,
) -> rvn_core::engine::DebugOutcome {
    for _ in 0..128 {
        let (progress, outcome) = e
            .debug_step(s, rvn_core::editor_debugger::StepKind::Continue)
            .unwrap();
        if progress == rvn_core::engine::DebugProgress::Complete {
            return outcome.unwrap();
        }
    }
    panic!("frame did not complete");
}
#[test]
fn frame0362_real_game_tick_cold_continuation_does_not_replay_clock_or_rng() {
    let p = frame_program();
    let mut e = frame_engine(p.clone());
    e.step_until_interaction().unwrap();
    let before = state(&e);
    let mut session = e
        .debug_begin(
            rvn_core::engine::DebugOperation::GameFrame {
                seconds: 0.1,
                reports: Default::default(),
                audio: vec![],
            },
            vec![sample_breakpoint(&p)],
        )
        .unwrap();
    assert_eq!(
        e.debug_step(&mut session, rvn_core::editor_debugger::StepKind::Continue)
            .unwrap()
            .0,
        rvn_core::engine::DebugProgress::Stopped
    );
    assert_eq!(state(&e), before);
    let save = e
        .capture_save15(&p, Some(&session), 8, "Tick real frame".into())
        .unwrap();
    let bytes = save.encode().unwrap();
    assert!(bytes.contains("frame_operation"));
    assert!(!bytes.contains("\"runtime_epoch\""));
    frame_finish(&mut e, &mut session);
    let expected = state(&e);
    let mut cold = frame_engine(p.clone());
    let mut resumed = cold
        .restore_save15(
            &p,
            rvn_core::save::consumer_checkpoint::Save15::decode(&bytes).unwrap(),
        )
        .unwrap()
        .unwrap();
    let resaved = cold
        .capture_save15(&p, Some(&resumed), 15, "Resave restored frame".into())
        .unwrap();
    let mut colder = frame_engine(p.clone());
    let mut rerestored = colder.restore_save15(&p, resaved).unwrap().unwrap();
    frame_finish(&mut colder, &mut rerestored);
    assert_eq!(state(&colder), expected);
    frame_finish(&mut cold, &mut resumed);
    assert_eq!(state(&cold), expected);
    assert_eq!(cold.state.vars["hits"], Value::Int(1));
    assert_eq!(cold.state.ui.screens[0].canvas_states["clock"].elapsed, 0.1);
    let mut forged: serde_json::Value = serde_json::from_str(&bytes).unwrap();
    forged["debugger"]["frame_operation"]["seconds"] = serde_json::json!(4000.0);
    let before = state(&cold);
    let epoch = cold.runtime_epoch();
    let count = cold.renderer.batches.lock().unwrap().len();
    assert!(cold
        .restore_save15(
            &p,
            rvn_core::save::consumer_checkpoint::Save15::decode(&forged.to_string()).unwrap()
        )
        .is_err());
    assert_eq!(state(&cold), before);
    assert_eq!(cold.runtime_epoch(), epoch);
    assert_eq!(cold.renderer.batches.lock().unwrap().len(), count);
}
#[test]
fn frame0362_video_ordinals_duplicate_end_and_stale_payload_preserve_single_ack_after_cold_resume()
{
    let base=SOURCE.replace(r#"component("act","button""#,r#"component("clock","canvas",{"draw":"paint","state":{},"events":{"tick":"hit"}},[]), component("act","button""#).replace("screen panel(marker)","function paint(state,props,frame){return []} screen panel(marker)");
    let source = base.replace(
        "\"parent\"\ncall C",
        r#"video.play("clip",video_clip("clip.webm",{"target":"ui:panel/act","on_end":"hit"}))
"parent"
call C"#,
    );
    let p = rvn_parser::parse_program(&source).unwrap();
    let mut e = frame_engine(p.clone());
    e.step_until_interaction().unwrap();
    let token = e.renderer.batches.lock().unwrap().last().unwrap().videos[0]
        .resource_token
        .clone()
        .unwrap();
    let mut stale = token.clone();
    stale.task.runtime_epoch = stale.task.runtime_epoch.checked_add(1).unwrap();
    let reports = rvn_core::engine::GameFrameReports {
        videos: vec![
            rvn_core::engine::GameVideoReport {
                token: stale,
                feedback: rvn_core::engine::GameVideoFeedback::Report(
                    rvn_core::video::Feedback::Error("ignored stale".into()),
                ),
            },
            rvn_core::engine::GameVideoReport {
                token: token.clone(),
                feedback: rvn_core::engine::GameVideoFeedback::Report(
                    rvn_core::video::Feedback::End,
                ),
            },
            rvn_core::engine::GameVideoReport {
                token: token.clone(),
                feedback: rvn_core::engine::GameVideoFeedback::Report(
                    rvn_core::video::Feedback::End,
                ),
            },
        ],
    };
    let mut session = e
        .debug_begin(
            rvn_core::engine::DebugOperation::GameFrame {
                seconds: 0.01,
                reports,
                audio: vec![],
            },
            vec![sample_breakpoint(&p)],
        )
        .unwrap();
    assert_eq!(
        e.debug_step(&mut session, rvn_core::editor_debugger::StepKind::Continue)
            .unwrap()
            .0,
        rvn_core::engine::DebugProgress::Stopped
    );
    let save = e
        .capture_save15(&p, Some(&session), 9, "Video ordinal frame".into())
        .unwrap();
    let expected = match frame_finish(&mut e, &mut session) {
        rvn_core::engine::DebugOutcome::Game((frame, _)) => frame,
        _ => panic!("wrong route"),
    };
    let expected_state = state(&e);
    let mut cold = frame_engine(p.clone());
    let mut resumed = cold.restore_save15(&p, save).unwrap().unwrap();
    let actual = match frame_finish(&mut cold, &mut resumed) {
        rvn_core::engine::DebugOutcome::Game((frame, _)) => frame,
        _ => panic!("wrong route"),
    };
    assert_eq!(actual.receipts, expected.receipts);
    assert_eq!(actual.frame, expected.frame);
    assert_eq!(state(&cold), expected_state);
    assert_eq!(cold.state.vars["hits"], Value::Int(2));
    assert_eq!(
        actual.receipts[0].disposition,
        rvn_core::engine::VideoReportDisposition::RejectedStale
    );
}

fn host_frame_program() -> ParsedProgram {
    let host = r#"
 function paint(state,props,frame){return []}
 screen host(context){flow.consume_v2("screen:host","gather"){capture "draw"=sample() inspect "i"=entry return component("hostroot","column",{},[
 component("clock","canvas",{"draw":"paint","state":{},"events":{"tick":"hit"}},[])])}}
 "#;
    rvn_parser::parse_program(&format!("{}\n{}", SOURCE, host)).unwrap()
}
fn open_frame_host<R: Renderer>(e: &mut Engine<R>) {
    e.synchronize_source_menu(
        rvn_ui::PageRole::Pause,
        Some("host"),
        Value::Dict(Default::default()),
        true,
        500,
    )
    .unwrap();
}
#[test]
fn frame0362_host_tick_requires_real_recreated_lifecycle_and_resumes_its_own_frame() {
    let scene = host_frame_program();
    let legacy =
        rvn_parser::parse_program(&scene.sources[0].source.replace("scene label", "label"))
            .unwrap();
    for p in [scene, legacy] {
        let mut hot = frame_engine(p.clone());
        hot.step_until_interaction().unwrap();
        open_frame_host(&mut hot);
        let target = hot.capture_ui_input_source("host").unwrap();
        let before = state(&hot);
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
        assert_eq!(state(&hot), before);
        let save = hot
            .capture_save15(&p, Some(&session), 10, "Host actual lifecycle".into())
            .unwrap();
        let bytes = save.encode().unwrap();
        let expected = match frame_finish(&mut hot, &mut session) {
            rvn_core::engine::DebugOutcome::Host((f, _)) => f,
            _ => panic!("Host continuation changed clock"),
        };
        let expected_state = state(&hot);
        let mut missing = frame_engine(p.clone());
        missing.step_until_interaction().unwrap();
        let missing_before = state(&missing);
        let epoch = missing.runtime_epoch();
        assert!(missing.restore_save15(&p, save.clone()).is_err());
        assert_eq!(state(&missing), missing_before);
        assert_eq!(missing.runtime_epoch(), epoch);
        let mut cold = frame_engine(p.clone());
        cold.step_until_interaction().unwrap();
        open_frame_host(&mut cold);
        let stale = cold.capture_ui_input_source("host").unwrap();
        let mut resumed = cold
            .restore_save15(
                &p,
                rvn_core::save::consumer_checkpoint::Save15::decode(&bytes).unwrap(),
            )
            .unwrap()
            .unwrap();
        let actual = match frame_finish(&mut cold, &mut resumed) {
            rvn_core::engine::DebugOutcome::Host((f, _)) => f,
            _ => panic!("wrong clock"),
        };
        assert_eq!(actual.frame, expected.frame);
        assert_eq!(state(&cold), expected_state);
        assert_eq!(cold.state.vars["hits"], Value::Int(1));
        let instance = cold
            .state
            .ui
            .screens
            .iter()
            .find(|i| i.name == "host")
            .unwrap();
        assert_eq!(instance.host_role, Some(rvn_ui::PageRole::Pause));
        assert_eq!(instance.canvas_states["clock"].elapsed, 0.1);
        assert!(instance.consumer_frames.contains_key("handler:hit"));
        for source in [stale, target] {
            assert!(!cold.ui_input_source_current(&source));
        }
        let mut replacement = frame_engine(p.clone());
        replacement.step_until_interaction().unwrap();
        open_frame_host(&mut replacement);
        replacement
            .synchronize_source_menu(
                rvn_ui::PageRole::Pause,
                None,
                Value::Dict(Default::default()),
                true,
                500,
            )
            .unwrap();
        open_frame_host(&mut replacement);
        let prior = state(&replacement);
        assert!(replacement.restore_save15(&p, save).is_err());
        assert_eq!(state(&replacement), prior);
    }
}

/// Descriptor fixture with an explicit controlled PCM cursor. This models the
/// renderer fence; it does not qualify a physical decoder or device.
#[derive(Clone, Default)]
struct AudioHost {
    inner: Host,
}
impl Renderer for AudioHost {
    fn supports_composable_motion(&self)->bool{true}
    fn update_motions(&mut self,_:&[rvn_core::motion::MotionView])->Result<(),String>{Ok(())}

    fn capture_audio_checkpoint(
        &self,
        s: &rvn_core::GameState,
        id: u64,
        epoch: u64,
    ) -> Result<rvn_core::audio_checkpoint::AudioCheckpointSnapshot, String> {
        Ok(rvn_core::audio_checkpoint::AudioCheckpointSnapshot {
            instance_id: id,
            runtime_epoch: epoch,
            rows: rvn_core::audio_playback::describe(s, epoch)?
                .into_iter()
                .filter(|v| {
                    !matches!(
                        v.token.task.key.owner,
                        rvn_core::scene_tasks::TaskOwner::Host { .. }
                    )
                })
                .map(|v| rvn_core::audio_checkpoint::AudioCheckpointRow {
                    token: v.token,
                    checkpoint: rvn_core::audio_checkpoint::AudioCheckpoint {
                        version: 1,
                        asset_sha256: [17; 32],
                        pcm_sha256: [19; 32],
                        channels: 2,
                        sample_rate: 48000,
                        total_samples: 8,
                        next_sample: 3,
                        next_channel: 1,
                        looping: false,
                        loops_completed: 0,
                    },
                })
                .collect(),
            music: vec![],
        })
    }
    fn validate_audio_restore(&self, s: &rvn_core::GameState) -> Result<(), String> {
        rvn_core::audio_checkpoint::require_saved_checkpoints(s)?;
        for v in rvn_core::audio_playback::describe(s, 0)? {
            let c = v.playback.checkpoint.ok_or("Fixture checkpoint absent")?;
            if v.playback.file != "sfx/frame.ogg"
                || c.asset_sha256 != [17; 32]
                || c.pcm_sha256 != [19; 32]
            {
                return Err("Fixture audio source changed".into());
            }
        }
        Ok(())
    }
    fn validate_scene_batch(&self, b: &SceneRenderBatch) -> Result<(), String> {
        self.inner.validate_scene_batch(b)
    }
    fn publish_scene_batch(&mut self, b: SceneRenderBatch) {
        self.inner.publish_scene_batch(b)
    }
    fn supports_programmable_ui(&self) -> bool {
        true
    }
    fn supports_custom_canvas(&self) -> bool {
        true
    }
    fn update_interfaces(&mut self, _: &[rvn_ui::programmable::ScreenView]) -> Result<(), String> {
        Ok(())
    }
    fn set_background(&mut self, a: &str, b: &Transition) {
        self.inner.set_background(a, b)
    }
    fn show_sprite(
        &mut self,
        a: &str,
        b: Option<&str>,
        c: &Position,
        d: &Transition,
        e: Option<&SpriteState>,
    ) {
        self.inner.show_sprite(a, b, c, d, e)
    }
    fn hide_sprite(&mut self, a: &str, b: &Transition, c: &SpriteState) {
        self.inner.hide_sprite(a, b, c)
    }
    fn move_sprite(&mut self, a: &str, b: &Position, c: &Transition, d: &SpriteState) {
        self.inner.move_sprite(a, b, c, d)
    }
    fn show_dialogue(&mut self, a: Option<&str>, b: &str) {
        self.inner.show_dialogue(a, b)
    }
    fn show_choice(&mut self, a: &[String]) -> usize {
        self.inner.show_choice(a)
    }
    fn show_imagemap(&mut self, a: &str, b: Option<&str>, c: &[Hotspot]) -> usize {
        self.inner.show_imagemap(a, b, c)
    }
    fn music_play(&mut self, a: &str, b: &Transition, c: Option<&str>) {
        self.inner.music_play(a, b, c)
    }
    fn music_stop(&mut self, a: &Transition) {
        self.inner.music_stop(a)
    }
    fn music_set_volume(&mut self, a: f32) {
        self.inner.music_set_volume(a)
    }
    fn sfx_play(&mut self, a: &str, b: &Transition) {
        self.inner.sfx_play(a, b)
    }
    fn sfx_stop(&mut self, a: &str, b: &Transition) {
        self.inner.sfx_stop(a, b)
    }
}
#[test]
fn frame0362_audio_end_receipt_uses_rehydrated_token_and_current_engine_lineage() {
    let source=SOURCE.replace("screen panel(marker)",r#"function paint(state,props,frame){return []} screen panel(marker)"#).replace(r#"component("act","button""#,r#"component("clock","canvas",{"draw":"paint","state":{},"events":{"tick":"hit"}},[]), component("act","button""#).replace("\"parent\"\ncall C","sfx.play(\"frame.ogg\")\n\"parent\"\ncall C");
    let p = rvn_parser::parse_program(&source).unwrap();
    let mut hot = Engine::from_program_with_debugger(p.clone(), AudioHost::default(), 16).unwrap();
    hot.state.random = RandomState::seeded(4);
    hot.step_until_interaction().unwrap();
    let token = hot.audio_playback_views().unwrap()[0].token.clone();
    let real = rvn_core::audio_playback::AudioReport {
        instance_id: hot.instance_id(),
        token,
        feedback: rvn_core::audio_playback::AudioFeedback::End,
    };
    let mut stale = real.clone();
    stale.instance_id += 1;
    let mut session = hot
        .debug_begin(
            rvn_core::engine::DebugOperation::GameFrame {
                seconds: 0.1,
                reports: Default::default(),
                audio: vec![stale, real.clone(), real],
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
        .capture_save15(&p, Some(&session), 11, "Real audio reports".into())
        .unwrap();
    let expected = match frame_finish(&mut hot, &mut session) {
        rvn_core::engine::DebugOutcome::Game((_, r)) => r,
        _ => panic!("wrong route"),
    };
    let expected_state = serde_json::to_value(&hot.state).unwrap();
    let mut cold = Engine::from_program_with_debugger(p.clone(), AudioHost::default(), 16).unwrap();
    let mut resumed = cold.restore_save15(&p, save).unwrap().unwrap();
    let actual = match frame_finish(&mut cold, &mut resumed) {
        rvn_core::engine::DebugOutcome::Game((_, r)) => r,
        _ => panic!("wrong route"),
    };
    assert_eq!(actual, expected);
    assert_eq!(
        actual[0].disposition,
        rvn_core::engine::AudioReportDisposition::RejectedStale
    );
    assert_eq!(serde_json::to_value(&cold.state).unwrap(), expected_state);
    assert!(cold.audio_playback_views().unwrap().is_empty());
    assert_eq!(cold.state.vars["hits"], Value::Int(1));
}

#[test]
fn frame0362_source_report_target_and_inventory_forgeries_refuse_atomically() {
    let source=SOURCE.replace("screen panel(marker)",r#"function paint(state,props,frame){return []} screen panel(marker)"#).replace(r#"component("act","button""#,r#"component("clock","canvas",{"draw":"paint","state":{},"events":{"tick":"hit"}},[]), component("act","button""#).replace("\"parent\"\ncall C",r#"video.play("clip",video_clip("clip.webm",{"target":"ui:panel/act","on_end":"hit"}))
"parent"
call C"#);
    let p = rvn_parser::parse_program(&source).unwrap();
    let mut hot = frame_engine(p.clone());
    hot.step_until_interaction().unwrap();
    let token = hot.renderer.batches.lock().unwrap().last().unwrap().videos[0]
        .resource_token
        .clone()
        .unwrap();
    let mut session = hot
        .debug_begin(
            rvn_core::engine::DebugOperation::GameFrame {
                seconds: 0.1,
                reports: rvn_core::engine::GameFrameReports {
                    videos: vec![rvn_core::engine::GameVideoReport {
                        token,
                        feedback: rvn_core::engine::GameVideoFeedback::Report(
                            rvn_core::video::Feedback::End,
                        ),
                    }],
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
    let original = hot
        .capture_save15(&p, Some(&session), 12, "Refusal sources".into())
        .unwrap();
    for case in 0..9 {
        let mut forged = serde_json::to_value(&original).unwrap();
        let f = &mut forged["debugger"]["frame_operation"];
        match case {
            0 => f["videos"][0]["source"]["target"]["incarnation"] = 999.into(),
            1 => f["videos"][0]["source"]["revision"] = 999.into(),
            2 => f["videos"][0]["source"]["payload"][0] = 255.into(),
            3 => f["videos"][0]["source"]["kind"] = "voice".into(),
            4 => f["interfaces"][0]["source_ordinal"] = 999.into(),
            5 => f["interfaces"][0]["target"]["alias"] = "other".into(),
            6 => f["clock"] = "host".into(),
            7 => {
                let a = f["interfaces"].as_array_mut().unwrap();
                a.push(a[0].clone());
            }
            _ => f["seconds"] = 4000.0.into(),
        }
        let mut cold = frame_engine(p.clone());
        let before = state(&cold);
        let random = cold.state.random.clone();
        let epoch = cold.runtime_epoch();
        let count = cold.renderer.batches.lock().unwrap().len();
        let bad = rvn_core::save::consumer_checkpoint::Save15::decode(&forged.to_string()).unwrap();
        assert!(
            cold.restore_save15(&p, bad).is_err(),
            "forgery {case} was admitted"
        );
        assert_eq!(state(&cold), before, "forgery {case}");
        assert_eq!(cold.state.random, random);
        assert_eq!(cold.runtime_epoch(), epoch);
        assert_eq!(cold.renderer.batches.lock().unwrap().len(), count);
    }
    let mut altered = p.clone();
    altered.sources[0].source = altered.sources[0].source.replace("clip.webm", "other.webm");
    let altered = rvn_parser::parse_program(&altered.sources[0].source).unwrap();
    let mut cold = frame_engine(altered.clone());
    let before = state(&cold);
    assert!(cold.restore_save15(&altered, original).is_err());
    assert_eq!(state(&cold), before);
}

#[test]
fn frame0362_real_host_video_end_runs_callback_once_and_stale_receipts_keep_order() {
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
    let mut resumed = cold.restore_save15(&p, save).unwrap().unwrap();
    let actual = match frame_finish(&mut cold, &mut resumed) {
        rvn_core::engine::DebugOutcome::Host((f, _)) => f,
        _ => panic!("wrong clock"),
    };
    assert_eq!(actual.receipts, expected.receipts);
    assert_eq!(actual.frame, expected.frame);
    assert_eq!(state(&cold), expected_state);
    assert_eq!(cold.state.vars["hits"], Value::Int(2));
    assert_eq!(
        actual.receipts[0].disposition,
        rvn_core::engine::VideoReportDisposition::RejectedStale
    );
}

#[test]
fn frame0362_host_frame_defers_true_story_audio_instead_of_reassigning_its_owner() {
    let base = host_frame_program();
    let p = rvn_parser::parse_program(&base.sources[0].source.replace(
        "\"parent\"\ncall C",
        "sfx.play(\"frame.ogg\")\n\"parent\"\ncall C",
    ))
    .unwrap();
    let setup = |e: &mut Engine<AudioHost>| {
        e.step_until_interaction().unwrap();
        open_frame_host(e);
    };
    let mut hot = Engine::from_program_with_debugger(p.clone(), AudioHost::default(), 16).unwrap();
    hot.state.random = RandomState::seeded(4);
    setup(&mut hot);
    let report = rvn_core::audio_playback::AudioReport {
        instance_id: hot.instance_id(),
        token: hot.audio_playback_views().unwrap()[0].token.clone(),
        feedback: rvn_core::audio_playback::AudioFeedback::End,
    };
    assert!(matches!(
        report.token.task.key.owner,
        rvn_core::scene_tasks::TaskOwner::Scene { .. }
    ));
    let mut session = hot
        .debug_begin(
            rvn_core::engine::DebugOperation::HostFrame {
                seconds: 0.1,
                reports: Default::default(),
                audio: vec![report],
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
            17,
            "Host clock deferred Story audio".into(),
        )
        .unwrap();
    let expected = match frame_finish(&mut hot, &mut session) {
        rvn_core::engine::DebugOutcome::Host((_, a)) => a,
        _ => panic!("wrong clock"),
    };
    let mut cold = Engine::from_program_with_debugger(p.clone(), AudioHost::default(), 16).unwrap();
    setup(&mut cold);
    let mut resumed = cold.restore_save15(&p, save).unwrap().unwrap();
    let actual = match frame_finish(&mut cold, &mut resumed) {
        rvn_core::engine::DebugOutcome::Host((_, a)) => a,
        _ => panic!("wrong clock"),
    };
    assert_eq!(actual, expected);
    assert_eq!(
        actual[0].disposition,
        rvn_core::engine::AudioReportDisposition::Deferred
    );
    assert_eq!(hot.audio_playback_views().unwrap().len(), 1);
    assert_eq!(cold.audio_playback_views().unwrap().len(), 1);
    let owner = &cold.audio_playback_views().unwrap()[0].token.task.key.owner;
    assert!(matches!(
        owner,
        rvn_core::scene_tasks::TaskOwner::Scene { .. }
    ));
    assert_eq!(cold.state.vars["hits"], Value::Int(1));
    // Metadata is freshly captured at Save; compare playback family/file/envelope
    // and all non-PCM state, without claiming a renderer-owned checkpoint existed
    // on the hot state before the explicit fence.
    let mut expected_state = hot.state.clone();
    for task in &mut expected_state
        .scene_execution
        .as_mut()
        .unwrap()
        .resources
        .tasks
    {
        if let Some(audio) = &mut task.audio {
            audio.checkpoint = cold
                .state
                .scene_execution
                .as_ref()
                .unwrap()
                .resources
                .tasks
                .iter()
                .find(|t| t.key == task.key)
                .unwrap()
                .audio
                .as_ref()
                .unwrap()
                .checkpoint
                .clone();
        }
    }
    assert_eq!(
        serde_json::to_value(&cold.state).unwrap(),
        serde_json::to_value(&expected_state).unwrap()
    );
}

#[test]
fn frame0362_memory_source_promotion_restores_actual_file_source() {
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
        .capture_save15(&p, Some(&session), 18, "Relocated Host source".into())
        .unwrap();
    frame_finish(&mut hot, &mut session);
    let mut relocated = p.clone();
    let path = Some(std::path::PathBuf::from(
        "C:/private/relocated-host0362.rvn",
    ));
    relocated.sources[0].path = path.clone();
    for origin in &mut relocated.origins {
        origin.source = path.clone();
    }
    let mut cold = frame_engine(relocated.clone());
    cold.step_until_interaction().unwrap();
    open_frame_host(&mut cold);
    let mut resumed = cold.restore_save15(&relocated, save).unwrap().unwrap();
    frame_finish(&mut cold, &mut resumed);
    assert_eq!(cold.state.vars, hot.state.vars);
    assert_eq!(cold.state.random, hot.state.random);
    let host = cold
        .state
        .ui
        .screens
        .iter()
        .find(|i| i.name == "host")
        .unwrap();
    assert_eq!(host.host_role, Some(rvn_ui::PageRole::Pause));
    assert_eq!(host.canvas_states["clock"].elapsed, 0.1);
    assert_eq!(host.consumer_frames["handler:hit"].owner.source_ordinal, 0);
}

#[test]
fn frame0362_host_definition_frames_rehydrate_from_relocated_real_source_without_saved_salt() {
    let mut p = host_frame_program();
    let original_path = Some(std::path::PathBuf::from("C:/private/original-host0362.rvn"));
    p.sources[0].path = original_path.clone();
    for origin in &mut p.origins {
        origin.source = original_path.clone();
    }
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
        .capture_save15(&p, Some(&session), 18, "Relocated Host source".into())
        .unwrap();
    frame_finish(&mut hot, &mut session);
    let mut relocated = p.clone();
    let path = Some(std::path::PathBuf::from(
        "C:/private/relocated-host0362.rvn",
    ));
    relocated.sources[0].path = path.clone();
    for origin in &mut relocated.origins {
        origin.source = path.clone();
    }
    let mut cold = frame_engine(relocated.clone());
    cold.step_until_interaction().unwrap();
    open_frame_host(&mut cold);
    let mut resumed = cold.restore_save15(&relocated, save).unwrap().unwrap();
    frame_finish(&mut cold, &mut resumed);
    assert_eq!(cold.state.vars, hot.state.vars);
    assert_eq!(cold.state.random, hot.state.random);
    let host = cold
        .state
        .ui
        .screens
        .iter()
        .find(|i| i.name == "host")
        .unwrap();
    assert_eq!(host.host_role, Some(rvn_ui::PageRole::Pause));
    assert_eq!(host.canvas_states["clock"].elapsed, 0.1);
    assert_eq!(host.consumer_frames["handler:hit"].owner.source_ordinal, 0);
}

#[test]
fn world0381_frame_warming_uses_recreated_host_and_refuses_missing_lifecycle_without_effects() {
    let p=host_frame_program();
    let mut hot=frame_engine(p.clone());hot.step_until_interaction().unwrap();open_frame_host(&mut hot);
    let mut session=hot.debug_begin(rvn_core::engine::DebugOperation::HostFrame{seconds:0.1,reports:Default::default(),audio:vec![]},vec![sample_breakpoint(&p)]).unwrap();
    assert_eq!(hot.debug_step(&mut session,rvn_core::editor_debugger::StepKind::Continue).unwrap().0,rvn_core::engine::DebugProgress::Stopped);
    let save=hot.capture_save15(&p,Some(&session),17,"Actual Host warming".into()).unwrap();
    let mut missing=frame_engine(p.clone());missing.step_until_interaction().unwrap();
    let before=state(&missing);let source=missing.debug_state_source().unwrap();let batches=missing.renderer.batches.lock().unwrap().len();
    assert!(missing.save15_state_for_audio_warming(&p,&save).is_err());
    assert_eq!(state(&missing),before);assert!(missing.accepts_debug_state_source(&source));assert_eq!(missing.renderer.batches.lock().unwrap().len(),batches);
    let mut cold=frame_engine(p.clone());cold.step_until_interaction().unwrap();open_frame_host(&mut cold);
    let before=state(&cold);let source=cold.debug_state_source().unwrap();let batches=cold.renderer.batches.lock().unwrap().len();
    let warming=cold.save15_state_for_audio_warming(&p,&save).unwrap();
    let host=warming.ui.screens.iter().find(|s|s.name=="host").unwrap();
    assert_eq!(host.host_role,Some(rvn_ui::PageRole::Pause));
    assert!(host.consumer_frames.contains_key("screen:host"));
    assert_eq!(state(&cold),before);assert!(cold.accepts_debug_state_source(&source));assert_eq!(cold.renderer.batches.lock().unwrap().len(),batches);
}
