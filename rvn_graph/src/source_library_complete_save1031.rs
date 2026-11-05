//! Complete Current Library/ordinary publication through the existing typed
//! Source journal. This is data preparation and one publisher, never a second
//! journal, a loop of partial saves, or an editor/native completion capability.
use super::*;
use rvn_parser::function_library_asset1023::FunctionLibraryId;
use std::collections::{BTreeMap, BTreeSet};

/// Immutable description of one successful CompleteSource publication.
/// Construction is private to its real writer. These images cannot authorize
/// a later write, acknowledge an editor, or stand in for a Created/Ready owner.
pub struct SourceLibraryCompletePublication1031 {
    source: String,
    before_source: String,
    after_source: String,
    before_saved: Vec<GraphDocument>,
    before_current: SourceRecoveryCohortLibrary1027,
    after_saved: Vec<GraphDocument>,
    saved_libraries: Vec<FunctionLibraryId>,
    files: Vec<SourceRefactorFile>,
    after: SourceLibraryRecoveryPublication1029,
}
impl SourceLibraryCompletePublication1031 {
    pub fn source1031(&self) -> &str { &self.source }
    pub fn before_source1031(&self) -> &str { &self.before_source }
    pub fn after_source1031(&self) -> &str { &self.after_source }
    pub fn before_saved1031(&self) -> &[GraphDocument] { &self.before_saved }
    pub fn before_current1031(&self) -> &SourceRecoveryCohortLibrary1027 { &self.before_current }
    pub fn after_saved1031(&self) -> &[GraphDocument] { &self.after_saved }
    /// Includes original Saved owners and exact Current removals, not just
    /// nonempty Libraries or the currently open toolkit.
    pub fn saved_libraries1031(&self) -> &[FunctionLibraryId] { &self.saved_libraries }
    pub fn files1031(&self) -> &[SourceRefactorFile] { &self.files }
    pub fn after1031(&self) -> &SourceLibraryRecoveryPublication1029 { &self.after }
}

/// Owned result of the real durable CompleteSource executor. It must be
/// retained by a caller before any fallible UI/notification work. Descriptive
/// images alone do not establish commit, and this result grants no native ACK.
#[must_use]
pub struct SourceLibraryCompleteCommitted1031 {
    committed:VerifiedCommitted1031,
    publication:SourceLibraryCompletePublication1031,
}
impl SourceLibraryCompleteCommitted1031 {
    pub fn committed1031(&self)->&VerifiedCommitted1031 { &self.committed }
    pub fn publication1031(&self)->&SourceLibraryCompletePublication1031 { &self.publication }
    pub fn into_parts1031(self)->(VerifiedCommitted1031,SourceLibraryCompletePublication1031){(self.committed,self.publication)}
}
struct PreparedComplete1031 {
    next:SourceProject,files:Vec<SourceRefactorFile>,library:LibraryTransition1027,
    enums:Vec<EnumRecoveryChange1025>,publication:SourceLibraryCompletePublication1031,
}

pub(super) struct PreparedCandidate1031 {
    pub(super) next: SourceProject,
    pub(super) files: Vec<SourceRefactorFile>,
    pub(super) snapshot: Snapshot,
    pub(super) sidecar: String,
}
/// Shared read-only preparer. Partial Save supplies its original published
/// placements; CompleteSource supplies the original complete Current map.
/// It has no commit callback and performs no filesystem mutation.
pub(super) fn prepare_candidate1031(
    workspace: &SourceWorkspace,
    expected: &SourceRecoveryCohortLibrary1027,
    source: &str,
    published_graphs: &[GraphDocument],
    additional: &[SourceRefactorFile],
    placements: Option<SourceAssetPlacements>,
) -> Result<PreparedCandidate1031, String> {
    let mut files = vec![SourceRefactorFile {
        path: workspace.relative_source.clone(),
        before: Some(workspace.project.source().into()),
        after: Some(source.into()),
    }];
    workspace.append_publication_files(&mut files, additional)?;
    check_files(&workspace.root, &files, false)?;
    let inputs = SourceLibraryInputs1027::from_transaction1027(&workspace.root, &files, true)?.map(std::sync::Arc::new);
    let next = SourceProject::open_library_at1027(
        &workspace.source_path(), source.into(), published_graphs, inputs,
        workspace.project.library_inputs1027().cloned(), &workspace.root, None, false,
    )?;
    if next.graphs() != published_graphs {
        return Err("Library Save Source differs from its actual published graph projection".into());
    }
    let snapshot = Snapshot {
        asset_placements: placements.clone(),
        graph_id_high_water: Some(workspace.graph_id_high_water().max(expected.high_water1027()).max(next.graphs().iter().map(|g| g.graph_id.get()).max().unwrap_or(0))),
        version: if placements.is_some() { 2 } else { 1 },
        source: workspace.relative_source.clone(), source_snapshot: source.into(),
        graphs: next.graphs().to_vec(), imports: next.imports_snapshot(),
    };
    let sidecar = serde_json::to_string_pretty(&snapshot).map_err(|e| e.to_string())?;
    files.push(SourceRefactorFile { path: SIDECAR.into(), before: workspace.expected_sidecar.clone(), after: Some(sidecar.clone()) });
    Ok(PreparedCandidate1031 { next, files, snapshot, sidecar })
}
fn same_images1031(left: &[GraphDocument], right: &[GraphDocument]) -> bool {
    let a = left.iter().map(|g| (g.graph_id, g)).collect::<BTreeMap<_, _>>();
    let b = right.iter().map(|g| (g.graph_id, g)).collect::<BTreeMap<_, _>>();
    a.len() == left.len() && b.len() == right.len() && a == b
}
fn all_library_ids1031(project: &SourceProject, records: &[crate::source_function_library_recovery1023::FunctionLibraryRecovery]) -> Result<Vec<FunctionLibraryId>, String> {
    let mut ids = library_recovery1027::saved_baselines_for_project1027(project)?.into_iter().map(|r| r.id().clone()).collect::<BTreeSet<_>>();
    ids.extend(records.iter().map(|r| r.id().clone()));
    Ok(ids.into_iter().collect())
}
fn manifest1031(project: &SourceProject) -> Result<toml::Value, String> {
    let bytes = project.library_inputs1027().and_then(|input| input.source_references().iter().find(|f| f.path == "rvn.toml")).and_then(|f| f.before.as_deref());
    match bytes { Some(bytes) => bytes.parse::<toml::Value>().map_err(|e| e.to_string()), None => Ok(toml::Value::Table(Default::default())) }
}
fn preserve_nonlibrary_manifest1031(before: &SourceProject, after: &SourceProject) -> Result<(), String> {
    let (mut before, mut after) = (manifest1031(before)?, manifest1031(after)?);
    if before.get("function_libraries").is_some() && after.get("function_libraries").is_none() {
        return Err("Complete Library Source Save cannot drop its original indexed namespace policy".into());
    }
    before.as_table_mut().ok_or("Invalid original Source manifest")?.remove("function_libraries");
    after.as_table_mut().ok_or("Invalid candidate Source manifest")?.remove("function_libraries");
    if before != after { return Err("Complete Library Source Save changed an unjoined non-Library manifest key or index row".into()); }
    Ok(())
}

pub(super) fn validate_complete1031(
    journal: &RefactorJournal, transition: &RecoveryTransition1025,
    before: &Recovery, after: &Recovery, before_snapshot: &Snapshot,
    after_snapshot: &Snapshot, before_project: &SourceProject, after_project: &SourceProject,
) -> Result<(), String> {
    // The existing scoped visual writer explicitly lacks the readonly-import
    // owner mapping. A full re-emission must not silently drop Use statements
    // or adopt imported definitions as writable Current owners.
    if before_project.has_imports() || after_project.has_imports() {
        return Err("Complete Library Source Save requires its original read-only imported owner mapping; imported Source was not flattened or acknowledged".into());
    }
    let library = transition.library1027.as_ref().ok_or("Complete Library Source Save lost its typed inventory")?;
    let changes = before.libraries.iter().cloned().map(|before| LibraryRecoveryChange1027::Remove { before }).collect::<Vec<_>>();
    if library.saved.is_empty() || library.changes != changes || !after.libraries.is_empty() || library.saved != all_library_ids1031(before_project, &before.libraries)? {
        return Err("Complete Library Source Save must acknowledge every exact original Saved/Current UUID and record".into());
    }
    let prior = library_recovery1027::saved_baselines_for_project1027(before_project)?;
    let expected = crate::source_function_library_recovery1023::validate_set(&prior, &before.libraries)?;
    if expected != library_recovery1027::saved_baselines_for_project1027(after_project)? {
        return Err("Complete Library Source Save differs from its full Current descriptor/index inventory".into());
    }
    preserve_nonlibrary_manifest1031(before_project, after_project)?;
    if !same_images1031(&before.graphs, &after_snapshot.graphs) || before.graphs != after_snapshot.graphs || after.graphs != after_snapshot.graphs {
        return Err("Complete Library Source Save must publish every complete Current graph image in its exact original editor order without missing, duplicate or substituted identity".into());
    }
    if before.asset_placements_after != after_snapshot.asset_placements
        || after.asset_placements_after != after_snapshot.asset_placements
        || validated_snapshot_high_water(after_snapshot)? < validated_recovery_high_water(before, validated_snapshot_high_water(before_snapshot)?)? {
        return Err("Complete Library Source Save changed a Current placement or dropped a reserved identity".into());
    }
    let enum_changes = before.enums.iter().cloned().map(|before| EnumRecoveryChange1025::Remove { before }).collect::<Vec<_>>();
    if transition.changes != enum_changes || !after.enums.is_empty() || !after.widgets.is_empty() || !after.widget_memberships.is_empty() || transition.after_recovery.is_some() {
        return Err("Complete Library Source Save must publish its exact joined metadata cohort, without residual or silently erased records".into());
    }
    let mut required = crate::source_function_library_recovery1023::publication_files(&before.libraries)?;
    for widget in &before.widgets {
        required.push(SourceRefactorFile { path: widget.locator.clone(), before: None, after: Some(widget.blueprint_json.clone()) });
    }
    for widget in &before.widget_memberships {
        required.push(SourceRefactorFile { path: widget.locator.clone(), before: Some(widget.before_blueprint_json.clone()), after: Some(widget.after_blueprint_json.clone()) });
    }
    for entry in &before.enums {
        required.push(SourceRefactorFile { path: entry.locator().into(), before: entry.saved_wire().map(str::to_owned), after: Some(entry.current().encode()?) });
    }
    for file in &required {
        if !journal.files.iter().any(|actual| actual == file) { return Err("Complete Library Source Save omitted an exact joined descriptor publication".into()); }
    }
    for file in &journal.files {
        if file.before != file.after && file.path != journal.source && file.path != SIDECAR && file.path != "rvn.toml" && !required.iter().any(|required| required == file) {
            return Err("Complete Library Source Save contains an unjoined authored file mutation".into());
        }
    }
    Ok(())
}

impl SourceWorkspace {
    /// One full Current publication, under the original Source lock/readsets.
    /// The optional caller preflight must retain its original project/model
    /// guards. It runs before the common publisher revalidates all Source and
    /// physical CAS participants. The infallible installer runs only on success.
    pub fn save_complete_library_recovery_receipt1031<T>(
        &mut self, expected: &SourceRecoveryCohortLibrary1027, source: &str,
        published_graphs: &[GraphDocument], additional: &[SourceRefactorFile],
        preflight: impl FnOnce() -> Result<(), String>, install: impl FnOnce() -> T,
    ) -> Result<(T, SourceLibraryCompletePublication1031), String> {
        let _lock = lock(&self.root)?;
        recover(&self.root)?;
        let PreparedComplete1031{next,files,library,enums,publication}=self.prepare_complete1031(expected,source,published_graphs,additional)?;
        preflight()?;
        self.publish_typed_transition1030(next, &files, &enums, TransitionMode1025::CompleteSource, Some(&library))?;
        Ok((install(), publication))
    }
    fn prepare_complete1031(&self,expected:&SourceRecoveryCohortLibrary1027,source:&str,published_graphs:&[GraphDocument],additional:&[SourceRefactorFile])->Result<PreparedComplete1031,String>{
        self.check_recovery_baseline()?; self.check_recovery_owner()?;
        if let Some(error) = &self.source_error { return Err(error.clone()); }
        if !self.project.observes_library_inputs1027() { return Err("Complete Library Source Save requires its original scoped Source observation".into()); }
        let actual = self.recovery_cohort_libraries1027()?.ok_or("Complete Library Source Save lost its original Current cohort")?;
        if actual != *expected { return Err("Complete Library Source Save changed before its exact original Current cohort".into()); }
        self.project.check_imports()?;
        if self.project.has_imports() {
            return Err("Complete Library Source Save requires its original read-only imported owner mapping".into());
        }
        let PreparedCandidate1031 { next, files, snapshot, sidecar: _ } = prepare_candidate1031(self, expected, source, published_graphs, additional, actual.batch.asset_placements.clone())?;
        let saved = all_library_ids1031(&self.project, &actual.libraries)?;
        if saved.is_empty() { return Err("Complete Library Source Save has no original Saved or Current Library owner; generic v5 admission refused".into()); }
        let library = LibraryTransition1027 { changes: actual.libraries.iter().cloned().map(|before| LibraryRecoveryChange1027::Remove { before }).collect(), saved: saved.clone() };
        let enums = actual.batch.enums.iter().cloned().map(|before| EnumRecoveryChange1025::Remove { before }).collect::<Vec<_>>();
        // Allocate every descriptive before/after image before journal or target
        // IO. No successful publication consults SourceLatest to synthesize it.
        let publication = SourceLibraryCompletePublication1031 {
            source: self.relative_source.clone(), before_source: self.project.source().into(), after_source: source.into(),
            before_saved: self.project.graphs().to_vec(), before_current: actual,
            after_saved: next.graphs().to_vec(), saved_libraries: saved, files: files.clone(),
            after: SourceLibraryRecoveryPublication1029 { cohort: None, placements: snapshot.asset_placements.clone(), high_water: self.graph_id_high_water().max(snapshot.graph_id_high_water.unwrap_or(0)) },
        };
        Ok(PreparedComplete1031{next,files,library,enums,publication})
    }
    /// Callback-free CompleteSource publication. The original full cohort,
    /// graph order, scoped owner mappings and physical readsets are required.
    /// A pending typed journal refuses a new writer; maintenance is explicit.
    pub fn save_complete_library_recovery_verified1031(
        &mut self,expected:&SourceRecoveryCohortLibrary1027,source:&str,
        published_graphs:&[GraphDocument],additional:&[SourceRefactorFile],
    )->Result<SourceLibraryCompleteCommitted1031,CauseBeforeCommit1031>{
        self.save_complete_library_verified_inner1031(expected,source,published_graphs,additional,&Execution1031::default()).map_err(Into::into)
    }
    pub(super) fn save_complete_library_verified_inner1031(
        &mut self,expected:&SourceRecoveryCohortLibrary1027,source:&str,
        published_graphs:&[GraphDocument],additional:&[SourceRefactorFile],execution:&Execution1031,
    )->Result<SourceLibraryCompleteCommitted1031,String>{
        let _lock=lock(&self.root)?;
        if self.root.join(JOURNAL).try_exists().map_err(|e|e.to_string())?||self.root.join(REFACTOR_JOURNAL).try_exists().map_err(|e|e.to_string())?{
            return Err("Terminez la maintenance de la transaction Source existante avant un nouvel enregistrement vérifié".into());
        }
        let PreparedComplete1031{next,files,library,enums,publication}=self.prepare_complete1031(expected,source,published_graphs,additional)?;
        let committed=self.publish_complete_verified1031(next,&files,&enums,&library,execution)?;
        // No callback or allocation follows the genuine physical proof.
        Ok(SourceLibraryCompleteCommitted1031{committed,publication})
    }

}
