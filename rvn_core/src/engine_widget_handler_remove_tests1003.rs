//! Actual SourceProject/Graph factories and one existing callback transaction.
//! Prepared tests only: CPU logical host, no native/pixel acceptance.
use super::*;
use rvn_graph::{GraphKind, NodeKind, PropertyValue};
fn authored_fixture(extra: &str) -> Fixture {authored_fixture_with_host(extra,false)}
fn authored_fixture_with_host(extra:&str,derived_property:bool)->Fixture{
    Fixture::with_source_transform_at(false, false, false, false, |root, source| {
        let source=if derived_property{
            let screen="screen panel(method){return component(\"go\",\"button\",{\"events\":{\"click\":method}},[])}";
            assert!(source.contains(screen));source.replace(screen,"screen panel(method){return {\"host\":{\"enabled\":hits==0},\"root\":component(\"go\",\"button\",{\"events\":{\"click\":method}},[])}}")
        }else{source};
        let source = source.replacen("label start\n", "scene label start\nset saved_object=0\nset neighbor_object=0\n", 1);
        let path = root.join("main.rvn"); fs::write(&path, &source).unwrap();
        let mut project = rvn_graph::SourceProject::open_at(&path, &source, &[]).unwrap();
        let mut graphs = project.graphs().to_vec();
        let scene = graphs.iter_mut().find(|graph| matches!(&graph.kind, GraphKind::Label { name } if name == "start")).unwrap();
        scene.enable_native_frame_preparation().unwrap();
        let assignments = ["saved_object", "neighbor_object"].map(|name| {
            let candidates = scene.nodes.values().filter(|node| node.kind == NodeKind::SetVariable
                && node.properties.get("name") == Some(&PropertyValue::String(name.into()))).map(|node| node.id).collect::<Vec<_>>();
            assert_eq!(candidates.len(), 1);
            (candidates[0], rvn_parser::Expr::Call { name: "__rvn_widget_class_v1".into(), args: vec![rvn_parser::Expr::Str(A.into())] },
                rvn_parser::Expr::ListLit(vec![rvn_parser::Expr::Str("allowed".into())]))
        });
        scene.replace_assignments_with_widget_factories(&assignments).unwrap();
        project.apply_visual(&source, &graphs).unwrap();
        // Publish typed global declarations before importing the Handler that
        // reads them; never retag a pin or replace a frozen Engine AST.
        let source = project.source().replace(
            "handler allowed(event){local roll=random(1,100) while false{set roll=roll+1} set hits=hits+1}",
            &format!("handler allowed(event){{local roll=random(1,100) local removed_marker=1 set hits=hits+1 {extra}}}"));
        fs::write(&path, &source).unwrap();
        let mut project = rvn_graph::SourceProject::open_at(&path, &source, &[]).unwrap(); let mut graphs = project.graphs().to_vec();
        let handler = graphs.iter_mut().find(|graph| matches!(&graph.kind, GraphKind::Handler { name } if name == "allowed")).unwrap();
        handler.enable_native_frame_preparation().unwrap();
        let marker = handler.nodes.values().find(|node| node.kind == NodeKind::LocalVariable
            && node.properties.get("name") == Some(&PropertyValue::String("removed_marker".into()))).unwrap().id;
        let input = handler.pin_by_key(marker, "exec_in").unwrap().id;
        let incoming = handler.edges.values().filter(|edge| edge.input == input).cloned().collect::<Vec<_>>();
        let [edge] = incoming.as_slice() else { panic!("one actual marker predecessor"); };
        let output = edge.output; handler.remove_edge(edge.id);
        let remove = handler.add_catalog_node(NodeKind::WidgetRemoveFromParent, [200., 0.]).unwrap();
        handler.connect(output, handler.pin_by_key(remove, "exec_in").unwrap().id).unwrap();
        handler.connect(handler.pin_by_key(remove, "exec_out").unwrap().id, input).unwrap();
        let read = handler.add_catalog_node(NodeKind::VariableGet, [0., 100.]).unwrap();
        handler.set_property(read, "name", PropertyValue::String("saved_object".into())).unwrap(); handler.refresh_variable_usage_bindings().unwrap();
        assert_eq!(handler.effective_pin_type(handler.pin_by_key(read, "value").unwrap().id), Some(rvn_graph::ValueType::WidgetObject));
        handler.connect(handler.pin_by_key(read, "value").unwrap().id, handler.pin_by_key(remove, "object").unwrap().id).unwrap();
        project.apply_visual(&source, &graphs).unwrap(); project.source().to_owned()
    })
}
fn attached(fixture: &Fixture) -> (Engine<Host>, WidgetInstanceHandle, WidgetInstanceHandle) {
    let mut engine = fixture.engine();
    let first = engine.widget_reference_handle(&engine.state.vars["saved_object"]).unwrap();
    let second = engine.widget_reference_handle(&engine.state.vars["neighbor_object"]).unwrap();
    assert_ne!(first, second); assert_eq!(engine.widget_backend_owner(&first).unwrap().class(), engine.widget_backend_owner(&second).unwrap().class());
    engine.add_widget_to_viewport(&first, false, 0).unwrap(); engine.add_widget_to_viewport(&second, false, 1).unwrap();
    (engine, first, second)
}
// Authenticate the real process/runtime handle using the existing public
// backend-owner API, then resolve that exact target incarnation in GameState.
// An alias lookup alone would hide stale/foreign/replaced-object mistakes.
fn actual_viewport_attached(engine: &Engine<Host>, handle: &WidgetInstanceHandle) -> bool {
    let owner = engine.widget_backend_owner(handle).expect("current admitted Widget handle");
    let resident = crate::ui_residency::resident_interface(&engine.state, owner.target())
        .expect("the exact admitted Widget target remains resident");
    assert_eq!(&resident.identity, owner.target());
    assert_eq!(resident.instance.order, owner.order());
    assert_eq!(&resident.instance.widget_class.as_ref().expect("actual Widget class").class, owner.class());
    resident.instance.viewport_attached()
}
#[test]
fn handler_remove1003_actual_native_callback_detaches_only_the_referenced_instance_once() {
    let fixture = authored_fixture(""); let (mut engine, first, second) = attached(&fixture);
    let first_value = engine.state.vars["saved_object"].clone(); let second_owner = engine.widget_backend_owner(&second).unwrap();
    let event = input(&engine, first.alias(), rvn_ui::programmable::ScreenEventKind::Click); let old = event.clone();
    let before = engine.renderer.batches.lock().unwrap().len();
    assert!(engine.interface_event_scoped(event).unwrap());
    assert_eq!(engine.state.vars["hits"], Value::Int(1)); assert_eq!(engine.renderer.batches.lock().unwrap().len(), before + 1);
    assert!(!actual_viewport_attached(&engine, &first));
    assert!(actual_viewport_attached(&engine, &second));
    assert_eq!(engine.widget_reference_handle(&first_value).unwrap(), first); assert_eq!(engine.widget_backend_owner(&second).unwrap(), second_owner);
    let before = serde_json::to_vec(&engine.state).unwrap(); let publications = engine.renderer.batches.lock().unwrap().len();
    assert!(!engine.interface_event_scoped(old.clone()).unwrap()); assert_eq!(serde_json::to_vec(&engine.state).unwrap(), before); assert_eq!(engine.renderer.batches.lock().unwrap().len(), publications);
    engine.add_widget_to_viewport(&first, false, 0).unwrap(); assert_eq!(engine.widget_reference_handle(&first_value).unwrap(), first);
    assert!(!engine.ui_input_source_current(&old.source),"callback detachment revoked the old presentation even after reattachment of the same object");
    assert_ne!(engine.capture_ui_input_source(first.alias()).unwrap().authority,old.source.authority);
}
#[test]
fn handler_remove1003_candidate_command_and_renderer_refusals_leave_rng_targets_and_publication_atomic() {
    let fixture = authored_fixture(""); let (mut engine, first, second) = attached(&fixture);
    engine.renderer.reject.store(true, Ordering::SeqCst);
    let before = serde_json::to_vec(&engine.state).unwrap(); let random = engine.host_random.clone(); let publications = engine.renderer.batches.lock().unwrap().len();
    let event = input(&engine, first.alias(), rvn_ui::programmable::ScreenEventKind::Click);
    assert!(engine.interface_event_scoped(event).is_err()); assert_eq!(serde_json::to_vec(&engine.state).unwrap(), before); assert_eq!(engine.host_random, random); assert_eq!(engine.renderer.batches.lock().unwrap().len(), publications);
    assert!(actual_viewport_attached(&engine, &first)); assert!(actual_viewport_attached(&engine, &second));
    engine.renderer.reject.store(false, Ordering::SeqCst);
    let event = input(&engine, first.alias(), rvn_ui::programmable::ScreenEventKind::Click); assert!(engine.interface_event_scoped(event).unwrap());
    let fixture = authored_fixture("local invalid_interface=\"handler_remove1003_unknown\" ui.open(invalid_interface,[],false,0)");
    let (mut engine, first, _) = attached(&fixture); let before = serde_json::to_vec(&engine.state).unwrap(); let random = engine.host_random.clone(); let publications = engine.renderer.batches.lock().unwrap().len();
    let event = input(&engine, first.alias(), rvn_ui::programmable::ScreenEventKind::Click); let error = engine.interface_event_scoped(event).unwrap_err();
    assert!(error.to_string().contains("handler_remove1003_unknown"), "{error}"); assert_eq!(serde_json::to_vec(&engine.state).unwrap(), before); assert_eq!(engine.host_random, random); assert_eq!(engine.renderer.batches.lock().unwrap().len(), publications);
}
#[test]
fn handler_remove1003_wrong_null_foreign_and_retired_objects_cannot_adopt_a_current_alias() {
    let fixture = authored_fixture(""); let (mut foreign, foreign_first, _) = attached(&fixture); let foreign_value = foreign.widget_reference_value(&foreign_first).unwrap();
    let (mut engine, first, second) = attached(&fixture); let value = engine.state.vars["saved_object"].clone();
    for wrong in [Value::Str(first.alias().into()), Value::Bool(false), Value::Widget(Box::new(rvn_parser::widget_reference::WidgetReference::null())), foreign_value] {
        engine.state.vars.insert("saved_object".into(), wrong); let before = serde_json::to_vec(&engine.state).unwrap(); let random = engine.host_random.clone(); let publications = engine.renderer.batches.lock().unwrap().len();
        let event = input(&engine, second.alias(), rvn_ui::programmable::ScreenEventKind::Click); assert!(engine.interface_event_scoped(event).is_err());
        assert_eq!(serde_json::to_vec(&engine.state).unwrap(), before); assert_eq!(engine.host_random, random); assert_eq!(engine.renderer.batches.lock().unwrap().len(), publications);
    }
    engine.state.vars.insert("saved_object".into(), value.clone()); let alias = first.alias().to_owned(); engine.destroy_widget(first).unwrap();
    let replacement = engine.create_widget(A, &alias, vec![Value::Str("allowed".into())]).unwrap(); engine.add_widget_to_viewport(&replacement, false, 0).unwrap();
    engine.state.vars.insert("saved_object".into(), value); let before = serde_json::to_vec(&engine.state).unwrap(); let publications = engine.renderer.batches.lock().unwrap().len();
    let event = input(&engine, second.alias(), rvn_ui::programmable::ScreenEventKind::Click); assert!(engine.interface_event_scoped(event).is_err()); assert_eq!(serde_json::to_vec(&engine.state).unwrap(), before); assert_eq!(engine.renderer.batches.lock().unwrap().len(), publications); assert!(actual_viewport_attached(&engine, &replacement));
    // Keep the second Engine live until after the foreign-reference refusal.
    foreign.remove_widget_from_viewport(&foreign_first).unwrap();
}
#[test]
fn handler_remove1003_queued_typed_command_roundtrips_actual_save15_and_resumes_once() {
    let fixture = authored_fixture(""); let (mut engine, first, _) = attached(&fixture); let old = engine.state.vars["saved_object"].clone();
    let event = input(&engine, first.alias(), rvn_ui::programmable::ScreenEventKind::Click); let mut session = engine.debug_begin(DebugOperation::Input(event), vec![]).unwrap(); let mut stopped = false;
    for _ in 0..256 {
        let (progress, _) = engine.debug_step(&mut session, crate::editor_debugger::StepKind::Into).unwrap(); assert_ne!(progress, DebugProgress::Complete);
        if session.snapshot().frames.iter().any(|frame| frame.bindings.iter().any(|(name, value)| name == "removed_marker" && matches!(value, crate::editor_debugger::WatchValue::Value(Value::Int(1))))) { stopped = true; break; }
    }
    assert!(stopped); assert_eq!(engine.state.vars["hits"], Value::Int(0)); assert!(actual_viewport_attached(&engine, &first));
    let save = engine.capture_save15(&fixture.program, Some(&session), 1, "typed queued removal".into()).unwrap(); let encoded = save.encode().unwrap(); assert!(encoded.contains("WidgetRemoveFromParent"), "the actual queued typed command must be retained, not reconstructed from globals"); let save = crate::save::consumer_checkpoint::Save15::decode(&encoded).unwrap();
    let mut session = engine.restore_save15(&fixture.program, save).unwrap().unwrap(); assert!(engine.widget_reference_handle(&old).is_err());
    let fresh = engine.widget_reference_handle(&engine.state.vars["saved_object"]).unwrap();
    assert!(engine.has_pending_widget_materialization());
    let logical=crate::ui_input::capture_logical(&engine.state,engine.instance_id(),engine.runtime_epoch(),engine.interface_epoch(),fresh.alias()).unwrap();
    assert!(!engine.ui_input_source_current(&logical),"restored logical input is not a new presentation grant");
    let save=engine.capture_save15(&fixture.program,Some(&session),2,"resaved stopped removal".into()).unwrap();
    let mut forged=serde_json::to_value(&save).unwrap();
    let records=forged["debugger"]["trace"]["records"].as_array_mut().expect("actual completed roots before the stopped Handler");
    let extra=records.first().expect("at least one genuine completed Screen root").clone();records.push(extra);
    let forged=crate::save::consumer_checkpoint::Save15::decode(&forged.to_string()).unwrap();
    let before=serde_json::to_vec(&engine.state).unwrap();let random=engine.host_random.clone();let publications=engine.renderer.batches.lock().unwrap().len();let epoch=engine.runtime_epoch();
    assert!(engine.restore_save15(&fixture.program,forged).is_err(),"a real internal continuation grant never admits an extra unrequested completed root");
    assert_eq!(serde_json::to_vec(&engine.state).unwrap(),before);assert_eq!(engine.host_random,random);assert_eq!(engine.renderer.batches.lock().unwrap().len(),publications);assert_eq!(engine.runtime_epoch(),epoch);assert!(engine.accepts_debug_session(&session));
    session=engine.restore_save15(&fixture.program,save).unwrap().unwrap();
    let fresh=engine.widget_reference_handle(&engine.state.vars["saved_object"]).unwrap(); let mut completed = false;
    for _ in 0..256 { let (progress, _) = engine.debug_step(&mut session, crate::editor_debugger::StepKind::Continue).unwrap(); if progress == DebugProgress::Complete { completed = true; break; } }
    assert!(completed); assert_eq!(engine.state.vars["hits"], Value::Int(1)); assert!(!actual_viewport_attached(&engine, &fresh));
    let before = serde_json::to_vec(&engine.state).unwrap(); assert!(engine.debug_step(&mut session, crate::editor_debugger::StepKind::Continue).is_err()); assert_eq!(serde_json::to_vec(&engine.state).unwrap(), before);
}

#[path="engine_widget_input_residency_tests1003.rs"]
mod input_residency1003;

#[path="engine_widget_detach_continuation_tests1003.rs"]
mod detach_continuation1003;
