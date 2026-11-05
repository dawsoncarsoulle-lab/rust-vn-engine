use super::*;
use crate::{
    bevy_renderer::BevyRenderer, string_table_assets::StringTableAssetsPlugin,
    vn_command::VnCommand,
};
use rvn_core::{
    string_table::{StringTableAsset, StringTableEntry, StringTableResources},
    string_table_file::StringTableFile,
    Engine,
};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
const ID: &str = "17411a3e431e4c9388b78a2b9825b130";
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture {
    root: PathBuf,
    app: App,
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
fn table(source: &str) -> StringTableAsset {
    StringTableAsset {
        id: ID.into(),
        namespace: "VN".into(),
        entries: BTreeMap::from([(
            "hello".into(),
            StringTableEntry {
                source: source.into(),
                developer_notes: "Contexte été".into(),
                metadata: BTreeMap::new(),
            },
        )]),
    }
}
fn write_table(root: &std::path::Path, source: &str) {
    fs::write(
        root.join("assets/table.rvnstrings"),
        StringTableFile::new(table(source))
            .unwrap()
            .encode()
            .unwrap(),
    )
    .unwrap();
}
fn write_locale(root: &std::path::Path, lang: &str, translation: &str) {
    use rvn_core::locale::LocaleTable;
    let key = rvn_parser::localized_text::TextIdentity {
        owner: format!("string-table:{ID}"),
        namespace: "VN".into(),
        key: "hello".into(),
    }
    .locale_key();
    let catalog = LocaleCatalog::with_text_sources(
        LocaleTable {
            lang: lang.into(),
            strings: HashMap::from([
                ("First".into(), translation.into()),
                (key.clone(), translation.into()),
            ]),
        },
        HashMap::from([(key, "First".into())]),
    )
    .unwrap();
    fs::write(root.join(format!("locales/{lang}.toml")), catalog.to_toml()).unwrap();
}
fn fixture() -> Fixture {
    let root = std::env::temp_dir().join(format!(
        "rvn-portable-host-0321-{}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(root.join("assets")).unwrap();
    fs::create_dir_all(root.join("locales")).unwrap();
    write_table(&root, "First");
    write_locale(&root, "fr", "Premier");
    write_locale(&root, "en", "First English");
    let mut registry = StringTableResources::default();
    registry.publish(table("First")).unwrap();
    let engine = Engine::new_with_text_resources(
        rvn_parser::parse("label start\n\"Hello\"\n").unwrap(),
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
                    .starts_with(&format!("rvn-portable-host-0321-{}-", std::process::id()))
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
            "Actual AssetServer boundary timed out"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}
fn ready(f: &mut Fixture) -> PortableTextHostReceipt {
    pump(f, |world| portable_receipt(world).is_ok());
    portable_receipt(f.app.world()).unwrap()
}
fn reload_locale(f: &Fixture, lang: &str) {
    f.app
        .world()
        .resource::<AssetServer>()
        .reload(bevy::asset::AssetPath::from(format!("{lang}.toml")).with_source(LOCALE_SOURCE));
}

#[test]
fn actual_assets_nonempty_closed_catalogues_relocate_and_preserve_portable_content_only() {
    let mut first = fixture();
    let a = ready(&mut first);
    let live = live_receipt(first.app.world()).unwrap();
    let mut relocated = fixture();
    let b = ready(&mut relocated);
    assert_ne!(first.root, relocated.root);
    assert_eq!(a, b);
    assert_eq!(a.canonical_digest().unwrap(), b.canonical_digest().unwrap());
    assert!(accepts_portable_receipt(relocated.app.world(), &a).unwrap());
    assert!(!accepts_live_receipt(relocated.app.world(), &live).unwrap());
    assert_eq!(a.locales.cache.len(), 2);
    assert_eq!(a.locales.current.strings["First"], "First English");
    let json = serde_json::to_string(&a).unwrap();
    assert!(!json.contains(first.root.to_string_lossy().as_ref()));
    assert!(!json.contains("AssetId"));
}
#[test]
fn actual_reload_equivalence_keeps_portable_receipt_but_new_locale_publication_invalidates_live() {
    let mut f = fixture();
    let before = ready(&mut f);
    let live = live_receipt(f.app.world()).unwrap();
    reload_locale(&f, "en");
    pump(&mut f, |world| {
        world.resource::<LocaleAssetHost>().publication > live.locale_publication
            && portable_receipt(world).is_ok()
    });
    assert_eq!(portable_receipt(f.app.world()).unwrap(), before);
    assert!(!accepts_live_receipt(f.app.world(), &live).unwrap());
}

#[test]
fn fresh_actual_engine_in_same_host_cannot_reuse_previous_live_authority_receipt() {
    let mut f = fixture();
    let before = ready(&mut f);
    let live = live_receipt(f.app.world()).unwrap();
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
    observe_publication(f.app.world_mut());
    assert_eq!(portable_receipt(f.app.world()).unwrap(), before);
    assert!(!accepts_live_receipt(f.app.world(), &live).unwrap());
}
#[test]
fn actual_table_and_locale_modifications_change_receipt_without_replaying_producers() {
    let mut f = fixture();
    let before = ready(&mut f);
    write_table(&f.root, "Second");
    f.app
        .world()
        .resource::<AssetServer>()
        .reload("table.rvnstrings");
    pump(&mut f, |world| {
        world
            .resource::<VnEngine>()
            .0
            .text_resources()
            .ready_table(ID)
            .is_some_and(|asset| asset.entries["hello"].source == "Second")
            && portable_receipt(world).is_ok()
    });
    assert!(!accepts_portable_receipt(f.app.world(), &before).unwrap());
    let middle = portable_receipt(f.app.world()).unwrap();
    write_locale(&f.root, "en", "Changed English");
    reload_locale(&f, "en");
    pump(&mut f, |world| {
        portable_receipt(world)
            .is_ok_and(|receipt| receipt.locales.current.strings["First"] == "Changed English")
    });
    assert!(!accepts_portable_receipt(f.app.world(), &middle).unwrap());
}
#[test]
fn actual_table_failed_reload_records_retained_content_state_and_observable_diagnostic() {
    let mut f = fixture();
    let before = ready(&mut f);
    fs::write(f.root.join("assets/table.rvnstrings"), b"{broken").unwrap();
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
    assert!(!failed.table_diagnostics[ID].is_empty());
    assert_ne!(failed, before);
    assert!(accepts_portable_receipt(f.app.world(), &failed).unwrap());
    write_table(&f.root, "First");
    f.app
        .world()
        .resource::<AssetServer>()
        .reload("table.rvnstrings");
    pump(&mut f, |world| {
        portable_receipt(world).is_ok_and(|receipt| receipt == before)
    });
}
#[test]
fn actual_locale_failed_reload_blocks_closed_boundary_then_recovers() {
    let mut f = fixture();
    let before = ready(&mut f);
    fs::write(f.root.join("locales/en.toml"), b"[strings]\nbroken").unwrap();
    reload_locale(&f, "en");
    pump(&mut f, |world| {
        world
            .resource::<LocaleAssetHost>()
            .diagnostics
            .contains_key("en")
    });
    assert!(portable_receipt(f.app.world()).is_err());
    assert_eq!(
        f.app
            .world()
            .resource::<VnEngine>()
            .0
            .locale
            .as_ref()
            .unwrap()
            .portable_receipt()
            .unwrap(),
        before.locales
    );
    write_locale(&f.root, "en", "First English");
    reload_locale(&f, "en");
    pump(&mut f, |world| {
        portable_receipt(world).is_ok_and(|receipt| receipt == before)
    });
}

#[test]
fn actual_loader_and_native_bootstrap_reject_oversized_locale_source_without_publication() {
    let mut f = fixture();
    let before = ready(&mut f);
    fs::write(
        f.root.join("locales/en.toml"),
        vec![b' '; 8 * 1024 * 1024 + 1],
    )
    .unwrap();
    assert!(load_native_frozen_locales(&f.root.join("locales"), &declarations()).is_err());
    reload_locale(&f, "en");
    pump(&mut f, |world| {
        world
            .resource::<LocaleAssetHost>()
            .diagnostics
            .contains_key("en")
    });
    assert!(portable_receipt(f.app.world()).is_err());
    assert_eq!(
        f.app
            .world()
            .resource::<VnEngine>()
            .0
            .locale
            .as_ref()
            .unwrap()
            .portable_receipt()
            .unwrap(),
        before.locales
    );
}
#[test]
fn actual_unpublished_asset_value_is_refused_even_before_event_delivery() {
    let mut f = fixture();
    ready(&mut f);
    let handle = f.app.world().resource::<LocaleAssetHost>().bound["en"].clone();
    f.app
        .world_mut()
        .resource_mut::<Assets<LoadedLocaleSource>>()
        .get_mut(handle.id())
        .unwrap()
        .0 = "[strings]\n\"First\"=\"Unpublished\"\n".into();
    assert!(portable_receipt(f.app.world()).is_err());
    // The negative mutation above is intentionally not a loader witness.
    // Only real bytes and the real registered AssetServer may supply success.
    fs::write(f.root.join("locales/en.toml"), "[strings]\n\"First\"=\"Unpublished\"\n").unwrap();
    reload_locale(&f,"en");
    pump(&mut f, |world| {
        portable_receipt(world)
            .is_ok_and(|receipt| receipt.locales.current.strings["First"] == "Unpublished")
    });
}
#[test]
fn actual_event_queues_missed_delivery_renderer_failure_and_loading_refuse_capture() {
    let mut f = fixture();
    ready(&mut f);
    let table_handle = f.app.world().resource::<TableHost>().bound[ID]
        .handle
        .clone();
    f.app
        .world_mut()
        .send_event(AssetEvent::<LoadedStringTable>::Modified {
            id: table_handle.id(),
        });
    assert!(portable_receipt(f.app.world()).is_err());
    ready(&mut f);
    f.app
        .world_mut()
        .resource_mut::<VnEngine>()
        .0
        .renderer
        .scene_frame_failed = true;
    assert!(portable_receipt(f.app.world()).is_err());
    f.app
        .world_mut()
        .resource_mut::<VnEngine>()
        .0
        .renderer
        .scene_frame_failed = false;
    f.app
        .world_mut()
        .resource_mut::<VnEngine>()
        .0
        .text_resources_mut()
        .begin_load(ID)
        .unwrap();
    assert!(portable_receipt(f.app.world()).is_err());
    let mut lost = fixture();
    ready(&mut lost);
    let handle = lost.app.world().resource::<TableHost>().bound[ID]
        .handle
        .clone();
    {
        let mut events = lost
            .app
            .world_mut()
            .resource_mut::<Events<AssetEvent<LoadedStringTable>>>();
        events.send(AssetEvent::Modified { id: handle.id() });
        events.update();
        events.update();
    }
    assert!(portable_receipt(lost.app.world()).is_err());
    lost.app.update();
    assert!(portable_receipt(lost.app.world()).is_err());
}
#[test]
fn declarations_budgets_aliases_and_missing_real_host_are_strict() {
    let good = declarations();
    assert!(good.validate().is_ok());
    for bad_path in [
        "../escape.toml",
        "/absolute.toml",
        "dir\\bad.toml",
        "x#label.toml",
        "C:/bad.toml",
    ] {
        let mut bad = good.clone();
        bad.files.insert("en".into(), bad_path.into());
        assert!(bad.validate().is_err());
    }
    let mut duplicate = good.clone();
    duplicate.files.insert("en".into(), "fr.toml".into());
    assert!(duplicate.validate().is_err());
    let mut incomplete = good.clone();
    incomplete.available.push("de".into());
    assert!(incomplete.validate().is_err());
    let mut oversized = good;
    oversized.files.insert("en".into(), "x".repeat(4097));
    assert!(oversized.validate().is_err());
    assert!(portable_receipt(&World::new()).is_err());
}

#[test]
fn redirects_language_selection_and_receipt_policy_are_observable_and_opaque_factory_is_refused() {
    use rvn_core::string_table::StringTableReference;
    let mut f = fixture();
    let before = ready(&mut f);
    f.app
        .world_mut()
        .resource_mut::<VnEngine>()
        .0
        .text_resources_mut()
        .redirect(
            StringTableReference {
                table: ID.into(),
                key: "old".into(),
            },
            StringTableReference {
                table: ID.into(),
                key: "hello".into(),
            },
        )
        .unwrap();
    observe_publication(f.app.world_mut());
    let redirect = portable_receipt(f.app.world()).unwrap();
    assert_eq!(redirect.registry.redirects.len(), 1);
    assert_ne!(
        redirect.canonical_digest().unwrap(),
        before.canonical_digest().unwrap()
    );
    f.app
        .world_mut()
        .resource_mut::<VnEngine>()
        .0
        .locale
        .as_mut()
        .unwrap()
        .set_language("fr")
        .unwrap();
    let selected = portable_receipt(f.app.world()).unwrap();
    assert_eq!(selected.locales.current_lang, "fr");
    assert_ne!(selected, redirect);
    let mut invalid = selected.clone();
    invalid.locale_loader_policy = 2;
    assert!(invalid.canonical_digest().is_err());
    assert!(!accepts_portable_receipt(f.app.world(), &invalid).unwrap());
    f.app.world_mut().remove_resource::<KnownTextSources>();
    assert!(portable_receipt(f.app.world()).is_err());
    assert!(register_text_sources(&mut f.app, "other".into(), "other".into()).is_err());
}
#[test]
fn actual_native_project_closed_bootstrap_supplies_all_locales_before_init_and_keeps_files() {
    let f = fixture();
    let story=format!("init{{\nset text=__rvn_string_table_text(\"{ID}\",\"hello\")\nset observed=text_to_string(text)\n}}\nlabel start\n\"Hello\"\n");
    fs::write(f.root.join("main.rvn"), &story).unwrap();
    fs::write(f.root.join("rvn.toml"),format!("[project]\ntitle='Closed resources'\nmain_script='main.rvn'\n[paths]\nassets='assets'\nlocales='locales'\nsaves='saves'\ntheme='theme.toml'\n[string_tables]\n'{ID}'='table.rvnstrings'\n")).unwrap();
    fs::write(
        f.root.join("assets/config.toml"),
        "[locale]\ndefault='fr'\ncurrent='en'\navailable=['fr','en']\n",
    )
    .unwrap();
    let before = fs::read(f.root.join("locales/fr.toml")).unwrap();
    let launch = crate::load_game_text_profile(&f.root, true).unwrap();
    assert_eq!(
        launch.engine.state.vars["observed"],
        rvn_parser::Value::Str("First English".into())
    );
    assert_eq!(
        launch
            .engine
            .locale
            .as_ref()
            .unwrap()
            .portable_receipt()
            .unwrap()
            .cache
            .len(),
        2
    );
    assert!(launch.closed_locales.is_some());
    assert_eq!(fs::read(f.root.join("locales/fr.toml")).unwrap(), before);
    assert_eq!(fs::read_to_string(f.root.join("main.rvn")).unwrap(), story);
    fs::remove_file(f.root.join("locales/en.toml")).unwrap();
    assert!(crate::load_game_text_profile(&f.root, true).is_err());
}
#[test]
fn publication_journal_is_bounded_and_generation_overflow_refuses_capture() {
    let mut f = fixture();
    ready(&mut f);
    for n in 0..270 {
        f.app
            .world_mut()
            .resource_mut::<VnEngine>()
            .0
            .text_resources_mut()
            .publish(table(&format!("Version {n}")))
            .unwrap();
        observe_publication(f.app.world_mut());
    }
    assert_eq!(
        f.app.world().resource::<PublicationBook>().journal.len(),
        256
    );
    f.app.world_mut().resource_mut::<PublicationBook>().epoch = u64::MAX;
    f.app
        .world_mut()
        .resource_mut::<VnEngine>()
        .0
        .text_resources_mut()
        .publish(table("Overflow"))
        .unwrap();
    observe_publication(f.app.world_mut());
    assert!(f.app.world().resource::<PublicationBook>().exhausted);
    assert!(portable_receipt(f.app.world()).is_err());
}

fn atomic_save_fixture() -> (Fixture, rvn_parser::ParsedProgram, rvn_core::save::consumer_checkpoint::Save15, PortableTextHostReceipt) {
    let mut f=fixture(); ready(&mut f);
    let source=format!("label start\nflow.consume_v2(\"legacy\",\"capture\"){{set greeting=__rvn_string_table_text_name(__rvn_literal_name(\"{ID}\"),\"hello\")}}\n\"First\"\n\"after\"\n");
    let path=f.root.join("main.rvn"); fs::write(&path,&source).unwrap();
    let mut program=rvn_parser::parse_program(&fs::read_to_string(&path).unwrap()).unwrap();
    program.sources[0].path=Some(path.clone()); for origin in &mut program.origins {origin.source=Some(path.clone());}
    let previous=&f.app.world().resource::<VnEngine>().0;
    let resources=previous.text_resources().clone(); let locale=previous.locale.as_ref().unwrap().clone();
    let cold=Engine::from_program_with_persistent_and_text_context(program.clone(),BevyRenderer::new(),8,HashMap::new(),rvn_core::TextRuntimeContext::new(resources.clone(),Some(locale.clone()))).unwrap();
    let mut selected=locale; selected.set_language("fr").unwrap();
    let mut source_engine=Engine::from_program_with_persistent_and_text_context(program.clone(),BevyRenderer::new(),8,HashMap::new(),rvn_core::TextRuntimeContext::new(resources,Some(selected))).unwrap();
    source_engine.step_silent().unwrap();
    assert!(!source_engine.renderer.scene_batch_pending(),"Capture requires genuinely settled Core/renderer state");
    let save=source_engine.capture_save15(&program,None,1,"actual selected French".into()).unwrap();
    f.app.world_mut().init_resource::<crate::scene_publication::ScenePresentation>();
    f.app.world_mut().insert_resource(VnEngine(cold));
    let mut settings=crate::systems::settings_menu::Settings::default(); settings.language="en".into(); f.app.world_mut().insert_resource(settings);
    let mut expected=portable_receipt(f.app.world()).unwrap(); expected.locales=save.locale.as_ref().unwrap().clone();
    (f,program,save,expected)
}
fn atomic_world_before(f:&Fixture)->(serde_json::Value,LocaleContentSnapshot,String,u64,bool) {
    let engine=&f.app.world().resource::<VnEngine>().0;
    (serde_json::to_value(&engine.state).unwrap(),engine.locale.as_ref().unwrap().portable_receipt().unwrap(),f.app.world().resource::<crate::systems::settings_menu::Settings>().language.clone(),engine.runtime_epoch(),engine.renderer.scene_batch_pending())
}
#[test]
fn actual_asset_host_atomic_prepare_and_commit_select_language_game_and_renderer_queue_together() {
    let(mut f,program,save,expected)=atomic_save_fixture(); let before=atomic_world_before(&f);
    let plan=crate::atomic_text_restore::prepare(f.app.world(),&program,save,&expected).unwrap(); assert_eq!(atomic_world_before(&f),before);
    assert!(crate::atomic_text_restore::commit(f.app.world_mut(),plan).unwrap().is_none());
    let e=&f.app.world().resource::<VnEngine>().0;
    assert_eq!(e.locale.as_ref().unwrap().current_lang(),"fr"); assert_eq!(f.app.world().resource::<crate::systems::settings_menu::Settings>().language,"fr");
    assert!(e.runtime_epoch()>before.3); assert!(e.state.vars.contains_key("greeting"));
    assert_eq!(e.displayed_interaction().unwrap(),Some(rvn_core::Interaction::Dialogue{character:None,text:"Premier".into()}));
    assert!(!e.renderer.scene_batch_pending(),"Atomic World commit must finish actual ECS installation synchronously");
    assert_eq!(serde_json::to_value(&f.app.world().resource::<crate::scene_publication::ScenePresentation>().batch.as_ref().unwrap().state).unwrap(),serde_json::to_value(&e.state).unwrap());
}
#[test]
fn actual_asset_edit_after_preparation_refuses_without_partial_settings_or_core_changes() {
    let(mut f,program,save,expected)=atomic_save_fixture();
    let plan=crate::atomic_text_restore::prepare(f.app.world(),&program,save,&expected).unwrap();
    let handle=f.app.world().resource::<LocaleAssetHost>().bound["fr"].clone();
    f.app.world_mut().resource_mut::<Assets<LoadedLocaleSource>>().get_mut(handle.id()).unwrap().0.push_str("\n# actual unpublished edit\n");
    let before=atomic_world_before(&f);
    assert!(crate::atomic_text_restore::commit(f.app.world_mut(),plan).is_err()); assert_eq!(atomic_world_before(&f),before);
}
#[test]
fn actual_settings_change_after_preparation_refuses_without_overwriting_the_new_preference() {
    let(mut f,program,save,expected)=atomic_save_fixture();
    let plan=crate::atomic_text_restore::prepare(f.app.world(),&program,save,&expected).unwrap();
    f.app.world_mut().resource_mut::<crate::systems::settings_menu::Settings>().text_speed=0.75;
    let before=atomic_world_before(&f);
    assert!(crate::atomic_text_restore::commit(f.app.world_mut(),plan).is_err()); assert_eq!(atomic_world_before(&f),before);
    assert_eq!(f.app.world().resource::<crate::systems::settings_menu::Settings>().text_speed,0.75);
}
#[test]
fn actual_language_round_trip_revokes_bevy_host_plan_with_equal_semantic_receipt() {
    let(mut f,program,save,expected)=atomic_save_fixture();
    let plan=crate::atomic_text_restore::prepare(f.app.world(),&program,save,&expected).unwrap();
    let original=portable_receipt(f.app.world()).unwrap();
    let mut engine=f.app.world_mut().resource_mut::<VnEngine>();
    engine.0.locale.as_mut().unwrap().set_language("fr").unwrap(); engine.0.locale.as_mut().unwrap().set_language("en").unwrap(); drop(engine);
    assert_eq!(portable_receipt(f.app.world()).unwrap(),original);
    let before=atomic_world_before(&f);
    assert!(crate::atomic_text_restore::commit(f.app.world_mut(),plan).is_err()); assert_eq!(atomic_world_before(&f),before);
}
#[test]
fn actual_changed_program_is_refused_before_any_asset_host_publication() {
    let(f,mut program,save,expected)=atomic_save_fixture(); let before=atomic_world_before(&f);
    program.sources[0].source.push('\n');
    assert!(crate::atomic_text_restore::prepare(f.app.world(),&program,save,&expected).is_err()); assert_eq!(atomic_world_before(&f),before);
}

#[path="atomic_world_restore_tests.rs"]
mod atomic_world_contracts;

#[cfg(all(feature="video",not(target_arch="wasm32")))]
#[path="native_atomic_restore_tests.rs"]
mod native_atomic_contracts;
#[cfg(all(feature="video",not(target_arch="wasm32")))]
#[path="frame_atomic_restore_tests.rs"]
mod frame_atomic_contracts;

#[path="game_audio_world_0521.rs"]
mod game_audio_world_0521;

#[path="locale_selection_mode_host_tests.rs"]
mod selection_mode_contracts;

#[path="locale_absence_policy_tests_0697.rs"]
mod absence_policy_contracts;
