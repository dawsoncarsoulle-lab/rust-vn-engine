//! Partial Library Save in the EXISTING Source/refactor/recovery transaction.
//! The caller supplies a strict prepared Source candidate, not a write grant.
use super::*;
use rvn_parser::function_library_asset1023::FunctionLibraryId;
use std::collections::BTreeSet;
fn owner(graph:&GraphDocument)->Option<&FunctionLibraryId>{graph.library1027.as_ref().and_then(|m|m.owner.as_ref())}
fn nonselected_source(source:&str,selected:&BTreeSet<&FunctionLibraryId>)->Result<rvn_parser::Script,String>{Ok(rvn_parser::parse(source).map_err(|e|e.to_string())?.into_iter().filter(|s|!matches!(s,rvn_parser::Statement::FunctionLibrary{scope}if selected.contains(scope.library()))).collect())}
fn manifest(project:&SourceProject)->Result<toml::Value,String>{let bytes=project.library_inputs1027().and_then(|input|input.source_references().iter().find(|f|f.path=="rvn.toml")).and_then(|f|f.before.as_deref());match bytes{Some(bytes)=>bytes.parse::<toml::Value>().map_err(|e|e.to_string()),None=>Ok(toml::Value::Table(Default::default()))}}
fn preserve_unselected_manifest(before:&SourceProject,after:&SourceProject,selected:&BTreeSet<&FunctionLibraryId>)->Result<(),String>{
    let(mut before,mut after)=(manifest(before)?,manifest(after)?);let before_had_table=before.get("function_libraries").is_some();
    for doc in [&mut before,&mut after]{if let Some(table)=doc.get_mut("function_libraries").and_then(toml::Value::as_table_mut){table.retain(|name,_|!FunctionLibraryId::parse(name.clone()).is_ok_and(|id|selected.contains(&id)));}}
    // The first Library may introduce its table. Existing table presence stays
    // exact: dropping the last row cannot implicitly weaken indexed admission.
    if !before_had_table&&after.get("function_libraries").and_then(toml::Value::as_table).is_some_and(|table|table.is_empty()){after.as_table_mut().ok_or("Invalid Source manifest root")?.remove("function_libraries");}
    if before!=after{return Err("Partial Library Save changed an unselected manifest key or index row".into())}Ok(())
}
pub(super) fn validate_partial1027(journal:&RefactorJournal,transition:&RecoveryTransition1025,before:&Recovery,after:&Recovery,before_snapshot:&Snapshot,after_snapshot:&Snapshot,before_project:&SourceProject,after_project:&SourceProject)->Result<(),String>{
    let library=transition.library1027.as_ref().ok_or("Partial Library Save has no typed participant")?;let selected=library.saved.iter().collect::<BTreeSet<_>>();if selected.is_empty()||selected.len()!=library.saved.len(){return Err("Partial Library Save requires unique actual selected UUIDs".into())}
    if !transition.changes.is_empty(){return Err("Partial Library Save cannot acknowledge unrelated Enum edits".into())}
    let expected_changes=before.libraries.iter().filter(|r|selected.contains(r.id())).cloned().map(|before|LibraryRecoveryChange1027::Remove{before}).collect::<Vec<_>>();
    if library.changes!=expected_changes{return Err("Partial Library Save must acknowledge exactly the selected original records".into())}
    preserve_unselected_manifest(before_project,after_project,&selected)?;
    let prior=library_recovery1027::saved_baselines_for_project1027(before_project)?;let changes=before.libraries.iter().filter(|r|selected.contains(r.id())).cloned().collect::<Vec<_>>();let expected=crate::source_function_library_recovery1023::validate_set(&prior,&changes)?;let actual=library_recovery1027::saved_baselines_for_project1027(after_project)?;if expected!=actual{return Err("Partial Library Save changed an unselected descriptor/index row".into())}
    for id in &selected{if !prior.iter().any(|r|r.id()==*id)&&!before.libraries.iter().any(|r|r.id()==*id){return Err("Selected Library UUID has no original Saved or Current owner".into())}}
    let unselected_before=before_snapshot.graphs.iter().filter(|g|owner(g).is_none_or(|o|!selected.contains(o))).collect::<Vec<_>>();let unselected_after=after_snapshot.graphs.iter().filter(|g|owner(g).is_none_or(|o|!selected.contains(o))).collect::<Vec<_>>();if unselected_before!=unselected_after{return Err("Partial Library Save would publish an unselected graph edit".into())}
    let selected_current=before.graphs.iter().filter(|g|owner(g).is_some_and(|o|selected.contains(o))).map(|g|(g.graph_id,g)).collect::<std::collections::BTreeMap<_,_>>();let selected_saved=after_snapshot.graphs.iter().filter(|g|owner(g).is_some_and(|o|selected.contains(o))).map(|g|(g.graph_id,g)).collect::<std::collections::BTreeMap<_,_>>();if selected_current!=selected_saved{return Err("Partial Library Save differs from the exact selected Current models".into())}
    if nonselected_source(&before_snapshot.source_snapshot,&selected)?!=nonselected_source(&after_snapshot.source_snapshot,&selected)?{return Err("Partial Library Save changed an unselected authored Source statement".into())}
    if before_snapshot.asset_placements!=after_snapshot.asset_placements{return Err("Partial Library Save changed an unrelated published asset placement".into())}
    if before.graphs!=after.graphs||before.widgets!=after.widgets||before.widget_memberships!=after.widget_memberships||before.asset_placements_after!=after.asset_placements_after||before.enums!=after.enums||after.graph_id_high_water.unwrap_or(0)<before.graph_id_high_water.unwrap_or(0){return Err("Partial Library Save lost an unselected Current participant or reserved identity".into())}
    let required=crate::source_function_library_recovery1023::publication_files(&changes)?;for file in &required{if !journal.files.iter().any(|f|f==file){return Err("Partial Library Save omitted an exact selected descriptor publication".into())}}
    for file in &journal.files{if file.before!=file.after&&file.path!=journal.source&&file.path!=SIDECAR&&file.path!="rvn.toml"&&!required.iter().any(|f|f==file){return Err("Partial Library Save added an unrelated authored file mutation".into())}}
    Ok(())
}
impl SourceWorkspace{
    /// Save only the selected Library models/descriptor rows. All other Current
    /// graphs, widgets, Enums and Library records remain in the same recovery.
    /// `source` and `published_graphs` must already be a coherent strict candidate;
    /// this method independently admits them and checks the exact selected scope.
    pub fn save_library_recovery_with1027<T>(&mut self,expected:&SourceRecoveryCohortLibrary1027,selected:&[FunctionLibraryId],source:&str,published_graphs:&[GraphDocument],additional:&[SourceRefactorFile],install:impl FnOnce()->T)->Result<T,String>{
        self.save_library_recovery_receipt1030(expected,selected,source,published_graphs,additional,install).map(|(value,_)|value)
    }
    /// The SAME partial Library publisher. Allocate/validate its descriptive
    /// after-image before IO; return it only after actual publication succeeds.
    pub fn save_library_recovery_receipt1030<T>(&mut self,expected:&SourceRecoveryCohortLibrary1027,selected:&[FunctionLibraryId],source:&str,published_graphs:&[GraphDocument],additional:&[SourceRefactorFile],install:impl FnOnce()->T)->Result<(T,SourceLibraryRecoveryPublication1029),String>{
        let _lock=lock(&self.root)?;recover(&self.root)?;self.check_recovery_baseline()?;self.check_recovery_owner()?;if let Some(error)=&self.source_error{return Err(error.clone())}if !self.project.observes_library_inputs1027(){return Err("Library Save requires the original scoped Source observation".into())}
        let actual=self.recovery_cohort_libraries1027()?.ok_or("Library Save lost its original Current cohort")?;if actual!=*expected{return Err("Library Save changed before its original exact Current cohort".into())}self.project.check_imports()?;
        let library_complete_save1031::PreparedCandidate1031{mut next,files,snapshot,sidecar}=library_complete_save1031::prepare_candidate1031(self,expected,source,published_graphs,additional,self.published_asset_placements.clone())?;
        let selected_set=selected.iter().collect::<BTreeSet<_>>();let changes=actual.libraries.iter().filter(|r|selected_set.contains(r.id())).cloned().map(|before|LibraryRecoveryChange1027::Remove{before}).collect::<Vec<_>>();let mut value=self.raw_enum_recovery1025()?;value.libraries=library_recovery1027::apply_changes1027(&value.libraries,&changes)?;
        // Current owns its complete ordered graph vector. A coherent Saved
        // candidate may use another presentation order; never replace Current
        // to manufacture clean. The existing exact serializer alone decides
        // whether the complete Current image is genuinely Saved and removable.
        let after=serialize_after1025(value,next.source(),&sidecar,&snapshot)?;
        let transition=RecoveryTransition1025{kind:"source_library_recovery_transition_v5".into(),version:5,mode:TransitionMode1025::PartialLibraries,before_recovery:self.expected_recovery.clone(),after_recovery:after.clone(),changes:Vec::new(),library1027:Some(LibraryTransition1027{changes,saved:selected.to_vec()})};// No fallible receipt derivation after package/source/recovery publication.
        let receipt_cohort1030=after.as_ref().map(|wire|->Result<SourceRecoveryCohortLibrary1027,String>{
            let mut record:Recovery=serde_json::from_str(wire).map_err(|error|error.to_string())?;
            let reserved_high1027=validated_recovery_high_water(&record,snapshot.graph_id_high_water.unwrap_or(0))?;
            for graph in &mut record.graphs{graph.normalize_assignment_value_outputs().map_err(|error|error.to_string())?;}
            Ok(SourceRecoveryCohortLibrary1027{libraries:record.libraries,reserved_high1027,
                batch:SourceRecoveryBatch{graphs:record.graphs,widgets:record.widgets,widget_memberships:record.widget_memberships,enums:record.enums,asset_placements:record.asset_placements_after}})
        }).transpose()?;
        let publication1030=SourceLibraryRecoveryPublication1029{cohort:receipt_cohort1030,placements:self.asset_placements.clone(),high_water:self.graph_id_high_water.max(snapshot.graph_id_high_water.unwrap_or(0))};
        let journal=RefactorJournal{library_before1027:self.library_journal_readset1027(),library_after1027:library_inputs1027::project_readset1027(&next),source:self.relative_source.clone(),graphs:next.graphs().to_vec(),files,recovery:Some(JournalRecovery1025::Transition(transition.clone()))};validate_transition1025(&self.root,&journal,&transition)?;atomic_write(&self.root.join(REFACTOR_JOURNAL),&serde_json::to_string(&journal).map_err(|e|e.to_string())?)?;apply(&self.root,&journal,false)?;
        next.commit_library_inputs1027();self.project=next;self.expected_sidecar=Some(sidecar);self.expected_recovery=after;self.published_asset_placements=snapshot.asset_placements;self.graph_id_high_water=self.graph_id_high_water.max(snapshot.graph_id_high_water.unwrap_or(0));self.snapshot_matches_project=true;Ok((install(),publication1030))
    }
}
