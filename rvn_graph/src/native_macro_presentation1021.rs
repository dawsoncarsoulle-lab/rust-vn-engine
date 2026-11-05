//! Non-executable editing objects, retained only by the existing authoring
//! sidecar. Plain RVN and strict Native preparation never claim these objects.
use super::*;

// This finite set is the original CLASSIC_EFFECT_TRANSACTION14 family.
// It grants authoring persistence only, never a consumer/VM/Exec capability.
fn classic_editing_kind(kind:NodeKind)->bool{
    matches!(kind,NodeKind::Config|NodeKind::CharacterCreate|NodeKind::Scene
        |NodeKind::CinematicShow|NodeKind::CinematicHide|NodeKind::UnlockEnding
        |NodeKind::SpriteShow|NodeKind::SpriteHide|NodeKind::SpriteMove
        |NodeKind::SpriteAnimate|NodeKind::SpriteStopAnimation|NodeKind::SpriteEffect
        |NodeKind::Timer|NodeKind::TimerCancel)
}
fn finite_property(value:&PropertyValue)->bool{!matches!(value,PropertyValue::Float(number)if !number.is_finite())}
fn isolated_macros(graph: &GraphDocument) -> Result<Vec<NodeId>, String> {
    let candidates = graph.nodes.values().filter(|node|
        (crate::standard_macros_1020::kind(node.kind).is_some()||classic_editing_kind(node.kind))
        && !graph.edges.values().any(|edge|
            graph.pins.get(&edge.input).is_some_and(|pin| pin.node == node.id)
            || graph.pins.get(&edge.output).is_some_and(|pin| pin.node == node.id)))
        .collect::<Vec<_>>();
    if candidates.is_empty() { return Ok(Vec::new()); }
    graph.validate_standard_macro_contracts()?;
    graph.validate_inspector_evaluation_contract()?;
    crate::native_frame_preparation::native_scope(graph).map_err(|error| error.to_string())?;
    if graph.validate().iter().any(|diagnostic| diagnostic.severity == crate::DiagnosticSeverity::Error) {
        return Err("Invalid authoring graph cannot supply isolated macro presentation".into());
    }
    let mut result = Vec::new();
    for node in candidates {
        let definition = node_definition(node.kind);
        let classic=classic_editing_kind(node.kind);
        if node.id.get() == 0 || graph.nodes.get(&node.id) != Some(node)
            || !node.position.iter().all(|coordinate| coordinate.is_finite())
            || (!classic&&!node.properties.is_empty())
            || (classic&&(node.standard_macro_contract.is_some()||node.properties.values().any(|value|!finite_property(value))))
            || node.inspector_contract.is_some()
            || node.string_table_contract.is_some() || node.native_loop_contract.is_some()
            || node.pins.len() != definition.pins.len()
            || graph.pins.values().filter(|pin| pin.node == node.id).count() != node.pins.len() {
            return Err("Isolated macro must retain its genuine catalogue shape".into());
        }
        for (id, expected) in node.pins.iter().zip(definition.pins) {
            let pin = graph.pins.get(id).ok_or("Isolated macro pin missing")?;
            if id.get() == 0 || pin.id != *id || pin.node != node.id
                || pin.key != expected.key || pin.direction != expected.direction
                || pin.value_type != expected.value_type || pin.cardinality != expected.cardinality
                || pin.parameter_id.is_some() || pin.text_localization.is_some()
                || pin.text_table_reference.is_some() {
                return Err("Isolated macro port is not its genuine catalogue port".into());
            }
            // An incomplete editor operand is retained as authored data. It
            // cannot be executed from this sidecar: reconnecting still must
            // pass the unchanged strict source/operand codec. No runtime value
            // or missing Character/Asset producer is synthesized here.
            let valid_default = if classic {
                match &pin.default_value {
                    None=>true,
                    Some(value)=>pin.direction==PinDirection::Input
                        &&pin.value_type!=ValueType::Execution&&finite_property(value),
                }
            } else { match (pin.key.as_str(), &pin.default_value) {
                ("start_closed", Some(PropertyValue::Bool(_))) => true,
                ("n", Some(PropertyValue::Int(value))) => i32::try_from(*value).is_ok(),
                (_, None) => pin.value_type == ValueType::Execution || pin.direction == PinDirection::Output,
                _ => false,
            }};
            if !valid_default { return Err("Invalid isolated macro default".into()); }
        }
        result.push(node.id);
    }
    // The sidecar owns its ordinary allocator reservations too. Never restore
    // objects from a cache whose next allocation would alias an existing port.
    if graph.next_node_id == 0 || graph.next_pin_id == 0 || graph.next_edge_id == 0
        || graph.nodes.keys().any(|id| id.get() >= graph.next_node_id)
        || graph.pins.keys().any(|id| id.get() >= graph.next_pin_id)
        || graph.edges.keys().any(|id| id.get() >= graph.next_edge_id) {
        return Err("Invalid isolated macro identity reservations".into());
    }
    Ok(result)
}

pub(crate) fn native_authoring_execution_projection(graph: &GraphDocument) -> Result<GraphDocument, String> {
    let mut execution = graph.clone();
    for id in isolated_macros(graph)? {
        let node = execution.nodes.remove(&id).unwrap();
        for pin in node.pins { execution.pins.remove(&pin); }
    }
    // No edge is removed, rewritten or synthesized. A disconnected component
    // containing any edge is deliberately outside this presentation contract.
    Ok(execution)
}

pub(crate) fn restore_isolated_macro_presentation(graph: &mut GraphDocument, old: &GraphDocument) -> Result<(), String> {
    let isolated = isolated_macros(old)?;
    if isolated.is_empty() { return Ok(()); }
    if graph.graph_id != old.graph_id || graph.kind != old.kind
        || graph.inspector_evaluation != old.inspector_evaluation
        || graph.schema_version != old.schema_version
        || crate::native_frame_preparation::native_scope(graph).map_err(|error| error.to_string())?
            != crate::native_frame_preparation::native_scope(old).map_err(|error| error.to_string())? {
        return Err("Isolated macro presentation requires the exact source-owned Native scope".into());
    }
    let mut candidate = graph.clone();
    for id in isolated {
        let node = &old.nodes[&id];
        if let Some(actual) = candidate.nodes.get(&id) {
            // A newly connected former orphan is now source-owned. Only the
            // same catalogue object/ports can make this transition; source owns
            // all current defaults and wires, never the old sidecar payload.
            if actual.kind != node.kind || actual.pins != node.pins
                || actual.standard_macro_contract != node.standard_macro_contract {
                return Err("Source node collides with isolated macro identity".into());
            }
            continue;
        }
        if node.pins.iter().any(|id| candidate.pins.contains_key(id)
            || candidate.edges.values().any(|edge| edge.input == *id || edge.output == *id)) {
            return Err("Source pin/edge collides with isolated macro identity".into());
        }
        candidate.nodes.insert(id, node.clone());
        for id in &node.pins { candidate.pins.insert(*id, old.pins[id].clone()); }
    }
    candidate.next_node_id = candidate.next_node_id.max(old.next_node_id);
    candidate.next_pin_id = candidate.next_pin_id.max(old.next_pin_id);
    candidate.next_edge_id = candidate.next_edge_id.max(old.next_edge_id);
    if candidate.validate().iter().any(|diagnostic| diagnostic.severity == crate::DiagnosticSeverity::Error) {
        return Err("Restored isolated macro presentation is invalid".into());
    }
    *graph = candidate;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;

    fn handler() -> GraphDocument {
        let mut graph = import_script(&rvn_parser::parse("handler macro_probe(event) { local marker = 1 }\n").unwrap())
            .unwrap().into_iter().find(|graph| matches!(graph.kind, GraphKind::Handler { .. })).unwrap();
        graph.enable_native_frame_preparation().unwrap();
        graph
    }

    #[test]
    fn isolated_macros1021_source_sidecar_cold_and_later_connection_keep_real_ids() {
        let root = std::env::temp_dir().join(format!("rvn-isolated-macros-{}-{}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("main.rvn");
        std::fs::write(&path, "handler macro_probe(event) { local marker = 1 }\nscene label start\nreturn\n").unwrap();
        let mut workspace = SourceWorkspace::link(&root, &path, &[]).unwrap();
        let mut edited = workspace.project().graphs().to_vec();
        let graph = edited.iter_mut().find(|graph| matches!(graph.kind, GraphKind::Handler { .. })).unwrap();
        graph.enable_native_frame_preparation().unwrap();
        let mut macros = Vec::new();
        for kind in [NodeKind::DoOnce, NodeKind::DoN, NodeKind::FlipFlop, NodeKind::Gate] {
            let id = graph.add_catalog_node(kind, [macros.len() as f64 * 200.0, 90.0]).unwrap();
            macros.push(id);
        }
        graph.set_pin_default(macros[1], "n", PropertyValue::Int(2)).unwrap();
        let authored = graph.clone();
        assert!(prepare_native_frame_graph(&authored).is_err());
        let compiled = prepare_native_authoring_graph(&authored).unwrap();
        assert!(!compiled.source.contains("flow.macro_dispatch_v1("));
        let plain = import_native_frame_script(&compiled.ast).unwrap();
        let plain = plain.iter().find(|graph| graph.graph_id == authored.graph_id).unwrap();
        for id in &macros { assert!(!plain.nodes.contains_key(id)); }
        workspace.save(&edited).unwrap();
        let mut cold = SourceWorkspace::open(&root).unwrap().unwrap();
        let restored = cold.project().graphs().iter().find(|graph| graph.graph_id == authored.graph_id).unwrap();
        for id in &macros {
            assert_eq!(restored.nodes[id], authored.nodes[id]);
            for pin in &authored.nodes[id].pins { assert_eq!(restored.pins[pin], authored.pins[pin]); }
        }
        let mut connected = cold.project().graphs().to_vec();
        let graph = connected.iter_mut().find(|graph| graph.graph_id == authored.graph_id).unwrap();
        let entry = graph.nodes.values().find(|node| node.kind == NodeKind::HandlerEntry).unwrap().id;
        let output = graph.pin_by_key(entry, "exec_out").unwrap().id;
        let input = graph.pin_by_key(macros[0], "exec_in").unwrap().id;
        let old_edge = graph.edges.values().find(|edge| edge.output == output).unwrap().clone();
        graph.remove_edge(old_edge.id).unwrap();
        graph.connect(output, input).unwrap();
        let completed = graph.pin_by_key(macros[0], "completed").unwrap().id;
        graph.connect(completed, old_edge.input).unwrap();
        graph.set_pin_default(macros[0], "start_closed", PropertyValue::Bool(true)).unwrap();
        let compiled = prepare_native_authoring_graph(graph).unwrap();
        assert!(compiled.source.contains("flow.macro_dispatch_v1("));
        let source_only = import_native_frame_script(&compiled.ast).unwrap();
        let source_only = source_only.iter().find(|graph| graph.graph_id == authored.graph_id).unwrap();
        assert_eq!(source_only.nodes[&macros[0]].pins, authored.nodes[&macros[0]].pins);
        assert_eq!(source_only.edges, graph.edges);
        let mut with_stale_metadata = source_only.clone();
        restore_isolated_macro_presentation(&mut with_stale_metadata, &authored).unwrap();
        assert_eq!(with_stale_metadata.pin_by_key(macros[0], "start_closed").unwrap().default_value,
            Some(PropertyValue::Bool(true)), "the connected source wins over the previous orphan default");
        assert_eq!(with_stale_metadata.edges, graph.edges);
        let connected_edges = graph.edges.clone();
        cold.save(&connected).unwrap();
        let mut final_cold = SourceWorkspace::open(&root).unwrap().unwrap();
        let final_graph = final_cold.project().graphs().iter().find(|graph| graph.graph_id == authored.graph_id).unwrap();
        assert_eq!(final_graph.edges, connected_edges);
        for id in &macros { assert_eq!(final_graph.nodes[id].pins, authored.nodes[id].pins); }
        let fresh = final_graph.clone().add_catalog_node(NodeKind::DoOnce, [900.0, 90.0]).unwrap();
        assert!(fresh.get() > macros.iter().map(|id| id.get()).max().unwrap());
        let mut deleted = final_cold.project().graphs().to_vec();
        deleted.iter_mut().find(|graph| graph.graph_id == authored.graph_id).unwrap()
            .remove_node(macros[3]).unwrap();
        final_cold.save(&deleted).unwrap();
        let after_delete = SourceWorkspace::open(&root).unwrap().unwrap();
        let after_delete = after_delete.project().graphs().iter().find(|graph| graph.graph_id == authored.graph_id).unwrap();
        assert!(!after_delete.nodes.contains_key(&macros[3]));
        for pin in &authored.nodes[&macros[3]].pins { assert!(!after_delete.pins.contains_key(pin)); }
        assert_eq!(after_delete.edges, connected_edges);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn isolated_macros1021_bad_scope_and_identity_collisions_refuse_atomically() {
        let mut old = handler();
        let orphan = old.add_catalog_node(NodeKind::DoOnce, [200.0, 90.0]).unwrap();
        let execution = native_authoring_execution_projection(&old).unwrap();
        let mut wrong = execution.clone();
        wrong.graph_id = GraphId::new(old.graph_id.get() + 100);
        let before = wrong.clone();
        assert!(restore_isolated_macro_presentation(&mut wrong, &old).is_err());
        assert_eq!(wrong, before);
        let mut collision = execution.clone();
        let id = collision.add_catalog_node(NodeKind::Literal, [400.0, 90.0]).unwrap();
        let mut node = collision.nodes.remove(&id).unwrap();
        node.id = orphan;
        collision.nodes.insert(orphan, node);
        let before = collision.clone();
        assert!(restore_isolated_macro_presentation(&mut collision, &old).is_err());
        assert_eq!(collision, before);
        let mut collision = execution;
        let pin = old.nodes[&orphan].pins[0];
        collision.pins.insert(pin, old.pins[&pin].clone());
        let before = collision.clone();
        assert!(restore_isolated_macro_presentation(&mut collision, &old).is_err());
        assert_eq!(collision, before);
        let mut collision = native_authoring_execution_projection(&old).unwrap();
        let edge = collision.edges.values_mut().next().unwrap();
        edge.input = pin;
        let before = collision.clone();
        assert!(restore_isolated_macro_presentation(&mut collision, &old).is_err());
        assert_eq!(collision, before);
    }

    #[test]
    fn isolated_macros1021_wired_disconnected_components_and_malformed_ports_remain_refused() {
        let mut graph = handler();
        let once = graph.add_catalog_node(NodeKind::DoOnce, [200.0, 90.0]).unwrap();
        let gate = graph.add_catalog_node(NodeKind::Gate, [400.0, 90.0]).unwrap();
        let output = graph.pin_by_key(once, "completed").unwrap().id;
        let input = graph.pin_by_key(gate, "exec_in").unwrap().id;
        graph.connect(output, input).unwrap();
        let before = graph.clone();
        assert!(prepare_native_authoring_graph(&graph).is_err());
        assert_eq!(graph, before);
        // A fresh graph makes the malformed-port refusal independent of the
        // disconnected wired component above (and of its unrelated root edge).
        let mut graph = handler();
        let once = graph.add_catalog_node(NodeKind::DoOnce, [200.0, 90.0]).unwrap();
        assert!(!graph.edges.values().any(|edge|
            graph.pins[&edge.input].node == once || graph.pins[&edge.output].node == once));
        assert!(!native_authoring_execution_projection(&graph).unwrap().nodes.contains_key(&once));
        let pin = graph.pin_by_key(once, "start_closed").unwrap().id;
        graph.pins.get_mut(&pin).unwrap().cardinality = PinCardinality::Many;
        let before = graph.clone();
        let error = prepare_native_authoring_graph(&graph).unwrap_err().to_string();
        assert!(error.contains("Isolated macro port is not its genuine catalogue port"), "{error}");
        assert_eq!(graph, before);
    }
}

#[cfg(test)]
#[path="native_classic_isolated_tests1022.rs"]
mod classic_isolated1022;
