use rvn_core::{
    localized_text::{render_text, render_text_with_resources, VnTextScalars},
    string_table::{
        StringTableAsset, StringTableEntry, StringTableReference, StringTableResolution,
        StringTableResources, MISSING_STRING_TABLE_ENTRY,
    },
    Engine, Interaction, TerminalRenderer,
};
use rvn_parser::localized_text::{DeferredText, TextArgument};
use std::collections::BTreeMap;

fn reference(key: &str) -> DeferredText {
    DeferredText::StringTable {
        reference: StringTableReference {
            table: "asset:projection".into(),
            key: key.into(),
        },
    }
}
fn asset(source: &str) -> StringTableAsset {
    StringTableAsset {
        id: "asset:projection".into(),
        namespace: "VN".into(),
        entries: BTreeMap::from([(
            "key".into(),
            StringTableEntry {metadata:Default::default(),
                source: source.into(),
                developer_notes: "Notes".into(),
            },
        )]),
    }
}
fn project(text: &DeferredText, host: &StringTableResources) -> String {
    render_text_with_resources(text, None, &VnTextScalars, Some(host)).unwrap()
}

#[test]
fn unavailable_projection_keeps_structured_and_strict_diagnostics_and_recovers_same_reference() {
    let text = reference("key");
    let before = serde_json::to_value(&text).unwrap();
    let pair = StringTableReference {
        table: "asset:projection".into(),
        key: "key".into(),
    };
    let mut host = StringTableResources::default();
    assert_eq!(project(&text, &host), MISSING_STRING_TABLE_ENTRY);
    assert!(matches!(
        host.resolve(&pair).unwrap(),
        StringTableResolution::MissingTable
    ));
    assert!(host.display(&pair, None).unwrap_err().contains("missing"));
    let token = host.begin_load(&pair.table).unwrap();
    assert_eq!(project(&text, &host), MISSING_STRING_TABLE_ENTRY);
    assert!(matches!(
        host.resolve(&pair).unwrap(),
        StringTableResolution::Loading
    ));
    host.fail_load(&token, "Failed current load".into())
        .unwrap();
    assert_eq!(project(&text, &host), MISSING_STRING_TABLE_ENTRY);
    assert!(host
        .display(&pair, None)
        .unwrap_err()
        .contains("Failed current load"));
    host.publish(asset("Current")).unwrap();
    assert_eq!(project(&text, &host), "Current");
    assert_eq!(
        project(&reference("missing_key"), &host),
        MISSING_STRING_TABLE_ENTRY
    );
    host.unload(&pair.table).unwrap();
    assert_eq!(project(&text, &host), MISSING_STRING_TABLE_ENTRY);
    host.publish(asset("Replacement")).unwrap();
    assert_eq!(project(&text, &host), "Replacement");
    assert_eq!(serde_json::to_value(&text).unwrap(), before);
}

#[test]
fn explicit_context_invalid_reference_and_projection_budget_remain_contract_errors() {
    let host = StringTableResources::default();
    let text = reference("key");
    assert!(render_text(&text, None, &VnTextScalars).is_err());
    let invalid = DeferredText::StringTable {
        reference: StringTableReference {
            table: String::new(),
            key: "key".into(),
        },
    };
    assert!(render_text_with_resources(&invalid, None, &VnTextScalars, Some(&host)).is_err());
    let format = DeferredText::Format {
        pattern: Box::new(text),
        arguments: BTreeMap::from([("Unused".into(), TextArgument::String("x".repeat(1_048_576)))]),
    };
    assert!(render_text_with_resources(&format, None, &VnTextScalars, Some(&host)).is_err());
    let mut host = host;
    host.publish(asset(&"x".repeat(1_048_577))).unwrap();
    assert!(
        render_text_with_resources(&reference("key"), None, &VnTextScalars, Some(&host)).is_err()
    );
}

#[test]
fn save13_keeps_missing_reference_and_captured_placeholder_separate_from_current_loaded_asset() {
    for header in ["label start", "scene label start"] {
        let source = format!("init {{set value=__rvn_string_table_text(\"asset:projection\",\"key\") set converted=text_to_string(value)}}\n{header}\n\"[converted] | [value]\"\n");
        let mut game =
            Engine::new(rvn_parser::parse(&source).unwrap(), TerminalRenderer, 8).unwrap();
        assert!(
            matches!(game.step_until_interaction().unwrap(), Some(Interaction::Dialogue { text, .. }) if text == format!("{MISSING_STRING_TABLE_ENTRY} | {MISSING_STRING_TABLE_ENTRY}"))
        );
        let saved = rvn_core::save::SaveData::try_from_snapshot(
            &game.snapshot_for_save().unwrap(),
            1,
            "Missing reference".into(),
            "main.rvn".into(),
        )
        .unwrap();
        assert_eq!(saved.format_version, 13);
        assert_eq!(
            saved.vars["converted"],
            rvn_core::save::SaveValue::Str(MISSING_STRING_TABLE_ENTRY.into())
        );
        let mut restored =
            Engine::new(rvn_parser::parse(&source).unwrap(), TerminalRenderer, 8).unwrap();
        restored
            .text_resources_mut()
            .publish(asset("Loaded now"))
            .unwrap();
        restored.load_data(saved).unwrap();
        assert!(
            matches!(restored.displayed_interaction().unwrap(), Some(Interaction::Dialogue { text, .. }) if text == format!("{MISSING_STRING_TABLE_ENTRY} | Loaded now"))
        );
    }
}
