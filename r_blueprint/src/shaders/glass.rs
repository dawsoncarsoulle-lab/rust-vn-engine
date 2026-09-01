//! Glass effect shader implementation for Bevy 0.17

use bevy::{
    prelude::*,
    render::{
        render_resource::{Shader, AsBindGroup},
        texture::Image,
    },
    reflect::TypePath,
};

/// Key for glass material
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GlassMaterialKey {
    pub distortion_strength: f32,
    pub blur_amount: f32,
    pub tint_color: Color,
}

impl From<GlassMaterial> for GlassMaterialKey {
    fn from(material: &GlassMaterial) -> Self {
        Self {
            distortion_strength: material.distortion_strength,
            blur_amount: material.blur_amount,
            tint_color: material.tint_color,
        }
    }
}

/// Custom material for glass effect
#[derive(Debug, Clone, Asset, AsBindGroup, TypePath)]
pub struct GlassMaterial {
    #[uniform(0)]
    pub distortion_strength: f32,
    
    #[uniform(1)]
    pub blur_amount: f32,
    
    #[uniform(2)]
    pub tint_color: Color,
    
    #[texture(3)]
    #[sampler(4)]
    pub screen_texture: Handle<Image>,
}

impl Material for GlassMaterial {
    fn fragment_shader() -> Shader {
        "embedded://r_blueprint/shaders/glass_material.wgsl".into()
    }
    
    fn vertex_shader() -> Shader {
        "embedded://r_blueprint/shaders/glass_material.wgsl".into()
    }
    
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }
}

/// Plugin for glass material
pub struct GlassMaterialPlugin;

impl Plugin for GlassMaterialPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<GlassMaterial>::default());
    }
}

/// Create a glass material
pub fn create_glass_material(
    distortion_strength: f32,
    blur_amount: f32,
    tint_color: Color,
    screen_texture: Handle<Image>,
) -> GlassMaterial {
    GlassMaterial {
        distortion_strength,
        blur_amount,
        tint_color,
        screen_texture,
    }
}

/// Glass panel component
#[derive(Component)]
pub struct GlassPanel {
    pub size: Vec2,
    pub position: Vec2,
}

/// Spawn a glass panel
pub fn spawn_glass_panel(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<GlassMaterial>>,
    size: Vec2,
    position: Vec2,
    screen_texture: Handle<Image>,
) -> Entity {
    let mesh = Mesh::from(Rectangle::new(size.x, size.y));
    let material = create_glass_material(0.1, 0.5, Color::rgba(0.8, 0.9, 1.0, 0.3), screen_texture);
    
    let mesh_handle = meshes.add(mesh);
    let material_handle = materials.add(material);
    
    commands.spawn((
        MaterialMeshBundle {
            mesh: mesh_handle,
            material: material_handle,
            transform: Transform::from_translation(position.extend(0.0)),
            ..default()
        },
        GlassPanel { size, position },
    )).id()
}

/// System to update glass panel materials
pub fn update_glass_materials(
    mut query: Query<(&mut Handle<GlassMaterial>, &GlassPanel)>, 
    materials: ResMut<Assets<GlassMaterial>>,
    screen_texture: Res<Handle<Image>>,
) {
    for (material_handle, _) in &mut query {
        if let Some(material) = materials.get_mut(material_handle) {
            material.screen_texture = screen_texture.clone();
        }
    }
}
