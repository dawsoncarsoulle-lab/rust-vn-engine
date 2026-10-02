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
        Kind::FunctionEntry | Kind::ScreenEntry | Kind::HandlerEntry => {
            vec![output("exec_out", "", ValueType::Execution)]
        }
        Kind::LocalVariable => vec![
            input("exec_in", "", ValueType::Execution),
            input_default(
                "name",
                "Variable locale",
                ValueType::String,
                PropertyValue::String("value".into()),
            ),
            input("value", "Valeur", ValueType::Any),
            output("exec_out", "", ValueType::Execution),
            output("value_out", "", ValueType::Any),
        ],
        Kind::UiOpen => vec![
            input("exec_in", "", ValueType::Execution),
            input_default(
                "name",
                "Écran",
                ValueType::String,
                PropertyValue::String(String::new()),
            ),
            input(
                "arguments",
                "Arguments",
                ValueType::List(Box::new(ValueType::Any)),
            ),
            input_default(
                "modal",
                "Modal",
                ValueType::Bool,
                PropertyValue::Bool(false),
            ),
            input_default("layer", "Couche", ValueType::Int, PropertyValue::Int(0)),
            output("exec_out", "", ValueType::Execution),
        ],
        Kind::UiClose => vec![
            input("exec_in", "", ValueType::Execution),
            input_default(
                "name",
                "Écran",
                ValueType::String,
                PropertyValue::String(String::new()),
            ),
            output("exec_out", "", ValueType::Execution),
        ],
        Kind::UiFocus => vec![
            input("exec_in", "", ValueType::Execution),
            input_default(
                "name",
                "Écran",
                ValueType::String,
                PropertyValue::String(String::new()),
            ),
            input_default(
                "element",
                "Contrôle",
                ValueType::String,
                PropertyValue::String(String::new()),
            ),
            output("exec_out", "", ValueType::Execution),
        ],
        Kind::UiSetState => vec![
            input("exec_in", "", ValueType::Execution),
            input_default(
                "name",
                "Écran",
                ValueType::String,
                PropertyValue::String(String::new()),
            ),
            input_default(
                "element",
                "Composant",
                ValueType::String,
                PropertyValue::String(String::new()),
            ),
            input("state", "Nouvel état", ValueType::Any),
            output("exec_out", "", ValueType::Execution),
        ],
        Kind::UiComponent => vec![
            input_default(
                "id",
                "Identifiant",
                ValueType::String,
                PropertyValue::String("component".into()),
            ),
            input_default(
                "kind",
                "Type de composant",
                ValueType::String,
                PropertyValue::String("text".into()),
            ),
            input("properties", "Propriétés", ValueType::Any),
            input(
                "children",
                "Enfants",
                ValueType::List(Box::new(ValueType::Any)),
            ),
            output("value", "Composant", ValueType::Any),
        ],
        Kind::CanvasRect => vec![
            input(
                "rect",
                "Rectangle",
                ValueType::List(Box::new(ValueType::Float)),
            ),
            input(
                "color",
                "Couleur",
                ValueType::List(Box::new(ValueType::Float)),
            ),
            input_default(
                "radius",
                "Rayon",
                ValueType::Float,
                PropertyValue::Float(0.0),
            ),
            output("value", "Dessin", ValueType::Any),
        ],
        Kind::CanvasEllipse => vec![
            input(
                "rect",
                "Rectangle",
                ValueType::List(Box::new(ValueType::Float)),
            ),
            input(
                "color",
                "Couleur",
                ValueType::List(Box::new(ValueType::Float)),
            ),
            output("value", "Dessin", ValueType::Any),
        ],
        Kind::CanvasLine => vec![
            input(
                "points",
                "Points",
                ValueType::List(Box::new(ValueType::Any)),
            ),
            input(
                "color",
                "Couleur",
                ValueType::List(Box::new(ValueType::Float)),
            ),
            input_default(
                "width",
                "Épaisseur",
                ValueType::Float,
                PropertyValue::Float(1.0),
            ),
            output("value", "Dessin", ValueType::Any),
        ],
        Kind::CanvasPolygon => vec![
            input(
                "points",
                "Points",
                ValueType::List(Box::new(ValueType::Any)),
            ),
            input(
                "color",
                "Couleur",
                ValueType::List(Box::new(ValueType::Float)),
            ),
            output("value", "Dessin", ValueType::Any),
        ],
        Kind::CanvasText => vec![
            input_default(
                "text",
                "Texte",
                ValueType::InterpolatedText,
                PropertyValue::String("Texte".into()),
            ),
            input(
                "position",
                "Position",
                ValueType::List(Box::new(ValueType::Float)),
            ),
            input(
                "color",
                "Couleur",
                ValueType::List(Box::new(ValueType::Float)),
            ),
            input_default(
                "size",
                "Taille de police",
                ValueType::Float,
                PropertyValue::Float(24.0),
            ),
            output("value", "Dessin", ValueType::Any),
        ],
        Kind::CanvasImage => vec![
            input_default(
                "image",
                "Image",
                ValueType::String,
                PropertyValue::String(String::new()),
            ),
            input(
                "rect",
                "Rectangle",
                ValueType::List(Box::new(ValueType::Float)),
            ),
            output("value", "Dessin", ValueType::Any),
        ],
        Kind::CanvasGroup => vec![
            input(
                "transform",
                "Transformation",
                ValueType::List(Box::new(ValueType::Float)),
            ),
            input(
                "clip",
                "Découpage",
                ValueType::List(Box::new(ValueType::Float)),
            ),
            input(
                "children",
                "Dessins",
                ValueType::List(Box::new(ValueType::Any)),
            ),
            output("value", "Dessin", ValueType::Any),
        ],
        Kind::CanvasHit => vec![
            input_default(
                "id",
                "Zone",
                ValueType::String,
                PropertyValue::String("zone".into()),
            ),
            input(
                "rect",
                "Rectangle",
                ValueType::List(Box::new(ValueType::Float)),
            ),
            output("value", "Dessin", ValueType::Any),
        ],
        Kind::MotionPlay => vec![
            input("exec_in", "", ValueType::Execution),
            input_default(
                "target",
                "Cible",
                ValueType::String,
                PropertyValue::String("background".into()),
            ),
            input("definition", "Animation", ValueType::Motion),
            output("exec_out", "", ValueType::Execution),
        ],
        Kind::MotionStop | Kind::MotionWait => vec![
            input("exec_in", "", ValueType::Execution),
            input_default(
                "target",
                "Cible",
                ValueType::String,
                PropertyValue::String("background".into()),
            ),
            output("exec_out", "", ValueType::Execution),
        ],
        Kind::MotionTween => vec![
            input_default(
                "seconds",
                "Durée (s)",
                ValueType::Float,
                PropertyValue::Float(1.0),
            ),
            input("from", "Départ", ValueType::Any),
            input("to", "Arrivée", ValueType::Any),
            input_default(
                "curve",
                "Courbe",
                ValueType::Any,
                PropertyValue::String("ease_in_out".into()),
            ),
            output("value", "Animation", ValueType::Motion),
        ],
        Kind::MotionSpline => vec![
            input_default(
                "seconds",
                "Durée (s)",
                ValueType::Float,
                PropertyValue::Float(1.0),
            ),
            input(
                "points",
                "Points",
                ValueType::List(Box::new(ValueType::Any)),
            ),
            input_default(
                "curve",
                "Courbe",
                ValueType::Any,
                PropertyValue::String("linear".into()),
            ),
            output("value", "Animation", ValueType::Motion),
        ],
        Kind::MotionBezier => vec![
            input_default(
                "x1",
                "Contrôle X1",
                ValueType::Float,
                PropertyValue::Float(0.25),
            ),
            input_default(
                "y1",
                "Contrôle Y1",
                ValueType::Float,
                PropertyValue::Float(0.1),
            ),
            input_default(
                "x2",
                "Contrôle X2",
                ValueType::Float,
                PropertyValue::Float(0.75),
            ),
            input_default(
                "y2",
                "Contrôle Y2",
                ValueType::Float,
                PropertyValue::Float(0.9),
            ),
            output("value", "Courbe", ValueType::Any),
        ],
        Kind::MotionCurve => vec![
            input_default(
                "function",
                "Fonction RVN",
                ValueType::String,
                PropertyValue::String(String::new()),
            ),
            input_default(
                "samples",
                "Échantillons",
                ValueType::Int,
                PropertyValue::Int(65),
            ),
            output("value", "Courbe", ValueType::Any),
        ],
        Kind::MotionPause => vec![
            input_default(
                "seconds",
                "Durée (s)",
                ValueType::Float,
                PropertyValue::Float(1.0),
            ),
            output("value", "Animation", ValueType::Motion),
        ],
        Kind::MotionSequence | Kind::MotionParallel => vec![
            input(
                "steps",
                "Animations",
                ValueType::List(Box::new(ValueType::Motion)),
            ),
            output("value", "Animation", ValueType::Motion),
        ],
        Kind::MotionRepeat => vec![
            input_default(
                "times",
                "Répétitions (0 = infini)",
                ValueType::Int,
                PropertyValue::Int(1),
            ),
            input("motion", "Animation", ValueType::Motion),
            output("value", "Animation", ValueType::Motion),
        ],
        Kind::MotionFrames => vec![
            input(
                "images",
                "Images",
                ValueType::List(Box::new(ValueType::String)),
            ),
            input_default(
                "fps",
                "Images/s",
                ValueType::Float,
                PropertyValue::Float(12.0),
            ),
            output("value", "Animation", ValueType::Motion),
        ],
        Kind::CharacterCompose => vec![
            input("exec_in", "", ValueType::Execution),
            input_default(
                "character",
                "Personnage",
                ValueType::String,
                PropertyValue::String(String::new()),
            ),
            input("definition", "Composition", ValueType::Composition),
            output("exec_out", "", ValueType::Execution),
        ],
        Kind::CharacterAttributes => vec![
            input("exec_in", "", ValueType::Execution),
            input_default(
                "character",
                "Personnage",
                ValueType::String,
                PropertyValue::String(String::new()),
            ),
            input("attributes", "Attributs", ValueType::Any),
            output("exec_out", "", ValueType::Execution),
        ],
        Kind::LayeredImage => vec![
            input(
                "size",
                "Dimensions",
                ValueType::List(Box::new(ValueType::Float)),
            ),
            input("defaults", "Attributs par défaut", ValueType::Any),
            input(
                "layers",
                "Calques",
                ValueType::List(Box::new(ValueType::ImageLayer)),
            ),
            input("options", "Variantes et sélection", ValueType::Any),
            output("value", "Composition", ValueType::Composition),
        ],
        Kind::ImageLayer => vec![
            input_default(
                "id",
                "Identifiant",
                ValueType::String,
                PropertyValue::String("layer".into()),
            ),
            input_default(
                "image",
                "Image",
                ValueType::String,
                PropertyValue::String(String::new()),
            ),
            input("properties", "Propriétés", ValueType::Any),
            output("value", "Calque", ValueType::ImageLayer),
        ],
        Kind::ImageLayers => vec![
            input_default(
                "prefix",
                "Préfixe des images",
                ValueType::String,
                PropertyValue::String("iris".into()),
            ),
            input(
                "images",
                "Images du projet",
                ValueType::List(Box::new(ValueType::String)),
            ),
            output(
                "value",
                "Calques découverts",
                ValueType::List(Box::new(ValueType::ImageLayer)),
            ),
        ],
        Kind::VideoPlay => vec![
            input("exec_in", "", ValueType::Execution),
            input_default(
                "name",
                "Lecteur",
                ValueType::String,
                PropertyValue::String("intro".into()),
            ),
            input("definition", "Vidéo", ValueType::VideoClip),
            output("exec_out", "", ValueType::Execution),
        ],
        Kind::VideoPause
        | Kind::VideoResume
        | Kind::VideoStop
        | Kind::VideoSkip
        | Kind::VideoWait => vec![
            input("exec_in", "", ValueType::Execution),
            input_default(
                "name",
                "Lecteur",
                ValueType::String,
                PropertyValue::String("intro".into()),
            ),
            output("exec_out", "", ValueType::Execution),
        ],
        Kind::VideoSeek => vec![
            input("exec_in", "", ValueType::Execution),
            input_default(
                "name",
                "Lecteur",
                ValueType::String,
                PropertyValue::String("intro".into()),
            ),
            input_default(
                "seconds",
                "Position (s)",
                ValueType::Float,
                PropertyValue::Float(0.0),
            ),
            output("exec_out", "", ValueType::Execution),
        ],
        Kind::VideoVolume => vec![
            input("exec_in", "", ValueType::Execution),
            input_default(
                "name",
                "Lecteur",
                ValueType::String,
                PropertyValue::String("intro".into()),
            ),
            input_default(
                "volume",
                "Volume",
                ValueType::Float,
                PropertyValue::Float(1.0),
            ),
            output("exec_out", "", ValueType::Execution),
        ],
        Kind::VideoClip => vec![
            input_default(
                "source",
                "Fichier WebM",
                ValueType::String,
                PropertyValue::String(String::new()),
            ),
            input("properties", "Propriétés", ValueType::Any),
            output("value", "Vidéo", ValueType::VideoClip),
        ],
        Kind::AccessibilityConfigure => vec![
            input("exec_in", "", ValueType::Execution),
            input("settings", "Réglages", ValueType::Any),
            output("exec_out", "", ValueType::Execution),
        ],
        Kind::AccessibilitySpeak => vec![
            input("exec_in", "", ValueType::Execution),
            input_default(
                "text",
                "Texte",
                ValueType::String,
                PropertyValue::String(String::new()),
            ),
            output("exec_out", "", ValueType::Execution),
        ],
        Kind::AccessibilityStop => vec![
            input("exec_in", "", ValueType::Execution),
            output("exec_out", "", ValueType::Execution),
        ],
        Kind::FunctionReturn => vec![
            input("exec_in", "", ValueType::Execution),
            input("value", "Valeur retournée", ValueType::Any),
        ],
        Kind::While => vec![
            input("exec_in", "", ValueType::Execution),
            input_default(
                "condition",
                "Condition",
                ValueType::Bool,
                PropertyValue::Bool(false),
            ),
            output("body", "Corps de boucle", ValueType::Execution),
            output("completed", "Terminé", ValueType::Execution),
        ],
        Kind::ForEach => vec![
            input("exec_in", "", ValueType::Execution),
            input(
                "collection",
                "Liste",
                ValueType::List(Box::new(ValueType::Any)),
            ),
            output("body", "Corps de boucle", ValueType::Execution),
            output("completed", "Terminé", ValueType::Execution),
        ],
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
            input_default(
                "target",
                "Label",
                ValueType::Label,
                PropertyValue::String(String::new()),
            ),
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
        Kind::MusicStop => vec![
            input("exec_in", "", ValueType::Execution),
            transition(),
            output("exec_out", "", ValueType::Execution),
        ],
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
            input(
                "sprite",
                "Sprite",
                ValueType::Asset(crate::AssetKind::Sprite),
            ),
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
        Kind::ConvertStringToText => vec![
            input("value", "Chaîne en entrée", ValueType::String),
            output("result", "Valeur de retour", ValueType::InterpolatedText),
        ],
        Kind::ConvertTextToString => vec![
            input("value", "Texte en entrée", ValueType::InterpolatedText),
            output("result", "Valeur de retour", ValueType::String),
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
        Kind::StringAppend => vec![
            input_default(
                "left",
                "A",
                ValueType::String,
                PropertyValue::String(String::new()),
            ),
            input_default(
                "right",
                "B",
                ValueType::String,
                PropertyValue::String(String::new()),
            ),
            output("value", "Return Value", ValueType::String),
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
            input("target", "Liste / texte", ValueType::Any),
            input_default(
                "index",
                "Index / clé",
                ValueType::IndexKey,
                PropertyValue::Int(0),
            ),
            output("value", "Valeur", ValueType::Any),
        ],
        Kind::BranchEnd => vec![input("exec_in", "", ValueType::Execution)],
    }
}
