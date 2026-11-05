//! Complete preflighted Graph/Widget cohort plus targeted Enum removals in
//! the SAME typed Source journal, writer lock and infallible install frontier.
use super::*;
fn same_target1025(a:&Path,b:&Path)->bool{if cfg!(windows){a.to_string_lossy().to_lowercase()==b.to_string_lossy().to_lowercase()}else{a==b}}
fn append_observer1025(workspace:&SourceWorkspace,files:&mut Vec<SourceRefactorFile>,file:SourceRefactorFile)->Result<(),String>{
    if file.before!=file.after{return Err("Mixed discard references must be readonly".into());}
    let mut checked=Vec::new();workspace.append_publication_files(&mut checked,std::slice::from_ref(&file))?;
    let path=target(&workspace.root,&file.path)?;
    for previous in files.iter(){
        if same_target1025(&target(&workspace.root,&previous.path)?,&path){
            if previous.before!=file.before||previous.after!=file.after{return Err("Mixed discard repeats a conflicting physical before-image".into());}
            return Ok(());
        }
    }
    files.push(file);Ok(())
}
fn widget_observers1025(value:&Recovery)->Vec<SourceRefactorFile>{
    let mut files=value.widgets.iter().map(|entry|SourceRefactorFile{path:entry.locator.clone(),before:None,after:None}).collect::<Vec<_>>();
    files.extend(widget_recovery::membership_baseline_files(&value.widget_memberships));files
}
pub(super) fn validate_cohort_discard1025(root:&Path,journal:&RefactorJournal,transition:&RecoveryTransition1025,
    before:&Recovery,after:&Recovery,before_snapshot:&Snapshot,after_snapshot:&Snapshot,
    before_source:&str,after_source:&str,before_sidecar:&str,after_sidecar:&str,
)->Result<(),String>{
    if before_source!=after_source||before_sidecar!=after_sidecar{return Err("Mixed discard cannot publish Source or presentation".into());}
    if transition.changes.iter().any(|change|!matches!(change,EnumRecoveryChange1025::Remove{..})){return Err("Mixed discard only admits exact Enum removals".into());}
    if validated_recovery_high_water(after,validated_snapshot_high_water(after_snapshot)?)?
        <validated_recovery_high_water(before,validated_snapshot_high_water(before_snapshot)?)?{
        return Err("Mixed discard lost a reserved graph identity".into());
    }
    let previous=SourceRecoveryBatch{graphs:before.graphs.clone(),widgets:before.widgets.clone(),widget_memberships:before.widget_memberships.clone(),asset_placements:before.asset_placements_after.clone(),enums:before.enums.clone()};
    widget_recovery::preserve_identities(&previous,&after.graphs,&after.widgets)?;
    widget_recovery::preserve_memberships(&previous,&after.graphs,&after.widget_memberships)?;
    let ids=after.graphs.iter().map(|graph|graph.graph_id).collect::<std::collections::BTreeSet<_>>();
    let placements=before.asset_placements_after.as_ref().map(|map|SourceAssetPlacements::from_entries(map.entries().filter(|row|ids.contains(&row.graph_id)).cloned())).transpose()?;
    if after.asset_placements_after!=placements{return Err("Mixed discard changed unmentioned Source placement data".into());}
    // All removed/retained Widget physical before-images must be included in
    // the durable journal, not checked just once before writing that journal.
    for file in widget_observers1025(before).into_iter().chain(widget_observers1025(after)){
        let path=target(root,&file.path)?;
        let actual=journal.files.iter().find(|item|target(root,&item.path).ok().is_some_and(|candidate|same_target1025(&candidate,&path))).ok_or("Mixed discard journal omitted a Widget before-image")?;
        if actual.before!=file.before||actual.after!=file.after{return Err("Mixed discard journal changed a Widget before-image".into());}
    }
    Ok(())
}
struct PreparedCohortDiscard1025{
    journal:RefactorJournal,
    cache:Option<String>,
    placements:Option<SourceAssetPlacements>,
    high:u64,
}
impl SourceWorkspace{
    /// `graphs/widgets/memberships` are the COMPLETE after-cohort prepared by
    /// the caller's existing ProjectDiscardScope, never a sparse selection.
    /// `expected` observes the complete actual previous cache (including None).
    fn prepare_recovery_cohort_discard1025(&self,expected:Option<&SourceRecoveryBatch>,
        graphs:&[GraphDocument],widgets:&[WidgetRecovery],memberships:&[WidgetMembershipRecovery],
        enum_changes:&[EnumRecoveryChange1025],references:&[SourceRefactorFile],
    )->Result<PreparedCohortDiscard1025,String>{
        self.check_recovery_baseline()?;self.check_recovery_owner()?;
        if let Some(error)=&self.source_error{return Err(error.clone());}
        let actual=self.recovery_batch()?;
        if actual.as_ref()!=expected{return Err("Mixed discard cache changed after its complete before-image".into());}
        if enum_changes.iter().any(|change|!matches!(change,EnumRecoveryChange1025::Remove{..})){return Err("Mixed discard only admits Enum Remove".into());}
        self.validate_recovery_with_widget_memberships(graphs,widgets,memberships)?;
        validate_recovery_graph_shapes(graphs)?;
        let before=self.raw_enum_recovery1025()?;let mut after=before.clone();
        after.enums=enum_recovery1025::apply_changes1025(&before.enums,enum_changes,true)?;
        after.graphs=graphs.to_vec();after.widgets=widgets.to_vec();after.widget_memberships=memberships.to_vec();
        after.asset_placements_after=self.placements_for_graphs(graphs)?;
        let committed_high=self.graph_id_high_water().max(graphs.iter().map(|graph|graph.graph_id.get()).max().unwrap_or(0));
        after.graph_id_high_water=Some(committed_high);
        enum_recovery1025::validate_records1025(&after.enums,&after.widgets)?;
        let retained_placements=after.asset_placements_after.clone();
        let sidecar=self.expected_sidecar.clone().ok_or("Mixed discard requires its actual published sidecar")?;
        let snapshot:Snapshot=serde_json::from_str(&sidecar).map_err(|error|error.to_string())?;
        let mut files=vec![SourceRefactorFile{path:self.relative_source.clone(),before:Some(self.project.source().into()),after:Some(self.project.source().into())},SourceRefactorFile{path:SIDECAR.into(),before:Some(sidecar.clone()),after:Some(sidecar.clone())}];
        for file in references.iter().cloned().chain(widget_observers1025(&before)).chain(widget_observers1025(&after)){
            append_observer1025(self,&mut files,file)?;
        }
        check_files(&self.root,&files,false)?;
        let next_recovery=serialize_after1025(after,self.project.source(),&sidecar,&snapshot)?;
        let journal=RefactorJournal{library_before1027:self.library_journal_readset1027(),library_after1027:self.library_journal_readset1027(),source:self.relative_source.clone(),graphs:self.project.graphs().to_vec(),files,recovery:Some(JournalRecovery1025::Transition(RecoveryTransition1025{kind:"enum_recovery_transition_v4".into(),version:4,mode:TransitionMode1025::CohortDiscard,before_recovery:self.expected_recovery.clone(),after_recovery:next_recovery.clone(),changes:enum_changes.to_vec(),library1027:None}))};
        let transition=match journal.recovery.as_ref().unwrap(){JournalRecovery1025::Transition(value)=>value,_=>unreachable!()};
        validate_transition1025(&self.root,&journal,transition)?;
        Ok(PreparedCohortDiscard1025{journal,cache:next_recovery,placements:retained_placements,high:committed_high})
    }
    pub fn discard_recovery_cohort_with1025<T>(&mut self,expected:Option<&SourceRecoveryBatch>,
        graphs:&[GraphDocument],widgets:&[WidgetRecovery],memberships:&[WidgetMembershipRecovery],
        enum_changes:&[EnumRecoveryChange1025],references:&[SourceRefactorFile],install:impl FnOnce()->T,
    )->Result<T,String>{
        let _lock=lock(&self.root)?;recover(&self.root)?;
        let prepared=self.prepare_recovery_cohort_discard1025(expected,graphs,widgets,memberships,enum_changes,references)?;
        atomic_write(&self.root.join(REFACTOR_JOURNAL),&serde_json::to_string(&prepared.journal).map_err(|error|error.to_string())?)?;
        apply(&self.root,&prepared.journal,false)?;
        // No fallible operation or second transaction after the durable commit.
        self.expected_recovery=prepared.cache;self.asset_placements=prepared.placements;
        self.graph_id_high_water=self.graph_id_high_water.max(prepared.high);
        Ok(install())
    }
}
#[cfg(test)]
#[path="source_recovery_cohort_discard_tests1025.rs"]
mod tests;
