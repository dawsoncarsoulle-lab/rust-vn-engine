# R-Blueprint

A **100% Rust** framework for creating Unreal Engine-style blueprint/node-graph systems. This framework provides a complete solution for building visual scripting editors, node-based UIs, and other blueprint-like systems.

## Features

- ✅ **Core Types**: Node, Pin, Wire, Blueprint, Graph with full serialization support
- ✅ **Rendering**: Bevy-based rendering with custom shaders
- ✅ **Shaders**: Custom WGSL shaders for nodes, wires, and glass effects
- ✅ **Glass Effect**: Unreal Engine-style frosted glass effect with blur and distortion
- ✅ **Camera Controls**: Pan and zoom with mouse
- ✅ **Interactions**: Node dragging, selection, and connection creation
- 🔄 **WIP**: Full interaction system (drag & drop, connections)

## Design Inspiration

This framework is inspired by:
- **Unreal Engine Blueprints** - Visual scripting system
- **Blender Node Editor** - Node-based compositing
- **Substance Designer** - Material node graphs

The design HTML you provided serves as the visual reference for the final look and feel.

## Structure

```
r-blueprint/
├── src/
│   ├── core/           # Core types (Node, Pin, Wire, Blueprint)
│   │   ├── node.rs     # Node definitions and types
│   │   ├── pin.rs      # Pin/port definitions
│   │   ├── wire.rs     # Wire/connection definitions
│   │   ├── blueprint.rs# Blueprint graph definitions
│   │   └── graph.rs    # Graph operations and utilities
│   │
│   ├── rendering/      # Rendering systems
│   │   ├── theme.rs    # Theme and styling
│   │   ├── camera.rs   # Camera controls
│   │   ├── node.rs     # Node rendering
│   │   ├── wire.rs     # Wire rendering
│   │   ├── canvas.rs   # Canvas rendering
│   │   └── glass.rs    # Glass effect rendering
│   │
│   ├── shaders/        # Custom shaders
│   │   ├── node.rs     # Node shader
│   │   ├── wire.rs     # Wire shader
│   │   └── glass.rs    # Glass effect shader
│   │
│   ├── utils/          # Utilities
│   │   ├── id.rs       # ID generation
│   │   ├── serialization.rs # Serialization helpers
│   │   └── geometry.rs  # Geometry utilities
│   │
│   └── lib.rs          # Main library exports
│
├── assets/
│   └── shaders/        # WGSL shader files
│       ├── node_material.wgsl
│       ├── wire_material.wgsl
│       └── glass_material.wgsl
│
├── examples/
│   ├── basic_blueprint.rs  # Basic blueprint example
│   └── glass_effect.rs      # Glass effect demonstration
│
├── Cargo.toml
└── README.md
```

## Core Types

### Node
Represents a node in the graph with:
- Unique ID
- Title and type (Event, Function, Branch, Math, Variable, etc.)
- Position and size
- Input and output pins
- Custom data

### Pin
Represents a connection point with:
- Unique ID
- Label
- Type (Exec, Bool, Float, Int, String, Object, Any)
- Direction (Input/Output)
- Default value
- Connection state

### Wire
Represents a connection between pins with:
- Unique ID
- Source and target (node_id, pin_id)
- Type
- Control points for bezier curves
- Selection/highlight state

### Blueprint
Represents a complete graph with:
- Unique ID and name
- List of nodes
- List of wires
- Camera position and zoom
- Variables
- Metadata

## Usage

### Basic Setup

```rust
use bevy::prelude::*;
use r_blueprint::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(BlueprintPlugins)
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands) {
    // Create a blueprint
    let mut blueprint = Blueprint::new("my_blueprint", "My Blueprint");
    
    // Add nodes
    let start = Node::event("start", "Event BeginPlay", Vec2::new(100.0, 200.0))
        .with_output(Pin::exec("out", PinDirection::Output));
    
    blueprint.add_node(start);
    
    // Spawn the blueprint
    commands.spawn(BlueprintBundle::new(blueprint));
}
```

### Creating a Node

```rust
use r_blueprint::core::*;
use glam::Vec2;

let node = Node::new("my_node", "My Node", NodeType::Function, Vec2::new(0.0, 0.0))
    .with_input(Pin::exec("in", PinDirection::Input))
    .with_input(Pin::float("value", "Value", PinDirection::Input))
    .with_output(Pin::exec("out", PinDirection::Output));
```

### Creating Connections

```rust
let wire = Wire::new(
    "my_wire",
    ("node1", "out"),
    ("node2", "in"),
    PinKind::Exec,
);
```

## Glass Effect

The glass effect is implemented using:
1. **Render Target**: Captures the scene to a texture
2. **Custom Shader**: Applies blur and distortion effects
3. **Full-Screen Quad**: Renders the glass overlay

### Configuration

```rust
use r_blueprint::rendering::GlassEffectSettings;

let settings = GlassEffectSettings {
    enabled: true,
    intensity: 0.5,
    tint: Color::rgba(0.1, 0.1, 0.2, 0.3),
    blur_amount: 0.5,
    distortion_amount: 0.001,
    edge_darkness: 0.3,
};
```

## Examples

### Basic Blueprint
```bash
cargo run --example basic_blueprint
```

Demonstrates:
- Node creation and rendering
- Wire connections
- Camera controls (pan with middle mouse, zoom with wheel)
- Basic node dragging

### Glass Effect
```bash
cargo run --example glass_effect
```

Demonstrates:
- Full-screen glass effect
- Animated glass panels
- Blur and distortion effects

## Customization

### Themes

```rust
use r_blueprint::rendering::BlueprintTheme;

let mut theme = BlueprintTheme::default();
theme.accent = Color::hex("#ffb443").unwrap();
theme.background_app = Color::hex("#131316").unwrap();
// ... customize other colors
```

### Node Types

```rust
use r_blueprint::core::{NodeType, NodeIcon};

// Define custom node types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MyNodeType {
    CustomNode,
    SpecialNode,
}

impl From<MyNodeType> for NodeType {
    fn from(t: MyNodeType) -> Self {
        match t {
            MyNodeType::CustomNode => NodeType::Custom,
            MyNodeType::SpecialNode => NodeType::Custom,
        }
    }
}
```

## Roadmap

- [x] Core type definitions
- [x] Basic rendering system
- [x] Custom shaders
- [x] Glass effect implementation
- [ ] Full interaction system
  - [ ] Node dragging and snapping
  - [ ] Wire creation with mouse
  - [ ] Node selection and multi-selection
  - [ ] Copy/paste nodes
  - [ ] Delete nodes and wires
- [ ] Advanced features
  - [ ] Node groups/collapsing
  - [ ] Comments/annotations
  - [ ] Undo/redo
  - [ ] Zoom to selection
  - [ ] Mini-map
- [ ] Performance optimizations
  - [ ] Spatial partitioning for large graphs
  - [ ] LOD for distant nodes
  - [ ] Wire simplification
- [ ] Serialization formats
  - [x] JSON
  - [x] RON
  - [x] Bincode
  - [ ] MessagePack
- [ ] Integration
  - [ ] Bevy UI integration
  - [ ] egui integration
  - [ ] Web/WASM support

## Comparison with HTML Design

The HTML design you provided features:

### Colors
- Background: `#131316` (dark gray)
- Canvas: `#18181c` (slightly lighter)
- Panels: `#1b1b20` (panel background)
- Accent: `#ffb443` (orange)

### Node Types
- **Event**: Red gradient (`#a13139` to `#701f26`)
- **Function**: Blue gradient (`#2064ab` to `#164a86`)
- **Branch**: Gray gradient (`#454f61` to `#2f3846`)
- **Math**: Green gradient (`#1f8f79` to `#14685a`)

### Pin Colors
- Exec: White (`#f3f3f5`)
- Bool: Red (`#c1444d`)
- Float: Green (`#5fd1a3`)
- Int: Blue (`#39c6d9`)
- String: Purple (`#e56bd6`)
- Object: Light Blue (`#5a93f5`)

All of these are implemented in the `BlueprintTheme` struct.

## Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## License

This project is licensed under either:
- **MIT License** ([LICENSE-MIT](LICENSE-MIT))
- **Apache License 2.0** ([LICENSE-APACHE](LICENSE-APACHE))

at your option.

## Credits

- Inspired by **Unreal Engine Blueprints**
- Built with **Bevy Engine**
- Designed based on the provided HTML/CSS reference
