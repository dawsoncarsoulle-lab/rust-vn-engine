//! Glass effect rendering

use bevy::prelude::*;
use bevy::render::camera::RenderTarget;
use bevy::render::render_resource::{
    Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
};
use bevy::sprite::MaterialMesh2dBundle;
use crate::shaders::glass::{GlassMaterial, GlassPanel};
use crate::rendering::theme::BlueprintTheme;

/// Plugin for glass effect
pub struct GlassEffectRenderingPlugin;

impl Plugin for GlassEffectRenderingPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(Material2dPlugin::<GlassMaterial>::default())
            .add_systems(Startup, setup_glass_effects)
            .add_systems(Update, update_glass_effects);
    }
}

/// Setup glass effects
fn setup_glass_effects(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<GlassMaterial>>,
    theme: Res<BlueprintTheme>,
) {
    // Create a full-screen glass overlay
    // This will be used to create the Unreal Engine-style glass effect
    spawn_full_screen_glass(&mut commands, &mut meshes, &mut materials, &theme);
}

/// Spawn full-screen glass effect
fn spawn_full_screen_glass(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<GlassMaterial>>,
    theme: &BlueprintTheme,
) {
    // Create a full-screen quad
    let mesh = meshes.add(Rectangle::new(10000.0, 10000.0));
    
    // Create glass material
    // Note: For a proper glass effect, we need to capture the screen texture
    // This requires a render target setup
    let material = materials.add(GlassMaterial {
        tint: theme.accent_soft.into(),
        intensity: 0.3,
        time: 0.0,
        screen_texture: Handle::default(),
    });
    
    commands.spawn((
        MaterialMesh2dBundle {
            mesh,
            material,
            transform: Transform::from_xyz(0.0, 0.0, 900.0), // High Z to render on top
            ..default()
        },
        GlassPanel {
            intensity: 0.3,
            tint: theme.accent_soft.into(),
        },
    ));
}

/// Update glass effects
fn update_glass_effects(
    time: Res<Time>,
    mut query: Query<(&mut GlassMaterial, &GlassPanel)>,
) {
    for (mut material, panel) in query.iter_mut() {
        material.time = time.elapsed_seconds();
        material.intensity = panel.intensity;
        material.tint = panel.tint;
    }
}

/// Spawn a glass panel for a specific region (like a node panel)
pub fn spawn_glass_panel(
    commands: &mut Commands,
    position: Vec2,
    size: Vec2,
    intensity: f32,
    tint: Color,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<GlassMaterial>>,
) -> Entity {
    let mesh = meshes.add(Rectangle::new(size.x, size.y));
    
    let material = materials.add(GlassMaterial {
        tint,
        intensity,
        time: 0.0,
        screen_texture: Handle::default(),
    });
    
    commands.spawn((
        MaterialMesh2dBundle {
            mesh,
            material,
            transform: Transform::from_translation(position.extend(0.0)),
            ..default()
        },
        GlassPanel { intensity, tint },
    )).id()
}

/// Create a render target for glass effect
/// This is needed to capture the scene for the glass distortion effect
pub fn create_glass_render_target(
    commands: &mut Commands,
    images: &mut ResMut<Assets<Image>>,
    windows: &Query<&Window>,
) -> Handle<Image> {
    let window = windows.single();
    
    let size = Extent3d {
        width: window.width() as u32,
        height: window.height() as u32,
        depth_or_array_layers: 1,
    };
    
    let mut image = Image {
        texture_descriptor: TextureDescriptor {
            label: Some("Glass Effect Render Target"),
            size,
            dimension: TextureDimension::D2,
            format: TextureFormat::Bgra8UnormSrgb,
            mip_level_count: 1,
            sample_count: 1,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        },
        ..default()
    };
    
    image.resize(size);
    let image_handle = images.add(image);
    
    // Create a camera for rendering to this target
    commands.spawn(Camera2dBundle {
        camera: Camera {
            target: RenderTarget::Image(image_handle.clone()),
            ..default()
        },
        ..default()
    });
    
    image_handle
}

/// Setup the glass effect pipeline
/// This creates the necessary render targets and cameras for the glass effect
pub struct GlassEffectPipelinePlugin;

impl Plugin for GlassEffectPipelinePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_glass_pipeline);
    }
}

fn setup_glass_pipeline(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    windows: Query<&Window>,
) {
    // Create render target for glass effect
    let render_target = create_glass_render_target(&mut commands, &mut images, &windows);
    
    // Store the render target as a resource
    commands.insert_resource(GlassRenderTarget(render_target));
}

/// Resource to store the glass render target
#[derive(Debug, Clone, Resource)]
pub struct GlassRenderTarget(pub Handle<Image>);

/// Component to mark entities that should be included in the glass effect
#[derive(Debug, Clone, Component)]
pub struct GlassEffectSource;

/// Component to mark entities that should be excluded from the glass effect
#[derive(Debug, Clone, Component)]
pub struct GlassEffectExclude;
