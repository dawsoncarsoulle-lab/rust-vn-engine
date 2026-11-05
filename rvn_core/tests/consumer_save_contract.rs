use rvn_core::save::consumer_checkpoint::{PortableConsumerProgram,PortableConsumerOwner};
use rvn_core::{Engine,TerminalRenderer};
use std::path::PathBuf;

fn program(path:&str)->rvn_parser::ParsedProgram {
    let mut program=rvn_parser::parse_program(r#"function f() { flow.consume_v1("scope","consumer") { inspect "i" = __rvn_invariant_text("literal") return __rvn_terminal_v1("i","valid") } } set x = f()"#).unwrap();
    program.sources[0].path=Some(PathBuf::from(path));
    for origin in &mut program.origins {origin.source=Some(PathBuf::from(path));}
    program
}
#[test]
fn portable_receipt_accepts_real_relocation_but_checks_entire_snapshot() {
    let original=program("C:/original/story.rvn"); let relocated=program("D:/relocated/story.rvn");
    let receipt=PortableConsumerProgram::from_program(&original).unwrap();
    receipt.validate_current(&relocated).unwrap();
    let live_original=rvn_core::consumer_schedule::ConsumerSourceBinding::from_program(&original).unwrap();
    let live_relocated=rvn_core::consumer_schedule::ConsumerSourceBinding::from_program(&relocated).unwrap();
    assert_ne!(live_original.closure,live_relocated.closure); assert_eq!(live_original.emitted,live_relocated.emitted); assert_eq!(live_original.content,live_relocated.content);
    let mut wrong=receipt.clone(); wrong.source_bytes_in_inclusion_order[0].push('\n'); assert!(wrong.validate_current(&original).is_err());
    let mut wrong=receipt.clone(); wrong.root_source_ordinals[0]=1; assert!(wrong.validate_current(&original).is_err());
    let mut wrong=receipt; let rvn_parser::Statement::Function {body,..}=&mut wrong.script[0] else {unreachable!()};
    let rvn_parser::Statement::ScheduledConsumer {consumer,..}=&mut body[0] else {unreachable!()}; consumer.push_str("changed");
    assert!(wrong.validate_current(&original).is_err());
}
#[test]
fn unknown_or_unattached_owner_cannot_fall_back_to_game() {
    let engine=Engine::new(rvn_parser::parse("\"old\"").unwrap(),TerminalRenderer,8).unwrap();
    assert!(serde_json::from_str::<PortableConsumerOwner>(r#"{"kind":"mystery"}"#).is_err());
    let scene=PortableConsumerOwner::Scene{activation:rvn_core::scene::ActivationId(1)};
    assert!(scene.require_story_frame(&engine.state).is_err());
    let host=PortableConsumerOwner::Host{target:rvn_core::scene_resources::TargetIdentity{kind:rvn_core::scene_resources::TargetKind::Interface,alias:"widget".into(),incarnation:1},source_ordinal:0,definition:"handler".into(),definition_kind:None};
    assert!(host.require_story_frame(&engine.state).unwrap_err().to_string().contains("TargetIdentity"));
    assert!(PortableConsumerOwner::Function{source_ordinal:0,definition:"f".into(),stack_frame:1}.require_story_frame(&engine.state).is_err());
}
