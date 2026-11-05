//! Real SourceProject/Widget descriptors and the existing CPU host. Bevy tests
//! separately prove the actual wrapper backend; no native/pixel claim here.
use super::*;
use rvn_graph::{GraphKind, NodeKind, PropertyValue};
// The imported Dialogue owns no text property. Resolve the exact authored
// TextValue.value output wired into its real text input, not graph order.
fn dialogue_with_literal(graph: &rvn_graph::GraphDocument, text: &str) -> rvn_graph::NodeId {
    let candidates = graph.nodes.values().filter(|node| {
        if node.kind != NodeKind::Dialogue { return false; }
        let input = graph.pin_by_key(node.id, "text").expect("Dialogue text input");
        let incoming: Vec<_> = graph.edges.values().filter(|edge| edge.input == input.id).collect();
        let [edge] = incoming.as_slice() else { panic!("one authored Dialogue text wire"); };
        let output = &graph.pins[&edge.output];
        let producer = &graph.nodes[&output.node];
        producer.kind == NodeKind::TextValue
            && graph.pin_by_key(producer.id, "value").is_some_and(|pin| pin.id == output.id)
            && producer.properties.get("value") == Some(&PropertyValue::String(text.into()))
    }).map(|node| node.id).collect::<Vec<_>>();
    assert_eq!(candidates.len(), 1, "one Dialogue fed by literal {text:?}");
    candidates[0]
}
fn wire(graph:&mut rvn_graph::GraphDocument,from:rvn_graph::NodeId,output:&str,to:rvn_graph::NodeId,input:&str){
    graph.connect(graph.pin_by_key(from,output).unwrap().id,graph.pin_by_key(to,input).unwrap().id).unwrap();
}
fn insert(graph:&mut rvn_graph::GraphDocument,before:rvn_graph::NodeId,kind:NodeKind)->rvn_graph::NodeId{
    let input=graph.pin_by_key(before,"exec_in").unwrap().id;
    let edge=graph.edges.values().find(|edge|edge.input==input).unwrap().clone();
    graph.remove_edge(edge.id);let node=graph.add_catalog_node(kind,[300.,0.]).unwrap();
    graph.connect(edge.output,graph.pin_by_key(node,"exec_in").unwrap().id).unwrap();wire(graph,node,"exec_out",before,"exec_in");node
}
fn fixture(z_order:i64,duplicate:bool,null:bool)->Fixture{
    fixture_with_slot(z_order,duplicate,null,0)
}
fn fixture_with_slot(z_order:i64,duplicate:bool,null:bool,slot:i32)->Fixture{
    fixture_with_input(z_order,duplicate,null,slot,false)
}
fn fixture_with_input(z_order:i64,duplicate:bool,null:bool,slot:i32,random:bool)->Fixture{
    Fixture::with_source_transform_at(false,false,false,false,|root,source|{
        let source=source.replacen("init{set hits=0}",&format!("init{{set hits=0 set authored_z={z_order}}}"),1)
            .replacen("label start","scene label start",1)
            .replace("\n\"Ready\"\nreturn\n","\nset instance=0\n\"Ready\"\n\"Attached\"\n\"Detached\"\nreturn\n");
        let source=source.replacen("screen panel(method){return component(\"go\",\"button\",{\"events\":{\"click\":method}},[])}",
            &format!("screen panel(method){{return {{\"host\":{{\"viewport_slot\":{{\"z_order\":{slot}}}}},\"root\":component(\"go\",\"button\",{{\"events\":{{\"click\":method}}}},[])}}}}"),1);
        let path=root.join("main.rvn");fs::write(&path,&source).unwrap();
        let mut project=rvn_graph::SourceProject::open_at(&path,&source,&[]).unwrap();let mut graphs=project.graphs().to_vec();
        let graph=graphs.iter_mut().find(|graph|matches!(&graph.kind,GraphKind::Label{name}if name=="start")).unwrap();
        graph.enable_native_frame_preparation().unwrap();
        let assignment=graph.nodes.values().find(|node|node.kind==NodeKind::SetVariable&&node.properties.get("name")==Some(&PropertyValue::String("instance".into()))).unwrap().id;
        let class=if null{rvn_parser::Expr::Call{name:"__rvn_widget_class_none_v1".into(),args:vec![]}}
            else{rvn_parser::Expr::Call{name:"__rvn_widget_class_v1".into(),args:vec![rvn_parser::Expr::Str(A.into())]}};
        let arguments=rvn_parser::Expr::ListLit(if null{vec![]}else{vec![rvn_parser::Expr::Str("allowed".into())]});
        let factory=graph.replace_assignment_with_widget_factory(assignment,&class,&arguments).unwrap();
        let attached=dialogue_with_literal(graph,"Attached");let detached=dialogue_with_literal(graph,"Detached");
        let add=insert(graph,attached,NodeKind::WidgetAddToViewport);wire(graph,factory,"value_out",add,"object");
        if random{
            let random=graph.add_catalog_node(NodeKind::FunctionCall,[0.,100.]).unwrap();graph.set_property(random,"function",PropertyValue::String("random".into())).unwrap();graph.resize_value_inputs(random,2).unwrap();
            graph.set_pin_default(random,"item_0",PropertyValue::Int(1)).unwrap();graph.set_pin_default(random,"item_1",PropertyValue::Int(9)).unwrap();wire(graph,random,"result",add,"z_order");
        }else{
            let read=graph.add_catalog_node(NodeKind::VariableGet,[0.,100.]).unwrap();graph.set_property(read,"name",PropertyValue::String("authored_z".into())).unwrap();graph.refresh_variable_usage_bindings().unwrap();wire(graph,read,"value",add,"z_order");
        }
        let remove=insert(graph,detached,if duplicate{NodeKind::WidgetAddToViewport}else{NodeKind::WidgetRemoveFromParent});wire(graph,factory,"value_out",remove,"object");
        project.apply_visual(&source,&graphs).unwrap();project.source().to_owned()
    })
}
fn at_add(fixture:&Fixture)->Engine<Host>{let mut engine=fixture.engine();engine.advance_dialogue().unwrap();engine}
#[test]
fn viewport1003_real_scene_attach_remove_and_i32_limits_publish_once_and_keep_object(){
    for(layer,slot)in [(i32::MIN,0),(0,0),(i32::MAX,0),(i32::MAX,1),(i32::MIN,-1),(42,-500)]{
        let fixture=fixture_with_slot(i64::from(layer),false,false,slot);let mut engine=at_add(&fixture);
        let value=engine.state.vars["instance"].clone();let handle=engine.widget_reference_handle(&value).unwrap();
        let baseline=engine.renderer.batches.lock().unwrap().len();let rng=engine.state.random;
        engine.step_silent().unwrap();assert_eq!(engine.renderer.batches.lock().unwrap().len(),baseline+1);assert_eq!(engine.state.random,rng);
        let screen=engine.state.ui.screens.iter().find(|screen|screen.name==handle.alias()).unwrap();assert!(screen.viewport_attached());assert_eq!(screen.layer,layer);
        assert_eq!(screen.owner_properties.viewport_slot.z_order,slot,"authored descriptor is retained, not rewritten into another definition");
        assert_eq!(engine.interface_views().unwrap().iter().find(|view|view.name==handle.alias()).unwrap().layer,layer,"global placement uses the Add argument, not a sum with the same viewport slot");
        assert_eq!(engine.widget_object_class(&value).unwrap(),engine.widget_object_class(&engine.state.vars["instance"]).unwrap());
        let old_input=engine.capture_ui_input_source(handle.alias()).unwrap();
        engine.step_until_interaction().unwrap();engine.advance_dialogue().unwrap();
        let baseline=engine.renderer.batches.lock().unwrap().len();engine.step_silent().unwrap();assert_eq!(engine.renderer.batches.lock().unwrap().len(),baseline+1);
        let screen=engine.state.ui.screens.iter().find(|screen|screen.name==handle.alias()).unwrap();assert!(!screen.viewport_attached());
        assert_eq!(engine.widget_reference_handle(&value).unwrap(),handle,"removal retains the logical object, not an alias replacement");
        assert!(!engine.ui_input_source_current(&old_input));assert!(engine.capture_ui_input_source(handle.alias()).is_none());
        engine.add_widget_to_viewport(&handle,false,layer).unwrap();
        assert!(!engine.ui_input_source_current(&old_input),"the reached Scene removal revoked the previous presentation");
        assert_ne!(engine.capture_ui_input_source(handle.alias()).unwrap().authority,old_input.authority);
    }
}
#[test]
fn viewport1003_wrong_null_stale_and_out_of_i32_operands_refuse_without_pc_rng_or_publication(){
    for layer in [i64::from(i32::MIN)-1,i64::from(i32::MAX)+1]{
        let fixture=fixture(layer,false,false);let mut engine=at_add(&fixture);let before=serde_json::to_vec(&engine.state).unwrap();let publications=engine.renderer.batches.lock().unwrap().len();
        assert!(engine.step_silent().is_err());assert_eq!(serde_json::to_vec(&engine.state).unwrap(),before);assert_eq!(engine.renderer.batches.lock().unwrap().len(),publications);
    }
    let null=fixture(0,false,true);let mut engine=at_add(&null);let before=serde_json::to_vec(&engine.state).unwrap();let publications=engine.renderer.batches.lock().unwrap().len();
    assert!(engine.step_silent().is_err());assert_eq!(serde_json::to_vec(&engine.state).unwrap(),before);assert_eq!(engine.renderer.batches.lock().unwrap().len(),publications);
    let fixture=fixture(0,false,false);
    for wrong in [Value::Bool(false),Value::Str(A.into()),Value::WidgetClass(Box::new(None))]{
        let mut engine=at_add(&fixture);engine.state.vars.insert("instance".into(),wrong);let before=serde_json::to_vec(&engine.state).unwrap();let publications=engine.renderer.batches.lock().unwrap().len();
        assert!(engine.step_silent().is_err());assert_eq!(serde_json::to_vec(&engine.state).unwrap(),before);assert_eq!(engine.renderer.batches.lock().unwrap().len(),publications);
    }
    let mut engine=at_add(&fixture);let value=engine.state.vars["instance"].clone();let handle=engine.widget_reference_handle(&value).unwrap();engine.destroy_widget(handle).unwrap();
    let before=serde_json::to_vec(&engine.state).unwrap();let publications=engine.renderer.batches.lock().unwrap().len();
    assert!(engine.step_silent().is_err());assert_eq!(serde_json::to_vec(&engine.state).unwrap(),before);assert_eq!(engine.renderer.batches.lock().unwrap().len(),publications);
}
#[test]
fn viewport1003_duplicate_attach_and_renderer_refusal_keep_old_instance_and_source_atomic(){
    let fixture=fixture(0,true,false);let mut engine=at_add(&fixture);engine.step_silent().unwrap();engine.step_until_interaction().unwrap();engine.advance_dialogue().unwrap();
    let before=serde_json::to_vec(&engine.state).unwrap();let publications=engine.renderer.batches.lock().unwrap().len();
    assert!(engine.step_silent().is_err());assert_eq!(serde_json::to_vec(&engine.state).unwrap(),before);assert_eq!(engine.renderer.batches.lock().unwrap().len(),publications);
    let mut engine=at_add(&fixture);engine.renderer.reject.store(true,Ordering::SeqCst);let before=serde_json::to_vec(&engine.state).unwrap();let publications=engine.renderer.batches.lock().unwrap().len();
    assert!(engine.step_silent().is_err());assert_eq!(serde_json::to_vec(&engine.state).unwrap(),before);assert_eq!(engine.renderer.batches.lock().unwrap().len(),publications);
    // An actual authored random producer is captured once by the reached
    // consumer. Refused publication keeps its RNG private; retry publishes
    // exactly the deterministic single-gather result, not a second draw.
    let fixture=fixture_with_input(0,false,false,0,true);let mut engine=at_add(&fixture);
    let Some(Statement::ScheduledConsumer{steps,..})=engine.script.get(engine.state.pc)else{panic!("actual reached viewport consumer")};
    assert_eq!(steps.iter().filter(|step|matches!(step,rvn_parser::ConsumerStep::Capture{value:rvn_parser::Expr::Call{name,..},..}if name=="random")).count(),1);
    let mut expected=engine.state.random;let layer=expected.integer(1,9).unwrap();
    engine.renderer.reject.store(true,Ordering::SeqCst);let before=serde_json::to_vec(&engine.state).unwrap();let publications=engine.renderer.batches.lock().unwrap().len();
    assert!(engine.step_silent().is_err());assert_eq!(serde_json::to_vec(&engine.state).unwrap(),before);assert_eq!(engine.renderer.batches.lock().unwrap().len(),publications);
    engine.renderer.reject.store(false,Ordering::SeqCst);engine.step_silent().unwrap();assert_eq!(engine.state.random,expected);assert_eq!(engine.renderer.batches.lock().unwrap().len(),publications+1);
    assert_eq!(i64::from(engine.state.ui.screens.iter().find(|screen|screen.widget_class.is_some()).unwrap().layer),layer);
}
#[test]
fn viewport1003_detached_save15_cold_rebind_keeps_object_and_revokes_the_old_handle(){
    let fixture=fixture(i64::from(i32::MAX),false,false);let mut engine=at_add(&fixture);engine.step_silent().unwrap();engine.step_until_interaction().unwrap();engine.advance_dialogue().unwrap();engine.step_silent().unwrap();engine.step_until_interaction().unwrap();
    let old=engine.widget_reference_handle(&engine.state.vars["instance"]).unwrap();
    let save=engine.capture_save15(&fixture.program,None,1,"detached Widget viewport".into()).unwrap();let save=crate::save::consumer_checkpoint::Save15::decode(&save.encode().unwrap()).unwrap();
    let startup=Engine::prepare_widget_class_startup(fixture.program.clone(),fixture.classes.clone(),Host::default(),16,Default::default(),Default::default()).unwrap();let(mut cold,_)=startup.restore_completed_save15(save).unwrap();
    assert!(cold.add_widget_to_viewport(&old,false,0).is_err());
    let fresh=cold.widget_reference_handle(&cold.state.vars["instance"]).unwrap();assert_ne!(fresh,old);
    cold.add_widget_to_viewport(&fresh,false,i32::MAX).unwrap();assert!(cold.state.ui.screens.iter().find(|screen|screen.name==fresh.alias()).unwrap().viewport_attached());
}
