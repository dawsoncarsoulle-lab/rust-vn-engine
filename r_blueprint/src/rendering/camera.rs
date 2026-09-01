//! Camera controls for blueprint editor

use bevy::prelude::*;

/// Marker component for the blueprint camera
#[derive(Debug, Clone, Component)]
pub struct BlueprintCamera;

/// Plugin for camera controls
pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_camera)
            .add_systems(Update, camera_movement);
    }
}

/// Setup the initial camera
fn setup_camera(mut commands: Commands) {
    commands.spawn((
        Camera2dBundle {
            transform: Transform::from_translation(Vec3::new(0.0, 0.0, 1000.0)),
            ..default()
        },
        BlueprintCamera,
    ));
}

/// System for camera movement (pan and zoom)
fn camera_movement(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mouse_button_input: Res<ButtonInput<MouseButton>>,
    mouse_wheel_events: Res<Events<MouseWheel>>,
    mut camera_query: Query<(&mut Transform, &mut OrthographicProjection), With<BlueprintCamera>>,
    windows: Query<&Window>,
) {
    for (mut transform, mut projection) in &mut camera_query {
        let window = windows.single();
        
        // Pan with arrow keys or WASD
        let mut pan_direction = Vec3::ZERO;
        
        if keyboard_input.pressed(KeyCode::ArrowLeft) || keyboard_input.pressed(KeyCode::KeyA) {
            pan_direction.x -= 1.0;
        }
        if keyboard_input.pressed(KeyCode::ArrowRight) || keyboard_input.pressed(KeyCode::KeyD) {
            pan_direction.x += 1.0;
        }
        if keyboard_input.pressed(KeyCode::ArrowUp) || keyboard_input.pressed(KeyCode::KeyW) {
            pan_direction.y += 1.0;
        }
        if keyboard_input.pressed(KeyCode::ArrowDown) || keyboard_input.pressed(KeyCode::KeyS) {
            pan_direction.y -= 1.0;
        }
        
        if pan_direction.length_squared() > 0.0 {
            let pan_speed = 500.0;
            transform.translation += pan_direction.normalize() * pan_speed * 0.016; // Assuming 60 FPS
        }
        
        // Pan with middle mouse button
        if mouse_button_input.pressed(MouseButton::Middle) {
            if let Some(cursor_position) = window.cursor_position() {
                // This would require storing previous cursor position for smooth panning
                // For now, we'll just use a simple approach
            }
        }
        
        // Zoom with mouse wheel
        let mut zoom_factor = 1.0;
        for event in mouse_wheel_events.read() {
            zoom_factor *= 1.0 - event.y * 0.001;
        }
        
        if zoom_factor != 1.0 {
            projection.scale /= zoom_factor;
            // Clamp scale to reasonable values
            projection.scale = projection.scale.clamp(0.1, 10.0);
        }
        
        // Reset camera with space bar
        if keyboard_input.just_pressed(KeyCode::Space) {
            transform.translation = Vec3::new(0.0, 0.0, 1000.0);
            projection.scale = 1.0;
        }
    }
}

/// System to track cursor position for panning
pub fn track_cursor_position(
    windows: Query<&Window>,
    mut cursor_position: ResMut<CursorPosition>,
) {
    let window = windows.single();
    if let Some(pos) = window.cursor_position() {
        cursor_position.x = pos.x;
        cursor_position.y = pos.y;
    }
}

/// Resource to track cursor position
#[derive(Resource)]
pub struct CursorPosition {
    pub x: f32,
    pub y: f32,
}

impl Default for CursorPosition {
    fn default() -> Self {
        Self { x: 0.0, y: 0.0 }
    }
}
