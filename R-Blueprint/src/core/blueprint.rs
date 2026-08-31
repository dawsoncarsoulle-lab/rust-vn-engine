//! Blueprint graph definition

use serde::{Serialize, Deserialize};
use glam::Vec2;
use crate::core::{Node, Wire};

/// A complete blueprint graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Blueprint {
    /// Unique identifier for this blueprint
    pub id: String,
    
    /// Display name
    pub name: String,
    
    /// Description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    
    /// All nodes in this blueprint
    pub nodes: Vec<Node>,
    
    /// All wires/connections in this blueprint
    pub wires: Vec<Wire>,
    
    /// Camera position (for viewport)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub camera_position: Option<Vec2>,
    
    /// Camera zoom level
    #[serde(skip_serializing_if = "Option::is_none")]
    pub camera_zoom: Option<f32>,
    
    /// Variables defined in this blueprint
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variables: Option<Vec<BlueprintVariable>>,
    
    /// Custom metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

impl Blueprint {
    /// Create a new empty blueprint
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: None,
            nodes: Vec::new(),
            wires: Vec::new(),
            camera_position: None,
            camera_zoom: None,
            variables: None,
            metadata: None,
        }
    }

    /// Add a node to the blueprint
    pub fn add_node(&mut self, node: Node) -> &mut Node {
        self.nodes.push(node);
        self.nodes.last_mut().unwrap()
    }

    /// Remove a node by ID
    pub fn remove_node(&mut self, node_id: &str) -> Option<Node> {
        if let Some(index) = self.nodes.iter().position(|n| n.id == node_id) {
            // Also remove any wires connected to this node
            self.wires.retain(|w| {
                w.from.0 != node_id && w.to.0 != node_id
            });
            Some(self.nodes.remove(index))
        } else {
            None
        }
    }

    /// Find a node by ID
    pub fn find_node(&self, node_id: &str) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id == node_id)
    }

    /// Find a node by ID (mutable)
    pub fn find_node_mut(&mut self, node_id: &str) -> Option<&mut Node> {
        self.nodes.iter_mut().find(|n| n.id == node_id)
    }

    /// Add a wire to the blueprint
    pub fn add_wire(&mut self, wire: Wire) -> &mut Wire {
        self.wires.push(wire);
        self.wires.last_mut().unwrap()
    }

    /// Remove a wire by ID
    pub fn remove_wire(&mut self, wire_id: &str) -> Option<Wire> {
        if let Some(index) = self.wires.iter().position(|w| w.id == wire_id) {
            // Mark the pins as disconnected
            let wire = self.wires.remove(index);
            if let Some(node) = self.find_node_mut(&wire.from.0) {
                if let Some(pin) = node.find_pin(&wire.from.1) {
                    // Note: We can't modify pin directly since it's a reference
                }
            }
            if let Some(node) = self.find_node_mut(&wire.to.0) {
                if let Some(pin) = node.find_pin(&wire.to.1) {
                    // Note: We can't modify pin directly since it's a reference
                }
            }
            Some(wire)
        } else {
            None
        }
    }

    /// Find a wire by ID
    pub fn find_wire(&self, wire_id: &str) -> Option<&Wire> {
        self.wires.iter().find(|w| w.id == wire_id)
    }

    /// Find all wires connected to a specific pin
    pub fn wires_for_pin(&self, node_id: &str, pin_id: &str) -> Vec<&Wire> {
        self.wires.iter()
            .filter(|w| {
                (w.from.0 == node_id && w.from.1 == pin_id) ||
                (w.to.0 == node_id && w.to.1 == pin_id)
            })
            .collect()
    }

    /// Find all wires connected to a node
    pub fn wires_for_node(&self, node_id: &str) -> Vec<&Wire> {
        self.wires.iter()
            .filter(|w| w.from.0 == node_id || w.to.0 == node_id)
            .collect()
    }

    /// Check if a connection would be valid
    pub fn is_connection_valid(
        &self,
        from_node: &str,
        from_pin: &str,
        to_node: &str,
        to_pin: &str,
    ) -> bool {
        // Can't connect to self
        if from_node == to_node {
            return false;
        }

        // Find the pins
        let from_node = self.find_node(from_node);
        let to_node = self.find_node(to_node);
        
        if from_node.is_none() || to_node.is_none() {
            return false;
        }

        let from_pin_obj = from_node.unwrap().find_pin(from_pin);
        let to_pin_obj = to_node.unwrap().find_pin(to_pin);
        
        if from_pin_obj.is_none() || to_pin_obj.is_none() {
            return false;
        }

        let from_pin = from_pin_obj.unwrap();
        let to_pin = to_pin_obj.unwrap();

        // Output must connect to input
        if from_pin.direction != crate::core::pin::PinDirection::Output {
            return false;
        }
        if to_pin.direction != crate::core::pin::PinDirection::Input {
            return false;
        }

        // Types must match (or be Any)
        if from_pin.kind != to_pin.kind {
            // Allow Any to connect to anything
            if from_pin.kind != crate::core::pin::PinKind::Any &&
               to_pin.kind != crate::core::pin::PinKind::Any {
                return false;
            }
        }

        // Check for existing connection
        self.wires.iter().any(|w| {
            w.from.0 == from_node && w.from.1 == from_pin &&
            w.to.0 == to_node && w.to.1 == to_pin
        })
        == false
    }

    /// Get all nodes at a specific position (for selection)
    pub fn nodes_at_position(&self, position: Vec2) -> Vec<&Node> {
        self.nodes.iter()
            .filter(|n| n.contains_point(position))
            .collect()
    }

    /// Calculate the bounds of all nodes
    pub fn bounds(&self) -> Option<(Vec2, Vec2)> {
        if self.nodes.is_empty() {
            return None;
        }
        
        let mut min_x = f32::INFINITY;
        let mut min_y = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut max_y = f32::NEG_INFINITY;
        
        for node in &self.nodes {
            let (node_min, node_max) = node.bounds();
            min_x = min_x.min(node_min.x);
            min_y = min_y.min(node_min.y);
            max_x = max_x.max(node_max.x);
            max_y = max_y.max(node_max.y);
        }
        
        Some((Vec2::new(min_x, min_y), Vec2::new(max_x, max_y)))
    }
}

impl Default for Blueprint {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            description: None,
            nodes: Vec::new(),
            wires: Vec::new(),
            camera_position: None,
            camera_zoom: None,
            variables: None,
            metadata: None,
        }
    }
}

/// A variable defined in a blueprint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlueprintVariable {
    /// Variable name
    pub name: String,
    
    /// Variable type
    pub type_name: String,
    
    /// Default value
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_value: Option<String>,
    
    /// Description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl BlueprintVariable {
    /// Create a new blueprint variable
    pub fn new(
        name: impl Into<String>,
        type_name: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            type_name: type_name.into(),
            default_value: None,
            description: None,
        }
    }
}
