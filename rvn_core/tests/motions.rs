use rvn_core::motion::MotionView;
use rvn_core::{Engine, Renderer, SpriteState};
use rvn_parser::{parse, Hotspot, Position, Transition};
#[derive(Default)]
struct Headless {
    poses: Vec<MotionView>,
    reject: bool,
}
impl Renderer for Headless {
    fn supports_composable_motion(&self) -> bool {
        true
    }
    fn supports_programmable_ui(&self) -> bool {
        true
    }
    fn update_interfaces(&mut self, _: &[rvn_ui::programmable::ScreenView]) -> Result<(), String> {
        Ok(())
    }
    fn update_motions(&mut self, poses: &[MotionView]) -> Result<(), String> {
        if self.reject {
            Err("test rejection".into())
        } else {
            self.poses = poses.to_vec();
            Ok(())
        }
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
function entrance(seconds) {
    return motion_sequence([
        motion_parallel([
            motion_tween(seconds, {}, {"x":100}, "linear"),
            motion_tween(seconds, {}, {"opacity":0.5}, "ease_in_out")
        ]),
        motion_pause(0.5)
    ])
}
label start
scene "test.png"
motion.play("background", entrance(2))
"First"
motion.wait("background")
"After wait"
motion.stop("background")
"Stopped"
"#;
fn game() -> Engine<Headless> {
    Engine::new(parse(PROGRAM).unwrap(), Headless::default(), 16).unwrap()
}

#[test]
fn clocks_replace_wait_save_load_and_rollback_preserve_exact_poses_and_rng() {
    let mut engine = game();
    engine.step_until_interaction().unwrap();
    let random = engine.state.random;
    engine.tick_motions(1.0).unwrap();
    assert_eq!(engine.renderer.poses[0].pose.x, 50.0);
    assert_eq!(engine.renderer.poses[0].pose.opacity, 0.75);
    assert_eq!(engine.state.random, random);
    engine.advance_dialogue().unwrap();
    engine.step_until_interaction().unwrap();
    assert!(engine.state.motions.waiting.is_some());
    let pc = engine.state.pc;
    engine.step_until_interaction().unwrap();
    assert_eq!(engine.state.pc, pc);
    let saved = rvn_core::save::SaveData::from_state(
        &engine.state,
        1,
        "Animation".into(),
        "test.rvn".into(),
    );
    assert_eq!(saved.pc, pc);
    engine.tick_motions(1.5).unwrap();
    assert!(engine.state.motions.waiting.is_none());
    engine.step_until_interaction().unwrap();
    assert_eq!(engine.renderer.poses[0].pose.x, 100.0);
    let mut loaded = game();
    loaded
        .load_data(serde_json::from_value(serde_json::to_value(saved).unwrap()).unwrap())
        .unwrap();
    assert_eq!(loaded.renderer.poses[0].pose.x, 50.0);
    assert!(loaded.state.motions.waiting.is_some());
    loaded.tick_motions(1.5).unwrap();
    loaded.step_until_interaction().unwrap();
    assert_eq!(loaded.renderer.poses, engine.renderer.poses);
    engine.advance_dialogue().unwrap();
    engine.step_until_interaction().unwrap();
    assert!(engine.renderer.poses.is_empty());
    assert!(engine.rollback());
    assert!(engine.renderer.poses.is_empty());
    assert!(engine.rollback());
    assert_eq!(engine.renderer.poses[0].pose.x, 100.0);
}

#[test]
fn invalid_definitions_capabilities_and_saved_clocks_fail_without_partial_state_changes() {
    let mut unsupported =
        Engine::new(parse(PROGRAM).unwrap(), rvn_core::TerminalRenderer, 16).unwrap();
    assert!(unsupported.step_until_interaction().is_err());
    assert!(unsupported.state.motions.tracks.is_empty());
    for source in [
        "motion_parallel([motion_tween(1,{}, {\"x\":1},\"linear\"),motion_tween(1,{}, {\"x\":2},\"linear\")])",
        "motion_tween(-1,{}, {\"x\":1},\"linear\")",
        "motion_frames([\"../private.png\"],12)",
    ] {
        let mut engine=Engine::new(parse(&format!("label start\nscene \"test.png\"\nmotion.play(\"background\",{source})")).unwrap(),Headless::default(),8).unwrap();
        assert!(engine.step_until_interaction().is_err());assert!(engine.state.motions.tracks.is_empty());
    }
    let mut engine = game();
    engine.step_until_interaction().unwrap();
    let before = serde_json::to_value(&engine.state).unwrap();
    let mut save = rvn_core::save::SaveData::from_state(&engine.state, 1, "".into(), "".into());
    save.motions.tracks.get_mut("background").unwrap().elapsed = -1.0;
    assert!(engine.load_data(save).is_err());
    assert_eq!(serde_json::to_value(&engine.state).unwrap(), before);
    engine.renderer.reject = true;
    assert!(engine.tick_motions(0.25).is_err());
    assert_eq!(serde_json::to_value(&engine.state).unwrap(), before);
    assert!(engine.tick_motions(f64::NAN).is_err());
    assert_eq!(serde_json::to_value(&engine.state).unwrap(), before);
}

#[test]
fn nonblocking_handlers_animate_interfaces_and_cancel_when_target_disappears() {
    let source = r#"
screen panel() {return component("root","button",{"text":"Animate","events":{"click":"animate"}},[])}
handler animate(event) {motion.play("ui:panel/root",motion_repeat(0,motion_tween(1,{}, {"rotation":360},"linear")))}
label start
ui.open("panel",[],false,0)
"Open"
ui.close("panel")
"Closed"
"#;
    let mut engine = Engine::new(parse(source).unwrap(), Headless::default(), 16).unwrap();
    engine.step_until_interaction().unwrap();
    engine
        .interface_event(rvn_core::ui::UiInput {
            screen: "panel".into(),
            element: "root".into(),
            kind: rvn_ui::programmable::ScreenEventKind::Click,
            value: None,
            key: None,
        })
        .unwrap();
    engine.tick_motions(0.5).unwrap();
    assert_eq!(engine.renderer.poses[0].pose.rotation, 180.0);
    assert!(engine.rollback());
    assert!(engine.state.motions.tracks.is_empty());
    engine
        .interface_event(rvn_core::ui::UiInput {
            screen: "panel".into(),
            element: "root".into(),
            kind: rvn_ui::programmable::ScreenEventKind::Click,
            value: None,
            key: None,
        })
        .unwrap();
    engine.advance_dialogue().unwrap();
    engine.step_until_interaction().unwrap();
    assert!(engine.state.motions.tracks.is_empty());
    assert!(Engine::new(
        parse("handler bad(event){motion.wait(\"background\")}").unwrap(),
        Headless::default(),
        8
    )
    .is_err());
}

#[test]
fn waiting_for_endless_motion_is_an_explicit_error_and_replacement_captures_current_pose() {
    let source="label start\nscene \"test.png\"\nmotion.play(\"background\",motion_repeat(0,motion_tween(1,{}, {\"x\":100},\"linear\")))\nmotion.wait(\"background\")";
    let mut engine = Engine::new(parse(source).unwrap(), Headless::default(), 8).unwrap();
    assert!(engine.step_until_interaction().is_err());
    assert!(engine.state.motions.waiting.is_none());
    let mut motions = rvn_core::motion::MotionState::default();
    let tween = |x| {
        rvn_ui::motion::Motion::parse(serde_json::json!({"kind":"tween","seconds":1,"to":{"x":x}}))
            .unwrap()
    };
    motions
        .play(rvn_core::motion::MotionTarget::Background, tween(100))
        .unwrap();
    motions.tick(0.5).unwrap();
    motions
        .play(rvn_core::motion::MotionTarget::Background, tween(150))
        .unwrap();
    motions.tick(0.5).unwrap();
    assert_eq!(motions.views().unwrap()[0].pose.x, 100.0);
}

#[test]
fn advanced_path_and_custom_curve_round_trip_mid_save_and_rollback() {
    let source = r#"
function warp(t){return t*t}
label start
scene "test.png"
motion.play("background",motion_spline(2,[{"x":0,"y":0},{"x":100,"y":-50},{"x":200,"y":0}],motion_curve("warp",65)))
"Moving"
motion.wait("background")
"Finished"
motion.stop("background")
"Stopped"
"#;
    let make = || Engine::new(parse(source).unwrap(), Headless::default(), 16).unwrap();
    let mut engine = make();
    engine.step_until_interaction().unwrap();
    engine.tick_motions(1.0).unwrap();
    let expected = engine.renderer.poses.clone();
    let saved =
        rvn_core::save::SaveData::from_state(&engine.state, 1, "Spline".into(), "main.rvn".into());
    let mut loaded = make();
    loaded
        .load_data(serde_json::from_value(serde_json::to_value(&saved).unwrap()).unwrap())
        .unwrap();
    assert_eq!(loaded.renderer.poses, expected);
    assert_eq!(loaded.state.random, engine.state.random);
    engine.advance_dialogue().unwrap();
    engine.step_until_interaction().unwrap();
    engine.tick_motions(1.0).unwrap();
    engine.step_until_interaction().unwrap();
    assert_eq!(engine.renderer.poses[0].pose.x, 200.0);
    engine.advance_dialogue().unwrap();
    engine.step_until_interaction().unwrap();
    assert!(engine.renderer.poses.is_empty());
    assert!(engine.rollback());
    assert!(engine.renderer.poses.is_empty());
    assert!(engine.rollback());
    assert_eq!(engine.renderer.poses[0].pose.x, 200.0);
    assert!(engine.rollback());
    assert_eq!(engine.renderer.poses[0].pose.x, 0.0);
}
