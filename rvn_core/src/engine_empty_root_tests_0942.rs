use crate::{
    random::RandomState, renderer::SceneRenderBatch, types::SpriteState, Engine, Renderer,
};
use rvn_parser::{Hotspot, ParsedProgram, Position, Transition, Value};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
#[derive(Clone, Default)]
struct Host {
    batches: Arc<Mutex<Vec<SceneRenderBatch>>>,
    reject: Arc<AtomicBool>,
}
impl Renderer for Host {
    fn validate_scene_batch(&self, _: &SceneRenderBatch) -> Result<(), String> {
        if self.reject.load(Ordering::SeqCst) {
            Err("private renderer rejection".into())
        } else {
            Ok(())
        }
    }
    fn publish_scene_batch(&mut self, b: SceneRenderBatch) {
        self.batches.lock().unwrap().push(b);
    }
    fn supports_programmable_ui(&self) -> bool {
        true
    }
    fn supports_custom_canvas(&self) -> bool {
        true
    }
    fn supports_video(&self) -> bool {
        true
    }
    fn update_videos(&mut self, _: &[crate::video::VideoView]) -> Result<(), String> {
        Ok(())
    }
    fn update_interfaces(&mut self, _: &[rvn_ui::programmable::ScreenView]) -> Result<(), String> {
        Ok(())
    }
    fn set_background(&mut self, _: &str, _: &Transition) {}
    fn show_sprite(
        &mut self,
        _: &str,
        _: Option<&str>,
        _: &Position,
        _: &Transition,
        _: Option<&SpriteState>,
    ) {
    }
    fn hide_sprite(&mut self, _: &str, _: &Transition, _: &SpriteState) {}
    fn move_sprite(&mut self, _: &str, _: &Position, _: &Transition, _: &SpriteState) {}
    fn show_dialogue(&mut self, _: Option<&str>, _: &str) {}
    fn show_choice(&mut self, _: &[String]) -> usize {
        0
    }
    fn show_imagemap(&mut self, _: &str, _: Option<&str>, _: &[Hotspot]) -> usize {
        0
    }
    fn music_play(&mut self, _: &str, _: &Transition, _: Option<&str>) {}
    fn music_stop(&mut self, _: &Transition) {}
    fn music_set_volume(&mut self, _: f32) {}
    fn sfx_play(&mut self, _: &str, _: &Transition) {}
    fn sfx_stop(&mut self, _: &str, _: &Transition) {}
}
const SOURCE:&str=r#"
init {set visible=false}
function sample(){return random(1,100)}
handler reveal(event){set visible=true}
handler conceal(event){set visible=false}
screen controls(){return component("controller","column",{},[component("reveal","button",{"events":{"click":"reveal"}},[]),component("conceal","button",{"events":{"click":"conceal"}},[])])}
function describe_panel(){if visible {return component("actual","text",{"text":"present"},[])} return []}
screen panel(){flow.consume_v2("screen:panel","gather"){capture "roll"=sample() return describe_panel()}}
scene label P
ui.open("panel",[],true,1)
ui.open("controls",[],false,2)
"waiting"
"#;
fn program()->ParsedProgram{let mut p=rvn_parser::parse_program(SOURCE).unwrap();let path=Some(std::path::PathBuf::from("C:/private/empty-root-0942.rvn"));p.sources[0].path=path.clone();for origin in &mut p.origins{origin.source=path.clone();}p}
fn engine()->Engine<Host>{Engine::from_program_with_debugger(program(),Host::default(),16).unwrap()}
fn at_wait()->Engine<Host>{let mut e=engine();e.step_until_interaction().unwrap();e}
fn panel(e:&Engine<Host>)->&crate::ui::ScreenInstance{e.state.ui.screens.iter().find(|instance|instance.name=="panel").unwrap()}
fn click(e:&mut Engine<Host>,id:&str)->Result<bool,crate::RuntimeError>{
    let source=e.capture_ui_input_source("controls").unwrap();
    e.interface_event_scoped(crate::ui_input::ScopedUiInput{source,event:crate::ui::UiInput{screen:"controls".into(),element:id.into(),kind:rvn_ui::programmable::ScreenEventKind::Click,value:None,key:None}})
}
#[test]
fn native_none_root_none_keeps_the_actual_target_and_projection_owner(){
    let mut e=at_wait();let first=panel(&e).clone();let owner=first.consumer_projection.as_ref().unwrap().owner.clone();
    assert!(first.consumer_projection.as_ref().unwrap().root.is_none());assert!(e.interface_is_modal());assert_eq!(e.interface_views().unwrap().len(),1);
    let before=serde_json::to_vec(&e.state).unwrap();for _ in 0..3{e.interface_views().unwrap();}assert_eq!(serde_json::to_vec(&e.state).unwrap(),before);
    assert!(click(&mut e,"reveal").unwrap());let rooted=panel(&e);assert_eq!(rooted.order,first.order);assert_eq!(rooted.consumer_projection.as_ref().unwrap().owner,owner);assert_eq!(rooted.consumer_projection.as_ref().unwrap().root.as_ref().unwrap().id,"actual");
    assert!(click(&mut e,"conceal").unwrap());let empty=panel(&e);assert_eq!(empty.order,first.order);assert_eq!(empty.consumer_projection.as_ref().unwrap().owner,owner);assert!(empty.consumer_projection.as_ref().unwrap().root.is_none());assert!(e.interface_is_modal());
    assert!(e.interface_views().unwrap().iter().all(|view|view.name!="panel"));
}
#[test]
fn native_empty_projection_save15_round_trip_keeps_owner_rng_and_refuses_forged_owner(){
    let e=at_wait();let instance=panel(&e).clone();let bytes=e.capture_save15(&program(),None,1,"empty child root".into()).unwrap().encode().unwrap();
    let json:serde_json::Value=serde_json::from_str(&bytes).unwrap();assert!(json["story"]["projections"].as_array().unwrap().iter().any(|projection|projection["root"].is_null()));
    let mut cold=engine();cold.restore_save15(&program(),crate::save::consumer_checkpoint::Save15::decode(&bytes).unwrap()).unwrap();assert_eq!(panel(&cold),&instance);
    let before=serde_json::to_vec(&cold.state).unwrap();for _ in 0..3{cold.interface_views().unwrap();}assert_eq!(serde_json::to_vec(&cold.state).unwrap(),before);
    let mut forged=json;forged["story"]["projections"][0]["owner"]["target"]["incarnation"]=serde_json::json!(999999);
    let epoch=cold.runtime_epoch();assert!(cold.restore_save15(&program(),crate::save::consumer_checkpoint::Save15::decode(&forged.to_string()).unwrap()).is_err());assert_eq!(serde_json::to_vec(&cold.state).unwrap(),before);assert_eq!(cold.runtime_epoch(),epoch);
}
#[test]
fn removing_native_root_is_atomic_under_renderer_refusal_and_rooted_save_wire_stays_an_object(){
    let mut e=at_wait();assert!(click(&mut e,"reveal").unwrap());
    let bytes=e.capture_save15(&program(),None,2,"rooted compatibility".into()).unwrap().encode().unwrap();let json:serde_json::Value=serde_json::from_str(&bytes).unwrap();assert!(json["story"]["projections"][0]["root"].is_object());
    let mut cold=engine();cold.restore_save15(&program(),crate::save::consumer_checkpoint::Save15::decode(&bytes).unwrap()).unwrap();assert!(panel(&cold).consumer_projection.as_ref().unwrap().root.is_some());
    let before=serde_json::to_vec(&e.state).unwrap();let publications=e.renderer.batches.lock().unwrap().len();e.renderer.reject.store(true,Ordering::SeqCst);
    assert!(click(&mut e,"conceal").is_err());assert_eq!(serde_json::to_vec(&e.state).unwrap(),before);assert_eq!(e.renderer.batches.lock().unwrap().len(),publications);
    e.renderer.reject.store(false,Ordering::SeqCst);assert!(click(&mut e,"conceal").unwrap());assert!(panel(&e).consumer_projection.as_ref().unwrap().root.is_none());
}

fn owner_program()->ParsedProgram{
    let source=SOURCE.replace("init {set visible=false}","init {set visible=false set owner_hits=0}")
        .replace("screen panel(){",r#"handler owner_construct(event){flow.consume_v2("handler:owner_construct","gather"){capture "roll"=sample() set owner_hits=owner_hits+1}}
screen panel(){"#)
        .replace("return describe_panel()",r#"return {"host":{"focusable":true,"minimum_desired_size":[24,16],"events":{"construct":"owner_construct"}},"root":describe_panel()}"#);
    let mut p=rvn_parser::parse_program(&source).unwrap();let path=Some(std::path::PathBuf::from("C:/private/owner-host-0942.rvn"));p.sources[0].path=path.clone();for origin in &mut p.origins{origin.source=path.clone();}p
}
#[test]
fn actual_native_empty_owner_properties_survive_portable_save_without_gather_replay(){
    let mut e=Engine::from_program_with_debugger(owner_program(),Host::default(),16).unwrap();e.step_until_interaction().unwrap();
    let instance=panel(&e).clone();assert!(instance.owner_properties.focusable);assert_eq!(instance.owner_properties.minimum_desired_size,[24.0,16.0]);assert_eq!(instance.consumer_projection.as_ref().unwrap().host,instance.owner_properties);
    let hosts=e.interface_hosts().unwrap();let host=hosts.iter().find(|host|host.name=="panel").unwrap();assert!(host.child.is_none());assert!(host.properties.focusable);
    let before=serde_json::to_vec(&e.state).unwrap();e.interface_hosts().unwrap();assert_eq!(serde_json::to_vec(&e.state).unwrap(),before);
    let bytes=e.capture_save15(&owner_program(),None,1,"owner properties".into()).unwrap().encode().unwrap();let mut cold=Engine::from_program_with_debugger(owner_program(),Host::default(),16).unwrap();cold.restore_save15(&owner_program(),crate::save::consumer_checkpoint::Save15::decode(&bytes).unwrap()).unwrap();assert_eq!(panel(&cold),&instance);
}
#[test]
fn native_owner_handler_uses_real_retained_frame_without_a_component_and_renderer_refusal_is_atomic(){
    let mut e=Engine::from_program_with_debugger(owner_program(),Host::default(),16).unwrap();e.step_until_interaction().unwrap();assert!(panel(&e).consumer_projection.as_ref().unwrap().root.is_none());
    let before=serde_json::to_vec(&e.state).unwrap();let publications=e.renderer.batches.lock().unwrap().len();let mut next=e.state.clone();
    let owner=Engine::<Host>::live_resource(&next,crate::scene_resources::ResourceTaskKind::Interface,"panel").unwrap().key.owner;
    let mut budget=crate::ui::CanvasBudget::default();let commands=e.owner_handler_owned_budgeted(&mut next,"panel",rvn_ui::programmable::HostEventKind::Construct,Value::Dict(Default::default()),&owner,None,None,&mut budget).unwrap();
    e.apply_callback_commands_budgeted(&mut next,commands,&mut budget).unwrap();assert_eq!(next.vars["owner_hits"],Value::Int(1));
    let frame=&next.ui.screens.iter().find(|instance|instance.name=="panel").unwrap().consumer_frames["handler:owner_construct"];assert_eq!(frame.owner.target,panel(&e).consumer_projection.as_ref().unwrap().owner.target);
    e.renderer.reject.store(true,Ordering::SeqCst);assert!(e.commit_ui_state(next.clone()).is_err());assert_eq!(serde_json::to_vec(&e.state).unwrap(),before);assert_eq!(e.renderer.batches.lock().unwrap().len(),publications);
    e.renderer.reject.store(false,Ordering::SeqCst);e.commit_ui_state(next).unwrap();assert_eq!(e.state.vars["owner_hits"],Value::Int(1));assert!(panel(&e).consumer_frames.contains_key("handler:owner_construct"));
}
#[test]
fn empty_modal_owner_blocks_lower_child_input_without_destroying_the_lower_projection(){
    let source=SOURCE.replace("ui.open(\"panel\",[],true,1)","ui.open(\"panel\",[],true,3)");let mut e=Engine::from_program_with_debugger(rvn_parser::parse_program(&source).unwrap(),Host::default(),16).unwrap();e.step_until_interaction().unwrap();
    assert_eq!(e.interface_views().unwrap().len(),1);assert!(e.interface_views().unwrap().iter().any(|view|view.name=="controls"));let before=serde_json::to_vec(&e.state).unwrap();
    assert!(click(&mut e,"reveal").is_err());assert_eq!(serde_json::to_vec(&e.state).unwrap(),before);
}

#[test]
fn existing_vm_focus_operand_keeps_legacy_string_and_explicit_owner_intent_distinct(){
    let source=r#"screen empty(){return []}
screen child(){return component("actual","button",{},[])}
scene label P
ui.open("empty",[],false,1)
ui.open("child",[],false,2)
ui.focus("child","actual")
ui.focus("empty",{"target":"owner","user":0})
"waiting"
"#;
    let mut e=Engine::from_program(rvn_parser::parse_program(source).unwrap(),Host::default(),16).unwrap();e.step_until_interaction().unwrap();
    let empty=e.state.ui.screens.iter().find(|instance|instance.name=="empty").unwrap();let child=e.state.ui.screens.iter().find(|instance|instance.name=="child").unwrap();assert_eq!(empty.focus,None);let request=empty.owner_focus_request.as_ref().unwrap();assert_eq!(request.target,rvn_ui::programmable::HostFocusTarget::Owner);assert_eq!(request.user,0);assert_eq!(request.serial,1);assert_eq!(child.focus.as_deref(),Some("actual"));assert!(child.owner_focus_request.is_none());
    assert!(e.interface_views().unwrap().iter().all(|view|view.name!="empty"));
}

#[test]
fn native_owner_reply_returns_data_without_a_second_vm_or_component_and_retains_actual_source(){
    let source=SOURCE.replace("init {set visible=false}","init {set visible=false set owner_hits=0}")
      .replace("screen panel(){",r#"handler owner_reply(event){set owner_hits=owner_hits+1 flow.consume_v2("handler:owner_reply","gather"){capture "roll"=sample() return {"handled":true}}}
screen panel(){"#)
      .replace("return describe_panel()",r#"return {"host":{"events":{"focus_received":"owner_reply"}},"root":describe_panel()}"#);
    let mut e=Engine::from_program_with_debugger(rvn_parser::parse_program(&source).unwrap(),Host::default(),16).unwrap();e.step_until_interaction().unwrap();let mut next=e.state.clone();let owner=Engine::<Host>::live_resource(&next,crate::scene_resources::ResourceTaskKind::Interface,"panel").unwrap().key.owner;
    let(commands,reply)=e.owner_handler_result_owned_budgeted(&mut next,"panel",rvn_ui::programmable::HostEventKind::FocusReceived,Value::Dict(Default::default()),&owner,None,None,&mut crate::ui::CanvasBudget::default()).unwrap();assert_eq!(crate::ui::value_to_json(&reply.unwrap()).unwrap(),serde_json::json!({"handled":true}));assert_eq!(next.vars["owner_hits"],Value::Int(1));assert_eq!(e.state.vars["owner_hits"],Value::Int(0));assert!(commands.commands.is_empty());let frame=&next.ui.screens.iter().find(|instance|instance.name=="panel").unwrap().consumer_frames["handler:owner_reply"];assert_eq!(frame.owner.target,panel(&e).consumer_projection.as_ref().unwrap().owner.target);
}
