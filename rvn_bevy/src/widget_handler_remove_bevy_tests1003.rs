//! Actual loader, SourceProject, one callback VM and production wrapper batch.
//! No rendered/natively injected input or pixel acceptance is claimed.
use super::*;
use rvn_graph::{GraphKind, NodeKind, PropertyValue};
fn authored_project(refuse_destruct: bool) -> Project {
    let project = Project::new(false); fs::remove_file(project.root.join(".rvn-authoring.json")).unwrap();
    let source = LIFECYCLE.replace("label start\n", "scene label start\nset saved_object=0\nset neighbor_object=0\n")
        .replace("function child(){if rooted{return component(\"real\",\"text\",{\"text\":\"actual\"},[])} return []}",
            "function child(){return component(\"go\",\"button\",{\"events\":{\"click\":\"click_remove\"}},[])}")
        .replacen("init{", "handler click_remove(event){local roll=random(1,100) local removed_marker=1 set hits=hits+1}\ninit{set hits=0 ", 1);
    let source = if refuse_destruct { source.replace("handler destruct(event){set destructed_count=destructed_count+1}",
        "handler destruct(event){set destructed_count=destructed_count+1 local invalid_interface=\"remove1003_destruct_refusal\" ui.open(invalid_interface,[],false,0)}") } else { source };
    let path = project.root.join("main.rvn"); fs::write(&path, &source).unwrap();
    let mut source_project = rvn_graph::SourceProject::open_at(&path, &source, &[]).unwrap(); let mut graphs = source_project.graphs().to_vec();
    let scene = graphs.iter_mut().find(|graph| matches!(&graph.kind, GraphKind::Label { name } if name == "start")).unwrap(); scene.enable_native_frame_preparation().unwrap();
    let assignments = ["saved_object", "neighbor_object"].map(|name| {
        let candidates = scene.nodes.values().filter(|node| node.kind == NodeKind::SetVariable && node.properties.get("name") == Some(&PropertyValue::String(name.into()))).map(|node| node.id).collect::<Vec<_>>(); assert_eq!(candidates.len(), 1);
        (candidates[0], rvn_parser::Expr::Call { name: "__rvn_widget_class_v1".into(), args: vec![rvn_parser::Expr::Str(UUID.into())] }, rvn_parser::Expr::ListLit(vec![rvn_parser::Expr::Int(1)]))
    });
    scene.replace_assignments_with_widget_factories(&assignments).unwrap(); source_project.apply_visual(&source, &graphs).unwrap(); let source = source_project.source().to_owned(); fs::write(&path, &source).unwrap();
    let mut source_project = rvn_graph::SourceProject::open_at(&path, &source, &[]).unwrap(); let mut graphs = source_project.graphs().to_vec();
    let handler = graphs.iter_mut().find(|graph| matches!(&graph.kind, GraphKind::Handler { name } if name == "click_remove")).unwrap(); handler.enable_native_frame_preparation().unwrap();
    let marker = handler.nodes.values().find(|node| node.kind == NodeKind::LocalVariable && node.properties.get("name") == Some(&PropertyValue::String("removed_marker".into()))).unwrap().id;
    let input = handler.pin_by_key(marker, "exec_in").unwrap().id; let incoming = handler.edges.values().filter(|edge| edge.input == input).cloned().collect::<Vec<_>>(); let [edge] = incoming.as_slice() else { panic!("one real predecessor"); }; let output = edge.output; handler.remove_edge(edge.id);
    let remove = handler.add_catalog_node(NodeKind::WidgetRemoveFromParent, [200., 0.]).unwrap(); handler.connect(output, handler.pin_by_key(remove, "exec_in").unwrap().id).unwrap(); handler.connect(handler.pin_by_key(remove, "exec_out").unwrap().id, input).unwrap();
    let read = handler.add_catalog_node(NodeKind::VariableGet, [0., 100.]).unwrap(); handler.set_property(read, "name", PropertyValue::String("saved_object".into())).unwrap(); handler.refresh_variable_usage_bindings().unwrap();
    assert_eq!(handler.effective_pin_type(handler.pin_by_key(read, "value").unwrap().id), Some(rvn_graph::ValueType::WidgetObject)); handler.connect(handler.pin_by_key(read, "value").unwrap().id, handler.pin_by_key(remove, "object").unwrap().id).unwrap();
    source_project.apply_visual(&source, &graphs).unwrap(); let source = source_project.source().to_owned(); fs::write(&path, &source).unwrap();
    let parsed = rvn_graph::SourceProject::open_at(&path, &source, &[]).unwrap(); let workspace = rvn_graph::SourceWorkspace::link(&project.root, &path, parsed.graphs()).unwrap();
    let screen = workspace.project().graphs().iter().find(|graph| matches!(&graph.kind, GraphKind::Screen { name } if name == "panel")).unwrap(); let id = WidgetGraphRef16::from_u64(screen.graph_id.get());
    let members = workspace.project().graphs().iter().filter(|graph| matches!(&graph.kind, GraphKind::Screen { name } if name == "panel") || matches!(&graph.kind, GraphKind::Handler { .. } | GraphKind::Function { .. })).map(|graph| WidgetGraphRef16::from_u64(graph.graph_id.get())).collect();
    fs::write(project.root.join("assets/panel.rvnwidget"), WidgetBlueprintV1 { uuid: UUID.into(), source: "main.rvn".into(), screen_graph: id, members }.encode().unwrap()).unwrap(); project
}
fn reference_alias(value: &Value) -> &str {
    let Value::Widget(reference) = value else { panic!("actual Scene factory result"); }; reference.portable().unwrap().alias()
}
fn click<R: rvn_core::Renderer>(engine: &rvn_core::Engine<R>, alias: &str) -> rvn_core::ui_input::ScopedUiInput {
    rvn_core::ui_input::ScopedUiInput { source: engine.capture_ui_input_source(alias).unwrap(), event: rvn_core::ui::UiInput { screen: alias.into(), element: "go".into(), kind: rvn_ui::programmable::ScreenEventKind::Click, value: None, key: None } }
}
#[test]
fn handler_remove1003_actual_bevy_callback_releases_only_one_wrapper_retains_object_and_reconstructs_once() {
    let project = authored_project(false); let mut engine = load_game(&project.root).unwrap().engine; engine.step_until_interaction().unwrap();
    let left = engine.widget_instance_handle(reference_alias(&engine.state.vars["saved_object"])).unwrap(); let right = engine.widget_instance_handle(reference_alias(&engine.state.vars["neighbor_object"])).unwrap(); assert_ne!(left, right);
    engine.add_widget_to_viewport(&left, false, 0).unwrap(); engine.add_widget_to_viewport(&right, false, 1).unwrap();
    let left_owner = engine.widget_backend_owner(&left).unwrap(); let right_owner = engine.widget_backend_owner(&right).unwrap(); assert_eq!(left_owner.class(), right_owner.class());
    let weak = engine.renderer.widget_backend.wrapper(&left_owner).unwrap(); let original = weak.upgrade().unwrap(); let generation = original.generation(); let other = engine.renderer.widget_backend.wrapper(&right_owner).unwrap().upgrade().unwrap();
    assert_eq!(engine.state.vars["initialized_count"], Value::Int(2)); assert_eq!(engine.state.vars["constructed_count"], Value::Int(2));
    let event = click(&engine, left.alias()); let old = event.clone(); assert!(engine.interface_event_scoped(event).unwrap());
    assert_eq!(engine.state.vars["hits"], Value::Int(1)); assert_eq!(engine.state.vars["destructed_count"], Value::Int(1)); assert!(!original.attached()); assert!(engine.renderer.widget_backend.wrapper(&left_owner).is_none());
    assert!(other.attached()); assert!(std::sync::Arc::ptr_eq(&other, &engine.renderer.widget_backend.wrapper(&right_owner).unwrap().upgrade().unwrap())); assert_eq!(engine.widget_instance_handle(left.alias()).unwrap(), left);
    let before = serde_json::to_vec(&engine.state).unwrap(); assert!(!engine.interface_event_scoped(old).unwrap()); assert_eq!(serde_json::to_vec(&engine.state).unwrap(), before);
    drop(original); assert!(weak.upgrade().is_none()); engine.add_widget_to_viewport(&left, false, 0).unwrap(); let replacement = engine.renderer.widget_backend.wrapper(&left_owner).unwrap().upgrade().unwrap(); assert!(replacement.generation() > generation);
    assert_eq!(engine.state.vars["constructed_count"], Value::Int(3)); assert_eq!(engine.state.vars["initialized_count"], Value::Int(2)); assert_eq!(engine.state.vars["destructed_count"], Value::Int(1)); assert!(std::sync::Arc::ptr_eq(&other, &engine.renderer.widget_backend.wrapper(&right_owner).unwrap().upgrade().unwrap()));
}
#[test]
fn handler_remove1003_actual_bevy_destruct_refusal_keeps_both_original_wrappers_and_candidate_unpublished() {
    let project = authored_project(true); let mut engine = load_game(&project.root).unwrap().engine; engine.step_until_interaction().unwrap();
    let left = engine.widget_instance_handle(reference_alias(&engine.state.vars["saved_object"])).unwrap(); let right = engine.widget_instance_handle(reference_alias(&engine.state.vars["neighbor_object"])).unwrap();
    engine.add_widget_to_viewport(&left, false, 0).unwrap(); engine.add_widget_to_viewport(&right, false, 1).unwrap();
    let left_owner = engine.widget_backend_owner(&left).unwrap(); let right_owner = engine.widget_backend_owner(&right).unwrap(); let first = engine.renderer.widget_backend.wrapper(&left_owner).unwrap().upgrade().unwrap(); let second = engine.renderer.widget_backend.wrapper(&right_owner).unwrap().upgrade().unwrap();
    let before = serde_json::to_vec(&engine.state).unwrap(); let published = serde_json::to_vec(&engine.renderer.scene_batch.as_ref().unwrap().state).unwrap(); let event = click(&engine, left.alias()); let error = engine.interface_event_scoped(event).unwrap_err();
    assert!(error.to_string().contains("remove1003_destruct_refusal"), "{error}"); assert_eq!(serde_json::to_vec(&engine.state).unwrap(), before); assert_eq!(serde_json::to_vec(&engine.renderer.scene_batch.as_ref().unwrap().state).unwrap(), published);
    assert!(first.attached() && second.attached()); assert!(std::sync::Arc::ptr_eq(&first, &engine.renderer.widget_backend.wrapper(&left_owner).unwrap().upgrade().unwrap())); assert!(std::sync::Arc::ptr_eq(&second, &engine.renderer.widget_backend.wrapper(&right_owner).unwrap().upgrade().unwrap()));
}
