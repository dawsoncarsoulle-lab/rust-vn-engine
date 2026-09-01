//! Node shader implementation for Bevy 0.17

use bevy::{
    prelude::*,
    render::render_resource::{Shader, AsBindGroup},
    reflect::TypePath,
};
use crate::core::{NodeType, NodeIcon};

/// Key for node material
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NodeMaterialKey {
    pub color_top: Color,
    pub color_bottom: Color,
    pub border_color: Color,
    pub corner_radius: f32,
}

impl From<NodeMaterial> for NodeMaterialKey {
    fn from(material: &NodeMaterial) -> Self {
        Self {
            color_top: material.color_top,
            color_bottom: material.color_bottom,
            border_color: material.border_color,
            corner_radius: material.corner_radius,
        }
    }
}

/// Custom material for node backgrounds
#[derive(Debug, Clone, Asset, AsBindGroup, TypePath)]
pub struct NodeMaterial {
    #[uniform(0)]
    pub color_top: Color,
    
    #[uniform(1)]
    pub color_bottom: Color,
    
    #[uniform(2)]
    pub border_color: Color,
    
    #[uniform(3)]
    pub corner_radius: f32,
    
    #[uniform(4)]
    pub is_selected: f32,
    
    #[uniform(5)]
    pub is_highlighted: f32,
}

impl Material for NodeMaterial {
    fn fragment_shader() -> Shader {
        "embedded://r_blueprint/shaders/node_material.wgsl".into()
    }
    
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }
}

/// Plugin for node material
pub struct NodeMaterialPlugin;

impl Plugin for NodeMaterialPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<NodeMaterial>::default());
    }
}

/// Create a node material based on node type
pub fn create_node_material(
    node_type: NodeType,
    theme: &crate::rendering::theme::BlueprintTheme,
    is_selected: bool,
    is_highlighted: bool,
) -> NodeMaterial {
    let (color_top, color_bottom) = match node_type {
        NodeType::Event => theme.node_header_gradients.get("event")
            .map(|(c1, c2)| (*c1, *c2))
            .unwrap_or((theme.accent, theme.accent)),
        NodeType::Function => theme.node_header_gradients.get("function")
            .map(|(c1, c2)| (*c1, *c2))
            .unwrap_or((theme.accent, theme.accent)),
        NodeType::Branch => theme.node_header_gradients.get("branch")
            .map(|(c1, c2)| (*c1, *c2))
            .unwrap_or((theme.accent, theme.accent)),
        NodeType::Math => theme.node_header_gradients.get("math")
            .map(|(c1, c2)| (*c1, *c2))
            .unwrap_or((theme.accent, theme.accent)),
        NodeType::Variable => theme.node_header_gradients.get("variable")
            .map(|(c1, c2)| (*c1, *c2))
            .unwrap_or((theme.accent, theme.accent)),
        _ => (theme.node_bg_top, theme.node_bg_bottom),
    };
    
    NodeMaterial {
        color_top,
        color_bottom,
        border_color: theme.node_border,
        corner_radius: theme.node_corner_radius,
        is_selected: if is_selected { 1.0 } else { 0.0 },
        is_highlighted: if is_highlighted { 1.0 } else { 0.0 },
    }
}

/// Create a node icon mesh
pub fn create_node_icon_mesh(icon: NodeIcon) -> Mesh {
    match icon {
        NodeIcon::Triangle => create_triangle_mesh(),
        NodeIcon::Dot => create_circle_mesh(4.0),
        NodeIcon::Diamond => create_diamond_mesh(),
        NodeIcon::Plus => create_plus_mesh(),
        NodeIcon::Circle => create_circle_mesh(4.0),
        NodeIcon::Square => create_square_mesh(8.0),
        _ => create_square_mesh(8.0),
    }
}

// Mesh creation helpers
fn create_triangle_mesh() -> Mesh {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList);
    let vertices = vec![
        ([0.0, 5.0, 0.0], [0.0, 0.0]),
        ([-5.0, -5.0, 0.0], [0.0, 0.0]),
        ([5.0, -5.0, 0.0], [0.0, 0.0]),
    ];
    let positions: Vec<[f32; 3]> = vertices.iter().map(|(pos, _)| *pos).collect();
    let uvs: Vec<[f32; 2]> = vertices.iter().map(|(_, uv)| *uv).collect();
    let indices = vec![0, 1, 2];
    
    mesh.insert_indices(Indices::U32(indices));
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh
}

fn create_circle_mesh(radius: f32) -> Mesh {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleFan);
    let segments = 16;
    let mut positions = vec![[0.0, 0.0, 0.0]];
    let mut uvs = vec![[0.5, 0.5]];
    
    for i in 0..=segments {
        let angle = (i as f32 / segments as f32) * std::f32::consts::TAU;
        let x = angle.cos() * radius;
        let y = angle.sin() * radius;
        positions.push([x, y, 0.0]);
        uvs.push([0.5 + x / (radius * 2.0), 0.5 + y / (radius * 2.0)]);
    }
    
    let indices: Vec<u32> = (0..=(segments + 1)).collect();
    
    mesh.insert_indices(Indices::U32(indices));
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh
}

fn create_diamond_mesh() -> Mesh {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList);
    let vertices = vec![
        ([0.0, 5.0, 0.0], [0.0, 0.0]),
        ([-5.0, 0.0, 0.0], [0.0, 0.0]),
        ([0.0, -5.0, 0.0], [0.0, 0.0]),
        ([0.0, -5.0, 0.0], [0.0, 0.0]),
        ([5.0, 0.0, 0.0], [0.0, 0.0]),
        ([0.0, 5.0, 0.0], [0.0, 0.0]),
    ];
    let positions: Vec<[f32; 3]> = vertices.iter().map(|(pos, _)| *pos).collect();
    let uvs: Vec<[f32; 2]> = vertices.iter().map(|(_, uv)| *uv).collect();
    let indices = vec![0, 1, 2, 3, 4, 5];
    
    mesh.insert_indices(Indices::U32(indices));
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh
}

fn create_plus_mesh() -> Mesh {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList);
    
    let h_vertices = vec![
        ([-3.0, 1.0, 0.0], [0.0, 0.0]),
        ([-3.0, -1.0, 0.0], [0.0, 0.0]),
        ([3.0, -1.0, 0.0], [0.0, 0.0]),
        ([3.0, -1.0, 0.0], [0.0, 0.0]),
        ([3.0, 1.0, 0.0], [0.0, 0.0]),
        ([-3.0, 1.0, 0.0], [0.0, 0.0]),
    ];
    
    let v_vertices = vec![
        ([-1.0, 3.0, 0.0], [0.0, 0.0]),
        ([-1.0, -3.0, 0.0], [0.0, 0.0]),
        ([1.0, -3.0, 0.0], [0.0, 0.0]),
        ([1.0, -3.0, 0.0], [0.0, 0.0]),
        ([1.0, 3.0, 0.0], [0.0, 0.0]),
        ([-1.0, 3.0, 0.0], [0.0, 0.0]),
    ];
    
    let mut positions: Vec<[f32; 3]> = h_vertices.iter().chain(v_vertices.iter()).map(|(pos, _)| *pos).collect();
    let uvs: Vec<[f32; 2]> = h_vertices.iter().chain(v_vertices.iter()).map(|(_, uv)| *uv).collect();
    let indices: Vec<u32> = (0..12).collect();
    
    mesh.insert_indices(Indices::U32(indices));
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh
}

fn create_square_mesh(size: f32) -> Mesh {
    Mesh::from(Rectangle::new(size, size))
}
