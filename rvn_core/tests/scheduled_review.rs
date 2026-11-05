use rvn_core::{Engine, Renderer};
use rvn_parser::{Hotspot, Position, Statement, Transition};
use rvn_core::types::SpriteState;
use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};

struct RecordingRenderer(Arc<AtomicUsize>);
impl Renderer for RecordingRenderer {
    fn set_background(&mut self, _: &str, _: &Transition) { self.0.fetch_add(1, Ordering::SeqCst); }
    fn show_sprite(&mut self, _: &str, _: Option<&str>, _: &Position, _: &Transition, _: Option<&SpriteState>) {}
    fn hide_sprite(&mut self, _: &str, _: &Transition, _: &SpriteState) {}
    fn move_sprite(&mut self, _: &str, _: &Position, _: &Transition, _: &SpriteState) {}
    fn show_dialogue(&mut self, _: Option<&str>, _: &str) {}
    fn show_choice(&mut self, _: &[String]) -> usize { 0 }
    fn music_play(&mut self, _: &str, _: &Transition, _: Option<&str>) {}
    fn music_stop(&mut self, _: &Transition) {}
    fn music_set_volume(&mut self, _: f32) {}
    fn sfx_play(&mut self, _: &str, _: &Transition) {}
    fn sfx_stop(&mut self, _: &str, _: &Transition) {}
    fn show_imagemap(&mut self, _: &str, _: Option<&str>, _: &[Hotspot]) -> usize { 0 }
}
fn schedule() -> Statement {
    let mut statement = rvn_parser::parse(r#"flow.consume_v1("scope", "consumer") { set x = 1 }"#).unwrap().remove(0);
    let Statement::ScheduledConsumer {version,..} = &mut statement else {unreachable!()};
    *version = 3; statement
}
fn background() -> Statement {
    Statement::Scene { background: "before.png".into(), transition: Transition::None }
}
fn assert_preflight(script: Vec<Statement>) {
    let publications = Arc::new(AtomicUsize::new(0));
    let result = Engine::new(script, RecordingRenderer(publications.clone()), 8);
    assert!(result.is_err());
    assert_eq!(publications.load(Ordering::SeqCst), 0, "unsupported Init must refuse before any earlier renderer publication");
}
#[test]
fn init_preflight_refuses_before_earlier_statement_publication() {
    assert_preflight(vec![Statement::Init { body: vec![background(), schedule()] }]);
}
#[test]
fn init_preflight_refuses_before_earlier_init_publication() {
    assert_preflight(vec![Statement::Init { body: vec![background()] }, Statement::Init { body: vec![schedule()] }]);
}
#[test]
fn init_preflight_catches_hidden_unreached_nested_schedule() {
    assert_preflight(vec![Statement::Init { body: vec![background()] }, Statement::Init { body: vec![
        Statement::If { condition: rvn_parser::Expr::Bool(false), then_branch: vec![schedule()], else_branch: vec![] }
    ] }]);
}
#[test]
fn supported_init_still_publishes_once() {
    let publications = Arc::new(AtomicUsize::new(0));
    assert!(Engine::new(vec![Statement::Init { body: vec![background()] }], RecordingRenderer(publications.clone()), 8).is_ok());
    assert_eq!(publications.load(Ordering::SeqCst), 1);
}

#[test]
fn hostile_lazy_plan_width_refuses_before_capture_random_effects() {
    use rvn_core::{eval::FunctionLibrary, consumer_schedule::ConsumerFrame, random::RandomState};
    use rvn_parser::{Expr, ConsumerStep};
    let body = [Statement::ScheduledConsumer { version: 1, scope: "scope".into(), consumer: "consumer".into(),
        steps: vec![ConsumerStep::Capture { producer: "draw".into(), value: Expr::Call { name: "random".into(), args: vec![Expr::Int(1), Expr::Int(1)] } }],
        body: vec![Statement::SetVar { name: "x".into(), value: Expr::And(Box::new(Expr::Bool(false)), Box::new(Expr::ListLit(vec![Expr::Bool(true); 100_001]))) }],
    }];
    let mut rng = RandomState::seeded(4); let before = rng; let mut frame = ConsumerFrame::default();
    let error = FunctionLibrary::default().execute_in_consumer_frame(&body, &Default::default(), &mut rng, &mut frame).unwrap_err();
    assert!(error.to_string().contains("expression nodes"));
    assert_eq!(rng, before); assert_eq!(frame, ConsumerFrame::default());
}

#[test]
fn hostile_plan_depth_refuses_before_capture_random_effects() {
    use rvn_core::{eval::FunctionLibrary, consumer_schedule::ConsumerFrame, random::RandomState};
    use rvn_parser::{Expr, ConsumerStep};
    let mut value = Expr::Bool(true);
    for _ in 0..257 { value = Expr::Not(Box::new(value)); }
    let body = [Statement::ScheduledConsumer { version: 1, scope: "scope".into(), consumer: "consumer".into(),
        steps: vec![ConsumerStep::Capture { producer: "draw".into(), value: Expr::Call { name: "random".into(), args: vec![Expr::Int(1), Expr::Int(1)] } }],
        body: vec![Statement::SetVar { name: "x".into(), value }],
    }];
    let mut rng = RandomState::seeded(4); let before = rng; let mut frame = ConsumerFrame::default();
    assert!(FunctionLibrary::default().execute_in_consumer_frame(&body, &Default::default(), &mut rng, &mut frame).is_err());
    assert_eq!(rng, before); assert_eq!(frame, ConsumerFrame::default());
}

#[test]
fn function_restores_caller_storage_and_terminals_with_same_declared_ids() {
    use rvn_core::{eval::FunctionLibrary, consumer_schedule::ConsumerFrame, random::RandomState};
    use rvn_parser::{Value, Name};
    let script = rvn_parser::parse(r#"
        function callee() {
            flow.consume_v1("same-scope", "callee") {
                inspect "inspector" = __rvn_invariant_text("literal")
                return __rvn_terminal_v1("inspector", "table_id")
            }
        }
        flow.consume_v1("same-scope", "first") {
            inspect "inspector" = __rvn_string_table_text_name(__rvn_literal_name("asset:CallerTable"), "caller-key")
            set before = __rvn_terminal_v1("inspector", "valid")
        }
        flow.consume_v1("same-scope", "second") {
            capture "returned" = callee()
            inspect "inspector" = __rvn_invariant_text("literal")
            set result = [__rvn_captured_v1("returned"), __rvn_terminal_v1("inspector", "table_id"), __rvn_terminal_v1("inspector", "key")]
        }
    "#).unwrap();
    let library = FunctionLibrary::from_script(&script).unwrap();
    let mut frame = ConsumerFrame::default();
    let values = library.execute_in_consumer_frame(&script[1..], &Default::default(), &mut RandomState::default(), &mut frame).unwrap();
    assert_eq!(values["result"], Value::List(vec![Value::Name(Name::default()), Value::Name(Name::new("asset:CallerTable").unwrap()), Value::Str("caller-key".into())]));
    frame.validate().unwrap();
    let serialized = serde_json::to_value(&frame).unwrap();
    assert_eq!(serialized.as_object().unwrap().len(), 2, "transient accounting must not enter the wire frame");
}
