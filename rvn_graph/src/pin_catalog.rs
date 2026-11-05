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

/// Extra real outputs of the explicitly selected demand contract only.
/// Historical ForEach catalogue/schema/pins are unchanged.
pub(crate) fn native_loop_output_definitions() -> Vec<PinDefinition> {
    vec![output("array_index", "Array Index", ValueType::Int),
         output("array_element", "Array Element", ValueType::Any)]
}

pub(crate) fn pin_definitions(kind: NodeKind) -> Vec<PinDefinition> {
    use NodeKind as Kind;
    if let Some(pins) = table_pin_definitions(kind, crate::StringTableQueryContract::NameV2) { return pins; }
    if let Some((_, keys)) = kind.menu_constructor() {
        let mut pins: Vec<_> = keys.iter().map(|key| match (kind, *key) {
            (_, "slot" | "index" | "token") | (Kind::MenuSavePage, "page") => input_default(key, key, ValueType::Int, PropertyValue::Int(if *key == "slot" || *key == "page" {1} else {0})),
            (_, "protected") | (Kind::MenuBoolPreference, "value") => input_default(key, key, ValueType::Bool, PropertyValue::Bool(true)),
            (Kind::MenuNumberPreference, "value") => input_default(key, key, ValueType::Float, PropertyValue::Float(1.0)),
            _ => input_default(key, key, ValueType::String, PropertyValue::String(match (kind, *key) {
                (Kind::MenuAction, _) => "resume", (Kind::MenuSlot, "operation") => "save",
                (Kind::MenuNumberPreference, "key") => "music_volume", (Kind::MenuBoolPreference, "key") => "typewriter",
                (Kind::MenuLanguage, _) => "fr", (Kind::MenuGalleryTab, _) => "cg", _ => "",
            }.into())),
        }).collect();
        pins.push(output("request", "Demande", ValueType::MenuRequest)); return pins;
    }
    match kind {
        Kind::WidgetClassValue => vec![output("value", "Classe", ValueType::WidgetClass)],
        Kind::WidgetObjectClass => vec![input("object", "Object", ValueType::WidgetObject), output("value", "Return Value", ValueType::WidgetClass)],
        Kind::WidgetAddToViewport => {let mut pins=exec_in_out();pins.extend([input("object","Target",ValueType::WidgetObject),input_default("z_order","ZOrder",ValueType::Int,PropertyValue::Int(0))]);pins},
        Kind::WidgetRemoveFromParent => {let mut pins=exec_in_out();pins.push(input("object","Target",ValueType::WidgetObject));pins},
        Kind::WidgetCreate => {
            let mut pins = exec_in_out();
            pins.extend([
                input("class", "Classe", ValueType::WidgetClass),
                input_default("arguments", "Arguments", ValueType::List(Box::new(ValueType::Any)), PropertyValue::StringList(vec![])),
                output("value_out", "Valeur de retour", ValueType::WidgetObject),
            ]);
            pins
        }
        Kind::TextFromStringTable => vec![
            input_default("table", "Table Id", ValueType::Name, PropertyValue::Name(crate::NameDefault::default())),
            input_default("key", "Key", ValueType::String, PropertyValue::String(String::new())),
            output("result", "Return Value", ValueType::InterpolatedText),
        ],
        Kind::GetStringTableIdAndKey => vec![
            input("text", "Text", ValueType::InterpolatedText),
            output("valid", "Return Value", ValueType::Bool),
            output("table_id", "Table Id", ValueType::Name),
            output("key", "Key", ValueType::String),
        ],
        Kind::StringTableId => vec![input_default("asset", "Table de chaînes", ValueType::Asset(AssetKind::StringTable), PropertyValue::String(String::new())), output("result", "Return Value", ValueType::String)],
        Kind::StringTableAsset => vec![output("value", "", ValueType::Asset(AssetKind::StringTable))],
        Kind::StringTableRegistered | Kind::StringTableEntryRegistered
        | Kind::StringTableNamespace | Kind::StringTableSource | Kind::StringTableMetadata
        | Kind::RegisteredStringTables | Kind::StringTableKeys | Kind::StringTableMetadataIds => unreachable!("Table query handled above"),
        Kind::MenuExecute => vec![input("exec_in", "", ValueType::Execution),
            input("request", "Demande", ValueType::MenuRequest), output("exec_out", "", ValueType::Execution)],
        Kind::MenuAction | Kind::MenuStartScene | Kind::MenuOpenPage | Kind::MenuSlot | Kind::MenuProtect
        | Kind::MenuSavePage | Kind::MenuNumberPreference | Kind::MenuBoolPreference | Kind::MenuLanguage
        | Kind::MenuAdvance | Kind::MenuSkipTypewriter | Kind::MenuChoose | Kind::MenuGalleryCg | Kind::MenuGalleryTab
        | Kind::MenuConfirm | Kind::MenuCancel => unreachable!("Menu constructor handled above"),
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
        Kind::UnitCall => exec_in_out(),
        Kind::UnitReturn => vec![input("exec_in", "", ValueType::Execution)],
        Kind::FunctionReturn => vec![
            input("exec_in", "", ValueType::Execution),
            input("value", "Valeur retournée", ValueType::Any),
        ],
        Kind::DoOnce|Kind::DoN|Kind::FlipFlop|Kind::Gate=>crate::standard_macros_1020::pins(kind),
        Kind::Sequence => vec![
            input("exec_in", "", ValueType::Execution),
            output("then_0", "Puis 0", ValueType::Execution),
            output("then_1", "Puis 1", ValueType::Execution),
            output("completed", "Après toutes les sorties", ValueType::Execution),
        ],
        Kind::ForLoopWithBreak => vec![
            input("exec_in", "", ValueType::Execution),
            input_default("first", "Premier indice", ValueType::Int, PropertyValue::Int(0)),
            input_default("last", "Dernier indice inclus", ValueType::Int, PropertyValue::Int(0)),
            {let mut p=input("break", "Arrêter la boucle", ValueType::Execution);p.cardinality=PinCardinality::Many;p},
            output("body", "Corps de boucle", ValueType::Execution),
            output("index", "Indice", ValueType::Int),
            output("completed", "Terminé", ValueType::Execution),
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
                PropertyValue::Bool(true),
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
        Kind::ConvertBoolToInt => vec![
            input_default("value", "", ValueType::Bool, PropertyValue::Bool(false)),
            output("result", "", ValueType::Int),
        ],
        Kind::ConvertBoolToFloat => vec![
            input_default("value", "", ValueType::Bool, PropertyValue::Bool(false)),
            output("result", "", ValueType::Float),
        ],
        Kind::ConvertIntToFloat => vec![
            input("value", "", ValueType::Int),
            output("result", "", ValueType::Float),
        ],
        Kind::ConvertIntToBool => vec![
            input_default("value", "", ValueType::Int, PropertyValue::Int(0)),
            output("result", "", ValueType::Bool),
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
        Kind::ConvertStringToName => vec![
            input_default("value", "", ValueType::String, PropertyValue::String(String::new())),
            output("result", "", ValueType::Name),
        ],
        Kind::ConvertNameToString => vec![
            input_default("value", "", ValueType::Name, PropertyValue::Name(crate::NameDefault::default())),
            output("result", "", ValueType::String),
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

pub(crate) fn table_pin_definitions(kind: NodeKind, contract: crate::StringTableQueryContract) -> Option<Vec<PinDefinition>> {
    if kind == NodeKind::StringTableId {
        return Some(vec![input_default("asset", "Table de chaînes", ValueType::Asset(AssetKind::StringTable), PropertyValue::String(String::new())),
            output("result", "Return Value", if contract == crate::StringTableQueryContract::NameV2 { ValueType::Name } else { ValueType::String })]);
    }
    let (_, keys, result) = kind.string_table_query_with_contract(contract)?;
    let mut pins: Vec<_> = keys.iter().map(|key| {
        let ty = contract.input_type(key);
        let value = if ty == ValueType::Name { PropertyValue::Name(crate::NameDefault::default()) } else { PropertyValue::String(String::new()) };
        input_default(key, match *key { "table" => "Identifiant de table", "key" => "Clé", _ => "Identifiant de métadonnée" }, ty, value)
    }).collect();
    pins.push(output("result", "Return Value", result));
    Some(pins)
}
