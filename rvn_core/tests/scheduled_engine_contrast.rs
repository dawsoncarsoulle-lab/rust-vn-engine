use rvn_core::{Engine, TerminalRenderer, random::RandomState, save::SaveData};
use rvn_parser::Value;
#[test]
fn ordinary_scheduled_consumer_executes_without_adapter_refusal() {
    let source=r#"flow.consume_v1("scope","consumer") { inspect "inspector" = __rvn_invariant_text("literal") set result = __rvn_terminal_v1("inspector","valid") }"#;
    let mut engine=Engine::new(rvn_parser::parse(source).unwrap(),TerminalRenderer,8).unwrap();
    engine.step_silent().unwrap(); assert_eq!(engine.state.vars["result"],Value::Bool(false));
}
#[test]
fn unchanged_historical_state_and_save_wire_receipt() {
    let mut engine=Engine::new(rvn_parser::parse("set x = 1\n\"done\"").unwrap(),TerminalRenderer,8).unwrap();
    engine.state.random=RandomState::seeded(4); engine.step_silent().unwrap();
    let mut dto=SaveData::try_from_state(&engine.state,1,"fixture".into(),"story.rvn".into()).unwrap(); dto.timestamp=0;
    let receipt=serde_json::json!({"state_json":serde_json::to_string(&engine.state).unwrap(),"save_json":serde_json::to_string(&dto).unwrap()});
    if let Ok(path)=std::env::var("SCHEDULED_WIRE_RECEIPT") { std::fs::write(path,serde_json::to_vec_pretty(&receipt).unwrap()).unwrap(); }
}
