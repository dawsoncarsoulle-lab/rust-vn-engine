//! Canvas rendering for blueprints

use bevy::prelude::*;
use crate::rendering::theme::BlueprintTheme;
use crate::shaders::glass::{GlassMaterial, GlassMaterialPlugin, spawn_glass_panel};

/// Marker component for the blueprint canvas
#[derive(Debug, Clone, Component)]
pub struct BlueprintCanvas;

/// Plugin for canvas rendering
pub struct CanvasPlugin;

impl Plugin for CanvasPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(GlassMaterialPlugin)
            .add_systems(Startup, setup_canvas)
            .add_systems(Update, update_canvas);
    }
}

/// Setup the canvas
fn setup_canvas(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    theme: Res<BlueprintTheme>,
) {
    // Spawn background
    commands.spawn((
        MaterialMeshBundle {
            mesh: meshes.add(Mesh::from(Rectangle::new(10000.0, 10000.0))),
            material: materials.add(ColorMaterial::from(theme.background_canvas)),
            ..default()
        },
        BlueprintCanvas,
        Name::new("Canvas_Background"),
    ));
    
    // Spawn grid
    spawn_grid(&mut commands, &mut meshes, &mut materials, &theme);
}

/// Spawn a grid on the canvas
fn spawn_grid(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<ColorMaterial>>,
    theme: &BlueprintTheme,
) {
    // Create grid lines
    let grid_size = 50.0;
    let grid_extent = 5000.0;
    
    // Vertical lines
    for x in (-grid_extent as i32..=grid_extent as i32).step_by(grid_size as usize) {
        let x_pos = x as f32;
        commands.spawn((
            MaterialMeshBundle {
                mesh: meshes.add(Mesh::from(Line2d::new(Vec2::new(x_pos, -grid_extent), Vec2::new(x_pos, grid_extent)))),
                material: materials.add(ColorMaterial::from(theme.grid_color)),
                ..default()
            },
            ChildOf(commands.spawn((SpatialBundle::default(), BlueprintCanvas)).id()),
        ));
    }
    
    // Horizontal lines
    for y in (-grid_extent as i32..=grid_extent as i32).step_by(grid_size as usize) {
        let y_pos = y as f32;
        commands.spawn((
            MaterialMeshBundle {
                mesh: meshes.add(Mesh::from(Line2d::new(Vec2::new(-grid_extent, y_pos), Vec2::new(grid_extent, y_pos)))),
                material: materials.add(ColorMaterial::from(theme.grid_color)),
                ..default()
            },
            ChildOf(commands.spawn((SpatialBundle::default(), BlueprintCanvas)).id()),
        ));
    }
}

/// Update canvas based on camera
fn update_canvas(
    mut canvas_query: Query<&mut Transform, With<BlueprintCanvas>>,
    camera_query: Query<&Transform, With<BlueprintCamera>>,
) {
    // Update canvas position to match camera (for infinite canvas effect)
    // This is a placeholder - actual implementation would be more complex
}

/// Spawn a glass panel for the canvas
pub fn spawn_canvas_glass_panel(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<GlassMaterial>>,
    theme: &BlueprintTheme,
    size: Vec2,
    position: Vec2,
    screen_texture: Handle<Image>,
) {
    spawn_glass_panel(commands, meshes, materials, size, position, screen_texture);
}

/// Handle canvas interactions (panning, zooming)
pub fn handle_canvas_interactions(
    mut camera_query: Query<(&mut Transform, &mut OrthographicProjection), With<BlueprintCamera>>,
    mouse_button_input: Res<ButtonInput<MouseButton>>,
    mouse_wheel_events: Res<Events<MouseWheel>>,
    windows: Query<&Window>,
) {
    for (mut transform, mut projection) in &mut camera_query {
        let window = windows.single();
        
        // Handle mouse wheel zoom
        let mut zoom_factor = 1.0;
        for event in mouse_wheel_events.read() {
            zoom_factor *= 1.0 - event.y * 0.001;
        }
        
        if zoom_factor != 1.0 {
            projection.scale /= zoom_factor;
            projection.scale = projection.scale.clamp(0.1, 10.0);
        }
    }
}
