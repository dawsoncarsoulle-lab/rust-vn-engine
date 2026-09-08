use crate::{
    AssetKind, NodeKind, PinCardinality, PinDefinition, PinDirection, PropertyValue, ValueType,
};

fn pin(
    key: &'static str,
    label: &'static str,
    direction: PinDirection,
    value_type: ValueType,
    default_value: Option<PropertyValue>,
) -> PinDefinition {
    PinDefinition {
        key,
        label,
        direction,
        value_type,
        cardinality: if direction == PinDirection::Input {
            PinCardinality::One
        } else {
            PinCardinality::Many
        },
        default_value,
    }
}

fn input(key: &'static str, label: &'static str, value_type: ValueType) -> PinDefinition {
    pin(key, label, PinDirection::Input, value_type, None)
}

fn input_default(
    key: &'static str,
    label: &'static str,
    value_type: ValueType,
    default_value: PropertyValue,
) -> PinDefinition {
    pin(
        key,
        label,
        PinDirection::Input,
        value_type,
        Some(default_value),
    )
}

fn output(key: &'static str, label: &'static str, value_type: ValueType) -> PinDefinition {
    pin(key, label, PinDirection::Output, value_type, None)
}

fn exec_in_out() -> Vec<PinDefinition> {
    vec![
        input("exec_in", "", ValueType::Execution),
        output("exec_out", "", ValueType::Execution),
    ]
}

fn transition() -> PinDefinition {
    input_default(
        "transition",
        "Transition",
        ValueType::Transition,
        PropertyValue::String("none".into()),
    )
}

pub(crate) fn pin_definitions(kind: NodeKind) -> Vec<PinDefinition> {
    use NodeKind as Kind;
    match kind {
        Kind::MakeColor => vec![
            input_default("r", "R", ValueType::Float, PropertyValue::Float(0.0)),
            input_default("g", "G", ValueType::Float, PropertyValue::Float(0.0)),
            input_default("b", "B", ValueType::Float, PropertyValue::Float(0.0)),
            input_default("a", "A", ValueType::Float, PropertyValue::Float(1.0)),
            output("result", "Return Value", ValueType::String),
        ],
        Kind::Use => vec![input_default(
            "paths",
            "Scripts",
            ValueType::List(Box::new(ValueType::Asset(AssetKind::Script))),
            PropertyValue::StringList(Vec::new()),
        )],
        Kind::Init => vec![output("exec_out", "", ValueType::Execution)],
        Kind::Config => vec![
            input("exec_in", "", ValueType::Execution),
            input_default(
                "key",
                "Clé",
                ValueType::String,
                PropertyValue::String(String::new()),
            ),
            input_default(
                "value",
                "Valeur",
                ValueType::String,
                PropertyValue::String(String::new()),
            ),
            output("exec_out", "", ValueType::Execution),
        ],
        Kind::CharacterCreate => vec![
            input("exec_in", "", ValueType::Execution),
            input_default(
                "id",
                "Identifiant",
                ValueType::String,
                PropertyValue::String(String::new()),
            ),
            input_default(
                "display_name",
                "Nom affiché",
                ValueType::String,
                PropertyValue::String(String::new()),
            ),
            output("exec_out", "", ValueType::Execution),
        ],
        Kind::Dialogue => vec![
            input("exec_in", "", ValueType::Execution),
            input_default(
                "character",
                "Personnage",
                ValueType::Character,
                PropertyValue::String(String::new()),
            ),
            input_default(
                "text",
                "Texte",
                ValueType::InterpolatedText,
                PropertyValue::String(String::new()),
            ),
            output("exec_out", "", ValueType::Execution),
        ],
        Kind::Choice => vec![
            input("exec_in", "", ValueType::Execution),
            output("completed", "Terminé", ValueType::Execution),
        ],
        Kind::SetVariable => vec![
            input("exec_in", "", ValueType::Execution),
            input_default(
                "name",
                "Variable",
                ValueType::String,
                PropertyValue::String(String::new()),
            ),
            input("value", "Valeur", ValueType::Any),
            output("exec_out", "", ValueType::Execution),
            output("value_out", "", ValueType::Any),
        ],
        Kind::If => vec![
            input("exec_in", "", ValueType::Execution),
            input_default(
                "condition",
                "Condition",
                ValueType::Bool,
                PropertyValue::Bool(false),
            ),
            output("then", "Vrai", ValueType::Execution),
            output("else", "Faux", ValueType::Execution),
            output("completed", "Terminé", ValueType::Execution),
        ],
        Kind::Label => vec![output("exec_out", "", ValueType::Execution)],
        Kind::Call => vec![
            input("exec_in", "", ValueType::Execution),
            input_default("target", "Label", ValueType::Label, PropertyValue::String(String::new())),
            output("exec_out", "Après retour", ValueType::Execution),
        ],
        Kind::Jump => vec![
            input("exec_in", "", ValueType::Execution),
            input_default(
                "target",
                "Label",
                ValueType::Label,
                PropertyValue::String(String::new()),
            ),
        ],
        Kind::Return => vec![input("exec_in", "", ValueType::Execution)],
        Kind::Scene => vec![
            input("exec_in", "", ValueType::Execution),
            input_default(
                "background",
                "Arrière-plan",
                ValueType::Asset(AssetKind::Background),
                PropertyValue::String(String::new()),
            ),
            transition(),
            output("exec_out", "", ValueType::Execution),
        ],
        Kind::CinematicShow => vec![
            input("exec_in", "", ValueType::Execution),
            input_default(
                "cinematic",
                "Cinématique",
                ValueType::Asset(AssetKind::Cinematic),
                PropertyValue::String(String::new()),
            ),
            transition(),
            output("exec_out", "", ValueType::Execution),
        ],
        Kind::CinematicHide => vec![
            input("exec_in", "", ValueType::Execution),
            transition(),
            output("exec_out", "", ValueType::Execution),
        ],
        Kind::UnlockEnding => vec![
            input("exec_in", "", ValueType::Execution),
            input_default(
                "id",
                "Fin",
                ValueType::String,
                PropertyValue::String(String::new()),
            ),
            output("exec_out", "", ValueType::Execution),
        ],
        Kind::SpriteShow => vec![
            input("exec_in", "", ValueType::Execution),
            input("character", "Personnage", ValueType::Character),
            input_default(
                "emotion",
                "Émotion",
                ValueType::String,
                PropertyValue::String(String::new()),
            ),
            input_default(
                "position",
                "Position",
                ValueType::Position,
                PropertyValue::String("center".into()),
            ),
            transition(),
            output("exec_out", "", ValueType::Execution),
        ],
        Kind::SpriteHide => vec![
            input("exec_in", "", ValueType::Execution),
            input("character", "Personnage", ValueType::Character),
            transition(),
            output("exec_out", "", ValueType::Execution),
        ],
        Kind::SpriteMove => vec![
            input("exec_in", "", ValueType::Execution),
            input("character", "Personnage", ValueType::Character),
            input_default(
                "position",
                "Position",
                ValueType::Position,
                PropertyValue::String("center".into()),
            ),
            transition(),
            output("exec_out", "", ValueType::Execution),
        ],
        Kind::SpriteAnimate => vec![
            input("exec_in", "", ValueType::Execution),
            input("character", "Personnage", ValueType::Character),
            input_default(
                "animation",
                "Animation",
                ValueType::String,
                PropertyValue::String(String::new()),
            ),
            output("exec_out", "", ValueType::Execution),
        ],
        Kind::SpriteStopAnimation => vec![
            input("exec_in", "", ValueType::Execution),
            input("character", "Personnage", ValueType::Character),
            output("exec_out", "", ValueType::Execution),
        ],
        Kind::SpriteEffect => vec![
            input("exec_in", "", ValueType::Execution),
            input("character", "Personnage", ValueType::Character),
            input_default(
                "flip_x",
                "Miroir X",
                ValueType::Bool,
                PropertyValue::Bool(false),
            ),
            input_default(
                "flip_y",
                "Miroir Y",
                ValueType::Bool,
                PropertyValue::Bool(false),
            ),
            input_default(
                "scale",
                "Échelle",
                ValueType::Float,
                PropertyValue::Float(1.0),
            ),
            input_default(
                "rotation",
                "Rotation",
                ValueType::Float,
                PropertyValue::Float(0.0),
            ),
            input_default(
                "tint",
                "Teinte",
                ValueType::String,
                PropertyValue::String("#ffffff".into()),
            ),
            output("exec_out", "", ValueType::Execution),
        ],
        Kind::Timer => vec![
            input("exec_in", "", ValueType::Execution),
            input_default(
                "duration",
                "Secondes",
                ValueType::Float,
                PropertyValue::Float(1.0),
            ),
            input_default(
                "target",
                "Label",
                ValueType::Label,
                PropertyValue::String(String::new()),
            ),
            output("exec_out", "", ValueType::Execution),
        ],
        Kind::MusicStop => vec![input("exec_in", "", ValueType::Execution), transition(), output("exec_out", "", ValueType::Execution)],
        Kind::TimerCancel | Kind::VoiceStop => exec_in_out(),
        Kind::MethodCall => vec![
            input("exec_in", "", ValueType::Execution),
            input_default(
                "target",
                "Cible",
                ValueType::String,
                PropertyValue::String(String::new()),
            ),
            input_default(
                "method",
                "Méthode",
                ValueType::String,
                PropertyValue::String(String::new()),
            ),
            input_default(
                "arg",
                "Argument",
                ValueType::String,
                PropertyValue::String(String::new()),
            ),
            transition(),
            output("exec_out", "", ValueType::Execution),
        ],
        Kind::MusicPlay => vec![
            input("exec_in", "", ValueType::Execution),
            input("file", "Musique", ValueType::Asset(AssetKind::Music)),
            transition(),
            output("exec_out", "", ValueType::Execution),
        ],
        Kind::MusicVolume => vec![
            input("exec_in", "", ValueType::Execution),
            input_default(
                "level",
                "Volume",
                ValueType::Float,
                PropertyValue::Float(1.0),
            ),
            output("exec_out", "", ValueType::Execution),
        ],
        Kind::SfxPlay | Kind::SfxStop => vec![
            input("exec_in", "", ValueType::Execution),
            input(
                "file",
                "Effet sonore",
                ValueType::Asset(AssetKind::SoundEffect),
            ),
            output("exec_out", "", ValueType::Execution),
        ],
        Kind::VoicePlay => vec![
            input("exec_in", "", ValueType::Execution),
            input("file", "Voix", ValueType::Asset(AssetKind::Voice)),
            output("exec_out", "", ValueType::Execution),
        ],
        Kind::Imagemap => vec![
            input("exec_in", "", ValueType::Execution),
            input(
                "background",
                "Arrière-plan",
                ValueType::Asset(AssetKind::Background),
            ),
            input_default(
                "hover",
                "Survol",
                ValueType::Asset(AssetKind::HoverImage),
                PropertyValue::String(String::new()),
            ),
            output("completed", "Terminé", ValueType::Execution),
        ],
        Kind::TypewriterSet => vec![
            input("exec_in", "", ValueType::Execution),
            input_default(
                "enabled",
                "Activé",
                ValueType::Bool,
                PropertyValue::Bool(true),
            ),
            output("exec_out", "", ValueType::Execution),
        ],
        Kind::TypewriterSpeed => vec![
            input("exec_in", "", ValueType::Execution),
            input_default(
                "speed",
                "Caractères/s",
                ValueType::Int,
                PropertyValue::Int(30),
            ),
            output("exec_out", "", ValueType::Execution),
        ],
        Kind::Literal => vec![output("value", "", ValueType::Any)],
        Kind::TextValue => vec![output("value", "", ValueType::InterpolatedText)],
        Kind::FormatText => vec![
            input_default(
                "format",
                "Format",
                ValueType::InterpolatedText,
                PropertyValue::String(String::new()),
            ),
            output("result", "Result", ValueType::InterpolatedText),
        ],
        Kind::Reroute => vec![
            input("value", "", ValueType::Any),
            output("value_out", "", ValueType::Any),
        ],
        Kind::CharacterValue => vec![
            input("sprite", "Sprite", ValueType::Asset(crate::AssetKind::Sprite)),
            output("value", "Personnage", ValueType::Character),
        ],
        Kind::SceneAsset => vec![output("value", "", ValueType::Asset(AssetKind::Background))],
        Kind::SpriteAsset => vec![output("value", "", ValueType::Asset(AssetKind::Sprite))],
        Kind::MusicAsset => vec![output("value", "", ValueType::Asset(AssetKind::Music))],
        Kind::SoundEffectAsset => vec![output(
            "value",
            "",
            ValueType::Asset(AssetKind::SoundEffect),
        )],
        Kind::VoiceAsset => vec![output("value", "", ValueType::Asset(AssetKind::Voice))],
        Kind::CinematicAsset => vec![output("value", "", ValueType::Asset(AssetKind::Cinematic))],
        Kind::HoverImageAsset => vec![output("value", "", ValueType::Asset(AssetKind::HoverImage))],
        Kind::ScriptAsset => vec![output("value", "", ValueType::Asset(AssetKind::Script))],
        Kind::TransitionNone
        | Kind::TransitionFade
        | Kind::TransitionDissolve
        | Kind::TransitionSlideLeft
        | Kind::TransitionSlideRight
        | Kind::TransitionSlideUp
        | Kind::TransitionSlideDown
        | Kind::TransitionZoomIn
        | Kind::TransitionZoomOut
        | Kind::TransitionWipe
        | Kind::TransitionBlur => vec![output("value", "", ValueType::Transition)],
        Kind::VariableGet => vec![output("value", "", ValueType::Any)],
        Kind::ConvertIntToFloat => vec![
            input("value", "", ValueType::Int),
            output("result", "", ValueType::Float),
        ],
        Kind::ConvertNumberToText => vec![
            input("value", "", ValueType::Float),
            output("result", "", ValueType::String),
        ],
        Kind::ConvertTextToInt => vec![
            input("value", "", ValueType::String),
            output("result", "", ValueType::Int),
        ],
        Kind::LabelValue => vec![output("value", "Label", ValueType::Label)],
        Kind::PositionValue => vec![output("value", "Position", ValueType::Position)],
        Kind::VariableReference => vec![output("value", "", ValueType::String)],
        Kind::BinaryOperator => vec![
            input("left", "A", ValueType::Any),
            input("right", "B", ValueType::Any),
            output("value", "Résultat", ValueType::Any),
        ],
        Kind::UnaryOperator => vec![
            input("value", "Valeur", ValueType::Any),
            output("result", "Résultat", ValueType::Any),
        ],
        Kind::MathAdd | Kind::MathSubtract | Kind::MathMultiply | Kind::MathDivide => vec![
            input("left", "A", ValueType::Any),
            input("right", "B", ValueType::Any),
            output("value", "Résultat", ValueType::Any),
        ],
        Kind::MathEqual
        | Kind::MathNotEqual
        | Kind::MathLess
        | Kind::MathLessEqual
        | Kind::MathGreater
        | Kind::MathGreaterEqual => vec![
            input("left", "A", ValueType::Any),
            input("right", "B", ValueType::Any),
            output("value", "Résultat", ValueType::Bool),
        ],
        Kind::LogicAnd | Kind::LogicOr => vec![
            input_default("left", "A", ValueType::Bool, PropertyValue::Bool(false)),
            input_default("right", "B", ValueType::Bool, PropertyValue::Bool(false)),
            output("value", "Résultat", ValueType::Bool),
        ],
        Kind::LogicNot => vec![
            input_default(
                "value",
                "Valeur",
                ValueType::Bool,
                PropertyValue::Bool(false),
            ),
            output("result", "Résultat", ValueType::Bool),
        ],
        Kind::MathNegate => vec![
            input("value", "Valeur", ValueType::Any),
            output("result", "Résultat", ValueType::Any),
        ],
        Kind::FunctionCall => vec![output("result", "Résultat", ValueType::Any)],
        Kind::ListLiteral => vec![output(
            "list",
            "Liste",
            ValueType::List(Box::new(ValueType::Any)),
        )],
        Kind::Index => vec![
            input("target", "Liste", ValueType::List(Box::new(ValueType::Any))),
            input_default("index", "Index", ValueType::Int, PropertyValue::Int(0)),
            output("value", "Valeur", ValueType::Any),
        ],
        Kind::BranchEnd => vec![input("exec_in", "", ValueType::Execution)],
    }
}
