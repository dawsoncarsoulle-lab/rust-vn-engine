//! Wire rendering for blueprints

use bevy::prelude::*;
use crate::core::{Wire, WirePath};
use crate::rendering::theme::BlueprintTheme;
use crate::shaders::wire::{WireMaterial, WireMaterialPlugin, create_wire_material, create_wire_mesh};

/// Component to mark an entity as a blueprint wire
#[derive(Debug, Clone, Component)]
pub struct BlueprintWire {
    pub wire_id: String,
    pub start_pin: String,
    pub end_pin: String,
}

/// Component for wire visual state
#[derive(Debug, Clone, Component)]
pub struct WireVisualState {
    pub is_selected: bool,
    pub is_highlighted: bool,
    pub is_hovered: bool,
}

impl Default for WireVisualState {
    fn default() -> Self {
        Self {
            is_selected: false,
            is_highlighted: false,
            is_hovered: false,
        }
    }
}

/// Plugin for wire rendering
pub struct WireRenderingPlugin;

impl Plugin for WireRenderingPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(WireMaterialPlugin)
            .add_systems(Startup, setup_wire_materials)
            .add_systems(Update, update_wire_visuals);
    }
}

/// System to setup wire materials
fn setup_wire_materials(mut commands: Commands) {
    // Wire materials are created dynamically
}

/// Spawn a wire between two points
pub fn spawn_wire(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<WireMaterial>>,
    wire: &Wire,
    start_pos: Vec2,
    end_pos: Vec2,
    theme: &BlueprintTheme,
) -> Entity {
    let wire_id = wire.id.clone();
    
    // Get wire color based on pin type or use default
    let wire_color = theme.wire_color;
    
    // Create wire material
    let wire_material = create_wire_material(wire_color, theme.wire_thickness, 0.5, false);
    let material_handle = materials.add(wire_material);
    
    // Calculate control points for bezier curve
    let distance = start_pos.distance(end_pos);
    let control_offset = distance * 0.3;
    
    let control1 = start_pos + Vec2::new(control_offset, 0.0);
    let control2 = end_pos - Vec2::new(control_offset, 0.0);
    
    // Create wire mesh
    let wire_mesh = create_wire_mesh(start_pos, end_pos, control1, control2, theme.wire_thickness, 32);
    let mesh_handle = meshes.add(wire_mesh);
    
    // Spawn wire entity
    commands.spawn((
        MaterialMeshBundle {
            mesh: mesh_handle,
            material: material_handle,
            ..default()
        },
        BlueprintWire {
            wire_id: wire_id.clone(),
            start_pin: wire.from_pin.clone(),
            end_pin: wire.to_pin.clone(),
        },
        WireVisualState::default(),
        Name::new(format!("Wire_{}", wire_id)),
    )).id()
}

/// Update wire visuals based on pin positions
pub fn update_wire_visuals(
    mut wire_query: Query<(&BlueprintWire, &mut Transform, &mut Handle<WireMaterial>, &WireVisualState)>, 
    pin_query: Query<(&BlueprintPin, &Transform)>, 
    materials: ResMut<Assets<WireMaterial>>,
    theme: Res<BlueprintTheme>,
) {
    for (wire, mut transform, material_handle, visual_state) in &mut wire_query {
        // Find start and end pin positions
        let mut start_pos = Vec2::ZERO;
        let mut end_pos = Vec2::ZERO;
        
        for (pin, pin_transform) in &pin_query {
            if pin.pin_id == wire.start_pin {
                start_pos = pin_transform.translation.truncate();
            } else if pin.pin_id == wire.end_pin {
                end_pos = pin_transform.translation.truncate();
            }
        }
        
        // Recreate wire mesh with new positions
        // (In a real implementation, you'd update the mesh data here)
        // For now, we just update the transform to center between pins
        let center = (start_pos + end_pos) / 2.0;
        transform.translation = center.extend(0.0);
        
        // Update material based on visual state
        if let Some(material) = materials.get_mut(material_handle) {
            let glow = if visual_state.is_highlighted { 1.0 } else { 0.5 };
            material.glow_intensity = glow;
            material.is_highlighted = if visual_state.is_highlighted { 1.0 } else { 0.0 };
        }
    }
}

/// Spawn a wire between two pins
pub fn spawn_wire_between_pins(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<WireMaterial>>,
    wire: Wire,
    start_pin_pos: Vec2,
    end_pin_pos: Vec2,
    theme: &BlueprintTheme,
) -> Entity {
    spawn_wire(commands, meshes, materials, &wire, start_pin_pos, end_pin_pos, theme)
}

/// Update wire positions when pins move
pub fn update_wire_positions(
    wire_query: Query<(Entity, &BlueprintWire)>, 
    pin_query: Query<(Entity, &BlueprintPin, &Transform)>, 
    mut wire_meshes: ResMut<Assets<Mesh>>,
    theme: Res<BlueprintTheme>,
) {
    // This system would update wire meshes when pins move
    // Implementation would involve recreating the bezier curve meshes
}

/// System to handle wire selection
pub fn handle_wire_selection(
    mut commands: Commands,
    mouse_button_input: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    cameras: Query<(&Camera, &GlobalTransform)>,
    wire_query: Query<(Entity, &BlueprintWire, &Transform)>, 
    mut selected_wire: Option<ResMut<SelectedWire>>,
) {
    if mouse_button_input.just_pressed(MouseButton::Left) {
        let window = windows.single();
        if let Some(cursor_position) = window.cursor_position() {
            for (camera, camera_transform) in &cameras {
                if let Some(ray) = camera.viewport_to_world(camera_transform, cursor_position) {
                    for (entity, wire, transform) in &wire_query {
                        // Simple line segment check for wire selection
                        // This is a simplified version - real implementation would use proper line intersection
                        let wire_ray = ray.truncate();
                        
                        // For now, just check distance to wire center
                        let distance = wire_ray.distance(transform.translation.truncate());
                        if distance < 10.0 { // Selection threshold
                            commands.entity(entity).insert(WireVisualState {
                                is_selected: true,
                                is_highlighted: false,
                                is_hovered: false,
                            });
                            
                            if let Some(mut selected) = selected_wire {
                                selected.0 = wire.wire_id.clone();
                            } else {
                                commands.insert_resource(SelectedWire(wire.wire_id.clone()));
                            }
                            return;
                        }
                    }
                }
            }
        }
    }
}

/// Component for selected wire
#[derive(Resource)]
pub struct SelectedWire(pub String);

/// System to highlight wires on hover
pub fn highlight_wires_on_hover(
    mut wire_query: Query<(&mut WireVisualState, &Transform)>, 
    mouse_position: Res<MousePosition>,
) {
    let mouse_pos = Vec2::new(mouse_position.x, mouse_position.y);
    
    for (mut visual_state, transform) in &mut wire_query {
        let distance = mouse_pos.distance(transform.translation.truncate());
        visual_state.is_hovered = distance < 15.0;
        visual_state.is_highlighted = visual_state.is_hovered || visual_state.is_selected;
    }
}

/// Component for mouse position
#[derive(Resource)]
pub struct MousePosition {
    pub x: f32,
    pub y: f32,
}

/// System to track mouse position
pub fn track_mouse_position(
    windows: Query<&Window>,
    cameras: Query<(&Camera, &GlobalTransform)>,
    mut mouse_position: ResMut<MousePosition>,
) {
    let window = windows.single();
    if let Some(cursor_position) = window.cursor_position() {
        for (camera, camera_transform) in &cameras {
            if let Some(ray) = camera.viewport_to_world(camera_transform, cursor_position) {
                mouse_position.x = ray.x;
                mouse_position.y = ray.y;
                return;
            }
        }
    }
}
