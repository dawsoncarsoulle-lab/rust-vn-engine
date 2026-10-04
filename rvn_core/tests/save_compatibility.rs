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

#[test]
fn legacy_ui_open_saves_keep_their_story_identity_after_narrative_ownership_is_added() {
    let source = r#"
screen hud(){return component("root","panel",{},[])}
handler open_hud(event){ui.open("hud",[],false,0)}
label start
"Hello"
return
"#;
    let mut original = game(source);
    let mut legacy_ast = serde_json::to_value(&original.script).unwrap();
    fn legacy_open_schema(value: &mut serde_json::Value) {
        match value {
            serde_json::Value::Object(object) => {
                if let Some(serde_json::Value::Object(open)) = object.get_mut("UiOpen") {
                    open.remove("story"); // The pre-extension statement has exactly four operands.
                    assert_eq!(open.len(), 4);
                }
                for nested in object.values_mut() { legacy_open_schema(nested); }
            }
            serde_json::Value::Array(values) => {
                for nested in values { legacy_open_schema(nested); }
            }
            _ => {}
        }
    }
    legacy_open_schema(&mut legacy_ast);
    // Value orders object keys, so decode the historical schema back to the
    // AST before using the existing canonical statement field ordering.
    let legacy_script: rvn_parser::Script = serde_json::from_value(legacy_ast).unwrap();
    let bytes = serde_json::to_vec(&legacy_script).unwrap();
    assert!(!String::from_utf8_lossy(&bytes).contains("\"story\":false"));
    let hash = bytes.iter().fold(0xcbf29ce484222325_u64, |hash, byte|
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3));
    let legacy_identity = format!("rvn-prepared-2:{hash:016x}:{}", bytes.len());
    let mut saved = SaveData::from_state(&original.state, 1, "start".into(), "main.rvn".into());
    saved.story_identity = Some(legacy_identity);
    assert_eq!(original.load_data(saved.clone()).unwrap(), LoadCompatibility::Verified);
    let mut changed = game(&source.replace("ui.open(", "ui.open_story("));
    assert!(changed.load_data(saved).is_err());
}
