//! Proposed Graph-only lexical migration contracts. NOT_RUN.
//! Runtime owner/cold/renderer qualification stays in its real Core/Host suite.
use rvn_graph::*;
use std::{path::PathBuf,sync::atomic::{AtomicU64,Ordering}};

// import_script supplies a neutral Init view even when no Init is authored.
// Such a root has no executable consumer and no Native source identity.
// Keep that view untouched; opt-in every actual source scope as before.
fn enable_native_source_scopes(graphs:&mut [GraphDocument],authored:&rvn_parser::Script){
    let source_has_init=authored.iter().any(|statement|matches!(statement,rvn_parser::Statement::Init{..}));
    for graph in graphs{
        let source_less_init=!source_has_init && graph.kind==GraphKind::Init
            && graph.nodes.len()==1 && graph.edges.is_empty() && graph.characters.is_empty()
            && graph.nodes.values().all(|node|node.kind==NodeKind::Init);
        if source_less_init{
            assert!(graph.inspector_evaluation.is_legacy(),"never demote an actual Native document");
        }else{graph.enable_native_frame_preparation().unwrap();}
    }
}

fn fixture_root(name:&str)->PathBuf{
    static NEXT:AtomicU64=AtomicU64::new(0);
    let base=PathBuf::from(std::env::var("RVN_GRAPH_LEXICAL0812_FIXTURES").expect("isolated D output root required"));
    let path=base.join(format!("{name}-{}-{}",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)));
    std::fs::create_dir_all(&path).unwrap();path
}
fn check(context:&str,loop_kind:NodeKind){
    let (header,parameters,ending)=match context{
        "function"=>("function","","return seen"),
        "handler"=>("handler","event","set observed0812=seen"),
        "screen"=>("screen","marker","return component(\"root\",\"label\",{\"text\":marker},[])"),
        _=>unreachable!(),
    };
    let body=if loop_kind==NodeKind::ForEach{"for item in supplier0812(){set seen=item}"}else{"flow.for_loop(i,first0812(),last0812()){set seen=i}"};
    let raw=format!("function supplier0812(){{local draw=random(1,1) return [11,22,33]}}\nfunction first0812(){{local draw=random(1,1) return 0}}\nfunction last0812(){{local draw=random(1,1) return 2}}\ninit{{set observed0812=0}}\n{header} target0812({parameters}){{local seen=0 {body} {ending}}}\nscene label start\n\"Ready0812\"\n");
    let mut graphs=import_script(&rvn_parser::parse(&raw).unwrap()).unwrap();
    for graph in &mut graphs{graph.enable_native_frame_preparation().unwrap();}
    let ix=graphs.iter().position(|g|match &g.kind{
        GraphKind::Function{name}|GraphKind::Handler{name}|GraphKind::Screen{name}=>name=="target0812",_=>false,
    }).unwrap();
    let owner=graphs[ix].nodes.values().find(|n|n.kind==loop_kind).unwrap().id;
    let before=graphs[ix].to_pretty_json().unwrap();
    let migration=graphs[ix].prepare_native_loop_demand(owner).unwrap();
    assert_eq!(graphs[ix].to_pretty_json().unwrap(),before);
    let [legacy_read]=migration.legacy_named_reads.as_slice() else {panic!("one actual legacy loop value read is required")};
    let legacy_read=*legacy_read;
    assert_eq!(migration.graph.nodes[&legacy_read].kind,NodeKind::VariableGet);
    assert_eq!(migration.graph.variable_node_name(legacy_read).as_deref(),Some(if loop_kind==NodeKind::ForEach{"item"}else{"i"}));
    graphs[ix]=migration.graph;
    let g=&mut graphs[ix];
    let role=if loop_kind==NodeKind::ForEach{"array_element"}else{"index"};
    let output=g.pin_by_key(owner,role).unwrap().id;
    let setter=g.nodes.values().find(|n|n.kind==NodeKind::SetVariable&&g.variable_node_name(n.id).as_deref()==Some("seen")).unwrap().id;
    let input=g.pin_by_key(setter,"value").unwrap().id;
    let removed:Vec<_>=g.edges.values().filter(|e|e.input==input).map(|e|(e.id,g.pins[&e.output].node)).collect();
    assert_eq!(removed.len(),1);
    assert_eq!(removed[0].1,legacy_read);
    assert_eq!(g.pins[&input].default_value,None,"replacing a real GET must not conceal an old inline operand");
    assert_eq!(g.variables["seen"].value_type,if loop_kind==NodeKind::ForEach{ValueType::Any}else{ValueType::Int});
    for (edge,_)in &removed{g.edges.remove(edge);}
    g.connect(output,input).unwrap();
    for (_,node)in removed{
        let n=&g.nodes[&node];
        if matches!(n.kind,NodeKind::Literal|NodeKind::VariableGet)&&!n.pins.iter().any(|p|g.edges.values().any(|e|e.output==*p||e.input==*p)){g.remove_node(node).unwrap();}
    }
    assert!(!g.nodes.contains_key(&legacy_read));
    assert_eq!(g.pins[&input].default_value,None);
    let graph_id=g.graph_id;
    let expected_nodes=g.nodes.keys().copied().collect::<Vec<_>>();
    let expected_pins=g.pins.clone();let expected_edges=g.edges.clone();
    let compiled=transpile_project_with_origins(&graphs).expect("actual lexical Graph export, not manually authored consume_v3");
    assert!(compiled.source.contains("flow.consume_v3("));
    assert_eq!(rvn_parser::parse(&compiled.source).unwrap(),compiled.ast);
    let root=fixture_root(&format!("{context}-{loop_kind:?}"));let file=root.join("native.rvn");
    std::fs::write(&file,&compiled.source).unwrap();
    let program=rvn_parser::parse_file_program_with_uses(&file).unwrap();
    let rebuilt=import_native_frame_script(&program.script).unwrap();
    let g=rebuilt.iter().find(|g|g.graph_id==graph_id).unwrap();
    assert_eq!(g.nodes.keys().copied().collect::<Vec<_>>(),expected_nodes);
    assert_eq!(g.pins,expected_pins);assert_eq!(g.edges,expected_edges);
    assert_eq!(g.nodes[&owner].native_loop_contract,Some(NativeLoopContract::DemandV1));
    assert_eq!(transpile_project(&rebuilt).unwrap().ast,compiled.ast);
    let map=NativeDebugSourceMap::from_program(&program).unwrap();
    let canonical=file.canonicalize().unwrap();
    let endpoints=map.endpoint(&canonical,graph_id,owner,output);
    assert!(!endpoints.is_empty(),"the genuine output must retain its source endpoint");
    if loop_kind==NodeKind::ForEach{
        assert!(endpoints.iter().any(|o|matches!(&o.role,NativeOriginRole::Producer{native_loop_output:Some(role),..}if role=="array_element")));
    }
    let mut source=SourceProject::open_native_preparation_at(&file,compiled.source.clone(),&graphs).unwrap();
    let current=source.source().to_owned();let same=source.graphs().to_vec();
    source.apply_visual(&current,&same).expect("real source-owned no-op visual publication");
    assert_eq!(source.source(),current,"a no-op must preserve the exact source bytes");
    let again=rvn_parser::parse(source.source()).unwrap();assert_eq!(again,compiled.ast);
    assert_eq!(std::fs::read_to_string(&file).unwrap(),compiled.source,"in-memory source preparation does not write the file");
}
#[test]fn graph0812_function_array_element_has_real_source_roundtrip(){check("function",NodeKind::ForEach);}
#[test]fn graph0812_function_range_index_has_real_source_roundtrip(){check("function",NodeKind::ForLoopWithBreak);}
#[test]fn graph0812_handler_array_element_has_real_source_roundtrip(){check("handler",NodeKind::ForEach);}
#[test]fn graph0812_handler_range_index_has_real_source_roundtrip(){check("handler",NodeKind::ForLoopWithBreak);}
#[test]fn graph0812_screen_array_element_has_real_source_roundtrip(){check("screen",NodeKind::ForEach);}
#[test]fn graph0812_screen_range_index_has_real_source_roundtrip(){check("screen",NodeKind::ForLoopWithBreak);}


// Literal and ListLiteral are distinct editable source identities, even when
// both evaluate to an equal StringList. These cases begin with public Legacy
// import/materialization and opt in through the real Graph preparation API.
fn literal_project0882(shared: bool) -> (Vec<GraphDocument>, GraphId, NodeId) {
    let raw = r#"screen panel(marker){return component("root","text",{"text":marker},[])}
scene label start
ui.open("panel",["actual first"],false,1)
ui.open("panel",["actual second"],false,1)
return
"#;
    let authored = rvn_parser::parse(raw).unwrap();
    let mut graphs = import_script(&authored).unwrap();
    let graph = graphs.iter_mut().find(|g| matches!(&g.kind, GraphKind::Label{name} if name=="start")).unwrap();
    let opens: Vec<_> = graph.nodes.values().filter(|n| n.kind==NodeKind::UiOpen).map(|n| n.id).collect();
    assert_eq!(opens.len(), 2);
    let first_input = graph.pin_by_key(opens[0], "arguments").unwrap().id;
    let first_output = graph.edges.values().find(|e| e.input==first_input).unwrap().output;
    let producer = graph.pins[&first_output].node;
    assert_eq!(graph.nodes[&producer].kind, NodeKind::Literal);
    assert_eq!(graph.nodes[&producer].properties["value"], PropertyValue::StringList(vec!["actual first".into()]));
    if shared {
        let input = graph.pin_by_key(opens[1], "arguments").unwrap().id;
        let old = graph.edges.values().find(|e| e.input==input).unwrap().clone();
        let old_node = graph.pins[&old.output].node;
        graph.edges.remove(&old.id);
        graph.connect(first_output, input).unwrap();
        assert!(!graph.edges.values().any(|e| graph.pins[&e.input].node==old_node || graph.pins[&e.output].node==old_node));
        graph.remove_node(old_node).unwrap();
    }
    let graph_id = graph.graph_id;
    enable_native_source_scopes(&mut graphs, &authored);
    (graphs, graph_id, producer)
}
fn literal_roundtrip0882(shared: bool) {
    let (graphs, graph_id, producer) = literal_project0882(shared);
    let frozen = graphs.clone();
    let compiled = transpile_project_with_origins(&graphs).unwrap();
    assert_eq!(graphs, frozen, "the exporter must not rewrite the actual graph");
    assert_eq!(rvn_parser::parse(&compiled.source).unwrap(), compiled.ast);
    let rebuilt = import_native_frame_script(&compiled.ast).unwrap();
    let before = graphs.iter().find(|g| g.graph_id==graph_id).unwrap();
    let after = rebuilt.iter().find(|g| g.graph_id==graph_id).unwrap();
    assert_eq!(after.nodes.keys().copied().collect::<Vec<_>>(), before.nodes.keys().copied().collect::<Vec<_>>());
    assert_eq!(after.pins, before.pins, "all typed pins/defaults, including scalar UiOpen operands, must remain exact");
    assert_eq!(after.edges, before.edges);
    assert_eq!(after.nodes[&producer].kind, NodeKind::Literal);
    assert_eq!(after.nodes[&producer].properties, before.nodes[&producer].properties);
    let output = after.pin_by_key(producer, "value").unwrap().id;
    assert_eq!(after.edges.values().filter(|edge| edge.output==output).count(), if shared {2} else {1});
    assert_eq!(transpile_project(&rebuilt).unwrap().ast, compiled.ast);
}
#[test]
fn graph0882_materialized_string_list_literal_and_scalar_operands_preserve_all_pins() {
    literal_roundtrip0882(false);
}
#[test]
fn graph0882_shared_string_list_literal_preserves_one_real_producer_and_both_edges() {
    literal_roundtrip0882(true);
}
#[test]
fn graph0882_literal_identity_cannot_authorize_a_dynamic_list_expression() {
    let (graphs, _, _) = literal_project0882(false);
    let frozen = graphs.clone();
    let compiled = transpile_project_with_origins(&graphs).unwrap();
    let mut forged = compiled.ast.clone();
    let mut changed = 0;
    for statement in &mut forged {
        if let rvn_parser::Statement::ScheduledConsumer { steps, .. } = statement {
            for step in steps {
                if let rvn_parser::ConsumerStep::Capture { producer, value } = step {
                    if producer.contains(":literal|") {
                        if let rvn_parser::Expr::ListLit(items) = value {
                            *value = rvn_parser::Expr::Call { name: "list".into(), args: items.clone() };
                            changed += 1;
                        }
                    }
                }
            }
        }
    }
    assert_eq!(changed, 2, "only the two actual UiOpen Literal captures are altered");
    let error = import_native_frame_script(&forged).unwrap_err();
    assert!(error.contains("Native Literal requires its actual literal value"), "{error}");
    assert_eq!(graphs, frozen);
    assert_eq!(transpile_project(&graphs).unwrap().ast, compiled.ast);
}


// The original hidden zero is legitimate editor state when prior presentation
// exists. It never supplies a runtime value until an explicit disconnect.
fn presentation_project0882() -> (Vec<GraphDocument>, GraphId, NodeId, EdgeId) {
    let raw = "function hidden0882(){local seen=0 flow.for_loop(i,0,2){set seen=0} return seen}\nscene label start\nset result0882=hidden0882()\n\"Ready\"\nreturn\n";
    let authored = rvn_parser::parse(raw).unwrap();
    let mut graphs = import_script(&authored).unwrap();
    enable_native_source_scopes(&mut graphs, &authored);
    let index = graphs.iter().position(|g| matches!(&g.kind,GraphKind::Function{name}if name=="hidden0882")).unwrap();
    let owner = graphs[index].nodes.values().find(|n| n.kind==NodeKind::ForLoopWithBreak).unwrap().id;
    let migration = graphs[index].prepare_native_loop_demand(owner).unwrap();
    assert!(migration.legacy_named_reads.is_empty());
    graphs[index] = migration.graph;
    let graph = &mut graphs[index];
    let setter = graph.nodes.values().find(|n| n.kind==NodeKind::SetVariable && graph.variable_node_name(n.id).as_deref()==Some("seen")).unwrap().id;
    let input = graph.pin_by_key(setter,"value").unwrap().id;
    let output = graph.pin_by_key(owner,"index").unwrap().id;
    assert_eq!(graph.pins[&input].default_value,Some(PropertyValue::Int(0)));
    let edge = graph.connect(output,input).unwrap();
    assert_eq!(graph.pins[&input].default_value,Some(PropertyValue::Int(0)),"wire hides the authored zero without deleting it");
    {let id=graph.graph_id;(graphs,id,setter,edge)}
}
fn run_presented_program0882(program:rvn_parser::ParsedProgram,expected:i64) {
    let mut engine = rvn_core::Engine::from_program_with_debugger(program,rvn_core::TerminalRenderer,8).unwrap();
    engine.enter_label("start").unwrap();
    for _ in 0..32 {
        if engine.state.vars.contains_key("result0882"){break;}
        engine.step_silent().unwrap();
    }
    assert_eq!(engine.state.vars.get("result0882"),Some(&rvn_parser::Value::Int(expected)));
}
#[test]
fn graph0882_hidden_zero_survives_real_source_workspace_save_cold_and_explicit_disconnect() {
    let (graphs,id,setter,edge) = presentation_project0882();
    let before = graphs.iter().find(|g| g.graph_id==id).unwrap();
    let input = before.pin_by_key(setter,"value").unwrap().id;
    let compiled = transpile_project_with_origins(&graphs).unwrap();
    let fresh = import_native_frame_script(&compiled.ast).unwrap();
    assert_eq!(fresh.iter().find(|g|g.graph_id==id).unwrap().pins[&input].default_value,None,"source alone must not invent a hidden fallback");
    let restored = reimport_native_frame_script(&compiled.ast,&graphs).unwrap();
    let actual = restored.iter().find(|g|g.graph_id==id).unwrap();
    assert_eq!(actual.pins,before.pins);
    assert_eq!(actual.edges,before.edges);
    assert_eq!(actual.nodes.keys().collect::<Vec<_>>(),before.nodes.keys().collect::<Vec<_>>());
    assert_eq!(transpile_project(&restored).unwrap().ast,compiled.ast);
    let root = fixture_root("hidden-presentation0882");
    let file = root.join("main.rvn");
    std::fs::write(&file,&compiled.source).unwrap();
    let mut workspace = SourceWorkspace::link_native_preparation(&root,&file,&graphs).unwrap();
    let initial_program = workspace.freeze_native_program().unwrap();
    let linked_graphs=workspace.project().graphs().to_vec();
    workspace.save(&linked_graphs).unwrap();
    let source_bytes = std::fs::read(&file).unwrap();
    let sidecar_bytes = std::fs::read(root.join(".rvn-authoring.json")).unwrap();
    drop(workspace);
    let mut cold = SourceWorkspace::open(&root).unwrap().unwrap();
    assert!(cold.source_error.is_none());
    assert_eq!(cold.freeze_native_program().unwrap(),initial_program);
    let actual = cold.project().graphs().iter().find(|g|g.graph_id==id).unwrap();
    assert_eq!(actual.pins,before.pins);
    assert_eq!(actual.edges,before.edges);
    assert_eq!(std::fs::read(&file).unwrap(),source_bytes);
    assert_eq!(std::fs::read(root.join(".rvn-authoring.json")).unwrap(),sidecar_bytes);
    run_presented_program0882(cold.freeze_native_program().unwrap(),2);
    let mut disconnected = cold.project().graphs().to_vec();
    let graph = disconnected.iter_mut().find(|g|g.graph_id==id).unwrap();
    assert!(graph.remove_edge(edge).is_some());
    assert_eq!(graph.pins[&input].default_value,Some(PropertyValue::Int(0)));
    cold.save(&disconnected).unwrap();
    assert_ne!(std::fs::read(&file).unwrap(),source_bytes);
    let reopened = SourceWorkspace::open(&root).unwrap().unwrap();
    let actual = reopened.project().graphs().iter().find(|g|g.graph_id==id).unwrap();
    assert_eq!(actual.pins[&input].default_value,Some(PropertyValue::Int(0)));
    assert!(!actual.edges.contains_key(&edge));
    run_presented_program0882(reopened.freeze_native_program().unwrap(),0);
}
#[test]
fn graph0882_hidden_presentation_cannot_override_changed_owner_or_an_authored_unwired_value() {
    let (graphs,id,setter,edge) = presentation_project0882();
    let compiled = transpile_project_with_origins(&graphs).unwrap();
    let input = graphs.iter().find(|g|g.graph_id==id).unwrap().pin_by_key(setter,"value").unwrap().id;
    let mut foreign = graphs.clone();
    foreign.iter_mut().find(|g|g.graph_id==id).unwrap().nodes.get_mut(&setter).unwrap().properties.insert("name".into(),PropertyValue::String("different_editor_owner".into()));
    let refused_presentation = reimport_native_frame_script(&compiled.ast,&foreign).unwrap();
    let actual = refused_presentation.iter().find(|g|g.graph_id==id).unwrap();
    assert_eq!(actual.variable_node_name(setter).as_deref(),Some("seen"));
    assert_eq!(actual.pins[&input].default_value,None);
    assert_eq!(transpile_project(&refused_presentation).unwrap().ast,compiled.ast);
    let mut authored = graphs.clone();
    let graph = authored.iter_mut().find(|g|g.graph_id==id).unwrap();
    graph.remove_edge(edge).unwrap();
    graph.set_pin_default(setter,"value",PropertyValue::Int(7)).unwrap();
    let changed = transpile_project_with_origins(&authored).unwrap();
    let rebuilt = reimport_native_frame_script(&changed.ast,&graphs).unwrap();
    let actual = rebuilt.iter().find(|g|g.graph_id==id).unwrap();
    assert_eq!(actual.pins[&input].default_value,Some(PropertyValue::Int(7)),"the actual unwired source value wins over old editor fallback");
    assert!(!actual.edges.contains_key(&edge));
    assert_eq!(transpile_project(&rebuilt).unwrap().ast,changed.ast);
}
