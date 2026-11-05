//! Source-map prerequisite tests, independently selected from live execution.
use super::*;

#[test]
fn unit_debug_sites1021_actual_crlf_utf8_unit_routes_and_legacy_function_sites_are_collected() {
    let source="// été 🍃\r\nfunction value(n){return n+1}\r\nfunction effect(`quoted parameter`)->(){if `quoted parameter`>0 {flow.sequence() {{set result=1} {return}}}}\r\nscene label start\r\ncall effect(1)\r\n\"After\"\r\nreturn\r\n";
    let program=rvn_parser::parse_program(source).unwrap();
    let map=crate::debugger_provenance::SourceMap::from_program(&program).unwrap();
    let root=program.script.iter().find(|statement|matches!(statement,Statement::UnitFunction{name,..}if name=="effect")).unwrap();
    assert_eq!(declaration_scope(root).as_deref(),Some("effect"));
    let expected=[("set result=1",vec![Route::Statement(0),Route::Then,Route::Statement(0),Route::Branch(0),Route::Statement(0)]),
        ("return",vec![Route::Statement(0),Route::Then,Route::Statement(0),Route::Branch(1),Route::Statement(0)])];
    for (spelling,route) in expected {
        let actual=map.sites.iter().find(|site|site.scope=="effect"&&site.route==route).unwrap();
        assert_eq!(source.get(actual.origin.span.clone().unwrap()),Some(spelling));
        assert!(validate_site(root,&Site{function:Some("effect".into()),route,depth:1,statement:true}));
        assert!(actual.origin.graph.is_none(),"plain source does not manufacture a Graph/Node/Pin identity");
    }
    assert!(map.sites.iter().any(|site|site.scope=="value"&&site.route==vec![Route::Statement(0)]));
    assert!(!validate_site(root,&Site{function:None,route:vec![Route::Statement(0)],depth:0,statement:true}),"a declaration is not a live executable root");
    assert!(!validate_site(root,&Site{function:Some("foreign".into()),route:vec![Route::Statement(0)],depth:1,statement:true}));
}

#[test]
fn unit_debug_sites1021_forged_or_truncated_actual_unit_children_refuse_source_map() {
    let mut program=rvn_parser::parse_program("function effect(n)->(){if n>0 {set result=n return}}\nscene label start\nreturn").unwrap();
    let declaration=program.script.iter().position(|statement|matches!(statement,Statement::UnitFunction{..})).unwrap();
    let before=program.clone();
    program.origins[declaration].span.children.clear();
    assert!(crate::debugger_provenance::SourceMap::from_program(&program).is_err());
    let mut foreign=before;foreign.origins[declaration].span.children[0].range=0..1;
    assert!(crate::debugger_provenance::SourceMap::from_program(&foreign).is_err());
}

#[test]
fn unit_debug_sites1021_metadata_collection_does_not_grant_live_debug_or_save_authority() {
    let program=rvn_parser::parse_program("init {set result=0}\nfunction effect(n)->(){set result=n}\nscene label start\ncall effect(7)\n\"After\"\nreturn").unwrap();
    let map=crate::debugger_provenance::SourceMap::from_program(&program).unwrap();
    assert!(map.sites.iter().any(|site|site.scope=="effect"));
    let mut engine=crate::Engine::from_program(program,crate::TerminalRenderer,8).unwrap();
    engine.enter_label("start").unwrap();
    let state=serde_json::to_vec(&engine.state).unwrap();let random=engine.state.random;
    assert!(engine.debug_site_page(0).is_err());
    assert!(engine.debug_begin(crate::engine::DebugOperation::Statement,vec![]).is_err());
    assert!(engine.snapshot_for_save().is_err());
    assert_eq!(serde_json::to_vec(&engine.state).unwrap(),state);assert_eq!(engine.state.random,random);
}
