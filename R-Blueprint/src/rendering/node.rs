//! Node rendering for blueprints

use bevy::prelude::*;
use bevy::render::render_resource::Shader;
use crate::core::{Node, NodeType, NodeIcon, Pin, PinKind, PinDirection};
use crate::rendering::theme::BlueprintTheme;

/// Component to mark an entity as a blueprint node
#[derive(Debug, Clone, Component)]
pub struct BlueprintNode {
    pub node_id: String,
}

/// Component for node visual state
#[derive(Debug, Clone, Component)]
pub struct NodeVisualState {
    pub is_selected: bool,
    pub is_highlighted: bool,
    pub is_hovered: bool,
}

impl Default for NodeVisualState {
    fn default() -> Self {
        Self {
            is_selected: false,
            is_highlighted: false,
            is_hovered: false,
        }
    }
}

/// Marker component for node header
#[derive(Debug, Clone, Component)]
pub struct NodeHeader;

/// Marker component for node body
#[derive(Debug, Clone, Component)]
pub struct NodeBody;

/// Marker component for node title
#[derive(Debug, Clone, Component)]
pub struct NodeTitle;

/// Marker component for node icon
#[derive(Debug, Clone, Component)]
pub struct NodeIconComponent {
    pub icon_type: NodeIcon,
}

/// Marker component for pin
#[derive(Debug, Clone, Component)]
pub struct BlueprintPin {
    pub node_id: String,
    pub pin_id: String,
    pub pin_type: PinKind,
    pub direction: PinDirection,
    pub is_connected: bool,
    pub is_hovered: bool,
}

/// Marker component for pin label
#[derive(Debug, Clone, Component)]
pub struct PinLabel {
    pub text: String,
}

/// Marker component for literal value box
#[derive(Debug, Clone, Component)]
pub struct LiteralBox {
    pub text: String,
}

/// Bundle for spawning a complete node
#[derive(Debug, Clone, Bundle)]
pub struct NodeBundle {
    pub node: BlueprintNode,
    pub visual_state: NodeVisualState,
    pub transform: Transform,
    pub global_transform: GlobalTransform,
}

impl NodeBundle {
    pub fn new(node_id: String, position: Vec2) -> Self {
        Self {
            node: BlueprintNode { node_id },
            visual_state: NodeVisualState::default(),
            transform: Transform::from_translation(position.extend(0.0)),
            global_transform: GlobalTransform::default(),
        }
    }
}

/// Plugin for node rendering
pub struct NodeRenderingPlugin;

impl Plugin for NodeRenderingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_node_materials)
            .add_systems(Update, (
                update_node_visuals,
                handle_pin_hover,
            ));
    }
}

/// Custom material for node backgrounds with gradient
#[derive(Debug, Clone, Asset, TypePath, AsBindGroup, Reflect)]
#[reflect(Component, Asset)]
#[bind_group_data(NodeMaterialKey)]
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

impl Material2d for NodeMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://r_blueprint/shaders/node_material.wgsl".into()
    }
}

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

/// Setup node materials
fn setup_node_materials(
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<NodeMaterial>>,
) {
    // Create a rectangle mesh for nodes
    let rect = meshes.add(Rectangle::new(200.0, 100.0));
}

/// Update node visuals based on state
fn update_node_visuals(
    query: Query<(&BlueprintNode, &NodeVisualState, &Transform), Changed<NodeVisualState>>,
    theme: Res<BlueprintTheme>,
    mut node_materials: ResMut<Assets<NodeMaterial>>,
    mut node_query: Query<&mut Handle<NodeMaterial>>,
) {
    for (node, state, _) in query.iter() {
        // This would update the node's material based on its state
        // In practice, we'd need to store the material handle on the node entity
    }
}

/// Handle pin hover effects
fn handle_pin_hover(
    // Implementation would handle pin hover state
) {
    // To be implemented
}

/// Spawn a complete node with all its components
pub fn spawn_node(
    commands: &mut Commands,
    node: &Node,
    theme: &BlueprintTheme,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<NodeMaterial>>,
) -> Entity {
    // Create the main node entity
    let node_entity = commands.spawn(NodeBundle::new(node.id.clone(), node.position.extend(0.0)))
        .id();
    
    // Calculate node size based on content
    let node_width = 200.0; // Will be calculated based on pins
    let node_height = 80.0 + (node.inputs.len() as f32 + node.outputs.len() as f32) * 24.0;
    
    // Spawn node background
    spawn_node_background(commands, node_entity, node, node_width, node_height, theme, materials);
    
    // Spawn node header
    spawn_node_header(commands, node_entity, node, theme);
    
    // Spawn node body
    spawn_node_body(commands, node_entity, node, theme);
    
    // Spawn pins
    spawn_node_pins(commands, node_entity, node, theme);
    
    node_entity
}

fn spawn_node_background(
    commands: &mut Commands,
    parent: Entity,
    node: &Node,
    width: f32,
    height: f32,
    theme: &BlueprintTheme,
    materials: &mut ResMut<Assets<NodeMaterial>>,
) {
    let material = materials.add(NodeMaterial {
        color_top: theme.node_bg_top,
        color_bottom: theme.node_bg_bottom,
        border_color: theme.node_border,
        corner_radius: theme.node_corner_radius,
        is_selected: if node.selected == Some(true) { 1.0 } else { 0.0 },
        is_highlighted: if node.highlighted == Some(true) { 1.0 } else { 0.0 },
    });
    
    commands.spawn((
        MaterialMesh2dBundle {
            mesh: Mesh::from(Rectangle::new(width, height)).into(),
            material,
            transform: Transform::from_xyz(0.0, 0.0, -1.0),
            ..default()
        },
        Parent(parent),
    ));
}

fn spawn_node_header(
    commands: &mut Commands,
    parent: Entity,
    node: &Node,
    theme: &BlueprintTheme,
) {
    // Get header colors based on node type
    let type_str = match node.node_type {
        NodeType::Event => "event",
        NodeType::Function => "function",
        NodeType::Branch => "branch",
        NodeType::Math => "math",
        NodeType::Variable => "variable",
        _ => "custom",
    };
    
    let (color1, color2) = theme.node_header_gradients.get(type_str)
        .cloned()
        .unwrap_or((theme.accent, theme.accent));
    
    // Spawn header background
    commands.spawn((
        SpriteBundle {
            sprite: Sprite {
                color: color1,
                custom_size: Some(Vec2::new(200.0, theme.node_header_height)),
                ..default()
            },
            transform: Transform::from_xyz(0.0, 90.0, -0.5),
            ..default()
        },
        NodeHeader,
        Parent(parent),
    ));
    
    // Spawn icon
    spawn_node_icon(commands, parent, node.icon, theme);
    
    // Spawn title
    commands.spawn((
        Text2dBundle {
            text: Text::from_section(
                &node.title,
                TextStyle {
                    font_size: 12.5,
                    color: Color::WHITE,
                    font: default(),
                },
            ),
            transform: Transform::from_xyz(20.0, 90.0, 0.0),
            ..default()
        },
        NodeTitle,
        Parent(parent),
    ));
}

fn spawn_node_icon(
    commands: &mut Commands,
    parent: Entity,
    icon: NodeIcon,
    theme: &BlueprintTheme,
) {
    let (mesh, color) = match icon {
        NodeIcon::Triangle => (create_triangle_mesh(), Color::WHITE),
        NodeIcon::Dot => (create_circle_mesh(4.0), Color::WHITE),
        NodeIcon::Diamond => (create_diamond_mesh(), Color::WHITE),
        NodeIcon::Plus => (create_plus_mesh(), Color::WHITE),
        NodeIcon::Circle => (create_circle_mesh(4.0), Color::WHITE),
        NodeIcon::Square => (create_square_mesh(8.0), Color::WHITE),
        _ => (Mesh::from(Rectangle::new(10.0, 10.0)), Color::WHITE),
    };
    
    commands.spawn((
        MaterialMesh2dBundle {
            mesh,
            material: materials::ColorMaterial::from(color),
            transform: Transform::from_xyz(10.0, 90.0, 0.0),
            ..default()
        },
        NodeIconComponent { icon_type: icon },
        Parent(parent),
    ));
}

fn spawn_node_body(
    commands: &mut Commands,
    parent: Entity,
    node: &Node,
    theme: &BlueprintTheme,
) {
    // Body will contain pins
    // This is a placeholder - pins will be spawned separately
}

fn spawn_node_pins(
    commands: &mut Commands,
    parent: Entity,
    node: &Node,
    theme: &BlueprintTheme,
) {
    // Spawn input pins (left side)
    for (i, pin) in node.inputs.iter().enumerate() {
        spawn_pin(
            commands,
            parent,
            node.id.clone(),
            pin,
            PinDirection::Input,
            i,
            node.inputs.len(),
            theme,
        );
    }
    
    // Spawn output pins (right side)
    for (i, pin) in node.outputs.iter().enumerate() {
        spawn_pin(
            commands,
            parent,
            node.id.clone(),
            pin,
            PinDirection::Output,
            i,
            node.outputs.len(),
            theme,
        );
    }
}

fn spawn_pin(
    commands: &mut Commands,
    parent: Entity,
    node_id: String,
    pin: &Pin,
    direction: PinDirection,
    index: usize,
    total: usize,
    theme: &BlueprintTheme,
) {
    let x = match direction {
        PinDirection::Input => -10.0,
        PinDirection::Output => 10.0,
    };
    
    let y = theme.node_header_height + 10.0 + (index as f32) * 24.0;
    
    // Get pin color
    let color = theme.pin_colors.get(&pin.kind).cloned().unwrap_or(Color::WHITE);
    
    // Spawn pin
    let pin_entity = commands.spawn((
        TransformBundle::from_transform(Transform::from_xyz(x, y, 0.0)),
        BlueprintPin {
            node_id: node_id.clone(),
            pin_id: pin.id.clone(),
            pin_type: pin.kind,
            direction,
            is_connected: pin.connected == Some(true),
            is_hovered: false,
        },
        Parent(parent),
    )).id();
    
    // Spawn pin shape
    if pin.is_exec() {
        // Triangle for exec pins
        spawn_exec_pin(commands, pin_entity, color, theme);
    } else {
        // Circle for data pins
        spawn_data_pin(commands, pin_entity, color, pin.is_connected, theme);
    }
    
    // Spawn pin label
    if !pin.label.is_empty() {
        let label_x = match direction {
            PinDirection::Input => -8.0,
            PinDirection::Output => 8.0,
        };
        
        commands.spawn((
            Text2dBundle {
                text: Text::from_section(
                    &pin.label,
                    TextStyle {
                        font_size: 11.5,
                        color: theme.text_secondary,
                        font: default(),
                    },
                ),
                transform: Transform::from_xyz(label_x, y, 0.0),
                ..default()
            },
            PinLabel { text: pin.label.clone() },
            Parent(parent),
        ));
    }
    
    // Spawn default value for input pins
    if let Some(ref default) = pin.default_value {
        spawn_literal_box(commands, parent, default.clone(), Vec2::new(x + 20.0, y), theme);
    }
}

fn spawn_exec_pin(
    commands: &mut Commands,
    parent: Entity,
    color: Color,
    theme: &BlueprintTheme,
) {
    // Create triangle mesh for exec pin
    let mesh = create_triangle_mesh();
    
    commands.spawn((
        MaterialMesh2dBundle {
            mesh,
            material: materials::ColorMaterial::from(color),
            transform: Transform::from_scale(Vec3::splat(0.8)),
            ..default()
        },
        Parent(parent),
    ));
}

fn spawn_data_pin(
    commands: &mut Commands,
    parent: Entity,
    color: Color,
    is_connected: bool,
    theme: &BlueprintTheme,
) {
    let radius = if is_connected { theme.pin_size } else { theme.pin_size * 0.8 };
    let mesh = create_circle_mesh(radius);
    
    // Use border color if not connected, solid color if connected
    let material_color = if is_connected {
        color
    } else {
        Color::TRANSPARENT
    };
    
    commands.spawn((
        MaterialMesh2dBundle {
            mesh,
            material: materials::ColorMaterial::from(material_color),
            ..default()
        },
        Parent(parent),
    ));
    
    // Add border
    commands.spawn((
        MaterialMesh2dBundle {
            mesh: create_circle_mesh(radius + 1.0),
            material: materials::ColorMaterial::from(color),
            ..default()
        },
        Parent(parent),
    ));
}

fn spawn_literal_box(
    commands: &mut Commands,
    parent: Entity,
    text: String,
    position: Vec2,
    theme: &BlueprintTheme,
) {
    commands.spawn((
        Text2dBundle {
            text: Text::from_section(
                &text,
                TextStyle {
                    font_size: 10.5,
                    color: Color::hex("#d7d7dc").unwrap(),
                    font: default(),
                },
            ),
            transform: Transform::from_translation(position.extend(0.0)),
            ..default()
        },
        LiteralBox { text },
        Parent(parent),
    ));
}

// Mesh creation helpers
fn create_triangle_mesh() -> Mesh {
    let mut mesh = Mesh::new(bevy::render::mesh::PrimitiveTopology::TriangleList);
    let vertices = vec![
        ([0.0, 5.0, 0.0], [0.0, 0.0]),
        ([-5.0, -5.0, 0.0], [0.0, 0.0]),
        ([5.0, -5.0, 0.0], [0.0, 0.0]),
    ];
    let positions: Vec<[f32; 3]> = vertices.iter().map(|(pos, _)| *pos).collect();
    let uvs: Vec<[f32; 2]> = vertices.iter().map(|(_, uv)| *uv).collect();
    let indices = vec![0, 1, 2];
    
    mesh.set_indices(Some(bevy::render::mesh::Indices::U32(indices)));
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh
}

fn create_circle_mesh(radius: f32) -> Mesh {
    let mut mesh = Mesh::new(bevy::render::mesh::PrimitiveTopology::TriangleFan);
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
    
    mesh.set_indices(Some(bevy::render::mesh::Indices::U32(indices)));
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh
}

fn create_diamond_mesh() -> Mesh {
    let mut mesh = Mesh::new(bevy::render::mesh::PrimitiveTopology::TriangleList);
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
    
    mesh.set_indices(Some(bevy::render::mesh::Indices::U32(indices)));
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh
}

fn create_plus_mesh() -> Mesh {
    // Create a plus sign mesh
    let mut mesh = Mesh::new(bevy::render::mesh::PrimitiveTopology::TriangleList);
    
    // Horizontal bar
    let h_vertices = vec![
        ([-3.0, 1.0, 0.0], [0.0, 0.0]),
        ([-3.0, -1.0, 0.0], [0.0, 0.0]),
        ([3.0, -1.0, 0.0], [0.0, 0.0]),
        ([3.0, -1.0, 0.0], [0.0, 0.0]),
        ([3.0, 1.0, 0.0], [0.0, 0.0]),
        ([-3.0, 1.0, 0.0], [0.0, 0.0]),
    ];
    
    // Vertical bar
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
    
    mesh.set_indices(Some(bevy::render::mesh::Indices::U32(indices)));
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh
}

fn create_square_mesh(size: f32) -> Mesh {
    Mesh::from(Rectangle::new(size, size))
}
