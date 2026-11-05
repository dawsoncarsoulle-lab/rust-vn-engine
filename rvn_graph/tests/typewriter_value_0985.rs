use rvn_graph::*;
fn graph() -> GraphDocument {
 // Import a valid Legacy source, then author the real dynamic wiring after
 // explicit opt-in. A raw dynamic Typewriter is not a Legacy Graph contract.
 let p=rvn_parser::parse_program("scene label start\nlocal enabled=false\ntypewriter=true\n\"boundary\"\n").unwrap();
 let mut g=import_script(&p.script).unwrap().into_iter().find(|g|matches!(g.kind,GraphKind::Label{..})).unwrap();
 g.enable_native_frame_preparation().unwrap();
 let command=g.nodes.values().find(|n|n.kind==NodeKind::TypewriterSet).unwrap().id;
 let input=g.pin_by_key(command,"enabled").unwrap().id;
 let old=g.edges.values().find(|e|e.input==input).unwrap().clone();let literal=g.pins[&old.output].node;
 g.remove_node(literal);
 let operand=g.nodes.values().find(|n|n.kind==NodeKind::LocalVariable).unwrap().id;
 let not=g.add_catalog_node(NodeKind::LogicNot,[400.,200.]).unwrap();
 g.connect(g.pin_by_key(operand,"value_out").unwrap().id,g.pin_by_key(not,"value").unwrap().id).unwrap();
 g.connect(g.pin_by_key(not,"result").unwrap().id,input).unwrap();
 g.refresh_variable_usage_bindings().unwrap();g
}
#[test] fn typewriter0985_real_not_bool_gather_roundtrips_all_actual_ids_once() {
 let g=graph();let before=g.clone();let c=prepare_native_frame_graph(&g).unwrap();assert_eq!(g,before);
 assert!(c.source.contains("typewriter = __rvn_captured_v1("));
 let actual=import_native_frame_script(&c.ast).unwrap().into_iter().find(|x|x.graph_id==g.graph_id).unwrap();
 assert_eq!(actual.edges,g.edges);assert_eq!(actual.nodes.keys().collect::<Vec<_>>(),g.nodes.keys().collect::<Vec<_>>());assert_eq!(actual.pins.keys().collect::<Vec<_>>(),g.pins.keys().collect::<Vec<_>>());
 let not=g.nodes.values().find(|n|n.kind==NodeKind::LogicNot).unwrap().id;
 assert_eq!(c.source.lines().filter(|line|line.trim_start().starts_with("capture ")&&line.contains(&format!("n{not}:logic_not|"))).count(),1);
}
#[test] fn typewriter0985_dynamic_reroute_preserves_real_boolean_source_and_edges() {
 let mut g=graph();let cmd=g.nodes.values().find(|n|n.kind==NodeKind::TypewriterSet).unwrap().id;let input=g.pin_by_key(cmd,"enabled").unwrap().id;
 let edge=g.edges.values().find(|e|e.input==input).unwrap().clone();let reroute=g.add_catalog_node(NodeKind::Reroute,[400.,200.]).unwrap();g.set_reroute_type(reroute,ValueType::Bool).unwrap();
 g.remove_edge(edge.id);g.connect(edge.output,g.pin_by_key(reroute,"value").unwrap().id).unwrap();g.connect(g.pin_by_key(reroute,"value_out").unwrap().id,input).unwrap();
 let c=prepare_native_frame_graph(&g).unwrap();let actual=import_native_frame_script(&c.ast).unwrap().into_iter().find(|x|x.graph_id==g.graph_id).unwrap();assert_eq!(actual.edges,g.edges);assert_eq!(actual.pins.keys().collect::<Vec<_>>(),g.pins.keys().collect::<Vec<_>>());
}
#[test] fn typewriter0985_dynamic_boolean_rejects_static_suffix_and_foreign_consumer_kind() {
 let g=graph();let c=prepare_native_frame_graph(&g).unwrap();let cmd=g.nodes.values().find(|n|n.kind==NodeKind::TypewriterSet).unwrap().id;let input=g.pin_by_key(cmd,"enabled").unwrap().id;let edge=g.edges.values().find(|e|e.input==input).unwrap();
 let line=c.source.lines().find(|line|line.contains(&format!("n{cmd}:typewriter_set|"))).unwrap();assert!(!line.contains('~'));
 for source in [c.source.replace(&format!("enabled={input}@{}",edge.id),&format!("enabled={input}@{}~{}",edge.id,edge.output)),c.source.replace(":typewriter_set|",":typewriter_speed|")] {
  assert_ne!(source,c.source);let p=rvn_parser::parse_program(&source).unwrap();assert!(rvn_parser::scheduled_source::scheduled_step_sources(&p).is_err());assert!(import_native_frame_script(&p.script).is_err());
 }
}
#[test] fn typewriter0985_real_source_project_save_projection_keeps_native_contract() {
 let g=graph();let c=prepare_native_frame_graph(&g).unwrap();let mut project=SourceProject::open(&c.source,&[g.clone()]).unwrap();let before=project.source().to_owned();
 let mut docs=project.graphs().to_vec();let g=docs.iter_mut().find(|x|x.graph_id==g.graph_id).unwrap();let not=g.nodes.values().find(|n|n.kind==NodeKind::LogicNot).unwrap().id;g.nodes.get_mut(&not).unwrap().position=[987.,123.];
 let expected=g.clone();project.apply_visual(&before,&docs).unwrap();let reopened=SourceProject::open(project.source(),project.graphs()).unwrap();let actual=reopened.graphs().iter().find(|x|x.graph_id==expected.graph_id).unwrap();assert_eq!(actual.edges,expected.edges);assert_eq!(actual.inspector_evaluation,expected.inspector_evaluation);
}

#[test]
fn typewriter0996_inline_native_operators_keep_real_ids_without_inventing_literal_nodes() {
 for kind in [NodeKind::LogicNot,NodeKind::LogicAnd,NodeKind::LogicOr,NodeKind::MathAdd,NodeKind::MathSubtract,NodeKind::MathMultiply,NodeKind::MathDivide,NodeKind::MathEqual,NodeKind::MathNotEqual,NodeKind::MathLess,NodeKind::MathLessEqual,NodeKind::MathGreater,NodeKind::MathGreaterEqual,NodeKind::MathNegate,NodeKind::Index] {
  let p=rvn_parser::parse_program("scene label start\ntypewriter=true\n\"boundary\"\n").unwrap();
  let mut g=import_script(&p.script).unwrap().into_iter().find(|g|matches!(g.kind,GraphKind::Label{..})).unwrap();
  g.enable_native_frame_preparation().unwrap();
  let command=g.nodes.values().find(|n|n.kind==NodeKind::TypewriterSet).unwrap().id;let input=g.pin_by_key(command,"enabled").unwrap().id;
  let old=g.edges.values().find(|e|e.input==input).unwrap().clone();g.remove_node(g.pins[&old.output].node);
  let operator=g.add_catalog_node(kind,[400.,200.]).unwrap();
  if kind==NodeKind::MathNegate {
   g.set_pin_default(operator,"value",PropertyValue::Int(3)).unwrap();
  } else if kind==NodeKind::Index {
   let list=g.add_catalog_node(NodeKind::ListLiteral,[100.,200.]).unwrap();g.resize_value_inputs(list,2).unwrap();g.set_pin_default(list,"item_0",PropertyValue::Int(3)).unwrap();g.set_pin_default(list,"item_1",PropertyValue::Int(2)).unwrap();g.connect(g.pin_by_key(list,"list").unwrap().id,g.pin_by_key(operator,"target").unwrap().id).unwrap();g.set_pin_default(operator,"index",PropertyValue::Int(0)).unwrap();
  } else if !matches!(kind,NodeKind::LogicNot|NodeKind::LogicAnd|NodeKind::LogicOr) {
   g.set_pin_default(operator,"left",PropertyValue::Int(3)).unwrap();g.set_pin_default(operator,"right",PropertyValue::Int(2)).unwrap();
  }
  let mut output=g.nodes[&operator].pins.iter().copied().find(|id|g.pins[id].direction==PinDirection::Output).unwrap();
  if matches!(kind,NodeKind::MathAdd|NodeKind::MathSubtract|NodeKind::MathMultiply|NodeKind::MathDivide|NodeKind::MathNegate|NodeKind::Index) {
   let comparison=g.add_catalog_node(NodeKind::MathGreater,[550.,200.]).unwrap();g.set_pin_default(comparison,"right",PropertyValue::Int(0)).unwrap();g.connect(output,g.pin_by_key(comparison,"left").unwrap().id).unwrap();output=g.pin_by_key(comparison,"value").unwrap().id;
  }
  g.connect(output,input).unwrap();
  let before=g.clone();let compiled=prepare_native_frame_graph(&g).unwrap();assert_eq!(g,before);
  let actual=import_native_frame_script(&compiled.ast).unwrap().into_iter().find(|x|x.graph_id==g.graph_id).unwrap();
  assert_eq!(actual.edges,g.edges,"{kind:?}");assert_eq!(actual.nodes.keys().collect::<Vec<_>>(),g.nodes.keys().collect::<Vec<_>>(),"{kind:?}");assert_eq!(actual.pins,g.pins,"{kind:?}");
  let input_keys=if matches!(kind,NodeKind::LogicNot|NodeKind::MathNegate) {vec!["value"]}else if kind==NodeKind::Index {vec!["index"]}else{vec!["left","right"]};
  for key in input_keys {let pin=actual.pin_by_key(operator,key).unwrap();assert!(!actual.edges.values().any(|e|e.input==pin.id),"{kind:?}.{key} must stay an inline operand");}
 }
}
#[test]
fn typewriter0996_legacy_dynamic_boolean_remains_refused_without_explicit_optin() {
 let p=rvn_parser::parse_program("scene label start\nlocal enabled=false\ntypewriter=not enabled\n\"boundary\"\n").unwrap();
 assert!(import_script(&p.script).is_err());
 let mut g=graph();g.inspector_evaluation=InspectorEvaluationContract::LegacyStateless;g.schema_version=GRAPH_NAME_TEXT_SCHEMA_VERSION;
 assert!(transpile(&g).is_err());
}

#[test]
fn typewriter1001_signed_literal_and_negate_keep_distinct_real_native_producer_identities() {
 for positive in [PropertyValue::Int(4),PropertyValue::Float(0.5)] {
  for mode in 0..3 {
   let program=rvn_parser::parse_program("scene label start\ntypewriter=true\n\"boundary\"\n").unwrap();
   let mut g=import_script(&program.script).unwrap().into_iter().find(|g|matches!(g.kind,GraphKind::Label{..})).unwrap();g.enable_native_frame_preparation().unwrap();
   let command=g.nodes.values().find(|n|n.kind==NodeKind::TypewriterSet).unwrap().id;let input=g.pin_by_key(command,"enabled").unwrap().id;
   let old=g.edges.values().find(|edge|edge.input==input).unwrap().clone();g.remove_node(g.pins[&old.output].node);
   let compare=g.add_catalog_node(NodeKind::MathGreater,[600.,200.]).unwrap();g.set_pin_default(compare,"right",PropertyValue::Int(0)).unwrap();
   let actual_kind=if mode==0{NodeKind::Literal}else{NodeKind::MathNegate};let producer=g.add_catalog_node(actual_kind,[400.,200.]).unwrap();
   let connected=if mode==0 {
    let negative=match &positive{PropertyValue::Int(value)=>PropertyValue::Int(-*value),PropertyValue::Float(value)=>PropertyValue::Float(-*value),_=>unreachable!()};g.set_property(producer,"value",negative).unwrap();None
   }else if mode==1{g.set_pin_default(producer,"value",positive.clone()).unwrap();None}
   else{let literal=g.add_catalog_node(NodeKind::Literal,[200.,200.]).unwrap();g.set_property(literal,"value",positive.clone()).unwrap();g.connect(g.pin_by_key(literal,"value").unwrap().id,g.pin_by_key(producer,"value").unwrap().id).unwrap();Some(literal)};
   let output=g.pin_by_key(producer,if mode==0{"value"}else{"result"}).unwrap().id;
   g.connect(output,g.pin_by_key(compare,"left").unwrap().id).unwrap();g.connect(g.pin_by_key(compare,"value").unwrap().id,input).unwrap();
   let before=g.clone();let compiled=prepare_native_frame_graph(&g).unwrap();assert_eq!(g,before);
   let recovered=import_native_frame_script(&compiled.ast).unwrap();let actual=recovered.iter().find(|other|other.graph_id==g.graph_id).unwrap();
   assert_eq!(actual.edges,g.edges);assert_eq!(actual.pins,g.pins);assert_eq!(actual.nodes.keys().collect::<Vec<_>>(),g.nodes.keys().collect::<Vec<_>>());assert_eq!(actual.nodes[&producer].kind,actual_kind);
   if mode==0{assert_eq!(actual.nodes[&producer].properties.get("value"),g.nodes[&producer].properties.get("value"));}
   else{let operand=actual.pin_by_key(producer,"value").unwrap();assert_eq!(actual.edges.values().find(|edge|edge.input==operand.id).map(|edge|actual.pins[&edge.output].node),connected);if connected.is_none(){assert_eq!(operand.default_value,Some(positive.clone()));}
    let mut forged=compiled.ast.clone();let mut changed=0;
    for statement in &mut forged{if let rvn_parser::Statement::ScheduledConsumer{steps,..}=statement{for step in steps{if let rvn_parser::ConsumerStep::Capture{producer:id,value}=step{if id.contains(":math_negate|"){*value=rvn_parser::Expr::Int(4);changed+=1;}}}}}
    assert_eq!(changed,1);assert!(import_native_frame_script(&forged).is_err(),"a declared MathNegate cannot become a Literal expression");
   }
   assert_eq!(prepare_native_frame_graph(actual).unwrap().ast,compiled.ast);
  }
 }
}
