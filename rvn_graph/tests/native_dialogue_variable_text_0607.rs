//! Target regressions: PREPARED, NOT COMPILED / NON_RUN. Existing strict
//! SourceProject APIs only; no new AST/schema/intrinsic is fabricated here.
use rvn_graph::*;
#[path="support/dialogue_mount_0624.rs"] mod mount;
use rvn_core::{Engine,TerminalRenderer,Interaction};
use rvn_core::random::RandomState;
use std::collections::HashMap;

enum Read {Get,Set,Local}
fn current(graph:&GraphDocument,node:NodeId,key:&str)->PinId{graph.pin_by_key(node,key).unwrap().id}
fn wire(graph:&mut GraphDocument,output:PinId,input:PinId){graph.connect(output,input).expect("actual catalog-compatible authored wire");}
fn replacement(graph:&mut GraphDocument,dialogue:NodeId,output:PinId,reroute:bool){mount::replace_text_source(graph,dialogue,output,reroute)}
fn output(graph:&mut GraphDocument,read:Read,name:&str)->PinId{
    match read{
        Read::Get=>{let node=graph.add_catalog_node(NodeKind::VariableGet,[600.0,280.0]).unwrap();graph.set_property(node,"name",PropertyValue::String(name.into())).unwrap();current(graph,node,"value")},
        Read::Local=>{let nodes:Vec<_>=graph.nodes.values().filter(|node|node.kind==NodeKind::LocalVariable&&graph.variable_node_name(node.id).as_deref()==Some(name)).map(|node|node.id).collect();assert_eq!(nodes.len(),1);current(graph,nodes[0],"value_out")},
        Read::Set=>{let nodes:Vec<_>=graph.nodes.values().filter(|node|node.kind==NodeKind::SetVariable&&graph.variable_node_name(node.id).as_deref()==Some(name)).filter(|node|{let input=current(graph,node.id,"value");graph.edges.values().find(|edge|edge.input==input).is_some_and(|edge|graph.nodes[&graph.pins[&edge.output].node].kind==NodeKind::FunctionCall)}).map(|node|node.id).collect();assert_eq!(nodes.len(),1,"Choose actual assignment whose authored RHS is the function call, not an ID/name/value guess");current(graph,nodes[0],"value_out")}
    }
}
fn prepare(source:&str,read:Read,reroute:bool)->(String,GraphDocument){
    let mut project=SourceProject::open(source,&[]).unwrap();let mut edited=project.graphs().to_vec();
    let graph=edited.iter_mut().find(|g|matches!(&g.kind,GraphKind::Label{name}if name=="start")).unwrap();
    let dialogues:Vec<_>=graph.nodes.values().filter(|node|node.kind==NodeKind::Dialogue).map(|node|node.id).collect();assert_eq!(dialogues.len(),1);
    let output=output(graph,read,"message");replacement(graph,dialogues[0],output,reroute);graph.enable_native_frame_preparation().unwrap();let expected=graph.clone();
    finish(project,source,edited,expected)
}
fn finish(mut project:SourceProject,source:&str,edited:Vec<GraphDocument>,expected:GraphDocument)->(String,GraphDocument){
    let before_source=project.source().to_string();let before_graphs=project.graphs().to_vec();
    let result=project.apply_visual(source,&edited);
    if let Err(error)=&result{assert_eq!(project.source(),before_source);assert_eq!(project.graphs(),before_graphs);if let Ok(root)=std::env::var("RVN_DIALOGUE_VARIABLE_FIXTURES"){let root=std::path::PathBuf::from(root);std::fs::create_dir_all(&root).unwrap();std::fs::write(root.join(format!("rejected-{}.graph.json",expected.graph_id)),expected.to_pretty_json().unwrap()).unwrap();std::fs::write(root.join(format!("rejected-{}.txt",expected.graph_id)),error).unwrap();}}
    result.expect("Actual variable/read output wired to Dialogue.text must survive the strict source writer without losing provenance");
    let parsed=rvn_parser::parse_program(project.source()).unwrap();
    let restored=import_native_frame_script(&parsed.script).unwrap();let actual=restored.iter().find(|g|g.graph_id==expected.graph_id).unwrap();
    assert_eq!(actual.edges,expected.edges);assert_eq!(actual.nodes.keys().collect::<Vec<_>>(),expected.nodes.keys().collect::<Vec<_>>());assert_eq!(actual.pins.keys().collect::<Vec<_>>(),expected.pins.keys().collect::<Vec<_>>());
    (project.source().to_string(),expected)
}
fn engine(source:&str)->Engine<TerminalRenderer>{let mut engine=Engine::from_program_with_debugger(rvn_parser::parse_program(source).unwrap(),TerminalRenderer,16).unwrap();engine.state.random=RandomState::seeded(11);engine}
fn shown(text:&str)->Option<Interaction>{Some(Interaction::Dialogue{character:None,text:text.into()})}
const LOCAL:&str=include_str!("fixtures/dialogue-text-0607/local-get.rvn");
const SET:&str=include_str!("fixtures/dialogue-text-0607/set-current-value.rvn");
const LOCAL_OUTPUT:&str=include_str!("fixtures/dialogue-text-0607/local-current-value.rvn");

#[test]
fn actual_get_local_dialogue_text_keeps_real_source_identity_and_runtime(){let(source,_)=prepare(LOCAL,Read::Get,false);let mut engine=engine(&source);assert_eq!(engine.step_until_interaction().unwrap(),shown("été 😀"));}
#[test]
fn actual_get_reroute_dialogue_text_keeps_all_real_edges_and_pins(){let(source,_)=prepare(LOCAL,Read::Get,true);assert_eq!(engine(&source).step_until_interaction().unwrap(),shown("été 😀"));}
#[test]
fn actual_set_output_reads_current_lvalue_without_reexecuting_old_random_rhs(){let(source,_)=prepare(SET,Read::Set,false);let mut engine=engine(&source);let mut expected=RandomState::seeded(11);expected.integer(1,999).unwrap();assert_eq!(engine.step_until_interaction().unwrap(),shown("late"));assert_eq!(engine.state.random,expected);}
#[test]
fn actual_local_output_reads_current_slot_after_later_assignment(){let(source,_)=prepare(LOCAL_OUTPUT,Read::Local,false);let mut engine=engine(&source);let mut expected=RandomState::seeded(11);expected.integer(1,999).unwrap();assert_eq!(engine.step_until_interaction().unwrap(),shown("late"));assert_eq!(engine.state.random,expected);}

fn locale()->rvn_core::locale::LocaleManager{
    use rvn_core::locale::{LocaleManager,LocaleTable};let catalogs=HashMap::from([
        ("en".into(),LocaleTable{lang:"en".into(),strings:HashMap::new()}.into()),
        ("fr".into(),LocaleTable{lang:"fr".into(),strings:HashMap::from([("[message]".into(),"Traduit [message] / [message]".into())])}.into())]);
    LocaleManager::from_frozen_catalogs("en","fr",vec!["en".into(),"fr".into()],catalogs).unwrap()
}
#[test]
fn authored_locale_key_survives_get_capture_cold_redraw_language_switch_and_rng_once(){
    let(source,_)=prepare(LOCAL_OUTPUT,Read::Get,false);let program=rvn_parser::parse_program(&source).unwrap();let mut warm=engine(&source);warm.locale=Some(locale());let mut expected=RandomState::seeded(11);expected.integer(1,999).unwrap();
    assert_eq!(warm.step_until_interaction().unwrap(),shown("Traduit late / late"));assert_eq!(warm.state.random,expected);
    let bytes=warm.capture_save15(&program,None,1,"Authored Dialogue key".into()).unwrap().encode().unwrap();let mut cold=engine(&source);cold.locale=Some(locale());cold.restore_save15(&program,rvn_core::save::consumer_checkpoint::Save15::decode(&bytes).unwrap()).unwrap();
    let before=serde_json::to_value(&cold.state).unwrap();for _ in 0..3{assert_eq!(cold.displayed_interaction().unwrap(),shown("Traduit late / late"));assert_eq!(cold.step_until_interaction().unwrap(),shown("Traduit late / late"));}assert_eq!(serde_json::to_value(&cold.state).unwrap(),before);assert_eq!(cold.state.random,expected);
    cold.locale.as_mut().unwrap().set_language("en").unwrap();assert_eq!(cold.displayed_interaction().unwrap(),shown("late"));assert_eq!(serde_json::to_value(&cold.state).unwrap(),before);
    let mut changed=engine(&source.replace("\"late\"","\"changed\""));changed.locale=Some(locale());let changed_program=rvn_parser::parse_program(&source.replace("\"late\"","\"changed\"")).unwrap();let before=serde_json::to_value(&changed.state).unwrap();assert!(changed.restore_save15(&changed_program,rvn_core::save::consumer_checkpoint::Save15::decode(&bytes).unwrap()).is_err());assert_eq!(serde_json::to_value(&changed.state).unwrap(),before);
}

#[test]
fn shared_get_reads_real_global_then_local_binding_and_rollback_retains_first_visit(){
    let source=include_str!("fixtures/dialogue-text-0607/shared-global-local.rvn");let project=SourceProject::open(source,&[]).unwrap();let mut edited=project.graphs().to_vec();let graph=edited.iter_mut().find(|g|matches!(&g.kind,GraphKind::Label{name}if name=="start")).unwrap();
    let dialogues:Vec<_>=graph.nodes.values().filter(|node|node.kind==NodeKind::Dialogue).map(|node|node.id).collect();assert_eq!(dialogues.len(),2);let get=output(graph,Read::Get,"message");for dialogue in dialogues{replacement(graph,dialogue,get,false);}graph.enable_native_frame_preparation().unwrap();let expected=graph.clone();
    let(source,_)=finish(project,source,edited,expected);let mut warm=engine(&source);assert_eq!(warm.step_until_interaction().unwrap(),shown("global"));let before=serde_json::to_value(&warm.state).unwrap();let random=warm.state.random;warm.advance_dialogue().unwrap();assert_eq!(warm.step_until_interaction().unwrap(),shown("local"));let latest=serde_json::to_value(&warm.state).unwrap();assert!(warm.rollback());assert_eq!(serde_json::to_value(&warm.state).unwrap(),latest,"first pop restores latest actual display boundary");assert!(warm.rollback());assert_eq!(serde_json::to_value(&warm.state).unwrap(),before);assert_eq!(warm.displayed_interaction().unwrap(),shown("global"));assert_eq!(warm.state.random,random);
}

#[test]
fn equal_variable_values_do_not_merge_distinct_get_identities_or_wires(){
    let source=include_str!("fixtures/dialogue-text-0607/equal-values-distinct-bindings.rvn");let project=SourceProject::open(source,&[]).unwrap();let mut edited=project.graphs().to_vec();let graph=edited.iter_mut().find(|g|matches!(&g.kind,GraphKind::Label{name}if name=="start")).unwrap();
    let dialogues:Vec<_>=graph.nodes.values().filter(|node|node.kind==NodeKind::Dialogue).map(|node|node.id).collect();assert_eq!(dialogues.len(),2);
    // Input templates resolve each actual lexical role; node IDs or positions
    // do not choose which of two equal-valued variables the user connected.
    for dialogue in dialogues{let name=mount::source_variable_name(graph,dialogue);assert!(matches!(name.as_str(),"a"|"b"));let get=output(graph,Read::Get,&name);replacement(graph,dialogue,get,false);}
    graph.enable_native_frame_preparation().unwrap();let expected=graph.clone();let(source,graph)=finish(project,source,edited,expected);assert_eq!(graph.nodes.values().filter(|node|node.kind==NodeKind::VariableGet).count(),2);let mut warm=engine(&source);assert_eq!(warm.step_until_interaction().unwrap(),shown("same"));warm.advance_dialogue().unwrap();assert_eq!(warm.step_until_interaction().unwrap(),shown("same"));
}

#[test]
fn translation_cannot_introduce_an_unbound_expression_or_commit_rng_on_failure(){
    let(source,_)=prepare(LOCAL_OUTPUT,Read::Get,false);use rvn_core::locale::{LocaleManager,LocaleTable};let catalogs=HashMap::from([("en".into(),LocaleTable{lang:"en".into(),strings:HashMap::new()}.into()),("fr".into(),LocaleTable{lang:"fr".into(),strings:HashMap::from([("[message]".into(),"[random(1,999)]".into())])}.into())]);let mut warm=engine(&source);warm.locale=Some(LocaleManager::from_frozen_catalogs("en","fr",vec!["en".into(),"fr".into()],catalogs).unwrap());
    // Advance noninteractive assignments first: the failed Dialogue must not
    // roll back a previous authored assignment, nor consume a new random draw.
    warm.enter_label("start").unwrap();let mut reached=false;for _ in 0..16{if matches!(warm.peek_statement(),Some(rvn_parser::Statement::ScheduledConsumer{body,..})if matches!(body.as_slice(),[rvn_parser::Statement::Dialogue{..}])){reached=true;break;}let mut session=warm.debug_begin(rvn_core::engine::DebugOperation::Statement,vec![]).unwrap();let mut finished=false;for _ in 0..64{if warm.debug_step(&mut session,rvn_core::editor_debugger::StepKind::Continue).unwrap().0==rvn_core::engine::DebugProgress::Complete{finished=true;break;}}assert!(finished);}assert!(reached,"Real preceding assignments must complete at the actual Dialogue PC");let before=serde_json::to_value(&warm.state).unwrap();let random=warm.state.random;assert!(warm.step_until_interaction().is_err());assert_eq!(serde_json::to_value(&warm.state).unwrap(),before);assert_eq!(warm.state.random,random);
}
