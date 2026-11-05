//! Actual Source publication oracles. Defined here; execution belongs to Root.
//! No UI/renderer or native parity is inferred from these filesystem tests.
use super::*;
use crate::{GraphKind,GraphId};
struct Fixture { root:PathBuf }
impl Fixture {
    fn new()->Self{
        static SEQUENCE:std::sync::atomic::AtomicU64=std::sync::atomic::AtomicU64::new(0);
        let root=std::env::temp_dir().join(format!("rvn-source-placement1023-{}-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos(),SEQUENCE.fetch_add(1,std::sync::atomic::Ordering::Relaxed)));
        fs::create_dir_all(root.join("assets/scenes/first")).unwrap();fs::create_dir_all(root.join("assets/scenes/second")).unwrap();
        fs::write(root.join("main.rvn"),"function caption(text){return text}\nscene label start\nreturn\n").unwrap();
        let workspace=SourceWorkspace::link(&root,&root.join("main.rvn"),&[]).unwrap();
        assert!(workspace.source_error.is_none());Self{root}
    }
    fn actual_scene(&self)->GraphId{
        let workspace=SourceWorkspace::open(&self.root).unwrap().unwrap();
        workspace.project().graphs().iter().find(|graph|matches!(&graph.kind,GraphKind::Label{name}if name=="start")).unwrap().graph_id
    }
    fn placements(&self,directory:&str)->serde_json::Value{
        serde_json::json!({"version":1,"entries":[{"graph_id":self.actual_scene(),"namespace":"physical","directory":directory}]})
    }
    fn install_proposed_editorial_v2(&self)->(String,serde_json::Value){
        let placements=self.placements("assets/scenes/first");
        let mut wire:serde_json::Value=serde_json::from_str(&fs::read_to_string(self.root.join(SIDECAR)).unwrap()).unwrap();
        assert_eq!(wire["version"],1);wire["version"]=serde_json::json!(2);wire["asset_placements"]=placements.clone();
        let bytes=serde_json::to_string_pretty(&wire).unwrap();fs::write(self.root.join(SIDECAR),&bytes).unwrap();(bytes,placements)
    }
}
impl Drop for Fixture {fn drop(&mut self){
    let root=self.root.canonicalize().unwrap();let temp=std::env::temp_dir().canonicalize().unwrap();
    assert_eq!(root.parent(),Some(temp.as_path()));assert!(root.file_name().unwrap().to_string_lossy().starts_with("rvn-source-placement1023-"));fs::remove_dir_all(root).unwrap();
}}
#[test]
fn source_placement1023_future_v2_actual_save_and_cold_preserve_editorial_map_without_changing_runtime_source(){
    let f=Fixture::new();let id=f.actual_scene();let (_,placements)=f.install_proposed_editorial_v2();
    let source_before=fs::read(f.root.join("main.rvn")).unwrap();
    let mut workspace=SourceWorkspace::open(&f.root).unwrap().unwrap();assert!(workspace.source_error.is_none());
    let mut graphs=workspace.project().graphs().to_vec();let scene=graphs.iter_mut().find(|graph|graph.graph_id==id).unwrap();
    scene.nodes.values_mut().next().expect("real imported scene root").position=[133.0,277.0];
    workspace.save(&graphs).unwrap();
    let after:serde_json::Value=serde_json::from_str(&fs::read_to_string(f.root.join(SIDECAR)).unwrap()).unwrap();
    assert_eq!(after["version"],2);assert_eq!(after["asset_placements"],placements);
    let cold=SourceWorkspace::open(&f.root).unwrap().unwrap();assert!(cold.source_error.is_none());
    assert_eq!(cold.project().graphs().iter().find(|graph|graph.graph_id==id).unwrap().narrative,Some(crate::NarrativeMetadata::SceneEntry));
    assert_eq!(fs::read(f.root.join("main.rvn")).unwrap(),source_before);
    assert_eq!(fs::read_dir(f.root.join("assets/scenes/first")).unwrap().count(),0,"the graph remains a Source document, not a second legacy story file");
}
#[test]
fn source_placement1023_future_v2_actual_refactor_undo_redo_preserves_the_same_placement_participant(){
    let f=Fixture::new();let (_,placements)=f.install_proposed_editorial_v2();
    let mut workspace=SourceWorkspace::open(&f.root).unwrap().unwrap();
    let id=workspace.project().graphs().iter().find(|graph|matches!(&graph.kind,GraphKind::Function{name}if name=="caption")).unwrap().graph_id;
    let graphs=crate::rename_callable_graphs(workspace.project().graphs(),id,"caption_new").unwrap();
    let receipt=workspace.refactor(&graphs,&[]).unwrap();
    for direction in [None,Some(false),Some(true)]{
        if let Some(redo)=direction{workspace.replay_refactor(&receipt,redo).unwrap();}
        let actual:serde_json::Value=serde_json::from_str(&fs::read_to_string(f.root.join(SIDECAR)).unwrap()).unwrap();
        assert_eq!(actual["version"],2);assert_eq!(actual["asset_placements"],placements);
        let cold=SourceWorkspace::open(&f.root).unwrap().unwrap();assert!(cold.source_error.is_none());
        assert_eq!(cold.project().graphs().iter().filter(|graph|graph.graph_id==id).count(),1);
    }
}
#[test]
fn source_placement1023_future_v3_placement_only_recovery_cannot_be_flattened_by_graph_only_apis(){
    let f=Fixture::new();let (_,placements)=f.install_proposed_editorial_v2();
    let mut workspace=SourceWorkspace::open(&f.root).unwrap().unwrap();let graphs=workspace.project().graphs().to_vec();
    let mut after=placements;after["entries"][0]["directory"]=serde_json::json!("assets/scenes/second");
    let sidecar=fs::read_to_string(f.root.join(SIDECAR)).unwrap();
    let recovery=serde_json::json!({"version":3,"source":"main.rvn","baseline_source":workspace.project().source(),"baseline_sidecar":sidecar,"graphs":graphs,"graph_id_high_water":workspace.graph_id_high_water(),"widgets":[],"widget_memberships":[],"asset_placements_after":after});
    let bytes=serde_json::to_string(&recovery).unwrap();fs::write(f.root.join(RECOVERY),&bytes).unwrap();drop(workspace);
    let mut workspace=SourceWorkspace::open(&f.root).unwrap().unwrap();
    let batch=workspace.recovery_batch().unwrap().expect("placement-only recovery is a real pending typed publication");assert_eq!(batch.graphs,graphs);
    assert!(workspace.recovery().is_err(),"the old Vec<GraphDocument> API may not imply all metadata was retained");
    assert!(workspace.write_recovery(&graphs).is_err(),"legacy graph-only overwrite must not erase the proposed placement");
    assert_eq!(fs::read_to_string(f.root.join(RECOVERY)).unwrap(),bytes);
}

impl Fixture {
    fn workspace(&self)->SourceWorkspace{SourceWorkspace::open(&self.root).unwrap().unwrap()}
    fn map(&self,directory:&str)->SourceAssetPlacements{
        SourceAssetPlacements::from_entries([SourceAssetPlacement{graph_id:self.actual_scene(),namespace:SourcePlacementNamespace::Physical,directory:directory.into()}]).unwrap()
    }
    fn bytes(&self)->(Vec<u8>,Vec<u8>,Option<Vec<u8>>){
        (fs::read(self.root.join("main.rvn")).unwrap(),fs::read(self.root.join(SIDECAR)).unwrap(),fs::read(self.root.join(RECOVERY)).ok())
    }
    fn fresh_scene(&self,workspace:&SourceWorkspace)->(Vec<GraphDocument>,GraphId){
        let next=SourceProject::open_at(&self.root.join("main.rvn"),format!("{}\nscene label added\nreturn\n",workspace.project().source()),workspace.project().graphs()).unwrap();
        let mut graphs=next.graphs().to_vec();
        let graph=graphs.iter_mut().find(|graph|matches!(&graph.kind,GraphKind::Label{name}if name=="added")).unwrap();
        let id=workspace.next_member_graph_id(workspace.project().graphs()).unwrap();graph.graph_id=id;
        assert_eq!(graph.narrative,Some(crate::NarrativeMetadata::SceneEntry));(graphs,id)
    }
}
fn commit_map(workspace:&mut SourceWorkspace,graphs:&[GraphDocument],map:SourceAssetPlacements,fresh:&[GraphId]){
    let expected=workspace.asset_placements().cloned();
    workspace.commit_asset_placement_edit(&expected,graphs,map,fresh,&[],&[],&[],||()).unwrap();
}
#[test]
fn source_placement1023_v1_open_and_clean_save_never_adopt_without_an_editorial_edit(){
    let f=Fixture::new();let before=f.bytes();
    let mut workspace=f.workspace();assert!(workspace.asset_placements().is_none());
    assert_eq!(f.bytes(),before);let graphs=workspace.project().graphs().to_vec();workspace.save(&graphs).unwrap();
    assert_eq!(f.bytes(),before);assert!(workspace.saved_asset_placements().unwrap().is_none());
    assert!(!f.root.join(JOURNAL).exists());
}
#[test]
fn source_placement1023_first_adoption_is_durable_before_infallible_model_install_under_the_same_lock(){
    let f=Fixture::new();let mut workspace=f.workspace();let before=f.bytes();let graphs=workspace.project().graphs().to_vec();let map=f.map("assets/scenes/first");
    let mut rival=f.workspace();let installed=std::cell::Cell::new(false);
    workspace.commit_asset_placement_edit(&None,&graphs,map.clone(),&[],&[],&[],&[],||{
        let saved:Snapshot=serde_json::from_slice(&fs::read(f.root.join(SIDECAR)).unwrap()).unwrap();
        let recovery:Recovery=serde_json::from_slice(&fs::read(f.root.join(RECOVERY)).unwrap()).unwrap();
        assert_eq!(saved.version,2);assert_eq!(saved.asset_placements,Some(SourceAssetPlacements::default()));
        assert_eq!(recovery.version,3);assert_eq!(recovery.asset_placements_after,Some(map.clone()));
        assert_eq!(recovery.baseline_sidecar,Some(fs::read_to_string(f.root.join(SIDECAR)).unwrap()));
        assert!(!f.root.join(JOURNAL).exists());assert!(rival.save(&graphs).unwrap_err().starts_with("Another authoring save is active:"));installed.set(true);
    }).unwrap();
    assert!(installed.get());assert_eq!(workspace.asset_placements(),Some(&map));assert!(workspace.has_pending_asset_placements().unwrap());
    assert_eq!(f.bytes().0,before.0);assert_eq!(workspace.project().graphs(),graphs);
}
#[test]
fn source_placement1023_fresh_scene_save_cold_and_inverse_replay_preserve_one_identity_and_reservation(){
    let f=Fixture::new();let mut workspace=f.workspace();let before=f.bytes().0;let (graphs,id)=f.fresh_scene(&workspace);
    let map=SourceAssetPlacements::from_entries([SourceAssetPlacement{graph_id:id,namespace:SourcePlacementNamespace::Physical,directory:"assets/scenes/second".into()}]).unwrap();
    commit_map(&mut workspace,&graphs,map.clone(),&[id]);
    assert_eq!(f.bytes().0,before,"new scene is still a recoverable edit until the actual Source save");
    let reopened=f.workspace();assert_eq!(reopened.graph_id_high_water(),id.get());assert!(!reopened.project().graphs().iter().any(|g|g.graph_id==id));
    assert_eq!(reopened.recovery_batch().unwrap().unwrap().asset_placements,Some(map.clone()));
    // A history inverse uses the same primitive, not another file/history or a v1 downgrade.
    let old=workspace.project().graphs().to_vec();commit_map(&mut workspace,&old,SourceAssetPlacements::default(),&[]);
    assert_eq!(workspace.graph_id_high_water(),id.get());
    commit_map(&mut workspace,&graphs,map.clone(),&[]);assert_eq!(workspace.graph_id_high_water(),id.get());
    workspace.save(&graphs).unwrap();let cold=f.workspace();assert!(cold.source_error.is_none());
    let restored=cold.project().graphs().iter().find(|g|g.graph_id==id).unwrap();assert_eq!(restored.narrative,Some(crate::NarrativeMetadata::SceneEntry));
    assert_eq!(cold.asset_placements(),Some(&map));assert!(!cold.has_pending_asset_placements().unwrap());assert!(cold.recovery_batch().unwrap().is_none());
    assert!(cold.project().source().contains("added"));assert_eq!(cold.graph_id_high_water(),id.get());
    assert_eq!(fs::read_dir(f.root.join("assets/scenes/second")).unwrap().count(),0);
}
#[test]
fn source_placement1023_stale_map_and_undeclared_or_duplicate_graphs_do_not_write_or_consume_an_id(){
    let f=Fixture::new();let mut workspace=f.workspace();let graphs=workspace.project().graphs().to_vec();commit_map(&mut workspace,&graphs,f.map("assets/scenes/first"),&[]);
    let bytes=f.bytes();let high=workspace.graph_id_high_water();let callback=std::cell::Cell::new(0);
    assert!(workspace.commit_asset_placement_edit(&None,&graphs,f.map("assets/scenes/second"),&[],&[],&[],&[],||callback.set(1)).is_err());
    let expected=workspace.asset_placements().cloned();let (fresh,id)=f.fresh_scene(&workspace);
    let mut duplicates=graphs.clone();duplicates.push(graphs[0].clone());
    for (candidate,declared) in [(fresh.clone(),vec![]),(graphs.clone(),vec![id]),(duplicates,vec![]),(fresh,vec![id,id])]{
        assert!(workspace.commit_asset_placement_edit(&expected,&candidate,f.map("assets/scenes/second"),&declared,&[],&[],&[],||callback.set(1)).is_err());
        assert_eq!(f.bytes(),bytes);assert_eq!(workspace.graph_id_high_water(),high);assert_eq!(callback.get(),0);assert!(!f.root.join(JOURNAL).exists());
    }
}
#[test]
fn source_placement1023_external_source_sidecar_and_readonly_reference_conflicts_preserve_external_bytes(){
    for participant in ["main.rvn",SIDECAR,"rvn.toml"]{
        let f=Fixture::new();fs::write(f.root.join("rvn.toml"),"[project]\ntitle='before'\n").unwrap();let mut workspace=f.workspace();let graphs=workspace.project().graphs().to_vec();
        let refs=[SourceRefactorFile{path:"rvn.toml".into(),before:Some(fs::read_to_string(f.root.join("rvn.toml")).unwrap()),after:Some(fs::read_to_string(f.root.join("rvn.toml")).unwrap())}];
        let path=f.root.join(participant);let mut external=fs::read(&path).unwrap();external.extend_from_slice(b"\n ");fs::write(&path,&external).unwrap();
        let before=f.bytes();let result=workspace.commit_asset_placement_edit(&None,&graphs,f.map("assets/scenes/first"),&[],&[],&[],&refs,||panic!("stale input may not install"));
        assert!(result.is_err());assert_eq!(f.bytes(),before);assert_eq!(fs::read(&path).unwrap(),external);assert!(!f.root.join(JOURNAL).exists());
    }
}
#[test]
fn source_placement1023_cold_pending_map_cannot_be_overwritten_by_another_typed_or_legacy_writer(){
    let f=Fixture::new();let mut workspace=f.workspace();let graphs=workspace.project().graphs().to_vec();commit_map(&mut workspace,&graphs,f.map("assets/scenes/first"),&[]);
    let mut cold=f.workspace();let bytes=f.bytes();let saved=cold.asset_placements().cloned();
    assert!(cold.validate_recovery_with_widget_memberships(&graphs,&[],&[]).is_err());
    assert!(cold.write_recovery_with_widget_memberships(&graphs,&[],&[]).is_err());
    assert!(cold.write_recovery(&graphs).is_err());assert!(cold.save(&graphs).is_err());
    assert_eq!(f.bytes(),bytes);assert_eq!(cold.asset_placements().cloned(),saved);
    assert!(cold.restore_asset_placement_recovery_with::<()>(|_,_|Err("real model admission refused".into())).is_err());
    assert_eq!(cold.asset_placements().cloned(),saved);assert_eq!(f.bytes(),bytes);
    cold.restore_asset_placement_recovery_with(|baseline,batch|{assert_eq!(baseline,graphs);assert_eq!(batch.graphs,graphs);Ok(())}).unwrap();
    assert_eq!(cold.asset_placements(),workspace.asset_placements());cold.write_recovery_with_widget_memberships(&graphs,&[],&[]).unwrap();
}
#[test]
fn source_placement1023_global_discard_keeps_adopted_version_and_fresh_identity_floor_on_cold_open(){
    let f=Fixture::new();let mut workspace=f.workspace();let (graphs,id)=f.fresh_scene(&workspace);let map=SourceAssetPlacements::from_entries([SourceAssetPlacement{graph_id:id,namespace:SourcePlacementNamespace::Physical,directory:"assets/scenes/second".into()}]).unwrap();
    commit_map(&mut workspace,&graphs,map,&[id]);workspace.archive_recovery().unwrap();
    assert_eq!(workspace.asset_placements(),Some(&SourceAssetPlacements::default()));assert!(workspace.recovery_batch().unwrap().is_none());
    let cold=f.workspace();assert_eq!(cold.graph_id_high_water(),id.get());assert!(cold.next_member_graph_id(cold.project().graphs()).unwrap().get()>id.get());
    let wire:serde_json::Value=serde_json::from_slice(&fs::read(f.root.join(SIDECAR)).unwrap()).unwrap();assert_eq!(wire["version"],2);
}
#[test]
fn source_placement1023_source_wins_removal_does_not_transfer_placement_to_a_homonymous_graph(){
    let f=Fixture::new();let mut workspace=f.workspace();let original=f.actual_scene();let graphs=workspace.project().graphs().to_vec();
    commit_map(&mut workspace,&graphs,f.map("assets/scenes/first"),&[]);workspace.save(&graphs).unwrap();drop(workspace);
    fs::write(f.root.join("main.rvn"),"function caption(text){return text}\nscene label other\nreturn\n").unwrap();
    let mut workspace=f.workspace();assert!(workspace.source_error.is_none());let removed=workspace.project().graphs().to_vec();assert!(!removed.iter().any(|g|g.graph_id==original));workspace.save(&removed).unwrap();assert_eq!(workspace.asset_placements(),Some(&SourceAssetPlacements::default()));drop(workspace);
    fs::write(f.root.join("main.rvn"),"function caption(text){return text}\nscene label start\nreturn\n").unwrap();
    let mut workspace=f.workspace();assert!(workspace.source_error.is_none());let recreated=workspace.project().graphs().to_vec();let fresh=recreated.iter().find(|g|matches!(&g.kind,GraphKind::Label{name}if name=="start")).unwrap().graph_id;assert_ne!(fresh,original);workspace.save(&recreated).unwrap();
    let cold=f.workspace();assert!(cold.project().graphs().iter().any(|g|g.graph_id==fresh));assert_eq!(cold.asset_placements(),Some(&SourceAssetPlacements::default()));
}
#[test]
fn source_placement1023_changed_metadata_invalidates_an_old_refactor_receipt_without_a_downgrade(){
    let f=Fixture::new();let mut workspace=f.workspace();let id=workspace.project().graphs().iter().find(|g|matches!(&g.kind,GraphKind::Function{name}if name=="caption")).unwrap().graph_id;
    let graphs=crate::rename_callable_graphs(workspace.project().graphs(),id,"renamed").unwrap();let receipt=workspace.refactor(&graphs,&[]).unwrap();
    commit_map(&mut workspace,&graphs,f.map("assets/scenes/first"),&[]);workspace.save(&graphs).unwrap();let bytes=f.bytes();
    assert!(workspace.replay_refactor(&receipt,false).is_err());assert_eq!(f.bytes(),bytes);
}
fn prepared(f:&Fixture,workspace:&SourceWorkspace)->asset_placement_workspace1023::PreparedAssetPlacementEdit{
    let graphs=workspace.project().graphs().to_vec();let map=f.map("assets/scenes/second");
    workspace.with_authoring_admission(|w|w.prepare_asset_placement_edit(&None,&graphs,&map,&[],&[],&[],&[])).unwrap()
}
fn stage_actual_publication(f:&Fixture,plan:&asset_placement_workspace1023::PreparedAssetPlacementEdit,stage:u8){
    atomic_write(&f.root.join(JOURNAL),&serde_json::to_string(&plan.transaction).unwrap()).unwrap();
    if stage>=1{atomic_write(&f.root.join(SIDECAR),&plan.after_sidecar).unwrap();}
    if stage>=2{atomic_write(&f.root.join(RECOVERY),&plan.after_recovery).unwrap();}
}
#[test]
fn source_placement1023_real_journal_reopens_at_each_existing_publisher_frontier_without_source_replay(){
    for stage in 0..=2{
        let f=Fixture::new();let workspace=f.workspace();let source=f.bytes().0;let plan=prepared(&f,&workspace);stage_actual_publication(&f,&plan,stage);drop(workspace);
        let cold=f.workspace();assert!(!f.root.join(JOURNAL).exists());assert_eq!(f.bytes().0,source);
        assert_eq!(fs::read_to_string(f.root.join(SIDECAR)).unwrap(),plan.after_sidecar);assert_eq!(fs::read_to_string(f.root.join(RECOVERY)).unwrap(),plan.after_recovery);
        assert_eq!(cold.asset_placements(),Some(&SourceAssetPlacements::default()));assert_eq!(cold.recovery_batch().unwrap().unwrap().asset_placements,Some(f.map("assets/scenes/second")));
    }
}
#[test]
fn source_placement1023_actual_io_refusal_after_protocol_adoption_retains_journal_then_resumes(){
    let f=Fixture::new();let workspace=f.workspace();let plan=prepared(&f,&workspace);stage_actual_publication(&f,&plan,1);drop(workspace);
    fs::create_dir(f.root.join(RECOVERY)).unwrap();let saved=fs::read(f.root.join(SIDECAR)).unwrap();let journal=fs::read(f.root.join(JOURNAL)).unwrap();
    assert!(SourceWorkspace::open(&f.root).is_err());assert_eq!(fs::read(f.root.join(SIDECAR)).unwrap(),saved);assert_eq!(fs::read(f.root.join(JOURNAL)).unwrap(),journal);
    fs::remove_dir(f.root.join(RECOVERY)).unwrap();let cold=f.workspace();assert!(cold.recovery_batch().unwrap().is_some());assert!(!f.root.join(JOURNAL).exists());assert_eq!(fs::read_to_string(f.root.join(RECOVERY)).unwrap(),plan.after_recovery);
}
#[test]
fn source_placement1023_external_recovery_conflict_after_adoption_is_never_overwritten_by_journal_replay(){
    let f=Fixture::new();let workspace=f.workspace();let plan=prepared(&f,&workspace);stage_actual_publication(&f,&plan,1);drop(workspace);
    fs::write(f.root.join(RECOVERY),"external bytes").unwrap();let journal=fs::read(f.root.join(JOURNAL)).unwrap();
    assert!(SourceWorkspace::open(&f.root).is_err());assert_eq!(fs::read_to_string(f.root.join(RECOVERY)).unwrap(),"external bytes");assert_eq!(fs::read(f.root.join(JOURNAL)).unwrap(),journal);
}
#[test]
fn source_placement1023_malformed_snapshot_fields_are_refused_without_migration_or_rewrite(){
    for case in 0..5{
        let f=Fixture::new();let (_,map)=f.install_proposed_editorial_v2();let mut wire:serde_json::Value=serde_json::from_slice(&fs::read(f.root.join(SIDECAR)).unwrap()).unwrap();
        match case{0=>{wire.as_object_mut().unwrap().remove("asset_placements");},1=>wire["asset_placements"]=serde_json::Value::Null,2=>wire["version"]=99.into(),3=>wire["asset_placements"]["entries"]=serde_json::json!([map["entries"][0].clone(),map["entries"][0].clone()]),_=>wire["asset_placements"]["entries"][0]["directory"]="../outside".into()}
        let bytes=serde_json::to_vec(&wire).unwrap();fs::write(f.root.join(SIDECAR),&bytes).unwrap();assert!(SourceWorkspace::open(&f.root).is_err());assert_eq!(fs::read(f.root.join(SIDECAR)).unwrap(),bytes);assert!(!f.root.join(JOURNAL).exists());
    }
}
#[test]
fn source_placement1023_malformed_recovery_map_or_owner_keeps_the_actual_file_for_explicit_resolution(){
    for case in 0..4{
        let f=Fixture::new();let mut workspace=f.workspace();let graphs=workspace.project().graphs().to_vec();commit_map(&mut workspace,&graphs,f.map("assets/scenes/first"),&[]);
        let mut wire:serde_json::Value=serde_json::from_slice(&fs::read(f.root.join(RECOVERY)).unwrap()).unwrap();
        match case{0=>wire["source"]="foreign.rvn".into(),1=>wire["baseline_sidecar"]="foreign sidecar".into(),2=>wire["asset_placements_after"]["entries"][0]["graph_id"]=999999.into(),_=>wire["asset_placements_after"]=serde_json::Value::Null}
        let bytes=serde_json::to_vec(&wire).unwrap();fs::write(f.root.join(RECOVERY),&bytes).unwrap();
        // Open preserves the saved Source even if its separate recovery is malformed.
        match SourceWorkspace::open(&f.root){Ok(Some(cold))=>assert!(cold.recovery_batch().is_err()),Err(_)=>{},Ok(None)=>panic!("actual linked Source disappeared")}
        assert_eq!(fs::read(f.root.join(RECOVERY)).unwrap(),bytes);
    }
}

#[test]
fn source_placement1023_mixed_widget_uuid_and_scene_placement_share_recovery_and_the_existing_group_save(){
    use rvn_parser::widget_asset::{WidgetBlueprintV1,WidgetGraphRef16};
    let f=Fixture::new();fs::create_dir_all(f.root.join("assets/widgets")).unwrap();fs::write(f.root.join("rvn.toml"),"[project]\ntitle='Mixed'\n").unwrap();
    let mut workspace=f.workspace();let baseline=workspace.project().graphs().to_vec();let added="screen panel(){return component(\"root\",\"text\",{\"text\":\"Panel\"},[])}\n";
    let graphs=SourceProject::open_at(&f.root.join("main.rvn"),format!("{added}{}",workspace.project().source()),&baseline).unwrap().graphs().to_vec();
    let members=graphs.iter().filter(|graph|!baseline.iter().any(|saved|saved.graph_id==graph.graph_id)).map(|graph|WidgetGraphRef16::from_u64(graph.graph_id.get())).collect::<Vec<_>>();
    let screen=graphs.iter().find(|graph|matches!(graph.kind,GraphKind::Screen{..})).unwrap().graph_id;
    let widget=WidgetRecovery{locator:"assets/widgets/panel.rvnwidget".into(),blueprint_json:WidgetBlueprintV1{uuid:"17411a3e431e4c9388b78a2b9825b130".into(),source:"main.rvn".into(),screen_graph:WidgetGraphRef16::from_u64(screen.get()),members}.encode().unwrap()};
    let mut fresh=graphs.iter().filter(|graph|!baseline.iter().any(|saved|saved.graph_id==graph.graph_id)).map(|graph|graph.graph_id).collect::<Vec<_>>();fresh.sort();let map=f.map("assets/scenes/first");
    workspace.write_recovery_with_widgets(&graphs,std::slice::from_ref(&widget)).unwrap();
    let prior:serde_json::Value=serde_json::from_slice(&fs::read(f.root.join(RECOVERY)).unwrap()).unwrap();assert_eq!(prior["version"],1);
    workspace.commit_asset_placement_edit(&None,&graphs,map.clone(),&fresh,std::slice::from_ref(&widget),&[],&[],||()).unwrap();
    let batch=workspace.recovery_batch().unwrap().unwrap();assert_eq!(batch.widgets,vec![widget.clone()]);assert_eq!(batch.asset_placements,Some(map.clone()));let before=f.bytes();assert!(workspace.save(&graphs).is_err());assert_eq!(f.bytes(),before);
    let manifest=fs::read_to_string(f.root.join("rvn.toml")).unwrap();let files=[SourceRefactorFile{path:"rvn.toml".into(),before:Some(manifest.clone()),after:Some(format!("{manifest}\n[widgets]\n{}='widgets/panel.rvnwidget'\n","17411a3e431e4c9388b78a2b9825b130"))},SourceRefactorFile{path:widget.locator.clone(),before:None,after:Some(widget.blueprint_json.clone())}];
    workspace.save_with_files(&graphs,&files).unwrap();let cold=f.workspace();assert_eq!(cold.asset_placements(),Some(&map));assert!(cold.recovery_batch().unwrap().is_none());assert_eq!(fs::read_to_string(f.root.join(&widget.locator)).unwrap(),widget.blueprint_json);
    for id in fresh{assert!(cold.project().graphs().iter().any(|graph|graph.graph_id==id));}
}

#[test]
fn source_placement1023_interrupted_plain_save_cannot_downgrade_an_adopted_sidecar(){
    let f=Fixture::new();let mut workspace=f.workspace();let graphs=workspace.project().graphs().to_vec();commit_map(&mut workspace,&graphs,f.map("assets/scenes/first"),&[]);workspace.save(&graphs).unwrap();
    let before=f.bytes();let mut old:serde_json::Value=serde_json::from_slice(&before.1).unwrap();old["version"]=1.into();old.as_object_mut().unwrap().remove("asset_placements");
    let transaction=Transaction{library_readset1027:None,source:"main.rvn".into(),before_source:workspace.project().source().into(),after_source:workspace.project().source().into(),before_sidecar:Some(String::from_utf8(before.1.clone()).unwrap()),after_sidecar:serde_json::to_string(&old).unwrap(),reservation_archive:None,editorial_publication:None};
    fs::write(f.root.join(JOURNAL),serde_json::to_vec(&transaction).unwrap()).unwrap();assert!(SourceWorkspace::open(&f.root).is_err());assert_eq!(f.bytes(),before);assert!(f.root.join(JOURNAL).exists());
}
#[test]
fn source_placement1023_imported_source_is_rechecked_before_any_editorial_adoption(){
    let f=Fixture::new();fs::write(f.root.join("chapter.rvn"),"label chapter\nreturn\n").unwrap();fs::write(f.root.join("main.rvn"),"use \"chapter.rvn\"\nscene label start\njump chapter\n").unwrap();
    // Relink is deliberately a real independent fixture setup, not another live writer.
    fs::remove_file(f.root.join(SIDECAR)).unwrap();let mut workspace=SourceWorkspace::link(&f.root,&f.root.join("main.rvn"),&[]).unwrap();assert!(workspace.source_error.is_none());let graphs=workspace.project().graphs().to_vec();let map=f.map("assets/scenes/first");
    fs::write(f.root.join("chapter.rvn"),"label chapter\n\"External\"\nreturn\n").unwrap();let before=f.bytes();let imported=fs::read(f.root.join("chapter.rvn")).unwrap();
    assert!(workspace.commit_asset_placement_edit(&None,&graphs,map,&[],&[],&[],&[],||panic!("changed imported source may not install")).is_err());assert_eq!(f.bytes(),before);assert_eq!(fs::read(f.root.join("chapter.rvn")).unwrap(),imported);assert!(!f.root.join(JOURNAL).exists());
}

#[test]
fn source_placement1023_actual_commit_io_failure_retains_owned_journal_and_reopens_after_obstruction_is_removed(){
    let f=Fixture::new();let mut workspace=f.workspace();let graphs=workspace.project().graphs().to_vec();let before=f.bytes();let map=f.map("assets/scenes/first");
    let backups=f.root.join(".rvn-backups");let parked=f.root.join("prior-backups");let had=backups.exists();if had{fs::rename(&backups,&parked).unwrap();}fs::write(&backups,"real obstruction").unwrap();
    assert!(workspace.commit_asset_placement_edit(&None,&graphs,map.clone(),&[],&[],&[],&[],||panic!("failed IO may not install model")).is_err());
    assert_eq!(f.bytes(),before);assert!(f.root.join(JOURNAL).is_file());assert!(workspace.asset_placements().is_none());
    fs::remove_file(&backups).unwrap();if had{fs::rename(&parked,&backups).unwrap();}
    drop(workspace);let cold=f.workspace();assert_eq!(cold.asset_placements(),Some(&SourceAssetPlacements::default()));assert_eq!(cold.recovery_batch().unwrap().unwrap().asset_placements,Some(map));assert!(!f.root.join(JOURNAL).exists());assert_eq!(f.bytes().0,before.0);
}
