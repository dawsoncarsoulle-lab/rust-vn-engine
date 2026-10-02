//! Platform-independent descriptions for reusable RVN screens. The existing
//! menu document and its interaction graphs remain unchanged.
pub use crate::programmable_layout::{layout_rects, ComponentRect};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComponentKind {
    Panel,
    Row,
    Column,
    Text,
    Image,
    Button,
    Input,
    Select,
    Toggle,
    Slider,
    Canvas,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScrollAxis {
    #[default]
    None,
    Vertical,
    Horizontal,
    Both,
}
impl ScrollAxis {
    pub fn vertical(self) -> bool {
        matches!(self, Self::Vertical | Self::Both)
    }
    pub fn horizontal(self) -> bool {
        matches!(self, Self::Horizontal | Self::Both)
    }
}

/// The same reference-pixel layout vocabulary is used by RVN, the designer
/// and every renderer. Defaults retain the original screen appearance.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Alignment {
    Start,
    Center,
    End,
    #[default]
    Stretch,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Justification {
    #[default]
    Start,
    Center,
    End,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextAlignment {
    #[default]
    Left,
    Center,
    Right,
}

/// Styles are ordinary RVN dictionaries, or a list of dictionaries applied
/// left-to-right. Identity, data bindings, events and content are deliberately
/// excluded: changing a shared visual style cannot change game logic.
pub const STYLE_PROPERTIES: &[&str] = &[
    "rect",
    "width",
    "height",
    "min_width",
    "min_height",
    "max_width",
    "max_height",
    "scroll",
    "spacing",
    "padding",
    "margin",
    "font",
    "font_size",
    "foreground",
    "background",
    "align",
    "justify",
    "text_align",
    "wrap",
    "clip",
    "opacity",
    "border_width",
    "border_color",
    "radius",
    "hover_background",
    "pressed_background",
    "focus_color",
    "auto_background",
];

/// Expand local style cascades without regenerating or mutating an authored
/// description. Direct component properties always win over the cascade.
fn expand_styles(
    value: &mut serde_json::Value,
    depth: usize,
    count: &mut usize,
) -> Result<(), String> {
    if depth > 32 || *count >= 512 {
        return Err("Screen limit: 512 components and 32 nesting levels".into());
    }
    *count += 1;
    let object = value
        .as_object_mut()
        .ok_or("A screen component must be a dictionary")?;
    if let Some(style) = object.remove("style") {
        let styles = match style {
            serde_json::Value::Object(style) => vec![serde_json::Value::Object(style)],
            serde_json::Value::Array(styles) if styles.len() <= 32 => styles,
            _ => {
                return Err(
                    "Component style must be a dictionary or at most 32 style dictionaries".into(),
                )
            }
        };
        let mut cascade = serde_json::Map::new();
        for style in styles {
            let style = style
                .as_object()
                .ok_or("Every style in a cascade must be a dictionary")?;
            for (key, value) in style {
                if !STYLE_PROPERTIES.contains(&key.as_str()) {
                    return Err(format!("Unknown visual style property: {key}"));
                }
                cascade.insert(key.clone(), value.clone());
            }
        }
        for (key, value) in cascade {
            object.entry(key).or_insert(value);
        }
    }
    if let Some(children) = object.get_mut("children") {
        let children = children
            .as_array_mut()
            .ok_or("Component children must be a list")?;
        if children.len() > 512 {
            return Err("Screen limit: 512 components and 32 nesting levels".into());
        }
        for child in children {
            expand_styles(child, depth + 1, count)?;
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScreenEventKind {
    Open,
    Close,
    Click,
    Activate,
    Focus,
    Change,
    Key,
    PointerDown,
    PointerMove,
    PointerUp,
    PointerCancel,
    Wheel,
    Tick,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScreenView {
    pub name: String,
    pub modal: bool,
    pub layer: i32,
    pub order: u64,
    pub focus: Option<String>,
    pub root: Component,
}

fn yes() -> bool {
    true
}
fn spacing() -> f32 {
    8.0
}
fn font_size() -> f32 {
    24.0
}
fn white() -> [f32; 4] {
    [0.9, 0.94, 1.0, 1.0]
}
fn transparent() -> [f32; 4] {
    [0.0; 4]
}
fn one() -> f64 {
    1.0
}
fn opacity() -> f32 {
    1.0
}
fn radius() -> f32 {
    6.0
}
fn border_color() -> [f32; 4] {
    [0.18, 0.29, 0.38, 1.0]
}
pub fn safe_asset_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 4096
        && !path.starts_with('/')
        && !path
            .chars()
            .any(|ch| ch == '\\' || ch == ':' || ch == '#' || ch.is_control())
        && path
            .split('/')
            .all(|part| !part.is_empty() && part != ".." && part != ".")
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Component {
    pub id: String,
    pub kind: ComponentKind,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub text_key: Option<String>,
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default)]
    pub value: serde_json::Value,
    /// Name of a game variable, not an expression or an editor preference.
    #[serde(default)]
    pub binding: Option<String>,
    #[serde(default)]
    pub options: Vec<String>,
    /// Display labels are separate from the stable values written to bindings.
    #[serde(default)]
    pub option_labels: Vec<String>,
    #[serde(default)]
    pub option_keys: Vec<String>,
    #[serde(default = "yes")]
    pub visible: bool,
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(default)]
    pub focus_order: i32,
    #[serde(default)]
    pub accessible_label: Option<String>,
    #[serde(default)]
    pub accessible_label_key: Option<String>,
    #[serde(default)]
    pub placeholder: String,
    #[serde(default)]
    pub placeholder_key: Option<String>,
    /// Optional absolute rectangle in the existing menu's 1920×1080 reference.
    /// Otherwise containers use flex layout, with automatic content height.
    #[serde(default)]
    pub rect: Option<[f32; 4]>,
    #[serde(default)]
    pub width: Option<f32>,
    #[serde(default)]
    pub height: Option<f32>,
    #[serde(default)]
    pub min_width: Option<f32>,
    #[serde(default)]
    pub min_height: Option<f32>,
    #[serde(default)]
    pub max_width: Option<f32>,
    #[serde(default)]
    pub max_height: Option<f32>,
    /// Scrollable containers retain their layout; children never shrink to fit.
    #[serde(default)]
    pub scroll: ScrollAxis,
    #[serde(default = "spacing")]
    pub spacing: f32,
    #[serde(default)]
    pub padding: f32,
    /// Top, right, bottom, left, in the 1920×1080 reference coordinates.
    #[serde(default)]
    pub margin: [f32; 4],
    #[serde(default)]
    pub align: Alignment,
    #[serde(default)]
    pub justify: Justification,
    #[serde(default)]
    pub text_align: TextAlignment,
    #[serde(default)]
    pub wrap: bool,
    #[serde(default)]
    pub clip: bool,
    #[serde(default = "opacity")]
    pub opacity: f32,
    #[serde(default)]
    pub border_width: Option<f32>,
    #[serde(default = "border_color")]
    pub border_color: [f32; 4],
    #[serde(default = "radius")]
    pub radius: f32,
    #[serde(default)]
    pub hover_background: Option<[f32; 4]>,
    #[serde(default)]
    pub pressed_background: Option<[f32; 4]>,
    #[serde(default)]
    pub focus_color: Option<[f32; 4]>,
    #[serde(default = "yes")]
    pub auto_background: bool,
    #[serde(default = "font_size")]
    pub font_size: f32,
    /// Project-relative TTF/OTF resource. Inherited by descendants when absent.
    #[serde(default)]
    pub font: Option<String>,
    #[serde(default = "white")]
    pub foreground: [f32; 4],
    #[serde(default = "transparent")]
    pub background: [f32; 4],
    #[serde(default)]
    pub min: f64,
    #[serde(default = "one")]
    pub max: f64,
    #[serde(default)]
    pub events: BTreeMap<ScreenEventKind, String>,
    /// Reusable components pass domain data to handlers as event["data"].
    #[serde(default)]
    pub event_data: BTreeMap<String, serde_json::Value>,
    /// Named pure RVN function with (state, props, frame) parameters.
    #[serde(default)]
    pub draw: Option<String>,
    #[serde(default)]
    pub props: BTreeMap<String, serde_json::Value>,
    /// Authored initial state, not a sampled or serialized render result.
    #[serde(default)]
    pub state: BTreeMap<String, serde_json::Value>,
    #[serde(default = "yes")]
    pub capture_pointer: bool,
    #[serde(default = "yes")]
    pub consume_input: bool,
    /// Computed on demand from RVN and the saved instance. Never accepted from
    /// source descriptions, saves, graph exports or external JSON.
    #[serde(skip)]
    pub drawing: Option<crate::custom_canvas::CanvasDrawing>,
    #[serde(skip)]
    pub canvas_frame: Option<crate::custom_canvas::CanvasFrame>,
    #[serde(default)]
    pub children: Vec<Component>,
}

impl Component {
    pub fn parse(mut description: serde_json::Value) -> Result<Self, String> {
        expand_styles(&mut description, 0, &mut 0)?;
        let mut root: Self = serde_json::from_value(description)
            .map_err(|error| format!("Invalid screen component: {error}"))?;
        root.visit_mut(&mut |component| {
            if component.value.is_null() && component.binding.is_none() {
                component.value = match component.kind {
                    ComponentKind::Input => "".into(),
                    ComponentKind::Toggle => false.into(),
                    ComponentKind::Select => component
                        .options
                        .first()
                        .cloned()
                        .unwrap_or_default()
                        .into(),
                    ComponentKind::Slider => serde_json::json!(component.min),
                    _ => serde_json::Value::Null,
                };
            }
        });
        root.validate()?;
        Ok(root)
    }

    pub fn validate(&self) -> Result<(), String> {
        fn walk(node: &Component, depth: usize, ids: &mut BTreeSet<String>) -> Result<(), String> {
            if depth > 32 || ids.len() >= 512 {
                return Err("Screen limit: 512 components and 32 nesting levels".into());
            }
            if node.id.is_empty() || node.id.len() > 128 || !ids.insert(node.id.clone()) {
                return Err(format!(
                    "Empty, oversized or duplicate component identity: {}",
                    node.id
                ));
            }
            for text in [&node.text, &node.placeholder] {
                if text.len() > 65_536 {
                    return Err(format!("Component text is too long: {}", node.id));
                }
            }
            if [
                &node.text_key,
                &node.placeholder_key,
                &node.accessible_label_key,
            ]
            .into_iter()
            .flatten()
            .any(|key| key.is_empty() || key.len() > 4096)
                || node
                    .accessible_label
                    .as_ref()
                    .is_some_and(|text| text.len() > 65_536)
            {
                return Err(format!(
                    "Invalid localization key or accessible label: {}",
                    node.id
                ));
            }
            if node.options.len() > 256 || node.options.iter().any(|text| text.len() > 4096) {
                return Err(format!(
                    "Too many or oversized selection options: {}",
                    node.id
                ));
            }
            if [&node.option_labels, &node.option_keys]
                .into_iter()
                .any(|items| {
                    !items.is_empty()
                        && (items.len() != node.options.len()
                            || items.iter().any(|text| text.len() > 4096))
                })
                || node.option_keys.iter().any(String::is_empty)
            {
                return Err(format!(
                    "Selection labels/translation keys must match option values: {}",
                    node.id
                ));
            }
            if node.binding.as_ref().is_some_and(|name| {
                name.is_empty() || name.starts_with("__rvn_") || name.starts_with("persistent.")
            }) {
                return Err(format!("Invalid game-variable binding: {}", node.id));
            }
            if node
                .events
                .values()
                .any(|name| name.is_empty() || name.len() > 128)
            {
                return Err(format!("Invalid handler reference: {}", node.id));
            }
            if node
                .image
                .as_ref()
                .is_some_and(|image| !safe_asset_path(image))
            {
                return Err(format!(
                    "Image must be a relative resource inside the project's asset directory: {}",
                    node.id
                ));
            }
            if node.font.as_ref().is_some_and(|font| {
                !safe_asset_path(font)
                    || !["ttf", "otf"].contains(
                        &font
                            .rsplit('.')
                            .next()
                            .unwrap_or_default()
                            .to_ascii_lowercase()
                            .as_str(),
                    )
            }) {
                return Err(format!("Font must be a relative TTF/OTF resource inside the project's asset directory: {}",node.id));
            }
            let geometry = [
                node.width,
                node.height,
                node.min_width,
                node.min_height,
                node.max_width,
                node.max_height,
                node.border_width,
                Some(node.radius),
                Some(node.spacing),
                Some(node.padding),
                Some(node.font_size),
            ];
            if geometry
                .into_iter()
                .flatten()
                .any(|n| !n.is_finite() || !(0.0..=16384.0).contains(&n))
                || node.font_size == 0.0
            {
                return Err(format!("Invalid component dimensions: {}", node.id));
            }
            if node
                .margin
                .iter()
                .any(|n| !n.is_finite() || n.abs() > 16384.0)
                || [
                    (node.min_width, node.max_width),
                    (node.min_height, node.max_height),
                ]
                .into_iter()
                .any(|(min, max)| min.zip(max).is_some_and(|(min, max)| min > max))
                || !node.opacity.is_finite()
                || !(0.0..=1.0).contains(&node.opacity)
            {
                return Err(format!("Invalid component layout or opacity: {}", node.id));
            }
            if node.rect.is_some_and(|rect| {
                rect.iter().any(|n| !n.is_finite() || n.abs() > 16384.0)
                    || rect[2] < 0.0
                    || rect[3] < 0.0
            }) {
                return Err(format!("Invalid component rectangle: {}", node.id));
            }
            if node
                .foreground
                .iter()
                .chain(&node.background)
                .chain(&node.border_color)
                .chain(node.hover_background.iter().flatten())
                .chain(node.pressed_background.iter().flatten())
                .chain(node.focus_color.iter().flatten())
                .any(|n| !n.is_finite() || !(0.0..=1.0).contains(n))
            {
                return Err(format!("Invalid component color: {}", node.id));
            }
            if node.event_data.len() > 128
                || serde_json::to_string(&node.event_data).map_or(true, |data| data.len() > 65_536)
            {
                return Err(format!("Component event data is too large: {}", node.id));
            }
            fn valid_data(value: &serde_json::Value, depth: usize) -> bool {
                if depth > 16 {
                    return false;
                }
                match value {
                    serde_json::Value::Null => false,
                    serde_json::Value::Array(items) => {
                        items.len() <= 512 && items.iter().all(|item| valid_data(item, depth + 1))
                    }
                    serde_json::Value::Object(items) => {
                        items.len() <= 128
                            && items.iter().all(|(key, value)| {
                                key.len() <= 4096 && valid_data(value, depth + 1)
                            })
                    }
                    _ => true,
                }
            }
            if node.event_data.values().any(|value| !valid_data(value, 0)) {
                return Err(format!(
                    "Component event data is invalid or too deeply nested: {}",
                    node.id
                ));
            }
            if node
                .draw
                .as_ref()
                .is_some_and(|name| name.is_empty() || name.len() > 128)
                || [&node.props, &node.state].into_iter().any(|data| {
                    data.len() > 128
                        || serde_json::to_string(data).map_or(true, |text| text.len() > 65_536)
                        || data.values().any(|value| !valid_data(value, 0))
                })
            {
                return Err(format!(
                    "Canvas function, props or initial state is invalid or oversized: {}",
                    node.id
                ));
            }
            if node.kind != ComponentKind::Canvas
                && (node.draw.is_some() || !node.props.is_empty() || !node.state.is_empty())
            {
                return Err(format!(
                    "Only a canvas accepts draw, props and local state: {}",
                    node.id
                ));
            }
            if node.kind != ComponentKind::Canvas && (!node.capture_pointer || !node.consume_input)
            {
                return Err(format!(
                    "Pointer capture and consumption properties require a canvas: {}",
                    node.id
                ));
            }
            if node.kind != ComponentKind::Canvas
                && node.events.keys().any(|event| {
                    matches!(
                        event,
                        ScreenEventKind::PointerDown
                            | ScreenEventKind::PointerMove
                            | ScreenEventKind::PointerUp
                            | ScreenEventKind::PointerCancel
                            | ScreenEventKind::Wheel
                            | ScreenEventKind::Tick
                    )
                })
            {
                return Err(format!(
                    "Pointer and tick handlers require a canvas: {}",
                    node.id
                ));
            }
            if let Some(drawing) = &node.drawing {
                drawing.validate()?;
            }
            if !node.min.is_finite() || !node.max.is_finite() || node.min >= node.max {
                return Err(format!("Invalid control range: {}", node.id));
            }
            let container = matches!(
                node.kind,
                ComponentKind::Panel | ComponentKind::Row | ComponentKind::Column
            );
            if !container && !node.children.is_empty() {
                return Err(format!(
                    "Only containers accept child components: {}",
                    node.id
                ));
            }
            if !container && node.scroll != ScrollAxis::None {
                return Err(format!("Only containers can scroll: {}", node.id));
            }
            if node.kind == ComponentKind::Image && node.image.is_none() {
                return Err(format!("Image resource missing: {}", node.id));
            }
            if node.binding.is_some() && !node.is_control() {
                return Err(format!(
                    "Only input controls support a variable binding: {}",
                    node.id
                ));
            }
            node.validate_value(&node.value)?;
            for child in &node.children {
                walk(child, depth + 1, ids)?;
            }
            Ok(())
        }
        walk(self, 0, &mut BTreeSet::new())
    }

    pub fn is_control(&self) -> bool {
        matches!(
            self.kind,
            ComponentKind::Input
                | ComponentKind::Select
                | ComponentKind::Toggle
                | ComponentKind::Slider
        )
    }
    pub fn option_label(&self, value: &str) -> &str {
        self.options
            .iter()
            .position(|option| option == value)
            .and_then(|index| self.option_labels.get(index))
            .map(String::as_str)
            .unwrap_or_else(|| {
                self.options
                    .iter()
                    .find(|option| option.as_str() == value)
                    .map(String::as_str)
                    .unwrap_or("")
            })
    }
    pub fn is_focusable(&self) -> bool {
        self.enabled
            && self.visible
            && self.focus_order >= 0
            && (self.is_control()
                || matches!(self.kind, ComponentKind::Button | ComponentKind::Canvas))
    }
    pub fn validate_value(&self, value: &serde_json::Value) -> Result<(), String> {
        if value.is_null() && self.binding.is_some() {
            return Ok(());
        } // Filled by the game before rendering.
        let valid = match self.kind {
            ComponentKind::Input => value.as_str().is_some_and(|text| text.len() <= 65_536),
            ComponentKind::Select => value.as_str().is_some_and(|text| {
                if self.options.is_empty() {
                    text.is_empty()
                } else {
                    self.options.iter().any(|option| option == text)
                }
            }),
            ComponentKind::Toggle => value.is_boolean(),
            ComponentKind::Slider => value
                .as_f64()
                .is_some_and(|n| n.is_finite() && n >= self.min && n <= self.max),
            _ => true,
        };
        if valid {
            Ok(())
        } else {
            Err(format!(
                "Invalid control value for {} ({:?})",
                self.id, self.kind
            ))
        }
    }
    pub fn find(&self, id: &str) -> Option<&Self> {
        if self.id == id {
            return Some(self);
        }
        self.children.iter().find_map(|child| child.find(id))
    }
    pub fn find_mut(&mut self, id: &str) -> Option<&mut Self> {
        if self.id == id {
            return Some(self);
        }
        self.children
            .iter_mut()
            .find_map(|child| child.find_mut(id))
    }
    pub fn visit(&self, action: &mut impl FnMut(&Self)) {
        action(self);
        for child in &self.children {
            child.visit(action);
        }
    }
    pub fn visit_mut(&mut self, action: &mut impl FnMut(&mut Self)) {
        action(self);
        for child in &mut self.children {
            child.visit_mut(action);
        }
    }
    pub fn focus_order(&self) -> Vec<String> {
        fn collect(node: &Component, available: bool, items: &mut Vec<(i32, String)>) {
            let available = available && node.enabled && node.visible;
            if available && node.is_focusable() {
                items.push((node.focus_order, node.id.clone()));
            }
            for child in &node.children {
                collect(child, available, items);
            }
        }
        let mut items = Vec::new();
        collect(self, true, &mut items);
        // Stable sort preserves authored order when tab indices are equal.
        items.sort_by_key(|item| item.0);
        items.into_iter().map(|item| item.1).collect()
    }
    pub fn available(&self, id: &str) -> bool {
        self.visible
            && self.enabled
            && (self.id == id || self.children.iter().any(|child| child.available(id)))
    }
    /// Opacity is inherited by children, unlike a color's own alpha channel.
    pub fn effective_opacity(&self, id: &str) -> Option<f32> {
        if self.id == id {
            return Some(self.opacity);
        }
        self.children
            .iter()
            .find_map(|child| child.effective_opacity(id))
            .map(|opacity| opacity * self.opacity)
    }
    pub fn effective_font(&self, id: &str) -> Option<&str> {
        fn walk<'a>(node: &'a Component, id: &str, inherited: Option<&'a str>) -> Option<&'a str> {
            let font = node.font.as_deref().or(inherited);
            if node.id == id {
                return font;
            }
            node.children
                .iter()
                .find(|child| child.find(id).is_some())
                .and_then(|child| walk(child, id, font))
        }
        walk(self, id, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn descriptions_are_strict_bounded_and_ordered() {
        let component =
            Component::parse(serde_json::json!({"id":"root","kind":"column","children":[
                {"id":"name","kind":"input","value":"","focus_order":2},
                {"id":"ok","kind":"button","text":"OK","focus_order":1}
            ]}))
            .unwrap();
        assert_eq!(component.focus_order(), ["ok", "name"]);
        assert!(Component::parse(serde_json::json!({"id":"x","kind":"text","typo":true})).is_err());
        assert!(Component::parse(serde_json::json!({"id":"root","kind":"column","children":[{"id":"root","kind":"button"}]})).is_err());
        assert!(Component::parse(serde_json::json!({"id":"x","kind":"slider","value":2})).is_err());
    }
}
