//! Wire/connection types for blueprint graphs

use serde::{Serialize, Deserialize};
use glam::Vec2;
use crate::core::pin::PinKind;

/// A connection between two pins
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wire {
    /// Unique identifier for this wire
    pub id: String,
    
    /// Source pin (node_id, pin_id)
    pub from: (String, String),
    
    /// Target pin (node_id, pin_id)
    pub to: (String, String),
    
    /// Type of data flowing through this wire
    pub kind: PinKind,
    
    /// Control points for bezier curve (in world coordinates)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub control_points: Option<Vec<Vec2>>,
    
    /// Whether this wire is currently selected
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected: Option<bool>,
    
    /// Whether this wire is highlighted (e.g., during execution)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub highlighted: Option<bool>,
}

impl Wire {
    /// Create a new wire connection
    pub fn new(
        id: impl Into<String>,
        from: (impl Into<String>, impl Into<String>),
        to: (impl Into<String>, impl Into<String>),
        kind: PinKind,
    ) -> Self {
        Self {
            id: id.into(),
            from: (from.0.into(), from.1.into()),
            to: (to.0.into(), to.1.into()),
            kind,
            control_points: None,
            selected: None,
            highlighted: None,
        }
    }

    /// Get the source node ID
    pub fn from_node(&self) -> &str {
        &self.from.0
    }

    /// Get the source pin ID
    pub fn from_pin(&self) -> &str {
        &self.from.1
    }

    /// Get the target node ID
    pub fn to_node(&self) -> &str {
        &self.to.0
    }

    /// Get the target pin ID
    pub fn to_pin(&self) -> &str {
        &self.to.1
    }

    /// Check if this wire is an execution wire
    pub fn is_exec(&self) -> bool {
        self.kind == PinKind::Exec
    }

    /// Check if this wire connects the given pin
    pub fn connects_pin(&self, node_id: &str, pin_id: &str) -> bool {
        (self.from_node() == node_id && self.from_pin() == pin_id) ||
        (self.to_node() == node_id && self.to_pin() == pin_id)
    }
}

/// Wire path representation for rendering
#[derive(Debug, Clone)]
pub struct WirePath {
    /// Start point (world coordinates)
    pub start: Vec2,
    
    /// Control point 1
    pub cp1: Vec2,
    
    /// Control point 2
    pub cp2: Vec2,
    
    /// End point (world coordinates)
    pub end: Vec2,
    
    /// Type of wire
    pub kind: PinKind,
}

impl WirePath {
    /// Create a new wire path
    pub fn new(start: Vec2, cp1: Vec2, cp2: Vec2, end: Vec2, kind: PinKind) -> Self {
        Self { start, cp1, cp2, end, kind }
    }

    /// Calculate a smooth bezier curve path
    pub fn calculate_control_points(start: Vec2, end: Vec2, tension: f32) -> (Vec2, Vec2) {
        let dx = end.x - start.x;
        let dy = end.y - start.y;
        let distance = dx * dx + dy * dy;
        
        // For horizontal wires, use a simple curve
        if dy.abs() < 10.0 {
            let mid_x = start.x + dx * 0.5;
            let control_y = start.y - 50.0 * tension;
            return (
                Vec2::new(start.x + dx * 0.25, control_y),
                Vec2::new(end.x - dx * 0.25, control_y),
            );
        }
        
        // For vertical or diagonal wires
        let control_distance = distance.sqrt() * 0.3 * tension;
        let angle = dy.atan2(dx);
        
        // Perpendicular angle for control points
        let perp_angle = angle + std::f32::consts::PI / 2.0;
        
        let cp1 = Vec2::new(
            start.x + perp_angle.cos() * control_distance,
            start.y + perp_angle.sin() * control_distance,
        );
        
        let cp2 = Vec2::new(
            end.x + (perp_angle + std::f32::consts::PI).cos() * control_distance,
            end.y + (perp_angle + std::f32::consts::PI).sin() * control_distance,
        );
        
        (cp1, cp2)
    }
}
