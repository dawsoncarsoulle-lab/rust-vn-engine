//! Real Graph edits and emitted-source admission; CPU definitions only.
use super::*;
use rvn_parser::function_library_asset1023::FunctionLibraryMember;
const A:&str="11111111111141118111111111111111";
const B:&str="22222222222242228222222222222222";
fn asset(id:&str,rows:&[(u64,bool)])->FunctionLibraryAsset{
    FunctionLibraryAsset::new(FunctionLibraryId::parse(id).unwrap(),rows.iter().map(|(id,pure)|FunctionLibraryMember{graph_id:*id,pure:*pure,access:LibraryFunctionAccess::Public}).collect()).unwrap()
}
fn original()->(Vec<GraphDocument>,Vec<FunctionLibraryAsset>){
    let text=format!("functions.library_v1(\"{A}\"){{functions.member_v1(\"11\",[]){{function same(){{return 1}}}}}}\nscene label start\nset answer=functions.value_v1(\"{A}\",\"11\",{{}})\n\"After\"");
    let script=rvn_parser::parse(&text).unwrap();let scopes=script.iter().filter_map(|s|if let Statement::FunctionLibrary{scope}=s{Some(scope.clone())}else{None}).collect::<Vec<_>>();let assets=vec![asset(A,&[(11,true)])];let resources=LibraryGraphResources1027::prepare(&scopes,&assets).unwrap();(import_library_program1027(&script,&resources,&[]).unwrap(),assets)
}
fn change_return(graphs:&mut[GraphDocument],value:PropertyValue){let graph=graphs.iter_mut().find(|g|g.graph_id.get()==11).unwrap();let id=graph.nodes.values().find(|n|n.kind==NodeKind::Literal&&n.properties.get("value")==Some(&PropertyValue::Int(1))).unwrap().id;graph.set_property(id,"value",value).unwrap();}
#[test]
fn library_current1027_emits_actual_changed_body_and_refuses_stale_caller_without_mutation(){
    let(mut graphs,assets)=original();change_return(&mut graphs,PropertyValue::Int(9));let prepared=prepare_library_current_cohort1027(&graphs,&assets).unwrap();assert!(prepared.source().contains("return 9"));assert_eq!(prepared.graphs(),graphs);
    let(mut graphs,assets)=original();change_return(&mut graphs,PropertyValue::String("changed".into()));let before=graphs.clone();validate_library_membership_shape1027(&graphs,&assets).unwrap();assert!(prepare_library_current_cohort1027(&graphs,&assets).err().unwrap().contains("output is stale"));assert_eq!(graphs,before,"compile refusal does not repair an obsolete typed caller implicitly");
    let member=graphs.iter().find(|g|g.graph_id.get()==11).unwrap().clone();let prepared=prepare_library_current_cohort1027(&[member],&assets).unwrap();assert!(prepared.source().contains("changed"));
}
#[test]
fn library_current1027_new_member_has_real_completion_entry_without_value_or_reserved_identity(){
    let owner=FunctionLibraryId::parse(A).unwrap();let graph=prepare_initial_library_member1027(owner,GraphId::new(11),"NewFunction").unwrap();assert!(matches!(graph.kind,GraphKind::UnitFunction{..}));assert_eq!(graph.nodes.len(),1);assert!(graph.nodes.values().all(|n|n.kind==NodeKind::FunctionEntry));assert!(graph.library1027.as_ref().unwrap().entry_parameter_ids.is_empty());let prepared=prepare_library_current_cohort1027(&[graph],&[asset(A,&[(11,false)])]).unwrap();let member=&prepared.scopes()[0].members()[0];assert!(matches!(member.declaration(),Statement::UnitFunction{body,..}if body.is_empty()));assert!(!prepared.source().contains("return"));
    assert!(prepare_initial_library_member1027(FunctionLibraryId::parse(A).unwrap(),GraphId::new(0),"bad").is_err());
}
#[test]
fn library_current1027_membership_rejects_same_owner_homonym_but_keeps_cross_owner_names_nominal(){
    let first=prepare_initial_library_member1027(FunctionLibraryId::parse(A).unwrap(),GraphId::new(11),"same").unwrap();let duplicate=prepare_initial_library_member1027(FunctionLibraryId::parse(A).unwrap(),GraphId::new(12),"same").unwrap();let graphs=vec![first.clone(),duplicate];let before=graphs.clone();assert!(validate_library_membership_shape1027(&graphs,&[asset(A,&[(11,false),(12,false)])]).unwrap_err().contains("local member name"));assert_eq!(graphs,before);
    let other=prepare_initial_library_member1027(FunctionLibraryId::parse(B).unwrap(),GraphId::new(22),"same").unwrap();prepare_library_current_cohort1027(&[first,other],&[asset(A,&[(11,false)]),asset(B,&[(22,false)])]).unwrap();
}
