use rvn_native_source_binding::FrozenNativeSource;
use rvn_graph::{SourceWorkspace,NodeKind,PropertyValue};
use rvn_core::{consumer_schedule::ConsumerSourceBinding,Engine,TerminalRenderer};
use std::{fs,path::PathBuf};

fn folder(name:&str)->PathBuf {
    let root=std::env::var("RVN_NATIVE_SOURCE_TEST_DIR").map(PathBuf::from).unwrap_or_else(|_|std::env::temp_dir().join("rvn-native-binding-0292"));
    fs::create_dir_all(&root).unwrap();
    let dir=root.join(format!("{name}-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));fs::create_dir(&dir).unwrap();dir
}
fn link(name:&str)->(PathBuf,SourceWorkspace) {
    let root=folder(name);let source=include_str!("../../rvn_graph/tests/fixtures/native_source_single.rvn");
    fs::write(root.join("main.rvn"),format!("{source}\nlabel start\nset outputs=probe()\n\"done\"\n")).unwrap();
    let workspace=SourceWorkspace::link_native_preparation(&root,&root.join("main.rvn"),&[]).unwrap();(root,workspace)
}
#[test]
fn writer_freeze_core_binding_and_real_engine_share_exact_program_fingerprints() {
    let (_,workspace)=link("fingerprints");let frozen=FrozenNativeSource::from_workspace(&workspace).unwrap();
    assert_eq!(*frozen.binding(),ConsumerSourceBinding::from_program(frozen.program()).unwrap());
    assert_eq!(frozen.compiler_map().emitted,frozen.binding().emitted);
    assert_eq!(frozen.compiler_map().closure,frozen.binding().closure);
    assert_eq!(frozen.compiler_map().content,frozen.binding().content);
    let native=frozen.sites();assert_eq!(native.len(),1);
    let site=frozen.compiler_map().sites.iter().find(|site|site.origin.graph.as_ref().is_some_and(|graph|graph.node==Some(5))).unwrap();
    assert_eq!(site.origin.graph.as_ref().unwrap().source.as_ref(),Some(&native[0].source));
    assert_eq!(site.origin.graph.as_ref().unwrap().graph,900);
    assert_eq!(site.declaration.graph.as_ref().unwrap().node,Some(1));
    let mut engine=frozen.into_engine(TerminalRenderer,4).unwrap();
    engine.step_until_interaction().unwrap();assert!(engine.state.vars.contains_key("outputs"));
}
#[test]
fn stale_binding_after_real_writer_edit_is_rejected_before_execution() {
    let (_,mut workspace)=link("stale-binding");let old=FrozenNativeSource::from_workspace(&workspace).unwrap();
    let binding=old.binding().clone();let mut edited=workspace.project().graphs().to_vec();
    let graph=edited.iter_mut().find(|graph|!graph.inspector_evaluation.is_legacy()).unwrap();
    let table=graph.nodes.values().find(|node|node.kind==NodeKind::TextFromStringTable).unwrap().id;
    graph.set_pin_default(table,"key",PropertyValue::String("updated".into())).unwrap();workspace.save(&edited).unwrap();
    let new=FrozenNativeSource::from_workspace(&workspace).unwrap();assert_ne!(new.binding(),&binding);
    assert!(Engine::from_program_with_consumer_binding(new.program().clone(),binding,TerminalRenderer,4).is_err());
}
#[test]
fn foreign_program_frame_and_legacy_checkpoint_are_refused_atomically() {
    let (_,workspace)=link("stale-frame");let frozen=FrozenNativeSource::from_workspace(&workspace).unwrap();
    let old=frozen.into_engine(TerminalRenderer,4).unwrap();
    let (_,mut other)=link("stale-frame-other");let mut edited=other.project().graphs().to_vec();
    let graph=edited.iter_mut().find(|graph|!graph.inspector_evaluation.is_legacy()).unwrap();
    let table=graph.nodes.values().find(|node|node.kind==NodeKind::TextFromStringTable).unwrap().id;
    graph.set_pin_default(table,"key",PropertyValue::String("different program".into())).unwrap();other.save(&edited).unwrap();
    let mut current=FrozenNativeSource::from_workspace(&other).unwrap().into_engine(TerminalRenderer,4).unwrap();
    current.state.consumer_frame=old.state.consumer_frame.clone();
    let before=serde_json::to_value(&current.state).unwrap();
    assert!(current.step_silent().is_err());assert_eq!(serde_json::to_value(&current.state).unwrap(),before);
    let mut legacy=Engine::new(rvn_parser::parse("label start\n\"legacy\"").unwrap(),TerminalRenderer,4).unwrap();
    legacy.step_until_interaction().unwrap();
    let checkpoint=rvn_core::save::SaveData::from_state(&legacy.state,1,"legacy".into(),"legacy.rvn".into());
    let before=serde_json::to_value(&current.state).unwrap();assert!(current.load_data(checkpoint).is_err());
    assert_eq!(serde_json::to_value(&current.state).unwrap(),before);
}
#[test]
fn source_comments_and_inclusion_revision_are_part_of_existing_core_closure() {
    let (root,workspace)=link("closure");let old=FrozenNativeSource::from_workspace(&workspace).unwrap();
    let source=fs::read_to_string(root.join("main.rvn")).unwrap();
    fs::write(root.join("main.rvn"),format!("// source revision\n{source}")).unwrap();
    assert!(FrozenNativeSource::from_workspace(&workspace).is_err());
    let current=SourceWorkspace::open_native_preparation(&root).unwrap().unwrap();let new=FrozenNativeSource::from_workspace(&current).unwrap();
    assert_eq!(old.binding().emitted,new.binding().emitted);
    assert_ne!(old.binding().closure,new.binding().closure);
    assert_ne!(old.binding().content,new.binding().content);
    assert!(Engine::from_program_with_consumer_binding(new.program().clone(),old.binding().clone(),TerminalRenderer,4).is_err());
}

#[test]
fn portable_receipt_survives_directory_move_and_rejects_changed_plan_bytes_or_order() {
    let (root,workspace)=link("portable-original");let old=FrozenNativeSource::from_workspace(&workspace).unwrap();
    let receipt=old.portable_receipt();let moved=folder("portable-moved");
    // No presentation sidecar is copied. Source .rvn reconstructs the complete
    // native graph identities and executable plan in a cold directory.
    fs::write(moved.join("main.rvn"),fs::read(root.join("main.rvn")).unwrap()).unwrap();
    let cold=SourceWorkspace::link_native_preparation(&moved,&moved.join("main.rvn"),&[]).unwrap();
    let current=FrozenNativeSource::from_workspace(&cold).unwrap();
    assert_eq!(old.binding().emitted,current.binding().emitted);
    assert_eq!(old.binding().content,current.binding().content);
    assert_ne!(old.binding().closure,current.binding().closure);
    assert!(current.accepts_portable_receipt(&receipt));
    let mut forged=receipt.clone();forged.plans[0].consumer+=1;assert!(!current.accepts_portable_receipt(&forged));
    let mut forged=receipt.clone();forged.source_bytes_in_inclusion_order[0].push_str("// forged");assert!(!current.accepts_portable_receipt(&forged));
    let mut forged=receipt.clone();forged.root_source_ordinals[0]+=1;assert!(!current.accepts_portable_receipt(&forged));
    let mut forged=receipt.clone();forged.plans[0].scope.push('x');assert!(!current.accepts_portable_receipt(&forged));
    let json=serde_json::to_string(&receipt).unwrap();let decoded=serde_json::from_str(&json).unwrap();assert!(current.accepts_portable_receipt(&decoded));
}
