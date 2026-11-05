//! Real data wire from an assignment whose execution occurs after its reader.
use rvn_graph::*;
#[path="support/dialogue_mount_0624.rs"] mod mount;
use rvn_core::{Engine,TerminalRenderer,Interaction};
use rvn_core::random::RandomState;
const SOURCE:&str=include_str!("fixtures/dialogue-text-0607/future-set-current-value.rvn");
fn generated_future_assignment()->(String,GraphDocument){
    let mut project=SourceProject::open(SOURCE,&[]).unwrap();let mut graphs=project.graphs().to_vec();
    let graph=graphs.iter_mut().find(|g|matches!(&g.kind,GraphKind::Label{name}if name=="start")).unwrap();
    let assignments:Vec<_>=graph.nodes.values().filter(|n|n.kind==NodeKind::SetVariable&&graph.variable_node_name(n.id).as_deref()==Some("message")).map(|n|n.id).collect();
    let [assignment]=assignments.as_slice() else{panic!("one real story assignment required")};
    let output=graph.pin_by_key(*assignment,"value_out").unwrap().id;
    let dialogues:Vec<_>=graph.nodes.values().filter(|n|n.kind==NodeKind::Dialogue).map(|n|n.id).collect();assert_eq!(dialogues.len(),2);
    for dialogue in dialogues{mount::replace_text_source(graph,dialogue,output,false);}
    graph.enable_native_frame_preparation().unwrap();let expected=graph.clone();
    project.apply_visual(SOURCE,&graphs).expect("strict source writer must preserve a real future SET data-output reference");
    let parsed=rvn_parser::parse_program(project.source()).unwrap();let restored=import_native_frame_script(&parsed.script).unwrap();
    let actual=restored.iter().find(|g|g.graph_id==expected.graph_id).unwrap();
    assert_eq!(actual.edges,expected.edges);assert_eq!(actual.nodes.keys().collect::<Vec<_>>(),expected.nodes.keys().collect::<Vec<_>>());assert_eq!(actual.pins.keys().collect::<Vec<_>>(),expected.pins.keys().collect::<Vec<_>>());
    (project.source().into(),expected)
}
#[test]
fn future_assignment_output_reads_current_variable_before_execution_without_replaying_rhs(){
    let (source,_)=generated_future_assignment();let parsed=rvn_parser::parse_program(&source).unwrap();
    let mut warm=Engine::from_program_with_debugger(parsed.clone(),TerminalRenderer,16).unwrap();warm.state.random=RandomState::seeded(11);
    let before=Some(Interaction::Dialogue{character:None,text:"before".into()});
    assert_eq!(warm.step_until_interaction().unwrap(),before);assert_eq!(warm.state.random,RandomState::seeded(11));
    let first=serde_json::to_value(&warm.state).unwrap();let save=warm.capture_save15(&parsed,None,1,"before actual SET execution".into()).unwrap();
    let mut cold=Engine::from_program_with_debugger(parsed.clone(),TerminalRenderer,16).unwrap();cold.restore_save15(&parsed,save).unwrap();assert_eq!(cold.displayed_interaction().unwrap(),before);assert_eq!(cold.step_until_interaction().unwrap(),before);
    cold.advance_dialogue().unwrap();let mut random=RandomState::seeded(11);random.integer(1,999).unwrap();
    assert_eq!(cold.step_until_interaction().unwrap(),Some(Interaction::Dialogue{character:None,text:"after".into()}));assert_eq!(cold.state.random,random);
    let latest=serde_json::to_value(&cold.state).unwrap();assert!(cold.rollback());assert_eq!(serde_json::to_value(&cold.state).unwrap(),latest);assert!(cold.rollback());assert_eq!(serde_json::to_value(&cold.state).unwrap(),first);assert_eq!(cold.displayed_interaction().unwrap(),before);assert_eq!(cold.state.random,RandomState::seeded(11));
}

#[test]
fn actual_assignment_output_breakpoint_cold_resumes_the_same_retained_source_port(){
    let (source,graph)=generated_future_assignment();
    let directory=std::path::PathBuf::from(std::env::var("RVN_DIALOGUE_VARIABLE_FIXTURES").unwrap());std::fs::create_dir_all(&directory).unwrap();
    let path=directory.join("future-assignment-debugger.rvn");std::fs::write(&path,&source).unwrap();
    let program=rvn_parser::parse_file_program_with_uses(&path).unwrap();
    let assignment=graph.nodes.values().find(|n|n.kind==NodeKind::SetVariable&&graph.variable_node_name(n.id).as_deref()==Some("message")).unwrap().id;
    let output=graph.pin_by_key(assignment,"value_out").unwrap().id;
    let mut warm=Engine::from_program_with_debugger(program.clone(),TerminalRenderer,16).unwrap();warm.state.random=RandomState::seeded(11);warm.enter_label("start").unwrap();
    let before=serde_json::to_value(&warm.state).unwrap();
    let mut offset=0;
    let breakpoint=loop{
        let page=warm.debug_site_page(offset).unwrap();
        if let Some(entry)=page.entries.iter().find(|entry|entry.origin.as_ref().and_then(|o|o.scheduled_step.as_ref()).and_then(|s|s.endpoint.as_ref()).is_some_and(|e|e.graph==graph.graph_id.get()&&e.node==assignment.get()&&e.ports.iter().any(|p|p.role=="value_out"&&p.pin==output.get()))){break entry.breakpoint.clone();}
        offset+=page.entries.len();assert!(offset<page.total,"actual data-output producer breakpoint required");
    };
    let mut session=warm.debug_begin(rvn_core::engine::DebugOperation::Statement,vec![breakpoint]).unwrap();
    assert_eq!(warm.debug_step(&mut session,rvn_core::editor_debugger::StepKind::Continue).unwrap().0,rvn_core::engine::DebugProgress::Stopped);
    assert_eq!(serde_json::to_value(&warm.state).unwrap(),before);
    let bytes=warm.capture_save15(&program,Some(&session),2,"actual future SET output capture paused".into()).unwrap().encode().unwrap();
    fn finish(engine:&mut Engine<TerminalRenderer>,session:&mut rvn_core::engine::DebugSession){for _ in 0..128{if engine.debug_step(session,rvn_core::editor_debugger::StepKind::Continue).unwrap().0==rvn_core::engine::DebugProgress::Complete{return;}}panic!("actual bound consumer did not finish");}
    finish(&mut warm,&mut session);
    let mut cold=Engine::from_program_with_debugger(program.clone(),TerminalRenderer,16).unwrap();
    let mut resumed=cold.restore_save15(&program,rvn_core::save::consumer_checkpoint::Save15::decode(&bytes).unwrap()).unwrap().unwrap();finish(&mut cold,&mut resumed);
    assert_eq!(serde_json::to_value(&cold.state).unwrap(),serde_json::to_value(&warm.state).unwrap());
    assert_eq!(cold.displayed_interaction().unwrap(),Some(Interaction::Dialogue{character:None,text:"before".into()}));
    assert_eq!(cold.state.random,RandomState::seeded(11));
    assert!(Engine::new(rvn_parser::parse(&source).unwrap(),TerminalRenderer,16).is_err(),"an AST alone must not authorize the dynamic source route");
}
