use rvn_core::{Engine, Renderer, types::SpriteState, renderer::SceneRenderBatch, random::RandomState, save::SaveData};
use rvn_parser::{Hotspot, Position, Transition, Value, Name, Statement};
use std::sync::{Arc,atomic::{AtomicUsize,AtomicBool,Ordering}};

#[derive(Clone,Default)]
struct Host { published: Arc<AtomicUsize>, reject: Arc<AtomicBool> }
impl Renderer for Host {
    fn validate_scene_batch(&self, _: &SceneRenderBatch)->Result<(),String> { if self.reject.load(Ordering::SeqCst) {Err("fixture rejects candidate".into())} else {Ok(())} }
    fn publish_scene_batch(&mut self, _: SceneRenderBatch) { self.published.fetch_add(1,Ordering::SeqCst); }
    fn set_background(&mut self, _: &str, _: &Transition) {self.published.fetch_add(1,Ordering::SeqCst);}
    fn show_sprite(&mut self, _: &str, _: Option<&str>, _: &Position, _: &Transition, _: Option<&SpriteState>) {}
    fn hide_sprite(&mut self, _: &str, _: &Transition, _: &SpriteState) {}
    fn move_sprite(&mut self, _: &str, _: &Position, _: &Transition, _: &SpriteState) {}
    fn show_dialogue(&mut self, _: Option<&str>, _: &str) {}
    fn show_choice(&mut self, _: &[String])->usize {0}
    fn music_play(&mut self, _: &str, _: &Transition, _: Option<&str>) {}
    fn music_stop(&mut self, _: &Transition) {}
    fn music_set_volume(&mut self, _: f32) {}
    fn sfx_play(&mut self, _: &str, _: &Transition) {}
    fn sfx_stop(&mut self, _: &str, _: &Transition) {}
    fn show_imagemap(&mut self, _: &str, _: Option<&str>, _: &[Hotspot])->usize {0}
}
fn inspect(consumer:&str,input:&str,binding:&str,target:&str)->String {
    format!(r#"flow.consume_v1("scope", "{consumer}") {{ inspect "inspector" = {input} {binding} {target} = [__rvn_terminal_v1("inspector","valid"),__rvn_terminal_v1("inspector","table_id"),__rvn_terminal_v1("inspector","key")] }}"#)
}
const TABLE:&str=r#"__rvn_string_table_text_name(__rvn_literal_name("asset:Caller"),"hello")"#;
const LITERAL:&str=r#"__rvn_invariant_text("literal")"#;
fn state(engine:&Engine<Host>)->serde_json::Value {serde_json::to_value(&engine.state).unwrap()}

#[test]
fn ordinary_engine_retains_false_terminals_across_real_consumers() {
    let source=format!("{}\n{}\n\"done\"",inspect("first",TABLE,"set","first"),inspect("second",LITERAL,"set","second"));
    let mut engine=Engine::new(rvn_parser::parse(&source).unwrap(),Host::default(),8).unwrap();
    engine.step_until_interaction().unwrap();
    assert_eq!(engine.state.vars["second"],Value::List(vec![Value::Bool(false),Value::Name(Name::new("asset:Caller").unwrap()),Value::Str("hello".into())]));
    assert!(engine.state.consumer_frame.is_some());
}

#[test]
fn shared_engine_producer_draws_once_and_lexical_function_cannot_see_scene_local() {
    let source=r#"
        function make() { set draw = random(1,1) return __rvn_string_table_text_name(__rvn_literal_name("asset:Caller"),"hello") }
        scene label A
        local secret = 44
        flow.consume_v1("scope","consume") {
            capture "shared" = make()
            inspect "inspector" = __rvn_captured_v1("shared")
            local result = [__rvn_terminal_v1("inspector","valid"),__rvn_terminal_v1("inspector","table_id"),__rvn_terminal_v1("inspector","key"),__rvn_captured_v1("shared")]
        }
        "done"
    "#;
    let mut engine=Engine::from_program(rvn_parser::parse_program(&source).unwrap(),Host::default(),8).unwrap();
    engine.state.random=RandomState::seeded(8); let mut expected=engine.state.random; expected.integer(1,1).unwrap();
    engine.step_silent().unwrap();
    assert_eq!(engine.state.random,expected);
    let active=engine.state.scene_execution.as_ref().unwrap().active.as_ref().unwrap();
    assert!(matches!(&active.locals["result"],Value::List(values) if values.len()==4));
    assert!(!engine.state.vars.contains_key("result")); assert!(!engine.state.vars.contains_key("draw"));
}

#[test]
fn scene_call_parks_caller_frame_and_callee_starts_fresh() {
    let source=format!("scene label A\n{}\ncall B\n{}\n\"parent\"\njump End\nscene label B\n{}\n\"child\"\nreturn\nscene label End\n\"end\"",
        inspect("first",TABLE,"set","first"),inspect("after",LITERAL,"set","after"),inspect("child",LITERAL,"set","child"));
    let mut engine=Engine::new(rvn_parser::parse(&source).unwrap(),Host::default(),8).unwrap();
    engine.step_until_interaction().unwrap();
    let execution=engine.state.scene_execution.as_ref().unwrap();
    let parent=execution.frames.iter().find_map(|frame|frame.parked.as_ref()).unwrap();
    assert!(parent.consumer_frame.is_some()); assert_ne!(parent.id,execution.active.as_ref().unwrap().id);
    assert_eq!(engine.state.vars["child"],Value::List(vec![Value::Bool(false),Value::Name(Name::default()),Value::Str(String::new())]));
    engine.advance_dialogue().unwrap(); engine.step_until_interaction().unwrap();
    assert_eq!(engine.state.vars["after"],Value::List(vec![Value::Bool(false),Value::Name(Name::new("asset:Caller").unwrap()),Value::Str("hello".into())]));
}

#[test]
fn helper_keeps_scene_frame_and_scene_jump_gets_new_frame() {
    let source=format!("scene label A\n{}\ncall helper\njump B\nlabel helper\n{}\nreturn\nscene label B\n{}\n\"done\"",
        inspect("first",TABLE,"set","first"),inspect("helper",LITERAL,"set","helper"),inspect("new",LITERAL,"set","new"));
    let mut engine=Engine::new(rvn_parser::parse(&source).unwrap(),Host::default(),8).unwrap();
    engine.step_until_interaction().unwrap();
    assert!(matches!(&engine.state.vars["helper"],Value::List(values) if values[1]==Value::Name(Name::new("asset:Caller").unwrap())));
    assert!(matches!(&engine.state.vars["new"],Value::List(values) if values[1]==Value::Name(Name::default())));
}

#[test]
fn renderer_rejection_preserves_variables_random_frame_pc_and_persistent_store() {
    let host=Host::default();
    let source=r#"scene label A
        "before"
        flow.consume_v1("scope","consume") { capture "draw" = random(1,1) inspect "inspector" = __rvn_string_table_text_name(__rvn_literal_name("asset:Caller"),"hello") set persistent.result = __rvn_terminal_v1("inspector","table_id") }
        "after"
    "#;
    let mut engine=Engine::new(rvn_parser::parse(&source).unwrap(),host.clone(),8).unwrap();
    engine.step_until_interaction().unwrap(); engine.advance_dialogue().unwrap();
    let before=state(&engine); let persistent=engine.persistent_vars.clone(); let publications=host.published.load(Ordering::SeqCst);
    host.reject.store(true,Ordering::SeqCst);
    assert!(engine.step_until_interaction().is_err());
    assert_eq!(state(&engine),before); assert_eq!(engine.persistent_vars,persistent); assert_eq!(host.published.load(Ordering::SeqCst),publications);
    host.reject.store(false,Ordering::SeqCst); engine.step_until_interaction().unwrap();
    assert_eq!(engine.persistent_vars["persistent.result"],Value::Name(Name::new("asset:Caller").unwrap()));
}

#[test]
fn producer_error_does_not_commit_frame_or_random() {
    let source=r#"flow.consume_v1("scope","consumer") { capture "draw" = random(1,1) inspect "inspector" = __rvn_string_table_text_name(__rvn_literal_name("asset:Caller"),"hello") set result = 1/0 }"#;
    let mut engine=Engine::new(rvn_parser::parse(source).unwrap(),Host::default(),8).unwrap();
    let before=state(&engine); assert!(engine.step_until_interaction().is_err()); assert_eq!(state(&engine),before);
}

#[test]
fn planned_or_executed_consumers_cannot_be_written_as_save_1_to_14() {
    let source=format!("\"before\"\n{}\n\"after\"",inspect("consumer",TABLE,"set","result"));
    let mut engine=Engine::new(rvn_parser::parse(&source).unwrap(),Host::default(),8).unwrap();
    assert!(SaveData::try_from_state(&engine.state,1,"".into(),"".into()).unwrap_err().to_string().contains("Save15"));
    let raw=SaveData::from_state(&engine.state,1,"".into(),"".into());
    assert_eq!(raw.format_version,15,"raw DTO cannot mislabel a new frame as a version 1–14 save");
    assert!(raw.validate_checkpoint_shape().is_err());
    engine.step_until_interaction().unwrap(); engine.advance_dialogue().unwrap(); engine.step_until_interaction().unwrap();
    assert!(SaveData::try_from_state(&engine.state,1,"".into(),"".into()).is_err());
}

#[test]
fn legacy_program_wire_has_no_new_empty_frame_fields() {
    let engine=Engine::new(rvn_parser::parse("set x = 1\n\"done\"").unwrap(),Host::default(),8).unwrap();
    assert!(state(&engine).get("consumer_frame").is_none());
    let old=SaveData::try_from_state(&engine.state,1,"".into(),"".into()).unwrap();
    assert!(serde_json::to_value(old).unwrap().get("consumer_frame").is_none());
}

#[test]
fn frozen_source_binding_is_recomputed_before_init_publication() {
    use rvn_core::consumer_schedule::ConsumerSourceBinding;
    let source=format!("init {{ scene \"before.png\" }}\n{}\n\"done\"",inspect("consumer",TABLE,"set","result"));
    let program=rvn_parser::parse_program(&source).unwrap();
    let mut binding=ConsumerSourceBinding::from_program(&program).unwrap(); binding.content[0]^=1;
    let host=Host::default();
    assert!(Engine::from_program_with_consumer_binding(program,binding,host.clone(),8).is_err());
    assert_eq!(host.published.load(Ordering::SeqCst),0);
}

#[test]
fn same_ast_with_changed_frozen_source_rejects_old_frame_before_random() {
    let source=format!("{}\n\"done\"",inspect("consumer",TABLE,"set","result"));
    let first=Engine::from_program(rvn_parser::parse_program(&source).unwrap(),Host::default(),8).unwrap();
    let mut second=Engine::from_program(rvn_parser::parse_program(&(source+"\n")).unwrap(),Host::default(),8).unwrap();
    second.state.consumer_frame=first.state.consumer_frame.clone();
    let before=state(&second); assert!(second.step_until_interaction().is_err()); assert_eq!(state(&second),before);
}

#[test]
fn changed_public_script_invalidates_frozen_plan_before_effects() {
    let source=format!("{}\n\"done\"",inspect("consumer",TABLE,"set","result"));
    let mut engine=Engine::new(rvn_parser::parse(&source).unwrap(),Host::default(),8).unwrap();
    let Statement::ScheduledConsumer {consumer,..}=&mut engine.script[0] else {unreachable!()}; consumer.push_str("changed");
    let before=state(&engine); assert!(engine.step_until_interaction().is_err()); assert_eq!(state(&engine),before);
}

#[test]
fn valid_init_scheduling_is_executed_and_false_outputs_are_retained_after_init() {
    let source=format!("init {{ {} }}\n{}\n\"done\"",inspect("init",TABLE,"set","initial"),inspect("later",LITERAL,"set","later"));
    let mut engine=Engine::from_program(rvn_parser::parse_program(&source).unwrap(),Host::default(),8).unwrap();
    assert!(engine.state.vars.contains_key("initial")); engine.step_until_interaction().unwrap();
    assert!(matches!(&engine.state.vars["later"],Value::List(values) if values[1]==Value::Name(Name::new("asset:Caller").unwrap())));
}

#[test]
fn old_checkpoint_cannot_attach_to_a_planned_scheduled_program() {
    let source=format!("\"before\"\n{}",inspect("consumer",TABLE,"set","result"));
    let mut engine=Engine::new(rvn_parser::parse(&source).unwrap(),Host::default(),8).unwrap();
    let old=Engine::new(rvn_parser::parse("\"old\"").unwrap(),Host::default(),8).unwrap();
    for version in [1,9,14] {
        let mut dto=SaveData::from_state(&old.state,1,"".into(),"".into()); dto.format_version=version; dto.story_identity=None;
        let before=state(&engine);
        assert!(engine.load_data(dto).unwrap_err().to_string().contains("Save15")); assert_eq!(state(&engine),before);
    }
}

#[test]
fn failed_lexical_function_cannot_capture_scene_local_or_consume_random() {
    let source=r#"function steal() { return secret }
        scene label A
        local secret = 44
        "before"
        flow.consume_v1("scope","consumer") { capture "draw" = random(1,1) capture "attempt" = steal() set x = __rvn_captured_v1("attempt") }
        "after"
    "#;
    let mut engine=Engine::from_program(rvn_parser::parse_program(source).unwrap(),Host::default(),8).unwrap();
    engine.step_until_interaction().unwrap(); engine.advance_dialogue().unwrap();
    let before=state(&engine); assert!(engine.step_until_interaction().is_err()); assert_eq!(state(&engine),before);
}

#[test]
fn scene_rollback_restores_frame_and_replays_no_producer() {
    let function=format!("function make(which) {{ set draw = random(1,1) if which {{ return {TABLE} }} return {LITERAL} }}\n");
    let source=format!("{function}scene label A\n{}\n\"one\"\n{}\n\"two\"",inspect("first","make(true)","set","first"),inspect("second","make(false)","set","second"));
    let mut engine=Engine::from_program(rvn_parser::parse_program(&source).unwrap(),Host::default(),8).unwrap();
    engine.step_until_interaction().unwrap();
    let first_frame=engine.state.scene_execution.as_ref().unwrap().active.as_ref().unwrap().consumer_frame.clone();
    let random=engine.state.random;
    engine.advance_dialogue().unwrap(); engine.step_until_interaction().unwrap();
    let second_random=engine.state.random;
    assert!(engine.rollback());
    assert_eq!(engine.state.random,second_random);
    // First rollback restores the last authorized display (two), and a second
    // returns to the previous display (one) with its exact owned frame.
    assert!(engine.rollback());
    assert_eq!(engine.state.random,random);
    assert_eq!(engine.state.scene_execution.as_ref().unwrap().active.as_ref().unwrap().consumer_frame,first_frame);
    assert!(!engine.state.vars.contains_key("second"));
}

#[test]
fn rollback_rejects_stale_owned_frame_without_popping_history_or_publishing() {
    let source=format!("{}\n\"done\"",inspect("consumer",TABLE,"set","result"));
    let host=Host::default(); let mut engine=Engine::new(rvn_parser::parse(&source).unwrap(),host.clone(),8).unwrap();
    engine.step_until_interaction().unwrap();
    let mut stale=engine.state.clone(); stale.consumer_frame.as_mut().unwrap().program[0]^=1;
    engine.history.push(stale,Some(rvn_core::HistoryDisplay::Dialogue{character:None,text:"stale".into()}));
    let before=state(&engine); let count=engine.history.entries().len(); let publications=host.published.load(Ordering::SeqCst);
    assert!(!engine.rollback()); assert_eq!(state(&engine),before); assert_eq!(engine.history.entries().len(),count); assert_eq!(host.published.load(Ordering::SeqCst),publications);
}

#[test]
fn frame_attached_to_wrong_actual_activation_is_rejected_before_effects() {
    let source=format!("scene label A\n{}\n\"one\"\n{}\n\"two\"",inspect("first",TABLE,"set","first"),inspect("second",LITERAL,"set","second"));
    let mut engine=Engine::new(rvn_parser::parse(&source).unwrap(),Host::default(),8).unwrap();
    engine.step_until_interaction().unwrap(); engine.advance_dialogue().unwrap();
    let active=engine.state.scene_execution.as_mut().unwrap().active.as_mut().unwrap();
    active.consumer_frame.as_mut().unwrap().owner=rvn_core::consumer_schedule::ConsumerOwner::Scene{activation:rvn_core::scene::ActivationId(active.id.0+1)};
    let before=state(&engine); assert!(engine.step_silent().is_err()); assert_eq!(state(&engine),before);
}

#[test]
fn narrative_loop_regathers_dependencies_and_retains_same_activation_terminals() {
    let source=format!(r#"
        function text_for(index) {{ if index == 0 {{ return {TABLE} }} return {LITERAL} }}
        set index = 0
        scene label A
        while index < 2 {{
            flow.consume_v1("scope","loop") {{
                capture "draw" = random(1,1)
                inspect "inspector" = text_for(index)
                set index = index + __rvn_captured_v1("draw")
            }}
        }}
        {}
        "done"
    "# ,inspect("after",LITERAL,"set","after"));
    let mut engine=Engine::from_program(rvn_parser::parse_program(&source).unwrap(),Host::default(),8).unwrap();
    engine.state.random=RandomState::seeded(4); let mut expected=engine.state.random;
    expected.integer(1,1).unwrap(); expected.integer(1,1).unwrap();
    engine.step_silent().unwrap(); assert_eq!(engine.state.random,expected); assert_eq!(engine.state.vars["index"],Value::Int(2));
    assert!(matches!(&engine.state.vars["after"],Value::List(values) if values[0]==Value::Bool(false) && values[1]==Value::Name(Name::new("asset:Caller").unwrap())));
}
