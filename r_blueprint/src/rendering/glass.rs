//! Glass effect rendering for blueprints

use bevy::prelude::*;
use crate::shaders::glass::{GlassMaterial, GlassMaterialPlugin, GlassMaterialKey, create_glass_material, GlassPanel};

/// Plugin for glass effect rendering
pub struct GlassEffectPlugin;

impl Plugin for GlassEffectPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(GlassMaterialPlugin)
            .add_systems(Startup, setup_glass_effect)
            .add_systems(Update, update_glass_effect);
    }
}

/// Setup glass effect
fn setup_glass_effect(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<GlassMaterial>>,
) {
    // Glass effect will be added to specific panels as needed
}

/// Update glass effect materials
fn update_glass_effect(
    mut query: Query<(&mut Handle<GlassMaterial>, &GlassPanel)>, 
    materials: ResMut<Assets<GlassMaterial>>,
) {
    for (material_handle, _) in &mut query {
        if let Some(material) = materials.get_mut(material_handle) {
            // Update glass material properties as needed
        }
    }
}

/// Spawn a glass panel with custom properties
pub fn spawn_glass_panel_custom(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<GlassMaterial>>,
    size: Vec2,
    position: Vec2,
    distortion_strength: f32,
    blur_amount: f32,
    tint_color: Color,
    screen_texture: Handle<Image>,
) -> Entity {
    let mesh = Mesh::from(Rectangle::new(size.x, size.y));
    let material = GlassMaterial {
        distortion_strength,
        blur_amount,
        tint_color,
        screen_texture,
    };
    
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

/// Glass effect settings
#[derive(Debug, Clone)]
pub struct GlassEffectSettings {
    pub distortion_strength: f32,
    pub blur_amount: f32,
    pub tint_color: Color,
}

impl Default for GlassEffectSettings {
    fn default() -> Self {
        Self {
            distortion_strength: 0.1,
            blur_amount: 0.5,
            tint_color: Color::rgba(0.8, 0.9, 1.0, 0.3),
        }
    }
}
