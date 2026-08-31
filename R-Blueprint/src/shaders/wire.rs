//! Wire shader implementation

use bevy::prelude::*;
use bevy::render::render_resource::Shader;
use bevy::sprite::MaterialMesh2dBundle;
use crate::core::{Wire, PinKind};
use crate::rendering::theme::BlueprintTheme;

/// Custom material for wire rendering
#[derive(Debug, Clone, Asset, TypePath, AsBindGroup, Reflect)]
#[reflect(Component, Asset)]
#[bind_group_data(WireMaterialKey)]
pub struct WireMaterial {
    #[uniform(0)]
    pub wire_color: Color,
    
    #[uniform(1)]
    pub glow_color: Color,
    
    #[uniform(2)]
    pub glow_intensity: f32,
    
    #[uniform(3)]
    pub thickness: f32,
}

impl Material2d for WireMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://r_blueprint/shaders/wire_material.wgsl".into()
    }
}

/// Key for wire material
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WireMaterialKey {
    pub wire_color: Color,
    pub glow_color: Color,
    pub glow_intensity: f32,
    pub thickness: f32,
}

impl From<WireMaterial> for WireMaterialKey {
    fn from(material: &WireMaterial) -> Self {
        Self {
            wire_color: material.wire_color,
            glow_color: material.glow_color,
            glow_intensity: material.glow_intensity,
            thickness: material.thickness,
        }
    }
}

/// Component to mark an entity as a wire
#[derive(Debug, Clone, Component)]
pub struct BlueprintWire {
    pub wire_id: String,
    pub from: (String, String),
    pub to: (String, String),
    pub kind: PinKind,
    pub is_selected: bool,
    pub is_highlighted: bool,
}

/// Marker component for wire path
#[derive(Debug, Clone, Component)]
pub struct WirePath;

/// Plugin for wire rendering
pub struct WireRenderingPlugin;

impl Plugin for WireRenderingPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(Material2dPlugin::<WireMaterial>::default())
            .add_systems(Startup, setup_wire_materials);
    }
}

fn setup_wire_materials(
    mut materials: ResMut<Assets<WireMaterial>>,
) {
    // Pre-create materials for each wire type
    // These will be cloned and modified as needed
}

/// Spawn a wire entity
pub fn spawn_wire(
    commands: &mut Commands,
    wire: &Wire,
    path_points: Vec<Vec2>,
    theme: &BlueprintTheme,
    materials: &mut ResMut<Assets<WireMaterial>>,
) -> Entity {
    // Get the color for this wire type
    let color = theme.pin_colors.get(&wire.kind).cloned().unwrap_or(Color::WHITE);
    let glow_color = theme.pin_glow_colors.get(&wire.kind).cloned().unwrap_or(Color::WHITE);
    
    // Create the material
    let material = materials.add(WireMaterial {
        wire_color: color,
        glow_color,
        glow_intensity: if wire.highlighted == Some(true) { 1.0 } else { 0.0 },
        thickness: if wire.kind == PinKind::Exec {
            theme.wire_thickness_exec
        } else {
            theme.wire_thickness
        },
    });
    
    // Create a mesh for the wire path
    // This is a simple line strip for now
    // In practice, we'd use a bezier curve mesh
    let mut mesh = Mesh::new(bevy::render::mesh::PrimitiveTopology::TriangleStrip);
    
    // For now, just create a simple line
    // A proper implementation would create a bezier curve mesh with thickness
    
    // Spawn the wire entity
    commands.spawn((
        MaterialMesh2dBundle {
            mesh: mesh.into(),
            material,
            transform: Transform::default(),
            ..default()
        },
        BlueprintWire {
            wire_id: wire.id.clone(),
            from: wire.from.clone(),
            to: wire.to.clone(),
            kind: wire.kind,
            is_selected: wire.selected == Some(true),
            is_highlighted: wire.highlighted == Some(true),
        },
        WirePath,
    )).id()
}

/// Update wire visuals
pub fn update_wire_visuals(
    query: Query<(&BlueprintWire, &mut Handle<WireMaterial>), Changed<BlueprintWire>>,
    mut materials: ResMut<Assets<WireMaterial>>,
    theme: Res<BlueprintTheme>,
) {
    for (wire, material_handle) in query.iter() {
        if let Some(material) = materials.get_mut(material_handle) {
            let glow_intensity = if wire.is_highlighted {
                1.0
            } else if wire.is_selected {
                0.5
            } else {
                0.0
            };
            
            material.glow_intensity = glow_intensity;
        }
    }
}

/// Create a bezier curve mesh for a wire
pub fn create_wire_mesh(
    start: Vec2,
    cp1: Vec2,
    cp2: Vec2,
    end: Vec2,
    thickness: f32,
    segments: usize,
) -> Mesh {
    let mut positions = Vec::new();
    let mut uvs = Vec::new();
    let mut colors = Vec::new();
    
    // Sample the bezier curve
    for i in 0..=segments {
        let t = i as f32 / segments as f32;
        
        // Calculate point on bezier curve
        let point = cubic_bezier(start, cp1, cp2, end, t);
        
        // Calculate tangent for normal
        let tangent = cubic_bezier_tangent(start, cp1, cp2, end, t);
        let normal = Vec2::new(-tangent.y, tangent.x).normalize();
        
        // Perpendicular offset for thickness
        let offset = normal * thickness * 0.5;
        
        // Two vertices for each point (left and right side of wire)
        let left = point - offset;
        let right = point + offset;
        
        positions.push([left.x, left.y, 0.0]);
        positions.push([right.x, right.y, 0.0]);
        
        // UV coordinates
        uvs.push([t, 0.0]);
        uvs.push([t, 1.0]);
        
        // Color (white for now, will be tinted by material)
        colors.push([1.0, 1.0, 1.0, 1.0]);
        colors.push([1.0, 1.0, 1.0, 1.0]);
    }
    
    // Create indices for triangle strip
    let mut indices = Vec::new();
    for i in 0..segments {
        let base = i * 2;
        indices.push(base as u32);
        indices.push(base as u32 + 1);
        indices.push(base as u32 + 2);
        
        indices.push(base as u32 + 1);
        indices.push(base as u32 + 3);
        indices.push(base as u32 + 2);
    }
    
    let mut mesh = Mesh::new(bevy::render::mesh::PrimitiveTopology::TriangleList);
    mesh.set_indices(Some(bevy::render::mesh::Indices::U32(indices)));
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh
}

/// Calculate point on cubic bezier curve
fn cubic_bezier(p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2, t: f32) -> Vec2 {
    let one_minus_t = 1.0 - t;
    
    p0 * (one_minus_t * one_minus_t * one_minus_t) +
    p1 * (3.0 * one_minus_t * one_minus_t * t) +
    p2 * (3.0 * one_minus_t * t * t) +
    p3 * (t * t * t)
}

/// Calculate tangent on cubic bezier curve
fn cubic_bezier_tangent(p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2, t: f32) -> Vec2 {
    let one_minus_t = 1.0 - t;
    
    let derivative = 
        p1 * 3.0 * one_minus_t * one_minus_t +
        p2 * 6.0 * one_minus_t * t +
        p3 * 3.0 * t * t -
        p0 * 3.0 * one_minus_t * one_minus_t;
    
    derivative.normalize()
}

/// Calculate control points for a smooth wire
pub fn calculate_wire_control_points(
    start: Vec2,
    end: Vec2,
    start_tangent: Vec2,
    end_tangent: Vec2,
    tension: f32,
) -> (Vec2, Vec2) {
    let length = (end - start).length();
    let control_distance = length * 0.3 * tension;
    
    let cp1 = start + start_tangent * control_distance;
    let cp2 = end - end_tangent * control_distance;
    
    (cp1, cp2)
}
