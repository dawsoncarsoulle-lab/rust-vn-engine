//! Ordered, source-friendly collection edits for composition/UI designers.
//! Computed collections remain expressions; shared producers are copied only
//! on the edited connection, never overwritten for their other consumers.
use crate::*;

#[derive(Clone)]
enum Binding {
    Connected(PinId),
    Literal(Option<PropertyValue>),
}
fn input(graph: &GraphDocument, node: NodeId, key: &str) -> Result<PinId, String> {
    graph
        .pin_by_key(node, key)
        .filter(|pin| pin.direction == PinDirection::Input)
        .map(|pin| pin.id)
        .ok_or_else(|| format!("Missing collection input {node}.{key}"))
}
fn producer(graph: &GraphDocument, pin: PinId) -> Result<Option<PinId>, String> {
    let mut edges = graph.edges.values().filter(|edge| edge.input == pin);
    let result = edges.next().map(|edge| edge.output);
    if edges.next().is_some() {
        return Err("A collection input needs exactly one producer".into());
    }
    Ok(result)
}
fn list(graph: &GraphDocument, pin: PinId) -> Result<Option<NodeId>, String> {
    let Some(output) = producer(graph, pin)? else {
        if graph.pins[&pin].default_value.as_ref().is_none_or(
            |value| matches!(value,PropertyValue::StringList(items) if items.is_empty()),
        ) {
            return Ok(None);
        }
        return Err("This collection is a literal value: edit its Blueprint expression".into());
    };
    let node = graph.pins[&output].node;
    if graph.nodes[&node].kind == NodeKind::ListLiteral
        && matches!(
            graph.nodes[&node].properties.get("input_count"),
            Some(PropertyValue::Int(_))
        )
    {
        Ok(Some(node))
    } else {
        Err("This collection is computed: edit its connected Blueprint expression".into())
    }
}
fn bindings(graph: &GraphDocument, node: NodeId) -> Result<Vec<Binding>, String> {
    let count = match graph.nodes[&node].properties.get("input_count") {
        Some(PropertyValue::Int(count)) if (0..=128).contains(count) => *count as usize,
        _ => return Err("Invalid collection input count".into()),
    };
    (0..count)
        .map(|index| {
            let pin = input(graph, node, &format!("item_{index}"))?;
            Ok(match producer(graph, pin)? {
                Some(output) => Binding::Connected(output),
                None => Binding::Literal(graph.pins[&pin].default_value.clone()),
            })
        })
        .collect()
}
fn copy_producer(graph: &mut GraphDocument, consumer: PinId) -> Result<Option<NodeId>, String> {
    let Some(output) = producer(graph, consumer)? else {
        return Ok(None);
    };
    let old_node = graph.pins[&output].node;
    if graph
        .edges
        .values()
        .filter(|edge| edge.output == output)
        .count()
        <= 1
    {
        return Ok(Some(old_node));
    }
    let old = graph.nodes[&old_node].clone();
    let new = graph.add_node(old.kind, [old.position[0], old.position[1] + 120.0]);
    graph.nodes.get_mut(&new).unwrap().properties = old.properties;
    graph.nodes.get_mut(&new).unwrap().title_override = old.title_override;
    let mut replacement = None;
    for id in old.pins {
        let old = graph.pins[&id].clone();
        let new_pin = graph
            .add_pin(
                new,
                old.key,
                old.label,
                old.direction,
                old.value_type,
                old.cardinality,
            )
            .map_err(|error| error.to_string())?;
        graph.pins.get_mut(&new_pin).unwrap().default_value = old.default_value;
        if id == output {
            replacement = Some(new_pin);
        }
        if old.direction == PinDirection::Input {
            if let Some(producer) = producer(graph, id)? {
                graph
                    .connect(producer, new_pin)
                    .map_err(|error| error.to_string())?;
            }
        }
    }
    graph.edges.retain(|_, edge| edge.input != consumer);
    graph
        .connect(replacement.ok_or("Missing copied output")?, consumer)
        .map_err(|error| error.to_string())?;
    Ok(Some(new))
}
fn owned_list(graph: &mut GraphDocument, node: NodeId, key: &str) -> Result<NodeId, String> {
    let pin = input(graph, node, key)?;
    let _ = list(graph, pin)?;
    if let Some(list) = copy_producer(graph, pin)? {
        return Ok(list);
    }
    let position = graph.nodes[&node].position;
    let list = graph
        .add_catalog_node(
            NodeKind::ListLiteral,
            [position[0] - 300.0, position[1] + 180.0],
        )
        .map_err(|error| error.to_string())?;
    graph
        .resize_value_inputs(list, 0)
        .map_err(|error| error.to_string())?;
    graph
        .connect(graph.pin_by_key(list, "list").unwrap().id, pin)
        .map_err(|error| error.to_string())?;
    Ok(list)
}
fn assign(graph: &mut GraphDocument, list: NodeId, values: Vec<Binding>) -> Result<(), String> {
    if values.len() > 128 {
        return Err("Collection limit: 128 inputs".into());
    }
    graph
        .resize_value_inputs(list, values.len())
        .map_err(|error| error.to_string())?;
    for (index, value) in values.into_iter().enumerate() {
        let pin = input(graph, list, &format!("item_{index}"))?;
        graph.edges.retain(|_, edge| edge.input != pin);
        match value {
            Binding::Connected(output) => {
                graph.pins.get_mut(&pin).unwrap().default_value = None;
                graph
                    .connect(output, pin)
                    .map_err(|error| error.to_string())?;
            }
            Binding::Literal(value) => graph.pins.get_mut(&pin).unwrap().default_value = value,
        }
    }
    Ok(())
}
impl GraphDocument {
    /// Read a literal numeric list; computed elements remain graph expressions.
    pub fn numeric_list_input_values(
        &self,
        node: NodeId,
        key: &str,
    ) -> Result<Option<Vec<f64>>, String> {
        let Some(list) = list(self, input(self, node, key)?)? else {
            return Ok(None);
        };
        bindings(self, list)?
            .into_iter()
            .map(|binding| {
                let value = match binding {
                    Binding::Literal(value) => value,
                    Binding::Connected(output) => {
                        let model = &self.nodes[&self.pins[&output].node];
                        if model.kind != NodeKind::Literal {
                            return Err("Numeric collection contains a computed expression".into());
                        }
                        model.properties.get("value").cloned()
                    }
                };
                match value {
                    Some(PropertyValue::Int(value)) => Ok(value as f64),
                    Some(PropertyValue::Float(value)) if value.is_finite() => Ok(value),
                    _ => Err("Expected a finite numeric collection value".into()),
                }
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Some)
    }
    /// Ordered node identities behind an authored list, not a computed result.
    pub fn list_input_sources(
        &self,
        node: NodeId,
        key: &str,
    ) -> Result<Vec<Option<NodeId>>, String> {
        let Some(list) = list(self, input(self, node, key)?)? else {
            return Ok(Vec::new());
        };
        Ok(bindings(self, list)?
            .into_iter()
            .map(|value| match value {
                Binding::Connected(output) => Some(self.pins[&output].node),
                Binding::Literal(_) => None,
            })
            .collect())
    }
    pub fn move_list_input(
        &mut self,
        node: NodeId,
        key: &str,
        from: usize,
        to: usize,
    ) -> Result<(), String> {
        let mut next = self.clone();
        let list = owned_list(&mut next, node, key)?;
        let mut values = bindings(&next, list)?;
        if from >= values.len() || to >= values.len() {
            return Err("Collection index outside its ordered inputs".into());
        }
        if from == to {
            return Ok(());
        }
        let item = values.remove(from);
        values.insert(to, item);
        assign(&mut next, list, values)?;
        *self = next;
        Ok(())
    }
    /// Removes the connection, not the producing node (which may be shared).
    pub fn remove_list_input(
        &mut self,
        node: NodeId,
        key: &str,
        index: usize,
    ) -> Result<(), String> {
        let mut next = self.clone();
        let list = owned_list(&mut next, node, key)?;
        let mut values = bindings(&next, list)?;
        if index >= values.len() {
            return Err("Collection index outside its ordered inputs".into());
        }
        values.remove(index);
        assign(&mut next, list, values)?;
        *self = next;
        Ok(())
    }
    pub fn append_list_input(
        &mut self,
        node: NodeId,
        key: &str,
        producer_node: NodeId,
        output_key: &str,
    ) -> Result<(), String> {
        let mut next = self.clone();
        let output = next
            .pin_by_key(producer_node, output_key)
            .filter(|pin| pin.direction == PinDirection::Output && !pin.value_type.is_execution())
            .ok_or("Expected a data output")?
            .id;
        let list = owned_list(&mut next, node, key)?;
        let mut values = bindings(&next, list)?;
        values.push(Binding::Connected(output));
        assign(&mut next, list, values)?;
        *self = next;
        Ok(())
    }
    /// A designer editing a shared layer gets an owned node for that one row.
    pub fn own_list_input_node(
        &mut self,
        node: NodeId,
        key: &str,
        index: usize,
    ) -> Result<NodeId, String> {
        let mut next = self.clone();
        let list = owned_list(&mut next, node, key)?;
        let pin = input(&next, list, &format!("item_{index}"))?;
        let child = copy_producer(&mut next, pin)?.ok_or("Expected a connected item node")?;
        *self = next;
        Ok(child)
    }
    pub fn set_numeric_list_input(
        &mut self,
        node: NodeId,
        key: &str,
        index: usize,
        value: f64,
        defaults: &[f64],
    ) -> Result<(), String> {
        if !value.is_finite()
            || index >= defaults.len()
            || defaults.len() > 128
            || defaults.iter().any(|value| !value.is_finite())
        {
            return Err("Expected a finite numeric collection value".into());
        }
        let mut next = self.clone();
        let list = owned_list(&mut next, node, key)?;
        let values = bindings(&next, list)?;
        if !values.is_empty() && values.len() != defaults.len() {
            return Err("Authored numeric collection has a different size".into());
        }
        if values.is_empty() {
            assign(
                &mut next,
                list,
                defaults
                    .iter()
                    .map(|value| Binding::Literal(Some(PropertyValue::Float(*value))))
                    .collect(),
            )?;
        }
        next.set_literal_input(list, &format!("item_{index}"), PropertyValue::Float(value))?;
        *self = next;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn composition() -> (GraphDocument, NodeId, NodeId) {
        let mut graph=import_script(&rvn_parser::parse("function portrait(){return layered_image([600,1000],{},[image_layer(\"body\",\"body.png\",{}),image_layer(\"coat\",\"coat.png\",{})])}").unwrap()).unwrap().into_iter().find(|graph|matches!(graph.kind,GraphKind::Function{..})).unwrap();
        let a = graph
            .nodes
            .values()
            .find(|node| node.kind == NodeKind::LayeredImage)
            .unwrap()
            .id;
        let b = graph
            .add_catalog_node(NodeKind::LayeredImage, [1000.0, 600.0])
            .unwrap();
        let output = producer(&graph, input(&graph, a, "layers").unwrap())
            .unwrap()
            .unwrap();
        graph
            .connect(output, input(&graph, b, "layers").unwrap())
            .unwrap();
        (graph, a, b)
    }
    #[test]
    fn reorder_remove_and_layer_edits_do_not_change_shared_consumers_or_placement() {
        let (mut graph, a, b) = composition();
        let before = graph.clone();
        let original = graph.list_input_sources(b, "layers").unwrap();
        graph.move_list_input(a, "layers", 0, 1).unwrap();
        assert_eq!(graph.list_input_sources(b, "layers").unwrap(), original);
        assert_eq!(
            graph.list_input_sources(a, "layers").unwrap(),
            original.iter().rev().copied().collect::<Vec<_>>()
        );
        let owned = graph.own_list_input_node(a, "layers", 0).unwrap();
        assert_ne!(Some(owned), original[1]);
        graph
            .set_literal_input(owned, "image", PropertyValue::String("other.png".into()))
            .unwrap();
        assert_eq!(
            graph
                .literal_input_value(original[1].unwrap(), "image")
                .unwrap(),
            Some(PropertyValue::String("coat.png".into()))
        );
        graph.remove_list_input(a, "layers", 0).unwrap();
        assert_eq!(graph.list_input_sources(a, "layers").unwrap().len(), 1);
        assert_eq!(graph.list_input_sources(b, "layers").unwrap(), original);
        for (id, node) in &before.nodes {
            assert_eq!(graph.nodes[id].position, node.position);
        }
        let before = graph.clone();
        assert!(graph.move_list_input(a, "layers", 4, 0).is_err());
        assert_eq!(graph, before);
    }
    #[test]
    fn numeric_collections_preserve_unchanged_values_and_reject_computed_edits() {
        let (mut graph, a, _) = composition();
        graph
            .set_numeric_list_input(a, "size", 1, 900.0, &[600.0, 1000.0])
            .unwrap();
        assert_eq!(
            transpile_value_input(&graph, a, "size").unwrap(),
            "[600, 900.0]"
        );
        let before = graph.clone();
        assert!(graph
            .set_numeric_list_input(a, "size", 2, 20.0, &[600.0, 1000.0])
            .is_err());
        assert_eq!(graph, before);
    }
}
