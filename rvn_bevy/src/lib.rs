// Bevy systems routinely exceed clippy's defaults for argument count and
// query type complexity; these are idiomatic for the ECS pattern.
#![allow(
    clippy::too_many_arguments,
    clippy::type_complexity,
    clippy::clone_on_copy,
    dead_code
)]
//! Library interface for the RVN Bevy runtime.
//!
//! This module exposes a single function, [`run_game`], which launches a
//! visual novel defined in a project directory.  The runtime loads the
//! project configuration from `rvn.toml`, reads the script file, theme
//! definition and locale files from the project's asset directories, and
//! then starts a Bevy application using the existing systems and resources.
//! This design allows the CLI to drive the engine without embedding any
//! project‑specific paths in the binary.

mod atomic_text_restore;
mod bevy_renderer;
mod widget_backend_0956;
#[cfg(test)]
mod choice_cleanup_tests;
mod components;
mod project_paths;
#[cfg(not(target_arch="wasm32"))]
mod widget_runtime_loader_0951;
#[cfg(not(target_arch="wasm32"))]
pub mod enum_runtime_loader1025;
#[cfg(not(target_arch = "wasm32"))]
mod qa_playthrough;
mod resources;
mod systems;
mod vn_command;
use crate::bevy_renderer::BevyRenderer;
mod accessibility;
mod composed_motion;
mod background_geometry;
mod presentation_order;
mod custom_canvas;
mod layered_characters;
mod menu_documents;
mod source_menus;
mod checkpoint_requests;
mod audio_rollback_restore;
mod scene_publication;
mod display_reveal;
mod scene_effects;
mod scene_retired;
mod scene_audio;
mod checkpoint_pcm;
// Bevy 0.14's RenderLabel derive cannot inspect target-specific dependencies.
#[cfg(not(target_arch = "wasm32"))]
use bevy::render::render_graph;
mod editor_preview;
mod editor_preview_inspection;
mod editor_debugger;
mod debugger_startup;
mod runtime_restart;
mod text_runtime_owner;
#[cfg(all(test, not(target_arch="wasm32")))]
mod startup_full_launch_tests;
#[cfg(not(target_arch="wasm32"))]
mod editor_preview_image;
mod editor_preview_input;
mod checkpoint_broker;
mod checkpoint_sources;
mod music_mix;
mod web_music;
#[cfg(not(target_arch = "wasm32"))]
mod menu_preview_data;
#[cfg(all(feature = "video", not(target_arch = "wasm32")))]
mod native_video;
mod portable_images;
mod string_table_assets;
mod portable_text_resources;
mod locale_selection;
mod programmable_ui;
mod ui_input_bridge;
mod save_thumbnails;
#[cfg(target_os = "linux")]
mod speech_linux;
#[cfg(target_arch = "wasm32")]
mod speech_web;
#[cfg(target_os = "windows")]
mod speech_windows;
#[cfg(not(target_arch = "wasm32"))]
mod speech_worker;
#[cfg(all(feature = "video", not(target_arch = "wasm32")))]
mod video_audio;
#[cfg(all(feature = "video", not(target_arch = "wasm32")))]
mod video_decoder;
#[cfg(all(feature = "video", not(target_arch = "wasm32")))]
mod video_transport;
mod video_visual;
#[cfg(target_arch = "wasm32")]
mod web_accessibility;
#[cfg(target_arch = "wasm32")]
mod web_inputs;
#[cfg(target_arch = "wasm32")]
mod web_video;
use crate::project_paths::ProjectPaths;
use crate::resources::{
    CgAssetRegistry, CharacterRegistry, ChoiceFocus, DialogueHistory, GalleryState, ImagemapState,
    LocaleConfig, MenuState, MusicAssetRegistry, MusicEntity, MusicPlaybackState, MusicVolume,
    PersistentDataResource, ProjectTitle, ScriptErrorMessage, Theme, ThemeWatcher,
    TypewriterConfig, TypewriterState, VnEngine, VnRenderState, VnState,
};
use crate::systems::save_menu::SaveMenuState;
use crate::systems::settings_menu::{Settings, SettingsMenuState};
use crate::systems::{
    apply_settings_to_runtime_system, apply_theme_system, audio_fade_system, audio_system,
    audio_unlock_system, background_cover_resize_system, background_system,
    build_character_registry, choice_interaction_system, choice_system,
    cinematic_cover_resize_system, cinematic_system, debug_step_input_system, debug_toggle_system,
    despawn_error_overlay, despawn_gallery_overlay, despawn_history_overlay, despawn_menu_overlay,
    despawn_save_menu_overlay, despawn_settings_menu_overlay, despawn_title_screen,
    dialogue_system, fade_system, gallery_interaction_system, history_input_system,
    imagemap_cleanup_system, imagemap_dimensions_system, imagemap_hover_system, imagemap_system,
    input_system, locale_lang_watch_system, locale_reload_system, menu_input_system,
    menu_interaction_system, persistent_unlock_system, player_input_system,
    save_menu_interaction_system, script_finished_system, settings_menu_interaction_system,
    setup_ui, spawn_error_overlay, spawn_gallery_overlay, spawn_history_overlay,
    spawn_menu_overlay, spawn_or_despawn_debug_overlay_system, spawn_save_menu_overlay,
    spawn_settings_menu_overlay, spawn_title_screen, sprite_animation_system, sprite_system,
    stepping_system, theme_reload_system, title_background_resize_system, title_interaction_system,
    typewriter_config_system, typewriter_system, update_choice_buttons,
    update_debug_overlay_system, update_settings_value_text_system, DebugOverlayState,
    DebugStepRequest,
};
use crate::vn_command::{PlayerInput, VnCommand};
use bevy::prelude::*;
use rvn_core::{
    locale::{collect_strings_from_flat_script, LocaleManager},
    Engine, PersistentData, PersistentDataManager,
};
use rvn_parser::{parse_file_program_with_uses, ParsedProgram, Statement};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::{collections::HashMap, fs};

/// Project configuration loaded from `rvn.toml`.
/// The top‑level table has sections for the project metadata, window size and asset paths.
#[derive(Deserialize)]
struct RvnToml {
    project: ProjectSection,
    #[serde(default)]
    window: WindowSection,
    #[serde(default)]
    paths: PathsSection,
    #[serde(default)]
    string_tables: rvn_core::string_table_project::StringTableDeclarations,
}

#[derive(Deserialize)]
struct ProjectSection {
    /// Title of the visual novel window.
    title: String,
    /// Relative path to the main script file within the project directory.
    #[serde(default = "default_main_script")]
    main_script: String,
    /// Optional start label.
    start_label: Option<String>,
}

fn default_main_script() -> String {
    "scripts/main.rvn".to_string()
}

/// Window configuration such as width and height.  If not provided the engine will
/// use sensible defaults.
#[derive(Deserialize, Default)]
struct WindowSection {
    width: Option<u32>,
    height: Option<u32>,
    resizable: Option<bool>,
}

#[cfg(test)]
mod window_configuration_tests {
    use super::WindowSection;
    #[test]
    fn optional_configs_default_only_when_missing_not_when_invalid() {
        use super::{parse_optional_toml, AppConfig};
        assert_eq!(
            parse_optional_toml::<AppConfig>(None, "config.toml")
                .unwrap()
                .locale
                .current,
            "fr"
        );
        assert!(
            parse_optional_toml::<AppConfig>(Some("[locale]\ncurrent = 42"), "config.toml")
                .err()
                .unwrap()
                .contains("config.toml")
        );
        assert!(parse_optional_toml::<super::Theme>(
            Some("[textbox]\nheight = \"wrong\""),
            "theme.toml"
        )
        .is_err());
        assert!(
            parse_optional_toml::<super::Theme>(Some("[textbox]\nheight = 190"), "theme.toml")
                .is_ok()
        );
    }
    #[test]
    fn typewriter_preference_round_trips_and_legacy_settings_keep_default() {
        let mut data = rvn_core::persistent::PersistentData::default();
        data.typewriter = Some(false);
        let json = serde_json::to_string(&data).unwrap();
        let restored = serde_json::from_str(&json).unwrap();
        assert!(!super::settings_from_persistent(&restored, "fr").typewriter);
        let legacy = serde_json::from_str("{}").unwrap();
        assert!(super::settings_from_persistent(&legacy, "fr").typewriter);
    }
    #[test]
    fn legacy_windows_allow_resize_and_explicit_fixed_windows_remain_supported() {
        let legacy: WindowSection = toml::from_str("width = 1280\nheight = 720\n").unwrap();
        assert!(legacy.resizable.unwrap_or(true));
        let fixed: WindowSection = toml::from_str("resizable = false\n").unwrap();
        assert!(!fixed.resizable.unwrap_or(true));
    }
}

/// Paths to various asset directories and files.
#[derive(Deserialize)]
struct PathsSection {
    /// Path to the asset directory containing backgrounds, sprites, music, etc.
    #[serde(default = "default_assets")]
    assets: String,
    /// Path to the locales directory.
    #[serde(default = "default_locales")]
    locales: String,
    /// Path to the theme file.
    #[serde(default = "default_theme")]
    theme: String,
    /// Path to the saves directory.
    #[serde(default = "default_saves")]
    saves: String,
}

fn default_assets() -> String {
    "assets".to_string()
}
fn default_locales() -> String {
    "locales".to_string()
}
fn default_theme() -> String {
    "theme.toml".to_string()
}
fn default_saves() -> String {
    "saves".to_string()
}

impl Default for PathsSection {
    fn default() -> Self {
        Self {
            assets: default_assets(),
            locales: default_locales(),
            theme: default_theme(),
            saves: default_saves(),
        }
    }
}

/// Application configuration used to load locale settings from `config.toml`.
#[derive(Deserialize, Default)]
struct AppConfig {
    #[serde(default)]
    locale: LocaleConfigFile,
}

fn parse_optional_toml<T: serde::de::DeserializeOwned + Default>(
    source: Option<&str>,
    path: &str,
) -> Result<T, String> {
    match source {
        None => Ok(T::default()),
        Some(source) => toml::from_str(source)
            .map_err(|error| format!("Invalid configuration `{path}`: {error}")),
    }
}

fn load_optional_toml<T: serde::de::DeserializeOwned + Default>(
    path: &std::path::Path,
) -> Result<T, String> {
    let content = match std::fs::read_to_string(path) {
        Ok(content) => Some(content),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => {
            return Err(format!(
                "Cannot read configuration `{}`: {error}",
                path.display()
            ))
        }
    };
    parse_optional_toml(content.as_deref(), &path.display().to_string())
}

#[derive(Deserialize)]
struct LocaleConfigFile {
    #[serde(default = "default_lang")]
    default: String,
    #[serde(default = "default_lang")]
    current: String,
    #[serde(default = "default_available")]
    available: Vec<String>,
}

fn default_lang() -> String {
    "fr".to_string()
}
fn default_available() -> Vec<String> {
    vec!["fr".to_string()]
}

impl Default for LocaleConfigFile {
    fn default() -> Self {
        Self {
            default: default_lang(),
            current: default_lang(),
            available: default_available(),
        }
    }
}

struct RuntimeLaunch<E=Engine<BevyRenderer>> {
    closed_locales: Option<(portable_text_resources::ClosedLocaleRoot,portable_text_resources::LocaleDeclarations)>,
    #[cfg(target_arch = "wasm32")]
    menus: Option<rvn_ui::Document>,
    cfg: RvnToml,
    engine: E,
    locale_cfg: LocaleConfigFile,
    theme: Theme,
    theme_watcher: ThemeWatcher,
    asset_root: String,
    project_paths: ProjectPaths,
    cg_registry: CgAssetRegistry,
    music_registry: MusicAssetRegistry,
    persistent_manager: PersistentDataManager,
    persistent_data: PersistentData,
}

/// Run a visual novel located in a project directory.
///
/// This function loads the configuration from `rvn.toml`, reads the script and
/// theme files, initialises the engine and Bevy app and then blocks while the
/// game runs.  Any I/O or parsing errors are returned as `Err`.
pub fn run_game<P: AsRef<Path>>(project_dir: P) -> Result<(), String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        if std::env::args().skip(2).any(|argument|argument=="--editor-debug-startup"){
            let value=std::env::args().skip(2).find_map(|argument|argument.strip_prefix("--editor-preview=").map(str::to_owned)).ok_or("Debug startup requires its actual authenticated editor preview")?;
            let options=editor_preview::Options::parse(&value)?;
            return run_loaded_game(load_debug_startup(project_dir.as_ref(),&options)?);
        }
        run_loaded_game(load_runtime_game(project_dir.as_ref())?)
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = project_dir;
        Err("Browser projects require the asynchronous run_game_web loader".into())
    }
}

// This transport owns the actual engine/bootstrap; only its pointer-sized
// variants cross the loader Result/RuntimeLaunch ABI. The World still receives
// the concrete resource, and Winit keeps running on the original main thread.
enum RuntimeLaunchOwner { Game(Box<Engine<BevyRenderer>>), Startup(Box<debugger_startup::DebugBootstrap>) }
#[cfg(not(target_arch="wasm32"))]
#[inline(never)]
fn build_runtime_launch_owner(
    program:ParsedProgram,cfg:&RvnToml,label:&str,locale:LocaleManager,
    debugger:bool,persistent:&PersistentData,tables:rvn_core::string_table::StringTableResources,classes:rvn_core::widget_class_source_0950::AdmittedWidgetClasses,enums:rvn_core::enum_runtime_resources1025::EnumAssets1025,libraries:Vec<rvn_parser::function_library_asset1023::FunctionLibraryAsset>,
)->Result<RuntimeLaunchOwner,String>{
    if !libraries.is_empty(){
        debugger_startup::prepare_runtime_with_library_resources1027(program,classes,enums,&libraries,persistent.script_vars.clone(),rvn_core::TextRuntimeContext::new(tables,Some(locale)),cfg.project.start_label.clone())
            .map(|startup|RuntimeLaunchOwner::Startup(Box::new(startup)))
    }else if !enums.is_empty(){
        debugger_startup::prepare_runtime_with_enum_assets1025(program,classes,enums,persistent.script_vars.clone(),rvn_core::TextRuntimeContext::new(tables,Some(locale)),cfg.project.start_label.clone())
            .map(|startup|RuntimeLaunchOwner::Startup(Box::new(startup)))
    }else if !classes.is_empty()||Engine::<BevyRenderer>::program_requires_owned_startup(&program){
        debugger_startup::prepare_runtime_with_classes(program,classes,persistent.script_vars.clone(),rvn_core::TextRuntimeContext::new(tables,Some(locale)),cfg.project.start_label.clone())
            .map(|startup|RuntimeLaunchOwner::Startup(Box::new(startup)))
    }else{
        build_engine(program,cfg,label,locale,debugger,persistent,tables,classes,enums,&libraries)
            .map(|engine|RuntimeLaunchOwner::Game(Box::new(engine)))
    }
}
#[cfg(not(target_arch="wasm32"))]
fn load_runtime_game(project_dir:&Path)->Result<RuntimeLaunch<RuntimeLaunchOwner>,String>{load_runtime_game_text_profile(project_dir,false)}
#[cfg(not(target_arch="wasm32"))]
fn load_runtime_game_text_profile(project_dir:&Path,closed_text:bool)->Result<RuntimeLaunch<RuntimeLaunchOwner>,String>{
    load_game_with_owner(project_dir,closed_text,build_runtime_launch_owner)
}
#[cfg(not(target_arch = "wasm32"))]
fn load_game(project_dir: &Path) -> Result<RuntimeLaunch, String> {
    load_game_text_profile(project_dir,false)
}

// Private preparation profile. Legacy run_game keeps its existing provider;
// the closed profile uses the real project loader before authored Init.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn run_game_closed_text(project_dir:&Path)->Result<(),String> {
    run_loaded_game(load_runtime_game_text_profile(project_dir,true)?)
}
#[cfg(not(target_arch = "wasm32"))]
fn load_game_text_profile(project_dir:&Path,closed_text:bool)->Result<RuntimeLaunch,String> {
    load_game_with_owner(project_dir,closed_text,|script,cfg,label,locale,debugger,persistent,tables,classes,enums,libraries|build_engine(script,cfg,label,locale,debugger,persistent,tables,classes,enums,&libraries))
}
#[cfg(not(target_arch="wasm32"))]
fn load_debug_startup(project_dir:&Path,options:&editor_preview::Options)->Result<RuntimeLaunch<RuntimeLaunchOwner>,String>{
    load_game_with_owner(project_dir,true,|program,cfg,_,locale,_,persistent,tables,classes,enums,libraries|{
        if !libraries.is_empty(){
            return debugger_startup::prepare_with_library_resources1027(project_dir,program,classes,enums,&libraries,options.nonce,persistent.script_vars.clone(),rvn_core::TextRuntimeContext::new(tables,Some(locale)),cfg.project.start_label.clone())
                .map(|startup|RuntimeLaunchOwner::Startup(Box::new(startup)));
        }
        debugger_startup::prepare_with_enum_assets1025(project_dir,program,classes,enums,options.nonce,persistent.script_vars.clone(),rvn_core::TextRuntimeContext::new(tables,Some(locale)),cfg.project.start_label.clone())
            .map(|startup|RuntimeLaunchOwner::Startup(Box::new(startup)))
    })
}
#[cfg(not(target_arch="wasm32"))]
fn load_game_with_owner<E>(project_dir:&Path,closed_text:bool,build:impl FnOnce(ParsedProgram,&RvnToml,&str,LocaleManager,bool,&PersistentData,rvn_core::string_table::StringTableResources,rvn_core::widget_class_source_0950::AdmittedWidgetClasses,rvn_core::enum_runtime_resources1025::EnumAssets1025,Vec<rvn_parser::function_library_asset1023::FunctionLibraryAsset>)->Result<E,String>)->Result<RuntimeLaunch<E>,String>{

    let canonical_project_root=fs::canonicalize(project_dir).map_err(|error|error.to_string())?;
    let project_dir=canonical_project_root.as_path();
    // 1. Parse the rvn.toml file.
    let rvn_toml_path = project_dir.join("rvn.toml");
    let rvn_toml_content = std::fs::read_to_string(&rvn_toml_path).map_err(|e| {
        format!(
            "Impossible de lire le fichier {}: {}",
            rvn_toml_path.display(),
            e
        )
    })?;
    let cfg: RvnToml = toml::from_str(&rvn_toml_content)
        .map_err(|e| format!("Impossible de parser {}: {}", rvn_toml_path.display(), e))?;

    // 2. Determine the script path relative to the project directory.
    // 2. Determine the script path relative to the project directory and resolve `use` directives.
    let script_path = project_dir.join(&cfg.project.main_script);
    let script = parse_file_program_with_uses(&script_path).map_err(|e| {
        format!(
            "Erreur de chargement des scripts depuis `{}`:\n{}",
            script_path.display(),
            e
        )
    })?;

    let widget_classes=editor_widget_preview1027::prepare(project_dir,&rvn_toml_content,&cfg.paths.assets,&script)?;
    let enum_assets1025=editor_enum_current1027::prepare(project_dir,&rvn_toml_content,&cfg.paths.assets,&script)?;
    let library_assets1027=editor_library_current1027::prepare(project_dir,&rvn_toml_content,&script)?;

    // 3. Determine the assets and other paths.
    let assets_dir: PathBuf = project_dir.join(&cfg.paths.assets);
    let locales_dir: PathBuf = project_dir.join(&cfg.paths.locales);
    let theme_path: PathBuf = project_dir.join(&cfg.paths.theme);
    // Build saves directory path relative to project dir
    let saves_dir: PathBuf = if let Some(directory) = std::env::var_os("RVN_QA_OUTPUT") {
        let directory = PathBuf::from(directory);
        if !directory.is_dir() {
            return Err("RVN_QA_OUTPUT must be an existing directory".into());
        }
        directory.join("saves")
    } else {
        project_dir.join(&cfg.paths.saves)
    };

    let table_resources = rvn_core::string_table_project::load_native(&assets_dir, &cfg.string_tables)?;

    // 4. Load locale configuration from config.toml in the assets directory if present.
    let config_toml_path = assets_dir.join("config.toml");
    let app_config: AppConfig = load_optional_toml(&config_toml_path)?;
    let mut locale_cfg = app_config.locale;
    let owned_text=closed_text||!widget_classes.classes().is_empty()||!enum_assets1025.is_empty()||!library_assets1027.is_empty()||Engine::<BevyRenderer>::program_requires_owned_startup(&script);
    let discovered=if owned_text{portable_text_resources::discover_native_locale_langs(&locales_dir)?}else{discover_locale_langs(&locales_dir)};
    for lang in discovered {
        if !locale_cfg
            .available
            .iter()
            .any(|available| available == &lang)
        {
            locale_cfg.available.push(lang);
        }
    }

    let persistent_manager = PersistentDataManager::new(&saves_dir)
        .map_err(|e| format!("[persistent] erreur initialisation : {e}"))?;
    let persistent_data = persistent_manager
        .load()
        .map_err(|e| format!("[persistent] erreur chargement : {e}"))?;
    if let Some(language) = &persistent_data.language {
        locale_cfg.current = language.clone();
    }

    // 5. Initialise the LocaleManager.
    let closed_locales=if owned_text {
        // A catalogue closure is separate from the historical UI language list.
        // Explicit closed/debug profiles retain their original strict policy.
        let declarations=locale_selection::initial_closed_declarations(&locale_cfg,closed_text);
        declarations.validate()?;
        Some((portable_text_resources::closed_native_locale_root_for_launch(&locales_dir,closed_text)?,declarations))
    }else{None};
    let locale_mgr = if let Some((root,declarations))=&closed_locales {
        portable_text_resources::load_closed_native_frozen_locales(root,declarations)?
    }else{LocaleManager::new(
        &locales_dir,
        &locale_cfg.default,
        &locale_cfg.current,
        locale_cfg.available.clone(),
    )
    .map_err(|e| format!("[locale] erreur initialisation : {e}"))?};
    let debugger=std::env::args().skip(2).find_map(|argument|argument.strip_prefix("--editor-preview=").map(str::to_owned))
        .map(|value|editor_preview::Options::parse(&value)).transpose()?.is_some();
    let classes=widget_classes.finish(&script)?;
    let enums=enum_assets1025.finish(&script)?;
    let libraries=library_assets1027.finish(&script)?;
    // Persistent DTO values are inert until the actual project resources admit
    // both identities; refuse before any build callback can execute Init.
    for value in persistent_data.script_vars.values(){rvn_core::enum_runtime_resources1025::validate_value(&enums,value).map_err(|error|error.to_string())?;}
    let engine = build(script, &cfg, &script_path.display().to_string(), locale_mgr,debugger,&persistent_data,table_resources,classes,enums,libraries)?;

    // 9. Load the UI theme and set up the watcher.
    let theme: Theme = load_optional_toml(&theme_path)?;
    let theme_watcher = ThemeWatcher::new(theme_path.to_string_lossy().to_string());

    // 11. Set the Bevy asset root to the configured assets directory.
    let bevy_asset_path = fs::canonicalize(&assets_dir)
        .map_err(|e| {
            format!(
                "Impossible de résoudre le dossier d'assets `{}`: {}",
                assets_dir.display(),
                e
            )
        })?
        .to_string_lossy()
        .to_string();

    // 11a. Build a ProjectPaths resource to make paths accessible to systems.
    let project_paths = ProjectPaths::new(
        project_dir.to_path_buf(),
        project_dir.join(&cfg.paths.assets),
        project_dir.join(&cfg.paths.locales),
        theme_path.clone(),
        saves_dir,
    );
    let cg_registry = build_cg_asset_registry(&assets_dir);
    let music_registry = build_music_asset_registry(&assets_dir);

    Ok(RuntimeLaunch {
        closed_locales,
        #[cfg(target_arch = "wasm32")]
        menus: None,
        cfg,
        engine,
        locale_cfg,
        theme,
        theme_watcher,
        asset_root: bevy_asset_path,
        project_paths,
        cg_registry,
        music_registry,
        persistent_manager,
        persistent_data,
    })
}

fn choice_button_update_systems() -> bevy::ecs::schedule::SystemConfigs {
    // Dialogue queues recursive choice removal. Apply that deferred cleanup
    // before refreshing buttons, so a choice -> dialogue transition cannot
    // queue a second despawn for the same entities.
    update_choice_buttons
        .after(dialogue_system)
        .after(choice_system)
}

trait LaunchOwner{
    fn locale(&self)->Option<&LocaleManager>;
    fn install(self,app:&mut App);
    fn startup(&self)->bool{false}
    fn debugger_profile(&self)->bool{false}
}
impl LaunchOwner for Engine<BevyRenderer>{
    fn locale(&self)->Option<&LocaleManager>{self.locale.as_ref()}
    fn install(self,app:&mut App){app.insert_resource(VnEngine(self));}
}
impl LaunchOwner for debugger_startup::DebugBootstrap{
    fn locale(&self)->Option<&LocaleManager>{self.owner.locale()}
    fn install(self,app:&mut App){app.insert_resource(self);}
    fn startup(&self)->bool{true}
    fn debugger_profile(&self)->bool{self.profile==debugger_startup::BootstrapProfile::Debug}
}
impl LaunchOwner for RuntimeLaunchOwner{
    fn locale(&self)->Option<&LocaleManager>{match self{Self::Game(engine)=>engine.locale.as_ref(),Self::Startup(startup)=>startup.owner.locale()}}
    fn install(self,app:&mut App){match self{Self::Game(engine)=>{app.insert_resource(VnEngine(*engine));},Self::Startup(startup)=>{app.insert_resource(*startup);}}}
    fn startup(&self)->bool{matches!(self,Self::Startup(_))}
    fn debugger_profile(&self)->bool{matches!(self,Self::Startup(startup) if startup.profile==debugger_startup::BootstrapProfile::Debug)}
}
#[cfg(test)]
mod runtime_launch_heap_tests_0922 {
    use super::*;
    #[test]
    fn launch_transport_has_bounded_inline_abi_for_both_real_owner_variants(){
        assert!(std::mem::size_of::<RuntimeLaunchOwner>()<=2*std::mem::size_of::<usize>());
        assert!(std::mem::size_of::<Result<RuntimeLaunch<RuntimeLaunchOwner>,String>>()
            <std::mem::size_of::<RuntimeLaunch<debugger_startup::DebugBootstrap>>());
    }
    #[test]
    fn heap_transport_installs_the_same_concrete_game_owner(){
        let engine=Engine::new(rvn_parser::parse("label start\n\"ready\"\n").unwrap(),BevyRenderer::new(),8).unwrap();
        let instance=engine.instance_id();let epoch=engine.runtime_epoch();
        let gate=engine.renderer.audio_checkpoints.gate();
        let transport=RuntimeLaunchOwner::Game(Box::new(engine));
        assert!(!transport.startup());assert!(!transport.debugger_profile());
        let mut app=App::new();transport.install(&mut app);
        assert!(!app.world().contains_resource::<debugger_startup::DebugBootstrap>());
        let installed=&app.world().resource::<VnEngine>().0;
        assert_eq!(installed.instance_id(),instance);assert_eq!(installed.runtime_epoch(),epoch);
        assert!(installed.renderer.audio_checkpoints.gate().same_gate(&gate));
    }
}
fn run_loaded_game<E:LaunchOwner>(launch: RuntimeLaunch<E>) -> Result<(), String> {
    let RuntimeLaunch {
        closed_locales,
        #[cfg(target_arch = "wasm32")]
        menus,
        cfg,
        engine,
        locale_cfg,
        theme,
        theme_watcher,
        asset_root,
        project_paths,
        cg_registry,
        music_registry,
        persistent_manager,
        persistent_data,
    } = launch;
    #[cfg(not(target_arch="wasm32"))]
    let mut preview=std::env::args().skip(2).find_map(|argument|argument.strip_prefix("--editor-preview=").map(str::to_owned))
        .map(|value|editor_preview::Options::parse(&value)).transpose()?;
    #[cfg(target_arch="wasm32")]
    let mut preview:Option<editor_preview::Options>=None;
    if let Some(preview)=preview.as_mut(){
        preview.width=cfg.window.width.unwrap_or(1280);preview.height=cfg.window.height.unwrap_or(720);
        rvn_core::editor_preview::image_bytes(preview.width,preview.height)?;
    }
    let window_width = preview.as_ref().map(|value|value.width).unwrap_or(cfg.window.width.unwrap_or(1280)) as f32;
    let window_height = preview.as_ref().map(|value|value.height).unwrap_or(cfg.window.height.unwrap_or(720)) as f32;
    let startup=engine.startup();
    let debugger_profile=engine.debugger_profile();
    let ts_current = if let Some(locale) = engine.locale() {
        locale
            .locale_path(&locale_cfg.current)
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
    } else {
        std::time::SystemTime::UNIX_EPOCH
    };
    let ts_default = if let Some(locale) = engine.locale() {
        locale
            .locale_path(&locale_cfg.default)
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
    } else {
        std::time::SystemTime::UNIX_EPOCH
    };
    let locale_config_res = LocaleConfig {
        available_langs: locale_cfg.available.clone(),
        last_modified_current: ts_current,
        last_modified_default: ts_default,
    };
    let settings = settings_from_persistent(&persistent_data, &locale_cfg.current);
    let persistent_resource = PersistentDataResource {
        manager: persistent_manager,
        data: persistent_data,
    };

    let mut app = App::new();
    if let Some((root,_))=&closed_locales {portable_text_resources::register_verified_text_sources(&mut app,asset_root.clone(),root.clone())?;}
    #[cfg(target_arch = "wasm32")]
    app.insert_resource(menu_documents::web_menus(menus)?);
    #[cfg(not(target_arch = "wasm32"))]
    if !debugger_profile{app.add_plugins(qa_playthrough::QaPlugin);}
    #[cfg(not(target_arch = "wasm32"))]
    if !debugger_profile { app.add_plugins(menu_preview_data::MenuPreviewDataPlugin); }
    let mut plugins = DefaultPlugins
            .set(AssetPlugin {
                file_path: asset_root,
                watch_for_changes_override: Some(true),
                ..default()
            })
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: cfg.project.title.clone(),
                    resolution: (window_width, window_height).into(),
                    resizable: cfg.window.resizable.unwrap_or(true),
                    ..default()
                }),
                exit_condition: if preview.is_some(){bevy::window::ExitCondition::DontExit}else{bevy::window::ExitCondition::OnAllClosed},
                ..default()
            });
    #[cfg(not(target_arch="wasm32"))]
    if preview.is_some(){plugins=plugins.disable::<bevy::winit::WinitPlugin>();}
    app.add_plugins(plugins);
    engine.install(&mut app);
    app
    // Resources
    .insert_resource(theme)
    .insert_resource(ProjectTitle(cfg.project.title.clone()))
    .insert_resource(resources::NarrativeStart(cfg.project.start_label.clone()))
    .insert_resource(theme_watcher)
    .insert_resource(VnRenderState::default())
    .insert_resource(crate::resources::SkipMode::default())
    .insert_resource(ImagemapState::default())
    .insert_resource(MusicEntity::default())
    .insert_resource(MusicPlaybackState::default())
    .insert_resource(MusicVolume::default())
    .insert_resource(TypewriterConfig::default())
    .insert_resource(TypewriterState::default())
    .insert_resource(MenuState::default())
    .init_resource::<editor_preview_input::Receipts>()
    .insert_resource(GalleryState::default())
    .insert_resource(CharacterRegistry::default())
    .insert_resource(cg_registry)
    .insert_resource(music_registry)
    .insert_resource(DialogueHistory::default())
    .insert_resource(ChoiceFocus::default())
    .insert_resource(ScriptErrorMessage::default())
    .insert_resource(locale_config_res)
    // Insert project path resource for save/load operations
    // Insert settings resource and UI states
    .insert_resource(settings)
    .insert_resource(SettingsMenuState::default())
    .insert_resource(SaveMenuState::default())
    .insert_resource(persistent_resource)
    // Save menu and project paths
    .insert_resource(project_paths)
    // Ressources de debug : état (visible / caché) et requête de pas-à-pas
    .insert_resource(DebugOverlayState::default())
    .insert_resource(DebugStepRequest::default())
    // Events & States
    .add_plugins(menu_documents::MenuDocumentsPlugin)
    .add_plugins(source_menus::SourceMenusPlugin)
    .add_plugins(checkpoint_requests::CheckpointRequestsPlugin)
        .add_plugins(display_reveal::DialogueRevealPlugin)
        .add_plugins((scene_publication::ScenePublicationPlugin,scene_effects::SceneEffectsPlugin,scene_retired::SceneRetiredPlugin,scene_audio::SceneAudioPlugin,web_music::WebMusicPlugin,presentation_order::PresentationOrderPlugin))
    .add_plugins(programmable_ui::ProgrammableUiPlugin)
    .add_plugins(custom_canvas::CustomCanvasPlugin)
    .add_plugins(accessibility::AccessibilityPlugin)
    .add_plugins(composed_motion::ComposedMotionPlugin)
    .add_plugins(layered_characters::LayeredCharactersPlugin)
    .add_plugins(portable_images::PortableImagesPlugin)
    .add_plugins(string_table_assets::StringTableAssetsPlugin::new(cfg.string_tables.clone()))
    .add_event::<VnCommand>()
    .add_event::<PlayerInput>()
    .init_state::<VnState>()
    ;
    register_runtime_systems(&mut app);
    if startup && !debugger_profile { app.world_mut().resource_mut::<NextState<VnState>>().set(VnState::Stepping); }
    if let Some((_,declarations))=closed_locales {app.add_plugins(portable_text_resources::ClosedLocaleAssetsPlugin::new(declarations)?);}
    #[cfg(all(feature = "video", not(target_arch = "wasm32")))]
    app.add_plugins(native_video::NativeVideoPlugin);
    #[cfg(target_arch = "wasm32")]
    app.add_plugins(web_video::WebVideoPlugin);
    #[cfg(not(target_arch="wasm32"))]
    if let Some(options)=preview {
        let instance=text_runtime_owner::current(app.world())?.instance;
        editor_preview::install(&mut app,options,instance)?;
        app.add_plugins(bevy::app::ScheduleRunnerPlugin::run_loop(std::time::Duration::from_secs_f64(1.0/60.0)));
        app.world_mut().resource_mut::<NextState<VnState>>().set(VnState::Stepping);
    }
    run_runtime_app(app)
}

/// Registers the actual runtime systems for both normal and Startup launch.
/// Keep the CPU harness on this same route; no duplicated predicate registry.
fn register_runtime_systems(app: &mut App) {
    app.add_plugins(debugger_startup::native_imagemap::NativeInputPlugin);
    app.init_resource::<runtime_restart::Requests>().add_systems(First, (runtime_restart::activate,debugger_startup::driver::drain_runtime).chain().before(scene_publication::consume).before(bevy::time::TimeSystem));
    app.add_systems(First,editor_debugger::drive_widget_materialization
        .run_if(|output:Option<Res<editor_preview::Output>>|output.is_none())
        .after(runtime_restart::activate).after(debugger_startup::driver::drain_runtime)
        .before(scene_publication::consume).before(bevy::time::TimeSystem));
    app
    .add_systems(PreUpdate, runtime_restart::capture_suppressed_events
        .after(bevy::input::InputSystem)
        .before(programmable_ui::UiInputCaptureSet)
        .before(bevy::ui::UiSystem::Focus))
    .add_systems(PreUpdate, runtime_restart::quarantine_pointer_presses
        .after(bevy::ui::UiSystem::Focus)
        .before(accessibility::AccessibilityInput)
        .before(programmable_ui::InterfaceFocusSet))
    // Startup systems
    .add_systems(Startup, ((setup_camera, setup_ui, build_character_registry)).distributive_run_if(crate::runtime_restart::presentation_available))
    // State transition systems
    .add_systems(OnEnter(VnState::TitleScreen), (spawn_title_screen).distributive_run_if(crate::runtime_restart::presentation_available))
    .add_systems(OnExit(VnState::TitleScreen), (despawn_title_screen).distributive_run_if(crate::runtime_restart::presentation_available))
    .add_systems(OnExit(VnState::Gallery), (despawn_gallery_overlay).distributive_run_if(crate::runtime_restart::presentation_available))
    .add_systems(OnEnter(VnState::Menu), (spawn_menu_overlay).distributive_run_if(crate::runtime_restart::presentation_available))
    .add_systems(OnExit(VnState::Menu), (despawn_menu_overlay).distributive_run_if(crate::runtime_restart::presentation_available))
    .add_systems(OnEnter(VnState::History), (spawn_history_overlay).distributive_run_if(crate::runtime_restart::presentation_available))
    .add_systems(OnExit(VnState::History), (despawn_history_overlay).distributive_run_if(crate::runtime_restart::presentation_available))
    .add_systems(OnEnter(VnState::Error), (spawn_error_overlay).distributive_run_if(crate::runtime_restart::presentation_available))
    .add_systems(OnExit(VnState::Error), (despawn_error_overlay).distributive_run_if(crate::runtime_restart::presentation_available))
    // Update systems
    .add_systems(
        Update,
        ((
            systems::setup::synchronize_character_registry
                .after(stepping_system)
                .before(dialogue_system)
                .before(apply_settings_to_runtime_system),
            theme_reload_system,
            apply_theme_system,
            locale_reload_system.run_if(resource_exists::<VnEngine>),     // ← hot-reload locales
            locale_lang_watch_system.run_if(resource_exists::<VnEngine>), // ← surveille __lang variable
            title_interaction_system.run_if(runtime_restart::input_available).run_if(resource_exists::<VnEngine>).run_if(scene_publication::ready).run_if(in_state(VnState::TitleScreen)),
            title_background_resize_system.run_if(in_state(VnState::TitleScreen)),
            spawn_gallery_overlay.run_if(resource_exists::<VnEngine>).run_if(in_state(VnState::Gallery)),
            gallery_interaction_system.run_if(runtime_restart::input_available).run_if(in_state(VnState::Gallery)),
            stepping_system.run_if(resource_exists::<VnEngine>).run_if(scene_publication::preview_statement_ready).run_if(in_state(VnState::Stepping)),
            (
                background_system,
                background_cover_resize_system.after(background_system).after(composed_motion::receive),
                sprite_system,
                systems::sprite::resize_sprite_stage,
                cinematic_system,
                cinematic_cover_resize_system.after(cinematic_system),
                dialogue_system.after(typewriter_config_system),
                choice_system,
                imagemap_system,
                audio_system,
                audio_unlock_system
                    .after(audio_system)
                    .after(title_interaction_system),
                typewriter_config_system,
                script_finished_system,
            )
                .after(stepping_system),
            imagemap_dimensions_system,
            imagemap_hover_system,
            imagemap_cleanup_system.run_if(not(in_state(VnState::Waiting))),
            choice_button_update_systems(),
            sprite_animation_system.run_if(scene_publication::ready).in_set(scene_publication::HostClockSet),
            fade_system.run_if(scene_publication::ready).run_if(in_state(VnState::Animating)).in_set(scene_publication::HostClockSet),
            audio_fade_system.run_if(scene_publication::ready).in_set(scene_publication::HostClockSet),
        )).distributive_run_if(crate::runtime_restart::presentation_available),
    )
    .add_systems(
        Update,
        ((
            typewriter_system.run_if(scene_publication::story_ready).run_if(resource_exists::<VnEngine>.and_then(scene_publication::story_reveal_allowed)).run_if(in_state(VnState::Waiting))
                .after(scene_publication::GameFrameSet).before(scene_publication::MotionFeedbackSet).before(stepping_system),
            systems::input::story_keyboard_systems().distributive_run_if(runtime_restart::input_available).run_if(resource_exists::<VnEngine>),
            choice_interaction_system.run_if(runtime_restart::input_available).run_if(resource_exists::<VnEngine>).run_if(scene_publication::story_ready).run_if(in_state(VnState::Waiting)),
            menu_input_system.run_if(runtime_restart::input_available).run_if(in_state(VnState::Menu)),
            menu_interaction_system.run_if(runtime_restart::input_available).run_if(scene_publication::ready).run_if(in_state(VnState::Menu)),
            history_input_system.run_if(runtime_restart::input_available).run_if(in_state(VnState::History)),
            player_input_system.run_if(runtime_restart::input_available).run_if(resource_exists::<VnEngine>).run_if(scene_publication::ready)
                .after(input_system)
                .after(systems::input::finished_quick_load_system)
                .after(choice_interaction_system)
                .after(menu_input_system)
                .after(history_input_system),
        )).distributive_run_if(crate::runtime_restart::presentation_available),
    )
    .add_systems(
        Update,
        ((
            spawn_save_menu_overlay.run_if(resource_exists::<VnEngine>).run_if(in_state(VnState::Menu)),
            save_menu_interaction_system.run_if(runtime_restart::input_available).run_if(resource_exists::<VnEngine>).run_if(scene_publication::ready).run_if(in_state(VnState::Menu)),
            despawn_save_menu_overlay,
            spawn_settings_menu_overlay.run_if(in_state(VnState::Menu)),
            settings_menu_interaction_system.run_if(runtime_restart::input_available).run_if(in_state(VnState::Menu)),
            update_settings_value_text_system.run_if(in_state(VnState::Menu)),
            apply_settings_to_runtime_system,
            despawn_settings_menu_overlay,
        )).distributive_run_if(crate::runtime_restart::presentation_available),
    )
    .add_systems(PostUpdate, (persistent_unlock_system).distributive_run_if(crate::runtime_restart::presentation_available))
    .add_systems(Update, (update_debug_overlay_system).distributive_run_if(crate::runtime_restart::presentation_available))
    // Debug overlay : toggling, spawn/despawn et gestion du step F10
    .add_systems(
        Update,
        ((
            debug_toggle_system.run_if(runtime_restart::input_available),
            spawn_or_despawn_debug_overlay_system,
            debug_step_input_system.run_if(runtime_restart::input_available).run_if(scene_publication::story_ready),
        )).distributive_run_if(crate::runtime_restart::presentation_available),
    );
}

fn run_runtime_app(mut app:App)->Result<(),String> {
    #[cfg(not(target_arch="wasm32"))]
    editor_preview::trace_once(0,format_args!("Production runner entered before actual app.run"));
    match app.run() {
        bevy::app::AppExit::Success=>Ok(()),
        bevy::app::AppExit::Error(code)=>Err(format!("Runtime exited with error code {}",code.get())),
    }
}

#[cfg(test)]
mod runtime_exit_tests {
    use super::*;
    #[test]
    fn runtime_propagates_success_and_failure_emitted_by_the_real_app_runner() {
        let mut successful=App::new();
        successful.add_systems(Update,|mut exit:EventWriter<bevy::app::AppExit>|{exit.send(bevy::app::AppExit::Success);});
        assert_eq!(run_runtime_app(successful),Ok(()));
        let mut failing=App::new();
        failing.add_systems(Update,|mut exit:EventWriter<bevy::app::AppExit>|{exit.send(bevy::app::AppExit::Error(std::num::NonZeroU8::new(7).unwrap()));});
        assert_eq!(run_runtime_app(failing),Err("Runtime exited with error code 7".into()));
    }
}

fn build_engine(
    script: ParsedProgram,
    cfg: &RvnToml,
    script_label: &str,
    locale_mgr: LocaleManager,
    debugger:bool,
    persistent:&PersistentData,
    table_resources:rvn_core::string_table::StringTableResources,
    classes:rvn_core::widget_class_source_0950::AdmittedWidgetClasses,
    enums:rvn_core::enum_runtime_resources1025::EnumAssets1025,
    libraries:&[rvn_parser::function_library_asset1023::FunctionLibraryAsset],
) -> Result<Engine<BevyRenderer>, String> {
    let renderer = BevyRenderer::new();
    let text_context = rvn_core::TextRuntimeContext::new(table_resources, Some(locale_mgr));
    let mut engine = (if !libraries.is_empty(){Engine::from_library_enum_program1027(script,libraries,enums,classes,renderer,64,persistent.script_vars.clone(),text_context)}else if !enums.is_empty(){Engine::from_program_with_enum_assets1025(script,enums,classes,renderer,64,persistent.script_vars.clone(),text_context)}else if !classes.is_empty(){Engine::from_program_with_widget_classes_and_text_context(script,classes,renderer,64,persistent.script_vars.clone(),text_context)}else if debugger{Engine::from_program_with_debugger_and_text_context(script,renderer,64,persistent.script_vars.clone(),text_context)}else{Engine::from_program_with_persistent_and_text_context(script,renderer,64,persistent.script_vars.clone(),text_context)})
        .map_err(|e| format!("Erreur d'initialisation du moteur: {e}"))?;

    if let Some(ref start_label) = cfg.project.start_label {
        engine.enter_label(start_label).map_err(|error|
            format!("start_label `{start_label}` dans `{script_label}` : {error}"))?;
    }

    let strings = collect_strings_from_flat_script(&engine.script);
    if let Some(locale) = &mut engine.locale {
        if locale.provider_policy()!=rvn_core::locale::LocaleProviderPolicy::FrozenCatalogs {
            let _ = locale.update_default_locale(&strings);
        }
    }

    Ok(engine)
}

pub fn run_game_from_path<P: AsRef<Path>>(path: P) -> Result<(), String> {
    run_game(path)
}

#[cfg(target_arch = "wasm32")]
#[derive(Deserialize)]
struct WebScriptManifest {
    scripts: Vec<String>,
}

#[cfg(target_arch = "wasm32")]
#[derive(Deserialize, Default)]
struct WebAssetManifest {
    #[serde(default)]
    has_config_toml: bool,
    #[serde(default)]
    cgs: HashMap<String, String>,
    #[serde(default)]
    music: HashMap<String, String>,
    #[serde(default)]
    locales: HashMap<String, String>,
    #[serde(default)]
    string_tables: Option<rvn_core::string_table_project::StringTableDeclarations>,
}

#[cfg(target_arch = "wasm32")]
pub async fn run_game_web(project_root: &str) -> Result<(), String> {
    let root = project_root.trim_end_matches('/').trim_end_matches('.');
    let root = if root.is_empty() { "." } else { root };
    let rvn_toml_content = fetch_text(&format!("{root}/rvn.toml")).await?;
    let cfg: RvnToml = toml::from_str(&rvn_toml_content)
        .map_err(|e| format!("Impossible de parser rvn.toml: {e}"))?;

    let manifest_url = format!("{root}/rvn_web_manifest.toml");
    let manifest = match fetch_optional_configuration(&manifest_url).await? {
        Some(content) => {
            toml::from_str::<WebScriptManifest>(&content)
                .map_err(|error| format!("Invalid configuration `{manifest_url}`: {error}"))?
                .scripts
        }
        None => vec![cfg.project.main_script.clone()],
    };
    let script = load_web_scripts(root, &cfg.project.main_script, &manifest).await?;

    let assets_dir = PathBuf::from(&cfg.paths.assets);
    let locales_dir = PathBuf::from(&cfg.paths.locales);
    let saves_dir = PathBuf::from(&cfg.paths.saves);
    let theme_path = PathBuf::from(&cfg.paths.theme);
    let manifest_url = format!("{root}/rvn_web_assets.toml");
    let manifest_content = fetch_optional_configuration(&manifest_url).await?;
    let asset_manifest: WebAssetManifest =
        parse_optional_toml(manifest_content.as_deref(), &manifest_url)?;

    rvn_core::string_table_project::validate_web_manifest(&cfg.string_tables, asset_manifest.string_tables.as_ref())?;
    let mut table_preparation = rvn_core::string_table_project::StringTablePreparation::new(&cfg.string_tables)?;
    for (id, relative) in &cfg.string_tables {
        let assets = cfg.paths.assets.trim_end_matches(['/', '\\']).replace('\\', "/");
        let path = format!("{root}/{}", rvn_core::string_table_project::url_path(&format!("{assets}/{relative}")));
        let content = fetch_text(&path).await?;
        table_preparation.supply(id, content.as_bytes()).map_err(|error| format!("String table {relative}: {error}"))?;
    }
    let table_resources = table_preparation.finish()?;

    let app_config = if asset_manifest.has_config_toml {
        let path = format!("{root}/{}/config.toml", cfg.paths.assets);
        let content = fetch_text(&path).await?;
        parse_optional_toml::<AppConfig>(Some(&content), &path)?
    } else {
        AppConfig::default()
    };
    let mut locale_cfg = app_config.locale;
    let mut manifest_locales: Vec<String> = asset_manifest.locales.keys().cloned().collect();
    manifest_locales.sort();
    for lang in manifest_locales {
        if !locale_cfg
            .available
            .iter()
            .any(|available| available == &lang)
        {
            locale_cfg.available.push(lang);
        }
    }

    let persistent_manager = PersistentDataManager::new(&saves_dir)
        .map_err(|e| format!("[persistent] erreur initialisation : {e}"))?;
    let persistent_data = persistent_manager
        .load()
        .map_err(|e| format!("[persistent] erreur chargement : {e}"))?;
    if let Some(language) = &persistent_data.language {
        locale_cfg.current = language.clone();
    }

    let mut locale_tables = HashMap::new();
    for lang in &locale_cfg.available {
        let locale_path = asset_manifest
            .locales
            .get(lang)
            .cloned()
            .unwrap_or_else(|| format!("{}/{}.toml", cfg.paths.locales, lang));
        let path = format!("{root}/{locale_path}");
        if let Some(content) = fetch_optional_text(&path).await {
            match rvn_core::locale::LocaleCatalog::parse(lang, &content) {
                Ok(table) => {
                    locale_tables.insert(lang.clone(), table);
                }
                Err(e) => bevy::log::warn!("[locale] {path} ignoré: {e}"),
            }
        }
    }
    let locale_mgr = LocaleManager::from_catalogs(
        &locales_dir,
        &locale_cfg.default,
        &locale_cfg.current,
        locale_cfg.available.clone(),
        locale_tables,
    );
    let engine = build_engine(script, &cfg, &cfg.project.main_script, locale_mgr,false,&persistent_data,table_resources,Default::default(),Default::default(),&[])?;

    let raw_config: toml::Value =
        toml::from_str(&rvn_toml_content).map_err(|e| format!("Configuration : {e}"))?;
    let menu_source = if let Some(path) = raw_config
        .get("paths")
        .and_then(|p| p.get("menus"))
        .and_then(toml::Value::as_str)
    {
        Some(fetch_text(&format!("{root}/{path}")).await?)
    } else {
        fetch_optional_text(&format!("{root}/menus.rvnui")).await
    };
    let menus = if let Some(source) = menu_source {
        let doc = rvn_ui::Document::from_json(&source)?;
        doc.validate()?;
        Some(doc)
    } else {
        None
    };
    let theme_url = format!("{root}/{}", cfg.paths.theme);
    let theme_content = fetch_optional_configuration(&theme_url).await?;
    let theme: Theme = parse_optional_toml(theme_content.as_deref(), &theme_url)?;
    let theme_watcher = ThemeWatcher::new(cfg.paths.theme.clone());
    let cg_registry = CgAssetRegistry(asset_manifest.cgs);
    prefetch_audio_files(root, &asset_manifest.music).await;
    let music_registry = MusicAssetRegistry(asset_manifest.music);

    let project_paths = ProjectPaths::new(
        PathBuf::from("."),
        assets_dir,
        locales_dir,
        theme_path,
        saves_dir,
    );

    let bevy_asset_path = string_table_assets::http_asset_root(root, &cfg.paths.assets);
    run_loaded_game(RuntimeLaunch {
        closed_locales:None,
        menus,
        cfg,
        engine,
        locale_cfg,
        theme,
        theme_watcher,
        asset_root: bevy_asset_path,
        project_paths,
        cg_registry,
        music_registry,
        persistent_manager,
        persistent_data,
    })
}

#[cfg(target_arch = "wasm32")]
async fn load_web_scripts(
    root: &str,
    main_script: &str,
    manifest: &[String],
) -> Result<ParsedProgram, String> {
    let mut sources = HashMap::new();
    let mut files = manifest.to_vec();
    if !files.iter().any(|path| path == main_script) {
        files.push(main_script.to_string());
    }
    files.sort();
    files.dedup();

    for path in &files {
        let content = fetch_text(&format!("{root}/{path}")).await?;
        sources.insert(normalize_web_path(path), content);
    }

    resolve_web_program(main_script, &sources, &files)
}

// The browser uses immutable fetched bytes and the same whole-program context
// as the desktop loader. Local-containing imports are not parsed as legacy
// fragments, and every resolved statement keeps its original file/span.
fn resolve_web_program(main_script: &str, sources: &HashMap<String, String>, manifest: &[String]) -> Result<ParsedProgram, String> {
    let mut loaded = std::collections::HashSet::new();
    let mut stack = Vec::new();
    let mut order = Vec::new();
    let mut files = Vec::new();
    discover_web_script(main_script, sources, manifest, &mut loaded, &mut stack, &mut files, &mut order)?;
    let mut parsed = HashMap::new();
    let context = if order.iter().any(|(_, _, statement)| matches!(statement, Statement::SceneLabel { .. })) {
        rvn_parser::NarrativeContext::ExplicitScenes
    } else { rvn_parser::NarrativeContext::Legacy };
    for path in &files {
        let source = &sources[path];
        parsed.insert(path.clone(), rvn_parser::parse_with_origins(source, context)
            .map_err(|error| format!("Erreur de parsing dans `{path}` : {error}"))?);
    }
    let mut script = Vec::new();
    let mut origins = Vec::new();
    for (path, index, _) in order {
        let file = &parsed[&path];
        script.push(file.script[index].clone());
        origins.push(rvn_parser::StatementOrigin { source: Some(PathBuf::from(&path)), span: file.spans[index].clone() });
    }
    Ok(ParsedProgram { script, context, origins, resolution: None, sources: files.into_iter().map(|path|
        rvn_parser::ProgramSource { source: sources[&path].clone(), path: Some(PathBuf::from(path)) }).collect() })
}

#[cfg(test)]
mod narrative_program_consumer_tests {
    use super::*;
    #[test]
    fn browser_resolver_uses_complete_scene_context_and_preserves_exact_import_origins() {
        let main="use \"chapter.rvn\"\r\nscene label start\r\nlocal score=2\r\ncall chapter\r\n\"[score]\"\r\n";
        let chapter="scene label chapter\nlocal score=7\n\"Été [score]\"\nreturn\n";
        let sources=HashMap::from([("main.rvn".into(),main.into()),("chapter.rvn".into(),chapter.into())]);
        let program=resolve_web_program("main.rvn",&sources,&["chapter.rvn".into(),"main.rvn".into()]).unwrap();
        assert_eq!(program.context,rvn_parser::NarrativeContext::ExplicitScenes);
        assert_eq!(program.sources.iter().map(|source|source.source.as_str()).collect::<Vec<_>>(),vec![main,chapter]);
        assert!(program.origins.iter().any(|origin|origin.source.as_deref()==Some(std::path::Path::new("chapter.rvn")) && chapter[origin.span.range.clone()].starts_with("local score")));
        let mut game=rvn_core::Engine::from_program(program,crate::bevy_renderer::BevyRenderer::new(),32).unwrap();
        game.enter_label("start").unwrap();
        assert!(matches!(game.step_until_interaction().unwrap(),Some(rvn_core::Interaction::Dialogue{text,..}) if text=="Été 7"));
        let first=game.state.scene_execution.as_ref().unwrap().owner();
        game.advance_dialogue().unwrap();
        assert!(matches!(game.step_until_interaction().unwrap(),Some(rvn_core::Interaction::Dialogue{text,..}) if text=="2"));
        assert_ne!(game.state.scene_execution.as_ref().unwrap().owner(),first);
    }
    #[test]
    fn browser_resolver_refuses_legacy_local_and_import_cycle_without_creating_an_engine() {
        for sources in [
            HashMap::from([("main.rvn".into(),"use \"fragment.rvn\"\nlabel start\n\"legacy\"".into()),("fragment.rvn".into(),"local score=2".into())]),
            HashMap::from([("main.rvn".into(),"use \"fragment.rvn\"\nscene label start\n\"scene\"".into()),("fragment.rvn".into(),"use \"main.rvn\"".into())]),
        ] {assert!(resolve_web_program("main.rvn",&sources,&["main.rvn".into(),"fragment.rvn".into()]).is_err());}
    }
}
fn discover_web_script(
    path: &str,
    sources: &HashMap<String, String>,
    manifest: &[String],
    loaded: &mut std::collections::HashSet<String>,
    stack: &mut Vec<String>,
    files: &mut Vec<String>,
    order: &mut Vec<(String, usize, Statement)>,
) -> Result<(), String> {
    let path = normalize_web_path(path);
    if let Some(pos) = stack.iter().position(|p| p == &path) {
        let mut cycle = stack[pos..].to_vec();
        cycle.push(path);
        return Err(format!("cycle de `use` détecté: {}", cycle.join(" -> ")));
    }
    if !loaded.insert(path.clone()) {
        return Ok(());
    }

    let source = sources
        .get(&path)
        .ok_or_else(|| format!("fichier RVN web introuvable `{path}`"))?;
    stack.push(path.clone());
    files.push(path.clone());
    let script =
        rvn_parser::parse_for_context_discovery(source).map_err(|e| format!("Erreur de parsing dans `{path}`:\n{e}"))?;
    let base_dir = path.rsplit_once('/').map(|(dir, _)| dir).unwrap_or("");

    for (index, stmt) in script.into_iter().enumerate() {
        match stmt {
            Statement::Use { paths } => {
                for use_path in paths {
                    for target in expand_web_use(base_dir, &use_path, manifest)? {
                        discover_web_script(&target, sources, manifest, loaded, stack, files, order)?;
                    }
                }
            }
            other => order.push((path.clone(), index, other)),
        }
    }

    stack.pop();
    Ok(())
}

fn expand_web_use(base_dir: &str, raw: &str, manifest: &[String]) -> Result<Vec<String>, String> {
    if raw.ends_with("/*") || raw.ends_with("/*.rvn") {
        let dir_part = raw
            .strip_suffix("/*.rvn")
            .or_else(|| raw.strip_suffix("/*"))
            .unwrap_or(raw);
        let dir = join_web_path(base_dir, dir_part);
        let prefix = if dir.is_empty() {
            String::new()
        } else {
            format!("{dir}/")
        };
        let mut files: Vec<String> = manifest
            .iter()
            .map(|path| normalize_web_path(path))
            .filter(|path| path.starts_with(&prefix) && path.ends_with(".rvn"))
            .collect();
        files.sort();
        if files.is_empty() {
            return Err(format!(
                "aucun fichier `.rvn` trouvé pour le `use` wildcard `{dir}`"
            ));
        }
        return Ok(files);
    }

    Ok(vec![join_web_path(base_dir, raw)])
}

fn join_web_path(base_dir: &str, raw: &str) -> String {
    let joined = if base_dir.is_empty() {
        raw.to_string()
    } else {
        format!("{base_dir}/{raw}")
    };
    normalize_web_path(&joined)
}

fn normalize_web_path(path: &str) -> String {
    let mut parts = Vec::new();
    let normalized = path.replace('\\', "/");
    for part in normalized.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            part => parts.push(part),
        }
    }
    parts.join("/")
}

#[cfg(target_arch = "wasm32")]
async fn fetch_optional_text(url: &str) -> Option<String> {
    fetch_text(url).await.ok()
}

#[cfg(target_arch = "wasm32")]
async fn prefetch_audio_files(root: &str, music: &HashMap<String, String>) {
    let mut paths: Vec<&String> = music.values().collect();
    paths.sort();
    paths.dedup();

    for path in paths {
        let url = format!("{root}/assets/{path}");
        if let Err(e) = fetch_bytes_for_cache(&url).await {
            bevy::log::warn!("[music] prefetch ignoré pour {url}: {e}");
        }
    }
}

#[cfg(target_arch = "wasm32")]
async fn fetch_bytes_for_cache(url: &str) -> Result<(), String> {
    use wasm_bindgen::JsCast;

    let window = web_sys::window().ok_or_else(|| "window indisponible".to_string())?;
    let response_value = wasm_bindgen_futures::JsFuture::from(window.fetch_with_str(url))
        .await
        .map_err(|_| format!("impossible de charger `{url}`"))?;
    let response: web_sys::Response = response_value
        .dyn_into()
        .map_err(|_| format!("réponse HTTP invalide pour `{url}`"))?;
    if !response.ok() {
        return Err(format!("HTTP {}", response.status()));
    }
    let _ = wasm_bindgen_futures::JsFuture::from(
        response
            .array_buffer()
            .map_err(|_| format!("réponse binaire invalide pour `{url}`"))?,
    )
    .await
    .map_err(|_| format!("lecture binaire impossible pour `{url}`"))?;
    Ok(())
}

#[cfg(target_arch = "wasm32")]
async fn fetch_optional_configuration(url: &str) -> Result<Option<String>, String> {
    use wasm_bindgen::JsCast;
    let window = web_sys::window().ok_or_else(|| "window unavailable".to_string())?;
    let value = wasm_bindgen_futures::JsFuture::from(window.fetch_with_str(url))
        .await
        .map_err(|_| format!("Cannot load configuration `{url}`"))?;
    let response: web_sys::Response = value
        .dyn_into()
        .map_err(|_| format!("Invalid HTTP response for `{url}`"))?;
    if response.status() == 404 {
        return Ok(None);
    }
    if !response.ok() {
        return Err(format!(
            "Cannot load configuration `{url}`: HTTP {}",
            response.status()
        ));
    }
    let content = wasm_bindgen_futures::JsFuture::from(
        response
            .text()
            .map_err(|_| format!("Invalid text response for `{url}`"))?,
    )
    .await
    .map_err(|_| format!("Cannot read configuration `{url}`"))?;
    content
        .as_string()
        .map(Some)
        .ok_or_else(|| format!("Non UTF-8 configuration `{url}`"))
}

#[cfg(target_arch = "wasm32")]
async fn fetch_text(url: &str) -> Result<String, String> {
    use wasm_bindgen::JsCast;

    let window = web_sys::window().ok_or_else(|| "window indisponible".to_string())?;
    let response_value = wasm_bindgen_futures::JsFuture::from(window.fetch_with_str(url))
        .await
        .map_err(|_| format!("impossible de charger `{url}`"))?;
    let response: web_sys::Response = response_value
        .dyn_into()
        .map_err(|_| format!("réponse HTTP invalide pour `{url}`"))?;
    if !response.ok() {
        return Err(format!(
            "impossible de charger `{url}`: HTTP {}",
            response.status()
        ));
    }
    let text = wasm_bindgen_futures::JsFuture::from(
        response
            .text()
            .map_err(|_| format!("réponse texte invalide pour `{url}`"))?,
    )
    .await
    .map_err(|_| format!("lecture texte impossible pour `{url}`"))?;
    text.as_string()
        .ok_or_else(|| format!("réponse non UTF-8 pour `{url}`"))
}

/// Spawns the 2D camera.  This duplicates the definition from the original main.rs so
/// that the library does not depend on the old binary entry point.
fn setup_camera(mut commands: Commands,preview:Option<Res<editor_preview::Options>>,mut images:ResMut<Assets<Image>>) {
    #[cfg(not(target_arch="wasm32"))]
    if let Some(preview)=preview.as_deref(){
        use bevy::render::{render_asset::RenderAssetUsages,render_resource::{Extent3d,TextureDimension,TextureFormat,TextureUsages},camera::RenderTarget};
        let mut image=Image::new_fill(Extent3d{width:preview.width,height:preview.height,depth_or_array_layers:1},
            TextureDimension::D2,&[0,0,0,255],TextureFormat::Bgra8UnormSrgb,RenderAssetUsages::default());
        image.texture_descriptor.usage=TextureUsages::RENDER_ATTACHMENT|TextureUsages::TEXTURE_BINDING|TextureUsages::COPY_SRC|TextureUsages::COPY_DST;
        let image=images.add(image);
        commands.spawn((Camera2dBundle{camera:Camera{target:RenderTarget::Image(image.clone()),..default()},..default()},bevy::ui::IsDefaultUiCamera));
        commands.insert_resource(editor_preview_image::Target{image,width:preview.width,height:preview.height});
    } else {commands.spawn(Camera2dBundle::default());}
    #[cfg(target_arch="wasm32")]
    {let _=preview;let _=images;commands.spawn(Camera2dBundle::default());}
    commands.spawn((systems::sprite::SpriteStage, SpatialBundle::default()));
}

#[allow(clippy::field_reassign_with_default)]
fn settings_from_persistent(data: &PersistentData, current_lang: &str) -> Settings {
    let mut settings = Settings::default();
    settings.language = current_lang.to_string();
    if let Some(language) = &data.language {
        settings.language = language.clone();
    }
    if let Some(volume) = data.music_volume {
        settings.music_volume = volume.clamp(0.0, 1.0);
    }
    if let Some(volume) = data.sfx_volume {
        settings.sfx_volume = volume.clamp(0.0, 1.0);
    }
    if let Some(speed) = data.text_speed {
        settings.text_speed = speed.clamp(0.1, 5.0);
    }
    if let Some(speed) = data.auto_speed {
        settings.auto_speed = speed.clamp(0.1, 5.0);
    }
    if let Some(fullscreen) = data.fullscreen {
        settings.fullscreen = fullscreen;
    }
    if let Some(typewriter) = data.typewriter {
        settings.typewriter = typewriter;
    }
    settings
}

fn discover_locale_langs(locales_dir: &Path) -> Vec<String> {
    let mut langs = Vec::new();
    let Ok(entries) = fs::read_dir(locales_dir) else {
        return langs;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() || path.extension().and_then(|ext| ext.to_str()) != Some("toml") {
            continue;
        }
        if let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) {
            langs.push(stem.to_string());
        }
    }
    langs.sort();
    langs
}

fn build_cg_asset_registry(assets_dir: &Path) -> CgAssetRegistry {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = assets_dir;
        return CgAssetRegistry::default();
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut registry = HashMap::new();
        let cgs_dir = assets_dir.join("cgs");
        let Ok(entries) = fs::read_dir(cgs_dir) else {
            return CgAssetRegistry(registry);
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Some(ext) = path.extension().and_then(|ext| ext.to_str()) else {
                continue;
            };
            if !matches!(ext, "png" | "jpg" | "jpeg" | "webp") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
                continue;
            };
            let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            registry.insert(stem.to_string(), format!("cgs/{file_name}"));
        }

        CgAssetRegistry(registry)
    }
}

fn build_music_asset_registry(assets_dir: &Path) -> MusicAssetRegistry {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = assets_dir;
        return MusicAssetRegistry::default();
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut registry = HashMap::new();
        let music_dir = assets_dir.join("music");
        let Ok(entries) = fs::read_dir(music_dir) else {
            return MusicAssetRegistry(registry);
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Some(ext) = path.extension().and_then(|ext| ext.to_str()) else {
                continue;
            };
            if !matches!(ext, "ogg" | "mp3" | "wav" | "flac") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
                continue;
            };
            let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            registry.insert(stem.to_string(), format!("music/{file_name}"));
        }

        MusicAssetRegistry(registry)
    }
}

// Native loader/store regressions using owned temporary projects.
#[cfg(all(test, not(target_arch = "wasm32")))]
mod persistent_bootstrap_0030 {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    struct Project(PathBuf);
    impl Project {
        fn new(story: &str) -> Self {
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
            let root = std::env::temp_dir().join(format!("rvn-persistent-boot-0030-{}-{stamp}", std::process::id()));
            fs::create_dir(&root).unwrap();
            fs::create_dir(root.join("assets")).unwrap();
            fs::create_dir(root.join("locales")).unwrap();
            fs::write(root.join("rvn.toml"), "[project]\ntitle='Owned persistence regression'\nmain_script='main.rvn'\nstart_label='start'\n[paths]\nassets='assets'\nlocales='locales'\nsaves='saves'\ntheme='theme.toml'\n").unwrap();
            fs::write(root.join("main.rvn"), story).unwrap();
            fs::write(root.join("theme.toml"), "[title_screen]\nenabled=false\n").unwrap();
            let manager = PersistentDataManager::new(root.join("saves")).unwrap();
            let mut data = PersistentData::default();
            data.script_vars.insert("persistent.counter".into(), rvn_parser::Value::Int(7));
            data.seen_cgs.insert("original-cg".into());
            data.seen_endings.insert("original-ending".into());
            data.music_volume = Some(0.37);
            manager.save(&data).unwrap();
            Self(root)
        }
        fn manager(&self) -> PersistentDataManager {
            PersistentDataManager::new(self.0.join("saves")).unwrap()
        }
    }
    impl Drop for Project {
        fn drop(&mut self) {
            let Ok(root) = self.0.canonicalize() else { return; };
            let Ok(temp) = std::env::temp_dir().canonicalize() else { return; };
            if root.parent() == Some(temp.as_path()) && root.file_name().is_some_and(|name|
                name.to_string_lossy().starts_with(&format!("rvn-persistent-boot-0030-{}-", std::process::id()))) {
                let _ = fs::remove_dir_all(root);
            }
        }
    }

    #[test]
    fn string_table_host_actual_native_loader_supplies_declared_file_before_init_and_rejects_mismatched_identity() {
        use rvn_core::{locale::{LocaleCatalog, LocaleTable}, string_table::{StringTableAsset, StringTableEntry}, string_table_file::StringTableFile};
        use rvn_parser::{localized_text::TextIdentity, Value};
        const ID: &str = "17411a3e431e4c9388b78a2b9825b130";
        const OTHER: &str = "3fc40c36f4e544ed82943e4c6fe9d87d";
        for header in ["label start", "scene label start"] {
            let project = Project::new(&format!(r#"init {{
set name=__rvn_string_table_text("{ID}","hello")
set converted=text_to_string(name)
set seen=persistent.counter
}}
{header}
"[converted] / [name] / [seen]"
"#));
            let mut cfg = fs::read_to_string(project.0.join("rvn.toml")).unwrap();
            cfg.push_str(&format!("\n[string_tables]\n\"{ID}\"='tables/greeting.rvnstrings'\n"));
            fs::write(project.0.join("rvn.toml"), &cfg).unwrap();
            fs::create_dir(project.0.join("assets/tables")).unwrap();
            let table = |id: &str| StringTableFile::new(StringTableAsset { id: id.into(), namespace: "VN".into(), entries: [("hello".into(), StringTableEntry {metadata:Default::default(), source: "Visitor".into(), developer_notes: "Context".into() })].into() }).unwrap();
            let original = table(ID).encode().unwrap();
            fs::write(project.0.join("assets/tables/greeting.rvnstrings"), &original).unwrap();
            fs::write(project.0.join("assets/config.toml"), "[locale]\ndefault='en'\ncurrent='fr'\navailable=['en','fr']\n").unwrap();
            let identity = TextIdentity { owner: format!("string-table:{ID}"), namespace: "VN".into(), key: "hello".into() };
            let locale = LocaleCatalog::with_text_sources(LocaleTable { lang: "fr".into(), strings: [(identity.locale_key(), "Éloïse".into())].into() }, [(identity.locale_key(), "Visitor".into())].into()).unwrap();
            fs::write(project.0.join("locales/fr.toml"), locale.to_toml()).unwrap();
            let before = fs::read(project.manager().path()).unwrap();
            let mut launch = load_game(&project.0).unwrap();
            assert_eq!(launch.engine.state.vars["converted"], Value::Str("Éloïse".into()));
            assert!(matches!(launch.engine.step_until_interaction().unwrap(), Some(rvn_core::Interaction::Dialogue { text, .. }) if text == "Éloïse / Éloïse / 7"));
            assert_eq!(fs::read(project.0.join("assets/tables/greeting.rvnstrings")).unwrap(), original);
            assert_eq!(fs::read(project.manager().path()).unwrap(), before);
            fs::write(project.0.join("assets/tables/greeting.rvnstrings"), table(OTHER).encode().unwrap()).unwrap();
            assert!(load_game(&project.0).err().unwrap().contains("identity differs"));
            assert_eq!(fs::read(project.manager().path()).unwrap(), before);
        }
    }

    #[test]
    fn text_source_witness_actual_native_load_and_reload_reject_stale_translation_before_new_init() {
        use rvn_core::locale::{LocaleCatalog, LocaleTable};
        use rvn_parser::{localized_text::TextIdentity, Value};
        for header in ["label start", "scene label start"] {
            let project = Project::new(&format!(r#"init {{
set name=__rvn_localizable_text("Visitor","owner:witness","VN.Boot","name")
set converted=text_to_string(name)
set seen=persistent.counter
}}
{header}
"[converted] / [name] / [seen]"
"#));
            fs::write(project.0.join("assets/config.toml"), "[locale]\ndefault='en'\ncurrent='fr'\navailable=['en','fr']\n").unwrap();
            let identity = TextIdentity { owner: "owner:witness".into(), namespace: "VN.Boot".into(), key: "name".into() };
            let catalog = |lang: &str, source: &str, display: &str| LocaleCatalog::with_text_sources(LocaleTable {
                lang: lang.into(), strings: [(identity.locale_key(), display.into())].into() }, [(identity.locale_key(), source.into())].into()).unwrap();
            fs::write(project.0.join("locales/en.toml"), catalog("en", "Visitor", "English").to_toml()).unwrap();
            fs::write(project.0.join("locales/fr.toml"), catalog("fr", "Visitor", "Éloïse").to_toml()).unwrap();
            let before = fs::read(project.manager().path()).unwrap();
            let mut launch = load_game(&project.0).unwrap();
            assert_eq!(launch.engine.state.vars["converted"], Value::Str("Éloïse".into()));
            fs::write(project.0.join("locales/fr.toml"), catalog("fr", "Other source", "Révisé").to_toml()).unwrap();
            launch.engine.locale.as_mut().unwrap().reload_current();
            assert!(matches!(launch.engine.step_until_interaction().unwrap(), Some(rvn_core::Interaction::Dialogue { text, .. }) if text == "Éloïse / English / 7"));
            let fresh = launch.engine.fresh(BevyRenderer::new(), 64).unwrap();
            assert_eq!(fresh.state.vars["converted"], Value::Str("English".into()));
            assert_eq!(fresh.state.vars["seen"], Value::Int(7));
            assert_eq!(fs::read(project.manager().path()).unwrap(), before);
        }
    }

    #[test]
    fn text_runtime_bootstrap_persisted_language_precedes_real_init_and_new_game() {
        use rvn_parser::{localized_text::TextIdentity, Value};
        for header in ["label start", "scene label start"] {
            let project = Project::new(&format!(r#"init {{
set name=__rvn_localizable_text("Visitor","owner:boot","VN.Boot","name")
set converted=text_to_string(name)
set seen=persistent.counter
}}
{header}
"[converted] / [name] / [seen]"
"#));
            fs::write(project.0.join("assets/config.toml"), "[locale]\ndefault='en'\ncurrent='en'\navailable=['en','fr']\n").unwrap();
            let identity = TextIdentity { owner: "owner:boot".into(), namespace: "VN.Boot".into(), key: "name".into() };
            fs::write(project.0.join("locales/fr.toml"), format!("[strings]\n{:?} = {:?}\n", identity.locale_key(), "Éloïse")).unwrap();
            let manager = project.manager(); let mut data = manager.load().unwrap();
            data.language = Some("fr".into()); manager.save(&data).unwrap();
            let before = fs::read(manager.path()).unwrap();
            let mut launch = load_game(&project.0).unwrap();
            assert_eq!(launch.locale_cfg.current, "fr");
            assert_eq!(launch.engine.state.vars["converted"], Value::Str("Éloïse".into()));
            assert_eq!(launch.engine.state.vars["seen"], Value::Int(7));
            launch.engine.locale.as_mut().unwrap().set_language("en").unwrap();
            assert!(matches!(launch.engine.step_until_interaction().unwrap(), Some(rvn_core::Interaction::Dialogue { text, .. }) if text == "Éloïse / Visitor / 7"));
            let fresh = launch.engine.fresh(BevyRenderer::new(), 64).unwrap();
            assert_eq!(fresh.state.vars["converted"], Value::Str("Visitor".into()));
            assert_eq!(fresh.state.vars["seen"], Value::Int(7));
            assert_eq!(fs::read(manager.path()).unwrap(), before, "startup and fresh must preserve the actual progression file");
        }
    }

    #[test]
    fn persistent_bootstrap_0030_true_native_loader_reads_progression_before_real_init() {
        let project = Project::new("init{set observed_boot=persistent.counter}\nscene label start\n\"Boot [observed_boot].\"\n");
        let bytes = fs::read(project.manager().path()).unwrap();
        // This is the exact preparation called by run_game, not a second loader.
        let launch = load_game(&project.0).expect("stored persistent variables must be available to actual init");
        assert_eq!(launch.project_paths.saves, project.0.join("saves"));
        assert_eq!(launch.engine.get_persistent_var("persistent.counter"), Some(&rvn_parser::Value::Int(7)));
        assert_eq!(launch.engine.state.vars["observed_boot"], rvn_parser::Value::Int(7));
        assert_eq!(fs::read(project.manager().path()).unwrap(), bytes, "boot observation must not rewrite the progression file");
    }

    #[test]
    fn persistent_bootstrap_0030_real_step_and_store_receiver_reach_a_second_loader() {
        let project = Project::new("init{set game_score=3}\nlabel start\nset persistent.counter=9\n\"Committed.\"\n");
        let mut launch = load_game(&project.0).unwrap();
        let original = launch.persistent_data.clone();
        assert!(matches!(launch.engine.step_until_interaction().unwrap(), Some(rvn_core::Interaction::Dialogue{..})));
        assert_eq!(launch.engine.get_persistent_var("persistent.counter"), Some(&rvn_parser::Value::Int(9)));
        assert!(!launch.engine.renderer.scene_batch_pending());
        let mut world = World::new();
        world.insert_resource(resources::VnEngine(launch.engine));
        world.insert_resource(resources::PersistentDataResource {
            manager: launch.persistent_manager, data: launch.persistent_data,
        });
        world.init_resource::<Events<vn_command::VnCommand>>();
        world.init_resource::<resources::ScriptErrorMessage>();
        world.init_resource::<NextState<resources::VnState>>();
        world.run_system_once(systems::persistent::persistent_unlock_system);
        let stored = project.manager().load().unwrap();
        assert_eq!(stored.script_vars.get("persistent.counter"), Some(&rvn_parser::Value::Int(9)),
            "the real existing persistence receiver must commit authored script variables");
        assert_eq!(stored.seen_cgs, original.seen_cgs);
        assert_eq!(stored.seen_endings, original.seen_endings);
        assert_eq!(stored.music_volume, original.music_volume);
        let bytes = fs::read(project.manager().path()).unwrap();
        world.run_system_once(systems::persistent::persistent_unlock_system);
        assert_eq!(fs::read(project.manager().path()).unwrap(), bytes);
        let second = load_game(&project.0).unwrap();
        assert_ne!(second.engine.instance_id(), world.resource::<resources::VnEngine>().0.instance_id());
        assert_eq!(second.engine.get_persistent_var("persistent.counter"), Some(&rvn_parser::Value::Int(9)));
        assert_eq!(second.engine.state.vars["game_score"], rvn_parser::Value::Int(3));
        assert_eq!(fs::read(project.manager().path()).unwrap(), bytes);
    }

    #[test]
    fn persistent_bootstrap_0030_true_loader_refuses_corrupt_progression_without_repairing_the_file() {
        let project = Project::new("init{set game_score=3}\nlabel start\n\"Start.\"\n");
        let invalid = b"{owned invalid progression fixture";
        fs::write(project.manager().path(), invalid).unwrap();
        let story = fs::read(project.0.join("main.rvn")).unwrap();
        let result = load_game(&project.0);
        assert!(matches!(result, Err(message) if message.contains("[persistent] erreur chargement")));
        assert_eq!(fs::read(project.manager().path()).unwrap(), invalid);
        assert_eq!(fs::read(project.0.join("main.rvn")).unwrap(), story);
    }

// Store regressions sharing the owned Project fixture.
// No mocked IO result, engine state replacement, or native process claim.
fn persistent_store_0030_world(launch: RuntimeLaunch) -> World {
    let mut world = World::new();
    world.insert_resource(resources::VnEngine(launch.engine));
    world.insert_resource(resources::PersistentDataResource {
        manager: launch.persistent_manager, data: launch.persistent_data,
    });
    world.init_resource::<Events<vn_command::VnCommand>>();
    world.init_resource::<resources::ScriptErrorMessage>();
    world.init_resource::<NextState<resources::VnState>>();
    world
}

#[test]
fn persistent_store_0030_real_disk_failure_keeps_last_resource_and_reports_error() {
    let project = Project::new("init{set game_score=3}\nlabel start\nset persistent.counter=9\n\"Committed.\"\n");
    let mut launch = load_game(&project.0).unwrap();
    let original = launch.persistent_data.clone();
    assert!(matches!(launch.engine.step_until_interaction().unwrap(), Some(rvn_core::Interaction::Dialogue{..})));
    assert!(!launch.engine.renderer.scene_batch_pending());
    let path = project.manager().path().to_path_buf();
    let bytes = fs::read(&path).unwrap();
    let backup = path.with_extension("owned-previous.json");
    fs::rename(&path, &backup).unwrap();
    // A real occupied directory makes atomic publication fail on Windows and Unix.
    fs::create_dir(&path).unwrap();
    let mut world = persistent_store_0030_world(launch);
    world.run_system_once(systems::persistent::persistent_unlock_system);
    assert_eq!(world.resource::<resources::PersistentDataResource>().data, original);
    assert!(world.resource::<resources::ScriptErrorMessage>().0.contains("[persistent]"));
    assert!(matches!(world.resource::<NextState<resources::VnState>>(), NextState::Pending(resources::VnState::Error)));
    assert_eq!(fs::read(&backup).unwrap(), bytes);
    assert!(path.is_dir());
    assert_eq!(fs::read_dir(&path).unwrap().count(), 0);
    assert!(!fs::read_dir(path.parent().unwrap()).unwrap().any(|entry|
        entry.unwrap().file_name().to_string_lossy().contains("pending-")));
    assert_eq!(world.resource::<resources::VnEngine>().0.get_persistent_var("persistent.counter"), Some(&rvn_parser::Value::Int(9)));
}

#[test]
fn persistent_store_0030_equal_authored_map_does_not_attempt_blocked_store_publication() {
    let project = Project::new("init{set game_score=3}\nlabel start\nset persistent.counter=7\n\"Unchanged.\"\n");
    let mut launch = load_game(&project.0).unwrap();
    let original = launch.persistent_data.clone();
    assert!(matches!(launch.engine.step_until_interaction().unwrap(), Some(rvn_core::Interaction::Dialogue{..})));
    assert_eq!(launch.engine.export_persistent_vars(), &original.script_vars);
    assert!(!launch.engine.renderer.scene_batch_pending());
    let path = project.manager().path().to_path_buf();
    let bytes = fs::read(&path).unwrap();
    let backup = path.with_extension("owned-previous.json");
    fs::rename(&path, &backup).unwrap();
    fs::create_dir(&path).unwrap();
    let mut world = persistent_store_0030_world(launch);
    world.run_system_once(systems::persistent::persistent_unlock_system);
    assert_eq!(world.resource::<resources::PersistentDataResource>().data, original);
    assert!(world.resource::<resources::ScriptErrorMessage>().0.is_empty());
    assert!(matches!(world.resource::<NextState<resources::VnState>>(), NextState::Unchanged));
    assert_eq!(fs::read(&backup).unwrap(), bytes);
    assert!(path.is_dir());
    assert_eq!(fs::read_dir(&path).unwrap().count(), 0);
}

// After-only service regressions inside the existing owned loader fixture.
#[test]
fn persistent_store_0030_failed_write_is_not_retried_while_error_is_pending() {
    let project = Project::new("init{set game_score=3}\nlabel start\nset persistent.counter=9\n\"Committed.\"\n");
    let mut launch = load_game(&project.0).unwrap();
    let original = launch.persistent_data.clone();
    launch.engine.step_until_interaction().unwrap();
    let path = project.manager().path().to_path_buf();
    let bytes = fs::read(&path).unwrap();
    let backup = path.with_extension("owned-previous.json");
    fs::rename(&path, &backup).unwrap();
    fs::create_dir(&path).unwrap();
    let mut world = persistent_store_0030_world(launch);
    world.run_system_once(systems::persistent::persistent_unlock_system);
    assert!(matches!(world.resource::<NextState<resources::VnState>>(), NextState::Pending(resources::VnState::Error)));
    let message = world.resource::<resources::ScriptErrorMessage>().0.clone();
    assert!(message.contains("[persistent]"));
    // Restore the owned IO destination, but do not dismiss the engine's error.
    fs::remove_dir(&path).unwrap();
    fs::rename(&backup, &path).unwrap();
    for _ in 0..3 { world.run_system_once(systems::persistent::persistent_unlock_system); }
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert_eq!(world.resource::<resources::PersistentDataResource>().data, original);
    assert_eq!(world.resource::<resources::ScriptErrorMessage>().0, message);
    world.insert_resource(State::new(resources::VnState::Error));
    world.insert_resource(NextState::<resources::VnState>::Unchanged);
    world.run_system_once(systems::persistent::persistent_unlock_system);
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert_eq!(world.resource::<resources::PersistentDataResource>().data, original);
}

#[test]
fn persistent_store_0030_waits_for_real_scene_publication_and_admitted_checkpoint() {
    let project = Project::new("init{set game_score=3}\nscene label start\nset persistent.counter=9\nset game_score=4\n\"Published.\"\n");
    let mut launch = load_game(&project.0).unwrap();
    assert!(matches!(launch.engine.step_until_interaction().unwrap(), Some(rvn_core::Interaction::Dialogue{..})));
    assert!(launch.engine.renderer.scene_batch_pending());
    let bytes = fs::read(project.manager().path()).unwrap();
    let mut world = persistent_store_0030_world(launch);
    world.init_resource::<crate::scene_publication::ScenePresentation>();
    world.init_resource::<resources::VnRenderState>();
    world.init_resource::<resources::ChoiceFocus>();
    world.init_resource::<resources::ImagemapState>();
    world.init_resource::<resources::TypewriterState>();
    world.init_resource::<crate::accessibility::Accessibility>();
    world.init_resource::<crate::editor_preview::PreviewState>();
    world.run_system_once(systems::persistent::persistent_unlock_system);
    assert_eq!(fs::read(project.manager().path()).unwrap(), bytes);
    let mut actual_queue = crate::checkpoint_requests::CheckpointRequests::default();
    assert!(crate::checkpoint_requests::defer_save(
        world.resource::<resources::VnEngine>(), Some(&mut actual_queue),
        crate::checkpoint_requests::SaveDestination::Auto, "Owned checkpoint".into(), "main.rvn".into(), None
    ).is_ok_and(|deferred| deferred));
    assert!(actual_queue.blocked());
    world.insert_resource(actual_queue);
    crate::scene_publication::consume(&mut world);
    assert!(!world.resource::<resources::VnEngine>().0.renderer.scene_batch_pending());
    assert!(!world.resource::<resources::VnEngine>().0.renderer.scene_frame_failed);
    world.run_system_once(systems::persistent::persistent_unlock_system);
    assert_eq!(fs::read(project.manager().path()).unwrap(), bytes);
    world.resource_mut::<crate::checkpoint_requests::CheckpointRequests>().cancel();
    world.run_system_once(systems::persistent::persistent_unlock_system);
    let data = project.manager().load().unwrap();
    assert_eq!(data.script_vars["persistent.counter"], rvn_parser::Value::Int(9));
    assert_eq!(data.music_volume, Some(0.37));
    assert!(data.seen_cgs.contains("original-cg"));
    assert!(data.seen_endings.contains("original-ending"));
}

}

#[cfg(all(test,not(target_arch="wasm32")))]
#[path="normal_runtime_startup_tests.rs"]
mod normal_runtime_startup_tests;

#[cfg(test)]
#[path="unit_runtime_source_tests1003.rs"]
mod unit_runtime_source_tests1003;

mod editor_enum_current1027;
#[cfg(not(target_arch="wasm32"))]
mod editor_library_current1027;
mod editor_widget_preview1027;
