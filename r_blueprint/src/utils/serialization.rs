//! Serialization utilities for blueprints

use serde::{Serialize, Deserialize};
use serde_json::{Value, to_value, from_value};
use std::path::Path;
use std::fs::File;
use std::io::{Read, Write};
use crate::core::Blueprint;

/// Serialization format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SerializationFormat {
    Json,
    Ron,
    Bincode,
}

impl Default for SerializationFormat {
    fn default() -> Self {
        Self::Json
    }
}

/// Save a blueprint to a file
pub fn save_blueprint(
    blueprint: &Blueprint,
    path: impl AsRef<Path>,
    format: SerializationFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    let path = path.as_ref();
    
    match format {
        SerializationFormat::Json => {
            let json = serde_json::to_string_pretty(blueprint)?;
            let mut file = File::create(path)?;
            file.write_all(json.as_bytes())?;
        }
        SerializationFormat::Ron => {
            let ron = ron::to_string(blueprint)?;
            let mut file = File::create(path)?;
            file.write_all(ron.as_bytes())?;
        }
        SerializationFormat::Bincode => {
            let bytes = bincode::serialize(blueprint)?;
            let mut file = File::create(path)?;
            file.write_all(&bytes)?;
        }
    }
    
    Ok(())
}

/// Load a blueprint from a file
pub fn load_blueprint(
    path: impl AsRef<Path>,
    format: SerializationFormat,
) -> Result<Blueprint, Box<dyn std::error::Error>> {
    let path = path.as_ref();
    
    let mut file = File::open(path)?;
    let mut contents = Vec::new();
    file.read_to_end(&mut contents)?;
    
    match format {
        SerializationFormat::Json => {
            let json = String::from_utf8(contents)?;
            Ok(serde_json::from_str(&json)?)
        }
        SerializationFormat::Ron => {
            let ron = String::from_utf8(contents)?;
            Ok(ron::from_str(&ron)?)
        }
        SerializationFormat::Bincode => {
            Ok(bincode::deserialize(&contents)?)
        }
    }
}

/// Serialize a blueprint to a string
pub fn blueprint_to_string(
    blueprint: &Blueprint,
    format: SerializationFormat,
) -> Result<String, Box<dyn std::error::Error>> {
    match format {
        SerializationFormat::Json => {
            Ok(serde_json::to_string_pretty(blueprint)?)
        }
        SerializationFormat::Ron => {
            Ok(ron::to_string(blueprint)?)
        }
        SerializationFormat::Bincode => {
            let bytes = bincode::serialize(blueprint)?;
            Ok(base64::encode(bytes))
        }
    }
}

/// Deserialize a blueprint from a string
pub fn blueprint_from_string(
    data: &str,
    format: SerializationFormat,
) -> Result<Blueprint, Box<dyn std::error::Error>> {
    match format {
        SerializationFormat::Json => {
            Ok(serde_json::from_str(data)?)
        }
        SerializationFormat::Ron => {
            Ok(ron::from_str(data)?)
        }
        SerializationFormat::Bincode => {
            let bytes = base64::decode(data)?;
            Ok(bincode::deserialize(&bytes)?)
        }
    }
}

/// Convert a blueprint to a Value for custom serialization
pub fn blueprint_to_value(blueprint: &Blueprint) -> Value {
    to_value(blueprint).unwrap_or(Value::Null)
}

/// Convert a Value to a blueprint
pub fn value_to_blueprint(value: Value) -> Result<Blueprint, Box<dyn std::error::Error>> {
    from_value(value).map_err(|e| e.into())
}

/// Clone a blueprint with new IDs
pub fn clone_blueprint_with_new_ids(blueprint: &Blueprint) -> Blueprint {
    use crate::utils::id::{generate_node_id, generate_wire_id};
    use std::collections::HashMap;
    
    let mut id_map = HashMap::new();
    
    // Clone nodes with new IDs
    let mut new_nodes = Vec::new();
    for node in &blueprint.nodes {
        let new_id = generate_node_id();
        id_map.insert(node.id.clone(), new_id.clone());
        
        let mut new_node = node.clone();
        new_node.id = new_id;
        new_nodes.push(new_node);
    }
    
    // Clone wires with new IDs and updated node references
    let mut new_wires = Vec::new();
    for wire in &blueprint.wires {
        let new_id = generate_wire_id();
        let new_from = (
            id_map.get(&wire.from.0).cloned().unwrap_or_else(|| wire.from.0.clone()),
            wire.from.1.clone(),
        );
        let new_to = (
            id_map.get(&wire.to.0).cloned().unwrap_or_else(|| wire.to.0.clone()),
            wire.to.1.clone(),
        );
        
        let mut new_wire = wire.clone();
        new_wire.id = new_id;
        new_wire.from = new_from;
        new_wire.to = new_to;
        new_wires.push(new_wire);
    }
    
    Blueprint {
        id: generate_id("bp"),
        nodes: new_nodes,
        wires: new_wires,
        ..blueprint.clone()
    }
}

/// Merge two blueprints
pub fn merge_blueprints(
    bp1: &Blueprint,
    bp2: &Blueprint,
) -> Blueprint {
    use crate::utils::id::{generate_blueprint_id};
    
    let mut merged = Blueprint {
        id: generate_blueprint_id(),
        name: format!("{} + {}", bp1.name, bp2.name),
        ..Default::default()
    };
    
    // Add nodes from both blueprints
    merged.nodes.extend_from_slice(&bp1.nodes);
    merged.nodes.extend_from_slice(&bp2.nodes);
    
    // Add wires from both blueprints
    merged.wires.extend_from_slice(&bp1.wires);
    merged.wires.extend_from_slice(&bp2.wires);
    
    merged
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Node, NodeType, Pin, PinKind, PinDirection, Wire};
    use glam::Vec2;
    
    #[test]
    fn test_save_and_load_blueprint() {
        let mut blueprint = Blueprint::new("test", "Test Blueprint");
        
        let node = Node::new("test_node", "Test Node", NodeType::Function, Vec2::ZERO)
            .with_input(Pin::exec("in", PinDirection::Input))
            .with_output(Pin::exec("out", PinDirection::Output));
        
        blueprint.add_node(node);
        
        let wire = Wire::new("test_wire", ("test_node", "out"), ("test_node", "in"), PinKind::Exec);
        blueprint.add_wire(wire);
        
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("test_blueprint.json");
        
        save_blueprint(&blueprint, &path, SerializationFormat::Json).unwrap();
        let loaded = load_blueprint(&path, SerializationFormat::Json).unwrap();
        
        assert_eq!(loaded.name, blueprint.name);
        assert_eq!(loaded.nodes.len(), blueprint.nodes.len());
    }
    
    #[test]
    fn test_blueprint_to_string() {
        let mut blueprint = Blueprint::new("test", "Test Blueprint");
        
        let node = Node::new("test_node", "Test Node", NodeType::Function, Vec2::ZERO);
        blueprint.add_node(node);
        
        let json = blueprint_to_string(&blueprint, SerializationFormat::Json).unwrap();
        assert!(json.contains("Test Blueprint"));
    }
}
