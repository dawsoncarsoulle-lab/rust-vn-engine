//! Glass effect example
//! This demonstrates the Unreal Engine-style glass effect

use bevy::prelude::*;
use r_blueprint::shaders::*;
use r_blueprint::rendering::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        // Add glass effect plugin
        .add_plugins((
            GlassEffectPlugin,
            GlassMaterialPlugin,
            CameraPlugin,
        ))
        // Add systems
        .add_systems(Startup, setup_glass_example)
        .run();
}

/// Setup the glass effect example
fn setup_glass_example(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<GlassMaterial>>,
    asset_server: Res<AssetServer>,
) {
    // Spawn camera
    commands.spawn((
        Camera2dBundle::default(),
        BlueprintCamera,
    ));
    
    // Load a sample image for the screen texture
    // In a real application, this would be a render texture
    let screen_texture: Handle<Image> = asset_server.load("embedded://r_blueprint/textures/default.png");
    
    // Spawn a glass panel
    spawn_glass_panel_custom(
        &mut commands,
        &mut meshes,
        &mut materials,
        Vec2::new(400.0, 300.0),
        Vec2::new(0.0, 0.0),
        0.1,
        0.5,
        Color::rgba(0.8, 0.9, 1.0, 0.3),
        screen_texture,
    );
    
    // Spawn some background elements to show the glass effect
    commands.spawn((
        SpriteBundle {
            sprite: Sprite {
                color: Color::RED,
                custom_size: Some(Vec2::new(100.0, 100.0)),
                ..default()
            },
            transform: Transform::from_translation(Vec3::new(-200.0, 0.0, -1.0)),
            ..default()
        },
    ));
    
    commands.spawn((
        SpriteBundle {
            sprite: Sprite {
                color: Color::GREEN,
                custom_size: Some(Vec2::new(100.0, 100.0)),
                ..default()
            },
            transform: Transform::from_translation(Vec3::new(200.0, 0.0, -1.0)),
            ..default()
        },
    ));
}
