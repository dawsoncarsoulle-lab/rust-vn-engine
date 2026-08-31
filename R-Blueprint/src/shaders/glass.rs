//! Glass effect shader implementation

use bevy::prelude::*;
use bevy::render::render_resource::Shader;
use bevy::render::texture::Image;
use bevy::sprite::MaterialMesh2dBundle;

/// Custom material for glass effect
#[derive(Debug, Clone, Asset, TypePath, AsBindGroup, Reflect)]
#[reflect(Component, Asset)]
#[bind_group_data(GlassMaterialKey)]
pub struct GlassMaterial {
    #[uniform(0)]
    pub tint: Color,
    
    #[uniform(1)]
    pub intensity: f32,
    
    #[uniform(2)]
    pub time: f32,
    
    #[texture(3)]
    #[sampler(4)]
    pub screen_texture: Handle<Image>,
}

impl Material2d for GlassMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://r_blueprint/shaders/glass_material.wgsl".into()
    }
    
    fn vertex_shader() -> ShaderRef {
        "embedded://r_blueprint/shaders/glass_material.wgsl".into()
    }
}

/// Key for glass material
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GlassMaterialKey {
    pub tint: Color,
    pub intensity: f32,
    pub time: f32,
}

impl From<GlassMaterial> for GlassMaterialKey {
    fn from(material: &GlassMaterial) -> Self {
        Self {
            tint: material.tint,
            intensity: material.intensity,
            time: material.time,
        }
    }
}

/// Component to mark an entity as having glass effect
#[derive(Debug, Clone, Component)]
pub struct GlassPanel {
    pub intensity: f32,
    pub tint: Color,
}

impl Default for GlassPanel {
    fn default() -> Self {
        Self {
            intensity: 0.5,
            tint: Color::rgba(0.1, 0.1, 0.15, 0.3),
        }
    }
}

/// Plugin for glass effect rendering
pub struct GlassEffectPlugin;

impl Plugin for GlassEffectPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(Material2dPlugin::<GlassMaterial>::default())
            .add_systems(Startup, setup_glass_materials)
            .add_systems(Update, update_glass_time);
    }
}

fn setup_glass_materials(
    mut materials: ResMut<Assets<GlassMaterial>>,
) {
    // Pre-create a default glass material
    // This will be used for panels and other glass surfaces
}

/// Update time for glass effect animations
fn update_glass_time(
    time: Res<Time>,
    mut query: Query<&mut GlassMaterial>,
) {
    for mut material in query.iter_mut() {
        material.time = time.elapsed_seconds();
    }
}

/// Create a glass panel
pub fn spawn_glass_panel(
    commands: &mut Commands,
    position: Vec2,
    size: Vec2,
    intensity: f32,
    tint: Color,
    screen_texture: Handle<Image>,
    materials: &mut ResMut<Assets<GlassMaterial>>,
) -> Entity {
    let material = materials.add(GlassMaterial {
        tint,
        intensity,
        time: 0.0,
        screen_texture: screen_texture.clone(),
    });
    
    commands.spawn((
        MaterialMesh2dBundle {
            mesh: Mesh::from(Rectangle::new(size.x, size.y)).into(),
            material,
            transform: Transform::from_translation(position.extend(0.0)),
            ..default()
        },
        GlassPanel { intensity, tint },
    )).id()
}

/// Glass effect settings for the entire canvas
#[derive(Debug, Clone, Resource)]
pub struct GlassEffectSettings {
    /// Enable glass effect
    pub enabled: bool,
    
    /// Overall intensity
    pub intensity: f32,
    
    /// Tint color
    pub tint: Color,
    
    /// Blur amount
    pub blur_amount: f32,
    
    /// Distortion amount
    pub distortion_amount: f32,
    
    /// Edge darkness (vignette)
    pub edge_darkness: f32,
}

impl Default for GlassEffectSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            intensity: 0.3,
            tint: Color::rgba(0.1, 0.1, 0.2, 0.2),
            blur_amount: 0.5,
            distortion_amount: 0.001,
            edge_darkness: 0.3,
        }
    }
}

/// Plugin for full-screen glass effect
pub struct FullScreenGlassPlugin;

impl Plugin for FullScreenGlassPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GlassEffectSettings>()
            .add_systems(Startup, setup_full_screen_glass)
            .add_systems(Update, update_full_screen_glass);
    }
}

fn setup_full_screen_glass(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<GlassMaterial>>,
) {
    // Create a full-screen quad
    let mesh = meshes.add(Rectangle::new(1.0, 1.0));
    
    // Create glass material (screen texture will be set later)
    let material = materials.add(GlassMaterial {
        tint: Color::rgba(0.1, 0.1, 0.2, 0.2),
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
            tint: Color::rgba(0.1, 0.1, 0.2, 0.2),
        },
    ));
}

fn update_full_screen_glass(
    // Implementation would update the glass effect based on settings
) {
    // To be implemented
}
