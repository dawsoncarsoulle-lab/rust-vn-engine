//! Real Source writer/cold reader and actual import reconciliation. No App,
//! native lifecycle acknowledgement or filtered graph equality is involved.
use super::*;
use rvn_parser::function_library_asset1023::{FunctionLibraryMember,LibraryFunctionAccess};
use std::{fs,path::PathBuf,sync::atomic::{AtomicU64,Ordering}};

const OWNER:&str="11111111111141118111111111111111";
const PARAM:&str="33333333333343338333333333333333";
const PARAM_NAME:&str="amount_named_1030";
fn asset()->FunctionLibraryAsset{
    FunctionLibraryAsset::new(FunctionLibraryId::parse(OWNER).unwrap(),vec![FunctionLibraryMember{graph_id:11,access:LibraryFunctionAccess::Public,pure:true}]).unwrap()
}
fn text(parameter:&str,body:&str,initial:&str,value:&str)->String{
    format!("functions.library_v1(\"{OWNER}\"){{functions.member_v1(\"11\",[\"{PARAM}\"]){{function plus({parameter}){{return {body}}}}}}}\ninit {{set actual={initial}}}\nscene label start\nset actual={value}\nlabel __blueprint_project_end\n")
}
fn call(value:&str)->String{format!("functions.value_v1(\"{OWNER}\",\"11\",{{\"{PARAM}\":{value}}})")}
fn scopes(script:&rvn_parser::Script)->Vec<FunctionLibraryScope>{script.iter().filter_map(|s|if let Statement::FunctionLibrary{scope}=s{Some(scope.clone())}else{None}).collect()}
struct Fixture(PathBuf);
impl Fixture{
    fn new()->Self{
        static NEXT:AtomicU64=AtomicU64::new(0);
        let root=std::env::temp_dir().join(format!("rvn-library-cold-authoring1030-{}-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos(),NEXT.fetch_add(1,Ordering::Relaxed)));
        fs::create_dir(&root).unwrap();fs::create_dir(root.join("assets")).unwrap();
        fs::write(root.join("rvn.toml"),format!("[paths]\nassets=\"assets\"\n[function_libraries]\n\"{OWNER}\"=\"one.rvnlibrary\"\n")).unwrap();
        fs::write(root.join("assets/one.rvnlibrary"),asset().encode().unwrap()).unwrap();
        fs::write(root.join("story.rvn"),text(PARAM_NAME,&format!("{PARAM_NAME}+2"),"0","0")).unwrap();
        Self(root.canonicalize().unwrap())
    }
}
impl Drop for Fixture{
    fn drop(&mut self){
        if let(Ok(root),Ok(temp))=(self.0.canonicalize(),std::env::temp_dir().canonicalize()){
            if root.starts_with(&temp)&&root.file_name().is_some_and(|name|name.to_string_lossy().starts_with("rvn-library-cold-authoring1030-")){let _=fs::remove_dir_all(root);}
        }
    }
}
fn caller(graphs:&[GraphDocument])->&GraphDocument{graphs.iter().find(|g|matches!(&g.kind,GraphKind::Label{name}if name=="start")).unwrap()}
fn consumer(graph:&GraphDocument)->NodeId{graph.nodes.values().find(|node|node.kind==NodeKind::SetVariable&&node.properties.get("name")==Some(&PropertyValue::String("actual".into()))).unwrap().id}
fn authored()->(Fixture,SourceWorkspace,Vec<GraphDocument>){
    let fixture=Fixture::new();let workspace=SourceWorkspace::link_scoped_libraries1027(&fixture.0,&fixture.0.join("story.rvn"),&[]).unwrap();
    let mut graphs=workspace.project().graphs().to_vec();
    let inputs=workspace.source_library_inputs1027().unwrap();let signatures=crate::prepare_library_current_signatures1027(&graphs,inputs.assets()).unwrap();let resources=signatures.resources().unwrap();
    let original=caller(&graphs);let graph_id=original.graph_id;
    let(mut next,node)=original.prepare_library_call_node1027(QualifiedLibraryFunction{library:FunctionLibraryId::parse(OWNER).unwrap(),graph_id:11},[400.,100.],&resources).unwrap();
    let binding=next.library_call1027(node).unwrap().clone();let argument=binding.arguments[0].1;let key=next.pins[&argument].key.clone();
    assert_eq!(binding.arguments[0].0.as_str(),PARAM);assert_eq!(next.pins[&argument].label,PARAM_NAME);
    next.set_pin_default(node,&key,PropertyValue::Int(7)).unwrap();next.refresh_library_result_type1027(node,&resources).unwrap();
    let set=consumer(&next);let input=next.pin_by_key(set,"value").unwrap().id;assert_eq!(next.pins[&input].value_type,ValueType::Int);
    next.set_pin_default(set,"value",PropertyValue::Int(37)).unwrap();
    next.connect(next.pin_by_key(node,"result").unwrap().id,input).unwrap();assert_eq!(next.pins[&input].default_value,Some(PropertyValue::Int(37)));
    *graphs.iter_mut().find(|g|g.graph_id==graph_id).unwrap()=next;
    resources.validate_project(&graphs).unwrap();drop(resources);drop(inputs);
    (fixture,workspace,graphs)
}

#[test]
fn library_cold_authoring1030_real_source_save_cold_reopen_keeps_full_ordered_graphs_connected_int37_and_nominal_parameter_label(){
    let(fixture,mut workspace,graphs)=authored();let before=graphs.clone();workspace.save(&graphs).unwrap();
    let cold=SourceWorkspace::open_scoped_libraries1027(&fixture.0,0).unwrap().unwrap();
    assert_eq!(cold.project().graphs(),before.as_slice());assert_eq!(graphs,before);
    let graph=caller(cold.project().graphs());let input=graph.pin_by_key(consumer(graph),"value").unwrap();assert_eq!(input.default_value,Some(PropertyValue::Int(37)));assert!(graph.edges.values().any(|edge|edge.input==input.id));
    let binding=graph.library1027.as_ref().unwrap().calls.values().next().unwrap();assert_eq!(binding.target.library.as_str(),OWNER);assert_eq!(binding.arguments[0].0.as_str(),PARAM);assert_eq!(graph.pins[&binding.arguments[0].1].label,PARAM_NAME);
}

#[test]
fn library_cold_authoring1030_changed_source_literal_wins_over_old_connected_fallback_without_filtered_equality(){
    let(_fixture,_workspace,graphs)=authored();let script=rvn_parser::parse(&text(PARAM_NAME,&format!("{PARAM_NAME}+2"),"0","99")).unwrap();let owned=scopes(&script);let assets=[asset()];let resources=LibraryGraphResources1027::prepare(&owned,&assets).unwrap();
    let next=crate::import_library_program1027(&script,&resources,&graphs).unwrap();let graph=caller(&next);let pin=graph.pin_by_key(consumer(graph),"value").unwrap();
    assert_eq!(pin.default_value,Some(PropertyValue::Int(99)));assert!(!graph.edges.values().any(|edge|edge.input==pin.id));assert_eq!(caller(&graphs).pin_by_key(consumer(caller(&graphs)),"value").unwrap().default_value,Some(PropertyValue::Int(37)));
}

#[test]
fn library_cold_authoring1030_incompatible_float_source_cannot_restore_old_int_fallback_behind_new_wire(){
    let(_fixture,_workspace,graphs)=authored();let script=rvn_parser::parse(&text(PARAM_NAME,&format!("{PARAM_NAME}+2.5"),"0.0",&call("7"))).unwrap();let owned=scopes(&script);let assets=[asset()];let resources=LibraryGraphResources1027::prepare(&owned,&assets).unwrap();
    let next=crate::import_library_program1027(&script,&resources,&graphs).unwrap();let graph=caller(&next);let pin=graph.pin_by_key(consumer(graph),"value").unwrap();
    assert_eq!(pin.value_type,ValueType::Float);assert_eq!(pin.default_value,None);assert!(graph.edges.values().any(|edge|edge.input==pin.id));
    let old=caller(&graphs).pin_by_key(consumer(caller(&graphs)),"value").unwrap();assert_eq!(old.value_type,ValueType::Int);assert_eq!(old.default_value,Some(PropertyValue::Int(37)));
}

#[test]
fn library_cold_authoring1030_signature_rename_uses_same_opaque_parameter_id_and_current_canonical_label(){
    let(_fixture,_workspace,graphs)=authored();let renamed="renamed_amount_1030";let script=rvn_parser::parse(&text(renamed,&format!("{renamed}+2"),"0",&call("7"))).unwrap();let owned=scopes(&script);let assets=[asset()];let resources=LibraryGraphResources1027::prepare(&owned,&assets).unwrap();
    let next=crate::import_library_program1027(&script,&resources,&graphs).unwrap();let before=caller(&graphs);let after=caller(&next);let old=before.library1027.as_ref().unwrap().calls.values().next().unwrap();let actual=after.library1027.as_ref().unwrap().calls.values().next().unwrap();
    assert_eq!(actual.target,old.target);assert_eq!(actual.arguments,old.arguments);assert_eq!(actual.arguments[0].0.as_str(),PARAM);assert_eq!(after.pins[&actual.arguments[0].1].label,renamed);assert_eq!(before.pins[&old.arguments[0].1].label,PARAM_NAME);
}

#[test]
fn library_cold_authoring1030_excluded_function_call_inputs_do_not_gain_inline_retention(){
    let(_fixture,_workspace,mut graphs)=authored();
    let graph=graphs.iter_mut().find(|g|matches!(&g.kind,GraphKind::Label{name}if name=="start")).unwrap();let(&node,binding)=graph.library1027.as_ref().unwrap().calls.first_key_value().unwrap();let argument=binding.arguments[0].1;let key=graph.pins[&argument].key.clone();
    let variable=graph.add_catalog_node(NodeKind::VariableGet,[100.,250.]).unwrap();graph.set_property(variable,"name",PropertyValue::String("actual".into())).unwrap();
    let output=graph.pin_by_key(variable,"value").unwrap().id;graph.connect(output,argument).unwrap();graph.set_pin_default(node,&key,PropertyValue::Int(37)).unwrap();
    assert!(!crate::document::retains_inline_pin_default(NodeKind::FunctionCall,&graph.pins[&argument].value_type));
    let mut next=graph.clone();assert_eq!(next.clear_shadowed_input_defaults(),1);let canonical=next.clone();next.reconcile_import(graph).unwrap();
    assert_eq!(next,canonical);assert!(next.edges.values().any(|edge|edge.input==argument));assert_eq!(next.pins[&argument].default_value,None);assert_eq!(graph.pins[&argument].default_value,Some(PropertyValue::Int(37)));
}

#[test]
fn library_cold_authoring1030_existing_asset_fallback_policy_still_restores_complete_graph(){
    let script=rvn_parser::parse("scene label assets\nscene \"background_wire.png\"\n").unwrap();let mut old=crate::import_script(&script).unwrap().into_iter().find(|graph|matches!(&graph.kind,GraphKind::Label{name}if name=="assets")).expect("the real authored assets scene");let scene=old.nodes.values().find(|node|node.kind==NodeKind::Scene).unwrap().id;
    old.set_pin_default(scene,"background",PropertyValue::String("background_fallback.png".into())).unwrap();let input=old.pin_by_key(scene,"background").unwrap().id;assert!(old.edges.values().any(|edge|edge.input==input));
    let mut next=crate::import_script(&script).unwrap().into_iter().find(|graph|matches!(&graph.kind,GraphKind::Label{name}if name=="assets")).expect("the real authored assets scene");next.reconcile_import(&old).unwrap();assert_eq!(next,old);
}
