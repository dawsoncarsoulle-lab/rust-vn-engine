use super::*;
const SCOPE:&str="g704/n1:init|exec_out=1@0";
const LOOP:&str="n2:for_each|exec_in=2@1,collection=3@0,body=4@0,completed=5@0,array_index=6@0,array_element=7@0";
const INDEX:&str="rvn_native_index_g704_n2";
fn script(body:&str)->Vec<Statement>{crate::parse(&format!("init{{flow.consume_v3(\"{SCOPE}\",\"{LOOP}\"){{for {INDEX} in [11,22]{{{body}}}}}}}\n")).unwrap()}
#[test]
fn native0704_binding_comes_from_actual_source_identity_and_real_port_roles(){
    let source=script(&format!("set seen={INDEX}"));validate_authored_bindings(&source).unwrap();let Statement::Init{body}=&source[0]else{unreachable!()};let b=binding(&body[0]).unwrap().unwrap();assert_eq!((b.graph,b.node,b.kind),(704,2,Kind::Array));assert_eq!(b.index,INDEX);assert_eq!(index_name(b.graph,b.node),INDEX);
    for name in [&b.index,&b.cursor,&b.phase]{assert!(crate::is_variable_binding_name(name));assert_eq!(crate::variable_binding_source_name(name).as_deref(),Some(name.as_str()));}
    // These are real parser checks, not AST-only acceptance of a reserved key.
    assert!(crate::parse("for __rvn_native_index_g704_n2 in [1]{}").is_err());
    assert!(crate::parse("flow.for_loop(__rvn_native_index_g704_n2,0,1){}").is_err());
    assert!(crate::parse("set __rvn_native_index_g704_n2=7").is_err());
    assert!(crate::parse("set `__rvn_native_index_g704_n2`=7").is_err());
    for value in [LOOP.replace("array_element=7@0","array_element=7@9"),LOOP.replace("array_index=6@0","value=6@0"),LOOP.replace("array_element=7@0","array_element=6@0")]{assert!(binding_for_identity(SCOPE,&value).is_err());}
}
#[test]
fn native0704_authored_source_cannot_write_private_cursor_or_escape_body_scope(){
    for body in [format!("set {INDEX}=9"),"set seen=rvn_native_cursor_g704_n2".into(),"set seen=rvn_native_phase_g704_n2".into()]{assert!(validate_authored_bindings(&script(&body)).is_err());}
    let mut source=script("");source.push(Statement::SetVar{name:"escaped".into(),value:Expr::Var(INDEX.into())});assert!(validate_authored_bindings(&source).is_err());
    let mut source=script("");let Statement::Init{body}=&mut source[0]else{unreachable!()};let Statement::ScheduledConsumer{body,..}=&mut body[0]else{unreachable!()};let Statement::ForEach{collection,..}=&mut body[0]else{unreachable!()};*collection=Expr::Var(INDEX.into());assert!(validate_authored_bindings(&source).is_err());
}
#[test]
fn native0704_exact_loop_alias_reservation_does_not_restrict_historical_scripts(){
    let old=crate::parse(&format!("set {INDEX}=7\nfor item in [1,2]{{set total=item}}\n")).unwrap();validate_authored_bindings(&old).unwrap();
    assert!(binding(&old[1]).unwrap().is_none());
    // Before a source3 declaration, all three new spellings are ordinary valid
    // legacy variables. No prefix-wide parser or runtime reservation is added.
    let old=crate::parse("set rvn_native_index_g704_n2=7 set rvn_native_cursor_g704_n2=8 set rvn_native_phase_g704_n2=9").unwrap();validate_authored_bindings(&old).unwrap();
    let lookalike=script("set rvn_native_index_g704_n3=9 set rvn_native_cursor_g704_n3=8 set rvn_native_phase_g704_n3=7");validate_authored_bindings(&lookalike).unwrap();
}
#[test]
fn native0704_array_element_capture_requires_actual_collection_and_active_loop_body(){
    let capture=|array:&str|format!("flow.consume_v1(\"{SCOPE}\",\"n4:set_variable|exec_in=8@2,name=9@0,value=10@0,exec_out=11@0,value_out=12@0\"){{capture \"{LOOP}\"={array}[{INDEX}] set seen=__rvn_captured_v1(\"{LOOP}\")}}");
    validate_authored_bindings(&script(&capture("[11,22]"))).unwrap();assert!(validate_authored_bindings(&script(&capture("[99,22]"))).is_err());
    let mut source=script("");let extra=crate::parse(&capture("[11,22]")).unwrap();source.extend(extra);assert!(validate_authored_bindings(&source).is_err());
}
#[test]
fn native0704_repeated_source_identity_uses_each_actual_body_not_global_id_equality(){
    let capture=|array:&str|format!("flow.consume_v1(\"{SCOPE}\",\"n4:set_variable|exec_in=8@2,name=9@0,value=10@0,exec_out=11@0,value_out=12@0\"){{capture \"{LOOP}\"={array}[{INDEX}] set seen=__rvn_captured_v1(\"{LOOP}\")}}");
    let mut first=script(&capture("[11,22]"));
    let second_raw=format!("init{{flow.consume_v3(\"{SCOPE}\",\"{LOOP}\"){{for {INDEX} in [99]{{{}}}}}}}\n",capture("[99]"));
    let second=crate::parse(&second_raw).unwrap();first.extend(second);validate_authored_bindings(&first).unwrap();
    // The same spelling in another root cannot authorize the first root's
    // capture to borrow that other source collection.
    let bad=script(&capture("[99]"));let mut combined=bad;combined.extend(crate::parse(&second_raw).unwrap());assert!(validate_authored_bindings(&combined).is_err());
}
#[test]
fn native0704_nested_same_source_identity_refuses_ambiguous_output_ownership(){
    let nested=format!("flow.consume_v3(\"{SCOPE}\",\"{LOOP}\"){{for {INDEX} in [99]{{set seen={INDEX}}}}}");
    assert!(validate_authored_bindings(&script(&nested)).is_err());
}
