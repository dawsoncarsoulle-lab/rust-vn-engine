//! # R-Blueprint
//!
//! A 100% Rust framework for creating Unreal Engine-style blueprint/node-graph systems.
//! 
//! ## Features
//! - Node-based visual scripting
//! - Custom shaders for connections (wires)
//! - Glass effect (blur + distortion) like Unreal Engine
//! - Full Bevy integration
//! - Serialization/deserialization support

pub mod core;
pub mod rendering;
pub mod shaders;
pub mod utils;

// Re-export main types for convenience
pub use core::*;
pub use rendering::*;
pub use shaders::*;
