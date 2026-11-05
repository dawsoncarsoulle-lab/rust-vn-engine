use rvn_core::renderer::SceneRenderBatch;
use rvn_core::save::{SaveData, SAVE_FORMAT_VERSION, SCENE_SAVE_FORMAT_VERSION};
use rvn_core::{Engine, Interaction, Renderer, SpriteState};
use rvn_parser::{Hotspot, Position, Transition, Value};

#[derive(Default)]
struct Host {
    batches: Vec<SceneRenderBatch>,
    reject: bool,
}
impl Renderer for Host {
    fn supports_composable_motion(&self) -> bool {
        true
    }
    fn validate_scene_batch(&self, _: &SceneRenderBatch) -> Result<(), String> {
        if self.reject {
            Err("Host cannot prepare this replacement".into())
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
fn state_json(game: &Engine<Host>) -> serde_json::Value {
    serde_json::to_value(&game.state).unwrap()
}

#[test]
fn scene_call_restores_locals_but_functions_keep_lexical_game_globals() {
    let mut game = game("function read() { return x }\nfunction identity(value) { return value }\ninit { set x=2 }\nscene label P\nlocal x=50\ncall A\n\"[x]/[read()]/[identity(x)]\"\njump done\nscene label A\nset x=9\nlocal x=100\n\"child [x]\"\nreturn\nlabel done\n");
    assert_eq!(dialogue(&mut game), "child 100");
    let child = game
        .state
        .scene_execution
        .as_ref()
        .unwrap()
        .active
        .as_ref()
        .unwrap()
        .id;
    assert_eq!(game.state.vars["x"], Value::Int(9));
    let saved = SaveData::from_state(&game.state, 1, "child".into(), "story.rvn".into());
    assert_eq!(saved.format_version, SCENE_SAVE_FORMAT_VERSION);
    let saved = serde_json::from_slice::<SaveData>(&serde_json::to_vec(&saved).unwrap()).unwrap();
    game.advance_dialogue().unwrap();
    assert_eq!(dialogue(&mut game), "50/9/50");
    assert_ne!(
        game.state
            .scene_execution
            .as_ref()
            .unwrap()
            .active
            .as_ref()
            .unwrap()
            .id,
        child
    );
    game.load_data(saved).unwrap();
    assert_eq!(
        game.state
            .scene_execution
            .as_ref()
            .unwrap()
            .active
            .as_ref()
            .unwrap()
            .id,
        child
    );
    game.advance_dialogue().unwrap();
    assert_eq!(dialogue(&mut game), "50/9/50");
}

#[test]
fn a_jump_replaces_a_visit_and_keeps_its_real_caller() {
    let mut game = game("scene label P\nlocal parent=7\ncall A\n\"parent [parent]\"\njump end\nscene label A\nlocal abandoned=1\nif true { jump B }\nscene label B\nlocal child=2\n\"B [child]\"\nreturn\nlabel end\n");
    assert_eq!(dialogue(&mut game), "B 2");
    let execution = game.state.scene_execution.as_ref().unwrap();
    assert_eq!(execution.frames.len(), 1);
    assert!(!execution
        .active
        .as_ref()
        .unwrap()
        .locals
        .contains_key("abandoned"));
    game.advance_dialogue().unwrap();
    assert_eq!(dialogue(&mut game), "parent 7");
    assert!(game.state.call_stack.is_empty());
}

#[test]
fn reentering_the_same_scene_initializes_a_new_visit_exactly_once() {
    let mut game =
        game("scene label A\nlocal visits=1\n\"visit [visits]\"\nset visits=2\njump A\n");
    assert_eq!(dialogue(&mut game), "visit 1");
    let first = game.state.scene_execution.as_ref().unwrap().owner();
    game.advance_dialogue().unwrap();
    assert_eq!(dialogue(&mut game), "visit 1");
    assert_ne!(game.state.scene_execution.as_ref().unwrap().owner(), first);
    assert!(!game.state.vars.contains_key("visits"));
}

#[test]
fn helper_labels_and_background_changes_keep_the_active_visit() {
    let mut game = game("scene label A\nlocal value=4\ncall helper\n\"after [value]\"\njump end\nlabel helper\nscene forest\nset value=5\n\"helper [value]\"\nreturn\nlabel end\n");
    assert_eq!(dialogue(&mut game), "helper 5");
    let id = game.state.scene_execution.as_ref().unwrap().owner();
    game.advance_dialogue().unwrap();
    assert_eq!(dialogue(&mut game), "after 5");
    assert_eq!(game.state.scene_execution.as_ref().unwrap().owner(), id);
    assert_eq!(game.state.background_image, "forest");
}

#[test]
fn narrative_preview_reads_locals_but_does_not_advance_random_or_runtime() {
    let mut game = game("function read() { return value }\ninit { set value=3 }\nscene label A\nlocal value=8\n\"waiting\"\n");
    assert_eq!(dialogue(&mut game), "waiting");
    let before = state_json(&game);
    let batches = game.renderer.batches.len();
    assert_eq!(
        game.eval_narrative_expression(&rvn_parser::Expr::Var("value".into()))
            .unwrap(),
        Value::Int(8)
    );
    let expressions = rvn_parser::parse("set result=read()\nset result=random(1,100)\n").unwrap();
    for (index, statement) in expressions.into_iter().enumerate() {
        let rvn_parser::Statement::SetVar { value, .. } = statement else {
            panic!("expected an expression fixture")
        };
        let first = game.eval_narrative_expression(&value).unwrap();
        let second = game.eval_narrative_expression(&value).unwrap();
        assert_eq!(first, second);
        if index == 0 {
            assert_eq!(first, Value::Int(3));
        }
    }
    assert!(game
        .eval_narrative_expression(&rvn_parser::Expr::Var("missing".into()))
        .is_err());
    assert_eq!(state_json(&game), before);
    assert_eq!(game.renderer.batches.len(), batches);
}

#[test]
fn global_initializers_keep_a_valid_boundary_and_fresh_batches_use_new_generations() {
    let mut original = game("init { set count=1 }\ninit { set count=count+1 }\nscene label A\nlocal value=count\n\"[value]\"\n");
    assert_eq!(dialogue(&mut original), "2");
    let saved = SaveData::from_state(&original.state, 1, "A".into(), "story.rvn".into());
    original.load_data(saved).unwrap();
    let old_epoch = original.runtime_epoch();
    let mut fresh = original.fresh(Host::default(), 32).unwrap();
    assert_eq!(fresh.state.vars["count"], Value::Int(2));
    assert!(fresh.runtime_epoch() > old_epoch);
    assert!(!fresh.renderer.batches.is_empty());
    assert!(fresh
        .renderer
        .batches
        .iter()
        .all(|batch| batch.runtime_epoch == fresh.runtime_epoch()));
    assert_eq!(dialogue(&mut fresh), "2");
    assert_eq!(original.state.vars["count"], Value::Int(2));
}

#[test]
fn an_animation_authored_before_the_first_projection_survives_a_silent_scene_call() {
    use rvn_core::scene_resources::ResourceTaskKind;
    let mut game = game("scene label A\neileen.show(\"smile\") at left\neileen.animate(\"shake\", intensity:12, duration:0.3)\ncall B\n\"parent\"\njump end\nscene label B\n\"child\"\nreturn\nlabel end\n");
    assert_eq!(dialogue(&mut game), "child");
    let execution = game.state.scene_execution.as_ref().unwrap();
    let task = execution
        .resources
        .tasks
        .iter()
        .find(|task| task.kind == ResourceTaskKind::Effect)
        .unwrap();
    assert!(task.parked);
    assert_eq!(task.effect.as_ref().unwrap().clock.elapsed, 0.0);
    assert!(!task.effect.as_ref().unwrap().base_resolved);
    let saved = SaveData::from_state(&game.state, 1, "child".into(), "story.rvn".into());
    game.load_data(saved).unwrap();
    game.advance_dialogue().unwrap();
    assert_eq!(dialogue(&mut game), "parent");
    let effects = game
        .state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .active_effects(game.state.scene_execution.as_ref().unwrap().owner());
    assert_eq!(effects.len(), 1);
    assert_eq!(effects[0].effect.as_ref().unwrap().clock.elapsed, 0.0);
    assert!(game
        .scene_resource_token(ResourceTaskKind::Effect, "sprite/eileen/animation")
        .is_some());
}

#[test]
fn a_stale_effect_snapshot_is_ignored_before_its_payload_is_validated() {
    use rvn_core::scene_resources::ResourceTaskKind;
    let mut game = game("scene label A\neileen.show(\"smile\") at left\neileen.animate(\"pulse\")\n\"A\"\nscene label B\n\"B\"\n");
    assert_eq!(dialogue(&mut game), "A");
    let token = game
        .scene_resource_token(ResourceTaskKind::Effect, "sprite/eileen/animation")
        .unwrap();
    let mut effect = game
        .state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .tasks
        .iter()
        .find_map(|task| task.effect.clone())
        .unwrap();
    game.execute_timer_action("jump B").unwrap();
    effect.clock.elapsed = f64::NAN;
    let before = state_json(&game);
    assert!(!game.capture_scene_effect(&token, effect).unwrap());
    assert!(!game.complete_scene_effect(&token).unwrap());
    assert_eq!(state_json(&game), before);
}

#[test]
fn scene_motion_wait_saves_the_next_position_and_never_exposes_it_early() {
    let mut game = game("scene label A\neileen.show(\"smile\") at left\n\"start\"\nmotion.wait(\"sprite:eileen\")\n\"done\"\n");
    assert_eq!(dialogue(&mut game), "start");
    game.state
        .motions
        .play(
            rvn_core::motion::MotionTarget::Sprite {
                id: "eileen".into(),
            },
            rvn_ui::motion::Motion::Pause { seconds: 1.0 },
        )
        .unwrap();
    game.advance_dialogue().unwrap();
    assert!(game.step_until_interaction().unwrap().is_none());
    let waiting_pc = game.state.pc;
    assert!(matches!(
        game.script[waiting_pc],
        rvn_parser::Statement::Dialogue { .. }
    ));
    assert!(game.current_interaction().unwrap().is_none());
    let saved = SaveData::from_state(&game.state, 1, "wait".into(), "story.rvn".into());
    assert_eq!(saved.pc, waiting_pc);
    game.load_data(saved).unwrap();
    game.tick_motions(0.5).unwrap();
    assert!(game.current_interaction().unwrap().is_none());
    assert_eq!(game.state.pc, waiting_pc);
    game.tick_motions(0.5).unwrap();
    assert_eq!(game.state.pc, waiting_pc);
    assert_eq!(dialogue(&mut game), "done");
}

#[test]
fn a_helper_local_before_the_first_scene_rejects_before_random_evaluation() {
    let mut game = game(
        "call helper\nscene label A\n\"A\"\nlabel helper\nlocal value=random(1,100)\nreturn\n",
    );
    game.step_silent().unwrap_err();
    let state = state_json(&game);
    assert!(game.step_silent().is_err());
    assert_eq!(state_json(&game), state);
    assert!(game
        .state
        .scene_execution
        .as_ref()
        .unwrap()
        .active
        .is_none());
}

#[test]
fn foreach_temporaries_resume_after_a_child_and_do_not_leak_into_globals() {
    let mut game = game("scene label A\nfor item in [10,20] { call B \"item [item]\" }\njump end\nscene label B\nlocal private=1\n\"child\"\nreturn\nlabel end\n");
    assert_eq!(dialogue(&mut game), "child");
    let saved = SaveData::from_state(&game.state, 1, "loop".into(), "story.rvn".into());
    game.load_data(saved).unwrap();
    game.advance_dialogue().unwrap();
    assert_eq!(dialogue(&mut game), "item 10");
    game.advance_dialogue().unwrap();
    assert_eq!(dialogue(&mut game), "child");
    game.advance_dialogue().unwrap();
    assert_eq!(dialogue(&mut game), "item 20");
    assert!(game.state.vars.keys().all(|key| key == "item"));
}

#[test]
fn authored_return_unwinds_branch_scaffolding_and_synthetic_return_does_not() {
    for choice in 0..2 {
        let mut game = game("scene label P\ncall A\n\"returned\"\njump end\nscene label A\nchoice { \"A\" => { if true { return } } \"B\" => { return } }\n\"never\"\nlabel end\n");
        assert!(matches!(
            game.step_until_interaction().unwrap(),
            Some(Interaction::Choice { .. })
        ));
        game.submit_choice(choice).unwrap();
        assert_eq!(dialogue(&mut game), "returned");
        assert!(game.state.call_stack.is_empty());
    }
}

#[test]
fn last_stage_renderer_rejection_leaves_the_whole_scene_candidate_unpublished() {
    let mut game =
        game("scene label P\n\"P\"\ncall A\nscene label A\nlocal number=random(1,100)\n\"A\"\n");
    assert_eq!(dialogue(&mut game), "P");
    game.advance_dialogue().unwrap();
    game.renderer.reject = true;
    let state = state_json(&game);
    let history = game.history.len();
    let epoch = game.runtime_epoch();
    let batches = game.renderer.batches.len();
    assert!(game.step_silent().is_err());
    assert_eq!(state_json(&game), state);
    assert_eq!(game.history.len(), history);
    assert_eq!(game.runtime_epoch(), epoch);
    assert_eq!(game.renderer.batches.len(), batches);
}

#[test]
fn save_captures_live_continuation_separately_from_the_displayed_dialogue() {
    let mut game = game("scene label A\nlocal value=3\n\"visible [value]\"\njump end\nscene label B\nlocal value=9\n\"new [value]\"\nlabel end\n");
    assert_eq!(dialogue(&mut game), "visible 3");
    game.execute_timer_action("jump B").unwrap();
    let before = state_json(&game);
    assert_eq!(
        game.displayed_interaction().unwrap(),
        Some(Interaction::Dialogue {
            character: None,
            text: "visible 3".into()
        })
    );
    let saved = SaveData::from_state(&game.state, 1, "async".into(), "story.rvn".into());
    assert_eq!(saved.pc, game.state.pc);
    assert_ne!(saved.pc, game.state.current_interactive_pc);
    game.step_silent().unwrap();
    game.load_data(saved).unwrap();
    assert_eq!(state_json(&game), before);
    assert_eq!(
        game.renderer.batches.last().unwrap().display,
        Some(Interaction::Dialogue {
            character: None,
            text: "visible 3".into()
        })
    );
    game.advance_dialogue().unwrap();
    assert_eq!(dialogue(&mut game), "new 9");
}

#[test]
fn malformed_scene_save_and_changed_story_preserve_state_history_and_epochs() {
    let source = "scene label A\nlocal n=2\n\"A [n]\"\n";
    let mut game = game(source);
    dialogue(&mut game);
    let valid = SaveData::from_state(&game.state, 1, "A".into(), "story.rvn".into());
    let state = state_json(&game);
    let epoch = game.runtime_epoch();
    let history = game.history.len();
    for variation in 0..5 {
        let mut data = valid.clone();
        match variation {
            0 => data.format_version = SAVE_FORMAT_VERSION,
            1 => data.scene_execution = None,
            2 => data.scene_execution.as_mut().unwrap().live_position.origin = None,
            3 => {
                data.scene_execution
                    .as_mut()
                    .unwrap()
                    .active
                    .as_mut()
                    .unwrap()
                    .scene = "missing".into()
            }
            _ => data.story_identity = Some("changed story".into()),
        }
        assert!(game.load_data(data).is_err());
        assert_eq!(state_json(&game), state);
        assert_eq!(game.runtime_epoch(), epoch);
        assert_eq!(game.history.len(), history);
    }
}

#[test]
fn timers_park_with_their_scene_and_resume_without_child_elapsed_time() {
    let mut game = game("scene label P\ntimer 1 => jump done\n\"P\"\ncall A\n\"P after\"\njump end\nscene label A\n\"child\"\nreturn\nlabel done\n\"timer done\"\nlabel end\n");
    assert_eq!(dialogue(&mut game), "P");
    game.tick_scene_tasks(0.4).unwrap();
    let token = game.timer_token().unwrap();
    game.advance_dialogue().unwrap();
    assert_eq!(dialogue(&mut game), "child");
    game.tick_scene_tasks(5.0).unwrap();
    assert!(!game.fire_timer(&token).unwrap());
    game.advance_dialogue().unwrap();
    assert_eq!(dialogue(&mut game), "P after");
    game.tick_scene_tasks(0.1).unwrap();
    let elapsed = game
        .state
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
        .elapsed;
    assert!((elapsed - 0.5).abs() < 1e-9);
    assert_ne!(game.timer_token().unwrap().revision, token.revision);
    game.tick_scene_tasks(0.5).unwrap();
    assert_eq!(dialogue(&mut game), "timer done");
}

#[test]
fn rollback_renews_callbacks_without_reentering_or_initializing_the_scene() {
    let mut game = game("scene label A\nlocal n=1\ntimer 2 => jump done\n\"one [n]\"\nset n=2\n\"two [n]\"\nlabel done\n");
    assert_eq!(dialogue(&mut game), "one 1");
    let id = game.state.scene_execution.as_ref().unwrap().owner();
    let old_token = game.timer_token().unwrap();
    game.advance_dialogue().unwrap();
    assert_eq!(dialogue(&mut game), "two 2");
    assert!(game.rollback());
    assert!(game.rollback());
    assert_eq!(game.state.scene_execution.as_ref().unwrap().owner(), id);
    assert_eq!(game.get_var("n"), Some(&Value::Int(1)));
    assert!(!game.fire_timer(&old_token).unwrap());
    assert_eq!(
        game.displayed_interaction().unwrap(),
        Some(Interaction::Dialogue {
            character: None,
            text: "one 1".into()
        })
    );
}

#[test]
fn aggregate_bindings_include_parked_callers() {
    let mut game =
        game("scene label P\nlocal p=1\ncall A\nscene label A\nlocal a=2\n\"A\"\nreturn\n");
    dialogue(&mut game);
    let mut data = SaveData::from_state(&game.state, 1, "A".into(), "story.rvn".into());
    for index in 0..4095 {
        data.vars
            .insert(format!("global_{index}"), rvn_core::save::SaveValue::Int(0));
    }
    let before = state_json(&game);
    assert!(game.load_data(data).is_err());
    assert_eq!(state_json(&game), before);
}

#[test]
fn legacy_stories_keep_version_nine_and_their_existing_ast_identity() {
    let script = rvn_parser::parse("label start\nset value=3\n\"[value]\"\n").unwrap();
    let mut game = Engine::new(script, Host::default(), 16).unwrap();
    dialogue(&mut game);
    let data = SaveData::from_state(&game.state, 1, "old".into(), "story.rvn".into());
    assert_eq!(data.format_version, SAVE_FORMAT_VERSION);
    assert!(data.scene_execution.is_none());
    assert!(!serde_json::to_value(&data)
        .unwrap()
        .as_object()
        .unwrap()
        .contains_key("scene_execution"));
    assert_eq!(
        game.load_data(data).unwrap(),
        rvn_core::LoadCompatibility::Verified
    );
    assert!(game.state.scene_execution.is_none());
}

#[test]
fn event_host_publishes_direct_choice_checkpoint_without_reexecuting_or_mutating_state() {
    let mut game = game("init {set result=0}\nscene label P\n\"first\"\nchoice {\"A\"=>{set result=10} \"B\"=>{set result=20}}\n\"after\"\n");
    game.enter_label("P").unwrap();
    game.step_until_interaction().unwrap();
    game.renderer.batches.clear();
    game.advance_dialogue().unwrap();
    assert!(matches!(game.step_until_interaction().unwrap(), Some(Interaction::Choice { options }) if options == vec!["A", "B"]));
    assert!(game.renderer.batches.is_empty());
    let state = serde_json::to_vec(&game.state).unwrap();
    let history = format!("{:?}", game.history.entries());
    let identity = (game.instance_id(), game.runtime_epoch());
    let source = game.preview_input_source();
    game.publish_current_scene_interaction().unwrap();
    assert_eq!(game.renderer.batches.len(), 1);
    let batch = &game.renderer.batches[0];
    assert_eq!(serde_json::to_vec(&batch.state).unwrap(), state);
    assert!(matches!(&batch.display, Some(Interaction::Choice { options }) if options == &vec!["A", "B"]));
    assert_eq!(serde_json::to_vec(&game.state).unwrap(), state);
    assert_eq!(format!("{:?}", game.history.entries()), history);
    assert_eq!((game.instance_id(), game.runtime_epoch()), identity);
    assert_eq!(game.preview_input_source(), source);
    game.submit_choice(1).unwrap();
    game.step_until_interaction().unwrap();
    assert_eq!(game.state.vars["result"], Value::Int(20));
}

#[test]
fn event_host_refuses_unexecuted_or_rejected_scene_interaction_without_publishing_or_consuming_input() {
    let mut game = game("scene label P\n\"first\"\nchoice {\"A\"=>{} \"B\"=>{}}\n");
    game.enter_label("P").unwrap();
    game.renderer.batches.clear();
    let unexecuted = serde_json::to_vec(&game.state).unwrap();
    assert!(game.publish_current_scene_interaction().is_err());
    assert!(game.renderer.batches.is_empty());
    assert_eq!(serde_json::to_vec(&game.state).unwrap(), unexecuted);
    game.step_until_interaction().unwrap();
    game.advance_dialogue().unwrap();
    game.step_until_interaction().unwrap();
    game.renderer.batches.clear();
    game.renderer.reject = true;
    let state = serde_json::to_vec(&game.state).unwrap();
    let history = format!("{:?}", game.history.entries());
    let source = game.preview_input_source();
    let epoch = game.runtime_epoch();
    assert!(game.publish_current_scene_interaction().unwrap_err().to_string().contains("Host cannot prepare"));
    assert!(game.renderer.batches.is_empty());
    assert_eq!(serde_json::to_vec(&game.state).unwrap(), state);
    assert_eq!(format!("{:?}", game.history.entries()), history);
    assert_eq!(game.preview_input_source(), source);
    assert_eq!(game.runtime_epoch(), epoch);
    game.renderer.reject = false;
    game.publish_current_scene_interaction().unwrap();
    assert_eq!(game.renderer.batches.len(), 1);
    assert_eq!(serde_json::to_vec(&game.state).unwrap(), state);
}

// Persistence contracts using the existing coherent narrative Host fixture.
// These tests qualify seeded initialization and preserve actual authored semantics.
#[test]
fn persistent_core_0030_seed_is_available_to_actual_init_in_both_program_contexts() {
    for declaration in ["label start", "scene label start"] {
        let program = rvn_parser::parse_program(&format!(
            "init{{set observed_boot=persistent.counter}}\n{declaration}\n\"Ready.\"\n"
        )).unwrap();
        let seed = [("persistent.counter".into(), Value::Int(7))].into_iter().collect();
        let game = Engine::from_program_with_persistent(program, Host::default(), 32, seed).unwrap();
        assert_eq!(game.state.vars["observed_boot"], Value::Int(7));
        assert_eq!(game.get_persistent_var("persistent.counter"), Some(&Value::Int(7)));
    }
}

#[test]
fn persistent_core_0030_real_fresh_preserves_progression_before_init_and_resets_game_globals() {
    let program = rvn_parser::parse_program("init{set observed_boot=persistent.counter set game_score=3}\nlabel start\nset game_score=12\nset persistent.counter=9\n\"Ready.\"\n").unwrap();
    let seed = [("persistent.counter".into(), Value::Int(7))].into_iter().collect();
    let mut game = Engine::from_program_with_persistent(program, Host::default(), 32, seed).unwrap();
    assert!(matches!(game.step_until_interaction().unwrap(), Some(Interaction::Dialogue{..})));
    assert_eq!(game.state.vars["game_score"], Value::Int(12));
    let fresh = game.fresh(Host::default(), 32).unwrap();
    assert_ne!(fresh.instance_id(), game.instance_id());
    assert_eq!(fresh.state.vars["observed_boot"], Value::Int(9));
    assert_eq!(fresh.state.vars["game_score"], Value::Int(3));
    assert_eq!(fresh.get_persistent_var("persistent.counter"), Some(&Value::Int(9)));
    assert_eq!(game.get_persistent_var("persistent.counter"), Some(&Value::Int(9)));
}

#[test]
fn persistent_core_0030_explicit_authored_init_assignment_is_not_a_hidden_default() {
    let program = rvn_parser::parse_program("init{set persistent.counter=5 set observed_boot=persistent.counter}\nlabel start\n\"Ready.\"\n").unwrap();
    let seed = [("persistent.counter".into(), Value::Int(9))].into_iter().collect();
    let game = Engine::from_program_with_persistent(program, Host::default(), 32, seed).unwrap();
    assert_eq!(game.state.vars["observed_boot"], Value::Int(5));
    assert_eq!(game.get_persistent_var("persistent.counter"), Some(&Value::Int(5)));
    let fresh = game.fresh(Host::default(), 32).unwrap();
    assert_eq!(fresh.get_persistent_var("persistent.counter"), Some(&Value::Int(5)));
}

#[test]
fn persistent_core_0030_actual_save_load_restores_game_without_rewinding_progression() {
    let program = rvn_parser::parse_program("init{set game_score=3}\nlabel start\n\"Before.\"\nset game_score=12\nset persistent.counter=9\n\"After.\"\n").unwrap();
    let seed = [("persistent.counter".into(), Value::Int(7))].into_iter().collect();
    let mut game = Engine::from_program_with_persistent(program, Host::default(), 32, seed).unwrap();
    assert!(matches!(game.step_until_interaction().unwrap(), Some(Interaction::Dialogue{..})));
    let snapshot = game.snapshot_for_save().unwrap();
    let saved = rvn_core::save::SaveData::try_from_snapshot(&snapshot,1,"Before".into(),"program.rvn".into()).unwrap();
    let bytes = serde_json::to_vec(&saved).unwrap();
    game.advance_dialogue().unwrap();
    assert!(matches!(game.step_until_interaction().unwrap(), Some(Interaction::Dialogue{..})));
    assert_eq!(game.state.vars["game_score"], Value::Int(12));
    assert_eq!(game.get_persistent_var("persistent.counter"), Some(&Value::Int(9)));
    game.load_data(serde_json::from_slice(&bytes).unwrap()).unwrap();
    assert_eq!(game.state.vars["game_score"], Value::Int(3));
    assert_eq!(game.get_persistent_var("persistent.counter"), Some(&Value::Int(9)));
    assert_eq!(serde_json::to_vec(&saved).unwrap(), bytes);
}

// After-only tests for debugger bootstrap and actual narrative rollback.
#[test]
fn persistent_core_0030_debugger_bootstrap_seeds_before_actual_init() {
    let program = rvn_parser::parse_program("init{set observed_boot=persistent.counter}\nscene label start\n\"Ready.\"\n").unwrap();
    let seed = [("persistent.counter".into(), Value::Int(7))].into_iter().collect();
    let game = Engine::from_program_with_debugger_and_persistent(program, Host::default(), 32, seed).unwrap();
    assert_eq!(game.state.vars["observed_boot"], Value::Int(7));
    assert_eq!(game.get_persistent_var("persistent.counter"), Some(&Value::Int(7)));
    assert!(game.debug_site_page(0).unwrap().total > 0);
}

#[test]
fn persistent_core_0030_actual_narrative_rollback_keeps_current_progression() {
    let program = rvn_parser::parse_program("init{set game_score=3}\nscene label start\n\"Before.\"\nset game_score=12\nset persistent.counter=9\n\"After.\"\n").unwrap();
    let seed = [("persistent.counter".into(), Value::Int(7))].into_iter().collect();
    let mut game = Engine::from_program_with_persistent(program, Host::default(), 32, seed).unwrap();
    game.enter_label("start").unwrap();
    assert!(matches!(game.step_until_interaction().unwrap(), Some(Interaction::Dialogue{text,..}) if text == "Before."));
    game.advance_dialogue().unwrap();
    assert!(matches!(game.step_until_interaction().unwrap(), Some(Interaction::Dialogue{text,..}) if text == "After."));
    assert_eq!(game.state.vars["game_score"], Value::Int(12));
    assert_eq!(game.get_persistent_var("persistent.counter"), Some(&Value::Int(9)));
    let (source, _) = game.preview_game_audio_rollback().unwrap().unwrap();
    assert!(game.rollback_from_audio_source(&source).unwrap());
    assert_eq!(game.state.vars["game_score"], Value::Int(3));
    assert_eq!(game.get_persistent_var("persistent.counter"), Some(&Value::Int(9)));
    assert!(matches!(game.displayed_interaction().unwrap(), Some(Interaction::Dialogue{text,..}) if text == "Before."));
    assert!(game.preview_input_source().is_some());
}


#[test]
fn exact_unicode_variables_execute_functions_iterators_persistent_and_scene_save_reload() {
    let source="function reward(`Limite été`) { local `Total été`=0 for `Élément été` in [1,2] { set `Total été`=`Total été`+`Élément été` } flow.for_loop(`Index été`,1,2) { set `Total été`=`Total été`+`Index été` } return `Total été`+`Limite été` }\ninit { set `Confiance été`=1 }\nscene label start\nlocal `Visite été`=50\nset `Résultat été`=reward(3)\nset persistent.`Été fini`=`Résultat été`\n\"[`Confiance été`]/[`Visite été`]/[`Résultat été`]/[persistent.`Été fini`]\"\n\"[`Confiance été`]/[`Visite été`]/[`Résultat été`]/[persistent.`Été fini`]\"\nreturn\n";
    let mut engine=game(source);assert_eq!(dialogue(&mut engine),"1/50/9/9");
    assert_eq!(engine.state.vars["Confiance été"],Value::Int(1));
    assert_eq!(engine.state.vars["Résultat été"],Value::Int(9));
    assert!(!engine.state.vars.contains_key("Total été"));
    assert_eq!(engine.get_persistent_var("persistent.Été fini"),Some(&Value::Int(9)));
    let saved=SaveData::from_state(&engine.state,1,"Unicode".into(),"story.rvn".into());
    let saved=serde_json::from_slice::<SaveData>(&serde_json::to_vec(&saved).unwrap()).unwrap();
    let persistent=serde_json::from_slice::<std::collections::HashMap<String,Value>>(&serde_json::to_vec(engine.export_persistent_vars()).unwrap()).unwrap();
    let mut reopened=game(source);reopened.load_persistent_vars(&persistent);reopened.load_data(saved).unwrap();
    assert_eq!(reopened.state.scene_execution.as_ref().unwrap().active.as_ref().unwrap().locals["Visite été"],Value::Int(50));
    reopened.advance_dialogue().unwrap();assert_eq!(dialogue(&mut reopened),"1/50/9/9");
}

#[test]
fn distinct_unicode_forms_and_escaped_keyword_variables_have_independent_runtime_keys() {
    let source=r#"init {set `é`=1 set `é`=2 set Score=3 set score=4 set `true`=5 set `valeur]été`=6 set `prix\`été`=7 set `chemin\\été`=8}
label start
"[`é`]/[`é`]/[Score]/[score]/[`true`]/[`valeur]été`]/[`prix\`été`]/[`chemin\\\\été`]"
return
"#;
    let mut engine=game(source);assert_eq!(dialogue(&mut engine),"1/2/3/4/5/6/7/8");
    assert_eq!(engine.state.vars["é"],Value::Int(1));assert_eq!(engine.state.vars["e\u{301}"],Value::Int(2));
    assert_eq!(engine.state.vars.len(),8);
}
