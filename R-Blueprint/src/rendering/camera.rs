//! Camera controls for the blueprint editor

use bevy::prelude::*;
use bevy::input::mouse::MouseWheel;

/// Component for the blueprint camera
#[derive(Debug, Clone, Component)]
pub struct BlueprintCamera {
    pub min_zoom: f32,
    pub max_zoom: f32,
    pub zoom_speed: f32,
    pub pan_speed: f32,
}

impl Default for BlueprintCamera {
    fn default() -> Self {
        Self {
            min_zoom: 0.4,
            max_zoom: 1.8,
            zoom_speed: 0.0016,
            pan_speed: 1.0,
        }
    }
}

/// Plugin for blueprint camera controls
pub struct BlueprintCameraPlugin;

impl Plugin for BlueprintCameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_blueprint_camera)
            .add_systems(Update, (
                camera_zoom,
                camera_pan,
            ).chain());
    }
}

fn setup_blueprint_camera(
    mut commands: Commands,
) {
    commands.spawn((
        Camera2dBundle {
            transform: Transform::from_xyz(0.0, 0.0, 999.0),
            ..default()
        },
        BlueprintCamera::default(),
    ));
}

/// State for camera panning
#[derive(Debug, Default)]
pub struct CameraPanState {
    pub is_panning: bool,
    pub start_position: Vec2,
    pub start_translation: Vec2,
}

/// Resource for camera state
#[derive(Debug, Default, Resource)]
pub struct CameraState {
    pub pan_state: CameraPanState,
    pub last_mouse_position: Vec2,
}

fn camera_zoom(
    mut camera_query: Query<(&mut OrthographicProjection, &mut Transform, &BlueprintCamera)>
    ,mouse_wheel_events: Res<Events<MouseWheel>>
    ,windows: Query<&Window>
    ,camera_state: ResMut<CameraState>
) {
    let Ok((mut projection, mut transform, camera)) = camera_query.get_single_mut() else {
        return;
    };

    let window = windows.single();
    
    for event in mouse_wheel_events.read() {
        let scroll_amount = event.y;
        
        // Calculate zoom factor
        let zoom_factor = 1.0 + scroll_amount * camera.zoom_speed;
        let new_scale = projection.scale * zoom_factor;
        
        // Clamp zoom
        projection.scale = new_scale.clamp(camera.min_zoom, camera.max_zoom);
        
        // Get mouse position in world coordinates before zoom
        if let Some(cursor_pos) = window.cursor_position() {
            let mouse_world_pos = cursor_pos - transform.translation.truncate();
            
            // Adjust translation to zoom toward mouse position
            let zoom_ratio = zoom_factor;
            transform.translation.x += mouse_world_pos.x * (1.0 - zoom_ratio);
            transform.translation.y += mouse_world_pos.y * (1.0 - zoom_ratio);
        }
    }
}

fn camera_pan(
    mut camera_query: Query<(&mut Transform, &BlueprintCamera)>
    ,mouse_buttons: Res<Input<MouseButton>>
    ,windows: Query<&Window>
    ,mut camera_state: ResMut<CameraState>
) {
    let Ok((mut transform, camera)) = camera_query.get_single_mut() else {
        return;
    };
    
    let window = windows.single();
    
    if let Some(cursor_pos) = window.cursor_position() {
        camera_state.last_mouse_position = cursor_pos;
        
        // Start panning on middle mouse button or space + left mouse
        if mouse_buttons.just_pressed(MouseButton::Middle) ||
           (mouse_buttons.just_pressed(MouseButton::Left) && 
            keyboard_pressed!(Input<KeyCode>, KeyCode::Space)) {
            camera_state.pan_state.is_panning = true;
            camera_state.pan_state.start_position = cursor_pos;
            camera_state.pan_state.start_translation = transform.translation.truncate();
        }
        
        // End panning
        if mouse_buttons.just_released(MouseButton::Middle) ||
           mouse_buttons.just_released(MouseButton::Left) {
            camera_state.pan_state.is_panning = false;
        }
        
        // Pan camera
        if camera_state.pan_state.is_panning {
            let delta = cursor_pos - camera_state.pan_state.start_position;
            transform.translation.x = camera_state.pan_state.start_translation.x - delta.x * camera.pan_speed;
            transform.translation.y = camera_state.pan_state.start_translation.y - delta.y * camera.pan_speed;
        }
    }
}

/// Helper trait for checking keyboard input
trait KeyboardPressed {
    fn pressed(&self, key: KeyCode) -> bool;
}

impl KeyboardPressed for Res<'_, Input<KeyCode>> {
    fn pressed(&self, key: KeyCode) -> bool {
        self.pressed(key)
    }
}

macro_rules! keyboard_pressed {
    ($input:expr, $key:expr) => {
        $input.pressed($key)
    };
}
