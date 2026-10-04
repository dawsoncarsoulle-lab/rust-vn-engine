use rvn_core::ui::UiInput;
use rvn_core::{Engine, GameState, MusicState, Renderer, SpriteState};
use rvn_parser::{parse, Hotspot, Position, Transition, Value};
use rvn_ui::programmable::{ScreenEventKind, ScreenView};

#[derive(Default)]
struct Headless {
    views: Vec<ScreenView>,
}
impl Renderer for Headless {
    fn supports_programmable_ui(&self) -> bool {
        true
    }
    fn update_interfaces(&mut self, views: &[ScreenView]) -> Result<(), String> {
        self.views = views.to_vec();
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
function item_button(item) {
    return {"id": item, "kind": "button", "text": item, "events": {"click": "pick"}}
}
screen inventory(title) {
    local rows = [{"id":"title", "kind":"text", "text":title}]
    for item in items { set rows = list_append(rows, item_button(item)) }
    set rows = list_append(rows, {"id":"name", "kind":"input", "binding":"player_name"})
    return {"id":"inventory_root", "kind":"column", "children":rows}
}
screen journal() { return {"id":"journal_root", "kind":"text", "text":player_name} }
handler pick(event) {
    local chosen = event["element"]
    set selected = chosen
    ui.close("inventory")
}
init {
    set items = ["apple", "letter"]
    set selected = ""
    set player_name = "Camille"
}
label start
    ui.open("journal", [], false, 0)
    ui.open("inventory", ["Inventory"], true, 1)
    "Waiting"
"#;

fn input(element: &str, kind: ScreenEventKind, value: Option<Value>) -> UiInput {
    UiInput {
        screen: "inventory".into(),
        element: element.into(),
        kind,
        value,
        key: None,
    }
}

#[test]
fn imported_narrative_does_not_skip_caller_init_or_replay_it_on_step_load_and_rollback() {
    use std::{fs, path::PathBuf, time::{SystemTime, UNIX_EPOCH}};
    struct Fixture { root: PathBuf, temp: PathBuf }
    impl Drop for Fixture {
        fn drop(&mut self) {
            // Only the exclusively created test tree may be removed.
            let root = fs::canonicalize(&self.root).unwrap();
            assert_eq!(root.parent(), Some(self.temp.as_path()));
            assert!(root.file_name().unwrap().to_string_lossy().starts_with("rvn-core-init-imports-"));
            assert!(!fs::symlink_metadata(&self.root).unwrap().file_type().is_symlink());
            fs::remove_dir_all(root).unwrap();
        }
    }
    let temp = fs::canonicalize(std::env::temp_dir()).unwrap();
    let root = temp.join(format!("rvn-core-init-imports-{}-{}", std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
    fs::create_dir(&root).unwrap();
    let fixture = Fixture { root, temp };
    fs::write(fixture.root.join("chapitre.rvn"), r#"
init { set init_order = ["chapter"] set imported_score = 4 }
function imported_caption() { return "Été à Montréal 🍃" }
handler close_imported(event) {
    set qa_closed = qa_closed + 1
    ui.close("inventory")
}
screen inventory() {
    return component("root", "column", {}, [
        component("caption", "text", {"text":imported_caption()}, []),
        component("score", "text", {"text":"Score " + imported_score}, []),
        component("close", "button", {"text":"Fermer", "events":{"click":"close_imported"}}, [])
    ])
}
label conclusion_voisine
"Neighbor"
return
"#).unwrap();
    fs::write(fixture.root.join("main.rvn"), r#"
use "chapitre.rvn"
use "./chapitre.rvn"
init {
    set qa_closed = 0
    set init_order = list_append(init_order, "main")
    set observed_imported_score = imported_score
}
if false { init { set leaked_initializer = 1 } }
label start
set imported_score = imported_score + 1
ui.open_story("inventory", [], true, 20)
"Waiting"
"Closed [qa_closed] score [imported_score]"
call conclusion_voisine
"Done"
"#).unwrap();

    let parsed = rvn_parser::parse_file_with_uses(fixture.root.join("main.rvn")).unwrap();
    assert_eq!(parsed.iter().filter(|stmt| matches!(stmt, rvn_parser::Statement::Init { .. })).count(), 2);
    let caller_init = parsed.iter().rposition(|stmt| matches!(stmt, rvn_parser::Statement::Init { .. })).unwrap();
    assert!(parsed[..caller_init].iter().any(|stmt| matches!(stmt, rvn_parser::Statement::Dialogue { .. })));
    assert!(parsed[..caller_init].iter().any(|stmt| matches!(stmt, rvn_parser::Statement::Return)));
    let mut game = Engine::new(parsed, Headless::default(), 16).unwrap();
    let expected_order = Value::List(vec![Value::Str("chapter".into()), Value::Str("main".into())]);
    assert_eq!(game.state.vars.get("qa_closed"), Some(&Value::Int(0)));
    assert_eq!(game.state.vars["imported_score"], Value::Int(4));
    assert_eq!(game.state.vars["observed_imported_score"], Value::Int(4));
    assert_eq!(game.state.vars["init_order"], expected_order);
    assert!(!game.state.vars.contains_key("leaked_initializer"));
    assert!(game.history.is_empty());
    let start = game.script.iter().position(|stmt|
        matches!(stmt, rvn_parser::Statement::Label { name } if name == "start")).unwrap();
    // Standalone runtime applies start_label only after Engine::new has run Init.
    game.state.pc = start;
    assert!(matches!(game.step_until_interaction().unwrap(), Some(rvn_core::Interaction::Dialogue { text, .. }) if text == "Waiting"));
    let views = game.interface_views().unwrap();
    assert_eq!(views[0].root.find("caption").unwrap().text, "Été à Montréal 🍃");
    assert_eq!(views[0].root.find("score").unwrap().text, "Score 5");
    assert_eq!(game.state.vars["init_order"], expected_order);
    assert!(!game.state.vars.contains_key("leaked_initializer"));
    let before_click = serde_json::to_value(&game.state).unwrap();
    let history = game.history.len();
    game.interface_event(UiInput { screen: "inventory".into(), element: "close".into(),
        kind: ScreenEventKind::Click, value: None, key: None }).unwrap();
    assert_eq!(game.state.vars["qa_closed"], Value::Int(1));
    assert_eq!(game.state.vars["imported_score"], Value::Int(5));
    assert_eq!(game.state.vars["init_order"], expected_order);
    assert!(!game.state.vars.contains_key("leaked_initializer"));
    assert!(game.interface_views().unwrap().is_empty());
    assert_eq!(game.history.len(), history + 1);
    let saved = rvn_core::save::SaveData::from_state(&game.state, 1, "start".into(), "main.rvn".into());
    assert!(game.rollback());
    assert_eq!(serde_json::to_value(&game.state).unwrap(), before_click);
    assert_eq!(game.history.len(), history);
    game.load_data(saved).unwrap();
    assert_eq!(game.state.vars["qa_closed"], Value::Int(1));
    assert_eq!(game.state.vars["imported_score"], Value::Int(5));
    assert_eq!(game.state.vars["init_order"], expected_order);
    assert!(!game.state.vars.contains_key("leaked_initializer"));
    assert!(game.interface_views().unwrap().is_empty());
    game.advance_dialogue().unwrap();
    assert!(matches!(game.step_until_interaction().unwrap(), Some(rvn_core::Interaction::Dialogue { text, .. }) if text == "Closed 1 score 5"));
    game.advance_dialogue().unwrap();
    assert!(matches!(game.step_until_interaction().unwrap(), Some(rvn_core::Interaction::Dialogue { text, .. }) if text == "Neighbor"));
    game.advance_dialogue().unwrap();
    assert!(matches!(game.step_until_interaction().unwrap(), Some(rvn_core::Interaction::Dialogue { text, .. }) if text == "Done"));
    assert_eq!(game.state.vars["qa_closed"], Value::Int(1));
    assert_eq!(game.state.vars["init_order"], expected_order);
    assert!(!game.state.vars.contains_key("leaked_initializer"));

    let mut fresh = game.fresh(Headless::default(), 16).unwrap();
    assert_eq!(fresh.script, game.script);
    assert_eq!(fresh.state.vars["qa_closed"], Value::Int(0));
    assert_eq!(fresh.state.vars["imported_score"], Value::Int(4));
    assert_eq!(fresh.state.vars["init_order"], expected_order);
    assert!(!fresh.state.vars.contains_key("leaked_initializer"));
    // Encountering the already executed caller Init during ordinary stepping
    // must skip its body instead of appending "main" a second time.
    fresh.state.pc = caller_init;
    assert!(matches!(fresh.step_until_interaction().unwrap(), Some(rvn_core::Interaction::Dialogue { text, .. }) if text == "Waiting"));
    assert_eq!(fresh.state.vars["qa_closed"], Value::Int(0));
    assert_eq!(fresh.state.vars["imported_score"], Value::Int(5));
    assert_eq!(fresh.state.vars["init_order"], expected_order);
    assert!(!fresh.state.vars.contains_key("leaked_initializer"));
}

#[test]
fn function_initializers_remain_rejected_instead_of_becoming_startup_globals() {
    let source = "function helper() { init { set leaked = 1 } return 0 }\nlabel start\n\"Waiting\"\n";
    let error = parse(source).expect_err("Init must be rejected inside a function before Engine construction");
    assert_eq!(error.kind, rvn_parser::ParseErrorKind::UnexpectedToken {
        got: "Some(Init)".into(),
        expected: "fonction de calcul : set, if, while, for ou return <valeur> (sans opération narrative)",
    });
    assert_eq!(error.location, rvn_parser::SourceLocation { line: 1, col: 21, len: 4 });
    assert_eq!(error.source_line, source.lines().next().unwrap());
}

#[test]
fn oversized_events_are_rejected_without_mutating_state_or_rollback() {
    let mut game = Engine::new(parse(PROGRAM).unwrap(), Headless::default(), 16).unwrap();
    game.step_until_interaction().unwrap();
    let before = game.state.clone();
    let mut oversized = input("name", ScreenEventKind::Key, None);
    oversized.key = Some("a".repeat(257));
    assert!(game.interface_event(oversized).is_err());
    assert_eq!(
        serde_json::to_value(&game.state).unwrap(),
        serde_json::to_value(&before).unwrap()
    );
    assert!(game
        .interface_event(input(
            "name",
            ScreenEventKind::Change,
            Some(Value::Str("é".repeat(65537)))
        ))
        .is_err());
    assert_eq!(
        serde_json::to_value(&game.state).unwrap(),
        serde_json::to_value(&before).unwrap()
    );
    game.interface_event(input(
        "name",
        ScreenEventKind::Change,
        Some(Value::Str("Zoë".into())),
    ))
    .unwrap();
    assert!(game.rollback());
    assert_eq!(game.state.vars["player_name"], before.vars["player_name"]);
}

#[test]
fn repeated_components_bindings_modal_layers_events_save_and_rollback() {
    let mut game = Engine::new(parse(PROGRAM).unwrap(), Headless::default(), 16).unwrap();
    game.step_until_interaction().unwrap();
    assert_eq!(game.interface_views().unwrap().len(), 2);
    assert!(game.interface_is_modal());
    let original_random = game.state.random;
    assert!(game
        .interface_event(UiInput {
            screen: "journal".into(),
            element: "journal_root".into(),
            kind: ScreenEventKind::Click,
            value: None,
            key: None
        })
        .is_err());
    assert_eq!(game.state.random, original_random);
    game.interface_event(input(
        "name",
        ScreenEventKind::Change,
        Some(Value::Str("Éloïse".into())),
    ))
    .unwrap();
    assert_eq!(game.state.vars["player_name"], Value::Str("Éloïse".into()));
    assert_eq!(game.interface_views().unwrap()[0].root.text, "Éloïse");
    let saved = rvn_core::save::SaveData::from_state(
        &game.state,
        1,
        "start".into(),
        "interfaces.rvn".into(),
    );
    game.interface_event(input("letter", ScreenEventKind::Click, None))
        .unwrap();
    assert_eq!(game.state.vars["selected"], Value::Str("letter".into()));
    assert_eq!(game.state.ui.screens.len(), 1);
    for local in ["event", "chosen", "rows", "item", "title"] {
        assert!(!game.state.vars.contains_key(local), "leaked {local}");
    }
    assert!(game.rollback());
    assert_eq!(game.state.vars["selected"], Value::Str(String::new()));
    assert_eq!(game.state.ui.screens.len(), 2);
    assert_eq!(game.renderer.views.len(), 2);
    let mut fresh = game.fresh(Headless::default(), 16).unwrap();
    fresh
        .load_data(serde_json::from_str(&serde_json::to_string(&saved).unwrap()).unwrap())
        .unwrap();
    assert_eq!(fresh.state.vars["player_name"], Value::Str("Éloïse".into()));
    assert_eq!(
        fresh.interface_views().unwrap(),
        game.interface_views().unwrap()
    );
}

#[test]
fn failed_or_looping_handlers_and_lifecycle_events_are_atomic() {
    let script = r#"
screen puzzle() { return {"id":"try", "kind":"button", "events":{"click":"fail"}} }
handler fail(event) { set attempts = attempts + 1 while true { set attempts = attempts + 1 } }
init { set attempts = 0 }
label start
ui.open("puzzle", [], true, 0)
"Puzzle"
"#;
    let mut game = Engine::new(parse(script).unwrap(), Headless::default(), 8).unwrap();
    game.step_until_interaction().unwrap();
    let before = serde_json::to_string(&game.state).unwrap();
    assert!(game
        .interface_event(UiInput {
            screen: "puzzle".into(),
            element: "try".into(),
            kind: ScreenEventKind::Click,
            value: None,
            key: None
        })
        .is_err());
    assert_eq!(serde_json::to_string(&game.state).unwrap(), before);
    let recursive = r#"
screen puzzle() { return {"id":"root", "kind":"panel", "events":{"open":"again"}} }
handler again(event) { ui.open("puzzle", [], true, 0) }
label start
ui.open("puzzle", [], true, 0)
"#;
    let mut game = Engine::new(parse(recursive).unwrap(), Headless::default(), 8).unwrap();
    assert!(game.step_until_interaction().is_err());
    assert!(game.state.ui.screens.is_empty());
}

#[test]
fn unsupported_renderers_and_corrupt_saved_screen_state_are_not_silent() {
    let mut unsupported =
        Engine::new(parse(PROGRAM).unwrap(), rvn_core::TerminalRenderer, 8).unwrap();
    assert!(unsupported.step_until_interaction().is_err());
    assert!(unsupported.state.ui.screens.is_empty());
    let mut game = Engine::new(parse(PROGRAM).unwrap(), Headless::default(), 8).unwrap();
    game.step_until_interaction().unwrap();
    let before = serde_json::to_string(&game.state).unwrap();
    let mut saved = rvn_core::save::SaveData::from_state(
        &game.state,
        1,
        "start".into(),
        "interfaces.rvn".into(),
    );
    saved.ui.screens[0].name = "missing".into();
    assert!(game.load_data(saved).is_err());
    assert_eq!(serde_json::to_string(&game.state).unwrap(), before);
}

#[test]
fn localization_uses_keys_variables_and_current_language_after_load() {
    let script = r#"
screen greeting() { return component("welcome", "text", {"text":"Hello [name]", "text_key":"ui.welcome", "accessible_label":"Welcome", "accessible_label_key":"ui.label"}, []) }
init { set name = "Éloïse" }
label start
ui.open("greeting", [], false, 0)
"Waiting"
"#;
    let mut game = Engine::new(parse(script).unwrap(), Headless::default(), 8).unwrap();
    let table = |language: &str, text: &str, label: &str| {
        rvn_core::locale::LocaleTable::parse(
            language,
            &format!("[strings]\n\"ui.welcome\" = \"{text}\"\n\"ui.label\" = \"{label}\"\n"),
        )
        .unwrap()
    };
    game.locale = Some(rvn_core::locale::LocaleManager::from_tables(
        "unused",
        "en",
        "en",
        vec!["en".into(), "fr".into()],
        std::collections::HashMap::from([
            ("en".into(), table("en", "Hello [name]", "Welcome")),
            ("fr".into(), table("fr", "Bonjour [name]", "Bienvenue")),
        ]),
    ));
    game.step_until_interaction().unwrap();
    assert_eq!(game.interface_views().unwrap()[0].root.text, "Hello Éloïse");
    let saved =
        rvn_core::save::SaveData::from_state(&game.state, 1, "start".into(), "greeting.rvn".into());
    let random = game.state.random;
    game.locale.as_mut().unwrap().set_language("fr").unwrap();
    game.load_data(saved).unwrap();
    assert_eq!(game.renderer.views[0].root.text, "Bonjour Éloïse");
    assert_eq!(
        game.renderer.views[0].root.accessible_label.as_deref(),
        Some("Bienvenue")
    );
    game.refresh_interfaces().unwrap();
    assert_eq!(game.state.random, random);
}

#[test]
fn focus_handlers_have_rollback_but_unhandled_keys_do_not() {
    let script = r#"
screen input_form() { return component("name","input",{"value":"","events":{"focus":"focus"}},[]) }
handler focus(event) { set count = count + 1 }
init { set count = 0 }
label start
ui.open("input_form",[],true,0)
"Waiting"
"#;
    let mut game = Engine::new(parse(script).unwrap(), Headless::default(), 8).unwrap();
    game.step_until_interaction().unwrap();
    let send = |kind| UiInput {
        screen: "input_form".into(),
        element: "name".into(),
        kind,
        value: None,
        key: Some("ArrowLeft".into()),
    };
    game.interface_event(send(ScreenEventKind::Focus)).unwrap();
    game.interface_event(send(ScreenEventKind::Key)).unwrap();
    assert_eq!(game.state.vars["count"], Value::Int(1));
    assert!(game.rollback());
    assert_eq!(game.state.vars["count"], Value::Int(0));
    let before = serde_json::to_string(&game.state).unwrap();
    let mut saved =
        rvn_core::save::SaveData::from_state(&game.state, 1, "start".into(), "screen.rvn".into());
    saved.ui.screens[0]
        .values
        .insert("old".into(), Value::List(vec![]));
    assert!(game.load_data(saved).is_err());
    assert_eq!(serde_json::to_string(&game.state).unwrap(), before);
}

#[test]
fn selection_labels_translate_without_changing_saved_values() {
    let script = r#"screen reading() {return component("theme","select",{"options":["dark","light"],"option_keys":["ui.dark","ui.light"],"binding":"theme"},[])}
init {set theme = "dark"}
label start
ui.open("reading",[],false,0)
"Waiting""#;
    let mut game = Engine::new(parse(script).unwrap(), Headless::default(), 8).unwrap();
    let table = rvn_core::locale::LocaleTable::parse(
        "fr",
        "[strings]\n\"ui.dark\" = \"Sombre\"\n\"ui.light\" = \"Clair\"\n",
    )
    .unwrap();
    game.locale = Some(rvn_core::locale::LocaleManager::from_tables(
        "unused",
        "fr",
        "fr",
        vec!["fr".into()],
        std::collections::HashMap::from([("fr".into(), table)]),
    ));
    game.step_until_interaction().unwrap();
    let component = game.interface_views().unwrap().remove(0).root;
    assert_eq!(component.option_label("dark"), "Sombre");
    assert_eq!(component.options, ["dark", "light"]);
    game.interface_event(UiInput {
        screen: "reading".into(),
        element: "theme".into(),
        kind: ScreenEventKind::Change,
        value: Some(Value::Str("light".into())),
        key: None,
    })
    .unwrap();
    assert_eq!(game.state.vars["theme"], Value::Str("light".into()));
    let saved =
        rvn_core::save::SaveData::from_state(&game.state, 1, "start".into(), "main.rvn".into());
    game.load_data(saved).unwrap();
    assert_eq!(game.renderer.views[0].root.option_label("light"), "Clair");
}

#[test]
fn reusable_styles_and_event_data_survive_bindings_save_and_rollback() {
    let script = r#"
function base_style(){return {"padding":12,"radius":14,"border_width":2,"border_color":[0.1,0.6,0.9,1],"hover_background":[0.2,0.4,0.6,1],"font":"fonts/interface.ttf"}}
function item_card(item){return component(item["id"],"button",{"text":item["label"],"style":[base_style(),{"opacity":0.8}],"event_data":item,"events":{"click":"pick"}},[])}
screen inventory(){
 local children=[]
 for item in items {set children=list_append(children,item_card(item))}
 if show_name {set children=list_append(children,component("name","input",{"binding":"player_name","style":base_style()},[]))}
 return component("root","column",{"style":{"padding":20,"width":460,"spacing":12}},children)
}
handler pick(event){set selected=event["data"]["id"]}
init {set items=[{"id":"apple","label":"Apple","price":2}] set selected="" set player_name="Camille" set show_name=true}
label start
 ui.open("inventory",[],true,0)
 "Waiting"
"#;
    let mut game = Engine::new(parse(script).unwrap(), Headless::default(), 16).unwrap();
    game.step_until_interaction().unwrap();
    let original = game.interface_views().unwrap();
    let apple = original[0].root.find("apple").unwrap();
    assert_eq!(apple.radius, 14.0);
    assert_eq!(apple.opacity, 0.8);
    assert_eq!(apple.event_data["price"], serde_json::json!(2));
    assert_eq!(apple.font.as_deref(), Some("fonts/interface.ttf"));
    game.interface_event(input("apple", ScreenEventKind::Click, None))
        .unwrap();
    assert_eq!(game.state.vars["selected"], Value::Str("apple".into()));
    assert!(game.rollback());
    assert_eq!(game.state.vars["selected"], Value::Str("".into()));
    game.interface_event(input(
        "name",
        ScreenEventKind::Change,
        Some(Value::Str("Zoë".into())),
    ))
    .unwrap();
    let saved =
        rvn_core::save::SaveData::from_state(&game.state, 1, "start".into(), "styles.rvn".into());
    let mut fresh = game.fresh(Headless::default(), 16).unwrap();
    fresh
        .load_data(serde_json::from_str(&serde_json::to_string(&saved).unwrap()).unwrap())
        .unwrap();
    assert_eq!(
        fresh.interface_views().unwrap(),
        game.interface_views().unwrap()
    );
    assert_eq!(fresh.state.vars["player_name"], Value::Str("Zoë".into()));
    assert_eq!(
        rvn_ui::programmable::layout_rects(
            &fresh.interface_views().unwrap()[0].root,
            [1920.0, 1080.0]
        )
        .unwrap()
        .len(),
        3
    );
}
