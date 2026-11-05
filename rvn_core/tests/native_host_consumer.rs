use rvn_core::{
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
    fn update_videos(&mut self, _: &[rvn_core::video::VideoView]) -> Result<(), String> {
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
const SOURCE: &str = r#"
init {set hits=0 set rolled=0 set output=[] set entry=__rvn_string_table_text_name(__rvn_literal_name("asset:347"),"hello")}
function sample(){local draw=random(1,100) return draw}
handler hit(event){
 flow.consume_v2("handler:hit","gather"){capture "roll"=sample() inspect "i"=entry
 set output=[__rvn_terminal_v1("i","valid"),__rvn_terminal_v1("i","table_id"),__rvn_terminal_v1("i","key")]}
 set hits=hits+1
}
handler renew(event){ui.open("panel",["renewed"],false,1)}
handler close_panel(event){ui.close("panel")}
handler bad(event){flow.consume_v2("handler:bad","gather"){inspect "i"=entry set hits=hits+100} ui.open("unknown",[],false,0)}
screen panel(marker){
 flow.consume_v2("screen:panel","gather"){capture "roll"=sample() inspect "i"=entry return component("root","column",{},[
 component("act","button",{"text":marker,"events":{"click":"hit"}},[]),
 component("renew","button",{"events":{"click":"renew"}},[]),
 component("close","button",{"events":{"click":"close_panel"}},[]),
 component("fail","button",{"events":{"click":"bad"}},[])])}
}
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
return
"#;
fn program() -> ParsedProgram {
    let mut p = rvn_parser::parse_program(SOURCE).unwrap();
    let path = Some(std::path::PathBuf::from("C:/private/host347.rvn"));
    p.sources[0].path = path.clone();
    for o in &mut p.origins {
        o.source = path.clone();
    }
    p
}
fn engine() -> Engine<Host> {
    let mut e = Engine::from_program_with_debugger(program(), Host::default(), 16).unwrap();
    e.state.random = RandomState::seeded(4);
    e.text_resources_mut()
        .publish(rvn_core::string_table::StringTableAsset {
            id: "asset:347".into(),
            namespace: "VN".into(),
            entries: [(
                "hello".into(),
                rvn_core::string_table::StringTableEntry {
                    source: "Hello".into(),
                    developer_notes: String::new(),
                    metadata: Default::default(),
                },
            )]
            .into(),
        })
        .unwrap();
    e
}
fn legacy_program() -> ParsedProgram {
    rvn_parser::parse_program(&SOURCE.replace("scene label", "label")).unwrap()
}
fn legacy_engine() -> Engine<Host> {
    let mut e = Engine::from_program_with_debugger(legacy_program(), Host::default(), 16).unwrap();
    e.state.random = RandomState::seeded(4);
    e.text_resources_mut()
        .publish(rvn_core::string_table::StringTableAsset {
            id: "asset:347".into(),
            namespace: "VN".into(),
            entries: [(
                "hello".into(),
                rvn_core::string_table::StringTableEntry {
                    source: "Hello".into(),
                    developer_notes: String::new(),
                    metadata: Default::default(),
                },
            )]
            .into(),
        })
        .unwrap();
    e
}
fn at_parent() -> Engine<Host> {
    let mut e = engine();
    e.step_until_interaction().unwrap();
    e
}
fn input(e: &Engine<Host>, element: &str) -> rvn_core::ui_input::ScopedUiInput {
    rvn_core::ui_input::ScopedUiInput {
        source: e.capture_ui_input_source("panel").unwrap(),
        event: rvn_core::ui::UiInput {
            screen: "panel".into(),
            element: element.into(),
            kind: rvn_ui::programmable::ScreenEventKind::Click,
            value: None,
            key: None,
        },
    }
}
fn state(e: &Engine<Host>) -> serde_json::Value {
    serde_json::to_value(&e.state).unwrap()
}
#[test]
fn native_description_is_once_per_real_projection_and_handler_retains_its_own_frame() {
    let mut e = at_parent();
    let screen = e.state.ui.screens[0].consumer_frames["screen:panel"].clone();
    assert_eq!(
        screen.owner.kind,
        rvn_core::consumer_schedule::HostDefinitionKind::Screen
    );
    assert!(screen.owner.target.incarnation > 0);
    let before = state(&e);
    for _ in 0..4 {
        e.interface_views().unwrap();
    }
    assert_eq!(state(&e), before);
    let event = input(&e, "act");
    assert!(e.interface_event_scoped(event).unwrap());
    assert_eq!(e.state.vars["hits"], Value::Int(1));
    assert_eq!(
        e.state.vars["output"],
        Value::List(vec![
            Value::Bool(true),
            Value::Name(rvn_parser::Name::new("asset:347").unwrap()),
            Value::Str("hello".into())
        ])
    );
    let handler = &e.state.ui.screens[0].consumer_frames["handler:hit"];
    assert_eq!(
        handler.owner.kind,
        rvn_core::consumer_schedule::HostDefinitionKind::Handler
    );
    assert_eq!(handler.owner.target, screen.owner.target);
    assert_ne!(handler.owner.definition, screen.owner.definition);
    e.state.vars.insert(
        "entry".into(),
        Value::Text(Box::new(
            rvn_parser::localized_text::DeferredText::invariant("literal"),
        )),
    );
    assert!(e.interface_event_scoped(input(&e, "act")).unwrap());
    assert_eq!(
        e.state.vars["output"],
        Value::List(vec![
            Value::Bool(false),
            Value::Name(rvn_parser::Name::new("asset:347").unwrap()),
            Value::Str("hello".into())
        ])
    );
}
#[test]
fn replacement_and_close_revoke_stale_events_and_do_not_reuse_frames() {
    let mut e = at_parent();
    assert!(e.interface_event_scoped(input(&e, "act")).unwrap());
    let old = input(&e, "act");
    let old_target = e.state.ui.screens[0].consumer_frames["screen:panel"]
        .owner
        .target
        .clone();
    assert!(e.interface_event_scoped(input(&e, "renew")).unwrap());
    assert!(!e.ui_input_source_current(&old.source));
    assert!(!e.interface_event_scoped(old).unwrap());
    let new = &e.state.ui.screens[0];
    assert_ne!(new.consumer_frames["screen:panel"].owner.target, old_target);
    assert!(!new.consumer_frames.contains_key("handler:hit"));
    assert!(e.interface_event_scoped(input(&e, "close")).unwrap());
    assert!(e.state.ui.screens.is_empty());
}
#[test]
fn invalid_commands_and_renderer_refusal_preserve_variables_rng_frames_and_publications() {
    let mut e = at_parent();
    let before = state(&e);
    let count = e.renderer.batches.lock().unwrap().len();
    assert!(e.interface_event_scoped(input(&e, "fail")).is_err());
    assert_eq!(state(&e), before);
    assert_eq!(e.renderer.batches.lock().unwrap().len(), count);
    e.renderer.reject.store(true, Ordering::SeqCst);
    assert!(e.interface_event_scoped(input(&e, "act")).is_err());
    assert_eq!(state(&e), before);
    assert_eq!(e.renderer.batches.lock().unwrap().len(), count);
}
#[test]
fn cold_save15_rebinds_real_host_frames_and_projection_without_gather_replay() {
    let mut e = at_parent();
    assert!(e.interface_event_scoped(input(&e, "act")).unwrap());
    let saved_random = e.state.random.clone();
    let saved_instance = e.state.ui.screens[0].clone();
    let save = e
        .capture_save15(&program(), None, 1, "real UI host".into())
        .unwrap();
    let bytes = save.encode().unwrap();
    assert!(bytes.contains("\"kind\":\"host\""));
    let mut cold = engine();
    let loaded = rvn_core::save::consumer_checkpoint::Save15::decode(&bytes).unwrap();
    cold.restore_save15(&program(), loaded).unwrap();
    assert_eq!(cold.state.random, saved_random);
    assert_eq!(cold.state.ui.screens[0], saved_instance);
    for _ in 0..4 {
        cold.interface_views().unwrap();
    }
    assert_eq!(cold.state.random, saved_random);
    assert!(cold.interface_event_scoped(input(&cold, "act")).unwrap());
    assert_eq!(cold.state.vars["hits"], Value::Int(2));
    let mut forged: serde_json::Value = serde_json::from_str(&bytes).unwrap();
    let frame = forged["story"]["frames"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|f| f["owner"]["kind"] == "host")
        .unwrap();
    frame["owner"]["target"]["incarnation"] = serde_json::json!(999999);
    let before = state(&cold);
    let epoch = cold.runtime_epoch();
    assert!(cold
        .restore_save15(
            &program(),
            rvn_core::save::consumer_checkpoint::Save15::decode(&forged.to_string()).unwrap()
        )
        .is_err());
    assert_eq!(state(&cold), before);
    assert_eq!(cold.runtime_epoch(), epoch);
}
fn finish(e: &mut Engine<Host>, session: &mut rvn_core::engine::DebugSession) {
    for _ in 0..128 {
        if e.debug_step(session, rvn_core::editor_debugger::StepKind::Continue)
            .unwrap()
            .0
            == rvn_core::engine::DebugProgress::Complete
        {
            return;
        }
    }
    panic!("real input continuation did not complete");
}
#[test]
fn legacy_named_screen_allocates_real_registry_and_cold_input_continuation() {
    let p = legacy_program();
    let mut e = legacy_engine();
    e.step_until_interaction().unwrap();
    assert!(e.state.scene_execution.is_none());
    let instance = &e.state.ui.screens[0];
    assert!(matches!(
        e.capture_ui_input_source("panel").unwrap().authority,
        rvn_core::ui_input::UiInputAuthority::Instance { .. }
    ));
    assert!(
        instance.consumer_frames["screen:panel"]
            .owner
            .target
            .incarnation
            > 0
    );
    let before = state(&e);
    let initial = input(&e, "act");
    let mut session = e
        .debug_begin(
            rvn_core::engine::DebugOperation::Input(initial.clone()),
            vec![],
        )
        .unwrap();
    assert_eq!(state(&e), before);
    let save = e
        .capture_save15(&p, Some(&session), 3, "Legacy native frame".into())
        .unwrap();
    finish(&mut e, &mut session);
    let expected = state(&e);
    let mut cold = legacy_engine();
    let mut resumed = cold
        .restore_save15(
            &p,
            rvn_core::save::consumer_checkpoint::Save15::decode(&save.encode().unwrap()).unwrap(),
        )
        .unwrap()
        .unwrap();
    assert!(!cold.ui_input_source_current(&initial.source));
    finish(&mut cold, &mut resumed);
    assert_eq!(state(&cold), expected);
    let old = input(&cold, "act");
    assert!(cold.interface_event_scoped(input(&cold, "renew")).unwrap());
    assert!(!cold.ui_input_source_current(&old.source));
    assert!(!cold.interface_event_scoped(old).unwrap());
    let before = state(&cold);
    let count = cold.renderer.batches.lock().unwrap().len();
    cold.renderer.reject.store(true, Ordering::SeqCst);
    assert!(cold.interface_event_scoped(input(&cold, "act")).is_err());
    assert_eq!(state(&cold), before);
    assert_eq!(cold.renderer.batches.lock().unwrap().len(), count);
}
#[test]
fn parked_parent_and_child_same_named_screen_have_distinct_frames_and_restore_exact_lifetimes() {
    let mut e = at_parent();
    assert!(e.interface_event_scoped(input(&e, "act")).unwrap());
    let parent = e.state.ui.screens[0].clone();
    let old = input(&e, "act");
    e.advance_dialogue().unwrap();
    e.step_until_interaction().unwrap();
    let child = e.state.ui.screens[0].clone();
    assert_ne!(
        parent.consumer_frames["screen:panel"].owner.target,
        child.consumer_frames["screen:panel"].owner.target
    );
    assert!(!child.consumer_frames.contains_key("handler:hit"));
    assert!(!e.ui_input_source_current(&old.source));
    assert!(rvn_core::ui_residency::resident_interface(
        &e.state,
        &parent.consumer_frames["screen:panel"].owner.target
    )
    .is_ok());
    assert!(e.interface_event_scoped(input(&e, "act")).unwrap());
    let save = e
        .capture_save15(&program(), None, 5, "parked parent and child".into())
        .unwrap();
    let mut cold = engine();
    cold.restore_save15(
        &program(),
        rvn_core::save::consumer_checkpoint::Save15::decode(&save.encode().unwrap()).unwrap(),
    )
    .unwrap();
    cold.advance_dialogue().unwrap();
    cold.step_until_interaction().unwrap();
    assert_eq!(
        cold.state.ui.screens[0].consumer_frames["handler:hit"],
        parent.consumer_frames["handler:hit"]
    );
    assert_eq!(
        cold.state.ui.screens[0].consumer_frames["screen:panel"],
        parent.consumer_frames["screen:panel"]
    );
    assert!(!cold.ui_input_source_current(&old.source));
    assert!(cold.interface_event_scoped(input(&cold, "act")).unwrap());
}
#[test]
fn source_definition_ordinals_come_from_real_included_program_and_foreign_receipt_is_refused() {
    let folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("fixtures/native-host-included");
    std::fs::create_dir_all(&folder).unwrap();
    let begin = SOURCE.find("handler hit").unwrap();
    let end = SOURCE.find("screen panel").unwrap();
    std::fs::write(folder.join("handlers.rvn"), &SOURCE[begin..end]).unwrap();
    std::fs::write(
        folder.join("main.rvn"),
        format!(
            "use \"handlers.rvn\"\n{}{}",
            &SOURCE[..begin],
            &SOURCE[end..]
        ),
    )
    .unwrap();
    let p = rvn_parser::parse_file_program_with_uses(folder.join("main.rvn")).unwrap();
    let receipt = rvn_core::consumer_schedule::ConsumerSourceBinding::from_program(&p).unwrap();
    let screen = &receipt.definitions["screen:panel"];
    let handler = &receipt.definitions["handler:hit"];
    assert_ne!(screen.source_ordinal, handler.source_ordinal);
    assert!(p.sources[handler.source_ordinal]
        .path
        .as_ref()
        .unwrap()
        .ends_with("handlers.rvn"));
    let mut forged = receipt.clone();
    forged
        .definitions
        .get_mut("handler:hit")
        .unwrap()
        .source_ordinal = screen.source_ordinal;
    let renderer = Host::default();
    let batches = renderer.batches.clone();
    assert!(Engine::from_program_with_consumer_binding(p.clone(), forged, renderer, 8).is_err());
    assert!(batches.lock().unwrap().is_empty());
    let mut e = Engine::from_program_with_consumer_binding(p, receipt, Host::default(), 8).unwrap();
    e.step_until_interaction().unwrap();
    assert!(e.interface_event_scoped(input(&e, "act")).unwrap());
    let instance = &e.state.ui.screens[0];
    assert_ne!(
        instance.consumer_frames["screen:panel"]
            .owner
            .source_ordinal,
        instance.consumer_frames["handler:hit"].owner.source_ordinal
    );
}
#[test]
fn projected_video_callback_uses_captured_interface_frame_and_unbound_media_refuses_atomically() {
    for projected in [true, false] {
        let target = if projected {
            r#", "target":"ui:panel/act""#
        } else {
            ""
        };
        let source=SOURCE.replace("\"parent\"\ncall C",&format!("video.play(\"clip\",video_clip(\"clip.webm\",{{\"on_end\":\"hit\"{target}}}))\n\"parent\"\ncall C"));
        let p = rvn_parser::parse_program(&source).unwrap();
        let mut e = Engine::from_program_with_debugger(p, Host::default(), 8).unwrap();
        e.step_until_interaction().unwrap();
        let view = e.renderer.batches.lock().unwrap().last().unwrap().videos[0].clone();
        let before = state(&e);
        let count = e.renderer.batches.lock().unwrap().len();
        let result = e.video_feedback_from_view(&view, rvn_core::video::Feedback::End);
        if projected {
            assert!(result.unwrap());
            let frame = &e.state.ui.screens[0].consumer_frames["handler:hit"];
            assert_eq!(
                frame.owner.target,
                e.state.ui.screens[0].consumer_frames["screen:panel"]
                    .owner
                    .target
            );
            assert_eq!(e.state.vars["hits"], Value::Int(1));
            assert!(!e
                .video_feedback_from_view(&view, rvn_core::video::Feedback::End)
                .unwrap());
        } else {
            assert!(result.is_err());
            assert_eq!(state(&e), before);
            assert_eq!(e.renderer.batches.lock().unwrap().len(), count);
        }
    }
}
#[test]
fn native_legacy_rollback_is_atomic_and_revokes_a_coincident_recreated_source() {
    let mut e = legacy_engine();
    e.step_until_interaction().unwrap();
    let initial = state(&e);
    assert!(e.interface_event_scoped(input(&e, "act")).unwrap());
    let old = input(&e, "act");
    let before = state(&e);
    let count = e.renderer.batches.lock().unwrap().len();
    e.renderer.reject.store(true, Ordering::SeqCst);
    assert!(!e.rollback());
    assert_eq!(state(&e), before);
    assert_eq!(e.renderer.batches.lock().unwrap().len(), count);
    e.renderer.reject.store(false, Ordering::SeqCst);
    assert!(e.rollback());
    assert_eq!(state(&e), initial);
    assert!(!e.ui_input_source_current(&old.source));
    assert!(!e.interface_event_scoped(old).unwrap());
    assert!(e.interface_event_scoped(input(&e, "act")).unwrap());
}
#[test]
fn native_legacy_inspection_uses_real_target_and_resource_token() {
    let mut e = legacy_engine();
    e.step_until_interaction().unwrap();
    let source = e.capture_ui_input_source("panel").unwrap();
    let rvn_core::ui_input::UiInputAuthority::Instance { token, .. } = &source.authority else {
        panic!("real lifetime token expected")
    };
    let inspection = rvn_core::editor_inspection::capture(
        &e.state,
        rvn_core::editor_inspection::FrameIdentity {
            instance: e.instance_id(),
            epoch: e.runtime_epoch(),
            sequence: 7,
        },
    );
    assert!(inspection.unavailable.is_none());
    assert!(inspection.has_interface("panel", source.order, source.host_role, Some(&token.target)));
    assert!(inspection.has_resource_token(token));
    let mut forged = source.clone();
    forged.order += 1;
    assert!(!e.ui_input_source_current(&forged));
    assert_eq!(
        rvn_core::ui_input::UiInputSource::from_wire(&source.to_wire().unwrap()).unwrap(),
        source
    );
}
#[test]
fn historical_non_native_state_and_save_wire_stay_exact() {
    let before: serde_json::Value =
        serde_json::from_str(include_str!("../../../frozen-old-wire-0342.private.json")).unwrap();
    let mut e = Engine::new(
        rvn_parser::parse("set x = 1\n\"done\"").unwrap(),
        rvn_core::TerminalRenderer,
        8,
    )
    .unwrap();
    e.state.random = RandomState::seeded(4);
    e.step_silent().unwrap();
    let mut dto =
        rvn_core::save::SaveData::try_from_state(&e.state, 1, "fixture".into(), "story.rvn".into())
            .unwrap();
    dto.timestamp = 0;
    assert_eq!(
        serde_json::to_string(&e.state).unwrap(),
        before["state_json"].as_str().unwrap()
    );
    assert_eq!(
        serde_json::to_string(&dto).unwrap(),
        before["save_json"].as_str().unwrap()
    );
    let old_ui = r#"{"screens":[{"name":"panel","arguments":[],"modal":false,"layer":0,"order":1,"focus":null,"values":{},"random":{"state":4},"canvas_states":{}}],"next_order":2}"#;
    let ui: rvn_core::ui::UiState = serde_json::from_str(old_ui).unwrap();
    assert_eq!(serde_json::to_string(&ui).unwrap(), old_ui);
    let old = r#"{"lineage":9,"screen":"panel","order":1,"host_role":null,"authority":{"kind":"legacy","interface_epoch":3}}"#;
    assert_eq!(
        rvn_core::ui_input::UiInputSource::from_wire(old)
            .unwrap()
            .to_wire()
            .unwrap(),
        old
    );
}
#[test]
fn duplicate_definition_name_remains_refused_and_shared_authored_scope_does_not_merge_frames() {
    use rvn_core::editor_debugger as w;
    let duplicate = SOURCE
        .replace("handler hit(event)", "handler panel(event)")
        .replace("\"click\":\"hit\"", "\"click\":\"panel\"");
    assert!(Engine::from_program(
        rvn_parser::parse_program(&duplicate).unwrap(),
        Host::default(),
        8
    )
    .is_err());
    let source = SOURCE
        .replace("\"handler:hit\"", "\"shared\"")
        .replace("\"screen:panel\"", "\"shared\"");
    let p = rvn_parser::parse_program(&source).unwrap();
    let receipt = rvn_core::consumer_schedule::ConsumerSourceBinding::from_program(&p).unwrap();
    assert_eq!(
        receipt.definitions["screen:panel"].kind,
        rvn_core::consumer_schedule::HostDefinitionKind::Screen
    );
    assert_eq!(
        receipt.definitions["handler:hit"].kind,
        rvn_core::consumer_schedule::HostDefinitionKind::Handler
    );
    let mut e = Engine::from_program_with_debugger(p.clone(), Host::default(), 8).unwrap();
    e.step_until_interaction().unwrap();
    let prepared = rvn_core::program::PreparedNarrative::from_parsed(p.clone()).unwrap();
    let pc = prepared
        .script
        .iter()
        .position(|s| matches!(s,rvn_parser::Statement::Function{name,..}if name=="sample"))
        .unwrap();
    let bp = w::Breakpoint {
        position: prepared.position(pc),
        site: Some(w::Site {
            function: Some("sample".into()),
            route: vec![w::Route::Statement(1)],
            depth: 2,
            statement: true,
        }),
        enabled: true,
    };
    let mut session = e
        .debug_begin(
            rvn_core::engine::DebugOperation::Input(input(&e, "act")),
            vec![bp],
        )
        .unwrap();
    assert_eq!(
        e.debug_step(&mut session, w::StepKind::Continue).unwrap().0,
        rvn_core::engine::DebugProgress::Stopped
    );
    assert_eq!(
        e.debug_step(&mut session, w::StepKind::Continue).unwrap().0,
        rvn_core::engine::DebugProgress::Stopped
    );
    let save = e
        .capture_save15(
            &p,
            Some(&session),
            6,
            "same definition name/scope, Screen refresh".into(),
        )
        .unwrap();
    finish(&mut e, &mut session);
    let expected = state(&e);
    assert!(e.state.ui.screens[0]
        .consumer_frames
        .contains_key("screen:panel"));
    assert!(e.state.ui.screens[0]
        .consumer_frames
        .contains_key("handler:hit"));
    let mut cold = Engine::from_program_with_debugger(p.clone(), Host::default(), 8).unwrap();
    let mut resumed = cold
        .restore_save15(
            &p,
            rvn_core::save::consumer_checkpoint::Save15::decode(&save.encode().unwrap()).unwrap(),
        )
        .unwrap()
        .unwrap();
    finish(&mut cold, &mut resumed);
    assert_eq!(state(&cold), expected);
}
#[test]
fn native_open_and_closed_receipt_route_real_lifecycle_without_frame_leak() {
    for legacy in [false, true] {
        let source=SOURCE.replace("set hits=0","set opens=0 set closes=0 set hits=0").replace("screen panel(marker)",r#"handler opened(event){flow.consume_v2("opened","count"){inspect "i"=entry set opens=opens+1}}
handler closed(event){flow.consume_v2("closed","count"){inspect "i"=entry set closes=closes+1}}
screen panel(marker)"#).replace(r#"component("root","column",{},["#,r#"component("root","column",{"events":{"open":"opened","close":"closed"}},["#);
        let source = if legacy {
            source.replace("scene label", "label")
        } else {
            source
        };
        let p = rvn_parser::parse_program(&source).unwrap();
        let mut e = Engine::from_program_with_debugger(p, Host::default(), 8).unwrap();
        e.step_until_interaction().unwrap();
        assert_eq!(e.state.vars["opens"], Value::Int(1));
        assert_eq!(e.state.vars["closes"], Value::Int(0));
        assert_eq!(
            e.state.ui.screens[0].consumer_frames["handler:opened"]
                .owner
                .target,
            e.state.ui.screens[0].consumer_frames["screen:panel"]
                .owner
                .target
        );
        let old = input(&e, "act");
        assert!(e.interface_event_scoped(input(&e, "close")).unwrap());
        assert_eq!(e.state.vars["closes"], Value::Int(1));
        assert!(e.state.ui.screens.is_empty());
        assert!(!e.ui_input_source_current(&old.source));
    }
}
#[test]
fn native_canvas_tick_uses_actual_instance_and_shared_frame_budget() {
    for legacy in [false, true] {
        let source=SOURCE.replace("screen panel(marker)",r#"function paint(state,props,frame){return [canvas_rect([0,0,10,10],[1,0,0,1],0)]}
screen panel(marker)"#).replace(r#"component("act","button""#,r#"component("clock","canvas",{"draw":"paint","state":{"ticks":0},"events":{"tick":"hit"}},[]), component("act","button""#);
        let source = if legacy {
            source.replace("scene label", "label")
        } else {
            source
        };
        let mut e = Engine::from_program_with_debugger(
            rvn_parser::parse_program(&source).unwrap(),
            Host::default(),
            8,
        )
        .unwrap();
        e.step_until_interaction().unwrap();
        if legacy {
            e.interface_tick(0.1).unwrap();
        } else {
            e.tick_game_frame(0.1, Default::default()).unwrap();
        };
        assert_eq!(e.state.vars["hits"], Value::Int(1));
        assert_eq!(
            e.state.ui.screens[0].consumer_frames["handler:hit"]
                .owner
                .target,
            e.state.ui.screens[0].consumer_frames["screen:panel"]
                .owner
                .target
        );
        if legacy {
            e.interface_tick(0.1).unwrap();
        } else {
            e.tick_game_frame(0.1, Default::default()).unwrap();
        };
        assert_eq!(e.state.vars["hits"], Value::Int(2));
    }
}
#[test]
fn native_legacy_tick_does_not_deliver_old_second_component_to_reopened_alias() {
    let source=SOURCE.replace("scene label","label").replace("screen panel(marker)",r#"function paint(state,props,frame){return [canvas_rect([0,0,10,10],[1,0,0,1],0)]}
screen panel(marker)"#).replace(r#"component("act","button""#,r#"component("first","canvas",{"draw":"paint","state":{},"events":{"tick":"renew"}},[]),component("second","canvas",{"draw":"paint","state":{},"events":{"tick":"hit"}},[]),component("act","button""#);
    let mut e = Engine::from_program(
        rvn_parser::parse_program(&source).unwrap(),
        Host::default(),
        8,
    )
    .unwrap();
    e.step_until_interaction().unwrap();
    let old = e.state.ui.screens[0].consumer_frames["screen:panel"]
        .owner
        .target
        .clone();
    e.interface_tick(0.1).unwrap();
    assert_ne!(
        e.state.ui.screens[0].consumer_frames["screen:panel"]
            .owner
            .target,
        old
    );
    assert_eq!(e.state.vars["hits"], Value::Int(0));
    assert!(!e.state.ui.screens[0]
        .consumer_frames
        .contains_key("handler:hit"));
}
#[test]
fn host_save15_actual_slot_roundtrip_and_wrong_kind_refusal_preserve_live_state() {
    let mut e = at_parent();
    assert!(e.interface_event_scoped(input(&e, "act")).unwrap());
    let save = e
        .capture_save15(&program(), None, 7, "Host disk checkpoint".into())
        .unwrap();
    let folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("fixtures/private-host-save15");
    let manager = rvn_core::save::SaveManager::new(&folder, 10).unwrap();
    manager.save15_slot(&save).unwrap();
    let loaded = manager.load15_slot(7).unwrap();
    assert_eq!(
        serde_json::to_value(&loaded).unwrap(),
        serde_json::to_value(&save).unwrap()
    );
    let mut cold = engine();
    cold.restore_save15(&program(), loaded).unwrap();
    assert_eq!(cold.state.ui.screens[0], e.state.ui.screens[0]);
    let mut forged = serde_json::to_value(&save).unwrap();
    forged["story"]["projections"][0]["owner"]["definition_kind"] = "handler".into();
    let before = state(&cold);
    let epoch = cold.runtime_epoch();
    let count = cold.renderer.batches.lock().unwrap().len();
    assert!(cold
        .restore_save15(
            &program(),
            rvn_core::save::consumer_checkpoint::Save15::decode(&forged.to_string()).unwrap()
        )
        .is_err());
    assert_eq!(state(&cold), before);
    assert_eq!(cold.runtime_epoch(), epoch);
    assert_eq!(cold.renderer.batches.lock().unwrap().len(), count);
}

#[test]
fn untrusted_script_constructor_still_refuses_host_plans_before_init_or_publication() {
    for source in [
        include_str!("fixtures/consume_v2_unowned_screen.rvn"),
        include_str!("fixtures/consume_v2_unowned_handler.rvn"),
    ] {
        let host = Host::default();
        let count = host.batches.clone();
        let error = match Engine::new(rvn_parser::parse(source).unwrap(), host, 8) {
            Err(error) => error,
            Ok(_) => panic!("an AST/declaration cannot authenticate an instance"),
        };
        assert!(error.to_string().contains("TargetIdentity"));
        assert!(count.lock().unwrap().is_empty());
    }
}
#[test]
fn stopped_real_input_with_called_function_roundtrips_cold_save15() {
    use rvn_core::editor_debugger as w;
    let mut e = at_parent();
    let p = program();
    let prepared = rvn_core::program::PreparedNarrative::from_parsed(p.clone()).unwrap();
    let pc = prepared
        .script
        .iter()
        .position(|s| matches!(s,rvn_parser::Statement::Function{name,..}if name=="sample"))
        .unwrap();
    let bp = w::Breakpoint {
        position: prepared.position(pc),
        site: Some(w::Site {
            function: Some("sample".into()),
            route: vec![w::Route::Statement(1)],
            depth: 2,
            statement: true,
        }),
        enabled: true,
    };
    let initial = state(&e);
    let old = input(&e, "act");
    let mut session = e
        .debug_begin(
            rvn_core::engine::DebugOperation::Input(old.clone()),
            vec![bp],
        )
        .unwrap();
    assert_eq!(
        e.debug_step(&mut session, w::StepKind::Continue).unwrap().0,
        rvn_core::engine::DebugProgress::Stopped
    );
    assert_eq!(state(&e), initial);
    let save = e
        .capture_save15(&p, Some(&session), 2, "host function continuation".into())
        .unwrap();
    let bytes = save.encode().unwrap();
    assert!(bytes.contains("host_input"));
    assert!(!bytes.contains("\"lineage\""));
    finish(&mut e, &mut session);
    let expected = state(&e);
    let mut cold = engine();
    let mut loaded = cold
        .restore_save15(
            &p,
            rvn_core::save::consumer_checkpoint::Save15::decode(&bytes).unwrap(),
        )
        .unwrap()
        .unwrap();
    assert!(!cold.ui_input_source_current(&old.source));
    finish(&mut cold, &mut loaded);
    assert_eq!(state(&cold), expected);
    let mut forged: serde_json::Value = serde_json::from_str(&bytes).unwrap();
    forged["debugger"]["host_input"]["handler"] = "bad".into();
    let before = state(&cold);
    let epoch = cold.runtime_epoch();
    let count = cold.renderer.batches.lock().unwrap().len();
    assert!(cold
        .restore_save15(
            &p,
            rvn_core::save::consumer_checkpoint::Save15::decode(&forged.to_string()).unwrap()
        )
        .is_err());
    assert_eq!(state(&cold), before);
    assert_eq!(cold.runtime_epoch(), epoch);
    assert_eq!(cold.renderer.batches.lock().unwrap().len(), count);
}

// Prepared Native v3 owner gates: require the separately reviewed lexical
// ConsumerProgram admission and2|3 detection hunks before these can run GREEN.
fn lexical0770_host_program()->ParsedProgram{
    let range="n2:for_loop_with_break|exec_in=2@1,first=3@0,last=4@0,break=5@0,body=6@0,index=7@0,completed=8@0";
    let source=format!(r#"
init{{set hits=0}}
function bound0770(){{local token=random(1,1) return 2}}
function paint0770(state,props,frame){{return []}}
handler hit(event){{
 local count=0
 flow.consume_v3("g773/n1:handler_entry|exec_out=1@0","{range}"){{
 flow.for_loop(rvn_native_index_g773_n2,0,bound0770()){{
 flow.sequence(){{{{set count=count+1 flow.break()}}{{set count=count+10}}}}
 }}
 }}
 set hits=hits+count
 ui.set_state("panel","state0770",{{"text":"committed"}})
}}
screen panel(marker){{
 local count=0
 flow.consume_v3("g772/n1:screen_entry|exec_out=1@0","{range}"){{flow.for_loop(rvn_native_index_g772_n2,0,1){{set count=count+1}}}}
 return component("root","column",{{}},[component("act","button",{{"text":marker,"events":{{"click":"hit"}}}},[]),component("state0770","canvas",{{"draw":"paint0770","state":{{"text":"initial"}}}},[])])
}}
scene label start
ui.open("panel",["actual"],false,1)
"ready"
return
"#);
    let mut p=rvn_parser::parse_program(&source).unwrap();
    let path=Some(std::path::PathBuf::from("C:/private/native-lexical-host0770.rvn"));p.sources[0].path=path.clone();for origin in &mut p.origins{origin.source=path.clone();}p
}
fn lexical0770_host_engine()->Engine<Host>{
    let mut e=Engine::from_program_with_debugger(lexical0770_host_program(),Host::default(),16).unwrap();e.state.random=RandomState::seeded(770);e
}
#[test]
fn lexical0770_screen_real_resident_definition_and_idle_cold_keep_owner(){
    let p=lexical0770_host_program();let mut e=lexical0770_host_engine();e.step_until_interaction().unwrap();
    let instance=e.state.ui.screens.iter().find(|s|s.name=="panel").unwrap();
    let bound=instance.consumer_frames.get("screen:panel").expect("Native3 must require its real instance frame");
    assert_eq!(bound.owner.kind,rvn_core::consumer_schedule::HostDefinitionKind::Screen);assert!(bound.owner.target.incarnation>0);
    assert_eq!(bound.owner.definition,"panel");let snapshot=state(&e);for _ in 0..3{e.interface_views().unwrap();}assert_eq!(state(&e),snapshot);
    let save=e.capture_save15(&p,None,4,"actual Native3 screen".into()).unwrap();let raw=save.encode().unwrap();
    let mut cold=lexical0770_host_engine();cold.restore_save15(&p,rvn_core::save::consumer_checkpoint::Save15::decode(&raw).unwrap()).unwrap();
    assert_eq!(cold.state.ui.screens[0].consumer_frames["screen:panel"].owner.definition,"panel");
    assert_eq!(cold.state.ui.screens[0].consumer_frames["screen:panel"].owner.kind,rvn_core::consumer_schedule::HostDefinitionKind::Screen);
    assert_eq!(cold.state.vars,e.state.vars);assert!(!cold.state.vars.keys().any(|name|name.starts_with("rvn_native_index_")));
}
#[test]
fn lexical0770_handler_actual_input_paused_function_and_cold_save15_are_atomic(){
    use rvn_core::editor_debugger as w;
    let p=lexical0770_host_program();let mut e=lexical0770_host_engine();e.step_until_interaction().unwrap();
    assert_eq!(e.state.ui.screens[0].canvas_states["state0770"].state,Value::Dict([("text".into(),Value::Str("initial".into()))].into()));
    let prepared=rvn_core::program::PreparedNarrative::from_parsed(p.clone()).unwrap();let pc=prepared.script.iter().position(|s|matches!(s,rvn_parser::Statement::Function{name,..}if name=="bound0770")).unwrap();
    let bp=w::Breakpoint{position:prepared.position(pc),site:Some(w::Site{function:Some("bound0770".into()),route:vec![w::Route::Statement(1)],depth:2,statement:true}),enabled:true};
    let before=state(&e);let old=input(&e,"act");let batches=e.renderer.batches.lock().unwrap().len();
    let mut session=e.debug_begin(rvn_core::engine::DebugOperation::Input(old.clone()),vec![bp]).unwrap();
    assert_eq!(e.debug_step(&mut session,w::StepKind::Continue).unwrap().0,rvn_core::engine::DebugProgress::Stopped);assert_eq!(state(&e),before);assert_eq!(e.renderer.batches.lock().unwrap().len(),batches);
    let save=e.capture_save15(&p,Some(&session),5,"actual Native3 handler".into()).unwrap();let raw=save.encode().unwrap();assert!(raw.contains("host_input"));assert!(raw.contains("native_loop_resume"));
    finish(&mut e,&mut session);assert_eq!(e.state.vars["hits"],Value::Int(11));assert_eq!(e.state.ui.screens[0].canvas_states["state0770"].state,Value::Dict([("text".into(),Value::Str("committed".into()))].into()));let expected=state(&e);
    let mut cold=lexical0770_host_engine();let mut loaded=cold.restore_save15(&p,rvn_core::save::consumer_checkpoint::Save15::decode(&raw).unwrap()).unwrap().unwrap();
    assert!(!cold.ui_input_source_current(&old.source));finish(&mut cold,&mut loaded);assert_eq!(state(&cold),expected);
    let mut bad:serde_json::Value=serde_json::from_str(&raw).unwrap();bad["debugger"]["host_input"]["handler"]="wrong".into();let before=state(&cold);let epoch=cold.runtime_epoch();let batches=cold.renderer.batches.lock().unwrap().len();
    assert!(cold.restore_save15(&p,rvn_core::save::consumer_checkpoint::Save15::decode(&bad.to_string()).unwrap()).is_err());assert_eq!(state(&cold),before);assert_eq!(cold.runtime_epoch(),epoch);assert_eq!(cold.renderer.batches.lock().unwrap().len(),batches);
    let current=input(&cold,"act");cold.renderer.reject.store(true,Ordering::SeqCst);assert!(cold.interface_event_scoped(current).is_err());assert_eq!(state(&cold),before);assert_eq!(cold.renderer.batches.lock().unwrap().len(),batches);
}

#[test]
fn lexical0770_screen_initial_description_stop_requires_real_save15_owner_adapter(){
    use rvn_core::editor_debugger as w;
    let p=lexical0770_host_program();let source=p.sources[0].source.replace("flow.for_loop(rvn_native_index_g772_n2,0,1)","flow.for_loop(rvn_native_index_g772_n2,0,bound0770())");
    let mut p=rvn_parser::parse_program(&source).unwrap();let path=Some(std::path::PathBuf::from("C:/private/native-lexical-screen-init0770.rvn"));p.sources[0].path=path.clone();for origin in &mut p.origins{origin.source=path.clone();}
    let mut e=Engine::from_program_with_debugger(p.clone(),Host::default(),16).unwrap();e.state.random=RandomState::seeded(770);e.enter_label("start").unwrap();
    assert!(e.state.ui.screens.is_empty());let before=state(&e);let batches=e.renderer.batches.lock().unwrap().len();
    let prepared=rvn_core::program::PreparedNarrative::from_parsed(p.clone()).unwrap();let pc=prepared.script.iter().position(|s|matches!(s,rvn_parser::Statement::Function{name,..}if name=="bound0770")).unwrap();
    let bp=w::Breakpoint{position:prepared.position(pc),site:Some(w::Site{function:Some("bound0770".into()),route:vec![w::Route::Statement(1)],depth:2,statement:true}),enabled:true};
    let mut session=e.debug_begin(rvn_core::engine::DebugOperation::Statement,vec![bp]).unwrap();
    assert_eq!(e.debug_step(&mut session,w::StepKind::Continue).unwrap().0,rvn_core::engine::DebugProgress::Stopped);assert_eq!(state(&e),before);assert_eq!(e.renderer.batches.lock().unwrap().len(),batches);
    // This positive capability gate must not be weakened into accepting the
    // current refusal. No permit/instance may be manufactured by this test.
    let save=e.capture_save15(&p,Some(&session),6,"actual pending initial screen".into()).expect("pending initial Screen description needs its real operation-owned Save15 adapter");
    let bytes=save.encode().unwrap();finish(&mut e,&mut session);let expected=state(&e);
    let mut cold=Engine::from_program_with_debugger(p.clone(),Host::default(),16).unwrap();cold.state.random=RandomState::seeded(770);
    let mut loaded=cold.restore_save15(&p,rvn_core::save::consumer_checkpoint::Save15::decode(&bytes).unwrap()).unwrap().unwrap();finish(&mut cold,&mut loaded);assert_eq!(state(&cold),expected);
}

#[test]
fn lexical0803_handler_after_idle_cold_restore_and_second_pending_cold_replay_are_equal(){
    use rvn_core::editor_debugger as w;
    let p=lexical0770_host_program();let mut warm=lexical0770_host_engine();warm.step_until_interaction().unwrap();
    let idle=warm.capture_save15(&p,None,7,"idle before genuine cold input".into()).unwrap().encode().unwrap();
    let mut e=lexical0770_host_engine();assert!(e.restore_save15(&p,rvn_core::save::consumer_checkpoint::Save15::decode(&idle).unwrap()).unwrap().is_none());
    let before=state(&e);let batches=e.renderer.batches.lock().unwrap().len();let prepared=rvn_core::program::PreparedNarrative::from_parsed(p.clone()).unwrap();
    let pc=prepared.script.iter().position(|s|matches!(s,rvn_parser::Statement::Function{name,..}if name=="bound0770")).unwrap();
    let bp=w::Breakpoint{position:prepared.position(pc),site:Some(w::Site{function:Some("bound0770".into()),route:vec![w::Route::Statement(1)],depth:2,statement:true}),enabled:true};
    let old=input(&e,"act");let mut session=e.debug_begin(rvn_core::engine::DebugOperation::Input(old.clone()),vec![bp]).unwrap();
    assert_eq!(e.debug_step(&mut session,w::StepKind::Continue).unwrap().0,rvn_core::engine::DebugProgress::Stopped);assert_eq!(state(&e),before);assert_eq!(e.renderer.batches.lock().unwrap().len(),batches);
    let capture_source=e.debug_state_source().unwrap();assert!(e.accepts_debug_session(&session));
    let paused=e.capture_save15(&p,Some(&session),8,"pending handler from cold idle".into()).unwrap().encode().unwrap();
    assert!(e.accepts_debug_state_source(&capture_source),"Save15 cold-idle handler capture changed the live Engine source");
    assert!(e.accepts_debug_session(&session),"Save15 cold-idle handler capture invalidated its stopped session");
    finish(&mut e,&mut session);assert_eq!(e.state.vars["hits"],Value::Int(11));let expected=state(&e);
    let mut cold=lexical0770_host_engine();let mut loaded=cold.restore_save15(&p,rvn_core::save::consumer_checkpoint::Save15::decode(&paused).unwrap()).unwrap().unwrap();
    assert!(cold.accepts_debug_session(&loaded),"Second pending cold restore did not bind its actual current source");
    assert!(!cold.ui_input_source_current(&old.source));finish(&mut cold,&mut loaded);assert_eq!(state(&cold),expected);
    let random=cold.state.random;for _ in 0..3{cold.interface_views().unwrap();}assert_eq!(cold.state.random,random);assert_eq!(state(&cold),expected);
}

#[test]
fn lexical0803_actual_canvas_draw_breakpoint_stays_debuggable_and_cold_atomic(){
    use rvn_core::editor_debugger as w;
    let p=lexical0770_host_program();let mut e=lexical0770_host_engine();e.step_until_interaction().unwrap();
    // Genuine warm views must not suppress a Canvas computation breakpoint.
    for _ in 0..3{e.interface_views().unwrap();}
    let before=state(&e);let random=e.state.random;let batches=e.renderer.batches.lock().unwrap().len();let prepared=rvn_core::program::PreparedNarrative::from_parsed(p.clone()).unwrap();
    let pc=prepared.script.iter().position(|s|matches!(s,rvn_parser::Statement::Function{name,..}if name=="paint0770")).unwrap();
    let bp=w::Breakpoint{position:prepared.position(pc),site:Some(w::Site{function:Some("paint0770".into()),route:vec![w::Route::Statement(0)],depth:1,statement:true}),enabled:true};
    let old=input(&e,"act");let mut session=e.debug_begin(rvn_core::engine::DebugOperation::Input(old.clone()),vec![bp]).unwrap();
    assert_eq!(e.debug_step(&mut session,w::StepKind::Continue).unwrap().0,rvn_core::engine::DebugProgress::Stopped);
    assert_eq!(session.snapshot().site.as_ref().and_then(|site|site.function.as_deref()),Some("paint0770"));assert_eq!(state(&e),before);assert_eq!(e.state.random,random);assert_eq!(e.renderer.batches.lock().unwrap().len(),batches);
    let capture_source=e.debug_state_source().unwrap();assert!(e.accepts_debug_session(&session));
    let saved=e.capture_save15(&p,Some(&session),9,"actual Canvas draw stop".into()).unwrap().encode().unwrap();
    assert!(e.accepts_debug_state_source(&capture_source),"Save15 Canvas draw capture changed the live Engine source");
    assert!(e.accepts_debug_session(&session),"Save15 Canvas draw capture invalidated its stopped session");
    finish(&mut e,&mut session);assert_eq!(e.state.vars["hits"],Value::Int(11));let expected=state(&e);
    let mut cold=lexical0770_host_engine();let mut loaded=cold.restore_save15(&p,rvn_core::save::consumer_checkpoint::Save15::decode(&saved).unwrap()).unwrap().unwrap();
    assert!(!cold.ui_input_source_current(&old.source));finish(&mut cold,&mut loaded);assert_eq!(state(&cold),expected);
    let before=state(&cold);let count=cold.renderer.batches.lock().unwrap().len();cold.renderer.reject.store(true,Ordering::SeqCst);assert!(cold.interface_event_scoped(input(&cold,"act")).is_err());assert_eq!(state(&cold),before);assert_eq!(cold.renderer.batches.lock().unwrap().len(),count);
}
