//! Actual class/registry/SourceProject paths. CPU contracts, no native parity claim.
use super::*;
use crate::ui_input::{ScopedUiInput, UiInputAuthority};
fn fixture(narrative:bool,native:bool)->Fixture{
    Fixture::with_source_transform(native,false,false,false,|source|if narrative{
        source.replacen("label start\n","scene label start\n",1)
    }else{source})
}
fn snapshot(engine:&Engine<Host>)->(Vec<u8>,usize,usize,crate::engine::host_random::HostRandomState){
    (serde_json::to_vec(&engine.state).unwrap(),engine.history.len(),engine.renderer.batches.lock().unwrap().len(),engine.host_random.clone())
}
fn stale_payload(source:&crate::ui_input::UiInputSource)->ScopedUiInput{
    ScopedUiInput{source:source.clone(),event:crate::ui::UiInput{screen:source.screen.clone(),element:"x".repeat(1024),kind:rvn_ui::programmable::ScreenEventKind::Tick,value:Some(Value::Float(f32::NAN)),key:None}}
}
#[test]
fn input_residency1003_detach_reattach_revokes_old_presentation_and_keeps_actual_handle_and_neighbor(){
    for narrative in [false,true]{
        let fixture=fixture(narrative,false);let mut engine=fixture.engine();
        let first=engine.create_widget(A,"input_first",vec![Value::Str("allowed".into())]).unwrap();
        let second=engine.create_widget(A,"input_second",vec![Value::Str("allowed".into())]).unwrap();
        engine.add_widget_to_viewport(&first,false,0).unwrap();engine.add_widget_to_viewport(&second,false,1).unwrap();
        let old=engine.capture_ui_input_source(first.alias()).unwrap();
        assert!(if narrative{matches!(&old.authority,UiInputAuthority::Narrative(_))}else{matches!(&old.authority,UiInputAuthority::Instance{..})});
        let neighbor=engine.capture_ui_input_source(second.alias()).unwrap();let owner=engine.widget_backend_owner(&first).unwrap();
        engine.remove_widget_from_viewport(&first).unwrap();
        assert_eq!(engine.widget_backend_owner(&first).unwrap(),owner,"the actual logical object stays valid");
        assert!(!actual_viewport_attached(&engine,&first));assert!(engine.capture_ui_input_source(first.alias()).is_none());assert!(!engine.ui_input_source_current(&old));
        assert!(engine.ui_input_source_current(&neighbor));
        let before=snapshot(&engine);assert!(!engine.interface_event_scoped(stale_payload(&old)).unwrap());assert_eq!(snapshot(&engine),before,"reject before payload/RNG/VM/history/publication");
        engine.add_widget_to_viewport(&first,false,0).unwrap();let fresh=engine.capture_ui_input_source(first.alias()).unwrap();
        assert_eq!(engine.widget_backend_owner(&first).unwrap(),owner);assert_eq!(fresh.order,old.order);assert_eq!(fresh.host_role,old.host_role);
        assert_ne!(fresh.authority,old.authority,"the detached presentation token must not adopt its replacement");
        assert!(!engine.ui_input_source_current(&old));assert!(engine.ui_input_source_current(&fresh));assert_eq!(engine.capture_ui_input_source(second.alias()).unwrap(),neighbor);
        let before=snapshot(&engine);assert!(!engine.interface_event_scoped(stale_payload(&old)).unwrap());assert_eq!(snapshot(&engine),before);
        let event=input(&engine,first.alias(),rvn_ui::programmable::ScreenEventKind::Click);assert!(engine.interface_event_scoped(event).unwrap());assert_eq!(engine.state.vars["hits"],Value::Int(1));
        engine.destroy_widget(first).unwrap();let replacement=engine.create_widget(A,"input_first",vec![Value::Str("allowed".into())]).unwrap();engine.add_widget_to_viewport(&replacement,false,0).unwrap();
        assert_ne!(engine.widget_backend_owner(&replacement).unwrap().target(),owner.target());
        let before=snapshot(&engine);assert!(!engine.interface_event_scoped(stale_payload(&fresh)).unwrap());assert_eq!(snapshot(&engine),before);
        let mut other=fixture.engine();let foreign=other.create_widget(A,"input_first",vec![Value::Str("allowed".into())]).unwrap();other.add_widget_to_viewport(&foreign,false,0).unwrap();
        let foreign=other.capture_ui_input_source("input_first").unwrap();let before=snapshot(&engine);assert!(!engine.interface_event_scoped(stale_payload(&foreign)).unwrap());assert_eq!(snapshot(&engine),before);
    }
}
#[test]
fn input_residency1003_detach_revision_and_renderer_refusals_do_not_revoke_live_sources(){
    for narrative in [false,true]{
        let fixture=fixture(narrative,false);let mut engine=fixture.engine();let first=engine.create_widget(A,"input_first",vec![Value::Str("allowed".into())]).unwrap();engine.add_widget_to_viewport(&first,false,0).unwrap();
        let old=engine.capture_ui_input_source(first.alias()).unwrap();engine.renderer.reject.store(true,Ordering::SeqCst);
        let before=snapshot(&engine);assert!(engine.remove_widget_from_viewport(&first).is_err());assert_eq!(snapshot(&engine),before);assert!(engine.ui_input_source_current(&old));
        engine.renderer.reject.store(false,Ordering::SeqCst);
        let registry=if let Some(execution)=&mut engine.state.scene_execution{&mut execution.resources}else{engine.state.ui.instance_registry.as_mut().unwrap()};registry.next_revision=u64::MAX;
        let old=engine.capture_ui_input_source(first.alias()).unwrap();let before=snapshot(&engine);
        assert!(engine.remove_widget_from_viewport(&first).unwrap_err().to_string().contains("revision exhausted"));assert_eq!(snapshot(&engine),before);assert!(engine.ui_input_source_current(&old));
    }
}
#[test]
fn input_residency1003_pending_restore_is_not_external_input_but_keeps_exact_logical_continuation(){
    for narrative in [false,true]{
        let fixture=fixture(narrative,false);let mut engine=fixture.engine();let first=engine.create_widget(A,"input_pending",vec![Value::Str("allowed".into())]).unwrap();engine.add_widget_to_viewport(&first,false,0).unwrap();
        let old=engine.capture_ui_input_source(first.alias()).unwrap();let save=engine.capture_save15(&fixture.program,None,1,"input pending".into()).unwrap();
        assert!(engine.restore_save15(&fixture.program,save).unwrap().is_none());assert!(engine.has_pending_widget_materialization());
        let logical=crate::ui_input::capture_logical(&engine.state,engine.instance_id(),engine.runtime_epoch(),engine.interface_epoch(),"input_pending").unwrap();
        assert!(crate::ui_input::current_logical(&engine.state,engine.instance_id(),engine.runtime_epoch(),engine.interface_epoch(),&logical));
        assert!(!engine.ui_input_source_current(&logical));assert!(engine.capture_ui_input_source("input_pending").is_none());
        let before=snapshot(&engine);assert!(!engine.interface_event_scoped(stale_payload(&logical)).unwrap());assert!(!engine.interface_event_scoped(stale_payload(&old)).unwrap());assert_eq!(snapshot(&engine),before);
        let mut session=engine.debug_begin(DebugOperation::Input(stale_payload(&logical)),vec![]).unwrap();
        assert!(session.snapshot().frames.is_empty(),"pending input must not enter the actual callback VM");
        let mut completed=false;for _ in 0..8{let(progress,outcome)=engine.debug_step(&mut session,crate::editor_debugger::StepKind::Continue).unwrap();if progress==DebugProgress::Complete{assert!(matches!(outcome,Some(DebugOutcome::Input(false))));completed=true;break;}}assert!(completed);assert_eq!(snapshot(&engine),before);
        let handle=engine.widget_instance_handle("input_pending").unwrap();engine.remove_widget_from_viewport(&handle).unwrap();
        assert!(crate::ui_input::capture_logical(&engine.state,engine.instance_id(),engine.runtime_epoch(),engine.interface_epoch(),"input_pending").is_none());
        assert!(!crate::ui_input::current_logical(&engine.state,engine.instance_id(),engine.runtime_epoch(),engine.interface_epoch(),&logical));
    }
}
#[test]
fn input_residency1003_plain_legacy_compatibility_keeps_live_source_and_rejects_replaced_or_foreign(){
    let text=r#"handler hit(event){set hits=hits+1} screen legacy(){return component("go","button",{"events":{"click":"hit"}},[])} init{set hits=0} label start ui.open("legacy",[],false,0) "Before" ui.close("legacy") ui.open("legacy",[],false,0) "After""#;
    let new=||{let mut engine=Engine::new(rvn_parser::parse(text).unwrap(),Host::default(),16).unwrap();engine.step_until_interaction().unwrap();engine};
    let mut engine=new();assert!(engine.state.ui.instance_registry.is_none());let source=engine.capture_ui_input_source("legacy").unwrap();assert!(matches!(&source.authority,UiInputAuthority::Legacy{..}));
    let event=input(&engine,"legacy",rvn_ui::programmable::ScreenEventKind::Click);assert!(engine.interface_event_scoped(event).unwrap());assert_eq!(engine.capture_ui_input_source("legacy").unwrap(),source);
    let foreign=new().capture_ui_input_source("legacy").unwrap();let before=snapshot(&engine);assert!(!engine.interface_event_scoped(stale_payload(&foreign)).unwrap());assert_eq!(snapshot(&engine),before);
    engine.advance_dialogue().unwrap();engine.step_until_interaction().unwrap();let before=snapshot(&engine);assert!(!engine.interface_event_scoped(stale_payload(&source)).unwrap());assert_eq!(snapshot(&engine),before);assert!(engine.capture_ui_input_source("legacy").is_some());assert!(engine.state.ui.instance_registry.is_none());
}
