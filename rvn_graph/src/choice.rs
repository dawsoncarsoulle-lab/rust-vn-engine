use crate::{GraphDocument, GraphEditError, NodeId, NodeKind, PinId, PropertyValue};

pub(crate) fn default_marker(key: &str) -> String {
    format!("@choice_default:{key}")
}

/// Option and availability sockets share an index, not a position in the
/// owner's pin list. This remains stable when the list grows or is reordered.
pub fn choice_option_index(key: &str) -> Option<usize> {
    key.strip_prefix("option_")?.trim_end_matches("_condition").parse().ok()
}

impl GraphDocument {
    /// A vacant availability socket means "always". Legacy source expressions
    /// remain authoritative until migrated to a wire; never show a misleading
    /// checked checkbox over one of those expressions.
    pub fn choice_condition_default(&self, pin: PinId) -> Option<bool> {
        let pin = self.pins.get(&pin)?;
        let owner = self.nodes.get(&pin.node)?;
        if owner.kind != NodeKind::Choice || !pin.key.ends_with("_condition") {
            return None;
        }
        if matches!(owner.properties.get(&pin.key), Some(PropertyValue::String(value)) if !value.trim().is_empty()) {
            return None;
        }
        if owner.properties.get(&default_marker(&pin.key)) == Some(&PropertyValue::Bool(true)) {
            if let Some(PropertyValue::Bool(value)) = pin.default_value {
                return Some(value);
            }
        }
        Some(true)
    }

    /// Remove one answer without replacing any surviving socket identities,
    /// defaults or wires. Only the removed answer's two sockets disappear.
    pub fn remove_choice_option(&mut self, node: NodeId, index: usize) -> Result<(), GraphEditError> {
        let owner = self.nodes.get(&node).ok_or(GraphEditError::NodeNotFound(node))?;
        if owner.kind != NodeKind::Choice {
            return Err(GraphEditError::WrongNodeKind { node, expected: NodeKind::Choice, found: owner.kind });
        }
        let Some(PropertyValue::StringList(options)) = owner.properties.get("options") else {
            return Err(GraphEditError::InvalidPropertyType { node, key: "options".into() });
        };
        if index >= options.len() {
            return Err(GraphEditError::PinKeyNotFound { node, key: format!("option_{index}") });
        }
        let count = options.len();
        let removed: Vec<_> = [format!("option_{index}"), format!("option_{index}_condition")]
            .iter().filter_map(|key| self.pin_by_key(node, key).map(|pin| pin.id)).collect();
        let owner = self.nodes.get_mut(&node).unwrap();
        if let Some(PropertyValue::StringList(options)) = owner.properties.get_mut("options") {
            options.remove(index);
        }
        owner.pins.retain(|pin| !removed.contains(pin));
        self.pins.retain(|id, _| !removed.contains(id));
        self.edges.retain(|_, edge| !removed.contains(&edge.input) && !removed.contains(&edge.output));
        // Shift keys, not identities; every surviving connection stays put.
        for old in index + 1..count {
            for suffix in ["", "_condition"] {
                let from = format!("option_{old}{suffix}");
                let to = format!("option_{}{suffix}", old - 1);
                if let Some(id) = self.pin_by_key(node, &from).map(|pin| pin.id) {
                    self.pins.get_mut(&id).unwrap().key = to;
                }
            }
        }
        let owner = self.nodes.get_mut(&node).unwrap();
        for prefix in ["", "@choice_default:"] {
            owner.properties.remove(&format!("{prefix}option_{index}_condition"));
            for old in index + 1..count {
                let value = owner.properties.remove(&format!("{prefix}option_{old}_condition"));
                if let Some(value) = value {
                    owner.properties.insert(format!("{prefix}option_{}_condition", old - 1), value);
                }
            }
        }
        Ok(())
    }
}
