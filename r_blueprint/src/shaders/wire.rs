//! Wire shader implementation for Bevy 0.17

use bevy::{
    prelude::*,
    render::render_resource::{Shader, AsBindGroup},
    reflect::TypePath,
};

/// Key for wire material
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WireMaterialKey {
    pub color: Color,
    pub thickness: f32,
    pub glow_intensity: f32,
}

impl From<WireMaterial> for WireMaterialKey {
    fn from(material: &WireMaterial) -> Self {
        Self {
            color: material.color,
            thickness: material.thickness,
            glow_intensity: material.glow_intensity,
        }
    }
}

/// Custom material for wire rendering
#[derive(Debug, Clone, Asset, AsBindGroup, TypePath)]
pub struct WireMaterial {
    #[uniform(0)]
    pub color: Color,
    
    #[uniform(1)]
    pub thickness: f32,
    
    #[uniform(2)]
    pub glow_intensity: f32,
    
    #[uniform(3)]
    pub is_highlighted: f32,
}

impl Material for WireMaterial {
    fn fragment_shader() -> Shader {
        "embedded://r_blueprint/shaders/wire_material.wgsl".into()
    }
    
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }
}

/// Plugin for wire material
pub struct WireMaterialPlugin;

impl Plugin for WireMaterialPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<WireMaterial>::default());
    }
}

/// Create a wire material
pub fn create_wire_material(
    color: Color,
    thickness: f32,
    glow_intensity: f32,
    is_highlighted: bool,
) -> WireMaterial {
    WireMaterial {
        color,
        thickness,
        glow_intensity,
        is_highlighted: if is_highlighted { 1.0 } else { 0.0 },
    }
}

/// Create a bezier curve mesh for wire rendering
pub fn create_wire_mesh(
    start: Vec2,
    end: Vec2,
    control1: Vec2,
    control2: Vec2,
    thickness: f32,
    segments: usize,
) -> Mesh {
    use std::f32::consts::*;
    
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleStrip);
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    
    for i in 0..=segments {
        let t = i as f32 / segments as f32;
        
        // Calculate bezier curve point
        let point = cubic_bezier(start, control1, control2, end, t);
        
        // Calculate derivative for normal
        let derivative = cubic_bezier_derivative(start, control1, control2, end, t);
        let normal = Vec2::new(-derivative.y, derivative.x).normalize_or_zero();
        
        // Create two vertices for the strip (left and right of center line)
        let offset = normal * thickness * 0.5;
        
        positions.push([point.x + offset.x, point.y + offset.y, 0.0]);
        positions.push([point.x - offset.x, point.y - offset.y, 0.0]);
        
        normals.push([0.0, 0.0, 1.0]);
        normals.push([0.0, 0.0, 1.0]);
    }
    
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh
}

/// Cubic bezier curve calculation
fn cubic_bezier(p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2, t: f32) -> Vec2 {
    let t2 = t * t;
    let t3 = t2 * t;
    let mt = 1.0 - t;
    let mt2 = mt * mt;
    let mt3 = mt2 * mt;
    
    p0 * mt3 + p1 * 3.0 * mt2 * t + p2 * 3.0 * mt * t2 + p3 * t3
}

/// Cubic bezier derivative calculation
fn cubic_bezier_derivative(p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2, t: f32) -> Vec2 {
    let t2 = t * t;
    let mt = 1.0 - t;
    let mt2 = mt * mt;
    
    3.0 * (p1 - p0) * mt2 + 6.0 * (p2 - p1) * mt * t + 3.0 * (p3 - p2) * t2
}

/// Spawn a wire with bezier curve
pub fn spawn_wire_visuals(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<WireMaterial>>,
    start: Vec2,
    end: Vec2,
    color: Color,
    thickness: f32,
) -> Entity {
    // Calculate control points for smooth curve
    let distance = start.distance(end);
    let control_offset = distance * 0.3;
    
    let control1 = start + Vec2::new(control_offset, 0.0);
    let control2 = end - Vec2::new(control_offset, 0.0);
    
    let mesh = create_wire_mesh(start, end, control1, control2, thickness, 32);
    let material = create_wire_material(color, thickness, 0.5, false);
    
    let mesh_handle = meshes.add(mesh);
    let material_handle = materials.add(material);
    
    commands.spawn(MaterialMeshBundle {
        mesh: mesh_handle,
        material: material_handle,
        ..default()
    }).id()
}
