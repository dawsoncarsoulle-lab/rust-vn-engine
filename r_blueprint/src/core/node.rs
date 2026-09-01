//! Node types for blueprint graphs

use serde::{Serialize, Deserialize};
use glam::Vec2;
use crate::core::pin::Pin;

/// Type/category of a node
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "lowercase")]
pub enum NodeType {
    /// Event nodes (start, triggers, etc.)
    Event,
    /// Function call nodes
    Function,
    /// Flow control nodes (if, branch, etc.)
    Branch,
    /// Math operation nodes
    Math,
    /// Variable access nodes
    Variable,
    /// Constant/literal value nodes
    Constant,
    /// Custom/composite nodes
    Custom,
    /// Comment/annotation nodes
    Comment,
}

impl NodeType {
    /// Get the header gradient colors for this node type
    pub fn header_colors(&self) -> ([f32; 4], [f32; 4]) {
        match self {
            NodeType::Event => (
                [0.63, 0.19, 0.21, 1.0],   // #a13139
                [0.44, 0.12, 0.15, 1.0],   // #701f26
            ),
            NodeType::Function => (
                [0.13, 0.39, 0.67, 1.0],   // #2064ab
                [0.09, 0.29, 0.53, 1.0],   // #164a86
            ),
            NodeType::Branch => (
                [0.27, 0.31, 0.38, 1.0],   // #454f61
                [0.18, 0.22, 0.28, 1.0],   // #2f3846
            ),
            NodeType::Math => (
                [0.12, 0.56, 0.47, 1.0],   // #1f8f79
                [0.08, 0.41, 0.35, 1.0],   // #14685a
            ),
            NodeType::Variable => (
                [0.42, 0.58, 0.96, 1.0],   // #6b91f5
                [0.28, 0.45, 0.80, 1.0],   // #4773cc
            ),
            NodeType::Constant => (
                [0.55, 0.65, 0.75, 1.0],
                [0.40, 0.50, 0.60, 1.0],
            ),
            NodeType::Custom => (
                [0.35, 0.35, 0.35, 1.0],
                [0.20, 0.20, 0.20, 1.0],
            ),
            NodeType::Comment => (
                [0.30, 0.30, 0.30, 0.8],
                [0.20, 0.20, 0.20, 0.8],
            ),
        }
    }

    /// Get the icon for this node type
    pub fn icon(&self) -> NodeIcon {
        match self {
            NodeType::Event => NodeIcon::Triangle,
            NodeType::Function => NodeIcon::Dot,
            NodeType::Branch => NodeIcon::Diamond,
            NodeType::Math => NodeIcon::Plus,
            NodeType::Variable => NodeIcon::Circle,
            NodeType::Constant => NodeIcon::Square,
            NodeType::Custom => NodeIcon::Custom,
            NodeType::Comment => NodeIcon::None,
        }
    }
}

/// Icon type for node header
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeIcon {
    None,
    Triangle,
    Dot,
    Diamond,
    Plus,
    Circle,
    Square,
    Custom,
}

/// A node in the blueprint graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    /// Unique identifier for this node
    pub id: String,
    
    /// Display title
    pub title: String,
    
    /// Type/category of the node
    pub node_type: NodeType,
    
    /// Icon to display in the header
    pub icon: NodeIcon,
    
    /// Position in world coordinates
    pub position: Vec2,
    
    /// Size of the node (width, height)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<Vec2>,
    
    /// Input pins (left side)
    pub inputs: Vec<Pin>,
    
    /// Output pins (right side)
    pub outputs: Vec<Pin>,
    
    /// Whether this node is currently selected
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected: Option<bool>,
    
    /// Whether this node is highlighted
    #[serde(skip_serializing_if = "Option::is_none")]
    pub highlighted: Option<bool>,
    
    /// Custom data associated with this node
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    
    /// For variable nodes: the variable name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variable_name: Option<String>,
    
    /// For variable nodes: the variable type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variable_type: Option<String>,
}

impl Node {
    /// Create a new node
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        node_type: NodeType,
        position: Vec2,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            node_type,
            icon: node_type.icon(),
            position,
            size: None,
            inputs: Vec::new(),
            outputs: Vec::new(),
            selected: None,
            highlighted: None,
            data: None,
            variable_name: None,
            variable_type: None,
        }
    }

    /// Create an event node
    pub fn event(id: impl Into<String>, title: impl Into<String>, position: Vec2) -> Self {
        Self::new(id, title, NodeType::Event, position)
    }

    /// Create a function node
    pub fn function(id: impl Into<String>, title: impl Into<String>, position: Vec2) -> Self {
        Self::new(id, title, NodeType::Function, position)
    }

    /// Create a branch node
    pub fn branch(id: impl Into<String>, title: impl Into<String>, position: Vec2) -> Self {
        Self::new(id, title, NodeType::Branch, position)
    }

    /// Create a math node
    pub fn math(id: impl Into<String>, title: impl Into<String>, position: Vec2) -> Self {
        Self::new(id, title, NodeType::Math, position)
    }

    /// Create a variable node
    pub fn variable(
        id: impl Into<String>,
        title: impl Into<String>,
        position: Vec2,
        var_name: impl Into<String>,
        var_type: impl Into<String>,
    ) -> Self {
        let mut node = Self::new(id, title, NodeType::Variable, position);
        node.variable_name = Some(var_name.into());
        node.variable_type = Some(var_type.into());
        node
    }

    /// Add an input pin
    pub fn with_input(mut self, pin: Pin) -> Self {
        self.inputs.push(pin);
        self
    }

    /// Add an output pin
    pub fn with_output(mut self, pin: Pin) -> Self {
        self.outputs.push(pin);
        self
    }

    /// Add multiple input pins
    pub fn with_inputs(mut self, pins: impl IntoIterator<Item = Pin>) -> Self {
        self.inputs.extend(pins);
        self
    }

    /// Add multiple output pins
    pub fn with_outputs(mut self, pins: impl IntoIterator<Item = Pin>) -> Self {
        self.outputs.extend(pins);
        self
    }

    /// Get all pins (inputs and outputs)
    pub fn all_pins(&self) -> impl Iterator<Item = &Pin> {
        self.inputs.iter().chain(self.outputs.iter())
    }

    /// Find a pin by ID
    pub fn find_pin(&self, pin_id: &str) -> Option<&Pin> {
        self.all_pins().find(|p| p.id == pin_id)
    }

    /// Check if this node has any connections
    pub fn has_connections(&self) -> bool {
        self.inputs.iter().any(|p| p.connected == Some(true)) ||
        self.outputs.iter().any(|p| p.connected == Some(true))
    }

    /// Calculate the node's bounds
    pub fn bounds(&self) -> (Vec2, Vec2) {
        let size = self.size.unwrap_or(Vec2::new(150.0, 80.0));
        let min = self.position;
        let max = self.position + size;
        (min, max)
    }

    /// Check if a point is inside the node
    pub fn contains_point(&self, point: Vec2) -> bool {
        let (min, max) = self.bounds();
        point.x >= min.x && point.x <= max.x && point.y >= min.y && point.y <= max.y
    }
}

impl Default for Node {
    fn default() -> Self {
        Self {
            id: String::new(),
            title: String::new(),
            node_type: NodeType::Custom,
            icon: NodeIcon::None,
            position: Vec2::ZERO,
            size: None,
            inputs: Vec::new(),
            outputs: Vec::new(),
            selected: None,
            highlighted: None,
            data: None,
            variable_name: None,
            variable_type: None,
        }
    }
}
