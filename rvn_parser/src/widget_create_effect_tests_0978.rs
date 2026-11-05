//! Structural guards for an Engine effect, never execution of a pure factory.
use super::*;
fn leaf(raw: &str) -> Statement {
    if raw.starts_with("local ") {
        let wrapped = format!("function parser_factory_fixture(a,b,class_value) {{\n{raw}\n}}");
        let parsed = crate::parse(&wrapped).unwrap();
        assert_eq!(parsed.len(), 1);
        let Statement::Function { mut body, .. } = parsed.into_iter().next().unwrap() else {
            panic!("Local assignment fixture must retain its actual function owner");
        };
        assert_eq!(body.len(), 1);
        body.remove(0)
    } else {
        crate::parse(raw).unwrap().remove(0)
    }
}
#[test]
fn factory0978_real_assignment_retains_exact_borrowed_binding_asset_and_argument_expression() {
    let statement = leaf("local result=__rvn_widget_create_v1(__rvn_widget_class_v1(\"17411a3e431e4c9388b78a2b9825b130\"),[random(1,9)])");
    let input = parts(&statement).unwrap().unwrap();
    assert!(input.local);
    assert_eq!(input.binding, "result");
    assert!(matches!(
        input.class,
        ClassInput::Asset("17411a3e431e4c9388b78a2b9825b130")
    ));
    assert!(matches!(input.arguments,Expr::ListLit(values) if values.len()==1));
    assert_eq!(direct_inputs(&input).len(), 1);
    assert_eq!(argument_ordinal(&input, 0), 1);
}
#[test]
fn factory0978_class_data_uses_typed_runtime_input_while_explicit_none_is_distinct_from_missing_asset(
) {
    let dynamic = leaf("set result=__rvn_widget_create_v1(class_value,[])");
    let input = parts(&dynamic).unwrap().unwrap();
    assert!(matches!(input.class, ClassInput::Value(Expr::Var(_))));
    assert_eq!(direct_inputs(&input).len(), 2);
    assert_eq!(argument_ordinal(&input, 0), 0);
    assert_eq!(argument_ordinal(&input, 1), 1);
    let null = leaf("set result=__rvn_widget_create_v1(__rvn_widget_class_none_v1(),[])");
    assert!(matches!(
        parts(&null).unwrap().unwrap().class,
        ClassInput::Null
    ));
    let missing = leaf("set result=__rvn_widget_create_v1(__rvn_widget_class_v1(\"27411a3e431e4c9388b78a2b9825b130\"),[])");
    assert!(matches!(
        parts(&missing).unwrap().unwrap().class,
        ClassInput::Asset("27411a3e431e4c9388b78a2b9825b130")
    ));
}
#[test]
fn factory0978_strings_dictionaries_bad_arity_and_persistent_destination_do_not_admit_factory() {
    for raw in [
        "set out=__rvn_widget_create_v1(\"17411a3e431e4c9388b78a2b9825b130\",[])",
        "set out=__rvn_widget_create_v1({},[])",
        "set out=__rvn_widget_create_v1(__rvn_widget_class_v1(),[])",
        "set out=__rvn_widget_create_v1(__rvn_widget_class_none_v1(1),[])",
        "set out=__rvn_widget_create_v1(__rvn_widget_class_none_v1())",
        "set persistent.out=__rvn_widget_create_v1(__rvn_widget_class_none_v1(),[])",
    ] {
        assert!(parts(&leaf(raw)).is_err(), "{raw}");
    }
}
#[test]
fn factory0978_nested_read_is_not_a_direct_consumer_factory_instruction() {
    let statement = leaf("set other=[__rvn_widget_create_v1(__rvn_widget_class_none_v1(),[])]");
    assert!(parts(&statement).unwrap().is_none());
    assert!(
        is_reserved("__rvn_widget_create_v1")
            && is_reserved("__rvn_widget_class_v1")
            && is_reserved("__rvn_widget_class_none_v1")
    );
    assert!(!is_reserved("ui.open"));
}

#[test]
fn factory0980_public_homonyms_remain_user_functions_and_only_versioned_reserved_names_are_effects()
{
    let script=crate::parse(r#"
        function widget_create(a,b) {return a+b}
        function widget_class(a) {return a*2}
        function widget_class_none() {return 3}
        function customwrapper(a,b) {local result=widget_create(a,b) return result+widget_class(a)+widget_class_none()}
    "#).unwrap();
    assert!(crate::validate_logic(&script, false).is_empty());
    for name in ["widget_create", "widget_class", "widget_class_none"] {
        assert!(!is_reserved(name));
        assert!(crate::builtin_arity(name).is_none());
        assert!(crate::is_binding_name(name));
    }
    for raw in [
        "local result=widget_create(a,b)",
        "set result=widget_class(a)",
        "local result=widget_class_none()",
    ] {
        assert!(parts(&leaf(raw)).unwrap().is_none());
    }
    for name in [
        "__rvn_widget_create_v1",
        "__rvn_widget_class_v1",
        "__rvn_widget_class_none_v1",
    ] {
        assert!(is_reserved(name));
        assert!(crate::builtin_arity(name).is_some());
        assert!(!crate::is_binding_name(name));
    }
}
