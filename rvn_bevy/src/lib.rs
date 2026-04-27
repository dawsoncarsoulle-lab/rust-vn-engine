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
mod resources;
mod systems;
mod vn_command;
mod project_paths;

use bevy::prelude::*;
use crate::bevy_renderer::BevyRenderer;
use crate::resources::{
    CharacterRegistry, ChoiceFocus, DialogueHistory, ImagemapState, LocaleConfig, MenuState,
    MusicEntity, MusicVolume, ScriptErrorMessage, Theme, ThemeWatcher, TypewriterConfig,
    TypewriterState, VnEngine, VnRenderState, VnState,
};
use crate::project_paths::ProjectPaths;
use crate::systems::settings_menu::{Settings, SettingsMenuState};
use crate::systems::save_menu::{SaveMenuState, SaveMenuMode};
use crate::systems::{
    apply_theme_system, audio_fade_system, audio_system, background_system,
    build_character_registry, choice_system, despawn_error_overlay, despawn_history_overlay,
    despawn_menu_overlay, despawn_title_screen, dialogue_system, fade_system,
    history_input_system, imagemap_cleanup_system, imagemap_dimensions_system,
    imagemap_hover_system, imagemap_system, input_system, locale_lang_watch_system,
    locale_reload_system, menu_input_system, menu_interaction_system, player_input_system,
    script_finished_system, spawn_error_overlay, spawn_history_overlay, spawn_menu_overlay,
    spawn_title_screen, sprite_system, stepping_system, theme_reload_system,
    title_interaction_system, typewriter_config_system, typewriter_system, update_choice_buttons,
    // Imports pour l'overlay de debug
    DebugOverlayState, DebugStepRequest, debug_toggle_system, debug_step_input_system,
    spawn_or_despawn_debug_overlay_system, update_debug_overlay_system,
    setup_ui,
    // Save/load menu systems
    spawn_save_menu_overlay, save_menu_interaction_system, despawn_save_menu_overlay,
    // Settings menu systems
    spawn_settings_menu_overlay, settings_menu_interaction_system, update_settings_value_text_system, apply_settings_to_runtime_system, despawn_settings_menu_overlay,
};
use crate::vn_command::{PlayerInput, VnCommand};
use rvn_core::{
    locale::{collect_strings_from_flat_script, LocaleManager},
    Engine,
};
use rvn_parser::parse;
use serde::Deserialize;
use std::path::{Path, PathBuf};

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
    /// Optional start label (unused for now).
    #[allow(dead_code)]
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
    let cfg: RvnToml = toml::from_str(&rvn_toml_content).map_err(|e| {
        format!(
            "Impossible de parser {}: {}",
            rvn_toml_path.display(),
            e
        )
    })?;

    // 2. Determine the script path relative to the project directory.
    let script_path = project_dir.join(&cfg.project.main_script);
    let script_content = std::fs::read_to_string(&script_path).map_err(|e| {
        format!(
            "Impossible de lire le fichier {}: {}",
            script_path.display(),
            e
        )
    })?;
    let script = parse(&script_content).map_err(|e| {
        format!(
            "Erreur de parsing dans `{}`:\n{}",
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
    let locale_cfg = app_config.locale;

    // 5. Initialise the LocaleManager.
    let mut locale_mgr = LocaleManager::new(
        &locales_dir,
        &locale_cfg.default,
        &locale_cfg.current,
        locale_cfg.available.clone(),
    )
    .map_err(|e| format!("[locale] erreur initialisation : {e}"))?;

    // 6. Initialise the engine with the script and renderer.
    let renderer = BevyRenderer::new();
    let mut engine = Engine::new(script, renderer, 64)
        .map_err(|e| format!("Erreur d'initialisation du moteur: {e}"))?;
    engine.locale = Some(locale_mgr);

    // 7. Collect strings from the flat script and update default locale.
    let strings = collect_strings_from_flat_script(&engine.script);
    if let Some(locale) = &mut engine.locale {
        let _ = locale.update_default_locale(&strings);
    }

    // 8. Compute last modification timestamps for locale files.
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
        default_lang: locale_cfg.default.clone(),
        available_langs: locale_cfg.available.clone(),
        last_modified_current: ts_current,
        last_modified_default: ts_default,
    };

    // 9. Load the UI theme and set up the watcher.
    let theme_content = std::fs::read_to_string(&theme_path).unwrap_or_default();
    let theme: Theme = toml::from_str(&theme_content).unwrap_or_default();
    let theme_watcher = ThemeWatcher::new(theme_path.to_string_lossy().to_string());

    // 10. Configure the window resolution.
    let window_width = cfg.window.width.unwrap_or(1280) as f32;
    let window_height = cfg.window.height.unwrap_or(720) as f32;

    // 11. Set the Bevy asset root to the project directory.
    //
    // Bevy resolves AssetServer paths through the default `assets/` folder under
    // BEVY_ASSET_ROOT. Therefore the root must be the project directory, not
    // the assets directory itself. Otherwise paths become `assets/assets/...`.
    std::env::set_var(
        "BEVY_ASSET_ROOT",
        project_dir.to_string_lossy().to_string(),
    );

    // 11a. Build a ProjectPaths resource to make paths accessible to systems.
    let project_paths = ProjectPaths::new(
        project_dir.to_path_buf(),
        project_dir.join(&cfg.paths.assets),
        project_dir.join(&cfg.paths.locales),
        theme_path.clone(),
        project_dir.join(&cfg.paths.saves),
    );

    // 12. Build and run the Bevy application.
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(AssetPlugin {
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
        .insert_resource(theme_watcher)
        .insert_resource(VnEngine(engine))
        .insert_resource(VnRenderState::default())
        .insert_resource(ImagemapState::default())
        .insert_resource(MusicEntity::default())
        .insert_resource(MusicVolume::default())
        .insert_resource(TypewriterConfig::default())
        .insert_resource(TypewriterState::default())
        .insert_resource(MenuState::default())
        .insert_resource(CharacterRegistry::default())
        .insert_resource(DialogueHistory::default())
        .insert_resource(ChoiceFocus::default())
        .insert_resource(ScriptErrorMessage::default())
        .insert_resource(locale_config_res)
        // Insert project path resource for save/load operations
        // Insert settings resource and UI states
        .insert_resource(Settings::default())
        .insert_resource(SettingsMenuState::default())
        .insert_resource(SaveMenuState::default())
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
                stepping_system.run_if(in_state(VnState::Stepping)),
                (
                    background_system,
                    sprite_system,
                    dialogue_system,
                    choice_system,
                    imagemap_system,
                    audio_system,
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
            ),
        )
        .add_systems(
            Update,
            (
                typewriter_system.run_if(in_state(VnState::Waiting)),
                input_system.run_if(in_state(VnState::Waiting)),
                menu_input_system.run_if(in_state(VnState::Menu)),
                menu_interaction_system.run_if(in_state(VnState::Menu)),
                history_input_system.run_if(in_state(VnState::History)),
                player_input_system
                    .after(input_system)
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
        .add_systems(Update, (debug_toggle_system, spawn_or_despawn_debug_overlay_system, debug_step_input_system))
        .run();

    Ok(())
}

/// Spawns the 2D camera.  This duplicates the definition from the original main.rs so
/// that the library does not depend on the old binary entry point.
fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera2dBundle::default());
}

