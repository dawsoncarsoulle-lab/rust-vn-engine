use crate::{
    EdgeId, GraphDocument, NodeId, NodeKind, PinCardinality, PinDirection, PinId, PropertyValue,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

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

    for node in graph.nodes.values() {
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
        if !input.value_type.accepts(&output.value_type)
            || graph.nodes.get(&input.node).is_some_and(|node| !node.kind.accepts_data_source(&output.value_type)) {
            diagnostics.push(GraphDiagnostic::error(
                "incompatible_types",
                format!(
                    "pin type {:?} cannot accept {:?}",
                    input.value_type, output.value_type
                ),
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
