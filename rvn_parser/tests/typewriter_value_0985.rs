use rvn_parser::{parse, Expr, Statement};
#[test] fn typewriter0985_literal_bool_ast_and_serde_remain_exact() {
 for enabled in [false,true] {
  let actual=parse(&format!("typewriter={enabled}")).unwrap();
  assert_eq!(actual,vec![Statement::TypewriterSet{enabled}]);
  assert_eq!(serde_json::to_string(&actual[0]).unwrap(),format!("{{\"TypewriterSet\":{{\"enabled\":{enabled}}}}}"));
 }
}
#[test] fn typewriter0985_expressions_are_genuine_ast_and_malformed_inputs_refuse() {
 for value in ["enabled","not enabled","draw()","1<2","__rvn_captured_v1(\"actual\")"] {
  assert!(matches!(parse(&format!("typewriter={value}")).unwrap().as_slice(),[Statement::TypewriterSetValue{..}]));
 }
 for value in ["","not","draw(,)","enabled+"] {assert!(parse(&format!("typewriter={value}")).is_err());}
 let p=parse("typewriter=not enabled").unwrap();assert!(matches!(&p[0],Statement::TypewriterSetValue{enabled:Expr::Not(_)}));
}
#[test] fn typewriter0985_shared_classic_shape_keeps_real_context_refusals() {
 let p=parse("flow.consume_v2(\"s\",\"c\"){capture \"draw\"=not false typewriter=__rvn_captured_v1(\"draw\")}").unwrap();
 let Statement::ScheduledConsumer{body,steps,..}=&p[0] else{panic!()};assert_eq!(steps.len(),1);
 assert_eq!(rvn_parser::scheduled_consumer_kind(&body[0]),Some(rvn_parser::ScheduledConsumerKind::Classic(rvn_parser::ClassicConsumerKind::TypewriterSet)));
 assert!(parse("flow.consume_v1(\"s\",\"c\"){typewriter=enabled}").is_err());
 for text in ["function f(){typewriter=enabled}","handler f(event){typewriter=enabled}","screen f(){typewriter=enabled return []}"] {assert!(rvn_parser::parse_program(text).is_err(),"{text}");}
}
#[test] fn typewriter0985_expression_functions_participate_in_real_logic_validation() {
 let p=parse("typewriter=missing_bool()").unwrap();assert!(rvn_parser::validate_logic(&p,false).iter().any(|e|e.name=="missing_bool"));
}
