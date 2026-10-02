//! Saved composition definitions and independently selected attribute groups.
use crate::eval::EvalError;
use rvn_parser::Value;
use rvn_ui::composition::{Attributes, Composition, ImageLayer};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CharacterComposition {
    pub definition: Composition,
    pub attributes: Attributes,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LayeredState {
    pub characters: BTreeMap<String, CharacterComposition>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct LayeredView {
    pub character: String,
    pub size: [f32; 2],
    pub layers: Vec<ImageLayer>,
    pub order: Vec<usize>,
    pub resources: Vec<String>,
    pub position: rvn_parser::Position,
}
impl LayeredState {
    pub fn validate(&self) -> Result<(), String> {
        if self.characters.len() > 64 {
            return Err("Composition limit: 64 characters".into());
        }
        let mut resources = std::collections::BTreeSet::new();
        for (id, character) in &self.characters {
            if !rvn_ui::composition::valid_name(id) {
                return Err("Invalid composed character identity".into());
            }
            character.definition.validate()?;
            character
                .definition
                .validate_attributes(&character.attributes)?;
            resources.extend(character.definition.image_paths());
        }
        if resources.len() > 4096 {
            return Err("Composition limit: 4096 simultaneous image resources".into());
        }
        Ok(())
    }
    pub fn compose(&mut self, id: String, definition: Composition) -> Result<(), String> {
        let mut next = self.clone();
        let attributes = definition.select_attributes(&Attributes::new(), &definition.defaults)?;
        next.characters.insert(
            id,
            CharacterComposition {
                attributes,
                definition,
            },
        );
        next.validate()?;
        *self = next;
        Ok(())
    }
    pub fn set_attributes(&mut self, id: &str, patch: &Attributes) -> Result<(), String> {
        let character = self
            .characters
            .get_mut(id)
            .ok_or_else(|| format!("Character '{id}' has no composition"))?;
        character.attributes = character
            .definition
            .select_attributes(&character.attributes, patch)?;
        Ok(())
    }
    pub fn views(
        &self,
        sprites: &std::collections::HashMap<String, crate::types::SpriteState>,
    ) -> Result<Vec<LayeredView>, String> {
        self.validate()?;
        Ok(self
            .characters
            .iter()
            .filter_map(|(id, character)| {
                let sprite = sprites.get(id).filter(|sprite| sprite.visible)?;
                Some(LayeredView {
                    character: id.clone(),
                    size: character.definition.size,
                    position: sprite.position.clone(),
                    layers: character
                        .definition
                        .visible_layers(&character.attributes)
                        .cloned()
                        .collect(),
                    order: character
                        .definition
                        .visible_layer_indices(&character.attributes),
                    resources: character.definition.image_paths().into_iter().collect(),
                })
            })
            .collect())
    }
    pub fn layer_visible(&self, id: &str, layer: &str) -> bool {
        self.characters.get(id).is_some_and(|character| {
            character
                .definition
                .visible_layers(&character.attributes)
                .any(|item| item.id == layer)
        })
    }
}
/// Resolve an optional user function once per composition/attribute update.
/// Functions receive the proposed attribute dictionary and return a patch.
/// The computation engine already limits loops and call depth; RNG changes
/// commit only after the returned patch is structurally and semantically valid.
pub fn resolve_attributes(
    definition: &Composition,
    previous: &Attributes,
    patch: &Attributes,
    library: &crate::eval::FunctionLibrary,
    globals: &std::collections::HashMap<String, Value>,
    random: &mut crate::random::RandomState,
) -> Result<Attributes, EvalError> {
    let invalid = |message: String| EvalError::InvalidFunction(message);
    let mut result = definition
        .select_attributes(previous, patch)
        .map_err(invalid)?;
    if let Some(name) = &definition.options.selector {
        let mut locals = globals.clone();
        locals.insert(
            "__rvn_attributes".into(),
            Value::Dict(
                result
                    .iter()
                    .map(|(key, value)| (key.clone(), Value::Str(value.clone())))
                    .collect(),
            ),
        );
        let mut next_random = *random;
        let value = library.eval_with_random(
            &rvn_parser::Expr::Call {
                name: name.clone(),
                args: vec![rvn_parser::Expr::Var("__rvn_attributes".into())],
            },
            &locals,
            &mut next_random,
        )?;
        let Value::Dict(values) = value else {
            return Err(invalid(format!(
                "Attribute selector '{name}' must return a dictionary patch"
            )));
        };
        let selected: Attributes = values
            .into_iter()
            .map(|(key, value)| match value {
                Value::Str(value) => Ok((key, value)),
                _ => Err(invalid(format!(
                    "Attribute selector '{name}' must return string attribute values"
                ))),
            })
            .collect::<Result<_, _>>()?;
        if selected.contains_key("variant") {
            return Err(invalid("Attribute selectors cannot recursively select variants; select the variant in character.attributes".into()));
        }
        definition.validate_attributes(&selected).map_err(invalid)?;
        result.extend(selected);
        *random = next_random;
    }
    Ok(result)
}
pub fn construct(name: &str, args: &[Value]) -> Result<Value, EvalError> {
    let invalid = |message: &str| EvalError::InvalidFunction(message.into());
    let value=match (name,args) {
        ("layered_image",[size,defaults,layers])=>Value::Dict(BTreeMap::from([("size".into(),size.clone()),("defaults".into(),defaults.clone()),("layers".into(),layers.clone())])),
        ("layered_image",[size,defaults,layers,options])=>Value::Dict(BTreeMap::from([("size".into(),size.clone()),("defaults".into(),defaults.clone()),("layers".into(),layers.clone()),("options".into(),options.clone())])),
        ("image_layers",[Value::Str(prefix),Value::List(images)])=> {
            let paths=images.iter().map(|value|match value {Value::Str(path)=>Ok(path.clone()),_=>Err(invalid("image_layers needs a list of project-relative image paths"))}).collect::<Result<Vec<_>,_>>()?;
            let layers=rvn_ui::composition::discover_layers(prefix,&paths).map_err(|message|invalid(&message))?;
            return Ok(Value::List(layers.into_iter().map(|layer| {
                let mut values=BTreeMap::from([("id".into(),Value::Str(layer.id)),("image".into(),Value::Str(layer.image))]);
                if let Some(group)=layer.group {values.insert("group".into(),Value::Str(group));}
                if let Some(attribute)=layer.attribute {values.insert("attribute".into(),Value::Str(attribute));}
                if let Some(variant)=layer.variant {values.insert("variant".into(),Value::Str(variant));}
                Value::Dict(values)
            }).collect()));
        },
        ("image_layer",[Value::Str(id),Value::Str(image),Value::Dict(properties)])=>{
            if properties.contains_key("id")||properties.contains_key("image") {return Err(invalid("image_layer: id and image are explicit arguments, not properties"));}
            let mut properties=properties.clone();properties.insert("id".into(),Value::Str(id.clone()));properties.insert("image".into(),Value::Str(image.clone()));Value::Dict(properties)
        },
        _=>return Err(invalid("layered_image(size, defaults, layers[, options]), image_layer(id, image, properties), or image_layers(prefix, images) expected")),
    };
    let json = crate::ui::value_to_json(&value)?;
    if name == "layered_image" {
        Composition::parse(json).map_err(|message| invalid(&message))?;
    } else {
        ImageLayer::parse(json).map_err(|message| invalid(&message))?;
    }
    Ok(value)
}
