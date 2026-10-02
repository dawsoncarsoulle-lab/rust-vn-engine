use crate::GRAPH_SCHEMA_VERSION;
use serde_json::{Map, Value};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationReport {
    pub from: u32,
    pub to: u32,
    pub steps: Vec<&'static str>,
}

pub fn migrate_graph_value(mut value: Value) -> Result<(Value, MigrationReport), u32> {
    let from = value
        .get("schema_version")
        .and_then(Value::as_u64)
        .unwrap_or(0) as u32;
    if from > GRAPH_SCHEMA_VERSION {
        return Err(from);
    }
    let mut version = from;
    let mut steps = Vec::new();
    if version == 0 && value.is_object() {
        migrate_v0_to_v1(&mut value);
        version = 1;
        steps.push("v0_to_v1_stable_ids_and_defaults");
    }
    if version == 1 && value.is_object() {
        migrate_v1_to_v2(&mut value);
        version = 2;
        steps.push("v1_to_v2_typed_variables");
    }
    if version == 2 && value.is_object() {
        value
            .as_object_mut()
            .unwrap()
            .insert("schema_version".into(), Value::from(3));
        version = 3;
        steps.push("v2_to_v3_explicit_text_conversions");
    }
    if version == 3 && value.is_object() {
        migrate_v3_to_v4(&mut value);
        version = 4;
        steps.push("v3_to_v4_advanced_authoring");
    }
    if version == 4 && value.is_object() {
        // The new canvas nodes do not change any existing pins, identities or
        // expressions. Versioning still prevents an older editor from silently
        // opening a graph whose programmable drawing it cannot execute.
        value
            .as_object_mut()
            .unwrap()
            .insert("schema_version".into(), Value::from(5));
        version = 5;
        steps.push("v4_to_v5_programmable_canvas");
    }
    Ok((
        value,
        MigrationReport {
            from,
            to: version,
            steps,
        },
    ))
}

fn migrate_v3_to_v4(value: &mut Value) {
    let object = value.as_object_mut().unwrap();
    let kinds: std::collections::BTreeMap<_, _> = object
        .get("nodes")
        .and_then(Value::as_object)
        .into_iter()
        .flat_map(|nodes| nodes.values())
        .filter_map(|node| {
            Some((
                node.get("id")?.as_u64()?,
                node.get("kind")?.as_str()?.to_owned(),
            ))
        })
        .collect();
    let mut next = object
        .get("next_pin_id")
        .and_then(Value::as_u64)
        .unwrap_or(1)
        .max(
            object
                .get("pins")
                .and_then(Value::as_object)
                .into_iter()
                .flat_map(|pins| pins.keys())
                .filter_map(|key| key.parse::<u64>().ok())
                .max()
                .unwrap_or(0)
                .saturating_add(1),
        );
    if let Some(pins) = object.get_mut("pins").and_then(Value::as_object_mut) {
        for pin in pins.values_mut() {
            if pin
                .get("node")
                .and_then(Value::as_u64)
                .and_then(|id| kinds.get(&id))
                .is_some_and(|kind| kind == "motion_tween")
                && pin.get("key").and_then(Value::as_str) == Some("curve")
            {
                pin.as_object_mut()
                    .unwrap()
                    .insert("value_type".into(), Value::String("any".into()));
            }
        }
        for (id, kind) in &kinds {
            if kind == "layered_image"
                && !pins.values().any(|pin| {
                    pin.get("node").and_then(Value::as_u64) == Some(*id)
                        && pin.get("key").and_then(Value::as_str) == Some("options")
                })
            {
                let pin = next;
                next = next.saturating_add(1);
                pins.insert(pin.to_string(),serde_json::json!({"id":pin,"node":id,"key":"options","label":"Variantes et règles","direction":"input","value_type":"any","cardinality":"one","default_value":null}));
            }
        }
    }
    let added: Vec<_> = object
        .get("pins")
        .and_then(Value::as_object)
        .into_iter()
        .flat_map(|pins| pins.values())
        .filter(|pin| pin.get("key").and_then(Value::as_str) == Some("options"))
        .filter_map(|pin| Some((pin.get("node")?.as_u64()?, pin.get("id")?.as_u64()?)))
        .collect();
    if let Some(nodes) = object.get_mut("nodes").and_then(Value::as_object_mut) {
        for (node, pin) in added {
            if let Some(pins) = nodes
                .get_mut(&node.to_string())
                .and_then(|node| node.get_mut("pins"))
                .and_then(Value::as_array_mut)
            {
                if !pins.iter().any(|id| id.as_u64() == Some(pin)) {
                    pins.push(Value::from(pin));
                }
            }
        }
    }
    object.insert("next_pin_id".into(), Value::from(next));
    object.insert("schema_version".into(), Value::from(4));
}

fn migrate_v1_to_v2(value: &mut Value) {
    let object = value.as_object_mut().unwrap();
    object.insert("schema_version".into(), Value::from(2));
    object
        .entry("variables")
        .or_insert_with(|| Value::Object(Map::new()));
}

fn migrate_v0_to_v1(value: &mut Value) {
    let object = value.as_object_mut().unwrap();
    object.insert("schema_version".into(), Value::from(1));
    add_collection_defaults(object, "nodes", &["title_override", "properties"]);
    add_collection_defaults(object, "pins", &["default_value"]);
    object
        .entry("edges")
        .or_insert_with(|| Value::Object(Map::new()));
    for (collection, counter) in [
        ("nodes", "next_node_id"),
        ("pins", "next_pin_id"),
        ("edges", "next_edge_id"),
    ] {
        let next = object
            .get(collection)
            .and_then(Value::as_object)
            .into_iter()
            .flat_map(|items| items.keys())
            .filter_map(|key| key.parse::<u64>().ok())
            .max()
            .unwrap_or(0)
            + 1;
        object.entry(counter).or_insert_with(|| Value::from(next));
    }
}

fn add_collection_defaults(root: &mut Map<String, Value>, key: &str, defaults: &[&str]) {
    let Some(items) = root.get_mut(key).and_then(Value::as_object_mut) else {
        return;
    };
    for item in items.values_mut().filter_map(Value::as_object_mut) {
        for field in defaults {
            item.entry(*field).or_insert_with(|| match *field {
                "properties" => Value::Object(Map::new()),
                _ => Value::Null,
            });
        }
    }
}
