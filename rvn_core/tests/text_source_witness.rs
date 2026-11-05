use rvn_core::{
    locale::{LocaleCatalog, LocaleManager, LocaleTable},
    localized_text::{render_text_with_resources, VnTextScalars},
    string_table::{StringTableAsset, StringTableEntry, StringTableResources},
};
use rvn_parser::localized_text::{DeferredText, LocalizedText, StringTableReference, TextArgument, TextIdentity, TextLocalization};
use std::collections::{BTreeMap, HashMap};

fn identity() -> TextIdentity {
    TextIdentity { owner: "asset:été:漢字".into(), namespace: "VN.été".into(), key: "Greeting".into() }
}
fn catalog(lang: &str, id: &TextIdentity, source: &str, display: &str) -> LocaleCatalog {
    LocaleCatalog::with_text_sources(LocaleTable { lang: lang.into(), strings: HashMap::from([(id.locale_key(), display.into())]) },
        HashMap::from([(id.locale_key(), source.into())])).unwrap()
}
fn manager(id: &TextIdentity, current_source: &str, fallback_source: &str) -> LocaleManager {
    LocaleManager::from_catalogs("unused", "en", "fr", vec!["en".into(), "fr".into()], HashMap::from([
        ("fr".into(), catalog("fr", id, current_source, "Bonjour")),
        ("en".into(), catalog("en", id, fallback_source, "Fallback")),
    ]))
}
fn metadata(id: &TextIdentity) -> TextLocalization {
    TextLocalization::Localizable { identity: id.clone(), developer_notes: "Translator context".into() }
}

#[test]
fn guarded_resolution_checks_exact_source_and_retains_language_fallback_and_empty_source() {
    let id = identity(); let locale = manager(&id, "Hello", "New source"); let metadata = metadata(&id);
    assert_eq!(locale.resolve_text("Hello", Some(&metadata)), "Bonjour");
    assert_eq!(locale.resolve_text("New source", Some(&metadata)), "Fallback");
    assert_eq!(locale.resolve_text("new source", Some(&metadata)), "new source");
    assert_eq!(locale.resolve_text("Neither", Some(&metadata)), "Neither");
    assert_eq!(locale.resolve_text("", Some(&metadata)), "Bonjour");
    assert_eq!(locale.resolve_text("Hello", Some(&TextLocalization::CultureInvariant)), "Hello");
    assert_eq!(locale.resolve_text("", Some(&TextLocalization::Empty)), "");
    let legacy = LocaleManager::from_tables("unused", "en", "fr", vec!["en".into(), "fr".into()], HashMap::from([
        ("fr".into(), LocaleTable { lang: "fr".into(), strings: HashMap::from([(id.locale_key(), "Old catalog".into()), ("Hello".into(), "Legacy".into())]) }),
    ]));
    assert_eq!(legacy.resolve_text("Changed", Some(&metadata)), "Old catalog");
    assert_eq!(legacy.resolve_text("Hello", None), "Legacy");
    assert_eq!(locale.resolve_text("Hello", None), "Hello");
}

#[test]
fn canonical_catalog_round_trip_preserves_literal_escapes_unicode_and_control_sources() {
    let id = identity();
    for source in [r"literal\n\t\r\u0041", "actual\nline\ttab\rreturn", "quote\"slash\\ # = é漢字😀", "\u{0000}\u{0008}\u{000c}"] {
        let original = catalog("fr", &id, source, source);
        let encoded = original.to_toml();
        assert_eq!(LocaleCatalog::parse("fr", &encoded).unwrap(), original);
        let mut with_comment = encoded.clone();
        with_comment.push_str("\n# preserved values\n");
        assert_eq!(LocaleCatalog::parse("fr", &with_comment).unwrap(), original);
    }
    assert_eq!(LocaleTable::parse("en", "[strings]\n\"k\"=\"\\U0001F600\\u00e9\" # comment\n").unwrap().strings["k"], "😀é");
    for malformed in ["\\q", "\\uD800", "\\u12", "\\UFFFFFFFF"] {
        assert!(LocaleTable::parse("en", &format!("[strings]\n\"k\"=\"{malformed}\"\n")).is_err());
    }
}

#[test]
fn explicit_migration_is_atomic_requires_translation_and_never_rebases_an_existing_witness() {
    let id = identity(); let table = LocaleTable { lang: "fr".into(), strings: HashMap::from([(id.locale_key(), "Bonjour".into())]) };
    let mut catalog = LocaleCatalog::from(table);
    assert!(catalog.record_text_source_witness(&id, "Hello").unwrap());
    let before = catalog.clone();
    assert!(!catalog.record_text_source_witness(&id, "Hello").unwrap());
    assert!(catalog.record_text_source_witness(&id, "Changed").is_err());
    let mut missing = id.clone(); missing.key = "missing".into();
    assert!(catalog.record_text_source_witness(&missing, "Hello").is_err());
    assert!(catalog.record_text_source_witness(&id, &"X".repeat(1_048_577)).is_err());
    assert_eq!(catalog, before);
    for (key, source) in [("Legacy".into(), "Hello".into()), ("@rvn.text:999:bad".into(), "Hello".into()), (missing.locale_key(), "Hello".into())] {
        assert!(LocaleCatalog::with_text_sources(catalog.table().clone(), HashMap::from([(key, source)])).is_err());
    }
    let encoded = catalog.to_toml(); let duplicate = format!("{encoded}{} = \"Hello\"\n", serde_json::to_string(&id.locale_key()).unwrap());
    assert!(LocaleCatalog::parse("fr", &duplicate).is_err());
    assert!(LocaleCatalog::parse("fr", &format!("{encoded}\n[text_sources]\n")).is_err());
    assert_eq!(catalog, before);
}

#[test]
fn string_table_replacement_uses_current_source_witness_while_references_notes_and_format_remain_retained() {
    let id = TextIdentity { owner: "string-table:asset:table".into(), namespace: "VN".into(), key: "greeting".into() };
    let table = |source: &str| StringTableAsset { id: "asset:table".into(), namespace: "VN".into(), entries: BTreeMap::from([
        ("greeting".into(), StringTableEntry {metadata:Default::default(), source: source.into(), developer_notes: "Context".into() }),
    ]) };
    // Reviewed translations may contain named parameters, just like source.
    let locale = LocaleManager::from_catalogs("unused", "en", "fr", vec!["en".into(), "fr".into()], HashMap::from([
        ("fr".into(), catalog("fr", &id, "Hello {Name}!", "Bonjour {Name}!")),
        ("en".into(), catalog("en", &id, "Good evening {Name}.", "Evening {Name}.")),
    ]));
    let mut resources = StringTableResources::default(); resources.publish(table("Hello {Name}!")).unwrap();
    let text = DeferredText::Format { pattern: Box::new(DeferredText::StringTable { reference: StringTableReference { table: "asset:table".into(), key: "greeting".into() } }),
        arguments: BTreeMap::from([("Name".into(), TextArgument::String("Éloïse".into()))]) };
    let encoded = serde_json::to_string(&text).unwrap();
    let display = |resources: &StringTableResources, locale: &LocaleManager| render_text_with_resources(&text, Some(locale), &VnTextScalars, Some(resources)).unwrap();
    assert_eq!(display(&resources, &locale), "Bonjour Éloïse!");
    let old = resources.clone(); resources.publish(table("Good evening {Name}.")).unwrap();
    assert_eq!(display(&resources, &locale), "Evening Éloïse.");
    resources.publish(table("New {Name}.")).unwrap();
    assert_eq!(display(&resources, &locale), "New Éloïse.");
    assert_eq!(display(&old, &locale), "Bonjour Éloïse!");
    assert_eq!(serde_json::to_string(&text).unwrap(), encoded);
    let literal = DeferredText::Literal { text: LocalizedText { source: "New {Name}.".into(), localization: metadata(&id) } };
    assert_eq!(render_text_with_resources(&literal, Some(&locale), &VnTextScalars, None).unwrap(), "New {Name}.");
}

struct OwnedLocale(std::path::PathBuf);
impl OwnedLocale {
    fn new() -> Self {
        let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("rvn-locale-witness-0110-{}-{nonce}", std::process::id()));
        std::fs::create_dir(&path).unwrap(); Self(path)
    }
}
impl Drop for OwnedLocale {
    fn drop(&mut self) {
        let Ok(root) = self.0.canonicalize() else { return; };
        let Ok(temp) = std::env::temp_dir().canonicalize() else { return; };
        if root.parent() == Some(temp.as_path()) && root.file_name().is_some_and(|name| name.to_string_lossy().starts_with(&format!("rvn-locale-witness-0110-{}-", std::process::id()))) {
            let _ = std::fs::remove_dir_all(root);
        }
    }
}

#[test]
fn native_reload_and_default_collection_preserve_witnesses_and_isolate_existing_snapshots() {
    let directory = OwnedLocale::new(); let id = identity(); let metadata = metadata(&id);
    let original = catalog("en", &id, "Hello", "English");
    std::fs::write(directory.0.join("en.toml"), original.to_toml()).unwrap();
    std::fs::write(directory.0.join("fr.toml"), catalog("fr", &id, "Hello", "Bonjour").to_toml()).unwrap();
    let mut locale = LocaleManager::new(&directory.0, "en", "fr", vec!["en".into(), "fr".into()]).unwrap();
    assert_eq!(locale.resolve_text("Hello", Some(&metadata)), "Bonjour");
    assert!(locale.update_default_locale(&["Legacy added".into()]).unwrap());
    let updated = LocaleCatalog::load("en", &directory.0.join("en.toml")).unwrap();
    assert_eq!(updated.text_sources(), original.text_sources());
    assert_eq!(updated.table().strings["Legacy added"], "Legacy added");
    let snapshot = locale.clone();
    std::fs::write(directory.0.join("fr.toml"), catalog("fr", &id, "Changed", "Révisé").to_toml()).unwrap();
    locale.reload_current();
    assert_eq!(locale.resolve_text("Hello", Some(&metadata)), "English");
    assert_eq!(locale.resolve_text("Changed", Some(&metadata)), "Révisé");
    assert_eq!(snapshot.resolve_text("Hello", Some(&metadata)), "Bonjour");
    assert!(!locale.update_default_locale(&["Legacy added".into()]).unwrap());
}

#[test]
fn preloaded_catalog_switches_keep_source_witnesses_without_disk_or_implicit_migration() {
    let id = identity(); let metadata = metadata(&id); let mut locale = manager(&id, "Hello", "New source");
    let before = locale.clone(); locale.set_language("en").unwrap();
    assert_eq!(locale.resolve_text("Hello", Some(&metadata)), "Hello");
    assert_eq!(locale.resolve_text("New source", Some(&metadata)), "Fallback");
    locale.set_language("fr").unwrap(); locale.reload_current();
    assert_eq!(locale, before);
}

#[test]
fn tiny_translations_cannot_hide_repeated_source_witness_comparison_work_or_commit_random_on_failure() {
    use rvn_core::{eval::{EvalError, FunctionLibrary}, random::RandomState};
    use rvn_parser::{Statement, Value};
    let source = "X".repeat(300_000);
    let id = TextIdentity { owner: "string-table:asset:large".into(), namespace: "VN".into(), key: "entry".into() };
    let locale = LocaleManager::from_catalogs("unused", "en", "fr", vec!["en".into(), "fr".into()], HashMap::from([
        ("fr".into(), catalog("fr", &id, &source, "B")),
    ]));
    let script = rvn_parser::parse(r#"function run() {
local draw=random(1,9)
local count=0
while count<110 {
    local converted=text_to_string(__rvn_string_table_text("asset:large","entry"))
    set count=count+1
}
return count
}
set result=run()
"#).unwrap();
    let Statement::SetVar { value: expression, .. } = &script[1] else { panic!("call") };
    let mut library = FunctionLibrary::from_script(&script).unwrap();
    library.text_resources_mut().publish(StringTableAsset { id: "asset:large".into(), namespace: "VN".into(), entries: BTreeMap::from([
        ("entry".into(), StringTableEntry {metadata:Default::default(), source: source.clone(), developer_notes: String::new() }),
    ]) }).unwrap();
    let mut random = RandomState::seeded(16);
    assert!(matches!(library.eval_scoped_with_locale(expression, &HashMap::new(), &HashMap::new(), &mut random, Some(&locale)), Err(EvalError::ExecutionLimit { limit }) if limit.starts_with("1 000 000")));
    assert_eq!(random, RandomState::seeded(16));
    let legacy = LocaleManager::from_tables("unused", "en", "fr", vec!["en".into(), "fr".into()], HashMap::from([
        ("fr".into(), LocaleTable { lang: "fr".into(), strings: HashMap::from([(id.locale_key(), "B".into())]) }),
    ]));
    assert_eq!(library.eval_scoped_with_locale(expression, &HashMap::new(), &HashMap::new(), &mut random, Some(&legacy)).unwrap(), Value::Int(110));
}
