use rvn_graph::*;

fn node(g: &mut GraphDocument, kind: NodeKind) -> NodeId {
    g.add_catalog_node(kind, [17., 29.]).unwrap()
}
fn wire(g: &mut GraphDocument, a: NodeId, ak: &str, b: NodeId, bk: &str) -> EdgeId {
    g.connect(g.pin_by_key(a,ak).unwrap().id, g.pin_by_key(b,bk).unwrap().id).unwrap()
}
fn fixture() -> (GraphDocument, NodeId, NodeId, NodeId) {
    let mut g = GraphDocument::new(GraphId::new(699), GraphKind::Handler { name: "loop_probe".into() });
    let root = node(&mut g, NodeKind::HandlerEntry);
    let each = node(&mut g, NodeKind::ForEach);
    let source = node(&mut g, NodeKind::ListLiteral);
    g.set_property(source, "items", PropertyValue::StringList(vec!["11".into(),"22".into(),"33".into()])).unwrap();
    let consumer = node(&mut g, NodeKind::UiSetState);
    // Real required operands for the source-preparation fixture. This test
    // never executes a UI host or claims that these strings own one.
    g.set_pin_default(consumer, "name", PropertyValue::String("loop_probe_screen".into())).unwrap();
    g.set_pin_default(consumer, "element", PropertyValue::String("loop_probe_control".into())).unwrap();
    g.set_pin_default(consumer, "state", PropertyValue::Bool(false)).unwrap();
    let end = g.add_branch_end(each, [81.,91.]).unwrap();
    wire(&mut g, root,"exec_out",each,"exec_in");
    wire(&mut g, source,"list",each,"collection");
    wire(&mut g, each,"body",consumer,"exec_in");
    wire(&mut g, consumer,"exec_out",end,"exec_in");
    (g, each, source, consumer)
}
fn pure_count(plan: &NativeLoopDemandPlan, wanted: NodeId) -> usize {
    plan.operations.iter().filter(|operation| matches!(operation, NativeDemandOperation::Pure {node,..} if *node==wanted)).count()
}

#[test]
fn historical_foreach_keeps_old_bytes_pins_ast_and_no_automatic_promotion() {
    let (g, each, _, consumer) = fixture();
    for key in ["name","element"]{
        let mut missing=g.clone();missing.set_pin_default(consumer,key,PropertyValue::String(String::new())).unwrap();
        assert!(matches!(transpile(&missing),Err(TranspileError::MissingInputValue{node,key:missing_key})if node==consumer&&missing_key==key),"empty catalogue placeholder must remain a refused operand");
    }
    let json = g.to_pretty_json().unwrap();
    assert!(!json.contains("native_loop_contract"));
    assert_eq!(g.nodes[&each].pins.len(),4);
    let ast = transpile(&g).unwrap().ast;
    let cold = GraphDocument::from_json(&json).unwrap();
    assert_eq!(cold.to_pretty_json().unwrap(),json);
    assert_eq!(transpile(&cold).unwrap().ast,ast);
    assert_eq!(cold.schema_version, GRAPH_SCHEMA_VERSION);
}
#[test]
fn migration_is_reviewable_and_preserves_all_authored_ids_edges_values_and_layout() {
    let (g, each, _, _) = fixture();
    let before = g.to_pretty_json().unwrap();
    let migration = g.prepare_native_loop_demand(each).unwrap();
    assert_eq!(g.to_pretty_json().unwrap(),before);
    assert_eq!(migration.graph.edges,g.edges);
    assert_eq!(migration.graph.nodes.keys().collect::<Vec<_>>(),g.nodes.keys().collect::<Vec<_>>());
    for (id,pin) in &g.pins { assert_eq!(migration.graph.pins[id],*pin); }
    for (id,node) in &g.nodes {
        let after = &migration.graph.nodes[id];
        assert_eq!(after.position,node.position);
        assert_eq!(after.properties,node.properties);
        assert_eq!(after.title_override,node.title_override);
        assert!(after.pins.starts_with(&node.pins));
    }
    assert_eq!(migration.added_pins.len(),2);
    assert!(migration.added_pins.iter().all(|id| !g.pins.contains_key(id)));
    assert_eq!(migration.graph.nodes[&each].native_loop_contract,Some(NativeLoopContract::DemandV1));
}
#[test]
fn explicit_contract_and_real_output_pins_roundtrip_without_changing_old_nodes() {
    let (g, each, _, _) = fixture();
    let m = g.prepare_native_loop_demand(each).unwrap();
    let cold = GraphDocument::from_json(&m.graph.to_pretty_json().unwrap()).unwrap();
    assert_eq!(cold,m.graph);
    assert_eq!(cold.schema_version,GRAPH_NATIVE_LOOP_DEMAND_SCHEMA_VERSION);
    assert_eq!(cold.pin_by_key(each,"array_index").unwrap().value_type,ValueType::Int);
    assert_eq!(cold.pin_by_key(each,"array_element").unwrap().direction,PinDirection::Output);
    let again = cold.prepare_native_loop_demand(each).unwrap();
    assert_eq!(again.graph,cold);
    assert!(again.added_pins.is_empty());
}
#[test]
fn migration_keeps_real_authored_text_owner_and_its_roundtrip() {
    let (mut g,each,_,_)=fixture();
    g.enable_text_owner(TextOwnerId::parse("797679920f7142be99d9d6fe5553ca34").unwrap()).unwrap();
    let owner=g.text_owner.clone();
    let migration=g.prepare_native_loop_demand(each).unwrap();
    assert_eq!(migration.graph.text_owner,owner);
    let cold=GraphDocument::from_json(&migration.graph.to_pretty_json().unwrap()).unwrap();
    assert_eq!(cold.text_owner,owner);
    assert_eq!(g.schema_version,GRAPH_TEXT_SCHEMA_VERSION);
}
#[test]
fn explicit_forloop_migration_preserves_existing_index_port_without_duplicate_pins() {
    let mut g = GraphDocument::new(GraphId::new(700),GraphKind::Init);
    let range = node(&mut g,NodeKind::ForLoopWithBreak);
    let pins = g.nodes[&range].pins.clone();
    let m = g.prepare_native_loop_demand(range).unwrap();
    assert!(m.added_pins.is_empty());
    assert_eq!(m.graph.nodes[&range].pins,pins);
    assert!(m.graph.plan_native_loop_header(range,NativeLoopPhase::FirstIndex).is_ok());
    assert!(m.graph.plan_native_loop_header(range,NativeLoopPhase::LastIndex).is_ok());
    assert!(m.graph.plan_native_loop_header(range,NativeLoopPhase::ArrayLength).is_err());
}
#[test]
fn named_historical_item_reads_are_reported_and_never_rewired_as_element() {
    let (mut g, each, _, _) = fixture();
    g.add_variable("item",ValueType::String,PropertyValue::String("old".into())).unwrap();
    let read = node(&mut g,NodeKind::VariableGet);
    g.set_property(read,"name",PropertyValue::String("item".into())).unwrap();
    let m = g.prepare_native_loop_demand(each).unwrap();
    assert_eq!(m.legacy_named_reads,vec![read]);
    assert_eq!(m.graph.nodes[&read],g.nodes[&read]);
    assert_eq!(m.graph.edges,g.edges);
}
#[test]
fn wrong_kind_schema_and_forged_output_shape_are_refused() {
    let (g, each, _, consumer) = fixture();
    assert!(g.prepare_native_loop_demand(consumer).is_err());
    let mut m = g.prepare_native_loop_demand(each).unwrap().graph;
    m.schema_version=GRAPH_MIXED_INSPECTOR_SCHEMA_VERSION;
    assert!(m.validate_native_loop_contracts().is_err());
    m.schema_version=GRAPH_NATIVE_LOOP_DEMAND_SCHEMA_VERSION;
    m.nodes.get_mut(&consumer).unwrap().native_loop_contract=Some(NativeLoopContract::DemandV1);
    assert!(m.validate_native_loop_contracts().is_err());
    m.nodes.get_mut(&consumer).unwrap().native_loop_contract=None;
    let output=m.pin_by_key(each,"array_index").unwrap().id;
    m.pins.get_mut(&output).unwrap().value_type=ValueType::Float;
    assert!(m.validate_native_loop_contracts().is_err());
}
#[test]
fn exhausted_pin_ids_refuse_without_mutating_authored_document() {
    let (g, each, _, _) = fixture();
    let mut value=serde_json::to_value(g).unwrap();
    value["next_pin_id"]=serde_json::json!(u64::MAX);
    let g:GraphDocument=serde_json::from_value(value).unwrap();
    let before=g.to_pretty_json().unwrap();
    assert!(g.prepare_native_loop_demand(each).is_err());
    assert_eq!(g.to_pretty_json().unwrap(),before);
}
#[test]
fn element_type_comes_from_actual_collection_and_not_from_its_consumer() {
    let (g, each, _, _) = fixture();
    let g=g.prepare_native_loop_demand(each).unwrap().graph;
    assert_eq!(g.effective_pin_type(g.pin_by_key(each,"array_element").unwrap().id),Some(ValueType::String));
    assert_eq!(g.effective_pin_type(g.pin_by_key(each,"array_index").unwrap().id),Some(ValueType::Int));
}
#[test]
fn header_and_index_only_consumer_have_distinct_dependency_placements() {
    let (g, each, source, consumer) = fixture();
    let mut g=g.prepare_native_loop_demand(each).unwrap().graph;
    wire(&mut g,each,"array_index",consumer,"state");
    let header=g.plan_native_loop_header(each,NativeLoopPhase::ArrayLength).unwrap();
    let body=g.plan_native_loop_consumer(each,consumer).unwrap();
    assert_eq!(pure_count(&header,source),1);
    assert_eq!(pure_count(&body,source),0);
    assert!(body.operations.iter().any(|op|matches!(op,NativeDemandOperation::LoopIndex{owner,..} if *owner==each)));
    assert!(!body.operations.iter().any(|op|matches!(op,NativeDemandOperation::ArrayElement{..})));
    assert!(header.operations.iter().all(|op|!matches!(op,NativeDemandOperation::ArrayElement{..})));
}
#[test]
fn same_actual_element_fanout_deduplicates_supplier_inside_one_consumer() {
    let (g, each, source, consumer) = fixture();
    let mut g=g.prepare_native_loop_demand(each).unwrap().graph;
    let a=wire(&mut g,each,"array_element",consumer,"name");
    let b=wire(&mut g,each,"array_element",consumer,"element");
    let plan=g.plan_native_loop_consumer(each,consumer).unwrap();
    assert_eq!(pure_count(&plan,source),1);
    assert_eq!(plan.operations.iter().filter(|op|matches!(op,NativeDemandOperation::ArrayElement{owner,..} if *owner==each)).count(),1);
    assert!(plan.wires.iter().any(|wire|wire.edge==a));
    assert!(plan.wires.iter().any(|wire|wire.edge==b));
    assert!(matches!(plan.operations.last(),Some(NativeDemandOperation::ArrayElement{..})));
}
#[test]
fn two_successive_consumers_get_independent_gathers_not_one_loop_cache() {
    let (g, each, source, first) = fixture();
    let mut g=g.prepare_native_loop_demand(each).unwrap().graph;
    let second=node(&mut g,NodeKind::UiSetState);
    g.set_pin_default(second,"state",PropertyValue::Bool(false)).unwrap();
    let old=g.edges.values().find(|e|e.output==g.pin_by_key(first,"exec_out").unwrap().id).unwrap().id;
    let old=g.edges.remove(&old).unwrap();
    wire(&mut g,first,"exec_out",second,"exec_in");
    let output=g.pin_by_key(second,"exec_out").unwrap().id;
    g.connect(output,old.input).unwrap();
    wire(&mut g,each,"array_element",first,"name");
    wire(&mut g,each,"array_element",second,"name");
    let a=g.plan_native_loop_consumer(each,first).unwrap();
    let b=g.plan_native_loop_consumer(each,second).unwrap();
    assert_ne!(a.phase,b.phase);
    assert_eq!(pure_count(&a,source),1);
    assert_eq!(pure_count(&b,source),1);
    assert_eq!(g.plan_native_loop_header(each,NativeLoopPhase::ArrayLength).unwrap().operations.len(),1);
}
#[test]
fn retained_set_output_never_replays_its_rhs_in_header_or_element_gather() {
    let (g, each, rhs, consumer) = fixture();
    let mut g=g.prepare_native_loop_demand(each).unwrap().graph;
    let assignment=node(&mut g,NodeKind::LocalVariable);
    g.set_property(assignment,"name",PropertyValue::String("items".into())).unwrap();
    let old=g.edges.values().find(|e|e.input==g.pin_by_key(each,"collection").unwrap().id).unwrap().id;
    g.edges.remove(&old);
    wire(&mut g,rhs,"list",assignment,"value");
    wire(&mut g,assignment,"value_out",each,"collection");
    wire(&mut g,each,"array_element",consumer,"name");
    for plan in [g.plan_native_loop_header(each,NativeLoopPhase::ArrayLength).unwrap(),g.plan_native_loop_consumer(each,consumer).unwrap()] {
        assert_eq!(pure_count(&plan,rhs),0);
        assert!(plan.operations.iter().any(|op|matches!(op,NativeDemandOperation::RetainedExecutionOutput{node,..} if *node==assignment)));
    }
}
#[test]
fn after_completed_or_isolated_pure_preview_cannot_borrow_body_owner() {
    let (g, each, pure, _) = fixture();
    let mut g=g.prepare_native_loop_demand(each).unwrap().graph;
    let after=node(&mut g,NodeKind::UiSetState);
    wire(&mut g,each,"completed",after,"exec_in");
    wire(&mut g,each,"array_element",after,"name");
    assert!(g.plan_native_loop_consumer(each,after).is_err());
    assert!(g.plan_native_loop_consumer(each,pure).is_err());
}
#[test]
fn graph_preparation_exports_real_lexical_loop_without_snapshot_or_isolated_owner() {
    let (g, each, _, _) = fixture();
    let historical=transpile(&g).unwrap();
    assert!(!historical.source.contains("flow.consume_v3("));
    let before=g.to_pretty_json().unwrap();
    let native=g.prepare_native_loop_demand(each).unwrap().graph;
    assert_eq!(g.to_pretty_json().unwrap(),before,"explicit preparation does not edit the old authored graph");
    let compiled=transpile(&native).expect("real Handler root has the existing lexical backend");
    assert_eq!(rvn_parser::parse(&compiled.source).unwrap(),compiled.ast);
    assert!(matches!(scheduled_loop(&compiled.ast),Some(rvn_parser::Statement::ScheduledConsumer{version:3,..})),"export the explicit demand contract, never snapshot fallback");
    let restored=import_native_frame_script(&compiled.ast).unwrap();
    let again=restored.iter().find(|g|g.graph_id==native.graph_id).expect("actual source graph identity");
    assert_eq!(again.kind,native.kind);
    assert_eq!(again.nodes.keys().collect::<Vec<_>>(),native.nodes.keys().collect::<Vec<_>>());
    assert_eq!(again.pins,native.pins);assert_eq!(again.edges,native.edges);
    assert_eq!(again.nodes[&each].native_loop_contract,Some(NativeLoopContract::DemandV1));
    assert_eq!(transpile(again).unwrap().ast,compiled.ast);
    assert!(transpile_value_output(&native,each,"array_element").is_err(),"no isolated preview may forge a body activation");
    assert!(transpile_value_input(&native,each,"collection").is_err(),"no isolated preview may own a header phase");
}

fn scheduled_loop(block: &[rvn_parser::Statement]) -> Option<&rvn_parser::Statement> {
    use rvn_parser::Statement as S;
    for statement in block {
        match statement {
            S::ScheduledConsumer{body,..} if matches!(body.as_slice(),[S::ForEach{..}|S::ForLoopWithBreak{..}]) => return Some(statement),
            S::Init{body}|S::Function{body,..}|S::Handler{body,..}|S::Screen{body,..} => if let Some(found)=scheduled_loop(body){return Some(found);},
            _=>{},
        }
    }
    None
}
#[test]
fn actual_native_graph_source_can_be_sliced_by_length_without_changing_snapshot_semantics() {
    let (mut g,_,_,_) = fixture();
    g.enable_native_frame_preparation().unwrap();
    let compiled=prepare_native_frame_graph(&g).unwrap();
    let plan=scheduled_loop(&compiled.ast).unwrap();
    let before=serde_json::to_vec(&compiled.ast).unwrap();
    let Some(rvn_parser::loop_phase_dependencies::LoopHeaderDependencies::Each{length})=
        rvn_parser::loop_phase_dependencies::analyze_loop_header_dependencies(plan).unwrap() else {panic!()};
    assert_eq!(length,vec![0]);
    assert_eq!(serde_json::to_vec(&compiled.ast).unwrap(),before);
    assert_eq!(rvn_parser::parse(&compiled.source).unwrap(),compiled.ast);
}
#[test]
fn actual_shared_first_last_graph_producer_remains_in_two_separate_phase_slices() {
    let mut g=GraphDocument::new(GraphId::new(701),GraphKind::Init);
    let root=node(&mut g,NodeKind::Init);
    let range=node(&mut g,NodeKind::ForLoopWithBreak);
    let supplier=node(&mut g,NodeKind::Literal);
    g.set_property(supplier,"value",PropertyValue::Int(0)).unwrap();
    let end=g.add_branch_end(range,[0.,0.]).unwrap();
    wire(&mut g,root,"exec_out",range,"exec_in");
    wire(&mut g,range,"body",end,"exec_in");
    wire(&mut g,supplier,"value",range,"first");
    wire(&mut g,supplier,"value",range,"last");
    g.enable_native_frame_preparation().unwrap();
    let compiled=prepare_native_frame_graph(&g).unwrap();
    let plan=scheduled_loop(&compiled.ast).unwrap();
    let Some(rvn_parser::loop_phase_dependencies::LoopHeaderDependencies::Range{first,last})=
        rvn_parser::loop_phase_dependencies::analyze_loop_header_dependencies(plan).unwrap() else {panic!()};
    assert_eq!(first,vec![0]);
    assert_eq!(last,vec![0]);
    // This analysis never executes them. The future explicit contract must
    // activate each phase separately; consume_v2 still captures once on entry.
    let rvn_parser::Statement::ScheduledConsumer{version,steps,..}=plan else {panic!()};
    assert_eq!(*version,2);
    assert_eq!(steps.len(),1);
}
