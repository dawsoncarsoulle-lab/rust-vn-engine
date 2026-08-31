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
            BlueprintThemePlugin,
            BlueprintCameraPlugin,
            NodeRenderingPlugin,
            NodeMaterialPlugin,
            WireRenderingPlugin,
            CanvasRenderingPlugin,
            GlassEffectRenderingPlugin,
            GlassEffectPipelinePlugin,
        ))
        // Add systems
        .add_systems(Startup, setup_blueprint_example)
        .add_systems(Update, (
            handle_node_drag,
            handle_wire_creation,
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
    pub connection_start: Option<(String, String)>,
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
    mut meshes: ResMut<Assets<Mesh>>,
    mut node_materials: ResMut<Assets<NodeMaterial>>,
    mut wire_materials: ResMut<Assets<WireMaterial>>,
    theme: Res<BlueprintTheme>,
    mut state: ResMut<ExampleState>,
) {
    // Create a sample blueprint
    let mut blueprint = Blueprint::new("example", "Example Blueprint");
    
    // Create nodes
    let start_node = Node::event("start", "Event BeginPlay", Vec2::new(100.0, 200.0))
        .with_output(Pin::exec("out", PinDirection::Output));
    
    let branch_node = Node::branch("branch", "Branch", Vec2::new(300.0, 200.0))
        .with_input(Pin::exec("in", PinDirection::Input))
        .with_input(Pin::bool("condition", "Condition", PinDirection::Input))
        .with_output(Pin::exec("true", PinDirection::Output))
        .with_output(Pin::exec("false", PinDirection::Output));
    
    let print_node = Node::function("print", "Print String", Vec2::new(500.0, 100.0))
        .with_input(Pin::exec("in", PinDirection::Input))
        .with_input(Pin::string("text", "In String", PinDirection::Input).with_default("Hello, Blueprint!"))
        .with_output(Pin::exec("out", PinDirection::Output));
    
    let set_var_node = Node::function("set_score", "Set Score", Vec2::new(500.0, 300.0))
        .with_input(Pin::exec("in", PinDirection::Input))
        .with_input(Pin::float("value", "Score", PinDirection::Input))
        .with_output(Pin::exec("out", PinDirection::Output));
    
    let score_var = Node::variable("score_var", "Score", Vec2::new(100.0, 400.0), "Score", "float")
        .with_output(Pin::float("out", PinDirection::Output));
    
    let add_node = Node::math("add", "float + float", Vec2::new(300.0, 400.0))
        .with_input(Pin::float("a", "A", PinDirection::Input))
        .with_input(Pin::float("b", "B", PinDirection::Input).with_default("10.0"))
        .with_output(Pin::float("out", "Return Value", PinDirection::Output));
    
    // Add nodes to blueprint
    blueprint.add_node(start_node);
    blueprint.add_node(branch_node);
    blueprint.add_node(print_node);
    blueprint.add_node(set_var_node);
    blueprint.add_node(score_var);
    blueprint.add_node(add_node);
    
    // Create wires
    blueprint.add_wire(Wire::new("wire1", ("start", "out"), ("branch", "in"), PinKind::Exec));
    blueprint.add_wire(Wire::new("wire2", ("branch", "true"), ("print", "in"), PinKind::Exec));
    blueprint.add_wire(Wire::new("wire3", ("branch", "false"), ("set_score", "in"), PinKind::Exec));
    blueprint.add_wire(Wire::new("wire4", ("score_var", "out"), ("add", "a"), PinKind::Float));
    blueprint.add_wire(Wire::new("wire5", ("add", "out"), ("set_score", "value"), PinKind::Float));
    
    // Store the blueprint
    state.blueprint = blueprint;
    
    // Spawn the blueprint on the canvas
    spawn_blueprint(
        &mut commands,
        &state.blueprint,
        &theme,
        &mut meshes,
        &mut node_materials,
        &mut wire_materials,
    );
    
    // Setup camera
    commands.spawn((
        Camera2dBundle {
            transform: Transform::from_xyz(0.0, 0.0, 999.0),
            ..default()
        },
        BlueprintCamera::default(),
    ));
}

/// Handle node dragging
fn handle_node_drag(
    mut state: ResMut<ExampleState>,
    mouse_buttons: Res<Input<MouseButton>>,
    windows: Query<&Window>,
    camera_query: Query<(&Camera, &GlobalTransform)>
) {
    let window = windows.single();
    let (camera, camera_transform) = camera_query.single();
    
    if let Some(cursor_pos) = window.cursor_position() {
        // Convert screen to world coordinates
        if let Some(world_pos) = camera.viewport_to_world_2d(camera_transform, cursor_pos) {
            match &mut state.drag_state {
                Some(drag) => {
                    // Update current position
                    drag.current_position = world_pos;
                    
                    // Check if mouse is released
                    if !mouse_buttons.pressed(MouseButton::Left) {
                        // Update the node position in the blueprint
                        if let Some(node) = state.blueprint.find_node_mut(&drag.node_id) {
                            node.position = drag.current_position;
                        }
                        state.drag_state = None;
                    }
                }
                None => {
                    // Check if we should start dragging
                    if mouse_buttons.just_pressed(MouseButton::Left) {
                        // Check if we clicked on a node
                        if let Some(node_id) = state.selection.clone() {
                            state.drag_state = Some(DragState {
                                node_id,
                                start_position: world_pos,
                                current_position: world_pos,
                            });
                        }
                    }
                }
            }
        }
    }
}

/// Handle wire creation
fn handle_wire_creation(
    mut state: ResMut<ExampleState>,
    mouse_buttons: Res<Input<MouseButton>>,
    windows: Query<&Window>,
    camera_query: Query<(&Camera, &GlobalTransform)>
) {
    let window = windows.single();
    let (camera, camera_transform) = camera_query.single();
    
    if let Some(cursor_pos) = window.cursor_position() {
        if let Some(world_pos) = camera.viewport_to_world_2d(camera_transform, cursor_pos) {
            match &mut state.connection_start {
                Some((node_id, pin_id)) => {
                    // Check if we clicked on another pin to complete the connection
                    if mouse_buttons.just_released(MouseButton::Left) {
                        // Find the pin under the cursor
                        // This would involve checking all pins for intersection
                        // For now, just clear the connection start
                        state.connection_start = None;
                    }
                }
                None => {
                    // Check if we clicked on an output pin to start a connection
                    if mouse_buttons.just_pressed(MouseButton::Left) {
                        if let Some(node_id) = state.selection.clone() {
                            // Check if the node has output pins
                            if let Some(node) = state.blueprint.find_node(&node_id) {
                                if !node.outputs.is_empty() {
                                    // Start connection from first output pin
                                    state.connection_start = Some((node_id, node.outputs[0].id.clone()));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Handle node selection
fn handle_node_selection(
    mut state: ResMut<ExampleState>,
    mouse_buttons: Res<Input<MouseButton>>,
    windows: Query<&Window>,
    camera_query: Query<(&Camera, &GlobalTransform)>
) {
    let window = windows.single();
    let (camera, camera_transform) = camera_query.single();
    
    if let Some(cursor_pos) = window.cursor_position() {
        if let Some(world_pos) = camera.viewport_to_world_2d(camera_transform, cursor_pos) {
            if mouse_buttons.just_pressed(MouseButton::Left) {
                // Find the node under the cursor
                let mut new_selection = None;
                
                for node in &state.blueprint.nodes {
                    let (min, max) = node.bounds();
                    if world_pos.x >= min.x && world_pos.x <= max.x &&
                       world_pos.y >= min.y && world_pos.y <= max.y {
                        new_selection = Some(node.id.clone());
                        break;
                    }
                }
                
                state.selection = new_selection;
            }
        }
    }
}
