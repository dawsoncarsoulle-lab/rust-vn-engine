use rvn_core::locale::{collect_strings_from_script,collect_strings_from_flat_script};
#[test]
fn unit_locale1003_definitions_and_statement_arguments_keep_existing_translation_keys(){
    let script=rvn_parser::parse(r#"
        function old(){return component("old","text",dict("text_key","legacy.key"),[])}
        function unused()->(){local tree=component("root","text",dict("text_key","unit.body.key"),[])}
        function accept(tree)->(){}
        label start
        call accept(component("argument","text",dict("text_key","unit.argument.key"),[]))
        "Dialogue"
    "#).unwrap();
    let expected=vec!["legacy.key","unit.body.key","unit.argument.key","Dialogue"];
    assert_eq!(collect_strings_from_script(&script),expected);
    assert_eq!(collect_strings_from_flat_script(&script),expected);
    // Discovery of a key neither invokes an unused definition nor creates a
    // separate locale/evaluation authority or a result value for Unit.
    assert!(matches!(&script[1],rvn_parser::Statement::UnitFunction{..}));
}
