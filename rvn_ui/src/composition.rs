//! Renderer-independent character compositions. The authored layer order is
//! also the drawing order; attributes select groups without resetting others.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub type Attributes = BTreeMap<String, String>;
pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && !name
            .chars()
            .any(|c| c.is_control() || matches!(c, ':' | '/'))
}
fn yes() -> bool {
    true
}
fn one() -> f32 {
    1.0
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageLayer {
    pub id: String,
    pub image: String,
    #[serde(default)]
    pub group: Option<String>,
    #[serde(default)]
    pub attribute: Option<String>,
    #[serde(default)]
    pub when: Attributes,
    #[serde(default)]
    pub unless: Attributes,
    /// Optional authored variant. A matching variant replaces the base layer
    /// in the same exclusive group, without hiding unrelated base layers.
    #[serde(default)]
    pub variant: Option<String>,
    #[serde(default = "yes")]
    pub visible: bool,
    #[serde(default = "one")]
    pub opacity: f32,
    /// Top-left position and size within the composition, in authored pixels.
    /// None uses the entire composition rectangle.
    #[serde(default)]
    pub rect: Option<[f32; 4]>,
}
impl ImageLayer {
    pub fn parse(value: serde_json::Value) -> Result<Self, String> {
        let layer: Self =
            serde_json::from_value(value).map_err(|e| format!("Invalid image layer: {e}"))?;
        layer.validate()?;
        Ok(layer)
    }
    pub fn validate(&self) -> Result<(), String> {
        if !valid_name(&self.id) {
            return Err("Layer identity must contain 1–128 characters, without / or :".into());
        }
        if !crate::programmable::safe_asset_path(&self.image) {
            return Err(format!(
                "Layer '{}' needs a project-relative image resource",
                self.id
            ));
        }
        match (&self.group, &self.attribute) {
            (None, None) => {}
            (Some(group), Some(attribute)) if valid_name(group) && valid_name(attribute) => {}
            _ => {
                return Err(format!(
                    "Layer '{}' needs both an exclusive group and an attribute",
                    self.id
                ))
            }
        }
        if self.variant.as_ref().is_some_and(|name| !valid_name(name)) {
            return Err(format!("Layer '{}' has an invalid variant name", self.id));
        }
        if !self.opacity.is_finite() || !(0.0..=1.0).contains(&self.opacity) {
            return Err("Layer opacity must be between 0 and 1".into());
        }
        if let Some([x, y, w, h]) = self.rect {
            if ![x, y, w, h].iter().all(|v| v.is_finite())
                || x.abs() > 8192.0
                || y.abs() > 8192.0
                || !(0.001..=8192.0).contains(&w)
                || !(0.001..=8192.0).contains(&h)
            {
                return Err(format!(
                    "Layer '{}' has an invalid finite rectangle",
                    self.id
                ));
            }
        }
        for values in [&self.when, &self.unless] {
            if values.len() > 64
                || values
                    .iter()
                    .any(|(group, value)| !valid_name(group) || !valid_name(value))
            {
                return Err("Layer conditions need at most 64 named attribute values".into());
            }
        }
        Ok(())
    }
    pub fn selected(&self, attributes: &Attributes) -> bool {
        self.visible
            && self
                .variant
                .as_ref()
                .is_none_or(|variant| attributes.get("variant") == Some(variant))
            && self
                .group
                .as_ref()
                .is_none_or(|group| attributes.get(group) == self.attribute.as_ref())
            && self
                .when
                .iter()
                .all(|(key, value)| attributes.get(key) == Some(value))
            && !self
                .unless
                .iter()
                .any(|(key, value)| attributes.get(key) == Some(value))
    }
}

/// Rules are applied once in their lexical name order. They cannot recurse or
/// execute narrative actions. A selector RVN function can express more complex
/// policies using the same bounded computation engine as other RVN functions.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttributeRule {
    #[serde(default)]
    pub when: Attributes,
    #[serde(default)]
    pub unless: Attributes,
    pub set: Attributes,
}
impl AttributeRule {
    fn matches(&self, attributes: &Attributes) -> bool {
        self.when
            .iter()
            .all(|(key, value)| attributes.get(key) == Some(value))
            && !self
                .unless
                .iter()
                .any(|(key, value)| attributes.get(key) == Some(value))
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompositionOptions {
    #[serde(default)]
    pub variants: BTreeMap<String, Attributes>,
    #[serde(default)]
    pub rules: BTreeMap<String, AttributeRule>,
    #[serde(default)]
    pub selector: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Composition {
    pub size: [f32; 2],
    #[serde(default)]
    pub defaults: Attributes,
    pub layers: Vec<ImageLayer>,
    #[serde(default)]
    pub options: CompositionOptions,
}
impl Composition {
    pub fn parse(value: serde_json::Value) -> Result<Self, String> {
        let definition: Self = serde_json::from_value(value)
            .map_err(|e| format!("Invalid character composition: {e}"))?;
        definition.validate()?;
        Ok(definition)
    }
    pub fn choices(&self) -> BTreeMap<String, BTreeSet<String>> {
        let mut groups: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for layer in &self.layers {
            if let (Some(group), Some(attribute)) = (&layer.group, &layer.attribute) {
                groups
                    .entry(group.clone())
                    .or_default()
                    .insert(attribute.clone());
            }
            for (key, value) in layer.when.iter().chain(&layer.unless) {
                groups.entry(key.clone()).or_default().insert(value.clone());
            }
            if let Some(variant) = &layer.variant {
                groups
                    .entry("variant".into())
                    .or_default()
                    .insert(variant.clone());
            }
        }
        for name in self.options.variants.keys() {
            groups
                .entry("variant".into())
                .or_default()
                .insert(name.clone());
        }
        groups
    }
    pub fn validate(&self) -> Result<(), String> {
        if !self
            .size
            .iter()
            .all(|v| v.is_finite() && (0.001..=8192.0).contains(v))
        {
            return Err("Composition width and height must be between 0 and 8192 pixels".into());
        }
        if self.layers.is_empty() || self.layers.len() > 128 {
            return Err("A character composition needs 1–128 ordered layers".into());
        }
        let mut names = BTreeSet::new();
        for layer in &self.layers {
            layer.validate()?;
            if !names.insert(&layer.id) {
                return Err(format!("Duplicate composition layer '{}'", layer.id));
            }
        }
        if self.choices().len() > 64 {
            return Err("Composition limit: 64 attribute groups".into());
        }
        self.validate_attributes(&self.defaults)?;
        if self.options.variants.len() > 64 || self.options.rules.len() > 64 {
            return Err("Composition limit: 64 variants and 64 selection rules".into());
        }
        for (name, values) in &self.options.variants {
            if !valid_name(name) || values.contains_key("variant") {
                return Err(
                    "Variant presets need a valid name and cannot select another variant".into(),
                );
            }
            self.validate_attributes(values)?;
        }
        for (name, rule) in &self.options.rules {
            if !valid_name(name) || rule.set.is_empty() || rule.set.contains_key("variant") {
                return Err("Selection rules need a valid name and a nonempty attribute patch, without variant changes".into());
            }
            for attributes in [&rule.when, &rule.unless, &rule.set] {
                self.validate_attributes(attributes)?;
            }
        }
        if self.options.selector.as_ref().is_some_and(|name| {
            !valid_name(name)
                || !name.chars().enumerate().all(|(index, c)| {
                    c == '_' || c.is_alphabetic() || index > 0 && c.is_ascii_digit()
                })
        }) {
            return Err("Attribute selector must name an RVN computation function".into());
        }
        Ok(())
    }
    pub fn validate_attributes(&self, attributes: &Attributes) -> Result<(), String> {
        if attributes.len() > 64 {
            return Err("Composition limit: 64 attributes".into());
        }
        let choices = self.choices();
        for (key, value) in attributes {
            let options = choices
                .get(key)
                .ok_or_else(|| format!("Unknown character attribute group '{key}'"))?;
            // Empty clears one group without resetting any other selection.
            if !value.is_empty() && !options.contains(value) {
                return Err(format!(
                    "Unknown value '{value}' for character attribute group '{key}'"
                ));
            }
        }
        Ok(())
    }
    /// Partial updates never reset unmentioned groups. Selecting a variant
    /// applies its preset before the explicit patch, so explicit values win.
    pub fn select_attributes(
        &self,
        previous: &Attributes,
        patch: &Attributes,
    ) -> Result<Attributes, String> {
        self.validate_attributes(previous)?;
        self.validate_attributes(patch)?;
        let mut result = previous.clone();
        if let Some(preset) = patch
            .get("variant")
            .and_then(|name| self.options.variants.get(name))
        {
            result.extend(preset.clone());
        }
        result.extend(patch.clone());
        for rule in self.options.rules.values() {
            if rule.matches(&result) {
                result.extend(rule.set.clone());
            }
        }
        self.validate_attributes(&result)?;
        Ok(result)
    }
    pub fn visible_layer_indices(&self, attributes: &Attributes) -> Vec<usize> {
        self.layers
            .iter()
            .enumerate()
            .filter(|(_, layer)| {
                layer.selected(attributes)
                    && !(layer.variant.is_none()
                        && layer.group.is_some()
                        && self.layers.iter().any(|override_layer| {
                            override_layer.variant.is_some()
                                && override_layer.group == layer.group
                                && override_layer.attribute == layer.attribute
                                && override_layer.selected(attributes)
                        }))
            })
            .map(|(index, _)| index)
            .collect()
    }
    pub fn visible_layers<'a>(
        &'a self,
        attributes: &Attributes,
    ) -> impl Iterator<Item = &'a ImageLayer> {
        self.visible_layer_indices(attributes)
            .into_iter()
            .map(|index| &self.layers[index])
    }
    pub fn image_paths(&self) -> BTreeSet<String> {
        self.layers
            .iter()
            .map(|layer| layer.image.clone())
            .collect()
    }
}

/// Discover portable image declarations from an explicit project asset list.
/// Filenames use `prefix__id`, `prefix__group__attribute`, or
/// `prefix__variant__group__attribute`. Directory and extension are retained;
/// nonmatching images are ignored, matching malformed names are diagnosed.
/// This is deterministic and filesystem-independent on Desktop and Web.
pub fn discover_layers(prefix: &str, images: &[String]) -> Result<Vec<ImageLayer>, String> {
    if !valid_name(prefix) || prefix.contains("__") {
        return Err("Image discovery needs a nonempty prefix without __".into());
    }
    if images.len() > 4096 {
        return Err("Image discovery limit: 4096 project resources".into());
    }
    let marker = format!("{prefix}__");
    let mut result = Vec::new();
    let mut names = BTreeSet::new();
    let mut seen_paths = BTreeSet::new();
    for image in images.iter().cloned() {
        if !seen_paths.insert(image.clone()) {
            continue;
        }
        if !crate::programmable::safe_asset_path(&image) {
            return Err("Image discovery accepts only project-relative resources".into());
        }
        let basename = image.rsplit('/').next().unwrap();
        let Some((stem, extension)) = basename.rsplit_once('.') else {
            continue;
        };
        if !matches!(
            extension.to_ascii_lowercase().as_str(),
            "png" | "jpg" | "jpeg" | "webp"
        ) {
            continue;
        }
        let Some(suffix) = stem.strip_prefix(&marker) else {
            continue;
        };
        let parts: Vec<_> = suffix.split("__").collect();
        if !(1..=3).contains(&parts.len()) || parts.iter().any(|part| !valid_name(part)) {
            return Err(format!("Invalid discovered image name '{image}': expected {prefix}__id or {prefix}__group__attribute or {prefix}__variant__group__attribute"));
        }
        let (group, attribute, variant) = match parts.as_slice() {
            [_] => (None, None, None),
            [group, attribute] => (Some((*group).into()), Some((*attribute).into()), None),
            [variant, group, attribute] => (
                Some((*group).into()),
                Some((*attribute).into()),
                Some((*variant).into()),
            ),
            _ => unreachable!(),
        };
        let id = suffix.replace("__", "_");
        if !names.insert(id.clone()) {
            return Err(format!(
                "Discovered layer identity '{id}' is ambiguous; use distinct asset names"
            ));
        }
        let layer = ImageLayer {
            id,
            image,
            group,
            attribute,
            variant,
            when: Attributes::new(),
            unless: Attributes::new(),
            visible: true,
            opacity: 1.0,
            rect: None,
        };
        layer.validate()?;
        result.push(layer);
        if result.len() > 128 {
            return Err("Image discovery limit: 128 matching layers".into());
        }
    }
    if result.is_empty() {
        return Err(format!("No project images match the prefix '{prefix}__'"));
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn discovery_variants_override_only_the_matching_base_group() {
        let paths = [
            "images/iris__body.png",
            "images/iris__face__neutral.png",
            "images/iris__face__happy.png",
            "images/iris__winter__face__neutral.png",
            "images/iris__outfit__coat.png",
            "images/unrelated.png",
        ]
        .map(String::from);
        let layers = discover_layers("iris", &paths).unwrap();
        assert_eq!(layers.len(), 5);
        let definition = Composition {
            size: [600.0, 1000.0],
            defaults: Attributes::from([
                ("face".into(), "neutral".into()),
                ("outfit".into(), "coat".into()),
            ]),
            layers,
            options: CompositionOptions {
                variants: BTreeMap::from([(
                    "winter".into(),
                    Attributes::from([("outfit".into(), "coat".into())]),
                )]),
                ..Default::default()
            },
        };
        definition.validate().unwrap();
        let selected = definition
            .select_attributes(
                &definition.defaults,
                &Attributes::from([("variant".into(), "winter".into())]),
            )
            .unwrap();
        assert_eq!(
            definition
                .visible_layers(&selected)
                .map(|layer| layer.id.as_str())
                .collect::<Vec<_>>(),
            ["body", "winter_face_neutral", "outfit_coat"]
        );
        let changed = definition
            .select_attributes(
                &selected,
                &Attributes::from([("face".into(), "happy".into())]),
            )
            .unwrap();
        assert_eq!(changed["variant"], "winter");
        assert_eq!(changed["outfit"], "coat");
        assert!(definition
            .visible_layers(&changed)
            .any(|layer| layer.id == "face_happy"));
        assert!(discover_layers("missing", &paths).is_err());
        assert!(discover_layers("iris", &["../iris__body.png".into()]).is_err());
        assert!(discover_layers("iris", &["iris____body.png".into()]).is_err());
        assert!(discover_layers(
            "iris",
            &["a/iris__body.png".into(), "b/iris__body.webp".into()]
        )
        .is_err());
    }
    #[test]
    fn presets_explicit_patches_and_ordered_rules_have_bounded_deterministic_precedence() {
        let definition=Composition::parse(serde_json::json!({"size":[600,1000],"defaults":{"face":"neutral","outfit":"shirt"},"layers":[
            {"id":"shirt","image":"shirt.png","group":"outfit","attribute":"shirt"},
            {"id":"coat","image":"coat.png","group":"outfit","attribute":"coat"},
            {"id":"happy","image":"happy.png","group":"face","attribute":"happy"},
            {"id":"neutral","image":"neutral.png","group":"face","attribute":"neutral"}
        ],"options":{"variants":{"evening":{"outfit":"coat","face":"happy"}},"rules":{"01_cold":{"when":{"outfit":"coat"},"set":{"face":"neutral"}},"02_evening":{"when":{"variant":"evening"},"set":{"face":"happy"}}}}})).unwrap();
        let selected = definition
            .select_attributes(
                &definition.defaults,
                &Attributes::from([
                    ("variant".into(), "evening".into()),
                    ("outfit".into(), "shirt".into()),
                ]),
            )
            .unwrap();
        assert_eq!(
            selected["outfit"], "shirt",
            "explicit patch wins over variant preset"
        );
        assert_eq!(selected["face"], "happy");
        let bad = serde_json::json!({"size":[1,1],"layers":[{"id":"body","image":"body.png"}],"options":{"variants":{"bad":{"face":"unknown"}}}});
        assert!(Composition::parse(bad).is_err());
    }
    #[test]
    fn order_groups_partial_changes_and_conditions() {
        let definition=Composition::parse(serde_json::json!({"size":[600,1000],"defaults":{"outfit":"shirt","face":"neutral"},"layers":[
            {"id":"body","image":"body.png"},
            {"id":"shirt","image":"shirt.png","group":"outfit","attribute":"shirt"},
            {"id":"coat","image":"coat.png","group":"outfit","attribute":"coat"},
            {"id":"neutral","image":"neutral.png","group":"face","attribute":"neutral"},
            {"id":"happy","image":"happy.png","group":"face","attribute":"happy"},
            {"id":"badge","image":"badge.png","when":{"outfit":"coat"},"unless":{"face":"neutral"}}
        ]})).unwrap();
        let mut attributes = definition.defaults.clone();
        attributes.insert("outfit".into(), "coat".into());
        assert_eq!(
            definition
                .visible_layers(&attributes)
                .map(|layer| layer.id.as_str())
                .collect::<Vec<_>>(),
            vec!["body", "coat", "neutral"]
        );
        attributes.insert("face".into(), "happy".into());
        assert_eq!(
            definition
                .visible_layers(&attributes)
                .map(|layer| layer.id.as_str())
                .collect::<Vec<_>>(),
            vec!["body", "coat", "happy", "badge"]
        );
        definition.validate_attributes(&attributes).unwrap();
        attributes.insert("face".into(), "typo".into());
        assert!(definition.validate_attributes(&attributes).is_err());
        attributes.insert("face".into(), "".into());
        definition.validate_attributes(&attributes).unwrap();
    }
    #[test]
    fn invalid_assets_names_geometry_and_schema_are_rejected() {
        for value in [
            serde_json::json!({"size":[1,1],"layers":[{"id":"body","image":"../secret.png"}]}),
            serde_json::json!({"size":[1,1],"layers":[{"id":"body","image":"body.png","group":"face"}]}),
            serde_json::json!({"size":[1,1],"defaults":{"typo":"yes"},"layers":[{"id":"body","image":"body.png"}]}),
            serde_json::json!({"size":[1,1],"layers":[{"id":"body","image":"body.png","opacity":2}]}),
            serde_json::json!({"size":[0,1],"layers":[{"id":"body","image":"body.png"}]}),
            serde_json::json!({"size":[1,1],"layers":[{"id":"body","image":"body.png","visble":true}]}),
        ] {
            assert!(Composition::parse(value).is_err());
        }
    }
}
