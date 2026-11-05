//! Private real Engine + evaluator contracts; no Cargo was executed.
use rvn_core::renderer::SceneRenderBatch;
use rvn_core::ui::UiInput;
use rvn_core::{Engine, Interaction, Renderer, SpriteState};
use rvn_parser::{Expr, Hotspot, Position, Transition, Value};
use rvn_ui::programmable::ScreenEventKind;
use std::collections::{BTreeMap, HashMap};
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
    fn supports_custom_canvas(&self) -> bool {
        true
    }
    fn supports_composable_motion(&self) -> bool {
        true
    }
    fn supports_layered_characters(&self) -> bool {
        true
    }
    fn validate_scene_batch(&self, _: &SceneRenderBatch) -> Result<(), String> {
        if self.reject {
            Err("complete Host media candidate rejected".into())
        } else {
            Ok(())
        }
    }
    fn publish_scene_batch(&mut self, batch: SceneRenderBatch) {
        self.batches.push(batch);
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
function pick_face(attributes) {
 set roll=random(1,100)
 set i=0 while i<SELECTOR_LOOPS {set i=i+1}
 if roll<=50 {return {"face":"a"}} return {"face":"b"}
}
function portrait() {return layered_image([64,64],{"face":"a"},image_layers("avatar",["avatar__face__a.png","avatar__face__b.png"]),{"selector":"pick_face"})}
screen host(ctx) {return component("launch","button",{"events":{"click":"launch_host"}},[])}
handler launch_host(event) {
 set launch_roll=random(1,100)
 local n=0 while n<HANDLER_LOOPS {set n=n+1}
 COMMANDS
}
init {set launch_roll=0}
scene label P
"parent"
scene label C
"child"
return
"#;
fn source(selector_loops: usize, handler_loops: usize, commands: &str) -> String {
    SOURCE.replace("SELECTOR_LOOPS", &selector_loops.to_string())
        .replace("HANDLER_LOOPS", &handler_loops.to_string()).replace("COMMANDS", commands)
}
fn prepared(source: &str) -> Engine<Host> {
    let mut game=Engine::from_program(rvn_parser::parse_program(source).unwrap(),Host::default(),32).unwrap();
    assert!(matches!(game.step_until_interaction().unwrap(),Some(Interaction::Dialogue{..})));
    let context=rvn_core::ui::value_from_json(&rvn_ui::source_menus::preview_context(rvn_ui::PageRole::Pause,true)).unwrap();
    game.synchronize_source_menu(rvn_ui::PageRole::Pause,Some("host"),context,true,500).unwrap();
    game
}
fn click(game: &mut Engine<Host>) -> Result<(),rvn_core::RuntimeError> {
    game.interface_event(UiInput{screen:"host".into(),element:"launch".into(),kind:ScreenEventKind::Click,value:None,key:None})
}
fn exact(game: &Engine<Host>) -> serde_json::Value {
    serde_json::json!({"state":game.state,"history":game.history.entries().iter().map(|entry|(&entry.state,&entry.display)).collect::<Vec<_>>(),"runtime":game.runtime_epoch(),"interface":game.interface_epoch()})
}
fn host_random(game: &Engine<Host>) -> rvn_core::random::RandomState {
    game.state.ui.screens.iter().find(|screen|screen.name=="host").unwrap().random
}
fn assert_budget_refused(game: &mut Engine<Host>) {
    let before=exact(game);let random=host_random(game);let batches=game.renderer.batches.len();
    let error=click(game).unwrap_err();
    assert!(error.to_string().contains("100 000 opérations"),"{error}");
    assert_eq!(exact(game),before);assert_eq!(host_random(game),random);
    assert_eq!(game.renderer.batches.len(),batches);
    assert!(game.state.layered.characters.is_empty());
}

#[test]
fn host_handler_and_selector_share_one_budget_and_refuse_all_state_rng_and_publication() {
    let commands=r#"character.compose("avatar",portrait())"#;
    // Each half really succeeds through the same production API independently.
    for (selector,handler) in [(0,7500),(7500,0)] {
        let mut game=prepared(&source(selector,handler,commands));click(&mut game).unwrap();
        assert!(game.state.layered.characters.contains_key("avatar"));
    }
    let mut game=prepared(&source(7500,7500,commands));assert_budget_refused(&mut game);
}
#[test]
fn compose_and_attributes_in_one_host_command_list_cannot_renew_selector_budget() {
    for command in [r#"character.compose("avatar",portrait())"#,r#"character.compose("avatar",portrait()) character.attributes("avatar",{"face":"a"})"#] {
        let mut game=prepared(&source(7500,0,command));
        if command.contains("character.attributes") {assert_budget_refused(&mut game)}
        else {click(&mut game).unwrap();assert!(game.state.layered.characters.contains_key("avatar"));}
    }
}
#[test]
fn closed_source_host_cursor_and_residence_are_restored_when_later_selectors_exceed_budget() {
    let command=r#"ui.close("host") character.compose("avatar",portrait()) character.attributes("avatar",{"face":"a"})"#;
    let mut game=prepared(&source(7500,0,command));
    let old_order=game.state.ui.screens[0].order;assert_budget_refused(&mut game);
    assert_eq!(game.state.ui.screens[0].order,old_order);
    assert_eq!(game.state.ui.screens[0].host_role,Some(rvn_ui::PageRole::Pause));
}
#[test]
fn budgeted_attribute_selection_accumulates_steps_while_legacy_wrapper_remains_fresh() {
    let script=rvn_parser::parse(&source(7500,0,"")).unwrap();
    let library=rvn_core::eval::FunctionLibrary::from_script(&script).unwrap();
    let mut random=rvn_core::random::RandomState::seeded(23);
    let definition=library.eval_with_random(&Expr::Call{name:"portrait".into(),args:vec![]},&HashMap::new(),&mut random).unwrap();
    let definition=rvn_ui::composition::Composition::parse(rvn_core::ui::value_to_json(&definition).unwrap()).unwrap();
    let attributes=BTreeMap::new();let mut budget=rvn_core::ui::CanvasBudget::default();
    let first=rvn_core::composition::resolve_attributes_budgeted(&definition,&attributes,&definition.defaults,&library,&HashMap::new(),&mut random,&mut budget).unwrap();
    let after_first=random;
    assert!(rvn_core::composition::resolve_attributes_budgeted(&definition,&first,&attributes,&library,&HashMap::new(),&mut random,&mut budget).unwrap_err().to_string().contains("100 000 opérations"));
    assert_eq!(random,after_first);
    for _ in 0..2 {rvn_core::composition::resolve_attributes(&definition,&attributes,&definition.defaults,&library,&HashMap::new(),&mut random).unwrap();}
}
#[test]
fn budgeted_expression_accumulates_value_work_and_preserves_lexical_scope_and_rng_on_refusal() {
    let library=rvn_core::eval::FunctionLibrary::from_script(&rvn_parser::parse("function payload_copy() {return payload}").unwrap()).unwrap();
    let expression=Expr::Call{name:"payload_copy".into(),args:vec![]};
    let globals=HashMap::from([("payload".into(),Value::List(vec![Value::Int(17);4096]))]);
    let shadow=HashMap::from([("payload".into(),Value::Int(99))]);
    let mut random=rvn_core::random::RandomState::seeded(31);let before=random;let mut budget=rvn_core::ui::CanvasBudget::default();
    let mut completed=0;
    loop {
        match library.eval_scoped_with_random_budgeted(&expression,&shadow,&globals,&mut random,&mut budget) {
            Ok(value)=>{assert_eq!(value,globals["payload"]);completed+=1;assert!(completed<1000)},
            Err(error)=>{assert!(error.to_string().contains("1 000 000"),"{error}");break}
        }
    }
    assert!(completed>1);assert_eq!(random,before);
    for _ in 0..(completed+1) {assert_eq!(library.eval_with_random(&expression,&globals,&mut random).unwrap(),globals["payload"]);}
}
