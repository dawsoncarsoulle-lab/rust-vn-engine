use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphKind {
    Init,
    Label { name: String },
    Function { name: String },
    Screen { name: String },
    Handler { name: String },
    Imagemap { name: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeCategory {
    Structure,
    Event,
    Narrative,
    Flow,
    Variable,
    Expression,
    Character,
    Scene,
    Audio,
    Interaction,
    Progression,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    MakeColor,
    Use,
    Init,
    FunctionEntry,
    ScreenEntry,
    HandlerEntry,
    FunctionReturn,
    LocalVariable,
    UiOpen,
    UiClose,
    UiFocus,
    UiSetState,
    UiComponent,
    CanvasRect,
    CanvasEllipse,
    CanvasLine,
    CanvasPolygon,
    CanvasText,
    CanvasImage,
    CanvasGroup,
    CanvasHit,
    MotionPlay,
    MotionStop,
    MotionWait,
    MotionTween,
    MotionSpline,
    MotionBezier,
    MotionCurve,
    MotionPause,
    MotionSequence,
    MotionParallel,
    MotionRepeat,
    MotionFrames,
    CharacterCompose,
    CharacterAttributes,
    LayeredImage,
    ImageLayer,
    ImageLayers,
    VideoPlay,
    VideoPause,
    VideoResume,
    VideoStop,
    VideoSkip,
    VideoSeek,
    VideoVolume,
    VideoWait,
    VideoClip,
    AccessibilityConfigure,
    AccessibilitySpeak,
    AccessibilityStop,
    While,
    ForEach,
    Config,
    CharacterCreate,
    Dialogue,
    Choice,
    SetVariable,
    If,
    Label,
    Jump,
    Call,
    Return,
    Scene,
    CinematicShow,
    CinematicHide,
    UnlockEnding,
    SpriteShow,
    SpriteHide,
    SpriteMove,
    SpriteAnimate,
    SpriteStopAnimation,
    SpriteEffect,
    Timer,
    TimerCancel,
    MethodCall,
    MusicPlay,
    MusicStop,
    MusicVolume,
    SfxPlay,
    SfxStop,
    VoicePlay,
    VoiceStop,
    Imagemap,
    TypewriterSet,
    TypewriterSpeed,
    Literal,
    TextValue,
    FormatText,
    Reroute,
    CharacterValue,
    LabelValue,
    PositionValue,
    SceneAsset,
    SpriteAsset,
    MusicAsset,
    SoundEffectAsset,
    VoiceAsset,
    CinematicAsset,
    HoverImageAsset,
    ScriptAsset,
    TransitionNone,
    TransitionFade,
    TransitionDissolve,
    TransitionSlideLeft,
    TransitionSlideRight,
    TransitionSlideUp,
    TransitionSlideDown,
    TransitionZoomIn,
    TransitionZoomOut,
    TransitionWipe,
    TransitionBlur,
    VariableGet,
    ConvertIntToFloat,
    ConvertNumberToText,
    ConvertTextToInt,
    ConvertStringToText,
    ConvertTextToString,
    VariableReference,
    BinaryOperator,
    UnaryOperator,
    MathAdd,
    StringAppend,
    MathSubtract,
    MathMultiply,
    MathDivide,
    MathEqual,
    MathNotEqual,
    MathLess,
    MathLessEqual,
    MathGreater,
    MathGreaterEqual,
    LogicAnd,
    LogicOr,
    LogicNot,
    MathNegate,
    FunctionCall,
    ListLiteral,
    Index,
    BranchEnd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetKind {
    Background,
    Sprite,
    Music,
    SoundEffect,
    Voice,
    Cinematic,
    HoverImage,
    Script,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValueType {
    Execution,
    Bool,
    Int,
    Float,
    String,
    InterpolatedText,
    List(Box<ValueType>),
    Position,
    Transition,
    Motion,
    Composition,
    ImageLayer,
    VideoClip,
    Asset(AssetKind),
    Character,
    Label,
    /// Integer list/text position or string dictionary key.
    IndexKey,
    Any,
}

impl ValueType {
    pub fn accepts(&self, source: &Self) -> bool {
        if self.is_execution() || source.is_execution() {
            return self == source;
        }
        if let (Self::List(input), Self::List(output)) = (self, source) {
            return input.accepts(output);
        }
        self == source
            || matches!(
                (self, source),
                (
                    Self::IndexKey,
                    Self::Int | Self::String | Self::InterpolatedText
                )
            )
            || matches!(
                (self, source),
                (
                    Self::Asset(AssetKind::Sprite),
                    Self::Asset(AssetKind::Background | AssetKind::HoverImage)
                )
            )
            || matches!(self, Self::Any)
            || matches!(source, Self::Any)
            || matches!((self, source), (Self::Float, Self::Int))
    }

    pub fn is_execution(&self) -> bool {
        matches!(self, Self::Execution)
    }

    pub fn conversion_from(&self, source: &Self) -> Option<NodeKind> {
        match (source, self) {
            (Self::String, Self::InterpolatedText) => Some(NodeKind::ConvertStringToText),
            (Self::InterpolatedText, Self::String) => Some(NodeKind::ConvertTextToString),
            (Self::Int, Self::Float) => Some(NodeKind::ConvertIntToFloat),
            (Self::Int | Self::Float, Self::String | Self::InterpolatedText) => {
                Some(NodeKind::ConvertNumberToText)
            }
            (Self::String | Self::InterpolatedText, Self::Int) => Some(NodeKind::ConvertTextToInt),
            _ => None,
        }
    }

    /// Explicit conversion nodes, ordered from the source to this input.
    /// Text is not a String alias: numeric conversions pass through String.
    pub fn conversion_path_from(&self, source: &Self) -> Option<Vec<NodeKind>> {
        match (source, self) {
            (Self::Int | Self::Float, Self::InterpolatedText) => Some(vec![
                NodeKind::ConvertNumberToText,
                NodeKind::ConvertStringToText,
            ]),
            (Self::InterpolatedText, Self::Int) => Some(vec![
                NodeKind::ConvertTextToString,
                NodeKind::ConvertTextToInt,
            ]),
            _ => self.conversion_from(source).map(|kind| vec![kind]),
        }
    }
}

impl NodeKind {
    /// New Blueprint arithmetic is numeric. The older RVN expression nodes
    /// remain dynamic unless their author explicitly enables this policy.
    pub fn accepts_blueprint_data_source(self, source: &ValueType) -> bool {
        use ValueType as T;
        if matches!(source, T::Any) {
            return true;
        }
        match self {
            Self::BinaryOperator => matches!(source, T::Int | T::Float),
            Self::UnaryOperator => *source == T::Bool,
            Self::MathAdd
            | Self::MathSubtract
            | Self::MathMultiply
            | Self::MathDivide
            | Self::MathNegate
            | Self::MathLess
            | Self::MathLessEqual
            | Self::MathGreater
            | Self::MathGreaterEqual => matches!(source, T::Int | T::Float),
            Self::MathEqual | Self::MathNotEqual => matches!(
                source,
                T::Int | T::Float | T::Bool | T::String | T::InterpolatedText
            ),
            Self::StringAppend => *source == T::String,
            _ => self.accepts_data_source(source),
        }
    }

    pub fn supports_blueprint_operator_policy(self) -> bool {
        matches!(
            self,
            Self::MathAdd
                | Self::MathSubtract
                | Self::MathMultiply
                | Self::MathDivide
                | Self::MathNegate
                | Self::MathEqual
                | Self::MathNotEqual
                | Self::MathLess
                | Self::MathLessEqual
                | Self::MathGreater
                | Self::MathGreaterEqual
                | Self::StringAppend
                | Self::BinaryOperator
                | Self::UnaryOperator
        )
    }

    /// Constraints on wildcard inputs, in addition to pin type compatibility.
    pub fn accepts_data_source(self, source: &ValueType) -> bool {
        use ValueType as T;
        if matches!(source, T::Any) {
            return true;
        }
        match self {
            Self::StringAppend => *source == T::String,
            Self::MathSubtract | Self::MathMultiply | Self::MathDivide | Self::MathNegate => {
                matches!(source, T::Int | T::Float)
            }
            Self::MathLess | Self::MathLessEqual | Self::MathGreater | Self::MathGreaterEqual => {
                matches!(source, T::Int | T::Float | T::String | T::InterpolatedText)
            }
            _ => true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PinDirection {
    Input,
    Output,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PinCardinality {
    One,
    Many,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum PropertyValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    StringList(Vec<String>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VariableDefinition {
    pub name: String,
    pub value_type: ValueType,
    pub default_value: PropertyValue,
}
