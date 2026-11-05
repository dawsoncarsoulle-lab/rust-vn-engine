use rvn_graph::*;
use rvn_native_source_binding::FrozenNativeSource;
fn wire(g:&mut GraphDocument,a:NodeId,ak:&str,b:NodeId,bk:&str){g.connect(g.pin_by_key(a,ak).unwrap().id,g.pin_by_key(b,bk).unwrap().id).unwrap();}
#[test]
fn authoritative_narrative_branch_source_freezes_all_plans_and_maps_real_runtime_sites(){
 let dir=std::path::PathBuf::from(std::env::var("RVN_MIXED_BINDING_DIR").expect("private fixture output required")).join(format!("binding-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));std::fs::create_dir_all(&dir).unwrap();
 let mut graph=GraphDocument::new(GraphId::new(970),GraphKind::Label{name:"start".into()});
 let root=graph.add_catalog_node(NodeKind::Label,[0.,0.]).unwrap();let inspector=graph.add_catalog_node(NodeKind::GetStringTableIdAndKey,[100.,0.]).unwrap();let branch=graph.add_catalog_node(NodeKind::If,[300.,0.]).unwrap();
 wire(&mut graph,root,"exec_out",branch,"exec_in");wire(&mut graph,inspector,"valid",branch,"condition");
 let command=graph.add_catalog_node(NodeKind::AccessibilitySpeak,[500.,0.]).unwrap();wire(&mut graph,branch,"then",command,"exec_in");wire(&mut graph,inspector,"key",command,"text");let end=graph.add_branch_end(branch,[700.,0.]).unwrap();wire(&mut graph,command,"exec_out",end,"exec_in");
 let source=prepare_native_frame_graph(&graph).unwrap().source;let main=dir.join("main.rvn");std::fs::write(&main,&source).unwrap();
 let workspace=SourceWorkspace::link_native_preparation(&dir,&main,&[]).unwrap();let frozen=FrozenNativeSource::from_workspace(&workspace).unwrap();
 assert_eq!(frozen.sites().len(),3);assert!(frozen.unmapped_compiler_sites().is_empty());
 let receipt=frozen.portable_receipt();assert_eq!(receipt.plans.len(),3);assert!(frozen.accepts_portable_receipt(&receipt));
 // Actual source-bound Core0359 admits v2; the plan remains recalculated.
 let mut engine=frozen.into_engine(rvn_core::TerminalRenderer,4).unwrap();engine.enter_label("start").unwrap();engine.preview_step_statement().unwrap();
 let cold=SourceWorkspace::open_native_preparation(&dir).unwrap().unwrap();let frozen=FrozenNativeSource::from_workspace(&cold).unwrap();assert!(frozen.accepts_portable_receipt(&receipt));
}
