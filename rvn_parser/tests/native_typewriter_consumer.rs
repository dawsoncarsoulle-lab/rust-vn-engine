use rvn_parser::{Statement,parse};

#[test]
fn scheduled_typewriter_preserves_both_literal_values_and_capture_order() {
    for enabled in [false,true] {
        let source=format!("flow.consume_v2(\"scope\",\"consumer\"){{capture \"draw\"=random(1,9) typewriter={enabled}}}");
        let parsed=parse(&source).unwrap();
        let [Statement::ScheduledConsumer{version,scope,consumer,steps,body}]=parsed.as_slice()else{panic!()};
        assert_eq!(*version,2);assert_eq!(scope,"scope");assert_eq!(consumer,"consumer");assert_eq!(steps.len(),1);
        assert!(matches!(body.as_slice(),[Statement::TypewriterSet{enabled:value}]if *value==enabled));
    }
}
#[test]
fn scheduled_typewriter_does_not_admit_v1_or_multiple_effects() {
    assert!(parse("flow.consume_v1(\"scope\",\"consumer\"){typewriter=true}").is_err());
    assert!(parse("flow.consume_v2(\"scope\",\"consumer\"){typewriter=true typewriter=false}").is_err());
}
#[test]
fn scheduled_typewriter_expression_shape_preserves_the_actual_typed_runtime_input() {
    // The shared18-family grammar now represents Speed, independently of Core
    // ownership/dispatch. Its new positive shape case lives in the common suite.
    // The new expression AST retains a real typed runtime input; Parser does
    // not invent a Boolean from a variable or numeric RNG result.
    for command in ["typewriter=enabled","typewriter=random(0,1)"] {
        let parsed=parse(&format!("flow.consume_v2(\"scope\",\"consumer\"){{{command}}}")).unwrap();
        let Statement::ScheduledConsumer{body,..}=&parsed[0] else{panic!()};
        assert!(matches!(body.as_slice(),[Statement::TypewriterSetValue{..}]));
    }
}
#[test]
fn unwrapped_typewriter_wire_remains_legacy_ast() {
    assert!(matches!(parse("typewriter=true\ntypewriter=false").unwrap().as_slice(),[Statement::TypewriterSet{enabled:true},Statement::TypewriterSet{enabled:false}]));
}
