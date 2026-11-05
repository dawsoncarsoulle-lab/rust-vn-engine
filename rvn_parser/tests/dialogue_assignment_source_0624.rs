//! Whole parsed-source authority for a SET output, including a later SET.
use rvn_parser::scheduled_source::scheduled_step_sources;
const SCOPE:&str="g9/n1:label|exec_out=1@0";
const SET:&str="n4:set_variable|exec_in=7@3,name=8@0,value=9@0,exec_out=10@0,value_out=11@0";
fn source(assignment_name:Option<&str>,assignment_after:bool)->String{
    let dialogue=format!("flow.consume_v2(\"{SCOPE}\",\"n2:dialogue|exec_in=2@1,character=3@0,text=4@2~11,exec_out=5@0\"){{capture \"{SET}\"=message \"[message]\"}}\n");
    let assignment=assignment_name.map(|name|format!("flow.consume_v1(\"{SCOPE}\",\"{SET}\"){{set {name}=\"after\"}}\n")).unwrap_or_default();
    format!("scene label start\n{}{}",if assignment_after{&dialogue}else{&assignment},if assignment_after{&assignment}else{&dialogue})
}
#[test]fn real_assignment_before_reader_is_source_bound(){let p=rvn_parser::parse_program(&source(Some("message"),false)).unwrap();assert!(scheduled_step_sources(&p).is_ok());}
#[test]fn real_assignment_after_reader_is_source_bound_without_execution_order_guess(){let p=rvn_parser::parse_program(&source(Some("message"),true)).unwrap();assert!(scheduled_step_sources(&p).is_ok());}
#[test]fn missing_assignment_never_lends_a_set_identity_to_a_read(){let p=rvn_parser::parse_program(&source(None,true)).unwrap();assert!(scheduled_step_sources(&p).is_err());}
#[test]fn other_variable_assignment_cannot_authorize_equal_value_read(){let p=rvn_parser::parse_program(&source(Some("other"),true)).unwrap();assert!(scheduled_step_sources(&p).is_err());}
