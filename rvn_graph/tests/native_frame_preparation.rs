use rvn_graph::*;
use rvn_parser::{ConsumerStep,Expr,Statement,Value};
use rvn_core::{consumer_schedule::ConsumerFrame,eval::FunctionLibrary,random::RandomState};
use std::collections::HashMap;

fn connect(graph:&mut GraphDocument,a:NodeId,ak:&str,b:NodeId,bk:&str) {
    graph.connect(graph.pin_by_key(a,ak).unwrap().id,graph.pin_by_key(b,bk).unwrap().id).unwrap();
}
fn write_fixture(name:&str,compiled:&TranspiledScriptWithOrigins,graph:&GraphDocument) {
    if let Ok(path)=std::env::var("RVN_NATIVE_FIXTURE_DIR") {
        let path=std::path::Path::new(&path);
        std::fs::write(path.join(format!("{name}.native.rvn")),&compiled.source).unwrap();
        std::fs::write(path.join(format!("{name}.native.graph.json")),graph.to_pretty_json().unwrap()).unwrap();
        std::fs::write(path.join(format!("{name}.native.ast.json")),serde_json::to_string_pretty(&compiled.ast).unwrap()).unwrap();
    }
}
fn fixture()->GraphDocument {
    let mut graph=GraphDocument::new(GraphId::new(900),GraphKind::Function{name:"probe".into()});
    let root=graph.add_catalog_node(NodeKind::FunctionEntry,[0.,0.]).unwrap();
    let text=graph.add_catalog_node(NodeKind::TextFromStringTable,[200.,0.]).unwrap();
    graph.set_pin_default(text,"table",PropertyValue::Name(NameDefault::new("asset:TableCase").unwrap())).unwrap();
    graph.set_pin_default(text,"key",PropertyValue::String("Hello\n世界".into())).unwrap();
    let inspector=graph.add_catalog_node(NodeKind::GetStringTableIdAndKey,[400.,0.]).unwrap();
    let list=graph.add_catalog_node(NodeKind::ListLiteral,[600.,0.]).unwrap();
    graph.resize_value_inputs(list,3).unwrap();
    let ret=graph.add_catalog_node(NodeKind::FunctionReturn,[800.,0.]).unwrap();
    connect(&mut graph,root,"exec_out",ret,"exec_in");
    connect(&mut graph,text,"result",inspector,"text");
    for (index,key) in ["valid","table_id","key"].into_iter().enumerate(){connect(&mut graph,inspector,key,list,&format!("item_{index}"));}
    connect(&mut graph,list,"list",ret,"value");
    graph.enable_native_frame_preparation().unwrap();graph
}
#[test]
fn automatic_lowering_one_real_inspector_three_typed_outputs_source_roundtrip() {
    let graph=fixture();let compiled=prepare_native_frame_graph(&graph).unwrap();
    let [Statement::Function{body,..}]=compiled.ast.as_slice()else{panic!()};
    let [Statement::ScheduledConsumer{steps,..}]=body.as_slice()else{panic!()};
    assert_eq!(steps.iter().filter(|step|matches!(step,ConsumerStep::InspectTextTable{..})).count(),1);
    assert_eq!(steps.iter().filter(|step|matches!(step,ConsumerStep::Capture{..})).count(),2);
    assert!(!compiled.source.contains("__rvn_text_table_valid("));
    let parsed=rvn_parser::parse(&compiled.source).unwrap();assert_eq!(parsed,compiled.ast);
    let imported=import_native_frame_script(&parsed).unwrap();
    let restored=imported.iter().find(|graph|graph.graph_id==GraphId::new(900)).unwrap();
    assert_eq!(restored.nodes.keys().collect::<Vec<_>>(),graph.nodes.keys().collect::<Vec<_>>());
    assert_eq!(restored.pins.keys().collect::<Vec<_>>(),graph.pins.keys().collect::<Vec<_>>());
    assert_eq!(restored.edges,graph.edges);
    assert_eq!(restored.nodes.values().filter(|node|node.kind==NodeKind::GetStringTableIdAndKey).count(),1);
    assert_eq!(prepare_native_frame_graph(restored).unwrap().ast,compiled.ast);
    write_fixture("single-inspector",&compiled,restored);
}
#[test]
fn cold_load_reimport_source_authority_preserves_identity_and_layout() {
    let mut graph=fixture();let compiled=prepare_native_frame_graph(&graph).unwrap();
    graph.nodes.values_mut().for_each(|node|node.position=[123.,456.]);
    let cold=GraphDocument::from_json(&graph.to_pretty_json().unwrap()).unwrap();
    assert_eq!(cold,graph);
    let restored=reimport_native_frame_script(&compiled.ast,&[cold]).unwrap();
    let restored=restored.iter().find(|graph|graph.graph_id==GraphId::new(900)).unwrap();
    assert!(restored.nodes.values().all(|node|node.position==[123.,456.]));
    assert_eq!(prepare_native_frame_graph(restored).unwrap().ast,compiled.ast);
    let fresh=import_native_frame_script(&compiled.ast).unwrap();
    assert_eq!(import_native_frame_script(&compiled.ast).unwrap(),fresh);
}
#[test]
fn regular_compiler_automatically_lowers_native_while_isolated_previews_remain_guarded() {
    let graph=fixture();let prepared=prepare_native_frame_graph(&graph).unwrap();
    assert_eq!(transpile(&graph).unwrap().ast,prepared.ast);
    let project=transpile_project(&[graph.clone()]).unwrap();
    assert_eq!(&project.ast[..prepared.ast.len()],prepared.ast.as_slice());
    assert!(matches!(project.ast.last(),Some(Statement::Label{name})if name=="__blueprint_project_end"));
    let inspector=graph.nodes.values().find(|node|node.kind==NodeKind::GetStringTableIdAndKey).unwrap().id;
    assert!(transpile_value_output(&graph,inspector,"valid").is_err());
    let imported=import_script(&prepared.ast).unwrap();
    let imported=imported.iter().find(|actual|actual.graph_id==graph.graph_id).unwrap();
    assert_eq!(prepare_native_frame_graph(imported).unwrap().ast,prepared.ast);
    let mut invalid=graph.clone();invalid.inspector_evaluation=InspectorEvaluationContract::LegacyStateless;
    assert!(invalid.validate_inspector_evaluation_contract().is_err());
    invalid.schema_version=10;invalid.inspector_evaluation=InspectorEvaluationContract::NativeFrameV1;
    assert!(invalid.validate_inspector_evaluation_contract().is_err());
}
#[test]
fn private_owned_core_evaluates_prepared_graph_without_engine_publication() {
    let graph=fixture();let compiled=prepare_native_frame_graph(&graph).unwrap();
    let library=FunctionLibrary::from_script(&compiled.ast).unwrap();
    let call=vec![Statement::SetVar{name:"result".into(),value:Expr::Call{name:"probe".into(),args:vec![]}}];
    let mut rng=RandomState::seeded(42);let mut frame=ConsumerFrame::default();
    let values=library.execute_in_consumer_frame(&call,&HashMap::new(),&mut rng,&mut frame).unwrap();
    assert_eq!(values["result"],Value::List(vec![Value::Bool(true),Value::Name(rvn_parser::Name::new("asset:TableCase").unwrap()),Value::Str("Hello\n世界".into())]));
}
#[test]
fn collision_missing_role_and_forward_references_refused_transactionally() {
    let compiled=prepare_native_frame_graph(&fixture()).unwrap();
    let mut cases=vec![];
    let mut collision=compiled.ast.clone();
    let [Statement::Function{body,..}]=collision.as_mut_slice()else{panic!()};
    let [Statement::ScheduledConsumer{steps,consumer,..}]=body.as_mut_slice()else{panic!()};
    let ConsumerStep::Capture{producer,..}=&mut steps[0]else{panic!()};
    *consumer=producer.clone();cases.push(collision);
    let mut missing=compiled.ast.clone();
    let [Statement::Function{body,..}]=missing.as_mut_slice()else{panic!()};
    let [Statement::ScheduledConsumer{scope,..}]=body.as_mut_slice()else{panic!()};
    *scope="g900/n1:function_entry|bad=1@0".into();cases.push(missing);
    let mut forward=compiled.ast.clone();
    let [Statement::Function{body,..}]=forward.as_mut_slice()else{panic!()};
    let [Statement::ScheduledConsumer{steps,..}]=body.as_mut_slice()else{panic!()};
    steps.reverse();cases.push(forward);
    for script in cases {assert!(import_native_frame_script(&script).is_err());}
}

#[test]
fn shared_effectful_function_rng_and_motion_curve_are_gathered_once_per_consumer() {
    let mut graph=fixture();
    let table=graph.nodes.values().find(|node|node.kind==NodeKind::TextFromStringTable).unwrap().id;
    let inspector=graph.nodes.values().find(|node|node.kind==NodeKind::GetStringTableIdAndKey).unwrap().id;
    let list=graph.nodes.values().find(|node|node.kind==NodeKind::ListLiteral).unwrap().id;
    graph.remove_node(table);
    let curve=graph.add_catalog_node(NodeKind::MotionCurve,[200.,200.]).unwrap();
    graph.set_pin_default(curve,"function",PropertyValue::String("warp".into())).unwrap();
    graph.set_pin_default(curve,"samples",PropertyValue::Int(3)).unwrap();
    let producer=graph.add_catalog_node(NodeKind::FunctionCall,[400.,200.]).unwrap();
    graph.set_property(producer,"function",PropertyValue::String("make_text".into())).unwrap();
    graph.resize_value_inputs(producer,1).unwrap();
    connect(&mut graph,curve,"value",producer,"item_0");
    connect(&mut graph,producer,"result",inspector,"text");
    graph.resize_value_inputs(list,4).unwrap();
    connect(&mut graph,producer,"result",list,"item_3");
    let compiled=prepare_native_frame_graph(&graph).unwrap();
    assert_eq!(compiled.source.matches("make_text(").count(),1);
    assert_eq!(compiled.source.matches("motion_curve(").count(),1);
    let imported=import_native_frame_script(&compiled.ast).unwrap();
    let restored=imported.iter().find(|candidate|candidate.graph_id==graph.graph_id).unwrap();
    assert_eq!(prepare_native_frame_graph(restored).unwrap().ast,compiled.ast);
    write_fixture("shared-function-motion",&compiled,restored);
    let helpers=r#"function warp(t){return t*t}
    function make_text(curve){set draw=random(1,1)
    return __rvn_string_table_text_name(__rvn_literal_name("asset:TableCase"),"hello")}
    "#;
    let mut script=rvn_parser::parse(helpers).unwrap();script.extend(compiled.ast.clone());
    if let Ok(path)=std::env::var("RVN_NATIVE_FIXTURE_DIR") {
        std::fs::write(std::path::Path::new(&path).join("shared-function-motion.program.rvn"),format!("{helpers}\n{}",compiled.source)).unwrap();
        std::fs::write(std::path::Path::new(&path).join("shared-function-motion.program.ast.json"),serde_json::to_string_pretty(&script).unwrap()).unwrap();
    }
    let library=FunctionLibrary::from_script(&script).unwrap();
    let call=rvn_parser::parse("set result=probe()\nset result2=probe()").unwrap();
    let mut rng=RandomState::seeded(77);let mut expected=rng;
    expected.integer(1,1).unwrap();expected.integer(1,1).unwrap();
    let values=library.execute_in_consumer_frame(&call,&HashMap::new(),&mut rng,&mut ConsumerFrame::default()).unwrap();
    assert_eq!(rng,expected);assert_eq!(values["result"],values["result2"]);
}

#[test]
fn set_local_return_reached_consumers_reuse_one_inspector_and_retain_false_outs() {
    let source=r#"function probe(){
    local text=__rvn_string_table_text_name(__rvn_literal_name("asset:TableCase"),"hello")
    local before=[__rvn_text_table_valid(text),__rvn_text_table_id(text),__rvn_text_table_key(text)]
    set text=__rvn_invariant_text("literal")
    return [__rvn_text_table_valid(text),__rvn_text_table_id(text),__rvn_text_table_key(text),before]
    }"#;
    let graphs=import_script(&rvn_parser::parse(source).unwrap()).unwrap();
    let mut graph=graphs.into_iter().find(|graph|matches!(graph.kind,GraphKind::Function{..})).unwrap();
    graph.graph_id=GraphId::new(910);graph.enable_native_frame_preparation().unwrap();
    // Selectively promote the intended inspector; a graph-level preparation
    // flag now preserves all historical per-node contracts.
    for node in graph.nodes.values_mut().filter(|node|node.kind==NodeKind::GetStringTableIdAndKey) {node.inspector_contract=Some(InspectorEvaluationContract::NativeFrameV1);}
    assert_eq!(graph.nodes.values().filter(|node|node.kind==NodeKind::GetStringTableIdAndKey).count(),1);
    let compiled=prepare_native_frame_graph(&graph).unwrap();
    let [Statement::Function{body,..}]=compiled.ast.as_slice()else{panic!()};
    assert_eq!(body.iter().filter(|statement|matches!(statement,Statement::ScheduledConsumer{..})).count(),4);
    let restored=import_native_frame_script(&compiled.ast).unwrap();
    let restored=restored.iter().find(|restored|restored.graph_id==graph.graph_id).unwrap();
    assert_eq!(restored.edges,graph.edges);
    write_fixture("set-local-return-reuse",&compiled,restored);
    let library=FunctionLibrary::from_script(&compiled.ast).unwrap();
    let mut rng=RandomState::seeded(3);
    let values=library.execute_in_consumer_frame(&rvn_parser::parse("set outputs=probe()").unwrap(),&HashMap::new(),&mut rng,&mut ConsumerFrame::default()).unwrap();
    let Value::List(outputs)=&values["outputs"]else{panic!()};
    assert_eq!(outputs[0],Value::Bool(false));
    assert_eq!(outputs[1],Value::Name(rvn_parser::Name::new("asset:TableCase").unwrap()));
    assert_eq!(outputs[2],Value::Str("hello".into()));
    assert_eq!(outputs[3],Value::List(vec![Value::Bool(true),outputs[1].clone(),outputs[2].clone()]));
}

#[test]
fn source_changes_override_old_graph_payload_without_reassigning_ids() {
    let graph=fixture();let mut ast=prepare_native_frame_graph(&graph).unwrap().ast;
    let [Statement::Function{body,..}]=ast.as_mut_slice()else{panic!()};
    let [Statement::ScheduledConsumer{steps,..}]=body.as_mut_slice()else{panic!()};
    let ConsumerStep::Capture{value:Expr::Call{args,..},..}=&mut steps[0]else{panic!()};
    args[1]=Expr::Str("source changed".into());
    let restored=reimport_native_frame_script(&ast,&[graph.clone()]).unwrap();
    let restored=restored.iter().find(|restored|restored.graph_id==graph.graph_id).unwrap();
    assert_eq!(prepare_native_frame_graph(restored).unwrap().ast,ast);
    assert_eq!(restored.nodes.keys().collect::<Vec<_>>(),graph.nodes.keys().collect::<Vec<_>>());
}

#[test]
fn graph_identity_one_is_authoritative_and_native_graph_collisions_are_refused() {
    let mut graph=fixture();graph.graph_id=GraphId::new(1);
    let compiled=prepare_native_frame_graph(&graph).unwrap();
    let imported=import_native_frame_script(&compiled.ast).unwrap();
    assert_eq!(imported.len(),1);assert_eq!(imported[0].graph_id,GraphId::new(1));
    let mut duplicate=compiled.ast.clone();
    let mut other=duplicate[0].clone();
    let Statement::Function{name,..}=&mut other else {panic!()};*name="another".into();
    duplicate.push(other);assert!(import_native_frame_script(&duplicate).is_err());
}

#[test]
fn empty_unconnected_text_has_one_inspector_and_canonical_default_outputs() {
    let mut graph=fixture();
    let table=graph.nodes.values().find(|node|node.kind==NodeKind::TextFromStringTable).unwrap().id;
    graph.remove_node(table);
    let compiled=prepare_native_frame_graph(&graph).unwrap();
    assert!(compiled.source.contains("__rvn_empty_text()"));
    let restored=import_native_frame_script(&compiled.ast).unwrap();
    assert_eq!(restored[0].edges,graph.edges);
    let library=FunctionLibrary::from_script(&compiled.ast).unwrap();
    let values=library.execute_in_consumer_frame(&rvn_parser::parse("set result=probe()").unwrap(),&HashMap::new(),&mut RandomState::seeded(2),&mut ConsumerFrame::default()).unwrap();
    assert_eq!(values["result"],Value::List(vec![Value::Bool(false),Value::Name(Default::default()),Value::Str(String::new())]));
}

#[test]
fn provisional_unrepresentable_nodes_and_overlong_metadata_are_refused() {
    let mut graph=fixture();
    graph.add_catalog_node(NodeKind::Literal,[1.,1.]).unwrap();
    assert!(prepare_native_frame_graph(&graph).is_err());
    let mut graph=fixture();
    let list=graph.nodes.values().find(|node|node.kind==NodeKind::ListLiteral).unwrap().id;
    graph.resize_value_inputs(list,30).unwrap();
    assert!(prepare_native_frame_graph(&graph).is_err());
}

#[test]
fn ordinary_legacy_graph_and_intrinsics_stay_stateless_without_opt_in() {
    let mut graph=fixture();graph.inspector_evaluation=InspectorEvaluationContract::LegacyStateless;graph.schema_version=10;
    for node in graph.nodes.values_mut().filter(|node|node.kind==NodeKind::GetStringTableIdAndKey) {node.inspector_contract=None;}
    let compiled=transpile(&graph).unwrap();
    assert!(compiled.source.contains("__rvn_text_table_valid("));assert!(compiled.source.contains("__rvn_text_table_id("));assert!(compiled.source.contains("__rvn_text_table_key("));
    assert!(!compiled.source.contains("flow.consume_v1"));
    assert!(!graph.to_pretty_json().unwrap().contains("inspector_evaluation"));
    let ast=rvn_parser::parse("set a=__rvn_text_table_id(__rvn_invariant_text(\"literal\"))").unwrap();
    let values=FunctionLibrary::default().execute(&ast,&HashMap::new()).unwrap();
    assert_eq!(values["a"],Value::Name(Default::default()));
}

#[test]
fn source_function_rename_retains_explicit_identity_and_overrides_previous_name() {
    let graph=fixture();let mut ast=prepare_native_frame_graph(&graph).unwrap().ast;
    let [Statement::Function{name,..}]=ast.as_mut_slice()else{panic!()};*name="renamed".into();
    let restored=reimport_native_frame_script(&ast,&[graph.clone()]).unwrap();
    assert_eq!(restored.len(),1);assert_eq!(restored[0].kind,GraphKind::Function{name:"renamed".into()});
    assert_eq!(restored[0].graph_id,graph.graph_id);assert_eq!(restored[0].edges,graph.edges);
    assert_eq!(prepare_native_frame_graph(&restored[0]).unwrap().ast,ast);
}
