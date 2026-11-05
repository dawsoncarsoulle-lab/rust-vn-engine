//! Actual Current member edits and diagnosed caller, without Saved projection.
use super::*;
use rvn_parser::function_library_asset1023::FunctionLibraryMember;
const A:&str="11111111111141118111111111111111";
const P:&str="33333333333343338333333333333333";
fn fixture(unit:bool)->(Vec<GraphDocument>,Vec<FunctionLibraryAsset>){
 let declaration=if unit{"function target(n)->(){set result=n}"}else{"function target(n){return 1}"};
 let source=format!("functions.library_v1(\"{A}\"){{functions.member_v1(\"11\",[\"{P}\"]){{{declaration}}}}}");
 let script=rvn_parser::parse(&source).unwrap();let scopes=script.iter().filter_map(|s|if let Statement::FunctionLibrary{scope}=s{Some(scope.clone())}else{None}).collect::<Vec<_>>();
 let assets=vec![FunctionLibraryAsset::new(FunctionLibraryId::parse(A).unwrap(),vec![FunctionLibraryMember{graph_id:11,access:LibraryFunctionAccess::Public,pure:!unit}]).unwrap()];
 let resources=LibraryGraphResources1027::prepare(&scopes,&assets).unwrap();let mut graphs=import_library_program1027(&script,&resources,&[]).unwrap();graphs.push(diagnosed());(graphs,assets)
}
fn diagnosed()->GraphDocument{
 let mut graph=GraphDocument::new(GraphId::new(100),GraphKind::Function{name:"caller".into()});let entry=graph.add_catalog_node(NodeKind::FunctionEntry,[0.,0.]).unwrap();let result=graph.add_catalog_node(NodeKind::FunctionReturn,[256.,0.]).unwrap();graph.connect(graph.pin_by_key(entry,"exec_out").unwrap().id,graph.pin_by_key(result,"exec_in").unwrap().id).unwrap();
 // Structural diagnostics are separate from the strict compiler's missing
 // FunctionReturn value. Preserve a real unresolved Current variable read.
 let unresolved=graph.add_catalog_node(NodeKind::VariableGet,[0.,150.]).unwrap();graph.set_property(unresolved,"name",PropertyValue::String("missing_current1027".into())).unwrap();graph
}
fn target()->QualifiedLibraryFunction{QualifiedLibraryFunction{library:FunctionLibraryId::parse(A).unwrap(),graph_id:11}}
#[test]
fn library_signatures1027_actual_current_body_result_repairs_ordinary_caller_without_compiling_its_diagnostics(){
 let(mut graphs,assets)=fixture(false);
 let member=graphs.iter_mut().find(|g|g.graph_id.get()==11).unwrap();let literal=member.nodes.values().find(|n|n.kind==NodeKind::Literal&&n.properties.get("value")==Some(&PropertyValue::Int(1))).unwrap().id;member.set_property(literal,"value",PropertyValue::String("Current changed result".into())).unwrap();
 let before=graphs.clone();assert!(prepare_library_current_cohort1027(&graphs,&assets).is_err());
 let view=prepare_library_current_signatures1027(&graphs,&assets).unwrap();let resources=view.resources().unwrap();let caller=graphs.iter().find(|g|g.graph_id.get()==100).unwrap();let diagnostics=caller.validate();assert!(diagnostics.iter().any(|d|d.severity==DiagnosticSeverity::Error));assert!(diagnostics.iter().any(|d|d.code=="unknown_variable"));
 let(next,node)=caller.prepare_library_call_node1027(target(),[400.,100.],&resources).unwrap();
 assert_eq!(next.pin_by_key(node,"result").unwrap().value_type,ValueType::String,"result comes from the actual edited Current body, never original Saved Int");
 for d in diagnostics{assert!(next.validate().contains(&d))}assert_eq!(graphs,before);
 let mut completed=next;
 let unresolved=completed.nodes.values().find(|n|n.kind==NodeKind::VariableGet&&n.properties.get("name")==Some(&PropertyValue::String("missing_current1027".into()))).unwrap().id;assert!(completed.remove_node(unresolved).is_some(),"repair the original diagnostic before strict Compile");let result=completed.nodes.values().find(|n|n.kind==NodeKind::FunctionReturn).unwrap().id;completed.connect(completed.pin_by_key(node,"result").unwrap().id,completed.pin_by_key(result,"value").unwrap().id).unwrap();completed.set_literal_input(node,"item_0",PropertyValue::Int(5)).unwrap();
 *graphs.iter_mut().find(|g|g.graph_id.get()==100).unwrap()=completed;
 let strict=prepare_library_current_cohort1027(&graphs,&assets).unwrap();let parsed=rvn_parser::parse(strict.source()).unwrap();let functions=rvn_core::eval::FunctionLibrary::from_script_with_library_assets(&parsed,&assets).unwrap();assert_eq!(functions.eval(&rvn_parser::Expr::Call{name:"caller".into(),args:vec![]},&Default::default()).unwrap(),rvn_parser::Value::Str("Current changed result".into()));
}
#[test]
fn library_signatures1027_missing_current_member_body_or_retired_identity_refuses_without_any_or_source_mutation(){
 let(mut graphs,assets)=fixture(false);let member=graphs.iter_mut().find(|g|g.graph_id.get()==11).unwrap();let literal=member.nodes.values().find(|n|n.kind==NodeKind::Literal).unwrap().id;member.remove_node(literal).unwrap();let before=graphs.clone();assert!(prepare_library_current_signatures1027(&graphs,&assets).is_err());assert_eq!(graphs,before);
 let(graphs,assets)=fixture(false);let before=graphs.clone();let retired=vec![FunctionLibraryAsset::new(assets[0].id().clone(),vec![]).unwrap()];assert!(prepare_library_current_signatures1027(&graphs,&retired).is_err());assert_eq!(graphs,before);
}
#[test]
fn library_signatures1027_real_unit_entry_and_call_keep_original_parameter_identity_and_empty_defaults(){
 let(graphs,assets)=fixture(true);let before=graphs.clone();let view=prepare_library_current_signatures1027(&graphs,&assets).unwrap();assert_eq!(view.scopes()[0].members()[0].parameter_ids()[0].as_str(),P);assert_eq!(view.assets(),assets);
 let caller=graphs.iter().find(|g|g.graph_id.get()==100).unwrap();let(next,node)=caller.prepare_library_call_node1027(target(),[400.,100.],&view.resources().unwrap()).unwrap();let call=next.library_call1027(node).unwrap();assert_eq!(call.arguments[0].0.as_str(),P);let pin=&next.pins[&call.arguments[0].1];assert_eq!(pin.parameter_id.as_ref().unwrap().as_str(),P);assert!(pin.default_value.is_none());assert_eq!(graphs,before);
}
