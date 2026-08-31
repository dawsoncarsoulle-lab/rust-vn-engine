//! Canvas rendering for blueprints

use bevy::prelude::*;
use bevy::render::camera::RenderTarget;
use bevy::render::render_resource::{
    Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
};
use bevy::sprite::MaterialMesh2dBundle;
use crate::core::Blueprint;
use crate::rendering::theme::BlueprintTheme;

/// Component to mark the canvas entity
#[derive(Debug, Clone, Component)]
pub struct BlueprintCanvas {
    pub blueprint_id: String,
    pub is_panning: bool,
    pub pan_start: Vec2,
    pub pan_offset: Vec2,
}

impl Default for BlueprintCanvas {
    fn default() -> Self {
        Self {
            blueprint_id: String::new(),
            is_panning: false,
            pan_start: Vec2::ZERO,
            pan_offset: Vec2::ZERO,
        }
    }
}

/// Marker component for canvas background
#[derive(Debug, Clone, Component)]
pub struct CanvasBackground;

/// Marker component for grid
#[derive(Debug, Clone, Component)]
pub struct CanvasGrid;

/// Plugin for canvas rendering
pub struct CanvasRenderingPlugin;

impl Plugin for CanvasRenderingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_canvas)
            .add_systems(Update, (
                update_canvas_grid,
                handle_canvas_interactions,
            ));
    }
}

/// Setup the canvas
fn setup_canvas(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    theme: Res<BlueprintTheme>,
) {
    // Create canvas background
    let background_mesh = meshes.add(Rectangle::new(10000.0, 10000.0));
    
    commands.spawn((
        MaterialMesh2dBundle {
            mesh: background_mesh,
            material: materials::ColorMaterial::from(theme.background_canvas),
            transform: Transform::from_xyz(0.0, 0.0, -1000.0),
            ..default()
        },
        CanvasBackground,
        BlueprintCanvas::default(),
    ));
    
    // Create grid
    spawn_grid(&mut commands, &mut meshes, &theme);
}

/// Spawn the grid
fn spawn_grid(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    theme: &BlueprintTheme,
) {
    // Create a large grid mesh
    let grid_size = 10000.0;
    let cell_size = theme.grid_size;
    
    // For now, just create a simple quad for the grid
    // A proper implementation would create a grid mesh with lines
    let grid_mesh = meshes.add(Rectangle::new(grid_size, grid_size));
    
    commands.spawn((
        MaterialMesh2dBundle {
            mesh: grid_mesh,
            material: materials::ColorMaterial::from(theme.grid_color),
            transform: Transform::from_xyz(0.0, 0.0, -999.0),
            ..default()
        },
        CanvasGrid,
    ));
}

/// Update the canvas grid
fn update_canvas_grid(
    // Implementation would update grid based on camera position
) {
    // To be implemented
}

/// Handle canvas interactions (panning, zooming)
pub fn handle_canvas_interactions(
    mut camera_query: Query<(&mut Transform, &mut OrthographicProjection, &BlueprintCamera)>
    ,mouse_buttons: Res<Input<MouseButton>>
    ,mouse_wheel_events: Res<Events<MouseWheel>>
    ,windows: Query<&Window>
    ,mut canvas_query: Query<&mut BlueprintCanvas>
) {
    let Ok((mut transform, mut projection, camera)) = camera_query.get_single_mut() else {
        return;
    };
    
    let window = windows.single();
    
    for mut canvas in canvas_query.iter_mut() {
        // Handle panning
        if mouse_buttons.pressed(MouseButton::Middle) {
            if !canvas.is_panning {
                canvas.is_panning = true;
                if let Some(cursor_pos) = window.cursor_position() {
                    canvas.pan_start = cursor_pos;
                    canvas.pan_offset = transform.translation.truncate();
                }
            }
            
            if canvas.is_panning {
                if let Some(cursor_pos) = window.cursor_position() {
                    let delta = cursor_pos - canvas.pan_start;
                    transform.translation.x = canvas.pan_offset.x - delta.x * camera.pan_speed;
                    transform.translation.y = canvas.pan_offset.y - delta.y * camera.pan_speed;
                }
            }
        } else {
            canvas.is_panning = false;
        }
        
        // Handle zooming
        for event in mouse_wheel_events.read() {
            let scroll_amount = event.y;
            let zoom_factor = 1.0 + scroll_amount * camera.zoom_speed;
            let new_scale = projection.scale * zoom_factor;
            
            projection.scale = new_scale.clamp(camera.min_zoom, camera.max_zoom);
            
            if let Some(cursor_pos) = window.cursor_position() {
                let mouse_world_pos = cursor_pos - transform.translation.truncate();
                let zoom_ratio = zoom_factor;
                transform.translation.x += mouse_world_pos.x * (1.0 - zoom_ratio);
                transform.translation.y += mouse_world_pos.y * (1.0 - zoom_ratio);
            }
        }
    }
}

/// Spawn a complete blueprint on the canvas
pub fn spawn_blueprint(
    commands: &mut Commands,
    blueprint: &Blueprint,
    theme: &BlueprintTheme,
    meshes: &mut ResMut<Assets<Mesh>>,
    node_materials: &mut ResMut<Assets<crate::shaders::node::NodeMaterial>>,
    wire_materials: &mut ResMut<Assets<crate::shaders::wire::WireMaterial>>,
) {
    // Spawn all nodes
    for node in &blueprint.nodes {
        crate::rendering::node::spawn_node(
            commands,
            node,
            theme,
            meshes,
            node_materials,
        );
    }
    
    // Spawn all wires
    for wire in &blueprint.wires {
        // Find the positions of the nodes this wire connects
        let from_node = blueprint.find_node(&wire.from.0);
        let to_node = blueprint.find_node(&wire.to.0);
        
        if let (Some(from), Some(to)) = (from_node, to_node) {
            crate::rendering::wire::spawn_wire(
                commands,
                wire,
                from.position,
                to.position,
                Vec2::new(1.0, 0.0),
                Vec2::new(-1.0, 0.0),
                theme,
                wire_materials,
            );
        }
    }
}

/// Clear all nodes and wires from the canvas
pub fn clear_canvas(
    commands: &mut Commands,
    node_query: Query<Entity, With<crate::rendering::node::BlueprintNode>>,
    wire_query: Query<Entity, With<crate::rendering::wire::BlueprintWire>>,
) {
    for entity in node_query.iter() {
        commands.entity(entity).despawn_recursive();
    }
    
    for entity in wire_query.iter() {
        commands.entity(entity).despawn_recursive();
    }
}

/// Update the canvas to reflect changes in the blueprint
pub fn update_canvas(
    commands: &mut Commands,
    blueprint: &Blueprint,
    theme: &BlueprintTheme,
    meshes: &mut ResMut<Assets<Mesh>>,
    node_materials: &mut ResMut<Assets<crate::shaders::node::NodeMaterial>>,
    wire_materials: &mut ResMut<Assets<crate::shaders::wire::WireMaterial>>,
) {
    // Clear existing nodes and wires
    clear_canvas(commands, 
        commands.query::<Entity, With<crate::rendering::node::BlueprintNode>>(),
        commands.query::<Entity, With<crate::rendering::wire::BlueprintWire>>()
    );
    
    // Respawn everything
    spawn_blueprint(commands, blueprint, theme, meshes, node_materials, wire_materials);
}

// Helper trait for querying
pub trait QueryEntities<T: bevy::prelude::Component> {
    fn query(&self) -> Query<Entity, With<T>>;
}

impl QueryEntities<crate::rendering::node::BlueprintNode> for Commands {
    fn query(&self) -> Query<Entity, With<crate::rendering::node::BlueprintNode>> {
        panic!("Cannot create query from Commands directly");
    }
}
