//! Opt-in authoring contracts, deliberately separate from dynamic RVN syntax.
//! Loading or importing a graph never enables the stricter Blueprint policy.
use crate::*;
use std::collections::BTreeMap;

const STRICT: &str = "blueprint_strict";
const DEFAULT_TYPE_PREFIX: &str = "@blueprint_default_type:";

#[derive(Clone, Copy)]
pub(crate) enum BlueprintOperandDomain {
    Numeric,
    Equality,
    Append,
    Boolean,
}

impl GraphNode {
    pub fn uses_blueprint_operator_policy(&self) -> bool {
        self.kind == NodeKind::StringAppend
            || (self.kind.supports_blueprint_operator_policy()
                && self.properties.get(STRICT) == Some(&PropertyValue::Bool(true)))
    }
}

pub(crate) fn blueprint_operand_domain(node: &GraphNode) -> Option<BlueprintOperandDomain> {
    use BlueprintOperandDomain as D;
    if !node.uses_blueprint_operator_policy() {
        return None;
    }
    match node.kind {
        NodeKind::StringAppend => Some(D::Append),
        NodeKind::MathEqual | NodeKind::MathNotEqual => Some(D::Equality),
        NodeKind::MathAdd
        | NodeKind::MathSubtract
        | NodeKind::MathMultiply
        | NodeKind::MathDivide
        | NodeKind::MathNegate
        | NodeKind::MathLess
        | NodeKind::MathLessEqual
        | NodeKind::MathGreater
        | NodeKind::MathGreaterEqual => Some(D::Numeric),
        NodeKind::BinaryOperator | NodeKind::UnaryOperator => {
            match node
                .properties
                .get("operator")
                .or_else(|| node.properties.get("op"))
            {
                Some(PropertyValue::String(op)) if matches!(op.as_str(), "==" | "!=") => {
                    Some(D::Equality)
                }
                Some(PropertyValue::String(op))
                    if matches!(op.as_str(), "+" | "-" | "*" | "/" | "<" | "<=" | ">" | ">=") =>
                {
                    Some(D::Numeric)
                }
                Some(PropertyValue::String(op)) if matches!(op.as_str(), "not" | "and" | "or") => {
                    Some(D::Boolean)
                }
                _ => None,
            }
        }
        _ => None,
    }
}

fn equality_family_matches(left: &ValueType, right: &ValueType) -> bool {
    use ValueType as T;
    matches!(left, T::Any)
        || matches!(right, T::Any)
        || (matches!(left, T::Int | T::Float) && matches!(right, T::Int | T::Float))
        || (left == right && matches!(left, T::Bool | T::String | T::InterpolatedText))
}

pub(crate) fn authored_default_type(
    graph: &GraphDocument,
    pin: &GraphPin,
    value: &PropertyValue,
) -> ValueType {
    if matches!(value, PropertyValue::String(_))
        && graph.nodes.get(&pin.node).is_some_and(|node| {
            node.properties
                .get(&format!("{DEFAULT_TYPE_PREFIX}{}", pin.key))
                == Some(&PropertyValue::String("Text".into()))
        })
    {
        ValueType::InterpolatedText
    } else {
        crate::type_inference::property_type(value)
    }
}

impl GraphDocument {
    pub fn has_blueprint_operator_policy(&self, node: NodeId) -> bool {
        self.nodes
            .get(&node)
            .is_some_and(GraphNode::uses_blueprint_operator_policy)
    }

    /// Called only by an explicit visual creation/edit. Generic catalog nodes
    /// and source imports deliberately retain their older RVN semantics.
    pub fn enable_blueprint_operator_policy(&mut self, node: NodeId) -> Result<(), GraphEditError> {
        let model = self
            .nodes
            .get_mut(&node)
            .ok_or(GraphEditError::NodeNotFound(node))?;
        if !model.kind.supports_blueprint_operator_policy() {
            return Err(GraphEditError::UnsupportedBlueprintOperator(node));
        }
        model
            .properties
            .insert(STRICT.into(), PropertyValue::Bool(true));
        Ok(())
    }

    /// Authoring compatibility, including the other equality operand's family.
    /// The existing value/default on the target input is ignored, allowing a
    /// replacement to be checked without deleting anything first.
    pub fn accepts_pin_source(&self, input: PinId, source: &ValueType) -> bool {
        let contextual = self
            .pins
            .get(&input)
            .and_then(|pin| self.nodes.get(&pin.node))
            .is_some_and(|node| {
                matches!(
                    blueprint_operand_domain(node),
                    Some(BlueprintOperandDomain::Equality)
                )
            });
        let types = if contextual {
            self.effective_pin_types()
        } else {
            BTreeMap::new()
        };
        self.accepts_pin_source_with_types(input, source, &types)
    }

    /// Reuse a graph projection for palette, hover and validation. No inference
    /// or model mutation is performed by this cached form.
    pub fn accepts_pin_source_with_types(
        &self,
        input: PinId,
        source: &ValueType,
        types: &BTreeMap<PinId, ValueType>,
    ) -> bool {
        self.accepts_pin_source_with_context(input, source, types, None)
    }

    pub(crate) fn accepts_pin_source_with_context(
        &self,
        input: PinId,
        source: &ValueType,
        types: &BTreeMap<PinId, ValueType>,
        incoming: Option<&BTreeMap<PinId, Option<PinId>>>,
    ) -> bool {
        let Some(pin) = self.pins.get(&input) else {
            return false;
        };
        let Some(node) = self.nodes.get(&pin.node) else {
            return false;
        };
        if pin.direction != PinDirection::Input {
            return false;
        }
        let declared = self.pin_constraint_type(input).unwrap_or(ValueType::Any);
        if !declared.accepts(source) || !node.kind.accepts_data_source(source) {
            return false;
        }
        let Some(domain) = blueprint_operand_domain(node) else {
            return true;
        };
        use BlueprintOperandDomain as D;
        use ValueType as T;
        match domain {
            D::Numeric => matches!(source, T::Any | T::Int | T::Float),
            D::Append => matches!(source, T::Any | T::String),
            D::Boolean => matches!(source, T::Any | T::Bool),
            D::Equality => {
                if !matches!(
                    source,
                    T::Any | T::Int | T::Float | T::Bool | T::String | T::InterpolatedText
                ) {
                    return false;
                }
                node.pins
                    .iter()
                    .filter_map(|id| self.pins.get(id))
                    .filter(|other| {
                        other.id != input
                            && other.direction == PinDirection::Input
                            && !other.value_type.is_execution()
                    })
                    .all(|other| {
                        let producer = if let Some(incoming) = incoming {
                            incoming.get(&other.id).copied()
                        } else {
                            let mut wires =
                                self.edges.values().filter(|edge| edge.input == other.id);
                            wires.next().map(|edge| {
                                if wires.next().is_some() {
                                    None
                                } else {
                                    Some(edge.output)
                                }
                            })
                        };
                        let evidence = match producer {
                            Some(Some(output)) => types.get(&output).cloned().unwrap_or(T::Any),
                            None => other
                                .default_value
                                .as_ref()
                                .map(|value| authored_default_type(self, other, value))
                                .unwrap_or(T::Any),
                            Some(None) => T::Any,
                        };
                        equality_family_matches(source, &evidence)
                    })
            }
        }
    }

    /// Commit a typed inline value. Text needs a retained semantic marker:
    /// PropertyValue::String alone cannot distinguish it from a raw String.
    /// Codegen emits string_to_text(...) for this marker, not a fake recolor.
    pub fn set_blueprint_pin_default(
        &mut self,
        input: PinId,
        value: PropertyValue,
        value_type: ValueType,
    ) -> Result<(), GraphEditError> {
        let pin = self
            .pins
            .get(&input)
            .ok_or(GraphEditError::PinNotFound(input))?;
        let actual = crate::type_inference::property_type(&value);
        let text = value_type == ValueType::InterpolatedText && actual == ValueType::String;
        if (!text && value_type != actual) || !self.accepts_pin_source(input, &value_type) {
            return Err(GraphEditError::IncompatibleTypes {
                output: value_type,
                input: self.pin_constraint_type(input).unwrap_or(ValueType::Any),
            });
        }
        let node = pin.node;
        let marker = format!("{DEFAULT_TYPE_PREFIX}{}", pin.key);
        self.pins.get_mut(&input).unwrap().default_value = Some(value);
        let properties = &mut self.nodes.get_mut(&node).unwrap().properties;
        if text {
            properties.insert(marker, PropertyValue::String("Text".into()));
        } else {
            properties.remove(&marker);
        }
        Ok(())
    }
}
