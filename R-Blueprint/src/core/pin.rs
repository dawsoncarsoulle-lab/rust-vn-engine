//! Pin types for blueprint nodes

use serde::{Serialize, Deserialize};
use glam::Vec2;

/// Type of data a pin can carry
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "lowercase")]
pub enum PinKind {
    /// Execution flow (white triangle)
    Exec,
    /// Boolean value (red)
    Bool,
    /// Floating point number (green)
    Float,
    /// Integer (blue)
    Int,
    /// String/text (purple)
    String,
    /// Generic object/struct (light blue)
    Object,
    /// Any type (wildcard)
    Any,
}

impl PinKind {
    /// Get the color associated with this pin type
    pub fn color(&self) -> [f32; 4] {
        match self {
            PinKind::Exec => [0.95, 0.95, 0.95, 1.0],      // White
            PinKind::Bool => [0.76, 0.27, 0.30, 1.0],     // Red (#c1444d)
            PinKind::Float => [0.37, 0.82, 0.64, 1.0],     // Green (#5fd1a3)
            PinKind::Int => [0.22, 0.78, 0.85, 1.0],       // Blue (#39c6d9)
            PinKind::String => [0.90, 0.42, 0.41, 1.0],   // Purple (#e56bd6)
            PinKind::Object => [0.35, 0.58, 0.96, 1.0],    // Light Blue (#5a93f5)
            PinKind::Any => [0.80, 0.80, 0.80, 1.0],       // Gray
        }
    }

    /// Get the glow color for connected pins
    pub fn glow_color(&self) -> [f32; 4] {
        match self {
            PinKind::Exec => [1.0, 1.0, 1.0, 0.8],
            PinKind::Bool => [1.0, 0.4, 0.4, 0.8],
            PinKind::Float => [0.4, 1.0, 0.6, 0.8],
            PinKind::Int => [0.4, 0.8, 1.0, 0.8],
            PinKind::String => [1.0, 0.6, 0.8, 0.8],
            PinKind::Object => [0.6, 0.8, 1.0, 0.8],
            PinKind::Any => [0.9, 0.9, 0.9, 0.8],
        }
    }
}

/// Direction of a pin (input or output)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PinDirection {
    Input,
    Output,
}

/// A single pin/port on a node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pin {
    /// Unique identifier for this pin
    pub id: String,
    
    /// Display name for the pin
    pub label: String,
    
    /// Type of data this pin handles
    pub kind: PinKind,
    
    /// Direction (input or output)
    pub direction: PinDirection,
    
    /// Default value (for input pins)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_value: Option<String>,
    
    /// Position offset from node origin (in node-local coordinates)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Vec2>,
    
    /// Whether this pin is connected
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connected: Option<bool>,
}

impl Pin {
    /// Create a new pin
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        kind: PinKind,
        direction: PinDirection,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            kind,
            direction,
            default_value: None,
            position: None,
            connected: None,
        }
    }

    /// Create an execution pin (flow control)
    pub fn exec(id: impl Into<String>, direction: PinDirection) -> Self {
        Self::new(id, "", PinKind::Exec, direction)
    }

    /// Create a boolean pin
    pub fn bool(id: impl Into<String>, label: impl Into<String>, direction: PinDirection) -> Self {
        Self::new(id, label, PinKind::Bool, direction)
    }

    /// Create a float pin
    pub fn float(id: impl Into<String>, label: impl Into<String>, direction: PinDirection) -> Self {
        Self::new(id, label, PinKind::Float, direction)
    }

    /// Create an integer pin
    pub fn int(id: impl Into<String>, label: impl Into<String>, direction: PinDirection) -> Self {
        Self::new(id, label, PinKind::Int, direction)
    }

    /// Create a string pin
    pub fn string(id: impl Into<String>, label: impl Into<String>, direction: PinDirection) -> Self {
        Self::new(id, label, PinKind::String, direction)
    }

    /// Create an object pin
    pub fn object(id: impl Into<String>, label: impl Into<String>, direction: PinDirection) -> Self {
        Self::new(id, label, PinKind::Object, direction)
    }

    /// Check if this pin is an execution pin
    pub fn is_exec(&self) -> bool {
        self.kind == PinKind::Exec
    }

    /// Check if this pin is a data pin
    pub fn is_data(&self) -> bool {
        !self.is_exec()
    }
}

impl Default for Pin {
    fn default() -> Self {
        Self {
            id: String::new(),
            label: String::new(),
            kind: PinKind::Any,
            direction: PinDirection::Input,
            default_value: None,
            position: None,
            connected: None,
        }
    }
}
