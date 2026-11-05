//! Real class/native Screen admission and Save15 writer/restore. The inherited
//! renderer is explicitly logical CPU-only: pending materialization is never
//! turned into physical readiness, and no native/pixel acceptance is claimed.
use super::*;
use rvn_ui::programmable::{BoxSlotSize,ParentSlot,ScreenEventKind};
use crate::save::{SaveManager,consumer_checkpoint::Save15};
const ALIAS:&str="saved_slot1031";
fn fixture(absent_weight:bool)->Fixture{
    let fixture=Fixture::with_source_transform_at(false,false,false,false,|root,source|{
        let source=source.replace("init{set hits=0}","init{set hits=0 set sizing_rule=\"auto\" set slot_weight=5}");
        let source=source.replace("set hits=hits+1}","set hits=hits+1 set sizing_rule=\"fill\"}");
        let size=if absent_weight{r#"{"rule":sizing_rule}"#}else{r#"{"rule":sizing_rule,"weight":slot_weight}"#};
        // Native host admission requires real v2/v3 executable control flow.
        // A lone Screen Return emits v1 and intentionally stays stateless in
        // Core; opting the Graph into native preparation alone is insufficient.
        // This authored hidden-state branch is compiled normally by Graph. Auto
        // and Fill still reach the identical component/slot expression below.
        let screen=r#"screen panel(method){if sizing_rule=="hidden"{return []} return component("root","column",{"layout_contract":"widget_v1"},[component("go","button",{"events":{"click":method},"parent_slot":{"kind":"vertical_box","size":$SIZE}},[])])}"#.replace("$SIZE",size);
        let old=r#"screen panel(method){return component("go","button",{"events":{"click":method}},[])}"#;
        assert_eq!(source.matches(old).count(),1);let source=source.replace(old,&screen);
        let path=root.join("main.rvn");fs::write(&path,&source).unwrap();
        let mut project=rvn_graph::SourceProject::open_at(&path,&source,&[]).unwrap();let mut graphs=project.graphs().to_vec();
        // Actual authored roots, not a neutral Init or hand-built frozen AST.
        let mut prepared=0;
        for graph in &mut graphs{
            if matches!(&graph.kind,rvn_graph::GraphKind::Screen{name}if name=="panel")
                ||matches!(&graph.kind,rvn_graph::GraphKind::Handler{name}if name=="allowed"){
                graph.enable_native_frame_preparation().unwrap();prepared+=1;
            }
        }
        assert_eq!(prepared,2);project.apply_visual(&source,&graphs).unwrap();project.source().to_owned()
    });
    let binding=crate::consumer_schedule::ConsumerSourceBinding::from_program(&fixture.program).unwrap();
    assert!(binding.definition(&crate::consumer_schedule::HostDefinitionKind::Screen,"panel").unwrap().native_plans,
        "the real final class program must contain the Screen native lifecycle contract before widget creation");
    fixture
}
fn attached(f:&Fixture)->(Engine<Host>,WidgetInstanceHandle){
    let mut engine=f.engine();let handle=engine.create_widget(A,ALIAS,vec![Value::Str("allowed".into())]).unwrap();
    engine.add_widget_to_viewport(&handle,false,0).unwrap();assert_eq!(actual_size(&engine,&handle),BoxSlotSize::Auto{weight:5.0});(engine,handle)
}
/// Read the genuine admitted native projection, never construct or patch one.
fn actual_size(engine:&Engine<Host>,handle:&WidgetInstanceHandle)->BoxSlotSize{
    let owner=engine.widget_backend_owner(handle).unwrap();let resident=crate::ui_residency::resident_interface(&engine.state,owner.target()).unwrap();
    let projection=resident.instance.consumer_projection.as_ref().expect("actual compiled native Screen projection, not re-evaluation fallback");
    assert_eq!(projection.owner.target,*owner.target());assert_eq!(projection.owner.definition,"panel");
    assert_eq!(projection.program,engine.consumer_program.identity().unwrap());
    let root=projection.root.as_ref().expect("actual rooted Screen");let child=root.find("go").unwrap();
    match child.parent_slot.as_ref().unwrap(){ParentSlot::VerticalBox{size,..}=>*size,_=>panic!("actual VerticalBox child slot")}
}
fn projection<'a>(wire:&'a serde_json::Value)->&'a serde_json::Value{
    let rows=wire["story"]["projections"].as_array().expect("Save15 exports native projections separately from the cleared portable base");
    let selected=rows.iter().filter(|p|p["owner"]["target"]["alias"]==ALIAS).collect::<Vec<_>>();assert_eq!(selected.len(),1);
    assert_eq!(selected[0]["owner"]["kind"],"host");assert_eq!(selected[0]["owner"]["definition"],"panel");selected[0]
}
fn serialized_size(save:&Save15)->serde_json::Value{
    let wire=serde_json::to_value(save).unwrap();let p=projection(&wire);
    let child=p["root"]["children"].as_array().unwrap().iter().find(|node|node["id"]=="go").unwrap();child["parent_slot"]["size"].clone()
}
fn written(f:&Fixture,engine:&Engine<Host>,label:&str)->Save15{
    let save=engine.capture_save15(&f.program,None,1,label.into()).unwrap();assert_eq!(save.format_version,15,"no implicit format bump");
    let manager=SaveManager::new(f.root.join("retained-slot-saves1031"),4).unwrap();manager.save15_slot(&save).unwrap();
    let loaded=manager.load15_slot(1).unwrap();
    // Last-dialogue variables use a HashMap. A round trip preserves their
    // values, but the rebuilt map can serialize keys in a different order.
    assert_eq!(serde_json::to_value(&loaded).unwrap(),serde_json::to_value(&save).unwrap());
    loaded
}
fn state_image(engine:&Engine<Host>)->(Vec<u8>,usize,usize,u64,u64,crate::engine::host_random::HostRandomState){
    (serde_json::to_vec(&engine.state).unwrap(),engine.history.len(),engine.renderer.batches.lock().unwrap().len(),engine.runtime_epoch(),engine.interface_epoch(),engine.host_random.clone())
}
#[test]
fn retained_weight_save15_1031_attached_payload_contains_auto_five_and_restore_stays_pending_without_fake_input(){
    let f=fixture(false);let(mut engine,old_handle)=attached(&f);let old_source=engine.capture_ui_input_source(ALIAS).unwrap();
    let save=written(&f,&engine,"attached native Auto5");assert_eq!(serialized_size(&save),serde_json::json!({"rule":"auto","weight":5.0}));
    assert!(engine.restore_save15(&f.program,save).unwrap().is_none());assert!(engine.has_pending_widget_materialization());
    assert!(engine.widget_backend_owner(&old_handle).is_err());assert!(!engine.ui_input_source_current(&old_source));assert!(engine.capture_ui_input_source(ALIAS).is_none());
    let fresh=engine.widget_instance_handle(ALIAS).unwrap();assert_ne!(fresh,old_handle);assert_eq!(actual_size(&engine,&fresh),BoxSlotSize::Auto{weight:5.0});
    let before=state_image(&engine);
    let logical=crate::ui_input::capture_logical(&engine.state,engine.instance_id(),engine.runtime_epoch(),engine.interface_epoch(),ALIAS).unwrap();
    assert!(!engine.ui_input_source_current(&logical));
    for source in [old_source,logical]{
        assert!(!engine.interface_event_scoped(crate::ui_input::ScopedUiInput{source,event:crate::ui::UiInput{screen:ALIAS.into(),element:"go".into(),kind:ScreenEventKind::Click,value:None,key:None}}).unwrap());
        assert_eq!(state_image(&engine),before);
    }
    // This inherited logical renderer cannot prepare an actual new wrapper.
    // Refusal is required; assigning a Ready/object bit would invalidate test.
    assert!(engine.next_widget_materialization().is_err());assert_eq!(state_image(&engine),before);
}
#[test]
fn retained_weight_save15_1031_detached_writer_cold_restore_and_public_reattach_keep_saved_weight_then_real_callback_fills(){
    let f=fixture(false);let(mut engine,handle)=attached(&f);let old_source=engine.capture_ui_input_source(ALIAS).unwrap();let old_value=engine.widget_reference_value(&handle).unwrap();
    engine.remove_widget_from_viewport(&handle).unwrap();assert_eq!(actual_size(&engine,&handle),BoxSlotSize::Auto{weight:5.0});
    let save=written(&f,&engine,"detached native Auto5");assert_eq!(serialized_size(&save),serde_json::json!({"rule":"auto","weight":5.0}));
    // The ordinary loader must retire a currently attached old life before
    // publishing the genuinely saved detached target, not mutate live fields.
    engine.add_widget_to_viewport(&handle,false,0).unwrap();let retirement_source=engine.capture_ui_input_source(ALIAS).unwrap();
    assert!(engine.restore_save15(&f.program,Save15::decode(&save.encode().unwrap()).unwrap()).unwrap().is_none());
    assert!(engine.widget_backend_owner(&handle).is_err());assert!(!engine.ui_input_source_current(&retirement_source));assert!(!engine.has_pending_widget_materialization());
    let restored=engine.widget_instance_handle(ALIAS).unwrap();assert_eq!(actual_size(&engine,&restored),BoxSlotSize::Auto{weight:5.0});
    let startup=Engine::prepare_widget_class_startup(f.program.clone(),f.classes.clone(),Host::default(),16,Default::default(),Default::default()).unwrap();
    let(mut cold,session)=startup.restore_completed_save15(save).unwrap();assert!(session.is_none());assert!(!cold.has_pending_widget_materialization());
    assert!(cold.widget_reference_handle(&old_value).is_err());assert!(!cold.ui_input_source_current(&old_source));assert!(cold.capture_ui_input_source(ALIAS).is_none());
    let fresh=cold.widget_instance_handle(ALIAS).unwrap();assert_eq!(actual_size(&cold,&fresh),BoxSlotSize::Auto{weight:5.0});
    cold.add_widget_to_viewport(&fresh,false,0).unwrap();let current=cold.capture_ui_input_source(ALIAS).unwrap();assert_ne!(current,old_source);
    assert!(cold.interface_event_scoped(input(&cold,ALIAS,ScreenEventKind::Click)).unwrap());
    assert_eq!(cold.state.vars["hits"],Value::Int(1));assert_eq!(actual_size(&cold,&fresh),BoxSlotSize::Fill{weight:5.0});
}
#[test]
fn retained_weight_save15_1031_genuine_old_auto_payload_omits_weight_and_restores_default_one(){
    let f=fixture(true);let mut engine=f.engine();let handle=engine.create_widget(A,ALIAS,vec![Value::Str("allowed".into())]).unwrap();
    engine.add_widget_to_viewport(&handle,false,0).unwrap();assert_eq!(actual_size(&engine,&handle),BoxSlotSize::Auto{weight:1.0});
    let save=written(&f,&engine,"old Auto wire");assert_eq!(serialized_size(&save),serde_json::json!({"rule":"auto"}));
    assert!(engine.restore_save15(&f.program,save).unwrap().is_none());let fresh=engine.widget_instance_handle(ALIAS).unwrap();
    assert_eq!(actual_size(&engine,&fresh),BoxSlotSize::Auto{weight:1.0});assert!(engine.has_pending_widget_materialization());assert!(engine.capture_ui_input_source(ALIAS).is_none());
}
fn corrupted_weight(save:&Save15,value:serde_json::Value)->serde_json::Value{
    let mut wire=serde_json::to_value(save).unwrap();let rows=wire["story"]["projections"].as_array_mut().unwrap();
    let p=rows.iter_mut().find(|p|p["owner"]["target"]["alias"]==ALIAS).unwrap();
    let child=p["root"]["children"].as_array_mut().unwrap().iter_mut().find(|node|node["id"]=="go").unwrap();
    assert_eq!(child["parent_slot"]["size"],serde_json::json!({"rule":"auto","weight":5.0}));child["parent_slot"]["size"]["weight"]=value;wire
}
#[test]
fn retained_weight_save15_1031_corrupt_dormant_weight_refuses_before_live_state_epoch_history_or_backend_publication(){
    let f=fixture(false);let(mut engine,handle)=attached(&f);let source=engine.capture_ui_input_source(ALIAS).unwrap();let original=engine.debug_state_source().unwrap();
    let save=written(&f,&engine,"corruption refusal");let before=state_image(&engine);
    for bad in [serde_json::json!(-1),serde_json::json!(16385),serde_json::json!("NaN"),serde_json::Value::Null]{
        let wire=corrupted_weight(&save,bad);let result=Save15::decode(&wire.to_string()).and_then(|decoded|engine.restore_save15(&f.program,decoded).map(|_|()));
        assert!(result.is_err());assert_eq!(state_image(&engine),before);assert!(engine.accepts_debug_state_source(&original));assert!(engine.ui_input_source_current(&source));assert_eq!(actual_size(&engine,&handle),BoxSlotSize::Auto{weight:5.0});
    }
    let wire=corrupted_weight(&save,serde_json::json!("__rvn_nonfinite_weight1031__")).to_string().replace("\"__rvn_nonfinite_weight1031__\"","1e999");
    assert!(Save15::decode(&wire).is_err());assert_eq!(state_image(&engine),before);assert!(engine.accepts_debug_state_source(&original));
}
