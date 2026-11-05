use rvn_parser::{parse,Statement,Expr};
#[test]fn historical_music_volume_ast_stays_literal(){
 for (source,expected) in [("music.volume()",1.0),("music.volume(\"oops\")",1.0),("music.volume(\"0.25\")",0.25),("music.volume(0.25)",0.25),("music.volume(-2)",0.0),("music.volume(2)",1.0)]{
  assert_eq!(parse(source).unwrap(),vec![Statement::MusicVolume{level:expected}],"{source}");
 }
}
#[test]fn genuine_expression_keeps_authored_ast(){
 for source in ["music.volume(level)","music.volume(-level)","music.volume(level+0.1)","music.volume(1+level)","music.volume(draw())","music.volume(__rvn_captured_v1(\"actual\"))"] {
  let parsed=parse(source).unwrap();let [Statement::MusicVolumeValue{level}]=parsed.as_slice() else {panic!("{source}")};
  assert!(!matches!(level,Expr::Float(_)|Expr::Int(_)),"{source}");
 }
}
#[test]fn malformed_expression_arguments_stay_errors(){for source in ["music.volume(1,2)","music.volume(level,other)","music.volume(level+)","music.volume(-)"]{assert!(parse(source).is_err(),"{source}");}}
#[test]fn dynamic_body_uses_shared_audio_contract(){
 let parsed=parse("flow.consume_v2(\"s\",\"c\"){capture \"actual\"=0.2 music.volume(__rvn_captured_v1(\"actual\"))}").unwrap();
 let Statement::ScheduledConsumer{body,..}=&parsed[0] else{panic!()};
 assert_eq!(rvn_parser::scheduled_consumer_kind(&body[0]),Some(rvn_parser::ScheduledConsumerKind::Audio));
}
#[test]fn dynamic_expression_participates_in_existing_logic_diagnostics(){let parsed=parse("music.volume(missing_volume())").unwrap();let errors=rvn_parser::validate_logic(&parsed,false);assert!(errors.iter().any(|error|error.name=="missing_volume"));}
