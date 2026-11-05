//! Authoring-only regression matrix of the existing fourteen classic kinds.
//! Real SourceWorkspace/source codec/sidecar; no Engine, VM, renderer or effect.
use super::*;
use crate::{SourceWorkspace, prepare_native_frame_graph, prepare_native_authoring_graph, import_native_frame_script};
fn fixture()->(std::path::PathBuf,SourceWorkspace){
    let root=std::env::temp_dir().join(format!("rvn-classic14-isolated1022-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir_all(&root).unwrap();let path=root.join("main.rvn");std::fs::write(&path,"scene label start\nreturn\n").unwrap();
    let workspace=SourceWorkspace::link(&root,&path,&[]).unwrap();(root,workspace)
}
fn classic(graph:&mut GraphDocument,kind:NodeKind)->NodeId{
    graph.enable_native_frame_preparation().unwrap();graph.add_catalog_node(kind,[420.0,180.0]).unwrap()
}
fn roundtrip(kind:NodeKind){
    let(root,mut workspace)=fixture();let mut edited=workspace.project().graphs().to_vec();
    let graph=edited.iter_mut().find(|graph|matches!(&graph.kind,GraphKind::Label{name}if name=="start")).unwrap();
    let id=classic(graph,kind);let authored=graph.clone();let node=authored.nodes[&id].clone();
    assert!(prepare_native_frame_graph(&authored).is_err(),"strict runtime/source preparation must not authenticate an omitted editor object");
    let compiled=prepare_native_authoring_graph(&authored).unwrap();let plain=import_native_frame_script(&compiled.ast).unwrap();
    let execution=plain.iter().find(|graph|graph.graph_id==authored.graph_id).unwrap();assert!(!execution.nodes.contains_key(&id));
    assert_eq!(execution.edges,authored.edges,"projection must not rewrite any original root wire");
    workspace.save(&edited).unwrap();let mut cold=SourceWorkspace::open(&root).unwrap().unwrap();
    let restored=cold.project().graphs().iter().find(|graph|graph.graph_id==authored.graph_id).unwrap();
    assert_eq!(restored.nodes[&id],node);for pin in &node.pins{assert_eq!(restored.pins[pin],authored.pins[pin]);}
    assert_eq!(restored.edges,authored.edges);assert!(prepare_native_frame_graph(restored).is_err());
    let mut allocating=restored.clone();assert!(allocating.add_catalog_node(NodeKind::TimerCancel,[700.0,180.0]).unwrap().get()>id.get());
    let mut deleted=cold.project().graphs().to_vec();deleted.iter_mut().find(|graph|graph.graph_id==authored.graph_id).unwrap().remove_node(id).unwrap();cold.save(&deleted).unwrap();
    let after=SourceWorkspace::open(&root).unwrap().unwrap();let after=after.project().graphs().iter().find(|graph|graph.graph_id==authored.graph_id).unwrap();
    assert!(!after.nodes.contains_key(&id));for pin in &node.pins{assert!(!after.pins.contains_key(pin));}assert_eq!(after.edges,authored.edges);
    std::fs::remove_dir_all(root).unwrap();
}
macro_rules! family {($name:ident,$kind:ident)=>{#[test]fn $name(){roundtrip(NodeKind::$kind);}}}
family!(classic14_isolated1022_config,Config);
family!(classic14_isolated1022_character_create,CharacterCreate);
family!(classic14_isolated1022_scene,Scene);
family!(classic14_isolated1022_cinematic_show,CinematicShow);
family!(classic14_isolated1022_cinematic_hide,CinematicHide);
family!(classic14_isolated1022_unlock_ending,UnlockEnding);
family!(classic14_isolated1022_sprite_show,SpriteShow);
family!(classic14_isolated1022_sprite_hide,SpriteHide);
family!(classic14_isolated1022_sprite_move,SpriteMove);
family!(classic14_isolated1022_sprite_animate,SpriteAnimate);
family!(classic14_isolated1022_sprite_stop_animation,SpriteStopAnimation);
family!(classic14_isolated1022_sprite_effect,SpriteEffect);
family!(classic14_isolated1022_timer,Timer);
family!(classic14_isolated1022_timer_cancel,TimerCancel);
#[test]
fn classic14_isolated1022_connected_source_wins_over_old_orphan_defaults_and_has_real_codec_identity(){
    let(root,workspace)=fixture();let mut graph=workspace.project().graphs().iter().find(|graph|matches!(&graph.kind,GraphKind::Label{name}if name=="start")).unwrap().clone();
    let id=classic(&mut graph,NodeKind::Config);graph.set_pin_default(id,"key",PropertyValue::String("bg".into())).unwrap();graph.set_pin_default(id,"value",PropertyValue::String("old.png".into())).unwrap();let old=graph.clone();
    let entry=graph.nodes.values().find(|node|node.kind==NodeKind::Label).unwrap().id;let output=graph.pin_by_key(entry,"exec_out").unwrap().id;
    let previous=graph.edges.values().find(|edge|edge.output==output).unwrap().clone();graph.remove_edge(previous.id).unwrap();
    graph.connect(output,graph.pin_by_key(id,"exec_in").unwrap().id).unwrap();graph.connect(graph.pin_by_key(id,"exec_out").unwrap().id,previous.input).unwrap();
    graph.set_pin_default(id,"value",PropertyValue::String("new.png".into())).unwrap();graph.materialize_visible_defaults().unwrap();
    let compiled=prepare_native_authoring_graph(&graph).unwrap();let plain=import_native_frame_script(&compiled.ast).unwrap();let source=plain.iter().find(|actual|actual.graph_id==graph.graph_id).unwrap();
    assert_eq!(source.nodes[&id].kind,NodeKind::Config);assert_eq!(source.nodes[&id].pins,old.nodes[&id].pins);assert_eq!(source.edges,graph.edges);
    let mut restored=source.clone();restore_isolated_macro_presentation(&mut restored,&old).unwrap();assert_eq!(restored,source.clone(),"old disconnected payload must not override the newly source-owned command/default/producer");
    assert!(prepare_native_frame_graph(&restored).is_ok());std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn classic14_isolated1022_wrong_scope_malformed_shape_and_identity_collisions_are_atomic(){
    for kind in [NodeKind::Config,NodeKind::CharacterCreate,NodeKind::Scene,NodeKind::CinematicShow,NodeKind::CinematicHide,NodeKind::UnlockEnding,NodeKind::SpriteShow,NodeKind::SpriteHide,NodeKind::SpriteMove,NodeKind::SpriteAnimate,NodeKind::SpriteStopAnimation,NodeKind::SpriteEffect,NodeKind::Timer,NodeKind::TimerCancel]{
        let(root,workspace)=fixture();let mut old=workspace.project().graphs()[0].clone();let id=classic(&mut old,kind);let execution=native_authoring_execution_projection(&old).unwrap();
        let mut wrong=execution.clone();wrong.graph_id=GraphId::new(old.graph_id.get()+100);let before=wrong.clone();assert!(restore_isolated_macro_presentation(&mut wrong,&old).is_err());assert_eq!(wrong,before);
        let mut bad=old.clone();let pin=bad.nodes[&id].pins[0];bad.pins.get_mut(&pin).unwrap().cardinality=PinCardinality::Many;let before=bad.clone();assert!(prepare_native_authoring_graph(&bad).is_err());assert_eq!(bad,before);
        let mut collision=execution;let pin=old.nodes[&id].pins[0];collision.pins.insert(pin,old.pins[&pin].clone());let before=collision.clone();assert!(restore_isolated_macro_presentation(&mut collision,&old).is_err());assert_eq!(collision,before);
        std::fs::remove_dir_all(root).unwrap();
    }
}
#[test]
fn classic14_isolated1022_disconnected_wired_commands_are_still_outside_this_presentation_slice(){
    let(root,workspace)=fixture();let mut graph=workspace.project().graphs()[0].clone();let first=classic(&mut graph,NodeKind::TimerCancel);let second=graph.add_catalog_node(NodeKind::TimerCancel,[700.0,180.0]).unwrap();
    graph.connect(graph.pin_by_key(first,"exec_out").unwrap().id,graph.pin_by_key(second,"exec_in").unwrap().id).unwrap();let before=graph.clone();
    assert!(prepare_native_authoring_graph(&graph).is_err());assert_eq!(graph,before);std::fs::remove_dir_all(root).unwrap();
}
