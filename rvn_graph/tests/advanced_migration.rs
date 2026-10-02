use rvn_graph::*;

fn function_graph(source: &str) -> GraphDocument {
    import_script(&rvn_parser::parse(source).unwrap())
        .unwrap()
        .into_iter()
        .find(|graph| matches!(graph.kind, GraphKind::Function { .. }))
        .unwrap()
}

#[test]
fn legacy_composition_adds_only_options_without_moving_or_reidentifying_nodes() {
    let mut original = function_graph(
        "function portrait(){return layered_image([600,1000],{},[image_layer(\"body\",\"body.png\",{})])}",
    );
    let composition = original
        .nodes
        .values()
        .find(|node| node.kind == NodeKind::LayeredImage)
        .unwrap()
        .id;
    let options = original.pin_by_key(composition, "options").unwrap().id;
    original.pins.remove(&options);
    original
        .nodes
        .get_mut(&composition)
        .unwrap()
        .pins
        .retain(|pin| *pin != options);
    original.nodes.get_mut(&composition).unwrap().position = [723.5, -44.25];
    let source = transpile(&original).unwrap().source;
    let mut raw = serde_json::to_value(&original).unwrap();
    raw["schema_version"] = serde_json::json!(3);
    // The migration must recover a stale counter without reusing an existing ID.
    raw["next_pin_id"] = serde_json::json!(0);
    let (migrated, report) = GraphDocument::from_json_with_report(&raw.to_string()).unwrap();
    assert_eq!(report.from, 3);
    assert_eq!(report.to, GRAPH_SCHEMA_VERSION);
    assert_eq!(
        report.steps,
        [
            "v3_to_v4_advanced_authoring",
            "v4_to_v5_programmable_canvas"
        ]
    );
    assert_eq!(migrated.graph_id, original.graph_id);
    assert_eq!(migrated.kind, original.kind);
    assert_eq!(migrated.edges, original.edges);
    assert_eq!(migrated.variables, original.variables);
    assert_eq!(migrated.nodes.len(), original.nodes.len());
    for (id, node) in &original.nodes {
        let mut expected = node.clone();
        if *id == composition {
            expected
                .pins
                .push(migrated.pin_by_key(composition, "options").unwrap().id);
        }
        assert_eq!(migrated.nodes[id], expected);
    }
    for (id, pin) in &original.pins {
        assert_eq!(migrated.pins[id], *pin);
    }
    let added = migrated.pin_by_key(composition, "options").unwrap();
    assert_eq!(added.value_type, ValueType::Any);
    assert_eq!(added.direction, PinDirection::Input);
    assert!(added.default_value.is_none());
    assert!(!original.pins.contains_key(&added.id));
    assert_eq!(migrated.pins.len(), original.pins.len() + 1);
    assert_eq!(transpile(&migrated).unwrap().source, source);
    assert_eq!(
        GraphDocument::from_json(&migrated.to_pretty_json().unwrap()).unwrap(),
        migrated
    );
}

#[test]
fn legacy_tween_string_curve_becomes_any_without_changing_its_connection() {
    let original = function_graph(
        "function fade_motion(){return motion_tween(1,{}, {\"opacity\":0.5},\"ease_in_out\")}",
    );
    let tween = original
        .nodes
        .values()
        .find(|node| node.kind == NodeKind::MotionTween)
        .unwrap()
        .id;
    let curve = original.pin_by_key(tween, "curve").unwrap().id;
    let mut raw = serde_json::to_value(&original).unwrap();
    raw["schema_version"] = serde_json::json!(3);
    raw["pins"][curve.get().to_string()]["value_type"] = serde_json::json!("string");
    let (migrated, report) = GraphDocument::from_json_with_report(&raw.to_string()).unwrap();
    assert_eq!(
        report.steps,
        [
            "v3_to_v4_advanced_authoring",
            "v4_to_v5_programmable_canvas"
        ]
    );
    assert_eq!(migrated.nodes, original.nodes);
    assert_eq!(migrated.edges, original.edges);
    assert_eq!(migrated.pins, original.pins);
    assert_eq!(
        migrated.pin_by_key(tween, "curve").unwrap().value_type,
        ValueType::Any
    );
    assert!(migrated.edges.values().any(|edge| edge.input == curve));
    assert!(migrated.validate().is_empty());
    assert_eq!(
        transpile(&migrated).unwrap().source,
        transpile(&original).unwrap().source
    );
}

#[test]
fn current_migration_is_idempotent_and_never_duplicates_options() {
    let graph = function_graph("function portrait(){return layered_image([600,1000],{},[])}");
    let current = serde_json::to_value(&graph).unwrap();
    let (unchanged, report) = migrate_graph_value(current.clone()).unwrap();
    assert_eq!(unchanged, current);
    assert_eq!(report.from, GRAPH_SCHEMA_VERSION);
    assert_eq!(report.to, GRAPH_SCHEMA_VERSION);
    assert!(report.steps.is_empty());
    let mut old = current.clone();
    old["schema_version"] = serde_json::json!(3);
    let (migrated, _) = migrate_graph_value(old).unwrap();
    assert_eq!(migrated, current);
}
