//! Producer-bound input identity contracts. CPU only; no DOM/native claims.
use rvn_core::{Engine, Renderer, SpriteState, Interaction};
use rvn_core::renderer::SceneRenderBatch;
use rvn_core::ui::UiInput;
use rvn_core::ui_input::{UiInputSource, UiInputAuthority, ScopedUiInput};
use rvn_core::scene_resources::ResourceTaskKind;
use rvn_ui::{PageRole, programmable::ScreenEventKind};
use rvn_parser::{Value, Transition, Position, Hotspot};
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
fn click(source:&UiInputSource)->ScopedUiInput{ScopedUiInput{source:source.clone(),event:UiInput{screen:source.screen.clone(),element:"act".into(),kind:ScreenEventKind::Click,value:None,key:None}}}
fn frozen(g:&Engine<Host>)->(serde_json::Value,usize,usize,u64){(serde_json::to_value(&g.state).unwrap(),g.history.len(),g.renderer.batches.len(),g.runtime_epoch())}

#[test]
fn legacy_preview_keeps_the_display_visit_after_real_ui_handlers_and_history_eviction(){
 let legacy=SOURCE.replace("scene label ","label ");
 let mut g=Engine::new(rvn_parser::parse(&legacy).unwrap(),Host::default(),2).unwrap();
 assert_eq!(dialogue(&mut g),"parent");let displayed=g.preview_input_source().unwrap();
 for value in 1..=4{
  let source=g.capture_ui_input_source("panel").unwrap();
  assert!(g.interface_event_scoped(click(&source)).unwrap());
  assert_eq!(g.state.vars["hits"],Value::Int(value));
  assert_eq!(g.preview_input_source(),Some(displayed.clone()),"a UI rollback step must not replace the displayed narrative visit");
 }
 assert_eq!(g.history.len(),2);assert!(g.accepts_preview_input_source(&displayed));
 assert_eq!(advance(&mut g),"child");assert!(!g.accepts_preview_input_source(&displayed));
 assert_eq!(g.state.vars["hits"],Value::Int(4));assert!(g.preview_input_source().is_some());
}

#[test]
fn legacy_preview_restore_has_a_fresh_display_visit_after_ui_rollback_and_save_load(){
 let legacy=SOURCE.replace("scene label ","label ");
 let mut g=Engine::new(rvn_parser::parse(&legacy).unwrap(),Host::default(),2).unwrap();
 assert_eq!(dialogue(&mut g),"parent");let first=g.preview_input_source().unwrap();
 let source=g.capture_ui_input_source("panel").unwrap();assert!(g.interface_event_scoped(click(&source)).unwrap());
 assert!(g.rollback());assert_eq!(g.state.vars["hits"],Value::Int(0));
 let restored=g.preview_input_source().expect("restored display must accept fresh input");
 assert_ne!(restored,first);assert!(!g.accepts_preview_input_source(&first));
 let saved=rvn_core::save::SaveData::from_state(&g.state,0,"Preview restore".into(),String::new());
 g.load_data(saved).unwrap();assert!(g.history.is_empty());
 let loaded=g.preview_input_source().expect("loaded display must not depend on retained rollback entries");
 assert_ne!(loaded,restored);assert!(!g.accepts_preview_input_source(&restored));
 assert_eq!(advance(&mut g),"child");assert!(!g.accepts_preview_input_source(&loaded));
}

#[test]
fn scoped_input_game_capability_survives_call_without_interface_epoch_rebinding(){
 let mut g=game();let source=g.capture_ui_input_source("game").unwrap();let epoch=g.interface_epoch();
 assert_eq!(advance(&mut g),"child");assert_ne!(g.interface_epoch(),epoch);
 assert!(g.ui_input_source_current(&source));assert!(g.interface_event_scoped(click(&source)).unwrap());assert_eq!(g.state.vars["hits"],Value::Int(1));
}
#[test]
fn scoped_input_scene_parked_refuses_before_payload_and_never_adopts_child_alias(){
 let mut g=game();let source=g.capture_ui_input_source("panel").unwrap();assert_eq!(advance(&mut g),"child");
 let current=g.capture_ui_input_source("panel").unwrap();assert_ne!(source,current);let before=frozen(&g);
 let mut old=click(&source);old.event.value=Some(Value::Float(f32::NAN));old.event.element="x".repeat(1024);old.event.kind=ScreenEventKind::Tick;
 assert!(!g.interface_event_scoped(old).unwrap());assert_eq!(frozen(&g),before);
 assert!(g.interface_event_scoped(click(&current)).unwrap());assert_eq!(g.state.vars["hits"],Value::Int(1));
}
#[test]
fn scoped_input_return_revision_rejects_old_parent_but_accepts_new_exact_source(){
 let mut g=game();let parent=g.capture_ui_input_source("panel").unwrap();assert_eq!(advance(&mut g),"child");assert_eq!(advance(&mut g),"returned");
 let renewed=g.capture_ui_input_source("panel").unwrap();
 let (UiInputAuthority::Narrative(old),UiInputAuthority::Narrative(new))=(&parent.authority,&renewed.authority)else{panic!("Expected exact resource tokens")};
 assert_eq!(old.target,new.target);assert_eq!(old.task.key,new.task.key);assert_ne!(old.task.revision,new.task.revision);
 let before=frozen(&g);assert!(!g.interface_event_scoped(click(&parent)).unwrap());assert_eq!(frozen(&g),before);assert!(g.interface_event_scoped(click(&renewed)).unwrap());
}
#[test]
fn scoped_input_same_alias_reopen_does_not_adopt_queued_click_or_ime_change(){
 let mut g=game();let old=g.capture_ui_input_source("panel").unwrap();let mut renew=click(&old);renew.event.element="renew".into();assert!(g.interface_event_scoped(renew).unwrap());
 let replacement=g.capture_ui_input_source("panel").unwrap();assert_ne!(replacement,old);let before=frozen(&g);
 assert!(!g.interface_event_scoped(click(&old)).unwrap());let mut ime=click(&old);ime.event.kind=ScreenEventKind::Change;ime.event.element="name".into();ime.event.value=Some(Value::Str("queued preedit".into()));assert!(!g.interface_event_scoped(ime).unwrap());assert_eq!(frozen(&g),before);
 assert!(g.interface_event_scoped(click(&replacement)).unwrap());
}
#[test]
fn scoped_input_exact_order_role_and_screen_are_checked_before_handler(){
 let mut g=game();let source=g.capture_ui_input_source("panel").unwrap();let before=frozen(&g);
 let mut order=click(&source);order.source.order+=1;assert!(!g.interface_event_scoped(order).unwrap());
 let mut role=click(&source);role.source.host_role=Some(PageRole::Pause);assert!(!g.interface_event_scoped(role).unwrap());
 let mut alias=click(&source);alias.event.screen="game".into();assert!(!g.interface_event_scoped(alias).unwrap());assert_eq!(frozen(&g),before);
}
#[test]
fn scoped_input_non_interface_resource_token_cannot_authorize_interface_event(){
 let mut g=game();let mut source=g.capture_ui_input_source("panel").unwrap();
 if let UiInputAuthority::Narrative(token)=&mut source.authority{token.target.kind=rvn_core::scene_resources::TargetKind::Canvas;}else{panic!()}
 let before=frozen(&g);assert!(!g.interface_event_scoped(click(&source)).unwrap());assert_eq!(frozen(&g),before);
 assert_eq!(g.state.scene_execution.as_ref().unwrap().resources.tasks.iter().filter(|t|t.kind==ResourceTaskKind::Interface).count(),2);
}
#[test]
fn scoped_input_legacy_epoch_order_adapter_and_old_missing_address_diagnostic_remain(){
 let legacy=SOURCE.replace("scene label ","label ");let mut g=Engine::new(rvn_parser::parse(&legacy).unwrap(),Host::default(),32).unwrap();assert_eq!(dialogue(&mut g),"parent");
 let source=g.capture_ui_input_source("panel").unwrap();assert!(matches!(source.authority,UiInputAuthority::Legacy{..}));
 assert!(g.interface_event_scoped(click(&source)).unwrap());let mut renew=click(&source).event;renew.element="renew".into();g.interface_event(renew).unwrap();let before=frozen(&g);
 assert!(!g.interface_event_scoped(click(&source)).unwrap());assert_eq!(frozen(&g),before);
 let mut absent=click(&source).event;absent.screen="absent".into();assert!(g.interface_event(absent).is_err());
}
#[test]
fn scoped_input_load_revokes_sources_but_serialized_save_never_contains_input_queue(){
 let mut g=game();let source=g.capture_ui_input_source("panel").unwrap();let save=rvn_core::save::SaveData::try_from_state(&g.state,1,"QA".into(),"main.rvn".into()).unwrap();
 // SaveData rebuilds HashMaps and stamps wall time. Compare every serialized
 // field semantically at the same metadata timestamp; object member order
 // does not encode an input queue or change the existing save contract.
 let before=serde_json::to_value(&save).unwrap();let _queued=vec![click(&source)];
 let mut after_save=rvn_core::save::SaveData::try_from_state(&g.state,1,"QA".into(),"main.rvn".into()).unwrap();after_save.timestamp=save.timestamp;
 let after=serde_json::to_value(&after_save).unwrap();assert_eq!(after,before);
 g.load_data(save).unwrap();let snapshot=frozen(&g);assert!(!g.interface_event_scoped(click(&source)).unwrap());assert_eq!(frozen(&g),snapshot);assert!(g.interface_event_scoped(click(&g.capture_ui_input_source("panel").unwrap())).unwrap());
}
#[test]
fn scoped_input_current_invalid_payload_and_internal_lifecycle_still_fail_atomically(){
 let mut g=game();let source=g.capture_ui_input_source("panel").unwrap();let before=frozen(&g);let mut invalid=click(&source);invalid.event.kind=ScreenEventKind::Tick;
 assert!(g.interface_event_scoped(invalid).is_err());assert_eq!(frozen(&g),before);
 let mut large=click(&source);large.event.key=Some("a".repeat(257));assert!(g.interface_event_scoped(large).is_err());assert_eq!(frozen(&g),before);
}
#[test]
fn scoped_input_host_role_closure_revokes_original_capability_and_same_order_is_not_identity(){
 let mut g=game();let context=rvn_core::ui::value_from_json(&rvn_ui::source_menus::preview_context(PageRole::Pause,true)).unwrap();
 g.synchronize_source_menu(PageRole::Pause,Some("host"),context.clone(),false,500).unwrap();let old=g.capture_ui_input_source("host").unwrap();assert_eq!(old.host_role,Some(PageRole::Pause));
 assert!(g.interface_event_scoped(click(&old)).unwrap());g.synchronize_source_menu(PageRole::Pause,None,Value::Dict(Default::default()),false,0).unwrap();g.synchronize_source_menu(PageRole::Pause,Some("host"),context,false,500).unwrap();let before=frozen(&g);
 assert!(!g.interface_event_scoped(click(&old)).unwrap());assert_eq!(frozen(&g),before);assert_ne!(old,g.capture_ui_input_source("host").unwrap());
}
#[test]
fn scoped_input_wire_roundtrip_keeps_u64_above_javascript_precision_and_rejects_unknown_header_fields(){
 let g=game();let mut source=g.capture_ui_input_source("panel").unwrap();source.lineage=u64::MAX;source.order=u64::MAX;
 if let UiInputAuthority::Narrative(token)=&mut source.authority{token.task.runtime_epoch=u64::MAX;token.task.revision=(1u64<<53)+1;token.target.incarnation=u64::MAX;}
 let wire=source.to_wire().unwrap();assert_eq!(UiInputSource::from_wire(&wire).unwrap(),source);
 let mut json:serde_json::Value=serde_json::from_str(&wire).unwrap();json["payload"]=true.into();assert!(UiInputSource::from_wire(&json.to_string()).is_err());assert!(UiInputSource::from_wire(&" ".repeat(4097)).is_err());
}

#[test]
fn scoped_input_corrupt_visible_snapshot_cannot_fabricate_authority_in_sync_adapter(){
 let mut g=game();let source=g.capture_ui_input_source("panel").unwrap();
 g.state.scene_execution.as_mut().unwrap().resources.tasks.retain(|task|!(task.kind==ResourceTaskKind::Interface && task.alias()=="panel"));
 let before=frozen(&g);assert!(g.capture_ui_input_source("panel").is_none());assert!(!g.interface_event_scoped(click(&source)).unwrap());
 assert!(g.interface_event(click(&source).event).is_err());assert_eq!(frozen(&g),before);assert_eq!(g.state.vars["hits"],Value::Int(0));
 let mut corrupt_order=game();let source=corrupt_order.capture_ui_input_source("panel").unwrap();corrupt_order.state.ui.screens.iter_mut().find(|screen|screen.name=="panel").unwrap().order=u64::MAX;let before=frozen(&corrupt_order);assert!(corrupt_order.capture_ui_input_source("panel").is_none());assert!(!corrupt_order.interface_event_scoped(click(&source)).unwrap());assert!(corrupt_order.interface_event(click(&source).event).is_err());assert_eq!(frozen(&corrupt_order),before);
}

#[test]
fn scoped_input_identical_new_engines_never_reuse_an_old_producer_lineage(){
 for legacy in [false,true]{
  let construct=||{if legacy {let script=SOURCE.replace("scene label ","label ");let mut g=Engine::new(rvn_parser::parse(&script).unwrap(),Host::default(),32).unwrap();assert_eq!(dialogue(&mut g),"parent");g}else{game()}};
  let first=construct();let old=first.capture_ui_input_source("panel").unwrap();let mut second=construct();let current=second.capture_ui_input_source("panel").unwrap();
  assert_eq!(first.runtime_epoch(),second.runtime_epoch());assert_eq!(first.interface_epoch(),second.interface_epoch());assert_eq!(old.authority,current.authority);assert_eq!(old.order,current.order);assert_eq!(old.host_role,current.host_role);assert_ne!(old.lineage,current.lineage);
  let before=frozen(&second);let mut pending=click(&old);pending.event.value=Some(Value::Float(f32::NAN));assert!(!second.interface_event_scoped(pending).unwrap());assert_eq!(frozen(&second),before);assert!(second.interface_event_scoped(click(&current)).unwrap());
  let mut fresh=second.fresh(Host::default(),32).unwrap();assert_eq!(dialogue(&mut fresh),"parent");assert_eq!(fresh.runtime_epoch(),second.runtime_epoch()+1);assert!(!fresh.ui_input_source_current(&current));assert_ne!(fresh.capture_ui_input_source("panel").unwrap().lineage,current.lineage);
  let save=rvn_core::save::SaveData::try_from_state(&fresh.state,1,"QA".into(),"main.rvn".into()).unwrap();assert!(!serde_json::to_string(&save).unwrap().contains("lineage"));
 }
}

#[test]
fn scoped_input_successful_internal_commits_keep_instance_and_existing_capability() {
    for legacy in [false, true] {
        let mut game = if legacy {
            let text = SOURCE.replace("scene label ", "label ");
            let mut game = Engine::new(rvn_parser::parse(&text).unwrap(), Host::default(), 32).unwrap();
            assert_eq!(dialogue(&mut game), "parent");
            game
        } else {
            let text = SOURCE.replace("scene label P\n", "scene label P\ntimer 5 => jump done\n");
            let mut game = Engine::from_program(rvn_parser::parse_program(&text).unwrap(), Host::default(), 32).unwrap();
            assert_eq!(dialogue(&mut game), "parent");
            game
        };
        let identity = game.instance_id();
        let source = game.capture_ui_input_source("panel").unwrap();
        let history = game.history.len();
        let publications = game.renderer.batches.len();
        assert!(game.interface_event_scoped(click(&source)).unwrap());
        assert_eq!(game.state.vars["hits"], Value::Int(1));
        assert_eq!(game.history.len(), history + 1);
        assert_eq!(game.instance_id(), identity);
        assert_eq!(game.capture_ui_input_source("panel").unwrap(), source);
        assert!(game.ui_input_source_current(&source));
        if !legacy {
            assert_eq!(game.renderer.batches.len(), publications + 1);
            let history = game.history.len();
            let publications = game.renderer.batches.len();
            let frame = game.tick_game_frame(0.125, Default::default()).unwrap();
            assert!(frame.progressed && frame.published);
            assert_eq!(game.renderer.batches.len(), publications + 1);
            assert_eq!(game.history.len(), history);
            assert_eq!(game.state.scene_execution.as_ref().unwrap().active.as_ref().unwrap().tasks.timer.as_ref().unwrap().elapsed, 0.125);
            assert_eq!(game.instance_id(), identity);
            assert_eq!(game.capture_ui_input_source("panel").unwrap(), source);
        }
        assert!(game.interface_event_scoped(click(&source)).unwrap());
        assert_eq!(game.state.vars["hits"], Value::Int(2));
        assert_eq!(game.instance_id(), identity);
    }
}

#[test]
fn scoped_input_refused_candidates_and_rollback_keep_instance_but_restore_renews_authority() {
    let text = SOURCE.replace("scene label P\n", "scene label P\ntimer 5 => jump done\n");
    let mut game = Engine::from_program(rvn_parser::parse_program(&text).unwrap(), Host::default(), 32).unwrap();
    assert_eq!(dialogue(&mut game), "parent");
    let identity = game.instance_id();
    let source = game.capture_ui_input_source("panel").unwrap();
    let before = frozen(&game);
    game.renderer.reject = true;
    assert!(game.interface_event_scoped(click(&source)).is_err());
    assert_eq!(frozen(&game), before);
    assert!(game.tick_game_frame(0.125, Default::default()).is_err());
    assert_eq!(frozen(&game), before);
    assert_eq!(game.instance_id(), identity);
    assert_eq!(game.capture_ui_input_source("panel").unwrap(), source);
    game.renderer.reject = false;
    assert!(game.interface_event_scoped(click(&source)).unwrap());
    assert_eq!(game.state.vars["hits"], Value::Int(1));
    let before = frozen(&game);
    game.renderer.reject = true;
    assert!(!game.rollback());
    assert_eq!(frozen(&game), before);
    assert_eq!(game.instance_id(), identity);
    assert!(game.ui_input_source_current(&source));
    game.renderer.reject = false;
    let runtime_epoch = game.runtime_epoch();
    let history = game.history.len();
    assert!(game.rollback());
    assert_eq!(game.instance_id(), identity);
    assert_eq!(game.runtime_epoch(), runtime_epoch + 1);
    assert_eq!(game.history.len(), history - 1);
    assert_eq!(game.state.vars["hits"], Value::Int(0));
    let restored = game.capture_ui_input_source("panel").unwrap();
    assert_eq!(restored.lineage, source.lineage);
    assert_ne!(restored.authority, source.authority);
    let before = frozen(&game);
    assert!(!game.interface_event_scoped(click(&source)).unwrap());
    assert_eq!(frozen(&game), before);
    assert!(game.interface_event_scoped(click(&restored)).unwrap());
    assert_eq!(game.state.vars["hits"], Value::Int(1));
    assert_eq!(game.instance_id(), identity);
}


// Existing programmable UI remains read-only for progression.
// Private additions to scoped_ui_input's real Host; no generic UserWidget claim.

#[test]
fn persistent_readonly_0032_real_scoped_handler_reads_progression_without_save_leak() {
    let source=r#"
handler inspect(event){set observed=persistent.counter}
screen progression(){return component("act","button",{"events":{"click":"inspect"}},[])}
init{set observed=0}
scene label P
ui.open_story("progression",[],false,1)
"Waiting"
"#;
    let seed=[("persistent.counter".into(),Value::Int(7))].into_iter().collect();
    let mut game=Engine::from_program_with_persistent(rvn_parser::parse_program(source).unwrap(),Host::default(),32,seed).unwrap();
    game.enter_label("P").unwrap();assert_eq!(dialogue(&mut game),"Waiting");
    let producer=game.capture_ui_input_source("progression").unwrap();
    assert!(game.interface_event_scoped(click(&producer)).unwrap());
    assert_eq!(game.state.vars["observed"],Value::Int(7));
    assert_eq!(game.get_persistent_var("persistent.counter"),Some(&Value::Int(7)));
    assert!(!game.state.vars.contains_key("persistent.counter"));
    let saved=rvn_core::save::SaveData::try_from_snapshot(&game.snapshot_for_save().unwrap(),1,"Owned read".into(),"program.rvn".into()).unwrap();
    assert!(!saved.vars.contains_key("persistent.counter"));
}

#[test]
fn persistent_readonly_0032_real_story_assignment_invalidates_presentation_cache() {
    let source=r#"
screen progression(){return component("flag","toggle",{"value":persistent.flag},[])}
scene label P
ui.open_story("progression",[],false,1)
"Before"
set persistent.flag=true
"After"
"#;
    let seed=[("persistent.flag".into(),Value::Bool(false))].into_iter().collect();
    let mut game=Engine::from_program_with_persistent(rvn_parser::parse_program(source).unwrap(),Host::default(),32,seed).unwrap();
    game.enter_label("P").unwrap();assert_eq!(dialogue(&mut game),"Before");
    assert_eq!(game.interface_views().unwrap()[0].root.value,serde_json::json!(false));
    let vars=game.state.vars.clone();let ui=game.state.ui.clone();
    assert_eq!(advance(&mut game),"After");
    assert_eq!(game.get_persistent_var("persistent.flag"),Some(&Value::Bool(true)));
    assert_eq!(game.state.vars,vars);assert_eq!(game.state.ui,ui);
    assert_eq!(game.interface_views().unwrap()[0].root.value,serde_json::json!(true));
}

#[test]
fn persistent_readonly_0032_renderer_refusal_keeps_progression_game_and_history() {
    let source=r#"
handler inspect(event){set observed=persistent.counter}
screen progression(){return component("act","button",{"events":{"click":"inspect"}},[])}
init{set observed=0}
scene label P
ui.open_story("progression",[],false,1)
"Waiting"
"#;
    let seed=[("persistent.counter".into(),Value::Int(7))].into_iter().collect();
    let mut game=Engine::from_program_with_persistent(rvn_parser::parse_program(source).unwrap(),Host::default(),32,seed).unwrap();
    game.enter_label("P").unwrap();assert_eq!(dialogue(&mut game),"Waiting");
    let before=frozen(&game);let progression=game.export_persistent_vars().clone();
    let producer=game.capture_ui_input_source("progression").unwrap();game.renderer.reject=true;
    assert!(game.interface_event_scoped(click(&producer)).unwrap_err().to_string().contains("Rejected exact resident projection"));
    assert_eq!(game.export_persistent_vars(),&progression);assert_eq!(frozen(&game),before);
}

#[test]
fn persistent_readonly_0032_stale_source_refuses_before_handler_and_payload() {
    let source=r#"
handler inspect(event){set observed=persistent.counter}
screen progression(){return component("act","button",{"events":{"click":"inspect"}},[])}
init{set observed=0}
scene label P
ui.open_story("progression",[],false,1)
"Before"
ui.close("progression")
ui.open_story("progression",[],false,1)
"After"
"#;
    let seed=[("persistent.counter".into(),Value::Int(7))].into_iter().collect();
    let mut game=Engine::from_program_with_persistent(rvn_parser::parse_program(source).unwrap(),Host::default(),32,seed).unwrap();
    game.enter_label("P").unwrap();assert_eq!(dialogue(&mut game),"Before");
    let producer=game.capture_ui_input_source("progression").unwrap();assert_eq!(advance(&mut game),"After");
    let before=frozen(&game);let progression=game.export_persistent_vars().clone();
    let mut input=click(&producer);input.event.kind=ScreenEventKind::Tick;input.event.element="x".repeat(1024);input.event.value=Some(Value::Float(f32::NAN));
    assert!(!game.interface_event_scoped(input).unwrap());assert_eq!(frozen(&game),before);assert_eq!(game.export_persistent_vars(),&progression);
}

#[test]
fn persistent_readonly_0032_nested_open_handler_reads_current_progression() {
    let source=r#"
handler spawn(event){ui.open_story("nested",[],false,2)}
handler opened(event){set observed=persistent.counter}
screen progression(){return component("act","button",{"events":{"click":"spawn"}},[])}
screen nested(){return component("root","panel",{"events":{"open":"opened"}},[])}
init{set observed=0}
scene label P
ui.open_story("progression",[],false,1)
"Waiting"
"#;
    let seed=[("persistent.counter".into(),Value::Int(7))].into_iter().collect();
    let mut game=Engine::from_program_with_persistent(rvn_parser::parse_program(source).unwrap(),Host::default(),32,seed).unwrap();
    game.enter_label("P").unwrap();assert_eq!(dialogue(&mut game),"Waiting");
    let producer=game.capture_ui_input_source("progression").unwrap();assert!(game.interface_event_scoped(click(&producer)).unwrap());
    assert_eq!(game.state.vars["observed"],Value::Int(7));assert_eq!(game.get_persistent_var("persistent.counter"),Some(&Value::Int(7)));
    assert!(!game.state.vars.contains_key("persistent.counter"));assert!(game.capture_ui_input_source("nested").is_some());
    assert_eq!(game.interface_views().unwrap().iter().filter(|view|view.name=="nested").count(),1);
}

#[test]
fn persistent_readonly_0032_existing_write_and_game_binding_guards_remain() {
    let handler=r#"handler denied(event){set persistent.counter=8} scene label P "Waiting""#;
    let program=rvn_parser::parse_program(handler).unwrap();
    let seed=[("persistent.counter".into(),Value::Int(7))].into_iter().collect();
    assert!(Engine::from_program_with_persistent(program,Host::default(),32,seed).is_err());
    let binding=r#"screen progression(){return component("flag","toggle",{"binding":"persistent.flag"},[])} scene label P ui.open_story("progression",[],false,1) "Waiting""#;
    let seed=[("persistent.flag".into(),Value::Bool(false))].into_iter().collect();
    let mut game=Engine::from_program_with_persistent(rvn_parser::parse_program(binding).unwrap(),Host::default(),32,seed).unwrap();
    game.enter_label("P").unwrap();
    assert!(game.step_until_interaction().unwrap_err().to_string().contains("Invalid game-variable binding"));
    assert_eq!(game.get_persistent_var("persistent.flag"),Some(&Value::Bool(false)));
}
