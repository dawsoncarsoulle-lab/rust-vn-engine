// rvn_bevy/src/resources.rs
//
// CHANGEMENTS vs version précédente :
//   - Ajout : `ChoiceFocus` resource (navigation clavier dans les choix).

use crate::bevy_renderer::BevyRenderer;
use bevy::prelude::*;
use rvn_core::{Engine, PersistentData, PersistentDataManager, RichTextSegment};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Resource)]
pub struct VnEngine(pub Engine<BevyRenderer>);

#[derive(Resource, Default)]
pub struct CgAssetRegistry(pub HashMap<String, String>);

#[derive(Resource, Default)]
pub struct MusicAssetRegistry(pub HashMap<String, String>);

impl MusicAssetRegistry {
    pub fn resolve(&self, file: &str) -> String {
        let normalized = file.replace('\\', "/");
        let normalized = normalized.strip_prefix("assets/").unwrap_or(&normalized);

        if let Some(path) = self.0.get(normalized) {
            return path.clone();
        }

        let music_path = if normalized.starts_with("music/") {
            normalized.to_string()
        } else {
            format!("music/{normalized}")
        };
        if self.0.values().any(|path| path == &music_path) {
            return music_path;
        }

        let Some(file_name) = normalized.rsplit('/').next() else {
            return music_path;
        };
        if let Some(stem) = file_name.rsplit_once('.').map(|(stem, _)| stem) {
            if let Some(path) = self.0.get(stem) {
                return path.clone();
            }
        }

        music_path
    }
}

#[derive(Resource)]
pub struct PersistentDataResource {
    pub manager: PersistentDataManager,
    pub data: PersistentData,
}

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
    Gallery,
    Finished,
    Error,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum GalleryView {
    #[default]
    Cg,
    Endings,
}

#[derive(Resource, Default)]
pub struct GalleryState {
    pub view: GalleryView,
    pub selected_cg: Option<String>,
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

#[cfg(target_arch = "wasm32")]
#[derive(Clone, Debug)]
pub struct PendingMusicPlayback {
    pub file: String,
    pub fade_ms: Option<u32>,
}

#[derive(Resource, Default)]
pub struct MusicPlaybackState {
    #[cfg(target_arch = "wasm32")]
    pub last_request: Option<PendingMusicPlayback>,
    #[cfg(target_arch = "wasm32")]
    pub web_resume_attempts: u32,
    #[cfg(target_arch = "wasm32")]
    pub web_music_replay_pending: bool,
}

#[derive(Resource)]
pub struct MusicVolume(pub f32);

impl Default for MusicVolume {
    fn default() -> Self {
        Self(1.0)
    }
}

#[derive(Resource, Clone, Debug)]
pub struct ProjectTitle(pub String);

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
    pub segments: Vec<RichTextSegment>,
    pub visible_chars: usize,
    pub chars_per_sec: f32,
    pub typing: bool,
    pub pause_remaining: f32,
    pub char_progress: f32,
}

impl Default for TypewriterState {
    fn default() -> Self {
        Self {
            full_text: String::new(),
            segments: Vec::new(),
            visible_chars: 0,
            chars_per_sec: 40.0,
            typing: false,
            pause_remaining: 0.0,
            char_progress: 0.0,
        }
    }
}

impl TypewriterState {
    pub fn start_segments(&mut self, segments: Vec<RichTextSegment>, speed: f32) {
        self.full_text = segments
            .iter()
            .map(|segment| segment.text.as_str())
            .collect();
        self.segments = segments;
        self.visible_chars = 0;
        self.chars_per_sec = speed;
        self.pause_remaining = 0.0;
        self.char_progress = 0.0;
        if self.chars_per_sec > 0.0 {
            self.typing = true;
        } else {
            self.visible_chars = self.full_text.chars().count();
            self.typing = false;
        }
    }

    pub fn skip(&mut self) {
        self.visible_chars = self.full_text.chars().count();
        self.pause_remaining = 0.0;
        self.typing = false;
    }

    pub fn is_done(&self) -> bool {
        !self.typing || self.visible_chars >= self.full_text.chars().count()
    }

    pub fn speed_for_next_char(&self) -> f32 {
        let mut seen = 0;
        for segment in &self.segments {
            let len = segment.text.chars().count();
            if self.visible_chars < seen + len {
                return self.chars_per_sec * segment.speed.unwrap_or(1.0).max(0.01);
            }
            seen += len;
        }
        self.chars_per_sec
    }

    pub fn pause_after_visible_char(&self) -> Option<f32> {
        if self.visible_chars == 0 {
            return None;
        }
        let mut seen = 0;
        for segment in &self.segments {
            seen += segment.text.chars().count();
            if self.visible_chars == seen {
                return segment.pause_after;
            }
        }
        None
    }
}

// ─── Localisation ─────────────────────────────────────────────────────────────

/// Configuration de localisation + timestamps pour le hot-reload.
#[derive(Resource)]
pub struct LocaleConfig {
    pub available_langs: Vec<String>,
    /// Timestamp du dernier chargement du fichier de la langue courante.
    pub last_modified_current: std::time::SystemTime,
    /// Timestamp du dernier chargement du fichier de fallback.
    pub last_modified_default: std::time::SystemTime,
}

// ─── Thème ────────────────────────────────────────────────────────────────────

#[derive(Resource, Deserialize, Serialize, Clone, Debug)]
pub struct Theme {
    pub textbox: TextboxTheme,
    pub text: TextThemeConfig,
    pub choice: ChoiceTheme,
    #[serde(default)]
    pub title_screen: TitleScreenTheme,
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

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct TitleScreenTheme {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub background: Option<TitleBackgroundSource>,
    #[serde(default)]
    pub logo: Option<TitleLogoSource>,
    #[serde(default)]
    pub music: Option<String>,
    #[serde(default)]
    pub layout: TitleLayout,
    #[serde(default)]
    pub button_align: TitleButtonAlign,
    #[serde(default = "default_title_button_x")]
    pub button_x: f32,
    #[serde(default = "default_title_button_y")]
    pub button_y: f32,
    #[serde(default = "default_title_button_spacing")]
    pub button_spacing: f32,
    #[serde(default = "default_title_button_order")]
    pub button_order: Vec<String>,
    #[serde(default = "default_true")]
    pub show_continue: bool,
    #[serde(default = "default_true")]
    pub show_new_game: bool,
    #[serde(default = "default_true")]
    pub show_load: bool,
    #[serde(default = "default_true")]
    pub show_gallery: bool,
    #[serde(default = "default_true")]
    pub show_settings: bool,
    #[serde(default = "default_true")]
    pub show_quit: bool,
    #[serde(default)]
    pub title: TitleTextTheme,
    #[serde(default)]
    pub buttons: TitleButtonTheme,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
#[serde(rename_all = "snake_case")]
pub enum TitleLayout {
    #[default]
    Vertical,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
#[serde(rename_all = "snake_case")]
pub enum TitleButtonAlign {
    Left,
    #[default]
    Center,
    Right,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct TitleTextTheme {
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub anchor: Option<TitleAnchor>,
    #[serde(default)]
    pub offset_x: Option<f32>,
    #[serde(default)]
    pub offset_y: Option<f32>,
    #[serde(default = "default_title_x")]
    pub x: f32,
    #[serde(default = "default_title_y")]
    pub y: f32,
    #[serde(default = "default_title_font_size")]
    pub font_size: f32,
    #[serde(default)]
    pub color: Option<String>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct TitleButtonTheme {
    #[serde(default)]
    pub anchor: Option<TitleAnchor>,
    #[serde(default)]
    pub offset_x: Option<f32>,
    #[serde(default)]
    pub offset_y: Option<f32>,
    #[serde(default)]
    pub spacing: Option<f32>,
    #[serde(default = "default_title_button_width")]
    pub width: f32,
    #[serde(default = "default_title_button_height")]
    pub height: f32,
    #[serde(default = "default_title_button_font_size")]
    pub font_size: f32,
    #[serde(default)]
    pub visibility: TitleButtonVisibility,
    #[serde(default)]
    pub labels: TitleButtonLabels,
    #[serde(default)]
    pub style: TitleButtonStyle,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum TitleBackgroundSource {
    Path(String),
    Config(TitleBackgroundTheme),
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct TitleBackgroundTheme {
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub mode: TitleBackgroundMode,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
#[serde(rename_all = "snake_case")]
pub enum TitleBackgroundMode {
    #[default]
    Cover,
    Contain,
    Stretch,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum TitleLogoSource {
    Path(String),
    Config(TitleLogoTheme),
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct TitleLogoTheme {
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub anchor: TitleAnchor,
    #[serde(default)]
    pub offset_x: f32,
    #[serde(default = "default_logo_offset_y")]
    pub offset_y: f32,
    #[serde(default = "default_logo_scale")]
    pub scale: f32,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
#[serde(rename_all = "snake_case")]
pub enum TitleAnchor {
    TopLeft,
    #[default]
    TopCenter,
    TopRight,
    CenterLeft,
    Center,
    CenterRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct TitleButtonVisibility {
    #[serde(default, rename = "continue")]
    pub continue_button: Option<bool>,
    #[serde(default)]
    pub new_game: Option<bool>,
    #[serde(default)]
    pub load: Option<bool>,
    #[serde(default)]
    pub gallery: Option<bool>,
    #[serde(default)]
    pub settings: Option<bool>,
    #[serde(default)]
    pub quit: Option<bool>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct TitleButtonStyle {
    #[serde(default = "default_title_button_bg")]
    pub background_color: String,
    #[serde(default = "default_title_button_hover")]
    pub hover_color: String,
    #[serde(default = "default_title_button_pressed")]
    pub pressed_color: String,
    #[serde(default = "default_title_button_text")]
    pub text_color: String,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct TitleButtonLabels {
    #[serde(default, rename = "continue")]
    pub continue_label: Option<String>,
    #[serde(default)]
    pub new_game: Option<String>,
    #[serde(default)]
    pub load: Option<String>,
    #[serde(default)]
    pub gallery: Option<String>,
    #[serde(default)]
    pub settings: Option<String>,
    #[serde(default)]
    pub quit: Option<String>,
}

impl Default for TitleScreenTheme {
    fn default() -> Self {
        Self {
            enabled: true,
            background: None,
            logo: None,
            music: None,
            layout: TitleLayout::Vertical,
            button_align: TitleButtonAlign::Center,
            button_x: default_title_button_x(),
            button_y: default_title_button_y(),
            button_spacing: default_title_button_spacing(),
            button_order: default_title_button_order(),
            show_continue: true,
            show_new_game: true,
            show_load: true,
            show_gallery: true,
            show_settings: true,
            show_quit: true,
            title: TitleTextTheme::default(),
            buttons: TitleButtonTheme::default(),
        }
    }
}

impl Default for TitleTextTheme {
    fn default() -> Self {
        Self {
            text: None,
            anchor: None,
            offset_x: None,
            offset_y: None,
            x: default_title_x(),
            y: default_title_y(),
            font_size: default_title_font_size(),
            color: None,
        }
    }
}

impl Default for TitleButtonTheme {
    fn default() -> Self {
        Self {
            anchor: None,
            offset_x: None,
            offset_y: None,
            spacing: None,
            width: default_title_button_width(),
            height: default_title_button_height(),
            font_size: default_title_button_font_size(),
            visibility: TitleButtonVisibility::default(),
            labels: TitleButtonLabels::default(),
            style: TitleButtonStyle::default(),
        }
    }
}

impl Default for TitleButtonStyle {
    fn default() -> Self {
        Self {
            background_color: default_title_button_bg(),
            hover_color: default_title_button_hover(),
            pressed_color: default_title_button_pressed(),
            text_color: default_title_button_text(),
        }
    }
}

impl TitleBackgroundSource {
    pub fn path(&self) -> Option<&str> {
        match self {
            Self::Path(path) => Some(path.as_str()),
            Self::Config(config) => config.path.as_deref(),
        }
    }

    pub fn mode(&self) -> TitleBackgroundMode {
        match self {
            Self::Path(_) => TitleBackgroundMode::Cover,
            Self::Config(config) => config.mode.clone(),
        }
    }
}

impl TitleLogoSource {
    pub fn path(&self) -> Option<&str> {
        match self {
            Self::Path(path) => Some(path.as_str()),
            Self::Config(config) => config.path.as_deref(),
        }
    }

    pub fn anchor(&self) -> TitleAnchor {
        match self {
            Self::Path(_) => TitleAnchor::TopCenter,
            Self::Config(config) => config.anchor.clone(),
        }
    }

    pub fn offset_x(&self) -> f32 {
        match self {
            Self::Path(_) => 0.0,
            Self::Config(config) => config.offset_x,
        }
    }

    pub fn offset_y(&self) -> f32 {
        match self {
            Self::Path(_) => default_logo_offset_y(),
            Self::Config(config) => config.offset_y,
        }
    }

    pub fn scale(&self) -> f32 {
        match self {
            Self::Path(_) => default_logo_scale(),
            Self::Config(config) => config.scale.max(0.01),
        }
    }
}

impl Default for TitleButtonLabels {
    fn default() -> Self {
        Self {
            continue_label: Default::default(),
            new_game: Default::default(),
            load: Default::default(),
            gallery: Default::default(),
            settings: Default::default(),
            quit: Default::default(),
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_title_x() -> f32 {
    0.5
}

fn default_title_y() -> f32 {
    0.18
}

fn default_title_font_size() -> f32 {
    80.0
}

fn default_title_button_x() -> f32 {
    0.5
}

fn default_title_button_y() -> f32 {
    0.62
}

fn default_title_button_spacing() -> f32 {
    20.0
}

fn default_title_button_order() -> Vec<String> {
    [
        "continue", "new_game", "load", "gallery", "settings", "quit",
    ]
    .iter()
    .map(|item| item.to_string())
    .collect()
}

fn default_title_button_width() -> f32 {
    350.0
}

fn default_title_button_height() -> f32 {
    60.0
}

fn default_title_button_font_size() -> f32 {
    30.0
}

fn default_logo_offset_y() -> f32 {
    32.0
}

fn default_logo_scale() -> f32 {
    1.0
}

fn default_title_button_bg() -> String {
    "#262640FF".to_string()
}

fn default_title_button_hover() -> String {
    "#404073FF".to_string()
}

fn default_title_button_pressed() -> String {
    "#5959A6FF".to_string()
}

fn default_title_button_text() -> String {
    "#FFFFFF".to_string()
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
            title_screen: TitleScreenTheme::default(),
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

#[cfg(test)]
mod theme_tests {
    use super::*;

    const BASE_THEME: &str = r##"
[textbox]
background_color = "#000000D9"
height = 170.0
padding = 25.0

[text.name]
font_size = 24.0
color = "#FFD700"

[text.dialogue]
font_size = 22.0
color = "#FFFFFF"

[choice]
background_color = "#0D0D2ED9"
text_color = "#FFFFFF"
number_color = "#FFD700E6"
font_size = 20.0
"##;

    #[test]
    fn title_screen_v1_theme_still_deserializes() {
        let source = format!(
            r#"{BASE_THEME}
[title_screen]
background = "backgrounds/title.png"
logo = "ui/logo.png"
button_x = 0.5
button_y = 0.62
button_spacing = 18.0
show_quit = false

[title_screen.title]
x = 0.5
y = 0.18
font_size = 64.0
"#
        );
        let theme: Theme = toml::from_str(&source).unwrap();
        assert!(matches!(
            theme.title_screen.background,
            Some(TitleBackgroundSource::Path(_))
        ));
        assert!(!theme.title_screen.show_quit);
        assert_eq!(theme.title_screen.button_spacing, 18.0);
    }

    #[test]
    fn title_screen_v2_theme_deserializes() {
        let source = format!(
            r##"{BASE_THEME}
[title_screen]
music = "music/title.ogg"
button_order = ["new_game", "quit"]

[title_screen.background]
path = "backgrounds/title.png"
mode = "contain"

[title_screen.logo]
path = "ui/logo.png"
anchor = "top_center"
offset_y = 32.0
scale = 1.2

[title_screen.title]
anchor = "top_center"
offset_y = 90.0
font_size = 58.0
color = "#FFFFFF"

[title_screen.buttons]
anchor = "center"
offset_y = 80.0
spacing = 12.0
width = 320.0
height = 52.0
font_size = 24.0

[title_screen.buttons.visibility]
quit = false

[title_screen.buttons.style]
background_color = "#252542DD"
hover_color = "#34345FEE"
pressed_color = "#45457AFF"
text_color = "#FFFFFF"
"##
        );
        let theme: Theme = toml::from_str(&source).unwrap();
        assert!(matches!(
            theme.title_screen.background,
            Some(TitleBackgroundSource::Config(_))
        ));
        assert_eq!(theme.title_screen.buttons.spacing, Some(12.0));
        assert_eq!(theme.title_screen.buttons.visibility.quit, Some(false));
    }
}
