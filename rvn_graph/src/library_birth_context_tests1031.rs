//! Structural factory and real Source lifecycle; no native admission.
use super::*;
use crate::{SourceProject,NodeKind,PropertyValue,VariableDefinition,ValueType};
fn owner()->FunctionLibraryId{FunctionLibraryId::parse("11111111111141118111111111111111").unwrap()}
fn source(value:i64,character:&str)->String{format!("init{{set amount={value}\ncharacter.create(\"iris\",\"{character}\")}}\nfunction shadow(amount){{local hidden=\"local\"\nreturn amount}}\nfunction local_shadow(){{local amount=\"Local shadow\"\nreturn amount}}\nscene label start\nreturn\n")}
#[test]
fn library_birth1031_captured_current_globals_and_characters_win_over_saved_and_function_shadows_without_compiling_incomplete_callers(){
 let saved=SourceProject::open(source(7,"Saved Iris"),&[]).unwrap();
 let current=SourceProject::open(source(19,"Current Iris"),saved.graphs()).unwrap();let mut graphs=current.graphs().to_vec();
 let init=graphs.iter().find(|graph|graph.kind==GraphKind::Init).unwrap().clone();
 let shadow=graphs.iter().find(|graph|matches!(&graph.kind,GraphKind::Function{name}if name=="shadow")).unwrap();
 assert_eq!(shadow.variable_scope("amount"),Some(VariableScope::Parameter));assert_eq!(shadow.variable_scope("hidden"),Some(VariableScope::Local));
 let local_shadow=graphs.iter().find(|graph|matches!(&graph.kind,GraphKind::Function{name}if name=="local_shadow")).unwrap();assert_eq!(local_shadow.variable_scope("amount"),Some(VariableScope::Local));assert_eq!(local_shadow.variables["amount"].value_type,ValueType::Any);let declarations=local_shadow.local_variable_initializers("amount").unwrap();assert_eq!(declarations.len(),1);assert_eq!(declarations[0].literal_value,Some(PropertyValue::String("Local shadow".into())));
 assert!(crate::transpile_project(&graphs).is_ok(),"positive control: complete captured project compiles before the connected unresolved UnitCall");
 let label=graphs.iter_mut().find(|graph|matches!(graph.kind,GraphKind::Label{..})).unwrap();
 let entry=label.nodes.values().find(|node|node.kind==NodeKind::Label).unwrap().id;let output=label.pin_by_key(entry,"exec_out").unwrap().id;
 label.disconnect_pin(output);let call=label.add_catalog_node(NodeKind::UnitCall,[200.,100.]).unwrap();label.set_property(call,"function",PropertyValue::String("unresolved_birth_caller".into())).unwrap();let input=label.pin_by_key(call,"exec_in").unwrap().id;label.connect(output,input).unwrap();
 let error=crate::transpile_project(&graphs).unwrap_err();assert!(error.contains("Unit callee must resolve uniquely in the actual project/source context"),"the connected actual UnitCall must be the strict refusal cause: {error}");
 let before=graphs.clone();let id=GraphId::new(graphs.iter().map(|graph|graph.graph_id.get()).max().unwrap()+1);
 let fresh=prepare_initial_library_member_in_current1027(owner(),id,"Fresh",&graphs).unwrap();
 assert_eq!(fresh.variables["amount"],init.variables["amount"]);assert_eq!(fresh.variables["amount"].default_value,PropertyValue::Int(19));assert!(!fresh.variables.contains_key("hidden"));assert_eq!(fresh.characters,init.characters);assert_eq!(fresh.characters["iris"],"Current Iris");
 let mut expected=crate::prepare_initial_library_member1027(owner(),id,"Fresh").unwrap();expected.variables=init.variables.clone();expected.characters=init.characters.clone();assert_eq!(fresh,expected);
 assert_eq!(graphs,before);assert_eq!(saved.graphs().iter().find(|graph|graph.kind==GraphKind::Init).unwrap().variables["amount"].default_value,PropertyValue::Int(7));
}
#[test]
fn library_birth1031_structural_context_filters_source_less_locals_and_rejects_conflicting_globals_characters_init_and_ids(){
 let project=SourceProject::open("function isolated(p){local local_value=7\nreturn p}\nscene label start\nreturn\n",&[]).unwrap();
 let function=project.graphs().iter().find(|graph|matches!(graph.kind,GraphKind::Function{..})).unwrap().clone();
 let fresh=prepare_initial_library_member_in_current1027(owner(),GraphId::new(50),"Fresh",std::slice::from_ref(&function)).unwrap();assert!(fresh.variables.is_empty());assert!(fresh.characters.is_empty());
 let mut bad_parameter=function.clone();let entry=bad_parameter.nodes.values_mut().find(|node|node.kind==NodeKind::FunctionEntry).unwrap();entry.properties.insert("parameters".into(),PropertyValue::String("malformed".into()));assert!(prepare_initial_library_member_in_current1027(owner(),GraphId::new(50),"Fresh",&[bad_parameter]).unwrap_err().contains("parameter declarations"));
 let mut foreign_without_init=function.clone();foreign_without_init.characters.insert("foreign".into(),"Foreign".into());assert!(prepare_initial_library_member_in_current1027(owner(),GraphId::new(50),"Fresh",&[foreign_without_init]).unwrap_err().contains("character registry"));
 let first=crate::prepare_initial_library_member1027(owner(),GraphId::new(51),"One").unwrap();let mut a=first.clone();a.variables.insert("global".into(),VariableDefinition{name:"global".into(),value_type:ValueType::Int,default_value:PropertyValue::Int(1)});let mut b=a.clone();b.graph_id=GraphId::new(52);b.variables.get_mut("global").unwrap().default_value=PropertyValue::Int(2);let conflicting=vec![a,b];let before=conflicting.clone();assert!(prepare_initial_library_member_in_current1027(owner(),GraphId::new(53),"Fresh",&conflicting).unwrap_err().contains("contradictory"));assert_eq!(conflicting,before);
 let project=SourceProject::open(source(1,"Iris"),&[]).unwrap();let graphs=project.graphs().to_vec();let mut foreign=graphs.clone();foreign.iter_mut().find(|graph|matches!(graph.kind,GraphKind::Label{..})).unwrap().characters.insert("iris".into(),"Foreign Iris".into());assert!(prepare_initial_library_member_in_current1027(owner(),GraphId::new(60),"Fresh",&foreign).unwrap_err().contains("character registry"));
 let mut malformed=graphs.clone();let init=malformed.iter_mut().find(|graph|graph.kind==GraphKind::Init).unwrap();let local=init.add_catalog_node(NodeKind::LocalVariable,[200.,100.]).unwrap();init.set_property(local,"name",PropertyValue::String("amount".into())).unwrap();assert!(prepare_initial_library_member_in_current1027(owner(),GraphId::new(60),"Fresh",&malformed).unwrap_err().contains("local ownership"));
 let mut duplicated=graphs.clone();duplicated.push(graphs[0].clone());assert!(prepare_initial_library_member_in_current1027(owner(),GraphId::new(60),"Fresh",&duplicated).unwrap_err().contains("repeats a GraphId"));assert!(prepare_initial_library_member_in_current1027(owner(),graphs[0].graph_id,"Fresh",&graphs).unwrap_err().contains("already belongs"));
 let mut second_init=graphs.iter().find(|graph|graph.kind==GraphKind::Init).unwrap().clone();second_init.graph_id=GraphId::new(61);let mut ambiguous=graphs.clone();ambiguous.push(second_init);assert!(prepare_initial_library_member_in_current1027(owner(),GraphId::new(60),"Fresh",&ambiguous).unwrap_err().contains("unambiguous Current Init"));
}
