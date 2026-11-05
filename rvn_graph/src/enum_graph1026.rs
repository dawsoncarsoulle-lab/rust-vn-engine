//! Nominal editor values share the Parser/Core UUID domain. This module owns
//! no asset registry, filesystem reader, Source lease or runtime capability.
use crate::{GraphDocument, NodeId, NodeKind, PropertyValue, ValueType};
use rvn_parser::{enum_asset::{EnumAssetId, EnumMemberId, EnumValue},
    enum_asset_admission1025::EnumAssets1025, Expr};

pub const ENUM_VALUE_CONSTRUCTOR1026: &str = "__rvn_enum_value_v1";

pub(crate) mod asset_id_wire {
    use super::EnumAssetId;
    use serde::{Deserialize, Deserializer, Serializer};
    pub fn serialize<S: Serializer>(id: &EnumAssetId, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&id.to_hex())
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<EnumAssetId, D::Error> {
        EnumAssetId::parse(&String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

/// Syntax recognition grants neither asset existence nor member admission.
/// Malformed reserved constructors fail instead of becoming an Any call.
pub(crate) fn literal_expression1026(expression: &Expr) -> Result<Option<EnumValue>, String> {
    let Expr::Call { name, args } = expression else { return Ok(None); };
    if name != ENUM_VALUE_CONSTRUCTOR1026 { return Ok(None); }
    let [Expr::Str(asset), Expr::Str(member)] = args.as_slice() else {
        return Err("An authored Enum literal needs two canonical opaque UUID strings".into());
    };
    Ok(Some(EnumValue { asset: EnumAssetId::parse(asset)?, member: EnumMemberId::parse(member)? }))
}

pub(crate) fn source1026(value: EnumValue) -> String {
    format!("{}(\"{}\", \"{}\")", ENUM_VALUE_CONSTRUCTOR1026, value.asset.to_hex(), value.member.to_hex())
}

pub(crate) fn contains_type1026(value: &ValueType) -> bool {
    match value { ValueType::Enum(_) => true, ValueType::List(item) => contains_type1026(item), _ => false }
}

pub(crate) fn is_equality1026(node: &crate::GraphNode) -> bool {
    matches!(node.kind, NodeKind::MathEqual | NodeKind::MathNotEqual)
        || node.kind == NodeKind::BinaryOperator
            && matches!(node.properties.get("operator").or_else(|| node.properties.get("op")), Some(PropertyValue::String(op)) if matches!(op.as_str(), "==" | "!="))
}

pub(crate) fn equality_family1026(a: &ValueType, b: &ValueType) -> bool {
    match (a,b) {
        (ValueType::Any, _) | (_, ValueType::Any) => true,
        (ValueType::Enum(a), ValueType::Enum(b)) => a == b,
        (ValueType::Enum(_), _) | (_, ValueType::Enum(_)) => false,
        _ => true,
    }
}

/// Prepare the complete real project cohort. The actual Source/Journal owner
/// still checks its snapshot and publishes this result once. A computed,
/// conditional or multiply initialized global is never rewritten as a literal.
pub fn replace_enum_global_initializer1026(graphs: &[GraphDocument], name: &str,
    value: EnumValue, resources: &EnumAssets1025) -> Result<Vec<GraphDocument>, String> {
    resources.validate_value(value)?;
    let compiled = crate::transpile_project_with_origins(graphs)?;
    let mut found = Vec::new();
    fn visit(statements: &[rvn_parser::Statement], origins: &[crate::GraphStatementOrigin],
        name: &str, in_init: bool, unconditional: bool,
        found: &mut Vec<Option<(crate::GraphId, NodeId)>>) {
        use rvn_parser::Statement as S;
        for (statement, origin) in statements.iter().zip(origins) {
            match statement {
                S::Init { body } => visit(body, &origin.children, name, true, true, found),
                S::SetVar { name: target, .. } if in_init && target == name => {
                    found.push(if unconditional { origin.graph.zip(origin.node) } else { None });
                }
                S::Sequence { branches } if in_init => {
                    let mut offset = 0;
                    for body in branches {
                        let end = offset + body.len();
                        if let Some(children) = origin.children.get(offset..end) {
                            visit(body, children, name, in_init, unconditional, found);
                        } else { found.push(None); }
                        offset = end;
                    }
                }
                S::ScheduledConsumer { body, .. } if in_init =>
                    visit(body, &origin.children, name, true, unconditional, found),
                S::If { then_branch, else_branch, .. } if in_init => {
                    let split = then_branch.len();
                    if let (Some(a), Some(b)) = (origin.children.get(..split), origin.children.get(split..)) {
                        visit(then_branch, a, name, true, false, found);
                        visit(else_branch, b, name, true, false, found);
                    } else { found.push(None); }
                }
                S::While { body, .. } | S::ForEach { body, .. } | S::ForLoopWithBreak { body, .. } if in_init =>
                    visit(body, &origin.children, name, true, false, found),
                _ => {},
            }
        }
        if statements.len() != origins.len() { found.push(None); }
    }
    visit(&compiled.ast, &compiled.origins, name, false, true, &mut found);
    let [Some((graph, node))] = found.as_slice() else {
        return Err("An Enum global edit needs one unconditional actual Init declaration owner".into());
    };
    let owners = graphs.iter().enumerate().filter(|(_, item)| item.graph_id == *graph).collect::<Vec<_>>();
    let [(index, owner)] = owners.as_slice() else { return Err("Enum global declaration graph is ambiguous or absent".into()); };
    if owner.kind != crate::GraphKind::Init || owner.variable_scope(name) != Some(crate::VariableScope::Global)
        || owner.variable_node_name(*node).as_deref() != Some(name)
        || owner.literal_input_value(*node, "value")?.is_none() {
        return Err("The actual global initializer is computed or belongs to another scope".into());
    }
    let mut next = graphs.to_vec();
    next[*index].set_literal_input(*node, "value", PropertyValue::Enum(value))?;
    for graph in &mut next {
        if graph.variable_scope(name) == Some(crate::VariableScope::Global) {
            let definition = graph.variables.get_mut(name).ok_or("The actual global definition disappeared")?;
            definition.value_type = ValueType::Enum(value.asset);
            definition.default_value = PropertyValue::Enum(value);
        }
        graph.adopt_nominal_enum_schema1026();
    }
    let context = next.clone();
    let mut globals = owner.source_global_binding_types()?;
    globals.insert(name.into(), ValueType::Enum(value.asset));
    for graph in &mut next {
        graph.refresh_narrative_binding_types_in_project(&globals, &context)?;
        graph.validate_enum_references1026(resources)?;
    }
    crate::transpile_project(&next)?;
    Ok(next)
}

impl GraphDocument {
    pub fn has_nominal_enum1026(&self) -> bool {
        self.pins.values().any(|pin| contains_type1026(&pin.value_type)
            || matches!(pin.default_value, Some(PropertyValue::Enum(_))))
            || self.nodes.values().any(|node| node.properties.values().any(|value| matches!(value, PropertyValue::Enum(_))))
            || self.variables.values().any(|variable| contains_type1026(&variable.value_type)
                || matches!(variable.default_value, PropertyValue::Enum(_)))
    }

    /// Explicit feature adoption on an actual in-memory authoring edit/import.
    /// Legacy documents without Enum retain their own schema and exact data.
    pub(crate) fn adopt_nominal_enum_schema1026(&mut self) {
        if self.has_nominal_enum1026() {
            self.schema_version = self.schema_version.max(crate::GRAPH_NOMINAL_ENUM_SCHEMA_VERSION);
        }
    }

    /// The existing Source/Journal owner must still publish this prepared edit.
    /// The selected member must belong to the supplied admitted project view.
    pub fn set_enum_literal1026(&mut self, node: NodeId, value: EnumValue,
        resources: &EnumAssets1025) -> Result<(), String> {
        resources.validate_value(value)?;
        if !self.nodes.get(&node).is_some_and(|node| node.kind == NodeKind::Literal) {
            return Err("Enum literal editing needs the actual Literal node".into());
        }
        let mut next = self.clone();
        next.set_property(node, "value", PropertyValue::Enum(value)).map_err(|error| error.to_string())?;
        next.validate_enum_references1026(resources)?;
        *self = next;
        Ok(())
    }

    pub fn set_enum_pin_default1026(&mut self, node: NodeId, key: &str, value: EnumValue,
        resources: &EnumAssets1025) -> Result<(), String> {
        resources.validate_value(value)?;
        let pin = self.pin_by_key(node, key).ok_or("The original Enum default pin is absent")?;
        if !self.accepts_pin_source(pin.id, &ValueType::Enum(value.asset)) {
            return Err("The Enum member has another nominal pin type".into());
        }
        let mut next = self.clone();
        next.set_pin_default(node, key, PropertyValue::Enum(value)).map_err(|error| error.to_string())?;
        next.adopt_nominal_enum_schema1026();
        next.validate_enum_references1026(resources)?;
        *self = next;
        Ok(())
    }

    /// Recheck real typed references against the caller's immutable admitted
    /// closure. Loading bytes/UUID syntax alone is deliberately insufficient.
    pub fn validate_enum_references1026(&self, resources: &EnumAssets1025) -> Result<(), String> {
        self.validate_nominal_enum_shape1026()?;
        fn ty(value: &ValueType, resources: &EnumAssets1025) -> Result<(), String> {
            match value {
                ValueType::Enum(id) if resources.asset(*id).is_none() => Err("Graph Enum type is absent from the admitted project".into()),
                ValueType::List(item) => ty(item, resources), _ => Ok(()),
            }
        }
        let property = |value: &PropertyValue| -> Result<(), String> {
            if let PropertyValue::Enum(value) = value { resources.validate_value(*value)?; }
            Ok(())
        };
        for pin in self.pins.values() { ty(&pin.value_type, resources)?; if let Some(value) = &pin.default_value { property(value)?; } }
        for node in self.nodes.values() { for value in node.properties.values() { property(value)?; } }
        for variable in self.variables.values() { ty(&variable.value_type, resources)?; property(&variable.default_value)?; }
        Ok(())
    }

    pub(crate) fn validate_nominal_enum_shape1026(&self) -> Result<(), String> {
        if !self.has_nominal_enum1026() && self.schema_version != crate::GRAPH_NOMINAL_ENUM_SCHEMA_VERSION {
            return Ok(());
        }
        if self.has_nominal_enum1026() && !matches!(self.schema_version,crate::GRAPH_NOMINAL_ENUM_SCHEMA_VERSION | crate::GRAPH_QUALIFIED_LIBRARY_SCHEMA_VERSION) {
            return Err("Nominal Enum graph data requires its explicit schema15 contract".into());
        }
        // Loading untrusted graph bytes must refuse malformed ownership before
        // inference can traverse indexed node/pin references.
        for (id, node) in &self.nodes {
            if node.id != *id || node.pins.iter().any(|pin| self.pins.get(pin).is_none_or(|pin| pin.node != *id)) {
                return Err("Nominal graph has malformed node/pin ownership".into());
            }
        }
        for (id, pin) in &self.pins {
            if pin.id != *id || self.nodes.get(&pin.node).is_none_or(|node| node.pins.iter().filter(|item| **item == *id).count() != 1) {
                return Err("Nominal graph pin lacks its exact node owner".into());
            }
        }
        if self.edges.values().any(|edge| !self.pins.contains_key(&edge.output) || !self.pins.contains_key(&edge.input)) {
            return Err("Nominal graph edge references an absent pin".into());
        }
        for pin in self.pins.values() {
            if let Some(PropertyValue::Enum(value)) = &pin.default_value {
                let target = self.pin_constraint_type(pin.id).ok_or("The Enum default pin has no actual constraint")?;
                if !target.accepts(&ValueType::Enum(value.asset)) {
                    return Err("Stored Enum default has another nominal pin type".into());
                }
            }
        }
        // Presentation caches may retain a nonliteral placeholder for a real
        // computed initializer. But two concrete nominal identities must never
        // disagree, including in serialized VariableDefinition defaults.
        for variable in self.variables.values() {
            if let PropertyValue::Enum(value) = &variable.default_value {
                if !variable.value_type.accepts(&ValueType::Enum(value.asset)) {
                    return Err("Stored Enum variable default has another nominal declared type".into());
                }
            }
        }
        let types = self.effective_pin_types();
        for edge in self.edges.values() {
            let Some(source) = types.get(&edge.output) else { continue; };
            let target = self.pin_constraint_type(edge.input).ok_or("Nominal link has no actual input constraint")?;
            if (contains_type1026(source) || contains_type1026(&target))
                && !self.accepts_pin_source_with_types(edge.input, source, &types) {
                return Err("Graph link mixes incompatible nominal Enum types".into());
            }
        }
        for node in self.nodes.values() {
            if node.kind == NodeKind::FunctionCall
                && node.properties.get("function") == Some(&PropertyValue::String(ENUM_VALUE_CONSTRUCTOR1026.into())) {
                return Err("The reserved Enum constructor must be an authored nominal Literal, not an untyped generic call".into());
            }
            let equality = is_equality1026(node);
            let operands = node.pins.iter().filter_map(|id| self.pins.get(id))
                .filter(|pin| pin.direction == crate::PinDirection::Input && !pin.value_type.is_execution())
                .filter_map(|pin| types.get(&pin.id)).filter(|ty| **ty != ValueType::Any).collect::<Vec<_>>();
            if let Some(id) = operands.iter().find_map(|ty| if let ValueType::Enum(id) = ty { Some(*id) } else { None }) {
                if equality && operands.iter().any(|ty| **ty != ValueType::Enum(id)) {
                    return Err("Enum equality requires the same nominal asset on both operands".into());
                }
                if !equality && matches!(node.kind, NodeKind::MathAdd | NodeKind::MathSubtract
                    | NodeKind::MathMultiply | NodeKind::MathDivide | NodeKind::MathNegate
                    | NodeKind::MathLess | NodeKind::MathLessEqual | NodeKind::MathGreater
                    | NodeKind::MathGreaterEqual | NodeKind::BinaryOperator | NodeKind::UnaryOperator
                    | NodeKind::LogicAnd | NodeKind::LogicOr | NodeKind::LogicNot) {
                    return Err("Enum values have no implicit arithmetic, ordering, Boolean or string conversion".into());
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "enum_graph_tests1026.rs"]
mod tests;
