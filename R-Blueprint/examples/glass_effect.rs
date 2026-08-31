//! Glass effect example
//! Demonstrates the Unreal Engine-style glass effect

use bevy::prelude::*;
use r_blueprint::rendering::*;
use r_blueprint::shaders::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        // Add blueprint plugins
        .add_plugins((
            BlueprintThemePlugin,
            BlueprintCameraPlugin,
            GlassEffectRenderingPlugin,
            GlassEffectPipelinePlugin,
        ))
        // Add systems
        .add_systems(Startup, setup_glass_example)
        .add_systems(Update, (
            animate_glass_panels,
        ))
        .run();
}

/// Setup the glass effect example
fn setup_glass_example(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<GlassMaterial>>,
    theme: Res<BlueprintTheme>,
) {
    // Setup camera
    commands.spawn((
        Camera2dBundle {
            transform: Transform::from_xyz(0.0, 0.0, 999.0),
            ..default()
        },
        BlueprintCamera::default(),
    ));
    
    // Create background
    commands.spawn((
        SpriteBundle {
            sprite: Sprite {
                color: theme.background_app,
                custom_size: Some(Vec2::new(2000.0, 2000.0)),
                ..default()
            },
            transform: Transform::from_xyz(0.0, 0.0, -1000.0),
            ..default()
        },
    ));
    
    // Create some content behind the glass
    for i in 0..10 {
        let x = (i as f32 - 5.0) * 200.0;
        let y = (i as f32 % 3.0 - 1.0) * 200.0;
        
        commands.spawn(SpriteBundle {
            sprite: Sprite {
                color: Color::hsla(i as f32 * 0.1, 0.7, 0.5, 1.0),
                custom_size: Some(Vec2::new(150.0, 150.0)),
                ..default()
            },
            transform: Transform::from_xyz(x, y, -500.0),
            ..default()
        });
    }
    
    // Create glass panels
    spawn_glass_panel(
        &mut commands,
        Vec2::new(-300.0, 0.0),
        Vec2::new(600.0, 400.0),
        0.5,
        Color::rgba(0.1, 0.1, 0.2, 0.3),
        &mut meshes,
        &mut materials,
    );
    
    spawn_glass_panel(
        &mut commands,
        Vec2::new(200.0, -200.0),
        Vec2::new(400.0, 400.0),
        0.7,
        Color::rgba(0.2, 0.1, 0.3, 0.4),
        &mut meshes,
        &mut materials,
    );
}

/// Animate glass panels
fn animate_glass_panels(
    time: Res<Time>,
    mut query: Query<(&mut Transform, &GlassPanel)>,
) {
    for (mut transform, _) in query.iter_mut() {
        // Subtle rotation animation
        transform.rotation = Quat::from_rotation_z(time.elapsed_seconds() * 0.1);
    }
}
