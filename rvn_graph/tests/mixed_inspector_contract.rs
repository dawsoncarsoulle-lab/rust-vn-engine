use rvn_graph::*;
use rvn_parser::{Statement,ConsumerStep};
fn wire(g:&mut GraphDocument,a:NodeId,ak:&str,b:NodeId,bk:&str){g.connect(g.pin_by_key(a,ak).unwrap().id,g.pin_by_key(b,bk).unwrap().id).unwrap();}
fn root(g:&mut GraphDocument)->NodeId {g.add_catalog_node(match g.kind {GraphKind::Init=>NodeKind::Init,GraphKind::Label{..}=>NodeKind::Label,GraphKind::Function{..}=>NodeKind::FunctionEntry,GraphKind::Screen{..}=>NodeKind::ScreenEntry,_=>NodeKind::HandlerEntry},[0.,0.]).unwrap()}
fn exact(g:&GraphDocument)->TranspiledScriptWithOrigins {
 let c=prepare_native_frame_graph(g).unwrap();let imported=import_native_frame_script(&c.ast).unwrap();let r=imported.iter().find(|r|r.graph_id==g.graph_id).unwrap();
 assert_eq!(g.edges,r.edges);assert_eq!(g.nodes.keys().collect::<Vec<_>>(),r.nodes.keys().collect::<Vec<_>>());assert_eq!(g.pins.keys().collect::<Vec<_>>(),r.pins.keys().collect::<Vec<_>>());
 assert_eq!(prepare_native_frame_graph(r).unwrap().ast,c.ast);c
}
#[test]
fn editor_factory_migrates_only_the_new_inspector_and_preserves_historical_json_semantics(){
 let mut g=GraphDocument::new(GraphId::new(900),GraphKind::Function{name:"mixed".into()});root(&mut g);
 let old=g.add_catalog_node(NodeKind::GetStringTableIdAndKey,[100.,0.]).unwrap();
 // Simulate an actual pre-native persisted document, rather than assuming the
 // current editor factory still creates historical nodes.
 g.nodes.get_mut(&old).unwrap().inspector_contract=None;g.schema_version=10;g.inspector_evaluation=InspectorEvaluationContract::LegacyStateless;
 let old=g.to_pretty_json().unwrap();let mut cold=GraphDocument::from_json(&old).unwrap();
 let historical=cold.nodes.keys().last().copied().unwrap();let fresh=cold.add_catalog_node(NodeKind::GetStringTableIdAndKey,[300.,0.]).unwrap();
 assert_eq!(cold.schema_version,12);assert_eq!(cold.nodes[&historical].inspector_contract,Some(InspectorEvaluationContract::LegacyStateless));
 assert_eq!(cold.nodes[&fresh].inspector_contract,Some(InspectorEvaluationContract::NativeFrameV1));
 assert_eq!(GraphDocument::from_json(&cold.to_pretty_json().unwrap()).unwrap(),cold);
}
#[test]
fn mixed_legacy_ports_and_one_native_inspector_roundtrip_without_fake_nodes(){
 let mut g=GraphDocument::new(GraphId::new(902),GraphKind::Function{name:"mixed".into()});let r=root(&mut g);
 let legacy=g.add_catalog_node(NodeKind::GetStringTableIdAndKey,[100.,0.]).unwrap();g.nodes.get_mut(&legacy).unwrap().inspector_contract=Some(InspectorEvaluationContract::LegacyStateless);
 let native=g.add_catalog_node(NodeKind::GetStringTableIdAndKey,[200.,0.]).unwrap();
 let list=g.add_catalog_node(NodeKind::ListLiteral,[300.,0.]).unwrap();g.resize_value_inputs(list,6).unwrap();
 for (i,(node,key)) in [legacy,native].into_iter().flat_map(|node|["valid","table_id","key"].into_iter().map(move|key|(node,key))).enumerate(){wire(&mut g,node,key,list,&format!("item_{i}"));}
 let ret=g.add_catalog_node(NodeKind::FunctionReturn,[500.,0.]).unwrap();wire(&mut g,r,"exec_out",ret,"exec_in");wire(&mut g,list,"list",ret,"value");
 let c=exact(&g);let [Statement::Function{body,..}]=c.ast.as_slice()else{panic!()};let [Statement::ScheduledConsumer{steps,..}]=body.as_slice()else{panic!()};
 assert_eq!(steps.iter().filter(|step|matches!(step,ConsumerStep::InspectTextTable{..})).count(),1);
 assert_eq!(steps.iter().filter(|step|matches!(step,ConsumerStep::Capture{producer,..}if producer.starts_with("p:"))).count(),3);
 assert!(c.source.contains("__rvn_text_table_valid("));
}
#[test]
fn init_and_narrative_label_set_consumers_have_actual_root_identities(){
 for kind in [GraphKind::Init,GraphKind::Label{name:"start".into()}] {
  let mut g=GraphDocument::new(GraphId::new(905),kind);let r=root(&mut g);
  let inspect=g.add_catalog_node(NodeKind::GetStringTableIdAndKey,[100.,0.]).unwrap();let set=g.add_catalog_node(NodeKind::SetVariable,[300.,0.]).unwrap();
  g.variables.insert("found".into(),VariableDefinition{name:"found".into(),value_type:ValueType::Bool,default_value:PropertyValue::Bool(false)});
  g.set_property(set,"name",PropertyValue::String("found".into())).unwrap();wire(&mut g,r,"exec_out",set,"exec_in");wire(&mut g,inspect,"valid",set,"value");
  exact(&g);
 }
}

#[test]
fn calculation_screen_and_handler_source_roots_are_preserved_without_claiming_host_owners(){
 for kind in [GraphKind::Screen{name:"screen_probe".into()},GraphKind::Handler{name:"handler_probe".into()}] {
  let mut g=GraphDocument::new(GraphId::new(960),kind);let r=root(&mut g);let inspect=g.add_catalog_node(NodeKind::GetStringTableIdAndKey,[100.,0.]).unwrap();let ret=g.add_catalog_node(NodeKind::FunctionReturn,[300.,0.]).unwrap();
  wire(&mut g,r,"exec_out",ret,"exec_in");wire(&mut g,inspect,"valid",ret,"value");exact(&g);
 }
}

#[test]
fn version_one_strict_admission_and_version_two_loop_admission_preserve_existing_semantics(){
 assert!(rvn_parser::parse("flow.consume_v1(\"s\",\"c\"){accessibility.stop()}\n").is_err());
 assert!(rvn_parser::parse("flow.consume_v1(\"s\",\"c\"){}\n").is_err());
 assert!(rvn_parser::parse("flow.consume_v2(\"s\",\"c\"){while false {set x=1}}\n").is_ok());
 assert!(rvn_parser::parse("flow.consume_v2(\"s\",\"c\"){capture \"p\" = 1}\n").is_err());
 assert!(rvn_parser::parse("flow.consume_v2(\"s\",\"c\"){accessibility.stop()}\n").is_ok());
 assert!(rvn_parser::parse("flow.consume_v1(\"s\",\"c\"){set x=1}\n").is_ok());
}
#[test]
fn branch_and_command_lowering_preserve_real_branch_end_nodes_and_scope(){
 let mut g=GraphDocument::new(GraphId::new(909),GraphKind::Label{name:"start".into()});let r=root(&mut g);
 let inspect=g.add_catalog_node(NodeKind::GetStringTableIdAndKey,[100.,0.]).unwrap();let branch=g.add_catalog_node(NodeKind::If,[300.,0.]).unwrap();wire(&mut g,r,"exec_out",branch,"exec_in");wire(&mut g,inspect,"valid",branch,"condition");
 for port in ["then","else"] {let command=g.add_catalog_node(NodeKind::AccessibilitySpeak,[500.,0.]).unwrap();wire(&mut g,branch,port,command,"exec_in");wire(&mut g,inspect,"key",command,"text");let end=g.add_branch_end(branch,[700.,0.]).unwrap();wire(&mut g,command,"exec_out",end,"exec_in");}
 let ret=g.add_catalog_node(NodeKind::Return,[900.,0.]).unwrap();wire(&mut g,branch,"completed",ret,"exec_in");
 let c=exact(&g);assert!(c.source.contains("accessibility.speak("));assert!(c.source.contains("if __rvn_terminal_v1("));
 // Core0359 now admits this body. Source producer breakpoint admission is
 // verified separately through actual ParsedProgram, never this AST-only API.
 let mut engine=rvn_core::Engine::new(c.ast,rvn_core::TerminalRenderer,16).unwrap();
 engine.enter_label("start").unwrap();engine.preview_step_statement().unwrap();
}

#[test]
fn mixed_reuse_retains_only_native_false_outputs_in_the_same_real_function(){
 let source=r#"function probe(){local text=__rvn_string_table_text_name(__rvn_literal_name("asset:Table"),"key")
 local before=[__rvn_text_table_valid(text),__rvn_text_table_id(text),__rvn_text_table_key(text)]
 set text=__rvn_invariant_text("literal")
 return [__rvn_text_table_valid(text),__rvn_text_table_id(text),__rvn_text_table_key(text),before]}"#;
 let mut g=import_script(&rvn_parser::parse(source).unwrap()).unwrap().into_iter().find(|g|matches!(g.kind,GraphKind::Function{..})).unwrap();
 let legacy=g.nodes.values().find(|n|n.kind==NodeKind::GetStringTableIdAndKey).unwrap().id;
 let text=g.pin_by_key(legacy,"text").unwrap().id;let input=g.edges.values().find(|edge|edge.input==text).unwrap().output;
 let native=g.add_catalog_node(NodeKind::GetStringTableIdAndKey,[400.,0.]).unwrap();let ntext=g.pin_by_key(native,"text").unwrap().id;g.connect(input,ntext).unwrap();
 let lists:Vec<_>=g.nodes.values().filter(|n|n.kind==NodeKind::ListLiteral).map(|n|n.id).collect();
 for list in lists {let old=g.nodes[&list].pins.iter().filter(|id|g.pins[id].key.starts_with("item_")).count();g.resize_value_inputs(list,old+3).unwrap();for (i,key) in ["valid","table_id","key"].into_iter().enumerate(){wire(&mut g,native,key,list,&format!("item_{}",old+i));}}
 let c=exact(&g);
 let library=rvn_core::eval::FunctionLibrary::from_script(&c.ast).unwrap();let mut rng=rvn_core::random::RandomState::seeded(9);let mut frame=rvn_core::consumer_schedule::ConsumerFrame::default();
 let result=library.execute_in_consumer_frame(&rvn_parser::parse("set result=probe()").unwrap(),&std::collections::HashMap::new(),&mut rng,&mut frame).unwrap();
 let rvn_parser::Value::List(values)=&result["result"]else{panic!()};
 assert_eq!(values[0],rvn_parser::Value::Bool(false));assert_eq!(values[1],rvn_parser::Value::Name(Default::default()));assert_eq!(values[2],rvn_parser::Value::Str(String::new()));
 assert_eq!(values[4],rvn_parser::Value::Bool(false));assert_eq!(values[5],rvn_parser::Value::Name(rvn_parser::Name::new("asset:Table").unwrap()));assert_eq!(values[6],rvn_parser::Value::Str("key".into()));
}

#[test]
fn source_writer_cold_binding_and_collision_refusal_cover_real_label_branch_commands(){
 let root_dir=std::path::PathBuf::from(std::env::var("RVN_MIXED_TEST_DIR").expect("private fixture output required")).join(format!("writer-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));std::fs::create_dir_all(&root_dir).unwrap();
 let mut g=GraphDocument::new(GraphId::new(950),GraphKind::Label{name:"start".into()});let r=root(&mut g);let inspect=g.add_catalog_node(NodeKind::GetStringTableIdAndKey,[100.,0.]).unwrap();
 let branch=g.add_catalog_node(NodeKind::If,[300.,0.]).unwrap();wire(&mut g,r,"exec_out",branch,"exec_in");wire(&mut g,inspect,"valid",branch,"condition");
 let speak=g.add_catalog_node(NodeKind::AccessibilitySpeak,[500.,0.]).unwrap();wire(&mut g,branch,"then",speak,"exec_in");wire(&mut g,inspect,"key",speak,"text");let end=g.add_branch_end(branch,[700.,0.]).unwrap();wire(&mut g,speak,"exec_out",end,"exec_in");
 let c=exact(&g);let main=root_dir.join("main.rvn");std::fs::write(&main,format!("// source trivia retained\n{}",c.source)).unwrap();
 let mut workspace=SourceWorkspace::link_native_preparation(&root_dir,&main,&[]).unwrap();let frozen=workspace.freeze_native_program().unwrap();let sites=native_source_sites(&frozen).unwrap();assert_eq!(sites.len(),3);assert_eq!(sites[0].definition_route,vec![0]);
 let graphs=workspace.project().graphs().to_vec();workspace.save(&graphs).unwrap();let bytes=std::fs::read(&main).unwrap();assert!(String::from_utf8(bytes.clone()).unwrap().contains("source trivia retained"));
 let cold=SourceWorkspace::open_native_preparation(&root_dir).unwrap().unwrap();let frozen=cold.freeze_native_program().unwrap();assert_eq!(native_source_sites(&frozen).unwrap().len(),3);
 let mut collision=graphs.clone();let graph=collision.iter_mut().find(|g|g.graph_id==GraphId::new(950)).unwrap();graph.nodes.get_mut(&inspect).unwrap().inspector_contract=None;
 assert!(workspace.save(&collision).is_err());assert_eq!(std::fs::read(&main).unwrap(),bytes);
 // Source owner/kind collisions are rejected before the original graph/cache
 // or source bytes can be published.
 let forged=c.source.replace("n1:label|","n1:function_entry|");assert!(import_native_frame_script(&rvn_parser::parse(&forged).unwrap()).is_err());
}
