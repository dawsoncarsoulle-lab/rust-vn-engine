//! Actual project loader and Bevy wrapper backend; no native pixels/OS input.
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
fn authored_project(refuse:bool)->Project{
    let project=Project::new(false);fs::remove_file(project.root.join(".rvn-authoring.json")).unwrap();
    let source=LIFECYCLE.replace("label start","scene label start").replace("set rooted=false","set rooted=true")
        .replace("\"Ready\"\nset rooted=true\n\"Rooted\"\nset rooted=true\n\"Empty\"\nset refuse=true\n\"Refuse\"\nreturn",
            "set instance=0\n\"Ready\"\n\"Shown\"\n\"Detached\"\n\"Reattached\"\nreturn");
    let source=if refuse{source.replacen("set refuse=false","set refuse=true",1)}else{source};
    let path=project.root.join("main.rvn");fs::write(&path,&source).unwrap();
    let mut source_project=rvn_graph::SourceProject::open_at(&path,&source,&[]).unwrap();let mut graphs=source_project.graphs().to_vec();
    let graph=graphs.iter_mut().find(|graph|matches!(&graph.kind,GraphKind::Label{name}if name=="start")).unwrap();graph.enable_native_frame_preparation().unwrap();
    let assignment=graph.nodes.values().find(|node|node.kind==NodeKind::SetVariable&&node.properties.get("name")==Some(&PropertyValue::String("instance".into()))).unwrap().id;
    let factory=graph.replace_assignment_with_widget_factory(assignment,&rvn_parser::Expr::Call{name:"__rvn_widget_class_v1".into(),args:vec![rvn_parser::Expr::Str(UUID.into())]},&rvn_parser::Expr::ListLit(vec![rvn_parser::Expr::Int(1)])).unwrap();
    for(text,kind,layer)in [("Shown",NodeKind::WidgetAddToViewport,i32::MIN),("Detached",NodeKind::WidgetRemoveFromParent,0),("Reattached",NodeKind::WidgetAddToViewport,i32::MAX)]{
        let before=dialogue_with_literal(graph,text);
        let input=graph.pin_by_key(before,"exec_in").unwrap().id;let edge=graph.edges.values().find(|edge|edge.input==input).unwrap().clone();graph.remove_edge(edge.id);
        let effect=graph.add_catalog_node(kind,[300.,0.]).unwrap();graph.connect(edge.output,graph.pin_by_key(effect,"exec_in").unwrap().id).unwrap();wire(graph,effect,"exec_out",before,"exec_in");wire(graph,factory,"value_out",effect,"object");
        if kind==NodeKind::WidgetAddToViewport{graph.set_pin_default(effect,"z_order",PropertyValue::Int(i64::from(layer))).unwrap();}
    }
    source_project.apply_visual(&source,&graphs).unwrap();let source=source_project.source().to_owned();fs::write(&path,&source).unwrap();
    // Final source and descriptor are linked before the actual loader; no
    // source metadata or object is substituted after program/class admission.
    let parsed=rvn_graph::SourceProject::open_at(&path,&source,&[]).unwrap();
    let workspace=rvn_graph::SourceWorkspace::link(&project.root,&path,parsed.graphs()).unwrap();
    let screen=workspace.project().graphs().iter().find(|graph|matches!(&graph.kind,GraphKind::Screen{name}if name=="panel")).unwrap();let id=WidgetGraphRef16::from_u64(screen.graph_id.get());
    let members=workspace.project().graphs().iter().filter(|graph|matches!(&graph.kind,GraphKind::Screen{name}if name=="panel")||matches!(&graph.kind,GraphKind::Handler{..}|GraphKind::Function{..})).map(|graph|WidgetGraphRef16::from_u64(graph.graph_id.get())).collect();
    fs::write(project.root.join("assets/panel.rvnwidget"),WidgetBlueprintV1{uuid:UUID.into(),source:"main.rvn".into(),screen_graph:id,members}.encode().unwrap()).unwrap();project
}
#[test]
fn viewport1003_actual_scene_effects_construct_detach_and_reattach_the_same_bevy_object(){
    let project=authored_project(false);let mut engine=load_game(&project.root).unwrap().engine;engine.step_until_interaction().unwrap();
    assert_eq!(engine.state.vars["initialized_count"],Value::Int(1));
    let screen=engine.state.ui.screens.iter().find(|screen|screen.widget_class.is_some()).unwrap();let alias=screen.name.clone();let handle=engine.widget_instance_handle(&alias).unwrap();let source=engine.widget_backend_owner(&handle).unwrap();
    assert!(engine.renderer.widget_backend.wrapper(&source).is_none());engine.advance_dialogue().unwrap();engine.step_silent().unwrap();
    let weak=engine.renderer.widget_backend.wrapper(&source).unwrap();let first=weak.upgrade().unwrap();assert!(first.attached());assert!(first.prepared_node().is_ok());let generation=first.generation();
    assert_eq!(engine.state.ui.screens.iter().find(|screen|screen.name==alias).unwrap().layer,i32::MIN);assert_eq!(engine.interface_views().unwrap().len(),1);
    let child=engine.interface_views().unwrap().into_iter().find(|view|view.name==alias).unwrap().root;
    assert_eq!(engine.state.vars["pre_count"],Value::Int(1));assert_eq!(engine.state.vars["constructed_count"],Value::Int(1));
    engine.step_until_interaction().unwrap();engine.advance_dialogue().unwrap();engine.step_silent().unwrap();
    assert!(!first.attached());assert!(engine.renderer.widget_backend.wrapper(&source).is_none());assert_eq!(engine.state.vars["destructed_count"],Value::Int(1));
    assert_eq!(engine.widget_instance_handle(&alias).unwrap(),handle);assert!(engine.interface_views().unwrap().is_empty());drop(first);assert!(weak.upgrade().is_none());
    engine.step_until_interaction().unwrap();engine.advance_dialogue().unwrap();engine.step_silent().unwrap();
    let second=engine.renderer.widget_backend.wrapper(&source).unwrap().upgrade().unwrap();assert!(second.attached());assert!(second.generation()>generation);assert!(second.prepared_node().is_ok());
    assert_eq!(engine.state.ui.screens.iter().find(|screen|screen.name==alias).unwrap().layer,i32::MAX);assert_eq!(engine.state.vars["initialized_count"],Value::Int(1));assert_eq!(engine.state.vars["constructed_count"],Value::Int(2));
    let view=engine.interface_views().unwrap().into_iter().find(|view|view.name==alias).unwrap();assert_eq!(view.layer,i32::MAX);assert_eq!(view.root,child,"global viewport order does not rewrite the authored child tree/slots");
}
#[test]
fn viewport1003_actual_bevy_construct_callback_refusal_leaves_no_wrapper_or_partial_scene(){
    let project=authored_project(true);let mut engine=load_game(&project.root).unwrap().engine;engine.step_until_interaction().unwrap();engine.advance_dialogue().unwrap();
    let screen=engine.state.ui.screens.iter().find(|screen|screen.widget_class.is_some()).unwrap();let handle=engine.widget_instance_handle(&screen.name).unwrap();let source=engine.widget_backend_owner(&handle).unwrap();
    let before=serde_json::to_vec(&engine.state).unwrap();assert!(engine.step_silent().is_err());assert_eq!(serde_json::to_vec(&engine.state).unwrap(),before);
    assert!(engine.renderer.widget_backend.wrapper(&source).is_none());assert_eq!(engine.state.vars["pre_count"],Value::Int(0));assert_eq!(engine.state.vars["constructed_count"],Value::Int(0));
}
