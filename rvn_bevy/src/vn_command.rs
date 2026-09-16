use bevy::prelude::*;
use rvn_parser::{AnimationParam, Position, Transition};

#[derive(Debug, Clone)]
pub struct ImagemapZone {
    pub area: rvn_parser::Rect,
    pub hover_area: rvn_parser::Rect,
}

impl From<&rvn_parser::Hotspot> for ImagemapZone {
    fn from(value: &rvn_parser::Hotspot) -> Self {
        Self { area: value.area.clone(), hover_area: value.hover_area.clone().unwrap_or_else(|| value.area.clone()) }
    }
}

#[derive(Event, Debug, Clone)]
pub enum VnCommand {
    /// Replace the rendered cast when restoring a complete saved screen.
    ClearSprites,
    // ── Visuels ───────────────────────────────────────────────────────────────
    SetBackground {
        path: String,
        transition: Transition,
    },
    ShowCinematic {
        id: String,
        transition: Option<String>,
    },
    HideCinematic {
        transition: Option<String>,
    },
    ShowSprite {
        id: String,
        emotion: Option<String>,
        position: Position,
        transition: Transition,
    },
    HideSprite {
        id: String,
        transition: Transition,
    },
    MoveSprite {
        id: String,
        position: Position,
        transition: Transition,
    },
    AnimateSprite {
        id: String,
        animation: String,
        params: Vec<AnimationParam>,
    },
    StopSpriteAnimation {
        id: String,
    },
    SetSpriteEffect {
        id: String,
        flip_x: Option<bool>,
        flip_y: Option<bool>,
        scale: Option<f32>,
        rotation: Option<f32>,
        tint: Option<String>,
    },

    // ── Dialogue / choix ──────────────────────────────────────────────────────
    ShowDialogue {
        character: Option<String>,
        text: String,
    },
    ShowChoice {
        options: Vec<String>,
    },

    // ── Imagemap ──────────────────────────────────────────────────────────────
    ShowImagemap {
        background: String,
        hover_image: Option<String>,
        hotspots: Vec<ImagemapZone>,
    },

    // ── Audio ─────────────────────────────────────────────────────────────────
    /// `transition` implémenté : Fade/Dissolve → crossfade réel.
    MusicPlay {
        file: String,
        transition: Transition,
    },
    MusicStop,
    MusicSetVolume {
        level: f32,
    },
    SfxPlay {
        file: String,
    },
    SfxStop {
        file: String,
    },
    VoicePlay {
        file: String,
    },
    VoiceStop,

    // ── Meta ──────────────────────────────────────────────────────────────────
    ScriptFinished,
    UnlockEnding {
        id: String,
    },
    SetTypewriterConfig {
        speed_cps: f32,
    },
}

#[derive(Event, Debug, Clone)]
pub enum PlayerInput {
    Advance,
    Choose(usize),
    Rollback,
    SkipTypewriter,
    ToggleMenu,
    OpenHistory,
    QuickSave,
    QuickLoad,
}
