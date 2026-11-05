use rvn_core::locale::{LocaleManager, LocaleTable};
use rvn_parser::localized_text::{
    identity_for_edit, validate_text_identifier, TextIdentity, TextIdentityEdit,
    DeferredText, LocalizedText, TextArgument, TextLocalization, TextReference,
};
use std::collections::HashMap;

fn id(owner: &str, namespace: &str, key: &str) -> TextIdentity {
    TextIdentity { owner: owner.into(), namespace: namespace.into(), key: key.into() }
}

#[test]
fn identity_edits_preserve_single_source_and_fork_shared_source() {
    let original = id("asset:1", "VN.Test", "greeting");
    let single = [TextReference { identity: &original, source: "Bonjour" }];
    let changed = identity_for_edit(original.clone(), TextIdentityEdit::Source,
        "Bonjour", &single, || panic!("single reference must keep its key")).unwrap();
    assert_eq!(changed, original);
    let shared = [
        TextReference { identity: &original, source: "Bonjour" },
        TextReference { identity: &original, source: "Bonjour" },
    ];
    assert_eq!(identity_for_edit(original.clone(), TextIdentityEdit::Source,
        "Bonjour", &shared, || "fresh".into()).unwrap().key, "fresh");
}

#[test]
fn identifier_edits_allow_same_source_but_fork_a_conflict() {
    let proposed = id("asset:1", "VN.Test", "greeting");
    let same = [TextReference { identity: &proposed, source: "Bonjour" }];
    for action in [TextIdentityEdit::Namespace, TextIdentityEdit::Key] {
        assert_eq!(identity_for_edit(proposed.clone(), action, "Bonjour", &same,
            || panic!("same source is compatible")).unwrap(), proposed);
        let conflicting = [TextReference { identity: &proposed, source: "Autre" }];
        assert_eq!(identity_for_edit(proposed.clone(), action, "Bonjour", &conflicting,
            || "fresh".into()).unwrap().key, "fresh");
    }
}

#[test]
fn empty_key_and_enable_allocate_fresh_identity_without_reusing_old_keys() {
    let old = id("asset:1", "", "old");
    let references = [TextReference { identity: &old, source: "Bonjour" }];
    for action in [TextIdentityEdit::Enable, TextIdentityEdit::Key] {
        let mut allocated = ["old", "new"].into_iter();
        let proposed = if action == TextIdentityEdit::Key { id("asset:1", "", "") } else { old.clone() };
        assert_eq!(identity_for_edit(proposed, action, "Bonjour", &references,
            || allocated.next().unwrap().into()).unwrap(), id("asset:1", "", "new"));
    }
    assert!(identity_for_edit(old.clone(), TextIdentityEdit::Enable, "Bonjour", &references,
        || "old".into()).is_err());
}

#[test]
fn identity_validation_and_serialization_preserve_unicode_case_and_notes() {
    for bad in ["[bad]", "a b", "a,b", "a\nb", "a\rb", "a\tb", "a'b", "a\"b"] {
        assert!(validate_text_identifier(bad).is_err());
    }
    for good in ["", "VN.Test", "été.漢字", "Key", "key", "a/b:c"] {
        assert!(validate_text_identifier(good).is_ok());
    }
    assert!(id("", "VN", "key").validate().is_err());
    assert!(id("asset:1", "VN", "").validate().is_err());
    let text = TextLocalization::Localizable {
        identity: id("asset:1", "été.漢字", "Greeting"),
        developer_notes: "Salutation — contexte narratif".into(),
    };
    let encoded = serde_json::to_string(&text).unwrap();
    assert_eq!(serde_json::from_str::<TextLocalization>(&encoded).unwrap(), text);
    assert_eq!(TextLocalization::CultureInvariant.identity(), None);
}

#[test]
fn locale_keys_do_not_alias_different_identity_tuples() {
    let identities = [id("a:b", "c", "d"), id("a", "b:c", "d"),
        id("a", "b", "c:d"), id("a", "b", "C:d"), id("a", "é", "漢字")];
    let keys: std::collections::HashSet<_> = identities.iter().map(TextIdentity::locale_key).collect();
    assert_eq!(keys.len(), identities.len());
}

#[test]
fn explicit_text_resolves_current_fallback_then_source_and_invariant_bypasses_locale() {
    let identity = id("asset:1", "VN.Test", "greeting");
    let metadata = TextLocalization::Localizable { identity: identity.clone(), developer_notes: String::new() };
    let mut manager = LocaleManager::from_tables("unused-locales", "fr", "en",
        vec!["fr".into(), "en".into(), "de".into()], HashMap::from([
        ("en".into(), LocaleTable { lang: "en".into(), strings: HashMap::from([
            (identity.locale_key(), "Hello".into()), ("Bonjour".into(), "Legacy hello".into()),
        ]) }),
        ("fr".into(), LocaleTable { lang: "fr".into(), strings: HashMap::from([
            (identity.locale_key(), "Salut".into()),
        ]) }),
    ]));
    assert_eq!(manager.resolve_text("Bonjour", Some(&metadata)), "Hello");
    assert_eq!(manager.resolve_text("Source modifiée", Some(&metadata)), "Hello");
    assert_eq!(manager.resolve_text("Bonjour", None), "Legacy hello");
    assert_eq!(manager.resolve_text("Bonjour", Some(&TextLocalization::CultureInvariant)), "Bonjour");
    for missing in [id("asset:2", "VN.Test", "greeting"), id("asset:1", "Other", "greeting")] {
        let other = TextLocalization::Localizable { identity: missing, developer_notes: String::new() };
        assert_eq!(manager.resolve_text("Bonjour", Some(&other)), "Bonjour");
    }
    manager.set_language("de").unwrap();
    assert_eq!(manager.resolve_text("Bonjour", Some(&metadata)), "Salut");
    manager.set_language("fr").unwrap();
    assert_eq!(manager.resolve_text("Bonjour", Some(&metadata)), "Salut");
}

#[test]
fn identity_translation_survives_the_existing_toml_table_round_trip() {
    let identity = id("asset:1", "VN.Test", "greeting");
    let table = LocaleTable { lang: "en".into(), strings: HashMap::from([
        (identity.locale_key(), "Hello [name]".into()), ("Ancien texte".into(), "Old text".into()),
    ]) };
    let reloaded = LocaleTable::parse("en", &table.to_toml()).unwrap();
    assert_eq!(reloaded.strings, table.strings);
}

#[test]
fn native_edit_sequence_preserves_notes_and_undo_snapshots_without_cached_translation() {
    let original = LocalizedText { source: "Bonjour".into(),
        localization: TextLocalization::Localizable {
            identity: id("asset:1", "VN.Test", "greeting"), developer_notes: "Greeting context".into(),
        } };
    let mut text = original.clone();
    let original_id = original.localization.identity().unwrap();
    let references = [TextReference { identity: original_id, source: &original.source }];
    assert!(!text.edit_source("Bonjour", "asset:1", &references, || panic!("no-op")).unwrap());
    assert!(text.edit_source("Salut", "asset:1", &references, || panic!("single reference")).unwrap());
    assert_eq!(text.localization, original.localization);
    let before_invalid = text.clone();
    assert!(text.edit_identifier(TextIdentityEdit::Namespace, "[bad]", &references,
        || panic!("validation precedes allocation")).is_err());
    assert_eq!(text, before_invalid);
    assert!(!text.edit_identifier(TextIdentityEdit::Key, "greeting", &references,
        || panic!("no-op")).unwrap());
    let undo_snapshot = text.clone();
    assert!(text.set_localizable(false, "asset:1", &references, || panic!("disable")).unwrap());
    assert_eq!(text.source, "Salut");
    assert_eq!(text.localization, TextLocalization::CultureInvariant);
    assert!(text.set_localizable(true, "asset:1", &references, || "new-key".into()).unwrap());
    assert_eq!(text.localization, TextLocalization::Localizable {
        identity: id("asset:1", "", "new-key"), developer_notes: String::new(),
    });
    let restored: LocalizedText = serde_json::from_str(&serde_json::to_string(&undo_snapshot).unwrap()).unwrap();
    assert_eq!(restored, before_invalid);
    assert_eq!(restored.localization.identity().unwrap(), original_id);
}

#[test]
fn clearing_source_removes_identity_and_the_next_nonempty_source_gets_a_fresh_key() {
    let mut text = LocalizedText { source: "Bonjour".into(),
        localization: TextLocalization::Localizable {
            identity: id("asset:1", "VN.Test", "greeting"), developer_notes: "context".into(),
        } };
    assert!(text.edit_source("", "asset:1", &[], || panic!("empty source does not allocate")).unwrap());
    assert_eq!(text.localization, TextLocalization::Empty);
    assert!(!text.set_localizable(true, "asset:1", &[], || panic!("empty text")).unwrap());
    assert!(text.edit_source("Salut", "asset:1", &[], || "fresh".into()).unwrap());
    assert_eq!(text.localization, TextLocalization::Localizable {
        identity: id("asset:1", "", "fresh"), developer_notes: String::new(),
    });
}

fn literal(identity: TextIdentity, source: &str) -> DeferredText {
    DeferredText::Literal { text: LocalizedText { source: source.into(),
        localization: TextLocalization::Localizable { identity, developer_notes: String::new() } } }
}

#[test]
fn deferred_format_translates_pattern_and_nested_text_before_substitution_and_after_reload() {
    let pattern_id = id("asset:1", "VN.Test", "welcome");
    let name_id = id("asset:2", "Names", "visitor");
    let text = DeferredText::Format {
        pattern: Box::new(literal(pattern_id.clone(), "Hello {Name}!")),
        arguments: std::collections::BTreeMap::from([
            ("Name".into(), TextArgument::Text(Box::new(literal(name_id.clone(), "Visitor")))),
        ]),
    };
    let mut locale = LocaleManager::from_tables("unused", "en", "en", vec!["en".into(), "fr".into()],
        HashMap::from([
            ("en".into(), LocaleTable { lang: "en".into(), strings: HashMap::from([
                (pattern_id.locale_key(), "Hello {Name}!".into()), (name_id.locale_key(), "Visitor".into()),
            ]) }),
            ("fr".into(), LocaleTable { lang: "fr".into(), strings: HashMap::from([
                (pattern_id.locale_key(), "Bienvenue, {Name}.".into()), (name_id.locale_key(), "Éloïse".into()),
            ]) }),
        ]));
    let saved = serde_json::to_string(&text).unwrap();
    assert_eq!(rvn_core::localized_text::render_text(&text, Some(&locale), &rvn_core::localized_text::VnTextScalars).unwrap(), "Hello Visitor!");
    locale.set_language("fr").unwrap();
    let restored: DeferredText = serde_json::from_str(&saved).unwrap();
    assert_eq!(rvn_core::localized_text::render_text(&restored, Some(&locale), &rvn_core::localized_text::VnTextScalars).unwrap(), "Bienvenue, Éloïse.");
    assert_eq!(serde_json::to_string(&text).unwrap(), saved);
    assert!(!saved.contains("Bienvenue"));
    assert_eq!(rvn_core::localized_text::render_text(&restored, None, &rvn_core::localized_text::VnTextScalars).unwrap(), "Hello Visitor!");
}

#[test]
fn deferred_format_keeps_case_whitespace_escape_and_argument_contents_literal() {
    let text = DeferredText::Format {
        pattern: Box::new(DeferredText::invariant("`{literal`} `` `x {Name} {name} { Name} {missing} {} trailing`")),
        arguments: std::collections::BTreeMap::from([
            ("Name".into(), TextArgument::String("[random(1, 9)] {Name}".into())),
            ("name".into(), TextArgument::String("lower".into())),
            (" Name".into(), TextArgument::String("spaced".into())),
        ]),
    };
    assert_eq!(rvn_core::localized_text::render_text(&text, None, &rvn_core::localized_text::VnTextScalars).unwrap(),
        "{literal} ` `x [random(1, 9)] {Name} lower spaced {missing} {} trailing`");
}

#[test]
fn deferred_format_supports_the_vn_scalar_contract_without_claiming_numeric_culture_parity() {
    let text = DeferredText::Format {
        pattern: Box::new(DeferredText::invariant("{integer}/{float}/{boolean}")),
        arguments: std::collections::BTreeMap::from([
            ("integer".into(), TextArgument::Int(1234)),
            ("float".into(), TextArgument::Float(1.5)),
            ("boolean".into(), TextArgument::Bool(true)),
        ]),
    };
    assert_eq!(rvn_core::localized_text::render_text(&text, None, &rvn_core::localized_text::VnTextScalars).unwrap(), "1234/1.5/true");
    struct OtherScalars;
    impl rvn_core::localized_text::TextScalarFormatter for OtherScalars {
        fn integer(&self, _: i64) -> String { "1 234".into() }
        fn float(&self, _: f32) -> String { "1,5".into() }
        fn boolean(&self, _: bool) -> String { "oui".into() }
    }
    assert_eq!(rvn_core::localized_text::render_text(&text, None, &OtherScalars).unwrap(), "1 234/1,5/oui");
}

#[test]
fn deferred_format_rejects_nonfinite_values_excessive_depth_and_unqualified_modifiers() {
    let invalid = DeferredText::Format { pattern: Box::new(DeferredText::invariant("{n}")),
        arguments: std::collections::BTreeMap::from([("n".into(), TextArgument::Float(f32::NAN))]) };
    assert!(rvn_core::localized_text::render_text(&invalid, None, &rvn_core::localized_text::VnTextScalars).is_err());
    let mut deep = DeferredText::invariant("end");
    for _ in 0..65 {
        deep = DeferredText::Format { pattern: Box::new(deep), arguments: Default::default() };
    }
    assert!(rvn_core::localized_text::render_text(&deep, None, &rvn_core::localized_text::VnTextScalars).is_err());
    let modifier = DeferredText::Format { pattern: Box::new(DeferredText::invariant("{n}|plural(one=item,other=items)")),
        arguments: std::collections::BTreeMap::from([("n".into(), TextArgument::Int(2))]) };
    assert!(rvn_core::localized_text::render_text(&modifier, None, &rvn_core::localized_text::VnTextScalars).is_err());
    let huge = DeferredText::Format { pattern: Box::new(DeferredText::invariant("{value}")),
        arguments: std::collections::BTreeMap::from([("value".into(), TextArgument::String("x".repeat(rvn_core::value_limits::MAX_VALUE_BYTES + 1)))]) };
    assert!(rvn_core::localized_text::render_text(&huge, None, &rvn_core::localized_text::VnTextScalars).is_err());
}

fn runtime_locale() -> LocaleManager {
    LocaleManager::from_tables("unused", "en", "en", vec!["en".into(), "fr".into()], HashMap::from([
        ("en".into(), LocaleTable { lang: "en".into(), strings: HashMap::new() }),
        ("fr".into(), LocaleTable { lang: "fr".into(), strings: HashMap::from([
            (id("asset:1", "VN", "welcome").locale_key(), "Bonjour {Name}!".into()),
            (id("asset:2", "Names", "visitor").locale_key(), "Éloïse".into()),
        ]) }),
    ]))
}

const RUNTIME_TEXT: &str = r#"
set pattern = __rvn_localizable_text("Hello {Name}!", "asset:1", "VN", "welcome", "Greeting")
set person = __rvn_localizable_text("Visitor", "asset:2", "Names", "visitor")
set message = __rvn_format_text(pattern, {"Name": person})
"#;

fn expression(source: &str) -> rvn_parser::Expr {
    let parsed = rvn_parser::parse(&format!("set answer = {source}")).unwrap();
    let rvn_parser::Statement::SetVar { value, .. } = &parsed[0] else { panic!("parsed assignment"); };
    value.clone()
}

#[test]
fn rvn_constructors_capture_typed_arguments_and_convert_only_at_the_display_boundary() {
    use rvn_core::eval::FunctionLibrary;
    use rvn_parser::Value;
    let library = FunctionLibrary::default();
    let statements = rvn_parser::parse(RUNTIME_TEXT).unwrap();
    assert!(rvn_parser::validate_logic(&statements, false).is_empty());
    let mut random = rvn_core::random::RandomState::seeded(91);
    let before = random;
    let values = library.execute_with_random(&statements, &HashMap::new(), &mut random).unwrap();
    assert!(matches!(values["message"], Value::Text(_)));
    assert_eq!(random, before);
    let saved = serde_json::to_string(&values).unwrap();
    let mut locale = runtime_locale();
    let display = expression("text_to_string(message)");
    assert_eq!(library.eval_scoped_with_locale(&display, &values, &values, &mut random, Some(&locale)).unwrap(), Value::Str("Hello Visitor!".into()));
    locale.set_language("fr").unwrap();
    assert_eq!(library.eval_scoped_with_locale(&display, &values, &values, &mut random, Some(&locale)).unwrap(), Value::Str("Bonjour Éloïse!".into()));
    assert_eq!(serde_json::to_string(&values).unwrap(), saved);
    assert_eq!(library.eval(&expression("text_to_string(string_to_text(\"[n]\"))"), &values).unwrap(), Value::Str("[n]".into()));
    assert!(library.eval(&expression("__rvn_format_text(\"{x}\", {\"x\": [1]})"), &values).is_err());
    assert!(library.eval(&expression("message + \"!\""), &values).is_err());
    assert!(rvn_core::ui::value_to_json(&values["message"]).is_err());
    assert_eq!(random, before);
}

#[test]
fn typed_text_dialogue_save_reload_history_and_language_change_preserve_source_and_random() {
    use rvn_core::{Engine, Interaction, TerminalRenderer};
    use rvn_core::save::{SaveData, TEXT_SAVE_FORMAT_VERSION};
    for header in ["label start", "scene label start"] {
        let source = format!("{header}\n{RUNTIME_TEXT}\n\"[message]\"\n\"Second\"\n");
        let mut game = Engine::new(rvn_parser::parse(&source).unwrap(), TerminalRenderer, 8).unwrap();
        game.locale = Some(runtime_locale());
        assert!(matches!(game.step_until_interaction().unwrap(), Some(Interaction::Dialogue { text, .. }) if text == "Hello Visitor!"));
        let saved = SaveData::try_from_state(&game.state, 1, "text".into(), "text.rvn".into()).unwrap();
        assert_eq!(saved.format_version, TEXT_SAVE_FORMAT_VERSION);
        let bytes = serde_json::to_string(&saved).unwrap();
        assert!(bytes.contains("Hello {Name}!"));
        assert!(!bytes.contains("Bonjour"));
        let random = (game.state.random, game.state.display_random);
        game.locale.as_mut().unwrap().set_language("fr").unwrap();
        assert!(matches!(game.displayed_interaction().unwrap(), Some(Interaction::Dialogue { text, .. }) if text == "Bonjour Éloïse!"));
        game.advance_dialogue().unwrap();
        game.step_until_interaction().unwrap();
        assert!(game.localized_dialogue_history().unwrap().iter().any(|(_, text)| text == "Bonjour Éloïse!"));
        let restored: SaveData = serde_json::from_str(&bytes).unwrap();
        game.load_data(restored).unwrap();
        assert!(matches!(game.displayed_interaction().unwrap(), Some(Interaction::Dialogue { text, .. }) if text == "Bonjour Éloïse!"));
        assert_eq!((game.state.random, game.state.display_random), random);
        let before = serde_json::to_value(&game.state).unwrap();
        let mut downgraded: SaveData = serde_json::from_str(&bytes).unwrap();
        downgraded.format_version = 11;
        assert!(game.load_data(downgraded).is_err());
        assert_eq!(serde_json::to_value(&game.state).unwrap(), before);
    }
}

#[test]
fn save_text_gate_covers_nested_values_and_frozen_or_suspended_contexts_without_matching_dictionary_keys() {
    use rvn_core::{Engine, TerminalRenderer};
    use rvn_core::save::{SaveData, SAVE_FORMAT_VERSION, SCENE_SAVE_FORMAT_VERSION, TEXT_SAVE_FORMAT_VERSION};
    use rvn_parser::Value;
    let plain = Engine::new(rvn_parser::parse("label start\n\"Waiting\"").unwrap(), TerminalRenderer, 8).unwrap().state;
    let text = Value::Text(Box::new(literal(id("asset:1", "VN", "welcome"), "Hello")));
    let mut state = plain.clone();
    state.vars.insert("dictionary".into(), Value::Dict(std::collections::BTreeMap::from([("Text".into(), Value::Str("ordinary value".into()))])));
    assert_eq!(SaveData::from_state(&state, 1, "".into(), "".into()).format_version, SAVE_FORMAT_VERSION);
    state.vars.insert("nested".into(), Value::List(vec![text.clone()]));
    assert_eq!(SaveData::from_state(&state, 1, "".into(), "".into()).format_version, TEXT_SAVE_FORMAT_VERSION);
    let mut state = plain.clone();
    state.last_dialogue = Some(rvn_core::types::DialogueSnapshot { consumer_dialogue: None, pc: 1, vars: HashMap::new(), lexical_globals: Some(HashMap::from([("text".into(), text.clone())])), random: Default::default() });
    let mut data = SaveData::from_state(&state, 1, "".into(), "".into());
    assert_eq!(data.format_version, TEXT_SAVE_FORMAT_VERSION);
    data.format_version = 9;
    assert!(data.validate_checkpoint_shape().is_err());
    let mut game = Engine::new(rvn_parser::parse("scene label start\n\"Waiting\"").unwrap(), TerminalRenderer, 8).unwrap();
    game.step_until_interaction().unwrap();
    assert_eq!(SaveData::from_state(&game.state, 1, "".into(), "".into()).format_version, SCENE_SAVE_FORMAT_VERSION);
    for location in 0..3 {
        let mut state = game.state.clone();
        let scene = state.scene_execution.as_mut().unwrap();
        match location {
            0 => { scene.active.as_mut().unwrap().locals.insert("text".into(), text.clone()); },
            1 => { scene.display.as_mut().unwrap().variables.insert("text".into(), text.clone()); },
            _ => { scene.display.as_mut().unwrap().lexical_globals.insert("text".into(), text.clone()); },
        }
        let mut data = SaveData::from_state(&state, 1, "".into(), "".into());
        assert_eq!(data.format_version, TEXT_SAVE_FORMAT_VERSION);
        data.validate_checkpoint_shape().unwrap();
        data.format_version = 11;
        assert!(data.validate_checkpoint_shape().is_err());
    }
}

#[test]
fn text_storage_bounds_include_notes_enclosing_collections_and_unfinished_format_drafts() {
    use rvn_parser::Value;
    let text = Value::Text(Box::new(DeferredText::invariant("x".repeat(600_000))));
    assert!(rvn_core::value_limits::value_work(&text).is_ok());
    assert!(rvn_core::value_limits::value_work(&Value::List(vec![text.clone(), text])).is_err());
    let mut noted = literal(id("asset:1", "VN", "greeting"), "Hello");
    let DeferredText::Literal { text } = &mut noted else { unreachable!() };
    let TextLocalization::Localizable { developer_notes, .. } = &mut text.localization else { unreachable!() };
    *developer_notes = "x".repeat(1_048_576);
    assert!(noted.storage_work().is_err());
    let draft = "{".repeat(100_000);
    assert_eq!(rvn_parser::localized_text::substitute_named(&draft, &Default::default(), draft.len()).unwrap(), draft);
    assert!(rvn_parser::localized_text::substitute_named("{n}", &std::collections::BTreeMap::from([("n", "é".into())]), 1).is_err());
}

#[test]
fn generated_rvn_expression_restores_identity_notes_empty_text_and_argument_types_exactly() {
    use rvn_parser::Value;
    let mut localized = literal(id("asset:1", "VN", "greeting"), "Hello {Name}\n\"quoted\" \\ été");
    let DeferredText::Literal { text } = &mut localized else { unreachable!() };
    let TextLocalization::Localizable { developer_notes, .. } = &mut text.localization else { unreachable!() };
    *developer_notes = "Notes\t\"quoted\"".into();
    let empty = DeferredText::Literal { text: LocalizedText { source: String::new(), localization: TextLocalization::Empty } };
    let format = DeferredText::Format { pattern: Box::new(localized.clone()), arguments: std::collections::BTreeMap::from([
        ("Name".into(), TextArgument::Text(Box::new(literal(id("asset:2", "Names", "visitor"), "Visitor")))),
        ("empty".into(), TextArgument::Text(Box::new(empty.clone()))),
        ("float".into(), TextArgument::Float(1.0)), ("zero".into(), TextArgument::Float(-0.0)),
        ("int".into(), TextArgument::Int(i64::MAX)), ("minimum".into(), TextArgument::Int(i64::MIN)),
        ("largest".into(), TextArgument::Float(f32::MAX)), ("smallest".into(), TextArgument::Float(f32::MIN)),
        ("tiny".into(), TextArgument::Float(f32::MIN_POSITIVE)), ("bool".into(), TextArgument::Bool(false)),
        ("payload".into(), TextArgument::String("[random(1, 9)] {Name} `".into())),
    ]) };
    for text in [localized, empty, DeferredText::invariant("exact"), format] {
        let source = rvn_core::localized_text::source_expression(&text).unwrap();
        let library = rvn_core::eval::FunctionLibrary::default();
        let mut random = rvn_core::random::RandomState::seeded(12);
        let value = library.eval_with_random(&expression(&source), &HashMap::new(), &mut random).unwrap();
        assert_eq!(value, Value::Text(Box::new(text)), "{source}");
        assert_eq!(random, rvn_core::random::RandomState::seeded(12));
    }
}

#[test]
fn new_internal_text_builtins_do_not_reserve_existing_user_function_names() {
    let source = "function localizable_text(x) {return x}\nfunction invariant_text(x) {return x}\nfunction empty_text() {return 7}\nfunction format_text(x,y) {return x+y}\n";
    let script = rvn_parser::parse(source).unwrap();
    assert!(rvn_parser::validate_logic(&script, false).is_empty());
    let library = rvn_core::eval::FunctionLibrary::from_script(&script).unwrap();
    assert_eq!(library.eval(&expression("format_text(localizable_text(3), empty_text())"), &HashMap::new()).unwrap(), rvn_parser::Value::Int(10));
    assert_eq!(library.eval(&expression("invariant_text(\"old function\")"), &HashMap::new()).unwrap(), rvn_parser::Value::Str("old function".into()));
}
