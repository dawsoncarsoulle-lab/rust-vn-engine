use crate::{
    EdgeId, GraphDocument, NodeId, NodeKind, PinCardinality, PinDirection, PinId, PropertyValue,
    ValueType,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSeverity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphDiagnostic {
    pub severity: DiagnosticSeverity,
    pub code: String,
    pub message: String,
    pub node: Option<NodeId>,
    pub pin: Option<PinId>,
    pub edge: Option<EdgeId>,
}

impl GraphDiagnostic {
    fn error(
        code: &str,
        message: impl Into<String>,
        node: Option<NodeId>,
        pin: Option<PinId>,
        edge: Option<EdgeId>,
    ) -> Self {
        Self {
            severity: DiagnosticSeverity::Error,
            code: code.to_owned(),
            message: message.into(),
            node,
            pin,
            edge,
        }
    }
}

pub(crate) fn validate_document(graph: &GraphDocument) -> Vec<GraphDiagnostic> {
    let mut diagnostics = Vec::new();
    let mut seen_pin_keys = BTreeSet::new();
    let types = graph.effective_pin_types();
    let mut incoming = BTreeMap::new();
    for edge in graph.edges.values() {
        incoming
            .entry(edge.input)
            .and_modify(|source| *source = None)
            .or_insert(Some(edge.output));
    }

    for node in graph.nodes.values() {
        if let (Some(PropertyValue::String(name)), Some(PropertyValue::StringList(types))) = (
            node.properties.get("name"),
            node.properties.get("inferred_conflict_types"),
        ) {
            diagnostics.push(GraphDiagnostic{severity:DiagnosticSeverity::Warning,code:"variable_type_conflict".into(),message:format!("La variable globale « {name} » reçoit plusieurs types connus ({}). Son type effectif est Wildcard ; RVN reste dynamique. Utilisez une conversion explicite pour stabiliser son type.",types.join(", ")),node:Some(node.id),pin:None,edge:None});
        }
        if matches!(node.kind, NodeKind::VariableGet | NodeKind::SetVariable) {
            let name = node
                .properties
                .get("name")
                .and_then(|value| match value {
                    PropertyValue::String(value) => Some(value.as_str()),
                    _ => None,
                })
                .or_else(|| {
                    (node.kind == NodeKind::SetVariable)
                        .then(|| graph.pin_by_key(node.id, "name"))
                        .flatten()
                        .and_then(|pin| match pin.default_value.as_ref() {
                            Some(PropertyValue::String(value)) => Some(value.as_str()),
                            _ => None,
                        })
                })
                .or_else(|| {
                    let input = (node.kind == NodeKind::SetVariable)
                        .then(|| graph.pin_by_key(node.id, "name"))
                        .flatten()?;
                    let source = graph
                        .edges
                        .values()
                        .find(|edge| edge.input == input.id)
                        .and_then(|edge| graph.pins.get(&edge.output))?;
                    let source_node = graph.nodes.get(&source.node)?;
                    (source_node.kind == NodeKind::VariableReference)
                        .then(|| source_node.properties.get("name"))
                        .flatten()
                        .and_then(|value| match value {
                            PropertyValue::String(value) => Some(value.as_str()),
                            _ => None,
                        })
                });
            if name.is_none_or(|name| !graph.variables.contains_key(name)) {
                diagnostics.push(GraphDiagnostic::error(
                    "unknown_variable",
                    format!(
                        "la variable `{}` doit d’abord être créée dans le panneau Variables",
                        name.unwrap_or("?")
                    ),
                    Some(node.id),
                    None,
                    None,
                ));
            }
        }
        for pin_id in &node.pins {
            let Some(pin) = graph.pins.get(pin_id) else {
                diagnostics.push(GraphDiagnostic::error(
                    "missing_pin",
                    format!("node {} references missing pin {}", node.id, pin_id),
                    Some(node.id),
                    Some(*pin_id),
                    None,
                ));
                continue;
            };
            if pin.node != node.id {
                diagnostics.push(GraphDiagnostic::error(
                    "wrong_pin_owner",
                    format!(
                        "pin {} belongs to node {}, not {}",
                        pin.id, pin.node, node.id
                    ),
                    Some(node.id),
                    Some(pin.id),
                    None,
                ));
            }
            if !seen_pin_keys.insert((node.id, pin.key.as_str())) {
                diagnostics.push(GraphDiagnostic::error(
                    "duplicate_pin_key",
                    format!("node {} contains duplicate pin key {:?}", node.id, pin.key),
                    Some(node.id),
                    Some(pin.id),
                    None,
                ));
            }
            // Preserve the initialized variable's declared type, and diagnose
            // incompatible authored assignments instead of erasing it to Any
            // or silently parsing a String as an Int during source import.
            if matches!(node.kind, NodeKind::SetVariable | NodeKind::LocalVariable)
                && pin.key == "value"
                && !graph.edges.values().any(|edge| edge.input == pin.id)
            {
                if let Some(value) = &pin.default_value {
                    let source = crate::type_inference::property_type(value);
                    let input = graph.pin_constraint_type(pin.id).unwrap_or(ValueType::Any);
                    if !input.accepts(&source)
                        && !(input == ValueType::InterpolatedText && source == ValueType::String)
                    {
                        diagnostics.push(GraphDiagnostic::error(
                            "incompatible_types",
                            format!("pin type {input:?} cannot accept {source:?}"),
                            Some(node.id),
                            Some(pin.id),
                            None,
                        ));
                    }
                }
            }
            if node.uses_blueprint_operator_policy()
                && pin.direction == PinDirection::Input
                && !pin.value_type.is_execution()
                && !incoming.contains_key(&pin.id)
            {
                if let Some(value) = &pin.default_value {
                    let source = crate::blueprint_policy::authored_default_type(graph, pin, value);
                    if !graph.accepts_pin_source_with_context(
                        pin.id,
                        &source,
                        &types,
                        Some(&incoming),
                    ) {
                        diagnostics.push(GraphDiagnostic::error(
                            "incompatible_types",
                            format!(
                                "Blueprint operand {} cannot accept {source:?} in this operator",
                                pin.key
                            ),
                            Some(node.id),
                            Some(pin.id),
                            None,
                        ));
                    }
                }
            }
        }
    }

    for pin in graph.pins.values() {
        if !graph.nodes.contains_key(&pin.node) {
            diagnostics.push(GraphDiagnostic::error(
                "missing_node",
                format!("pin {} references missing node {}", pin.id, pin.node),
                Some(pin.node),
                Some(pin.id),
                None,
            ));
        }
    }

    let mut connected_inputs = BTreeSet::new();
    for edge in graph.edges.values() {
        let Some(output) = graph.pins.get(&edge.output) else {
            diagnostics.push(GraphDiagnostic::error(
                "missing_output_pin",
                format!(
                    "edge {} references missing output pin {}",
                    edge.id, edge.output
                ),
                None,
                Some(edge.output),
                Some(edge.id),
            ));
            continue;
        };
        let Some(input) = graph.pins.get(&edge.input) else {
            diagnostics.push(GraphDiagnostic::error(
                "missing_input_pin",
                format!(
                    "edge {} references missing input pin {}",
                    edge.id, edge.input
                ),
                None,
                Some(edge.input),
                Some(edge.id),
            ));
            continue;
        };
        if output.direction != PinDirection::Output || input.direction != PinDirection::Input {
            diagnostics.push(GraphDiagnostic::error(
                "invalid_direction",
                format!("edge {} is not connected from output to input", edge.id),
                Some(input.node),
                Some(input.id),
                Some(edge.id),
            ));
        }
        let output_type = types.get(&output.id).unwrap_or(&output.value_type);
        let input_type = graph
            .pin_constraint_type(input.id)
            .unwrap_or_else(|| input.value_type.clone());
        if !graph.accepts_pin_source_with_context(input.id, output_type, &types, Some(&incoming)) {
            diagnostics.push(GraphDiagnostic::error(
                "incompatible_types",
                format!("pin type {:?} cannot accept {:?}", input_type, output_type),
                Some(input.node),
                Some(input.id),
                Some(edge.id),
            ));
        }
        if input.cardinality == PinCardinality::One && !connected_inputs.insert(input.id) {
            diagnostics.push(GraphDiagnostic::error(
                "input_cardinality",
                format!("input pin {} accepts only one connection", input.id),
                Some(input.node),
                Some(input.id),
                Some(edge.id),
            ));
        }
    }

    if let Some(node) = first_data_cycle(graph) {
        diagnostics.push(GraphDiagnostic::error(
            "data_cycle",
            format!("expression graph contains a cycle through node {node}"),
            Some(node),
            None,
            None,
        ));
    }

    diagnostics
}

fn first_data_cycle(graph: &GraphDocument) -> Option<NodeId> {
    fn visit(
        graph: &GraphDocument,
        node: NodeId,
        active: &mut BTreeSet<NodeId>,
        complete: &mut BTreeSet<NodeId>,
    ) -> Option<NodeId> {
        if active.contains(&node) {
            return Some(node);
        }
        if complete.contains(&node) {
            return None;
        }
        active.insert(node);
        for edge in graph.edges.values() {
            let (Some(output), Some(input)) =
                (graph.pins.get(&edge.output), graph.pins.get(&edge.input))
            else {
                continue;
            };
            if output.node == node && !output.value_type.is_execution() {
                if let Some(cycle) = visit(graph, input.node, active, complete) {
                    return Some(cycle);
                }
            }
        }
        active.remove(&node);
        complete.insert(node);
        None
    }

    let mut active = BTreeSet::new();
    let mut complete = BTreeSet::new();
    for node in graph.nodes.keys().copied() {
        if let Some(cycle) = visit(graph, node, &mut active, &mut complete) {
            return Some(cycle);
        }
    }
    None
}
