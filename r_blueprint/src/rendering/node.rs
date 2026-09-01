//! Node rendering for blueprints

use bevy::prelude::*;
use crate::core::{Node, NodeType, NodeIcon, Pin, PinKind, PinDirection};
use crate::rendering::theme::BlueprintTheme;
use crate::shaders::node::{NodeMaterial, NodeMaterialPlugin, create_node_material, create_node_icon_mesh};

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
        app.add_plugins(NodeMaterialPlugin)
            .add_systems(Startup, setup_node_materials)
            .add_systems(Update, (
                update_node_visuals,
                update_node_icon_colors,
            ));
    }
}

/// System to setup node materials
fn setup_node_materials(mut commands: Commands) {
    // Node materials are created dynamically, no need for ambient light
}

/// Spawn a complete node with all its components
pub fn spawn_node(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<NodeMaterial>>,
    node: &Node,
    position: Vec2,
    theme: &BlueprintTheme,
) -> Entity {
    let node_id = node.id.clone();
    let node_type = node.node_type.clone();
    
    // Create node material
    let node_material = create_node_material(node_type, theme, false, false);
    let material_handle = materials.add(node_material);
    
    // Create node mesh (rectangle for body)
    let node_width = theme.node_width;
    let node_height = theme.node_height;
    let body_mesh = Mesh::from(Rectangle::new(node_width, node_height));
    let body_mesh_handle = meshes.add(body_mesh);
    
    // Create header mesh
    let header_height = theme.node_header_height;
    let header_mesh = Mesh::from(Rectangle::new(node_width, header_height));
    let header_mesh_handle = meshes.add(header_mesh);
    
    // Create node entity
    let node_entity = commands.spawn((
        NodeBundle::new(node_id.clone(), position),
        Name::new(format!("Node_{}", node_id)),
    )).id();
    
    // Spawn body
    commands.spawn((
        MaterialMeshBundle {
            mesh: body_mesh_handle,
            material: material_handle.clone(),
            transform: Transform::from_translation(Vec3::new(0.0, -header_height/2.0, 0.0)),
            ..default()
        },
        NodeBody,
        ChildOf(node_entity),
    ));
    
    // Spawn header
    commands.spawn((
        MaterialMeshBundle {
            mesh: header_mesh_handle,
            material: material_handle.clone(),
            transform: Transform::from_translation(Vec3::new(0.0, node_height/2.0 - header_height/2.0, 0.0)),
            ..default()
        },
        NodeHeader,
        ChildOf(node_entity),
    ));
    
    // Spawn title text
    commands.spawn((
        Text2dBundle {
            text: Text::from_section(
                node.name.clone(),
                TextStyle {
                    font_size: theme.font_size,
                    color: theme.text_color,
                    ..default()
                },
            ),
            transform: Transform::from_translation(Vec3::new(0.0, node_height/2.0 - header_height/2.0, 0.1)),
            ..default()
        },
        NodeTitle,
        ChildOf(node_entity),
    ));
    
    // Spawn icon
    let icon_mesh = create_node_icon_mesh(node.icon);
    let icon_mesh_handle = meshes.add(icon_mesh);
    
    let icon_color = theme.node_icon_color;
    let icon_material = materials.add(create_node_material(node_type, theme, false, false));
    
    commands.spawn((
        MaterialMeshBundle {
            mesh: icon_mesh_handle,
            material: icon_material,
            transform: Transform::from_translation(Vec3::new(-node_width/2.0 + 15.0, node_height/2.0 - header_height/2.0, 0.2)),
            ..default()
        },
        NodeIconComponent { icon_type: node.icon },
        ChildOf(node_entity),
    ));
    
    // Spawn pins
    for (i, input_pin) in node.inputs.iter().enumerate() {
        spawn_pin(
            commands,
            meshes,
            materials,
            node_id.clone(),
            input_pin.id.clone(),
            input_pin.pin_type,
            PinDirection::Input,
            input_pin.name.clone(),
            Vec2::new(-node_width/2.0, node_height/2.0 - header_height - 20.0 - i as f32 * 25.0),
            theme,
            node_entity,
        );
    }
    
    for (i, output_pin) in node.outputs.iter().enumerate() {
        spawn_pin(
            commands,
            meshes,
            materials,
            node_id.clone(),
            output_pin.id.clone(),
            output_pin.pin_type,
            PinDirection::Output,
            output_pin.name.clone(),
            Vec2::new(node_width/2.0, node_height/2.0 - header_height - 20.0 - i as f32 * 25.0),
            theme,
            node_entity,
        );
    }
    
    node_entity
}

/// Spawn a pin
pub fn spawn_pin(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<NodeMaterial>>,
    node_id: String,
    pin_id: String,
    pin_type: PinKind,
    direction: PinDirection,
    name: String,
    position: Vec2,
    theme: &BlueprintTheme,
    parent: Entity,
) -> Entity {
    let pin_color = theme.pin_colors.get(&pin_type).copied().unwrap_or(theme.pin_default_color);
    
    // Create pin circle mesh
    let pin_mesh = Mesh::from(Circle::new(6.0));
    let pin_mesh_handle = meshes.add(pin_mesh);
    
    // Create pin material
    let pin_material = materials.add(create_node_material(NodeType::Custom, theme, false, false));
    
    // Spawn pin
    let pin_entity = commands.spawn((
        MaterialMeshBundle {
            mesh: pin_mesh_handle,
            material: pin_material,
            transform: Transform::from_translation(position.extend(0.0)),
            ..default()
        },
        BlueprintPin {
            node_id,
            pin_id,
            pin_type,
            direction,
            is_connected: false,
            is_hovered: false,
        },
        ChildOf(parent),
    )).id();
    
    // Spawn pin label
    commands.spawn((
        Text2dBundle {
            text: Text::from_section(
                name,
                TextStyle {
                    font_size: theme.font_size_small,
                    color: theme.text_color,
                    ..default()
                },
            ),
            transform: Transform::from_translation(Vec3::new(
                position.x + if direction == PinDirection::Input { -80.0 } else { 10.0 },
                position.y,
                0.1,
            )),
            ..default()
        },
        PinLabel { text: name },
        ChildOf(parent),
    ));
    
    pin_entity
}

/// System to update node visuals based on state
fn update_node_visuals(
    mut query: Query<(&BlueprintNode, &NodeVisualState, &mut Transform)>, 
) {
    // Node visual state updates will be handled here
}

/// System to update node icon colors
fn update_node_icon_colors(
    mut query: Query<(&NodeIconComponent, &mut Handle<NodeMaterial>), With<ChildOf>>,
    materials: ResMut<Assets<NodeMaterial>>,
    theme: Res<BlueprintTheme>,
) {
    for (icon, material_handle) in &mut query {
        if let Some(material) = materials.get_mut(material_handle) {
            // Update icon material based on node type
            let node_type = match icon.icon_type {
                NodeIcon::Triangle => NodeType::Event,
                NodeIcon::Dot => NodeType::Variable,
                NodeIcon::Diamond => NodeType::Branch,
                NodeIcon::Plus => NodeType::Math,
                _ => NodeType::Function,
            };
            *material = create_node_material(node_type, &theme, false, false);
        }
    }
}

/// Update node selection state
pub fn update_node_selection(
    mut query: Query<&mut NodeVisualState>,
    selected_node: Option<Res<SelectedNode>>,
) {
    for mut visual_state in &mut query {
        visual_state.is_selected = selected_node.as_ref().map_or(false, |n| n.0 == visual_state.node_id);
    }
}

/// Component for selected node
#[derive(Resource)]
pub struct SelectedNode(pub String);

/// System to handle node selection via mouse
pub fn handle_node_selection(
    mut commands: Commands,
    mouse_button_input: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    cameras: Query<(&Camera, &GlobalTransform)>,
    node_query: Query<(Entity, &BlueprintNode, &Transform)>, 
    mut selected_node: Option<ResMut<SelectedNode>>,
) {
    if mouse_button_input.just_pressed(MouseButton::Left) {
        let window = windows.single();
        if let Some(cursor_position) = window.cursor_position() {
            for (camera, camera_transform) in &cameras {
                if let Some(ray) = camera.viewport_to_world(camera_transform, cursor_position) {
                    for (entity, node, transform) in &node_query {
                        // Simple bounding box check for node selection
                        let node_size = Vec2::new(120.0, 80.0); // Approximate node size
                        let min = transform.translation.truncate() - node_size / 2.0;
                        let max = transform.translation.truncate() + node_size / 2.0;
                        
                        if ray.x >= min.x && ray.x <= max.x && ray.y >= min.y && ray.y <= max.y {
                            commands.entity(entity).insert(NodeVisualState {
                                is_selected: true,
                                is_highlighted: false,
                                is_hovered: false,
                            });
                            
                            if let Some(mut selected) = selected_node {
                                selected.0 = node.node_id.clone();
                            } else {
                                commands.insert_resource(SelectedNode(node.node_id.clone()));
                            }
                            return;
                        }
                    }
                }
            }
        }
    }
}
