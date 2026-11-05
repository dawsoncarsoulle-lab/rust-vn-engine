//! Real catalogue pins, source identities and strict compiler refusal; CPU only.
use super::*;
use rvn_parser::{
    function_library_asset1023::{
        FunctionLibraryAsset, FunctionLibraryId, FunctionLibraryMember, LibraryFunctionAccess,
    },
    function_library_program1023::FunctionLibraryScope,
};
const A: &str = "11111111111141118111111111111111";
const P: &str = "33333333333343338333333333333333";
fn resources(unit: bool, private: bool) -> (Vec<FunctionLibraryScope>, Vec<FunctionLibraryAsset>) {
    let body = if unit {
        "function target(n)->(){set result=n}"
    } else {
        "function target(n){return 3}"
    };
    let source =
        format!("functions.library_v1(\"{A}\"){{functions.member_v1(\"11\",[\"{P}\"]){{{body}}}}}");
    let script = rvn_parser::parse(&source).unwrap();
    let scopes = script
        .iter()
        .filter_map(|s| {
            if let Statement::FunctionLibrary { scope } = s {
                Some(scope.clone())
            } else {
                None
            }
        })
        .collect();
    let assets = vec![FunctionLibraryAsset::new(
        FunctionLibraryId::parse(A).unwrap(),
        vec![FunctionLibraryMember {
            graph_id: 11,
            pure: !unit,
            access: if private {
                LibraryFunctionAccess::Private
            } else {
                LibraryFunctionAccess::Public
            },
        }],
    )
    .unwrap()];
    (scopes, assets)
}
fn target() -> QualifiedLibraryFunction {
    QualifiedLibraryFunction {
        library: FunctionLibraryId::parse(A).unwrap(),
        graph_id: 11,
    }
}
fn diagnosed() -> GraphDocument {
    let mut graph = GraphDocument::new(
        GraphId::new(100),
        GraphKind::Function {
            name: "broken".into(),
        },
    );
    let entry = graph
        .add_catalog_node(NodeKind::FunctionEntry, [0., 0.])
        .unwrap();
    let result = graph
        .add_catalog_node(NodeKind::FunctionReturn, [256., 0.])
        .unwrap();
    let a = graph.pin_by_key(entry, "exec_out").unwrap().id;
    let b = graph.pin_by_key(result, "exec_in").unwrap().id;
    graph.connect(a, b).unwrap();
    // An absent return value is a strict compiler error, not a structural
    // validate_document diagnostic. Author a real unresolved read so the
    // structural authoring test actually has a diagnostic to preserve.
    let unresolved = graph.add_catalog_node(NodeKind::VariableGet, [0., 150.]).unwrap();
    graph.set_property(unresolved, "name", PropertyValue::String("missing_current1027".into())).unwrap();
    graph
}
#[test]
fn library_call_authoring1027_empty_value_args_keep_diagnostics_ids_and_roundtrip_without_default_values(
) {
    let (scopes, assets) = resources(false, false);
    let resources = LibraryGraphResources1027::prepare(&scopes, &assets).unwrap();
    let graph = diagnosed();
    let diagnostics = graph.validate();
    assert!(diagnostics
        .iter()
        .any(|d| d.severity == DiagnosticSeverity::Error));
    assert!(diagnostics.iter().any(|d| d.code == "unknown_variable"));
    let before = graph.to_pretty_json().unwrap();
    let (next, node) = graph
        .prepare_library_call_node1027(target(), [400., 100.], &resources)
        .unwrap();
    assert_eq!(graph.to_pretty_json().unwrap(), before);
    for diagnostic in &diagnostics {
        assert!(
            next.validate().contains(diagnostic),
            "existing diagnostic lost: {diagnostic:?}"
        );
    }
    let call = &next.library1027.as_ref().unwrap().calls[&node];
    assert_eq!(call.target, target());
    assert_eq!(call.arguments.len(), 1);
    assert_eq!(call.arguments[0].0.as_str(), P);
    let pin = &next.pins[&call.arguments[0].1];
    assert_eq!(pin.key, "item_0");
    assert_eq!(pin.default_value, None);
    assert_eq!(pin.value_type, ValueType::Any);
    assert_eq!(
        next.pin_by_key(node, "result").unwrap().value_type,
        ValueType::Int
    );
    assert_eq!(
        GraphDocument::from_json(&next.to_pretty_json().unwrap()).unwrap(),
        next
    );
    let mut cohort=import_library_program1027(&vec![Statement::FunctionLibrary{scope:scopes[0].clone()}],&resources,&[]).unwrap();
    cohort.push(next.clone());
    assert!(
        transpile_library_project1027(&cohort, &resources).is_err(),
        "structural authoring does not grant compilation to incomplete logic"
    );
    let mut completed=next;
    let unresolved=completed.nodes.values().find(|n|n.kind==NodeKind::VariableGet&&n.properties.get("name")==Some(&PropertyValue::String("missing_current1027".into()))).unwrap().id;
    assert!(completed.remove_node(unresolved).is_some(), "repair the same original diagnosed node before strict Compile");
    let result=completed.nodes.values().find(|n|n.kind==NodeKind::FunctionReturn).unwrap().id;
    let output=completed.pin_by_key(node,"result").unwrap().id;
    let input=completed.pin_by_key(result,"value").unwrap().id;
    completed.connect(output,input).unwrap();
    completed.set_literal_input(node,"item_0",PropertyValue::Int(5)).unwrap();
    *cohort.last_mut().unwrap()=completed;
    let emitted=transpile_library_project1027(&cohort,&resources).unwrap();
    let actual=emitted.ast.iter().find_map(|s|if let Statement::Function{name,body,..}=s{if name=="broken"{body.iter().find_map(|s|if let Statement::FunctionReturn{value:Expr::LibraryCall{call}}=s{Some(call)}else{None})}else{None}}else{None}).unwrap();
    assert_eq!(actual.reference(),&target());
    assert_eq!(actual.arguments()[0].0.as_str(),P);
    assert_eq!(actual.arguments()[0].1,Expr::Int(5));
    let functions=rvn_core::eval::FunctionLibrary::from_script_with_library_assets(&emitted.ast,&assets).unwrap();
    assert_eq!(functions.eval(&Expr::Call{name:"broken".into(),args:Vec::new()},&Default::default()).unwrap(),rvn_parser::Value::Int(3));
}
#[test]
fn library_call_authoring1027_unit_uses_actual_exec_ports_and_parameter_uuid_without_allocating_semantic_ids(
) {
    let (scopes, assets) = resources(true, false);
    let resources = LibraryGraphResources1027::prepare(&scopes, &assets).unwrap();
    let graph = diagnosed();
    let (next, node) = graph
        .prepare_library_call_node1027(target(), [400., 100.], &resources)
        .unwrap();
    assert_eq!(next.nodes[&node].kind, NodeKind::UnitCall);
    assert!(next.pin_by_key(node, "exec_in").is_some());
    assert!(next.pin_by_key(node, "exec_out").is_some());
    let call = &next.library1027.as_ref().unwrap().calls[&node];
    let pin = &next.pins[&call.arguments[0].1];
    assert_eq!(pin.key, format!("argument_{P}"));
    assert_eq!(pin.parameter_id.as_ref().unwrap().as_str(), P);
    assert!(pin.default_value.is_none());
    assert!(next.nodes[&node]
        .pins
        .iter()
        .all(|id| next.pins[id].direction != PinDirection::Output
            || next.pins[id].value_type == ValueType::Execution));
    assert_eq!(
        GraphDocument::from_json(&next.to_pretty_json().unwrap()).unwrap(),
        next
    );
}
#[test]
fn library_call_authoring1027_private_retired_owner_and_bad_position_refuse_before_mutating_original(
) {
    let (scopes, assets) = resources(false, true);
    let resources = LibraryGraphResources1027::prepare(&scopes, &assets).unwrap();
    let graph = diagnosed();
    let before = graph.to_pretty_json().unwrap();
    for target in [
        target(),
        QualifiedLibraryFunction {
            library: FunctionLibraryId::parse(A).unwrap(),
            graph_id: 999,
        },
        QualifiedLibraryFunction {
            library: FunctionLibraryId::parse("22222222222242228222222222222222").unwrap(),
            graph_id: 11,
        },
    ] {
        assert!(graph
            .prepare_library_call_node1027(target, [0., 0.], &resources)
            .is_err());
        assert_eq!(graph.to_pretty_json().unwrap(), before);
    }
    assert!(graph
        .prepare_library_call_node1027(target(), [f64::NAN, 0.], &resources)
        .is_err());
    assert_eq!(graph.to_pretty_json().unwrap(), before);
}
