use rvn_graph::*;
fn graph(dynamic:bool)->GraphDocument {
 let source=if dynamic {"scene label start\nlocal level=0.25\nmusic.volume(level)\n\"boundary\"\n"}else{"scene label start\nlocal level=0.25\nmusic.volume(\"0.25\")\n\"boundary\"\n"};
 let parsed=rvn_parser::parse_program(source).unwrap();let mut graphs=import_script(&parsed.script).unwrap();
 let mut graph=graphs.remove(graphs.iter().position(|g|matches!(&g.kind,GraphKind::Label{name}if name=="start")).unwrap());
 graph.enable_native_frame_preparation().unwrap();let consumer=graph.nodes.values().find(|n|n.kind==NodeKind::MusicVolume).unwrap().id;
 let input=graph.pin_by_key(consumer,"level").unwrap().id;let edge=graph.edges.values().find(|edge|edge.input==input).unwrap();
 assert_eq!(graph.effective_pin_type(edge.output),Some(ValueType::Float));assert_eq!(graph.effective_pin_type(input),Some(ValueType::Float));graph
}
#[test]
fn real_float_variable_volume_graph_must_prepare_without_losing_actual_wire_identity(){
 let graph=graph(true);let before=graph.clone();
 let compiled=prepare_native_frame_graph(&graph).expect("A real declared Float VariableGet wired to the Float level input must be representable");
 assert_eq!(graph,before);let graphs=import_native_frame_script(&compiled.ast).unwrap();let actual=graphs.iter().find(|g|g.graph_id==graph.graph_id).unwrap();
 assert_eq!(actual.edges,graph.edges);assert_eq!(actual.nodes.keys().collect::<Vec<_>>(),graph.nodes.keys().collect::<Vec<_>>());assert_eq!(actual.pins.keys().collect::<Vec<_>>(),graph.pins.keys().collect::<Vec<_>>());
}
#[test]
fn real_constant_volume_graph_keeps_existing_preparation_contract(){
 let graph=graph(false);let compiled=prepare_native_frame_graph(&graph).unwrap();
 let program=rvn_parser::parse_program(&compiled.source).unwrap();assert_eq!(program.script,compiled.ast);
}

#[test]fn dynamic_level_uses_real_capture_without_static_suffix(){
 let g=graph(true);let compiled=prepare_native_frame_graph(&g).unwrap();
 let command=g.nodes.values().find(|n|n.kind==NodeKind::MusicVolume).unwrap().id;
 let line=compiled.source.lines().find(|line|line.contains(&format!("n{command}:music_volume|"))).unwrap();
 assert!(!line.contains('~'),"{line}");assert!(compiled.source.contains("music.volume(__rvn_captured_v1("));
 let p=rvn_parser::parse_program(&compiled.source).unwrap();rvn_parser::scheduled_source::scheduled_step_sources(&p).unwrap();
}
#[test]fn real_dynamic_reroute_keeps_all_node_pin_and_edge_ids(){
 let mut g=graph(true);let command=g.nodes.values().find(|n|n.kind==NodeKind::MusicVolume).unwrap().id;let input=g.pin_by_key(command,"level").unwrap().id;
 let edge=g.edges.values().find(|e|e.input==input).unwrap().clone();let reroute=g.add_catalog_node(NodeKind::Reroute,[400.,200.]).unwrap();g.set_reroute_type(reroute,ValueType::Float).unwrap();let middle=g.pin_by_key(reroute,"value").unwrap().id;let output=g.pin_by_key(reroute,"value_out").unwrap().id;
 g.remove_edge(edge.id);g.connect(edge.output,middle).unwrap();g.connect(output,input).unwrap();
 let compiled=prepare_native_frame_graph(&g).unwrap();let restored=import_native_frame_script(&compiled.ast).unwrap();let actual=restored.iter().find(|x|x.graph_id==g.graph_id).unwrap();assert_eq!(actual.edges,g.edges);assert_eq!(actual.pins.keys().collect::<Vec<_>>(),g.pins.keys().collect::<Vec<_>>());
}
#[test]fn dynamic_value_cannot_borrow_static_constant_source_suffix(){
 let g=graph(true);let compiled=prepare_native_frame_graph(&g).unwrap();let consumer=g.nodes.values().find(|n|n.kind==NodeKind::MusicVolume).unwrap().id;let input=g.pin_by_key(consumer,"level").unwrap().id;let edge=g.edges.values().find(|e|e.input==input).unwrap();
 let bad=compiled.source.replace(&format!("level={input}@{}",edge.id),&format!("level={input}@{}~{}",edge.id,edge.output));assert_ne!(bad,compiled.source);let program=rvn_parser::parse_program(&bad).unwrap();assert!(rvn_parser::scheduled_source::scheduled_step_sources(&program).is_err());assert!(import_native_frame_script(&program.script).is_err());
}

#[test]fn typed_arithmetic_producer_is_preserved_without_constant_folding(){
 for expression in ["level+0.125","level*2.0"] {
  let p=rvn_parser::parse_program(&format!("scene label start\nlocal level=0.25\nmusic.volume({expression})\n\"boundary\"\n")).unwrap();let mut graphs=import_script(&p.script).unwrap();let g=graphs.iter_mut().find(|g|matches!(&g.kind,GraphKind::Label{name}if name=="start")).unwrap();g.enable_native_frame_preparation().unwrap();
  let original=g.clone();let compiled=prepare_native_frame_graph(g).unwrap();let restored=import_native_frame_script(&compiled.ast).unwrap();let actual=restored.iter().find(|g|g.graph_id==original.graph_id).unwrap();assert_eq!(actual.edges,original.edges);assert_eq!(actual.nodes.keys().collect::<Vec<_>>(),original.nodes.keys().collect::<Vec<_>>());assert!(compiled.source.contains("music.volume(__rvn_captured_v1("));
 }
}
#[test]fn dynamic_command_cannot_claim_another_catalogue_consumer_kind(){
 let g=graph(true);let compiled=prepare_native_frame_graph(&g).unwrap();let bad=compiled.source.replace(":music_volume|",":sfx_stop|");assert_ne!(bad,compiled.source);let program=rvn_parser::parse_program(&bad).unwrap();assert!(rvn_parser::scheduled_source::scheduled_step_sources(&program).is_err());assert!(rvn_core::Engine::from_program(program,rvn_core::TerminalRenderer,8).is_err());
}
