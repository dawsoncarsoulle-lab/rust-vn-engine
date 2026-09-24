use crate::{
    node_definition, AssetKind, EdgeId, GraphDiagnostic, GraphId, GraphKind, NodeId, NodeKind,
    PinCardinality, PinDirection, PinId, PropertyValue, ValueType, VariableDefinition,
    GRAPH_SCHEMA_VERSION,
};

fn asset_node_kind(kind: AssetKind) -> NodeKind {
    match kind {
        AssetKind::Background => NodeKind::SceneAsset,
        AssetKind::Sprite => NodeKind::SpriteAsset,
        AssetKind::Music => NodeKind::MusicAsset,
        AssetKind::SoundEffect => NodeKind::SoundEffectAsset,
        AssetKind::Voice => NodeKind::VoiceAsset,
        AssetKind::Cinematic => NodeKind::CinematicAsset,
        AssetKind::HoverImage => NodeKind::HoverImageAsset,
        AssetKind::Script => NodeKind::ScriptAsset,
    }
}
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: NodeId,
    pub kind: NodeKind,
    pub position: [f64; 2],
    pub title_override: Option<String>,
    pub properties: BTreeMap<String, PropertyValue>,
    pub pins: Vec<PinId>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphPin {
    pub id: PinId,
    pub node: NodeId,
    pub key: String,
    pub label: String,
    pub direction: PinDirection,
    pub value_type: ValueType,
    pub cardinality: PinCardinality,
    pub default_value: Option<PropertyValue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphEdge {
    pub id: EdgeId,
    pub output: PinId,
    pub input: PinId,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphDocument {
    pub schema_version: u32,
    pub graph_id: GraphId,
    pub kind: GraphKind,
    pub nodes: BTreeMap<NodeId, GraphNode>,
    pub pins: BTreeMap<PinId, GraphPin>,
    pub edges: BTreeMap<EdgeId, GraphEdge>,
    #[serde(default)]
    pub variables: BTreeMap<String, VariableDefinition>,
    /// Characters declared through the editor: identifier → display name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub characters: BTreeMap<String, String>,
    next_node_id: u64,
    next_pin_id: u64,
    next_edge_id: u64,
}

impl GraphDocument {
    pub fn new(graph_id: GraphId, kind: GraphKind) -> Self {
        Self {
            schema_version: GRAPH_SCHEMA_VERSION,
            graph_id,
            kind,
            nodes: BTreeMap::new(),
            pins: BTreeMap::new(),
            edges: BTreeMap::new(),
            variables: BTreeMap::new(),
            characters: BTreeMap::new(),
            next_node_id: 1,
            next_pin_id: 1,
            next_edge_id: 1,
        }
    }

    pub fn add_node(&mut self, kind: NodeKind, position: [f64; 2]) -> NodeId {
        let id = NodeId::new(self.next_node_id);
        self.next_node_id += 1;
        self.nodes.insert(
            id,
            GraphNode {
                id,
                kind,
                position,
                title_override: None,
                properties: BTreeMap::new(),
                pins: Vec::new(),
            },
        );
        id
    }

    pub fn add_variable(
        &mut self,
        name: impl Into<String>,
        value_type: ValueType,
        default_value: PropertyValue,
    ) -> Result<(), GraphEditError> {
        let name = name.into();
        if name.trim().is_empty() || self.variables.contains_key(&name) {
            return Err(GraphEditError::InvalidVariableName(name));
        }
        self.variables.insert(
            name.clone(),
            VariableDefinition {
                name,
                value_type,
                default_value,
            },
        );
        Ok(())
    }

    pub fn rename_variable(
        &mut self,
        old: &str,
        new: impl Into<String>,
    ) -> Result<(), GraphEditError> {
        let new = new.into();
        if new.trim().is_empty() || (old != new && self.variables.contains_key(&new)) {
            return Err(GraphEditError::InvalidVariableName(new));
        }
        let Some(mut definition) = self.variables.remove(old) else {
            return Err(GraphEditError::InvalidVariableName(old.into()));
        };
        definition.name = new.clone();
        for node in self.nodes.values_mut() {
            if matches!(node.kind, NodeKind::VariableGet | NodeKind::SetVariable)
                && node.properties.get("name") == Some(&PropertyValue::String(old.into()))
            {
                node.properties
                    .insert("name".into(), PropertyValue::String(new.clone()));
            }
        }
        self.variables.insert(new, definition);
        Ok(())
    }

    pub fn add_catalog_node(
        &mut self,
        kind: NodeKind,
        position: [f64; 2],
    ) -> Result<NodeId, GraphEditError> {
        let definition = node_definition(kind);
        let node = self.add_node(kind, position);
        for pin_definition in definition.pins {
            let pin = self.add_pin(
                node,
                pin_definition.key,
                pin_definition.label,
                pin_definition.direction,
                pin_definition.value_type,
                pin_definition.cardinality,
            )?;
            self.pins.get_mut(&pin).unwrap().default_value = pin_definition.default_value;
        }
        // Les propriétés qui configurent la forme d'un nœud (et non une entrée)
        // doivent être présentes dès sa création. L'éditeur peut ainsi proposer
        // un inspecteur typé sans connaître des valeurs implicites propres au codegen.
        let defaults = match kind {
            NodeKind::Label => vec![(
                "label",
                PropertyValue::String(
                    if self
                        .nodes
                        .values()
                        .filter(|n| n.kind == NodeKind::Label)
                        .count()
                        == 1
                    {
                        String::new()
                    } else {
                        format!("label_{}", node)
                    },
                ),
            )],
            NodeKind::MakeColor => vec![("rgb_max", PropertyValue::Int(255))],
            NodeKind::Literal => vec![("value", PropertyValue::String(String::new()))],
            NodeKind::TextValue => vec![("value", PropertyValue::String(String::new()))],
            NodeKind::LabelValue => vec![("label", PropertyValue::String(String::new()))],
            NodeKind::PositionValue => vec![("position", PropertyValue::String("center".into()))],
            NodeKind::CharacterValue => vec![
                ("character", PropertyValue::String(String::new())),
                ("emotion", PropertyValue::String(String::new())),
            ],
            NodeKind::SceneAsset
            | NodeKind::SpriteAsset
            | NodeKind::MusicAsset
            | NodeKind::SoundEffectAsset
            | NodeKind::VoiceAsset
            | NodeKind::CinematicAsset
            | NodeKind::HoverImageAsset
            | NodeKind::ScriptAsset => {
                vec![("path", PropertyValue::String(String::new()))]
            }
            NodeKind::TransitionNone => Vec::new(),
            NodeKind::TransitionFade => vec![("duration_ms", PropertyValue::Int(500))],
            NodeKind::TransitionDissolve => vec![("duration_ms", PropertyValue::Int(300))],
            NodeKind::TransitionSlideLeft
            | NodeKind::TransitionSlideRight
            | NodeKind::TransitionSlideUp
            | NodeKind::TransitionSlideDown
            | NodeKind::TransitionZoomIn
            | NodeKind::TransitionZoomOut
            | NodeKind::TransitionBlur => vec![("duration_ms", PropertyValue::Int(400))],
            NodeKind::TransitionWipe => vec![("duration_ms", PropertyValue::Int(500))],
            NodeKind::VariableGet => {
                vec![("name", PropertyValue::String("variable".into()))]
            }
            NodeKind::VariableReference => {
                vec![("name", PropertyValue::String("variable".into()))]
            }
            NodeKind::BinaryOperator => {
                vec![("operator", PropertyValue::String("+".into()))]
            }
            NodeKind::UnaryOperator => {
                vec![("operator", PropertyValue::String("not".into()))]
            }
            NodeKind::FunctionCall => {
                vec![
                    ("function", PropertyValue::String("min".into())),
                    ("args", PropertyValue::StringList(Vec::new())),
                ]
            }
            NodeKind::ListLiteral => {
                vec![("items", PropertyValue::StringList(Vec::new()))]
            }
            NodeKind::Choice => {
                vec![("options", PropertyValue::StringList(Vec::new()))]
            }
            NodeKind::SpriteAnimate => {
                vec![("params", PropertyValue::StringList(Vec::new()))]
            }
            NodeKind::SpriteEffect => ["flip_x", "flip_y", "scale", "rotation", "tint"]
                .into_iter()
                .map(|key| {
                    (
                        match key {
                            "flip_x" => "apply_flip_x",
                            "flip_y" => "apply_flip_y",
                            "scale" => "apply_scale",
                            "rotation" => "apply_rotation",
                            _ => "apply_tint",
                        },
                        PropertyValue::Bool(false),
                    )
                })
                .collect(),
            NodeKind::Imagemap => {
                vec![("hotspots", PropertyValue::StringList(Vec::new()))]
            }
            _ => Vec::new(),
        };
        for (key, value) in defaults {
            self.set_property(node, key, value)?;
        }
        Ok(node)
    }

    /// Typed, connectable arguments. Legacy string arrays are kept as strings
    /// when explicitly upgraded; numeric-looking old text is never reinterpreted.
    pub fn resize_value_inputs(
        &mut self,
        node: NodeId,
        count: usize,
    ) -> Result<(), GraphEditError> {
        let owner = self
            .nodes
            .get(&node)
            .ok_or(GraphEditError::NodeNotFound(node))?;
        if !matches!(owner.kind, NodeKind::FunctionCall | NodeKind::ListLiteral) {
            return Err(GraphEditError::WrongNodeKind {
                node,
                expected: NodeKind::FunctionCall,
                found: owner.kind,
            });
        }
        let legacy_key = if owner.kind == NodeKind::FunctionCall {
            "args"
        } else {
            "items"
        };
        let legacy = match owner.properties.get(legacy_key) {
            Some(PropertyValue::StringList(values)) => values.clone(),
            _ => Vec::new(),
        };
        let count = count.min(128);
        for index in 0..count {
            let key = format!("item_{index}");
            if self.pin_by_key(node, &key).is_none() {
                let pin = self.add_pin(
                    node,
                    key,
                    format!("[{index}]"),
                    PinDirection::Input,
                    ValueType::Any,
                    PinCardinality::One,
                )?;
                self.pins.get_mut(&pin).unwrap().default_value = Some(
                    legacy
                        .get(index)
                        .map(|v| PropertyValue::String(v.clone()))
                        .unwrap_or(PropertyValue::Int(0)),
                );
            }
        }
        let removed: BTreeSet<_> = self.nodes[&node]
            .pins
            .iter()
            .copied()
            .filter(|id| {
                self.pins[id]
                    .key
                    .strip_prefix("item_")
                    .and_then(|s| s.parse::<usize>().ok())
                    .is_some_and(|index| index >= count)
            })
            .collect();
        self.nodes
            .get_mut(&node)
            .unwrap()
            .pins
            .retain(|id| !removed.contains(id));
        self.edges
            .retain(|_, e| !removed.contains(&e.input) && !removed.contains(&e.output));
        self.pins.retain(|id, _| !removed.contains(id));
        self.set_property(node, "input_count", PropertyValue::Int(count as i64))?;
        Ok(())
    }

    pub fn add_pin(
        &mut self,
        node: NodeId,
        key: impl Into<String>,
        label: impl Into<String>,
        direction: PinDirection,
        value_type: ValueType,
        cardinality: PinCardinality,
    ) -> Result<PinId, GraphEditError> {
        let Some(owner) = self.nodes.get_mut(&node) else {
            return Err(GraphEditError::NodeNotFound(node));
        };
        let key = key.into();
        if owner
            .pins
            .iter()
            .filter_map(|pin| self.pins.get(pin))
            .any(|pin| pin.key == key)
        {
            return Err(GraphEditError::DuplicatePinKey { node, key });
        }
        let id = PinId::new(self.next_pin_id);
        self.next_pin_id += 1;
        owner.pins.push(id);
        self.pins.insert(
            id,
            GraphPin {
                id,
                node,
                key,
                label: label.into(),
                direction,
                value_type,
                cardinality,
                default_value: None,
            },
        );
        Ok(id)
    }

    /// Donne aux deux extrémités d'un nœud de reroute le type exact du câble.
    /// Le type est sérialisé avec le graphe afin que le nœud reste fidèle après
    /// rechargement, y compris pour le flux d'exécution.
    pub fn set_reroute_type(
        &mut self,
        node: NodeId,
        value_type: ValueType,
    ) -> Result<(), GraphEditError> {
        let owner = self
            .nodes
            .get(&node)
            .ok_or(GraphEditError::NodeNotFound(node))?;
        if owner.kind != NodeKind::Reroute {
            return Err(GraphEditError::WrongNodeKind {
                node,
                expected: NodeKind::Reroute,
                found: owner.kind,
            });
        }
        let pins = owner.pins.clone();
        for pin in pins {
            self.pins.get_mut(&pin).unwrap().value_type = value_type.clone();
        }
        Ok(())
    }

    pub fn pin_by_key(&self, node: NodeId, key: &str) -> Option<&GraphPin> {
        self.nodes.get(&node)?.pins.iter().find_map(|pin_id| {
            let pin = self.pins.get(pin_id)?;
            (pin.key == key).then_some(pin)
        })
    }

    pub fn set_pin_default(
        &mut self,
        node: NodeId,
        key: &str,
        value: PropertyValue,
    ) -> Result<(), GraphEditError> {
        let pin = self
            .pin_by_key(node, key)
            .map(|pin| pin.id)
            .ok_or_else(|| GraphEditError::PinKeyNotFound {
                node,
                key: key.to_owned(),
            })?;
        self.pins.get_mut(&pin).unwrap().default_value = Some(value);
        Ok(())
    }

    pub fn set_property(
        &mut self,
        node: NodeId,
        key: impl Into<String>,
        value: PropertyValue,
    ) -> Result<(), GraphEditError> {
        let key = key.into();
        let literal_type = property_value_type(&value);
        let owner = self
            .nodes
            .get_mut(&node)
            .ok_or(GraphEditError::NodeNotFound(node))?;
        let is_literal_value = owner.kind == NodeKind::Literal && key == "value";
        owner.properties.insert(key, value);
        if is_literal_value {
            let output = owner.pins.iter().copied().find(|pin_id| {
                self.pins
                    .get(pin_id)
                    .is_some_and(|pin| pin.direction == PinDirection::Output && pin.key == "value")
            });
            if let Some(output) = output {
                self.pins.get_mut(&output).unwrap().value_type = literal_type;
            }
        }
        Ok(())
    }

    pub fn add_choice_option(
        &mut self,
        node: NodeId,
        label: impl Into<String>,
    ) -> Result<ChoiceOptionPins, GraphEditError> {
        let owner = self
            .nodes
            .get(&node)
            .ok_or(GraphEditError::NodeNotFound(node))?;
        if owner.kind != NodeKind::Choice {
            return Err(GraphEditError::WrongNodeKind {
                node,
                expected: NodeKind::Choice,
                found: owner.kind,
            });
        }
        let label = label.into();
        let index = match owner.properties.get("options") {
            Some(PropertyValue::StringList(options)) => options.len(),
            _ => 0,
        };
        let owner = self.nodes.get_mut(&node).unwrap();
        match owner.properties.entry("options".into()) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(PropertyValue::StringList(vec![label.clone()]));
            }
            std::collections::btree_map::Entry::Occupied(mut entry) => {
                let PropertyValue::StringList(options) = entry.get_mut() else {
                    return Err(GraphEditError::InvalidPropertyType {
                        node,
                        key: "options".into(),
                    });
                };
                options.push(label.clone());
            }
        }
        let branch = self.add_pin(
            node,
            format!("option_{index}"),
            label,
            PinDirection::Output,
            ValueType::Execution,
            PinCardinality::Many,
        )?;
        let condition = self.add_pin(
            node,
            format!("option_{index}_condition"),
            "Condition",
            PinDirection::Input,
            ValueType::Bool,
            PinCardinality::One,
        )?;
        self.set_property(
            node,
            format!("option_{index}_condition"),
            PropertyValue::String(String::new()),
        )?;
        Ok(ChoiceOptionPins {
            index,
            branch,
            condition,
        })
    }

    pub fn add_operator_operand(&mut self, node: NodeId) -> Result<PinId, GraphEditError> {
        let owner = self
            .nodes
            .get(&node)
            .ok_or(GraphEditError::NodeNotFound(node))?;
        let value_type = match owner.kind {
            NodeKind::LogicAnd | NodeKind::LogicOr => ValueType::Bool,
            NodeKind::MathAdd | NodeKind::MathMultiply => ValueType::Any,
            _ => return Err(GraphEditError::UnsupportedDynamicOperand(node)),
        };
        let input_count = owner
            .pins
            .iter()
            .filter(|pin| self.pins[pin].direction == PinDirection::Input)
            .count();
        let pin = self.add_pin(
            node,
            format!("operand_{input_count}"),
            format!("Entrée {}", input_count + 1),
            PinDirection::Input,
            value_type.clone(),
            PinCardinality::One,
        )?;
        if value_type == ValueType::Bool {
            self.pins.get_mut(&pin).unwrap().default_value = Some(PropertyValue::Bool(false));
        }
        Ok(pin)
    }

    /// Met à jour le patron d'un nœud Format Text et synchronise ses broches
    /// nommées comme le nœud Blueprint d'Unreal. Les broches dont le nom reste
    /// présent conservent leur identifiant et donc leur câble.
    pub fn set_format_text_pattern(
        &mut self,
        node: NodeId,
        pattern: impl Into<String>,
    ) -> Result<(), GraphEditError> {
        let pattern = pattern.into();
        let owner = self
            .nodes
            .get(&node)
            .ok_or(GraphEditError::NodeNotFound(node))?;
        if owner.kind != NodeKind::FormatText {
            return Err(GraphEditError::WrongNodeKind {
                node,
                expected: NodeKind::FormatText,
                found: owner.kind,
            });
        }

        let arguments = format_text_arguments(&pattern);
        let wanted: BTreeSet<_> = arguments
            .iter()
            .map(|name| format!("argument_{name}"))
            .collect();
        let removed: BTreeSet<_> = owner
            .pins
            .iter()
            .filter_map(|pin| self.pins.get(pin))
            .filter(|pin| pin.key.starts_with("argument_") && !wanted.contains(&pin.key))
            .map(|pin| pin.id)
            .collect();

        self.nodes
            .get_mut(&node)
            .unwrap()
            .pins
            .retain(|pin| !removed.contains(pin));
        self.pins.retain(|pin, _| !removed.contains(pin));
        self.edges
            .retain(|_, edge| !removed.contains(&edge.output) && !removed.contains(&edge.input));

        for argument in &arguments {
            let key = format!("argument_{argument}");
            if self.pin_by_key(node, &key).is_none() {
                self.add_pin(
                    node,
                    key,
                    argument.clone(),
                    PinDirection::Input,
                    ValueType::Any,
                    PinCardinality::One,
                )?;
            }
        }

        // Unreal place Result après les arguments dans la liste visuelle.
        // On réordonne sans recréer la pin de sortie.
        let format_pin = self.pin_by_key(node, "format").map(|pin| pin.id);
        let result_pin = self.pin_by_key(node, "result").map(|pin| pin.id);
        let argument_pins: Vec<_> = arguments
            .iter()
            .filter_map(|argument| {
                self.pin_by_key(node, &format!("argument_{argument}"))
                    .map(|pin| pin.id)
            })
            .collect();
        let pins = &mut self.nodes.get_mut(&node).unwrap().pins;
        pins.clear();
        if let Some(pin) = format_pin {
            pins.push(pin);
        }
        pins.extend(argument_pins);
        if let Some(pin) = result_pin {
            pins.push(pin);
        }
        self.set_pin_default(node, "format", PropertyValue::String(pattern))?;
        Ok(())
    }

    /// Remplace la liste d'options tout en conservant les identifiants des pins
    /// existants. Les câbles des options qui restent en place ne sautent donc pas
    /// pendant une simple modification de libellé dans l'inspecteur.
    pub fn set_choice_options(
        &mut self,
        node: NodeId,
        labels: Vec<String>,
    ) -> Result<(), GraphEditError> {
        let owner = self
            .nodes
            .get(&node)
            .ok_or(GraphEditError::NodeNotFound(node))?;
        if owner.kind != NodeKind::Choice {
            return Err(GraphEditError::WrongNodeKind {
                node,
                expected: NodeKind::Choice,
                found: owner.kind,
            });
        }
        let previous_len = match owner.properties.get("options") {
            Some(PropertyValue::StringList(options)) => options.len(),
            _ => 0,
        };
        let shared_len = previous_len.min(labels.len());
        for (index, label) in labels.iter().take(shared_len).enumerate() {
            if let Some(pin) = self
                .pin_by_key(node, &format!("option_{index}"))
                .map(|pin| pin.id)
            {
                self.pins.get_mut(&pin).unwrap().label = label.clone();
            }
        }
        for label in labels.iter().skip(previous_len) {
            self.add_choice_option(node, label.clone())?;
        }
        if labels.len() < previous_len {
            for index in labels.len()..previous_len {
                self.nodes
                    .get_mut(&node)
                    .unwrap()
                    .properties
                    .remove(&format!("option_{index}_condition"));
            }
            let removed: BTreeSet<_> = (labels.len()..previous_len)
                .flat_map(|index| {
                    [
                        format!("option_{index}"),
                        format!("option_{index}_condition"),
                    ]
                })
                .filter_map(|key| self.pin_by_key(node, &key).map(|pin| pin.id))
                .collect();
            self.nodes
                .get_mut(&node)
                .unwrap()
                .pins
                .retain(|pin| !removed.contains(pin));
            self.pins.retain(|pin, _| !removed.contains(pin));
            self.edges.retain(|_, edge| {
                !removed.contains(&edge.output) && !removed.contains(&edge.input)
            });
        }
        self.nodes
            .get_mut(&node)
            .unwrap()
            .properties
            .insert("options".into(), PropertyValue::StringList(labels));
        Ok(())
    }

    pub fn set_imagemap_hotspots(
        &mut self,
        node: NodeId,
        hotspots: Vec<String>,
    ) -> Result<(), GraphEditError> {
        let owner = self
            .nodes
            .get(&node)
            .ok_or(GraphEditError::NodeNotFound(node))?;
        if owner.kind != NodeKind::Imagemap {
            return Err(GraphEditError::WrongNodeKind {
                node,
                expected: NodeKind::Imagemap,
                found: owner.kind,
            });
        }
        let previous_len = match owner.properties.get("hotspots") {
            Some(PropertyValue::StringList(values)) => values.len(),
            _ => 0,
        };
        for (index, hotspot) in hotspots.iter().enumerate().take(previous_len) {
            if let Some(pin) = self
                .pin_by_key(node, &format!("hotspot_{index}"))
                .map(|pin| pin.id)
            {
                self.pins.get_mut(&pin).unwrap().label = hotspot_name(hotspot).to_owned();
            }
        }
        for (index, hotspot) in hotspots.iter().enumerate().skip(previous_len) {
            self.add_pin(
                node,
                format!("hotspot_{index}"),
                hotspot_name(hotspot),
                PinDirection::Output,
                ValueType::Execution,
                PinCardinality::Many,
            )?;
        }
        if hotspots.len() < previous_len {
            let removed: BTreeSet<_> = (hotspots.len()..previous_len)
                .filter_map(|index| {
                    self.pin_by_key(node, &format!("hotspot_{index}"))
                        .map(|pin| pin.id)
                })
                .collect();
            self.nodes
                .get_mut(&node)
                .unwrap()
                .pins
                .retain(|pin| !removed.contains(pin));
            self.pins.retain(|pin, _| !removed.contains(pin));
            self.edges.retain(|_, edge| {
                !removed.contains(&edge.output) && !removed.contains(&edge.input)
            });
        }
        self.nodes
            .get_mut(&node)
            .unwrap()
            .properties
            .insert("hotspots".into(), PropertyValue::StringList(hotspots));
        Ok(())
    }

    /// Transforme les valeurs implicites significatives en nœuds compacts
    /// connectés. L’éditeur peut ainsi montrer la véritable source de chaque
    /// donnée au lieu de cacher une valeur dans le pin consommateur.
    pub fn materialize_visible_defaults(&mut self) -> Result<usize, GraphEditError> {
        let mut created = self.normalize_legacy_operators();
        let candidates: Vec<_> = self
            .pins
            .values()
            .filter(|pin| pin.direction == PinDirection::Input)
            .filter(|pin| !pin.value_type.is_execution())
            .filter(|pin| {
                let node = &self.nodes[&pin.node];
                node.kind != NodeKind::MakeColor
                    && !(matches!(node.kind, NodeKind::FunctionCall | NodeKind::ListLiteral)
                        && node.properties.contains_key("input_count"))
                    && !(node.kind == NodeKind::SpriteEffect
                        && node.properties.get(&format!("apply_{}", pin.key))
                            == Some(&PropertyValue::Bool(false)))
            })
            .filter(|pin| {
                !matches!(
                    self.nodes[&pin.node].kind,
                    NodeKind::LogicAnd | NodeKind::LogicOr | NodeKind::LogicNot
                )
            })
            // Le nom affiché par un SET est son identité, pas une valeur de
            // graphe. UE l'intègre au nœud SET et ne crée jamais une fausse
            // constante texte reliée à une broche `name`.
            .filter(|pin| self.nodes[&pin.node].kind != NodeKind::SetVariable)
            // Le patron est volontairement édité dans le nœud Format Text,
            // comme dans UE5. Il ne doit pas être extrait dans une constante.
            .filter(|pin| {
                !(self.nodes[&pin.node].kind == NodeKind::FormatText && pin.key == "format")
            })
            .filter(|pin| !self.edges.values().any(|edge| edge.input == pin.id))
            .filter_map(|pin| {
                let value = pin.default_value.clone()?;
                let visible = match &value {
                    PropertyValue::String(value) => !value.trim().is_empty(),
                    PropertyValue::Bool(_) | PropertyValue::Int(_) | PropertyValue::Float(_) => {
                        true
                    }
                    PropertyValue::StringList(values) => !values.is_empty(),
                };
                visible.then_some((
                    pin.id,
                    pin.node,
                    pin.key.clone(),
                    pin.value_type.clone(),
                    value,
                ))
            })
            .collect();
        let mut owner_slots = BTreeMap::<NodeId, usize>::new();
        for (input, owner, key, value_type, value) in candidates {
            if self.nodes[&owner].kind == NodeKind::Choice
                && key.ends_with("_condition")
                && value == PropertyValue::Bool(false)
            {
                // Une condition absente signifie « option toujours visible ».
                // Les anciens graphes sérialisaient `false`, ce qui était à la
                // fois trompeur dans l’inspecteur et différent sémantiquement.
                self.pins.get_mut(&input).unwrap().default_value = None;
                created += 1;
                continue;
            }
            let owner_position = self.nodes[&owner].position;
            let slot = owner_slots.entry(owner).or_default();
            let offset = 112.0 + *slot as f64 * 96.0;
            *slot += 1;
            let kind = match value_type {
                ValueType::InterpolatedText => NodeKind::TextValue,
                ValueType::Character => NodeKind::CharacterValue,
                ValueType::Label => NodeKind::LabelValue,
                ValueType::Position => NodeKind::PositionValue,
                ValueType::Asset(kind) => asset_node_kind(kind),
                ValueType::Transition => transition_kind(&value),
                _ => NodeKind::Literal,
            };
            let source_position = [owner_position[0] - 235.0, owner_position[1] + offset];
            let source = self.add_catalog_node(kind, source_position)?;
            self.place_without_overlap(source);
            match kind {
                NodeKind::TextValue => self.set_property(source, "value", value.clone())?,
                NodeKind::LabelValue => self.set_property(source, "label", value.clone())?,
                NodeKind::PositionValue => self.set_property(source, "position", value.clone())?,
                NodeKind::CharacterValue => {
                    self.set_property(source, "character", value.clone())?
                }
                NodeKind::SceneAsset
                | NodeKind::SpriteAsset
                | NodeKind::MusicAsset
                | NodeKind::SoundEffectAsset
                | NodeKind::VoiceAsset
                | NodeKind::CinematicAsset
                | NodeKind::HoverImageAsset
                | NodeKind::ScriptAsset => self.set_property(source, "path", value.clone())?,
                NodeKind::TransitionNone
                | NodeKind::TransitionFade
                | NodeKind::TransitionDissolve
                | NodeKind::TransitionSlideLeft
                | NodeKind::TransitionSlideRight
                | NodeKind::TransitionSlideUp
                | NodeKind::TransitionSlideDown
                | NodeKind::TransitionZoomIn
                | NodeKind::TransitionZoomOut
                | NodeKind::TransitionWipe
                | NodeKind::TransitionBlur => {
                    if let Some(duration) = transition_duration(&value) {
                        self.set_property(source, "duration_ms", PropertyValue::Int(duration))?;
                    }
                }
                _ => self.set_property(source, "value", value.clone())?,
            }
            let output = self.pin_by_key(source, "value").unwrap().id;
            self.connect(output, input)?;
            self.pins.get_mut(&input).unwrap().default_value = None;
            created += 1;
        }
        created += self.materialize_legacy_sprite_sources()?;
        Ok(created)
    }

    /// Consume an old explicit emotion once; never recreate a disconnected sprite.
    pub fn materialize_legacy_sprite_sources(&mut self) -> Result<usize, GraphEditError> {
        fn source(
            g: &GraphDocument,
            node: NodeId,
            key: &str,
            seen: &mut BTreeSet<NodeId>,
        ) -> Option<NodeId> {
            if !seen.insert(node) {
                return None;
            }
            let pin = g.pin_by_key(node, key)?;
            let edge = g.edges.values().find(|e| e.input == pin.id)?;
            let owner = g.pins.get(&edge.output)?.node;
            if g.nodes[&owner].kind == NodeKind::Reroute {
                source(g, owner, "value", seen)
            } else {
                Some(owner)
            }
        }
        let mut candidates = Vec::new();
        for node in self
            .nodes
            .values()
            .filter(|n| n.kind == NodeKind::SpriteShow)
        {
            let Some(character) = source(self, node.id, "character", &mut BTreeSet::new())
                .filter(|id| self.nodes[id].kind == NodeKind::CharacterValue)
            else {
                continue;
            };
            let Some(PropertyValue::String(id)) =
                self.nodes[&character].properties.get("character")
            else {
                continue;
            };
            let Some(pin) = self.pin_by_key(character, "sprite") else {
                continue;
            };
            if self.edges.values().any(|e| e.input == pin.id) {
                continue;
            }
            let emotion = source(self, node.id, "emotion", &mut BTreeSet::new())
                .and_then(|id| self.nodes[&id].properties.get("value"))
                .or_else(|| {
                    self.pin_by_key(node.id, "emotion")
                        .and_then(|p| p.default_value.as_ref())
                });
            let Some(PropertyValue::String(emotion)) =
                emotion.filter(|v| matches!(v,PropertyValue::String(s) if !s.is_empty()))
            else {
                continue;
            };
            let path = if emotion.contains('/') {
                emotion.clone()
            } else {
                format!("sprites/{id}/{emotion}.png")
            };
            candidates.push((node.id, character, pin.id, path));
        }
        for (show, character, input, path) in &candidates {
            let pos = self.nodes[character].position;
            let image =
                self.add_catalog_node(NodeKind::SpriteAsset, [pos[0] - 280.0, pos[1] + 160.0])?;
            self.set_property(image, "path", PropertyValue::String(path.clone()))?;
            self.place_without_overlap(image);
            let output = self.pin_by_key(image, "value").unwrap().id;
            self.connect(output, *input)?;
            if let Some(pin) = self.pin_by_key(*show, "emotion").map(|p| p.id) {
                self.edges.retain(|_, e| e.input != pin);
                self.pins.get_mut(&pin).unwrap().default_value = None;
            }
            self.nodes
                .get_mut(character)
                .unwrap()
                .properties
                .remove("emotion");
        }
        Ok(candidates.len())
    }

    /// Donne aux anciens littéraux leur type réel puis matérialise les
    /// conversions implicites connues. Les graphes créés avant les littéraux
    /// typés contenaient des sorties `Any`, ce qui masquait notamment les
    /// liaisons Texte → Entier.
    pub fn materialize_implicit_conversions(&mut self) -> Result<usize, GraphEditError> {
        let mut trial = self.clone();
        let mut changed = trial.normalize_literal_pin_types();
        let candidates: Vec<_> = trial
            .edges
            .values()
            .filter_map(|edge| {
                let output = trial.pins.get(&edge.output)?;
                let input = trial.pins.get(&edge.input)?;
                if input.value_type.accepts(&output.value_type) {
                    return None;
                }
                conversion_kind_for(&output.value_type, &input.value_type)
                    .map(|kind| (edge.id, edge.output, edge.input, kind))
            })
            .collect();

        for (edge, output, input, kind) in candidates {
            let output_node = trial.pins[&output].node;
            let input_node = trial.pins[&input].node;
            let output_position = trial.nodes[&output_node].position;
            let input_position = trial.nodes[&input_node].position;
            let position = [
                (output_position[0] + input_position[0]) * 0.5,
                (output_position[1] + input_position[1]) * 0.5,
            ];
            trial.remove_edge(edge);
            let converter = trial.add_catalog_node(kind, position)?;
            let converter_input = trial.pin_by_key(converter, "value").unwrap().id;
            let converter_output = trial.pin_by_key(converter, "result").unwrap().id;
            trial.connect(output, converter_input)?;
            trial.connect(converter_output, input)?;
            changed += 1;
        }

        *self = trial;
        Ok(changed)
    }

    /// Supprime uniquement les anciennes références de variable génériques.
    /// Une référence utilisée comme nom d'un SET est absorbée par celui-ci ;
    /// une référence de lecture encore utile devient un véritable GET, mais
    /// seulement si sa variable a été déclarée dans le panneau Variables.
    /// Les constantes restent des nœuds visibles et explicitement branchés.
    pub fn normalize_legacy_variable_references(&mut self) -> usize {
        let mut changed = 0;
        let references: Vec<_> = self
            .nodes
            .values()
            .filter(|node| node.kind == NodeKind::VariableReference)
            .map(|node| node.id)
            .collect();
        for node in references {
            let name = match self.nodes[&node].properties.get("name") {
                Some(PropertyValue::String(name)) => name.clone(),
                _ => String::new(),
            };
            let output = self.pin_by_key(node, "value").map(|pin| pin.id);
            let set_name_edges: Vec<_> = output
                .into_iter()
                .flat_map(|output| {
                    self.edges
                        .values()
                        .filter(move |edge| edge.output == output)
                })
                .filter_map(|edge| {
                    let input = self.pins.get(&edge.input)?;
                    (input.key == "name"
                        && self.nodes.get(&input.node)?.kind == NodeKind::SetVariable)
                        .then_some((edge.id, input.id, input.node))
                })
                .collect();
            for (edge, input, setter) in &set_name_edges {
                if let Some(setter) = self.nodes.get_mut(setter) {
                    setter
                        .properties
                        .insert("name".into(), PropertyValue::String(name.clone()));
                }
                if let Some(pin) = self.pins.get_mut(input) {
                    pin.default_value = Some(PropertyValue::String(name.clone()));
                }
                self.remove_edge(*edge);
            }

            let has_remaining_edges =
                output.is_some_and(|output| self.edges.values().any(|edge| edge.output == output));
            if has_remaining_edges || set_name_edges.is_empty() {
                if let Some(definition) = self.variables.get(&name).cloned() {
                    self.nodes.get_mut(&node).unwrap().kind = NodeKind::VariableGet;
                    if let Some(output) = output {
                        self.pins.get_mut(&output).unwrap().value_type = definition.value_type;
                    }
                    changed += 1;
                    continue;
                }
            }
            self.remove_node(node);
            changed += 1;
        }
        changed
    }

    /// Une entrée déjà câblée ne doit pas conserver une seconde valeur cachée
    /// susceptible d'apparaître dans l'inspecteur ou de ressurgir plus tard.
    pub fn clear_shadowed_input_defaults(&mut self) -> usize {
        let connected_inputs: BTreeSet<_> = self.edges.values().map(|edge| edge.input).collect();
        let mut changed = 0;
        for input in connected_inputs {
            if let Some(pin) = self.pins.get_mut(&input) {
                if pin.default_value.take().is_some() {
                    changed += 1;
                }
            }
        }
        changed
    }

    fn normalize_literal_pin_types(&mut self) -> usize {
        let literals: Vec<_> = self
            .nodes
            .values()
            .filter(|node| node.kind == NodeKind::Literal)
            .filter_map(|node| {
                let value_type = property_value_type(node.properties.get("value")?);
                let output = node.pins.iter().copied().find(|pin_id| {
                    self.pins.get(pin_id).is_some_and(|pin| {
                        pin.direction == PinDirection::Output && pin.key == "value"
                    })
                })?;
                Some((output, value_type))
            })
            .collect();
        let mut changed = 0;
        for (output, value_type) in literals {
            let pin = self.pins.get_mut(&output).unwrap();
            if pin.value_type != value_type {
                pin.value_type = value_type;
                changed += 1;
            }
        }
        changed
    }

    fn normalize_legacy_operators(&mut self) -> usize {
        let replacements: Vec<_> = self
            .nodes
            .values()
            .filter_map(|node| {
                let PropertyValue::String(operator) = node.properties.get("operator")? else {
                    return None;
                };
                let replacement = match (node.kind, operator.as_str()) {
                    (NodeKind::BinaryOperator, "+") => NodeKind::MathAdd,
                    (NodeKind::BinaryOperator, "-") => NodeKind::MathSubtract,
                    (NodeKind::BinaryOperator, "*") => NodeKind::MathMultiply,
                    (NodeKind::BinaryOperator, "/") => NodeKind::MathDivide,
                    (NodeKind::BinaryOperator, "==") => NodeKind::MathEqual,
                    (NodeKind::BinaryOperator, "!=") => NodeKind::MathNotEqual,
                    (NodeKind::BinaryOperator, "<") => NodeKind::MathLess,
                    (NodeKind::BinaryOperator, "<=") => NodeKind::MathLessEqual,
                    (NodeKind::BinaryOperator, ">") => NodeKind::MathGreater,
                    (NodeKind::BinaryOperator, ">=") => NodeKind::MathGreaterEqual,
                    (NodeKind::BinaryOperator, "and") => NodeKind::LogicAnd,
                    (NodeKind::BinaryOperator, "or") => NodeKind::LogicOr,
                    (NodeKind::UnaryOperator, "not") => NodeKind::LogicNot,
                    (NodeKind::UnaryOperator, "-") => NodeKind::MathNegate,
                    _ => return None,
                };
                Some((node.id, replacement))
            })
            .collect();
        for (node, replacement) in &replacements {
            self.nodes.get_mut(node).unwrap().kind = *replacement;
            let definitions = node_definition(*replacement);
            let pin_ids = self.nodes[node].pins.clone();
            for pin in pin_ids {
                let key = self.pins[&pin].key.clone();
                if let Some(definition) = definitions.pins.iter().find(|item| item.key == key) {
                    let graph_pin = self.pins.get_mut(&pin).unwrap();
                    graph_pin.value_type = definition.value_type.clone();
                    graph_pin.label = definition.label.into();
                    if graph_pin.default_value.is_none() {
                        graph_pin.default_value = definition.default_value.clone();
                    }
                }
            }
            self.nodes
                .get_mut(node)
                .unwrap()
                .properties
                .remove("operator");
        }
        replacements.len()
    }

    fn place_without_overlap(&mut self, source: NodeId) {
        const MARGIN: f64 = 18.0;
        let source_size = self.estimated_node_size(source);
        loop {
            let source_position = self.nodes[&source].position;
            let collision_bottom = self
                .nodes
                .keys()
                .copied()
                .filter(|node| *node != source)
                .filter_map(|node| {
                    let other = &self.nodes[&node];
                    let other_size = self.estimated_node_size(node);
                    rectangles_overlap(
                        source_position,
                        source_size,
                        other.position,
                        other_size,
                        MARGIN,
                    )
                    .then_some(other.position[1] + other_size[1] + MARGIN)
                })
                .max_by(f64::total_cmp);
            let Some(next_y) = collision_bottom else {
                break;
            };
            self.nodes.get_mut(&source).unwrap().position[1] = next_y;
        }
    }

    fn estimated_node_size(&self, node: NodeId) -> [f64; 2] {
        let graph_node = &self.nodes[&node];
        let compact = is_compact_node(graph_node.kind);
        let title = graph_node
            .title_override
            .as_deref()
            .unwrap_or_else(|| node_definition(graph_node.kind).title);
        let mut inputs = 0usize;
        let mut outputs = 0usize;
        for pin in graph_node.pins.iter().map(|pin| &self.pins[pin]) {
            if pin.key == "completed" && matches!(graph_node.kind, NodeKind::If | NodeKind::Choice)
            {
                continue;
            }
            match pin.direction {
                PinDirection::Input => inputs += 1,
                PinDirection::Output => outputs += 1,
            }
        }
        let rows = inputs.max(outputs).max(1) as f64;
        if compact {
            [
                (title.chars().count() as f64 * 8.2 + 70.0).clamp(132.0, 238.0),
                (22.0 + rows * 22.0).max(48.0),
            ]
        } else {
            [
                (title.chars().count() as f64 * 6.6 + 52.0).clamp(176.0, 286.0),
                (52.0 + rows * 23.0).max(76.0),
            ]
        }
    }

    pub fn add_branch_end(
        &mut self,
        owner: NodeId,
        position: [f64; 2],
    ) -> Result<NodeId, GraphEditError> {
        if !self.nodes.contains_key(&owner) {
            return Err(GraphEditError::NodeNotFound(owner));
        }
        let branch_end = self.add_catalog_node(NodeKind::BranchEnd, position)?;
        self.set_property(branch_end, "owner", PropertyValue::Int(owner.get() as i64))?;
        Ok(branch_end)
    }

    pub fn connect(&mut self, output: PinId, input: PinId) -> Result<EdgeId, GraphEditError> {
        let output_pin = self
            .pins
            .get(&output)
            .ok_or(GraphEditError::PinNotFound(output))?;
        let input_pin = self
            .pins
            .get(&input)
            .ok_or(GraphEditError::PinNotFound(input))?;
        if output_pin.direction != PinDirection::Output
            || input_pin.direction != PinDirection::Input
        {
            return Err(GraphEditError::InvalidDirection { output, input });
        }
        if !input_pin.value_type.accepts(&output_pin.value_type)
            || !self.nodes[&input_pin.node]
                .kind
                .accepts_data_source(&output_pin.value_type)
        {
            return Err(GraphEditError::IncompatibleTypes {
                output: output_pin.value_type.clone(),
                input: input_pin.value_type.clone(),
            });
        }
        if self
            .edges
            .values()
            .any(|edge| edge.output == output && edge.input == input)
        {
            return Err(GraphEditError::DuplicateEdge { output, input });
        }
        if input_pin.cardinality == PinCardinality::One
            && self.edges.values().any(|edge| edge.input == input)
        {
            return Err(GraphEditError::InputAlreadyConnected(input));
        }
        if !output_pin.value_type.is_execution()
            && self.data_path_exists(input_pin.node, output_pin.node)
        {
            return Err(GraphEditError::DataCycle {
                from: output_pin.node,
                to: input_pin.node,
            });
        }

        let id = EdgeId::new(self.next_edge_id);
        self.next_edge_id += 1;
        self.edges.insert(id, GraphEdge { id, output, input });
        if !self.pins[&input].value_type.is_execution() {
            self.pins.get_mut(&input).unwrap().default_value = None;
        }
        Ok(id)
    }

    pub fn remove_node(&mut self, node: NodeId) -> Option<GraphNode> {
        let removed = self.nodes.remove(&node)?;
        let removed_pins: BTreeSet<_> = removed.pins.iter().copied().collect();
        self.pins.retain(|id, _| !removed_pins.contains(id));
        self.edges.retain(|_, edge| {
            !removed_pins.contains(&edge.output) && !removed_pins.contains(&edge.input)
        });
        Some(removed)
    }

    pub fn remove_edge(&mut self, edge: EdgeId) -> Option<GraphEdge> {
        self.edges.remove(&edge)
    }

    pub fn to_pretty_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Upgrade normalized RGB inputs without changing their rendered color.
    /// Connected sources are left intact and explicitly scaled at the color input.
    pub fn upgrade_color_range(&mut self) -> Result<usize, GraphEditError> {
        let legacy: Vec<_> = self
            .nodes
            .values()
            .filter(|n| {
                n.kind == NodeKind::MakeColor
                    && n.properties.get("rgb_max") != Some(&PropertyValue::Int(255))
            })
            .map(|n| n.id)
            .collect();
        for node in &legacy {
            for (index, key) in ["r", "g", "b"].into_iter().enumerate() {
                let Some(pin) = self.pin_by_key(*node, key).map(|p| p.id) else {
                    continue;
                };
                if let Some(edge) = self.edges.values().find(|e| e.input == pin).cloned() {
                    let pos = self.nodes[node].position;
                    let scale = self.add_catalog_node(
                        NodeKind::MathMultiply,
                        [pos[0] - 220.0, pos[1] + index as f64 * 90.0],
                    )?;
                    let left = self.pin_by_key(scale, "left").unwrap().id;
                    let right = self.pin_by_key(scale, "right").unwrap().id;
                    let result = self.pin_by_key(scale, "value").unwrap().id;
                    self.pins.get_mut(&right).unwrap().default_value =
                        Some(PropertyValue::Float(255.0));
                    self.remove_edge(edge.id);
                    self.connect(edge.output, left)?;
                    self.connect(result, pin)?;
                } else if let Some(value) = &mut self.pins.get_mut(&pin).unwrap().default_value {
                    *value = match value {
                        PropertyValue::Float(n) => PropertyValue::Float(*n * 255.0),
                        PropertyValue::Int(n) => PropertyValue::Float(*n as f64 * 255.0),
                        _ => value.clone(),
                    };
                }
            }
            self.set_property(*node, "rgb_max", PropertyValue::Int(255))?;
        }
        Ok(legacy.len())
    }

    pub fn from_json(source: &str) -> Result<Self, GraphLoadError> {
        Self::from_json_with_report(source).map(|(graph, _)| graph)
    }

    pub fn from_json_with_report(
        source: &str,
    ) -> Result<(Self, crate::MigrationReport), GraphLoadError> {
        let value = serde_json::from_str(source).map_err(GraphLoadError::Json)?;
        let (value, report) = crate::migrate_graph_value(value).map_err(|found| {
            GraphLoadError::UnsupportedSchema {
                found,
                supported: GRAPH_SCHEMA_VERSION,
            }
        })?;
        let mut graph: Self = serde_json::from_value(value).map_err(GraphLoadError::Json)?;
        graph.next_node_id = graph.next_node_id.max(
            graph
                .nodes
                .keys()
                .map(|id| id.get().saturating_add(1))
                .max()
                .unwrap_or(1),
        );
        graph.next_pin_id = graph.next_pin_id.max(
            graph
                .pins
                .keys()
                .map(|id| id.get().saturating_add(1))
                .max()
                .unwrap_or(1),
        );
        graph.next_edge_id = graph.next_edge_id.max(
            graph
                .edges
                .keys()
                .map(|id| id.get().saturating_add(1))
                .max()
                .unwrap_or(1),
        );
        let legacy_characters: Vec<_> = graph
            .nodes
            .values()
            .filter(|node| {
                node.kind == NodeKind::CharacterValue
                    && graph.pin_by_key(node.id, "sprite").is_none()
            })
            .map(|node| node.id)
            .collect();
        for node in legacy_characters {
            let _ = graph.add_pin(
                node,
                "sprite",
                "Sprite",
                PinDirection::Input,
                ValueType::Asset(AssetKind::Sprite),
                PinCardinality::One,
            );
        }
        let legacy_calls: Vec<_> = graph
            .nodes
            .values()
            .filter(|node| {
                node.kind == NodeKind::Call && graph.pin_by_key(node.id, "exec_out").is_none()
            })
            .map(|node| node.id)
            .collect();
        for node in legacy_calls {
            graph
                .add_pin(
                    node,
                    "exec_out",
                    "Après retour",
                    PinDirection::Output,
                    ValueType::Execution,
                    PinCardinality::Many,
                )
                .expect("existing Call node");
        }
        let legacy_music: Vec<_> = graph
            .nodes
            .values()
            .filter(|node| {
                node.kind == NodeKind::MusicStop
                    && graph.pin_by_key(node.id, "transition").is_none()
            })
            .map(|node| node.id)
            .collect();
        for node in legacy_music {
            let id = graph
                .add_pin(
                    node,
                    "transition",
                    "Transition",
                    PinDirection::Input,
                    ValueType::Transition,
                    PinCardinality::One,
                )
                .expect("existing MusicStop node");
            graph.pins.get_mut(&id).unwrap().default_value =
                Some(PropertyValue::String("none".into()));
        }
        Ok((graph, report))
    }

    pub fn validate(&self) -> Vec<GraphDiagnostic> {
        crate::validate_document(self)
    }

    fn data_path_exists(&self, start: NodeId, target: NodeId) -> bool {
        let mut pending = vec![start];
        let mut visited = BTreeSet::new();
        while let Some(node) = pending.pop() {
            if node == target {
                return true;
            }
            if !visited.insert(node) {
                continue;
            }
            for edge in self.edges.values() {
                let Some(output) = self.pins.get(&edge.output) else {
                    continue;
                };
                let Some(input) = self.pins.get(&edge.input) else {
                    continue;
                };
                if output.node == node && !output.value_type.is_execution() {
                    pending.push(input.node);
                }
            }
        }
        false
    }
}

fn hotspot_name(value: &str) -> &str {
    value.split(':').next().unwrap_or(value).trim()
}

fn property_value_type(value: &PropertyValue) -> ValueType {
    match value {
        PropertyValue::Bool(_) => ValueType::Bool,
        PropertyValue::Int(_) => ValueType::Int,
        PropertyValue::Float(_) => ValueType::Float,
        PropertyValue::String(_) => ValueType::String,
        PropertyValue::StringList(_) => ValueType::List(Box::new(ValueType::String)),
    }
}

fn conversion_kind_for(output: &ValueType, input: &ValueType) -> Option<NodeKind> {
    input.conversion_from(output)
}

fn transition_kind(value: &PropertyValue) -> NodeKind {
    let PropertyValue::String(value) = value else {
        return NodeKind::TransitionNone;
    };
    match value.split(['(', ':']).next().unwrap_or(value).trim() {
        "fade" => NodeKind::TransitionFade,
        "dissolve" => NodeKind::TransitionDissolve,
        "slideleft" | "slide_left" => NodeKind::TransitionSlideLeft,
        "slideright" | "slide_right" => NodeKind::TransitionSlideRight,
        "slideup" | "slide_up" => NodeKind::TransitionSlideUp,
        "slidedown" | "slide_down" => NodeKind::TransitionSlideDown,
        "zoomin" | "zoom_in" => NodeKind::TransitionZoomIn,
        "zoomout" | "zoom_out" => NodeKind::TransitionZoomOut,
        "wipe" => NodeKind::TransitionWipe,
        "blur" => NodeKind::TransitionBlur,
        _ => NodeKind::TransitionNone,
    }
}

fn transition_duration(value: &PropertyValue) -> Option<i64> {
    let PropertyValue::String(value) = value else {
        return None;
    };
    let start = value.find('(')? + 1;
    let digits: String = value[start..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse().ok()
}

fn format_text_arguments(pattern: &str) -> Vec<String> {
    let mut arguments = Vec::new();
    let mut chars = pattern.char_indices().peekable();
    while let Some((_, character)) = chars.next() {
        // Unreal utilise l'accent grave pour échapper les marqueurs de format.
        if character == '`' {
            chars.next();
            continue;
        }
        if character != '{' {
            continue;
        }
        let start = chars
            .peek()
            .map(|(index, _)| *index)
            .unwrap_or(pattern.len());
        let mut end = None;
        while let Some((index, next)) = chars.next() {
            if next == '}' {
                end = Some(index);
                break;
            }
            if next == '{' {
                break;
            }
        }
        let Some(end) = end else { continue };
        let name = pattern[start..end].trim();
        let valid = !name.is_empty()
            && name
                .chars()
                .next()
                .is_some_and(|first| first == '_' || first.is_alphabetic())
            && name
                .chars()
                .all(|value| value == '_' || value.is_alphanumeric());
        if valid && !arguments.iter().any(|argument| argument == name) {
            arguments.push(name.to_owned());
        }
    }
    arguments
}

fn is_compact_node(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Literal
            | NodeKind::VariableGet
            | NodeKind::Reroute
            | NodeKind::ConvertIntToFloat
            | NodeKind::ConvertNumberToText
            | NodeKind::ConvertTextToInt
            | NodeKind::BinaryOperator
            | NodeKind::UnaryOperator
            | NodeKind::TextValue
            | NodeKind::SceneAsset
            | NodeKind::TransitionNone
            | NodeKind::TransitionFade
            | NodeKind::TransitionDissolve
            | NodeKind::TransitionSlideLeft
            | NodeKind::TransitionSlideRight
            | NodeKind::TransitionSlideUp
            | NodeKind::TransitionSlideDown
            | NodeKind::TransitionZoomIn
            | NodeKind::TransitionZoomOut
            | NodeKind::TransitionWipe
            | NodeKind::TransitionBlur
            | NodeKind::VariableReference
            | NodeKind::MathAdd
            | NodeKind::MathSubtract
            | NodeKind::MathMultiply
            | NodeKind::MathDivide
            | NodeKind::MathEqual
            | NodeKind::MathNotEqual
            | NodeKind::MathLess
            | NodeKind::MathLessEqual
            | NodeKind::MathGreater
            | NodeKind::MathGreaterEqual
            | NodeKind::LogicAnd
            | NodeKind::LogicOr
            | NodeKind::LogicNot
            | NodeKind::MathNegate
    )
}

fn rectangles_overlap(
    first_position: [f64; 2],
    first_size: [f64; 2],
    second_position: [f64; 2],
    second_size: [f64; 2],
    margin: f64,
) -> bool {
    first_position[0] < second_position[0] + second_size[0] + margin
        && first_position[0] + first_size[0] + margin > second_position[0]
        && first_position[1] < second_position[1] + second_size[1] + margin
        && first_position[1] + first_size[1] + margin > second_position[1]
}

#[derive(Debug)]
pub enum GraphLoadError {
    Json(serde_json::Error),
    UnsupportedSchema { found: u32, supported: u32 },
}

impl std::fmt::Display for GraphLoadError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Json(error) => error.fmt(formatter),
            Self::UnsupportedSchema { found, supported } => write!(
                formatter,
                "unsupported graph schema {found}; this build supports schema {supported}"
            ),
        }
    }
}

impl std::error::Error for GraphLoadError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphEditError {
    InvalidVariableName(String),
    NodeNotFound(NodeId),
    PinNotFound(PinId),
    InvalidDirection {
        output: PinId,
        input: PinId,
    },
    IncompatibleTypes {
        output: ValueType,
        input: ValueType,
    },
    DuplicateEdge {
        output: PinId,
        input: PinId,
    },
    InputAlreadyConnected(PinId),
    DataCycle {
        from: NodeId,
        to: NodeId,
    },
    DuplicatePinKey {
        node: NodeId,
        key: String,
    },
    PinKeyNotFound {
        node: NodeId,
        key: String,
    },
    WrongNodeKind {
        node: NodeId,
        expected: NodeKind,
        found: NodeKind,
    },
    InvalidPropertyType {
        node: NodeId,
        key: String,
    },
    UnsupportedDynamicOperand(NodeId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChoiceOptionPins {
    pub index: usize,
    pub branch: PinId,
    pub condition: PinId,
}

impl std::fmt::Display for GraphEditError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for GraphEditError {}
