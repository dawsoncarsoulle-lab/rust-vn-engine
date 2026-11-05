//! Exact metadata-only archival and the existing source transaction recovery.
use super::*;
fn pending()->(Fixture,SourceWorkspace,crate::GraphId,Vec<GraphDocument>){
    let f=Fixture::new();let mut w=f.link();let graphs=w.project().graphs().to_vec();let id=w.next_member_graph_id(&graphs).unwrap();
    w.with_graph_creation_admission(&[id], |_|Ok(())).unwrap();w.write_recovery(&graphs).unwrap();(f,w,id,graphs)
}
#[test]
fn source_reserve_archive1003_discard_persists_floor_only_preserving_unknown_sidecar_fields(){
    let f=Fixture::new();drop(f.link());let sidecar_path=f.0.join(SIDECAR);
    let mut wire:serde_json::Value=serde_json::from_slice(&fs::read(&sidecar_path).unwrap()).unwrap();wire["private_neighbor_metadata"]=serde_json::json!({"retained":[1,2,3]});fs::write(&sidecar_path,serde_json::to_vec_pretty(&wire).unwrap()).unwrap();
    let mut w=SourceWorkspace::open(&f.0).unwrap().unwrap();let graphs=w.project().graphs().to_vec();let id=w.next_member_graph_id(&graphs).unwrap();let source=fs::read(f.0.join("main.rvn")).unwrap();
    w.commit_member_graph_id(id).unwrap();w.write_recovery(&graphs).unwrap();w.archive_recovery().unwrap();
    let mut after:serde_json::Value=serde_json::from_slice(&fs::read(&sidecar_path).unwrap()).unwrap();assert_eq!(after["graph_id_high_water"],id.get());
    after.as_object_mut().unwrap().remove("graph_id_high_water");wire.as_object_mut().unwrap().remove("graph_id_high_water");assert_eq!(after,wire);
    assert!(!f.0.join(RECOVERY).exists());assert_eq!(fs::read(f.0.join("main.rvn")).unwrap(),source);assert_eq!(w.project().graphs(),graphs);drop(w);
    let cold=SourceWorkspace::open(&f.0).unwrap().unwrap();assert!(cold.recovery_batch().unwrap().is_none());assert!(cold.next_member_graph_id(&graphs).unwrap().get()>id.get());
}
#[test]
fn source_reserve_archive1003_each_sidecar_and_recovery_cas_refuses_without_publication(){
    for file in [SIDECAR,RECOVERY]{let(f,mut w,id,graphs)=pending();let source=fs::read(f.0.join("main.rvn")).unwrap();
        let path=f.0.join(file);let mut changed=fs::read(&path).unwrap();changed.extend_from_slice(b"\n ");fs::write(&path,&changed).unwrap();
        let sidecar=fs::read(f.0.join(SIDECAR)).unwrap();let recovery=fs::read(f.0.join(RECOVERY)).unwrap();
        assert!(w.archive_recovery().is_err(),"{file}");assert_eq!(fs::read(f.0.join(SIDECAR)).unwrap(),sidecar);assert_eq!(fs::read(f.0.join(RECOVERY)).unwrap(),recovery);
        assert_eq!(fs::read(f.0.join("main.rvn")).unwrap(),source);assert_eq!(w.graph_id_high_water(),id.get());assert_eq!(w.project().graphs(),graphs);assert!(!f.0.join(JOURNAL).exists());
    }
}
#[test]
fn source_reserve_archive1003_existing_transaction_resumes_each_actual_publication_phase(){
    for phase in 0..3{let(f,w,id,graphs)=pending();let source=fs::read(f.0.join("main.rvn")).unwrap();let transaction=w.prepare_reservation_archive().unwrap();
        fs::write(f.0.join(JOURNAL),serde_json::to_vec(&transaction).unwrap()).unwrap();
        if phase>=1{fs::write(f.0.join(SIDECAR),&transaction.after_sidecar).unwrap();}
        if phase>=2{fs::remove_file(f.0.join(RECOVERY)).unwrap();}drop(w);
        let cold=SourceWorkspace::open(&f.0).unwrap().unwrap();assert_eq!(cold.project().graphs(),graphs);assert!(cold.graph_id_high_water()>=id.get());
        assert!(cold.next_member_graph_id(&graphs).unwrap().get()>id.get());assert!(cold.recovery_batch().unwrap().is_none());assert!(!f.0.join(JOURNAL).exists());assert_eq!(fs::read(f.0.join("main.rvn")).unwrap(),source);
    }
}
#[test]
fn source_reserve_archive1003_durable_record_refuses_any_non_reserve_change_or_foreign_recovery(){
    for mode in 0..3{let(f,w,_,_)=pending();let mut transaction=w.prepare_reservation_archive().unwrap();
        if mode==0{let mut value:serde_json::Value=serde_json::from_str(&transaction.after_sidecar).unwrap();value["unexpected"]=serde_json::json!(true);transaction.after_sidecar=serde_json::to_string(&value).unwrap();}
        if mode==1{transaction.after_source.push_str("// forbidden source change\n");}
        if mode==2{fs::write(f.0.join(RECOVERY),"foreign recovery owner").unwrap();}
        fs::write(f.0.join(JOURNAL),serde_json::to_vec(&transaction).unwrap()).unwrap();let source=fs::read(f.0.join("main.rvn")).unwrap();let sidecar=fs::read(f.0.join(SIDECAR)).unwrap();let recovery=fs::read(f.0.join(RECOVERY)).unwrap();drop(w);
        assert!(SourceWorkspace::open(&f.0).is_err());assert_eq!(fs::read(f.0.join("main.rvn")).unwrap(),source);assert_eq!(fs::read(f.0.join(SIDECAR)).unwrap(),sidecar);assert_eq!(fs::read(f.0.join(RECOVERY)).unwrap(),recovery);assert!(f.0.join(JOURNAL).exists());
    }
}
#[test]
fn source_reserve_archive1003_source_conflict_is_not_regenerated_to_persist_identity(){
    let(f,mut w,id,_)=pending();let mut changed=fs::read_to_string(f.0.join("main.rvn")).unwrap();changed.push_str("this is deliberately invalid external RVN\n");fs::write(f.0.join("main.rvn"),&changed).unwrap();
    w.archive_recovery().unwrap();assert_eq!(fs::read_to_string(f.0.join("main.rvn")).unwrap(),changed);drop(w);
    let cold=SourceWorkspace::open(&f.0).unwrap().unwrap();assert!(cold.source_error.is_some());assert!(cold.graph_id_high_water()>=id.get());assert!(!f.0.join(RECOVERY).exists());
}
#[test]
fn source_reserve_archive1003_clean_model_save_carries_real_recovery_until_same_writer_publication(){
    let(f,mut w,id,graphs)=pending();let source=fs::read(f.0.join("main.rvn")).unwrap();w.save(&graphs).unwrap();assert!(!f.0.join(RECOVERY).exists());assert!(!f.0.join(refactor::REFACTOR_JOURNAL).exists());
    assert_eq!(fs::read(f.0.join("main.rvn")).unwrap(),source);drop(w);let cold=SourceWorkspace::open(&f.0).unwrap().unwrap();assert!(cold.next_member_graph_id(&graphs).unwrap().get()>id.get());assert_eq!(cold.project().graphs(),graphs);
}
