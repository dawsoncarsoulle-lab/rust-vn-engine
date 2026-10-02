use rvn_core::ui::{evaluate_canvas, CanvasBudget, UiInput};
use rvn_core::{Engine, GameState, MusicState, Renderer, SpriteState};
use rvn_parser::{parse, Hotspot, Position, Transition, Value};
use rvn_ui::custom_canvas::{flatten, hit_test, CanvasFrame, CanvasPrimitive};
use rvn_ui::programmable::{Component, ScreenEventKind, ScreenView};
use std::collections::{BTreeMap, HashMap};

#[derive(Default)]
struct Headless {
    views: Vec<ScreenView>,
    unsupported: bool,
    fail_next_update: bool,
}
impl Renderer for Headless {
    fn supports_programmable_ui(&self) -> bool {
        true
    }
    fn supports_custom_canvas(&self) -> bool {
        !self.unsupported
    }
    fn update_interfaces(&mut self, views: &[ScreenView]) -> Result<(), String> {
        self.views = views.to_vec();
        if std::mem::take(&mut self.fail_next_update) {
            return Err("Simulated renderer failure after a partial update".into());
        }
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
    fn restore_screen(&mut self, _: &GameState) {}
    fn restore_audio(&mut self, _: &MusicState) {}
}

const PROGRAM: &str = r#"
function paint(state,props,frame) {
    return [canvas_rect([state["x"],0,40,40],props["color"],5),
        canvas_text("Score",[0,50],[1,1,1,1],18),
        canvas_group([10,20,2,2,0,0.5],[0,0,100,100],[canvas_hit("knob",[0,0,20,20])])]
}
function widget(id,color) {
    return component(id,"canvas",{"draw":"paint","props":{"color":color},"state":{"x":0,"ticks":0},"width":400,"height":200,"accessible_label":"Custom control","events":{"pointer_down":"drag","tick":"advance","pointer_cancel":"cancel"}},[])
}
screen custom() {return component("root","column",{},[widget("one",[1,0,0,1]),widget("two",[0,1,0,1])])}
handler drag(event) {
    local state=dict_set(event["state"],"x",event["value"]["x"])
    ui.set_state(event["screen"],event["element"],state)
}
handler advance(event) {
    ui.set_state(event["screen"],event["element"],dict_set(event["state"],"ticks",event["state"]["ticks"]+1))
    set reported_time=event["frame"]["time"]
}
handler cancel(event) {ui.set_state(event["screen"],event["element"],dict_set(event["state"],"x",0))}
init {set reported_time=0}
label start
ui.open("custom",[],true,0)
"Waiting"
"After"
"#;

fn engine(source: &str) -> Engine<Headless> {
    let mut game = Engine::new(parse(source).unwrap(), Headless::default(), 16).unwrap();
    game.step_until_interaction().unwrap();
    game
}
fn pointer(id: &str, x: i64) -> UiInput {
    UiInput {
        screen: "custom".into(),
        element: id.into(),
        kind: ScreenEventKind::PointerDown,
        value: Some(Value::Dict(BTreeMap::from([
            ("x".into(), Value::Int(x)),
            ("y".into(), Value::Int(0)),
        ]))),
        key: None,
    }
}
fn state<'a>(game: &'a Engine<Headless>, id: &str) -> &'a rvn_core::ui::CanvasState {
    &game.state.ui.screens[0].canvas_states[id]
}
fn snapshot(state: &GameState) -> serde_json::Value {
    serde_json::to_value(state).unwrap()
}

#[test]
fn local_instances_are_independent_and_drawing_is_pure_reconstructed_geometry() {
    let mut game = engine(PROGRAM);
    let before = game.state.clone();
    let views = game.interface_views().unwrap();
    assert_eq!(
        snapshot(&game.state),
        snapshot(&before),
        "Reading views must not mutate local state, time or RNG"
    );
    let one = views[0].root.find("one").unwrap();
    assert_eq!(one.canvas_frame.unwrap().width, 400.0);
    let drawing = one.drawing.as_ref().unwrap();
    assert_eq!(
        hit_test(&flatten(drawing).unwrap(), [20.0, 30.0]),
        Some("knob".into())
    );
    let encoded = serde_json::to_value(one).unwrap();
    assert!(encoded.get("drawing").is_none());
    assert!(encoded.get("canvas_frame").is_none());
    game.interface_event(pointer("one", 25)).unwrap();
    let Value::Dict(one) = &state(&game, "one").state else {
        panic!()
    };
    let Value::Dict(two) = &state(&game, "two").state else {
        panic!()
    };
    assert_eq!(one["x"], Value::Int(25));
    assert_eq!(two["x"], Value::Int(0));
    assert_eq!(game.state.random, before.random);
    for local in ["event", "state", "props", "frame"] {
        assert!(!game.state.vars.contains_key(local));
    }
}

#[test]
fn tick_save_load_rollback_and_capture_generations_restore_the_same_instance_state() {
    let mut game = engine(PROGRAM);
    let epoch = game.interface_epoch();
    game.interface_tick(0.1).unwrap();
    assert_eq!(state(&game, "one").elapsed, 0.1);
    let Value::Dict(local) = &state(&game, "one").state else {
        panic!()
    };
    assert_eq!(local["ticks"], Value::Int(1));
    let saved =
        rvn_core::save::SaveData::from_state(&game.state, 1, "start".into(), "canvas.rvn".into());
    assert_eq!(saved.format_version, 9);
    assert_eq!(
        game.interface_epoch(),
        epoch,
        "Frames do not invalidate a drag capture"
    );
    game.interface_event(pointer("one", 99)).unwrap();
    game.interface_tick(0.2).unwrap();
    assert!(game.rollback());
    assert_eq!(state(&game, "one").elapsed, 0.1);
    let Value::Dict(local) = &state(&game, "one").state else {
        panic!()
    };
    assert_eq!(local["x"], Value::Int(0));
    assert_eq!(local["ticks"], Value::Int(1));
    assert_ne!(game.interface_epoch(), epoch);
    let epoch = game.interface_epoch();
    game.load_data(serde_json::from_value(serde_json::to_value(&saved).unwrap()).unwrap())
        .unwrap();
    assert_ne!(game.interface_epoch(), epoch);
    assert_eq!(state(&game, "one").elapsed, 0.1);
    assert_eq!(
        game.fresh(Headless::default(), 16)
            .unwrap()
            .interface_epoch(),
        game.interface_epoch() + 1
    );
}

#[test]
fn old_save_defaults_are_hydrated_and_invalid_saved_state_is_atomic() {
    let mut game = engine(PROGRAM);
    let saved =
        rvn_core::save::SaveData::from_state(&game.state, 1, "start".into(), "canvas.rvn".into());
    let mut old = serde_json::to_value(&saved).unwrap();
    old["format_version"] = 8.into();
    for screen in old["ui"]["screens"].as_array_mut().unwrap() {
        screen.as_object_mut().unwrap().remove("canvas_states");
    }
    game.load_data(serde_json::from_value(old).unwrap())
        .unwrap();
    assert_eq!(state(&game, "one").elapsed, 0.0);
    assert_eq!(
        state(&game, "one").state,
        Value::Dict(BTreeMap::from([
            ("x".into(), Value::Int(0)),
            ("ticks".into(), Value::Int(0))
        ]))
    );
    let before = game.state.clone();
    let epoch = game.interface_epoch();
    let mut bad = saved.clone();
    bad.ui.screens[0]
        .canvas_states
        .get_mut("one")
        .unwrap()
        .state = Value::List(vec![]);
    assert!(game.load_data(bad).is_err());
    assert_eq!(snapshot(&game.state), snapshot(&before));
    assert_eq!(game.interface_epoch(), epoch);
    let mut bad = saved;
    bad.ui.screens[0]
        .canvas_states
        .get_mut("one")
        .unwrap()
        .elapsed = f64::INFINITY;
    assert!(game.load_data(bad).is_err());
    assert_eq!(snapshot(&game.state), snapshot(&before));
}

#[test]
fn invalid_drawing_or_event_commands_do_not_commit_globals_local_state_rng_or_history() {
    let source = r#"function paint(state,props,frame){return [canvas_rect([0,0,10,10],[1,0,0,1],radius)]}
screen custom(){return component("one","canvas",{"draw":"paint","state":{"x":0},"events":{"pointer_down":"bad"}},[])}
handler bad(event){set radius=-1 set roll=random(1,10) ui.set_state("custom","one",{"x":99})}
init{set radius=0}
label start
ui.open("custom",[],true,0)
"Waiting""#;
    let mut game = engine(source);
    let before = game.state.clone();
    let rendered = game.renderer.views.clone();
    assert!(game.interface_event(pointer("one", 0)).is_err());
    assert_eq!(snapshot(&game.state), snapshot(&before));
    assert_eq!(game.renderer.views, rendered);
    let source = PROGRAM.replace(
        "local state=dict_set(event[\"state\"],\"x\",event[\"value\"][\"x\"])",
        "local state=[]",
    );
    let mut game = engine(&source);
    let before = game.state.clone();
    assert!(game.interface_event(pointer("one", 10)).is_err());
    assert_eq!(snapshot(&game.state), snapshot(&before));
}

#[test]
fn cached_views_preserve_renderer_failure_recovery_and_rollback_after_a_successful_retry() {
    let mut game = engine(PROGRAM);
    let before = snapshot(&game.state);
    let rendered = game.interface_views().unwrap();
    let can_rollback = game.can_rollback();
    game.renderer.fail_next_update = true;
    assert!(game.interface_event(pointer("one", 99)).is_err());
    assert_eq!(snapshot(&game.state), before);
    assert_eq!(game.renderer.views, rendered);
    assert_eq!(game.interface_views().unwrap(), rendered);
    assert_eq!(game.can_rollback(), can_rollback);
    game.interface_event(pointer("one", 99)).unwrap();
    assert_ne!(game.interface_views().unwrap(), rendered);
    assert!(game.rollback());
    assert_eq!(snapshot(&game.state), before);
    assert_eq!(game.interface_views().unwrap(), rendered);
    assert_eq!(game.renderer.views, rendered);
}

#[test]
fn structural_focus_delivers_handlers_but_invalid_final_drawing_stays_atomic() {
    let source = r#"
function paint(state,props,frame){return [canvas_rect([0,0,10,10],[1,0,0,1],radius)]}
screen custom(){return component("one","canvas",{
    "draw":"paint","state":{"x":0},"events":{"pointer_down":"focus","focus":"focused"}},[])}
handler focus(event){set radius=new_radius ui.focus("custom","one")}
handler focused(event){set focus_calls=focus_calls+1 set roll=random(1,10)}
init{set radius=0 set new_radius=-1 set focus_calls=0}
label start
ui.open("custom",[],true,0)
"Waiting"
"#;
    let mut game = engine(source);
    let before = snapshot(&game.state);
    let rendered = game.renderer.views.clone();
    assert!(game.interface_event(pointer("one", 0)).is_err());
    assert_eq!(snapshot(&game.state), before);
    assert_eq!(game.renderer.views, rendered);
    game.state.vars.insert("new_radius".into(), Value::Int(1));
    game.interface_event(pointer("one", 0)).unwrap();
    assert_eq!(game.state.vars["focus_calls"], Value::Int(1));
    assert_eq!(game.state.ui.screens[0].focus.as_deref(), Some("one"));
    assert!(game.state.vars.contains_key("roll"));
    assert_ne!(game.interface_views().unwrap(), rendered);
}

#[test]
fn structural_tick_checks_refresh_after_callbacks_remove_a_target_or_add_a_modal() {
    let source = PROGRAM
        .replace("init {set reported_time=0}", "init {set reported_time=0 set calls=0}")
        .replace(
            "ui.set_state(event[\"screen\"],event[\"element\"],dict_set(event[\"state\"],\"ticks\",event[\"state\"][\"ticks\"]+1))",
            "set calls=calls+1 ui.close(\"custom\")",
        );
    let mut game = engine(&source);
    game.interface_tick(0.1).unwrap();
    assert_eq!(game.state.vars["calls"], Value::Int(1));
    assert!(game.state.ui.screens.is_empty());
    assert!(game.interface_views().unwrap().is_empty());

    let source = source
        .replace("ui.close(\"custom\")", "ui.open(\"overlay\",[],true,10)")
        .replace(
            "init {set reported_time=0 set calls=0}",
            "screen overlay(){return component(\"overlay\",\"text\",{\"text\":\"Pause\"},[])}\ninit {set reported_time=0 set calls=0}",
        );
    let mut game = engine(&source);
    game.interface_tick(0.1).unwrap();
    assert_eq!(game.state.vars["calls"], Value::Int(1));
    assert_eq!(state(&game, "one").elapsed, 0.1);
    assert_eq!(state(&game, "two").elapsed, 0.0);
}

#[test]
fn draw_random_input_unknown_functions_and_unbounded_callbacks_are_diagnosed() {
    for statement in [
        "return [canvas_rect([random(0,1),0,1,1],[1,1,1,1],0)]",
        "return [canvas_text(mouse_x(),[0,0],[1,1,1,1],12)]",
        "while true {set x=1}",
    ] {
        let source=format!("function paint(state,props,frame){{{statement}}}\nscreen custom(){{return component(\"one\",\"canvas\",{{\"draw\":\"paint\"}},[])}}\nlabel start\nui.open(\"custom\",[],true,0)\n\"Waiting\"");
        let mut game = Engine::new(parse(&source).unwrap(), Headless::default(), 16).unwrap();
        let random = game.state.random;
        assert!(game.step_until_interaction().is_err());
        assert!(game.state.ui.screens.is_empty());
        assert_eq!(game.state.random, random);
    }
    let mut game = Engine::new(
        parse(&PROGRAM.replace("\"draw\":\"paint\"", "\"draw\":\"missing\"")).unwrap(),
        Headless::default(),
        16,
    )
    .unwrap();
    assert!(game.step_until_interaction().is_err());
    assert!(game.state.ui.screens.is_empty());
}

#[test]
fn ticking_is_bounded_pauses_under_modal_or_hidden_ancestors_and_cancel_can_release_hidden_state() {
    let source=PROGRAM.replace("screen custom() {return component(\"root\",\"column\",{},", "screen custom() {return component(\"root\",\"column\",{\"visible\":shown},")
        .replace("init {set reported_time=0}","screen overlay(){return component(\"overlay\",\"text\",{\"text\":\"Pause\"},[])}\ninit {set reported_time=0 set shown=true}");
    let mut game = engine(&source);
    let before = game.state.clone();
    for delta in [-0.1, 0.251, f64::NAN, f64::INFINITY] {
        assert!(game.interface_tick(delta).is_err());
        assert_eq!(snapshot(&game.state), snapshot(&before));
    }
    game.interface_event(pointer("one", 40)).unwrap();
    game.state.vars.insert("shown".into(), Value::Bool(false));
    let before = game.state.clone();
    game.interface_tick(0.1).unwrap();
    assert_eq!(snapshot(&game.state), snapshot(&before));
    let mut cancel = pointer("one", 0);
    cancel.kind = ScreenEventKind::PointerCancel;
    cancel.value = None;
    game.interface_event(cancel).unwrap();
    let Value::Dict(local) = &state(&game, "one").state else {
        panic!()
    };
    assert_eq!(local["x"], Value::Int(0));
    let source=PROGRAM.replace("\"After\"","ui.open(\"overlay\",[],true,10)\n\"After\"")
        .replace("init {set reported_time=0}","screen overlay(){return component(\"overlay\",\"text\",{\"text\":\"Pause\",\"events\":{\"click\":\"dismiss\"}},[])}\nhandler dismiss(event){ui.close(\"overlay\")}\ninit {set reported_time=0}");
    let mut game = engine(&source);
    game.interface_tick(0.1).unwrap();
    game.advance_dialogue().unwrap();
    game.step_until_interaction().unwrap();
    let elapsed = state(&game, "one").elapsed;
    let before = game.state.clone();
    game.interface_tick(0.1).unwrap();
    assert_eq!(snapshot(&game.state), snapshot(&before));
    assert!(game.interface_event(pointer("one", 50)).is_err());
    game.interface_event(UiInput {
        screen: "overlay".into(),
        element: "overlay".into(),
        kind: ScreenEventKind::Click,
        value: None,
        key: None,
    })
    .unwrap();
    game.interface_tick(0.1).unwrap();
    assert_eq!(state(&game, "one").elapsed, elapsed + 0.1);
}

#[test]
fn unknown_handlers_renderer_capabilities_and_global_draw_budgets_fail_visibly() {
    let mut game = Engine::new(
        parse(PROGRAM).unwrap(),
        Headless {
            unsupported: true,
            ..Default::default()
        },
        16,
    )
    .unwrap();
    let message = game.step_until_interaction().unwrap_err().to_string();
    assert!(message.contains("canvas"), "{message}");
    assert!(game.state.ui.screens.is_empty());
    let mut game = Engine::new(
        parse(&PROGRAM.replace("\"pointer_down\":\"drag\"", "\"pointer_down\":\"missing\""))
            .unwrap(),
        Headless::default(),
        16,
    )
    .unwrap();
    assert!(game.step_until_interaction().is_err());
    let script =
        parse("function paint(state,props,frame){local i=0 while i<30000 {set i=i+1} return []}")
            .unwrap();
    let functions = rvn_core::eval::FunctionLibrary::from_script(&script).unwrap();
    let component =
        Component::parse(serde_json::json!({"id":"one","kind":"canvas","draw":"paint"})).unwrap();
    let mut budget = CanvasBudget::default();
    let frame = CanvasFrame {
        width: 200.0,
        height: 100.0,
        time: 0.0,
    };
    assert!(evaluate_canvas(
        &functions,
        &component,
        &Value::Dict(BTreeMap::new()),
        frame,
        &HashMap::new(),
        &mut budget
    )
    .is_err());
    let script =
        parse("function paint(state,props,frame){local i=0 while i<7000 {set i=i+1} return []}")
            .unwrap();
    let functions = rvn_core::eval::FunctionLibrary::from_script(&script).unwrap();
    let mut budget = CanvasBudget::default();
    assert!(evaluate_canvas(
        &functions,
        &component,
        &Value::Dict(BTreeMap::new()),
        frame,
        &HashMap::new(),
        &mut budget
    )
    .is_ok());
    assert!(
        evaluate_canvas(
            &functions,
            &component,
            &Value::Dict(BTreeMap::new()),
            frame,
            &HashMap::new(),
            &mut budget
        )
        .is_err(),
        "Two individually bounded callbacks must share one global budget"
    );
    let source = PROGRAM.replace(
        "dict_set(event[\"state\"],\"ticks\",event[\"state\"][\"ticks\"]+1)",
        "[]",
    );
    let mut game = engine(&source);
    let before = game.state.clone();
    assert!(game.interface_tick(0.1).is_err());
    assert_eq!(snapshot(&game.state), snapshot(&before));
    let source=PROGRAM.replace("ui.set_state(event[\"screen\"],event[\"element\"],dict_set(event[\"state\"],\"ticks\",event[\"state\"][\"ticks\"]+1))","local i=0 while i<7000 {set i=i+1}");
    let mut game = engine(&source);
    let before = game.state.clone();
    assert!(game.interface_tick(0.1).is_err());
    assert_eq!(
        snapshot(&game.state),
        snapshot(&before),
        "Tick budgets and clocks are atomic across components"
    );
}

#[test]
fn constructors_cover_all_portable_primitives_and_empty_canvas_is_supported() {
    let source = r#"function paint(state,props,frame){return [canvas_rect([0,0,20,20],[1,0,0,1],2),canvas_ellipse([0,0,20,10],[0,1,0,1]),canvas_line([[0,0],[10,10]],[0,0,1,1],2),canvas_polygon([[0,0],[10,0],[5,10]],[1,1,1,1]),canvas_text("Hello",[0,0],[1,1,1,1],12),canvas_image("sample.png",[0,0,20,20]),canvas_group([0,0,1,1,0,1],[],[canvas_hit("region",[0,0,10,10])])]}
screen custom(){return component("one","canvas",{"draw":"paint"},[])}
label start
ui.open("custom",[],true,0)
"Waiting""#;
    assert!(rvn_parser::validate_logic(&parse(source).unwrap(), false).is_empty());
    let game = engine(source);
    assert_eq!(
        game.interface_views().unwrap()[0]
            .root
            .drawing
            .as_ref()
            .unwrap()
            .primitive_count(),
        8
    );
    let game=engine("screen custom(){return component(\"empty\",\"canvas\",{},[])}\nlabel start\nui.open(\"custom\",[],true,0)\n\"Waiting\"");
    assert_eq!(
        game.interface_views().unwrap()[0]
            .root
            .drawing
            .as_ref()
            .unwrap()
            .primitives,
        Vec::<CanvasPrimitive>::new()
    );
}
