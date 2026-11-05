//! Typed, demanded Get Class projection. Runtime object admission stays in Core.
use crate::{GraphDocument,GraphNode,NodeKind,PinDirection,ValueType};

pub(crate) fn validate_node(graph:&GraphDocument,node:&GraphNode)->Result<(),String>{
    let expected=crate::node_definition(NodeKind::WidgetObjectClass).pins;
    if node.kind!=NodeKind::WidgetObjectClass || node.pins.len()!=expected.len(){
        return Err("Get Class requires its exact Object input and Class output".into());
    }
    for role in expected{
        let pins:Vec<_>=node.pins.iter().filter_map(|id|graph.pins.get(id)).filter(|pin|pin.key==role.key).collect();
        let [pin]=pins.as_slice() else{return Err("Get Class has a missing or duplicate port role".into());};
        if pin.node!=node.id || pin.direction!=role.direction || pin.value_type!=role.value_type
            || pin.cardinality!=role.cardinality || pin.default_value.is_some(){
            return Err("Get Class port direction/type/cardinality/default differs from its typed contract".into());
        }
    }
    let input=graph.pin_by_key(node.id,"object").ok_or("Get Class Object input is missing")?;
    let mut incoming=graph.edges.values().filter(|edge|edge.input==input.id);
    let edge=incoming.next().ok_or("Get Class requires a connected typed Widget object, including a real nullable factory result")?;
    if incoming.next().is_some(){return Err("Get Class Object input has ambiguous sources".into());}
    let source=graph.pins.get(&edge.output).ok_or("Get Class source output is missing")?;
    if source.direction!=PinDirection::Output || graph.nodes.get(&source.node).is_none_or(|owner|!owner.pins.contains(&source.id)){
        return Err("Get Class source is not an actual node output".into());
    }
    let actual=graph.effective_pin_type(source.id);
    if actual!=Some(ValueType::WidgetObject){
        return Err(format!("Get Class requires an actual WidgetObject producer, not {actual:?}; no wildcard or scalar conversion grants an object"));
    }
    Ok(())
}

#[cfg(test)]
mod tests{
    use super::*;
    use crate::{GraphKind,NodeId,PropertyValue};
    use rvn_parser::{Expr,Statement};
    fn expression(source:&str)->Expr{
        let script=rvn_parser::parse(&format!("set value={source}\n")).unwrap();
        let [Statement::SetVar{value,..}]=script.as_slice() else{panic!("actual expression")};value.clone()
    }
    fn seed_source(value:&str)->(GraphDocument,NodeId){
        let program=rvn_parser::parse_program(&format!("scene label Play\nset saved_object=0\n\"Ready\"\nset inspected={value}\n\"Applied\"\nreturn\n")).unwrap();
        let mut graphs=crate::import_script(&program.script).unwrap();
        let index=graphs.iter().position(|graph|matches!(&graph.kind,GraphKind::Label{name}if name=="Play")).unwrap();
        let mut graph=graphs.remove(index);graph.enable_native_frame_preparation().unwrap();
        let old=graph.nodes.values().find(|node|node.kind==NodeKind::SetVariable&&node.properties.get("name")==Some(&PropertyValue::String("saved_object".into()))).unwrap().id;
        let factory=graph.replace_assignment_with_widget_factory(old,&expression("__rvn_widget_class_none_v1()"),&Expr::ListLit(vec![])).unwrap();
        (graph,factory)
    }
    fn seed()->(GraphDocument,NodeId){seed_source("__rvn_text_table_valid(string_to_text(\"probe\"))")}
    fn connect(graph:&mut GraphDocument,from:NodeId,output:&str,to:NodeId,input:&str){
        graph.connect(graph.pin_by_key(from,output).unwrap().id,graph.pin_by_key(to,input).unwrap().id).unwrap();
    }
    fn getter(graph:&mut GraphDocument,factory:NodeId)->NodeId{
        let getter=graph.add_catalog_node(NodeKind::WidgetObjectClass,[200.,0.]).unwrap();
        // The actual executed factory's value output reads its assigned slot;
        // gathering it at a later consumer never replays object creation.
        connect(graph,factory,"value_out",getter,"object");getter
    }
    fn inspector_graph()->(GraphDocument,NodeId,NodeId){
        let(mut graph,factory)=seed();let getter=getter(&mut graph,factory);
        let old=graph.nodes.values().find(|node|node.kind==NodeKind::GetStringTableIdAndKey).unwrap().id;
        let text=graph.pin_by_key(old,"text").unwrap().id;
        let text_source=graph.edges.values().find(|edge|edge.input==text).unwrap().output;
        let outputs:Vec<_>=graph.edges.values().filter_map(|edge|{
            let pin=&graph.pins[&edge.output];(pin.node==old).then(||(pin.key.clone(),edge.input))
        }).collect();
        // A newly authored public inspector after opt-in has the Native
        // contract. Preserve the old Legacy inspector until its wires are read.
        let inspector=graph.add_catalog_node(NodeKind::GetStringTableIdAndKey,[800.,0.]).unwrap();
        graph.remove_node(old);
        for(key,input)in outputs{graph.connect(graph.pin_by_key(inspector,&key).unwrap().id,input).unwrap();}
        let list=graph.add_catalog_node(NodeKind::ListLiteral,[400.,0.]).unwrap();graph.resize_value_inputs(list,2).unwrap();
        connect(&mut graph,getter,"value",list,"item_0");
        graph.connect(text_source,graph.pin_by_key(list,"item_1").unwrap().id).unwrap();
        let index=graph.add_catalog_node(NodeKind::Index,[600.,0.]).unwrap();
        graph.set_pin_default(index,"index",PropertyValue::Int(1)).unwrap();
        connect(&mut graph,list,"list",index,"target");connect(&mut graph,index,"value",inspector,"text");
        graph.refresh_variable_usage_bindings().unwrap();
        (graph,getter,inspector)
    }
    #[test]
    fn object_class_graph1002_catalog_has_exact_typed_pure_ports_and_no_scalar_cast(){
        let definition=crate::node_definition(NodeKind::WidgetObjectClass);
        assert_eq!(definition.title,"Get Class");assert_eq!(definition.pins.len(),2);
        let input=&definition.pins[0];let output=&definition.pins[1];
        assert_eq!((input.key,input.direction,input.value_type.clone()),("object",PinDirection::Input,ValueType::WidgetObject));
        assert_eq!((output.key,output.direction,output.value_type.clone()),("value",PinDirection::Output,ValueType::WidgetClass));
        assert!(definition.pins.iter().all(|pin|!pin.value_type.is_execution()&&pin.default_value.is_none()));
        assert!(crate::ALL_NODE_KINDS.contains(&NodeKind::WidgetObjectClass));
        assert!(ValueType::WidgetClass.conversion_from(&ValueType::WidgetObject).is_none());
        assert!(ValueType::WidgetObject.conversion_from(&ValueType::String).is_none());
        assert_ne!(ValueType::WidgetClass,ValueType::WidgetObject);
    }
    #[test]
    fn object_class_graph1002_real_nullable_factory_feeds_getter_and_another_factory_with_strict_ids(){
        let(mut graph,factory)=seed_source("0");let getter=getter(&mut graph,factory);
        let old=graph.nodes.values().find(|node|node.kind==NodeKind::SetVariable&&node.properties.get("name")==Some(&PropertyValue::String("inspected".into()))).unwrap().id;
        let next=graph.replace_assignment_with_widget_factory(old,&expression("__rvn_widget_class_none_v1()"),&Expr::ListLit(vec![])).unwrap();
        let input=graph.pin_by_key(next,"class").unwrap().id;
        let edge=graph.edges.values().find(|edge|edge.input==input).unwrap().clone();
        let removed=graph.pins[&edge.output].node;graph.remove_edge(edge.id);graph.remove_node(removed);
        connect(&mut graph,getter,"value",next,"class");
        graph.refresh_variable_usage_bindings().unwrap();
        assert_eq!(graph.effective_pin_type(graph.pin_by_key(factory,"value_out").unwrap().id),Some(ValueType::WidgetObject));
        assert_eq!(graph.effective_pin_type(graph.pin_by_key(getter,"value").unwrap().id),Some(ValueType::WidgetClass));
        let prepared=crate::prepare_native_frame_graph(&graph).unwrap();
        assert!(prepared.source.contains("widget_object_class"));
        let recovered=crate::import_native_frame_script(&prepared.ast).unwrap();let same=recovered.iter().find(|actual|actual.graph_id==graph.graph_id).unwrap();
        assert_eq!(same.edges,graph.edges);assert!(same.nodes.keys().eq(graph.nodes.keys()));assert!(same.pins.keys().eq(graph.pins.keys()));
        assert_eq!(crate::prepare_native_frame_graph(same).unwrap().ast,prepared.ast);
    }
    #[test]
    fn object_class_graph1002_real_inspector_gathers_getter_once_and_roundtrips_all_owned_ids(){
        let(graph,getter,inspector)=inspector_graph();
        assert_eq!(graph.nodes[&inspector].inspector_contract,Some(crate::InspectorEvaluationContract::NativeFrameV1));
        assert!(graph.nodes.values().all(|node|node.kind!=NodeKind::FunctionCall));
        let before=serde_json::to_vec(&graph).unwrap();let prepared=crate::prepare_native_frame_graph(&graph).unwrap();
        let plans:Vec<_>=prepared.ast.iter().filter_map(|statement|match statement{
            Statement::ScheduledConsumer{steps,..}if steps.iter().any(|step|matches!(step,rvn_parser::ConsumerStep::InspectTextTable{..}))=>Some(steps),_=>None
        }).collect();assert_eq!(plans.len(),1);
        let steps=plans[0];
        let found:Vec<_>=steps.iter().filter_map(|step|match step{
            rvn_parser::ConsumerStep::Capture{producer,value:Expr::Call{name,..}}if name=="__rvn_widget_object_class_v1"=>Some(producer),_=>None
        }).collect();assert_eq!(found.len(),1);
        let(identity,terminal)=crate::native_frame_preparation::decode_producer(found[0]).unwrap();assert_eq!(identity.node,getter);assert_eq!(identity.kind,NodeKind::WidgetObjectClass);assert!(terminal.is_none());
        assert_eq!(steps.iter().filter(|step|matches!(step,rvn_parser::ConsumerStep::InspectTextTable{..})).count(),1);
        let recovered=crate::import_native_frame_script(&prepared.ast).unwrap();let same=recovered.iter().find(|actual|actual.graph_id==graph.graph_id).unwrap();
        assert_eq!(same.edges,graph.edges);assert!(same.nodes.keys().eq(graph.nodes.keys()));assert!(same.pins.keys().eq(graph.pins.keys()));
        assert_eq!(crate::prepare_native_frame_graph(same).unwrap().ast,prepared.ast);assert_eq!(serde_json::to_vec(&graph).unwrap(),before);
        let forged=prepared.source.replace(":widget_object_class|",":function_call|");assert_ne!(forged,prepared.source);
        let forged=rvn_parser::parse_program(&forged).unwrap();assert!(crate::import_native_frame_script(&forged.script).is_err());
        assert_eq!(serde_json::to_vec(&graph).unwrap(),before);
    }
    #[test]
    fn object_class_graph1002_inspector_reads_assigned_factory_result_without_replaying_random_arguments(){
        let(mut graph,_,inspector)=inspector_graph();
        let factory=graph.nodes.values().find(|node|node.kind==NodeKind::WidgetCreate).unwrap().id;
        // Empty [] is a genuine authored pin default, not an existing wire.
        // Materialize the new argument list through the public catalogue API.
        let arguments=graph.add_catalog_node(NodeKind::ListLiteral,[0.,100.]).unwrap();
        graph.resize_value_inputs(arguments,1).unwrap();
        connect(&mut graph,arguments,"list",factory,"arguments");
        let random=graph.add_catalog_node(NodeKind::FunctionCall,[0.,200.]).unwrap();
        graph.set_property(random,"function",PropertyValue::String("random".into())).unwrap();graph.resize_value_inputs(random,2).unwrap();
        graph.set_pin_default(random,"item_0",PropertyValue::Int(1)).unwrap();graph.set_pin_default(random,"item_1",PropertyValue::Int(9)).unwrap();
        connect(&mut graph,random,"result",arguments,"item_0");
        graph.refresh_variable_usage_bindings().unwrap();
        // The Legacy reproducibility check must also read the assigned result,
        // rather than inspect/reject/re-execute its earlier initializer.
        graph.validate_inspector_input_is_reproducible(inspector).unwrap();
        let prepared=crate::prepare_native_frame_graph(&graph).unwrap();
        let steps=prepared.ast.iter().find_map(|statement|match statement{
            Statement::ScheduledConsumer{steps,..}if steps.iter().any(|step|matches!(step,rvn_parser::ConsumerStep::InspectTextTable{..}))=>Some(steps),_=>None
        }).unwrap();
        assert!(steps.iter().any(|step|matches!(step,rvn_parser::ConsumerStep::Capture{value:Expr::Var(name),..}if name=="saved_object")));
        assert!(!steps.iter().any(|step|matches!(step,rvn_parser::ConsumerStep::Capture{value:Expr::Call{name,..},..}if name=="random")));
    }
    #[test]
    fn object_class_graph1002_nullable_object_is_typed_none_not_a_scalar_or_class(){
        let expression=expression("__rvn_widget_object_class_v1(value)");
        let null=rvn_parser::Value::Widget(Box::new(rvn_parser::widget_reference::WidgetReference::null()));
        let functions=rvn_core::eval::FunctionLibrary::default();
        assert_eq!(functions.eval(&expression,&std::collections::HashMap::from([("value".into(),null)])).unwrap(),rvn_parser::Value::WidgetClass(Box::new(None)));
        for wrong in [rvn_parser::Value::Int(0),rvn_parser::Value::Str("None".into()),rvn_parser::Value::WidgetClass(Box::new(None))]{
            assert!(functions.eval(&expression,&std::collections::HashMap::from([("value".into(),wrong)])).is_err());
        }
    }
    #[test]
    fn object_class_graph1002_purity_is_exact_and_still_rejects_unknown_rng_and_factory(){
        assert!(crate::text_table_nodes::reproducible_source(&expression("__rvn_widget_object_class_v1(saved_object)")));
        assert_eq!(crate::text_table_nodes::source_binding_names(&expression("__rvn_widget_object_class_v1(saved_object)")),std::collections::BTreeSet::from(["saved_object".into()]));
        for source in ["unknown(saved_object)","__rvn_widget_object_class_v1(random(1,9))","__rvn_widget_create_v1(__rvn_widget_class_none_v1(),[])","__rvn_widget_object_class_v1(unknown(saved_object))"]{
            assert!(!crate::text_table_nodes::reproducible_source(&expression(source)),"{source}");
        }
    }
    #[test]
    fn object_class_graph1002_reserved_builtin_arity_refuses_during_real_import(){
        for source in ["__rvn_widget_object_class_v1()","__rvn_widget_object_class_v1(0,0)"]{
            let program=rvn_parser::parse_program(&format!("scene label Play\nset value={source}\n\"Ready\"\nreturn\n")).unwrap();
            let before=serde_json::to_vec(&program.script).unwrap();
            assert!(crate::import_script(&program.script).unwrap_err().contains("Get Class expects exactly one"));
            assert_eq!(serde_json::to_vec(&program.script).unwrap(),before);
        }
    }
    #[test]
    fn object_class_graph1002_missing_any_and_forged_output_refuse_before_source_publication(){
        let(mut graph,_)=seed();let node=graph.add_catalog_node(NodeKind::WidgetObjectClass,[200.,0.]).unwrap();
        let before=serde_json::to_vec(&graph).unwrap();assert!(validate_node(&graph,&graph.nodes[&node]).unwrap_err().contains("connected typed Widget object"));
        assert!(crate::prepare_native_frame_graph(&graph).is_err());assert_eq!(serde_json::to_vec(&graph).unwrap(),before);
        let unknown=graph.add_catalog_node(NodeKind::FunctionCall,[0.,0.]).unwrap();graph.set_property(unknown,"function",PropertyValue::String("unknown".into())).unwrap();
        connect(&mut graph,unknown,"result",node,"object");
        let before=serde_json::to_vec(&graph).unwrap();assert!(validate_node(&graph,&graph.nodes[&node]).unwrap_err().contains("not Some(Any)"));
        assert!(crate::prepare_native_frame_graph(&graph).is_err());assert_eq!(serde_json::to_vec(&graph).unwrap(),before);
        let output=graph.pin_by_key(node,"value").unwrap().id;graph.pins.get_mut(&output).unwrap().value_type=ValueType::WidgetObject;
        let before=serde_json::to_vec(&graph).unwrap();assert!(validate_node(&graph,&graph.nodes[&node]).unwrap_err().contains("typed contract"));
        assert!(crate::prepare_native_frame_graph(&graph).is_err());assert_eq!(serde_json::to_vec(&graph).unwrap(),before);
    }
    #[test]
    fn object_class_graph1002_real_variable_get_inspector_roundtrips_its_distinct_owned_node_and_wire(){
        let(mut graph,getter,_)=inspector_graph();
        let factory=graph.nodes.values().find(|node|node.kind==NodeKind::WidgetCreate).unwrap().id;
        let object_input=graph.pin_by_key(getter,"object").unwrap().id;
        let previous=graph.edges.values().find(|edge|edge.input==object_input).unwrap().clone();
        assert_eq!(previous.output,graph.pin_by_key(factory,"value_out").unwrap().id);
        let read=graph.add_catalog_node(NodeKind::VariableGet,[100.,0.]).unwrap();
        graph.set_property(read,"name",PropertyValue::String("saved_object".into())).unwrap();
        graph.refresh_variable_usage_bindings().unwrap();
        assert_eq!(graph.effective_pin_type(graph.pin_by_key(read,"value").unwrap().id),Some(ValueType::WidgetObject));
        graph.remove_edge(previous.id);
        connect(&mut graph,read,"value",getter,"object");
        let before=serde_json::to_vec(&graph).unwrap();
        let prepared=crate::prepare_native_frame_graph(&graph).unwrap();
        let recovered=crate::import_native_frame_script(&prepared.ast).unwrap();
        let same=recovered.iter().find(|actual|actual.graph_id==graph.graph_id).unwrap();
        assert_eq!(same.nodes[&read].kind,NodeKind::VariableGet);
        assert_eq!(same.pins[&same.edges.values().find(|edge|edge.input==object_input).unwrap().output].node,read);
        assert_eq!(same.edges,graph.edges);
        assert!(same.nodes.keys().eq(graph.nodes.keys()));
        assert!(same.pins.keys().eq(graph.pins.keys()));
        assert_eq!(crate::prepare_native_frame_graph(same).unwrap().ast,prepared.ast);
        assert_eq!(serde_json::to_vec(&graph).unwrap(),before);
    }
}
