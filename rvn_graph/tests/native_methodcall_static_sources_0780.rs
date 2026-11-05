//! Real source/Graph roundtrips only. No Engine/video/native/pixel acceptance.
use rvn_graph::*;
use rvn_parser::{ConsumerStep,Statement};

fn imported(body:&str)->GraphDocument{
    let parsed=rvn_parser::parse_program(&format!("scene label start\n{body}\n\"boundary\"\n")).unwrap();
    let g=import_script(&parsed.script).unwrap().into_iter().find(|g|matches!(&g.kind,GraphKind::Label{name}if name=="start")).unwrap();
    let mut g=GraphDocument::from_json(&g.to_pretty_json().unwrap()).unwrap();
    g.enable_native_frame_preparation().unwrap();g
}
fn method(g:&GraphDocument)->NodeId{g.nodes.values().find(|n|n.kind==NodeKind::MethodCall).unwrap().id}
fn inline(arg:&str)->GraphDocument{
    let mut g=GraphDocument::new(GraphId::new(780),GraphKind::Label{name:"start".into()});
    let root=g.add_catalog_node(NodeKind::Label,[0.,0.]).unwrap();
    let call=g.add_catalog_node(NodeKind::MethodCall,[400.,0.]).unwrap();
    for(key,value)in[("target","intro"),("method","pause"),("arg",arg),("transition","none")]{g.set_pin_default(call,key,PropertyValue::String(value.into())).unwrap();}
    g.connect(g.pin_by_key(root,"exec_out").unwrap().id,g.pin_by_key(call,"exec_in").unwrap().id).unwrap();
    g.enable_native_frame_preparation().unwrap();g
}
fn exact(g:&GraphDocument)->TranspiledScriptWithOrigins{
    let before=g.clone();let emitted=prepare_native_frame_graph(g).unwrap();assert_eq!(g,&before);
    assert_eq!(rvn_parser::parse_program(&emitted.source).unwrap().script,emitted.ast);
    let restored=import_native_frame_script(&emitted.ast).unwrap().into_iter().find(|r|r.graph_id==g.graph_id).unwrap();
    assert_eq!(restored.edges,g.edges);assert_eq!(restored.nodes.keys().collect::<Vec<_>>(),g.nodes.keys().collect::<Vec<_>>());assert_eq!(restored.pins.keys().collect::<Vec<_>>(),g.pins.keys().collect::<Vec<_>>());
    assert_eq!(prepare_native_frame_graph(&restored).unwrap().ast,emitted.ast);emitted
}
fn call_plan(script:&mut[Statement])->&mut Statement{
    script.iter_mut().find(|s|matches!(s,Statement::ScheduledConsumer{body,..}if matches!(body.as_slice(),[Statement::MethodCall{..}]))).unwrap()
}
#[test]
fn method0780_materialized_target_method_argument_transition_keep_actual_edges(){
    let g=imported("intro.pause(\"nonempty\") with fade(777)");let n=method(&g);
    for role in ["target","method","arg","transition"]{let p=g.pin_by_key(n,role).unwrap();assert!(g.edges.values().any(|e|e.input==p.id));}
    let mut emitted=exact(&g);let Statement::ScheduledConsumer{consumer,body,..}=call_plan(&mut emitted.ast)else{unreachable!()};assert_eq!(consumer.matches('~').count(),4);
    assert!(matches!(body.as_slice(),[Statement::MethodCall{target,method,arg:Some(arg),transition:rvn_parser::Transition::Fade{duration_ms:777}}]if target=="intro"&&method=="pause"&&arg=="nonempty"));
}
#[test]
fn method0780_inline_defaults_and_none_argument_do_not_invent_sources(){
    for arg in ["","nonempty"]{let g=inline(arg);let n=method(&g);for role in ["target","method","arg","transition"]{assert!(!g.edges.values().any(|e|e.input==g.pin_by_key(n,role).unwrap().id));}
        let mut emitted=exact(&g);let Statement::ScheduledConsumer{consumer,body,..}=call_plan(&mut emitted.ast)else{unreachable!()};assert!(!consumer.contains('~'));
        assert!(matches!(body.as_slice(),[Statement::MethodCall{arg:actual,..}]if actual.as_deref()==if arg.is_empty(){None}else{Some(arg)}));}
}
#[test]
fn method0780_connected_empty_placeholder_keeps_its_wire_and_zero_argument_semantics(){
    let mut g=inline("");let n=method(&g);let literal=g.add_catalog_node(NodeKind::Literal,[100.,100.]).unwrap();g.set_property(literal,"value",PropertyValue::String(String::new())).unwrap();
    g.connect(g.pin_by_key(literal,"value").unwrap().id,g.pin_by_key(n,"arg").unwrap().id).unwrap();
    let mut emitted=exact(&g);let Statement::ScheduledConsumer{consumer,body,..}=call_plan(&mut emitted.ast)else{unreachable!()};assert!(consumer.contains("arg=")&&consumer.contains('~'));assert!(matches!(body.as_slice(),[Statement::MethodCall{arg:None,..}]));
    assert!(import_script(&rvn_parser::parse("label start\nintro.pause(\"\")\n").unwrap()).unwrap_err().contains("explicit empty argument"));
}
#[test]
fn method0780_rerouted_static_fields_preserve_each_real_source_output(){
    let mut g=imported("intro.pause(\"nonempty\") with fade(777)");let n=method(&g);
    for role in ["target","method","arg","transition"]{
        let input=g.pin_by_key(n,role).unwrap().id;let edge=g.edges.values().find(|e|e.input==input).unwrap().clone();let ty=g.effective_pin_type(edge.output).unwrap();
        let reroute=g.add_catalog_node(NodeKind::Reroute,[200.,200.]).unwrap();g.set_reroute_type(reroute,ty).unwrap();assert!(g.remove_edge(edge.id).is_some());
        g.connect(edge.output,g.pin_by_key(reroute,"value").unwrap().id).unwrap();g.connect(g.pin_by_key(reroute,"value_out").unwrap().id,input).unwrap();
    }exact(&g);
}
#[test]
fn method0780_changed_body_target_method_argument_or_transition_refuses_original_sources(){
    let emitted=exact(&imported("intro.pause(\"original\") with fade(777)"));
    for field in 0..4{let mut forged=emitted.ast.clone();let Statement::ScheduledConsumer{body,..}=call_plan(&mut forged)else{unreachable!()};let Statement::MethodCall{target,method,arg,transition}=&mut body[0]else{unreachable!()};
        match field{0=>*target="other".into(),1=>*method="stop".into(),2=>*arg=Some("changed".into()),_=>*transition=rvn_parser::Transition::None};assert!(import_native_frame_script(&forged).is_err());}
}
#[test]
fn method0780_missing_actual_capture_cannot_borrow_an_equal_string_value(){
    let g=imported("intro.pause(\"intro\")");let n=method(&g);let input=g.pin_by_key(n,"target").unwrap().id;let edge=g.edges.values().find(|e|e.input==input).unwrap();let producer=g.pins[&edge.output].node;
    let emitted=exact(&g);let mut forged=emitted.ast.clone();let Statement::ScheduledConsumer{steps,..}=call_plan(&mut forged)else{unreachable!()};
    let prefix=format!("n{}:",producer.get());let before=steps.len();steps.retain(|s|!matches!(s,ConsumerStep::Capture{producer,..}if producer.starts_with(&prefix)));assert_eq!(before,steps.len()+1);
    assert!(steps.iter().any(|s|matches!(s,ConsumerStep::Capture{value:rvn_parser::Expr::Str(value),..}if value=="intro")));assert!(import_native_frame_script(&forged).is_err());
}
#[test]
fn method0780_wrong_source_output_and_forged_consumer_kind_are_refused(){
    let emitted=exact(&imported("intro.pause()"));
    for kind in [false,true]{let mut forged=emitted.ast.clone();let Statement::ScheduledConsumer{consumer,..}=call_plan(&mut forged)else{unreachable!()};
        if kind{assert!(consumer.contains(":method_call|"));*consumer=consumer.replacen(":method_call|",":music_play|",1);}else{let begin=consumer.find('~').unwrap()+1;let end=begin+consumer[begin..].bytes().take_while(u8::is_ascii_digit).count();consumer.replace_range(begin..end,"1000000");}
        assert!(import_native_frame_script(&forged).is_err());}
}
