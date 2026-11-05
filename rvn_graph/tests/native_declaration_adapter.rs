use rvn_graph::*;

fn project() -> SourceProject {
    let source=format!("// protected declaration context été\r\ninit {{\nset seed=7\n}}\n{}\nscene label start\n\"Existing scene\"\n",include_str!("fixtures/native_source_single.rvn"));
    let baseline=SourceProject::open(source.clone(),&[]).unwrap();
    let mut graphs=baseline.graphs().to_vec();
    let init=graphs.iter_mut().find(|g|g.kind==GraphKind::Init).unwrap();
    init.enable_native_frame_preparation().unwrap();
    let compiled=prepare_native_frame_graph(init).unwrap();
    let source=source.replace("init {\nset seed=7\n}",&compiled.source);
    SourceProject::open(source,&[]).unwrap()
}

#[test]
fn native_declaration_add_from_native_owner_is_real_init_and_cold_source_not_registry_only() {
    let mut p=project();let old=p.source().to_owned();let original_probe=p.graphs().iter().find(|g|matches!(&g.kind,GraphKind::Function{name}if name=="probe")).unwrap().clone();
    let mut graphs=p.graphs().to_vec();let owner=graphs.iter_mut().find(|g|g.graph_id==original_probe.graph_id).unwrap();
    owner.add_variable("added_native",ValueType::Int,PropertyValue::Int(42)).unwrap();
    p.apply_visual(&old,&graphs).unwrap();
    assert!(p.source().contains("protected declaration context été"));
    let parsed=rvn_parser::parse_program(p.source()).unwrap();
    let engine=rvn_core::Engine::from_program_with_debugger(parsed,rvn_core::TerminalRenderer,4).unwrap();
    assert_eq!(serde_json::to_value(engine.state.vars.get("added_native")).unwrap(),serde_json::to_value(Some(&rvn_parser::Value::Int(42))).unwrap());
    let cold=SourceProject::open(p.source(),&[]).unwrap();
    assert_eq!(cold.global_declaration_values(cold.graphs()).unwrap()["added_native"],GlobalDeclarationValue::Literal(PropertyValue::Int(42)));
    let probe=cold.graphs().iter().find(|g|g.graph_id==original_probe.graph_id).unwrap();
    assert_eq!(probe.inspector_evaluation,original_probe.inspector_evaluation);
    assert_eq!(probe.edges,original_probe.edges);
}

#[test]
fn native_declaration_remove_actual_unused_init_binding_keeps_other_scopes() {
    let mut p=project();let old=p.source().to_owned();let mut graphs=p.graphs().to_vec();
    let init=graphs.iter_mut().find(|g|g.kind==GraphKind::Init).unwrap();assert!(!init.inspector_evaluation.is_legacy());init.variables.remove("seed");
    p.apply_visual(&old,&graphs).unwrap();
    let cold=SourceProject::open(p.source(),&[]).unwrap();
    assert!(!cold.global_declaration_values(cold.graphs()).unwrap().contains_key("seed"));
    assert!(cold.graphs().iter().any(|g|matches!(&g.kind,GraphKind::Function{name}if name=="probe")&&!g.inspector_evaluation.is_legacy()));
}

#[test]
fn native_declaration_stale_source_refuses_without_mutating_project_or_graphs() {
    let mut p=project();let source=p.source().to_owned();let before=p.graphs().to_vec();let mut changed=before.clone();
    changed.iter_mut().find(|g|g.kind==GraphKind::Init).unwrap().add_variable("new_name",ValueType::Bool,PropertyValue::Bool(true)).unwrap();
    assert!(p.apply_visual(&(source.clone()+"// outside change\n"),&changed).is_err());
    assert_eq!(p.source(),source);assert_eq!(p.graphs(),before);
}

fn local_project(body:&str)->SourceProject {
    let prefix=format!("function local_case() {{\n{body}\n}}\n");
    let source=prefix.clone()+project().source();
    let p=SourceProject::open(source.clone(),&[]).unwrap();
    let mut graph=p.graphs().iter().find(|g|matches!(&g.kind,GraphKind::Function{name}if name=="local_case")).unwrap().clone();
    graph.enable_native_frame_preparation().unwrap();
    let native=prepare_native_frame_graph(&graph).unwrap();
    SourceProject::open(source.replacen(&prefix,&native.source,1),&[]).unwrap()
}

#[test]
fn native_local_query_uses_real_local_child_not_later_assignment() {
    let p=local_project("local score=2\nset score=9\nreturn score");
    let graph=p.graphs().iter().find(|g|matches!(&g.kind,GraphKind::Function{name}if name=="local_case")).unwrap();
    let declarations=graph.local_variable_initializers("score").unwrap();assert_eq!(declarations.len(),1);
    assert_eq!(declarations[0].literal_value,Some(PropertyValue::Int(2)));
    assert_eq!(graph.nodes[&declarations[0].node].kind,NodeKind::LocalVariable);
    assert_eq!(declarations[0].graph,graph.graph_id);
    let before=graph.clone();let projection=graph.variable_binding_projection("score").unwrap();
    assert!(projection.local_declarations.contains(&declarations[0].node));assert_eq!(*graph,before);
}

#[test]
fn native_local_query_retains_each_repeated_declaration_identity() {
    let p=local_project("local score=2\nlocal score=3\nreturn score");
    let graph=p.graphs().iter().find(|g|matches!(&g.kind,GraphKind::Function{name}if name=="local_case")).unwrap();
    let declarations=graph.local_variable_initializers("score").unwrap();assert_eq!(declarations.len(),2);
    assert_ne!(declarations[0].node,declarations[1].node);
    assert_eq!(declarations.iter().map(|d|d.literal_value.clone()).collect::<Vec<_>>(),vec![Some(PropertyValue::Int(2)),Some(PropertyValue::Int(3))]);
}

#[test]
fn native_local_type_readonly_proposal_preserves_authored_source_and_layout() {
    let p=local_project("local score=2\nreturn score");
    let graph=p.graphs().iter().find(|g|matches!(&g.kind,GraphKind::Function{name}if name=="local_case")).unwrap();
    let declaration=graph.local_variable_initializers("score").unwrap().remove(0);let before=graph.clone();let source=p.source().to_owned();
    let _proposal=graph.prepare_local_variable_type("score",&declaration,ValueType::String).unwrap();
    assert_eq!(*graph,before);assert_eq!(p.source(),source);
}

#[test]
fn native_declaration_dead_legacy_setter_does_not_suppress_actual_init_default() {
    let mut p=project();let old=p.source().to_owned();let mut graphs=p.graphs().to_vec();
    graphs.iter_mut().find(|g|matches!(&g.kind,GraphKind::Function{name}if name=="probe")).unwrap()
        .add_variable("dead_neighbour",ValueType::Int,PropertyValue::Int(42)).unwrap();
    let scene=graphs.iter_mut().find(|g|matches!(&g.kind,GraphKind::Label{name}if name=="start")).unwrap();assert!(scene.inspector_evaluation.is_legacy());
    scene.add_variable("dead_neighbour",ValueType::Int,PropertyValue::Int(42)).unwrap();
    let dead=scene.add_catalog_node(NodeKind::SetVariable,[400.,500.]).unwrap();
    scene.set_property(dead,"name",PropertyValue::String("dead_neighbour".into())).unwrap();
    scene.set_pin_default(dead,"value",PropertyValue::Int(99)).unwrap();
    let scene_id=scene.graph_id;
    p.apply_visual(&old,&graphs).unwrap();
    let engine=rvn_core::Engine::from_program_with_debugger(rvn_parser::parse_program(p.source()).unwrap(),rvn_core::TerminalRenderer,4).unwrap();
    assert_eq!(serde_json::to_value(engine.state.vars.get("dead_neighbour")).unwrap(),serde_json::to_value(Some(&rvn_parser::Value::Int(42))).unwrap());
    assert!(p.graphs().iter().find(|g|g.graph_id==scene_id).unwrap().nodes.contains_key(&dead));
}

#[test]
fn native_function_global_read_uses_complete_actual_candidate_context() {
    let source="init {set shared=7}\nfunction read_shared(){return shared}\nscene label start\nset result=read_shared()\n";
    let mut p=SourceProject::open(source,&[]).unwrap();let mut graphs=p.graphs().to_vec();
    let function=graphs.iter_mut().find(|g|matches!(&g.kind,GraphKind::Function{name}if name=="read_shared")).unwrap();
    function.enable_native_frame_preparation().unwrap();let id=function.graph_id;
    p.apply_visual(source,&graphs).unwrap();
    let cold=SourceProject::open(p.source(),p.graphs()).unwrap();
    let actual=cold.graphs().iter().find(|g|g.graph_id==id).unwrap();
    assert!(!actual.inspector_evaluation.is_legacy());assert_eq!(actual.variables["shared"].value_type,ValueType::Int);
    assert_eq!(actual.variable_scope("shared"),Some(VariableScope::Global));
    let mut engine=rvn_core::Engine::from_program_with_debugger(rvn_parser::parse_program(p.source()).unwrap(),rvn_core::TerminalRenderer,4).unwrap();
    engine.step_until_interaction().unwrap();assert_eq!(engine.state.vars.get("result"),Some(&rvn_parser::Value::Int(7)));
    let exported=transpile_project(p.graphs()).unwrap();
    assert!(exported.source.contains("flow.consume_v1"));
}

#[test]
fn native_imported_global_stays_readonly_with_real_frozen_source_origins() {
    let folder=std::env::temp_dir().join(format!("rvn-native-import0438-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir(&folder).unwrap();
    let library=folder.join("library.rvn");let library_source="init {set shared=41}\n";
    std::fs::write(&library,library_source).unwrap();
    let path=folder.join("main.rvn");let source="use \"library.rvn\"\nfunction read_shared(){return shared}\nscene label start\nset result=read_shared()\n";
    std::fs::write(&path,source).unwrap();let mut p=SourceProject::open_at(&path,source,&[]).unwrap();
    let mut graphs=p.graphs().to_vec();let function=graphs.iter_mut().find(|g|matches!(&g.kind,GraphKind::Function{name}if name=="read_shared")).unwrap();
    function.enable_native_frame_preparation().unwrap();let id=function.graph_id;
    p.apply_visual(source,&graphs).unwrap();assert!(p.is_imported_global("shared"));
    assert_eq!(std::fs::read_to_string(&library).unwrap(),library_source);
    assert!(!p.source().contains("set shared="));
    let cold=SourceProject::open_at(&path,p.source(),p.graphs()).unwrap();
    let actual=cold.graphs().iter().find(|g|g.graph_id==id).unwrap();assert_eq!(actual.variables["shared"].value_type,ValueType::Int);
    let program=cold.resolved_program().unwrap();let sites=native_source_sites(&program).unwrap();
    assert!(!sites.is_empty());assert!(sites.iter().all(|site|site.source==path.canonicalize().unwrap()));
    let mut engine=rvn_core::Engine::from_program_with_debugger(program,rvn_core::TerminalRenderer,4).unwrap();
    engine.step_until_interaction().unwrap();assert_eq!(engine.state.vars.get("result"),Some(&rvn_parser::Value::Int(41)));
}

#[test]
fn native_local_type_reconstruction_saves_and_cold_executes_real_default() {
    let p=local_project("local score=2\nreturn score");let before=p.graphs().to_vec();
    let graph=before.iter().find(|g|matches!(&g.kind,GraphKind::Function{name}if name=="local_case")).unwrap();
    let declaration=graph.local_variable_initializers("score").unwrap().remove(0);
    let proposal=graph.prepare_local_variable_type("score",&declaration,ValueType::String).unwrap();
    let next=p.reconstruct_local_variable_type(&proposal,graph,&before).unwrap();
    let mut edited=before.clone();*edited.iter_mut().find(|g|g.graph_id==graph.graph_id).unwrap()=next;
    let folder=std::env::temp_dir().join(format!("rvn-native-type0442-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir(&folder).unwrap();let path=folder.join("main.rvn");std::fs::write(&path,p.source()).unwrap();
    let mut workspace=SourceWorkspace::link(&folder,&path,p.graphs()).unwrap();workspace.save(&edited).unwrap();drop(workspace);
    let cold=SourceWorkspace::open(&folder).unwrap().unwrap();let actual=cold.project().graphs().iter().find(|g|g.graph_id==graph.graph_id).unwrap();
    // String reconstruction creates a genuine connected producer. The query
    // deliberately reports that as an expression, without evaluating it into
    // a literal. Check its true type/topology and its executed result below.
    assert_eq!(actual.local_variable_initializers("score").unwrap()[0].literal_value,None);
    assert_eq!(actual.variables["score"].value_type,ValueType::String);
    assert_eq!(actual.edges,edited.iter().find(|g|g.graph_id==graph.graph_id).unwrap().edges);
    assert_eq!(actual.nodes[&declaration.node].position,graph.nodes[&declaration.node].position);
    let source=format!("{}\nscene label run_changed\nset type_result=local_case()\n",cold.project().source());
    let mut engine=rvn_core::Engine::from_program_with_debugger(rvn_parser::parse_program(&source).unwrap(),rvn_core::TerminalRenderer,4).unwrap();
    while engine.step_until_interaction().unwrap().is_some(){engine.advance_dialogue().unwrap();}
    assert_eq!(engine.state.vars.get("type_result"),Some(&rvn_parser::Value::Str(String::new())));
}

#[test]
fn native_local_rename_updates_real_source_and_leaves_other_scope_unchanged() {
    let mut p=local_project("local score=2\nset score=9\nreturn score");let source=p.source().to_owned();let before=p.graphs().to_vec();let mut edited=before.clone();
    let graph=edited.iter_mut().find(|g|matches!(&g.kind,GraphKind::Function{name}if name=="local_case")).unwrap();let id=graph.graph_id;
    let node=graph.local_variable_initializers("score").unwrap()[0].node;
    graph.rename_variable("score","renamed").unwrap();
    p.apply_visual(&source,&edited).unwrap();let cold=SourceProject::open(p.source(),p.graphs()).unwrap();
    let actual=cold.graphs().iter().find(|g|g.graph_id==id).unwrap();
    assert_eq!(actual.local_variable_initializers("renamed").unwrap()[0].node,node);
    assert!(actual.local_variable_initializers("score").unwrap().is_empty());
    for graph in before.iter().filter(|g|g.graph_id!=id){assert_eq!(cold.graphs().iter().find(|g|g.graph_id==graph.graph_id),Some(graph));}
    let mut engine=rvn_core::Engine::from_program_with_debugger(rvn_parser::parse_program(&format!("{}\nscene label renamed_run\nset rename_result=local_case()\n",p.source())).unwrap(),rvn_core::TerminalRenderer,4).unwrap();
    while engine.step_until_interaction().unwrap().is_some(){engine.advance_dialogue().unwrap();}
    assert_eq!(engine.state.vars.get("rename_result"),Some(&rvn_parser::Value::Int(9)));
}
