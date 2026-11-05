//! The ordering boundary moves only source-reconstructed documents. Source
//! text/program/origins and real narrative execution remain authoritative.
use super::*;
use crate::{GraphKind,NodeKind,PropertyValue,SourceProject};

const BASE:&str="function old(){return 1}\nfunction keep(){return 2}\nlabel start\n\"First\"\nlabel other\n\"Second\"\n";
fn ids(graphs:&[GraphDocument])->Vec<GraphId>{graphs.iter().map(|g|g.graph_id).collect()}
fn same_fields(left:&[GraphDocument],right:&[GraphDocument]){
    assert_eq!(left.len(),right.len());
    for graph in left{assert_eq!(Some(graph),right.iter().find(|g|g.graph_id==graph.graph_id),"full fresh graph fields must remain exact");}
}
fn function(graphs:&[GraphDocument],name:&str)->GraphId{
    graphs.iter().find(|g|matches!(&g.kind,GraphKind::Function{name:actual}if actual==name)).unwrap().graph_id
}
#[test]
fn source_order1031_old_tampered_payload_cannot_replace_any_fresh_node_pin_schema_or_field(){
    let project=SourceProject::open(BASE,&[]).unwrap();let mut old=project.graphs().to_vec();old.reverse();
    let id=function(&old,"old");let graph=old.iter_mut().find(|g|g.graph_id==id).unwrap();
    let literal=graph.nodes.values().find(|n|n.kind==NodeKind::Literal).unwrap().id;
    graph.set_property(literal,"value",PropertyValue::String("Old payload must not return".into())).unwrap();
    let fresh=SourceProject::open(BASE.replace("return 1","return 7"),&[]).unwrap().graphs().to_vec();
    let snapshot=fresh.clone();let ordered=retain_order(fresh,&old).unwrap();
    assert_eq!(ids(&ordered),ids(&old));same_fields(&ordered,&snapshot);
    assert_eq!(ordered.iter().find(|g|g.graph_id==id).unwrap().nodes[&literal].properties.get("value"),Some(&PropertyValue::Int(7)));
}
#[test]
fn source_order1031_removed_scopes_stay_removed_and_new_declarations_keep_their_actual_source_order(){
    let initial=SourceProject::open(BASE,&[]).unwrap();let mut old=initial.graphs().to_vec();old.reverse();
    let removed=function(&old,"old");let next="function keep(){return 9}\nfunction NewFirst(){return 3}\nfunction NewSecond(){return 4}\nlabel start\n\"First\"\nlabel other\n\"Second\"\n";
    let current=SourceProject::open(next,&old).unwrap();let graphs=current.graphs();
    assert!(!graphs.iter().any(|g|g.graph_id==removed));
    let old_ids=old.iter().filter(|g|g.graph_id!=removed).map(|g|g.graph_id).collect::<Vec<_>>();
    assert_eq!(&ids(graphs)[..old_ids.len()],old_ids.as_slice());
    let added=graphs[old_ids.len()..].iter().map(|g|g.kind.clone()).collect::<Vec<_>>();
    assert_eq!(added,vec![GraphKind::Function{name:"NewFirst".into()},GraphKind::Function{name:"NewSecond".into()}]);
    assert_eq!(current.source(),next);assert_eq!(current.resolved_program().unwrap().script,rvn_parser::parse_program(next).unwrap().script);
}
#[test]
fn source_order1031_duplicate_prior_or_fresh_ids_refuse_without_mutating_original_documents(){
    let graphs=SourceProject::open(BASE,&[]).unwrap().graphs().to_vec();let snapshot=graphs.clone();
    let mut duplicate=graphs.clone();duplicate.push(graphs[0].clone());
    assert!(retain_order(graphs.clone(),&duplicate).unwrap_err().contains("Original Source"));
    assert!(retain_order(duplicate,&graphs).unwrap_err().contains("Fresh Source"));assert_eq!(graphs,snapshot);
}
fn dialogues(program:rvn_parser::ParsedProgram)->Vec<String>{
    let mut engine=rvn_core::Engine::from_program(program,rvn_core::TerminalRenderer,16).unwrap();let mut text=Vec::new();
    for _ in 0..4{match engine.step_until_interaction().unwrap(){
        Some(rvn_core::Interaction::Dialogue{character:None,text:value})=>{text.push(value);engine.advance_dialogue().unwrap();},
        None=>return text,other=>panic!("unexpected narrative interaction: {other:?}"),
    }}panic!("finite fallthrough fixture did not finish")
}
#[test]
fn source_order1031_external_label_order_keeps_exact_program_origins_and_real_fallthrough_execution(){
    let old_project=SourceProject::open(BASE,&[]).unwrap();let mut old=old_project.graphs().to_vec();old.rotate_left(1);
    let source="function old(){return 3}\nfunction keep(){return 2}\nlabel other\n\"Actual first\"\nlabel start\n\"Actual second\"\n";
    let mut project=SourceProject::open(source,&old).unwrap();assert_eq!(project.source(),source);
    let expected=rvn_parser::parse_program(source).unwrap();let actual=project.resolved_program().unwrap();
    assert_eq!(actual.script,expected.script);assert_eq!(actual.origins,expected.origins);
    assert_eq!(dialogues(actual),vec!["Actual first","Actual second"]);assert_eq!(dialogues(expected),vec!["Actual first","Actual second"]);
    // Presentation can remain old-slot ordered while the Source AST/PC order
    // above follows the external file. No Source rewrite happens on reopen.
    assert_eq!(ids(project.graphs()),ids(&old));
    let labels=project.graphs().iter().filter_map(|g|if let GraphKind::Label{name}=&g.kind{Some(name.as_str())}else{None}).collect::<Vec<_>>();
    assert_eq!(labels,vec!["start","other"],"presentation is deliberately opposite to the actual Source labels");
    // A later genuine visual edit must still publish into the original Source
    // statement locations, never turn presentation order into executable PC.
    let mut edited=project.graphs().to_vec();let old_id=function(&edited,"old");let graph=edited.iter_mut().find(|g|g.graph_id==old_id).unwrap();
    let literal=graph.nodes.values().find(|n|n.kind==NodeKind::Literal&&n.properties.get("value")==Some(&PropertyValue::Int(3))).unwrap().id;
    graph.set_property(literal,"value",PropertyValue::Int(4)).unwrap();let baseline=project.source().to_owned();project.apply_visual(&baseline,&edited).unwrap();
    assert!(project.source().find("label other").unwrap()<project.source().find("label start").unwrap());
    let parsed=rvn_parser::parse_program(project.source()).unwrap();let actual=project.resolved_program().unwrap();
    assert_eq!(actual.script,parsed.script);assert_eq!(actual.origins,parsed.origins);
    assert!(actual.script.iter().any(|statement|matches!(statement,rvn_parser::Statement::Function{name,body,..}if name=="old"&&body.iter().any(|s|matches!(s,rvn_parser::Statement::FunctionReturn{value:rvn_parser::Expr::Int(4)})))));
    assert_eq!(dialogues(actual),vec!["Actual first","Actual second"]);
}
#[test]
fn source_order1031_real_native_function_rename_keeps_frozen_id_but_does_not_reuse_an_old_kind_name_slot(){
    let source="function old(){return 1}\nfunction keep(){return 2}\nlabel start\n\"Waiting\"\n";
    let mut project=SourceProject::open(source,&[]).unwrap();let mut edited=project.graphs().to_vec();let id=function(&edited,"old");
    edited.iter_mut().find(|g|g.graph_id==id).unwrap().enable_native_frame_preparation().unwrap();edited.reverse();
    project.apply_visual(source,&edited).unwrap();let native_source=project.source().to_owned();
    assert_eq!(native_source.matches("function old").count(),1);let renamed_source=native_source.replace("function old","function renamed");
    let renamed=SourceProject::open(&renamed_source,project.graphs()).unwrap();
    assert_eq!(function(renamed.graphs(),"renamed"),id);assert_eq!(renamed.graphs().last().unwrap().graph_id,id);
    assert_eq!(renamed.source(),renamed_source);let expected=rvn_parser::parse_program(&renamed_source).unwrap();let actual=renamed.resolved_program().unwrap();
    assert_eq!(actual.script,expected.script);assert_eq!(actual.origins,expected.origins);
    assert_eq!(dialogues(actual),vec!["Waiting"]);
}
#[test]
fn source_order1031_nominal_library_owner_is_part_of_slot_identity_and_interleaved_current_is_not_source_grouping(){
    use rvn_parser::function_library_asset1023::FunctionLibraryId;
    let a=FunctionLibraryId::parse("11111111111141118111111111111111").unwrap();let b=FunctionLibraryId::parse("22222222222242228222222222222222").unwrap();
    let a1=crate::prepare_initial_library_member1027(a.clone(),GraphId::new(11),"AFirst").unwrap();
    let b1=crate::prepare_initial_library_member1027(b.clone(),GraphId::new(12),"BFirst").unwrap();
    let a2=crate::prepare_initial_library_member1027(a,GraphId::new(13),"ASecond").unwrap();
    let current=vec![a1.clone(),b1.clone(),a2.clone()];let grouped=vec![a1,a2,b1];let snapshot=grouped.clone();
    let ordered=retain_order(grouped,&current).unwrap();assert_eq!(ids(&ordered),vec![GraphId::new(11),GraphId::new(12),GraphId::new(13)]);same_fields(&ordered,&snapshot);
    // Negative identity probe uses inert typed graph templates only. It grants
    // no Library Source resource or descriptor admission to the changed owner.
    let changed_owner=crate::prepare_initial_library_member1027(b.clone(),GraphId::new(11),"AFirst").unwrap();
    let changed=vec![changed_owner,ordered[2].clone(),ordered[1].clone()];let snapshot=changed.clone();let reordered=retain_order(changed,&current).unwrap();
    assert_eq!(ids(&reordered),vec![GraphId::new(12),GraphId::new(13),GraphId::new(11)]);same_fields(&reordered,&snapshot);
    assert_eq!(nominal_owner(reordered.last().unwrap()),Some(&b));
}
