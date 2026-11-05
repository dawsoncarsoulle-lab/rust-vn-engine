use super::*;
#[test]
fn unit_parser1003_explicit_source_has_distinct_resultless_definition_call_and_return(){
    let script=parse("function touch(n) -> () {if n>0{return} set seen=n}\ncall touch(2)").unwrap();
    assert!(matches!(&script[0],Statement::UnitFunction{name,parameters,body}if name=="touch"&&parameters==&["n"]&&matches!(&body[0],Statement::If{then_branch,..}if then_branch==&[Statement::UnitReturn])));
    assert!(matches!(&script[1],Statement::UnitCall{name,arguments}if name=="touch"&&arguments==&[Expr::Int(2)]));
    assert!(crate::validate_logic(&script,false).is_empty());
}
#[test]
fn unit_parser1003_old_value_and_narrative_ast_and_serde_keep_exact_variants(){
    let script=parse("function f(n){return n}\nlabel start\ncall child\nreturn\n").unwrap();
    assert!(matches!(&script[0],Statement::Function{body,..}if matches!(&body[0],Statement::FunctionReturn{..})));
    assert_eq!(script[2],Statement::Call{target:"child".into()});assert_eq!(script[3],Statement::Return);
    assert_eq!(serde_json::to_string(&Statement::Call{target:"child".into()}).unwrap(),r#"{"Call":{"target":"child"}}"#);
    assert_eq!(serde_json::to_string(&Statement::Return).unwrap(),"\"Return\"");
    assert!(parse("function f(){return}").is_err());
}
#[test]
fn unit_parser1003_new_syntax_cannot_weaken_screen_handler_value_or_label_contracts(){
    for source in ["screen f() -> () {}","handler f(event) -> () {}","function f() -> bool {}","function f() -> () {return 0}","function f() -> () {call child}","function f(){call other()}","function f() -> () {\"latent dialogue\"}"]{assert!(parse(source).is_err(),"{source}");}
    assert!(parse("function f() -> () {}").is_ok());
    // The source3 loop aliases remain owned by the actual lexical occurrence.
    // These assertions grant no Engine/runtime Unit admission.
    let scope="g772/n1:init|exec_out=1@0";
    let each="n2:for_each|exec_in=2@1,collection=3@0,body=4@0,completed=5@0,array_index=6@0,array_element=7@0";
    let alias=crate::scheduled_source::native_loop_contract::index_name(772,2);
    let native=|body:&str|format!("flow.consume_v3(\"{scope}\",\"{each}\"){{for {alias} in [11]{{{body}}}}}");
    let actual=parse(&native(&format!("call target({alias})"))).unwrap();
    crate::scheduled_source::native_loop_contract::validate_authored_bindings(&actual).unwrap();
    let outside=parse(&format!("{} call target({alias})",native(""))).unwrap();
    assert!(crate::scheduled_source::native_loop_contract::validate_authored_bindings(&outside).is_err());
    let parameter=parse(&format!("{} function target({alias})->(){{}}",native(""))).unwrap();
    assert!(crate::scheduled_source::native_loop_contract::validate_authored_bindings(&parameter).is_err());
    let lexical=parse(&format!("{} function target()->(){{set seen={alias} return}}",native(""))).unwrap();
    assert!(crate::scheduled_source::native_loop_contract::validate_authored_bindings(&lexical).is_err());
    // A forged nested declaration must still reset the lexical walk. It is
    // rejected here independently of the parser's root-declaration grammar.
    let mut nested=parse(&native("")).unwrap();
    let Statement::ScheduledConsumer{body,..}=&mut nested[0]else{panic!()};
    let Statement::ForEach{body,..}=&mut body[0]else{panic!()};
    body.push(Statement::UnitFunction{name:"nested".into(),parameters:vec![],body:vec![Statement::UnitCall{name:"target".into(),arguments:vec![Expr::Var(alias.clone())]}]});
    assert!(crate::scheduled_source::native_loop_contract::validate_authored_bindings(&nested).is_err());
    let declaration=Statement::UnitFunction{name:"target".into(),parameters:vec![],body:actual};
    assert!(crate::scheduled_source::native_loop_contract::contains_explicit_loop(&[declaration]));
    let valid=parse(&format!("{} function target(n)->(){{set seen=n return}}",native(""))).unwrap();
    crate::scheduled_source::native_loop_contract::validate_authored_bindings(&valid).unwrap();

}
#[test]
fn unit_parser1003_statement_calls_validate_actual_namespace_arity_and_arguments(){
    for source in [
        "function f(n)->(){} call f()",
        "function f(){return 1} call f()",
        "function f()->(){} function f(){return 1}",
        "function f()->(){} set x=f()",
        "function f()->(){} function g(n)->(){} call g(f())",
        "screen s(){return component(\"root\",\"column\",{},[])} call s()",
        "handler h(event){} call h(0)",
        "call abs(4)",
        "screen s(){return component(\"root\",\"column\",{},[])} set x=s()",
        "handler h(event){} set x=h(0)",
        "function f(n){return n} set x=f()",
    ] {
        let script=parse(source).unwrap();
        for external in [false,true]{assert!(!crate::validate_logic(&script,external).is_empty(),"external={external}: {source}");}
    }
    for source in ["call imported_unit(1)","set x=imported_value(1)"] {
        let script=parse(source).unwrap();assert!(!crate::validate_logic(&script,false).is_empty());assert!(crate::validate_logic(&script,true).is_empty());
    }
    let legacy=parse("function value(n){return n} function unit()->(){} set x=value(abs(-2)) call unit()").unwrap();
    for external in [false,true]{assert!(crate::validate_logic(&legacy,external).is_empty());}
}

#[test]
fn unit_parser1003_real_utf8_crlf_spans_keep_unit_body_and_comments_inert(){
    let source="// -> () call phantom()\r\nfunction touch(n) -> () {\r\n // é\r\n if n>0 { return }\r\n}\r\ncall touch(1)\r\n";
    let parsed=parse_with_origins(source,NarrativeContext::Legacy).unwrap();assert_eq!(parsed.script.len(),2);assert_eq!(parsed.spans.len(),2);
    assert!(source[parsed.spans[0].range.clone()].starts_with("function touch"));assert_eq!(parsed.spans[0].children.len(),1);
    assert_eq!(&source[parsed.spans[0].children[0].children[0].range.clone()],"return");
    let json=serde_json::to_string(&parsed.script).unwrap();let cold:Script=serde_json::from_str(&json).unwrap();assert_eq!(cold,parsed.script);
}
