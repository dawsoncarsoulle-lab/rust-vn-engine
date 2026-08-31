//! Wire rendering for blueprints

use bevy::prelude::*;
use glam::Vec2;
use crate::core::{Wire, PinKind, PinDirection};
use crate::rendering::theme::BlueprintTheme;
use crate::shaders::wire::{WireMaterial, create_wire_mesh, calculate_wire_control_points};

/// Component to mark an entity as a wire
#[derive(Debug, Clone, Component)]
pub struct BlueprintWire {
    pub wire_id: String,
    pub from: (String, String),
    pub to: (String, String),
    pub kind: PinKind,
    pub is_selected: bool,
    pub is_highlighted: bool,
    pub control_points: Vec<Vec2>,
}

/// Marker component for wire start point
#[derive(Debug, Clone, Component)]
pub struct WireStart {
    pub node_id: String,
    pub pin_id: String,
}

/// Marker component for wire end point
#[derive(Debug, Clone, Component)]
pub struct WireEnd {
    pub node_id: String,
    pub pin_id: String,
}

/// Plugin for wire rendering
pub struct WireRenderingPlugin;

impl Plugin for WireRenderingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_wire_resources)
            .add_systems(Update, (
                update_wire_positions,
                update_wire_visuals,
            ));
    }
}

fn setup_wire_resources(
    mut commands: Commands,
) {
    // Setup wire resources
}

/// Spawn a wire entity
pub fn spawn_wire(
    commands: &mut Commands,
    wire: &Wire,
    from_position: Vec2,
    to_position: Vec2,
    from_tangent: Vec2,
    to_tangent: Vec2,
    theme: &BlueprintTheme,
    materials: &mut ResMut<Assets<WireMaterial>>,
) -> Entity {
    // Calculate control points
    let (cp1, cp2) = calculate_wire_control_points(
        from_position,
        to_position,
        from_tangent,
        to_tangent,
        0.3,
    );
    
    // Get wire color and thickness
    let color = theme.pin_colors.get(&wire.kind).cloned().unwrap_or(Color::WHITE);
    let glow_color = theme.pin_glow_colors.get(&wire.kind).cloned().unwrap_or(Color::WHITE);
    let thickness = if wire.kind == PinKind::Exec {
        theme.wire_thickness_exec
    } else {
        theme.wire_thickness
    };
    
    // Create the wire mesh
    let mesh = create_wire_mesh(
        from_position,
        cp1,
        cp2,
        to_position,
        thickness,
        20, // segments
    );
    
    // Create the material
    let material = materials.add(WireMaterial {
        wire_color: color,
        glow_color,
        glow_intensity: if wire.highlighted == Some(true) { 1.0 } else { 0.0 },
        thickness,
    });
    
    // Spawn the wire entity
    commands.spawn((
        MaterialMesh2dBundle {
            mesh: mesh.into(),
            material,
            transform: Transform::default(),
            ..default()
        },
        BlueprintWire {
            wire_id: wire.id.clone(),
            from: wire.from.clone(),
            to: wire.to.clone(),
            kind: wire.kind,
            is_selected: wire.selected == Some(true),
            is_highlighted: wire.highlighted == Some(true),
            control_points: vec![cp1, cp2],
        },
    )).id()
}

/// Update wire positions when nodes move
pub fn update_wire_positions(
    mut wire_query: Query<(&mut BlueprintWire, &mut Transform, &mut Handle<Mesh>)>,
    node_query: Query<(&crate::rendering::node::BlueprintNode, &Transform, &crate::rendering::node::NodeVisualState)>,
    pin_query: Query<&crate::rendering::node::BlueprintPin>,
    mut meshes: ResMut<Assets<Mesh>>,
    theme: Res<BlueprintTheme>,
) {
    for (mut wire, mut transform, mesh_handle) in wire_query.iter_mut() {
        // Find the positions of the pins this wire connects to
        if let (Some(from_node), Some(to_node)) = (
            find_pin_position(&wire.from, &node_query, &pin_query),
            find_pin_position(&wire.to, &node_query, &pin_query),
        ) {
            // Calculate tangents based on pin direction
            let from_tangent = Vec2::new(1.0, 0.0); // Default tangent
            let to_tangent = Vec2::new(-1.0, 0.0); // Default tangent
            
            // Calculate control points
            let (cp1, cp2) = calculate_wire_control_points(
                from_node,
                to_node,
                from_tangent,
                to_tangent,
                0.3,
            );
            
            // Update control points
            wire.control_points = vec![cp1, cp2];
            
            // Recreate the mesh with new positions
            let thickness = if wire.kind == PinKind::Exec {
                theme.wire_thickness_exec
            } else {
                theme.wire_thickness
            };
            
            let mesh = create_wire_mesh(
                from_node,
                cp1,
                cp2,
                to_node,
                thickness,
                20,
            );
            
            if let Some(mesh_asset) = meshes.get_mut(mesh_handle) {
                *mesh_asset = mesh;
            }
        }
    }
}

/// Find the world position of a pin
fn find_pin_position(
    pin_ref: &(String, String),
    node_query: &Query<(&crate::rendering::node::BlueprintNode, &Transform, &crate::rendering::node::NodeVisualState)>,
    pin_query: &Query<&crate::rendering::node::BlueprintPin>,
) -> Option<Vec2> {
    // Find the node
    for (node, transform, _) in node_query.iter() {
        if node.node_id == pin_ref.0 {
            // Find the pin in this node
            // In practice, we'd need to store pin positions on the node
            // For now, return the node position as a placeholder
            return Some(transform.translation.truncate());
        }
    }
    None
}

/// Update wire visuals (selection, highlighting)
pub fn update_wire_visuals(
    mut wire_query: Query<(&BlueprintWire, &mut Handle<WireMaterial>)>,
    mut materials: ResMut<Assets<WireMaterial>>,
) {
    for (wire, material_handle) in wire_query.iter_mut() {
        if let Some(material) = materials.get_mut(material_handle) {
            let glow_intensity = if wire.is_highlighted {
                1.0
            } else if wire.is_selected {
                0.5
            } else {
                0.0
            };
            
            material.glow_intensity = glow_intensity;
        }
    }
}

/// Create a wire with pulse effect (for execution visualization)
pub fn create_pulse_wire(
    commands: &mut Commands,
    wire: &Wire,
    theme: &BlueprintTheme,
    materials: &mut ResMut<Assets<WireMaterial>>,
) {
    // This would create a wire with an animated pulse effect
    // Implementation would involve creating a separate entity for the pulse
    // that moves along the wire path
}

/// Calculate the direction from a node for wire routing
pub fn calculate_wire_direction(
    node_position: Vec2,
    pin_position: Vec2,
    pin_direction: PinDirection,
) -> Vec2 {
    match pin_direction {
        PinDirection::Input => Vec2::new(-1.0, 0.0),
        PinDirection::Output => Vec2::new(1.0, 0.0),
    }
}
