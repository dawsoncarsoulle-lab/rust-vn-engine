use rvn_core::{random::RandomState, save::SaveData, Engine, Interaction, TerminalRenderer};
use rvn_parser::{parse, Value};

#[test]
fn saved_and_rolled_back_random_sequences_match_in_code_and_blueprints() {
    let source = parse("function roll() { return random(1, 1000000) }\nlabel start\n\"Before\"\nset a = roll()\n\"A [a] / [roll()]\"\nset b = roll()\n\"B [b]\"\n").unwrap();
    let compiled =
        rvn_graph::transpile_project(&rvn_graph::import_script(&source).unwrap()).unwrap();
    for script in [source, compiled.ast] {
        let mut game = Engine::new(script, TerminalRenderer, 16).unwrap();
        game.state.random = RandomState::seeded(100);
        game.step_until_interaction().unwrap();
        let before = SaveData::from_state(&game.state, 1, "start".into(), "random.rvn".into());
        game.advance_dialogue().unwrap();
        let first = game.step_until_interaction().unwrap();
        let Value::Int(a) = game.state.vars["a"] else {
            panic!()
        };
        assert_eq!(game.current_interaction().unwrap(), first);
        assert_eq!(game.current_interaction().unwrap(), first);
        let saved = SaveData::from_state(&game.state, 2, "start".into(), "random.rvn".into());
        game.advance_dialogue().unwrap();
        let second = game.step_until_interaction().unwrap();
        let b = game.state.vars["b"].clone();
        game.load_data(serde_json::from_str(&serde_json::to_string(&saved).unwrap()).unwrap())
            .unwrap();
        assert_eq!(game.current_interaction().unwrap(), first);
        game.advance_dialogue().unwrap();
        assert_eq!(game.step_until_interaction().unwrap(), second);
        assert_eq!(game.state.vars["b"], b);
        game.load_data(before).unwrap();
        game.advance_dialogue().unwrap();
        assert_eq!(game.step_until_interaction().unwrap(), first);
        assert_eq!(game.state.vars["a"], Value::Int(a));
        game.advance_dialogue().unwrap();
        assert!(matches!(
            game.step_until_interaction().unwrap(),
            Some(Interaction::Dialogue { .. })
        ));
        assert!(game.rollback());
        assert!(game.rollback());
        assert_eq!(game.current_interaction().unwrap(), first);
        game.advance_dialogue().unwrap();
        assert_eq!(game.step_until_interaction().unwrap(), second);
    }
}

#[test]
fn failed_computations_do_not_consume_the_random_stream() {
    let statements = parse("set result = random(0, 100) + 1 / 0").unwrap();
    let mut random = RandomState::seeded(17);
    let original = random;
    assert!(rvn_core::eval::FunctionLibrary::default()
        .execute_with_random(&statements, &Default::default(), &mut random)
        .is_err());
    assert_eq!(random, original);
}

#[test]
fn legacy_saves_receive_defaults_and_unknown_versions_are_rejected() {
    let game = Engine::new(
        parse("label start\n\"hello\"").unwrap(),
        TerminalRenderer,
        8,
    )
    .unwrap();
    let save = SaveData::from_state(&game.state, 1, "start".into(), "legacy.rvn".into());
    let mut json = serde_json::to_value(&save).unwrap();
    let object = json.as_object_mut().unwrap();
    for field in [
        "format_version",
        "random",
        "display_random",
        "display_random_pc",
    ] {
        object.remove(field);
    }
    let legacy: SaveData = serde_json::from_value(json.clone()).unwrap();
    assert_eq!(legacy.format_version, 1);
    assert_eq!(legacy.random, RandomState::default());
    for version in [0, rvn_core::save::SAVE_FORMAT_VERSION + 1, 9999] {
        json["format_version"] = version.into();
        assert!(serde_json::from_value::<SaveData>(json.clone()).is_err());
    }
}
