// rvn_parser/src/ast.rs
//
// Types de l'AST.
//
// CHANGEMENTS vs version précédente :
//   - `Condition` est maintenant un alias de `Expr`.
//   - `Statement::SetVar` prend un `Expr` au lieu d'un `Value` littéral.
//   - `Statement::Dialogue` prend un `InterpolatedText`.
//   - `Statement::Choice` : labels de choix aussi en `InterpolatedText`.

use crate::expr::{Expr, InterpolatedText};
use serde::{Deserialize, Serialize};

// ─── POSITION ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Position {
    Left,
    Center,
    Right,
    Custom(f32),
}

impl Position {
    pub fn to_normalized(&self) -> f32 {
        match self {
            Position::Left => 0.2,
            Position::Center => 0.5,
            Position::Right => 0.8,
            Position::Custom(x) => x.clamp(0.0, 1.0),
        }
    }
}

impl std::fmt::Display for Position {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Position::Left => write!(f, "left"),
            Position::Center => write!(f, "center"),
            Position::Right => write!(f, "right"),
            Position::Custom(x) => write!(f, "at({x:.2})"),
        }
    }
}

// ─── TRANSITIONS ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Transition {
    Fade { duration_ms: u32 },
    Dissolve { duration_ms: u32 },
    SlideLeft { duration_ms: u32 },
    SlideRight { duration_ms: u32 },
    SlideUp { duration_ms: u32 },
    SlideDown { duration_ms: u32 },
    ZoomIn { duration_ms: u32 },
    ZoomOut { duration_ms: u32 },
    Wipe { duration_ms: u32 },
    Blur { duration_ms: u32 },
    None,
}

impl Transition {
    pub const DEFAULT_FADE_MS: u32 = 500;
    pub const DEFAULT_DISSOLVE_MS: u32 = 300;
    pub const DEFAULT_SLIDE_MS: u32 = 400;
    pub const DEFAULT_ZOOM_MS: u32 = 400;
    pub const DEFAULT_WIPE_MS: u32 = 500;
    pub const DEFAULT_BLUR_MS: u32 = 400;

    /// Returns the duration in milliseconds for any non-None transition.
    pub fn duration_ms(&self) -> u32 {
        match self {
            Transition::Fade { duration_ms }
            | Transition::Dissolve { duration_ms }
            | Transition::SlideLeft { duration_ms }
            | Transition::SlideRight { duration_ms }
            | Transition::SlideUp { duration_ms }
            | Transition::SlideDown { duration_ms }
            | Transition::ZoomIn { duration_ms }
            | Transition::ZoomOut { duration_ms }
            | Transition::Wipe { duration_ms }
            | Transition::Blur { duration_ms } => *duration_ms,
            Transition::None => 0,
        }
    }
}

impl std::fmt::Display for Transition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Transition::Fade { duration_ms } => write!(f, "fade({}ms)", duration_ms),
            Transition::Dissolve { duration_ms } => write!(f, "dissolve({}ms)", duration_ms),
            Transition::SlideLeft { duration_ms } => write!(f, "slideleft({}ms)", duration_ms),
            Transition::SlideRight { duration_ms } => write!(f, "slideright({}ms)", duration_ms),
            Transition::SlideUp { duration_ms } => write!(f, "slideup({}ms)", duration_ms),
            Transition::SlideDown { duration_ms } => write!(f, "slidedown({}ms)", duration_ms),
            Transition::ZoomIn { duration_ms } => write!(f, "zoomin({}ms)", duration_ms),
            Transition::ZoomOut { duration_ms } => write!(f, "zoomout({}ms)", duration_ms),
            Transition::Wipe { duration_ms } => write!(f, "wipe({}ms)", duration_ms),
            Transition::Blur { duration_ms } => write!(f, "blur({}ms)", duration_ms),
            Transition::None => write!(f, "none"),
        }
    }
}

// ─── VALUE ───────────────────────────────────────────────────────────────────

/// Valeur d'exécution produite par l'évaluateur. Stockée dans GameState::vars.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Value {
    Bool(bool),
    Int(i64),
    Float(f32),
    Str(String),
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Bool(b) => write!(f, "{b}"),
            Value::Int(n) => write!(f, "{n}"),
            Value::Float(x) => write!(f, "{x}"),
            Value::Str(s) => write!(f, "{s}"),
        }
    }
}

// ─── CONDITIONS ──────────────────────────────────────────────────────────────
//
// Condition = Expr — unifie le système de types.
// L'évaluateur interprète le résultat comme booléen.

pub type Condition = Expr;

// ─── IMAGEMAP ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct Rect {
    pub x1: i32,
    pub y1: i32,
    pub x2: i32,
    pub y2: i32,
}

impl Rect {
    pub fn new(x1: i32, y1: i32, x2: i32, y2: i32) -> Self {
        Self { x1, y1, x2, y2 }
    }

    pub fn contains(&self, px: i32, py: i32) -> bool {
        px >= self.x1 && px < self.x2 && py >= self.y1 && py < self.y2
    }

    pub fn width(&self) -> i32 {
        self.x2 - self.x1
    }
    pub fn height(&self) -> i32 {
        self.y2 - self.y1
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Hotspot {
    pub name: Option<String>,
    pub area: Rect,
    pub hover_area: Option<Rect>,
    pub body: Vec<Statement>,
}

// ─── ANIMATIONS ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum AnimationValue {
    Bool(bool),
    Int(i64),
    Float(f32),
    Str(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnimationParam {
    pub name: String,
    pub value: AnimationValue,
}

// ─── AST ─────────────────────────────────────────────────────────────────────

/// A single option within a `choice` block.
/// `condition` is optional — when present, the option only appears if it evaluates to true.
#[derive(Debug, Clone, PartialEq)]
pub struct ChoiceOption {
    pub label: InterpolatedText,
    pub condition: Option<Condition>,
    pub body: Vec<Statement>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    /// Charge un ou plusieurs fichiers .rvn avant exécution.
    /// Cette instruction est résolue par le loader de projet avant de créer le moteur.
    Use {
        paths: Vec<String>,
    },
    Init {
        body: Vec<Statement>,
    },
    Config {
        key: String,
        value: String,
    },
    CharacterCreate {
        id: String,
        display_name: String,
    },

    /// Texte potentiellement interpolé : "Bonjour [prenom], tu as [score] pts !"
    Dialogue {
        character_id: Option<String>,
        text: InterpolatedText,
    },

    /// Labels de choix interpolés aussi.
    Choice {
        options: Vec<ChoiceOption>,
    },

    /// La valeur est une expression complète (pas juste un littéral).
    SetVar {
        name: String,
        value: Expr,
    },

    If {
        condition: Condition,
        then_branch: Vec<Statement>,
        else_branch: Vec<Statement>,
    },

    Label {
        name: String,
    },
    Jump {
        target: String,
    },
    Call {
        target: String,
    },
    Return,
    Scene {
        background: String,
        transition: Transition,
    },
    CinematicShow {
        id: String,
        transition: Option<String>,
    },
    CinematicHide {
        transition: Option<String>,
    },
    UnlockEnding {
        id: String,
    },

    ShowSprite {
        character_id: String,
        emotion: Option<String>,
        position: Option<Position>,
        transition: Transition,
    },
    HideSprite {
        character_id: String,
        transition: Transition,
    },
    MoveSprite {
        character_id: String,
        position: Position,
        transition: Transition,
    },
    SpriteAnimate {
        character_id: String,
        animation: String,
        params: Vec<AnimationParam>,
    },
    SpriteStopAnimation {
        character_id: String,
    },
    /// Apply visual effects to a sprite: flip, scale, rotation, tint.
    SpriteEffect {
        character_id: String,
        flip_x: Option<bool>,
        flip_y: Option<bool>,
        scale: Option<f32>,
        rotation: Option<f32>,
        tint: Option<String>,
    },
    MethodCall {
        target: String,
        method: String,
        arg: Option<String>,
        transition: Transition,
    },

    MusicPlay {
        file: String,
        transition: Transition,
    },
    MusicStop {
        transition: Transition,
    },
    MusicVolume {
        level: f32,
    },
    SfxPlay {
        file: String,
        transition: Transition,
    },
    SfxStop {
        file: String,
        transition: Transition,
    },

    Imagemap {
        background: String,
        hover_image: Option<String>,
        hotspots: Vec<Hotspot>,
    },

    TypewriterSet {
        enabled: bool,
    },
    TypewriterSpeed {
        chars_per_sec: u32,
    },
}

pub type Script = Vec<Statement>;
