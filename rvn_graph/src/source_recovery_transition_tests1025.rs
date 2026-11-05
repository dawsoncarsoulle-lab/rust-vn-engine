use super::*;
use rvn_parser::enum_asset::{EnumAsset,EnumAssetId,EnumCommand};
use rvn_parser::enum_recovery1025::EnumRecoveryRecord1025 as Record;
const A:&str="17411a3e431e4c9388b78a2b9825b130";
const B:&str="797679920f7142be99d9d6fe5553ca34";
struct Fixture{root:PathBuf,source:SourceWorkspace}
impl Fixture{
    fn new()->Self{
        let root=std::env::temp_dir().join(format!("rvn-enum-transition1025-{}",unique_name()));
        fs::create_dir(&root).unwrap();fs::create_dir(root.join("assets")).unwrap();
        fs::write(root.join("story.rvn"),"label start\n\"Hello\"\n").unwrap();
        fs::write(root.join("rvn.toml"),"[project]\nname='test'\n").unwrap();
        let source=SourceWorkspace::link(&root,&root.join("story.rvn"),&[]).unwrap();Self{root,source}
    }
    fn record(&self,id:&str,name:&str)->Record{Record::unpublished(format!("assets/{name}.rvnenum"),EnumAsset::new(EnumAssetId::parse(id).unwrap(),name.into()).unwrap()).unwrap()}
    fn birth(&mut self,record:Record){self.source.transition_enum_recovery_with1025(&[EnumRecoveryChange1025::Upsert{before:None,after:record.clone()}],&[SourceRefactorFile{path:record.locator().into(),before:None,after:None}],||()).unwrap();}
    fn files(&self,record:&Record,wire:String)->Vec<SourceRefactorFile>{
        let manifest=fs::read_to_string(self.root.join("rvn.toml")).unwrap();
        vec![SourceRefactorFile{path:"rvn.toml".into(),before:Some(manifest.clone()),after:Some(format!("{manifest}\n[enum_assets]\n{}='{}'\n",record.asset_id().to_hex(),record.locator()))},SourceRefactorFile{path:record.locator().into(),before:record.saved_wire().map(str::to_owned),after:Some(wire)}]
    }
    fn observers(&self)->Vec<Vec<u8>>{["story.rvn",SIDECAR,"rvn.toml"].iter().map(|path|fs::read(self.root.join(path)).unwrap()).collect()}
    fn staged_journal(&self,record:&Record)->RefactorJournal{
        let before=self.source.expected_recovery.clone();let mut raw=self.source.raw_enum_recovery1025().unwrap();raw.enums.clear();
        let snapshot:Snapshot=serde_json::from_str(self.source.expected_sidecar.as_ref().unwrap()).unwrap();
        let after=serialize_after1025(raw,self.source.project.source(),self.source.expected_sidecar.as_ref().unwrap(),&snapshot).unwrap();
        let source=self.source.project.source().to_owned();let sidecar=self.source.expected_sidecar.clone();
        let mut files=vec![SourceRefactorFile{path:"story.rvn".into(),before:Some(source.clone()),after:Some(source)},SourceRefactorFile{path:SIDECAR.into(),before:sidecar.clone(),after:sidecar}];files.extend(self.files(record,record.current().encode().unwrap()));
        RefactorJournal{library_before1027:self.source.library_journal_readset1027(),library_after1027:self.source.library_journal_readset1027(),source:"story.rvn".into(),graphs:self.source.project.graphs().to_vec(),files,recovery:Some(JournalRecovery1025::Transition(RecoveryTransition1025{library1027:None,kind:"enum_recovery_transition_v4".into(),version:4,mode:TransitionMode1025::Metadata,before_recovery:before,after_recovery:after,changes:vec![EnumRecoveryChange1025::Remove{before:record.clone()}]}))}
    }
}
impl Drop for Fixture{fn drop(&mut self){assert!(self.root.starts_with(std::env::temp_dir()));assert!(self.root.file_name().unwrap().to_string_lossy().starts_with("rvn-enum-transition1025-"));let _=fs::remove_dir_all(&self.root);}}
#[test]
fn enum_transition1025_birth_is_durable_recovery_only_no_authored_file_or_index(){
    let mut f=Fixture::new();let original=f.observers();let a=f.record(A,"Alpha");
    let installed=std::cell::Cell::new(false);
    f.source.transition_enum_recovery_with1025(&[EnumRecoveryChange1025::Upsert{before:None,after:a.clone()}],&[SourceRefactorFile{path:a.locator().into(),before:None,after:None}],||installed.set(true)).unwrap();
    assert!(installed.get());assert_eq!(f.observers(),original);assert!(!f.root.join(a.locator()).exists());
    assert_eq!(f.source.recovery_batch().unwrap().unwrap().enums,vec![a.clone()]);
    let raw=fs::read_to_string(f.root.join(RECOVERY)).unwrap();assert_eq!(serde_json::from_str::<serde_json::Value>(&raw).unwrap()["version"],4);
    let cold=SourceWorkspace::open(&f.root).unwrap().unwrap();assert_eq!(cold.recovery_batch().unwrap().unwrap().enums,vec![a]);assert!(cold.recovery().is_err());assert!(!f.root.join(REFACTOR_JOURNAL).exists());
}
#[test]
fn enum_transition1025_first_save_uses_current_frozen_file_and_preserves_dirty_graph_and_enum_peers(){
    let mut f=Fixture::new();let a=f.record(A,"Alpha");let b=f.record(B,"Beta");
    let mut dirty=f.source.project.graphs().to_vec();dirty[0].nodes.values_mut().next().unwrap().title_override=Some("Unpublished peer graph".into());f.source.write_recovery(&dirty).unwrap();
    f.birth(a.clone());f.birth(b.clone());
    let before_source=fs::read(f.root.join("story.rvn")).unwrap();let before_sidecar=fs::read(f.root.join(SIDECAR)).unwrap();
    let newer=a.current().prepare(EnumCommand::DescribeAsset("Edited AFTER autosave".into())).unwrap().after().clone();let wire=newer.encode().unwrap();let files=f.files(&a,wire.clone());let baseline=f.source.project.graphs().to_vec();
    f.source.save_metadata_files_with_enum_recovery1025(&baseline,&files,&[EnumRecoveryChange1025::Remove{before:a.clone()}]).unwrap();
    assert_eq!(fs::read_to_string(f.root.join(a.locator())).unwrap(),wire);assert!(!f.root.join(b.locator()).exists());
    assert_eq!(fs::read(f.root.join("story.rvn")).unwrap(),before_source);assert_eq!(fs::read(f.root.join(SIDECAR)).unwrap(),before_sidecar);
    let batch=f.source.recovery_batch().unwrap().unwrap();assert_eq!(batch.graphs,dirty);assert_eq!(batch.enums,vec![b]);assert!(!f.root.join(REFACTOR_JOURNAL).exists());
}
#[test]
fn enum_transition1025_failed_exact_before_or_missing_reference_never_installs_or_changes_cache(){
    let mut f=Fixture::new();let a=f.record(A,"Alpha");f.birth(a.clone());let original=fs::read(f.root.join(RECOVERY)).unwrap();let observers=f.observers();
    let changed=Record::unpublished(a.locator().into(),a.current().prepare(EnumCommand::DescribeAsset("stale".into())).unwrap().after().clone()).unwrap();let installed=std::cell::Cell::new(false);
    assert!(f.source.transition_enum_recovery_with1025(&[EnumRecoveryChange1025::Remove{before:changed}],&[],||installed.set(true)).is_err());assert!(!installed.get());
    let b=f.record(B,"Beta");fs::write(f.root.join(b.locator()),"external").unwrap();assert!(f.source.transition_enum_recovery_with1025(&[EnumRecoveryChange1025::Upsert{before:None,after:b.clone()}],&[SourceRefactorFile{path:b.locator().into(),before:None,after:None}],||installed.set(true)).is_err());
    assert!(!installed.get());assert_eq!(fs::read(f.root.join(RECOVERY)).unwrap(),original);assert_eq!(f.observers(),observers);
}
#[test]
fn enum_transition1025_replay_before_partial_and_complete_publishes_one_exact_cohort(){
    for stage in 0..3{
        let mut f=Fixture::new();let a=f.record(A,"Alpha");f.birth(a.clone());let journal=f.staged_journal(&a);
        if stage>=1{let file=journal.files.iter().find(|file|file.path==a.locator()).unwrap();replace(&target(&f.source.root,&file.path).unwrap(),&file.after).unwrap();}
        if stage==2{for file in &journal.files{replace(&target(&f.source.root,&file.path).unwrap(),&file.after).unwrap();}replace_recovery1025(&f.source.root,&None).unwrap();}
        fs::write(f.root.join(REFACTOR_JOURNAL),serde_json::to_string(&journal).unwrap()).unwrap();
        let cold=SourceWorkspace::open(&f.root).unwrap().unwrap();assert!(cold.recovery_batch().unwrap().is_none());assert_eq!(fs::read_to_string(f.root.join(a.locator())).unwrap(),a.current().encode().unwrap());assert!(!f.root.join(REFACTOR_JOURNAL).exists());
    }
}
#[test]
fn enum_transition1025_external_conflict_preserves_journal_recovery_and_all_other_files(){
    let mut f=Fixture::new();let a=f.record(A,"Alpha");f.birth(a.clone());let journal=f.staged_journal(&a);let recovery=fs::read(f.root.join(RECOVERY)).unwrap();let original=f.observers();
    fs::write(f.root.join(a.locator()),"External file").unwrap();fs::write(f.root.join(REFACTOR_JOURNAL),serde_json::to_string(&journal).unwrap()).unwrap();
    assert!(SourceWorkspace::open(&f.root).is_err());assert_eq!(f.observers(),original);assert_eq!(fs::read(f.root.join(RECOVERY)).unwrap(),recovery);assert_eq!(fs::read(f.root.join(a.locator())).unwrap(),b"External file");assert!(f.root.join(REFACTOR_JOURNAL).exists());
}
#[test]
fn enum_transition1025_legacy_string_journal_reader_refuses_tagged_transition(){
    #[allow(dead_code)]#[derive(Deserialize)]struct OldJournal{source:String,graphs:Vec<GraphDocument>,files:Vec<SourceRefactorFile>,recovery:Option<String>}
    let mut f=Fixture::new();let a=f.record(A,"Alpha");f.birth(a.clone());let journal=f.staged_journal(&a);let wire=serde_json::to_string(&journal).unwrap();
    assert!(serde_json::from_str::<OldJournal>(&wire).is_err());let mut v:serde_json::Value=serde_json::from_str(&wire).unwrap();v["recovery"]["unknown_grant"]=true.into();assert!(serde_json::from_value::<RefactorJournal>(v).is_err());
    let legacy=RefactorJournal{library_before1027:None,library_after1027:None,source:journal.source,graphs:journal.graphs,files:journal.files,recovery:Some(JournalRecovery1025::LegacyArchive("owned legacy bytes".into()))};assert!(serde_json::from_str::<OldJournal>(&serde_json::to_string(&legacy).unwrap()).is_ok());
}
#[test]
fn enum_transition1025_legacy_maintenance_preserves_records_and_legacy_save_archive_refuse(){
    let mut f=Fixture::new();let a=f.record(A,"Alpha");f.birth(a.clone());let baseline=f.source.project.graphs().to_vec();
    f.source.write_recovery_with_widget_memberships(&baseline,&[],&[]).unwrap();assert_eq!(f.source.recovery_batch().unwrap().unwrap().enums,vec![a.clone()]);let original=fs::read(f.root.join(RECOVERY)).unwrap();
    assert!(f.source.archive_recovery().is_err());assert!(f.source.save(&baseline).is_err());assert!(f.source.save_metadata_files_preserving_authoring1025(&baseline,&f.files(&a,a.current().encode().unwrap())).is_err());assert_eq!(fs::read(f.root.join(RECOVERY)).unwrap(),original);assert!(!f.root.join(a.locator()).exists());
}
#[test]
fn enum_transition1025_scoped_discard_preserves_peers_global_discard_archives_all_before_install(){
    let mut f=Fixture::new();let a=f.record(A,"Alpha");let b=f.record(B,"Beta");f.birth(a.clone());f.birth(b.clone());
    f.source.transition_enum_recovery_with1025(&[EnumRecoveryChange1025::Remove{before:a}],&[],||()).unwrap();assert_eq!(f.source.recovery_batch().unwrap().unwrap().enums,vec![b]);
    let batch=f.source.recovery_batch().unwrap().unwrap();let installed=std::cell::Cell::new(false);f.source.discard_enum_recovery_with1025(&batch,||{assert!(!f.root.join(RECOVERY).exists());installed.set(true)}).unwrap();assert!(installed.get());assert!(f.source.recovery_batch().unwrap().is_none());assert!(!f.root.join(REFACTOR_JOURNAL).exists());
    assert!(fs::read_dir(f.root.join(".rvn-backups")).unwrap().any(|entry|entry.unwrap().path().join("recovery.json").exists()));
}

#[test]
fn enum_transition1025_owned_partial_compensation_restores_before_but_never_external_bytes(){
    for external in [false,true]{
        let mut f=Fixture::new();let a=f.record(A,"Alpha");f.birth(a.clone());let journal=f.staged_journal(&a);let before_cache=fs::read(f.root.join(RECOVERY)).unwrap();
        let transition=match journal.recovery.as_ref().unwrap(){JournalRecovery1025::Transition(value)=>value,_=>unreachable!()};
        for file in &journal.files{replace(&target(&f.source.root,&file.path).unwrap(),&file.after).unwrap();}replace_recovery1025(&f.source.root,&transition.after_recovery).unwrap();
        if external{fs::write(f.root.join(a.locator()),"Intervening external data").unwrap();}
        let failures=compensate1025(&f.source.root,&journal,transition);assert_eq!(failures.is_empty(),!external);
        assert_eq!(fs::read(f.root.join(RECOVERY)).unwrap(),before_cache);
        if external{assert_eq!(fs::read(f.root.join(a.locator())).unwrap(),b"Intervening external data");}
        else{assert!(!f.root.join(a.locator()).exists());}
        for file in journal.files.iter().filter(|file|file.path!=a.locator()){assert_eq!(read_optional(&target(&f.source.root,&file.path).unwrap()).unwrap(),file.before);}
    }
}
#[test]
fn enum_transition1025_complete_source_save_keeps_unmentioned_unpublished_enum_recoverable(){
    let mut f=Fixture::new();let a=f.record(A,"Alpha");let b=f.record(B,"Beta");let mut graphs=f.source.project.graphs().to_vec();graphs[0].nodes.values_mut().next().unwrap().title_override=Some("Now publish full Source snapshot".into());f.source.write_recovery(&graphs).unwrap();f.birth(a.clone());f.birth(b.clone());
    let files=f.files(&a,a.current().encode().unwrap());f.source.save_with_files_and_enum_recovery1025(&graphs,&files,&[EnumRecoveryChange1025::Remove{before:a}]).unwrap();
    let batch=f.source.recovery_batch().unwrap().unwrap();assert_eq!(batch.enums,vec![b.clone()]);assert_eq!(batch.graphs,f.source.project.graphs());assert_eq!(f.source.project.graphs(),graphs);assert!(!f.root.join(b.locator()).exists());
    let cold=SourceWorkspace::open(&f.root).unwrap().unwrap();assert_eq!(cold.recovery_batch().unwrap().unwrap(),batch);
}

#[test]
fn enum_transition1025_birth_persists_existing_fresh_graph_reservation_without_publishing_graphs(){
    let mut f=Fixture::new();let before_source=fs::read(f.root.join("story.rvn")).unwrap();let graphs=f.source.project.graphs().to_vec();let fresh=f.source.next_member_graph_id(&graphs).unwrap();f.source.commit_member_graph_id(fresh).unwrap();let a=f.record(A,"Alpha");f.birth(a);
    assert_eq!(fs::read(f.root.join("story.rvn")).unwrap(),before_source);let cold=SourceWorkspace::open(&f.root).unwrap().unwrap();assert!(cold.graph_id_high_water()>=fresh.get());assert_eq!(cold.project.graphs(),graphs);
}

#[test]
fn enum_transition1025_rejects_early_after_cache_without_mutating_journal_files_or_cache(){
    for retain_peer in [false,true]{
        let mut f=Fixture::new();let a=f.record(A,"Alpha");f.birth(a.clone());
        if retain_peer{let b=f.record(B,"Beta");f.birth(b);}
        let mut journal=f.staged_journal(&a);
        let mut after=f.source.raw_enum_recovery1025().unwrap();after.enums.retain(|record|record.asset_id()!=a.asset_id());
        let snapshot:Snapshot=serde_json::from_str(f.source.expected_sidecar.as_ref().unwrap()).unwrap();
        let after=serialize_after1025(after,f.source.project.source(),f.source.expected_sidecar.as_ref().unwrap(),&snapshot).unwrap();
        let transition=match journal.recovery.as_mut().unwrap(){JournalRecovery1025::Transition(value)=>value,_=>unreachable!()};transition.after_recovery=after.clone();
        // The writer always publishes all files FIRST. Simulate an external
        // premature cache update/deletion, not an authentic crash stage.
        replace_recovery1025(&f.source.root,&after).unwrap();
        let original_files=f.observers();let wire=serde_json::to_string(&journal).unwrap();fs::write(f.root.join(REFACTOR_JOURNAL),&wire).unwrap();
        assert!(SourceWorkspace::open(&f.root).is_err());
        assert_eq!(f.observers(),original_files);assert_eq!(read_optional(&f.root.join(RECOVERY)).unwrap(),after);
        assert_eq!(fs::read_to_string(f.root.join(REFACTOR_JOURNAL)).unwrap(),wire);assert!(!f.root.join(a.locator()).exists());
    }
}
#[test]
fn enum_transition1025_equal_before_after_cache_does_not_block_actual_file_publication(){
    let mut f=Fixture::new();let b=f.record(B,"Beta");f.birth(b.clone());let before=fs::read(f.root.join(RECOVERY)).unwrap();
    let a=f.record(A,"Alpha");let files=f.files(&a,a.current().encode().unwrap());let baseline=f.source.project.graphs().to_vec();
    // No Enum recovery participant changes; the retained peer bytes are equal
    // before/after and therefore cannot prove anything about file ordering.
    f.source.save_metadata_files_with_enum_recovery1025(&baseline,&files,&[]).unwrap();
    assert_eq!(fs::read(f.root.join(RECOVERY)).unwrap(),before);assert_eq!(f.source.recovery_batch().unwrap().unwrap().enums,vec![b]);
    assert_eq!(fs::read_to_string(f.root.join(a.locator())).unwrap(),a.current().encode().unwrap());assert!(!f.root.join(REFACTOR_JOURNAL).exists());
}
