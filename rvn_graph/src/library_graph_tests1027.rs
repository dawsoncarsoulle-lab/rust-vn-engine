//! Real Parser -> existing Graph -> scoped source -> existing Core admission.
//! Definitions only; no editor publication, native ACK or pixel acceptance.
use super::*;
use rvn_parser::function_library_asset1023::{
    FunctionLibraryMember, LibraryFunctionAccess as Access,
};
const A: &str = "11111111111141118111111111111111";
const B: &str = "22222222222242228222222222222222";
const P: &str = "33333333333343338333333333333333";
const Q: &str = "44444444444444448444444444444444";
fn asset(owner: &str, rows: &[(u64, Access, bool)]) -> FunctionLibraryAsset {
    FunctionLibraryAsset::new(
        FunctionLibraryId::parse(owner).unwrap(),
        rows.iter()
            .map(|(graph_id, access, pure)| FunctionLibraryMember {
                graph_id: *graph_id,
                access: *access,
                pure: *pure,
            })
            .collect(),
    )
    .unwrap()
}
fn scope(owner: &str, graph: u64, ids: &[&str], body: &str) -> String {
    format!(
        "functions.library_v1(\"{owner}\"){{functions.member_v1(\"{graph}\",{}){{{body}}}}}\n",
        serde_json::to_string(ids).unwrap()
    )
}
fn value(owner: &str, graph: u64, args: &str) -> String {
    format!("functions.value_v1(\"{owner}\",\"{graph}\",{{{args}}})")
}
fn source_scopes(script: &rvn_parser::Script) -> Vec<FunctionLibraryScope> {
    script
        .iter()
        .filter_map(|s| {
            if let Statement::FunctionLibrary { scope } = s {
                Some(scope.clone())
            } else {
                None
            }
        })
        .collect()
}
fn expression(script: &[Statement], name: &str) -> Expr {
    script
        .iter()
        .find_map(|s| {
            if let Statement::SetVar { name: n, value } = s {
                (n == name).then(|| value.clone())
            } else {
                None
            }
        })
        .unwrap()
}

#[test]
fn library_graph1027_two_actual_homonyms_roundtrip_compile_to_original_core_without_global_alias() {
    let source = scope(A, 11, &[P], "function same(n){return n+1}")
        + &scope(B, 22, &[Q], "function same(n){return n+2}")
        + &format!(
            "scene label start\nset first={}\nset second={}\n\"After\"",
            value(A, 11, &format!("\"{P}\":3")),
            value(B, 22, &format!("\"{Q}\":3"))
        );
    let parsed = rvn_parser::parse_program(&source).unwrap();
    let scopes = source_scopes(&parsed.script);
    let assets = [
        asset(A, &[(11, Access::Public, true)]),
        asset(B, &[(22, Access::Public, true)]),
    ];
    let resources = LibraryGraphResources1027::prepare(&scopes, &assets).unwrap();
    let graphs = import_library_program1027(&parsed.script, &resources, &[]).unwrap();
    assert_eq!(
        graphs
            .iter()
            .filter(|g| g
                .library1027
                .as_ref()
                .and_then(|m| m.owner.as_ref())
                .is_some())
            .count(),
        2
    );
    let restored = graphs
        .iter()
        .map(|g| GraphDocument::from_json(&g.to_pretty_json().unwrap()).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(restored, graphs);
    let compiled = transpile_library_project1027(&restored, &resources).unwrap();
    let program = rvn_parser::parse_program(&compiled.source).unwrap();
    let prepared =
        rvn_core::engine::PreparedFunctionLibraryProgram1023::prepare(program, &assets).unwrap();
    assert_eq!(source_scopes(&prepared.program().script), scopes);
    let library =
        rvn_core::eval::FunctionLibrary::from_script_with_library_assets(&compiled.ast, &assets)
            .unwrap();
    assert_eq!(
        library
            .eval(&expression(&compiled.ast, "first"), &Default::default())
            .unwrap(),
        rvn_parser::Value::Int(4)
    );
    assert_eq!(
        library
            .eval(&expression(&compiled.ast, "second"), &Default::default())
            .unwrap(),
        rvn_parser::Value::Int(5)
    );
    assert!(
        !compiled
            .ast
            .iter()
            .any(|s| matches!(s,Statement::Function{name,..}if name=="same")),
        "no equal-name Library member escapes into the global namespace"
    );
    assert!(transpile_project(&graphs).is_err());
    assert!(crate::import_script(&parsed.script).is_err());
}

#[test]
fn library_graph1027_retired_owner_private_scope_and_old_schema_refuse_before_mutating_graph() {
    let source = scope(A, 11, &[], "function same(){return 7}")
        + &scope(B, 22, &[], "function secret(){return 9}")
        + &format!(
            "scene label start\nset first={}\n\"After\"",
            value(A, 11, "")
        );
    let parsed = rvn_parser::parse(&source).unwrap();
    let scopes = source_scopes(&parsed);
    let assets = [
        asset(A, &[(11, Access::Public, true)]),
        asset(B, &[(22, Access::Private, true)]),
    ];
    let resources = LibraryGraphResources1027::prepare(&scopes, &assets).unwrap();
    let graphs = import_library_program1027(&parsed, &resources, &[]).unwrap();
    let mut caller = graphs
        .iter()
        .find(|g| matches!(&g.kind,GraphKind::Label{name}if name=="start"))
        .unwrap()
        .clone();
    let node = *caller
        .library1027
        .as_ref()
        .unwrap()
        .calls
        .keys()
        .next()
        .unwrap();
    let before = caller.to_pretty_json().unwrap();
    for target in [
        QualifiedLibraryFunction {
            library: FunctionLibraryId::parse(B).unwrap(),
            graph_id: 22,
        },
        QualifiedLibraryFunction {
            library: FunctionLibraryId::parse(A).unwrap(),
            graph_id: 999,
        },
    ] {
        assert!(caller
            .set_library_call_target1027(node, target, &resources)
            .is_err());
        assert_eq!(caller.to_pretty_json().unwrap(), before);
    }
    assert!(caller.resize_value_inputs(node, 1).is_err());
    assert!(caller
        .set_property(node, "function", PropertyValue::String("secret".into()))
        .is_err());
    assert_eq!(
        caller.to_pretty_json().unwrap(),
        before,
        "legacy authoring cannot silently erase or retarget a nominal call"
    );
    for schema in [5, 14, 15] {
        let mut wire = serde_json::to_value(&caller).unwrap();
        wire["schema_version"] = serde_json::Value::from(schema);
        assert!(
            GraphDocument::from_json(&wire.to_string()).is_err(),
            "nominal serialized metadata cannot masquerade under an old schema"
        );
    }
    let mut forged = caller.clone();
    forged.library1027.as_mut().unwrap().owner = Some(FunctionLibraryId::parse(B).unwrap());
    assert!(
        resources.validate_graph(&forged).is_err(),
        "a label cannot claim another actual Library caller frame"
    );
    let original = caller.clone();
    caller.remove_node(node).unwrap();
    assert!(!caller
        .library1027
        .as_ref()
        .unwrap()
        .calls
        .contains_key(&node));
    caller.validate_library_metadata1027().unwrap();
    caller = original;
    assert_eq!(
        caller.to_pretty_json().unwrap(),
        before,
        "whole-document transaction replay restores the exact typed target alongside original pins"
    );
}

#[test]
fn library_graph1027_actual_unit_source_preserves_parameter_ids_and_completion_shape() {
    let source = scope(
        A,
        11,
        &[P, Q],
        "function apply(left,right)->(){set result=left*100+right}",
    ) + &format!(
        "scene label start\nfunctions.invoke_v1(\"{A}\",\"11\",{{\"{Q}\":2,\"{P}\":1}})\n\"After\""
    );
    let parsed = rvn_parser::parse(&source).unwrap();
    let scopes = source_scopes(&parsed);
    let assets = [asset(A, &[(11, Access::Public, false)])];
    let resources = LibraryGraphResources1027::prepare(&scopes, &assets).unwrap();
    let graphs = import_library_program1027(&parsed, &resources, &[]).unwrap();
    let unit = graphs.iter().find(|g| g.graph_id.get() == 11).unwrap();
    assert_eq!(unit.schema_version, 16);
    let ids = crate::unit_graph_source1003::parameters(unit)
        .unwrap()
        .iter()
        .map(|p| p.parameter_id.as_ref().unwrap().as_str())
        .collect::<Vec<_>>();
    assert_eq!(ids, [P, Q]);
    let compiled = transpile_library_project1027(&graphs, &resources).unwrap();
    let invocation = compiled
        .ast
        .iter()
        .find_map(|s| {
            if let Statement::LibraryUnitCall { call } = s {
                Some(call)
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(
        invocation
            .arguments()
            .iter()
            .map(|(id, _)| id.as_str())
            .collect::<Vec<_>>(),
        [P, Q]
    );
    let caller = graphs
        .iter()
        .find(|g| g.library1027.as_ref().is_some_and(|m| !m.calls.is_empty()))
        .unwrap();
    let node = *caller
        .library1027
        .as_ref()
        .unwrap()
        .calls
        .keys()
        .next()
        .unwrap();
    assert!(
        caller.nodes[&node].pins.iter().all(|id| {
            let p = &caller.pins[id];
            p.direction != PinDirection::Output || p.value_type == ValueType::Execution
        }),
        "Unit remains completion-only"
    );
    let legacy = crate::import_script(
        &rvn_parser::parse("function plain()->(){return}\nlabel start\ncall plain()\n\"After\"")
            .unwrap(),
    )
    .unwrap();
    assert!(legacy
        .iter()
        .filter(|g| matches!(g.kind, GraphKind::UnitFunction { .. })
            || g.nodes.values().any(|n| n.kind == NodeKind::UnitCall))
        .all(|g| g.schema_version == 14 && g.library1027.is_none()));
    crate::transpile_project(&legacy).unwrap();
}

#[test]
fn library_graph1027_source_rename_and_reorder_keep_original_member_and_parameter_pin_mapping() {
    let assets = [asset(A, &[(11, Access::Public, true)])];
    let make = |name: &str, ids: &[&str], parameters: &str| {
        scope(
            A,
            11,
            ids,
            &format!("function {name}({parameters}){{return left*100+right}}"),
        ) + &format!(
            "scene label start\nset first={}\n\"After\"",
            value(A, 11, &format!("\"{P}\":1,\"{Q}\":2"))
        )
    };
    let first = rvn_parser::parse(&make("same", &[P, Q], "left,right")).unwrap();
    let s1 = source_scopes(&first);
    let r1 = LibraryGraphResources1027::prepare(&s1, &assets).unwrap();
    let original = import_library_program1027(&first, &r1, &[]).unwrap();
    let second = rvn_parser::parse(&make("renamed", &[Q, P], "right,left")).unwrap();
    let s2 = source_scopes(&second);
    let r2 = LibraryGraphResources1027::prepare(&s2, &assets).unwrap();
    let changed = import_library_program1027(&second, &r2, &original).unwrap();
    let target = changed.iter().find(|g| g.graph_id.get() == 11).unwrap();
    assert_eq!(
        target
            .library1027
            .as_ref()
            .unwrap()
            .entry_parameter_ids
            .iter()
            .map(|v| v.as_str())
            .collect::<Vec<_>>(),
        [Q, P]
    );
    let mapping = |graphs: &[GraphDocument]| {
        let caller = graphs
            .iter()
            .find(|g| g.library1027.as_ref().is_some_and(|m| !m.calls.is_empty()))
            .unwrap();
        let (node, call) = caller
            .library1027
            .as_ref()
            .unwrap()
            .calls
            .iter()
            .next()
            .unwrap();
        (
            *node,
            call.arguments.iter().cloned().collect::<BTreeMap<_, _>>(),
        )
    };
    assert_eq!(
        mapping(&original),
        mapping(&changed),
        "reorder preserves each actual parameter-owned input PinId, not item ordinal identity"
    );
    let compiled = transpile_library_project1027(&changed, &r2).unwrap();
    let library =
        rvn_core::eval::FunctionLibrary::from_script_with_library_assets(&compiled.ast, &assets)
            .unwrap();
    assert_eq!(
        library
            .eval(&expression(&compiled.ast, "first"), &Default::default())
            .unwrap(),
        rvn_parser::Value::Int(102)
    );
    assert_eq!(source_scopes(&compiled.ast)[0].members()[0].graph_id(), 11);
}

#[test]
fn library_graph1027_private_call_uses_actual_owner_scope_and_cross_owner_body_is_refused() {
    let inner = value(A, 12, "");
    let source=format!("functions.library_v1(\"{A}\"){{functions.member_v1(\"11\",[]){{function entry(){{return {inner}}}}}functions.member_v1(\"12\",[]){{function secret(){{return 9}}}}}}\nscene label start\nset first={}\n\"After\"",value(A,11,""));
    let parsed = rvn_parser::parse(&source).unwrap();
    let scopes = source_scopes(&parsed);
    let assets = [asset(
        A,
        &[(11, Access::Public, true), (12, Access::Private, true)],
    )];
    let resources = LibraryGraphResources1027::prepare(&scopes, &assets).unwrap();
    let graphs = import_library_program1027(&parsed, &resources, &[]).unwrap();
    let compiled = transpile_library_project1027(&graphs, &resources).unwrap();
    let library =
        rvn_core::eval::FunctionLibrary::from_script_with_library_assets(&compiled.ast, &assets)
            .unwrap();
    assert_eq!(
        library
            .eval(&expression(&compiled.ast, "first"), &Default::default())
            .unwrap(),
        rvn_parser::Value::Int(9)
    );
    let foreign = scope(B, 22, &[], &format!("function foreign(){{return {inner}}}"));
    let mut changed = parsed.clone();
    let declaration = rvn_parser::parse(&foreign).unwrap().remove(0);
    changed.insert(1, declaration);
    let foreign_scopes = source_scopes(&changed);
    let assets = [assets[0].clone(), asset(B, &[(22, Access::Public, true)])];
    let resources = LibraryGraphResources1027::prepare(&foreign_scopes, &assets).unwrap();
    let before = serde_json::to_vec(&graphs).unwrap();
    assert!(import_library_program1027(&changed, &resources, &graphs).is_err());
    assert_eq!(serde_json::to_vec(&graphs).unwrap(), before);
}

#[test]
fn library_graph1027_nominal_enum_output_is_not_erased_by_qualified_call_inference() {
    const E: &str = "329f41eae6ba4b4bb71dfaf60fb50001";
    const M: &str = "229f41eae6ba4b4bb71dfaf60fb50001";
    let source = scope(A, 11, &[P], "function identity(n){return n}")
        + &format!(
            "scene label start\nset held={}\n\"After\"",
            value(
                A,
                11,
                &format!("\"{P}\":__rvn_enum_value_v1(\"{E}\",\"{M}\")")
            )
        );
    let parsed = rvn_parser::parse(&source).unwrap();
    let scopes = source_scopes(&parsed);
    let assets = [asset(A, &[(11, Access::Public, true)])];
    let resources = LibraryGraphResources1027::prepare(&scopes, &assets).unwrap();
    let graphs = import_library_program1027(&parsed, &resources, &[]).unwrap();
    let caller = graphs
        .iter()
        .find(|g| g.library1027.as_ref().is_some_and(|m| !m.calls.is_empty()))
        .unwrap();
    let node = *caller
        .library1027
        .as_ref()
        .unwrap()
        .calls
        .keys()
        .next()
        .unwrap();
    let output = caller.nodes[&node]
        .pins
        .iter()
        .find(|id| caller.pins[id].direction == PinDirection::Output)
        .unwrap();
    assert_eq!(
        caller.effective_pin_types()[output],
        ValueType::Enum(rvn_parser::enum_asset::EnumAssetId::parse(E).unwrap())
    );
    let mut stale = caller.clone();
    stale.pins.get_mut(output).unwrap().value_type = ValueType::Any;
    assert!(
        resources.validate_graph(&stale).is_err(),
        "a concrete nominal output cannot be whitened into Any by a stale cache"
    );
    let compiled = transpile_library_project1027(&graphs, &resources).unwrap();
    assert!(format!("{:?}", expression(&compiled.ast, "held")).contains("LibraryCall"));
}

#[test]
fn library_graph1027_explicit_existing_pin_binding_creates_real_call_and_rejects_foreign_parameter_atomically(
) {
    let source = scope(A, 11, &[P], "function same(n){return n+1}")
        + "scene label start\nset first=min(3)\n\"After\"";
    let parsed = rvn_parser::parse(&source).unwrap();
    let scopes = source_scopes(&parsed);
    let assets = [asset(A, &[(11, Access::Public, true)])];
    let resources = LibraryGraphResources1027::prepare(&scopes, &assets).unwrap();
    let mut graphs = import_library_program1027(&parsed, &resources, &[]).unwrap();
    let caller = graphs
        .iter_mut()
        .find(|g| matches!(&g.kind,GraphKind::Label{name}if name=="start"))
        .unwrap();
    let node = caller
        .nodes
        .values()
        .find(|n| n.kind == NodeKind::FunctionCall)
        .unwrap()
        .id;
    let pin = caller.pin_by_key(node, "item_0").unwrap().id;
    let target = QualifiedLibraryFunction {
        library: FunctionLibraryId::parse(A).unwrap(),
        graph_id: 11,
    };
    let before = caller.to_pretty_json().unwrap();
    assert!(caller
        .set_library_call_arguments1027(
            node,
            target.clone(),
            vec![(LibraryParameterId::parse(Q).unwrap(), pin)],
            &resources
        )
        .is_err());
    assert_eq!(caller.to_pretty_json().unwrap(), before);
    caller
        .set_library_call_arguments1027(
            node,
            target,
            vec![(LibraryParameterId::parse(P).unwrap(), pin)],
            &resources,
        )
        .unwrap();
    assert_eq!(
        caller.library1027.as_ref().unwrap().calls[&node].arguments[0].1,
        pin
    );
    let compiled = transpile_library_project1027(&graphs, &resources).unwrap();
    let library =
        rvn_core::eval::FunctionLibrary::from_script_with_library_assets(&compiled.ast, &assets)
            .unwrap();
    assert_eq!(
        library
            .eval(&expression(&compiled.ast, "first"), &Default::default())
            .unwrap(),
        rvn_parser::Value::Int(4)
    );
}

#[test]
fn library_graph1027_actual_member_body_edit_cannot_publish_stale_caller_result_type() {
    let source = scope(A, 11, &[], "function same(){return 1}")
        + &format!(
            "scene label start\nset first={}\n\"After\"",
            value(A, 11, "")
        );
    let parsed = rvn_parser::parse(&source).unwrap();
    let scopes = source_scopes(&parsed);
    let assets = [asset(A, &[(11, Access::Public, true)])];
    let resources = LibraryGraphResources1027::prepare(&scopes, &assets).unwrap();
    let mut graphs = import_library_program1027(&parsed, &resources, &[]).unwrap();
    let member = graphs.iter_mut().find(|g| g.graph_id.get() == 11).unwrap();
    let literal = member
        .nodes
        .values()
        .find(|n| {
            n.kind == NodeKind::Literal && n.properties.get("value") == Some(&PropertyValue::Int(1))
        })
        .unwrap()
        .id;
    member
        .set_property(literal, "value", PropertyValue::String("changed".into()))
        .unwrap();
    let before = serde_json::to_vec(&graphs).unwrap();
    let refused = transpile_library_project1027(&graphs, &resources).unwrap_err();
    assert!(refused.contains("output is stale"),"must reach comparison against actual emitted body, not an unrelated structural refusal: {refused}");
    assert_eq!(serde_json::to_vec(&graphs).unwrap(), before);
    // A real source transaction may prepare the new candidate scope and refresh
    // its exact callers together. This is explicit, never an autosave/Any fallback.
    let changed = scope(A, 11, &[], "function same(){return \"changed\"}")
        + &format!("scene label start\nset first={}\n\"After\"", value(A, 11, ""));
    let new_source = rvn_parser::parse(&changed).unwrap();
    let new_scopes = source_scopes(&new_source);
    let current = LibraryGraphResources1027::prepare(&new_scopes, &assets).unwrap();
    let candidate_before = serde_json::to_vec(&graphs).unwrap();
    let caller = graphs
        .iter_mut()
        .find(|g| g.library1027.as_ref().is_some_and(|m| !m.calls.is_empty()))
        .unwrap();
    let (node, target) = caller
        .library1027
        .as_ref()
        .unwrap()
        .calls
        .iter()
        .next()
        .map(|(n, c)| (*n, c.target.clone()))
        .unwrap();
    // The original consumer is now proved Int. Retargeting only its call to
    // String must refuse atomically, rather than hiding that consumer in Any.
    let caller_before = caller.clone();
    assert_eq!(caller.pin_by_key(node, "result").unwrap().value_type, ValueType::Int);
    let rejected = caller.set_library_call_target1027(node, target.clone(), &current).unwrap_err();
    assert!(rejected.contains("pin type Int cannot accept String"), "must refuse the actual typed consumer: {rejected}");
    assert_eq!(*caller, caller_before);
    assert_eq!(serde_json::to_vec(&graphs).unwrap(), candidate_before);
    // Reimport the actual complete candidate Source as one projection. It
    // derives the new String global/consumer and the qualified result together.
    let graphs = import_library_program1027(&new_source, &current, &graphs).unwrap();
    let updated = graphs.iter().find(|graph| graph.graph_id == caller_before.graph_id).unwrap();
    assert_eq!(updated.variables["first"].value_type, ValueType::String);
    let consumer = updated.nodes.values().find(|node| node.kind == NodeKind::SetVariable && matches!(node.properties.get("name"), Some(PropertyValue::String(name)) if name == "first")).unwrap();
    assert_eq!(updated.pin_by_key(consumer.id, "value").unwrap().value_type, ValueType::String);
    let (updated_node, binding) = updated.library1027.as_ref().unwrap().calls.first_key_value().unwrap();
    assert_eq!(binding.target, target);
    assert_eq!(updated.pin_by_key(*updated_node, "result").unwrap().value_type, ValueType::String);
    let compiled = transpile_library_project1027(&graphs, &current).unwrap();
    let library =
        rvn_core::eval::FunctionLibrary::from_script_with_library_assets(&compiled.ast, &assets)
            .unwrap();
    assert_eq!(
        library
            .eval(&expression(&compiled.ast, "first"), &Default::default())
            .unwrap(),
        rvn_parser::Value::Str("changed".into())
    );
}

#[test]
fn library_graph1027_member_import_reads_real_complete_source_globals_without_projecting_them_as_member_init(
) {
    let source = scope(A, 11, &[P], "function sum(n){return total+n}")
        + &format!(
            "init{{set total=2}}\nscene label start\nset first={}\n\"After\"",
            value(A, 11, &format!("\"{P}\":3"))
        );
    let parsed = rvn_parser::parse(&source).unwrap();
    let scopes = source_scopes(&parsed);
    let assets = [asset(A, &[(11, Access::Public, true)])];
    let resources = LibraryGraphResources1027::prepare(&scopes, &assets).unwrap();
    let graphs = import_library_program1027(&parsed, &resources, &[]).unwrap();
    let member = graphs.iter().find(|g| g.graph_id.get() == 11).unwrap();
    assert!(member.variables.contains_key("total"));
    assert!(!member.nodes.values().any(|n| n.kind == NodeKind::Init));
    let compiled = transpile_library_project1027(&graphs, &resources).unwrap();
    assert_eq!(
        compiled
            .ast
            .iter()
            .filter(|s| matches!(s, Statement::Init { .. }))
            .count(),
        1
    );
    let library =
        rvn_core::eval::FunctionLibrary::from_script_with_library_assets(&compiled.ast, &assets)
            .unwrap();
    let globals = std::collections::HashMap::from([("total".into(), rvn_parser::Value::Int(2))]);
    assert_eq!(
        library
            .eval(&expression(&compiled.ast, "first"), &globals)
            .unwrap(),
        rvn_parser::Value::Int(5)
    );
}

// The raw project import deliberately owns the compiler transport declaration
// too. Its identity is explicit fixture evidence, never a graph filtered out of
// a later equality comparison or a counter repaired after reimport.
fn canonical_transport_fixture1030(graphs: &[GraphDocument]) -> GraphId {
    let matching = graphs.iter().filter(|graph| matches!(&graph.kind, GraphKind::Label { name } if name == "__blueprint_project_end")).collect::<Vec<_>>();
    let [footer] = matching.as_slice() else { panic!("exactly one actual fixture footer required") };
    assert_eq!(footer.narrative, Some(NarrativeMetadata::SceneFragment));
    assert_eq!(footer.nodes.len(), 1);
    assert!(footer.nodes.values().all(|node| node.kind == NodeKind::Label));
    assert_eq!(footer.pins.len(), 1);
    assert!(footer.edges.is_empty());
    footer.graph_id
}

#[test]
fn library_graph1030_qualified_global_type_evidence_survives_scene_and_init_roundtrip_without_any() {
    for in_init in [false, true] {
        let call = value(A, 11, &format!("\"{P}\":7"));
        let assignment = format!("set actual={call}");
        // A scene must yield a genuine narrative interaction. A bare return
        // here would correctly raise ReturnWithoutCall in the existing VM.
        let source = scope(A, 11, &[P], "function target(n){return to_int(n)+2}")
            + &if in_init { format!("init{{{assignment}}}\nscene label start\n\"Typed fixture end\"\nlabel __blueprint_project_end\n") }
                else { format!("scene label start\n{assignment}\n\"Typed fixture end\"\nlabel __blueprint_project_end\n") };
        // The project exporter always emits its final empty transport label.
        // Include that actual source declaration in the initial raw-import
        // fixture too; full graph equality below must never discard a graph.
        let parsed = rvn_parser::parse_program(&source).unwrap();
        let scopes = source_scopes(&parsed.script);
        let assets = [asset(A, &[(11, Access::Public, true)])];
        let resources = LibraryGraphResources1027::prepare(&scopes, &assets).unwrap();
        let graphs = import_library_program1027(&parsed.script, &resources, &[]).unwrap();
        let caller = graphs.iter().find(|g| g.library1027.as_ref().is_some_and(|m| !m.calls.is_empty())).unwrap();
        assert_eq!(caller.variables["actual"].value_type, ValueType::Int);
        let node = *caller.library1027.as_ref().unwrap().calls.keys().next().unwrap();
        assert_eq!(caller.pin_by_key(node, "result").unwrap().value_type, ValueType::Int);
        if !in_init {
            let entry = caller.nodes.values().find(|node| node.kind == NodeKind::Label).unwrap();
            let PropertyValue::String(cache) = &entry.properties["inferred_narrative_global_types"] else { panic!("actual source cache missing") };
            let globals: BTreeMap<String, ValueType> = serde_json::from_str(cache).unwrap();
            assert_eq!(globals["actual"], ValueType::Int);
        }
        let emitted = transpile_library_project1027(&graphs, &resources).unwrap();
        let emitted_scopes = source_scopes(&emitted.ast);
        let emitted_resources = LibraryGraphResources1027::prepare(&emitted_scopes, &assets).unwrap();
        let footer = canonical_transport_fixture1030(&graphs);
        let reopened = import_library_program1027(&emitted.ast, &emitted_resources, &graphs).unwrap();
        assert_eq!(canonical_transport_fixture1030(&reopened), footer);
        // Full snapshots include every graph, allocator counter and order.
        assert_eq!(reopened, graphs);
        let program = rvn_parser::parse_program(&emitted.source).unwrap();
        let prepared = rvn_core::engine::PreparedFunctionLibraryProgram1023::prepare(program, &assets).unwrap();
        let mut engine = rvn_core::Engine::from_function_library_program1023(prepared, rvn_core::TerminalRenderer, 8).unwrap();
        engine.step_until_interaction().unwrap();
        assert_eq!(engine.state.vars.get("actual"), Some(&rvn_parser::Value::Int(9)));
    }
}

#[test]
fn library_graph1030_mixed_ordinary_unit_roundtrip_keeps_structural_allocation_frontiers() {
    let source = scope(A, 11, &[P], "function target(n)->(){set library_seen=n}")
        + "function global_only()->(){set global_calls=1}\nscene label start\ncall global_only()\nset actual=0\nreturn\nlabel __blueprint_project_end\n";
    // Raw import owns every declaration, including the real canonical project
    // footer. Keep it from the initial baseline; do not filter it after import.
    let parsed = rvn_parser::parse_program(&source).unwrap();
    let scopes = source_scopes(&parsed.script);
    let assets = [asset(A, &[(11, Access::Public, false)])];
    let resources = LibraryGraphResources1027::prepare(&scopes, &assets).unwrap();
    let graphs = import_library_program1027(&parsed.script, &resources, &[]).unwrap();
    for graph in &graphs {
        let mut probe = graph.clone();
        probe.reconcile_import(graph).unwrap_or_else(|error| panic!("Current graph {:?} {:?}: {error}; {}", graph.graph_id, graph.kind, graph.to_pretty_json().unwrap()));
    }
    let emitted = transpile_library_project1027(&graphs, &resources).unwrap();
    let emitted_scopes = source_scopes(&emitted.ast);
    let emitted_resources = LibraryGraphResources1027::prepare(&emitted_scopes, &assets).unwrap();
    let footer = canonical_transport_fixture1030(&graphs);
    let reopened = import_library_program1027(&emitted.ast, &emitted_resources, &graphs).unwrap();
    assert_eq!(canonical_transport_fixture1030(&reopened), footer);
    assert_eq!(reopened, graphs);
}


#[test]
fn library_graph1031_project_character_declarations_belong_to_project_headers_and_never_to_value_or_unit_member_bodies(){
 let source=scope(A,11,&[],"function value(){return 7}")+&scope(B,22,&[],"function effect()->(){}")+"init{set amount=19\ncharacter.create(\"iris\",\"Iris\")}\nscene label start\nreturn\n";
 let parsed=rvn_parser::parse(&source).unwrap();let scopes=source_scopes(&parsed);let assets=[asset(A,&[(11,Access::Public,true)]),asset(B,&[(22,Access::Public,false)])];let resources=LibraryGraphResources1027::prepare(&scopes,&assets).unwrap();let graphs=import_library_program1027(&parsed,&resources,&[]).unwrap();let before=graphs.clone();
 for id in [11,22]{
  let graph=graphs.iter().find(|graph|graph.graph_id.get()==id).unwrap();assert_eq!(graph.characters["iris"],"Iris");
  let emitted=crate::codegen::transpile_library_graph1027(graph,&graphs,&resources).unwrap();assert_eq!(emitted.ast.len(),1);assert!(matches!((&emitted.ast[0],id),(Statement::Function{..},11)|(Statement::UnitFunction{..},22)));assert!(!emitted.source.contains("character.create("));
  let mut invalid=graph.clone();invalid.characters.insert("bad-character-id".into(),"Invalid".into());assert!(crate::codegen::transpile_library_graph1027(&invalid,&graphs,&resources).is_err(),"a member retains character validation even though it owns no project header");
  let mut native=graph.clone();assert!(native.enable_native_frame_preparation().is_err(),"a character header change cannot promote nominal Library Native/signature capabilities");assert_eq!(native,*graph);
 }
 let compiled=transpile_library_project1027(&graphs,&resources).unwrap();assert_eq!(compiled.source.matches("character.create(").count(),1);assert_eq!(graphs,before);
 let prepared=crate::prepare_library_current_cohort1027(&graphs,&assets).unwrap();assert_eq!(prepared.source().matches("character.create(").count(),1);assert_eq!(prepared.graphs(),graphs);
 let fresh_scopes=source_scopes(&compiled.ast);let fresh_resources=LibraryGraphResources1027::prepare(&fresh_scopes,&assets).unwrap();let reopened=import_library_program1027(&compiled.ast,&fresh_resources,&graphs).unwrap();for graph in &graphs{assert_eq!(reopened.iter().find(|fresh|fresh.graph_id==graph.graph_id),Some(graph),"full character registries and member payloads survive scoped export/import");}
 let ordinary=crate::SourceProject::open("init{character.create(\"iris\",\"Iris\")}\nscene label start\nreturn\n",&[]).unwrap();let graph=ordinary.graphs().iter().find(|graph|matches!(graph.kind,GraphKind::Label{..})).unwrap();assert!(crate::transpile_with_origins(graph).unwrap().source.contains("character.create("));
 let mut native=graph.clone();native.enable_native_frame_preparation().unwrap();assert!(crate::native_frame_preparation::prepare_native_unchecked(&native).unwrap().source.contains("character.create("));
}
