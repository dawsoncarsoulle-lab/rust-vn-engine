use rvn_graph::*;
fn graph(body:&str)->GraphDocument {
    let p=rvn_parser::parse_program(&format!("scene label start\n{body}\n\"boundary\"\n")).unwrap();
    let mut graphs=import_script(&p.script).unwrap();
    let mut g=graphs.remove(graphs.iter().position(|g|matches!(&g.kind,GraphKind::Label{name}if name=="start")).unwrap());
    g.enable_native_frame_preparation().unwrap();g
}
fn exact(g:&GraphDocument)->TranspiledScriptWithOrigins {
    let generated=prepare_native_frame_graph(g).unwrap();
    let restored=import_native_frame_script(&generated.ast).unwrap();
    let actual=restored.iter().find(|x|x.graph_id==g.graph_id).unwrap();
    assert_eq!(actual.edges,g.edges);
    assert_eq!(actual.nodes.keys().collect::<Vec<_>>(),g.nodes.keys().collect::<Vec<_>>());
    assert_eq!(actual.pins.keys().collect::<Vec<_>>(),g.pins.keys().collect::<Vec<_>>());
    for (id,node) in &g.nodes {assert_eq!(actual.nodes[id].kind,node.kind);}
    generated
}
#[test]fn equal_paths_keep_distinct_real_assets_and_fade_duration(){
    let g=graph("music.play(\"host.wav\") with fade(777)\nmusic.play(\"host.wav\") with fade(777)");
    assert_eq!(g.nodes.values().filter(|n|n.kind==NodeKind::MusicAsset).count(),2);
    let output=exact(&g);assert!(output.source.contains("fade(777)"));
}
#[test]fn one_shared_asset_remains_one_real_output_for_two_consumers(){
    let mut g=graph("music.play(\"host.wav\")\nmusic.play(\"host.wav\")");
    let assets:Vec<_>=g.nodes.values().filter(|n|n.kind==NodeKind::MusicAsset).map(|n|n.id).collect();assert_eq!(assets.len(),2);
    let first=g.pin_by_key(assets[0],"value").unwrap().id;let other=g.pin_by_key(assets[1],"value").unwrap().id;
    let edge=g.edges.values().find(|edge|edge.output==other).unwrap().clone();g.remove_edge(edge.id);g.connect(first,edge.input).unwrap();g.remove_node(assets[1]);
    exact(&g);
}
#[test]fn real_asset_reroute_preserves_all_three_connection_identities(){
    let mut g=graph("sfx.play(\"host.wav\")");let command=g.nodes.values().find(|n|n.kind==NodeKind::SfxPlay).unwrap().id;
    let input=g.pin_by_key(command,"file").unwrap().id;let edge=g.edges.values().find(|e|e.input==input).unwrap().clone();let ty=g.pins[&edge.output].value_type.clone();
    let reroute=g.add_catalog_node(NodeKind::Reroute,[400.0,300.0]).unwrap();g.set_reroute_type(reroute,ty).unwrap();
    let middle_in=g.pin_by_key(reroute,"value").unwrap().id;let middle_out=g.pin_by_key(reroute,"value_out").unwrap().id;
    g.remove_edge(edge.id);g.connect(edge.output,middle_in).unwrap();g.connect(middle_out,input).unwrap();exact(&g);
}
#[test]fn missing_real_output_and_changed_literal_refuse_native_import(){
    let g=graph("music.play(\"host.wav\")");let generated=exact(&g);let mut forged=generated.source.clone();
    let start=forged.find('~').unwrap()+1;let end=start+forged[start..].bytes().take_while(u8::is_ascii_digit).count();forged.replace_range(start..end,"1000000");
    let p=rvn_parser::parse_program(&forged).unwrap();assert!(import_native_frame_script(&p.script).is_err());
    let forged=generated.source.replacen("music.play(\"host.wav\")","music.play(\"other.wav\")",1);assert_ne!(forged,generated.source);
    let p=rvn_parser::parse_program(&forged).unwrap();assert!(import_native_frame_script(&p.script).is_err());
}
