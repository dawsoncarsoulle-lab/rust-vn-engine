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

// ─── GAME STATE ──────────────────────────────────────────────────────────────

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct GameState {
    pub pc: usize,
    pub current_interactive_pc: usize,
    pub background_image: String,
    pub vars: HashMap<String, Value>,
    pub call_stack: Vec<usize>,
    pub last_transition: Transition,
    pub sprites: HashMap<String, SpriteState>,
    pub music: MusicState,
    pub typewriter: TypewriterState,
}
