//! Prepared targeted CPU tests. None has been run for packet0666.
//! Missing selected catalogue admission additionally requires absence0664.
use super::*;
use crate::{load_game_text_profile, load_runtime_game, settings_from_persistent, RuntimeLaunchOwner};
use rvn_core::{locale::LocaleProviderPolicy, PersistentData, PersistentDataManager};
use rvn_graph::{import_script, prepare_native_frame_graph, GraphDocument, GraphKind, NodeKind};
use std::{fs, path::{Path, PathBuf}};

fn config(default: &str, current: &str, available: &[&str]) -> LocaleConfigFile {
    LocaleConfigFile { default: default.into(), current: current.into(),
        available: available.iter().map(|value| (*value).into()).collect() }
}

#[test]
fn initial_locale_domain_adds_selected_values_without_mutating_ui_order_or_duplicates() {
    let config = config("de", "zz", &["en", "fr", "en"]);
    let declarations = initial_closed_declarations(&config, false);
    assert_eq!(declarations.available, ["en", "fr", "de", "zz"]);
    assert_eq!(config.available, ["en", "fr", "en"]);
    assert_eq!(config.default, "de");
    assert_eq!(config.current, "zz");
    declarations.validate().unwrap();
    assert_eq!(declarations.files.keys().cloned().collect::<Vec<_>>(), ["de", "en", "fr", "zz"]);
}

#[test]
fn initial_locale_explicit_closed_keeps_its_existing_incomplete_domain_refusal() {
    let selected = config("fr", "zz", &["fr"]);
    let strict = initial_closed_declarations(&selected, true);
    assert_eq!(strict.available, ["fr"]);
    assert!(strict.validate().is_err());
    assert!(initial_closed_declarations(&selected, false).validate().is_ok());
    let duplicated = initial_closed_declarations(&config("fr", "fr", &["fr", "fr"]), true);
    assert!(duplicated.validate().is_err());
}

#[test]
fn initial_locale_compatible_domain_keeps_identifier_and_source_validation() {
    let declarations = initial_closed_declarations(&config("fr", "../escape", &["fr"]), false);
    assert!(declarations.validate().is_err());
}

#[test]
fn initial_locale_empty_ui_keeps_only_selected_provider_catalogues() {
    let config = config("fr", "zz", &[]);
    let declarations = initial_closed_declarations(&config, false);
    assert!(config.available.is_empty());
    assert_eq!(declarations.available, ["fr", "zz"]);
    declarations.validate().unwrap();
}

fn project(name: &str, native: bool, locale_config: &str) -> PathBuf {
    project_with_catalogue(name, native, locale_config, true)
}

fn project_with_catalogue(name: &str, native: bool, locale_config: &str, present_fr: bool) -> PathBuf {
    let base = PathBuf::from(std::env::var("RVN_INITIAL_LOCALE_FIXTURES")
        .expect("owned initial-locale fixture directory on SSD"));
    assert!(base.is_absolute());
    let root = base.join(format!("{name}-{}-{}", std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    assert!(!root.exists());
    for directory in ["assets", "locales", "saves"] { fs::create_dir_all(root.join(directory)).unwrap(); }
    fs::write(root.join("rvn.toml"), "[project]\ntitle='Initial locale selection'\nmain_script='main.rvn'\nstart_label='start'\n[paths]\nassets='assets'\nlocales='locales'\nsaves='saves'\ntheme='theme.toml'\n").unwrap();
    fs::write(root.join("theme.toml"), "[title_screen]\nenabled=true\n").unwrap();
    fs::write(root.join("assets/config.toml"), locale_config).unwrap();
    if present_fr { fs::write(root.join("locales/fr.toml"), "[strings]\n").unwrap(); }
    let init = if native {
        let ast = rvn_parser::parse("init {while false {set never_reached=1}}\n").unwrap();
        let graph = import_script(&ast).unwrap().into_iter().find(|graph| matches!(graph.kind, GraphKind::Init)).unwrap();
        let mut graph = GraphDocument::from_json(&graph.to_pretty_json().unwrap()).unwrap();
        assert!(graph.nodes.values().any(|node| node.kind == NodeKind::While));
        graph.enable_native_frame_preparation().unwrap();
        let compiled = prepare_native_frame_graph(&graph).unwrap();
        assert!(compiled.source.contains("flow.consume_v2("));
        fs::write(root.join("actual.graph.json"), graph.to_pretty_json().unwrap()).unwrap();
        compiled.source
    } else { "init {set initialized=true}\n".into() };
    fs::write(root.join("main.rvn"), format!("{init}\nscene label start\n\"Ready\"\n")).unwrap();
    assert!(!root.join(rvn_core::debugger_provenance::SIDECAR).exists());
    root
}

fn assert_pending_selection(root: &Path, selected: &str, expected_ui: &[&str], expected_closure: &[&str]) {
    let launch = load_runtime_game(root).unwrap();
    assert_eq!(launch.locale_cfg.available, expected_ui);
    assert_eq!(launch.locale_cfg.current, selected);
    assert_eq!(settings_from_persistent(&launch.persistent_data, &launch.locale_cfg.current).language, selected);
    let (_, declarations) = launch.closed_locales.as_ref().expect("actual closed provider");
    assert_eq!(declarations.available, expected_closure);
    let RuntimeLaunchOwner::Startup(startup) = &launch.engine else { panic!("real native Init must keep its Startup owner") };
    assert!(!startup.owner.is_complete());
    assert!(!startup.owner.state().vars.contains_key("never_reached"));
    let locale = startup.owner.locale().unwrap();
    assert_eq!(locale.provider_policy(), LocaleProviderPolicy::FrozenCatalogs);
    assert_eq!(locale.current_lang(), selected);
    assert_eq!(locale.available_langs, expected_closure);
    assert!(locale.portable_restore_digest().is_ok());
    assert!(!root.join(rvn_core::debugger_provenance::SIDECAR).exists());
}

#[test]
fn initial_locale_normal_loader_admits_missing_configured_current_outside_ui() {
    let root = project("current-outside", true, "[locale]\ndefault='fr'\ncurrent='zz'\navailable=['fr']\n");
    assert_pending_selection(&root, "zz", &["fr"], &["fr", "zz"]);
    assert!(!root.join("locales/zz.toml").exists());
}

#[test]
fn initial_locale_normal_loader_uses_real_persisted_selection_without_ui_union() {
    let root = project("persistent-outside", true, "[locale]\ndefault='fr'\ncurrent='fr'\navailable=['fr']\n");
    let manager = PersistentDataManager::new(root.join("saves")).unwrap();
    let mut data = PersistentData::default(); data.language = Some("zz".into()); manager.save(&data).unwrap();
    let before = fs::read(root.join("saves/persistent.json")).unwrap();
    assert_pending_selection(&root, "zz", &["fr"], &["fr", "zz"]);
    assert_eq!(manager.load().unwrap().language.as_deref(), Some("zz"));
    assert_eq!(fs::read(root.join("saves/persistent.json")).unwrap(), before);
    assert!(!root.join("locales/zz.toml").exists());
}

#[test]
fn initial_locale_normal_loader_admits_missing_default_without_promoting_ui_choice() {
    let root = project("default-outside", true, "[locale]\ndefault='zz'\ncurrent='fr'\navailable=['fr']\n");
    assert_pending_selection(&root, "fr", &["fr"], &["fr", "zz"]);
    assert!(!root.join("locales/zz.toml").exists());
}

#[test]
fn initial_locale_normal_loader_keeps_empty_ui_when_no_catalogue_is_discovered() {
    let root = project_with_catalogue("empty-ui", true, "[locale]\ndefault='fr'\ncurrent='zz'\navailable=[]\n", false);
    assert_pending_selection(&root, "zz", &[], &["fr", "zz"]);
    assert!(!root.join("locales/fr.toml").exists());
    assert!(!root.join("locales/zz.toml").exists());
}

#[test]
fn initial_locale_normal_loader_preserves_discovery_order_and_duplicate_ui_entries() {
    let root = project("discovery-ui", true, "[locale]\ndefault='fr'\ncurrent='zz'\navailable=['fr','fr']\n");
    fs::write(root.join("locales/zz.toml"), "[strings]\n").unwrap();
    assert_pending_selection(&root, "zz", &["fr", "fr", "zz"], &["fr", "zz"]);
}

#[test]
fn initial_locale_real_explicit_closed_loader_remains_strict() {
    let root = project("strict-outside", true, "[locale]\ndefault='fr'\ncurrent='zz'\navailable=['fr']\n");
    let error = match load_game_text_profile(&root, true) { Ok(_) => panic!("explicit incomplete closure must remain refused"), Err(error) => error };
    assert!(error.contains("complete selected language closure"), "{error}");
    assert!(!root.join("locales/zz.toml").exists());
}

#[test]
fn initial_locale_historical_normal_without_owned_init_keeps_external_provider() {
    let root = project("legacy-external", false, "[locale]\ndefault='fr'\ncurrent='zz'\navailable=['fr']\n");
    let launch = load_runtime_game(&root).unwrap();
    assert_eq!(launch.locale_cfg.available, ["fr"]);
    assert!(launch.closed_locales.is_none());
    let RuntimeLaunchOwner::Game(engine) = launch.engine else { panic!("legacy Init should remain inline") };
    let locale = engine.locale.as_ref().unwrap();
    assert_eq!(locale.current_lang(), "zz");
    assert_eq!(locale.provider_policy(), LocaleProviderPolicy::ExternalFilesystem);
    assert!(!root.join("locales/zz.toml").exists());
}
