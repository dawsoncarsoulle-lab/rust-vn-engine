//! Basic blueprint example
//! This demonstrates a simple blueprint with nodes and connections

use bevy::prelude::*;
use r_blueprint::core::*;
use r_blueprint::rendering::*;
use r_blueprint::shaders::*;
use glam::Vec2;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        // Add blueprint plugins
        .add_plugins((
            NodeRenderingPlugin,
            NodeMaterialPlugin,
            WireRenderingPlugin,
            WireMaterialPlugin,
            CanvasPlugin,
            CameraPlugin,
            GlassEffectPlugin,
            GlassMaterialPlugin,
        ))
        // Add systems
        .add_systems(Startup, setup_blueprint_example)
        .add_systems(Update, (
            handle_node_drag,
            handle_node_selection,
        ))
        .run();
}

/// State for the example
#[derive(Debug, Default, Resource)]
pub struct ExampleState {
    pub blueprint: Blueprint,
    pub drag_state: Option<DragState>,
    pub selection: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DragState {
    pub node_id: String,
    pub start_position: Vec2,
    pub current_position: Vec2,
}

/// Setup the example blueprint
fn setup_blueprint_example(
    mut commands: Commands,
    mut state: ResMut<ExampleState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut node_materials: ResMut<Assets<NodeMaterial>>,
    theme: Res<BlueprintTheme>,
) {
    // Create a simple blueprint with a few nodes
    let mut blueprint = Blueprint::new("Example Blueprint".to_string());
    
    // Add an event node
    let event_node = Node::new(
        "event".to_string(),
        "Start".to_string(),
        NodeType::Event,
        NodeIcon::Triangle,
        Vec2::new(-200.0, 0.0),
    );
    blueprint.add_node(event_node);
    
    // Add a function node
    let function_node = Node::new(
        "function".to_string(),
        "Print Message".to_string(),
        NodeType::Function,
        NodeIcon::Plus,
        Vec2::new(100.0, 0.0),
    );
    blueprint.add_node(function_node);
    
    // Add a branch node
    let branch_node = Node::new(
        "branch".to_string(),
        "If Condition".to_string(),
        NodeType::Branch,
        NodeIcon::Diamond,
        Vec2::new(-50.0, -150.0),
    );
    blueprint.add_node(branch_node);
    
    state.blueprint = blueprint;
    
    // Spawn nodes in the scene
    for node in &state.blueprint.nodes {
        spawn_node(
            &mut commands,
            &mut meshes,
            &mut node_materials,
            node,
            node.position,
            &theme,
        );
    }
    
    // Insert theme resource
    commands.insert_resource(theme.clone());
}

/// Handle node dragging
fn handle_node_drag(
    mut commands: Commands,
    mouse_button_input: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    cameras: Query<(&Camera, &GlobalTransform)>,
    mut node_query: Query<(Entity, &BlueprintNode, &mut Transform)>, 
    mut state: ResMut<ExampleState>,
) {
    let window = windows.single();
    
    if mouse_button_input.just_pressed(MouseButton::Left) {
        if let Some(cursor_position) = window.cursor_position() {
            for (camera, camera_transform) in &cameras {
                if let Some(ray) = camera.viewport_to_world(camera_transform, cursor_position) {
                    for (entity, node, transform) in &node_query {
                        let node_size = Vec2::new(120.0, 80.0);
                        let min = transform.translation.truncate() - node_size / 2.0;
                        let max = transform.translation.truncate() + node_size / 2.0;
                        
                        if ray.x >= min.x && ray.x <= max.x && ray.y >= min.y && ray.y <= max.y {
                            state.drag_state = Some(DragState {
                                node_id: node.node_id.clone(),
                                start_position: transform.translation.truncate(),
                                current_position: transform.translation.truncate(),
                            });
                            return;
                        }
                    }
                }
            }
        }
    }
    
    if mouse_button_input.pressed(MouseButton::Left) {
        if let Some(drag_state) = &mut state.drag_state {
            if let Some(cursor_position) = window.cursor_position() {
                for (camera, camera_transform) in &cameras {
                    if let Some(ray) = camera.viewport_to_world(camera_transform, cursor_position) {
                        drag_state.current_position = ray.truncate();
                        
                        // Update node position
                        for (entity, node, mut transform) in &mut node_query {
                            if node.node_id == drag_state.node_id {
                                transform.translation = drag_state.current_position.extend(0.0);
                            }
                        }
                    }
                }
            }
        }
    }
    
    if mouse_button_input.just_released(MouseButton::Left) {
        state.drag_state = None;
    }
}

/// Handle node selection
fn handle_node_selection(
    mut commands: Commands,
    mouse_button_input: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    cameras: Query<(&Camera, &GlobalTransform)>,
    node_query: Query<(Entity, &BlueprintNode, &Transform)>, 
    mut state: ResMut<ExampleState>,
) {
    if mouse_button_input.just_pressed(MouseButton::Left) {
        let window = windows.single();
        if let Some(cursor_position) = window.cursor_position() {
            for (camera, camera_transform) in &cameras {
                if let Some(ray) = camera.viewport_to_world(camera_transform, cursor_position) {
                    for (entity, node, transform) in &node_query {
                        let node_size = Vec2::new(120.0, 80.0);
                        let min = transform.translation.truncate() - node_size / 2.0;
                        let max = transform.translation.truncate() + node_size / 2.0;
                        
                        if ray.x >= min.x && ray.x <= max.x && ray.y >= min.y && ray.y <= max.y {
                            state.selection = Some(node.node_id.clone());
                            println!("Selected node: {}", node.node_id);
                            return;
                        }
                    }
                    
                    // Clicked on empty space - deselect
                    state.selection = None;
                }
            }
        }
    }
}
