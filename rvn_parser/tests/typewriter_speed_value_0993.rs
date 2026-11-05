use rvn_parser::{parse,Expr,Statement};
#[test] fn speed0993_literal_unsigned_ast_and_serde_stay_exact_without_overflow_wrap() {
 for speed in [0,30,u32::MAX] {let p=parse(&format!("typewriter.speed({speed})")).unwrap();assert_eq!(p,vec![Statement::TypewriterSpeed{chars_per_sec:speed}]);assert_eq!(serde_json::to_string(&p[0]).unwrap(),format!("{{\"TypewriterSpeed\":{{\"chars_per_sec\":{speed}}}}}"));}
 for speed in ["-1","4294967296","9223372036854775807"] {assert!(parse(&format!("typewriter.speed({speed})")).is_err(),"{speed}");}
}
#[test] fn speed0993_expressions_are_genuine_ast_with_no_editor_evaluation() {
 for speed in ["rate","rate+1","draw()","__rvn_captured_v1(\"actual\")"] {assert!(matches!(parse(&format!("typewriter.speed({speed})")).unwrap().as_slice(),[Statement::TypewriterSpeedValue{..}]));}
 let p=parse("typewriter.speed(-rate)").unwrap();assert!(matches!(&p[0],Statement::TypewriterSpeedValue{chars_per_sec:Expr::Neg(_)}));
 for speed in ["","rate+","draw(,)"] {assert!(parse(&format!("typewriter.speed({speed})")).is_err());}
}
#[test] fn speed0993_shared_classic_context_and_logic_contracts_remain_strict() {
 let p=parse("flow.consume_v2(\"s\",\"c\"){capture \"draw\"=random(1,9) typewriter.speed(__rvn_captured_v1(\"draw\"))}").unwrap();let Statement::ScheduledConsumer{steps,body,..}=&p[0]else{panic!()};assert_eq!(steps.len(),1);assert_eq!(rvn_parser::scheduled_consumer_kind(&body[0]),Some(rvn_parser::ScheduledConsumerKind::Classic(rvn_parser::ClassicConsumerKind::TypewriterSpeed)));
 assert!(parse("flow.consume_v1(\"s\",\"c\"){typewriter.speed(rate)}").is_err());
 for text in ["function f(){typewriter.speed(rate)}","handler f(event){typewriter.speed(rate)}","screen f(){typewriter.speed(rate) return []}"] {assert!(rvn_parser::parse_program(text).is_err(),"{text}");}
 let p=parse("typewriter.speed(missing_speed())").unwrap();assert!(rvn_parser::validate_logic(&p,false).iter().any(|e|e.name=="missing_speed"));
}
