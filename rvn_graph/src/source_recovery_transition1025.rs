//! Typed recovery transitions inside the EXISTING refactor journal. Legacy
//! readers see an object where they require String and refuse BEFORE IO.
use super::*;
#[path="source_publication_outcome1031.rs"]
mod publication_outcome1031;
pub use publication_outcome1031::{CauseBeforeCommit1031,CleanupWarning1031,CleanupStatus1031,VerifiedCommitted1031};
pub(super) use publication_outcome1031::{PreparedCommit1031,Execution1031};
use crate::source_workspace::enum_recovery1025::{self,EnumRecoveryChange1025};
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(untagged)]
pub(super) enum JournalRecovery1025 {
    LegacyArchive(String),
    Transition(RecoveryTransition1025),
}
impl JournalRecovery1025 {
    pub(super) fn legacy(&self)->Option<&String>{match self{Self::LegacyArchive(bytes)=>Some(bytes),Self::Transition(_)=>None}}
}
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub(super) enum TransitionMode1025 { RecoveryOnly,Metadata,CompleteSource,DiscardAll,CohortDiscard,PartialLibraries }
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LibraryTransition1027 {
    changes:Vec<LibraryRecoveryChange1027>,
    saved:Vec<rvn_parser::function_library_asset1023::FunctionLibraryId>,
}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RecoveryTransition1025 {
    kind:String,
    version:u32,
    mode:TransitionMode1025,
    #[serde(deserialize_with="required_bytes1025")]
    before_recovery:Option<String>,
    #[serde(deserialize_with="required_bytes1025")]
    after_recovery:Option<String>,
    changes:Vec<EnumRecoveryChange1025>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    library1027:Option<LibraryTransition1027>,
}
#[path="source_recovery_cohort_discard1025.rs"]
mod cohort_discard1025;
#[path="source_library_partial_save1027.rs"]
mod library_partial_save1027;
#[path="source_library_complete_save1031.rs"]
mod library_complete_save1031;
pub use library_complete_save1031::{SourceLibraryCompletePublication1031,SourceLibraryCompleteCommitted1031};
fn required_bytes1025<'de,D:serde::Deserializer<'de>>(d:D)->Result<Option<String>,D::Error>{Option::<String>::deserialize(d)}
fn recovery_value1025(bytes:&Option<String>,source:&str,sidecar:&Option<String>,snapshot:&Snapshot)->Result<Recovery,String>{
    let value=match bytes{
        Some(bytes)=>serde_json::from_str::<Recovery>(bytes).map_err(|error|format!("Invalid typed Enum recovery in journal: {error}"))?,
        None=>Recovery{libraries:Vec::new(),version:if snapshot.asset_placements.is_some(){3}else{1},source:source.into(),baseline_source:snapshot.source_snapshot.clone(),baseline_sidecar:sidecar.clone(),graphs:snapshot.graphs.clone(),widgets:Vec::new(),widget_memberships:Vec::new(),asset_placements_after:snapshot.asset_placements.clone(),graph_id_high_water:Some(validated_snapshot_high_water(snapshot)?),enums:Vec::new()},
    };
    if bytes.is_some(){
        validate_recovery_editorial(&value)?;
        if value.source!=source||value.baseline_sidecar!=*sidecar{return Err("Typed recovery journal lost its exact presentation baseline".into());}
        validate_recovery_graph_shapes(&value.graphs)?;
        enum_recovery1025::validate_records1025(&value.enums,&value.widgets)?;
    }
    Ok(value)
}
fn clean1025(value:&Recovery,published:&Snapshot)->bool{
    value.libraries.is_empty()&&value.enums.is_empty()&&value.widgets.is_empty()&&value.widget_memberships.is_empty()
        &&value.graphs==published.graphs&&value.asset_placements_after==published.asset_placements
        &&validated_snapshot_high_water(published).is_ok_and(|high|value.graph_id_high_water.unwrap_or(0)<=high)
}
fn serialize_after1025(mut value:Recovery,source_text:&str,sidecar:&str,snapshot:&Snapshot)->Result<Option<String>,String>{
    value.source=snapshot.source.clone();value.baseline_source=source_text.into();value.baseline_sidecar=Some(sidecar.into());
    value.version=if !value.libraries.is_empty(){5}else if !value.enums.is_empty(){4}else if value.asset_placements_after.is_some(){3}else if value.widget_memberships.is_empty(){1}else{2};
    if clean1025(&value,snapshot){return Ok(None);}
    validate_recovery_editorial(&value)?;
    serde_json::to_string(&value).map(Some).map_err(|error|error.to_string())
}
fn journal_snapshot1025(journal:&RefactorJournal,after:bool)->Result<(String,String,Snapshot),String>{
    let source=journal.files.iter().find(|file|file.path==journal.source).and_then(|file|if after{file.after.clone()}else{file.before.clone()}).ok_or("Typed recovery journal requires its complete Source file")?;
    let sidecar=journal.files.iter().find(|file|file.path==SIDECAR).and_then(|file|if after{file.after.clone()}else{file.before.clone()}).ok_or("Typed recovery journal requires its complete sidecar")?;
    let snapshot:Snapshot=serde_json::from_str(&sidecar).map_err(|error|error.to_string())?;validate_snapshot_editorial(&snapshot)?;
    if snapshot.source!=journal.source||snapshot.source_snapshot!=source{return Err("Typed recovery journal source/sidecar mismatch".into());}
    Ok((source,sidecar,snapshot))
}
fn validate_transition1025(root:&Path,journal:&RefactorJournal,transition:&RecoveryTransition1025)->Result<(),String>{
    let library_version=transition.kind=="source_library_recovery_transition_v5"&&transition.version==5&&transition.library1027.is_some();
    if !library_version&&(transition.kind!="enum_recovery_transition_v4"||transition.version!=4||transition.library1027.is_some()||transition.mode==TransitionMode1025::PartialLibraries){return Err("Unsupported typed recovery journal".into());}
    if transition.mode==TransitionMode1025::RecoveryOnly&&transition.changes.is_empty(){return Err("Recovery-only transition has no exact mutation".into());}
    let (before_source,before_sidecar,before_snapshot)=journal_snapshot1025(journal,false)?;
    let (after_source,after_sidecar,after_snapshot)=journal_snapshot1025(journal,true)?;
    if after_snapshot.graphs!=journal.graphs{return Err("Typed journal graph list differs from its exact sidecar".into());}
    let before_project=library_inputs1027::validate_refactor_snapshot1027(root,&before_snapshot,&journal.files,journal.library_before1027.as_ref(),journal.library_after1027.as_ref(),false)?;before_project.check_imports()?;
    let after_project=library_inputs1027::validate_refactor_snapshot1027(root,&after_snapshot,&journal.files,journal.library_before1027.as_ref(),journal.library_after1027.as_ref(),true)?;after_project.check_imports()?;
    let before=recovery_value1025(&transition.before_recovery,&journal.source,&Some(before_sidecar.clone()),&before_snapshot)?;
    let after=recovery_value1025(&transition.after_recovery,&journal.source,&Some(after_sidecar.clone()),&after_snapshot)?;
    if !library_version&&(!before.libraries.is_empty()||!after.libraries.is_empty()){return Err("Library participants require their exact version5 typed transition; legacy mutation refused".into())}
    if let Some(library)=&transition.library1027{let expected=library_recovery1027::apply_changes1027(&before.libraries,&library.changes)?;if expected!=after.libraries{return Err("Library transition changed an unmentioned recovery record".into())}for(project,value)in[(&before_project,&before),(&after_project,&after)]{library_recovery1027::validate_snapshot_records1027(project,&value.libraries,&value.graphs,&value.widgets,&value.widget_memberships,&value.enums)?;}if transition.mode==TransitionMode1025::DiscardAll{
        let removed=before.libraries.iter().cloned().map(|before|LibraryRecoveryChange1027::Remove{before}).collect::<Vec<_>>();
        if library.changes!=removed||!library.saved.is_empty()||!after.libraries.is_empty(){return Err("Library global discard requires exactly all original recovery records, without Saved acknowledgement".into())}
    }else if !matches!(transition.mode,TransitionMode1025::PartialLibraries|TransitionMode1025::CompleteSource)&&(!library.changes.is_empty()||!library.saved.is_empty()){return Err("Non-Library transition cannot acknowledge Library records".into())}}
    if transition.before_recovery.is_some()&&before.baseline_source!=before_source
        ||transition.after_recovery.is_some()&&after.baseline_source!=after_source{return Err("Enum recovery journal lost exact Source bytes".into());}
    for (value,snapshot) in [(&before,&before_snapshot),(&after,&after_snapshot)]{
        let high=validated_recovery_high_water(value,validated_snapshot_high_water(snapshot)?)?;
        if high<validated_snapshot_high_water(snapshot)?{return Err("Typed recovery dropped a reserved graph identity".into());}
        widget_recovery::validate(&value.widgets,&value.graphs,&snapshot.graphs,&journal.source)?;
        widget_recovery::validate_memberships(&value.widget_memberships,&value.widgets,&value.graphs,&snapshot.graphs,&journal.source)?;
        crate::text_owner::validate_text_owner_edit(&snapshot.graphs,&value.graphs)?;
    }
    let expected=enum_recovery1025::apply_changes1025(&before.enums,&transition.changes,transition.mode==TransitionMode1025::RecoveryOnly)?;
    if expected!=after.enums{return Err("Typed recovery journal changed an unmentioned Enum participant".into());}
    if library_version&&transition.mode==TransitionMode1025::CompleteSource{
        library_complete_save1031::validate_complete1031(journal,transition,&before,&after,&before_snapshot,&after_snapshot,&before_project,&after_project)?;
    }
    if transition.mode==TransitionMode1025::PartialLibraries{
        library_partial_save1027::validate_partial1027(journal,transition,&before,&after,&before_snapshot,&after_snapshot,&before_project,&after_project)?;
    }else if !matches!(transition.mode,TransitionMode1025::CompleteSource|TransitionMode1025::DiscardAll|TransitionMode1025::CohortDiscard){
        if before_source!=after_source||before_snapshot.graphs!=after_snapshot.graphs||before_snapshot.asset_placements!=after_snapshot.asset_placements{return Err("Metadata/recovery-only transition changed authored graphs".into());}
        if transition.after_recovery.is_some(){
            if before.graphs!=after.graphs||before.widgets!=after.widgets||before.widget_memberships!=after.widget_memberships
                ||before.asset_placements_after!=after.asset_placements_after||after.graph_id_high_water.unwrap_or(0)<before.graph_id_high_water.unwrap_or(0){return Err("Enum-only transition changed an unmentioned recovery participant".into());}
        }else if !clean1025(&before_without_enums1025(&before),&before_snapshot){return Err("Enum-only transition erased non-Enum recovery".into());}
    }else if transition.mode==TransitionMode1025::CohortDiscard{
        cohort_discard1025::validate_cohort_discard1025(root,journal,transition,&before,&after,&before_snapshot,&after_snapshot,&before_source,&after_source,&before_sidecar,&after_sidecar)?;
    }else if transition.mode==TransitionMode1025::DiscardAll{
        if transition.after_recovery.is_some()||!expected.is_empty(){return Err("Global discard must remove its complete exact recovery cohort".into());}
        if before_source!=after_source||before_snapshot.graphs!=after_snapshot.graphs||before_snapshot.asset_placements!=after_snapshot.asset_placements
            ||validated_snapshot_high_water(&after_snapshot)?<validated_recovery_high_water(&before,validated_snapshot_high_water(&before_snapshot)?)?{
            return Err("Global discard changed authored data or lost reserved identities".into());
        }
        let mut old:serde_json::Value=serde_json::from_str(&before_sidecar).map_err(|error|error.to_string())?;
        let mut next:serde_json::Value=serde_json::from_str(&after_sidecar).map_err(|error|error.to_string())?;
        old.as_object_mut().ok_or("Invalid prior sidecar")?.remove("graph_id_high_water");
        next.as_object_mut().ok_or("Invalid next sidecar")?.remove("graph_id_high_water");
        if old!=next||journal.files.iter().any(|file|file.path!=SIDECAR&&file.before!=file.after){return Err("Global discard may only reserve identities and remove exact recovery".into());}
    }else{
        widget_recovery::publication_participants(&SourceRecoveryBatch{graphs:before.graphs.clone(),widgets:before.widgets.clone(),widget_memberships:before.widget_memberships.clone(),asset_placements:before.asset_placements_after.clone(),enums:Vec::new()},&journal.files)?;
        if transition.after_recovery.is_some()&&(after.graphs!=after_snapshot.graphs||!after.widgets.is_empty()||!after.widget_memberships.is_empty()||after.asset_placements_after!=after_snapshot.asset_placements){return Err("Complete Source save has inconsistent residual recovery".into());}
    }
    if matches!(transition.mode,TransitionMode1025::RecoveryOnly|TransitionMode1025::CohortDiscard){
        if journal.files.iter().any(|file|file.before!=file.after){return Err("Recovery-only mutation cannot publish authored files".into());}
    }else if transition.mode!=TransitionMode1025::DiscardAll{enum_recovery1025::validate_publication1025(root,&transition.changes,&journal.files)?;}
    enum_recovery1025::check_baselines1025(root,&before.enums,&journal.files,false)?;
    enum_recovery1025::check_baselines1025(root,&after.enums,&journal.files,true)?;
    let active=read_optional(&root.join(RECOVERY))?;
    if active!=transition.before_recovery&&active!=transition.after_recovery{return Err("External recovery changed; typed journal and all files retained".into());}
    // Recovery is the LAST published participant. An after-cache cannot
    // precede unfinished files in a legal forward interruption. If the cache
    // is unchanged (before==after), it conveys no such ordering information.
    if active==transition.after_recovery&&active!=transition.before_recovery{
        for file in &journal.files{
            if read_optional(&target(root,&file.path)?)?!=file.after{
                return Err("Recovery reached after before every file; journal, files and cache retained".into());
            }
        }
    }
    // Absence/after-cache before complete file publication cannot be mistaken
    // for a successful previous cleanup. Replaying still owns every file CAS.
    Ok(())
}
fn before_without_enums1025(value:&Recovery)->Recovery{let mut next=value.clone();next.enums.clear();next}
fn replace_recovery1025(root:&Path,bytes:&Option<String>)->Result<(),String>{replace(&root.join(RECOVERY),bytes)}
fn compensate1025(root:&Path,journal:&RefactorJournal,transition:&RecoveryTransition1025)->Vec<String>{
    let mut errors=Vec::new();
    match read_optional(&root.join(RECOVERY)){
        Ok(current)if current==transition.after_recovery=>if let Err(error)=replace_recovery1025(root,&transition.before_recovery){errors.push(error)},
        Ok(current)if current==transition.before_recovery=>{},
        Ok(_)=>errors.push("External recovery changed during compensation".into()),
        Err(error)=>errors.push(error),
    }
    for file in journal.files.iter().rev(){
        match target(root,&file.path).and_then(|path|Ok((path.clone(),read_optional(&path)?))){
            Ok((path,current))if current==file.after=>if let Err(error)=replace(&path,&file.before){errors.push(error)},
            Ok((_,current))if current==file.before=>{},
            Ok((_,_))=>errors.push("External file changed during typed compensation".into()),
            Err(error)=>errors.push(error),
        }
    }
    errors
}
// One actual target/recovery operation shared by strict legacy and verified
// callers. Only the latter binds an exact original raw journal. Neither path
// changes the precommit compensation/error policy.
fn publish_operation1031(root:&Path,journal:&RefactorJournal,transition:&RecoveryTransition1025,recovering:bool,original:Option<&PreparedCommit1031>,execution:&Execution1031)->Result<(),String>{
    check_files(root,&journal.files,recovering)?;validate_transition1025(root,journal,transition)?;
    execution.backup();
    let backup=root.join(".rvn-backups").join(unique_name());fs::create_dir_all(&backup).map_err(|error|error.to_string())?;
    // Keep exact prior recovery bytes before the first durable target changes.
    if let Some(bytes)=&transition.before_recovery{atomic_write(&backup.join("recovery.json"),bytes)?;}
    for (index,file) in journal.files.iter().enumerate(){if let Some(before)=&file.before{atomic_write(&backup.join(format!("participant-{index}.txt")),before)?;}}
    let operation=(||->Result<(),String>{
        check_files(root,&journal.files,recovering)?;validate_transition1025(root,journal,transition)?;
        for file in &journal.files{
            let path=target(root,&file.path)?;let current=read_optional(&path)?;
            if current==file.after{continue;}if current!=file.before{return Err("Typed file publication changed externally".into());}
            execution.target()?;replace(&path,&file.after)?;
        }
        let current=read_optional(&root.join(RECOVERY))?;
        if current!=transition.before_recovery&&current!=transition.after_recovery{return Err("Typed recovery changed before final publication".into());}
        if current!=transition.after_recovery{execution.recovery()?;replace_recovery1025(root,&transition.after_recovery)?;}
        execution.before_verify()?;
        for file in &journal.files{if read_optional(&target(root,&file.path)?)?!=file.after{return Err("Typed publication lost an exact after-file before completion".into());}}
        if read_optional(&root.join(RECOVERY))?!=transition.after_recovery{return Err("Typed recovery publication did not reach its exact after bytes".into());}
        if let Some(original)=original{original.check_original(root)?;}
        Ok(())
    })();
    if let Err(error)=operation{
        let failures=compensate1025(root,journal,transition);
        if failures.is_empty()&&!recovering{if let Some(original)=original{original.check_original(root)?;}fs::remove_file(root.join(REFACTOR_JOURNAL)).map_err(|reason|reason.to_string())?;}
        return Err(format!("{error}; typed recovery compensation: {}",if failures.is_empty(){"restored original participants".into()}else{failures.join("; ")}));
    }
    Ok(())
}
pub(super) fn execute1025(root:&Path,journal:&RefactorJournal,transition:&RecoveryTransition1025,recovering:bool)->Result<(),String>{
    execute_legacy_inner1031(root,journal,transition,recovering,&Execution1031::default())
}
fn execute_legacy_inner1031(root:&Path,journal:&RefactorJournal,transition:&RecoveryTransition1025,recovering:bool,execution:&Execution1031)->Result<(),String>{
    publish_operation1031(root,journal,transition,recovering,None,execution)?;
    execution.remove_journal(root).map_err(|error|error.to_string())?;Ok(())
}
pub(super) fn execute_verified1031(root:&Path,journal:&RefactorJournal,transition:&RecoveryTransition1025,recovering:bool,original:Box<PreparedCommit1031>,execution:&Execution1031,cleanup_only:bool)->Result<VerifiedCommitted1031,String>{
    original.check_original(root)?;
    if cleanup_only {
        check_files(root,&journal.files,true)?;validate_transition1025(root,journal,transition)?;
        execution.before_verify()?;
        for file in &journal.files{if read_optional(&target(root,&file.path)?)?!=file.after{return Err("Cleanup-only requires every exact original after-file".into());}}
        if read_optional(&root.join(RECOVERY))?!=transition.after_recovery{return Err("Cleanup-only requires the exact original after-recovery".into());}
        original.check_original(root)?;
    }else{
        publish_operation1031(root,journal,transition,recovering,Some(&original),execution)?;
    }
    Ok(publication_outcome1031::finish_verified1031(root,original,execution))
}

struct PreparedTypedPublication1031 {
    next:SourceProject,journal:RefactorJournal,after_sidecar:String,after_recovery:Option<String>,
    after_snapshot:Snapshot,committed_high:u64,published_placements:Option<SourceAssetPlacements>,mode:TransitionMode1025,
}
impl SourceWorkspace{
    fn raw_enum_recovery1025(&self)->Result<Recovery,String>{
        self.recovery_cohort_libraries1027()?;
        if let Some(bytes)=&self.expected_recovery{return serde_json::from_str(bytes).map_err(|error|error.to_string());}
        let sidecar=self.expected_sidecar.clone().ok_or("Typed recovery requires an actual Source sidecar")?;
        let snapshot:Snapshot=serde_json::from_str(&sidecar).map_err(|error|error.to_string())?;
        let mut value=recovery_value1025(&None,&self.relative_source,&Some(sidecar),&snapshot)?;
        // Fresh member allocation may reserve IDs before the periodic graph
        // autosave. Adding an Enum recovery must persist, never rewind, that
        // existing in-memory high-water even when no prior cache exists.
        value.graph_id_high_water=Some(self.graph_id_high_water());
        Ok(value)
    }
    fn publish_enum_transition1025(&mut self,next:SourceProject,files:&[SourceRefactorFile],changes:&[EnumRecoveryChange1025],mode:TransitionMode1025)->Result<(),String>{
        self.publish_typed_transition1030(next,files,changes,mode,None)
    }
    fn prepare_typed_publication1031(&mut self,mut next:SourceProject,files:&[SourceRefactorFile],changes:&[EnumRecoveryChange1025],mode:TransitionMode1025,
        library_transition:Option<&LibraryTransition1027>)->Result<PreparedTypedPublication1031,String>{
        if next.observes_library_inputs1027(){let staged=next.stage_library_files1027(&self.root,&self.source_path(),files)?;if staged.source()!=next.source()||staged.graphs()!=next.graphs(){return Err("Enum publication changed the original scoped Library graph/source projection".into())}next=staged;}
        self.check_recovery_baseline()?;self.check_recovery_owner()?;check_files(&self.root,files,false)?;
        self.project.check_imports()?;next.check_imports()?;
        if mode!=TransitionMode1025::DiscardAll{if let Some(error)=&self.source_error{return Err(error.clone());}}
        let mut value=self.raw_enum_recovery1025()?;
        let after_enums=enum_recovery1025::apply_changes1025(&value.enums,changes,mode==TransitionMode1025::RecoveryOnly)?;
        value.enums=after_enums;
        let after_sidecar=files.iter().find(|file|file.path==SIDECAR).and_then(|file|file.after.clone()).ok_or("Enum transition lost its published sidecar")?;
        let after_snapshot:Snapshot=serde_json::from_str(&after_sidecar).map_err(|error|error.to_string())?;
        if matches!(mode,TransitionMode1025::CompleteSource|TransitionMode1025::DiscardAll){
            value.graphs=next.graphs().to_vec();value.widgets.clear();value.widget_memberships.clear();value.asset_placements_after=after_snapshot.asset_placements.clone();value.graph_id_high_water=Some(validated_snapshot_high_water(&after_snapshot)?);
        }
        let library_version=!value.libraries.is_empty()||library_transition.is_some();
        if let Some(library)=library_transition{
            if !matches!(mode,TransitionMode1025::DiscardAll|TransitionMode1025::CompleteSource){return Err("Library complete transition cannot masquerade as another mode".into())}
            let expected=value.libraries.iter().cloned().map(|before|LibraryRecoveryChange1027::Remove{before}).collect::<Vec<_>>();
            if library.changes!=expected||mode==TransitionMode1025::DiscardAll&&!library.saved.is_empty(){return Err("Library complete transition lost its exact original record list".into())}
            value.libraries=library_recovery1027::apply_changes1027(&value.libraries,&library.changes)?;
        }
        let after=serialize_after1025(value,next.source(),&after_sidecar,&after_snapshot)?;
        let journal=RefactorJournal{library_before1027:self.library_journal_readset1027(),library_after1027:library_inputs1027::project_readset1027(&next),source:self.relative_source.clone(),graphs:next.graphs().to_vec(),files:files.to_vec(),recovery:Some(JournalRecovery1025::Transition(RecoveryTransition1025{kind:if library_version{"source_library_recovery_transition_v5"}else{"enum_recovery_transition_v4"}.into(),version:if library_version{5}else{4},mode,before_recovery:self.expected_recovery.clone(),after_recovery:after.clone(),changes:changes.to_vec(),library1027:library_version.then(||library_transition.cloned().unwrap_or(LibraryTransition1027{changes:Vec::new(),saved:Vec::new()}))}))};
        validate_transition1025(&self.root,&journal,match journal.recovery.as_ref().unwrap(){JournalRecovery1025::Transition(value)=>value,_=>unreachable!()})?;
        let committed_high=validated_snapshot_high_water(&after_snapshot)?;
        // All adoption allocations and the local scoped fence precede IO.
        let published_placements=after_snapshot.asset_placements.clone();
        next.commit_library_inputs1027();
        Ok(PreparedTypedPublication1031{next,journal,after_sidecar,after_recovery:after,after_snapshot,committed_high,published_placements,mode})
    }
    fn publish_typed_transition1030(&mut self,next:SourceProject,files:&[SourceRefactorFile],changes:&[EnumRecoveryChange1025],mode:TransitionMode1025,
        library_transition:Option<&LibraryTransition1027>)->Result<(),String>{
        let prepared=self.prepare_typed_publication1031(next,files,changes,mode,library_transition)?;
        atomic_write(&self.root.join(REFACTOR_JOURNAL),&serde_json::to_string(&prepared.journal).map_err(|error|error.to_string())?)?;
        // Legacy callers deliberately retain their original cleanup-error policy.
        apply(&self.root,&prepared.journal,false)?;
        self.adopt_typed_publication1031(prepared);Ok(())
    }
    fn adopt_typed_publication1031(&mut self,prepared:PreparedTypedPublication1031){
        let PreparedTypedPublication1031{next,after_sidecar,after_recovery,after_snapshot,committed_high,published_placements,mode,..}=prepared;
        self.project=next;self.expected_sidecar=Some(after_sidecar);self.expected_recovery=after_recovery;
        self.published_asset_placements=published_placements;
        if matches!(mode,TransitionMode1025::CompleteSource|TransitionMode1025::DiscardAll){self.asset_placements=after_snapshot.asset_placements;self.snapshot_matches_project=true;}
        self.graph_id_high_water=self.graph_id_high_water.max(committed_high);
    }
    fn publish_complete_verified1031(&mut self,next:SourceProject,files:&[SourceRefactorFile],changes:&[EnumRecoveryChange1025],library:&LibraryTransition1027,execution:&Execution1031)->Result<VerifiedCommitted1031,String>{
        let prepared=self.prepare_typed_publication1031(next,files,changes,TransitionMode1025::CompleteSource,Some(library))?;
        let wire=serde_json::to_string(&prepared.journal).map_err(|error|error.to_string())?;
        let original=Box::new(PreparedCommit1031::prepare(&self.root,&self.relative_source,wire));
        execution.before_journal()?;
        atomic_write(&self.root.join(REFACTOR_JOURNAL),original.wire())?;
        let committed=super::apply_typed_verified1031(&self.root,&prepared.journal,false,original,execution,false)?;
        // Retain the real proof before any adoption; only prepared moves remain.
        self.adopt_typed_publication1031(prepared);
        Ok(committed)
    }
    /// Complete v5 discard under the caller's ORIGINAL cooperative lock.
    /// Only the Source wrapper below can enter this method. No authored file
    /// or Saved descriptor/index changes; existing typed journal owns backup,
    /// high-water publication, compensation and exact crash recovery.
    pub(in crate::source_workspace) fn discard_library_cohort_locked1030(&mut self,expected:&SourceRecoveryCohortLibrary1027,
        preflight:impl FnOnce()->Result<(),String>)->Result<(),String>{
        let actual=self.recovery_cohort_libraries1027()?.ok_or("Original Library cohort disappeared before discard")?;
        if actual!=*expected{return Err("Library discard lost its complete original cohort".into())}
        let library_changes=actual.libraries.iter().cloned().map(|before|LibraryRecoveryChange1027::Remove{before}).collect::<Vec<_>>();
        let enum_changes=actual.batch.enums.iter().cloned().map(|before|EnumRecoveryChange1025::Remove{before}).collect::<Vec<_>>();
        let before=self.expected_sidecar.clone().ok_or("Library discard requires its real published sidecar")?;
        let mut fields:serde_json::Value=serde_json::from_str(&before).map_err(|e|e.to_string())?;
        fields.as_object_mut().ok_or("Invalid Source sidecar")?.insert("graph_id_high_water".into(),serde_json::json!(self.graph_id_high_water().max(actual.high_water1027())));
        let after=serde_json::to_string(&fields).map_err(|e|e.to_string())?;
        let text=self.project.source().to_owned();
        let mut files=vec![SourceRefactorFile{path:self.relative_source.clone(),before:Some(text.clone()),after:Some(text)},
            SourceRefactorFile{path:SIDECAR.into(),before:Some(before),after:Some(after)}];
        // Discard still stages the same scoped Source. Its original manifest
        // and descriptor bytes are readonly participants of this SAME journal,
        // not files recaptured after the user decision or another writer.
        self.append_library_readsets1027(&mut files)?;
        // All models/metadata/Registry resources must already be held. A
        // refusal here writes no journal, files, backup or recovery.
        preflight()?;
        self.publish_typed_transition1030(self.project.clone(),&files,&enum_changes,TransitionMode1025::DiscardAll,Some(&LibraryTransition1027{changes:library_changes,saved:Vec::new()}))
    }
    fn discard_library_verified_inner1031(&mut self,expected:&SourceRecoveryCohortLibrary1027,execution:&Execution1031)->Result<VerifiedCommitted1031,String>{
        let _lock=lock(&self.root)?;
        if self.root.join(JOURNAL).try_exists().map_err(|e|e.to_string())?||self.root.join(REFACTOR_JOURNAL).try_exists().map_err(|e|e.to_string())?{return Err("Finish the existing Source transaction before a new verified publication".into());}
        self.check_recovery_baseline()?;self.check_recovery_owner()?;
        let actual=self.recovery_cohort_libraries1027()?.ok_or("Original Library cohort disappeared before verified discard")?;
        if actual!=*expected{return Err("Verified discard lost its exact original full Library cohort".into());}
        if !self.project.observes_library_inputs1027()||actual.libraries.is_empty()&&library_recovery1027::saved_baselines_for_project1027(&self.project)?.is_empty(){return Err("Verified Library discard requires an actual scoped Saved or Current Library owner".into());}
        let library_changes=actual.libraries.iter().cloned().map(|before|LibraryRecoveryChange1027::Remove{before}).collect::<Vec<_>>();
        let enum_changes=actual.batch.enums.iter().cloned().map(|before|EnumRecoveryChange1025::Remove{before}).collect::<Vec<_>>();
        let before=self.expected_sidecar.clone().ok_or("Verified Library discard requires its actual sidecar")?;
        let mut fields:serde_json::Value=serde_json::from_str(&before).map_err(|e|e.to_string())?;
        fields.as_object_mut().ok_or("Invalid Source sidecar")?.insert("graph_id_high_water".into(),serde_json::json!(self.graph_id_high_water().max(actual.high_water1027())));
        let after_sidecar=serde_json::to_string(&fields).map_err(|e|e.to_string())?;
        let after_snapshot:Snapshot=serde_json::from_str(&after_sidecar).map_err(|e|e.to_string())?;
        let committed_high=validated_snapshot_high_water(&after_snapshot)?;
        let text=self.project.source().to_owned();
        let mut files=vec![SourceRefactorFile{path:self.relative_source.clone(),before:Some(text.clone()),after:Some(text)},SourceRefactorFile{path:SIDECAR.into(),before:Some(before),after:Some(after_sidecar.clone())}];
        self.append_library_readsets1027(&mut files)?;
        let mut next=self.project.clone();
        if next.observes_library_inputs1027(){let staged=next.stage_library_files1027(&self.root,&self.source_path(),&files)?;if staged.source()!=next.source()||staged.graphs()!=next.graphs(){return Err("Verified discard changed its original scoped Source projection".into());}next=staged;}
        self.project.check_imports()?;next.check_imports()?;check_files(&self.root,&files,false)?;
        let mut value=self.raw_enum_recovery1025()?;
        value.enums=enum_recovery1025::apply_changes1025(&value.enums,&enum_changes,false)?;
        value.libraries=library_recovery1027::apply_changes1027(&value.libraries,&library_changes)?;
        value.graphs=next.graphs().to_vec();value.widgets.clear();value.widget_memberships.clear();value.asset_placements_after=after_snapshot.asset_placements.clone();value.graph_id_high_water=Some(committed_high);
        let after_recovery=serialize_after1025(value,next.source(),&after_sidecar,&after_snapshot)?;
        let transition=RecoveryTransition1025{kind:"source_library_recovery_transition_v5".into(),version:5,mode:TransitionMode1025::DiscardAll,before_recovery:self.expected_recovery.clone(),after_recovery:after_recovery.clone(),changes:enum_changes,library1027:Some(LibraryTransition1027{changes:library_changes,saved:Vec::new()})};
        let journal=RefactorJournal{library_before1027:self.library_journal_readset1027(),library_after1027:library_inputs1027::project_readset1027(&next),source:self.relative_source.clone(),graphs:next.graphs().to_vec(),files,recovery:Some(JournalRecovery1025::Transition(transition))};
        validate_transition1025(&self.root,&journal,match journal.recovery.as_ref().unwrap(){JournalRecovery1025::Transition(t)=>t,_=>unreachable!()})?;
        // Every owned image and allocation needed by adoption precedes IO.
        let published_placements=after_snapshot.asset_placements.clone();
        next.commit_library_inputs1027();
        let wire=serde_json::to_string(&journal).map_err(|e|e.to_string())?;
        let original=Box::new(PreparedCommit1031::prepare(&self.root,&self.relative_source,wire));
        execution.before_journal()?;
        atomic_write(&self.root.join(REFACTOR_JOURNAL),original.wire())?;
        let committed=super::apply_typed_verified1031(&self.root,&journal,false,original,execution,false)?;
        // Only moves/scalars remain, and the genuine proof is already retained.
        self.project=next;self.expected_sidecar=Some(after_sidecar);self.expected_recovery=after_recovery;
        self.published_asset_placements=published_placements;self.asset_placements=after_snapshot.asset_placements;
        self.graph_id_high_water=self.graph_id_high_water.max(committed_high);self.snapshot_matches_project=true;
        Ok(committed)
    }
    pub fn transition_enum_recovery_with1025<T>(&mut self,changes:&[EnumRecoveryChange1025],references:&[SourceRefactorFile],install:impl FnOnce()->T)->Result<T,String>{
        let _lock=lock(&self.root)?;recover(&self.root)?;
        if references.iter().any(|file|file.before!=file.after){return Err("Recovery transition references must be readonly".into());}
        let sidecar=self.expected_sidecar.clone().ok_or("Enum recovery requires its actual published sidecar")?;
        let mut files=vec![SourceRefactorFile{path:self.relative_source.clone(),before:Some(self.project.source().into()),after:Some(self.project.source().into())},SourceRefactorFile{path:SIDECAR.into(),before:Some(sidecar.clone()),after:Some(sidecar)}];
        self.append_publication_files(&mut files,references)?;
        self.publish_enum_transition1025(self.project.clone(),&files,changes,TransitionMode1025::RecoveryOnly)?;Ok(install())
    }
    /// Global Source recovery decision only. Removes the complete current
    /// cohort after exact admission; backup+journal remain the existing ones.
    pub fn discard_enum_recovery_with1025<T>(&mut self,expected:&SourceRecoveryBatch,
        install:impl FnOnce()->T,
    )->Result<T,String>{
        let _lock=lock(&self.root)?;recover(&self.root)?;
        let batch=self.recovery_batch()?.ok_or("Global typed recovery disappeared")?;
        if batch!=*expected||batch.enums.is_empty(){return Err("Global typed recovery changed before discard".into());}
        let changes=batch.enums.iter().cloned().map(|before|EnumRecoveryChange1025::Remove{before}).collect::<Vec<_>>();
        let old=self.expected_sidecar.clone().ok_or("Global recovery discard requires its sidecar")?;
        let mut value:serde_json::Value=serde_json::from_str(&old).map_err(|error|error.to_string())?;
        value.as_object_mut().ok_or("Invalid Source sidecar")?.insert("graph_id_high_water".into(),serde_json::json!(self.graph_id_high_water()));
        let after=serde_json::to_string(&value).map_err(|error|error.to_string())?;
        let source=self.project.source().to_owned();
        let files=vec![SourceRefactorFile{path:self.relative_source.clone(),before:Some(source.clone()),after:Some(source)},SourceRefactorFile{path:SIDECAR.into(),before:Some(old),after:Some(after)}];
        self.publish_enum_transition1025(self.project.clone(),&files,&changes,TransitionMode1025::DiscardAll)?;
        Ok(install())
    }
    pub fn save_metadata_files_with_enum_recovery1025(&mut self,graphs:&[GraphDocument],additional:&[SourceRefactorFile],changes:&[EnumRecoveryChange1025])->Result<(),String>{
        let _lock=lock(&self.root)?;recover(&self.root)?;
        if graphs!=self.project.graphs()||additional.is_empty(){return Err("Enum metadata save requires exact published graph baseline and actual files".into());}
        if let Some(error)=&self.source_error{return Err(error.clone());}
        let sidecar=self.expected_sidecar.clone().ok_or("Enum save requires its actual sidecar")?;
        let mut files=vec![SourceRefactorFile{path:self.relative_source.clone(),before:Some(self.project.source().into()),after:Some(self.project.source().into())},SourceRefactorFile{path:SIDECAR.into(),before:Some(sidecar.clone()),after:Some(sidecar)}];self.append_publication_files(&mut files,additional)?;
        self.publish_enum_transition1025(self.project.clone(),&files,changes,TransitionMode1025::Metadata)
    }
    /// Complete Source/Widget save: callers may not use a sparse graph scope.
    /// Enum changes remain targeted and all remaining Enum records survive.
    pub fn save_with_files_and_enum_recovery1025(&mut self,graphs:&[GraphDocument],additional:&[SourceRefactorFile],changes:&[EnumRecoveryChange1025])->Result<(),String>{
        let _lock=lock(&self.root)?;recover(&self.root)?;
        if let Some(error)=&self.source_error{return Err(error.clone());}
        self.check_recovery_baseline()?;self.check_recovery_owner()?;
        let current=fs::read_to_string(self.source_path()).map_err(|error|error.to_string())?;
        if current!=self.project.source(){return Err("Source changed before complete Enum publication".into());}
        let mut next=self.project.clone();next.apply_visual(&current,graphs)?;next.check_imports()?;
        let snapshot=Snapshot{asset_placements:self.placements_for_graphs(next.graphs())?,graph_id_high_water:Some(self.graph_id_high_water().max(next.graphs().iter().map(|graph|graph.graph_id.get()).max().unwrap_or(0))),version:if self.asset_placements.is_some(){2}else{1},source:self.relative_source.clone(),source_snapshot:next.source().into(),graphs:next.graphs().to_vec(),imports:next.imports_snapshot()};
        let sidecar=serde_json::to_string_pretty(&snapshot).map_err(|error|error.to_string())?;
        let mut files=vec![SourceRefactorFile{path:self.relative_source.clone(),before:Some(current),after:Some(next.source().into())},SourceRefactorFile{path:SIDECAR.into(),before:self.expected_sidecar.clone(),after:Some(sidecar)}];self.append_publication_files(&mut files,additional)?;
        self.publish_enum_transition1025(next,&files,changes,TransitionMode1025::CompleteSource)
    }
}
#[cfg(test)]
#[path="source_recovery_transition_tests1025.rs"]
mod tests;
#[cfg(test)]
#[path="source_library_complete_save_tests1031.rs"]
mod complete_library_tests1031;
