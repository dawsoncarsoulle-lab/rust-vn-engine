use rvn_graph::*;
fn remove_exclusive_pure_branch(graph:&mut GraphDocument,output:PinId){
    let Some(pin)=graph.pins.get(&output) else{return;};let node=pin.node;
    if graph.nodes[&node].pins.iter().any(|p|graph.pins[p].value_type==ValueType::Execution)
        || graph.edges.values().any(|e|graph.pins[&e.output].node==node){return;}
    let upstream:Vec<_>=graph.edges.values().filter(|e|graph.pins[&e.input].node==node).map(|e|e.output).collect();
    assert!(graph.remove_node(node).is_some());
    for output in upstream{remove_exclusive_pure_branch(graph,output);}
}
pub fn replace_text_source(graph:&mut GraphDocument,dialogue:NodeId,output:PinId,reroute:bool){
    let mut input=graph.pin_by_key(dialogue,"text").unwrap().id;
    let original=graph.edges.values().find(|e|e.input==input).unwrap();let source=graph.pins[&original.output].node;
    if graph.nodes[&source].kind==NodeKind::ConvertStringToText&&graph.nodes[&source].properties.get("legacy_passthrough")==Some(&PropertyValue::Bool(true)){
        input=graph.pin_by_key(source,"value").unwrap().id;
    }
    let edges:Vec<_>=graph.edges.values().filter(|e|e.input==input).map(|e|e.id).collect();assert_eq!(edges.len(),1);
    let edge=graph.remove_edge(edges[0]).unwrap();remove_exclusive_pure_branch(graph,edge.output);
    let output=if reroute{let node=graph.add_catalog_node(NodeKind::Reroute,[700.0,300.0]).unwrap();let input=graph.pin_by_key(node,"value").unwrap().id;graph.connect(output,input).unwrap();graph.pin_by_key(node,"value_out").unwrap().id}else{output};
    graph.connect(output,input).expect("actual typed authored source wire");
}
pub fn source_variable_name(graph:&GraphDocument,dialogue:NodeId)->String{
    let mut input=graph.pin_by_key(dialogue,"text").unwrap().id;
    for _ in 0..128{
        let edge=graph.edges.values().find(|e|e.input==input).unwrap();let node=&graph.nodes[&graph.pins[&edge.output].node];
        if matches!(node.kind,NodeKind::VariableGet|NodeKind::SetVariable|NodeKind::LocalVariable){return graph.variable_node_name(node.id).unwrap();}
        if node.kind==NodeKind::TextValue{let PropertyValue::String(raw)=&node.properties["value"]else{panic!()};return raw.strip_prefix('[').and_then(|s|s.strip_suffix(']')).expect("actual single authored slot").into();}
        assert!(matches!(node.kind,NodeKind::ConvertStringToText|NodeKind::Reroute),"read actual source branch, never infer by equal values");input=graph.pin_by_key(node.id,"value").unwrap().id;
    }
    panic!("source branch exceeds bound")
}
