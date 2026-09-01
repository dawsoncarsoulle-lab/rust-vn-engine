//! ID generation utilities

use std::sync::atomic::{AtomicU64, Ordering};

/// Global counter for generating unique IDs
static ID_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Generate a unique ID
pub fn generate_id(prefix: &str) -> String {
    let count = ID_COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("{}_{}", prefix, count)
}

/// Generate a node ID
pub fn generate_node_id() -> String {
    generate_id("node")
}

/// Generate a wire ID
pub fn generate_wire_id() -> String {
    generate_id("wire")
}

/// Generate a pin ID
pub fn generate_pin_id() -> String {
    generate_id("pin")
}

/// Generate a blueprint ID
pub fn generate_blueprint_id() -> String {
    generate_id("bp")
}

/// Reset the ID counter (useful for testing)
pub fn reset_id_counter() {
    ID_COUNTER.store(0, Ordering::SeqCst);
}

/// Trait for types that can generate IDs
pub trait IdGenerator {
    fn generate_id(&self) -> String;
}

impl IdGenerator for String {
    fn generate_id(&self) -> String {
        generate_id(self)
    }
}

impl IdGenerator for &str {
    fn generate_id(&self) -> String {
        generate_id(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_generate_id() {
        reset_id_counter();
        
        let id1 = generate_id("test");
        let id2 = generate_id("test");
        
        assert_eq!(id1, "test_0");
        assert_eq!(id2, "test_1");
    }
    
    #[test]
    fn test_generate_node_id() {
        reset_id_counter();
        
        let id = generate_node_id();
        assert!(id.starts_with("node_"));
    }
    
    #[test]
    fn test_generate_wire_id() {
        reset_id_counter();
        
        let id = generate_wire_id();
        assert!(id.starts_with("wire_"));
    }
}
