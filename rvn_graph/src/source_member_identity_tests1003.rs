//! Actual source writer and sidecar identity contracts; no runtime/native host.
use super::*;
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!("rvn-member-identity1003-{}-{}-{n}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir(&root).unwrap();
        fs::write(root.join("main.rvn"), "init {set seen=0}\nlabel start\n\"Ready\"\nreturn\n").unwrap();
        Self(root)
    }
    fn link(&self) -> SourceWorkspace { SourceWorkspace::link(&self.0, &self.0.join("main.rvn"), &[]).unwrap() }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if let (Ok(root), Ok(temp)) = (self.0.canonicalize(), std::env::temp_dir().canonicalize()) {
            if root.parent() == Some(temp.as_path()) && root.file_name().is_some_and(|name| name.to_string_lossy().starts_with("rvn-member-identity1003-")) { let _ = fs::remove_dir_all(root); }
        }
    }
}
fn actual_handler(id: crate::GraphId) -> GraphDocument {
    let script = rvn_parser::parse("handler ActualClick1003(event){return 0}").unwrap();
    let mut graph = crate::import_script(&script).unwrap().into_iter().find(|g| matches!(&g.kind, crate::GraphKind::Handler{name} if name == "ActualClick1003")).unwrap();
    graph.graph_id = id;
    graph
}
#[test]
fn source_member_identity1003_actual_handler_add_remove_save_cold_never_recycles_committed_id() {
    let f = Fixture::new(); let mut w = f.link(); let before = w.project().graphs().to_vec(); let id = w.next_member_graph_id(&before).unwrap();
    let mut added = before.clone(); added.push(actual_handler(id)); let mut candidate = w.project().clone(); candidate.apply_visual(w.project().source(), &added).unwrap();
    assert!(candidate.graphs().iter().any(|g| g.graph_id == id && matches!(g.kind, crate::GraphKind::Handler{..})));
    w.commit_member_graph_id(id).unwrap(); w.save(&added).unwrap();
    assert!(fs::read_to_string(f.0.join("main.rvn")).unwrap().contains("ActualClick1003"));
    let removed = w.project().graphs().iter().filter(|g| g.graph_id != id).cloned().collect::<Vec<_>>(); w.save(&removed).unwrap();
    assert!(!fs::read_to_string(f.0.join("main.rvn")).unwrap().contains("ActualClick1003")); drop(w);
    let mut cold = SourceWorkspace::open(&f.0).unwrap().unwrap(); assert!(cold.project().graphs().iter().all(|g| g.graph_id != id));
    assert!(cold.next_member_graph_id(cold.project().graphs()).unwrap().get() > id.get());
    // Redo restores its authenticated original identity, rather than allocating.
    let mut redo = cold.project().graphs().to_vec(); redo.push(actual_handler(id)); cold.save(&redo).unwrap();
    assert!(cold.project().graphs().iter().any(|g| g.graph_id == id));
    assert!(cold.next_member_graph_id(cold.project().graphs()).unwrap().get() > id.get());
}
#[test]
fn source_member_identity1003_preparation_without_commit_preserves_sidecar_and_identity() {
    let f = Fixture::new(); let w = f.link(); let bytes = fs::read(f.0.join(SIDECAR)).unwrap(); let first = w.next_member_graph_id(w.project().graphs()).unwrap();
    let mut malformed = w.project().graphs().to_vec(); let mut graph = actual_handler(first); graph.graph_id = malformed[0].graph_id; malformed.push(graph);
    assert!(w.project().clone().apply_visual(w.project().source(), &malformed).is_err());
    assert_eq!(w.next_member_graph_id(w.project().graphs()).unwrap(), first); assert_eq!(fs::read(f.0.join(SIDECAR)).unwrap(), bytes);
}
#[test]
fn source_member_identity1003_legacy_sidecar_defaults_and_checked_exhaustion() {
    let f = Fixture::new(); let w = f.link(); let maximum = w.project().graphs().iter().map(|g| g.graph_id.get()).max().unwrap(); drop(w);
    let path = f.0.join(SIDECAR); let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap(); value.as_object_mut().unwrap().remove("graph_id_high_water"); fs::write(&path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
    let legacy = SourceWorkspace::open(&f.0).unwrap().unwrap(); assert_eq!(legacy.next_member_graph_id(legacy.project().graphs()).unwrap().get(), maximum + 1); drop(legacy);
    value["graph_id_high_water"] = serde_json::json!(u64::MAX); fs::write(&path, serde_json::to_vec_pretty(&value).unwrap()).unwrap(); let bytes = fs::read(&path).unwrap();
    let exhausted = SourceWorkspace::open(&f.0).unwrap().unwrap(); assert!(exhausted.next_member_graph_id(exhausted.project().graphs()).is_err()); assert_eq!(fs::read(&path).unwrap(), bytes);
}
#[test]
fn source_member_identity1003_refactor_replay_preserves_reservation_and_all_other_fields() {
    let f=Fixture::new();let mut w=f.link();let mut edited=w.project().graphs().to_vec();
    let graph=edited.iter_mut().find(|g|matches!(g.kind,crate::GraphKind::Label{..})).unwrap();
    let text=graph.nodes.values().find(|n|n.kind==crate::NodeKind::TextValue&&n.properties.get("value")==Some(&crate::PropertyValue::String("Ready".into()))).unwrap().id;
    graph.set_property(text,"value",crate::PropertyValue::String("Edited".into())).unwrap();
    let receipt=w.refactor(&edited,&[]).unwrap();let receipt_files=serde_json::to_value(&receipt.files).unwrap();
    let baseline=w.project().graphs().to_vec();let id=w.next_member_graph_id(&baseline).unwrap();
    let mut added=baseline.clone();added.push(actual_handler(id));
    let mut candidate=w.project().clone();candidate.apply_visual(w.project().source(),&added).unwrap();
    assert!(candidate.graphs().iter().any(|g|g.graph_id==id));
    w.commit_member_graph_id(id).unwrap();
    // Model Undo precedes Save: the committed identity survives via the real
    // recovery/archive writer, without adding/removing source declarations.
    w.write_recovery(&baseline).unwrap();assert!(f.0.join(RECOVERY).is_file());
    w.archive_recovery().unwrap();assert!(!f.0.join(RECOVERY).exists());
    assert!(w.project().graphs().iter().all(|g|g.graph_id!=id));
    let historical=receipt.files.iter().find(|file|file.path==SIDECAR).unwrap();
    let mut expected:serde_json::Value=serde_json::from_str(historical.after.as_deref().unwrap()).unwrap();
    let mut actual:serde_json::Value=serde_json::from_slice(&fs::read(f.0.join(SIDECAR)).unwrap()).unwrap();
    assert_eq!(actual["graph_id_high_water"],serde_json::json!(id.get()));
    expected.as_object_mut().unwrap().remove("graph_id_high_water");actual.as_object_mut().unwrap().remove("graph_id_high_water");
    assert_eq!(actual,expected,"only the real monotone reservation may differ before replay");
    let historical_source=receipt.files.iter().find(|file|file.path=="main.rvn").unwrap();
    assert_eq!(fs::read_to_string(f.0.join("main.rvn")).unwrap(),historical_source.after.as_deref().unwrap());
    w.replay_refactor(&receipt,false).unwrap();assert_eq!(fs::read_to_string(f.0.join("main.rvn")).unwrap(),historical_source.before.as_deref().unwrap());drop(w);
    let mut cold=SourceWorkspace::open(&f.0).unwrap().unwrap();assert!(cold.next_member_graph_id(cold.project().graphs()).unwrap().get()>id.get());
    cold.replay_refactor(&receipt,true).unwrap();assert_eq!(fs::read_to_string(f.0.join("main.rvn")).unwrap(),historical_source.after.as_deref().unwrap());drop(cold);
    let cold=SourceWorkspace::open(&f.0).unwrap().unwrap();assert!(cold.next_member_graph_id(cold.project().graphs()).unwrap().get()>id.get());
    assert_eq!(serde_json::to_value(&receipt.files).unwrap(),receipt_files,"the historical receipt is immutable");
}

#[test]
fn source_member_identity1003_saved_member_add_remove_is_not_a_reservation_only_refactor_rebase() {
    let f=Fixture::new();let mut w=f.link();let mut edited=w.project().graphs().to_vec();
    let graph=edited.iter_mut().find(|g|matches!(g.kind,crate::GraphKind::Label{..})).unwrap();
    let text=graph.nodes.values().find(|n|n.kind==crate::NodeKind::TextValue&&n.properties.get("value")==Some(&crate::PropertyValue::String("Ready".into()))).unwrap().id;
    graph.set_property(text,"value",crate::PropertyValue::String("Edited".into())).unwrap();
    let receipt=w.refactor(&edited,&[]).unwrap();let receipt_files=serde_json::to_value(&receipt.files).unwrap();
    let id=w.next_member_graph_id(w.project().graphs()).unwrap();let mut added=w.project().graphs().to_vec();added.push(actual_handler(id));
    w.commit_member_graph_id(id).unwrap();w.save(&added).unwrap();
    let removed=w.project().graphs().iter().filter(|g|g.graph_id!=id).cloned().collect::<Vec<_>>();w.save(&removed).unwrap();
    let source=fs::read(f.0.join("main.rvn")).unwrap();let sidecar=fs::read(f.0.join(SIDECAR)).unwrap();
    let historical_source=receipt.files.iter().find(|file|file.path=="main.rvn").unwrap().after.as_deref().unwrap();
    // The source writer deliberately retains insertion delimiters/trivia.
    // Equal live membership is not byte equality of the authentic participant.
    assert_ne!(source,historical_source.as_bytes());
    let graphs=w.project().graphs().to_vec();let high=w.graph_id_high_water();let recovery=read_optional(&f.0.join(RECOVERY)).unwrap();
    assert!(w.replay_refactor(&receipt,false).unwrap_err().contains("sidecar"));
    assert_eq!(w.project().graphs(),graphs);assert_eq!(w.graph_id_high_water(),high);
    assert_eq!(fs::read(f.0.join("main.rvn")).unwrap(),source);assert_eq!(fs::read(f.0.join(SIDECAR)).unwrap(),sidecar);
    assert_eq!(read_optional(&f.0.join(RECOVERY)).unwrap(),recovery);assert!(!f.0.join(refactor::REFACTOR_JOURNAL).exists());
    assert_eq!(serde_json::to_value(&receipt.files).unwrap(),receipt_files);
}

#[test]
fn source_member_identity1003_recovery_retains_committed_reservation_after_model_undo(){
    let f=Fixture::new();let mut w=f.link();let graphs=w.project().graphs().to_vec();let id=w.next_member_graph_id(&graphs).unwrap();w.commit_member_graph_id(id).unwrap();
    w.write_recovery(&graphs).unwrap();drop(w);
    let cold=SourceWorkspace::open(&f.0).unwrap().unwrap();assert!(cold.next_member_graph_id(cold.project().graphs()).unwrap().get()>id.get());
}
#[test]
fn source_member_identity1003_refactor_refuses_any_non_reservation_sidecar_change(){
    for change in ["unknown_metadata","presentation"]{
        let f=Fixture::new();let mut w=f.link();let graphs=w.project().graphs().to_vec();let receipt=w.refactor(&graphs,&[]).unwrap();drop(w);
        let path=f.0.join(SIDECAR);let mut value:serde_json::Value=serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        if change=="unknown_metadata"{value["foreign_metadata"]=serde_json::json!(true);}
        else{let graph=value["graphs"].as_array_mut().unwrap().first_mut().unwrap();let node=graph["nodes"].as_object_mut().unwrap().values_mut().next().unwrap();node["position"][0]=serde_json::json!(17.0);}
        fs::write(&path,serde_json::to_vec_pretty(&value).unwrap()).unwrap();let source=fs::read(f.0.join("main.rvn")).unwrap();let bytes=fs::read(&path).unwrap();
        let mut cold=SourceWorkspace::open(&f.0).unwrap().unwrap();let before=cold.project().graphs().to_vec();let high=cold.graph_id_high_water();
        assert!(cold.replay_refactor(&receipt,false).is_err(),"{change}");assert_eq!(cold.project().graphs(),before);assert_eq!(cold.graph_id_high_water(),high);assert_eq!(fs::read(&path).unwrap(),bytes);assert_eq!(fs::read(f.0.join("main.rvn")).unwrap(),source);
    }
}
#[test]
fn source_member_identity1003_invalid_or_regressed_reservation_refuses_without_writes(){
    for value in [serde_json::Value::Null,serde_json::json!(-1),serde_json::json!(1.5),serde_json::json!("123"),serde_json::json!(0)]{
        let f=Fixture::new();let w=f.link();drop(w);let path=f.0.join(SIDECAR);let mut snapshot:serde_json::Value=serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();snapshot["graph_id_high_water"]=value.clone();fs::write(&path,serde_json::to_vec_pretty(&snapshot).unwrap()).unwrap();
        let before=fs::read(&path).unwrap();assert!(SourceWorkspace::open(&f.0).is_err(),"{value}");assert_eq!(fs::read(&path).unwrap(),before);
    }
}

#[test]
fn source_member_identity1003_recovery_absent_field_is_legacy_but_explicit_null_and_non_integer_refuse(){
    let f=Fixture::new();let mut w=f.link();let graphs=w.project().graphs().to_vec();
    let reserved=w.next_member_graph_id(&graphs).unwrap();w.commit_member_graph_id(reserved).unwrap();
    w.write_recovery(&graphs).unwrap();drop(w);
    let path=f.0.join(RECOVERY);assert!(path.is_file(),"the actual committed reservation requires a real recovery file");let mut legacy:serde_json::Value=serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();legacy.as_object_mut().unwrap().remove("graph_id_high_water");
    fs::write(&path,serde_json::to_vec(&legacy).unwrap()).unwrap();let cold=SourceWorkspace::open(&f.0).unwrap().unwrap();assert!(cold.recovery_batch().unwrap().is_some());drop(cold);
    for value in [serde_json::Value::Null,serde_json::json!(-1),serde_json::json!(1.5),serde_json::json!("123")]{
        let mut malformed=legacy.clone();malformed["graph_id_high_water"]=value.clone();fs::write(&path,serde_json::to_vec(&malformed).unwrap()).unwrap();let bytes=fs::read(&path).unwrap();
        // Opening retains malformed recovery for the existing explicit Conflict.
        let cold=SourceWorkspace::open(&f.0).unwrap().unwrap();assert!(cold.recovery_batch().is_err(),"{value}");assert_eq!(fs::read(&path).unwrap(),bytes);
    }
}

#[test]
fn source_member_identity1003_common_admission_refusal_never_consumes_or_calls_after_source_cas(){
    let f=Fixture::new();let mut w=f.link();let original=fs::read(f.0.join(SIDECAR)).unwrap();
    let id=w.next_member_graph_id(w.project().graphs()).unwrap();let high=w.graph_id_high_water();
    let refusal:Result<(),String>=w.with_graph_creation_admission(&[id], |_|Err("actual participant refusal".into()));
    assert!(refusal.is_err());assert_eq!(w.graph_id_high_water(),high);
    assert_eq!(w.next_member_graph_id(w.project().graphs()).unwrap(),id);
    assert_eq!(fs::read(f.0.join(SIDECAR)).unwrap(),original);
    let called=std::cell::Cell::new(false);fs::write(f.0.join("main.rvn"),"label external\nreturn\n").unwrap();
    assert!(w.with_graph_creation_admission(&[id], |_|{called.set(true);Ok(())}).is_err());
    assert!(!called.get());assert_eq!(w.graph_id_high_water(),high);
    assert_eq!(fs::read(f.0.join(SIDECAR)).unwrap(),original);
}
#[test]
fn source_member_identity1003_committed_handler_undo_other_creator_refresh_cold_and_same_id_redo(){
    let f=Fixture::new();let mut w=f.link();let baseline=w.project().graphs().to_vec();
    let first=w.next_member_graph_id(&baseline).unwrap();
    let mut handler_models=baseline.clone();handler_models.push(actual_handler(first));
    let mut check=w.project().clone();check.apply_visual(w.project().source(),&handler_models).unwrap();
    w.with_graph_creation_admission(&[first], |_|Ok(())).unwrap();
    // Real model Undo is represented at the writer boundary by the complete
    // original graph set; saving it must retain the committed reservation.
    w.save(&baseline).unwrap();drop(w);
    let cold=SourceWorkspace::open(&f.0).unwrap().unwrap();
    assert!(cold.next_member_graph_id(cold.project().graphs()).unwrap().get()>first.get());
    let before_ids=cold.project().graphs().iter().map(|g|(g.kind.clone(),g.graph_id)).collect::<Vec<_>>();
    let mut text=fs::read_to_string(f.0.join("main.rvn")).unwrap();text.push_str("\nlabel another_scene\nreturn\n");
    fs::write(f.0.join("main.rvn"),text).unwrap();drop(cold);
    let mut refreshed=SourceWorkspace::open(&f.0).unwrap().unwrap();assert!(refreshed.source_error.is_none());
    let other=refreshed.project().graphs().iter().find(|g|matches!(&g.kind,crate::GraphKind::Label{name} if name=="another_scene")).unwrap().graph_id;
    assert!(other.get()>first.get());
    for (kind,id) in before_ids{assert!(refreshed.project().graphs().iter().any(|g|g.kind==kind&&g.graph_id==id));}
    let graphs=refreshed.project().graphs().to_vec();refreshed.save(&graphs).unwrap();drop(refreshed);
    let mut cold=SourceWorkspace::open(&f.0).unwrap().unwrap();assert!(cold.next_member_graph_id(cold.project().graphs()).unwrap().get()>other.get());
    let mut redo=cold.project().graphs().to_vec();redo.push(actual_handler(first));cold.save(&redo).unwrap();
    assert!(cold.project().graphs().iter().any(|g|g.graph_id==first));
    assert!(cold.project().graphs().iter().any(|g|g.graph_id==other));
}
#[test]
fn source_member_identity1003_common_batch_reserves_every_widget_id_and_pending_undo_recovery(){
    let f=Fixture::new();let mut w=f.link();let baseline=w.project().graphs().to_vec();
    let first=w.next_member_graph_id(&baseline).unwrap();let ids=(0..3).map(|n|crate::GraphId::new(first.get()+n)).collect::<Vec<_>>();
    assert!(w.with_graph_creation_admission(&[ids[0],ids[2]], |_|Ok(())).is_err());
    assert_eq!(w.next_member_graph_id(&baseline).unwrap(),first);
    w.with_graph_creation_admission(&ids, |_|Ok(())).unwrap();
    assert!(w.has_pending_graph_identity_reservation().unwrap());
    w.write_recovery(&baseline).unwrap();drop(w);
    let mut cold=SourceWorkspace::open(&f.0).unwrap().unwrap();
    assert!(cold.next_member_graph_id(cold.project().graphs()).unwrap().get()>ids[2].get());
    cold.save(&baseline).unwrap();assert!(!cold.has_pending_graph_identity_reservation().unwrap());
}

#[test]
fn source_member_identity1003_explicit_native_reserved_identity_is_refused_without_renumbering(){
    let f=Fixture::new();let mut w=f.link();let baseline=w.project().graphs().to_vec();
    let id=w.next_member_graph_id(&baseline).unwrap();
    let mut native=actual_handler(id);native.enable_native_frame_preparation().unwrap();
    let native_source=crate::transpile(&native).unwrap().source;
    w.with_graph_creation_admission(&[id], |_|Ok(())).unwrap();w.save(&baseline).unwrap();drop(w);
    let sidecar=fs::read(f.0.join(SIDECAR)).unwrap();
    let mut text=fs::read_to_string(f.0.join("main.rvn")).unwrap();text.push_str("\n");text.push_str(&native_source);
    fs::write(f.0.join("main.rvn"),&text).unwrap();
    let cold=SourceWorkspace::open(&f.0).unwrap().unwrap();
    assert!(cold.source_error.as_deref().is_some_and(|message|message.contains("reserved graph identity")));
    assert_eq!(cold.project().graphs(),baseline);assert_eq!(cold.graph_id_high_water(),id.get());
    assert_eq!(fs::read(f.0.join(SIDECAR)).unwrap(),sidecar);
    assert_eq!(fs::read_to_string(f.0.join("main.rvn")).unwrap(),text);
}
#[test]
fn source_member_identity1003_recovery_reservation_below_saved_history_refuses_neutrally(){
    let f=Fixture::new();let mut w=f.link();let baseline=w.project().graphs().to_vec();
    let id=w.next_member_graph_id(&baseline).unwrap();w.commit_member_graph_id(id).unwrap();w.save(&baseline).unwrap();
    w.write_recovery(&baseline).unwrap();let path=f.0.join(RECOVERY);
    let mut pending:serde_json::Value=serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    pending["graph_id_high_water"]=serde_json::json!(id.get()-1);fs::write(&path,serde_json::to_vec(&pending).unwrap()).unwrap();
    let recovery=fs::read(&path).unwrap();let sidecar=fs::read(f.0.join(SIDECAR)).unwrap();drop(w);
    let cold=SourceWorkspace::open(&f.0).unwrap().unwrap();assert!(cold.recovery_batch().is_err());
    assert_eq!(cold.graph_id_high_water(),id.get());assert_eq!(fs::read(&path).unwrap(),recovery);
    assert_eq!(fs::read(f.0.join(SIDECAR)).unwrap(),sidecar);
}

#[path="source_member_reservation_archive_tests1003.rs"]
mod reservation_archive;
