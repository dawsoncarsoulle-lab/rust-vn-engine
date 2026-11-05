use rvn_core::{Engine,TerminalRenderer,Interaction};
use rvn_core::random::RandomState;
use rvn_graph::*;
fn generated(source:&str)->String {
    let parsed=rvn_parser::parse_program(source).unwrap();
    let mut graphs=import_script(&parsed.script).unwrap();
    let scene=graphs.iter_mut().find(|g|matches!(&g.kind,GraphKind::Label{name}if name=="start")).unwrap();
    scene.enable_native_frame_preparation().unwrap();
    let native=prepare_native_frame_graph(scene).unwrap();
    let scene=scene.clone();
    let mut prefix=String::new();
    for graph in &graphs {if graph.kind!=scene_kind(){prefix.push_str(&transpile(graph).unwrap().source);}}
    {let source=format!("{prefix}\n{}",native.source);fixture(&fixture_name(if source.contains("function producer") {"calculated"} else {"plain"},&source),&source,&native.ast,&scene);source}
}
fn scene_kind()->GraphKind {GraphKind::Label{name:"start".into()}}
fn engine(source:&str)->Engine<TerminalRenderer> {let mut e=Engine::from_program_with_debugger(rvn_parser::parse_program(source).unwrap(),TerminalRenderer,16).unwrap();e.state.random=RandomState::seeded(11);e}
#[test]
fn actual_graph_generated_plain_dialogue_reaches_real_scene_and_next() {
    let source=generated("scene label start\n\"été 😀\"\n\"second\"\n");
    let mut e=engine(&source);let expected=Some(Interaction::Dialogue{character:None,text:"été 😀".into()});
    assert_eq!(e.step_until_interaction().unwrap(),expected);
    let guard=serde_json::to_value(&e.state).unwrap();let rng=e.state.random;
    assert_eq!(e.step_until_interaction().unwrap(),expected);assert_eq!(e.current_interaction().unwrap(),expected);assert_eq!(e.displayed_interaction().unwrap(),expected);
    assert_eq!(serde_json::to_value(&e.state).unwrap(),guard);assert_eq!(e.state.random,rng);
    assert!(e.preview_input_source().unwrap().owner.is_some());e.advance_dialogue().unwrap();
    assert_eq!(e.step_until_interaction().unwrap(),Some(Interaction::Dialogue{character:None,text:"second".into()}));
}
#[test]
fn actual_graph_generated_calculated_dialogue_gathers_once() {
    let source=generated("function producer(){local n=random(1,999999) return n}\nscene label start\n\"[producer()]\"\n\"done\"\n");
    let mut e=engine(&source);let mut expected=RandomState::seeded(11);let value=expected.integer(1,999999).unwrap();
    assert_eq!(e.step_until_interaction().unwrap(),Some(Interaction::Dialogue{character:None,text:value.to_string()}));
    assert_eq!(e.state.random,expected);
    let guard=serde_json::to_value(&e.state).unwrap();e.step_until_interaction().unwrap();assert_eq!(serde_json::to_value(&e.state).unwrap(),guard);
}

fn wire(g:&mut GraphDocument,a:NodeId,ak:&str,b:NodeId,bk:&str){g.connect(g.pin_by_key(a,ak).unwrap().id,g.pin_by_key(b,bk).unwrap().id).unwrap();}
fn terminal_graph()->GraphDocument {
 let mut g=GraphDocument::new(GraphId::new(436),GraphKind::Label{name:"start".into()});g.narrative=Some(NarrativeMetadata::SceneEntry);
 let root=g.add_catalog_node(NodeKind::Label,[0.,0.]).unwrap();let dialogue=g.add_catalog_node(NodeKind::Dialogue,[800.,0.]).unwrap();let text=g.add_catalog_node(NodeKind::TextFromStringTable,[200.,0.]).unwrap();g.set_pin_default(text,"table",PropertyValue::Name(NameDefault::new("asset:Case").unwrap())).unwrap();g.set_pin_default(text,"key",PropertyValue::String("clé\r\n😀".into())).unwrap();
 let inspector=g.add_catalog_node(NodeKind::GetStringTableIdAndKey,[400.,0.]).unwrap();let conversion=g.add_catalog_node(NodeKind::ConvertStringToText,[600.,0.]).unwrap();wire(&mut g,root,"exec_out",dialogue,"exec_in");wire(&mut g,text,"result",inspector,"text");wire(&mut g,inspector,"key",conversion,"value");wire(&mut g,conversion,"result",dialogue,"text");g.enable_native_frame_preparation().unwrap();g
}
#[test]
fn actual_inspector_typed_terminal_dialogue_preserves_ids_crlf_cold_and_ports() {
 let g=terminal_graph();let native=prepare_native_frame_graph(&g).unwrap();fixture("typed-terminal",&native.source,&native.ast,&g);let parsed=rvn_parser::parse_program(&native.source).unwrap();let restored=import_native_frame_script(&parsed.script).unwrap();let actual=restored.iter().find(|a|a.graph_id==g.graph_id).unwrap();assert_eq!(actual.edges,g.edges);assert_eq!(actual.nodes.keys().collect::<Vec<_>>(),g.nodes.keys().collect::<Vec<_>>());assert_eq!(actual.pins.keys().collect::<Vec<_>>(),g.pins.keys().collect::<Vec<_>>());
 let mut e=engine(&native.source);let shown=Some(Interaction::Dialogue{character:None,text:"clé\r\n😀".into()});assert_eq!(e.step_until_interaction().unwrap(),shown);
 let save=e.capture_save15(&parsed,None,4,"Graph inspector Dialogue".into()).unwrap();let mut cold=engine(&native.source);cold.restore_save15(&parsed,save).unwrap();assert_eq!(cold.displayed_interaction().unwrap(),shown);assert_eq!(cold.state.random,e.state.random);
}
#[test]
fn dialogue_source_identity_collision_forward_and_unknown_typed_port_refused() {
 let native=prepare_native_frame_graph(&terminal_graph()).unwrap();let mut cases=vec![];
 let mut collision=native.ast.clone();let rvn_parser::Statement::ScheduledConsumer{consumer,steps,..}=collision.iter_mut().find(|s|matches!(s,rvn_parser::Statement::ScheduledConsumer{..})).unwrap()else{panic!()};let rvn_parser::ConsumerStep::Capture{producer,..}=&steps[0]else{panic!()};*consumer=producer.clone();cases.push(collision);
 let mut forward=native.ast.clone();let rvn_parser::Statement::ScheduledConsumer{steps,..}=forward.iter_mut().find(|s|matches!(s,rvn_parser::Statement::ScheduledConsumer{..})).unwrap()else{panic!()};steps.reverse();cases.push(forward);
 let mut unknown=native.ast.clone();let rvn_parser::Statement::ScheduledConsumer{steps,..}=unknown.iter_mut().find(|s|matches!(s,rvn_parser::Statement::ScheduledConsumer{..})).unwrap()else{panic!()};let value=steps.iter_mut().find_map(|step|match step{rvn_parser::ConsumerStep::Capture{value:rvn_parser::Expr::Call{name,args},..}if name=="string_to_text"=>Some(&mut args[0]),_=>None}).unwrap();let rvn_parser::Expr::Call{args,..}=value else{panic!()};args[1]=rvn_parser::Expr::Str("invented_port".into());cases.push(unknown);
 for script in cases {assert!(import_native_frame_script(&script).is_err());}
}

fn fixture(name:&str,source:&str,ast:&rvn_parser::Script,graph:&GraphDocument) {
 if let Ok(directory)=std::env::var("RVN_DIALOGUE_FIXTURE_DIR") {let path=std::path::Path::new(&directory);std::fs::create_dir_all(path).unwrap();std::fs::write(path.join(format!("{name}.native.rvn")),source).unwrap();std::fs::write(path.join(format!("{name}.native.ast.json")),serde_json::to_vec_pretty(ast).unwrap()).unwrap();std::fs::write(path.join(format!("{name}.native.graph.json")),graph.to_pretty_json().unwrap()).unwrap();}
}

#[test]
fn actual_source_file_inspector_pin_debugger_pause_cold_continuation_keeps_real_ids() {
 let g=terminal_graph();let native=prepare_native_frame_graph(&g).unwrap();
 let directory=std::env::var("RVN_DIALOGUE_FIXTURE_DIR").map(std::path::PathBuf::from).unwrap_or_else(|_|std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures-received"));std::fs::create_dir_all(&directory).unwrap();let path=directory.join("typed-debugger.native.rvn");std::fs::write(&path,&native.source).unwrap();let p=rvn_parser::parse_file_program_with_uses(&path).unwrap();
 let inspector=g.nodes.values().find(|n|n.kind==NodeKind::GetStringTableIdAndKey).unwrap().id;let pin=g.pin_by_key(inspector,"key").unwrap().id;
 let mut e=Engine::from_program_with_debugger(p.clone(),TerminalRenderer,16).unwrap();e.enter_label("start").unwrap();let before=serde_json::to_value(&e.state).unwrap();
 let mut offset=0;let bp=loop {let page=e.debug_site_page(offset).unwrap();if let Some(entry)=page.entries.iter().find(|entry|entry.origin.as_ref().and_then(|o|o.scheduled_step.as_ref()).and_then(|step|step.endpoint.as_ref()).is_some_and(|endpoint|endpoint.graph==g.graph_id.get()&&endpoint.node==inspector.get()&&endpoint.ports.iter().any(|p|p.role=="key"&&p.pin==pin.get()))) {break entry.breakpoint.clone();}offset+=page.entries.len();assert!(offset<page.total,"actual inspector producer endpoint was not offered");};
 let mut session=e.debug_begin(rvn_core::engine::DebugOperation::Statement,vec![bp]).unwrap();assert_eq!(e.debug_step(&mut session,rvn_core::editor_debugger::StepKind::Continue).unwrap().0,rvn_core::engine::DebugProgress::Stopped);assert_eq!(serde_json::to_value(&e.state).unwrap(),before);
 let origin=session.snapshot().origin.unwrap();assert_eq!(origin.graph.as_ref().unwrap().graph,g.graph_id.get());assert_eq!(origin.graph.as_ref().unwrap().node,Some(inspector.get()));let step=origin.scheduled_step.unwrap();assert_eq!(origin.source.as_ref(),p.sources[step.source_ordinal].path.as_ref());assert!(native.source[step.expression].contains("__rvn_captured_v1"));
 let bytes=e.capture_save15(&p,Some(&session),7,"Real pin paused".into()).unwrap().encode().unwrap();finish_graph(&mut e,&mut session);let mut cold=Engine::from_program_with_debugger(p.clone(),TerminalRenderer,16).unwrap();let mut resumed=cold.restore_save15(&p,rvn_core::save::consumer_checkpoint::Save15::decode(&bytes).unwrap()).unwrap().unwrap();finish_graph(&mut cold,&mut resumed);assert_eq!(serde_json::to_value(&cold.state).unwrap(),serde_json::to_value(&e.state).unwrap());assert_eq!(cold.displayed_interaction().unwrap(),Some(Interaction::Dialogue{character:None,text:"clé\r\n😀".into()}));
}
fn finish_graph(e:&mut Engine<TerminalRenderer>,session:&mut rvn_core::engine::DebugSession){for _ in 0..128 {if e.debug_step(session,rvn_core::editor_debugger::StepKind::Continue).unwrap().0==rvn_core::engine::DebugProgress::Complete{return;}}panic!("real graph continuation did not finish")}

fn fixture_name(prefix:&str,source:&str)->String{use sha2::{Digest,Sha256};let digest=Sha256::digest(source.as_bytes());format!("{prefix}-{}",digest[..6].iter().map(|byte|format!("{byte:02x}")).collect::<String>())}
#[test]
fn graph_raw_template_body_function_pause_after_draw_cold_does_not_replay() {
 let source=generated("function producer(){local n=random(1,999999) return n}\nscene label start\n\"[producer()]\"\n\"done\"\n");let p=rvn_parser::parse_program(&source).unwrap();let mut warm=engine(&source);warm.enter_label("start").unwrap();let before=serde_json::to_value(&warm.state).unwrap();
 let mut offset=0;let bp=loop {let page=warm.debug_site_page(offset).unwrap();if let Some(entry)=page.entries.iter().find(|entry|entry.breakpoint.site.as_ref().is_some_and(|site|site.function.as_deref()==Some("producer")&&site.route==[rvn_core::editor_debugger::Route::Statement(1)])){break entry.breakpoint.clone();}offset+=page.entries.len();assert!(offset<page.total);};
 let mut session=warm.debug_begin(rvn_core::engine::DebugOperation::Statement,vec![bp]).unwrap();assert_eq!(warm.debug_step(&mut session,rvn_core::editor_debugger::StepKind::Continue).unwrap().0,rvn_core::engine::DebugProgress::Stopped);assert_eq!(serde_json::to_value(&warm.state).unwrap(),before);
 let bytes=warm.capture_save15(&p,Some(&session),8,"Raw template body paused after draw".into()).unwrap().encode().unwrap();finish_graph(&mut warm,&mut session);let mut cold=engine(&source);let mut resumed=cold.restore_save15(&p,rvn_core::save::consumer_checkpoint::Save15::decode(&bytes).unwrap()).unwrap().unwrap();finish_graph(&mut cold,&mut resumed);assert_eq!(serde_json::to_value(&cold.state).unwrap(),serde_json::to_value(&warm.state).unwrap());let mut expected=RandomState::seeded(11);let value=expected.integer(1,999999).unwrap();assert_eq!(cold.state.random,expected);assert_eq!(cold.displayed_interaction().unwrap(),Some(Interaction::Dialogue{character:None,text:value.to_string()}));
}
#[test]
fn identical_raw_templates_in_successive_consumers_and_new_scene_are_distinct_visits() {
 let source=generated("function producer(){local n=random(1,999999) return n}\nscene label start\n\"same [producer()]\"\n\"same [producer()]\"\n");let p=rvn_parser::parse_program(&source).unwrap();let mut warm=engine(&source);let mut expected=RandomState::seeded(11);let first=expected.integer(1,999999).unwrap();let first_shown=Some(Interaction::Dialogue{character:None,text:format!("same {first}")});assert_eq!(warm.step_until_interaction().unwrap(),first_shown);let activation=warm.state.scene_execution.as_ref().unwrap().active.as_ref().unwrap().id;
 let bytes=warm.capture_save15(&p,None,9,"First duplicate template".into()).unwrap().encode().unwrap();let mut cold=engine(&source);cold.restore_save15(&p,rvn_core::save::consumer_checkpoint::Save15::decode(&bytes).unwrap()).unwrap();assert_eq!(cold.step_until_interaction().unwrap(),first_shown);assert_eq!(cold.state.random,expected);
 cold.advance_dialogue().unwrap();let second=expected.integer(1,999999).unwrap();assert_eq!(cold.step_until_interaction().unwrap(),Some(Interaction::Dialogue{character:None,text:format!("same {second}")}));assert_eq!(cold.state.random,expected);assert_eq!(cold.state.scene_execution.as_ref().unwrap().active.as_ref().unwrap().id,activation);
 cold.enter_label("start").unwrap();let reopened=cold.state.scene_execution.as_ref().unwrap().active.as_ref().unwrap().id;assert_ne!(reopened,activation);let third=expected.integer(1,999999).unwrap();assert_eq!(cold.step_until_interaction().unwrap(),Some(Interaction::Dialogue{character:None,text:format!("same {third}")}));assert_eq!(cold.state.random,expected);assert_eq!(cold.state.scene_execution.as_ref().unwrap().active.as_ref().unwrap().consumer_frame.as_ref().unwrap().owner,rvn_core::consumer_schedule::ConsumerOwner::Scene{activation:reopened});
}

#[test]
fn ambiguous_active_raw_template_producers_refuse_instead_of_guessing_an_edge() {
 let source=generated("scene label start\n\"raw\"\n");let mut script=rvn_parser::parse(&source).unwrap();
 let rvn_parser::Statement::ScheduledConsumer{steps,..}=script.iter_mut().find(|s|matches!(s,rvn_parser::Statement::ScheduledConsumer{..})).unwrap()else{panic!()};let template=steps.iter().find_map(|s|match s{rvn_parser::ConsumerStep::Capture{value:rvn_parser::Expr::Str(value),..}=>Some(value.clone()),_=>None}).unwrap();
 steps.push(rvn_parser::ConsumerStep::Capture{producer:"n9999:text_value|value=999999@0".into(),value:rvn_parser::Expr::Str(template)});assert!(import_native_frame_script(&script).unwrap_err().contains("Ambiguous native raw Dialogue template producer"));
}
#[test]
fn blank_whitespace_and_crlf_raw_templates_remain_exact_through_graph_and_cold() {
 for text in [""," ","été\r\n😀"] {let source=generated(&format!("scene label start\n{text:?}\n"));let p=rvn_parser::parse_program(&source).unwrap();let mut e=engine(&source);let shown=Some(Interaction::Dialogue{character:None,text:text.into()});assert_eq!(e.step_until_interaction().unwrap(),shown);assert_eq!(e.state.random,RandomState::seeded(11));let save=e.capture_save15(&p,None,11,"Exact raw template".into()).unwrap();let mut cold=engine(&source);cold.restore_save15(&p,save).unwrap();assert_eq!(cold.displayed_interaction().unwrap(),shown);assert_eq!(cold.state.random,e.state.random);}
}
fn raw_locale()->rvn_core::locale::LocaleManager {
 use std::collections::HashMap;use rvn_core::locale::{LocaleManager,LocaleTable};let catalogs=HashMap::from([("en".into(),LocaleTable{lang:"en".into(),strings:HashMap::new()}.into()),("fr".into(),LocaleTable{lang:"fr".into(),strings:HashMap::from([("same [producer()]".into(),"Traduit [producer()]".into())])}.into())]);LocaleManager::from_frozen_catalogs("en","fr",vec!["en".into(),"fr".into()],catalogs).unwrap()
}
#[test]
fn raw_template_original_locale_key_survives_capture_cold_and_redraw_without_rng() {
 let source=generated("function producer(){local n=random(1,999999) return n}\nscene label start\n\"same [producer()]\"\n");let p=rvn_parser::parse_program(&source).unwrap();let mut e=engine(&source);e.locale=Some(raw_locale());let mut expected=RandomState::seeded(11);let value=expected.integer(1,999999).unwrap();assert_eq!(e.step_until_interaction().unwrap(),Some(Interaction::Dialogue{character:None,text:format!("Traduit {value}")}));assert_eq!(e.state.random,expected);
 let save=e.capture_save15(&p,None,12,"Original locale template key".into()).unwrap();let mut cold=engine(&source);cold.locale=Some(raw_locale());cold.restore_save15(&p,save).unwrap();assert_eq!(cold.displayed_interaction().unwrap(),Some(Interaction::Dialogue{character:None,text:format!("Traduit {value}")}));let before=serde_json::to_value(&cold.state).unwrap();cold.locale.as_mut().unwrap().set_language("en").unwrap();assert_eq!(cold.displayed_interaction().unwrap(),Some(Interaction::Dialogue{character:None,text:format!("same {value}")}));assert_eq!(serde_json::to_value(&cold.state).unwrap(),before);assert_eq!(cold.state.random,expected);
}
