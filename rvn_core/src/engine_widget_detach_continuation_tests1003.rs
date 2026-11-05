//! A stopped VM AFTER Remove is still an unpublished atomic Host transaction.
use super::*;
#[test]
fn detach_continuation1003_after_remove_and_following_property_computation_save15_resumes_once(){
    let fixture=authored_fixture_with_host("local continuation_after_remove=7 set hits=hits+1",true);
    let(mut engine,first,second)=attached(&fixture);let neighbor=engine.widget_backend_owner(&second).unwrap();
    let event=input(&engine,first.alias(),rvn_ui::programmable::ScreenEventKind::Click);
    let old=event.clone();let before=serde_json::to_vec(&engine.state).unwrap();let publications=engine.renderer.batches.lock().unwrap().len();
    let mut session=engine.debug_begin(DebugOperation::Input(event),vec![]).unwrap();let mut stopped=false;
    for _ in 0..512{let(progress,_)=engine.debug_step(&mut session,crate::editor_debugger::StepKind::Into).unwrap();assert_ne!(progress,DebugProgress::Complete);
        if session.snapshot().frames.iter().any(|frame|frame.bindings.iter().any(|(name,value)|name=="continuation_after_remove"&&matches!(value,crate::editor_debugger::WatchValue::Value(Value::Int(7))))){stopped=true;break;}}
    assert!(stopped,"real VM site following Remove and the first hits assignment");
    assert_eq!(serde_json::to_vec(&engine.state).unwrap(),before,"a stopped candidate never publishes a partial detach/property");
    assert_eq!(engine.renderer.batches.lock().unwrap().len(),publications);assert!(actual_viewport_attached(&engine,&first));assert!(engine.ui_input_source_current(&old.source));
    let save=engine.capture_save15(&fixture.program,Some(&session),1,"after logical Remove".into()).unwrap();
    let encoded=save.encode().unwrap();assert!(encoded.contains("WidgetRemoveFromParent"));
    session=engine.restore_save15(&fixture.program,crate::save::consumer_checkpoint::Save15::decode(&encoded).unwrap()).unwrap().unwrap();
    let save=engine.capture_save15(&fixture.program,Some(&session),2,"same admitted continuation resaved".into()).unwrap();
    session=engine.restore_save15(&fixture.program,save).unwrap().unwrap();
    let current=engine.widget_reference_handle(&engine.state.vars["saved_object"]).unwrap();
    let logical=crate::ui_input::capture_logical(&engine.state,engine.instance_id(),engine.runtime_epoch(),engine.interface_epoch(),current.alias()).unwrap();
    assert!(!engine.ui_input_source_current(&logical),"pending reconstruction never admits a new physical event");
    let mut complete=false;for _ in 0..512{let(progress,_)=engine.debug_step(&mut session,crate::editor_debugger::StepKind::Continue).unwrap();if progress==DebugProgress::Complete{complete=true;break;}}
    assert!(complete);assert_eq!(engine.state.vars["hits"],Value::Int(2));assert!(!actual_viewport_attached(&engine,&current));
    let owner=engine.widget_backend_owner(&current).unwrap();let resident=crate::ui_residency::resident_interface(&engine.state,owner.target()).unwrap();
    assert!(!resident.instance.owner_properties.enabled,"the later global assignment updates the same retained object's derived owner property after removal");
    assert!(engine.capture_ui_input_source(current.alias()).is_none());let before=serde_json::to_vec(&engine.state).unwrap();
    assert!(!engine.interface_event_scoped(crate::ui_input::ScopedUiInput{source:logical.clone(),event:old.event}).unwrap());assert_eq!(serde_json::to_vec(&engine.state).unwrap(),before);
    // The neighboring object stays resident; Save15 legitimately renewed its
    // process epoch, so only the retained target/class identity is compared.
    let second=engine.widget_reference_handle(&engine.state.vars["neighbor_object"]).unwrap();assert!(actual_viewport_attached(&engine,&second));
    assert_eq!(engine.widget_backend_owner(&second).unwrap().target(),neighbor.target());
    let save=engine.capture_save15(&fixture.program,None,3,"detached completed callback".into()).unwrap();
    assert!(engine.restore_save15(&fixture.program,save).unwrap().is_none());let current=engine.widget_reference_handle(&engine.state.vars["saved_object"]).unwrap();
    assert!(!actual_viewport_attached(&engine,&current));assert!(engine.capture_ui_input_source(current.alias()).is_none());assert_eq!(engine.state.vars["hits"],Value::Int(2));
}
