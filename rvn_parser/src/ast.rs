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

fn is_false(value: &bool) -> bool { !*value }

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
            Transition::Fade { duration_ms } => write!(f, "fade({duration_ms})"),
            Transition::Dissolve { duration_ms } => write!(f, "dissolve({duration_ms})"),
            Transition::SlideLeft { duration_ms } => write!(f, "slideleft({duration_ms})"),
            Transition::SlideRight { duration_ms } => write!(f, "slideright({duration_ms})"),
            Transition::SlideUp { duration_ms } => write!(f, "slideup({duration_ms})"),
            Transition::SlideDown { duration_ms } => write!(f, "slidedown({duration_ms})"),
            Transition::ZoomIn { duration_ms } => write!(f, "zoomin({duration_ms})"),
            Transition::ZoomOut { duration_ms } => write!(f, "zoomout({duration_ms})"),
            Transition::Wipe { duration_ms } => write!(f, "wipe({duration_ms})"),
            Transition::Blur { duration_ms } => write!(f, "blur({duration_ms})"),
            Transition::None => write!(f, "none"),
        }
    }
}

// ─── VALUE ───────────────────────────────────────────────────────────────────

/// A portable name keeps its authored spelling and a separate ASCII identity.
/// The entry limit uses Windows reference units on every platform so a saved
/// value cannot change validity when a VN project moves between machines.
#[derive(Debug, Clone)]
pub struct Name {
    display: String,
    identity: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameError {
    EmbeddedNull,
    EntryTooLong,
}

impl std::fmt::Display for NameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::EmbeddedNull => "Un nom ne peut pas contenir de caractère nul",
            Self::EntryTooLong => "Le nom dépasse la limite de 1023 unités UTF-16",
        })
    }
}
impl std::error::Error for NameError {}

impl Default for Name {
    fn default() -> Self {
        Self { display: "None".into(), identity: "none".into() }
    }
}

impl Name {
    pub fn new(text: &str) -> Result<Self, NameError> {
        // Reject embedded nulls explicitly; native character admission is not
        // the persistence contract of this independent implementation.
        if text.contains('\0') { return Err(NameError::EmbeddedNull); }
        let base = Self::numbered_base(text).unwrap_or(text);
        if base.is_empty() { return Ok(Self::default()); }
        if base.encode_utf16().count() >= 1024 { return Err(NameError::EntryTooLong); }
        Ok(Self { display: text.into(), identity: text.to_ascii_lowercase() })
    }

    fn numbered_base(text: &str) -> Option<&str> {
        let (base, digits) = text.rsplit_once('_')?;
        if digits.is_empty() || digits.len() > 10 || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        if digits.len() > 1 && digits.starts_with('0') { return None; }
        let value = digits.parse::<u32>().ok()?;
        (value < i32::MAX as u32).then_some(base)
    }

    pub fn as_str(&self) -> &str { &self.display }
    pub fn is_none(&self) -> bool { self.identity == "none" }
}

impl PartialEq for Name {
    fn eq(&self, other: &Self) -> bool { self.identity == other.identity }
}
impl Eq for Name {}
impl std::hash::Hash for Name {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::hash::Hash::hash(&self.identity, state);
    }
}
impl std::fmt::Display for Name {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str(&self.display) }
}
impl Serialize for Name {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.display)
    }
}
impl<'de> Deserialize<'de> for Name {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let display = String::deserialize(deserializer)?;
        Self::new(&display).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod name_tests {
    use super::*;
    use std::hash::{Hash, Hasher};
    fn hash(name: &Name) -> u64 {
        let mut state = std::collections::hash_map::DefaultHasher::new();
        name.hash(&mut state); state.finish()
    }
    #[test]
    fn name_native_absence_suffix_case_and_hash_contract() {
        for source in ["", "None", "none", "_0", "_1", "_2147483646"] {
            let name = Name::new(source).unwrap();
            assert!(name.is_none()); assert_eq!(hash(&name), hash(&Name::default()));
        }
        assert_eq!(Name::new("none").unwrap().as_str(), "none");
        let authored = Name::new("RvnCase_0").unwrap();
        let same = Name::new("rvncase_0").unwrap();
        assert_eq!(authored, same); assert_eq!(hash(&authored), hash(&same));
        assert_eq!(authored.as_str(), "RvnCase_0");
        assert_eq!(same.as_str(), "rvncase_0");
        for source in ["None_0", "_00", "_01", "_2147483647"] {
            assert!(!Name::new(source).unwrap().is_none());
        }
        assert_ne!(Name::new("É").unwrap(), Name::new("é").unwrap());
        assert_ne!(authored, Name::new("RvnCase_00").unwrap());
    }
}

/// Valeur d'exécution produite par l'évaluateur. Stockée dans GameState::vars.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Value {
    Bool(bool),
    Int(i64),
    Float(f32),
    Str(String),
    /// Opt-in typed identity; existing strings retain case-sensitive semantics.
    Name(Name),
    /// Nominal identity; raw decoding remains inert until actual resource admission.
    Enum(crate::enum_asset::EnumValue),
    /// Typed canonical class data. Actual loader admission is still required.
    WidgetClass(Box<Option<crate::widget_reference::WidgetClassReference>>),
    /// A live runtime reference, or inert decoded portable data before hydration.
    Widget(Box<crate::widget_reference::WidgetReference>),
    /// Opt-in text values retain identity, source and captured format arguments.
    Text(Box<crate::localized_text::DeferredText>),
    List(Vec<Value>),
    Dict(std::collections::BTreeMap<String, Value>),
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Bool(b) => write!(f, "{b}"),
            Value::Int(n) => write!(f, "{n}"),
            Value::Float(x) => write!(f, "{x}"),
            Value::Str(s) => write!(f, "{s}"),
            Value::Name(name) => write!(f, "{name}"),
            Value::Enum(value) => write!(f, "<Enum {}:{}>",value.asset.to_hex(),value.member.to_hex()),
            Value::WidgetClass(_) => write!(f, "<WidgetClass>"),
            Value::Widget(_) => write!(f, "<WidgetReference>"),
            Value::Text(text) => match text.source_display() {
                Ok(source) => write!(f, "{source}"),
                Err(error) => write!(f, "<invalid text: {error}>"),
            },
            Value::List(items) => {
                let parts: Vec<String> = items.iter().map(|v| v.to_string()).collect();
                write!(f, "[{}]", parts.join(", "))
            }
            Value::Dict(items) => {
                let parts = items
                    .iter()
                    .map(|(key, value)| format!("{key:?}: {value}"))
                    .collect::<Vec<_>>();
                write!(f, "{{{}}}", parts.join(", "))
            }
        }
    }
}

// ─── CONDITIONS ──────────────────────────────────────────────────────────────
//
// Condition = Expr — unifie le système de types.
// L'évaluateur interprète le résultat comme booléen.

pub type Condition = Expr;

// ─── IMAGEMAP ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hotspot {
    pub name: Option<String>,
    pub area: Rect,
    pub hover_area: Option<Rect>,
    pub body: Vec<Statement>,
}

// ─── ANIMATIONS ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AnimationValue {
    Bool(bool),
    Int(i64),
    Float(f32),
    Str(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnimationParam {
    pub name: String,
    pub value: AnimationValue,
}

// ─── AST ─────────────────────────────────────────────────────────────────────

/// A single option within a `choice` block.
/// `condition` is optional — when present, the option only appears if it evaluates to true.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChoiceOption {
    pub label: InterpolatedText,
    pub condition: Option<Condition>,
    pub body: Vec<Statement>,
}

/// Ordered pure dependencies of one executed consumer. IDs are exact source
/// identities, not variable names; captured values never enter user variables.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ConsumerStep {
    Capture { producer: String, value: Expr },
    InspectTextTable { producer: String, value: Expr },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Statement {
    /// Typed source-bound flow macro. Instance ownership is admitted by Core.
    StandardMacroDispatch { dispatch: Box<crate::scheduled_source::standard_macro_contract::Dispatch> },
    /// Opt-in versioned lowering contract. Historical expressions remain pure.
    ScheduledConsumer {
        version: u32,
        scope: String,
        consumer: String,
        steps: Vec<ConsumerStep>,
        body: Vec<Statement>,
    },
    /// Charge un ou plusieurs fichiers .rvn avant exécution.
    /// Cette instruction est résolue par le loader de projet avant de créer le moteur.
    Use {
        paths: Vec<String>,
    },
    Init {
        body: Vec<Statement>,
    },
    /// Pure computation: assignments are local and narrative operations are forbidden.
    Function {
        name: String,
        parameters: Vec<String>,
        body: Vec<Statement>,
    },
    /// Explicit synchronous, sequenced function with no data result. Legacy
    /// Function and FunctionReturn serialization and semantics stay unchanged.
    UnitFunction { name: String, parameters: Vec<String>, body: Vec<Statement> },
    /// A reached synchronous function invocation, never a narrative label call.
    UnitCall { name: String, arguments: Vec<Expr> },
    /// One nominal Source declaration. Member bodies are the original actual
    /// Function/UnitFunction AST; metadata is admitted separately from its codec.
    FunctionLibrary { scope: crate::function_library_program1023::FunctionLibraryScope },
    /// Synchronous completion call by immutable Library/member/parameter IDs.
    LibraryUnitCall { call: crate::function_library_program1023::QualifiedLibraryCall },
    /// Early completion of an actual UnitFunction activation, not Value::Unit.
    UnitReturn,
    /// A pure, reusable interface description, evaluated against current game variables.
    Screen {
        name: String,
        parameters: Vec<String>,
        body: Vec<Statement>,
    },
    /// Non-blocking event code. Parameters and `local` bindings are not game variables.
    Handler {
        name: String,
        parameters: Vec<String>,
        body: Vec<Statement>,
    },
    LocalVar {
        name: String,
        value: Expr,
    },
    /// Actual typed object effects, distinct from legacy named Screen commands.
    WidgetAddToViewport { object: Expr, z_order: Expr },
    WidgetRemoveFromParent { object: Expr },
    UiOpen {
        name: Expr,
        arguments: Expr,
        modal: Expr,
        layer: Expr,
        /// Explicit narrative ownership from a presenter handler.
        // Keep the legacy AST encoding and saved story identities unchanged.
        #[serde(default, skip_serializing_if = "is_false")]
        story: bool,
    },
    UiClose {
        name: Expr,
    },
    UiFocus {
        name: Expr,
        element: Expr,
    },
    UiSetState {
        name: Expr,
        element: Expr,
        state: Expr,
    },
    MenuExecute { request: Expr },
    MotionPlay {
        target: Expr,
        definition: Expr,
    },
    MotionStop {
        target: Expr,
    },
    MotionWait {
        target: Expr,
    },
    CharacterCompose {
        character: Expr,
        definition: Expr,
    },
    CharacterAttributes {
        character: Expr,
        attributes: Expr,
    },
    VideoPlay {
        name: Expr,
        definition: Expr,
    },
    VideoPause {
        name: Expr,
    },
    VideoResume {
        name: Expr,
    },
    VideoStop {
        name: Expr,
    },
    VideoSkip {
        name: Expr,
    },
    VideoSeek {
        name: Expr,
        seconds: Expr,
    },
    VideoVolume {
        name: Expr,
        volume: Expr,
    },
    VideoWait {
        name: Expr,
    },
    AccessibilityConfigure {
        settings: Expr,
    },
    AccessibilitySpeak {
        text: Expr,
    },
    AccessibilityStop,
    FunctionReturn {
        value: Expr,
    },
    /// Ordered VN branches. An interaction suspends this order until resumed.
    Sequence { branches: Vec<Vec<Statement>> },
    /// Inclusive integer range, evaluated once on entry.
    ForLoopWithBreak { name: String, first: Expr, last: Expr, body: Vec<Statement> },
    /// Lexically exits the nearest ForLoopWithBreak, never a narrative Call.
    LoopBreak,
    While {
        condition: Expr,
        body: Vec<Statement>,
    },
    ForEach {
        name: String,
        collection: Expr,
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
    /// An explicit narrative scene entry. Ordinary labels remain helpers.
    /// This is distinct from `Scene`, which only changes the background.
    SceneLabel {
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
    /// Set a timer that triggers a label after N seconds.
    /// `timer 5.0 => jump escape_failed`
    Timer {
        duration_secs: f32,
        action: String,
    },
    /// Cancel any active timer.
    TimerCancel,
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
    /// Additive expression form; historical literal MusicVolume bytes stay unchanged.
    MusicVolumeValue {
        level: Expr,
    },
    SfxPlay {
        file: String,
        transition: Transition,
    },
    SfxStop {
        file: String,
        transition: Transition,
    },

    /// Play a voice line associated with the current dialogue.
    VoicePlay {
        file: String,
    },
    /// Stop any currently playing voice line.
    VoiceStop,

    Imagemap {
        background: String,
        hover_image: Option<String>,
        hotspots: Vec<Hotspot>,
    },

    TypewriterSet {
        enabled: bool,
    },
    /// Genuine typed expression, distinct from the historical literal AST.
    TypewriterSetValue {
        enabled: Expr,
    },
    TypewriterSpeed {
        chars_per_sec: u32,
    },
    /// Evaluated integer speed; historical literal AST and serde are unchanged.
    TypewriterSpeedValue {
        chars_per_sec: Expr,
    },
}

pub type Script = Vec<Statement>;
