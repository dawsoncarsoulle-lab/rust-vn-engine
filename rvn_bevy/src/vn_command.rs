// rvn_bevy/src/vn_command.rs
//
// CHANGEMENTS vs version précédente :
//   - MusicPlay : le champ `transition` est restauré.
//     Il était supprimé car non-implémenté — il l'est maintenant (crossfade).
//   - MusicStop : reste unitaire (pas de fade-out demandé pour l'instant).

use bevy::prelude::*;
use rvn_parser::{Position, Transition};

#[derive(Event, Debug, Clone)]
pub enum VnCommand {
    // ── Visuels ───────────────────────────────────────────────────────────────
    SetBackground {
        path: String,
        transition: Transition,
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
        hotspots: Vec<(Option<String>, (i32, i32, i32, i32))>,
    },

    // ── Audio ─────────────────────────────────────────────────────────────────
    /// `transition` implémenté : Fade/Dissolve → crossfade réel.
    MusicPlay {
        file: String,
        transition: Transition,
        #[allow(dead_code)]
        previous: Option<String>,
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

    // ── Meta ──────────────────────────────────────────────────────────────────
    ScriptFinished,
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
}
