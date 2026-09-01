//! Graph operations and utilities

use crate::core::{Blueprint, Node, Wire, PinKind, PinDirection};
use glam::Vec2;
use std::collections::HashMap;

/// Graph operations for blueprints
pub struct GraphOperations;

impl GraphOperations {
    /// Create a simple execution chain: Event -> Node -> Node -> ...
    pub fn create_execution_chain(
        nodes: Vec<(String, String, Vec2)>,
    ) -> Blueprint {
        let mut blueprint = Blueprint::new("execution_chain", "Execution Chain");
        
        let mut prev_node_id: Option<String> = None;
        
        for (i, (id, title, pos)) in nodes.into_iter().enumerate() {
            let node_type = if i == 0 {
                crate::core::NodeType::Event
            } else if i == nodes.len() - 1 {
                crate::core::NodeType::Function
            } else {
                crate::core::NodeType::Function
            };
            
            let mut node = Node::new(id.clone(), title, node_type, pos);
            
            // Add exec input (except for first node)
            if i > 0 {
                node = node.with_input(crate::core::Pin::exec("in", PinDirection::Input));
            }
            
            // Add exec output
            node = node.with_output(crate::core::Pin::exec("out", PinDirection::Output));
            
            blueprint.add_node(node);
            
            // Connect to previous node
            if let Some(prev_id) = prev_node_id {
                let wire = Wire::new(
                    format!("wire_{}_{}", prev_id, id),
                    (prev_id.clone(), "out"),
                    (id.clone(), "in"),
                    PinKind::Exec,
                );
                blueprint.add_wire(wire);
            }
            
            prev_node_id = Some(id);
        }
        
        blueprint
    }

    /// Create a branch (if-then-else) structure
    pub fn create_branch(
        condition_node: (String, String, Vec2),
        true_path: Vec<(String, String, Vec2)>,
        false_path: Vec<(String, String, Vec2)>,
    ) -> Blueprint {
        let mut blueprint = Blueprint::new("branch_example", "Branch Example");
        
        // Create condition node (branch)
        let branch_id = condition_node.0.clone();
        let mut branch_node = Node::branch(condition_node.0, condition_node.1, condition_node.2);
        branch_node = branch_node
            .with_input(crate::core::Pin::exec("in", PinDirection::Input))
            .with_input(crate::core::Pin::bool("condition", "Condition", PinDirection::Input))
            .with_output(crate::core::Pin::exec("true", PinDirection::Output))
            .with_output(crate::core::Pin::exec("false", PinDirection::Output));
        blueprint.add_node(branch_node);
        
        // Create true path
        let mut prev_node_id: Option<String> = Some(branch_id.clone());
        for (id, title, pos) in true_path {
            let mut node = Node::function(id.clone(), title, pos);
            node = node
                .with_input(crate::core::Pin::exec("in", PinDirection::Input))
                .with_output(crate::core::Pin::exec("out", PinDirection::Output));
            blueprint.add_node(node);
            
            if let Some(prev_id) = prev_node_id {
                let wire = Wire::new(
                    format!("wire_true_{}_{}", prev_id, id),
                    (prev_id, "true"),
                    (id.clone(), "in"),
                    PinKind::Exec,
                );
                blueprint.add_wire(wire);
            }
            prev_node_id = Some(id);
        }
        
        // Create false path
        let mut prev_node_id: Option<String> = Some(branch_id.clone());
        for (id, title, pos) in false_path {
            let mut node = Node::function(id.clone(), title, pos);
            node = node
                .with_input(crate::core::Pin::exec("in", PinDirection::Input))
                .with_output(crate::core::Pin::exec("out", PinDirection::Output));
            blueprint.add_node(node);
            
            if let Some(prev_id) = prev_node_id {
                let wire = Wire::new(
                    format!("wire_false_{}_{}", prev_id, id),
                    (prev_id, "false"),
                    (id.clone(), "in"),
                    PinKind::Exec,
                );
                blueprint.add_wire(wire);
            }
            prev_node_id = Some(id);
        }
        
        blueprint
    }

    /// Validate a blueprint graph
    pub fn validate(blueprint: &Blueprint) -> Vec<ValidationError> {
        let mut errors = Vec::new();
        
        // Check for nodes with no connections
        for node in &blueprint.nodes {
            if node.inputs.is_empty() && node.outputs.is_empty() {
                errors.push(ValidationError::IsolatedNode(node.id.clone()));
            }
        }
        
        // Check for broken connections
        for wire in &blueprint.wires {
            if blueprint.find_node(&wire.from.0).is_none() {
                errors.push(ValidationError::MissingNode(wire.from.0.clone()));
            }
            if blueprint.find_node(&wire.to.0).is_none() {
                errors.push(ValidationError::MissingNode(wire.to.0.clone()));
            }
            
            if let Some(node) = blueprint.find_node(&wire.from.0) {
                if node.find_pin(&wire.from.1).is_none() {
                    errors.push(ValidationError::MissingPin(
                        wire.from.0.clone(),
                        wire.from.1.clone(),
                    ));
                }
            }
            
            if let Some(node) = blueprint.find_node(&wire.to.0) {
                if node.find_pin(&wire.to.1).is_none() {
                    errors.push(ValidationError::MissingPin(
                        wire.to.0.clone(),
                        wire.to.1.clone(),
                    ));
                }
            }
        }
        
        // Check for type mismatches
        for wire in &blueprint.wires {
            if let (Some(from_node), Some(to_node)) = (
                blueprint.find_node(&wire.from.0),
                blueprint.find_node(&wire.to.0),
            ) {
                if let (Some(from_pin), Some(to_pin)) = (
                    from_node.find_pin(&wire.from.1),
                    to_node.find_pin(&wire.to.1),
                ) {
                    if from_pin.kind != to_pin.kind {
                        if from_pin.kind != PinKind::Any && to_pin.kind != PinKind::Any {
                            errors.push(ValidationError::TypeMismatch(
                                wire.from.0.clone(),
                                wire.from.1.clone(),
                                wire.to.0.clone(),
                                wire.to.1.clone(),
                                from_pin.kind,
                                to_pin.kind,
                            ));
                        }
                    }
                }
            }
        }
        
        errors
    }

    /// Topologically sort nodes (for execution order)
    pub fn topological_sort(blueprint: &Blueprint) -> Vec<String> {
        let mut sorted = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut temp_visited = std::collections::HashSet::new();
        
        // Build adjacency list
        let mut adj: HashMap<String, Vec<String>> = HashMap::new();
        for node in &blueprint.nodes {
            adj.insert(node.id.clone(), Vec::new());
        }
        
        for wire in &blueprint.wires {
            if let Some(entries) = adj.get_mut(&wire.from.0) {
                entries.push(wire.to.0.clone());
            }
        }
        
        // DFS-based topological sort
        fn dfs(
            node_id: &str,
            adj: &HashMap<String, Vec<String>>,
            visited: &mut std::collections::HashSet<String>,
            temp_visited: &mut std::collections::HashSet<String>,
            sorted: &mut Vec<String>,
        ) -> bool {
            if temp_visited.contains(node_id) {
                return false; // Cycle detected
            }
            if visited.contains(node_id) {
                return true;
            }
            
            temp_visited.insert(node_id.to_string());
            
            if let Some(neighbors) = adj.get(node_id) {
                for neighbor in neighbors {
                    if !dfs(neighbor, adj, visited, temp_visited, sorted) {
                        return false;
                    }
                }
            }
            
            temp_visited.remove(node_id);
            visited.insert(node_id.to_string());
            sorted.push(node_id.to_string());
            true
        }
        
        for node in &blueprint.nodes {
            if !visited.contains(&node.id) {
                let mut temp = std::collections::HashSet::new();
                dfs(&node.id, &adj, &mut visited, &mut temp, &mut sorted);
            }
        }
        
        sorted.reverse();
        sorted
    }
}

/// Validation error types
#[derive(Debug, Clone)]
pub enum ValidationError {
    /// A node has no connections
    IsolatedNode(String),
    /// A referenced node doesn't exist
    MissingNode(String),
    /// A referenced pin doesn't exist
    MissingPin(String, String),
    /// Type mismatch between connected pins
    TypeMismatch(String, String, String, String, PinKind, PinKind),
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationError::IsolatedNode(node_id) => {
                write!(f, "Node '{}' has no connections", node_id)
            }
            ValidationError::MissingNode(node_id) => {
                write!(f, "Missing node: '{}'", node_id)
            }
            ValidationError::MissingPin(node_id, pin_id) => {
                write!(f, "Missing pin '{}' on node '{}'", pin_id, node_id)
            }
            ValidationError::TypeMismatch(from_node, from_pin, to_node, to_pin, from_type, to_type) => {
                write!(
                    f,
                    "Type mismatch: '{}' (pin '{}' on '{}') cannot connect to '{}' (pin '{}' on '{}')",
                    from_type, from_pin, from_node, to_type, to_pin, to_node
                )
            }
        }
    }
}

impl std::error::Error for ValidationError {}
