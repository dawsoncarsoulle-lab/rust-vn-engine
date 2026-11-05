//! Prepared contracts for the existing REROUTE-S01..S12 mechanisms.
use rvn_graph::*;
fn graph()->GraphDocument {GraphDocument::new(GraphId::new(1003111),GraphKind::Init)}
fn add(g:&mut GraphDocument,kind:NodeKind)->NodeId {g.add_catalog_node(kind,[0.,0.]).unwrap()}
fn pin(g:&GraphDocument,n:NodeId,key:&str)->PinId {g.pin_by_key(n,key).unwrap().id}
fn route(g:&mut GraphDocument)->NodeId {add(g,NodeKind::Reroute)}
fn literal(g:&mut GraphDocument,v:PropertyValue)->NodeId {let n=add(g,NodeKind::Literal);g.set_property(n,"value",v).unwrap();n}
fn wire(g:&mut GraphDocument,a:NodeId,ak:&str,b:NodeId,bk:&str)->EdgeId {g.connect(pin(g,a,ak),pin(g,b,bk)).unwrap()}
fn assert_route(g:&GraphDocument,n:NodeId,ty:ValueType){
    assert_eq!(g.reroute_constraint_type(n),Some(ty.clone()));
    for key in ["value","value_out"] {assert_eq!(g.pin_constraint_type(pin(g,n,key)),Some(ty.clone()));}
}
#[test]
fn reroute1003_orphan_historical_caches_project_wildcard_without_load_draw_or_json_migration(){
    for cached in [ValueType::Execution,ValueType::String,ValueType::List(Box::new(ValueType::Name)),ValueType::Asset(AssetKind::Sprite)] {
        let mut g=graph();let r=route(&mut g);g.set_reroute_type(r,cached.clone()).unwrap();let json=g.to_pretty_json().unwrap();
        let loaded=GraphDocument::from_json(&json).unwrap();assert_eq!(loaded,g);assert_route(&loaded,r,ValueType::Any);
        for key in ["value","value_out"] {let p=pin(&loaded,r,key);assert_eq!(loaded.effective_pin_type(p),Some(ValueType::Any));assert_eq!(loaded.pins[&p].value_type,cached);assert!(!loaded.resolved_pin_is_execution(p));}
        let _=loaded.effective_pin_types();let _=loaded.reroute_constraint_types();assert_eq!(loaded.to_pretty_json().unwrap(),json);
    }
}
#[test]
fn reroute1003_consumer_first_types_both_ports_and_the_whole_chain_without_inventing_unknown_values(){
    for (kind,ty) in [(NodeKind::ConvertStringToText,ValueType::String),(NodeKind::ConvertIntToFloat,ValueType::Int),(NodeKind::ConvertBoolToInt,ValueType::Bool),(NodeKind::ConvertTextToString,ValueType::InterpolatedText),(NodeKind::ConvertNameToString,ValueType::Name)] {
        let mut g=graph();let a=route(&mut g);let b=route(&mut g);let consumer=add(&mut g,kind);
        wire(&mut g,a,"value_out",b,"value");let edge=wire(&mut g,b,"value_out",consumer,"value");
        for r in [a,b] {assert_route(&g,r,ty.clone());for key in ["value","value_out"] {assert_eq!(g.pins[&pin(&g,r,key)].value_type,ty);}}
        g.remove_edge(edge).unwrap();for r in [a,b] {assert_route(&g,r,ValueType::Any);}
    }
    let mut g=graph();let unknown=add(&mut g,NodeKind::FunctionCall);g.set_property(unknown,"function",PropertyValue::String("user_unknown".into())).unwrap();
    let r=route(&mut g);let consumer=add(&mut g,NodeKind::ConvertStringToText);wire(&mut g,unknown,"result",r,"value");wire(&mut g,r,"value_out",consumer,"value");
    assert_route(&g,r,ValueType::String);assert_eq!(g.effective_pin_type(pin(&g,r,"value_out")),Some(ValueType::Any));assert_eq!(g.effective_pin_type(pin(&g,unknown,"result")),Some(ValueType::Any));
    let ret=add(&mut g,NodeKind::Return);assert!(!g.reroute_wildcard_exec_connection(pin(&g,r,"value_out"),pin(&g,ret,"exec_in")));
}
#[test]
fn reroute1003_producer_families_fanout_and_reconnect_update_only_the_affected_net(){
    for v in [PropertyValue::Bool(true),PropertyValue::Int(7),PropertyValue::Float(0.5),PropertyValue::String("a".into()),PropertyValue::StringList(vec!["a".into()])] {
        let mut g=graph();let source=literal(&mut g,v);let ty=g.effective_pin_type(pin(&g,source,"value")).unwrap();let a=route(&mut g);let b=route(&mut g);let c=route(&mut g);let untouched=route(&mut g);g.set_reroute_type(untouched,ValueType::Execution).unwrap();
        let first=wire(&mut g,source,"value",a,"value");let ab=wire(&mut g,a,"value_out",b,"value");let ac=wire(&mut g,a,"value_out",c,"value");
        for r in [a,b,c] {assert_route(&g,r,ty.clone());assert_eq!(g.effective_pin_type(pin(&g,r,"value_out")),Some(ty.clone()));}
        let ids=g.nodes.keys().copied().collect::<Vec<_>>();g.remove_edge(first).unwrap();for r in [a,b,c] {assert_route(&g,r,ValueType::Any);}
        assert_eq!(g.pins[&pin(&g,untouched,"value")].value_type,ValueType::Execution);assert!(g.edges.contains_key(&ab)&&g.edges.contains_key(&ac));assert_eq!(g.nodes.keys().copied().collect::<Vec<_>>(),ids);
        let second=literal(&mut g,PropertyValue::String("new".into()));wire(&mut g,second,"value",a,"value");for r in [a,b,c] {assert_route(&g,r,ValueType::String);}
    }
}
#[test]
fn reroute1003_strict_new_conflicts_refuse_atomically_or_use_the_existing_explicit_converter(){
    let mut g=graph();let r=route(&mut g);let consumer=add(&mut g,NodeKind::ConvertStringToText);wire(&mut g,r,"value_out",consumer,"value");let source=literal(&mut g,PropertyValue::Int(4));let output=pin(&g,source,"value");let input=pin(&g,r,"value");let before=g.clone();
    assert!(matches!(g.connect(output,input),Err(GraphEditError::IncompatibleTypes{..})));assert_eq!(g,before);
    let linked=g.connect_with_conversions(output,input,[20.,30.]).unwrap();assert_eq!(linked.conversions.len(),1);assert_eq!(g.nodes[&linked.conversions[0]].kind,NodeKind::ConvertNumberToText);assert_route(&g,r,ValueType::String);assert!(!g.validate().iter().any(|d|d.code=="incompatible_types"));
    let before=g.clone();assert!(g.connect(pin(&g,r,"value_out"),pin(&g,r,"value")).is_err());assert_eq!(g,before);
}
#[test]
fn reroute1003_unlink_splits_components_and_disconnect_remove_keep_surviving_anchor_roles(){
    let mut g=graph();let source=literal(&mut g,PropertyValue::String("x".into()));let a=route(&mut g);let b=route(&mut g);let consumer=add(&mut g,NodeKind::ConvertStringToText);
    wire(&mut g,source,"value",a,"value");let middle=wire(&mut g,a,"value_out",b,"value");wire(&mut g,b,"value_out",consumer,"value");g.remove_edge(middle).unwrap();assert_route(&g,a,ValueType::String);assert_route(&g,b,ValueType::String);
    g.remove_node(source).unwrap();assert_route(&g,a,ValueType::Any);assert_route(&g,b,ValueType::String);let input=pin(&g,consumer,"value");assert_eq!(g.disconnect_pin(input),1);assert_route(&g,b,ValueType::Any);assert_eq!(g.disconnect_pin(input),0);
}
#[test]
fn reroute1003_exec_can_type_a_neutral_net_without_promoting_ordinary_any_or_data(){
    let mut g=graph();let entry=add(&mut g,NodeKind::Init);let r=route(&mut g);let ret=add(&mut g,NodeKind::Return);let out=pin(&g,entry,"exec_out");let input=pin(&g,r,"value");assert!(g.reroute_wildcard_exec_connection(out,input));g.connect(out,input).unwrap();wire(&mut g,r,"value_out",ret,"exec_in");assert_route(&g,r,ValueType::Execution);
    for key in ["value","value_out"] {assert!(g.resolved_pin_is_execution(pin(&g,r,key)));}
    assert!(!g.validate().iter().any(|d|d.code=="incompatible_types"||d.code=="data_cycle"));
    let data=literal(&mut g,PropertyValue::Int(1));let before=g.clone();assert!(g.connect(pin(&g,data,"value"),pin(&g,ret,"exec_in")).is_err());assert_eq!(g,before);
    let other=route(&mut g);wire(&mut g,data,"value",other,"value");let before=g.clone();assert!(!g.reroute_wildcard_exec_connection(pin(&g,other,"value_out"),pin(&g,ret,"exec_in")));assert!(g.connect(pin(&g,other,"value_out"),pin(&g,ret,"exec_in")).is_err());assert_eq!(g,before);
}
#[test]
fn reroute1003_dynamic_exec_pin_removal_clears_only_its_now_orphaned_network(){
    let mut g=graph();let sequence=add(&mut g,NodeKind::Sequence);g.resize_sequence_outputs(sequence,2).unwrap();let r=route(&mut g);wire(&mut g,sequence,"then_1",r,"value");assert_route(&g,r,ValueType::Execution);g.resize_sequence_outputs(sequence,1).unwrap();assert_route(&g,r,ValueType::Any);for key in ["value","value_out"] {assert_eq!(g.pins[&pin(&g,r,key)].value_type,ValueType::Any);}
}
#[test]
fn reroute1003_historical_cycles_and_large_nets_stay_bounded_neutral_and_read_only(){
    let mut g=graph();let a=route(&mut g);let b=route(&mut g);wire(&mut g,a,"value_out",b,"value");g.edges.insert(EdgeId::new(9000),GraphEdge{id:EdgeId::new(9000),output:pin(&g,b,"value_out"),input:pin(&g,a,"value")});let before=g.clone();assert_route(&g,a,ValueType::Any);assert_route(&g,b,ValueType::Any);assert!(g.validate().iter().any(|d|d.code=="data_cycle"));assert_eq!(g,before);
    let mut large=graph();let source=literal(&mut large,PropertyValue::Int(1));let mut tail=pin(&large,source,"value");let mut last=None;
    for _ in 0..150 {let r=route(&mut large);large.connect(tail,pin(&large,r,"value")).unwrap();tail=pin(&large,r,"value_out");last=Some(r);}
    let r=last.unwrap();large.set_reroute_type(r,ValueType::String).unwrap();let before=large.to_pretty_json().unwrap();assert_eq!(large.effective_pin_type(tail),Some(ValueType::Any));assert_route(&large,r,ValueType::Any);let _=large.reroute_constraint_types();assert_eq!(large.to_pretty_json().unwrap(),before);
}
#[test]
fn reroute1003_legacy_source_is_transparent_and_actual_json_save_preserves_all_ids(){
    for source in ["label start\nreturn\n","function answer() { return 7 }\nlabel start\nreturn\n"] {
        let mut graphs=import_script(&rvn_parser::parse(source).unwrap()).unwrap();let index=if source.starts_with("function") {graphs.iter().position(|g|matches!(g.kind,GraphKind::Function{..})).unwrap()} else {graphs.iter().position(|g|matches!(g.kind,GraphKind::Label{..})).unwrap()};let g=&mut graphs[index];let expected=transpile(g).unwrap().source;
        let edge=g.edges.values().find(|e|g.pins[&e.output].value_type.is_execution()==!source.starts_with("function")).cloned().unwrap();g.remove_edge(edge.id).unwrap();let r=route(g);let rin=pin(g,r,"value");let rout=pin(g,r,"value_out");g.connect(edge.output,rin).unwrap();g.connect(rout,edge.input).unwrap();assert_eq!(transpile(g).unwrap().source,expected);
        let saved=g.to_pretty_json().unwrap();let restored=GraphDocument::from_json(&saved).unwrap();assert_eq!(restored,*g);assert_eq!(restored.to_pretty_json().unwrap(),saved);assert_eq!(transpile(&restored).unwrap().source,expected);
    }
}

fn workspace_fixture()->(std::path::PathBuf,SourceWorkspace){
    let root=std::env::temp_dir().join(format!("rvn-reroute1003-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("rvn.toml"),"[project]\nmain_script='main.rvn'\nstart_label='start'\n").unwrap();std::fs::write(root.join("main.rvn"),"// Reroute source witness\nfunction answer() { return 7 }\nlabel start\nreturn\nlabel neighbour\nreturn\n").unwrap();
    let workspace=SourceWorkspace::link(&root,&root.join("main.rvn"),&[]).unwrap();(root,workspace)
}
#[test]
fn reroute1003_existing_source_workspace_save_and_cold_open_retain_real_graph_pin_edge_ids_and_neighbour(){
    let(root,mut workspace)=workspace_fixture();let before=workspace.project().graphs().to_vec();let mut edited=before.clone();let index=edited.iter().position(|g|matches!(g.kind,GraphKind::Function{..})).unwrap();let g=&mut edited[index];let edge=g.edges.values().find(|e|!g.pins[&e.output].value_type.is_execution()).cloned().unwrap();g.remove_edge(edge.id).unwrap();let r=route(g);g.connect(edge.output,pin(g,r,"value")).unwrap();g.connect(pin(g,r,"value_out"),edge.input).unwrap();let orphan=route(g);g.set_reroute_type(orphan,ValueType::Execution).unwrap();let authored=g.clone();
    workspace.save(&edited).unwrap();let saved=std::fs::read(root.join("main.rvn")).unwrap();drop(workspace);let cold=SourceWorkspace::open(&root).unwrap().unwrap();let actual=cold.project().graphs().iter().find(|g|g.graph_id==authored.graph_id).unwrap();assert_eq!(actual,&authored);assert_route(actual,r,ValueType::Int);assert_route(actual,orphan,ValueType::Any);assert_eq!(actual.pins[&pin(actual,orphan,"value")].value_type,ValueType::Execution);
    for neighbour in before.iter().filter(|g|g.graph_id!=authored.graph_id){assert_eq!(cold.project().graphs().iter().find(|g|g.graph_id==neighbour.graph_id),Some(neighbour));}assert_eq!(std::fs::read(root.join("main.rvn")).unwrap(),saved);assert!(cold.project().source().contains("Reroute source witness"));
}
#[test]
fn reroute1003_existing_source_workspace_cas_refuses_without_publishing_reroute_or_overwriting_external_bytes(){
    let(root,mut workspace)=workspace_fixture();let before=workspace.project().graphs().to_vec();let mut edited=before.clone();let g=edited.iter_mut().find(|g|matches!(g.kind,GraphKind::Label{ref name}if name=="start")).unwrap();let edge=g.edges.values().find(|e|g.pins[&e.output].value_type.is_execution()).cloned().unwrap();g.remove_edge(edge.id).unwrap();let r=route(g);g.connect(edge.output,pin(g,r,"value")).unwrap();g.connect(pin(g,r,"value_out"),edge.input).unwrap();let external=b"// External independent edit\nlabel start\nreturn\n";std::fs::write(root.join("main.rvn"),external).unwrap();assert!(workspace.save(&edited).is_err());assert_eq!(workspace.project().graphs(),before);assert_eq!(std::fs::read(root.join("main.rvn")).unwrap(),external);
}

#[test]
fn reroute1003_malformed_historical_role_cannot_acquire_execution_through_the_neutral_exception(){
    let mut g=graph();let source=add(&mut g,NodeKind::Init);let r=route(&mut g);g.set_reroute_type(r,ValueType::Execution).unwrap();g.add_pin(r,"foreign_role","",PinDirection::Input,ValueType::Execution,PinCardinality::One).unwrap();let before=g.clone();let output=pin(&g,source,"exec_out");let input=pin(&g,r,"value");assert_route(&g,r,ValueType::Any);assert!(!g.reroute_wildcard_exec_connection(output,input));assert!(g.connect(output,input).is_err());assert_eq!(g,before);
}

#[test]
fn reroute1003_wide_valid_fanout_keeps_full_type_without_spending_the_depth_budget_on_siblings(){
    let mut g=graph();let r=route(&mut g);let source=literal(&mut g,PropertyValue::String("wide".into()));wire(&mut g,source,"value",r,"value");
    for _ in 0..140 {let consumer=add(&mut g,NodeKind::ConvertStringToText);wire(&mut g,r,"value_out",consumer,"value");}
    assert_route(&g,r,ValueType::String);for key in ["value","value_out"] {assert_eq!(g.effective_pin_type(pin(&g,r,key)),Some(ValueType::String));assert_eq!(g.pins[&pin(&g,r,key)].value_type,ValueType::String);}assert_eq!(g.edges.len(),141);
}

#[test]
fn reroute1003_historical_anchorless_cycle_refuses_a_new_exec_link_without_mutating_graph_or_counters(){
    let mut g=graph();let source=literal(&mut g,PropertyValue::Int(1));let a=route(&mut g);let retired=wire(&mut g,source,"value",a,"value");let b=route(&mut g);wire(&mut g,a,"value_out",b,"value");g.remove_node(source).unwrap();g.edges.insert(retired,GraphEdge{id:retired,output:pin(&g,b,"value_out"),input:pin(&g,a,"value")});let ret=add(&mut g,NodeKind::Return);let output=pin(&g,b,"value_out");let input=pin(&g,ret,"exec_in");let before=g.clone();assert_route(&g,b,ValueType::Any);assert!(!g.reroute_wildcard_exec_connection(output,input));assert!(!g.reroute_exec_connection_is_resolvable(output,input));assert!(g.connect(output,input).is_err());assert_eq!(g,before);
}
#[test]
fn reroute1003_historical_deep_exec_anchor_cannot_use_neutral_projection_as_a_new_execution_grant(){
    let mut g=graph();let data=literal(&mut g,PropertyValue::Int(1));let first=route(&mut g);let retired=wire(&mut g,data,"value",first,"value");let mut last=first;
    for _ in 1..150 {let next=route(&mut g);wire(&mut g,last,"value_out",next,"value");last=next;}
    g.remove_node(data).unwrap();let entry=add(&mut g,NodeKind::Init);g.edges.insert(retired,GraphEdge{id:retired,output:pin(&g,entry,"exec_out"),input:pin(&g,first,"value")});let ret=add(&mut g,NodeKind::Return);let output=pin(&g,last,"value_out");let input=pin(&g,ret,"exec_in");let before=g.clone();assert_route(&g,last,ValueType::Any);assert!(!g.reroute_wildcard_exec_connection(output,input));assert!(!g.reroute_exec_connection_is_resolvable(output,input));assert!(g.connect_with_conversions(output,input,[20.,40.]).is_err());assert_eq!(g,before);
}
#[test]
fn reroute1003_prospective_exec_merge_and_neutral_extension_refuse_before_crossing_the_actual_depth_limit(){
    let mut g=graph();let entry=add(&mut g,NodeKind::Init);let mut output=pin(&g,entry,"exec_out");
    for _ in 0..80 {let r=route(&mut g);g.connect(output,pin(&g,r,"value")).unwrap();output=pin(&g,r,"value_out");}
    let first=route(&mut g);let mut last=first;for _ in 1..80 {let r=route(&mut g);wire(&mut g,last,"value_out",r,"value");last=r;}
    let ret=add(&mut g,NodeKind::Return);wire(&mut g,last,"value_out",ret,"exec_in");let input=pin(&g,first,"value");assert_eq!(g.connection_output_type(output),Some(ValueType::Execution));assert_route(&g,first,ValueType::Execution);let before=g.clone();assert!(!g.reroute_exec_connection_is_resolvable(output,input));assert!(g.connect(output,input).is_err());assert_eq!(g,before);
    let mut g=graph();let entry=add(&mut g,NodeKind::Init);let mut output=pin(&g,entry,"exec_out");for _ in 0..128 {let r=route(&mut g);g.connect(output,pin(&g,r,"value")).unwrap();output=pin(&g,r,"value_out");}let neutral=route(&mut g);let input=pin(&g,neutral,"value");let before=g.clone();assert!(!g.reroute_wildcard_exec_connection(output,input));assert!(g.connect(output,input).is_err());assert_eq!(g,before);
}
