use rvn_core::{
    locale::{LocaleManager, LocaleTable},
    string_table::{StringTableAsset, StringTableEntry, StringTableResources},
    Engine, Interaction, TerminalRenderer, TextRuntimeContext,
};
use rvn_parser::{localized_text::TextIdentity, Value};
use std::collections::HashMap;

fn context(language: &str) -> TextRuntimeContext {
    let mut resources = StringTableResources::default();
    resources
        .publish(StringTableAsset {
            id: "asset:boot".into(),
            namespace: "VN.Boot".into(),
            entries: [(
                "name".into(),
                StringTableEntry {metadata:Default::default(),
                    source: "Visitor".into(),
                    developer_notes: String::new(),
                },
            )]
            .into(),
        })
        .unwrap();
    let identity = TextIdentity {
        owner: "string-table:asset:boot".into(),
        namespace: "VN.Boot".into(),
        key: "name".into(),
    };
    let locale = LocaleManager::from_tables(
        "unused",
        "en",
        language,
        vec!["en".into(), "fr".into()],
        HashMap::from([(
            "fr".into(),
            LocaleTable {
                lang: "fr".into(),
                strings: HashMap::from([(identity.locale_key(), "Éloïse".into())]),
            },
        )]),
    );
    TextRuntimeContext::new(resources, Some(locale))
}
fn source(header: &str) -> String {
    format!(
        r#"init {{
set name=__rvn_string_table_text("asset:boot","name")
set converted=text_to_string(name)
set seen=persistent.counter
}}
{header}
"[converted] / [name] / [seen]"
"#
    )
}
fn progression() -> HashMap<String, Value> {
    HashMap::from([("persistent.counter".into(), Value::Int(7))])
}
fn assert_init(game: &Engine<TerminalRenderer>) {
    assert_eq!(game.state.vars["converted"], Value::Str("Éloïse".into()));
    assert!(matches!(game.state.vars["name"], Value::Text(_)));
    assert_eq!(game.state.vars["seen"], Value::Int(7));
    assert_eq!(game.locale.as_ref().unwrap().current_lang(), "fr");
}
fn assert_retained(mut game: Engine<TerminalRenderer>) {
    assert_init(&game);
    game.locale.as_mut().unwrap().set_language("en").unwrap();
    assert!(
        matches!(game.step_until_interaction().unwrap(),Some(Interaction::Dialogue{text,..})if text=="Éloïse / Visitor / 7")
    );
    assert_eq!(game.state.vars["converted"], Value::Str("Éloïse".into()));
}

#[test]
fn culture_and_resources_are_installed_before_script_and_parsed_program_initialization() {
    for header in ["label start", "scene label start"] {
        let story = source(header);
        assert_retained(
            Engine::new_with_persistent_and_text_context(
                rvn_parser::parse(&story).unwrap(),
                TerminalRenderer,
                8,
                progression(),
                context("fr"),
            )
            .unwrap(),
        );
        assert_retained(
            Engine::from_program_with_persistent_and_text_context(
                rvn_parser::parse_program(&story).unwrap(),
                TerminalRenderer,
                8,
                progression(),
                context("fr"),
            )
            .unwrap(),
        );
    }
}

#[test]
fn prepared_narrative_bootstrap_and_debugger_keep_authored_initialization_and_provenance() {
    let story = source("scene label start");
    let prepared = rvn_core::program::PreparedNarrative::from_parsed(
        rvn_parser::parse_program(&story).unwrap(),
    )
    .unwrap();
    assert_retained(
        Engine::from_narrative_with_persistent_and_text_context(
            prepared,
            TerminalRenderer,
            8,
            progression(),
            context("fr"),
        )
        .unwrap(),
    );
    let mut game = Engine::from_program_with_debugger_and_text_context(
        rvn_parser::parse_program(&story).unwrap(),
        TerminalRenderer,
        8,
        progression(),
        context("fr"),
    )
    .unwrap();
    assert_init(&game);
    game.enter_label("start").unwrap();
    let snapshot = game.debug_current_snapshot(&[]).unwrap();
    let origin = snapshot
        .origin
        .expect("real source map installed by debugger startup");
    let span = origin.span.expect("original source span");
    assert!(story.is_char_boundary(span.start) && story.is_char_boundary(span.end));
    assert!(!story[span].is_empty());
    game.locale.as_mut().unwrap().set_language("en").unwrap();
    assert!(
        matches!(game.step_until_interaction().unwrap(),Some(Interaction::Dialogue{text,..})if text=="Éloïse / Visitor / 7")
    );
}

#[test]
fn new_game_uses_current_culture_before_init_and_keeps_context_isolated() {
    for header in ["label start", "scene label start"] {
        let mut old = Engine::new_with_persistent_and_text_context(
            rvn_parser::parse(&source(header)).unwrap(),
            TerminalRenderer,
            8,
            progression(),
            context("en"),
        )
        .unwrap();
        assert_eq!(old.state.vars["converted"], Value::Str("Visitor".into()));
        old.locale.as_mut().unwrap().set_language("fr").unwrap();
        let mut fresh = old.fresh(TerminalRenderer, 8).unwrap();
        assert_init(&fresh);
        assert_eq!(
            fresh.get_persistent_var("persistent.counter"),
            Some(&Value::Int(7))
        );
        assert_eq!(
            fresh.text_resources().revision_stamp(),
            old.text_resources().revision_stamp()
        );
        fresh.locale.as_mut().unwrap().set_language("en").unwrap();
        fresh.text_resources_mut().unload("asset:boot").unwrap();
        assert_eq!(old.locale.as_ref().unwrap().current_lang(), "fr");
        assert_eq!(
            old.text_resources()
                .display(
                    &rvn_parser::localized_text::StringTableReference {
                        table: "asset:boot".into(),
                        key: "name".into()
                    },
                    old.locale.as_ref()
                )
                .unwrap(),
            "Éloïse"
        );
        assert_eq!(old.state.vars["converted"], Value::Str("Visitor".into()));
    }
}
