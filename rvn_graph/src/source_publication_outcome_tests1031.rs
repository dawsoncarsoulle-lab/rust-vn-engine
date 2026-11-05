//! Real files/typed publishers and exact crash images. CPU tests, no UI grant.
use super::*;
use crate::source_function_library_recovery1023::{FunctionLibraryRecovery,FunctionLibraryWire,LibraryRecoveryState};
use rvn_parser::function_library_asset1023::{FunctionLibraryAsset,FunctionLibraryId};
use rvn_parser::enum_asset::{EnumAsset,EnumAssetId};
use rvn_parser::enum_recovery1025::EnumRecoveryRecord1025;
use std::{cell::RefCell,collections::BTreeMap,rc::Rc};
const A:&str="11111111111141118111111111111111";
const E:&str="99999999999949998999999999999999";
struct Fixture { root:PathBuf,workspace:SourceWorkspace }
impl Fixture {
    fn new(scoped:bool)->Self {
        let path=std::env::temp_dir().join(format!("rvn-durable-outcome1031-{}",unique_name()));
        fs::create_dir(&path).unwrap();let root=fs::canonicalize(path).unwrap();fs::create_dir(root.join("assets")).unwrap();
        fs::write(root.join("story.rvn"),"scene label start\n\"Before\"\n").unwrap();
        fs::write(root.join("rvn.toml"),"[paths]\nassets='assets'\n[project]\nname='genuine durable fixture'\n").unwrap();
        let workspace=if scoped{SourceWorkspace::link_scoped_libraries1027(&root,&root.join("story.rvn"),&[])}else{SourceWorkspace::link(&root,&root.join("story.rvn"),&[])}.unwrap();
        Self { root,workspace }
    }
    fn library()->Self {
        let mut f=Self::new(true);let id=FunctionLibraryId::parse(A).unwrap();
        let asset=FunctionLibraryAsset::new(id.clone(),Vec::new()).unwrap();
        let record=FunctionLibraryRecovery::new(id,None,Some(LibraryRecoveryState::new("assets/Alpha.rvnlibrary".into(),FunctionLibraryWire::from_asset(&asset).unwrap()).unwrap())).unwrap();
        let graphs=f.workspace.project.graphs().to_vec();
        f.workspace.replace_recovery_with_library_changes1027(None,&graphs,&[],&[],&[],&[LibraryRecoveryChange1027::Upsert{before:None,after:record}],&[],&[],&[],||()).unwrap();
        // A genuine unmaterialized ID reservation must remain retired after discard.
        let current=f.workspace.recovery_cohort_libraries1027().unwrap().unwrap();
        let reserved=f.workspace.next_library_member_graph_id1027(Some(&current)).unwrap();f.workspace.commit_member_graph_id(reserved).unwrap();f.workspace.write_recovery(&current.batch.graphs).unwrap();
        f
    }
    fn cohort(&self)->SourceRecoveryCohortLibrary1027 { self.workspace.recovery_cohort_libraries1027().unwrap().unwrap() }
    fn images(&self)->BTreeMap<String,Option<Vec<u8>>> {
        ["story.rvn","rvn.toml",SIDECAR,RECOVERY,"assets/Alpha.rvnlibrary","assets/State.rvnenum"].into_iter().map(|p|(p.into(),fs::read(self.root.join(p)).ok())).collect()
    }
    fn backups(&self)->BTreeMap<PathBuf,Vec<u8>> {
        fn visit(root:&Path,path:&Path,out:&mut BTreeMap<PathBuf,Vec<u8>>){
            if !path.is_dir(){return;}for item in fs::read_dir(path).unwrap(){let p=item.unwrap().path();if p.is_dir(){visit(root,&p,out)}else{out.insert(p.strip_prefix(root).unwrap().to_path_buf(),fs::read(p).unwrap());}}
        }
        let mut out=BTreeMap::new();visit(&self.root,&self.root.join(".rvn-backups"),&mut out);out
    }
    fn metadata(&mut self)->RefactorJournal {
        let record=EnumRecoveryRecord1025::unpublished("assets/State.rvnenum".into(),EnumAsset::new(EnumAssetId::parse(E).unwrap(),"State".into()).unwrap()).unwrap();
        self.workspace.transition_enum_recovery_with1025(&[EnumRecoveryChange1025::Upsert{before:None,after:record.clone()}],&[SourceRefactorFile{path:record.locator().into(),before:None,after:None}],||()).unwrap();
        let before=self.workspace.expected_recovery.clone();let mut after=self.workspace.raw_enum_recovery1025().unwrap();after.enums.clear();
        let snapshot:Snapshot=serde_json::from_str(self.workspace.expected_sidecar.as_deref().unwrap()).unwrap();
        let after=serialize_after1025(after,self.workspace.project.source(),self.workspace.expected_sidecar.as_deref().unwrap(),&snapshot).unwrap();
        let text=self.workspace.project.source().to_owned();let sidecar=self.workspace.expected_sidecar.clone();let manifest=fs::read_to_string(self.root.join("rvn.toml")).unwrap();
        RefactorJournal{library_before1027:None,library_after1027:None,source:"story.rvn".into(),graphs:self.workspace.project.graphs().to_vec(),files:vec![
            SourceRefactorFile{path:"story.rvn".into(),before:Some(text.clone()),after:Some(text)},
            SourceRefactorFile{path:SIDECAR.into(),before:sidecar.clone(),after:sidecar},
            SourceRefactorFile{path:"rvn.toml".into(),before:Some(manifest.clone()),after:Some(format!("{manifest}\n[enum_assets]\n{E}='assets/State.rvnenum'\n"))},
            SourceRefactorFile{path:record.locator().into(),before:None,after:Some(record.current().encode().unwrap())},
        ],recovery:Some(JournalRecovery1025::Transition(RecoveryTransition1025{kind:"enum_recovery_transition_v4".into(),version:4,mode:TransitionMode1025::Metadata,before_recovery:before,after_recovery:after,changes:vec![EnumRecoveryChange1025::Remove{before:record}],library1027:None}))}
    }
}
impl Drop for Fixture {fn drop(&mut self){assert!(self.root.file_name().unwrap().to_string_lossy().starts_with("rvn-durable-outcome1031-"));let _=fs::remove_dir_all(&self.root);}}
fn execution(fail:Fault1031)->(Execution1031,Rc<RefCell<Probe1031>>){let p=Rc::new(RefCell::new(Probe1031{fail,..Default::default()}));(Execution1031{probe:Some(p.clone())},p)}
fn apply_metadata(f:&Fixture,journal:&RefactorJournal,execution:&Execution1031)->Result<VerifiedCommitted1031,String>{
    let _lock=lock(&f.root)?;
    let wire=serde_json::to_string(journal).unwrap();let original=Box::new(PreparedCommit1031::prepare(&f.root,&journal.source,wire));
    execution.before_journal()?;atomic_write(&f.root.join(REFACTOR_JOURNAL),original.wire())?;
    super::super::super::apply_typed_verified1031(&f.root,journal,false,original,execution,false)
}
fn assert_after(f:&Fixture,journal:&RefactorJournal){for file in &journal.files{assert_eq!(read_optional(&f.root.join(&file.path)).unwrap(),file.after);}let Some(JournalRecovery1025::Transition(t))=&journal.recovery else{panic!("real typed journal")};assert_eq!(read_optional(&f.root.join(RECOVERY)).unwrap(),t.after_recovery);}

#[test]
fn durable1031_public_discard_returns_real_committed_fact_and_adopts_only_saved_core_without_user_installer(){
    let mut f=Fixture::library();let current=f.cohort();let high=f.workspace.graph_id_high_water();let source=f.workspace.project.clone();let before=f.images();
    let proof=f.workspace.discard_library_recovery_verified1031(&current).unwrap();
    assert_eq!(proof.cleanup1031(),CleanupStatus1031::Removed);assert_eq!(proof.root1031(),f.root.as_path());assert_eq!(proof.source1031(),"story.rvn");
    assert_eq!(f.workspace.project.graphs(),source.graphs());assert_eq!(f.workspace.project.source(),source.source());assert!(f.workspace.recovery_cohort_libraries1027().unwrap().is_none());
    assert!(f.workspace.graph_id_high_water()>=high);assert!(f.workspace.snapshot_matches_project);assert!(!f.root.join(REFACTOR_JOURNAL).exists());
    for p in ["story.rvn","rvn.toml","assets/Alpha.rvnlibrary"]{assert_eq!(f.images()[p],before[p]);}
    let journal:RefactorJournal=serde_json::from_str(proof.original_journal1031()).unwrap();let Some(JournalRecovery1025::Transition(t))=journal.recovery else{panic!()};
    assert_eq!(t.mode,TransitionMode1025::DiscardAll);assert!(t.library1027.unwrap().saved.is_empty(),"discard is not a Saved acknowledgement");
    let cold=SourceWorkspace::open_scoped_libraries1027(&f.root,0).unwrap().unwrap();assert!(cold.recovery_cohort_libraries1027().unwrap().is_none());assert!(cold.graph_id_high_water()>=high);
}
#[test]
fn durable1031_before_journal_failure_never_mints_changes_core_or_targets(){
    let mut f=Fixture::library();let current=f.cohort();let images=f.images();let backups=f.backups();let core=f.workspace.expected_sidecar.clone();let(ex,p)=execution(Fault1031::BeforeJournal);
    assert!(f.workspace.discard_library_verified_inner1031(&current,&ex).is_err());assert_eq!(f.images(),images);assert_eq!(f.backups(),backups);assert_eq!(f.workspace.expected_sidecar,core);assert_eq!(f.cohort(),current);assert!(!f.root.join(REFACTOR_JOURNAL).exists());
    let p=p.borrow();assert_eq!(p.prepared,1);assert_eq!((p.backups,p.target_writes,p.recovery_writes,p.mints,p.cleanup_attempts),(0,0,0,0,0));
}
#[test]
fn durable1031_real_backup_namespace_failure_retains_original_journal_and_current_for_actual_cold_recovery(){
    let mut f=Fixture::library();let current=f.cohort();let images=f.images();let high=f.workspace.graph_id_high_water();let original=f.root.join(".rvn-backups");let held=f.root.join("held-backups");fs::rename(&original,&held).unwrap();fs::write(&original,b"actual occupied backup namespace").unwrap();
    let(ex,p)=execution(Fault1031::None);let result=f.workspace.discard_library_verified_inner1031(&current,&ex);
    assert!(result.is_err());assert_eq!(f.images(),images);assert_eq!(f.cohort(),current);assert_eq!(p.borrow().mints,0);assert!(f.root.join(REFACTOR_JOURNAL).is_file());
    fs::remove_file(&original).unwrap();fs::rename(&held,&original).unwrap();
    let cold=SourceWorkspace::open_scoped_libraries1027(&f.root,0).unwrap().unwrap();assert!(cold.recovery_cohort_libraries1027().unwrap().is_none());assert!(cold.graph_id_high_water()>=high);
}
#[test]
fn durable1031_nth_actual_target_failure_compensates_prior_target_and_returns_no_committed_fact(){
    let mut f=Fixture::new(false);let journal=f.metadata();let images=f.images();let(ex,p)=execution(Fault1031::Target(1));
    assert!(apply_metadata(&f,&journal,&ex).is_err());assert_eq!(f.images(),images);assert!(!f.root.join(REFACTOR_JOURNAL).exists());let p=p.borrow();assert_eq!(p.target_writes,1);assert_eq!(p.mints,0);assert_eq!(p.recovery_writes,0);
}
#[test]
fn durable1031_actual_recovery_write_failure_compensates_all_written_targets_without_mint(){
    let mut f=Fixture::new(false);let journal=f.metadata();let images=f.images();let(ex,p)=execution(Fault1031::Recovery);
    assert!(apply_metadata(&f,&journal,&ex).is_err());assert_eq!(f.images(),images);assert!(!f.root.join(REFACTOR_JOURNAL).exists());let p=p.borrow();assert_eq!(p.target_writes,2);assert_eq!(p.mints,0);assert_eq!(p.cleanup_attempts,0);
}
#[test]
fn durable1031_failure_before_full_exact_readback_is_compensated_not_inferred_from_completed_writes(){
    let mut f=Fixture::new(false);let journal=f.metadata();let images=f.images();let(ex,p)=execution(Fault1031::Readback);
    assert!(apply_metadata(&f,&journal,&ex).is_err());assert_eq!(f.images(),images);let p=p.borrow();assert_eq!((p.target_writes,p.recovery_writes,p.mints),(2,1,0));assert!(!f.root.join(REFACTOR_JOURNAL).exists());
}
#[test]
fn durable1031_cleanup_failure_is_committed_warning_and_core_adoption_not_a_beforecommit_error(){
    let mut f=Fixture::library();let current=f.cohort();let graphs=f.workspace.project.graphs().to_vec();let(ex,p)=execution(Fault1031::Cleanup);
    let proof=f.workspace.discard_library_verified_inner1031(&current,&ex).unwrap();
    let CleanupStatus1031::Pending(warning)=proof.cleanup1031()else{panic!("real cleanup warning")};assert_eq!(warning.kind1031(),io::ErrorKind::PermissionDenied);assert_eq!(warning.os_code1031(),None);
    assert_eq!(read_optional(&f.root.join(REFACTOR_JOURNAL)).unwrap().as_deref(),Some(proof.original_journal1031()));assert!(f.workspace.expected_recovery.is_none());assert_eq!(f.workspace.project.graphs(),graphs);assert!(f.workspace.snapshot_matches_project);
    let journal:RefactorJournal=serde_json::from_str(proof.original_journal1031()).unwrap();assert_after(&f,&journal);let p=p.borrow();assert_eq!((p.prepared,p.mints,p.cleanup_attempts),(1,1,1));assert!(p.target_writes>0);assert_eq!(p.recovery_writes,1);
}
#[test]
fn durable1031_cleanup_only_retry_revalidates_same_proof_and_has_zero_targets_recovery_backup_or_installer(){
    let mut f=Fixture::new(false);let journal=f.metadata();let(ex,_)=execution(Fault1031::Cleanup);let proof=apply_metadata(&f,&journal,&ex).unwrap();assert_after(&f,&journal);let images=f.images();let backups=f.backups();let(retry,p)=execution(Fault1031::None);
    let after={let _lock=lock(&f.root).unwrap();super::super::super::cleanup_typed_journal1031(&f.root,&proof,&retry).unwrap()};
    assert_eq!(after.cleanup1031(),CleanupStatus1031::Removed);assert_eq!(after.original_journal1031(),proof.original_journal1031());assert_eq!(f.images(),images);assert_eq!(f.backups(),backups);assert!(!f.root.join(REFACTOR_JOURNAL).exists());let p=p.borrow();assert_eq!((p.target_writes,p.recovery_writes,p.backups,p.mints,p.cleanup_attempts),(0,0,0,1,1));
}
#[test]
fn durable1031_cleanup_retry_refuses_external_target_or_journal_and_retains_original_proof_without_io(){
    for journal_change in [false,true]{let mut f=Fixture::new(false);let journal=f.metadata();let(ex,_)=execution(Fault1031::Cleanup);let proof=apply_metadata(&f,&journal,&ex).unwrap();
        if journal_change{fs::write(f.root.join(REFACTOR_JOURNAL),format!("{}\n",proof.original_journal1031())).unwrap();}else{fs::write(f.root.join("story.rvn"),"external third state\n").unwrap();}
        let images=f.images();let bytes=fs::read(f.root.join(REFACTOR_JOURNAL)).unwrap();let backups=f.backups();let(retry,p)=execution(Fault1031::None);
        let result={let _lock=lock(&f.root).unwrap();super::super::super::cleanup_typed_journal1031(&f.root,&proof,&retry)};assert!(result.is_err());assert_eq!(f.images(),images);assert_eq!(fs::read(f.root.join(REFACTOR_JOURNAL)).unwrap(),bytes);assert_eq!(f.backups(),backups);assert_eq!(p.borrow().mints,0);assert!(matches!(proof.cleanup1031(),CleanupStatus1031::Pending(_)));
    }
}
#[test]
fn durable1031_cleanup_only_refuses_half_publication_and_legacy_replay_completes_the_same_real_typed_cohort(){
    let mut f=Fixture::new(false);let journal=f.metadata();let(ex,_)=execution(Fault1031::Cleanup);let proof=apply_metadata(&f,&journal,&ex).unwrap();
    let descriptor=journal.files.iter().find(|file|file.path=="assets/State.rvnenum").unwrap();replace(&f.root.join(&descriptor.path),&descriptor.before).unwrap();
    let Some(JournalRecovery1025::Transition(t))=&journal.recovery else{panic!()};replace_recovery1025(&f.root,&t.before_recovery).unwrap();
    let images=f.images();let backups=f.backups();assert!(SourceWorkspace::cleanup_typed_journal_verified1031(&f.root,&proof).is_err());assert_eq!(f.images(),images);assert_eq!(f.backups(),backups);
    let cold=SourceWorkspace::open(&f.root).unwrap().unwrap();assert!(cold.recovery_batch().unwrap().is_none());assert_after(&f,&journal);assert!(!f.root.join(REFACTOR_JOURNAL).exists());
}
#[test]
fn durable1031_legacy_typed_cleanup_keeps_strict_error_and_cold_maintenance_refusal_policy(){
    let mut f=Fixture::new(false);let journal=f.metadata();let wire=serde_json::to_string(&journal).unwrap();fs::write(f.root.join(REFACTOR_JOURNAL),wire).unwrap();let Some(JournalRecovery1025::Transition(t))=&journal.recovery else{panic!()};let(ex,p)=execution(Fault1031::Cleanup);
    let result={let _lock=lock(&f.root).unwrap();super::super::super::validate_apply_header1031(&f.root,&journal,false).unwrap();execute_legacy_inner1031(&f.root,&journal,t,false,&ex)};assert!(result.is_err());assert_after(&f,&journal);assert_eq!(p.borrow().mints,0);assert!(f.root.join(REFACTOR_JOURNAL).exists());
    #[cfg(windows)]{
        use std::os::windows::fs::OpenOptionsExt;
        let held=fs::OpenOptions::new().read(true).share_mode(1|2).open(f.root.join(REFACTOR_JOURNAL)).unwrap();
        assert!(SourceWorkspace::open(&f.root).is_err(),"legacy cold open must retain its maintenance refusal on actual OS cleanup denial");assert_after(&f,&journal);assert!(f.root.join(REFACTOR_JOURNAL).exists());drop(held);
    }
    let cold=SourceWorkspace::open(&f.root).unwrap().unwrap();assert!(cold.recovery_batch().unwrap().is_none());assert_after(&f,&journal);
}
#[test]
fn durable1031_pending_cleanup_refuses_new_verified_writer_without_second_io_or_new_journal(){
    let mut f=Fixture::library();let current=f.cohort();let(ex,p)=execution(Fault1031::Cleanup);let proof=f.workspace.discard_library_verified_inner1031(&current,&ex).unwrap();let images=f.images();let backups=f.backups();let bytes=fs::read(f.root.join(REFACTOR_JOURNAL)).unwrap();let counts=(p.borrow().target_writes,p.borrow().recovery_writes,p.borrow().mints);
    assert!(f.workspace.discard_library_verified_inner1031(&current,&ex).unwrap_err().contains("existing Source transaction"));assert_eq!(f.images(),images);assert_eq!(f.backups(),backups);assert_eq!(fs::read(f.root.join(REFACTOR_JOURNAL)).unwrap(),bytes);assert_eq!((p.borrow().target_writes,p.borrow().recovery_writes,p.borrow().mints),counts);assert!(f.workspace.expected_recovery.is_none());
    let after=SourceWorkspace::cleanup_typed_journal_verified1031(&f.root,&proof).unwrap();assert_eq!(after.cleanup1031(),CleanupStatus1031::Removed);
}
#[test]
fn durable1031_opaque_owns_true_send_data_preallocation_precedes_io_and_ordinary_only_cannot_promote_to_v5(){
    let mut f=Fixture::library();let current=f.cohort();let(ex,p)=execution(Fault1031::None);let proof=f.workspace.discard_library_verified_inner1031(&current,&ex).unwrap();fn send<T:Send>(_:&T){}send(&proof);
    assert_eq!((p.borrow().prepared,p.borrow().mints),(1,1));let expected=proof.original_journal1031().to_owned();let proof=std::thread::spawn(move||proof).join().unwrap();assert_eq!(proof.original_journal1031(),expected);
    let mut ordinary=Fixture::new(true);let mut graphs=ordinary.workspace.project.graphs().to_vec();graphs[0].nodes.values_mut().next().unwrap().position=[81.,92.];ordinary.workspace.write_recovery(&graphs).unwrap();let cohort=ordinary.cohort();let images=ordinary.images();assert!(ordinary.workspace.discard_library_recovery_verified1031(&cohort).unwrap_err().to_string().contains("actual scoped Saved or Current Library"));assert_eq!(ordinary.images(),images);assert!(!ordinary.root.join(REFACTOR_JOURNAL).exists());
}
