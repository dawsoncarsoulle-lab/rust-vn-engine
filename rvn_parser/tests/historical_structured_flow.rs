// PRIVATE, NOT_RUN. Actual parser/spans and typed AST.
use rvn_parser::{parse,parse_with_origins,NarrativeContext,Statement as S};
#[test]
fn flow_parser_preserves_order_empty_branches_and_unicode_crlf_origins(){
    let source="// é🌿\r\nscene label start\r\nflow.sequence() {\r\n{ set a = 1 }\r\n{}\r\n{ flow.for_loop(index, -2, 2) { if index == 0 { flow.break() } } }\r\n}\r\n";
    let parsed=parse_with_origins(source,NarrativeContext::ExplicitScenes).unwrap();
    let S::Sequence{branches}=&parsed.script[1] else{panic!("typed sequence absent")};
    assert_eq!(branches.len(),3);assert!(branches[1].is_empty());
    let span=&parsed.spans[1];assert_eq!(&source[span.range.clone()],source.split_once("flow.sequence").unwrap().1.strip_suffix("\r\n").map(|tail|format!("flow.sequence{tail}")).unwrap());
    assert_eq!(span.children.len(),2);
    assert!(matches!(branches[2][0],S::ForLoopWithBreak{..}));
}
#[test]
fn flow_break_refuses_outside_lexical_range_at_real_source(){
    for tail in ["flow.break()","while true { flow.break() }","for item in [] { flow.break() }"]{
        let source=format!("// é🌿\r\n{tail}\r\n");let error=parse(&source).unwrap_err();
        assert_eq!(error.location.line,2);assert_eq!(error.location.col,tail.find("flow.break()").unwrap()+12);assert_eq!(error.source_line,tail);assert!(error.to_string().contains("ForLoopWithBreak"));
    }
}
#[test]
fn flow_sequence_arity_reserved_bindings_and_legacy_identifiers_are_checked(){
    for source in ["flow.sequence() {}","flow.for_loop(__rvn_cursor,0,1) {}","flow.for_loop(i,0) {}","flow.break(1)"]{assert!(parse(source).is_err(),"{source}");}
    let many=format!("flow.sequence() {{{}}}","{}".repeat(33));assert!(parse(&many).is_err());
    assert!(parse("set sequence = 2\nset break = 3\nfor item in [1] {set total = item}\nwhile false {}\n").is_ok());
    assert!(matches!(&parse("flow.show(\"neutral\")").unwrap()[0],S::ShowSprite{character_id,..}if character_id=="flow"));
    assert!(parse("function invalid() {flow.show(\"neutral\") return 1}").is_err());
}
#[test]
fn flow_function_body_is_typed_without_widening_narrative_function_rules(){
    let source="function sum() { flow.sequence() { {set x=0} {flow.for_loop(i,0,2) {set x=x+i}} } return x }";
    let script=parse(source).unwrap();let S::Function{body,..}=&script[0] else{panic!()};assert!(matches!(body[0],S::Sequence{..}));
    assert!(parse("function invalid() {flow.sequence() {{\"Narrative\"}} return 1}").is_err());
}
