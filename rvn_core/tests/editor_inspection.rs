//! Readonly inspection contracts: actual Core visits, no GPU or native claim.
use rvn_core::{Engine,Renderer,SpriteState,Interaction};
use rvn_core::renderer::SceneRenderBatch;
use rvn_parser::{Value,Transition,Position,Hotspot};
#[derive(Default)]
struct Host {
    batches: Vec<SceneRenderBatch>,
    reject: bool,
}
impl Renderer for Host {
    fn supports_video(&self) -> bool {
        true
    }
    fn supports_programmable_ui(&self) -> bool {
        true
    }
    fn supports_composable_motion(&self) -> bool {
        true
    }
    fn supports_custom_canvas(&self) -> bool {
        true
    }
    fn validate_scene_batch(&self, _: &SceneRenderBatch) -> Result<(), String> {
        if self.reject {
            Err("Rejected exact resident projection".into())
        } else {
            Ok(())
        }
    }
    fn publish_scene_batch(&mut self, batch: SceneRenderBatch) {
        self.batches.push(batch);
    }
    fn update_interfaces(&mut self, _: &[rvn_ui::programmable::ScreenView]) -> Result<(), String> {
        if self.reject { Err("Rejected exact resident projection".into()) } else { Ok(()) }
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


const SOURCE: &str = r#"
handler hit(event){set hits=hits+1}
handler renew(event){ui.close("panel") ui.open("panel",["replacement"],false,1)}
screen panel(marker){return component("root","column",{},[
 component("act","button",{"text":marker,"events":{"click":"hit"}},[]),
 component("renew","button",{"events":{"click":"renew"}},[]),
 component("name","input",{"binding":"name"},[])])}
screen game(){return component("act","button",{"events":{"click":"hit"}},[])}
screen host(ctx){return component("act","button",{"events":{"click":"hit"}},[])}
init {set hits=0 set name="" ui.open("game",[],false,0)}
scene label P
ui.open("panel",["parent"],false,1)
"parent"
call C
"returned"
jump done
scene label C
ui.open("panel",["child"],false,1)
"child"
return
label done
"#;
fn game()->Engine<Host>{let mut g=Engine::from_program(rvn_parser::parse_program(SOURCE).unwrap(),Host::default(),32).unwrap();assert_eq!(dialogue(&mut g),"parent");g}
fn dialogue(g:&mut Engine<Host>)->String{match g.step_until_interaction().unwrap().unwrap(){Interaction::Dialogue{text,..}=>text,x=>panic!("Expected dialogue: {x:?}")}}
fn advance(g:&mut Engine<Host>)->String{g.advance_dialogue().unwrap();dialogue(g)}

use rvn_core::editor_inspection::{self as inspect,FrameIdentity,Inspection,InstanceKey,MAX_INSPECTION_BYTES};
fn identity(game:&Engine<Host>,sequence:u64)->FrameIdentity{FrameIdentity{instance:game.instance_id(),epoch:game.runtime_epoch(),sequence}}
fn state_digest(game:&Engine<Host>)->(serde_json::Value,usize,usize,u64,u64){(serde_json::to_value(&game.state).unwrap(),game.history.len(),game.renderer.batches.len(),game.runtime_epoch(),game.instance_id())}
fn observe(game:&Engine<Host>,sequence:u64)->Inspection{
    let before=state_digest(game);let observed=inspect::capture(&game.state,identity(game,0));
    assert!(observed.unavailable.is_none(),"{:?}",observed.unavailable);
    let bytes=observed.encode_for(identity(game,sequence)).unwrap();let decoded=Inspection::decode(&bytes).unwrap();
    assert_eq!(state_digest(game),before);decoded
}
#[test]
fn runtime_inspection_real_call_return_keeps_parked_visits_and_exact_resident_interfaces(){
    let mut game=game();let parent=game.state.scene_execution.as_ref().unwrap().owner().unwrap();
    let original=game.capture_ui_input_source("panel").unwrap();
    let first=observe(&game,1);let original_interface=first.rows.iter().find(|row|matches!(&row.key,InstanceKey::Interface{screen,target:Some(_),..}if screen=="panel")).unwrap().key.clone();
    assert_eq!(advance(&mut game),"child");
    let child=game.state.scene_execution.as_ref().unwrap().owner().unwrap();assert_ne!(parent,child);
    let parked=observe(&game,2);
    assert!(parked.rows.iter().any(|row|row.key==InstanceKey::Activation{id:parent}&&row.parked));
    assert!(parked.rows.iter().any(|row|row.key==InstanceKey::Activation{id:child}&&!row.parked));
    assert!(parked.rows.iter().any(|row|row.key==original_interface&&row.parked));
    let child_interface=parked.rows.iter().find(|row|matches!(&row.key,InstanceKey::Interface{screen,..}if screen=="panel")&&!row.parked).unwrap().key.clone();
    assert_ne!(child_interface,original_interface);
    assert_eq!(advance(&mut game),"returned");let returned=observe(&game,3);
    assert!(returned.rows.iter().any(|row|row.key==original_interface&&!row.parked));
    assert!(!returned.rows.iter().any(|row|row.key==child_interface));
    assert!(!game.ui_input_source_current(&original));
    assert_eq!(returned.position.pc,game.state.scene_execution.as_ref().unwrap().live_position.pc);
}
#[test]
fn runtime_inspection_independent_engines_and_completed_images_never_reuse_selection(){
    let first=game();let second=game();assert_eq!(first.runtime_epoch(),second.runtime_epoch());assert_ne!(first.instance_id(),second.instance_id());
    let a=observe(&first,1);let b=observe(&second,1);assert_eq!(a.position,b.position);
    assert!(a.selected(&a.frame,&InstanceKey::Game).is_some());assert!(b.selected(&a.frame,&InstanceKey::Game).is_none());
    assert!(a.encode_for(b.frame.clone()).is_err());
    let mut old=a.frame.clone();old.sequence+=1;assert!(a.selected(&old,&InstanceKey::Game).is_none());
    let save=rvn_core::save::SaveData::try_from_state(&first.state,1,"QA".into(),"main.rvn".into()).unwrap();
    let json=serde_json::to_value(save).unwrap();assert!(json.get("inspection").is_none());assert!(json.get("frame").is_none());
}
#[test]
fn runtime_inspection_streaming_budget_refuses_large_global_without_retaining_partial_rows_or_mutating_game(){
    let mut game=game();game.state.vars.insert("large".into(),Value::Str("x".repeat(MAX_INSPECTION_BYTES+1)));
    let before=state_digest(&game);let observed=inspect::capture(&game.state,identity(&game,0));
    assert!(observed.unavailable.is_some());assert!(observed.rows.is_empty());assert_eq!(state_digest(&game),before);
    let bytes=observed.encode_for(identity(&game,1)).unwrap();assert!(bytes.len()<8192);let decoded=Inspection::decode(&bytes).unwrap();
    assert!(decoded.selected(&decoded.frame,&InstanceKey::Game).is_none());
}
#[test]
fn runtime_inspection_decoding_rejects_duplicate_keys_or_cyclic_parentage(){
    let game=game();let observed=observe(&game,1);
    let mut duplicate=observed.clone();duplicate.rows.push(duplicate.rows[0].clone());assert!(Inspection::decode(&serde_json::to_vec(&duplicate).unwrap()).is_err());
    let role=InstanceKey::Host{role:"pause".into()};let mut cyclic=observed;
    cyclic.rows.push(inspect::Instance{key:role.clone(),parent:Some(role),label:"Host".into(),parked:false,details:serde_json::Value::Null});
    assert!(Inspection::decode(&serde_json::to_vec(&cyclic).unwrap()).is_err());
}
#[test]
fn runtime_inspection_host_page_is_an_exact_host_instance_separate_from_story_visit(){
    let mut game=game();let visit=game.state.scene_execution.as_ref().unwrap().owner().unwrap();
    let context=rvn_core::ui::value_from_json(&rvn_ui::source_menus::preview_context(rvn_ui::PageRole::Pause,true)).unwrap();
    game.synchronize_source_menu(rvn_ui::PageRole::Pause,Some("host"),context,true,500).unwrap();
    let snapshot=observe(&game,1);let role=rvn_ui::PageRole::Pause;
    let parent=InstanceKey::Host{role:role.id().into()};let row=snapshot.selected(&snapshot.frame,&parent).unwrap();
    assert_eq!(row.parent,Some(InstanceKey::Game));assert!(!row.parked);
    let screen=game.state.ui.screens.iter().find(|screen|screen.host_role==Some(role)).unwrap();
    let source=game.capture_ui_input_source("host").unwrap();
    let rvn_core::ui_input::UiInputAuthority::Narrative(token)=source.authority else{panic!("exact modern Host token")};
    let key=InstanceKey::Interface{screen:screen.name.clone(),order:screen.order,host_role:Some(role),
        target:Some(token.target)};
    let row=snapshot.selected(&snapshot.frame,&key).unwrap();assert!(!row.parked);
    let target=row.parent.as_ref().unwrap();assert!(matches!(target,InstanceKey::Target{owner:rvn_core::scene_tasks::TaskOwner::Host{role:actual},..}if actual==role.id()));
    assert_eq!(snapshot.selected(&snapshot.frame,target).unwrap().parent,Some(parent));
    assert_eq!(game.state.scene_execution.as_ref().unwrap().owner(),Some(visit));
}
