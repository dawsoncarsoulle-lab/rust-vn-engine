//! Public constructor admission against actual authored source, not a synthetic Program.
use super::*;
const SCOPE: &str = "g770/n1:function_entry|exec_out=1@0";
const RANGE: &str = "n2:for_loop_with_break|exec_in=2@1,first=3@0,last=4@0,break=5@0,body=6@0,index=7@0,completed=8@0";
const INDEX: &str = "rvn_native_index_g770_n2";
const CURSOR: &str = "rvn_native_cursor_g770_n2";
const PHASE: &str = "rvn_native_phase_g770_n2";
fn native(first: &str, last: &str, body: &str) -> String {
    format!("flow.consume_v3(\"{SCOPE}\",\"{RANGE}\"){{flow.for_loop({INDEX},{first},{last}){{{body}}}}}")
}
fn source(parameters: &str, before: &str, first: &str, body: &str, after: &str) -> String {
    format!("function probe({parameters}){{{before} {} {after}}}", native(first,"2",body))
}
fn rejected(raw: &str, expected: &str) {
    let script=rvn_parser::parse(raw).expect("authored input must be accepted by the actual parser first");
    assert!(matches!(FunctionLibrary::from_script(&script),Err(EvalError::InvalidFunction(message)) if message.contains(expected)));
}
fn call(raw: &str) -> (HashMap<String,Value>,RandomState) {
    let script=rvn_parser::parse(raw).unwrap();
    let library=FunctionLibrary::from_script(&script).unwrap();
    let mut random=RandomState::seeded(770);
    let result=library.execute_in_consumer_frame(&rvn_parser::parse("set answer=probe()").unwrap(),&HashMap::new(),&mut random,&mut crate::consumer_schedule::ConsumerFrame::default()).unwrap();
    (result,random)
}

#[test]
fn native0770_constructor_rejects_actual_loop_private_parameter_collisions() {
    for name in [INDEX,CURSOR,PHASE] {
        rejected(&source(name,"","0","","return 1"),"Authored parameter collides");
    }
}

#[test]
fn native0770_constructor_checks_authored_handler_and_screen_parameter_contexts() {
    // A library built from a complete script must not skip the authored-source
    // check simply because only Function declarations enter its definition map.
    for root in ["handler","screen"] {
        let raw=format!("{} {root} other({INDEX}){{}}",source("","","0","","return 1"));
        rejected(&raw,"Authored parameter collides");
    }
}

#[test]
fn native0770_constructor_rejects_actual_private_local_and_assignment_targets() {
    for keyword in ["local","set"] {
        for name in [INDEX,CURSOR,PHASE] {
            rejected(&source("","","0",&format!("{keyword} {name}=99"),"return 1"),"Authored assignment collides");
        }
    }
}

#[test]
fn native0770_constructor_rejects_index_reads_after_its_actual_body() {
    rejected(&source("","","0","",&format!("return {INDEX}")),"outside its actual lexical body");
}

#[test]
fn native0770_constructor_rejects_self_header_and_cursor_phase_reads() {
    rejected(&source("","",INDEX,"","return 1"),"Loop header cannot borrow its own body index");
    for name in [CURSOR,PHASE] {
        rejected(&source("","","0",&format!("local seen={name}"),"return 1"),"outside its actual lexical body");
    }
}

#[test]
fn native0770_constructor_resets_alias_authority_at_real_function_boundary() {
    let raw=format!("function helper(){{return {INDEX}}} {}",source("","","0","local seen=helper()","return 1"));
    rejected(&raw,"outside its actual lexical body");
}

#[test]
fn native0770_constructor_admits_owned_index_and_unrelated_lookalike_names() {
    let raw=source("","local total=0 local rvn_native_index_g770_n3=10 local rvn_native_cursor_g770_n3=20 local rvn_native_phase_g770_n3=30","0",&format!("set total=total+{INDEX}"),"return total+rvn_native_index_g770_n3+rvn_native_cursor_g770_n3+rvn_native_phase_g770_n3");
    let (values,random)=call(&raw);
    assert_eq!(values,HashMap::from([("answer".into(),Value::Int(63))]));
    assert_eq!(random,RandomState::seeded(770));
}

#[test]
fn native0770_constructor_keeps_legacy_and_v2_alias_spellings_ordinary_without_v3_owner() {
    for v2 in [false,true] {
        let body="flow.for_loop(i,0,2){set total=total+i}";
        let body=if v2 {format!("flow.consume_v2(\"{SCOPE}\",\"{RANGE}\"){{{body}}}")}else{body.into()};
        let raw=format!("function probe(){{local total=0 local {INDEX}=10 local {CURSOR}=20 local {PHASE}=30 {body} return total+{INDEX}+{CURSOR}+{PHASE}}}");
        let (values,random)=call(&raw);
        assert_eq!(values,HashMap::from([("answer".into(),Value::Int(63))]));
        assert_eq!(random,RandomState::seeded(770));
    }
}

