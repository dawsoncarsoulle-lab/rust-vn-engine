use super::*;
use crate::{GraphId, GraphKind, PinDirection};
use rvn_parser::{enum_asset::{EnumAsset, EnumCommand}, enum_asset_json::EnumIndexInput, Statement, Value};
use std::{collections::HashMap, sync::Arc};
const A: &str = "329f41eae6ba4b4bb71dfaf60fb50001";
const B: &str = "329f41eae6ba4b4bb71dfaf60fb50002";
const M: &str = "229f41eae6ba4b4bb71dfaf60fb50001";
const N: &str = "229f41eae6ba4b4bb71dfaf60fb50002";
fn resources() -> EnumAssets1025 {
    let rows = [(A, Some("Alpha.rvnenum")), (B, Some("Beta.rvnenum"))];
    let files = rows.iter().map(|(id, locator)| {
        let name = if *id == A { "Alpha" } else { "Beta" };
        let mut asset = EnumAsset::new(EnumAssetId::parse(id).unwrap(), name.into()).unwrap();
        for (id, label) in [(M,"First"),(N,"Second")] {
            asset = asset.prepare(EnumCommand::Add { member: EnumMemberId::parse(id).unwrap(), display_name: label.into(), description: String::new() }).unwrap().after().clone();
        }
        (locator.unwrap().into(), asset.encode().unwrap().into_bytes())
    }).collect::<Vec<_>>();
    EnumAssets1025::admit(EnumIndexInput::Table(&rows), &files, true).unwrap()
}
fn value(asset: &str, member: &str) -> EnumValue {
    resources().value(EnumAssetId::parse(asset).unwrap(), EnumMemberId::parse(member).unwrap()).unwrap()
}
fn source(asset: &str, member: &str) -> String { source1026(value(asset,member)) }
fn init(expression: &str) -> Vec<GraphDocument> {
    crate::import_script(&rvn_parser::parse(&format!("init {{ set held = {expression} }}")).unwrap()).unwrap()
}
fn held_expression(script: &[Statement]) -> &Expr {
    for statement in script {
        if let Statement::Init { body } = statement {
            for statement in body {
                if let Statement::SetVar { name, value } = statement {
                    if name == "held" { return value; }
                }
            }
        }
    }
    panic!("actual held Init declaration missing")
}

#[test]
fn enum_graph1026_real_source_import_json_roundtrip_keeps_opaque_ids_and_optin_schema() {
    let graphs = init(&source(A,M));
    let graph = graphs.iter().find(|graph| graph.kind == GraphKind::Init).unwrap();
    assert_eq!(graph.schema_version, crate::GRAPH_NOMINAL_ENUM_SCHEMA_VERSION);
    assert_eq!(graph.variables["held"].value_type, ValueType::Enum(value(A,M).asset));
    graph.validate_enum_references1026(&resources()).unwrap();
    let json = graph.to_pretty_json().unwrap();
    let (loaded, report) = GraphDocument::from_json_with_report(&json).unwrap();
    assert_eq!(loaded, *graph);
    assert_eq!(report.from, 15); assert_eq!(report.to, 15);
    assert_eq!(held_expression(&crate::transpile(&loaded).unwrap().ast), &Expr::Call {
        name: ENUM_VALUE_CONSTRUCTOR1026.into(), args: vec![Expr::Str(A.into()),Expr::Str(M.into())],
    });
}

#[test]
fn enum_graph1026_nominal_bytes_under_every_old_schema_refuse_without_migration() {
    let graph = init(&source(A,M)).into_iter().find(|graph| graph.kind == GraphKind::Init).unwrap();
    for schema in 5..=14 {
        let mut json = serde_json::to_value(&graph).unwrap(); json["schema_version"] = schema.into();
        assert!(GraphDocument::from_json(&serde_json::to_string(&json).unwrap()).is_err(), "schema {schema}");
    }
    let legacy = init("1").into_iter().find(|graph| graph.kind == GraphKind::Init).unwrap();
    assert_eq!(legacy.schema_version, crate::GRAPH_SCHEMA_VERSION);
    let (loaded, report) = GraphDocument::from_json_with_report(&legacy.to_pretty_json().unwrap()).unwrap();
    assert_eq!(loaded, legacy); assert_eq!(report.from, 5); assert_eq!(report.to, 5);
}

#[test]
fn enum_graph1026_uuid_wire_rejects_zero_uppercase_extra_fields_and_number_surrogates() {
    for wire in [r#"{"enum":0}"#, r#"{"enum":"00000000000000000000000000000000"}"#,
        r#"{"enum":"329F41EAE6BA4B4BB71DFAF60FB50001"}"#] {
        assert!(serde_json::from_str::<ValueType>(wire).is_err());
    }
    assert!(serde_json::from_value::<PropertyValue>(serde_json::json!({"type":"enum","value":{"asset":A,"member":M,"ordinal":0}})).is_err());
    let property = PropertyValue::Enum(value(A,M));
    assert_eq!(serde_json::from_str::<PropertyValue>(&serde_json::to_string(&property).unwrap()).unwrap(), property);
}

#[test]
fn enum_graph1026_codegen_feeds_real_runtime_constructor_only_with_actual_resource_admission() {
    let graphs = init(&source(A,M)); let compiled = crate::transpile_project(&graphs).unwrap();
    let expression = held_expression(&compiled.ast);
    assert!(rvn_core::eval::FunctionLibrary::default().eval(expression,&HashMap::new()).is_err());
    let library = rvn_core::eval::FunctionLibrary::default().with_enum_assets1025(Arc::new(resources()));
    assert_eq!(library.eval(expression,&HashMap::new()).unwrap(), Value::Enum(value(A,M)));
    let imported = crate::reimport_script(&compiled.ast, &graphs).unwrap();
    let before = graphs.iter().find(|graph| graph.kind == GraphKind::Init).unwrap();
    let after = imported.iter().find(|graph| graph.kind == GraphKind::Init).unwrap();
    assert_eq!(before.graph_id,after.graph_id);
    assert_eq!(before.nodes.keys().collect::<Vec<_>>(),after.nodes.keys().collect::<Vec<_>>());
    assert_eq!(crate::transpile_project(&imported).unwrap().ast,compiled.ast);
}

#[test]
fn enum_graph1026_cross_asset_equality_and_scalar_arithmetic_refuse_real_import() {
    for expression in [format!("{} == {}",source(A,M),source(B,M)), format!("{} + \"label\"",source(A,M)),
        format!("{} < {}",source(A,M),source(A,N)), format!("{} == 0",source(A,M))] {
        let parsed = rvn_parser::parse(&format!("init {{ set held = {expression} }}")).unwrap();
        assert!(crate::import_script(&parsed).is_err(),"{expression}");
    }
    let graphs = init(&format!("{} != {}",source(A,M),source(A,N)));
    let compiled = crate::transpile_project(&graphs).unwrap();
    let library = rvn_core::eval::FunctionLibrary::default().with_enum_assets1025(Arc::new(resources()));
    assert_eq!(library.eval(held_expression(&compiled.ast),&HashMap::new()).unwrap(),Value::Bool(true));
}

#[test]
fn enum_graph1026_reroute_and_list_inference_keep_nominal_type_and_no_implicit_conversion() {
    let mut graph = GraphDocument::new(GraphId::new(1),GraphKind::Init);
    let literal = graph.add_catalog_node(NodeKind::Literal,[0.0,0.0]).unwrap();
    graph.set_enum_literal1026(literal,value(A,M),&resources()).unwrap();
    let reroute = graph.add_catalog_node(NodeKind::Reroute,[200.0,0.0]).unwrap();
    graph.connect(graph.pin_by_key(literal,"value").unwrap().id,graph.pin_by_key(reroute,"value").unwrap().id).unwrap();
    let output = graph.nodes[&reroute].pins.iter().filter_map(|id|graph.pins.get(id)).find(|pin|pin.direction==PinDirection::Output).unwrap();
    assert_eq!(graph.effective_pin_type(output.id),Some(ValueType::Enum(value(A,M).asset)));
    let graphs = init(&format!("[{}, {}]",source(A,M),source(A,N)));
    let list = graphs.iter().flat_map(|graph|graph.nodes.values().map(move|node|(graph,node)))
        .find(|(_,node)|node.kind==NodeKind::ListLiteral).unwrap();
    let output = list.1.pins.iter().filter_map(|id|list.0.pins.get(id)).find(|pin|pin.direction==PinDirection::Output).unwrap();
    assert_eq!(list.0.effective_pin_type(output.id),Some(ValueType::List(Box::new(ValueType::Enum(value(A,M).asset)))));
    assert!(!ValueType::Enum(value(A,M).asset).accepts(&ValueType::Enum(value(B,M).asset)));
    for ty in [ValueType::Int,ValueType::String,ValueType::Name,ValueType::Bool] {
        assert!(ty.conversion_path_from(&ValueType::Enum(value(A,M).asset)).is_none());
    }
}

#[test]
fn enum_graph1026_stale_member_missing_asset_and_empty_enum_never_become_any_or_first_member() {
    let graphs = init(&source(A,M));
    assert!(graphs[0].validate_enum_references1026(&EnumAssets1025::default()).is_err());
    // Import keeps a literal SET initializer in its actual input default.
    // This case exercises explicit Literal authoring, so create that real node
    // rather than assuming the importer materialized a separate producer.
    let mut graph = graphs[0].clone();
    let node = graph.add_catalog_node(NodeKind::Literal,[0.0,0.0]).unwrap();
    graph.set_enum_literal1026(node,value(A,M),&resources()).unwrap();
    assert_eq!(graph.nodes[&node].properties.get("value"),Some(&PropertyValue::Enum(value(A,M))));
    let before = graph.clone();
    let missing = EnumValue { asset:value(A,M).asset, member:EnumMemberId::parse("229f41eae6ba4b4bb71dfaf60fb50099").unwrap() };
    assert!(graph.set_enum_literal1026(node,missing,&resources()).is_err()); assert_eq!(graph,before);
    let empty = EnumAsset::new(value(A,M).asset,"Alpha".into()).unwrap();
    let rows = [(A,Some("Alpha.rvnenum"))];
    let projection = EnumAssets1025::admit(EnumIndexInput::Table(&rows),&[("Alpha.rvnenum".into(),empty.encode().unwrap().into_bytes())],true).unwrap();
    assert!(graph.validate_enum_references1026(&projection).is_err());
}

#[test]
fn enum_graph1026_global_default_edits_real_init_cohort_and_preserves_original_on_refusal() {
    let graphs = init(&source(A,M)); let before = graphs.clone();
    let edited = replace_enum_global_initializer1026(&graphs,"held",value(A,N),&resources()).unwrap();
    assert_eq!(graphs,before);
    assert_eq!(held_expression(&crate::transpile_project(&edited).unwrap().ast), &Expr::Call {
        name:ENUM_VALUE_CONSTRUCTOR1026.into(), args:vec![Expr::Str(A.into()),Expr::Str(N.into())],
    });
    assert!(replace_enum_global_initializer1026(&graphs,"absent",value(A,N),&resources()).is_err());
    assert_eq!(graphs,before);
}

#[test]
fn enum_graph1026_global_computed_initializer_refuses_instead_of_rewriting_metadata_only() {
    let graphs = init("1 + 2"); let before = graphs.clone();
    assert!(replace_enum_global_initializer1026(&graphs,"held",value(A,M),&resources()).is_err());
    assert_eq!(graphs,before);
}

#[test]
fn enum_graph1026_same_asset_local_default_uses_actual_existing_initializer_edit() {
    let script = rvn_parser::parse(&format!("function choose() {{ local held = {}\n return held }}",source(A,M))).unwrap();
    let mut graph = crate::import_script(&script).unwrap().into_iter().find(|graph|matches!(graph.kind,GraphKind::Function{..})).unwrap();
    let initializer = graph.local_variable_initializers("held").unwrap().remove(0);
    assert_eq!(initializer.literal_value,Some(PropertyValue::Enum(value(A,M))));
    assert!(graph.replace_local_initializer("held",&initializer,PropertyValue::Enum(value(A,N))).unwrap());
    assert_eq!(graph.local_variable_initializers("held").unwrap()[0].literal_value,Some(PropertyValue::Enum(value(A,N))));
    let now = graph.local_variable_initializers("held").unwrap().remove(0); let before = graph.clone();
    assert!(graph.replace_local_initializer("held",&now,PropertyValue::Enum(value(B,M))).is_err());
    assert_eq!(graph,before);
}

#[test]
fn enum_graph1026_local_type_change_requires_explicit_admitted_member_and_real_proposal() {
    let script = rvn_parser::parse("function choose() { local held = 1\n return held }").unwrap();
    let graph = crate::import_script(&script).unwrap().into_iter().find(|graph|matches!(graph.kind,GraphKind::Function{..})).unwrap();
    let initializer = graph.local_variable_initializers("held").unwrap().remove(0);
    let proposal = graph.prepare_local_variable_type("held",&initializer,ValueType::Enum(value(A,M).asset)).unwrap();
    assert!(proposal.reconstruct(&graph).is_err());
    assert!(proposal.clone().with_enum_default1026(value(B,M),&resources()).is_err());
    let next = proposal.with_enum_default1026(value(A,M),&resources()).unwrap().reconstruct(&graph).unwrap();
    assert_eq!(next.local_variable_initializers("held").unwrap()[0].literal_value,Some(PropertyValue::Enum(value(A,M))));
    assert_eq!(graph.local_variable_initializers("held").unwrap()[0].literal_value,Some(PropertyValue::Int(1)));
}

#[test]
fn enum_graph1026_malformed_reserved_constructor_refuses_import_instead_of_generic_any_call() {
    for expression in [format!("{ENUM_VALUE_CONSTRUCTOR1026}(0, 1)"),format!("{ENUM_VALUE_CONSTRUCTOR1026}(\"00000000000000000000000000000000\", \"{M}\")")] {
        let script = rvn_parser::parse(&format!("init {{ set held = {expression} }}")).unwrap();
        assert!(crate::import_script(&script).is_err());
    }
}

#[test]
fn enum_graph1026_cross_enum_connect_default_and_literal_retarget_refuse_before_mutation() {
    let mut graph = GraphDocument::new(GraphId::new(1),GraphKind::Init);
    let a = graph.add_catalog_node(NodeKind::Literal,[0.0,0.0]).unwrap();
    let b = graph.add_catalog_node(NodeKind::Literal,[0.0,200.0]).unwrap();
    let eq = graph.add_catalog_node(NodeKind::MathEqual,[300.0,0.0]).unwrap();
    graph.set_enum_literal1026(a,value(A,M),&resources()).unwrap();
    graph.set_enum_literal1026(b,value(B,M),&resources()).unwrap();
    graph.connect(graph.pin_by_key(a,"value").unwrap().id,graph.pin_by_key(eq,"left").unwrap().id).unwrap();
    let before = graph.clone();
    assert!(graph.connect(graph.pin_by_key(b,"value").unwrap().id,graph.pin_by_key(eq,"right").unwrap().id).is_err());
    assert_eq!(graph,before);
    assert!(graph.set_enum_pin_default1026(eq,"right",value(B,M),&resources()).is_err()); assert_eq!(graph,before);
    graph.set_enum_pin_default1026(eq,"right",value(A,N),&resources()).unwrap();
    let before = graph.clone();
    assert!(graph.set_enum_literal1026(a,value(B,M),&resources()).is_err()); assert_eq!(graph,before);
}

#[test]
fn enum_graph1026_admitted_member_rename_and_reorder_preserve_graph_value_ids() {
    let original = resources(); let graph = init(&source(A,M)).remove(0);
    let asset = original.asset(value(A,M).asset).unwrap();
    let renamed = asset.prepare(EnumCommand::RenameMember {member:value(A,M).member,display_name:"Renamed".into()}).unwrap().after().clone();
    let moved = renamed.prepare(EnumCommand::Move {member:value(A,M).member,index:1}).unwrap().after().clone();
    let rows = [(A,Some("Alpha.rvnenum"))];
    let changed = EnumAssets1025::admit(EnumIndexInput::Table(&rows),&[("Alpha.rvnenum".into(),moved.encode().unwrap().into_bytes())],true).unwrap();
    let before = graph.clone(); graph.validate_enum_references1026(&changed).unwrap();
    assert_eq!(graph,before);
    assert_eq!(changed.value(value(A,M).asset,value(A,M).member).unwrap(),value(A,M));
}

#[test]
fn enum_graph1026_serialized_variable_default_cannot_cross_nominal_asset_or_scalar_type() {
    let mut graph = GraphDocument::new(GraphId::new(1),GraphKind::Init);
    graph.schema_version = crate::GRAPH_NOMINAL_ENUM_SCHEMA_VERSION;
    graph.variables.insert("held".into(),crate::VariableDefinition {
        name:"held".into(),value_type:ValueType::Enum(value(A,M).asset),
        default_value:PropertyValue::Enum(value(A,M)),
    });
    let original = graph.clone();
    graph.validate_enum_references1026(&resources()).unwrap();
    assert_eq!(GraphDocument::from_json(&graph.to_pretty_json().unwrap()).unwrap(),graph);
    for target in [ValueType::Enum(value(B,M).asset),ValueType::String,ValueType::Int] {
        let mut invalid=graph.clone();invalid.variables.get_mut("held").unwrap().value_type=target;
        let before=invalid.clone();
        assert!(invalid.validate_enum_references1026(&resources()).is_err());
        assert!(GraphDocument::from_json(&invalid.to_pretty_json().unwrap()).is_err());
        assert_eq!(invalid,before);
    }
    // A real Wildcard remains a Wildcard. It is not a second concrete Enum
    // asset and is not globally forbidden to conceal an inference defect.
    let mut wildcard=graph.clone();wildcard.variables.get_mut("held").unwrap().value_type=ValueType::Any;
    wildcard.validate_enum_references1026(&resources()).unwrap();
    assert_eq!(GraphDocument::from_json(&wildcard.to_pretty_json().unwrap()).unwrap(),wildcard);
    assert_eq!(graph,original);
}

#[test]
fn enum_graph1026_non_enum_unit14_roundtrip_keeps_signature_ids_schema_and_native_refusal() {
    let parsed=rvn_parser::parse("init {set result=0}\nfunction pass(value)->(){set result=value}\nscene label start\ncall pass(7)\nreturn").unwrap();
    let graphs=crate::import_script_with_parameter_allocator(&parsed,
        ||Ok("11111111111111111111111111111111".into())).unwrap();
    assert!(graphs.iter().all(|graph|!graph.has_nominal_enum1026()
        &&graph.schema_version!=crate::GRAPH_NOMINAL_ENUM_SCHEMA_VERSION));
    let unit=graphs.iter().find(|graph|matches!(graph.kind,GraphKind::UnitFunction{..})).unwrap();
    assert_eq!(unit.schema_version,crate::GRAPH_UNIT_SIGNATURE_SCHEMA_VERSION);
    let ids=crate::unit_graph_source1003::parameters(unit).unwrap().iter()
        .map(|pin|pin.parameter_id.clone()).collect::<Vec<_>>();
    let compiled=crate::transpile_project(&graphs).unwrap();
    let loaded=graphs.iter().map(|graph|GraphDocument::from_json(&graph.to_pretty_json().unwrap()).unwrap()).collect::<Vec<_>>();
    assert_eq!(loaded,graphs);
    assert_eq!(crate::transpile_project(&loaded).unwrap().ast,compiled.ast);
    let reopened=loaded.iter().find(|graph|graph.graph_id==unit.graph_id).unwrap();
    assert_eq!(crate::unit_graph_source1003::parameters(reopened).unwrap().iter()
        .map(|pin|pin.parameter_id.clone()).collect::<Vec<_>>(),ids);
    let mut native=unit.clone();assert!(native.enable_native_frame_preparation().is_err());assert_eq!(&native,unit);
    let ordinary=init("\"__rvn_enum_value_v1\"");
    let init=ordinary.iter().find(|graph|graph.kind==GraphKind::Init).unwrap();
    assert_eq!(init.variables["held"].value_type,ValueType::String);
    assert_eq!(init.schema_version,crate::GRAPH_SCHEMA_VERSION);
}
