use rvn_graph::*;
fn graph() -> GraphDocument {
 let p=rvn_parser::parse_program("scene label start\ntypewriter.speed(20)\n\"boundary\"\n").unwrap();
 let mut g=import_script(&p.script).unwrap().into_iter().find(|g|matches!(g.kind,GraphKind::Label{..})).unwrap();g.enable_native_frame_preparation().unwrap();
 let command=g.nodes.values().find(|n|n.kind==NodeKind::TypewriterSpeed).unwrap().id;let input=g.pin_by_key(command,"speed").unwrap().id;
 let old=g.edges.values().find(|e|e.input==input).unwrap().clone();g.remove_node(g.pins[&old.output].node);
 let random=g.add_catalog_node(NodeKind::FunctionCall,[400.,200.]).unwrap();g.set_property(random,"function",PropertyValue::String("random".into())).unwrap();g.resize_value_inputs(random,2).unwrap();
 g.set_pin_default(random,"item_0",PropertyValue::Int(1)).unwrap();g.set_pin_default(random,"item_1",PropertyValue::Int(9)).unwrap();
 g.connect(g.pin_by_key(random,"result").unwrap().id,input).unwrap();g
}
fn exact(g:&GraphDocument)->TranspiledScriptWithOrigins {
 let before=g.clone();let c=prepare_native_frame_graph(g).unwrap();assert_eq!(*g,before);
 let actual=import_native_frame_script(&c.ast).unwrap().into_iter().find(|x|x.graph_id==g.graph_id).unwrap();
 assert_eq!(actual.edges,g.edges);assert_eq!(actual.nodes.keys().collect::<Vec<_>>(),g.nodes.keys().collect::<Vec<_>>());assert_eq!(actual.pins.keys().collect::<Vec<_>>(),g.pins.keys().collect::<Vec<_>>());c
}
#[test] fn speed0993_real_random_integer_gather_roundtrips_all_ids_once() {
 let g=graph();let c=exact(&g);assert!(c.source.contains("typewriter.speed(__rvn_captured_v1("));
 let captures=c.source.lines().filter(|s|s.trim_start().starts_with("capture ")&&s.contains("random(")).count();assert_eq!(captures,1);
 let parsed=rvn_parser::parse_program(&c.source).unwrap();assert_eq!(parsed.script,c.ast);
}
#[test] fn speed0993_dynamic_reroute_preserves_integer_producer_and_edges() {
 let mut g=graph();let command=g.nodes.values().find(|n|n.kind==NodeKind::TypewriterSpeed).unwrap().id;let input=g.pin_by_key(command,"speed").unwrap().id;
 let edge=g.edges.values().find(|e|e.input==input).unwrap().clone();let reroute=g.add_catalog_node(NodeKind::Reroute,[400.,200.]).unwrap();g.set_reroute_type(reroute,ValueType::Int).unwrap();g.remove_edge(edge.id);g.connect(edge.output,g.pin_by_key(reroute,"value").unwrap().id).unwrap();g.connect(g.pin_by_key(reroute,"value_out").unwrap().id,input).unwrap();exact(&g);
}
#[test] fn speed0993_dynamic_input_refuses_static_suffix_and_foreign_consumer_kind() {
 let g=graph();let c=exact(&g);let command=g.nodes.values().find(|n|n.kind==NodeKind::TypewriterSpeed).unwrap().id;let input=g.pin_by_key(command,"speed").unwrap().id;let edge=g.edges.values().find(|e|e.input==input).unwrap();
 let line=c.source.lines().find(|s|s.contains(&format!("n{command}:typewriter_speed|"))).unwrap();assert!(!line.contains('~'));
 for source in [c.source.replace(&format!("speed={input}@{}",edge.id),&format!("speed={input}@{}~{}",edge.id,edge.output)),c.source.replace(":typewriter_speed|",":typewriter_set|")] {
  assert_ne!(source,c.source);let p=rvn_parser::parse_program(&source).unwrap();assert!(rvn_parser::scheduled_source::scheduled_step_sources(&p).is_err());assert!(import_native_frame_script(&p.script).is_err());
 }
}
#[test] fn speed0993_source_project_reopen_keeps_dynamic_native_source_and_real_ids() {
 let g=graph();let c=exact(&g);let mut project=SourceProject::open(&c.source,&[g.clone()]).unwrap();let before=project.source().to_owned();let mut docs=project.graphs().to_vec();let actual=docs.iter_mut().find(|x|x.graph_id==g.graph_id).unwrap();let command=actual.nodes.values().find(|n|n.kind==NodeKind::TypewriterSpeed).unwrap().id;actual.nodes.get_mut(&command).unwrap().position=[913.,221.];let expected=actual.clone();project.apply_visual(&before,&docs).unwrap();let reopened=SourceProject::open(project.source(),project.graphs()).unwrap();let actual=reopened.graphs().iter().find(|x|x.graph_id==g.graph_id).unwrap();assert_eq!(actual.edges,expected.edges);assert_eq!(actual.inspector_evaluation,expected.inspector_evaluation);assert_eq!(actual.nodes[&command].position,[913.,221.]);
}

#[test]
fn speed0996_legacy_dynamic_integer_remains_refused_without_explicit_optin() {
 let p=rvn_parser::parse_program("scene label start\ntypewriter.speed(random(1,9))\n\"boundary\"\n").unwrap();assert!(import_script(&p.script).is_err());
 let mut g=graph();g.inspector_evaluation=InspectorEvaluationContract::LegacyStateless;g.schema_version=GRAPH_NAME_TEXT_SCHEMA_VERSION;assert!(transpile(&g).is_err());
}
#[test]
fn speed0996_inline_native_negation_keeps_its_actual_unconnected_integer_operand() {
 let mut g=graph();let command=g.nodes.values().find(|n|n.kind==NodeKind::TypewriterSpeed).unwrap().id;let input=g.pin_by_key(command,"speed").unwrap().id;
 let old=g.edges.values().find(|e|e.input==input).unwrap().clone();g.remove_node(g.pins[&old.output].node);
 let negate=g.add_catalog_node(NodeKind::MathNegate,[400.,200.]).unwrap();g.set_pin_default(negate,"value",PropertyValue::Int(4)).unwrap();g.connect(g.pin_by_key(negate,"result").unwrap().id,input).unwrap();
 let compiled=exact(&g);let actual=import_native_frame_script(&compiled.ast).unwrap().into_iter().find(|x|x.graph_id==g.graph_id).unwrap();
 assert_eq!(actual.pin_by_key(negate,"value").unwrap().default_value,Some(PropertyValue::Int(4)));
 assert!(!actual.edges.values().any(|e|e.input==actual.pin_by_key(negate,"value").unwrap().id));
}
