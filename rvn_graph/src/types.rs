use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphKind {
    Init,
    Label { name: String },
    Function { name: String },
    UnitFunction { name: String },
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
    WidgetClassValue,
    WidgetObjectClass,
    WidgetAddToViewport,
    WidgetRemoveFromParent,
    WidgetCreate,
    MakeColor,
    Use,
    Init,
    FunctionEntry,
    ScreenEntry,
    HandlerEntry,
    FunctionReturn,
    UnitCall,
    UnitReturn,
    LocalVariable,
    UiOpen,
    UiClose,
    UiFocus,
    UiSetState,
    MenuExecute,
    MenuAction,
    MenuStartScene,
    MenuOpenPage,
    MenuSlot,
    MenuProtect,
    MenuSavePage,
    MenuNumberPreference,
    MenuBoolPreference,
    MenuLanguage,
    MenuAdvance,
    MenuSkipTypewriter,
    MenuChoose,
    MenuGalleryCg,
    MenuGalleryTab,
    MenuConfirm,
    MenuCancel,
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
    DoOnce,
    DoN,
    FlipFlop,
    Gate,
    Sequence,
    ForLoopWithBreak,
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
    TextFromStringTable,
    GetStringTableIdAndKey,
    StringTableRegistered,
    StringTableEntryRegistered,
    StringTableNamespace,
    StringTableSource,
    StringTableMetadata,
    RegisteredStringTables,
    StringTableKeys,
    StringTableMetadataIds,
    StringTableId,
    StringTableAsset,
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
    ConvertBoolToInt,
    ConvertBoolToFloat,
    ConvertIntToBool,
    ConvertIntToFloat,
    ConvertNumberToText,
    ConvertTextToInt,
    ConvertStringToText,
    ConvertTextToString,
    ConvertStringToName,
    ConvertNameToString,
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

/// Persistent behavior boundary, independent of presentation properties.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StringTableQueryContract { LegacyStringV1, NameV2 }

impl StringTableQueryContract {
    pub fn input_type(self, key: &str) -> ValueType {
        if self == Self::NameV2 && matches!(key, "table" | "metadata") { ValueType::Name }
        else { ValueType::String }
    }
    pub fn id_function(self) -> &'static str {
        if self == Self::NameV2 { "__rvn_string_table_id_name" } else { "__rvn_string_table_id" }
    }
}

impl NodeKind {
    pub const fn has_string_table_contract(self) -> bool {
        self.is_string_table_query() || matches!(self, Self::StringTableId)
    }
    pub const fn is_string_table_query(self) -> bool {
        matches!(self, Self::StringTableRegistered | Self::StringTableEntryRegistered
            | Self::StringTableNamespace | Self::StringTableSource | Self::StringTableMetadata
            | Self::RegisteredStringTables | Self::StringTableKeys | Self::StringTableMetadataIds)
    }

    /// Ordinary catalogue nodes use the current typed Name boundary.
    pub fn string_table_query(self) -> Option<(&'static str, &'static [&'static str], ValueType)> {
        self.string_table_query_with_contract(StringTableQueryContract::NameV2)
    }
    pub fn string_table_query_with_contract(self, contract: StringTableQueryContract) -> Option<(&'static str, &'static [&'static str], ValueType)> {
        use ValueType as T;
        if contract == StringTableQueryContract::NameV2 {
            return Some(match self {
                Self::StringTableRegistered => ("__rvn_string_table_registered_name", &["table"], T::Bool),
                Self::StringTableEntryRegistered => ("__rvn_string_table_entry_registered_name", &["table", "key"], T::Bool),
                Self::StringTableNamespace => ("__rvn_string_table_namespace_name", &["table"], T::String),
                Self::StringTableSource => ("__rvn_string_table_source_name", &["table", "key"], T::String),
                Self::StringTableMetadata => ("__rvn_string_table_metadata_name", &["table", "key", "metadata"], T::String),
                Self::RegisteredStringTables => ("__rvn_registered_string_tables_name", &[], T::List(Box::new(T::Name))),
                Self::StringTableKeys => ("__rvn_string_table_keys_name", &["table"], T::List(Box::new(T::String))),
                Self::StringTableMetadataIds => ("__rvn_string_table_metadata_ids_name", &["table", "key"], T::List(Box::new(T::Name))),
                _ => return None,
            });
        }
        Some(match self {
            Self::StringTableRegistered => ("__rvn_string_table_registered", &["table"], T::Bool),
            Self::StringTableEntryRegistered => ("__rvn_string_table_entry_registered", &["table", "key"], T::Bool),
            Self::StringTableNamespace => ("__rvn_string_table_namespace", &["table"], T::String),
            Self::StringTableSource => ("__rvn_string_table_source", &["table", "key"], T::String),
            Self::StringTableMetadata => ("__rvn_string_table_metadata", &["table", "key", "metadata"], T::String),
            Self::RegisteredStringTables => ("__rvn_registered_string_tables", &[], T::List(Box::new(T::String))),
            Self::StringTableKeys => ("__rvn_string_table_keys", &["table"], T::List(Box::new(T::String))),
            Self::StringTableMetadataIds => ("__rvn_string_table_metadata_ids", &["table", "key"], T::List(Box::new(T::String))),
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetKind {
    StringTable,
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
    /// The exact authoring asset identity; never a byte/label surrogate.
    Enum(#[serde(with = "crate::enum_graph1026::asset_id_wire")] rvn_parser::enum_asset::EnumAssetId),
    WidgetClass,
    WidgetObject,
    Execution,
    Bool,
    Int,
    Float,
    String,
    Name,
    InterpolatedText,
    List(Box<ValueType>),
    Position,
    Transition,
    Motion,
    Composition,
    ImageLayer,
    VideoClip,
    Asset(AssetKind),
    MenuRequest,
    Character,
    Label,
    /// Integer list/text position or string dictionary key.
    IndexKey,
    Any,
}

/// One executable conversion step. This is authoring configuration, not a
/// serialized node kind or a new runtime capability. Construct it through the
/// finite ValueType conversion table and materialize its actual catalogue pins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConversionSpec {
    kind: NodeKind,
    invariant_text: bool,
}

impl ConversionSpec {
    fn catalog(kind: NodeKind) -> Self { Self { kind, invariant_text: false } }
    pub fn node_kind(self) -> NodeKind { self.kind }
    pub(crate) fn function(self) -> Option<&'static str> {
        self.invariant_text.then_some("__rvn_invariant_text")
    }
    pub(crate) fn input_key(self) -> &'static str {
        if self.invariant_text { "item_0" } else { "value" }
    }
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

    /// Single-step kind for previews. Use conversion_specs_from and
    /// add_conversion_node when creating the configured executable node.
    pub fn conversion_from(&self, source: &Self) -> Option<NodeKind> {
        match (source, self) {
            (Self::String, Self::InterpolatedText) => Some(NodeKind::ConvertStringToText),
            (Self::InterpolatedText, Self::String) => Some(NodeKind::ConvertTextToString),
            (Self::String, Self::Name) => Some(NodeKind::ConvertStringToName),
            (Self::Name, Self::String) => Some(NodeKind::ConvertNameToString),
            (Self::Name, Self::InterpolatedText) => Some(NodeKind::FunctionCall),
            (Self::Bool, Self::Int) => Some(NodeKind::ConvertBoolToInt),
            (Self::Bool, Self::Float) => Some(NodeKind::ConvertBoolToFloat),
            (Self::Int, Self::Bool) => Some(NodeKind::ConvertIntToBool),
            (Self::Int, Self::Float) => Some(NodeKind::ConvertIntToFloat),
            (Self::Int | Self::Float, Self::String | Self::InterpolatedText) => {
                Some(NodeKind::ConvertNumberToText)
            }
            (Self::String | Self::InterpolatedText, Self::Int) => Some(NodeKind::ConvertTextToInt),
            _ => None,
        }
    }

    /// Node kinds for connection previews. Construction must use the complete
    /// specs: a configured FunctionCall cannot be reconstructed from its kind.
    pub fn conversion_path_from(&self, source: &Self) -> Option<Vec<NodeKind>> {
        self.conversion_specs_from(source).map(|path| path.into_iter().map(ConversionSpec::node_kind).collect())
    }

    /// Explicit steps ordered from source to input. Existing scalar contracts
    /// remain exact; containers never acquire an elementwise conversion here.
    pub fn conversion_specs_from(&self, source: &Self) -> Option<Vec<ConversionSpec>> {
        let kinds = match (source, self) {
            (Self::Name, Self::InterpolatedText) => return Some(vec![ConversionSpec {
                kind: NodeKind::FunctionCall, invariant_text: true,
            }]),
            (Self::Int | Self::Float, Self::InterpolatedText) => vec![
                NodeKind::ConvertNumberToText,
                NodeKind::ConvertStringToText,
            ],
            (Self::InterpolatedText, Self::Int) => vec![
                NodeKind::ConvertTextToString,
                NodeKind::ConvertTextToInt,
            ],
            _ => vec![self.conversion_from(source)?],
        };
        Some(kinds.into_iter().map(ConversionSpec::catalog).collect())
    }
}

impl NodeKind {
    pub fn menu_constructor(self) -> Option<(&'static str, &'static [&'static str])> {
        Some(match self {
            Self::MenuAction => ("menu_action", &["action"]),
            Self::MenuStartScene => ("menu_start_scene", &["label"]),
            Self::MenuOpenPage => ("menu_open_page", &["page"]),
            Self::MenuSlot => ("menu_slot", &["operation", "slot"]),
            Self::MenuProtect => ("menu_protect", &["slot", "protected"]),
            Self::MenuSavePage => ("menu_save_page", &["page"]),
            Self::MenuNumberPreference => ("menu_number", &["key", "value"]),
            Self::MenuBoolPreference => ("menu_bool", &["key", "value"]),
            Self::MenuLanguage => ("menu_language", &["language"]),
            Self::MenuAdvance => ("menu_advance", &[]),
            Self::MenuSkipTypewriter => ("menu_skip_typewriter", &[]),
            Self::MenuChoose => ("menu_choose", &["index"]),
            Self::MenuGalleryCg => ("menu_gallery_cg", &["id"]),
            Self::MenuGalleryTab => ("menu_gallery_tab", &["tab"]),
            Self::MenuConfirm => ("menu_confirm", &["token"]),
            Self::MenuCancel => ("menu_cancel", &["token"]),
            _ => return None,
        })
    }
    pub fn menu_constructor_named(name: &str) -> Option<(Self, &'static [&'static str])> {
        MENU_CONSTRUCTORS.iter().find_map(|kind| kind.menu_constructor().filter(|(found, _)| *found == name).map(|(_, keys)| (*kind, keys)))
    }
    pub const fn menu_title(self) -> &'static str {
        match self {
            Self::MenuExecute => "Exécuter une commande de jeu",
            Self::MenuAction => "Navigation de menu", Self::MenuStartScene => "Nouvelle partie depuis une scène",
            Self::MenuOpenPage => "Ouvrir une page", Self::MenuSlot => "Opération de sauvegarde",
            Self::MenuProtect => "Protection de sauvegarde", Self::MenuSavePage => "Page de sauvegardes",
            Self::MenuNumberPreference => "Préférence numérique", Self::MenuBoolPreference => "Préférence Boolean",
            Self::MenuLanguage => "Langue du jeu", Self::MenuAdvance => "Avancer le dialogue",
            Self::MenuSkipTypewriter => "Terminer le texte", Self::MenuChoose => "Sélectionner une réponse",
            Self::MenuGalleryCg => "Afficher une CG", Self::MenuGalleryTab => "Onglet de galerie",
            Self::MenuConfirm => "Confirmer une opération", Self::MenuCancel => "Annuler une confirmation",
            _ => "Commande de jeu",
        }
    }
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
                T::Int | T::Float | T::Bool | T::String | T::Name | T::InterpolatedText | T::Enum(_)
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
pub const MENU_CONSTRUCTORS: &[NodeKind] = &[NodeKind::MenuAction, NodeKind::MenuStartScene, NodeKind::MenuOpenPage,
    NodeKind::MenuSlot, NodeKind::MenuProtect, NodeKind::MenuSavePage, NodeKind::MenuNumberPreference,
    NodeKind::MenuBoolPreference, NodeKind::MenuLanguage, NodeKind::MenuAdvance, NodeKind::MenuSkipTypewriter,
    NodeKind::MenuChoose, NodeKind::MenuGalleryCg, NodeKind::MenuGalleryTab, NodeKind::MenuConfirm, NodeKind::MenuCancel];

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

/// Validated authoring text for a Name pin, distinct from its runtime spelling.
/// Empty text and bare numeric suffixes remain editable and persist verbatim;
/// the runtime Name still applies its independently validated absence contract.
#[derive(Debug, Clone)]
pub struct NameDefault {
    display: String,
    runtime: rvn_parser::Name,
}

impl NameDefault {
    pub fn new(text: &str) -> Result<Self, rvn_parser::NameError> {
        let runtime = rvn_parser::Name::new(text)?;
        Ok(Self { display: text.into(), runtime })
    }
    pub fn as_str(&self) -> &str { &self.display }
    pub fn runtime(&self) -> &rvn_parser::Name { &self.runtime }
}

impl Default for NameDefault {
    fn default() -> Self { Self::from(rvn_parser::Name::default()) }
}

impl From<rvn_parser::Name> for NameDefault {
    fn from(runtime: rvn_parser::Name) -> Self {
        Self { display: runtime.as_str().into(), runtime }
    }
}

impl PartialEq for NameDefault {
    fn eq(&self, other: &Self) -> bool { self.display == other.display }
}
impl Eq for NameDefault {}

impl Serialize for NameDefault {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.display)
    }
}
impl<'de> Deserialize<'de> for NameDefault {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::new(&text).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum PropertyValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Name(NameDefault),
    Enum(rvn_parser::enum_asset::EnumValue),
    StringList(Vec<String>),
}

impl PartialEq for PropertyValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Bool(a), Self::Bool(b)) => a == b,
            (Self::Int(a), Self::Int(b)) => a == b,
            (Self::Float(a), Self::Float(b)) => a == b,
            (Self::String(a), Self::String(b)) => a == b,
            // Authoring equality retains display edits. Runtime comparison
            // uses the separate case-insensitive Name identity.
            (Self::Name(a), Self::Name(b)) => a.as_str() == b.as_str(),
            (Self::Enum(a), Self::Enum(b)) => a == b,
            (Self::StringList(a), Self::StringList(b)) => a == b,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VariableDefinition {
    pub name: String,
    pub value_type: ValueType,
    pub default_value: PropertyValue,
}
