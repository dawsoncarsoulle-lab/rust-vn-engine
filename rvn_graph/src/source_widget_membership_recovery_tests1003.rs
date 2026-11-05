//! Published-member Recovery uses the actual source writer; no runtime/native host.
use super::*;
use rvn_parser::widget_asset::{WidgetBlueprintV1,WidgetGraphRef16};
const UUID:&str="17411a3e431e4c9388b78a2b9825b130";
struct Fixture{root:PathBuf,workspace:SourceWorkspace,before:WidgetBlueprintV1}
impl Fixture{
 fn new()->Self{
  static NEXT:std::sync::atomic::AtomicU64=std::sync::atomic::AtomicU64::new(0);
  let root=std::env::temp_dir().join(format!("rvn-member-recovery1003-{}-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos(),NEXT.fetch_add(1,std::sync::atomic::Ordering::Relaxed)));
  fs::create_dir_all(root.join("assets")).unwrap();fs::write(root.join("main.rvn"),"init{set seen=0}\nhandler initial_click(event){return 0}\nscreen ButtonOwner(){return component(\"root\",\"button\",{\"layout_contract\":\"widget_v1\"},[])}\nlabel start\n\"Ready\"\nreturn\n").unwrap();
  let workspace=SourceWorkspace::link(&root,&root.join("main.rvn"),&[]).unwrap();let graphs=workspace.project().graphs();let screen=graphs.iter().find(|g|matches!(g.kind,crate::GraphKind::Screen{..})).unwrap().graph_id;let handler=graphs.iter().find(|g|matches!(g.kind,crate::GraphKind::Handler{..})).unwrap().graph_id;
  let before=WidgetBlueprintV1{uuid:UUID.into(),source:"main.rvn".into(),screen_graph:WidgetGraphRef16::from_u64(screen.get()),members:vec![WidgetGraphRef16::from_u64(screen.get()),WidgetGraphRef16::from_u64(handler.get())]};fs::write(root.join("assets/owner.rvnwidget"),before.encode().unwrap()).unwrap();fs::write(root.join("rvn.toml"),format!("# keep manifest\n[widget_blueprints]\n{UUID} = 'owner.rvnwidget'\n")).unwrap();Self{root,workspace,before}
 }
 fn bytes(&self,name:&str)->Vec<u8>{fs::read(self.root.join(name)).unwrap()}
 fn candidate(&self)->(Vec<GraphDocument>,WidgetMembershipRecovery,crate::GraphId){
  let mut graphs=self.workspace.project().graphs().to_vec();let id=self.workspace.next_member_graph_id(&graphs).unwrap();let script=rvn_parser::parse("handler AddedClick1003(event){return 0}").unwrap();let mut handler=crate::import_script(&script).unwrap().into_iter().find(|g|matches!(g.kind,crate::GraphKind::Handler{..})).unwrap();handler.graph_id=id;graphs.push(handler);
  let screen=graphs.iter_mut().find(|g|g.graph_id.get()==self.before.screen_graph.get()).unwrap();let button=screen.nodes.values().find(|n|n.kind==crate::NodeKind::UiComponent).unwrap().id;screen.set_component_property(button,&["events","click"],crate::PropertyValue::String("AddedClick1003".into())).unwrap();let mut after=self.before.clone();after.members.push(WidgetGraphRef16::from_u64(id.get()));
  let delta=WidgetMembershipRecovery{locator:"assets/owner.rvnwidget".into(),before_blueprint_json:self.before.encode().unwrap(),after_blueprint_json:after.encode().unwrap()};(graphs,delta,id)
 }
 fn files(&self,delta:&WidgetMembershipRecovery)->Vec<SourceRefactorFile>{vec![SourceRefactorFile{path:"rvn.toml".into(),before:Some(String::from_utf8(self.bytes("rvn.toml")).unwrap()),after:Some(String::from_utf8(self.bytes("rvn.toml")).unwrap())},SourceRefactorFile{path:delta.locator.clone(),before:Some(delta.before_blueprint_json.clone()),after:Some(delta.after_blueprint_json.clone())}]}
}
impl Drop for Fixture{fn drop(&mut self){if let(Ok(root),Ok(temp))=(self.root.canonicalize(),std::env::temp_dir().canonicalize()){if root.parent()==Some(temp.as_path())&&root.file_name().is_some_and(|name|name.to_string_lossy().starts_with("rvn-member-recovery1003-")){let _=fs::remove_dir_all(root);}}}}
#[test]
fn member_recovery1003_actual_delta_cold_admission_same_writer_publication_and_inverse(){
 let mut f=Fixture::new();let original=f.workspace.project().graphs().to_vec();let source=f.bytes("main.rvn");let sidecar=f.bytes(SIDECAR);let(graphs,delta,id)=f.candidate();f.workspace.commit_member_graph_id(id).unwrap();f.workspace.write_recovery_with_widget_memberships(&graphs,&[],&[delta.clone()]).unwrap();
 assert_eq!(f.bytes("main.rvn"),source);assert_eq!(f.bytes(SIDECAR),sidecar);assert_eq!(f.bytes(&delta.locator),delta.before_blueprint_json.as_bytes());let wire:serde_json::Value=serde_json::from_slice(&f.bytes(RECOVERY)).unwrap();assert_eq!(wire["version"],2);
 let cold=SourceWorkspace::open(&f.root).unwrap().unwrap();assert!(cold.recovery().is_err());let batch=cold.recovery_batch().unwrap().unwrap();assert_eq!(batch.widget_memberships,vec![delta.clone()]);assert_eq!(batch.graphs,graphs);assert!(cold.next_member_graph_id(cold.project().graphs()).unwrap().get()>id.get());drop(cold);
 let files=f.files(&delta);f.workspace.save_with_files(&graphs,&files).unwrap();assert!(!f.root.join(RECOVERY).exists());assert_eq!(f.bytes(&delta.locator),delta.after_blueprint_json.as_bytes());assert!(fs::read_to_string(f.root.join("main.rvn")).unwrap().contains("AddedClick1003"));
 let inverse=WidgetMembershipRecovery{locator:delta.locator,before_blueprint_json:delta.after_blueprint_json,after_blueprint_json:delta.before_blueprint_json};f.workspace.write_recovery_with_widget_memberships(&original,&[],&[inverse.clone()]).unwrap();let files=f.files(&inverse);f.workspace.save_with_files(&original,&files).unwrap();
 let cold=SourceWorkspace::open(&f.root).unwrap().unwrap();assert!(cold.project().graphs().iter().all(|g|g.graph_id!=id));assert!(cold.next_member_graph_id(cold.project().graphs()).unwrap().get()>id.get());assert_eq!(f.bytes(&inverse.locator),inverse.after_blueprint_json.as_bytes());
}
#[test]
fn member_recovery1003_missing_descriptor_and_external_cas_preserve_recovery_and_source(){
 let mut f=Fixture::new();let(graphs,delta,id)=f.candidate();f.workspace.commit_member_graph_id(id).unwrap();f.workspace.write_recovery_with_widget_memberships(&graphs,&[],&[delta.clone()]).unwrap();let source=f.bytes("main.rvn");let sidecar=f.bytes(SIDECAR);let recovery=f.bytes(RECOVERY);
 assert!(f.workspace.save(&graphs).is_err());assert_eq!(f.bytes("main.rvn"),source);assert_eq!(f.bytes(SIDECAR),sidecar);assert_eq!(f.bytes(RECOVERY),recovery);
 let files=f.files(&delta);let mut external=f.bytes(&delta.locator);external.extend_from_slice(b"\n ");fs::write(f.root.join(&delta.locator),&external).unwrap();assert!(f.workspace.save_with_files(&graphs,&files).is_err());assert_eq!(f.bytes("main.rvn"),source);assert_eq!(f.bytes(SIDECAR),sidecar);assert_eq!(f.bytes(RECOVERY),recovery);assert_eq!(f.bytes(&delta.locator),external);
}
#[test]
fn member_recovery1003_undo_before_save_can_drop_only_a_fully_reverted_membership(){
 let mut f=Fixture::new();let original=f.workspace.project().graphs().to_vec();let(graphs,delta,id)=f.candidate();f.workspace.commit_member_graph_id(id).unwrap();f.workspace.write_recovery_with_widget_memberships(&graphs,&[],&[delta.clone()]).unwrap();let recovery=f.bytes(RECOVERY);
 assert!(f.workspace.write_recovery_with_widgets(&graphs,&[]).is_err());assert_eq!(f.bytes(RECOVERY),recovery);
 f.workspace.write_recovery_with_widget_memberships(&original,&[],&[]).unwrap();let batch=f.workspace.recovery_batch().unwrap().unwrap();assert!(batch.widget_memberships.is_empty());assert_eq!(batch.graphs,original);assert_eq!(f.bytes(&delta.locator),delta.before_blueprint_json.as_bytes());let cold=SourceWorkspace::open(&f.root).unwrap().unwrap();assert!(cold.next_member_graph_id(cold.project().graphs()).unwrap().get()>id.get());
}
#[test]
fn member_recovery1003_wrong_identity_kind_and_locator_refuse_without_neighbor_or_source_change(){
 let mut f=Fixture::new();let(graphs,delta,id)=f.candidate();f.workspace.commit_member_graph_id(id).unwrap();
 let source=f.bytes("main.rvn");let sidecar=f.bytes(SIDECAR);let descriptor=f.bytes(&delta.locator);
 fs::write(f.root.join("assets/neighbor.private"),b"neighbor unchanged").unwrap();
 for mode in 0..5{
  let mut wrong=delta.clone();let mut after=wrong.after().unwrap();
  match mode{
   0=>after.uuid="797679920f7142be99d9d6fe5553ca34".into(),
   1=>after.source="foreign.rvn".into(),
   2=>after.screen_graph=WidgetGraphRef16::from_u64(id.get()),
   3=>wrong.locator="../escape.rvnwidget".into(),
   _=>{let label=graphs.iter().find(|g|matches!(g.kind,crate::GraphKind::Label{..})).unwrap().graph_id;after.members.push(WidgetGraphRef16::from_u64(label.get()));},
  }
  wrong.after_blueprint_json=after.encode().unwrap();
  assert!(f.workspace.write_recovery_with_widget_memberships(&graphs,&[],&[wrong]).is_err(),"mode {mode}");
  assert_eq!(f.bytes("main.rvn"),source);assert_eq!(f.bytes(SIDECAR),sidecar);assert_eq!(f.bytes(&delta.locator),descriptor);
  assert!(!f.root.join(RECOVERY).exists());assert_eq!(f.bytes("assets/neighbor.private"),b"neighbor unchanged");
 }
}
#[test]
fn member_recovery1003_unpublished_handler_add_and_undo_preserve_birth_identity_and_reservation(){
 let mut f=Fixture::new();let baseline=f.workspace.project().graphs().to_vec();let first=f.workspace.next_member_graph_id(&baseline).unwrap();
 let script=rvn_parser::parse("screen FreshOwner1003(){return component(\"root\",\"button\",{\"layout_contract\":\"widget_v1\"},[])}\nhandler FreshEvent1003(event){return 0}\nfunction FreshValue1003(){return 0}").unwrap();
 let mut born=crate::import_script(&script).unwrap().into_iter().filter(|g|g.kind!=crate::GraphKind::Init).collect::<Vec<_>>();
 for(i,graph)in born.iter_mut().enumerate(){graph.graph_id=crate::GraphId::new(first.get()+i as u64);}
 let ids=born.iter().map(|g|g.graph_id).collect::<Vec<_>>();f.workspace.with_graph_creation_admission(&ids, |_|Ok(())).unwrap();
 let screen=born.iter().find(|g|matches!(g.kind,crate::GraphKind::Screen{..})).unwrap().graph_id;
 let blueprint=WidgetBlueprintV1{uuid:"797679920f7142be99d9d6fe5553ca34".into(),source:"main.rvn".into(),screen_graph:WidgetGraphRef16::from_u64(screen.get()),members:ids.iter().map(|id|WidgetGraphRef16::from_u64(id.get())).collect()};
 let birth=WidgetRecovery{locator:"assets/fresh.rvnwidget".into(),blueprint_json:blueprint.encode().unwrap()};
 let mut original=baseline;original.extend(born);f.workspace.write_recovery_with_widgets(&original,&[birth.clone()]).unwrap();
 let added_id=f.workspace.next_member_graph_id(&original).unwrap();let added_script=rvn_parser::parse("handler FreshAdded1003(event){return 0}").unwrap();
 let mut handler=crate::import_script(&added_script).unwrap().into_iter().find(|g|matches!(g.kind,crate::GraphKind::Handler{..})).unwrap();handler.graph_id=added_id;
 let mut added=original.clone();added.push(handler);let mut after=blueprint.clone();after.members.push(WidgetGraphRef16::from_u64(added_id.get()));
 let updated=WidgetRecovery{locator:birth.locator.clone(),blueprint_json:after.encode().unwrap()};
 f.workspace.with_graph_creation_admission(&[added_id], |_|Ok(())).unwrap();f.workspace.write_recovery_with_widgets(&added,&[updated]).unwrap();
 let batch=f.workspace.recovery_batch().unwrap().unwrap();assert_eq!(batch.widgets[0].blueprint().unwrap().screen_graph,blueprint.screen_graph);assert!(batch.graphs.iter().any(|g|g.graph_id==added_id));
 f.workspace.write_recovery_with_widgets(&original,&[birth.clone()]).unwrap();let cold=SourceWorkspace::open(&f.root).unwrap().unwrap();let batch=cold.recovery_batch().unwrap().unwrap();
 assert_eq!(batch.widgets,vec![birth]);assert_eq!(batch.graphs,original);assert!(cold.next_member_graph_id(&batch.graphs).unwrap().get()>added_id.get());
 assert!(!f.root.join("assets/fresh.rvnwidget").exists());assert_eq!(f.bytes("assets/owner.rvnwidget"),f.before.encode().unwrap().as_bytes());
}
#[test]
fn member_discard1003_dropped_delta_keeps_descriptor_and_index_cas_inside_same_archive_writer(){
    let mut f=Fixture::new();let original=f.workspace.project().graphs().to_vec();let(graphs,delta,id)=f.candidate();
    f.workspace.commit_member_graph_id(id).unwrap();f.workspace.write_recovery_with_widget_memberships(&graphs,&[],&[delta.clone()]).unwrap();
    let baselines=f.files(&delta).into_iter().map(|mut file|{file.after=file.before.clone();file}).collect::<Vec<_>>();
    let source=f.bytes("main.rvn");let sidecar=f.bytes(SIDECAR);let recovery=f.bytes(RECOVERY);
    // Preparation succeeded, then an actual independent descriptor write
    // happens before the archive API obtains its existing cooperative lock.
    let mut external=f.bytes(&delta.locator);external.extend_from_slice(b"\n ");fs::write(f.root.join(&delta.locator),&external).unwrap();
    let called=std::cell::Cell::new(false);
    assert!(f.workspace.discard_recovery_with_memberships_and_references(&original,&[],&[],&baselines,||called.set(true)).is_err());
    assert!(!called.get());assert_eq!(f.bytes("main.rvn"),source);assert_eq!(f.bytes(SIDECAR),sidecar);assert_eq!(f.bytes(RECOVERY),recovery);assert_eq!(f.bytes(&delta.locator),external);
    fs::write(f.root.join(&delta.locator),&delta.before_blueprint_json).unwrap();
    let mut invalid=baselines.clone();invalid[0].after=Some("mutating participant is forbidden".into());
    assert!(f.workspace.discard_recovery_with_memberships_and_references(&original,&[],&[],&invalid,||called.set(true)).is_err());assert!(!called.get());assert_eq!(f.bytes(RECOVERY),recovery);
    f.workspace.discard_recovery_with_memberships_and_references(&original,&[],&[],&baselines,||called.set(true)).unwrap();
    assert!(called.get());let batch=f.workspace.recovery_batch().unwrap().unwrap();assert!(batch.widget_memberships.is_empty());assert_eq!(batch.graphs,original);
    assert_eq!(f.bytes("main.rvn"),source);assert_eq!(f.bytes(SIDECAR),sidecar);assert_eq!(f.bytes(&delta.locator),delta.before_blueprint_json.as_bytes());
    let cold=SourceWorkspace::open(&f.root).unwrap().unwrap();assert!(cold.next_member_graph_id(cold.project().graphs()).unwrap().get()>id.get());
}
