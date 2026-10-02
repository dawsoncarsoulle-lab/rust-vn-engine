use rvn_core::{save::SaveData, Engine, LoadCompatibility, TerminalRenderer};

fn game(source: &str) -> Engine<TerminalRenderer> {
    let mut game = Engine::new(rvn_parser::parse(source).unwrap(), TerminalRenderer, 16).unwrap();
    game.step_until_interaction().unwrap();
    game
}

#[test]
fn story_changes_reject_loading_without_overwriting_the_current_game() {
    let original = game("init { set score = 3 }\nlabel start\n\"Hello\"\nreturn");
    let saved = SaveData::from_state(&original.state, 1, "start".into(), "main.rvn".into());
    let mut changed = game("init { set score = 99 }\nlabel start\n\"New dialogue\"\nreturn");
    let before = serde_json::to_value(&changed.state).unwrap();
    assert!(changed
        .load_data(saved)
        .unwrap_err()
        .to_string()
        .contains("histoire"));
    assert_eq!(serde_json::to_value(&changed.state).unwrap(), before);
}

#[test]
fn comments_and_whitespace_do_not_invalidate_saves_and_old_saves_report_uncertainty() {
    let mut original = game("label start\n\"Hello\"\nreturn");
    let saved = SaveData::from_state(&original.state, 1, "start".into(), "main.rvn".into());
    let mut reformat =
        game("// different comment\nlabel start\n   \"Hello\" // same story\nreturn");
    assert_eq!(
        reformat.load_data(saved.clone()).unwrap(),
        LoadCompatibility::Verified
    );
    let mut legacy = serde_json::to_value(saved).unwrap();
    legacy.as_object_mut().unwrap().remove("story_identity");
    legacy.as_object_mut().unwrap().remove("format_version");
    assert_eq!(
        original
            .load_data(serde_json::from_value(legacy).unwrap())
            .unwrap(),
        LoadCompatibility::LegacyUnchecked
    );
    assert!(original.state.story_identity.is_some());
}

#[test]
fn invalid_saved_program_positions_and_state_are_atomic() {
    let mut original = game("init { set score = 3 }\nlabel start\n\"Score [score]\"\nreturn");
    let saved = SaveData::from_state(&original.state, 1, "start".into(), "main.rvn".into());
    let before = serde_json::to_value(&original.state).unwrap();
    for fault in 0..4 {
        let mut broken = saved.clone();
        match fault {
            0 => broken.pc = usize::MAX,
            1 => broken.call_stack = vec![usize::MAX],
            2 => broken.vars.clear(),
            _ => broken.format_version = 900,
        }
        assert!(original.load_data(broken).is_err());
        assert_eq!(serde_json::to_value(&original.state).unwrap(), before);
    }
}
