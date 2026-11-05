use super::*;
use crate::{
    bevy_renderer::BevyRenderer, string_table_assets::StringTableAssetsPlugin,
    vn_command::VnCommand,
};
use rvn_core::{
    locale::LocaleTable,
    string_table::{StringTableAsset, StringTableEntry, StringTableResources},
    string_table_file::StringTableFile,
    Engine,
};
use rvn_parser::{
    localized_text::{DeferredText, StringTableReference},
    Name, Value,
};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
const ID: &str = "17411a3e431e4c9388b78a2b9825b130";
const CASES: [(&str, &str); 7] = [
    ("clé😀\nfin", "Source LF0316"),
    ("clé😀\r\nfin", "Source CRLF0316"),
    ("clé avec espace", "Source Space0316"),
    ("clé\"quote", "Source Quote0316"),
    ("clé\tfin", "Source Tab0316"),
    ("", "Source Empty0316"),
    ("hello", "Source Simple0316"),
];
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture {
    root: PathBuf,
    app: App,
}
fn table() -> StringTableAsset {
    StringTableAsset {
        id: ID.into(),
        namespace: "VN".into(),
        entries: CASES
            .iter()
            .map(|(key, source)| {
                (
                    key.to_string(),
                    StringTableEntry {
                        source: source.to_string(),
                        developer_notes: "Raw stored key, exact UTF-8".into(),
                        metadata: Default::default(),
                    },
                )
            })
            .collect(),
    }
}
fn declarations() -> LocaleDeclarations {
    LocaleDeclarations {
        default_lang: "fr".into(),
        current_lang: "en".into(),
        available: vec!["fr".into(), "en".into()],
        files: BTreeMap::from([
            ("fr".into(), "fr.toml".into()),
            ("en".into(), "en.toml".into()),
        ]),
    }
}
fn catalog(asset: &StringTableAsset, lang: &str) -> LocaleCatalog {
    let strings = asset
        .entries
        .iter()
        .map(|(key, entry)| {
            (
                asset
                    .localization(key)
                    .unwrap()
                    .identity()
                    .unwrap()
                    .locale_key(),
                format!("{lang}:{}", entry.source),
            )
        })
        .collect();
    let mut catalog = LocaleCatalog::from(LocaleTable {
        lang: lang.into(),
        strings,
    });
    for key in asset.entries.keys() {
        assert!(catalog
            .record_string_table_source_witness(asset, key)
            .unwrap());
    }
    catalog
}
fn write_table(f: &Fixture, asset: StringTableAsset) {
    fs::write(
        f.root.join("assets/table.rvnstrings"),
        StringTableFile::new(asset).unwrap().encode().unwrap(),
    )
    .unwrap();
}
fn fixture() -> Fixture {
    let root = std::env::temp_dir().join(format!(
        "rvn-key-restore-0340-{}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(root.join("assets")).unwrap();
    fs::create_dir_all(root.join("locales")).unwrap();
    let asset = table();
    fs::write(
        root.join("assets/table.rvnstrings"),
        StringTableFile::new(asset.clone())
            .unwrap()
            .encode()
            .unwrap(),
    )
    .unwrap();
    for lang in ["fr", "en"] {
        fs::write(
            root.join(format!("locales/{lang}.toml")),
            catalog(&asset, lang).to_toml(),
        )
        .unwrap();
    }
    let mut registry = StringTableResources::default();
    registry.publish(asset).unwrap();
    let engine = Engine::new_with_text_resources(
        rvn_parser::parse("init{set calls=0}\nlabel start\nset calls=calls+1\n\"Hello\"\n")
            .unwrap(),
        BevyRenderer::new(),
        8,
        registry,
    )
    .unwrap();
    let mut app = App::new();
    register_text_sources(
        &mut app,
        root.join("assets").to_string_lossy().into_owned(),
        root.join("locales").to_string_lossy().into_owned(),
    )
    .unwrap();
    app.add_plugins(MinimalPlugins)
        .add_plugins(AssetPlugin {
            file_path: root.join("assets").to_string_lossy().into_owned(),
            watch_for_changes_override: Some(false),
            ..default()
        })
        .insert_resource(VnEngine(engine))
        .add_event::<VnCommand>()
        .add_plugins(StringTableAssetsPlugin::new(BTreeMap::from([(
            ID.into(),
            "table.rvnstrings".into(),
        )])))
        .add_plugins(ClosedLocaleAssetsPlugin::new(declarations()).unwrap());
    app.update();
    Fixture { root, app }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let Ok(root) = self.root.canonicalize() else {
            return;
        };
        let Ok(temp) = std::env::temp_dir().canonicalize() else {
            return;
        };
        if root.parent() == Some(temp.as_path())
            && root.file_name().is_some_and(|name| {
                name.to_string_lossy()
                    .starts_with(&format!("rvn-key-restore-0340-{}-", std::process::id()))
            })
        {
            let _ = fs::remove_dir_all(root);
        }
    }
}
fn pump(f: &mut Fixture, predicate: impl Fn(&World) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        f.app.update();
        if predicate(f.app.world()) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "Actual key AssetServer publication timed out"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}
fn ready(f: &mut Fixture) -> PortableTextHostReceipt {
    pump(f, |world| portable_receipt(world).is_ok());
    portable_receipt(f.app.world()).unwrap()
}
fn text(key: &str) -> Value {
    rvn_core::localized_text::construct(
        "__rvn_string_table_text_name",
        &[Value::Name(Name::new(ID).unwrap()), Value::Str(key.into())],
    )
    .unwrap()
}
fn render(world: &World, value: &Value, locale: Option<&LocaleManager>) -> String {
    let Value::Text(value) = value else {
        panic!("Expected real Name Text")
    };
    rvn_core::localized_text::render_text_with_resources(
        value,
        locale,
        &rvn_core::localized_text::VnTextScalars,
        Some(world.resource::<VnEngine>().0.text_resources()),
    )
    .unwrap()
}
fn saved_fr(f: &mut Fixture) -> PortableTextHostReceipt {
    f.app
        .world_mut()
        .resource_mut::<VnEngine>()
        .0
        .locale
        .as_mut()
        .unwrap()
        .set_language("fr")
        .unwrap();
    let saved = portable_receipt(f.app.world()).unwrap();
    f.app
        .world_mut()
        .resource_mut::<VnEngine>()
        .0
        .locale
        .as_mut()
        .unwrap()
        .set_language("en")
        .unwrap();
    saved
}

#[test]
fn actual_seven_ue0332_raw_keys_load_transport_inspect_and_localize_without_normalization() {
    let mut f = fixture();
    let receipt = ready(&mut f);
    assert_eq!(receipt.registry.tables.len(), 1);
    let host = f.app.world().resource::<TableHost>();
    let decoded = &f
        .app
        .world()
        .resource::<Assets<LoadedStringTable>>()
        .get(host.bound[ID].handle.id())
        .unwrap()
        .0;
    assert_eq!(decoded.as_ref(), &table());
    assert_ne!(CASES[0].0, CASES[1].0);
    let locale = f
        .app
        .world()
        .resource::<VnEngine>()
        .0
        .locale
        .as_ref()
        .unwrap();
    assert_eq!(
        locale
            .portable_receipt()
            .unwrap()
            .current
            .text_sources
            .len(),
        7
    );
    for (key, source) in CASES {
        let value = text(key);
        let transported: Value =
            serde_json::from_slice(&serde_json::to_vec(&value).unwrap()).unwrap();
        assert_eq!(transported, value);
        assert_eq!(
            rvn_core::localized_text::inspect_table_identity(
                "__rvn_text_table_valid",
                &[transported.clone()]
            )
            .unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            rvn_core::localized_text::inspect_table_identity(
                "__rvn_text_table_id",
                &[transported.clone()]
            )
            .unwrap(),
            Value::Name(Name::new(ID).unwrap())
        );
        assert_eq!(
            rvn_core::localized_text::inspect_table_identity(
                "__rvn_text_table_key",
                &[transported.clone()]
            )
            .unwrap(),
            Value::Str(key.into())
        );
        assert_eq!(render(f.app.world(), &transported, None), source);
        assert_eq!(
            render(f.app.world(), &transported, Some(locale)),
            format!("en:{source}")
        );
    }
    assert!(f.app.world().resource::<VnEngine>().0.state.vars["calls"] == Value::Int(0));
}
#[test]
fn raw_keys_portable_receipt_relocation_and_changed_source_do_not_trust_stale_witness() {
    let mut first = fixture();
    let before = ready(&mut first);
    let mut relocated = fixture();
    assert_eq!(ready(&mut relocated), before);
    let restored: PortableTextHostReceipt =
        serde_json::from_slice(&serde_json::to_vec(&before).unwrap()).unwrap();
    assert!(accepts_portable_receipt(relocated.app.world(), &restored).unwrap());
    let old = text(CASES[0].0);
    let mut changed = table();
    changed.entries.get_mut(CASES[0].0).unwrap().source = "Changed exact LF source".into();
    write_table(&first, changed);
    first
        .app
        .world()
        .resource::<AssetServer>()
        .reload("table.rvnstrings");
    pump(&mut first, |world| {
        world
            .resource::<VnEngine>()
            .0
            .text_resources()
            .ready_table(ID)
            .is_some_and(|asset| asset.entries[CASES[0].0].source == "Changed exact LF source")
            && portable_receipt(world).is_ok()
    });
    assert!(!accepts_portable_receipt(first.app.world(), &before).unwrap());
    assert_ne!(
        portable_receipt(first.app.world())
            .unwrap()
            .canonical_digest()
            .unwrap(),
        before.canonical_digest().unwrap()
    );
    let locale = first
        .app
        .world()
        .resource::<VnEngine>()
        .0
        .locale
        .as_ref()
        .unwrap();
    assert_eq!(
        render(first.app.world(), &old, Some(locale)),
        "Changed exact LF source"
    );
    assert_eq!(
        render(first.app.world(), &text(CASES[1].0), Some(locale)),
        "en:Source CRLF0316"
    );
}
#[test]
fn actual_nul_and_oversized_key_decode_failure_retains_ready_content_and_never_normalizes() {
    let mut f = fixture();
    let before = ready(&mut f);
    let pc = f.app.world().resource::<VnEngine>().0.state.pc;
    for invalid in ["bad\0key".to_string(), "x".repeat(4097)] {
        let mut wire: serde_json::Value =
            serde_json::from_slice(&StringTableFile::new(table()).unwrap().encode().unwrap())
                .unwrap();
        wire["entries"].as_object_mut().unwrap().insert(
            invalid,
            serde_json::json!({"source":"Rejected","developer_notes":""}),
        );
        fs::write(
            f.root.join("assets/table.rvnstrings"),
            serde_json::to_vec(&wire).unwrap(),
        )
        .unwrap();
        f.app
            .world()
            .resource::<AssetServer>()
            .reload("table.rvnstrings");
        pump(&mut f, |world| {
            world
                .resource::<StringTableDiagnostics>()
                .0
                .contains_key(ID)
                && portable_receipt(world).is_ok()
        });
        let failed = portable_receipt(f.app.world()).unwrap();
        assert_eq!(failed.registry, before.registry);
        assert_eq!(failed.table_states[ID], PublishedLoadState::Failed);
        assert_eq!(f.app.world().resource::<VnEngine>().0.state.pc, pc);
        assert_eq!(
            f.app.world().resource::<VnEngine>().0.state.vars["calls"],
            Value::Int(0)
        );
        write_table(&f, table());
        f.app
            .world()
            .resource::<AssetServer>()
            .reload("table.rvnstrings");
        pump(&mut f, |world| {
            portable_receipt(world).is_ok_and(|receipt| receipt == before)
        });
    }
}
#[test]
fn legacy_string_text_and_authored_identity_admission_remain_strict() {
    for key in [CASES[0].0, CASES[1].0, CASES[4].0, CASES[5].0] {
        assert!(rvn_core::localized_text::construct(
            "__rvn_string_table_text",
            &[Value::Str(ID.into()), Value::Str(key.into())]
        )
        .is_err());
        assert!(StringTableReference {
            table: ID.into(),
            key: key.into()
        }
        .validate()
        .is_err());
        let identity = table().localization(key).unwrap();
        let asset = table();
        let mut legacy = catalog(&asset, "en");
        assert!(legacy
            .record_text_source_witness(identity.identity().unwrap(), &asset.entries[key].source)
            .is_err());
    }
    assert!(rvn_core::localized_text::construct(
        "__rvn_string_table_text",
        &[Value::Str(ID.into()), Value::Str("hello".into())]
    )
    .is_ok());
}
#[test]
fn selected_language_candidate_is_reconstructed_readonly_from_actual_assets() {
    let mut f = fixture();
    ready(&mut f);
    let saved = saved_fr(&mut f);
    let before = live_receipt(f.app.world()).unwrap();
    let pc = f.app.world().resource::<VnEngine>().0.state.pc;
    let vars = f.app.world().resource::<VnEngine>().0.state.vars.clone();
    let prepared = prepare_selected_language(f.app.world(), &saved).unwrap();
    assert_eq!(prepared.receipt(), &saved);
    assert_eq!(prepared.locale().current_lang(), "fr");
    assert_eq!(
        f.app
            .world()
            .resource::<VnEngine>()
            .0
            .locale
            .as_ref()
            .unwrap()
            .current_lang(),
        "en"
    );
    assert!(accepts_live_receipt(f.app.world(), &before).unwrap());
    prepared.verify_host(f.app.world()).unwrap();
    for (key, source) in CASES {
        assert_eq!(
            render(f.app.world(), &text(key), Some(prepared.locale())),
            format!("fr:{source}")
        );
    }
    assert_eq!(f.app.world().resource::<VnEngine>().0.state.pc, pc);
    assert_eq!(f.app.world().resource::<VnEngine>().0.state.vars, vars);
    assert!(f
        .app
        .world()
        .resource::<VnEngine>()
        .0
        .renderer
        .pending
        .is_empty());
}
#[test]
fn selected_language_refuses_forged_catalogues_unknown_language_and_unpublished_assets_before_effects(
) {
    let mut f = fixture();
    ready(&mut f);
    let saved = saved_fr(&mut f);
    let pc = f.app.world().resource::<VnEngine>().0.state.pc;
    let vars = f.app.world().resource::<VnEngine>().0.state.vars.clone();
    let mut unknown = saved.clone();
    unknown.locales.current_lang = "de".into();
    assert!(prepare_selected_language(f.app.world(), &unknown).is_err());
    let address = table()
        .localization("hello")
        .unwrap()
        .identity()
        .unwrap()
        .locale_key();
    let mut forged = saved.clone();
    forged
        .locales
        .cache
        .get_mut("fr")
        .unwrap()
        .strings
        .insert(address.clone(), "Forged".into());
    forged
        .locales
        .current
        .strings
        .insert(address.clone(), "Forged".into());
    forged
        .locales
        .fallback
        .strings
        .insert(address, "Forged".into());
    assert!(forged.canonical_digest().is_ok());
    assert!(prepare_selected_language(f.app.world(), &forged).is_err());
    let handle = f.app.world().resource::<LocaleAssetHost>().bound["fr"].clone();
    f.app
        .world_mut()
        .resource_mut::<Assets<LoadedLocaleSource>>()
        .get_mut(handle.id())
        .unwrap()
        .0 = "[strings]\n\"changed\"=\"Changed\"\n".into();
    assert!(prepare_selected_language(f.app.world(), &saved).is_err());
    assert_eq!(f.app.world().resource::<VnEngine>().0.state.pc, pc);
    assert_eq!(f.app.world().resource::<VnEngine>().0.state.vars, vars);
    assert_eq!(
        f.app
            .world()
            .resource::<VnEngine>()
            .0
            .locale
            .as_ref()
            .unwrap()
            .current_lang(),
        "en"
    );
}
#[test]
fn prepared_language_is_revoked_by_actual_equivalent_reload_journal_and_engine_replacement() {
    let mut f = fixture();
    ready(&mut f);
    let saved = saved_fr(&mut f);
    let prepared = prepare_selected_language(f.app.world(), &saved).unwrap();
    let generation = f.app.world().resource::<LocaleAssetHost>().publication;
    f.app
        .world()
        .resource::<AssetServer>()
        .reload(bevy::asset::AssetPath::from("fr.toml").with_source(LOCALE_SOURCE));
    pump(&mut f, |world| {
        world.resource::<LocaleAssetHost>().publication > generation
            && portable_receipt(world).is_ok()
    });
    assert!(prepared.verify_host(f.app.world()).is_err());
    let prepared = prepare_selected_language(f.app.world(), &saved).unwrap();
    f.app.world_mut().resource_mut::<PublicationBook>().epoch += 1;
    assert!(prepared.verify_host(f.app.world()).is_err());
    let prepared = prepare_selected_language(f.app.world(), &saved).unwrap();
    let registry = f
        .app
        .world()
        .resource::<VnEngine>()
        .0
        .text_resources()
        .clone();
    let locale = f.app.world().resource::<VnEngine>().0.locale.clone();
    let mut engine = Engine::new_with_text_resources(
        rvn_parser::parse("label start\n\"Fresh\"\n").unwrap(),
        BevyRenderer::new(),
        8,
        registry,
    )
    .unwrap();
    engine.locale = locale;
    f.app.world_mut().insert_resource(VnEngine(engine));
    assert!(prepared.verify_host(f.app.world()).is_err());
}
