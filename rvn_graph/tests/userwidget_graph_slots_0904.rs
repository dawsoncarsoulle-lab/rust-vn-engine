//! Source/Graph slot transactions, not UserWidget instance or native parity tests.
use rvn_graph::*;
use rvn_ui::programmable::{Alignment, BoxSlotSize, Component, ParentSlot, SlotEdges};

const SOURCE: &str = r#"// This is the authored Widget source, not a serialized preview.
screen form(title) {
 // Keep this Designer source comment across the transaction.
 return component("root","column",{"layout_contract":"widget_v1","spacing":0},[
  component("left","column",{"spacing":0,"parent_slot":{"kind":"vertical_box"}},[
   component("moving","text",{"spacing":0,"text":title,"parent_slot":{"kind":"vertical_box","padding":{"left":8,"top":8,"right":8,"bottom":8}}},[]),
   component("sibling","text",{"spacing":0,"text":"Sibling","parent_slot":{"kind":"vertical_box"}},[])
  ]),
  component("empty","button",{"spacing":0,"parent_slot":{"kind":"vertical_box"}},[]),
  component("occupied","button",{"spacing":0,"parent_slot":{"kind":"vertical_box"}},[
   component("content","text",{"spacing":0,"text":"Occupied","parent_slot":{"kind":"button","padding":{"left":6,"top":6,"right":6,"bottom":6},"horizontal":"center","vertical":"center"}},[])
  ]),
  component("other","column",{"spacing":0,"parent_slot":{"kind":"vertical_box"}},[])
 ])
}
"#;
fn graph(source: &str) -> GraphDocument {
    import_script(&rvn_parser::parse(source).unwrap()).unwrap().into_iter()
        .find(|graph| matches!(graph.kind, GraphKind::Screen { .. })).unwrap()
}
fn returned(graph: &GraphDocument) -> NodeId {
    graph.nodes.values().find(|node| node.kind == NodeKind::FunctionReturn).unwrap().id
}
fn evaluate(graph: &GraphDocument) -> Component {
    let generated = transpile(graph).unwrap();
    let library = rvn_core::eval::FunctionLibrary::from_script(&generated.ast).unwrap();
    let (parameters, body) = generated.ast.iter().find_map(|statement| match statement {
        rvn_parser::Statement::Screen { parameters, body, .. } => Some((parameters, body)),
        _ => None,
    }).unwrap();
    let value = library.describe_screen(parameters, body, &[rvn_parser::Value::Str("Dynamic title".into())],
        &Default::default(), &mut rvn_core::random::RandomState::default()).unwrap();
    Component::parse(rvn_core::ui::value_to_json(&value).unwrap()).unwrap()
}
fn vertical(padding: f32) -> ParentSlot {
    ParentSlot::VerticalBox { padding: SlotEdges::uniform(padding), horizontal: Alignment::Stretch,
        vertical: Alignment::Stretch, size: BoxSlotSize::default() }
}
fn source_reparse(graph: &GraphDocument) -> GraphDocument {
    let source = transpile(graph).unwrap().source;
    let reparsed = reimport_script(&rvn_parser::parse(&source).unwrap(), &[graph.clone()]).unwrap();
    reparsed.into_iter().find(|item| item.graph_id == graph.graph_id).unwrap()
}

#[test]
fn widget0904_reparent_to_button_uses_real_child_slot_and_preserves_dynamic_text_and_ids() {
    let mut graph = graph(SOURCE);
    let ids = graph.interface_component_ids();
    let before = graph.clone();
    let returned = returned(&graph);
    assert_eq!(graph.interface_widget_reparent(returned, ids["moving"], ids["empty"]).unwrap(), ids["moving"]);
    assert_eq!(graph.interface_component_ids(), ids);
    for (id, node) in &before.nodes { assert_eq!(graph.nodes[id].position, node.position); }
    let result = evaluate(&graph);
    assert_eq!(result.find("left").unwrap().children.iter().map(|child| child.id.as_str()).collect::<Vec<_>>(), vec!["sibling"]);
    assert_eq!(result.find("empty").unwrap().children[0].id, "moving");
    assert_eq!(result.find("moving").unwrap().text, "Dynamic title");
    assert_eq!(result.find("moving").unwrap().parent_slot, Some(ParentSlot::for_parent(rvn_ui::programmable::ComponentKind::Button).unwrap()));
    let reparsed = source_reparse(&graph);
    assert_eq!(reparsed.interface_component_ids(), ids);
    assert_eq!(evaluate(&reparsed), result);
}

#[test]
fn widget0904_occupied_button_refuses_before_detachment_allocation_or_source_changes() {
    let mut graph = graph(SOURCE);
    let ids = graph.interface_component_ids();
    let before = graph.clone();
    let source = transpile(&graph).unwrap().source;
    let returned = returned(&graph);
    let error = graph.interface_widget_reparent(returned, ids["moving"], ids["occupied"]).unwrap_err();
    assert!(error.contains("before detachment"), "{error}");
    assert_eq!(graph, before);
    assert_eq!(transpile(&graph).unwrap().source, source);
    assert_eq!(evaluate(&graph).find("occupied").unwrap().children[0].id, "content");
}

#[test]
fn widget0904_reparent_back_replaces_button_slot_with_column_defaults_without_changing_content() {
    let mut graph = graph(SOURCE);
    let ids = graph.interface_component_ids();
    let returned = returned(&graph);
    graph.interface_widget_reparent(returned, ids["content"], ids["other"]).unwrap();
    let value = evaluate(&graph);
    assert!(value.find("occupied").unwrap().children.is_empty());
    assert_eq!(value.find("other").unwrap().children[0].id, "content");
    assert_eq!(value.find("content").unwrap().parent_slot, Some(vertical(0.)));
    assert_eq!(value.find("content").unwrap().text, "Occupied");
}

#[test]
fn widget0904_same_parent_is_exact_noop_including_authored_slot_and_retained_snapshot() {
    let mut graph = graph(SOURCE);
    let ids = graph.interface_component_ids();
    let before = graph.clone();
    let retained_redo = source_reparse(&graph);
    let returned = returned(&graph);
    assert_eq!(graph.interface_widget_reparent(returned, ids["moving"], ids["left"]).unwrap(), ids["moving"]);
    assert_eq!(graph, before);
    assert_eq!(source_reparse(&graph), retained_redo);
    graph.interface_widget_set_parent_slot(returned, ids["moving"], vertical(8.)).unwrap();
    assert_eq!(graph, before);
}

#[test]
fn widget0904_slot_edit_survives_source_publication_reimport_and_graph_snapshot_replay() {
    let mut project = SourceProject::open(SOURCE, &[]).unwrap();
    let mut graphs = project.graphs().to_vec();
    let graph = graphs.iter_mut().find(|graph| matches!(graph.kind, GraphKind::Screen { .. })).unwrap();
    let before = graph.clone();
    let ids = graph.interface_component_ids();
    let returned = returned(graph);
    graph.interface_widget_set_parent_slot(returned, ids["content"], ParentSlot::Button {
        padding: SlotEdges::uniform(12.), horizontal: Alignment::End, vertical: Alignment::Center,
    }).unwrap();
    let after = graph.clone();
    let after_value = evaluate(graph);
    assert_eq!(after_value.find("content").unwrap().parent_slot, Some(ParentSlot::Button {
        padding: SlotEdges::uniform(12.), horizontal: Alignment::End, vertical: Alignment::Center,
    }));
    // These are the real GraphDocument snapshots consumed by the existing
    // owner/history, with no separate tree/history implementation in this test.
    *graph = before.clone();
    assert_eq!(evaluate(graph).find("content").unwrap().parent_slot.as_ref().unwrap(),
        evaluate(&before).find("content").unwrap().parent_slot.as_ref().unwrap());
    *graph = after;
    project.apply_visual(SOURCE, &graphs).unwrap();
    assert!(project.source().contains("// Keep this Designer source comment across the transaction."));
    assert!(project.source().contains("// This is the authored Widget source"));
    let reparsed = SourceProject::open(project.source(), &graphs).unwrap();
    let saved = reparsed.graphs().iter().find(|graph| matches!(graph.kind, GraphKind::Screen { .. })).unwrap();
    assert_eq!(saved.interface_component_ids(), ids);
    assert_eq!(evaluate(saved), after_value);
}

#[test]
fn widget0904_shared_slot_dictionary_and_dynamic_value_producers_remain_owned_by_other_consumers() {
    let mut graph = graph(SOURCE);
    let ids = graph.interface_component_ids();
    let slot = graph.dictionary_property_source(ids["moving"], "properties", &["parent_slot"]).unwrap().unwrap();
    graph.remove_dictionary_entry(ids["sibling"], "properties", &["parent_slot"]).unwrap();
    graph.connect_dictionary_property(ids["sibling"], "properties", &["parent_slot"], slot, "result").unwrap();
    let before = graph.clone();
    let old_slot = graph.nodes[&slot].clone();
    let old_sibling_properties = transpile_value_input(&graph, ids["sibling"], "properties").unwrap();
    let title = graph.dictionary_property_source(ids["moving"], "properties", &["text"]).unwrap();
    let returned = returned(&graph);
    graph.interface_widget_set_parent_slot(returned, ids["moving"], vertical(16.)).unwrap();
    assert_eq!(graph.nodes[&slot], old_slot);
    assert_eq!(transpile_value_input(&graph, ids["sibling"], "properties").unwrap(), old_sibling_properties);
    assert_eq!(graph.dictionary_property_source(ids["moving"], "properties", &["text"]).unwrap(), title);
    assert_eq!(evaluate(&graph).find("sibling").unwrap().parent_slot, Some(vertical(8.)));
    assert_eq!(evaluate(&graph).find("moving").unwrap().parent_slot, Some(vertical(16.)));
    for (id, node) in &before.nodes { assert_eq!(graph.nodes[id].position, node.position); }
}

#[test]
fn widget0904_computed_slot_field_refuses_edits_and_moves_without_freezing_the_expression() {
    let source = SOURCE.replacen("\"left\":8", "\"left\":padding_value", 1).replace("screen form(title)", "screen form(title,padding_value)");
    let mut graph = graph(&source);
    let ids = graph.interface_component_ids();
    let before = graph.clone();
    let returned = returned(&graph);
    assert!(graph.interface_widget_set_parent_slot(returned, ids["moving"], vertical(2.)).is_err());
    assert_eq!(graph, before);
    assert!(graph.interface_widget_reparent(returned, ids["moving"], ids["empty"]).is_err());
    assert_eq!(graph, before);
    assert_eq!(graph.interface_widget_reparent(returned, ids["moving"], ids["left"]).unwrap(), ids["moving"]);
    assert_eq!(graph, before);
    assert!(transpile(&graph).unwrap().source.contains("padding_value"));
}

#[test]
fn widget0904_computed_target_children_refuse_without_replacing_the_real_function_expression() {
    let source = SOURCE.replace("screen form(title)", "screen form(title,rows)")
        .replace("component(\"other\",\"column\",{\"spacing\":0,\"parent_slot\":{\"kind\":\"vertical_box\"}},[])",
            "component(\"other\",\"column\",{\"spacing\":0,\"parent_slot\":{\"kind\":\"vertical_box\"}},rows)");
    let mut graph = graph(&source);
    let ids = graph.interface_component_ids();
    let before = graph.clone();
    let returned = returned(&graph);
    assert!(graph.interface_widget_reparent(returned, ids["moving"], ids["other"]).is_err());
    assert_eq!(graph, before);
    assert!(transpile(&graph).unwrap().source.contains("rows"));
}

#[test]
fn widget0904_slot_mismatch_nonfinite_bounds_root_and_cycle_are_atomic_refusals() {
    let mut graph = graph(SOURCE);
    let ids = graph.interface_component_ids();
    let before = graph.clone();
    let returned = returned(&graph);
    for slot in [ParentSlot::for_parent(rvn_ui::programmable::ComponentKind::Button).unwrap(),
        vertical(f32::NAN), vertical(16385.)] {
        assert!(graph.interface_widget_set_parent_slot(returned, ids["moving"], slot).is_err());
        assert_eq!(graph, before);
    }
    assert!(graph.interface_widget_set_parent_slot(returned, ids["root"], vertical(1.)).is_err());
    assert!(graph.interface_widget_reparent(returned, ids["left"], ids["moving"]).is_err());
    assert!(graph.interface_widget_reparent(returned, ids["root"], ids["empty"]).is_err());
    assert_eq!(graph, before);
}

#[test]
fn widget0904_legacy_reparent_api_and_missing_opt_in_remain_unchanged() {
    let source = r#"screen legacy(){return component("root","column",{},[component("left","column",{},[component("label","text",{"text":"Legacy"},[])]),component("button","button",{"text":"Leaf"},[]),component("right","column",{},[])])}"#;
    let mut graph = graph(source);
    let ids = graph.interface_component_ids();
    let returned = returned(&graph);
    let before = graph.clone();
    assert!(graph.interface_widget_reparent(returned, ids["label"], ids["right"]).is_err());
    assert_eq!(graph, before);
    assert!(graph.interface_reparent(returned, ids["label"], ids["button"]).is_err());
    assert_eq!(graph, before);
    graph.interface_reparent(returned, ids["label"], ids["right"]).unwrap();
    assert_eq!(graph.interface_component_ids(), ids);
    assert!(!transpile(&graph).unwrap().source.contains("parent_slot"));
}

#[test]
fn widget0904_raw_dictionaries_keep_component_identity_and_dynamic_content_when_moved() {
    let source = r#"screen raw(title){return {"id":"root","kind":"column","spacing":0,"layout_contract":"widget_v1","children":[{"id":"left","kind":"column","spacing":0,"parent_slot":{"kind":"vertical_box"},"children":[{"id":"label","kind":"text","spacing":0,"text":title,"parent_slot":{"kind":"vertical_box"}}]},{"id":"button","kind":"button","spacing":0,"parent_slot":{"kind":"vertical_box"},"children":[]}]}}"#;
    let mut graph = graph(source);
    let ids = graph.interface_component_ids();
    let returned = returned(&graph);
    graph.interface_widget_reparent(returned, ids["label"], ids["button"]).unwrap();
    assert_eq!(graph.interface_component_ids(), ids);
    assert_eq!(evaluate(&graph).find("button").unwrap().children[0].id, "label");
    assert_eq!(evaluate(&source_reparse(&graph)).find("label").unwrap().text, "Dynamic title");
}

#[test]
fn widget0904_shared_component_owner_refuses_instead_of_forking_a_duplicate_component_id() {
    let mut graph = graph(SOURCE);
    let ids = graph.interface_component_ids();
    let observing_list = graph.add_catalog_node(NodeKind::ListLiteral, [1400., 600.]).unwrap();
    graph.resize_value_inputs(observing_list, 1).unwrap();
    graph.connect(graph.pin_by_key(ids["moving"], "value").unwrap().id,
        graph.pin_by_key(observing_list, "item_0").unwrap().id).unwrap();
    let before = graph.clone();
    let returned = returned(&graph);
    assert!(graph.interface_widget_reparent(returned, ids["moving"], ids["empty"]).unwrap_err().contains("Shared component ownership"));
    assert_eq!(graph, before);
    assert!(graph.interface_widget_set_parent_slot(returned, ids["moving"], vertical(16.)).is_err());
    assert_eq!(graph, before);
    assert_eq!(graph.interface_component_ids(), ids);
}

#[test]
fn widget0904_shared_old_children_collection_refuses_without_detaching_another_consumer() {
    let mut graph = graph(SOURCE);
    let ids = graph.interface_component_ids();
    let children = graph.pin_by_key(ids["left"], "children").unwrap().id;
    let output = graph.edges.values().find(|edge| edge.input == children).unwrap().output;
    let observing_list = graph.add_catalog_node(NodeKind::ListLiteral, [1400., 600.]).unwrap();
    graph.resize_value_inputs(observing_list, 1).unwrap();
    graph.connect(output, graph.pin_by_key(observing_list, "item_0").unwrap().id).unwrap();
    let before = graph.clone();
    let returned = returned(&graph);
    assert!(graph.interface_widget_reparent(returned, ids["moving"], ids["empty"]).unwrap_err().contains("Shared child collection"));
    assert_eq!(graph, before);
    assert_eq!(graph.edges.values().filter(|edge| edge.output == output).count(), 2);
    assert_eq!(graph.interface_component_ids(), ids);
}

#[test]
fn widget0904_wrong_actual_parent_slot_and_calculated_root_contract_refuse_atomically() {
    let mut graph = graph(SOURCE);
    let ids = graph.interface_component_ids();
    graph.set_component_property(ids["moving"], &["parent_slot", "kind"], PropertyValue::String("button".into())).unwrap();
    let before = graph.clone();
    let returned = returned(&graph);
    assert!(graph.interface_widget_reparent(returned, ids["moving"], ids["empty"]).is_err());
    assert_eq!(graph, before);
    let source = SOURCE.replace("\"layout_contract\":\"widget_v1\"", "\"layout_contract\":title");
    let mut dynamic = crate::graph(&source);
    let ids = dynamic.interface_component_ids();
    let before = dynamic.clone();
    let returned = crate::returned(&dynamic);
    assert!(dynamic.interface_widget_reparent(returned, ids["moving"], ids["empty"]).is_err());
    assert_eq!(dynamic, before);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn widget0904_actual_source_workspace_saves_and_cold_opens_slots_and_refuses_external_conflict() {
    use std::{fs, time::{SystemTime, UNIX_EPOCH}};
    let root = std::env::temp_dir().join(format!("rvn-widget-graph-slots-0904-{}-{}", std::process::id(), SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
    fs::create_dir(&root).unwrap();
    let path = root.join("widget.rvn");
    fs::write(&path, SOURCE).unwrap();
    let mut workspace = SourceWorkspace::link(&root, &path, &[]).unwrap();
    let mut graphs = workspace.project().graphs().to_vec();
    let graph = graphs.iter_mut().find(|graph| matches!(graph.kind, GraphKind::Screen { .. })).unwrap();
    let ids = graph.interface_component_ids();
    let returned = returned(graph);
    graph.interface_widget_reparent(returned, ids["moving"], ids["empty"]).unwrap();
    graph.interface_widget_set_parent_slot(returned, ids["moving"], ParentSlot::Button {
        padding: SlotEdges::uniform(6.), horizontal: Alignment::Center, vertical: Alignment::Center,
    }).unwrap();
    let expected = evaluate(graph);
    workspace.save(&graphs).unwrap();
    let mut cold = SourceWorkspace::open(&root).unwrap().unwrap();
    assert_eq!(cold.project().graphs(), workspace.project().graphs());
    let saved = cold.project().graphs().iter().find(|graph| matches!(graph.kind, GraphKind::Screen { .. })).unwrap();
    assert_eq!(saved.interface_component_ids(), ids);
    assert_eq!(evaluate(saved), expected);
    let mut pending = cold.project().graphs().to_vec();
    let graph = pending.iter_mut().find(|graph| matches!(graph.kind, GraphKind::Screen { .. })).unwrap();
    graph.interface_widget_set_parent_slot(returned, ids["moving"], ParentSlot::Button {
        padding: SlotEdges::uniform(12.), horizontal: Alignment::Center, vertical: Alignment::Center,
    }).unwrap();
    let baseline = cold.project().graphs().to_vec();
    let before_sidecar = fs::read(root.join(".rvn-authoring.json")).unwrap();
    let externally_changed = format!("{}\n// Actual external source edit must survive failed Save.\n", fs::read_to_string(&path).unwrap());
    fs::write(&path, &externally_changed).unwrap();
    assert!(cold.save(&pending).is_err());
    assert_eq!(fs::read_to_string(&path).unwrap(), externally_changed);
    assert_eq!(fs::read(root.join(".rvn-authoring.json")).unwrap(), before_sidecar);
    assert_eq!(cold.project().graphs(), baseline);
    // Preserve the private source/backups for Root's qualification evidence.
}
