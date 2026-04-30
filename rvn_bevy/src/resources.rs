// rvn_bevy/src/resources.rs
//
// CHANGEMENTS vs version précédente :
//   - Ajout : `ChoiceFocus` resource (navigation clavier dans les choix).

use crate::bevy_renderer::BevyRenderer;
use bevy::prelude::*;
use rvn_core::Engine;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Resource)]
pub struct VnEngine(pub Engine<BevyRenderer>);

// ─── Machine à états ─────────────────────────────────────────────────────────

#[derive(States, Default, PartialEq, Eq, Hash, Clone, Debug)]
pub enum VnState {
    #[default]
    TitleScreen,
    Stepping,
    Waiting,
    Animating,
    Menu,
    History,
    Finished,
    Error,
}

// ─── Menu State ───────────────────────────────────────────────────────────────

#[derive(Resource, Default)]
pub struct MenuState {
    pub return_to: Option<VnState>,
}

// ─── Character Registry ───────────────────────────────────────────────────────

#[derive(Resource, Default)]
pub struct CharacterRegistry(pub HashMap<String, String>);

impl CharacterRegistry {
    pub fn display_name<'a>(&'a self, id: &'a str) -> &'a str {
        self.0.get(id).map(String::as_str).unwrap_or(id)
    }
}

// ─── VnRenderState ────────────────────────────────────────────────────────────

#[derive(Resource, Default)]
pub struct VnRenderState {
    pub background: Option<String>,
    pub choice_options: Vec<String>,
    pub state_after_anim: Option<VnState>,
}

// ─── ScriptErrorMessage ──────────────────────────────────────────────────────
//
// Stocke le message d'erreur runtime à afficher dans l'overlay.
// Peuplé par stepping_system avant de passer en VnState::Error.

#[derive(Resource, Default)]
pub struct ScriptErrorMessage(pub String);

// ─── ChoiceFocus ─────────────────────────────────────────────────────────────
//
// Index du bouton de choix actuellement sélectionné par le clavier.
// `None` = pas de focus (le joueur utilise la souris).

#[derive(Resource, Default)]
pub struct ChoiceFocus(pub Option<usize>);

impl ChoiceFocus {
    pub fn clear(&mut self) {
        self.0 = None;
    }

    /// Déplace le focus vers le bouton précédent (wrap).
    pub fn move_up(&mut self, count: usize) {
        if count == 0 {
            return;
        }
        self.0 = Some(match self.0 {
            None | Some(0) => count - 1,
            Some(i) => i - 1,
        });
    }

    /// Déplace le focus vers le bouton suivant (wrap).
    pub fn move_down(&mut self, count: usize) {
        if count == 0 {
            return;
        }
        self.0 = Some(match self.0 {
            None => 0,
            Some(i) => (i + 1) % count,
        });
    }
}

// ─── ImagemapState ────────────────────────────────────────────────────────────

#[derive(Resource, Default)]
pub struct ImagemapState {
    pub hotspots: Vec<(usize, i32, i32, i32, i32)>,
    pub source_w: f32,
    pub source_h: f32,
    pub active: bool,
    pub bg_handle: Option<Handle<Image>>,
}

impl ImagemapState {
    pub fn clear(&mut self) {
        self.hotspots.clear();
        self.active = false;
        self.source_w = 0.0;
        self.source_h = 0.0;
        self.bg_handle = None;
    }

    pub fn hit_test(&self, cursor_x: f32, cursor_y: f32, win_w: f32, win_h: f32) -> Option<usize> {
        if !self.active || self.hotspots.is_empty() || self.source_w == 0.0 || self.source_h == 0.0
        {
            return None;
        }
        let nx = cursor_x / win_w;
        let ny = cursor_y / win_h;
        let src_x = (nx * self.source_w) as i32;
        let src_y = (ny * self.source_h) as i32;
        for &(idx, x1, y1, x2, y2) in &self.hotspots {
            if src_x >= x1 && src_x < x2 && src_y >= y1 && src_y < y2 {
                return Some(idx);
            }
        }
        None
    }
}

// ─── Audio ────────────────────────────────────────────────────────────────────

#[derive(Resource, Default)]
pub struct MusicEntity(pub Option<Entity>);

#[derive(Resource)]
pub struct MusicVolume(pub f32);

impl Default for MusicVolume {
    fn default() -> Self {
        Self(1.0)
    }
}

// ─── Typewriter ───────────────────────────────────────────────────────────────

#[derive(Resource)]
pub struct TypewriterConfig {
    pub enabled: bool,
    pub chars_per_sec: f32,
}

impl Default for TypewriterConfig {
    fn default() -> Self {
        Self {
            // Settings defaults to typewriter = true, so the runtime default must match
            // the menu display. Authors can still disable it from Settings or by script.
            enabled: true,
            chars_per_sec: 40.0,
        }
    }
}

#[derive(Resource)]
pub struct TypewriterState {
    pub full_text: String,
    pub visible_chars: usize,
    pub elapsed: f32,
    pub chars_per_sec: f32,
    pub typing: bool,
}

impl Default for TypewriterState {
    fn default() -> Self {
        Self {
            full_text: String::new(),
            visible_chars: 0,
            elapsed: 0.0,
            chars_per_sec: 40.0,
            typing: false,
        }
    }
}

impl TypewriterState {
    pub fn start(&mut self, text: String, speed: f32) {
        self.full_text = text;
        self.visible_chars = 0;
        self.elapsed = 0.0;
        self.chars_per_sec = speed;
        if self.chars_per_sec > 0.0 {
            self.typing = true;
        } else {
            self.visible_chars = self.full_text.chars().count();
            self.typing = false;
        }
    }

    pub fn skip(&mut self) {
        if self.chars_per_sec > 0.0 {
            self.elapsed = (self.full_text.chars().count() as f32 / self.chars_per_sec) + 1.0;
        } else {
            self.visible_chars = self.full_text.chars().count();
            self.typing = false;
        }
    }

    pub fn is_done(&self) -> bool {
        !self.typing || self.visible_chars >= self.full_text.chars().count()
    }

    pub fn current_slice(&self) -> &str {
        let byte_idx = self
            .full_text
            .char_indices()
            .nth(self.visible_chars)
            .map(|(i, _)| i)
            .unwrap_or(self.full_text.len());
        &self.full_text[..byte_idx]
    }
}

// ─── Localisation ─────────────────────────────────────────────────────────────

/// Configuration de localisation + timestamps pour le hot-reload.
#[allow(dead_code)]
#[derive(Resource)]
pub struct LocaleConfig {
    pub default_lang: String,
    pub available_langs: Vec<String>,
    /// Timestamp du dernier chargement du fichier de la langue courante.
    pub last_modified_current: std::time::SystemTime,
    /// Timestamp du dernier chargement du fichier de fallback.
    pub last_modified_default: std::time::SystemTime,
}

impl LocaleConfig {
    pub fn new(default_lang: String, available_langs: Vec<String>) -> Self {
        Self {
            default_lang,
            available_langs,
            last_modified_current: std::time::SystemTime::UNIX_EPOCH,
            last_modified_default: std::time::SystemTime::UNIX_EPOCH,
        }
    }
}

// ─── Thème ────────────────────────────────────────────────────────────────────

#[derive(Resource, Deserialize, Serialize, Clone, Debug)]
pub struct Theme {
    pub textbox: TextboxTheme,
    pub text: TextThemeConfig,
    pub choice: ChoiceTheme,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct TextboxTheme {
    pub background_color: String,
    pub height: f32,
    pub padding: f32,
    #[serde(default)]
    pub image_path: Option<String>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct TextThemeConfig {
    pub name: TextTheme,
    pub dialogue: TextTheme,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct TextTheme {
    pub font_size: f32,
    pub color: String,
    #[serde(default)]
    pub font_path: Option<String>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct ChoiceTheme {
    pub background_color: String,
    pub text_color: String,
    pub number_color: String,
    pub font_size: f32,
    #[serde(default)]
    pub font_path: Option<String>,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            textbox: TextboxTheme {
                background_color: "#000000D9".to_string(),
                height: 170.0,
                padding: 25.0,
                image_path: None,
            },
            text: TextThemeConfig {
                name: TextTheme {
                    font_size: 24.0,
                    color: "#FFD700".to_string(),
                    font_path: None,
                },
                dialogue: TextTheme {
                    font_size: 22.0,
                    color: "#FFFFFF".to_string(),
                    font_path: None,
                },
            },
            choice: ChoiceTheme {
                background_color: "#0D0D2ED9".to_string(),
                text_color: "#FFFFFF".to_string(),
                number_color: "#FFD700E6".to_string(),
                font_size: 20.0,
                font_path: None,
            },
        }
    }
}

/// Watcher for the UI theme file. Stores the path to the theme file and the last
/// modification time observed so that we can hot‑reload the theme when it changes.
#[derive(Resource, Clone)]
pub struct ThemeWatcher {
    /// The absolute path to the theme file to watch.
    pub path: String,
    /// The last modification timestamp of the theme file when it was loaded.
    pub last_modified: std::time::SystemTime,
}

impl ThemeWatcher {
    /// Creates a new `ThemeWatcher` for the given theme file path. If the file
    /// exists on disk, its current modification time is recorded; otherwise
    /// the `last_modified` field defaults to `SystemTime::UNIX_EPOCH`.
    pub fn new(path: impl Into<String>) -> Self {
        let path_str = path.into();
        let last_modified = std::fs::metadata(&path_str)
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        Self {
            path: path_str,
            last_modified,
        }
    }
}

// ─── Historique des dialogues ─────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct HistoryLine {
    pub character: String,
    pub text: String,
}

#[derive(Resource)]
pub struct DialogueHistory {
    pub lines: Vec<HistoryLine>,
    pub max_lines: usize,
}

impl Default for DialogueHistory {
    fn default() -> Self {
        Self {
            lines: Vec::new(),
            max_lines: 50,
        }
    }
}

impl DialogueHistory {
    pub fn add(&mut self, character: String, text: String) {
        if let Some(last) = self.lines.last() {
            if last.text == text && last.character == character {
                return;
            }
        }
        self.lines.push(HistoryLine { character, text });
        if self.lines.len() > self.max_lines {
            self.lines.remove(0);
        }
    }

    pub fn clear(&mut self) {
        self.lines.clear();
    }
}
