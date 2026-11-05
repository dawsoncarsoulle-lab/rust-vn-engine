use rvn_graph::*;

fn graph(body: &str, boundary: bool) -> GraphDocument {
    let raw = format!("scene label start\n{body}\n{}", if boundary {"\"boundary\"\n"} else {""});
    let parsed = rvn_parser::parse_program(&raw).unwrap();
    let mut g = import_script(&parsed.script).unwrap().into_iter()
        .find(|g|matches!(&g.kind, GraphKind::Label{name} if name=="start")).unwrap();
    g.enable_native_frame_preparation().unwrap();
    g
}
fn connect_literal(g: &mut GraphDocument, kind: NodeKind, key: &str, value: PropertyValue) {
    let owner = g.nodes.values().find(|n| n.kind==kind).unwrap().id;
    let input = g.pin_by_key(owner,key).unwrap().id;
    if let Some(edge)=g.edges.values().find(|e| e.input==input) {
        let source=g.pins[&edge.output].node;
        assert_eq!(g.nodes[&source].kind,NodeKind::Literal);
        g.set_property(source,"value",value).unwrap();
        return;
    }
    let literal = g.add_catalog_node(NodeKind::Literal,[300.0,300.0]).unwrap();
    g.set_property(literal,"value",value).unwrap();
    let output = g.pin_by_key(literal,"value").unwrap().id;
    g.connect(output,input).unwrap();
}
fn exact(g: &GraphDocument) {
    let before=g.clone();
    let prepared=prepare_native_frame_graph(g).unwrap();
    assert_eq!(g,&before);
    let parsed=rvn_parser::parse_program(&prepared.source).unwrap();
    assert_eq!(parsed.script,prepared.ast);
    let graphs=import_native_frame_script(&parsed.script).unwrap();
    let actual=graphs.iter().find(|x|x.graph_id==g.graph_id).unwrap();
    assert_eq!(actual.edges,g.edges);
    assert_eq!(actual.nodes.keys().collect::<Vec<_>>(),g.nodes.keys().collect::<Vec<_>>());
    assert_eq!(actual.pins.keys().collect::<Vec<_>>(),g.pins.keys().collect::<Vec<_>>());
    for (id,node) in &g.nodes {assert_eq!(actual.nodes[id].kind,node.kind);}
}
fn reroute(g:&mut GraphDocument,kind:NodeKind,key:&str) {
    let consumer=g.nodes.values().find(|n|n.kind==kind).unwrap().id;
    let input=g.pin_by_key(consumer,key).unwrap().id;
    let edge=g.edges.values().find(|e|e.input==input).unwrap().clone();
    let ty=g.effective_pin_type(edge.output).unwrap();
    let node=g.add_catalog_node(NodeKind::Reroute,[500.0,300.0]).unwrap();
    g.set_reroute_type(node,ty).unwrap();
    let incoming=g.pin_by_key(node,"value").unwrap().id;
    let outgoing=g.pin_by_key(node,"value_out").unwrap().id;
    assert!(g.remove_edge(edge.id).is_some());
    g.connect(edge.output,incoming).unwrap();g.connect(outgoing,input).unwrap();
}
macro_rules! audio {
    ($name:ident,$raw:expr)=>{#[test]fn $name(){exact(&graph($raw,true));}};
}
audio!(music_play,"music.play(\"host.wav\") with fade(777)");
audio!(music_stop,"music.stop() with fade(777)");
audio!(music_volume,"music.volume(\"0.25\")");
audio!(sfx_play,"sfx.play(\"host.wav\")");
audio!(sfx_stop,"sfx.stop(\"host.wav\")");
audio!(voice_play,"voice \"host.wav\"");

#[test]fn typewriter_boolean_source_is_retained() {
    let mut g=graph("typewriter=true",true);
    connect_literal(&mut g,NodeKind::TypewriterSet,"enabled",PropertyValue::Bool(true));
    exact(&g);
}
#[test]fn typewriter_speed_source_is_retained() {
    let mut g=graph("typewriter.speed(30)",true);
    connect_literal(&mut g,NodeKind::TypewriterSpeed,"speed",PropertyValue::Int(30));
    exact(&g);
}
#[test]fn jump_target_is_actual_label_output() {exact(&graph("jump start",false));}
#[test]fn call_target_is_actual_label_output() {exact(&graph("call start",true));}

#[test]fn boolean_reroute_keeps_source_and_both_edges() {
    let mut g=graph("typewriter=true",true);
    reroute(&mut g,NodeKind::TypewriterSet,"enabled");exact(&g);
}
#[test]fn label_reroute_keeps_typed_target_source() {
    let mut g=graph("jump start",false);
    reroute(&mut g,NodeKind::Jump,"target");exact(&g);
}
#[test]fn integer_reroute_keeps_actual_speed_literal() {
    let mut g=graph("typewriter.speed(30)",true);
    reroute(&mut g,NodeKind::TypewriterSpeed,"speed");exact(&g);
}
#[test]fn equal_boolean_literals_remain_distinct_sources() {
    exact(&graph("typewriter=true\ntypewriter=true",true));
}
#[test]fn one_shared_boolean_output_remains_shared_across_consumers() {
    let mut g=graph("typewriter=true\ntypewriter=true",true);
    let nodes=g.nodes.values().filter(|n|n.kind==NodeKind::TypewriterSet).map(|n|n.id).collect::<Vec<_>>();
    let inputs=nodes.iter().map(|n|g.pin_by_key(*n,"enabled").unwrap().id).collect::<Vec<_>>();
    let first=g.edges.values().find(|e|e.input==inputs[0]).unwrap().output;
    let second=g.edges.values().find(|e|e.input==inputs[1]).unwrap().clone();
    let old=g.pins[&second.output].node;assert!(g.remove_edge(second.id).is_some());
    g.connect(first,second.input).unwrap();assert!(g.remove_node(old).is_some());exact(&g);
}
#[test]fn boolean_operand_change_and_missing_actual_pin_are_refused() {
    let g=graph("typewriter=true",true);
    let compiled=prepare_native_frame_graph(&g).unwrap();
    let wrong=compiled.source.replacen("typewriter = true","typewriter = false",1);
    assert_ne!(wrong,compiled.source);
    let parsed=rvn_parser::parse_program(&wrong).unwrap();
    assert!(import_native_frame_script(&parsed.script).is_err());
    let mut wrong=compiled.source.clone();
    let begin=wrong.find('~').unwrap()+1;
    let end=begin+wrong[begin..].bytes().take_while(u8::is_ascii_digit).count();
    wrong.replace_range(begin..end,"1000000");
    let parsed=rvn_parser::parse_program(&wrong).unwrap();
    assert!(import_native_frame_script(&parsed.script).is_err());
}
