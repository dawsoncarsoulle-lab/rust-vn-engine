use rvn_parser::{Position, Transition, Value};
use std::collections::HashMap;

// ─── SPRITE ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SpriteState {
    pub emotion: Option<String>,
    pub position: Position,
    pub visible: bool,
}

impl SpriteState {
    pub fn new(emotion: Option<String>, position: Position) -> Self {
        Self {
            emotion,
            position,
            visible: true,
        }
    }
}

// ─── AUDIO ───────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MusicState {
    pub current_file: Option<String>,
    pub volume: f32,
}

impl MusicState {
    pub fn new() -> Self {
        Self {
            current_file: None,
            volume: 1.0,
        }
    }
}

// ─── TYPEWRITER ──────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TypewriterState {
    pub enabled: bool,
    pub chars_per_sec: u32,
}

impl TypewriterState {
    pub fn new() -> Self {
        Self {
            enabled: false,
            chars_per_sec: 40,
        }
    }

    pub fn effective_speed(&self) -> f32 {
        if self.enabled && self.chars_per_sec > 0 {
            self.chars_per_sec as f32
        } else {
            0.0
        }
    }
}

impl Default for TypewriterState {
    fn default() -> Self {
        Self::new()
    }
}

// ─── CINEMATIC ───────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CinematicState {
    pub current: Option<String>,
    pub transition: Option<String>,
}

// ─── GAME STATE ──────────────────────────────────────────────────────────────

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct DialogueSnapshot {
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub consumer_dialogue:Option<crate::consumer_schedule::BoundConsumerDialogue>,
    pub pc: usize,
    pub vars: HashMap<String, Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lexical_globals: Option<HashMap<String, Value>>,
    #[serde(default)]
    pub random: crate::random::RandomState,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct GameState {
    /// Source-admitted standalone Game lifetime; never a synthetic Scene.
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub game_tasks:Option<crate::game_tasks::GameTasksState>,
    /// Native Game audio lifetime; absent in historical and Scene stories.
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub game_audio:Option<crate::scene_resources::ResourceRegistry>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub consumer_dialogue:Option<crate::consumer_schedule::BoundConsumerDialogue>,
    /// Ordinary game/pre-scene owner. Scene visits carry their own parked frame.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consumer_frame: Option<crate::consumer_schedule::BoundConsumerFrame>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub consumer_choice:Option<crate::consumer_schedule::BoundConsumerChoice>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub consumer_imagemap:Option<crate::consumer_schedule::BoundConsumerImagemap>,
    /// Absent in legacy stories. Explicit scenes retain both live continuation
    /// and frozen display input, including suspended callers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scene_execution: Option<crate::scene::SceneExecution>,
    #[serde(default)]
    pub accessibility: rvn_ui::accessibility::AccessibilitySettings,
    /// Transaction-local speech requests. They must not be replayed by load or rollback.
    #[serde(skip)]
    pub(crate) speech_requests: Vec<rvn_ui::accessibility::SpeechRequest>,
    /// Event-local requests: never persisted or replayed by load/rollback.
    #[serde(skip)]
    pub(crate) menu_requests: Vec<rvn_ui::source_menus::MenuEffect>,
    #[serde(default)]
    pub videos: crate::video::VideoState,
    #[serde(default)]
    pub layered: crate::composition::LayeredState,
    #[serde(default)]
    pub motions: crate::motion::MotionState,
    #[serde(default)]
    pub ui: crate::ui::UiState,
    /// Compatibility identity of the prepared story, not a security checksum.
    #[serde(default)]
    pub story_identity: Option<String>,
    #[serde(default)]
    pub random: crate::random::RandomState,
    /// A separate stream keeps redraws and language switches side-effect free.
    #[serde(default)]
    pub display_random: crate::random::RandomState,
    #[serde(default)]
    pub display_random_pc: Option<usize>,
    #[serde(default)]
    pub last_dialogue: Option<DialogueSnapshot>,
    pub pc: usize,
    pub current_interactive_pc: usize,
    pub background_image: String,
    pub vars: HashMap<String, Value>,
    pub call_stack: Vec<usize>,
    pub last_transition: Transition,
    pub sprites: HashMap<String, SpriteState>,
    pub cinematic: CinematicState,
    pub music: MusicState,
    pub typewriter: TypewriterState,
}
