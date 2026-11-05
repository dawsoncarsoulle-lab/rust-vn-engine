use rvn_core::{Engine,TerminalRenderer,engine::{DebugOperation,DebugProgress},editor_debugger::StepKind,random::RandomState};
use rvn_parser::Value;
fn execute(source:&str,local:bool){
    let mut engine=Engine::from_program(rvn_parser::parse_program(source).unwrap(),TerminalRenderer,8).unwrap();engine.enter_label("start").unwrap();engine.state.random=RandomState::seeded(4);
    let mut session=engine.debug_begin(DebugOperation::Statement,vec![]).unwrap();let mut complete=false;
    for _ in 0..8 {if engine.debug_step(&mut session,StepKind::Continue).unwrap().0==DebugProgress::Complete{complete=true;break;}}
    assert!(complete);let mut expected=RandomState::seeded(4);expected.integer(1,1).unwrap();assert_eq!(engine.state.random,expected);
    if local{assert_eq!(engine.state.scene_execution.as_ref().unwrap().active.as_ref().unwrap().locals["result"],Value::Bool(false));}else{assert_eq!(engine.state.vars["result"],Value::Bool(false));}
}
const PRODUCER:&str=r#"function producer() {local draw=random(1,1) return __rvn_invariant_text("literal")}"#;
fn root(binding:&str)->String{format!(r#"{PRODUCER}
scene label start
flow.consume_v1("scope","consumer") {{inspect "i"=producer() {binding} result=__rvn_terminal_v1("i","valid")}}
"done""#)}
#[test] fn actual_scheduled_engine_set_debugger_contrast(){execute(&root("set"),false);}
#[test] fn actual_scheduled_engine_local_debugger_contrast(){execute(&root("local"),true);}
#[test] fn actual_scheduled_function_debugger_contrast(){execute(&format!(r#"{PRODUCER}
function consumer() {{flow.consume_v1("scope","consumer") {{inspect "i"=producer() return __rvn_terminal_v1("i","valid")}}}}
scene label start
set result=consumer()
"done""#),false);}
