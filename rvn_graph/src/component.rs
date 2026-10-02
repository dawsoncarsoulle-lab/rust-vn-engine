//! Semantic editing of component dictionaries, without opaque generated code.
//! Computed dictionaries stay graph expressions and are never overwritten.
use crate::{GraphDocument, NodeId, NodeKind, PinId, PropertyValue};

fn input(graph: &GraphDocument, node: NodeId, key: &str) -> Result<PinId, String> {
    graph
        .pin_by_key(node, key)
        .map(|pin| pin.id)
        .ok_or_else(|| format!("Missing component input {key}"))
}
fn producer(graph: &GraphDocument, pin: PinId) -> Option<NodeId> {
    graph
        .edges
        .values()
        .find(|edge| edge.input == pin)
        .and_then(|edge| graph.pins.get(&edge.output))
        .map(|pin| pin.node)
}
fn literal(graph: &GraphDocument, pin: PinId) -> Result<Option<PropertyValue>, String> {
    if let Some(node) = producer(graph, pin) {
        let node = &graph.nodes[&node];
        if matches!(node.kind, NodeKind::Literal | NodeKind::TextValue) {
            return Ok(node.properties.get("value").cloned());
        }
        return Err("This property is computed: edit its connected Blueprint expression".into());
    }
    Ok(graph.pins[&pin].default_value.clone())
}
fn dictionary(graph: &GraphDocument, pin: PinId) -> Result<Option<NodeId>, String> {
    if graph.pins[&pin].value_type != crate::ValueType::Any {
        return Err("This input does not accept a dictionary".into());
    }
    let Some(node) = producer(graph, pin) else {
        return if graph.pins[&pin].default_value.is_none() {
            Ok(None)
        } else {
            Err("This input contains a value, not an editable dictionary".into())
        };
    };
    let model = &graph.nodes[&node];
    if model.kind == NodeKind::FunctionCall
        && model.properties.get("function") == Some(&PropertyValue::String("dict".into()))
    {
        Ok(Some(node))
    } else {
        Err("This dictionary is computed: edit its connected Blueprint expression".into())
    }
}
fn pair(graph: &GraphDocument, node: NodeId, name: &str) -> Result<Option<PinId>, String> {
    let count = match graph.nodes[&node].properties.get("input_count") {
        Some(PropertyValue::Int(count)) => *count as usize,
        _ => 0,
    };
    if count % 2 != 0 {
        return Err("Dictionary key/value inputs must be paired".into());
    }
    let mut found = None;
    for index in (0..count).step_by(2) {
        let key = literal(graph, input(graph, node, &format!("item_{index}"))?)?;
        let Some(PropertyValue::String(key)) = key else {
            return Err(
                "Dictionary property names must be literal text for inspector editing".into(),
            );
        };
        if key == name {
            if found.is_some() {
                return Err(format!("Duplicate component property {name}"));
            }
            found = Some(input(graph, node, &format!("item_{}", index + 1))?);
        }
    }
    Ok(found)
}

/// Copy just the dictionary on this connection when other consumers use it.
/// Nested edits repeat this at every level, so siblings keep their expressions,
/// identities and values. The caller works on a transaction clone.
fn owned_dictionary(graph: &mut GraphDocument, consumer: PinId) -> Result<Option<NodeId>, String> {
    let Some(id) = dictionary(graph, consumer)? else {
        return Ok(None);
    };
    let output = graph
        .pin_by_key(id, "result")
        .ok_or("Dictionary has no result pin")?
        .id;
    if graph
        .edges
        .values()
        .filter(|edge| edge.output == output)
        .count()
        <= 1
    {
        return Ok(Some(id));
    }
    let model = graph.nodes[&id].clone();
    let cloned = graph.add_node(model.kind, [model.position[0], model.position[1] + 120.0]);
    graph.nodes.get_mut(&cloned).unwrap().properties = model.properties;
    graph.nodes.get_mut(&cloned).unwrap().title_override = model.title_override;
    let mut cloned_output = None;
    for old in model.pins {
        let pin = graph.pins[&old].clone();
        let new = graph
            .add_pin(
                cloned,
                pin.key.clone(),
                pin.label,
                pin.direction,
                pin.value_type,
                pin.cardinality,
            )
            .map_err(|error| error.to_string())?;
        graph.pins.get_mut(&new).unwrap().default_value = pin.default_value;
        if pin.key == "result" {
            cloned_output = Some(new);
        }
        if pin.direction == crate::PinDirection::Input {
            let producers: Vec<_> = graph
                .edges
                .values()
                .filter(|edge| edge.input == old)
                .map(|edge| edge.output)
                .collect();
            for producer in producers {
                graph
                    .connect(producer, new)
                    .map_err(|error| error.to_string())?;
            }
        }
    }
    graph.edges.retain(|_, edge| edge.input != consumer);
    graph
        .connect(
            cloned_output.ok_or("Dictionary has no result pin")?,
            consumer,
        )
        .map_err(|error| error.to_string())?;
    Ok(Some(cloned))
}

fn dictionary_list_slot(
    graph: &mut GraphDocument,
    node: NodeId,
    input_key: &str,
    key: &str,
) -> Result<(NodeId, String), String> {
    let absent = match dictionary(graph, input(graph, node, input_key)?)? {
        Some(dict) => pair(graph, dict, key)?.is_none(),
        None => true,
    };
    if absent {
        graph.set_dictionary_property(
            node,
            input_key,
            &[key],
            PropertyValue::StringList(Vec::new()),
        )?;
    }
    let consumer = input(graph, node, input_key)?;
    let dict = owned_dictionary(graph, consumer)?.ok_or("Missing dictionary")?;
    let pin = pair(graph, dict, key)?.ok_or("Missing collection")?;
    Ok((dict, graph.pins[&pin].key.clone()))
}

impl GraphDocument {
    /// Navigate to the real producer of a named property, including a
    /// calculated dictionary. Reading never freezes or replaces that expression.
    pub fn dictionary_property_source(
        &self,
        node: NodeId,
        input_key: &str,
        path: &[&str],
    ) -> Result<Option<NodeId>, String> {
        let mut value = input(self, node, input_key)?;
        for key in path {
            let Some(parent) = dictionary(self, value)? else {
                return Ok(None);
            };
            let Some(next) = pair(self, parent, key)? else {
                return Ok(None);
            };
            value = next;
        }
        Ok(producer(self, value))
    }

    /// Materialize only an absent authored dictionary. A calculated parent or
    /// final value is protected, and existing data is never regenerated.
    pub fn ensure_dictionary_property(
        &mut self,
        node: NodeId,
        input_key: &str,
        path: &[&str],
    ) -> Result<NodeId, String> {
        if path.is_empty() {
            return Err("Expected a named dictionary property".into());
        }
        if let Some(id) = self.dictionary_property_source(node, input_key, path)? {
            return Ok(id);
        }
        let mut next = self.clone();
        let mut nested = path.to_vec();
        nested.push("__rvn_designer_placeholder");
        next.set_dictionary_property(node, input_key, &nested, PropertyValue::Bool(false))?;
        next.remove_dictionary_property(node, input_key, &nested)?;
        let id = next
            .dictionary_property_source(node, input_key, path)?
            .ok_or("Missing authored dictionary")?;
        *self = next;
        Ok(id)
    }

    /// Attach a reusable style/data expression to an authored dictionary field.
    /// Parents are copied on write. Existing calculated values must be changed
    /// explicitly in the graph, so a designer never silently freezes them.
    pub fn connect_dictionary_property(
        &mut self,
        component: NodeId,
        input_key: &str,
        path: &[&str],
        producer_node: NodeId,
        output_key: &str,
    ) -> Result<(), String> {
        let _ = self.dictionary_property(component, input_key, path)?;
        let output = self
            .pin_by_key(producer_node, output_key)
            .filter(|pin| {
                pin.direction == crate::PinDirection::Output && !pin.value_type.is_execution()
            })
            .ok_or("Expected a data expression output")?
            .id;
        let mut next = self.clone();
        next.set_dictionary_property(component, input_key, path, PropertyValue::Bool(false))?;
        let mut value = input(&next, component, input_key)?;
        for key in path {
            let dictionary = dictionary(&next, value)?.ok_or("Missing authored dictionary")?;
            value = pair(&next, dictionary, key)?.ok_or("Missing authored dictionary field")?;
        }
        next.edges.retain(|_, edge| edge.input != value);
        next.pins.get_mut(&value).unwrap().default_value = None;
        next.connect(output, value)
            .map_err(|error| error.to_string())?;
        *self = next;
        Ok(())
    }

    pub fn dictionary_list_item_properties(
        &self,
        node: NodeId,
        input_key: &str,
        key: &str,
        fields: &[&str],
    ) -> Result<Vec<Vec<Option<PropertyValue>>>, String> {
        let Some(dict) = dictionary(self, input(self, node, input_key)?)? else {
            return Ok(Vec::new());
        };
        let Some(pin) = pair(self, dict, key)? else {
            return Ok(Vec::new());
        };
        let sources = self.list_input_sources(dict, &self.pins[&pin].key)?;
        sources
            .into_iter()
            .map(|node| {
                let node = node.ok_or("Collection item must be an authored dictionary")?;
                let model = &self.nodes[&node];
                if model.kind != NodeKind::FunctionCall
                    || model.properties.get("function")
                        != Some(&PropertyValue::String("dict".into()))
                {
                    return Err("Collection item is computed: edit its Blueprint expression".into());
                }
                fields
                    .iter()
                    .map(|field| {
                        pair(self, node, field)?
                            .map(|pin| literal(self, pin))
                            .transpose()
                            .map(|value| value.flatten())
                    })
                    .collect()
            })
            .collect()
    }
    pub fn set_dictionary_list_item_property(
        &mut self,
        node: NodeId,
        input_key: &str,
        key: &str,
        index: usize,
        field: &str,
        value: PropertyValue,
    ) -> Result<(), String> {
        let _ = self.dictionary_list_item_properties(node, input_key, key, &[field])?;
        let mut next = self.clone();
        let (dict, key) = dictionary_list_slot(&mut next, node, input_key, key)?;
        let child = next.own_list_input_node(dict, &key, index)?;
        let pin = if let Some(pin) = pair(&next, child, field)? {
            pin
        } else {
            let count = match next.nodes[&child].properties.get("input_count") {
                Some(PropertyValue::Int(count)) => *count as usize,
                _ => return Err("Invalid dictionary".into()),
            };
            if count > 126 {
                return Err("Dictionary limit: 64 fields".into());
            }
            next.resize_value_inputs(child, count + 2)
                .map_err(|error| error.to_string())?;
            let name = input(&next, child, &format!("item_{count}"))?;
            next.pins.get_mut(&name).unwrap().default_value =
                Some(PropertyValue::String(field.into()));
            input(&next, child, &format!("item_{}", count + 1))?
        };
        next.edges.retain(|_, edge| edge.input != pin);
        next.pins.get_mut(&pin).unwrap().default_value = Some(value);
        *self = next;
        Ok(())
    }
    pub fn append_dictionary_list_item(
        &mut self,
        node: NodeId,
        input_key: &str,
        key: &str,
        fields: &[(&str, PropertyValue)],
    ) -> Result<(), String> {
        if fields.len() > 64 {
            return Err("Dictionary limit: 64 fields".into());
        }
        let _ = self.dictionary_list_item_properties(node, input_key, key, &[])?;
        let mut next = self.clone();
        let (dict, key) = dictionary_list_slot(&mut next, node, input_key, key)?;
        let position = next.nodes[&node].position;
        let child = next
            .add_catalog_node(
                NodeKind::FunctionCall,
                [position[0] - 600.0, position[1] + 240.0],
            )
            .map_err(|error| error.to_string())?;
        next.set_property(child, "function", PropertyValue::String("dict".into()))
            .map_err(|error| error.to_string())?;
        next.resize_value_inputs(child, fields.len() * 2)
            .map_err(|error| error.to_string())?;
        for (index, (name, value)) in fields.iter().enumerate() {
            for (offset, value) in [PropertyValue::String((*name).into()), value.clone()]
                .into_iter()
                .enumerate()
            {
                let pin = input(&next, child, &format!("item_{}", index * 2 + offset))?;
                next.pins.get_mut(&pin).unwrap().default_value = Some(value);
            }
        }
        next.append_list_input(dict, &key, child, "result")?;
        *self = next;
        Ok(())
    }
    pub fn remove_dictionary_list_item(
        &mut self,
        node: NodeId,
        input_key: &str,
        key: &str,
        index: usize,
    ) -> Result<(), String> {
        let _ = self.dictionary_list_item_properties(node, input_key, key, &[])?;
        let mut next = self.clone();
        let (dict, key) = dictionary_list_slot(&mut next, node, input_key, key)?;
        next.remove_list_input(dict, &key, index)?;
        *self = next;
        Ok(())
    }
    /// Scalar inspector controls for an authored list nested in a dictionary.
    /// Both dictionary and list are copied on write on this consumer only.
    pub fn numeric_dictionary_list_property(
        &self,
        node: NodeId,
        input_key: &str,
        key: &str,
    ) -> Result<Option<Vec<f64>>, String> {
        let Some(dict) = dictionary(self, input(self, node, input_key)?)? else {
            return Ok(None);
        };
        let Some(pin) = pair(self, dict, key)? else {
            return Ok(None);
        };
        self.numeric_list_input_values(dict, &self.pins[&pin].key)
    }
    pub fn set_numeric_dictionary_list_property(
        &mut self,
        node: NodeId,
        input_key: &str,
        key: &str,
        index: usize,
        value: f64,
        defaults: &[f64],
    ) -> Result<(), String> {
        let _ = self.numeric_dictionary_list_property(node, input_key, key)?;
        let mut next = self.clone();
        let absent = match dictionary(&next, input(&next, node, input_key)?)? {
            Some(dict) => pair(&next, dict, key)?.is_none(),
            None => true,
        };
        if absent {
            next.set_dictionary_property(
                node,
                input_key,
                &[key],
                PropertyValue::StringList(Vec::new()),
            )?;
        }
        let consumer = input(&next, node, input_key)?;
        let dict = owned_dictionary(&mut next, consumer)?.ok_or("Missing property dictionary")?;
        let pin = pair(&next, dict, key)?.ok_or("Missing numeric collection")?;
        let input_key = next.pins[&pin].key.clone();
        next.set_numeric_list_input(dict, &input_key, index, value, defaults)?;
        *self = next;
        Ok(())
    }
    /// Literal inputs may be materialized as value nodes on import. Inspector
    /// edits detach that one input, never change a possibly shared producer.
    pub fn literal_input_value(
        &self,
        node: NodeId,
        key: &str,
    ) -> Result<Option<PropertyValue>, String> {
        literal(self, input(self, node, key)?)
    }
    pub fn set_literal_input(
        &mut self,
        node: NodeId,
        key: &str,
        value: PropertyValue,
    ) -> Result<(), String> {
        let pin = input(self, node, key)?;
        let _ = literal(self, pin)?;
        let model = &self.pins[&pin];
        if model.direction != crate::PinDirection::Input || model.value_type.is_execution() {
            return Err("Expected a literal data input".into());
        }
        self.edges.retain(|_, edge| edge.input != pin);
        self.pins.get_mut(&pin).unwrap().default_value = Some(value);
        Ok(())
    }
    pub fn remove_component_property(
        &mut self,
        component: NodeId,
        path: &[&str],
    ) -> Result<(), String> {
        self.remove_dictionary_property(component, "properties", path)
    }
    pub fn remove_dictionary_property(
        &mut self,
        component: NodeId,
        input_key: &str,
        path: &[&str],
    ) -> Result<(), String> {
        if path.is_empty() {
            return Err("Expected a component property".into());
        }
        if self
            .dictionary_property(component, input_key, path)?
            .is_none()
        {
            return Ok(());
        }
        let mut next = self.clone();
        next.remove_dictionary_property_owned(component, input_key, path)?;
        *self = next;
        Ok(())
    }
    /// Delete one authored named dictionary entry, including a nested preset.
    /// Computed producers are protected; shared parents are copied on write.
    pub fn remove_dictionary_entry(
        &mut self,
        component: NodeId,
        input_key: &str,
        path: &[&str],
    ) -> Result<(), String> {
        if path.is_empty() {
            return Err("Expected an authored dictionary entry".into());
        }
        let mut pin = input(self, component, input_key)?;
        for (index, key) in path.iter().enumerate() {
            let Some(dict) = dictionary(self, pin)? else {
                return Ok(());
            };
            let Some(value) = pair(self, dict, key)? else {
                return Ok(());
            };
            if index + 1 == path.len() {
                if literal(self, value).is_err() {
                    let _ = dictionary(self, value)?;
                }
            } else {
                pin = value;
            }
        }
        let mut next = self.clone();
        next.remove_dictionary_property_owned(component, input_key, path)?;
        *self = next;
        Ok(())
    }
    /// Explicitly remove the named entry from an authored dictionary, even
    /// when its final value is a calculated expression. Only that consumer's
    /// connection is detached; shared expression nodes remain untouched.
    /// Calculated parent dictionaries are never expanded or overwritten.
    /// This is an intentional designer command, not a scalar inspector edit.
    pub fn remove_authored_dictionary_entry(
        &mut self,
        component: NodeId,
        input_key: &str,
        path: &[&str],
    ) -> Result<(), String> {
        if path.is_empty() {
            return Err("Expected an authored dictionary entry".into());
        }
        let mut pin = input(self, component, input_key)?;
        for (index, key) in path.iter().enumerate() {
            let Some(dict) = dictionary(self, pin)? else {
                return Ok(());
            };
            let Some(value) = pair(self, dict, key)? else {
                return Ok(());
            };
            if index + 1 != path.len() {
                pin = value;
            }
        }
        let mut next = self.clone();
        next.remove_dictionary_property_owned(component, input_key, path)?;
        *self = next;
        Ok(())
    }
    fn remove_dictionary_property_owned(
        &mut self,
        component: NodeId,
        input_key: &str,
        path: &[&str],
    ) -> Result<(), String> {
        let mut pin = input(self, component, input_key)?;
        for (index, key) in path.iter().enumerate() {
            let Some(dict) = owned_dictionary(self, pin)? else {
                return Ok(());
            };
            let Some(value) = pair(self, dict, key)? else {
                return Ok(());
            };
            if index + 1 != path.len() {
                pin = value;
                continue;
            }
            let number = self.pins[&value]
                .key
                .strip_prefix("item_")
                .unwrap()
                .parse::<usize>()
                .unwrap()
                - 1;
            let key_pin = input(self, dict, &format!("item_{number}"))?;
            self.edges.retain(|_, edge| {
                ![key_pin, value].contains(&edge.input) && ![key_pin, value].contains(&edge.output)
            });
            self.pins.remove(&key_pin);
            self.pins.remove(&value);
            self.nodes
                .get_mut(&dict)
                .unwrap()
                .pins
                .retain(|pin| ![key_pin, value].contains(pin));
            for pin in self.pins.values_mut().filter(|pin| pin.node == dict) {
                if let Some(index) = pin
                    .key
                    .strip_prefix("item_")
                    .and_then(|index| index.parse::<usize>().ok())
                    .filter(|index| *index > number)
                {
                    pin.key = format!("item_{}", index - 2);
                    pin.label = format!("[{}]", index - 2);
                }
            }
            if let Some(PropertyValue::Int(count)) = self
                .nodes
                .get_mut(&dict)
                .unwrap()
                .properties
                .get_mut("input_count")
            {
                *count -= 2;
            }
            return Ok(());
        }
        unreachable!()
    }
    /// Read a literal property (including nested event-handler references).
    /// None means absent; Err means computed and must remain connected.
    pub fn component_property(
        &self,
        component: NodeId,
        path: &[&str],
    ) -> Result<Option<PropertyValue>, String> {
        if path.is_empty()
            || !self
                .nodes
                .get(&component)
                .is_some_and(|node| node.kind == NodeKind::UiComponent)
        {
            return Err("Expected a component property".into());
        }
        self.dictionary_property(component, "properties", path)
    }
    pub fn dictionary_property(
        &self,
        component: NodeId,
        input_key: &str,
        path: &[&str],
    ) -> Result<Option<PropertyValue>, String> {
        if path.is_empty() {
            return Err("Expected a dictionary property".into());
        }
        let mut pin = input(self, component, input_key)?;
        for (index, key) in path.iter().enumerate() {
            let Some(dict) = dictionary(self, pin)? else {
                return Ok(None);
            };
            let Some(value) = pair(self, dict, key)? else {
                return Ok(None);
            };
            if index == path.len() - 1 {
                return literal(self, value);
            }
            pin = value;
        }
        unreachable!()
    }

    /// Atomic, undo-friendly edit. Existing connections to computed values
    /// are rejected; no shared value node or unrelated field is modified.
    pub fn set_component_property(
        &mut self,
        component: NodeId,
        path: &[&str],
        value: PropertyValue,
    ) -> Result<(), String> {
        let _ = self.component_property(component, path)?;
        self.set_dictionary_property(component, "properties", path, value)
    }
    pub fn set_dictionary_property(
        &mut self,
        component: NodeId,
        input_key: &str,
        path: &[&str],
        value: PropertyValue,
    ) -> Result<(), String> {
        let _ = self.dictionary_property(component, input_key, path)?;
        let mut next = self.clone();
        let mut pin = input(&next, component, input_key)?;
        for (index, key) in path.iter().enumerate() {
            let dict = if let Some(node) = owned_dictionary(&mut next, pin)? {
                node
            } else {
                let position = next.nodes[&component].position;
                let node = next
                    .add_catalog_node(
                        NodeKind::FunctionCall,
                        [
                            position[0] - 320.0 - index as f64 * 280.0,
                            position[1] + 160.0,
                        ],
                    )
                    .map_err(|error| error.to_string())?;
                next.set_property(node, "function", PropertyValue::String("dict".into()))
                    .map_err(|error| error.to_string())?;
                next.resize_value_inputs(node, 0)
                    .map_err(|error| error.to_string())?;
                let output = next.pin_by_key(node, "result").unwrap().id;
                next.connect(output, pin)
                    .map_err(|error| error.to_string())?;
                node
            };
            pin = if let Some(pin) = pair(&next, dict, key)? {
                pin
            } else {
                let count = match next.nodes[&dict].properties.get("input_count") {
                    Some(PropertyValue::Int(count)) => *count as usize,
                    _ => 0,
                };
                if count > 126 {
                    return Err("Component dictionary limit: 64 fields".into());
                }
                next.resize_value_inputs(dict, count + 2)
                    .map_err(|error| error.to_string())?;
                let key_pin = input(&next, dict, &format!("item_{count}"))?;
                next.pins.get_mut(&key_pin).unwrap().default_value =
                    Some(PropertyValue::String((*key).into()));
                let value_pin = input(&next, dict, &format!("item_{}", count + 1))?;
                // The generic function catalog supplies scalar defaults. A
                // newly authored field has no value yet, so a nested dictionary
                // may be materialized without mistaking an existing scalar.
                next.pins.get_mut(&value_pin).unwrap().default_value = None;
                value_pin
            };
            if index == path.len() - 1 {
                next.edges.retain(|_, edge| edge.input != pin);
                if matches!(&value,PropertyValue::String(text) if text.is_empty()) {
                    // An explicitly authored empty string is data (e.g. clear
                    // an attribute). Empty catalogue defaults remain missing
                    // placeholders; represent this value as a real producer.
                    let position = next.nodes[&component].position;
                    let literal = next
                        .add_catalog_node(
                            NodeKind::Literal,
                            [position[0] - 600.0, position[1] + 280.0],
                        )
                        .map_err(|error| error.to_string())?;
                    next.set_property(literal, "value", value.clone())
                        .map_err(|error| error.to_string())?;
                    next.pins.get_mut(&pin).unwrap().default_value = None;
                    next.connect(next.pin_by_key(literal, "value").unwrap().id, pin)
                        .map_err(|error| error.to_string())?;
                } else {
                    next.pins.get_mut(&pin).unwrap().default_value = Some(value.clone());
                }
            }
        }
        *self = next;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_authored_removal_can_detach_calculated_leaf_but_not_parent() {
        let mut graph=crate::import_script(&rvn_parser::parse("screen panel(style){return component(\"a\",\"button\",{\"text\":\"original\",\"style\":style},[])}").unwrap()).unwrap().into_iter().find(|graph|matches!(graph.kind,crate::GraphKind::Screen{..})).unwrap();
        let a = graph
            .nodes
            .values()
            .find(|node| node.kind == NodeKind::UiComponent)
            .unwrap()
            .id;
        let b = graph
            .add_catalog_node(NodeKind::UiComponent, [400.0, 200.0])
            .unwrap();
        let shared = graph
            .edges
            .values()
            .find(|edge| edge.input == graph.pin_by_key(a, "properties").unwrap().id)
            .unwrap()
            .output;
        graph
            .connect(shared, graph.pin_by_key(b, "properties").unwrap().id)
            .unwrap();
        let before = graph.clone();
        assert!(graph
            .remove_authored_dictionary_entry(a, "properties", &["style", "background"])
            .is_err());
        assert_eq!(graph, before);
        assert!(graph
            .remove_dictionary_entry(a, "properties", &["style"])
            .is_err());
        graph
            .remove_authored_dictionary_entry(a, "properties", &["style"])
            .unwrap();
        assert_eq!(graph.component_property(a, &["style"]).unwrap(), None);
        assert!(graph.component_property(b, &["style"]).is_err());
        assert_eq!(
            graph.component_property(a, &["text"]).unwrap(),
            Some(PropertyValue::String("original".into()))
        );
        // Every prior producer survives, so changing one consumer cannot
        // delete a reusable style function or its source identity.
        for (id, node) in &before.nodes {
            assert_eq!(&graph.nodes[id], node);
        }
        let stable = graph.clone();
        graph
            .remove_authored_dictionary_entry(a, "properties", &["missing"])
            .unwrap();
        assert_eq!(graph, stable);
    }
    #[test]
    fn explicit_empty_dictionary_strings_are_values_not_catalog_placeholders() {
        let mut graph=crate::import_script(&rvn_parser::parse("function portrait(){return layered_image([600,1000],{},[image_layer(\"body\",\"body.png\",{})])}").unwrap()).unwrap().into_iter().find(|graph|matches!(graph.kind,crate::GraphKind::Function{..})).unwrap();
        let node = graph
            .nodes
            .values()
            .find(|node| node.kind == NodeKind::LayeredImage)
            .unwrap()
            .id;
        graph
            .set_dictionary_property(
                node,
                "options",
                &["variants", "clear", "outfit"],
                PropertyValue::String(String::new()),
            )
            .unwrap();
        assert_eq!(
            graph
                .dictionary_property(node, "options", &["variants", "clear", "outfit"])
                .unwrap(),
            Some(PropertyValue::String(String::new()))
        );
        assert!(crate::transpile_value_output(&graph, node, "value")
            .unwrap()
            .contains("\"outfit\", \"\""));
    }
    #[test]
    fn numeric_dictionary_lists_copy_shared_parents_and_preserve_siblings() {
        let mut graph=crate::import_script(&rvn_parser::parse("function clip(){return video_clip(\"clip.webm\",{\"rect\":[10,20,640,360],\"volume\":0.5})}").unwrap()).unwrap().into_iter().find(|graph|matches!(graph.kind,crate::GraphKind::Function{..})).unwrap();
        let a = graph
            .nodes
            .values()
            .find(|node| node.kind == NodeKind::VideoClip)
            .unwrap()
            .id;
        let b = graph
            .add_catalog_node(NodeKind::VideoClip, [400.0, 200.0])
            .unwrap();
        let shared = graph
            .edges
            .values()
            .find(|edge| edge.input == graph.pin_by_key(a, "properties").unwrap().id)
            .unwrap()
            .output;
        graph
            .connect(shared, graph.pin_by_key(b, "properties").unwrap().id)
            .unwrap();
        graph
            .set_numeric_dictionary_list_property(
                a,
                "properties",
                "rect",
                2,
                800.0,
                &[0.0, 0.0, 1280.0, 720.0],
            )
            .unwrap();
        assert_eq!(
            graph
                .numeric_dictionary_list_property(a, "properties", "rect")
                .unwrap(),
            Some(vec![10.0, 20.0, 800.0, 360.0])
        );
        assert_eq!(
            graph
                .numeric_dictionary_list_property(b, "properties", "rect")
                .unwrap(),
            Some(vec![10.0, 20.0, 640.0, 360.0])
        );
        assert_eq!(
            graph
                .dictionary_property(a, "properties", &["volume"])
                .unwrap(),
            Some(PropertyValue::Float(0.5))
        );
        let before = graph.clone();
        assert!(graph
            .set_numeric_dictionary_list_property(
                a,
                "properties",
                "rect",
                4,
                80.0,
                &[0.0, 0.0, 1280.0, 720.0]
            )
            .is_err());
        assert_eq!(before, graph);
        graph
            .set_numeric_dictionary_list_property(
                b,
                "properties",
                "new_rect",
                0,
                32.0,
                &[0.0, 0.0, 1280.0, 720.0],
            )
            .unwrap();
        assert_eq!(
            graph
                .numeric_dictionary_list_property(b, "properties", "new_rect")
                .unwrap(),
            Some(vec![32.0, 0.0, 1280.0, 720.0])
        );
    }
    #[test]
    fn computed_numeric_dictionary_lists_cannot_be_overwritten() {
        let mut graph = crate::import_script(
            &rvn_parser::parse(
                "function clip(rect){return video_clip(\"clip.webm\",{\"rect\":rect})}",
            )
            .unwrap(),
        )
        .unwrap()
        .into_iter()
        .find(|graph| matches!(graph.kind, crate::GraphKind::Function { .. }))
        .unwrap();
        let node = graph
            .nodes
            .values()
            .find(|node| node.kind == NodeKind::VideoClip)
            .unwrap()
            .id;
        let before = graph.clone();
        assert!(graph
            .set_numeric_dictionary_list_property(
                node,
                "properties",
                "rect",
                0,
                32.0,
                &[0.0, 0.0, 1280.0, 720.0]
            )
            .is_err());
        assert_eq!(before, graph);
    }
    #[test]
    fn shared_dictionaries_are_copy_on_write_for_nested_changes_and_removal() {
        let mut graph=crate::import_script(&rvn_parser::parse("screen panel(){return component(\"a\",\"button\",{\"text\":\"original\",\"events\":{\"click\":\"accept\"}},[])}").unwrap()).unwrap().into_iter().find(|graph|matches!(graph.kind,crate::GraphKind::Screen{..})).unwrap();
        let a = graph
            .nodes
            .values()
            .find(|node| node.kind == NodeKind::UiComponent)
            .unwrap()
            .id;
        let b = graph
            .add_catalog_node(NodeKind::UiComponent, [400.0, 200.0])
            .unwrap();
        let shared = graph
            .edges
            .values()
            .find(|edge| edge.input == graph.pin_by_key(a, "properties").unwrap().id)
            .unwrap()
            .output;
        graph
            .connect(shared, graph.pin_by_key(b, "properties").unwrap().id)
            .unwrap();
        let before = graph.clone();
        graph
            .set_component_property(
                a,
                &["events", "click"],
                PropertyValue::String("confirm".into()),
            )
            .unwrap();
        assert_eq!(
            graph.component_property(b, &["events", "click"]).unwrap(),
            Some(PropertyValue::String("accept".into()))
        );
        assert_eq!(
            graph.component_property(a, &["events", "click"]).unwrap(),
            Some(PropertyValue::String("confirm".into()))
        );
        for (id, node) in &before.nodes {
            assert_eq!(graph.nodes[id].position, node.position);
        }
        graph.remove_component_property(b, &["text"]).unwrap();
        assert_eq!(graph.component_property(b, &["text"]).unwrap(), None);
        assert_eq!(
            graph.component_property(a, &["text"]).unwrap(),
            Some(PropertyValue::String("original".into()))
        );
        graph
            .set_dictionary_property(a, "properties", &["new", "nested"], PropertyValue::Int(7))
            .unwrap();
        assert_eq!(
            graph.component_property(a, &["new", "nested"]).unwrap(),
            Some(PropertyValue::Int(7))
        );
        assert_eq!(
            graph.component_property(b, &["new", "nested"]).unwrap(),
            None
        );
    }
    #[test]
    fn empty_motion_channels_can_be_added_without_panics_and_invalid_edits_are_atomic() {
        let mut graph = GraphDocument::new(
            crate::GraphId::new(1),
            crate::GraphKind::Label {
                name: "start".into(),
            },
        );
        let tween = graph
            .add_catalog_node(NodeKind::MotionTween, [0.0, 0.0])
            .unwrap();
        graph
            .set_dictionary_property(tween, "to", &["x"], PropertyValue::Int(100))
            .unwrap();
        assert_eq!(
            graph.dictionary_property(tween, "to", &["x"]).unwrap(),
            Some(PropertyValue::Int(100))
        );
        let before = graph.clone();
        assert!(graph
            .set_dictionary_property(tween, "seconds", &["x"], PropertyValue::Int(2))
            .is_err());
        assert_eq!(graph, before);
        graph
            .set_literal_input(tween, "from", PropertyValue::Int(7))
            .unwrap();
        let before = graph.clone();
        assert!(graph
            .set_dictionary_property(tween, "from", &["x"], PropertyValue::Int(2))
            .is_err());
        assert_eq!(graph, before);
    }
    #[test]
    fn inspector_edits_keep_dynamic_fields_connections_and_identities() {
        let script = rvn_parser::parse("screen panel(title) { return component(\"button\", \"button\", {\"text\": title, \"visible\": true, \"events\":{\"click\":\"accept\"}}, []) }").unwrap();
        let mut graph = crate::import_script(&script)
            .unwrap()
            .into_iter()
            .find(|graph| matches!(graph.kind, crate::GraphKind::Screen { .. }))
            .unwrap();
        let component = graph
            .nodes
            .values()
            .find(|node| node.kind == NodeKind::UiComponent)
            .unwrap()
            .id;
        assert_eq!(
            graph.literal_input_value(component, "kind").unwrap(),
            Some(PropertyValue::String("button".into()))
        );
        graph
            .set_literal_input(component, "kind", PropertyValue::String("input".into()))
            .unwrap();
        assert_eq!(
            graph.literal_input_value(component, "kind").unwrap(),
            Some(PropertyValue::String("input".into()))
        );
        let before = graph.clone();
        assert!(graph
            .set_component_property(
                component,
                &["text"],
                PropertyValue::String("No overwrite".into())
            )
            .is_err());
        assert_eq!(graph, before);
        graph
            .set_component_property(component, &["visible"], PropertyValue::Bool(false))
            .unwrap();
        graph
            .set_component_property(
                component,
                &["events", "click"],
                PropertyValue::String("confirm".into()),
            )
            .unwrap();
        graph
            .set_component_property(component, &["padding"], PropertyValue::Int(16))
            .unwrap();
        assert_eq!(
            graph.component_property(component, &["padding"]).unwrap(),
            Some(PropertyValue::Int(16))
        );
        for (id, node) in &before.nodes {
            assert_eq!(graph.nodes[id].position, node.position);
        }
        graph
            .remove_component_property(component, &["visible"])
            .unwrap();
        assert_eq!(
            graph.component_property(component, &["visible"]).unwrap(),
            None
        );
        assert_eq!(
            graph.component_property(component, &["padding"]).unwrap(),
            Some(PropertyValue::Int(16))
        );
        graph
            .set_component_property(component, &["visible"], PropertyValue::Bool(false))
            .unwrap();
        let source = crate::transpile_project(&[graph]).unwrap().source;
        assert!(source.contains("title"));
        assert!(source.contains("confirm"));
        assert!(source.contains("false"));
        assert!(rvn_parser::parse(&source).is_ok());
    }
}
