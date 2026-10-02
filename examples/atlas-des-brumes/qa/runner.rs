//! Independent behavioral checks for the authored Atlas example.
//! The renderer records calls, not pixels, sound or decoding; those require UI QA.
#![cfg(test)]

use std::{fs, path::{Path, PathBuf}};
use rvn_core::{Engine, GameState, Interaction, LoadCompatibility, MusicState, Renderer, SpriteState};
use rvn_core::ui::UiInput;
use rvn_parser::{parse, Hotspot, Position, Statement, Transition, Value};
use rvn_ui::programmable::{ScreenEventKind, ScreenView};

#[derive(Default)]
struct Headless {
    screens: Vec<ScreenView>,
    dialogues: Vec<String>,
    endings: Vec<String>,
}
impl Renderer for Headless {
    fn supports_programmable_ui(&self) -> bool { true }
    fn supports_custom_canvas(&self) -> bool { true }
    fn supports_composable_motion(&self) -> bool { true }
    fn supports_layered_characters(&self) -> bool { true }
    fn supports_video(&self) -> bool { true }
    fn supports_accessibility(&self) -> bool { true }
    fn update_interfaces(&mut self, views: &[ScreenView]) -> Result<(), String> { self.screens = views.to_vec(); Ok(()) }
    fn update_motions(&mut self, _: &[rvn_core::motion::MotionView]) -> Result<(), String> { Ok(()) }
    fn update_layered_characters(&mut self, _: &[rvn_core::composition::LayeredView]) -> Result<(), String> { Ok(()) }
    fn update_videos(&mut self, _: &[rvn_core::video::VideoView]) -> Result<(), String> { Ok(()) }
    fn update_accessibility(&mut self, _: &rvn_ui::accessibility::AccessibilitySettings) -> Result<(), String> { Ok(()) }
    fn accessibility_speech(&mut self, _: &rvn_ui::accessibility::SpeechRequest) -> Result<(), String> { Ok(()) }
    fn set_background(&mut self, _: &str, _: &Transition) {}
    fn show_sprite(&mut self, _: &str, _: Option<&str>, _: &Position, _: &Transition, _: Option<&SpriteState>) {}
    fn hide_sprite(&mut self, _: &str, _: &Transition, _: &SpriteState) {}
    fn move_sprite(&mut self, _: &str, _: &Position, _: &Transition, _: &SpriteState) {}
    fn show_dialogue(&mut self, _: Option<&str>, text: &str) { self.dialogues.push(text.into()); }
    fn unlock_ending(&mut self, id: &str) { self.endings.push(id.into()); }
    fn show_choice(&mut self, _: &[String]) -> usize { 0 }
    fn music_play(&mut self, _: &str, _: &Transition, _: Option<&str>) {}
    fn music_stop(&mut self, _: &Transition) {}
    fn music_set_volume(&mut self, _: f32) {}
    fn sfx_play(&mut self, _: &str, _: &Transition) {}
    fn sfx_stop(&mut self, _: &str, _: &Transition) {}
    fn show_imagemap(&mut self, _: &str, _: Option<&str>, _: &[Hotspot]) -> usize { 0 }
    fn restore_screen(&mut self, _: &GameState) {}
    fn restore_audio(&mut self, _: &MusicState) {}
}

fn input(screen: &str, element: &str, kind: ScreenEventKind, value: Option<Value>, key: Option<&str>) -> UiInput {
    UiInput { screen: screen.into(), element: element.into(), kind, value, key: key.map(str::to_owned) }
}
fn snapshot(game: &Engine<Headless>) -> serde_json::Value { serde_json::to_value(&game.state).unwrap() }
fn save_reload(game: &Engine<Headless>) -> Engine<Headless> {
    let save = rvn_core::save::SaveData::from_state(&game.state, 1, "Atlas QA".into(), "main.rvn".into());
    let serialized = serde_json::to_string(&save).unwrap();
    let mut restored = game.fresh(Headless::default(), 128).unwrap();
    assert_eq!(restored.load_data(serde_json::from_str(&serialized).unwrap()).unwrap(), LoadCompatibility::Verified);
    restored
}

const WAIT_PROGRAM: &str = r#"
// A finite invisible animation is a menu lifetime gate, not a timer plugin.
screen atlas() {
    return component("atlas_root", "panel", {}, [
        component("travel", "button", {"events":{"click":"travel"}}, []),
        component("cancel", "button", {"events":{"click":"cancel"}}, [])
    ])
}
handler travel(event) { set atlas_destination = "clairiere" ui.close("atlas") }
handler cancel(event) { ui.close("atlas") }
init { set atlas_destination = "" }
label start
ui.open("atlas", [], true, 5)
motion.play("ui:atlas/atlas_root", motion_pause(86400))
motion.wait("ui:atlas/atlas_root")
if atlas_destination == "clairiere" { jump clairiere }
"Atlas cancelled"
jump end
label clairiere
"Arrived without an extra narrative click"
jump end
label end
"Finished"
"#;

fn wait_game() -> Engine<Headless> {
    let mut game = Engine::new(parse(WAIT_PROGRAM).unwrap(), Headless::default(), 128).unwrap();
    assert_eq!(game.step_until_interaction().unwrap(), None);
    assert_eq!(game.state.ui.screens.len(), 1);
    assert!(game.state.motions.waiting.is_some());
    game
}

#[test]
fn modal_lifetime_gate_travels_on_the_next_clock_without_advancing_dialogue() {
    let mut game = wait_game();
    let pc = game.state.pc;
    game.interface_event(input("atlas", "travel", ScreenEventKind::Click, None, None)).unwrap();
    assert!(game.state.ui.screens.is_empty());
    assert!(game.state.motions.tracks.is_empty());
    assert_eq!(game.state.pc, pc, "closing the screen alone must not skip a narrative operation");
    let rng = game.state.random;
    game.tick_motions(0.0).unwrap();
    assert!(game.state.motions.waiting.is_none());
    assert_eq!(game.state.random, rng, "clocking a disappearing target consumed gameplay randomness");
    assert_eq!(game.step_until_interaction().unwrap(), Some(Interaction::Dialogue {
        character: None, text: "Arrived without an extra narrative click".into(),
    }));
}

#[test]
fn modal_lifetime_gate_restores_open_screen_clock_and_rollback_after_cancel() {
    let mut game = wait_game();
    game.tick_motions(4.25).unwrap();
    let before = snapshot(&game);
    let mut restored = save_reload(&game);
    assert_eq!(snapshot(&restored), before);
    assert_eq!(restored.interface_views().unwrap(), game.interface_views().unwrap());
    restored.interface_event(input("atlas", "cancel", ScreenEventKind::Activate, None, None)).unwrap();
    assert!(restored.state.ui.screens.is_empty());
    assert!(restored.rollback());
    assert_eq!(snapshot(&restored), before);
    restored.interface_event(input("atlas", "cancel", ScreenEventKind::Click, None, None)).unwrap();
    restored.tick_motions(1.0 / 60.0).unwrap();
    assert_eq!(restored.step_until_interaction().unwrap(), Some(Interaction::Dialogue {
        character: None, text: "Atlas cancelled".into(),
    }));
}

#[test]
fn modal_lifetime_gate_is_finite_and_not_a_promise_of_unlimited_menu_waiting() {
    let mut game = wait_game();
    for _ in 0..23 { game.tick_motions(3600.0).unwrap(); }
    game.tick_motions(3599.0).unwrap();
    assert!(game.state.motions.waiting.is_some());
    game.tick_motions(1.0).unwrap();
    assert!(game.state.motions.waiting.is_none());
    assert_eq!(game.state.ui.screens.len(), 1, "natural timeout does not close the menu");
    // The example must close the menu after wait as well, to handle this bound.
}

fn project() -> PathBuf { Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf() }
fn canonical_source() -> String { fs::read_to_string(project().join("main.rvn")).expect("Build the example's initial canonical main.rvn first") }
fn canonical_game() -> Engine<Headless> { Engine::new(parse(&canonical_source()).unwrap(), Headless::default(), 128).unwrap() }

// Asset-free test preconditions. The screens/functions/handlers themselves are
// loaded from canonical main.rvn, not copied into or faked by tests.
const MENU_PRECONDITIONS: &str = r#"
init {
    set first_name = "Camille" set last_name = "Brume" set origin = "rivage"
    set atlas_page = 1 set atlas_view = 1.0 set atlas_selected = 0 set atlas_marker = 0
    set atlas_location = "havre" set atlas_destination = "" set atlas_equipped = ""
    set atlas_notice = "" set atlas_quest = "QA : essayer les trois volets" set atlas_open = false set atlas_melody_played = false
    set atlas_health = 3 set atlas_max_health = 5 set atlas_gold = 15
    set atlas_quantities = [1,0,0,1,0,1,3,0,1,0,0,0,0,0,1,0,0,0]
    set atlas_reduced_motion = false set atlas_high_contrast = false
    set atlas_text_scale = 1.0 set atlas_self_voicing = false
}
label qa_menu
ui.open("atlas_hud", [], false, 0)
ui.open("atlas", [], true, 5)
motion.play("ui:atlas/atlas_root", motion_pause(86400))
motion.wait("ui:atlas/atlas_root")
ui.close("atlas")
"QA menu closed"
"#;

fn menu_probe() -> Engine<Headless> {
    let mut ast=parse(&canonical_source()).unwrap();
    ast.retain(|statement|matches!(statement,Statement::Function{..}|Statement::Screen{..}|Statement::Handler{..}));
    ast.extend(parse(MENU_PRECONDITIONS).unwrap());
    let mut game = Engine::new(ast, Headless::default(), 128).unwrap();
    assert_eq!(game.step_until_interaction().unwrap(), None);
    game
}
fn click(game: &mut Engine<Headless>, id: &str) {
    game.interface_event(input("atlas", id, ScreenEventKind::Click, None, None)).unwrap();
}
fn key(game: &mut Engine<Headless>, key: &str) {
    game.interface_event(input("atlas", "atlas_root", ScreenEventKind::Key, None, Some(key))).unwrap();
}
fn page(game: &mut Engine<Headless>, page: i64) {
    click(game, ["atlas_identity_tab", "atlas_inventory_tab", "atlas_map_tab"][page as usize]);
    game.interface_tick(0.25).unwrap();
    game.interface_tick(0.25).unwrap();
    assert_eq!(game.state.vars["atlas_page"], Value::Int(page));
    assert_eq!(game.state.vars["atlas_view"], Value::Float(page as f32));
}
fn surface_items(game: &Engine<Headless>) -> Vec<rvn_ui::custom_canvas::CanvasItem> {
    let views = game.interface_views().unwrap();
    let view = views.iter().find(|view| view.name == "atlas").unwrap();
    let surface = view.root.find("atlas_surface").unwrap();
    rvn_ui::custom_canvas::flatten(surface.drawing.as_ref().expect("real Canvas callback must produce drawing")).unwrap()
}
fn pointer(game: &mut Engine<Headless>, hit: &str, kind: ScreenEventKind) {
    use rvn_ui::custom_canvas::{CanvasPrimitive, transform_point, hit_test};
    let items = surface_items(game);
    let item = items.iter().rev().find(|item| matches!(&item.primitive, CanvasPrimitive::Hit { id, .. } if id == hit)).unwrap();
    let CanvasPrimitive::Hit { rect, .. } = item.primitive else { unreachable!() };
    let [x, y] = transform_point(item.matrix, [rect[0]+rect[2]*0.5, rect[1]+rect[3]*0.5]);
    assert_eq!(hit_test(&items, [x,y]).as_deref(), Some(hit), "requested target is obscured by another page");
    let value = Value::Dict(std::collections::BTreeMap::from([
        ("x".into(), Value::Float(x)), ("y".into(), Value::Float(y)), ("hit".into(), Value::Str(hit.into())),
        ("button".into(), Value::Str("left".into()))
    ]));
    game.interface_event(input("atlas", "atlas_surface", kind, Some(value), None)).unwrap();
}
fn number(game: &Engine<Headless>, name: &str) -> i64 {
    match game.state.vars.get(name) { Some(Value::Int(value)) => *value, value => panic!("{name}: expected integer, got {value:?}") }
}
fn quantity(game: &Engine<Headless>, index: usize) -> i64 {
    let Value::List(items) = &game.state.vars["atlas_quantities"] else { panic!("quantities must be a list") };
    let Value::Int(value) = items[index] else { panic!("quantity must be an integer") }; value
}

#[test]
fn atlas_module_renders_all_three_panels_within_real_canvas_computation_budgets() {
    let mut game = menu_probe();
    for active in [0,1,2,0] {
        page(&mut game, active);
        let items = surface_items(&game);
        assert!(items.len() > 100, "the authored panel was not drawn");
        assert!(items.iter().any(|item| matches!(&item.primitive, rvn_ui::custom_canvas::CanvasPrimitive::Image { asset, .. } if asset == "ui/frame.png")));
        assert!(game.interface_views().unwrap().iter().find(|view| view.name == "atlas").unwrap().modal);
        let views = game.interface_views().unwrap();
        let root = &views.iter().find(|view|view.name=="atlas").unwrap().root;
        assert_eq!(root.find("atlas_first_name").is_some(), active == 0);
        assert_eq!(root.find("atlas_confirm").is_some(), active == 1);
        assert_eq!(root.find("atlas_travel").is_some(), active == 2);
    }
    key(&mut game, "q");
    assert_eq!(number(&game,"atlas_page"), 2, "left from identity should wrap to map");
    key(&mut game, "e");
    assert_eq!(number(&game,"atlas_page"), 0);
}

#[test]
fn atlas_identity_and_accessibility_bindings_save_restore_and_do_not_navigate_while_typing() {
    let mut game = menu_probe();
    page(&mut game, 0);
    for (id, value) in [
        ("atlas_first_name", Value::Str("Éloïse Q".into())),
        ("atlas_last_name", Value::Str("Des Brumes".into())),
        ("atlas_origin", Value::Str("hautes_terres".into())),
        ("atlas_reduce_motion", Value::Bool(true)),
        ("atlas_high_contrast", Value::Bool(true)),
        ("atlas_self_voicing", Value::Bool(true)),
        ("atlas_text_scale", Value::Float(1.2)),
    ] {
        game.interface_event(input("atlas", id, ScreenEventKind::Change, Some(value), None)).unwrap();
    }
    game.interface_event(input("atlas", "atlas_first_name", ScreenEventKind::Key, None, Some("q"))).unwrap();
    assert_eq!(number(&game,"atlas_page"),0);
    assert_eq!(game.state.vars["first_name"],Value::Str("Éloïse Q".into()));
    assert_eq!(game.state.vars["origin"],Value::Str("hautes_terres".into()));
    assert!(game.state.accessibility.reduced_motion);
    assert!(game.state.accessibility.high_contrast);
    assert!(game.state.accessibility.self_voicing);
    assert!((game.state.accessibility.text_scale-1.2).abs()<0.001);
    let saved = snapshot(&game);
    let restored = save_reload(&game);
    assert_eq!(snapshot(&restored),saved);
    assert_eq!(restored.interface_views().unwrap(),game.interface_views().unwrap());
    game.interface_event(input("atlas", "atlas_first_name", ScreenEventKind::Change, Some(Value::Str("C".repeat(50))), None)).unwrap();
    assert_eq!(game.state.vars["first_name"],Value::Str("C".repeat(24)));
    assert!(game.rollback());
    assert_eq!(snapshot(&game),saved);
}

#[test]
fn atlas_inventory_real_hit_geometry_equips_uses_and_refuses_empty_or_full_health_actions() {
    let mut game = menu_probe();
    page(&mut game, 1);
    pointer(&mut game,"item_0",ScreenEventKind::PointerDown);
    click(&mut game,"atlas_confirm");
    assert_eq!(game.state.vars["atlas_equipped"],Value::Str("boussole".into()));
    pointer(&mut game,"item_1",ScreenEventKind::PointerDown);
    let before = game.state.vars["atlas_quantities"].clone();
    click(&mut game,"atlas_confirm");
    assert_eq!(game.state.vars["atlas_quantities"],before);
    assert_eq!(game.state.vars["atlas_equipped"],Value::Str("boussole".into()));
    assert_eq!(game.state.vars["atlas_notice"],Value::Str("Cet emplacement est vide.".into()));
    pointer(&mut game,"item_6",ScreenEventKind::PointerDown);
    for health in [4,5] { click(&mut game,"atlas_confirm"); assert_eq!(number(&game,"atlas_health"),health); }
    assert_eq!(quantity(&game,6),1);
    click(&mut game,"atlas_confirm");
    assert_eq!(quantity(&game,6),1, "a potion at full health must not be consumed");
    assert_eq!(game.state.vars["atlas_notice"],Value::Str("Votre vitalité est déjà complète.".into()));
    let before = snapshot(&game);
    pointer(&mut game,"item_5",ScreenEventKind::PointerDown);
    key(&mut game,"Enter");
    assert_eq!(number(&game,"atlas_page"),2);
    assert!(game.state.ui.screens.iter().any(|screen|screen.name=="atlas"), "using the map item must not travel in the same key event");
    assert_eq!(game.state.vars["atlas_destination"],Value::Str(String::new()));
    assert!(game.rollback());
    assert_eq!(number(&game,"atlas_page"),1);
    assert!(game.rollback());
    assert_eq!(snapshot(&game),before);
}

#[test]
fn atlas_map_true_marker_hit_tests_tooltip_locks_and_instant_travel_survive_save() {
    let mut game = menu_probe();
    page(&mut game,2);
    for locked in [1,3,4,5] {
        pointer(&mut game,&format!("marker_{locked}"),ScreenEventKind::PointerDown);
        click(&mut game,"atlas_travel");
        assert!(game.state.ui.screens.iter().any(|screen|screen.name=="atlas"));
        assert_eq!(game.state.vars["atlas_destination"],Value::Str(String::new()));
        assert_eq!(game.state.vars["atlas_location"],Value::Str("havre".into()));
        assert!(!matches!(&game.state.vars["atlas_notice"],Value::Str(text) if text.is_empty()));
    }
    pointer(&mut game,"marker_2",ScreenEventKind::PointerMove);
    let instance = game.state.ui.screens.iter().find(|screen|screen.name=="atlas").unwrap();
    let Value::Dict(state)=&instance.canvas_states["atlas_surface"].state else {panic!("local Canvas state")};
    assert_eq!(state["hover"],Value::Int(2));
    assert!(surface_items(&game).iter().any(|item|matches!(&item.primitive,rvn_ui::custom_canvas::CanvasPrimitive::Text{text,..} if text=="Clairière")));
    let before = snapshot(&game);
    let mut restored = save_reload(&game);
    assert_eq!(snapshot(&restored),before);
    restored.interface_event(input("atlas","atlas_surface",ScreenEventKind::PointerCancel,None,None)).unwrap();
    let instance = restored.state.ui.screens.iter().find(|screen|screen.name=="atlas").unwrap();
    let Value::Dict(state)=&instance.canvas_states["atlas_surface"].state else {panic!("local Canvas state")};
    assert_eq!(state["hover"],Value::Int(-1));
    pointer(&mut restored,"marker_2",ScreenEventKind::PointerDown);
    key(&mut restored,"Enter");
    assert_eq!(restored.state.vars["atlas_destination"],Value::Str("clairiere".into()));
    assert_eq!(restored.state.vars["atlas_location"],Value::Str("clairiere".into()));
    assert!(!restored.state.ui.screens.iter().any(|screen|screen.name=="atlas"));
    restored.tick_motions(0.0).unwrap();
    assert!(matches!(restored.step_until_interaction().unwrap(),Some(Interaction::Dialogue{text,..}) if text=="QA menu closed"));
}

#[test]
fn canonical_source_has_all_authoring_scopes_in_one_linkable_file() {
    let source = canonical_source();
    let ast = parse(&source).unwrap();
    assert!(!ast.iter().any(|statement| matches!(statement, Statement::Use { .. })), "Imported runtime files are not yet a cross-file visual workspace; main.rvn must be canonical and self-contained");
    rvn_graph::validate_project_script(&ast, false).unwrap();
    let workspace = rvn_graph::SourceProject::open(source, &[]).unwrap();
    for wanted in ["atlas", "atlas_hud"] {
        assert!(workspace.graphs().iter().any(|graph| matches!(&graph.kind, rvn_graph::GraphKind::Screen { name } if name == wanted)), "screen {wanted} missing from visual workspace");
    }
    for wanted in ["start", "explore", "atlas_visit_havre", "atlas_visit_clairiere", "atlas_visit_observatoire", "atlas_visit_archives", "atlas_visit_falaises", "atlas_visit_tour", "atlas_end_light", "atlas_end_mist"] {
        assert!(workspace.graphs().iter().any(|graph| matches!(&graph.kind, rvn_graph::GraphKind::Label { name } if name == wanted)), "narrative label {wanted} missing from visual workspace");
    }
}

#[test]
fn canonical_graph_roundtrips_preserve_comments_code_ids_and_existing_positions() {
    let source = canonical_source();
    let mut workspace = rvn_graph::SourceProject::open(source.clone(), &[]).unwrap();
    for round in 0..3 {
        let mut graphs = workspace.graphs().to_vec();
        let graph = graphs.iter_mut().find(|graph| matches!(&graph.kind, rvn_graph::GraphKind::Screen { name } if name == "atlas")).unwrap();
        let node = graph.nodes.values_mut().next().unwrap();
        node.position = [350.0 + round as f64 * 19.0, 210.0];
        workspace.apply_visual(&source, &graphs).unwrap();
        assert_eq!(workspace.source(), source, "a presentation edit regenerated or altered the author's code/comments");
        let reopened = rvn_graph::SourceProject::open(workspace.source(), workspace.graphs()).unwrap();
        assert_eq!(reopened.graphs(), workspace.graphs(), "stable node identities or placement were lost");
        workspace = reopened;
    }
    let before = workspace.graphs().to_vec();
    assert!(workspace.refresh(format!("{source}\nhandler unfinished(")).is_err());
    assert_eq!(workspace.graphs(), before, "invalid source destroyed the last valid graph");
    assert_eq!(workspace.source(), source);
    assert!(workspace.apply_visual(&format!("{source}\n// concurrent edit"), &before).is_err());
    assert_eq!(workspace.source(), source, "concurrent edits were silently overwritten");
    let _ = canonical_game(); // Definitions must also validate in the engine.
}

fn feature_game() -> Engine<Headless> {
    let mut ast = parse(&canonical_source()).unwrap();
    let index = ast.iter().position(|statement|matches!(statement,Statement::Label{..})).unwrap();
    let entry = parse("label __qa_feature_entry\nscene \"scenes/harbor.png\"\n\"__qa_enter\"\njump __qa_feature_done\n").unwrap();
    ast.splice(index..index,entry);
    ast.extend(parse("label __qa_feature_done\n\"__qa_done\"\n").unwrap());
    let mut game = Engine::new(ast,Headless::default(),128).unwrap();
    assert!(matches!(game.step_until_interaction().unwrap(),Some(Interaction::Dialogue{text,..}) if text=="__qa_enter"));
    game
}

fn drain_feature(game: &mut Engine<Headless>) {
    for _ in 0..128 {
        match game.step_until_interaction().unwrap() {
            Some(Interaction::Dialogue { text,.. }) if text=="__qa_done" => return,
            Some(Interaction::Dialogue { .. }) => game.advance_dialogue().unwrap(),
            Some(Interaction::Choice { .. }) => game.submit_choice(0).unwrap(),
            Some(Interaction::Imagemap { .. }) => panic!("feature unexpectedly requests a hotspot"),
            None if game.state.videos.waiting.is_some() => {
                // End feedback is a playback state contract, not a codec test.
                let id = game.state.videos.waiting.clone().unwrap();
                let epoch=game.video_views().unwrap().iter().find(|view|view.id==id).unwrap().epoch;
                assert!(game.video_feedback(epoch,&id,rvn_core::video::Feedback::End).unwrap());
            }
            None if game.state.motions.waiting.is_some() => game.tick_motions(10.0).unwrap(),
            None if game.state.call_stack.is_empty() && game.state.pc >= game.script.len() => return,
            None => panic!("feature stopped without a narrative interaction or a media wait"),
        }
    }
    panic!("feature exceeded 128 bounded narrative steps")
}

#[test]
fn canonical_crossing_requires_both_tools_and_has_a_true_reduced_motion_branch() {
    for (compass,lantern,ready) in [(false,false,false),(true,false,false),(false,true,false),(true,true,true)] {
        let mut game=feature_game();
        game.state.vars.insert("atlas_has_compass".into(),Value::Bool(compass));
        game.state.vars.insert("atlas_has_lantern".into(),Value::Bool(lantern));
        game.state.vars.insert("atlas_reduced_motion".into(),Value::Bool(true));
        game.execute_timer_action("call atlas_crossing").unwrap();
        drain_feature(&mut game);
        assert_eq!(game.state.vars["atlas_crossing_ready"],Value::Bool(ready));
        assert!(game.state.motions.tracks.is_empty());
        assert!(game.state.videos.tracks.is_empty());
    }
}

#[test]
fn canonical_full_crossing_saves_spline_custom_curve_bezier_frames_and_second_wait() {
    use rvn_ui::motion::{Motion,Easing};
    let mut game=feature_game();
    game.state.vars.insert("atlas_has_compass".into(),Value::Bool(true));
    game.state.vars.insert("atlas_has_lantern".into(),Value::Bool(true));
    game.execute_timer_action("call atlas_crossing").unwrap();
    assert_eq!(game.step_until_interaction().unwrap(),None);
    assert_eq!(game.state.motions.waiting.as_deref(),Some("sprite:lantern"));
    let Motion::Parallel {steps}=&game.state.motions.tracks["sprite:lantern"].definition else {panic!("parallel flight missing")};
    assert!(steps.iter().any(|motion|matches!(motion,Motion::Spline{curve:Easing::Samples{values},points,..} if values.len()==65 && points.len()==4)));
    assert!(steps.iter().any(|motion|matches!(motion,Motion::Tween{curve:Easing::Bezier{..},..})));
    assert!(steps.iter().any(|motion|matches!(motion,Motion::Repeat{times:Some(4),motion} if matches!(motion.as_ref(),Motion::Frames{images,..} if images.len()==2))));
    game.tick_motions(1.5).unwrap();
    let mut restored=save_reload(&game);
    assert_eq!(restored.state.motions,game.state.motions);
    assert_eq!(restored.state.random,game.state.random);
    assert_eq!(restored.state.motions.views().unwrap(),game.state.motions.views().unwrap());
    for game in [&mut game,&mut restored] {
        game.tick_motions(2.5).unwrap();
        assert_eq!(game.step_until_interaction().unwrap(),None);
        assert!(matches!(game.state.motions.tracks["sprite:lantern"].definition,Motion::Sequence{..}));
        assert_eq!(game.state.motions.waiting.as_deref(),Some("sprite:lantern"));
        drain_feature(game);
        assert_eq!(game.state.vars["atlas_crossing_ready"],Value::Bool(true));
        assert!(game.state.motions.tracks.is_empty());
        assert!(!game.state.sprites["lantern"].visible);
    }
    assert_eq!(snapshot(&game),snapshot(&restored));
}

#[test]
fn canonical_keeper_outfit_face_variant_rule_and_selector_survive_save_and_rollback() {
    let mut game=feature_game();
    game.state.vars.insert("atlas_keeper_trust".into(),Value::Int(2));
    game.state.vars.insert("atlas_has_lantern".into(),Value::Bool(true));
    game.state.vars.insert("atlas_reduced_motion".into(),Value::Bool(true));
    game.execute_timer_action("call atlas_keeper_outfit").unwrap();
    assert!(matches!(game.step_until_interaction().unwrap(),Some(Interaction::Dialogue{..})));
    let attributes=&game.state.layered.characters["keeper"].attributes;
    assert_eq!(attributes["outfit"],"");
    assert_eq!(attributes["face"],"neutral");
    game.advance_dialogue().unwrap();
    assert!(matches!(game.step_until_interaction().unwrap(),Some(Interaction::Dialogue{..})));
    let attributes=&game.state.layered.characters["keeper"].attributes;
    assert_eq!(attributes["outfit"],"travel");
    assert_eq!(attributes["face"],"smile");
    let saved=save_reload(&game);
    assert_eq!(saved.state.layered,game.state.layered);
    assert!(game.rollback());
    assert!(game.rollback());
    assert_eq!(game.state.layered.characters["keeper"].attributes["face"],"neutral");
    drain_feature(&mut game);
    game.execute_timer_action("call atlas_temple_vision").unwrap();
    drain_feature(&mut game);
    let attributes=&game.state.layered.characters["keeper"].attributes;
    assert_eq!(attributes["variant"],"evening");
    assert_eq!(attributes["outfit"],"travel");
    assert_eq!(attributes["face"],"smile");
    assert_eq!(attributes["accessory"],"lantern");
    assert_eq!(save_reload(&game).state.layered,game.state.layered);
}

#[derive(Clone,Copy)]
enum Route { SharedLight, Beacon }
fn choose_story(options: &[String], route: Route) -> usize {
    let preferences=match route {
        Route::SharedLight=>["Partager mes relevés", "Suivre la boussole", "Partager la lumière", "Rouvrir l’Atlas"],
        Route::Beacon=>["Négocier une récompense", "Tendre la corde", "Rallumer le phare", "Rouvrir l’Atlas"],
    };
    for preference in preferences {if let Some(index)=options.iter().position(|text|text.contains(preference)){return index;}}
    panic!("No expected authored story choice in {options:?}")
}
#[derive(Debug,PartialEq)]
enum Stop { Atlas, Dial }
fn pump_to_menu(game: &mut Engine<Headless>, route: Route) -> Stop {
    for _ in 0..512 {
        if game.state.ui.screens.iter().any(|screen|screen.name=="atlas") && game.state.motions.waiting.as_deref()==Some("ui:atlas/atlas_root") { return Stop::Atlas; }
        if game.state.ui.screens.iter().any(|screen|screen.name=="atlas_dial") && game.state.motions.waiting.as_deref()==Some("ui:atlas_dial/dial_root") { return Stop::Dial; }
        match game.step_until_interaction().unwrap() {
            Some(Interaction::Dialogue { .. })=>game.advance_dialogue().unwrap(),
            Some(Interaction::Choice { options })=>game.submit_choice(choose_story(&options,route)).unwrap(),
            Some(Interaction::Imagemap { .. })=>panic!("unexpected authored imagemap interaction"),
            None if game.state.videos.waiting.is_some()=>{
                let id=game.state.videos.waiting.clone().unwrap();
                let epoch=game.video_views().unwrap().iter().find(|view|view.id==id).unwrap().epoch;
                game.video_feedback(epoch,&id,rvn_core::video::Feedback::End).unwrap();
            }
            None if game.state.motions.waiting.is_some()=>{
                if game.state.ui.screens.iter().any(|screen|screen.name=="atlas"||screen.name=="atlas_dial"){continue;}
                game.tick_motions(10.0).unwrap();
            }
            None=>panic!("story ended unexpectedly at pc {} before reopening a menu",game.state.pc),
        }
    }
    panic!("story did not stop within 512 bounded narrative operations")
}
fn travel(game: &mut Engine<Headless>, marker: usize) {
    page(game,2);
    pointer(game,&format!("marker_{marker}"),ScreenEventKind::PointerDown);
    key(game,"Enter");
    let destination=["havre","observatoire","clairiere","archives","falaises","tour"][marker];
    assert_eq!(game.state.vars["atlas_destination"],Value::Str(destination.into()));
    assert!(!game.state.ui.screens.iter().any(|screen|screen.name=="atlas"));
    game.tick_motions(0.0).unwrap();
    assert!(game.state.motions.waiting.is_none());
}
fn equip(game: &mut Engine<Headless>, index: usize) {
    page(game,1);
    pointer(game,&format!("item_{index}"),ScreenEventKind::PointerDown);
    click(game,"atlas_confirm");
}
fn dial_key(game: &mut Engine<Headless>, code: &str) {
    let value=Value::Dict(std::collections::BTreeMap::from([
        ("code".into(),Value::Str(code.into())),("pressed".into(),Value::Bool(true))
    ]));
    game.interface_event(input("atlas_dial","dial_root",ScreenEventKind::Key,Some(value),Some(code))).unwrap();
}
fn solve_dial(game: &mut Engine<Headless>) {
    dial_key(game,"Enter");
    assert_eq!(game.state.vars["atlas_puzzle_solved"],Value::Bool(false));
    assert!(game.state.ui.screens.iter().any(|screen|screen.name=="atlas_dial"));
    let before=save_reload(game);
    assert_eq!(snapshot(&before),snapshot(game));
    for (code,turns) in [("Digit1",2),("Digit2",5),("Digit3",1)] {
        dial_key(game,code);
        for _ in 0..turns { dial_key(game,"ArrowRight"); }
    }
    let instance=game.state.ui.screens.iter().find(|screen|screen.name=="atlas_dial").unwrap();
    let Value::Dict(state)=&instance.canvas_states["dial_root"].state else{panic!("dial local state")};
    assert_eq!(state["rings"],Value::List(vec![Value::Int(2),Value::Int(5),Value::Int(1)]));
    dial_key(game,"Enter");
    assert_eq!(game.state.vars["atlas_puzzle_solved"],Value::Bool(true));
    assert!(!game.state.ui.screens.iter().any(|screen|screen.name=="atlas_dial"));
    game.tick_motions(0.0).unwrap();
}
fn seeded_story() -> Engine<Headless> {
    let mut game=canonical_game();
    game.state.random=rvn_core::random::RandomState::seeded(0xA71A5);
    game.state.display_random=rvn_core::random::RandomState::seeded(0xD15A1A7);
    game
}
fn play_route(route: Route) -> Engine<Headless> {
    let mut game=seeded_story();
    assert_eq!(pump_to_menu(&mut game,route),Stop::Atlas);
    assert_eq!(quantity(&game,1),0);
    assert_eq!(quantity(&game,2),0);
    travel(&mut game,0);
    assert_eq!(pump_to_menu(&mut game,route),Stop::Atlas);
    for index in [1,4,11,14] {assert_eq!(quantity(&game,index),1,"Havre reward missing at slot{index}");}
    assert_eq!(save_reload(&game).state.vars,game.state.vars);
    travel(&mut game,2);
    assert_eq!(pump_to_menu(&mut game,route),Stop::Atlas);
    for index in [2,10,17] {assert_eq!(quantity(&game,index),1,"Clairière reward missing at slot{index}");}
    assert_eq!(game.state.vars["atlas_crossing_ready"],Value::Bool(true));
    if matches!(route,Route::SharedLight) {
        equip(&mut game,10);
        assert_eq!(game.state.vars["atlas_melody_played"],Value::Bool(true));
        travel(&mut game,1);
        assert_eq!(pump_to_menu(&mut game,route),Stop::Dial);
        solve_dial(&mut game);
        assert_eq!(pump_to_menu(&mut game,route),Stop::Atlas);
        assert_eq!(quantity(&game,9),1);
        assert_eq!(quantity(&game,12),1);
    }
    travel(&mut game,3);
    assert_eq!(pump_to_menu(&mut game,route),Stop::Atlas);
    assert_eq!(quantity(&game,7),1);
    equip(&mut game,2);
    assert_eq!(game.state.vars["atlas_equipped"],Value::Str("grappin".into()));
    travel(&mut game,4);
    assert_eq!(pump_to_menu(&mut game,route),Stop::Atlas);
    assert_eq!(quantity(&game,13),2);
    travel(&mut game,5);
    assert_eq!(pump_to_menu(&mut game,route),Stop::Atlas);
    assert_eq!(game.state.vars["atlas_finished"],Value::Bool(true));
    assert_eq!(game.renderer.endings,vec![match route{Route::SharedLight=>"atlas_shared_light",Route::Beacon=>"atlas_beacon"}]);
    assert_eq!(save_reload(&game).state.vars,game.state.vars);
    game
}

#[test]
fn canonical_adventure_visits_all_six_locations_solves_real_dial_and_finishes_shared_light() {
    let game=play_route(Route::SharedLight);
    for name in ["atlas_seen_havre","atlas_seen_clairiere","atlas_seen_observatoire","atlas_seen_archives","atlas_seen_falaises"] {
        assert_eq!(game.state.vars[name],Value::Bool(true),"not actually visited: {name}");
    }
    assert!(number(&game,"atlas_keeper_trust")>=2);
    assert!(game.state.cinematic.current.is_none());
}

#[test]
fn canonical_second_ending_replays_deterministically_from_real_choices_without_observatory() {
    let game=play_route(Route::Beacon);
    let repeated=play_route(Route::Beacon);
    assert_eq!(game.state.vars["atlas_seen_observatoire"],Value::Bool(false));
    assert_eq!(game.state.vars["atlas_melody_played"],Value::Bool(false));
    assert!(number(&game,"atlas_keeper_trust")<2);
    assert_eq!(snapshot(&game),snapshot(&repeated));
}

#[test]
fn canonical_static_and_computed_asset_references_exist_in_the_project() {
    let ast=parse(&canonical_source()).unwrap();
    let mut resources=std::collections::BTreeSet::new();
    fn visit(value:&serde_json::Value,resources:&mut std::collections::BTreeSet<String>) {
        match value {
            serde_json::Value::String(value) if [".png",".wav",".webm",".ttf",".otf"].iter().any(|extension|value.ends_with(extension)) && value.contains('/')=>{resources.insert(value.clone());},
            serde_json::Value::Array(values)=>for value in values{visit(value,resources);},
            serde_json::Value::Object(values)=>for value in values.values(){visit(value,resources);},
            _=>{},
        }
    }
    visit(&serde_json::to_value(ast).unwrap(),&mut resources);
    let mut game=menu_probe();
    for active in [0,1,2] {
        page(&mut game,active);
        for item in surface_items(&game) {
            if let rvn_ui::custom_canvas::CanvasPrimitive::Image{asset,..}=item.primitive {resources.insert(asset);}
        }
    }
    assert!(resources.len()>=20,"not enough authored asset references were inspected: {resources:?}");
    for asset in resources {
        assert!(rvn_ui::programmable::safe_asset_path(&asset),"unsafe asset path: {asset}");
        let path=project().join("assets").join(&asset);
        assert!(path.is_file(),"missing required asset: {}",path.display());
        assert!(fs::metadata(path).unwrap().len()>0,"empty required asset: {asset}");
    }
}

#[test]
fn canonical_meaningful_visual_text_edit_changes_menu_behavior_and_roundtrips_back_to_rvn() {
    let source=canonical_source();
    let old="Une lanterne est nécessaire pour cette route.";
    let changed="QA : trouvez une lanterne au Havre.";
    let mut workspace=rvn_graph::SourceProject::open(source.clone(),&[]).unwrap();
    let mut graphs=workspace.graphs().to_vec();
    let function=graphs.iter_mut().find(|graph|matches!(&graph.kind,rvn_graph::GraphKind::Function{name} if name=="atlas_route_requirement")).unwrap();
    let before_positions:std::collections::BTreeMap<_,_>=function.nodes.iter().map(|(id,node)|(*id,node.position)).collect();
    let value=function.nodes.values_mut().find(|node|node.kind==rvn_graph::NodeKind::Literal && node.properties.get("value")==Some(&rvn_graph::PropertyValue::String(old.into()))).expect("text must be a typed, editable Blueprint constant");
    value.properties.insert("value".into(),rvn_graph::PropertyValue::String(changed.into()));
    workspace.apply_visual(&source,&graphs).unwrap();
    assert_eq!(workspace.source(),source.replace(old,changed),"editing one typed text value reformatted other RVN/comment text");
    let mut game=Engine::new(parse(workspace.source()).unwrap(),Headless::default(),128).unwrap();
    assert_eq!(pump_to_menu(&mut game,Route::SharedLight),Stop::Atlas);
    page(&mut game,2);
    pointer(&mut game,"marker_1",ScreenEventKind::PointerDown);
    assert!(surface_items(&game).iter().any(|item|matches!(&item.primitive,rvn_ui::custom_canvas::CanvasPrimitive::Text{text,..} if text.contains(changed))));
    click(&mut game,"atlas_travel");
    assert_eq!(game.state.vars["atlas_notice"],Value::Str(changed.into()));
    workspace.refresh(workspace.source().replace(changed,old)).unwrap();
    assert_eq!(workspace.source(),source);
    let function=workspace.graphs().iter().find(|graph|matches!(&graph.kind,rvn_graph::GraphKind::Function{name} if name=="atlas_route_requirement")).unwrap();
    assert_eq!(function.nodes.iter().map(|(id,node)|(*id,node.position)).collect::<std::collections::BTreeMap<_,_>>(),before_positions,"reverse RVN edit recreated or moved existing nodes");
}

#[test]
fn atlas_button_captions_are_centered_without_changing_native_hit_rectangles_or_actions() {
    use rvn_ui::programmable::{ComponentKind, layout_rects};
    let buttons = [
        ("atlas_identity_tab", [405.0,112.0,320.0,44.0], "atlas_tab"),
        ("atlas_inventory_tab", [800.0,112.0,320.0,44.0], "atlas_tab"),
        ("atlas_map_tab", [1195.0,112.0,320.0,44.0], "atlas_tab"),
        ("atlas_previous", [37.0,766.0,245.0,60.0], "atlas_step"),
        ("atlas_next", [1638.0,766.0,245.0,60.0], "atlas_step"),
        ("atlas_return", [1510.0,986.0,350.0,56.0], "atlas_close"),
        ("atlas_confirm", [1138.0,824.0,340.0,57.0], "atlas_confirm_item"),
        ("atlas_travel", [1138.0,824.0,340.0,57.0], "atlas_confirm_travel"),
    ];
    let mut game = menu_probe();
    for scale in [0.85_f32,1.0,1.25] {
        page(&mut game,0);
        game.interface_event(input("atlas","atlas_text_scale",ScreenEventKind::Change,Some(Value::Float(scale)),None)).unwrap();
        for active in [0,1,2] {
            page(&mut game,active);
            let views=game.interface_views().unwrap();
            let root=&views.iter().find(|view|view.name=="atlas").unwrap().root;
            let rects=layout_rects(root,[1920.0,1080.0]).unwrap();
            let focus=root.focus_order();
            for (id,rect,handler) in buttons {
                let Some(button)=root.find(id) else {
                    assert!(matches!(id,"atlas_confirm"|"atlas_travel"));
                    continue;
                };
                assert_eq!(button.kind,ComponentKind::Button);
                assert_eq!(button.rect,Some([0.0,0.0,rect[2],rect[3]]),"button must stay local to its tightly bounded presentation: {id}");
                let presentation=root.find(&format!("{id}_presentation")).unwrap();
                assert_eq!(presentation.rect,Some(rect),"presentation must never extend beyond the original button hit area: {id}");
                assert!(!presentation.is_control()&&!presentation.is_focusable());
                assert!(presentation.events.is_empty());
                assert!(button.text.is_empty(),"a second native label would be drawn: {id}");
                assert_eq!(button.events.get(&ScreenEventKind::Click).map(String::as_str),Some(handler));
                assert_eq!(button.events.get(&ScreenEventKind::Key).map(String::as_str),Some("atlas_button_key"));
                assert!(focus.iter().any(|target|target==id));
                let caption_id=format!("{id}_caption");
                let caption=root.find(&caption_id).unwrap();
                assert_eq!(caption.kind,ComponentKind::Text);
                assert!(!caption.is_control()&&!caption.is_focusable());
                assert!(caption.events.is_empty()&&caption.binding.is_none());
                assert_eq!(caption.background[3],0.0);
                assert!(!caption.auto_background);
                assert_eq!(button.accessible_label.as_deref(),Some(caption.text.as_str()));
                assert_eq!(button.accessible_label_key.as_deref(),caption.text_key.as_deref());
                assert!(!focus.contains(&caption_id));
                let button_rect=rects.iter().find(|item|item.id==id).unwrap().rect;
                let caption_rect=rects.iter().find(|item|item.id==caption_id).unwrap().rect;
                assert_eq!(button_rect,rect,"resolved button moved: {id}");
                assert_eq!(caption_rect[0],rect[0]);
                assert_eq!(caption_rect[2],rect[2]);
                assert!((caption_rect[1]+caption_rect[3]/2.0-(rect[1]+rect[3]/2.0)).abs()<0.002,"caption is not vertically centred: {id}, scale {scale}");
                assert!(caption_rect[1]>=rect[1]&&caption_rect[1]+caption_rect[3]<=rect[1]+rect[3],"caption exceeds native hit area: {id}");
                // The actual renderer only adds pointer interaction to controls,
                // Button and Canvas; the presentation Panel and Text pass hits.
                let point=[rect[0]+rect[2]/2.0,rect[1]+rect[3]/2.0];
                let mut targets=Vec::new();
                root.visit(&mut |node| {
                    if node.is_control()||matches!(node.kind,ComponentKind::Button|ComponentKind::Canvas) {
                        let r=rects.iter().find(|item|item.id==node.id).unwrap().rect;
                        if point[0]>=r[0]&&point[0]<=r[0]+r[2]&&point[1]>=r[1]&&point[1]<=r[1]+r[3] {targets.push(node.id.clone());}
                    }
                });
                assert_eq!(targets.last().map(String::as_str),Some(id),"caption masks the original pointer target: {id}");
            }
        }
    }
}
