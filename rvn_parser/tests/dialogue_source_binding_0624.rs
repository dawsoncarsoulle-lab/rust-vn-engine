//! Real source/reparse validation of the dynamic input codec. No value equality.
use rvn_parser::{Statement,scheduled_source::{dialogue_text_binding,scheduled_step_sources}};
fn source(output:&str,steps:&str,body:&str)->String{
    format!("scene label start\nflow.consume_v2(\"g9/n1:label|exec_out=1@0\",\"n2:dialogue|exec_in=2@1,character=3@0,text=4@2{output},exec_out=5@0\") {{\n{steps}\n{body}\n}}\n")
}
const GET:&str="capture \"n3:variable_get|value=6@0\" = message";
fn binding(raw:&str)->Result<Option<rvn_parser::scheduled_source::DialogueTextBinding>,String>{
    let program=rvn_parser::parse_program(raw).map_err(|e|e.to_string())?;
    scheduled_step_sources(&program)?;
    let plan=program.script.iter().find(|s|matches!(s,Statement::ScheduledConsumer{..})).unwrap();
    dialogue_text_binding(plan)
}
#[test]fn actual_source_output_binds_original_variable_slot(){let b=binding(&source("~6",GET,"\"[message]\"")).unwrap().unwrap();assert_eq!(b.authored_slot,0);assert_eq!(b.output_pin,6);assert_eq!(b.producer,"n3:variable_get|value=6@0");}
#[test]fn historical_unsuffixed_template_is_unchanged(){assert!(binding(&source("",GET,"\"[message]\"")).unwrap().is_none());}
#[test]fn foreign_missing_or_wrong_direction_output_is_refused(){for pin in [99,2,4]{assert!(binding(&source(&format!("~{pin}"),GET,"\"[message]\"")).is_err());}}
#[test]fn equal_values_never_substitute_another_authored_binding(){assert!(binding(&source("~6",GET,"\"[other]\"")).is_err());}
#[test]fn reroute_binds_its_exact_output_and_keeps_real_parent(){let steps=format!("{GET}\ncapture \"n4:reroute|value=7@3,value_out=8@0\" = __rvn_captured_v1(\"n3:variable_get|value=6@0\")");assert_eq!(binding(&source("~8",&steps,"\"[message]\"")).unwrap().unwrap().output_pin,8);}
#[test]fn inactive_forward_or_function_reroute_is_refused(){for expression in ["__rvn_captured_v1(\"not-active\")","random(1,9)"]{let steps=format!("{GET}\ncapture \"n4:reroute|value=7@3,value_out=8@0\" = {expression}");assert!(binding(&source("~8",&steps,"\"[message]\"")).is_err());}}
#[test]fn literal_or_multiple_authored_slots_are_not_mislabeled_as_variable_reads(){for body in ["\"literal\"","\"[message] [message]\""]{assert!(binding(&source("~6",GET,body)).is_err());}}
#[test]fn dynamic_role_is_separate_from_static_operands(){assert!(rvn_parser::scheduled_operand_contract::is_dynamic_source_role("dialogue","text"));assert!(!rvn_parser::scheduled_operand_contract::is_static_operand_role("dialogue","text"));assert!(rvn_parser::scheduled_operand_contract::is_static_operand_role("dialogue","character"));}
