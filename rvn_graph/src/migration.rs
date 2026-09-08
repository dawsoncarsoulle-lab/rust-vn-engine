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
    Ok((
        value,
        MigrationReport {
            from,
            to: version,
            steps,
        },
    ))
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
