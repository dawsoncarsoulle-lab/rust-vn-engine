use rvn_core::composition::LayeredView;
use rvn_core::{Engine, Renderer, SpriteState};
use rvn_parser::{parse, Hotspot, Position, Transition};
#[derive(Default)]
struct Headless {
    views: Vec<LayeredView>,
    reject: bool,
}
impl Renderer for Headless {
    fn supports_layered_characters(&self) -> bool {
        true
    }
    fn supports_composable_motion(&self) -> bool {
        true
    }
    fn update_layered_characters(&mut self, views: &[LayeredView]) -> Result<(), String> {
        if self.reject {
            Err("Test rejection".into())
        } else {
            self.views = views.to_vec();
            Ok(())
        }
    }
    fn update_motions(&mut self, _: &[rvn_core::motion::MotionView]) -> Result<(), String> {
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
    fn music_play(&mut self, _: &str, _: &Transition, _: Option<&str>) {}
    fn music_stop(&mut self, _: &Transition) {}
    fn music_set_volume(&mut self, _: f32) {}
    fn sfx_play(&mut self, _: &str, _: &Transition) {}
    fn sfx_stop(&mut self, _: &str, _: &Transition) {}
    fn show_imagemap(&mut self, _: &str, _: Option<&str>, _: &[Hotspot]) -> usize {
        0
    }
}
const PROGRAM: &str = r#"
function portrait() {
    return layered_image([600,1000], {"outfit":"shirt","face":"neutral"}, [
        image_layer("body","body.png",{}),
        image_layer("shirt","shirt.png",{"group":"outfit","attribute":"shirt"}),
        image_layer("coat","coat.png",{"group":"outfit","attribute":"coat"}),
        image_layer("neutral","neutral.png",{"group":"face","attribute":"neutral"}),
        image_layer("happy","happy.png",{"group":"face","attribute":"happy"})
    ])
}
label start
character.compose("iris",portrait())
iris.show() at left
"Default"
character.attributes("iris",{"outfit":"coat"})
motion.play("layer:iris/neutral",motion_tween(2,{}, {"rotation":30},"linear"))
"Coat"
character.attributes("iris",{"face":"happy"})
motion.play("sprite:iris",motion_tween(2,{}, {"opacity":0.5},"linear"))
"Happy"
iris.hide()
"Hidden"
"#;
fn game() -> Engine<Headless> {
    Engine::new(parse(PROGRAM).unwrap(), Headless::default(), 16).unwrap()
}
fn layers(engine: &Engine<Headless>) -> Vec<&str> {
    engine.renderer.views[0]
        .layers
        .iter()
        .map(|layer| layer.id.as_str())
        .collect()
}
fn advance(engine: &mut Engine<Headless>) {
    engine.advance_dialogue().unwrap();
    engine.step_until_interaction().unwrap();
}
const ADVANCED: &str = r#"
function select_accessories(attributes) {
    set ignored_global = 99
    if attributes["face"] == "happy" {return {"accessory":"badge"}}
    return {"accessory":"none"}
}
function advanced_portrait() {
    return layered_image([600,1000],{"outfit":"shirt","face":"neutral","accessory":"none","variant":""},image_layers("iris",[
        "iris__body.png","iris__outfit__shirt.png","iris__outfit__coat.png",
        "iris__face__neutral.png","iris__face__happy.png","iris__evening__face__happy.png",
        "iris__accessory__none.png","iris__accessory__badge.png"
    ]),{"variants":{"evening":{"outfit":"coat"}},"rules":{"evening_face":{"when":{"variant":"evening"},"set":{"face":"happy"}}},"selector":"select_accessories"})
}
label start
character.compose("iris",advanced_portrait())
iris.show()
"Default"
character.attributes("iris",{"variant":"evening"})
"Evening"
character.attributes("iris",{"outfit":"shirt"})
"Shirt"
"Finish"
"#;
#[test]
fn discovery_variants_rules_and_rvn_selection_restore_save_and_rollback() {
    let mut engine = Engine::new(parse(ADVANCED).unwrap(), Headless::default(), 16).unwrap();
    engine.step_until_interaction().unwrap();
    assert_eq!(
        engine.state.layered.characters["iris"].attributes["face"],
        "neutral"
    );
    advance(&mut engine);
    assert_eq!(
        layers(&engine),
        [
            "body",
            "outfit_coat",
            "evening_face_happy",
            "accessory_badge"
        ]
    );
    let saved = rvn_core::save::SaveData::from_state(
        &engine.state,
        1,
        "Advanced".into(),
        "main.rvn".into(),
    );
    assert!(!engine.state.vars.contains_key("ignored_global"));
    advance(&mut engine);
    assert_eq!(
        engine.state.layered.characters["iris"].attributes["outfit"],
        "shirt"
    );
    assert_eq!(
        engine.state.layered.characters["iris"].attributes["variant"],
        "evening"
    );
    assert!(engine.rollback());
    assert!(engine.rollback());
    assert_eq!(
        engine.state.layered.characters["iris"].attributes["outfit"],
        "coat"
    );
    engine
        .load_data(serde_json::from_value(serde_json::to_value(&saved).unwrap()).unwrap())
        .unwrap();
    assert_eq!(engine.state.layered, saved.layered);
    assert_eq!(
        layers(&engine),
        [
            "body",
            "outfit_coat",
            "evening_face_happy",
            "accessory_badge"
        ]
    );
}
#[test]
fn missing_invalid_or_nonterminating_attribute_selectors_fail_without_partial_state() {
    for body in [
        "return 12",
        "return {\"face\":\"typo\"}",
        "while true {} return {}",
        "return {\"variant\":\"evening\"}",
    ] {
        let source=ADVANCED.replace("set ignored_global = 99\n    if attributes[\"face\"] == \"happy\" {return {\"accessory\":\"badge\"}}\n    return {\"accessory\":\"none\"}",body);
        let mut engine = Engine::new(parse(&source).unwrap(), Headless::default(), 8).unwrap();
        let mut before = serde_json::to_value(&engine.state).unwrap();
        assert!(engine.step_until_interaction().is_err(), "{body}");
        let mut after = serde_json::to_value(&engine.state).unwrap();
        before.as_object_mut().unwrap().remove("pc");
        after.as_object_mut().unwrap().remove("pc");
        assert_eq!(after, before, "{body}");
        assert!(engine.renderer.views.is_empty());
    }
    let source = ADVANCED.replace(
        "\"selector\":\"select_accessories\"",
        "\"selector\":\"missing_selector\"",
    );
    let mut engine = Engine::new(parse(&source).unwrap(), Headless::default(), 8).unwrap();
    assert!(engine.step_until_interaction().is_err());
}
#[test]
fn variant_override_cancels_only_disappearing_layer_animations() {
    let source=ADVANCED.replace("\"Default\"","motion.play(\"layer:iris/face_neutral\",motion_tween(4,{}, {\"opacity\":0.5},\"linear\"))\nmotion.play(\"sprite:iris\",motion_tween(4,{}, {\"x\":100},\"linear\"))\n\"Default\"");
    let mut engine = Engine::new(parse(&source).unwrap(), Headless::default(), 8).unwrap();
    engine.step_until_interaction().unwrap();
    assert!(engine
        .state
        .motions
        .tracks
        .contains_key("layer:iris/face_neutral"));
    advance(&mut engine);
    assert!(!engine
        .state
        .motions
        .tracks
        .contains_key("layer:iris/face_neutral"));
    assert!(engine.state.motions.tracks.contains_key("sprite:iris"));
}
#[test]
fn attributes_are_independent_and_restore_with_motion_save_load_and_rollback() {
    let mut engine = game();
    engine.step_until_interaction().unwrap();
    assert_eq!(layers(&engine), ["body", "shirt", "neutral"]);
    advance(&mut engine);
    assert_eq!(layers(&engine), ["body", "coat", "neutral"]);
    engine.tick_motions(1.0).unwrap();
    let saved =
        rvn_core::save::SaveData::from_state(&engine.state, 1, "Layers".into(), "main.rvn".into());
    let saved_motion = saved.motions.clone();
    advance(&mut engine);
    assert_eq!(layers(&engine), ["body", "coat", "happy"]);
    assert!(!engine
        .state
        .motions
        .tracks
        .contains_key("layer:iris/neutral"));
    advance(&mut engine);
    assert!(engine.renderer.views.is_empty());
    assert!(engine.state.motions.tracks.is_empty());
    assert!(engine.rollback());
    assert!(engine.renderer.views.is_empty());
    assert!(engine.rollback());
    assert_eq!(layers(&engine), ["body", "coat", "happy"]);
    engine
        .load_data(serde_json::from_value(serde_json::to_value(saved).unwrap()).unwrap())
        .unwrap();
    assert_eq!(layers(&engine), ["body", "coat", "neutral"]);
    assert_eq!(engine.state.motions, saved_motion);
}
#[test]
fn invalid_attributes_saved_definitions_and_renderer_errors_leave_gameplay_intact() {
    let mut engine = game();
    engine.step_until_interaction().unwrap();
    let before = serde_json::to_value(&engine.state).unwrap();
    let mut save = rvn_core::save::SaveData::from_state(&engine.state, 1, "".into(), "".into());
    save.layered
        .characters
        .get_mut("iris")
        .unwrap()
        .attributes
        .insert("face".into(), "typo".into());
    assert!(engine.load_data(save).is_err());
    assert_eq!(serde_json::to_value(&engine.state).unwrap(), before);
    engine.renderer.reject = true;
    engine.advance_dialogue().unwrap();
    let before = serde_json::to_value(&engine.state).unwrap();
    assert!(engine.step_until_interaction().is_err());
    assert_eq!(serde_json::to_value(&engine.state).unwrap(), before);
    let mut unsupported =
        Engine::new(parse(PROGRAM).unwrap(), rvn_core::TerminalRenderer, 8).unwrap();
    assert!(unsupported.step_until_interaction().is_err());
    assert!(unsupported.state.layered.characters.is_empty());
    assert!(parse("function bad(){character.compose(\"iris\",{})}").is_err());
}
#[test]
fn old_saves_default_to_empty_compositions_and_frame_animation_needs_a_layer() {
    let engine = game();
    let saved = rvn_core::save::SaveData::from_state(&engine.state, 1, "".into(), "".into());
    let mut json = serde_json::to_value(saved).unwrap();
    json.as_object_mut().unwrap().remove("layered");
    json["format_version"] = serde_json::json!(4);
    let decoded: rvn_core::save::SaveData = serde_json::from_value(json).unwrap();
    assert!(decoded.layered.characters.is_empty());
    let program = PROGRAM.replace(
        "\"Default\"",
        "motion.play(\"sprite:iris\",motion_frames([\"one.png\"],1))",
    );
    let mut engine = Engine::new(parse(&program).unwrap(), Headless::default(), 8).unwrap();
    assert!(engine.step_until_interaction().is_err());
    assert!(engine.state.motions.tracks.is_empty());
}

#[test]
fn rejected_show_hide_and_move_do_not_partially_change_sprite_or_animation_state() {
    let definition = "layered_image([600,1000],{},[image_layer(\"body\",\"body.png\",{})])";
    for action in ["iris.show()", "iris.hide()", "iris.move() at right"] {
        let source=format!("label start\ncharacter.compose(\"iris\",{definition})\niris.show()\nmotion.play(\"sprite:iris\",motion_tween(2,{{}},{{\"opacity\":0.5}},\"linear\"))\n\"Before\"\n{action}\n\"After\"\n");
        let mut engine = Engine::new(parse(&source).unwrap(), Headless::default(), 8).unwrap();
        engine.step_until_interaction().unwrap();
        engine.advance_dialogue().unwrap();
        engine.renderer.reject = true;
        let before = serde_json::to_value(&engine.state).unwrap();
        assert!(engine.step_until_interaction().is_err(), "{action}");
        assert_eq!(
            serde_json::to_value(&engine.state).unwrap(),
            before,
            "{action}"
        );
    }
}
