use crate::pin_catalog::pin_definitions;
use crate::{NodeCategory, NodeKind, PinCardinality, PinDirection, PropertyValue, ValueType};

#[derive(Debug, Clone, PartialEq)]
pub struct NodeDefinition {
    pub kind: NodeKind,
    pub category: NodeCategory,
    pub title: &'static str,
    pub keywords: &'static [&'static str],
    pub has_dynamic_pins: bool,
    pub pins: Vec<PinDefinition>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PinDefinition {
    pub key: &'static str,
    pub label: &'static str,
    pub direction: PinDirection,
    pub value_type: ValueType,
    pub cardinality: PinCardinality,
    pub default_value: Option<PropertyValue>,
}

impl NodeCategory {
    pub const fn header_rgb(self) -> [u8; 3] {
        match self {
            Self::Structure => [0x59, 0x63, 0x6f],
            Self::Event => [0x9e, 0x1b, 0x1b],
            Self::Narrative => [0x71, 0x3f, 0x82],
            Self::Flow => [0xa9, 0x68, 0x1d],
            Self::Variable => [0x27, 0x6b, 0x61],
            Self::Expression => [0x31, 0x5f, 0x38],
            Self::Character => [0x35, 0x6d, 0x42],
            Self::Scene => [0x27, 0x65, 0x7a],
            Self::Audio => [0x59, 0x4b, 0x83],
            Self::Interaction => [0x8a, 0x57, 0x27],
            Self::Progression => [0x8b, 0x70, 0x23],
        }
    }
}

macro_rules! definition {
    ($kind:expr, $category:expr, $title:expr, [$($keyword:expr),* $(,)?]) => {
        NodeDefinition {
            kind: $kind,
            category: $category,
            title: $title,
            keywords: &[$($keyword),*],
            has_dynamic_pins: false,
            pins: Vec::new(),
        }
    };
    ($kind:expr, $category:expr, $title:expr, [$($keyword:expr),* $(,)?], dynamic) => {
        NodeDefinition {
            kind: $kind,
            category: $category,
            title: $title,
            keywords: &[$($keyword),*],
            has_dynamic_pins: true,
            pins: Vec::new(),
        }
    };
}

pub fn node_definition(kind: NodeKind) -> NodeDefinition {
    use NodeCategory as Category;
    use NodeKind as Kind;
    let mut definition = match kind {
        Kind::Use => definition!(
            kind,
            Category::Structure,
            "Importer des scripts",
            ["use", "import"]
        ),
        Kind::Init => definition!(
            kind,
            Category::Structure,
            "Initialisation",
            ["init", "global"]
        ),
        Kind::Config => definition!(
            kind,
            Category::Structure,
            "Configuration",
            ["config", "project"]
        ),
        Kind::CharacterCreate => definition!(
            kind,
            Category::Character,
            "Créer un personnage",
            ["character", "create"]
        ),
        Kind::Dialogue => definition!(
            kind,
            Category::Narrative,
            "Dialogue",
            ["dialogue", "narrateur", "text"]
        ),
        Kind::FormatText => definition!(
            kind,
            Category::Expression,
            "Format Text",
            ["format", "texte", "interpolation", "placeholder"],
            dynamic
        ),
        Kind::Reroute => definition!(
            kind,
            Category::Expression,
            "Nœud de reroute",
            ["reroute", "knot", "cable", "fil", "déviation"]
        ),
        Kind::Choice => definition!(
            kind,
            Category::Narrative,
            "Choix",
            ["choice", "option"],
            dynamic
        ),
        Kind::SetVariable => definition!(
            kind,
            Category::Variable,
            "Définir une variable",
            ["set", "persistent"]
        ),
        Kind::If => definition!(kind, Category::Flow, "Condition", ["if", "else", "branch"]),
        Kind::Label => definition!(kind, Category::Event, "Entrée de label", ["label", "event"]),
        Kind::Jump => definition!(kind, Category::Flow, "Aller au label", ["jump", "goto"]),
        Kind::Call => definition!(
            kind,
            Category::Flow,
            "Appeler un label",
            ["call", "subroutine"]
        ),
        Kind::Return => definition!(kind, Category::Flow, "Retour", ["return"]),
        Kind::Scene => definition!(
            kind,
            Category::Scene,
            "Changer de scène",
            ["scene", "background"]
        ),
        Kind::CinematicShow => definition!(
            kind,
            Category::Scene,
            "Afficher une cinématique",
            ["cinematic", "cg", "show"]
        ),
        Kind::CinematicHide => definition!(
            kind,
            Category::Scene,
            "Masquer la cinématique",
            ["cinematic", "cg", "hide"]
        ),
        Kind::UnlockEnding => definition!(
            kind,
            Category::Progression,
            "Débloquer une fin",
            ["unlock", "ending"]
        ),
        Kind::SpriteShow => definition!(
            kind,
            Category::Character,
            "Afficher un sprite",
            ["sprite", "show", "emotion"]
        ),
        Kind::SpriteHide => definition!(
            kind,
            Category::Character,
            "Retirer le sprite",
            [
                "sprite",
                "hide",
                "masquer",
                "retirer",
                "destroy actor",
                "destroy",
                "détruire"
            ]
        ),
        Kind::SpriteMove => definition!(
            kind,
            Category::Character,
            "Déplacer un sprite",
            ["sprite", "move", "position"]
        ),
        Kind::SpriteAnimate => definition!(
            kind,
            Category::Character,
            "Animer un sprite",
            ["sprite", "animate"],
            dynamic
        ),
        Kind::SpriteStopAnimation => definition!(
            kind,
            Category::Character,
            "Arrêter l’animation",
            ["sprite", "stop", "animation"]
        ),
        Kind::SpriteEffect => definition!(
            kind,
            Category::Character,
            "Effet de sprite",
            ["flip", "scale", "rotation", "tint"],
            dynamic
        ),
        Kind::Timer => definition!(
            kind,
            Category::Flow,
            "Démarrer un timer",
            ["timer", "delay"]
        ),
        Kind::TimerCancel => definition!(
            kind,
            Category::Flow,
            "Annuler le timer",
            ["timer", "cancel"]
        ),
        Kind::MethodCall => definition!(
            kind,
            Category::Structure,
            "Appeler une méthode",
            ["method", "call"],
            dynamic
        ),
        Kind::MusicPlay => definition!(
            kind,
            Category::Audio,
            "Jouer une musique",
            ["music", "play"]
        ),
        Kind::MusicStop => definition!(
            kind,
            Category::Audio,
            "Arrêter la musique",
            ["music", "stop"]
        ),
        Kind::MusicVolume => definition!(
            kind,
            Category::Audio,
            "Volume de la musique",
            ["music", "volume"]
        ),
        Kind::SfxPlay => definition!(
            kind,
            Category::Audio,
            "Jouer un effet sonore",
            ["sfx", "sound", "play"]
        ),
        Kind::SfxStop => definition!(
            kind,
            Category::Audio,
            "Arrêter un effet sonore",
            ["sfx", "sound", "stop"]
        ),
        Kind::VoicePlay => definition!(kind, Category::Audio, "Jouer une voix", ["voice", "play"]),
        Kind::VoiceStop => definition!(kind, Category::Audio, "Arrêter la voix", ["voice", "stop"]),
        Kind::Imagemap => definition!(
            kind,
            Category::Interaction,
            "Carte interactive",
            ["imagemap", "hotspot"],
            dynamic
        ),
        Kind::TypewriterSet => definition!(
            kind,
            Category::Interaction,
            "Activer le typewriter",
            ["typewriter", "enable"]
        ),
        Kind::TypewriterSpeed => definition!(
            kind,
            Category::Interaction,
            "Vitesse du typewriter",
            ["typewriter", "speed"]
        ),
        Kind::MakeColor => definition!(
            kind,
            Category::Expression,
            "Make Color",
            ["couleur", "color", "rgb", "rgba", "teinte"]
        ),
        Kind::Literal => definition!(
            kind,
            Category::Expression,
            "Valeur littérale",
            ["literal", "value"],
            dynamic
        ),
        Kind::TextValue => definition!(
            kind,
            Category::Narrative,
            "Texte",
            ["text", "dialogue", "string"]
        ),
        Kind::LabelValue => definition!(
            kind,
            Category::Flow,
            "Référence de label",
            ["label", "destination", "reference", "saut", "chapitre"]
        ),
        Kind::PositionValue => definition!(
            kind,
            Category::Scene,
            "Position",
            ["position", "left", "center", "right"]
        ),
        Kind::CharacterValue => definition!(
            kind,
            Category::Character,
            "Personnage",
            ["character", "emotion", "personnage"]
        ),
        Kind::SceneAsset => definition!(
            kind,
            Category::Scene,
            "Sélectionner une scène",
            ["scene", "background", "image", "asset"]
        ),
        Kind::SpriteAsset => definition!(
            kind,
            Category::Scene,
            "Image de personnage",
            ["sprite", "image", "asset"]
        ),
        Kind::MusicAsset => definition!(
            kind,
            Category::Audio,
            "Musique",
            ["music", "audio", "asset"]
        ),
        Kind::SoundEffectAsset => definition!(
            kind,
            Category::Audio,
            "Effet sonore",
            ["sound", "sfx", "audio", "asset"]
        ),
        Kind::VoiceAsset => definition!(kind, Category::Audio, "Voix", ["voice", "audio", "asset"]),
        Kind::CinematicAsset => definition!(
            kind,
            Category::Scene,
            "Cinématique",
            ["video", "cinematic", "asset"]
        ),
        Kind::HoverImageAsset => definition!(
            kind,
            Category::Scene,
            "Image de survol",
            ["hover", "image", "asset"]
        ),
        Kind::ScriptAsset => definition!(kind, Category::Structure, "Script", ["script", "asset"]),
        Kind::TransitionNone => {
            definition!(kind, Category::Scene, "AUCUNE", ["transition", "none"])
        }
        Kind::TransitionFade => definition!(kind, Category::Scene, "FONDU", ["transition", "fade"]),
        Kind::TransitionDissolve => definition!(
            kind,
            Category::Scene,
            "DISSOLVE",
            ["transition", "dissolve"]
        ),
        Kind::TransitionSlideLeft => definition!(
            kind,
            Category::Scene,
            "GLISSER À GAUCHE",
            ["transition", "slide", "left"]
        ),
        Kind::TransitionSlideRight => definition!(
            kind,
            Category::Scene,
            "GLISSER À DROITE",
            ["transition", "slide", "right"]
        ),
        Kind::TransitionSlideUp => definition!(
            kind,
            Category::Scene,
            "GLISSER EN HAUT",
            ["transition", "slide", "up"]
        ),
        Kind::TransitionSlideDown => definition!(
            kind,
            Category::Scene,
            "GLISSER EN BAS",
            ["transition", "slide", "down"]
        ),
        Kind::TransitionZoomIn => definition!(
            kind,
            Category::Scene,
            "ZOOM AVANT",
            ["transition", "zoom", "in"]
        ),
        Kind::TransitionZoomOut => definition!(
            kind,
            Category::Scene,
            "ZOOM ARRIÈRE",
            ["transition", "zoom", "out"]
        ),
        Kind::TransitionWipe => {
            definition!(kind, Category::Scene, "BALAYAGE", ["transition", "wipe"])
        }
        Kind::TransitionBlur => definition!(kind, Category::Scene, "FLOU", ["transition", "blur"]),
        Kind::VariableGet => definition!(
            kind,
            Category::Variable,
            "Lire une variable",
            ["get", "variable", "persistent"]
        ),
        Kind::ConvertIntToFloat => definition!(
            kind,
            Category::Expression,
            "Convertir Entier en Décimal",
            [
                "convertir",
                "conversion",
                "entier",
                "décimal",
                "int",
                "float"
            ]
        ),
        Kind::ConvertNumberToText => definition!(
            kind,
            Category::Expression,
            "Convertir Nombre en Texte",
            ["convertir", "conversion", "nombre", "texte", "string"]
        ),
        Kind::ConvertTextToInt => definition!(
            kind,
            Category::Expression,
            "Convertir Texte en Entier",
            [
                "convertir",
                "conversion",
                "texte",
                "entier",
                "string",
                "int"
            ]
        ),
        Kind::VariableReference => definition!(
            kind,
            Category::Variable,
            "Variable",
            ["variable", "reference", "name"]
        ),
        Kind::BinaryOperator => definition!(
            kind,
            Category::Expression,
            "Opérateur binaire",
            ["add", "compare", "and", "or"],
            dynamic
        ),
        Kind::UnaryOperator => definition!(
            kind,
            Category::Expression,
            "Opérateur unaire",
            ["not", "negate"],
            dynamic
        ),
        Kind::MathAdd => definition!(kind, Category::Expression, "+", ["addition", "add", "+"]),
        Kind::MathSubtract => definition!(
            kind,
            Category::Expression,
            "−",
            ["soustraction", "subtract", "-"]
        ),
        Kind::MathMultiply => definition!(
            kind,
            Category::Expression,
            "×",
            ["multiplication", "multiply", "*"]
        ),
        Kind::MathDivide => {
            definition!(kind, Category::Expression, "÷", ["division", "divide", "/"])
        }
        Kind::MathEqual => definition!(kind, Category::Expression, "==", ["égal", "equal", "=="]),
        Kind::MathNotEqual => definition!(
            kind,
            Category::Expression,
            "!=",
            ["différent", "not equal", "!="]
        ),
        Kind::MathLess => definition!(kind, Category::Expression, "<", ["inférieur", "less", "<"]),
        Kind::MathLessEqual => definition!(
            kind,
            Category::Expression,
            "<=",
            ["inférieur égal", "less equal", "<="]
        ),
        Kind::MathGreater => definition!(
            kind,
            Category::Expression,
            ">",
            ["supérieur", "greater", ">"]
        ),
        Kind::MathGreaterEqual => definition!(
            kind,
            Category::Expression,
            ">=",
            ["supérieur égal", "greater equal", ">="]
        ),
        Kind::LogicAnd => definition!(kind, Category::Expression, "AND", ["et", "and", "bool"]),
        Kind::LogicOr => definition!(kind, Category::Expression, "OR", ["ou", "or", "bool"]),
        Kind::LogicNot => definition!(kind, Category::Expression, "NOT", ["non", "not", "bool"]),
        Kind::MathNegate => definition!(
            kind,
            Category::Expression,
            "−",
            ["négatif", "negate", "negative"]
        ),
        Kind::FunctionCall => definition!(
            kind,
            Category::Expression,
            "Fonction",
            ["min", "max", "abs", "random"],
            dynamic
        ),
        Kind::ListLiteral => definition!(
            kind,
            Category::Expression,
            "Liste",
            ["list", "array"],
            dynamic
        ),
        Kind::Index => definition!(
            kind,
            Category::Expression,
            "Accès par index",
            ["index", "list"]
        ),
        Kind::BranchEnd => definition!(
            kind,
            Category::Structure,
            "Fin de branche",
            ["branch", "end"]
        ),
    };
    definition.pins = pin_definitions(kind);
    definition
}

pub const ALL_NODE_KINDS: &[NodeKind] = &[
    NodeKind::Use,
    NodeKind::Init,
    NodeKind::Config,
    NodeKind::CharacterCreate,
    NodeKind::Dialogue,
    NodeKind::Choice,
    NodeKind::SetVariable,
    NodeKind::If,
    NodeKind::Label,
    NodeKind::Jump,
    NodeKind::Call,
    NodeKind::Return,
    NodeKind::Scene,
    NodeKind::CinematicShow,
    NodeKind::CinematicHide,
    NodeKind::UnlockEnding,
    NodeKind::SpriteShow,
    NodeKind::SpriteHide,
    NodeKind::SpriteMove,
    NodeKind::SpriteAnimate,
    NodeKind::SpriteStopAnimation,
    NodeKind::SpriteEffect,
    NodeKind::Timer,
    NodeKind::TimerCancel,
    NodeKind::MethodCall,
    NodeKind::MusicPlay,
    NodeKind::MusicStop,
    NodeKind::MusicVolume,
    NodeKind::SfxPlay,
    NodeKind::SfxStop,
    NodeKind::VoicePlay,
    NodeKind::VoiceStop,
    NodeKind::Imagemap,
    NodeKind::TypewriterSet,
    NodeKind::TypewriterSpeed,
    NodeKind::MakeColor,
    NodeKind::Literal,
    NodeKind::TextValue,
    NodeKind::FormatText,
    NodeKind::Reroute,
    NodeKind::CharacterValue,
    NodeKind::LabelValue,
    NodeKind::PositionValue,
    NodeKind::SceneAsset,
    NodeKind::SpriteAsset,
    NodeKind::MusicAsset,
    NodeKind::SoundEffectAsset,
    NodeKind::VoiceAsset,
    NodeKind::CinematicAsset,
    NodeKind::HoverImageAsset,
    NodeKind::ScriptAsset,
    NodeKind::TransitionNone,
    NodeKind::TransitionFade,
    NodeKind::TransitionDissolve,
    NodeKind::TransitionSlideLeft,
    NodeKind::TransitionSlideRight,
    NodeKind::TransitionSlideUp,
    NodeKind::TransitionSlideDown,
    NodeKind::TransitionZoomIn,
    NodeKind::TransitionZoomOut,
    NodeKind::TransitionWipe,
    NodeKind::TransitionBlur,
    NodeKind::VariableGet,
    NodeKind::ConvertIntToFloat,
    NodeKind::ConvertNumberToText,
    NodeKind::ConvertTextToInt,
    NodeKind::VariableReference,
    NodeKind::BinaryOperator,
    NodeKind::UnaryOperator,
    NodeKind::MathAdd,
    NodeKind::MathSubtract,
    NodeKind::MathMultiply,
    NodeKind::MathDivide,
    NodeKind::MathEqual,
    NodeKind::MathNotEqual,
    NodeKind::MathLess,
    NodeKind::MathLessEqual,
    NodeKind::MathGreater,
    NodeKind::MathGreaterEqual,
    NodeKind::LogicAnd,
    NodeKind::LogicOr,
    NodeKind::LogicNot,
    NodeKind::MathNegate,
    NodeKind::FunctionCall,
    NodeKind::ListLiteral,
    NodeKind::Index,
    NodeKind::BranchEnd,
];
