// Bevy systems routinely exceed clippy's defaults for argument count and
// query type complexity; these are idiomatic for the ECS pattern.
#![allow(
    clippy::too_many_arguments,
    clippy::type_complexity,
    clippy::clone_on_copy,
    dead_code,
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

mod bevy_renderer;
mod components;
mod project_paths;
mod resources;
mod systems;
mod vn_command;
use crate::bevy_renderer::BevyRenderer;
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
use rvn_parser::{parse_file_with_uses, Script, Statement};
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

struct RuntimeLaunch {
    cfg: RvnToml,
    engine: Engine<BevyRenderer>,
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
    let project_dir: &Path = project_dir.as_ref();

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
    let script = parse_file_with_uses(&script_path).map_err(|e| {
        format!(
            "Erreur de chargement des scripts depuis `{}`:\n{}",
            script_path.display(),
            e
        )
    })?;

    // 3. Determine the assets and other paths.
    let assets_dir: PathBuf = project_dir.join(&cfg.paths.assets);
    let locales_dir: PathBuf = project_dir.join(&cfg.paths.locales);
    let theme_path: PathBuf = project_dir.join(&cfg.paths.theme);
    // Build saves directory path relative to project dir
    let saves_dir: PathBuf = project_dir.join(&cfg.paths.saves);

    // 4. Load locale configuration from config.toml in the assets directory if present.
    let config_toml_path = assets_dir.join("config.toml");
    let app_config: AppConfig = std::fs::read_to_string(&config_toml_path)
        .ok()
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_default();
    let mut locale_cfg = app_config.locale;
    for lang in discover_locale_langs(&locales_dir) {
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
    let locale_mgr = LocaleManager::new(
        &locales_dir,
        &locale_cfg.default,
        &locale_cfg.current,
        locale_cfg.available.clone(),
    )
    .map_err(|e| format!("[locale] erreur initialisation : {e}"))?;
    let engine = build_engine(script, &cfg, &script_path.display().to_string(), locale_mgr)?;

    // 9. Load the UI theme and set up the watcher.
    let theme_content = std::fs::read_to_string(&theme_path).unwrap_or_default();
    let theme: Theme = toml::from_str(&theme_content).unwrap_or_default();
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
        project_dir.join(&cfg.paths.saves),
    );
    let cg_registry = build_cg_asset_registry(&assets_dir);
    let music_registry = build_music_asset_registry(&assets_dir);

    run_loaded_game(RuntimeLaunch {
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

fn run_loaded_game(launch: RuntimeLaunch) -> Result<(), String> {
    let RuntimeLaunch {
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
    let window_width = cfg.window.width.unwrap_or(1280) as f32;
    let window_height = cfg.window.height.unwrap_or(720) as f32;
    let ts_current = if let Some(locale) = &engine.locale {
        locale
            .locale_path(&locale_cfg.current)
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
    } else {
        std::time::SystemTime::UNIX_EPOCH
    };
    let ts_default = if let Some(locale) = &engine.locale {
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

    App::new()
        .add_plugins(
            DefaultPlugins
                .set(AssetPlugin {
                    file_path: asset_root,
                    watch_for_changes_override: Some(true),
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: cfg.project.title.clone(),
                        resolution: (window_width, window_height).into(),
                        resizable: false,
                        ..default()
                    }),
                    ..default()
                }),
        )
        // Resources
        .insert_resource(theme)
        .insert_resource(ProjectTitle(cfg.project.title.clone()))
        .insert_resource(theme_watcher)
        .insert_resource(VnEngine(engine))
        .insert_resource(VnRenderState::default())
        .insert_resource(crate::resources::SkipMode::default())
        .insert_resource(ImagemapState::default())
        .insert_resource(MusicEntity::default())
        .insert_resource(MusicPlaybackState::default())
        .insert_resource(MusicVolume::default())
        .insert_resource(TypewriterConfig::default())
        .insert_resource(TypewriterState::default())
        .insert_resource(MenuState::default())
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
        .add_event::<VnCommand>()
        .add_event::<PlayerInput>()
        .init_state::<VnState>()
        // Startup systems
        .add_systems(Startup, (setup_camera, setup_ui, build_character_registry))
        // State transition systems
        .add_systems(OnEnter(VnState::TitleScreen), spawn_title_screen)
        .add_systems(OnExit(VnState::TitleScreen), despawn_title_screen)
        .add_systems(OnExit(VnState::Gallery), despawn_gallery_overlay)
        .add_systems(OnEnter(VnState::Menu), spawn_menu_overlay)
        .add_systems(OnExit(VnState::Menu), despawn_menu_overlay)
        .add_systems(OnEnter(VnState::History), spawn_history_overlay)
        .add_systems(OnExit(VnState::History), despawn_history_overlay)
        .add_systems(OnEnter(VnState::Error), spawn_error_overlay)
        .add_systems(OnExit(VnState::Error), despawn_error_overlay)
        // Update systems
        .add_systems(
            Update,
            (
                theme_reload_system,
                apply_theme_system,
                locale_reload_system,     // ← hot-reload locales
                locale_lang_watch_system, // ← surveille __lang variable
                title_interaction_system.run_if(in_state(VnState::TitleScreen)),
                title_background_resize_system.run_if(in_state(VnState::TitleScreen)),
                spawn_gallery_overlay.run_if(in_state(VnState::Gallery)),
                gallery_interaction_system.run_if(in_state(VnState::Gallery)),
                stepping_system.run_if(in_state(VnState::Stepping)),
                (
                    background_system,
                    background_cover_resize_system,
                    sprite_system,
                    sprite_animation_system,
                    cinematic_system,
                    cinematic_cover_resize_system,
                    dialogue_system,
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
                update_choice_buttons,
                fade_system.run_if(in_state(VnState::Animating)),
                audio_fade_system,
                persistent_unlock_system,
            ),
        )
        .add_systems(
            Update,
            (
                typewriter_system.run_if(in_state(VnState::Waiting)),
                input_system.run_if(in_state(VnState::Waiting)),
                choice_interaction_system.run_if(in_state(VnState::Waiting)),
                menu_input_system.run_if(in_state(VnState::Menu)),
                menu_interaction_system.run_if(in_state(VnState::Menu)),
                history_input_system.run_if(in_state(VnState::History)),
                player_input_system
                    .after(input_system)
                    .after(choice_interaction_system)
                    .after(menu_input_system)
                    .after(history_input_system),
            ),
        )
        .add_systems(
            Update,
            (
                spawn_save_menu_overlay.run_if(in_state(VnState::Menu)),
                save_menu_interaction_system.run_if(in_state(VnState::Menu)),
                despawn_save_menu_overlay,
                spawn_settings_menu_overlay.run_if(in_state(VnState::Menu)),
                settings_menu_interaction_system.run_if(in_state(VnState::Menu)),
                update_settings_value_text_system.run_if(in_state(VnState::Menu)),
                apply_settings_to_runtime_system,
                despawn_settings_menu_overlay,
            ),
        )
        .add_systems(Update, update_debug_overlay_system)
        // Debug overlay : toggling, spawn/despawn et gestion du step F10
        .add_systems(
            Update,
            (
                debug_toggle_system,
                spawn_or_despawn_debug_overlay_system,
                debug_step_input_system,
            ),
        )
        .run();

    Ok(())
}

fn build_engine(
    script: Script,
    cfg: &RvnToml,
    script_label: &str,
    locale_mgr: LocaleManager,
) -> Result<Engine<BevyRenderer>, String> {
    let renderer = BevyRenderer::new();
    let mut engine = Engine::new(script, renderer, 64)
        .map_err(|e| format!("Erreur d'initialisation du moteur: {e}"))?;
    engine.locale = Some(locale_mgr);

    if let Some(ref start_label) = cfg.project.start_label {
        if let Some(pos) = engine
            .script
            .iter()
            .position(|stmt| matches!(stmt, Statement::Label { name } if name == start_label))
        {
            engine.state.pc = pos;
        } else {
            return Err(format!(
                "start_label `{start_label}` introuvable dans `{script_label}`"
            ));
        }
    }

    let strings = collect_strings_from_flat_script(&engine.script);
    if let Some(locale) = &mut engine.locale {
        let _ = locale.update_default_locale(&strings);
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
}

#[cfg(target_arch = "wasm32")]
pub async fn run_game_web(project_root: &str) -> Result<(), String> {
    let root = project_root.trim_end_matches('/').trim_end_matches('.');
    let root = if root.is_empty() { "." } else { root };
    let rvn_toml_content = fetch_text(&format!("{root}/rvn.toml")).await?;
    let cfg: RvnToml = toml::from_str(&rvn_toml_content)
        .map_err(|e| format!("Impossible de parser rvn.toml: {e}"))?;

    let manifest = fetch_optional_text(&format!("{root}/rvn_web_manifest.toml"))
        .await
        .and_then(|content| toml::from_str::<WebScriptManifest>(&content).ok())
        .map(|manifest| manifest.scripts)
        .unwrap_or_else(|| vec![cfg.project.main_script.clone()]);
    let script = load_web_scripts(root, &cfg.project.main_script, &manifest).await?;

    let assets_dir = PathBuf::from(&cfg.paths.assets);
    let locales_dir = PathBuf::from(&cfg.paths.locales);
    let saves_dir = PathBuf::from(&cfg.paths.saves);
    let theme_path = PathBuf::from(&cfg.paths.theme);
    let asset_manifest = fetch_optional_text(&format!("{root}/rvn_web_assets.toml"))
        .await
        .and_then(|content| toml::from_str::<WebAssetManifest>(&content).ok())
        .unwrap_or_default();

    let app_config = if asset_manifest.has_config_toml {
        fetch_optional_text(&format!("{root}/{}/config.toml", cfg.paths.assets))
            .await
            .and_then(|s| toml::from_str::<AppConfig>(&s).ok())
            .unwrap_or_default()
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
            match rvn_core::locale::LocaleTable::parse(lang, &content) {
                Ok(table) => {
                    locale_tables.insert(lang.clone(), table);
                }
                Err(e) => bevy::log::warn!("[locale] {path} ignoré: {e}"),
            }
        }
    }
    let locale_mgr = LocaleManager::from_tables(
        &locales_dir,
        &locale_cfg.default,
        &locale_cfg.current,
        locale_cfg.available.clone(),
        locale_tables,
    );
    let engine = build_engine(script, &cfg, &cfg.project.main_script, locale_mgr)?;

    let theme_content = fetch_optional_text(&format!("{root}/{}", cfg.paths.theme))
        .await
        .unwrap_or_default();
    let theme: Theme = toml::from_str(&theme_content).unwrap_or_default();
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

    run_loaded_game(RuntimeLaunch {
        cfg,
        engine,
        locale_cfg,
        theme,
        theme_watcher,
        asset_root: "assets".to_string(),
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
) -> Result<Script, String> {
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

    let mut loaded = std::collections::HashSet::new();
    let mut stack = Vec::new();
    parse_web_script(
        &normalize_web_path(main_script),
        &sources,
        &files,
        &mut loaded,
        &mut stack,
    )
}

#[cfg(target_arch = "wasm32")]
fn parse_web_script(
    path: &str,
    sources: &HashMap<String, String>,
    manifest: &[String],
    loaded: &mut std::collections::HashSet<String>,
    stack: &mut Vec<String>,
) -> Result<Script, String> {
    let path = normalize_web_path(path);
    if let Some(pos) = stack.iter().position(|p| p == &path) {
        let mut cycle = stack[pos..].to_vec();
        cycle.push(path);
        return Err(format!("cycle de `use` détecté: {}", cycle.join(" -> ")));
    }
    if !loaded.insert(path.clone()) {
        return Ok(Vec::new());
    }

    let source = sources
        .get(&path)
        .ok_or_else(|| format!("fichier RVN web introuvable `{path}`"))?;
    stack.push(path.clone());
    let script =
        rvn_parser::parse(source).map_err(|e| format!("Erreur de parsing dans `{path}`:\n{e}"))?;
    let base_dir = path.rsplit_once('/').map(|(dir, _)| dir).unwrap_or("");

    let mut resolved = Vec::new();
    for stmt in script {
        match stmt {
            Statement::Use { paths } => {
                for use_path in paths {
                    for target in expand_web_use(base_dir, &use_path, manifest)? {
                        let mut nested =
                            parse_web_script(&target, sources, manifest, loaded, stack)?;
                        resolved.append(&mut nested);
                    }
                }
            }
            other => resolved.push(other),
        }
    }

    stack.pop();
    Ok(resolved)
}

#[cfg(target_arch = "wasm32")]
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

#[cfg(target_arch = "wasm32")]
fn join_web_path(base_dir: &str, raw: &str) -> String {
    let joined = if base_dir.is_empty() {
        raw.to_string()
    } else {
        format!("{base_dir}/{raw}")
    };
    normalize_web_path(&joined)
}

#[cfg(target_arch = "wasm32")]
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
fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera2dBundle::default());
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
