//! Recoverable source/presentation transactions. Authored RVN is authoritative.
use crate::{GraphDocument, GraphId, SourceProject};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write,
    path::{Component, Path, PathBuf},
};

const SIDECAR: &str = ".rvn-authoring.json";
const JOURNAL: &str = ".rvn-authoring.transaction.json";
const RECOVERY: &str = ".rvn-authoring.recovery.json";
#[path = "source_refactor.rs"]
mod refactor;
pub use refactor::{SourceRefactorFile,SourceRefactorReceipt,CauseBeforeCommit1031,CleanupWarning1031,CleanupStatus1031,VerifiedCommitted1031,SourceLibraryCompletePublication1031,SourceLibraryCompleteCommitted1031};
#[path = "source_widget_recovery.rs"]
mod widget_recovery;
pub use widget_recovery::{SourceRecoveryBatch, WidgetRecovery, WidgetMembershipRecovery};
#[path="source_asset_placement1023.rs"]
mod asset_placement1023;
pub use asset_placement1023::{SourceAssetPlacement,SourceAssetPlacements,SourcePlacementNamespace};
#[path="source_asset_placement_workspace1023.rs"]
mod asset_placement_workspace1023;
#[path="source_library_inputs1027.rs"]
mod library_inputs1027;
pub use library_inputs1027::SourceLibraryInputs1027;
use library_inputs1027::validate_published_snapshot_library1027;
#[path="source_library_refactor1027.rs"]
mod library_refactor1027;
#[path="source_library_recovery1027.rs"]
mod library_recovery1027;
pub use library_recovery1027::{LibraryRecoveryChange1027,SourceRecoveryCohortLibrary1027,SourceLibraryMemberIdentity1027};
#[cfg(test)]
#[path="source_library_tests1027.rs"]
mod library_tests1027;

use asset_placement_workspace1023::{AssetPlacementPublication,strict_asset_placements,validate_snapshot_editorial,validate_recovery_editorial,recover_asset_placement_publication};

#[path="source_enum_recovery1025.rs"]
mod enum_recovery1025;
pub use enum_recovery1025::EnumRecoveryChange1025;
#[path="source_reservation_only_recovery1025.rs"]
mod reservation_only1025;

/// Descriptive after-image emitted only by a successful existing Library writer.
/// It is neither authoring admission nor native/runtime completion authority.
pub struct SourceLibraryRecoveryPublication1029 {
    cohort: Option<SourceRecoveryCohortLibrary1027>,
    placements: Option<SourceAssetPlacements>,
    high_water: u64,
}
impl SourceLibraryRecoveryPublication1029 {
    pub fn cohort1029(&self) -> Option<&SourceRecoveryCohortLibrary1027> { self.cohort.as_ref() }
    pub fn placements1029(&self) -> Option<&SourceAssetPlacements> { self.placements.as_ref() }
    pub fn high_water1029(&self) -> u64 { self.high_water }
    pub fn into_after1029(self) -> (Option<SourceRecoveryCohortLibrary1027>,Option<SourceAssetPlacements>,u64) { (self.cohort,self.placements,self.high_water) }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Recovery {
    #[serde(default,skip_serializing_if="Vec::is_empty")]
    libraries:Vec<crate::source_function_library_recovery1023::FunctionLibraryRecovery>,
    #[serde(default,skip_serializing_if="Vec::is_empty")]
    enums:Vec<rvn_parser::enum_recovery1025::EnumRecoveryRecord1025>,
    #[serde(default,skip_serializing_if="Option::is_none",deserialize_with="strict_asset_placements")]
    asset_placements_after:Option<SourceAssetPlacements>,
    #[serde(default, skip_serializing_if="Option::is_none", deserialize_with="strict_graph_id_high_water")]
    graph_id_high_water: Option<u64>,
    version: u32,
    source: String,
    baseline_source: String,
    baseline_sidecar: Option<String>,
    graphs: Vec<GraphDocument>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    widgets: Vec<WidgetRecovery>,
    #[serde(default, skip_serializing_if="Vec::is_empty")]
    widget_memberships: Vec<WidgetMembershipRecovery>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Snapshot {
    #[serde(default,skip_serializing_if="Option::is_none",deserialize_with="strict_asset_placements")]
    asset_placements:Option<SourceAssetPlacements>,
    #[serde(default, skip_serializing_if="Option::is_none", deserialize_with="strict_graph_id_high_water")]
    graph_id_high_water: Option<u64>,
    version: u32,
    source: String,
    source_snapshot: String,
    graphs: Vec<GraphDocument>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    imports: Option<crate::source_project::ImportSnapshot>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Transaction {
    #[serde(default,skip_serializing_if="Option::is_none")]
    library_readset1027:Option<library_inputs1027::LibraryReadset1027>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    editorial_publication:Option<AssetPlacementPublication>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    reservation_archive:Option<ReservationArchive>,
    source: String,
    before_source: String,
    after_source: String,
    before_sidecar: Option<String>,
    after_sidecar: String,
}

#[derive(Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct ReservationArchive{before_recovery:Option<String>}

#[derive(Debug, Clone)]
pub struct SourceWorkspace {
    library_origin1027:std::sync::Arc<()>,
    // Immutable projection of the SAME expected_sidecar, avoids reparsing graphs per observation.
    published_asset_placements:Option<SourceAssetPlacements>,
    asset_placements:Option<SourceAssetPlacements>,
    graph_id_high_water: u64,
    root: PathBuf,
    relative_source: String,
    expected_sidecar: Option<String>,
    expected_recovery: Option<String>,
    project: SourceProject,
    // Set only after full import/validation, when the on-disk snapshot needs
    // neither migration, source refresh nor project-reference reconciliation.
    snapshot_matches_project: bool,
    pub source_error: Option<String>,
}

impl SourceWorkspace {
    /// Explicit linking never removes or overwrites legacy graph files.
    pub fn link(
        root: &Path,
        source: &Path,
        presentation: &[GraphDocument],
    ) -> Result<Self, String> {
        Self::link_mode(root, source, presentation, false)
    }
    pub fn link_native_preparation(
        root: &Path,
        source: &Path,
        presentation: &[GraphDocument],
    ) -> Result<Self, String> {
        Self::link_mode(root, source, presentation, true)
    }
    fn link_mode(root:&Path,source:&Path,presentation:&[GraphDocument],native_preparation:bool)->Result<Self,String>{Self::link_mode_library1027(root,source,presentation,native_preparation,false)}
    pub fn link_scoped_libraries1027(root:&Path,source:&Path,presentation:&[GraphDocument])->Result<Self,String>{Self::link_mode_library1027(root,source,presentation,false,true)}
    pub fn link_scoped_native_libraries1027(root:&Path,source:&Path,presentation:&[GraphDocument])->Result<Self,String>{Self::link_mode_library1027(root,source,presentation,true,true)}
    fn link_mode_library1027(root:&Path,source:&Path,presentation:&[GraphDocument],native_preparation:bool,scoped_libraries:bool)->Result<Self,String>{
        let root = fs::canonicalize(root).map_err(|error| error.to_string())?;
        let _lock = lock(&root)?;
        recover(&root)?;
        if root.join(SIDECAR).exists() {
            return Err("A source is already linked to this project; reopen it before changing authoring mode.".into());
        }
        let source = fs::canonicalize(source).map_err(|error| error.to_string())?;
        let relative = source
            .strip_prefix(&root)
            .map_err(|_| "The authored RVN file must be inside the project folder")?;
        if relative
            .extension()
            .and_then(|extension| extension.to_str())
            != Some("rvn")
        {
            return Err("Choose an authored .rvn file".into());
        }
        let relative_source = relative
            .to_str()
            .ok_or("The source path must be valid Unicode")?
            .replace('\\', "/");
        let contents = fs::read_to_string(&source).map_err(|error| error.to_string())?;
        let library_inputs=if scoped_libraries{SourceLibraryInputs1027::read(&root)?.map(std::sync::Arc::new)}else{None};
        let project = if scoped_libraries {SourceProject::open_library_at1027(&source,contents,presentation,library_inputs.clone(),library_inputs,&root,None,native_preparation)?}else if native_preparation {
            SourceProject::open_native_preparation_at(&source, contents, presentation)?
        } else {
            SourceProject::open_at(&source, contents, presentation)?
        };
        let mut workspace = Self {
            library_origin1027:std::sync::Arc::new(()),
            graph_id_high_water: project.graphs().iter().map(|g|g.graph_id.get()).max().unwrap_or(0),
            root,
            relative_source,
            published_asset_placements:None,
            asset_placements:None,
            expected_sidecar: None,
            expected_recovery: None,
            project,
            snapshot_matches_project: false,
            source_error: None,
        };
        let graphs = workspace.project.graphs().to_vec();
        workspace.save_locked(&graphs, &[])?;
        Ok(workspace)
    }

    /// Invalid external source retains the last valid graph, with its error.
    pub fn open(root: &Path) -> Result<Option<Self>, String> {
        Self::open_mode(root, false, 0)
    }
    pub fn open_native_preparation(root: &Path) -> Result<Option<Self>, String> {
        Self::open_mode(root, true, 0)
    }
    pub fn open_with_graph_id_floor(root:&Path, floor:u64)->Result<Option<Self>,String>{
        Self::open_mode(root,false,floor)
    }
    fn open_mode(root:&Path,native_preparation:bool,floor:u64)->Result<Option<Self>,String>{Self::open_mode_library1027(root,native_preparation,floor,false)}
    pub fn open_scoped_libraries1027(root:&Path,floor:u64)->Result<Option<Self>,String>{Self::open_mode_library1027(root,false,floor,true)}
    pub fn open_scoped_native_libraries1027(root:&Path,floor:u64)->Result<Option<Self>,String>{Self::open_mode_library1027(root,true,floor,true)}
    /// Read-only identity for the same source/controller frontier. App guards
    /// retain this alongside Source bytes/models; it is no additional registry.
    pub fn source_library_inputs1027(&self)->Option<std::sync::Arc<SourceLibraryInputs1027>>{self.project.library_inputs1027().cloned()}
    pub fn source_observes_libraries1027(&self)->bool{self.project.observes_library_inputs1027()}
    pub fn validate_source_library_inputs1027(&self)->Result<(),String>{self.project.check_library_inputs1027()}
    fn open_mode_library1027(root:&Path,native_preparation:bool,floor:u64,scoped_libraries:bool)->Result<Option<Self>,String>{
        let root = fs::canonicalize(root).map_err(|error| error.to_string())?;
        if !root.join(SIDECAR).exists()
            && !root.join(JOURNAL).exists()
            && !root.join(refactor::REFACTOR_JOURNAL).exists()
        {
            return Ok(None);
        }
        let _lock = common_lock1026(&root)?;
        if media_recovery_pending1026(&root)?{
            // Cold read can expose the true project for its media decision, but
            // cannot apply a Source/refactor journal ahead of that decision.
            if fs::symlink_metadata(root.join(JOURNAL)).is_ok()||fs::symlink_metadata(root.join(refactor::REFACTOR_JOURNAL)).is_ok(){
                return Err("La reprise média doit précéder la récupération Source ; ouvrez le projet depuis sa décision d’accueil.".into());
            }
        }else{recover(&root)?;}
        let Some(sidecar) = read_optional(&root.join(SIDECAR))? else {
            return Ok(None);
        };
        let snapshot: Snapshot =
            serde_json::from_str(&sidecar).map_err(|error| error.to_string())?;
        validate_snapshot_editorial(&snapshot)?;
        let mut loaded_high_water=validated_snapshot_high_water(&snapshot)?.max(floor);
        let expected_recovery = read_optional(&root.join(RECOVERY))?;
        // Invalid/foreign recovery retains the existing explicit conflict path.
        // Only the owned baseline may contribute reserved member identities.
        if let Some(bytes)=expected_recovery.as_deref(){
            if let Ok(pending)=serde_json::from_str::<Recovery>(bytes){
                if validate_recovery_editorial(&pending).is_ok() && pending.source==snapshot.source
                    && pending.baseline_source==snapshot.source_snapshot
                    && pending.baseline_sidecar.as_deref()==Some(sidecar.as_str()) {
                    if let Ok(reserved)=validated_recovery_high_water(&pending,validated_snapshot_high_water(&snapshot)?){loaded_high_water=loaded_high_water.max(reserved);}
                } else if let Ok(Some(reserved))=reservation_only1025::owned_reservation1025(bytes,&pending,&snapshot,&sidecar){
                    // Counter-only sidecar publication can precede cache
                    // archival. Keep the union; do not invent model recovery.
                    loaded_high_water=loaded_high_water.max(reserved);
                }
            }
        }
        let path = source_path(&root, &snapshot.source)?;
        let library_inputs=if scoped_libraries{SourceLibraryInputs1027::read(&root)?.map(std::sync::Arc::new)}else{None};
        let mut project = if scoped_libraries {SourceProject::open_library_at1027(&path,snapshot.source_snapshot.clone(),&snapshot.graphs,library_inputs.clone(),library_inputs,&root,snapshot.imports.clone(),native_preparation)?}else if native_preparation {
            SourceProject::open_native_snapshot(
                &path,
                snapshot.source_snapshot.clone(),
                &snapshot.graphs,
                snapshot.imports.clone(),
            )?
        } else {
            SourceProject::open_snapshot(
                &path,
                snapshot.source_snapshot.clone(),
                &snapshot.graphs,
                snapshot.imports.clone(),
            )?
        };
        let source_error = match fs::read_to_string(&path) {
            Ok(source)
                if source == project.source()
                    && project.imports_snapshot().is_none()
                    && !project.has_imports() =>
            {
                None
            }
            // Imports can change even while the linked file's bytes do not.
            // A failed refresh retains the validated, cached context/graphs.
            Ok(source) => project
                .refresh_with_graph_id_floor(source,loaded_high_water)
                .err()
                .map(|error| format!("{}: {error}", path.display())),
            Err(error) => Some(format!("{}: {error}", path.display())),
        };
        let snapshot_matches_project = loaded_high_water==validated_snapshot_high_water(&snapshot)?
            && snapshot.source_snapshot == project.source()
            && snapshot.graphs == project.graphs()
            && snapshot.imports == project.imports_snapshot()
            && references_are_current(project.graphs())
            && snapshot.graphs.iter().all(|graph| {
                graph.schema_version <= crate::GRAPH_MAX_SUPPORTED_SCHEMA_VERSION
                    && graph.schema_version >= crate::GRAPH_SCHEMA_VERSION
                    && graph.validate_narrative_metadata().is_ok()
            });
        Ok(Some(Self {
            library_origin1027:std::sync::Arc::new(()),
            graph_id_high_water: loaded_high_water.max(project.graphs().iter().map(|g|g.graph_id.get()).max().unwrap_or(0)),
            root,
            relative_source: snapshot.source,
            asset_placements:snapshot.asset_placements.clone(),
            published_asset_placements:snapshot.asset_placements,
            expected_sidecar: Some(sidecar),
            expected_recovery,
            project,
            snapshot_matches_project,
            source_error,
        }))
    }

    /// PRIVATE freeze from actual source bytes; cached presentation is never
    /// an executable authority. Every included file is rechecked after loading.
    pub fn freeze_native_program(&self) -> Result<rvn_parser::ParsedProgram, String> {
        if let Some(error) = &self.source_error {
            return Err(error.clone());
        }
        let _lock = lock(&self.root)?;
        let current = fs::read_to_string(self.source_path()).map_err(|error| error.to_string())?;
        if current != self.project.source() {
            return Err(
                "Native source changed since the writer snapshot; re-open before binding".into(),
            );
        }
        let program = self.project.resolved_program()?;
        if !crate::native_source_binding::contains_native(&program.script)
            && !crate::empty_native_init_0984::program_has_declarations(&program)? {
            return Err("No explicit Native source plan to bind".into());
        }
        crate::native_source_binding::validate_frozen_source_graphs(&program)?;
        // This is still the authorized filesystem capture, under the writer
        // lock. Replay and Native reconstruction never perform this recheck.
        if program.resolution.is_some() { rvn_parser::recheck_file_program_resolution(&program)?; }
        for source in &program.sources {
            let path = source
                .path
                .as_ref()
                .ok_or("Native binding requires each real source path")?;
            if fs::read_to_string(path).map_err(|error| error.to_string())? != source.source {
                return Err("Native included source changed during freezing".into());
            }
        }
        Ok(program)
    }
    pub fn source_path(&self) -> PathBuf {
        self.root.join(&self.relative_source)
    }
    pub fn project(&self) -> &SourceProject {
        &self.project
    }
    /// Hold the existing cooperative writer lock across a synchronous
    /// in-memory admission. Checking files and then releasing this guard
    /// before the caller installs owners would permit a competing Workspace
    /// to publish a different source/recovery inside that installation.
    pub fn with_authoring_admission<T>(
        &self,
        admit: impl FnOnce(&Self) -> Result<T, String>,
    ) -> Result<T, String> {
        let _lock = lock(&self.root)?;
        self.check_recovery_baseline()?;
        self.check_recovery_owner()?;
        admit(self)
    }
    pub fn with_recovery_admission<T>(
        &self,
        admit: impl FnOnce(&[GraphDocument], &SourceRecoveryBatch) -> Result<T, String>,
    ) -> Result<T, String> {
        self.with_authoring_admission(|workspace| {
            let batch = workspace
                .recovery_batch()?
                .ok_or("The owned recovery disappeared before admission")?;
            admit(workspace.project().graphs(), &batch)
        })
    }
    /// Explicit Discard rewrites only the existing recovery, keeping an exact
    /// backup before its atomic replacement. Unrelated unpublished identities
    /// must survive. The synchronous commit contains prevalidated memory moves
    /// only and runs under the SAME cooperative writer lock.
    pub fn discard_recovery_with<T>(&mut self,graphs:&[GraphDocument],widgets:&[WidgetRecovery],commit:impl FnOnce()->T)->Result<T,String>{
        self.discard_recovery_with_memberships(graphs,widgets,&[],commit)
    }
    pub fn discard_recovery_with_memberships<T>(
        &mut self,graphs:&[GraphDocument],widgets:&[WidgetRecovery],memberships:&[WidgetMembershipRecovery],commit:impl FnOnce()->T,
    )->Result<T,String>{
        self.discard_recovery_with_memberships_and_references(graphs,widgets,memberships,&[],commit)
    }
    /// Read-only descriptor/index CAS participants join this SAME recovery
    /// writer lock. Dropping a typed delta may never drop its physical BEFORE
    /// check between the App preparation and the actual archive frontier.
    pub fn discard_recovery_with_memberships_and_references<T>(
        &mut self,graphs:&[GraphDocument],widgets:&[WidgetRecovery],memberships:&[WidgetMembershipRecovery],
        references:&[SourceRefactorFile],commit:impl FnOnce()->T,
    )->Result<T,String>{
        self.replace_recovery_with_enum_changes1027(graphs,widgets,memberships,&[],references,commit)
    }
    /// Existing recovery writer, shared by ordinary dirty content and Discard.
    /// Graphs and current Enum records reach ONE atomic cache replacement.
    /// Authored source, sidecar, descriptors and Enum Saved files stay read-only.
    /// The synchronous callback must contain prevalidated memory moves only.
    pub fn replace_recovery_with_enum_changes1027<T>(
        &mut self,graphs:&[GraphDocument],widgets:&[WidgetRecovery],memberships:&[WidgetMembershipRecovery],
        changes:&[EnumRecoveryChange1025],references:&[SourceRefactorFile],commit:impl FnOnce()->T,
    )->Result<T,String>{
        self.replace_recovery_content_libraries1027(None,graphs,widgets,memberships,changes,&[],&[],&[],references,false,move |_|commit())
    }
    pub fn replace_recovery_with_library_changes1027<T>(
        &mut self,expected:Option<&SourceRecoveryCohortLibrary1027>,graphs:&[GraphDocument],widgets:&[WidgetRecovery],memberships:&[WidgetMembershipRecovery],
        enum_changes:&[EnumRecoveryChange1025],library_changes:&[LibraryRecoveryChange1027],fresh_graph_ids:&[GraphId],identity_plans:&[SourceLibraryMemberIdentity1027],references:&[SourceRefactorFile],commit:impl FnOnce()->T,
    )->Result<T,String>{
        if !self.project.observes_library_inputs1027(){return Err("Library recovery requires the original scoped Source resource observation".into())}
        self.replace_recovery_content_libraries1027(Some(expected),graphs,widgets,memberships,enum_changes,library_changes,fresh_graph_ids,identity_plans,references,false,move |_|commit())
    }
    /// Same guarded writer as the historical wrapper, returning its exact
    /// preallocated after-image. No read of a later on-disk cache is performed.
    pub fn replace_recovery_with_library_receipt1029<T>(
        &mut self,expected:Option<&SourceRecoveryCohortLibrary1027>,graphs:&[GraphDocument],widgets:&[WidgetRecovery],memberships:&[WidgetMembershipRecovery],
        enum_changes:&[EnumRecoveryChange1025],library_changes:&[LibraryRecoveryChange1027],fresh_graph_ids:&[GraphId],identity_plans:&[SourceLibraryMemberIdentity1027],references:&[SourceRefactorFile],commit:impl FnOnce()->T,
    )->Result<(T,SourceLibraryRecoveryPublication1029),String>{
        if !self.project.observes_library_inputs1027(){return Err("Library recovery requires the original scoped Source resource observation".into())}
        self.replace_recovery_content_libraries1027(Some(expected),graphs,widgets,memberships,enum_changes,library_changes,fresh_graph_ids,identity_plans,references,true,move |after|(commit(),after.expect("receipt requested by this exact original writer")))
    }
    fn replace_recovery_content_libraries1027<T>(
        &mut self,expected:Option<Option<&SourceRecoveryCohortLibrary1027>>,graphs:&[GraphDocument],widgets:&[WidgetRecovery],memberships:&[WidgetMembershipRecovery],
        changes:&[EnumRecoveryChange1025],library_changes:&[LibraryRecoveryChange1027],fresh_graph_ids:&[GraphId],identity_plans:&[SourceLibraryMemberIdentity1027],references:&[SourceRefactorFile],receipt_requested1029:bool,commit:impl FnOnce(Option<SourceLibraryRecoveryPublication1029>)->T,
    )->Result<T,String>{
        let _lock = lock(&self.root)?;
        if references.iter().any(|file|file.before!=file.after){return Err("Recovery reference participants must be read-only exact baselines".into());}
        refactor::check_files(&self.root,references,false)?;
        self.check_recovery_baseline()?;
        self.check_recovery_owner()?;
        widget_recovery::validate(
            widgets,
            graphs,
            self.project.graphs(),
            &self.relative_source,
        )?;
        widget_recovery::validate_memberships(memberships,widgets,graphs,self.project.graphs(),&self.relative_source)?;
        refactor::check_files(&self.root,&widget_recovery::membership_baseline_files(memberships),false)?;
        crate::text_owner::validate_text_owner_edit(self.project.graphs(), graphs)?;
        let previous_cohort1027=self.recovery_cohort_libraries1027()?;
        if let Some(expected)=expected{if previous_cohort1027.as_ref()!=expected{return Err("Library mutation lost its original complete recovery cohort".into())}}
        let identities1027=if expected.is_some(){validate_recovery_graph_shapes(graphs)?;library_recovery1027::validate_identity_changes1027(self,previous_cohort1027.as_ref(),graphs,fresh_graph_ids,identity_plans)?}else{Vec::new()};
        let previous_libraries1027=previous_cohort1027.as_ref().map(|c|c.libraries.clone()).unwrap_or_default();
        let next_libraries1027=library_recovery1027::apply_changes1027(&previous_libraries1027,library_changes)?;
        if !next_libraries1027.is_empty(){validate_recovery_graph_shapes(graphs)?;}
        let previous1025=previous_cohort1027.as_ref().map(|c|c.batch.clone());
        let previous_enums1025=previous1025.as_ref().map(|batch|batch.enums.clone()).unwrap_or_default();
        enum_recovery1025::validate_records1025(&previous_enums1025,widgets)?;
        enum_recovery1025::check_baselines1025(&self.root,&previous_enums1025,&[],false)?;
        let next_enums1027=enum_recovery1025::apply_changes1025(&previous_enums1025,changes,true)?;
        enum_recovery1025::validate_records1025(&next_enums1027,widgets)?;
        enum_recovery1025::check_baselines1025(&self.root,&next_enums1027,references,false)?;
        library_recovery1027::check_baselines1027(self,&next_libraries1027,graphs,widgets,memberships,&next_enums1027)?;
        if let Some(previous) = previous1025 {
            widget_recovery::preserve_identities(&previous, graphs, widgets)?;
            widget_recovery::preserve_memberships(&previous,graphs,memberships)?;
            if previous.asset_placements.as_ref().is_some_and(|map|Some(map)!=self.asset_placements.as_ref()){
                return Err("Restore or archive the actual Source asset placement recovery before replacing it".into());
            }
        }
        // Recovery owns the complete Current vector, including its original
        // presentation order. A repeated autosave cannot erase order-only
        // Current by treating the same documents as an unordered bag.
        let clean = graphs == self.project.graphs();
        let persisted_high_water=self.expected_sidecar.as_deref().map(|bytes|serde_json::from_str::<Snapshot>(bytes).map_err(|e|e.to_string()).and_then(|snapshot|validated_snapshot_high_water(&snapshot))).transpose()?.unwrap_or(0);
        let next_placements=self.placements_for_graphs(graphs)?;
        let next = if next_libraries1027.is_empty() && next_enums1027.is_empty() && widgets.is_empty() && memberships.is_empty() && clean && next_placements==self.saved_asset_placements()? && self.graph_id_high_water<=persisted_high_water {
            None
        } else {
            Some(
                serde_json::to_string(&Recovery {
                    libraries:next_libraries1027.clone(),
                    asset_placements_after:self.placements_for_graphs(graphs)?,
                    graph_id_high_water: Some(self.graph_id_high_water().max(previous_cohort1027.as_ref().map(|c|c.high_water1027()).unwrap_or(0)).max(graphs.iter().map(|g|g.graph_id.get()).max().unwrap_or(0))),                    version: if !next_libraries1027.is_empty(){5}else if !next_enums1027.is_empty(){4}else if self.asset_placements.is_some(){3}else if memberships.is_empty(){1}else{2},
                    source: self.relative_source.clone(),
                    baseline_source: self.project.source().into(),
                    baseline_sidecar: self.expected_sidecar.clone(),
                    graphs: graphs.to_vec(),
                    widgets: widgets.to_vec(),
                    widget_memberships: memberships.to_vec(),
                    enums: next_enums1027.clone(),
                })
                .map_err(|error| error.to_string())?,
            )
        };
        // Allocate/validate the complete receipt BEFORE any recovery IO.
        // Its cohort comes from exactly our already-prepared next wire, never
        // from recovery_cohort_libraries1027 after the writer lock is released.
        let publication1029=if receipt_requested1029 {
        let receipt_cohort1029=next.as_ref().map(|wire|->Result<SourceRecoveryCohortLibrary1027,String>{
            let mut record:Recovery=serde_json::from_str(wire).map_err(|error|error.to_string())?;
            let baseline_reserve=if self.expected_sidecar.is_some(){persisted_high_water}else{self.project.graphs().iter().map(|g|g.graph_id.get()).max().unwrap_or(0)};
            let reserved_high1027=validated_recovery_high_water(&record,baseline_reserve)?;
            for graph in &mut record.graphs{graph.normalize_assignment_value_outputs().map_err(|error|error.to_string())?;}
            Ok(SourceRecoveryCohortLibrary1027{libraries:record.libraries,reserved_high1027,
                batch:SourceRecoveryBatch{graphs:record.graphs,widgets:record.widgets,
                    widget_memberships:record.widget_memberships,enums:record.enums,asset_placements:record.asset_placements_after}})
        }).transpose()?;
        let receipt_high1029=if expected.is_some()||!previous_libraries1027.is_empty()||!next_libraries1027.is_empty(){
            self.graph_id_high_water.max(previous_cohort1027.as_ref().map(|c|c.high_water1027()).unwrap_or(0)).max(graphs.iter().map(|g|g.graph_id.get()).max().unwrap_or(0))
        }else{self.graph_id_high_water};
        Some(SourceLibraryRecoveryPublication1029{cohort:receipt_cohort1029,placements:next_placements.clone(),high_water:receipt_high1029})
        }else{None};
        if let Some(contents) = &self.expected_recovery {
            let folder = self.root.join(".rvn-backups").join(unique_name());
            fs::create_dir_all(&folder).map_err(|error| error.to_string())?;
            atomic_write(&folder.join("recovery.json"), contents)?;
        }
        refactor::check_files(&self.root,references,false)?;
        self.check_recovery_baseline()?;
        self.check_recovery_owner()?;
        enum_recovery1025::check_baselines1025(&self.root,&next_enums1027,references,false)?;
        library_recovery1027::check_baselines1027(self,&next_libraries1027,graphs,widgets,memberships,&next_enums1027)?;
        match &next {
            Some(contents) => atomic_write(&self.root.join(RECOVERY), contents)?,
            None if self.expected_recovery.is_some() => {
                fs::remove_file(self.root.join(RECOVERY)).map_err(|error| error.to_string())?
            }
            None => {}
        }
        self.asset_placements=next_placements;
        self.expected_recovery = next;
        if expected.is_some()||!previous_libraries1027.is_empty()||!next_libraries1027.is_empty(){self.graph_id_high_water=self.graph_id_high_water.max(previous_cohort1027.as_ref().map(|c|c.high_water1027()).unwrap_or(0)).max(graphs.iter().map(|g|g.graph_id.get()).max().unwrap_or(0));}
        library_recovery1027::admit_identities1027(&identities1027);
        Ok(commit(publication1029))
    }

    /// Autosave presentation, including incomplete graphs, without touching
    /// authored RVN. A stale cache is retained for explicit conflict resolution.
    pub fn write_recovery(&mut self, graphs: &[GraphDocument]) -> Result<(), String> {
        if let Some(cohort)=self.recovery_cohort_libraries1027()?{let batch=cohort.batch;
            if batch.asset_placements.is_some(){return Err("Source asset placement recovery requires the complete typed publication; graph-only overwrite refused".into());}
            if !batch.widgets.is_empty()||!batch.widget_memberships.is_empty(){return Err("Widget recovery must retain its typed identity participants; legacy graph-only overwrite refused".into());}
        }
        self.write_recovery_with_widgets(graphs, &[])
    }
    pub fn write_recovery_with_widgets(&mut self,graphs:&[GraphDocument],widgets:&[WidgetRecovery])->Result<(),String>{
        self.write_recovery_with_widget_memberships(graphs,widgets,&[])
    }
    pub fn write_recovery_with_widget_memberships(&mut self,graphs:&[GraphDocument],widgets:&[WidgetRecovery],memberships:&[WidgetMembershipRecovery])->Result<(),String>{
        if graphs.iter().any(|g|g.library1027.is_some())||self.project.library_inputs1027().is_some_and(|input|!input.assets().is_empty())||self.recovery_cohort_libraries1027()?.is_some_and(|cohort|!cohort.libraries.is_empty()||cohort.batch.graphs.iter().any(|g|g.library1027.is_some())){
            return self.replace_recovery_with_enum_changes1027(graphs,widgets,memberships,&[],&[],||())
        }
        if self.asset_placements.is_some(){validate_recovery_graph_shapes(graphs)?;}
        widget_recovery::validate(
            widgets,
            graphs,
            self.project.graphs(),
            &self.relative_source,
        )?;
        widget_recovery::validate_memberships(memberships,widgets,graphs,self.project.graphs(),&self.relative_source)?;
        refactor::check_files(&self.root,&widget_recovery::membership_baseline_files(memberships),false)?;
        crate::text_owner::validate_text_owner_edit(self.project.graphs(), graphs)?;
        let _lock = lock(&self.root)?;
        self.check_recovery_baseline()?;
        self.check_recovery_owner()?;
        refactor::check_files(&self.root,&widget_recovery::membership_baseline_files(memberships),false)?;
        let previous1025=self.recovery_batch()?;
        let previous_enums1025=previous1025.as_ref().map(|batch|batch.enums.clone()).unwrap_or_default();
        enum_recovery1025::validate_records1025(&previous_enums1025,widgets)?;
        enum_recovery1025::check_baselines1025(&self.root,&previous_enums1025,&[],false)?;
        if let Some(previous) = previous1025 {
            widget_recovery::preserve_identities(&previous, graphs, widgets)?;
            widget_recovery::preserve_memberships(&previous,graphs,memberships)?;
            if previous.asset_placements.as_ref().is_some_and(|map|Some(map)!=self.asset_placements.as_ref()){
                return Err("Restore or archive the actual Source asset placement recovery before replacing it".into());
            }
        }
        let recovery = Recovery {
            libraries:Vec::new(),
            asset_placements_after:self.placements_for_graphs(graphs)?,
            graph_id_high_water: Some(self.graph_id_high_water().max(graphs.iter().map(|g|g.graph_id.get()).max().unwrap_or(0))),            version: if !previous_enums1025.is_empty(){4}else if self.asset_placements.is_some(){3}else if memberships.is_empty(){1}else{2},
            source: self.relative_source.clone(),
            baseline_source: self.project.source().into(),
            baseline_sidecar: self.expected_sidecar.clone(),
            graphs: graphs.to_vec(),
            widgets: widgets.to_vec(),
            widget_memberships: memberships.to_vec(),
            enums:previous_enums1025,
        };
        let contents = serde_json::to_string(&recovery).map_err(|error| error.to_string())?;
        atomic_write(&self.root.join(RECOVERY), &contents)?;
        self.asset_placements=recovery.asset_placements_after;
        self.expected_recovery = Some(contents);
        Ok(())
    }
    /// Read-only schema/budget preflight for a prepared owner installation.
    /// The caller's with_authoring_admission guard supplies disk stability;
    /// this function neither writes a cache nor creates an authority.
    pub fn validate_recovery_with_widgets(&self,graphs:&[GraphDocument],widgets:&[WidgetRecovery])->Result<(),String>{
        self.validate_recovery_with_widget_memberships(graphs,widgets,&[])
    }
    pub fn validate_recovery_with_widget_memberships(&self,graphs:&[GraphDocument],widgets:&[WidgetRecovery],memberships:&[WidgetMembershipRecovery])->Result<(),String>{
        widget_recovery::validate(
            widgets,
            graphs,
            self.project.graphs(),
            &self.relative_source,
        )?;
        widget_recovery::validate_memberships(memberships,widgets,graphs,self.project.graphs(),&self.relative_source)?;
        refactor::check_files(&self.root,&widget_recovery::membership_baseline_files(memberships),false)?;
        crate::text_owner::validate_text_owner_edit(self.project.graphs(), graphs)?;
        if let Some(cohort) = self.recovery_cohort_libraries1027()? {let previous=cohort.batch;
            library_recovery1027::check_baselines1027(self,&cohort.libraries,graphs,widgets,memberships,&previous.enums)?;
            enum_recovery1025::validate_records1025(&previous.enums,widgets)?;
            enum_recovery1025::check_baselines1025(&self.root,&previous.enums,&[],false)?;
            widget_recovery::preserve_identities(&previous, graphs, widgets)?;
            widget_recovery::preserve_memberships(&previous,graphs,memberships)?;
            if previous.asset_placements.as_ref().is_some_and(|map|Some(map)!=self.asset_placements.as_ref()){
                return Err("Restore or archive the actual Source asset placement recovery before replacing it".into());
            }
        }
        Ok(())
    }

    pub fn recovery(&self) -> Result<Option<Vec<GraphDocument>>, String> {
        self.recovery_batch()?.map(|batch|{
            if batch.asset_placements.is_some(){return Err("Source asset placement recovery may not be flattened into graph-only models".into());}
            if !batch.enums.is_empty(){return Err("Typed Enum recovery must be installed with its actual document owners; graph-only flatten refused".into());}
            if !batch.widgets.is_empty()||!batch.widget_memberships.is_empty(){return Err("Typed Widget recovery must be installed atomically with its UUID owners, never as SourceGraphs".into());}
            Ok(batch.graphs)
        }).transpose()
    }
    pub fn recovery_batch(&self) -> Result<Option<SourceRecoveryBatch>, String> {
        self.recovery_cohort_libraries1027()?.map(|cohort|{if !cohort.libraries.is_empty()||cohort.batch.graphs.iter().any(|g|g.library1027.is_some())||self.project.library_inputs1027().is_some_and(|input|!input.assets().is_empty()){return Err("Library recovery requires its complete typed cohort; legacy flatten refused".into())}Ok(cohort.batch)}).transpose()
    }
    pub fn recovery_cohort_libraries1027(&self) -> Result<Option<SourceRecoveryCohortLibrary1027>, String> {
        let Some(contents) = read_optional(&self.root.join(RECOVERY))? else {
            return Ok(None);
        };
        let mut recovery: Recovery = serde_json::from_str(&contents).map_err(|error| {
            format!("Unreadable recovery; the original cache was retained: {error}")
        })?;
        // Recovery has always required an explicit version. Diagnose unknown
        // versions before participant validation can mistake them for a legacy
        // downgrade or a changed baseline. Known versions keep their exact rules.
        if !matches!(recovery.version, 1..=5) {
            return Err(format!(
                "Unsupported Source recovery version {}; the original cache was retained",
                recovery.version
            ));
        }
        if validate_recovery_editorial(&recovery).is_err()
            || recovery.source != self.relative_source
            || recovery.baseline_source != self.project.source()
            || recovery.baseline_sidecar != self.expected_sidecar
        {
            return Err("Recovery conflicts with a changed source or presentation. The cache was retained; resolve the conflict explicitly.".into());
        }
        let baseline_reserve=match self.expected_sidecar.as_deref(){
            Some(bytes)=>validated_snapshot_high_water(&serde_json::from_str::<Snapshot>(bytes).map_err(|error|error.to_string())?)?,
            None=>self.project.graphs().iter().map(|g|g.graph_id.get()).max().unwrap_or(0),
        };
        let reserved_high1027=validated_recovery_high_water(&recovery,baseline_reserve)?;
        self.check_recovery_baseline()?;
        // Recover incomplete logic, but never corrupt identities/pins/edges.
        crate::text_owner::validate_text_owner_edit(self.project.graphs(), &recovery.graphs)?;
        validate_recovery_graph_shapes(&recovery.graphs)?;
        for graph in &mut recovery.graphs {
            // Old autosaves bypass GraphDocument::from_json. Upgrade only
            // additive assignment outputs in memory, retaining the original
            // recovery file and every authored pin, wire and position.
            graph
                .normalize_assignment_value_outputs()
                .map_err(|error| error.to_string())?;
        }
        widget_recovery::validate(
            &recovery.widgets,
            &recovery.graphs,
            self.project.graphs(),
            &self.relative_source,
        )?;
        widget_recovery::validate_memberships(&recovery.widget_memberships,&recovery.widgets,&recovery.graphs,self.project.graphs(),&self.relative_source)?;
        refactor::check_files(&self.root,&widget_recovery::membership_baseline_files(&recovery.widget_memberships),false)?;
        enum_recovery1025::validate_records1025(&recovery.enums,&recovery.widgets)?;
        enum_recovery1025::check_baselines1025(&self.root,&recovery.enums,&[],false)?;
        library_recovery1027::check_baselines1027(self,&recovery.libraries,&recovery.graphs,&recovery.widgets,&recovery.widget_memberships,&recovery.enums)?;
        Ok(Some(SourceRecoveryCohortLibrary1027 { libraries:recovery.libraries,reserved_high1027,batch:SourceRecoveryBatch {
            graphs: recovery.graphs,
            widgets: recovery.widgets,
            asset_placements:recovery.asset_placements_after,
            widget_memberships: recovery.widget_memberships,
            enums: recovery.enums,
        }}))
    }

    /// Discarding a cache archives it first; it remains manually recoverable.
    pub fn archive_recovery(&mut self) -> Result<(), String> {

        let _lock = lock(&self.root)?;
        let saved_placements=self.saved_asset_placements()?;
        self.archive_recovery_locked(true)?;
        self.asset_placements=saved_placements;
        Ok(())
    }

    fn archive_recovery_locked(&mut self,persist_reservation:bool) -> Result<(), String> {
        if self.recovery_batch()?.is_some_and(|batch|!batch.enums.is_empty()){
            return Err("Typed Enum recovery requires exact targeted removal before archive".into());
        }
        self.check_recovery_owner()?;
        if persist_reservation && self.has_pending_graph_identity_reservation()?{
            return self.persist_reservation_archive_locked1025();
        }
        self.archive_owned_recovery_locked1025()
    }
    // These are the SAME existing transaction/archive sequences, shared only
    // with the strict reservation-only maintenance path under its own lock.
    fn persist_reservation_archive_locked1025(&mut self)->Result<(),String>{
        let transaction=self.prepare_reservation_archive()?;
        atomic_write(&self.root.join(JOURNAL),&serde_json::to_string(&transaction).map_err(|error|error.to_string())?)?;
        recover(&self.root)?;
        self.expected_sidecar=Some(transaction.after_sidecar);
        self.expected_recovery=None;
        Ok(())
    }
    fn archive_owned_recovery_locked1025(&mut self)->Result<(),String>{
        self.check_recovery_owner()?;
        if let Some(contents) = read_optional(&self.root.join(RECOVERY))? {
            let folder = self.root.join(".rvn-backups").join(unique_name());
            fs::create_dir_all(&folder).map_err(|error| error.to_string())?;
            atomic_write(&folder.join("recovery.json"), &contents)?;
            fs::remove_file(self.root.join(RECOVERY)).map_err(|error| error.to_string())?;
        }
        self.expected_recovery = None;
        Ok(())
    }

    /// Exact sidecar-only identity publication. Authored source bytes are a
    /// CAS participant but are never regenerated, including a conflicted source.
    fn prepare_reservation_archive(&self)->Result<Transaction,String>{
        if self.root.join(JOURNAL).exists()||self.root.join(refactor::REFACTOR_JOURNAL).exists(){return Err("Finish the existing source transaction before discarding recovery".into());}
        self.check_recovery_owner()?;
        let before=read_optional(&self.root.join(SIDECAR))?;
        if before!=self.expected_sidecar{return Err("Source presentation changed before reservation archive".into());}
        let original=before.as_deref().ok_or("Reservation archive requires its actual published sidecar")?;
        let mut value:serde_json::Value=serde_json::from_str(original).map_err(|error|error.to_string())?;
        let snapshot:Snapshot=serde_json::from_str(original).map_err(|error|error.to_string())?;
        let high=self.graph_id_high_water().max(validated_snapshot_high_water(&snapshot)?);
        value.as_object_mut().ok_or("Invalid source sidecar")?.insert("graph_id_high_water".into(),serde_json::json!(high));
        let source=fs::read_to_string(self.source_path()).map_err(|error|error.to_string())?;
        Ok(Transaction{library_readset1027:self.library_journal_readset1027(),editorial_publication:None,reservation_archive:Some(ReservationArchive{before_recovery:self.expected_recovery.clone()}),
            source:self.relative_source.clone(),before_source:source.clone(),after_source:source,
            before_sidecar:before,after_sidecar:serde_json::to_string_pretty(&value).map_err(|error|error.to_string())?})
    }

    fn check_recovery_owner(&self) -> Result<(), String> {
        if read_optional(&self.root.join(RECOVERY))? != self.expected_recovery {
            return Err(
                "Another session changed recovery. Its cache was retained; reopen before saving."
                    .into(),
            );
        }
        Ok(())
    }

    fn check_recovery_baseline(&self) -> Result<(), String> {
        self.project.check_imports()?;
        if fs::read_to_string(self.source_path()).map_err(|error| error.to_string())?
            != self.project.source()
            || read_optional(&self.root.join(SIDECAR))? != self.expected_sidecar
        {
            return Err(
                "Source or presentation changed externally. Recovery was not overwritten.".into(),
            );
        }
        Ok(())
    }

    pub fn save(&mut self, graphs: &[GraphDocument]) -> Result<(), String> {
        let _lock = lock(&self.root)?;
        recover(&self.root)?;
        self.save_locked(graphs, &[])
    }

    /// Save source/presentation and explicitly frozen project assets through
    /// one existing recoverable group publisher. This is a Save, not a new
    /// user refactor or an independently editable copy of the source project.
    /// Additional targets retain their exact before bytes and cannot replace
    /// the source, presentation, imported scripts, lock, journals or recovery.
    pub fn save_with_files(
        &mut self,
        graphs: &[GraphDocument],
        additional: &[SourceRefactorFile],
    ) -> Result<(), String> {
        let _lock = lock(&self.root)?;
        recover(&self.root)?;
        self.save_locked(graphs, additional)
    }

    fn save_locked(
        &mut self,
        graphs: &[GraphDocument],
        additional: &[SourceRefactorFile],
    ) -> Result<(), String> {
        if let Some(error) = &self.source_error {
            return Err(error.clone());
        }
        self.check_recovery_owner()?;
        let widget_recovery = if let Some(batch) = self.recovery_batch()? {
            if !batch.enums.is_empty(){return Err("Saving with Enum recovery requires its exact typed publication transition".into());}
            widget_recovery::publication_participants(&batch, additional)?;
            if batch.asset_placements.as_ref().is_some_and(|placements|Some(placements)!=self.asset_placements.as_ref()){return Err("Restore the actual Source asset placement recovery before saving".into());}
            widget_recovery::validate(
                &batch.widgets,
                graphs,
                self.project.graphs(),
                &self.relative_source,
            )?;
            widget_recovery::validate_memberships(&batch.widget_memberships,&batch.widgets,graphs,self.project.graphs(),&self.relative_source)?;
            !batch.widgets.is_empty() || !batch.widget_memberships.is_empty() || batch.asset_placements.is_some() || self.has_pending_graph_identity_reservation()?
        } else {
            false
        };
        let path = source_path(&self.root, &self.relative_source)?;
        let current = fs::read_to_string(&path).map_err(|error| error.to_string())?;
        let sidecar = read_optional(&self.root.join(SIDECAR))?;
        if sidecar != self.expected_sidecar {
            return Err("Blueprint presentation was changed externally. Reopen or resolve the conflict; no files were overwritten.".into());
        }
        if additional.is_empty()
            && self.snapshot_matches_project
            && current == self.project.source()
            && graphs == self.project.graphs()
            && !self.has_pending_asset_placements()?
        {
            // A clean explicit Save does not need to compile/reimport every
            // graph again. The baseline was fully checked on open/save, and
            // all disk/recovery ownership checks remain under the save lock.
            self.check_save_inputs(&path, &current, &sidecar)?;
            self.archive_recovery_locked(false)?;
            return Ok(());
        }
        let mut next = self.project.clone();
        next.apply_visual(&current, graphs)?;
        let snapshot = Snapshot {
            asset_placements:self.placements_for_graphs(next.graphs())?,
            graph_id_high_water: Some(self.graph_id_high_water().max(next.graphs().iter().map(|g|g.graph_id.get()).max().unwrap_or(0))),
            version:if self.asset_placements.is_some(){2}else{1},
            source: self.relative_source.clone(),
            source_snapshot: next.source().into(),
            graphs: next.graphs().to_vec(),
            imports: next.imports_snapshot(),
        };
        let after_sidecar =
            serde_json::to_string_pretty(&snapshot).map_err(|error| error.to_string())?;
        let transaction = Transaction {
            library_readset1027:self.library_journal_readset1027(),
            reservation_archive:None,editorial_publication:None,
            source: self.relative_source.clone(),
            before_source: current.clone(),
            after_source: next.source().into(),
            before_sidecar: sidecar.clone(),
            after_sidecar: after_sidecar.clone(),
        };
        let mut group = Vec::new();
        if !additional.is_empty() || widget_recovery {
            group.push(SourceRefactorFile {
                path: self.relative_source.clone(),
                before: Some(current.clone()),
                after: Some(next.source().into()),
            });
            group.push(SourceRefactorFile {
                path: SIDECAR.into(),
                before: sidecar.clone(),
                after: Some(after_sidecar.clone()),
            });
            self.append_publication_files(&mut group, additional)?;
            refactor::check_files(&self.root, &group, false)?;
            let staged=next.stage_library_files1027(&self.root,&path,&group)?;
            if staged.source()!=next.source()||staged.graphs()!=next.graphs(){return Err("Library resource changes require a coherent Source/graph candidate".into())}
            next=staged;
        }
        let migrating_presentation = sidecar
            .as_deref()
            .map(|contents| {
                serde_json::from_str::<Snapshot>(contents)
                    .map(|snapshot| {
                        snapshot
                            .graphs
                            .iter()
                            .any(|graph| graph.schema_version < crate::GRAPH_SCHEMA_VERSION)
                    })
                    .map_err(|error| error.to_string())
            })
            .transpose()?
            .unwrap_or(false);
        if current != next.source() || sidecar.is_none() || migrating_presentation {
            let backup = self.root.join(".rvn-backups").join(unique_name());
            fs::create_dir_all(&backup).map_err(|error| error.to_string())?;
            atomic_write(&backup.join("source.rvn"), &current)?;
            if let Some(sidecar) = &sidecar {
                atomic_write(&backup.join("presentation.json"), sidecar)?;
            }
        }
        self.check_save_inputs(&path, &current, &sidecar)?;
        next.check_imports()?;
        // Successful explicit save supersedes autosaved edits, but archives
        // rather than deletes them. If archiving fails, the cache stays intact.
        // A new Widget's UUID definition stays in the active recovery until
        // the SAME durable group journal has successfully published all files.
        // Legacy saves retain their exact historical archive timing.
        if !widget_recovery {
            self.archive_recovery_locked(false)?;
        }
        if additional.is_empty() && !widget_recovery {
            atomic_write(
                &self.root.join(JOURNAL),
                &serde_json::to_string(&transaction).map_err(|error| error.to_string())?,
            )?;
            recover(&self.root)?;
        } else {
            let result = if widget_recovery {
                self.publish_refactor_retaining_recovery(
                    next.clone(),
                    &group,
                    self.expected_recovery.clone(),
                )
            } else {
                self.publish_refactor(next.clone(), &group)
            };
            // An archive/journal-cleanup error may occur after publication.
            // Preserve exact worker ownership metadata even on normal errors.
            if widget_recovery && read_optional(&self.root.join(RECOVERY))?.is_none() {
                self.expected_recovery = None;
            }
            result?;
        }
        self.expected_sidecar = Some(after_sidecar);
        self.snapshot_matches_project = next.graphs().iter().all(|graph| {
            graph.schema_version <= crate::GRAPH_MAX_SUPPORTED_SCHEMA_VERSION
                && graph.schema_version >= crate::GRAPH_SCHEMA_VERSION
                && graph.validate_narrative_metadata().is_ok()
        });
        self.graph_id_high_water=self.graph_id_high_water.max(snapshot.graph_id_high_water.unwrap_or(0));
        self.published_asset_placements=snapshot.asset_placements.clone();
        self.asset_placements=snapshot.asset_placements;
        next.commit_library_inputs1027();
        self.project = next;
        Ok(())
    }

    fn check_save_inputs(
        &self,
        path: &Path,
        current: &str,
        sidecar: &Option<String>,
    ) -> Result<(), String> {
        self.project.check_imports()?;
        if fs::read_to_string(path).map_err(|error| error.to_string())? != current
            || read_optional(&self.root.join(SIDECAR))? != *sidecar
        {
            return Err("Source or presentation changed while preparing the save. Resolve the conflict before retrying.".into());
        }
        Ok(())
    }
}

fn references_are_current(graphs: &[GraphDocument]) -> bool {
    let mut resolved = graphs.to_vec();
    // Eligibility is only an optimization: it must never add a new Open
    // failure. A noncanonical/unresolvable cache takes the full Save path.
    crate::resolve_label_references(&mut resolved).is_ok() && resolved == graphs
}

fn common_lock1026(root: &Path) -> Result<fs::File, String> {
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(root.join(".rvn-authoring.lock"))
        .map_err(|error| error.to_string())?;
    file.try_lock()
        .map_err(|error| format!("Another authoring save is active: {error}"))?;
    Ok(file)
}

// E media recovery is a pending filesystem transaction, not a Source owner.
// Every existing authoring mutator still uses the same real lock function.
fn media_recovery_pending1026(root:&Path)->Result<bool,String>{
    match fs::symlink_metadata(root.join(".rvn-media-transaction.lock")){
        Ok(_)=>Ok(true),Err(error)if error.kind()==std::io::ErrorKind::NotFound=>Ok(false),Err(error)=>Err(error.to_string()),
    }
}
fn lock(root:&Path)->Result<fs::File,String>{
    let file=common_lock1026(root)?;
    if media_recovery_pending1026(root)?{return Err("Une réimportation média attend sa décision de reprise avant toute écriture Source.".into());}
    Ok(file)
}
fn source_path(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let path = Path::new(relative);
    if relative.is_empty()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_) | Component::CurDir))
        || path.extension().and_then(|extension| extension.to_str()) != Some("rvn")
    {
        return Err("Unsafe source path in authoring metadata".into());
    }
    let path = root.join(path);
    let canonical = match fs::canonicalize(&path) {
        Ok(path) => path,
        // A deleted source still has a valid cached graph. Preserve it and
        // let the read report the missing file, without recreating authored
        // source. Broken symlinks are not treated as absent ordinary files.
        Err(error)
            if error.kind() == std::io::ErrorKind::NotFound
                && fs::symlink_metadata(&path)
                    .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound) =>
        {
            let parent = fs::canonicalize(path.parent().ok_or("Missing source parent")?)
                .map_err(|error| error.to_string())?;
            parent.join(path.file_name().ok_or("Missing source filename")?)
        }
        Err(error) => return Err(error.to_string()),
    };
    if !canonical.starts_with(root) {
        return Err("Linked source escapes the project folder".into());
    }
    Ok(canonical)
}

fn read_optional(path: &Path) -> Result<Option<String>, String> {
    match fs::read_to_string(path) {
        Ok(source) => Ok(Some(source)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

fn recover(root: &Path) -> Result<(), String> {
    refactor::recover_refactor(root)?;
    let Some(journal) = read_optional(&root.join(JOURNAL))? else {
        return Ok(());
    };
    let transaction: Transaction = serde_json::from_str(&journal)
        .map_err(|error| format!("Incomplete save journal: {error}"))?;
    if let Some(readset)=&transaction.library_readset1027{readset.validate(root)?;}
    let source = source_path(root, &transaction.source)?;
    let current = fs::read_to_string(&source).map_err(|error| error.to_string())?;
    let sidecar = read_optional(&root.join(SIDECAR))?;
    if (current != transaction.before_source && current != transaction.after_source)
        || (sidecar != transaction.before_sidecar
            && sidecar.as_deref() != Some(&transaction.after_sidecar))
    {
        return Err("An interrupted save conflicts with external edits. The journal and backups were retained; resolve the conflict explicitly.".into());
    }
    if let Some(publication)=&transaction.editorial_publication{return recover_asset_placement_publication(root,&transaction,publication);}
    if let Some(archive)=&transaction.reservation_archive{
        let before=transaction.before_sidecar.as_deref().ok_or("Reservation archive lost its before-sidecar")?;
        let old:Snapshot=serde_json::from_str(before).map_err(|error|error.to_string())?;
        let next:Snapshot=serde_json::from_str(&transaction.after_sidecar).map_err(|error|error.to_string())?;
        let mut old_fields:serde_json::Value=serde_json::from_str(before).map_err(|error|error.to_string())?;
        let mut next_fields:serde_json::Value=serde_json::from_str(&transaction.after_sidecar).map_err(|error|error.to_string())?;
        old_fields.as_object_mut().ok_or("Invalid before-sidecar")?.remove("graph_id_high_water");
        next_fields.as_object_mut().ok_or("Invalid after-sidecar")?.remove("graph_id_high_water");
        let old_high=validated_snapshot_high_water(&old)?;let next_high=validated_snapshot_high_water(&next)?;
        if transaction.before_source!=transaction.after_source||old_fields!=next_fields||validate_snapshot_editorial(&old).is_err()
            ||old.source!=transaction.source||next.graph_id_high_water.is_none()||next_high<old_high{
            return Err("Reservation archive may only advance the actual sidecar high-water".into());
        }
        if let Some(contents)=archive.before_recovery.as_deref(){
            if let Ok(owned)=serde_json::from_str::<Recovery>(contents){
                if !owned.enums.is_empty(){return Err("Legacy reservation archive cannot discard typed Enum recovery".into());}
                if validate_recovery_editorial(&owned).is_ok()&&owned.source==old.source&&owned.baseline_source==old.source_snapshot
                    &&owned.baseline_sidecar.as_deref()==Some(before)
                    &&validated_recovery_high_water(&owned,old_high).is_ok_and(|high|high>next_high){
                    return Err("Reservation archive would drop an owned committed identity".into());
                }
            }
        }
        let current_recovery=read_optional(&root.join(RECOVERY))?;
        if current_recovery!=archive.before_recovery&&current_recovery.is_some(){return Err("Reservation archive Recovery CAS changed; journal retained".into());}
        if archive.before_recovery.is_some()&&current_recovery.is_none()&&sidecar.as_deref()!=Some(&transaction.after_sidecar){
            return Err("Recovery disappeared before the reservation sidecar was published".into());
        }
        // Backup is part of this same existing source transaction before any
        // publication. A crash resumes only exact before/after participants.
        let backup=root.join(".rvn-backups").join(unique_name());fs::create_dir_all(&backup).map_err(|error|error.to_string())?;
        atomic_write(&backup.join("presentation.json"),before)?;
        if let Some(contents)=&current_recovery{atomic_write(&backup.join("recovery.json"),contents)?;}
        if fs::read_to_string(&source).map_err(|error|error.to_string())?!=transaction.before_source
            ||read_optional(&root.join(SIDECAR))?!=sidecar||read_optional(&root.join(RECOVERY))?!=current_recovery{
            return Err("Source reservation archive inputs changed before publication".into());
        }
        if sidecar.as_deref()!=Some(&transaction.after_sidecar){atomic_write(&root.join(SIDECAR),&transaction.after_sidecar)?;}
        if read_optional(&root.join(RECOVERY))?!=current_recovery{return Err("Recovery changed after reservation publication; journal retained".into());}
        if current_recovery.is_some(){fs::remove_file(root.join(RECOVERY)).map_err(|error|error.to_string())?;}
        fs::remove_file(root.join(JOURNAL)).map_err(|error|error.to_string())?;return Ok(());
    }
    let snapshot: Snapshot =
        serde_json::from_str(&transaction.after_sidecar).map_err(|error| error.to_string())?;
    if validate_snapshot_editorial(&snapshot).is_err()
        || snapshot.source != transaction.source
        || snapshot.source_snapshot != transaction.after_source
    {
        return Err("Inconsistent save journal; files were not overwritten".into());
    }
    if let Some(before)=transaction.before_sidecar.as_deref(){
        let old:Snapshot=serde_json::from_str(before).map_err(|error|error.to_string())?;
        validate_snapshot_editorial(&old)?;
        if old.asset_placements.is_some()&&snapshot.asset_placements.is_none(){return Err("Source journal may not downgrade adopted asset placement metadata".into());}
    }
    let project=if let Some(readset)=&transaction.library_readset1027{validate_published_snapshot_library1027(root,&snapshot,readset)?}else{validate_published_snapshot_project(root,&snapshot)?};
    project.check_imports()?;
    if let Some(readset)=&transaction.library_readset1027{readset.validate(root)?;}
    if current != transaction.after_source {
        atomic_write(&source, &transaction.after_source)?;
    }
    if sidecar.as_deref() != Some(&transaction.after_sidecar) {
        atomic_write(&root.join(SIDECAR), &transaction.after_sidecar)?;
    }
    fs::remove_file(root.join(JOURNAL)).map_err(|error| error.to_string())?;
    Ok(())
}

fn unique_name() -> String {
    format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    )
}

fn atomic_write(path: &Path, source: &str) -> Result<(), String> {
    let parent = path.parent().ok_or("Missing output folder")?;
    let temporary = parent.join(format!(".rvn-write-{}.tmp", unique_name()));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|error| error.to_string())?;
    let result = (|| {
        file.write_all(source.as_bytes())
            .and_then(|_| file.sync_all())
            .map_err(|error| error.to_string())?;
        fs::rename(&temporary, path).map_err(|error| error.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    #[cfg(unix)]
    if result.is_ok() {
        fs::File::open(parent)
            .and_then(|file| file.sync_all())
            .map_err(|error| error.to_string())?;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GraphKind, NodeKind, PropertyValue};

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("rvn-source-storage-{}", unique_name()));
            fs::create_dir(&path).unwrap();
            fs::write(path.join("story.rvn"), "// keep this note\nfunction score(n) { return n + 2 }\nlabel start\n\"[score(3)]\"\nreturn\n").unwrap();
            Self(path)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let Ok(path) = self.0.canonicalize() else {
                return;
            };
            let Ok(temporary) = std::env::temp_dir().canonicalize() else {
                return;
            };
            if path.parent() == Some(temporary.as_path())
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("rvn-source-storage-"))
            {
                let _ = fs::remove_dir_all(path);
            }
        }
    }

    #[test]
    fn grouped_asset_save_publishes_source_presentation_and_unicode_assets_then_reopens() {
        let (fixture, path) = imported_fixture();
        let mut workspace = SourceWorkspace::link(&fixture.0, &path, &[]).unwrap();
        let graphs = imported_edits(&workspace);
        fs::create_dir_all(fixture.0.join("assets/tables")).unwrap();
        let a = "assets/tables/a.rvnstrings";
        let b = "assets/tables/b.rvnstrings";
        fs::write(fixture.0.join(a), "avant A\n").unwrap();
        fs::write(fixture.0.join(b), "avant B\n").unwrap();
        let files = [
            SourceRefactorFile {
                path: a.into(),
                before: Some("avant A\n".into()),
                after: Some("Bonjour — 雨 {Name}\n".into()),
            },
            SourceRefactorFile {
                path: b.into(),
                before: Some("avant B\n".into()),
                after: Some("À bientôt\n".into()),
            },
        ];
        workspace.save_with_files(&graphs, &files).unwrap();
        for file in &files {
            assert_eq!(
                fs::read_to_string(fixture.0.join(&file.path)).unwrap(),
                file.after.as_ref().unwrap().as_str()
            );
        }
        let reopened = SourceWorkspace::open(&fixture.0)
            .unwrap()
            .expect("linked saved source project");
        assert_eq!(reopened.project().graphs(), workspace.project().graphs());
        assert_eq!(reopened.project().source(), workspace.project().source());
        assert_eq!(
            fs::read_to_string(fixture.0.join("scripts/chapter.rvn")).unwrap(),
            IMPORTED_CHAPTER
        );
        assert!(!fixture.0.join(JOURNAL).exists());
        assert!(!fixture.0.join(refactor::REFACTOR_JOURNAL).exists());
    }

    #[test]
    fn grouped_asset_save_conflict_retains_all_files_and_models_and_can_retry() {
        let (fixture, path) = imported_fixture();
        let mut workspace = SourceWorkspace::link(&fixture.0, &path, &[]).unwrap();
        let graphs = imported_edits(&workspace);
        let before_source = fs::read(&path).unwrap();
        let before_sidecar = fs::read(fixture.0.join(SIDECAR)).unwrap();
        let before_models = workspace.project().graphs().to_vec();
        fs::write(fixture.0.join("a.rvnstrings"), "old A").unwrap();
        fs::write(fixture.0.join("b.rvnstrings"), "external B").unwrap();
        let files = [
            SourceRefactorFile {
                path: "a.rvnstrings".into(),
                before: Some("old A".into()),
                after: Some("new A".into()),
            },
            SourceRefactorFile {
                path: "b.rvnstrings".into(),
                before: Some("old B".into()),
                after: Some("new B".into()),
            },
        ];
        assert!(workspace
            .save_with_files(&graphs, &files)
            .unwrap_err()
            .contains("conflict"));
        assert_eq!(fs::read(&path).unwrap(), before_source);
        assert_eq!(fs::read(fixture.0.join(SIDECAR)).unwrap(), before_sidecar);
        assert_eq!(workspace.project().graphs(), before_models);
        assert_eq!(
            fs::read_to_string(fixture.0.join("a.rvnstrings")).unwrap(),
            "old A"
        );
        assert_eq!(
            fs::read_to_string(fixture.0.join("b.rvnstrings")).unwrap(),
            "external B"
        );
        fs::write(fixture.0.join("b.rvnstrings"), "old B").unwrap();
        workspace.save_with_files(&graphs, &files).unwrap();
        assert_eq!(
            fs::read_to_string(fixture.0.join("a.rvnstrings")).unwrap(),
            "new A"
        );
        assert_ne!(fs::read(&path).unwrap(), before_source);
    }

    #[test]
    fn grouped_asset_save_rejects_imports_reserved_and_duplicate_targets_before_writing() {
        let (fixture, path) = imported_fixture();
        let mut workspace = SourceWorkspace::link(&fixture.0, &path, &[]).unwrap();
        let graphs = imported_edits(&workspace);
        let source = fs::read(&path).unwrap();
        let sidecar = fs::read(fixture.0.join(SIDECAR)).unwrap();
        let lock_bytes = fs::read(fixture.0.join(".rvn-authoring.lock")).unwrap();
        for target in [
            "scripts/chapter.rvn",
            SIDECAR,
            JOURNAL,
            RECOVERY,
            refactor::REFACTOR_JOURNAL,
            ".rvn-authoring.lock",
            "../escape.rvnstrings",
        ] {
            let file = SourceRefactorFile {
                path: target.into(),
                before: None,
                after: Some("replacement".into()),
            };
            assert!(workspace.save_with_files(&graphs, &[file]).is_err());
        }
        if cfg!(windows) {
            for target in [
                ".RVN-AUTHORING.LOCK",
                ".rvn-authoring.lock.",
                ".RVN-AUTHORING.TRANSACTION.JSON",
                ".RVN-REFACTOR.TRANSACTION.JSON",
                ".RVN-AUTHORING.RECOVERY.JSON",
            ] {
                let before = fs::read_to_string(fixture.0.join(target)).ok();
                let file = SourceRefactorFile {
                    path: target.into(),
                    before,
                    after: Some("replacement".into()),
                };
                assert!(
                    workspace.save_with_files(&graphs, &[file]).is_err(),
                    "reserved alias {target}"
                );
            }
        }
        fs::write(fixture.0.join("one.rvnstrings"), "old").unwrap();
        let file = SourceRefactorFile {
            path: "one.rvnstrings".into(),
            before: Some("old".into()),
            after: Some("new".into()),
        };
        assert!(workspace
            .save_with_files(&graphs, &[file.clone(), file])
            .unwrap_err()
            .contains("more than once"));
        assert_eq!(fs::read(&path).unwrap(), source);
        assert_eq!(fs::read(fixture.0.join(SIDECAR)).unwrap(), sidecar);
        assert_eq!(
            fs::read(fixture.0.join(".rvn-authoring.lock")).unwrap(),
            lock_bytes
        );
        assert_eq!(
            fs::read_to_string(fixture.0.join("scripts/chapter.rvn")).unwrap(),
            IMPORTED_CHAPTER
        );
        assert_eq!(
            fs::read_to_string(fixture.0.join("one.rvnstrings")).unwrap(),
            "old"
        );
    }

    const IMPORTED_MAIN: &str = "// main stays authored\nuse \"chapter.rvn\"\nfunction local_value(n) { return helper(n) }\nlabel start\n music.volume(\"0.2\")\n \"[local_value(3)]\"\n ui.open(\"hud\", [], false, 1)\n jump chapter\n";
    const IMPORTED_CHAPTER: &str = "// exact imported CRLF and Unicode — 雨\r\nfunction helper(n) { return n + 2 }\r\nscreen hud() { return {\"id\":\"root\",\"kind\":\"text\",\"text\":\"Imported\"} }\r\nhandler clicked(event) { return true }\r\nlabel chapter\r\n\"Imported chapter\"\r\nreturn\r\n";

    fn imported_fixture() -> (Fixture, PathBuf) {
        let fixture = Fixture::new();
        fs::create_dir(fixture.0.join("scripts")).unwrap();
        let path = fixture.0.join("scripts/main.rvn");
        fs::write(&path, IMPORTED_MAIN).unwrap();
        fs::write(fixture.0.join("scripts/chapter.rvn"), IMPORTED_CHAPTER).unwrap();
        // An identically named root file must never supply the import's scope.
        fs::write(
            fixture.0.join("chapter.rvn"),
            "label wrong_origin\nreturn\n",
        )
        .unwrap();
        (fixture, path)
    }

    fn imported_edits(workspace: &SourceWorkspace) -> Vec<GraphDocument> {
        let mut graphs = workspace.project().graphs().to_vec();
        let graph = graphs
            .iter_mut()
            .find(|graph| matches!(&graph.kind, GraphKind::Label { name } if name == "start"))
            .unwrap();
        let volume = graph
            .nodes
            .values()
            .find(|node| node.kind == NodeKind::MusicVolume)
            .unwrap()
            .id;
        let pin = graph.pin_by_key(volume, "level").unwrap().id;
        let producer = graph.pins[&graph
            .edges
            .values()
            .find(|edge| edge.input == pin)
            .unwrap()
            .output]
            .node;
        graph
            .set_property(producer, "value", PropertyValue::Float(0.4))
            .unwrap();
        graph.nodes.get_mut(&volume).unwrap().position = [430.0, 210.0];
        graphs
    }

    #[test]
    fn linked_imports_validate_the_real_project_without_writing_or_projecting_imported_scopes() {
        let (fixture, path) = imported_fixture();
        let mut workspace = SourceWorkspace::link(&fixture.0, &path, &[]).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), IMPORTED_MAIN);
        assert!(workspace
            .project()
            .graphs()
            .iter()
            .all(|graph| match &graph.kind {
                GraphKind::Init => true,
                GraphKind::Label { name } => name == "start",
                GraphKind::Function { name } => name == "local_value",
                _ => false,
            }));
        let compiled = workspace.project().resolved_script().unwrap();
        assert_eq!(compiled, rvn_parser::parse_file_with_uses(&path).unwrap());
        assert!(compiled.iter().any(|statement| matches!(statement, rvn_parser::Statement::Screen { name, .. } if name == "hud")));
        assert!(compiled.iter().any(|statement| matches!(statement, rvn_parser::Statement::Handler { name, .. } if name == "clicked")));
        let files = workspace.project().resolved_source_files().unwrap();
        assert_eq!(files.len(), 2);
        assert!(files.iter().any(|(file, source)| file
            == &fixture
                .0
                .join("scripts/chapter.rvn")
                .canonicalize()
                .unwrap()
            && source == IMPORTED_CHAPTER));
        let graphs = imported_edits(&workspace);
        workspace.save(&graphs).unwrap();
        assert!(workspace
            .project()
            .source()
            .contains("// main stays authored"));
        assert!(workspace.project().source().contains("use \"chapter.rvn\""));
        assert!(workspace.project().resolved_script().unwrap().iter().any(|statement| matches!(statement, rvn_parser::Statement::MusicVolume { level } if *level == 0.4)));
        assert_eq!(
            fs::read_to_string(fixture.0.join("scripts/chapter.rvn")).unwrap(),
            IMPORTED_CHAPTER
        );
        assert_eq!(
            fs::read_to_string(fixture.0.join("chapter.rvn")).unwrap(),
            "label wrong_origin\nreturn\n"
        );
        let backups: Vec<_> = fs::read_dir(fixture.0.join(".rvn-backups"))
            .unwrap()
            .filter_map(Result::ok)
            .collect();
        assert!(backups
            .iter()
            .any(|entry| fs::read_to_string(entry.path().join("source.rvn"))
                .ok()
                .as_deref()
                == Some(IMPORTED_MAIN)));
        assert!(backups
            .iter()
            .all(|entry| !entry.path().join("chapter.rvn").exists()));
        // Reopening exercises the same source projection with prior graph
        // identities and layout, including the cached import snapshot.
        let reopened = SourceWorkspace::open(&fixture.0).unwrap().unwrap();
        assert!(reopened.source_error.is_none());
        assert_eq!(reopened.project().graphs(), workspace.project().graphs());
        assert_eq!(
            reopened.project().resolved_script().unwrap(),
            rvn_parser::parse_file_with_uses(&path).unwrap()
        );
        assert!(reopened.project().source().contains("use \"chapter.rvn\""));
        assert_eq!(
            fs::read_to_string(fixture.0.join("scripts/chapter.rvn")).unwrap(),
            IMPORTED_CHAPTER
        );
    }

    #[test]
    fn real_imports_never_relax_unknown_references_at_link_or_save() {
        let (fixture, path) = imported_fixture();
        let invalid = IMPORTED_MAIN.replace("jump chapter", "jump absent");
        fs::write(&path, &invalid).unwrap();
        assert!(SourceWorkspace::link(&fixture.0, &path, &[])
            .unwrap_err()
            .contains("absent"));
        assert!(!fixture.0.join(SIDECAR).exists());
        assert_eq!(fs::read_to_string(&path).unwrap(), invalid);
        let invalid = IMPORTED_MAIN.replace("helper(n)", "missing_function(n)");
        assert!(SourceProject::open_at(&path, invalid, &[])
            .unwrap_err()
            .contains("missing_function"));
        fs::write(&path, IMPORTED_MAIN).unwrap();
        let mut workspace = SourceWorkspace::link(&fixture.0, &path, &[]).unwrap();
        let original = workspace.project().graphs().to_vec();
        let mut invalid = original.clone();
        let graph = invalid
            .iter_mut()
            .find(|graph| matches!(&graph.kind, GraphKind::Label { name } if name == "start"))
            .unwrap();
        let target = graph
            .nodes
            .values()
            .find(|node| {
                node.kind == NodeKind::LabelValue
                    && node.properties.get("label")
                        == Some(&PropertyValue::String("chapter".into()))
            })
            .unwrap()
            .id;
        graph
            .set_property(target, "label", PropertyValue::String("absent".into()))
            .unwrap();
        workspace.write_recovery(&invalid).unwrap();
        let sidecar = fs::read(fixture.0.join(SIDECAR)).unwrap();
        let recovery = fs::read(fixture.0.join(RECOVERY)).unwrap();
        assert!(workspace.save(&invalid).unwrap_err().contains("absent"));
        assert_eq!(workspace.project().graphs(), original);
        assert_eq!(fs::read_to_string(&path).unwrap(), IMPORTED_MAIN);
        assert_eq!(fs::read(fixture.0.join(SIDECAR)).unwrap(), sidecar);
        assert_eq!(fs::read(fixture.0.join(RECOVERY)).unwrap(), recovery);
        assert_eq!(
            fs::read_to_string(fixture.0.join("scripts/chapter.rvn")).unwrap(),
            IMPORTED_CHAPTER
        );
    }

    #[test]
    fn a_use_after_a_label_refuses_visual_projection_and_preserves_last_valid_cache_and_recovery() {
        let (fixture, path) = imported_fixture();
        let mut workspace = SourceWorkspace::link(&fixture.0, &path, &[]).unwrap();
        let edited = imported_edits(&workspace);
        workspace.write_recovery(&edited).unwrap();
        let graphs = workspace.project().graphs().to_vec();
        let sidecar = fs::read(fixture.0.join(SIDECAR)).unwrap();
        let recovery = fs::read(fixture.0.join(RECOVERY)).unwrap();
        let interleaved = IMPORTED_MAIN
            .replacen("use \"chapter.rvn\"\n", "", 1)
            .replacen("label start\n", "label start\nuse \"chapter.rvn\"\n", 1);
        fs::write(&path, &interleaved).unwrap();
        // This is a legal runtime import. Only visual ownership is unsafe:
        // the imported label would own the following narrative statements.
        assert!(rvn_parser::parse_file_with_uses(&path).is_ok());
        assert!(SourceProject::open_at(&path, &interleaved, &[])
            .unwrap_err()
            .contains("before the first narrative label"));
        assert!(workspace
            .project
            .refresh(&interleaved)
            .unwrap_err()
            .contains("before the first narrative label"));
        assert_eq!(workspace.project().source(), IMPORTED_MAIN);
        assert_eq!(workspace.project().graphs(), graphs);
        let mut reopened = SourceWorkspace::open(&fixture.0).unwrap().unwrap();
        assert!(reopened
            .source_error
            .as_deref()
            .unwrap()
            .contains("before the first narrative label"));
        assert_eq!(reopened.project().source(), IMPORTED_MAIN);
        assert_eq!(reopened.project().graphs(), graphs);
        assert!(reopened.recovery().is_err());
        assert!(reopened.save(&edited).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), interleaved);
        assert_eq!(fs::read(fixture.0.join(SIDECAR)).unwrap(), sidecar);
        assert_eq!(fs::read(fixture.0.join(RECOVERY)).unwrap(), recovery);
        assert_eq!(
            fs::read_to_string(fixture.0.join("scripts/chapter.rvn")).unwrap(),
            IMPORTED_CHAPTER
        );
    }

    #[test]
    fn changed_or_missing_import_keeps_recovery_and_the_last_valid_portable_snapshot() {
        let (fixture, path) = imported_fixture();
        let mut workspace = SourceWorkspace::link(&fixture.0, &path, &[]).unwrap();
        let graphs = imported_edits(&workspace);
        workspace.write_recovery(&graphs).unwrap();
        let sidecar = fs::read(fixture.0.join(SIDECAR)).unwrap();
        let recovery = fs::read(fixture.0.join(RECOVERY)).unwrap();
        let external = format!("{IMPORTED_CHAPTER}// externally edited\n");
        fs::write(fixture.0.join("scripts/chapter.rvn"), &external).unwrap();
        assert!(workspace
            .save(&graphs)
            .unwrap_err()
            .contains("changed externally"));
        assert!(workspace.project().resolved_script().is_err());
        assert_eq!(
            fs::read_to_string(fixture.0.join("scripts/chapter.rvn")).unwrap(),
            external
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), IMPORTED_MAIN);
        assert_eq!(fs::read(fixture.0.join(SIDECAR)).unwrap(), sidecar);
        assert_eq!(fs::read(fixture.0.join(RECOVERY)).unwrap(), recovery);
        fs::remove_file(fixture.0.join("scripts/chapter.rvn")).unwrap();
        let reopened = SourceWorkspace::open(&fixture.0).unwrap().unwrap();
        assert!(reopened.source_error.is_some());
        assert_eq!(reopened.project().graphs(), workspace.project().graphs());
        assert_eq!(fs::read(fixture.0.join(RECOVERY)).unwrap(), recovery);
        fs::write(fixture.0.join("scripts/chapter.rvn"), IMPORTED_CHAPTER).unwrap();

        let moved = Fixture::new();
        fs::create_dir(moved.0.join("scripts")).unwrap();
        fs::copy(&path, moved.0.join("scripts/main.rvn")).unwrap();
        fs::copy(
            fixture.0.join("scripts/chapter.rvn"),
            moved.0.join("scripts/chapter.rvn"),
        )
        .unwrap();
        fs::copy(fixture.0.join(SIDECAR), moved.0.join(SIDECAR)).unwrap();
        let portable = SourceWorkspace::open(&moved.0).unwrap().unwrap();
        assert!(portable.source_error.is_none());
        assert_eq!(portable.project().graphs(), workspace.project().graphs());
        assert_eq!(
            portable.project().resolved_script().unwrap(),
            rvn_parser::parse_file_with_uses(moved.0.join("scripts/main.rvn")).unwrap()
        );
    }

    #[test]
    fn wildcards_nested_deduplicated_imports_and_cycles_match_runtime_resolution() {
        let fixture = Fixture::new();
        fs::create_dir(fixture.0.join("chapters")).unwrap();
        fs::write(
            fixture.0.join("chapters/a.rvn"),
            "use \"../helper.rvn\"\nlabel a\nreturn\n",
        )
        .unwrap();
        fs::write(
            fixture.0.join("chapters/b.rvn"),
            "use \"../helper.rvn\"\nlabel b\nreturn\n",
        )
        .unwrap();
        fs::write(
            fixture.0.join("helper.rvn"),
            "function helper(n) { return n + 1 }\n",
        )
        .unwrap();
        let path = fixture.0.join("story.rvn");
        let main =
            "use { \"chapters/*.rvn\", \"helper.rvn\" }\nlabel start\n\"[helper(3)]\"\njump a\n";
        fs::write(&path, main).unwrap();
        let mut workspace = SourceWorkspace::link(&fixture.0, &path, &[]).unwrap();
        assert_eq!(
            workspace.project().resolved_script().unwrap(),
            rvn_parser::parse_file_with_uses(&path).unwrap()
        );
        assert_eq!(
            workspace.project().resolved_source_files().unwrap().len(),
            4
        );
        let graphs = workspace.project().graphs().to_vec();
        fs::write(fixture.0.join("chapters/c.rvn"), "label c\nreturn\n").unwrap();
        assert!(workspace
            .save(&graphs)
            .unwrap_err()
            .contains("changed externally"));
        let reopened = SourceWorkspace::open(&fixture.0).unwrap().unwrap();
        assert!(reopened.source_error.is_none());
        assert!(reopened.project().resolved_script().unwrap().iter().any(
            |statement| matches!(statement, rvn_parser::Statement::Label { name } if name == "c")
        ));
        fs::write(
            fixture.0.join("helper.rvn"),
            "use \"story.rvn\"\nfunction helper(n) { return n + 1 }\n",
        )
        .unwrap();
        assert!(SourceProject::open_at(&path, main, &[])
            .unwrap_err()
            .contains("cycle"));
        assert!(rvn_parser::parse_file_with_uses(&path).is_err());
        assert!(SourceWorkspace::open(&fixture.0)
            .unwrap()
            .unwrap()
            .source_error
            .is_some());
    }

    #[test]
    fn interrupted_save_checks_import_bytes_before_publishing_main_or_presentation() {
        let (fixture, path) = imported_fixture();
        let workspace = SourceWorkspace::link(&fixture.0, &path, &[]).unwrap();
        let before_sidecar = fs::read_to_string(fixture.0.join(SIDECAR)).unwrap();
        let mut next = workspace.project().clone();
        next.apply_visual(IMPORTED_MAIN, &imported_edits(&workspace))
            .unwrap();
        let after_sidecar = serde_json::to_string(&Snapshot {
            asset_placements:None,
            graph_id_high_water: None,
            version: 1,
            source: "scripts/main.rvn".into(),
            source_snapshot: next.source().into(),
            graphs: next.graphs().to_vec(),
            imports: next.imports_snapshot(),
        })
        .unwrap();
        let transaction = Transaction {
            library_readset1027:None,
            reservation_archive:None,editorial_publication:None,
            source: "scripts/main.rvn".into(),
            before_source: IMPORTED_MAIN.into(),
            after_source: next.source().into(),
            before_sidecar: Some(before_sidecar.clone()),
            after_sidecar,
        };
        let journal = serde_json::to_string(&transaction).unwrap();
        fs::write(fixture.0.join(JOURNAL), &journal).unwrap();
        let external = format!("{IMPORTED_CHAPTER}// third party\n");
        fs::write(fixture.0.join("scripts/chapter.rvn"), &external).unwrap();
        assert!(SourceWorkspace::open(&fixture.0)
            .unwrap_err()
            .contains("changed externally"));
        assert_eq!(fs::read_to_string(&path).unwrap(), IMPORTED_MAIN);
        assert_eq!(
            fs::read_to_string(fixture.0.join(SIDECAR)).unwrap(),
            before_sidecar
        );
        assert_eq!(
            fs::read_to_string(fixture.0.join(JOURNAL)).unwrap(),
            journal
        );
        assert_eq!(
            fs::read_to_string(fixture.0.join("scripts/chapter.rvn")).unwrap(),
            external
        );
        fs::write(fixture.0.join("scripts/chapter.rvn"), IMPORTED_CHAPTER).unwrap();
        let recovered = SourceWorkspace::open(&fixture.0).unwrap().unwrap();
        assert_eq!(recovered.project().source(), next.source());
        assert!(!fixture.0.join(JOURNAL).exists());
        assert_eq!(
            fs::read_to_string(fixture.0.join("scripts/chapter.rvn")).unwrap(),
            IMPORTED_CHAPTER
        );
    }

    #[test]
    fn refactor_and_replay_keep_imports_read_only_and_compare_their_exact_baseline() {
        let (fixture, path) = imported_fixture();
        let mut workspace = SourceWorkspace::link(&fixture.0, &path, &[]).unwrap();
        let original = workspace.project().graphs().to_vec();
        let graphs = imported_edits(&workspace);
        let extra = SourceRefactorFile {
            path: "scripts/chapter.rvn".into(),
            before: Some(IMPORTED_CHAPTER.into()),
            after: Some("label chapter\nreturn\n".into()),
        };
        assert!(workspace
            .refactor(&graphs, &[extra])
            .unwrap_err()
            .contains("read-only"));
        assert_eq!(fs::read_to_string(&path).unwrap(), IMPORTED_MAIN);
        assert_eq!(workspace.project().graphs(), original);
        let receipt = workspace.refactor(&graphs, &[]).unwrap();
        assert_eq!(
            workspace.project().resolved_script().unwrap(),
            rvn_parser::parse_file_with_uses(&path).unwrap()
        );
        workspace.replay_refactor(&receipt, false).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), IMPORTED_MAIN);
        let sidecar = fs::read(fixture.0.join(SIDECAR)).unwrap();
        let external = format!("{IMPORTED_CHAPTER}// external edit\n");
        fs::write(fixture.0.join("scripts/chapter.rvn"), &external).unwrap();
        assert!(workspace
            .replay_refactor(&receipt, true)
            .unwrap_err()
            .contains("changed externally"));
        assert_eq!(fs::read_to_string(&path).unwrap(), IMPORTED_MAIN);
        assert_eq!(fs::read(fixture.0.join(SIDECAR)).unwrap(), sidecar);
        assert_eq!(
            fs::read_to_string(fixture.0.join("scripts/chapter.rvn")).unwrap(),
            external
        );
    }

    #[test]
    fn use_path_resolution_keeps_the_same_filesystem_semantics_as_the_runtime() {
        let fixture = Fixture::new();
        fs::write(fixture.0.join("helper.rvn"), "label imported\nreturn\n").unwrap();
        let path = fixture.0.join("story.rvn");
        for raw in ["not_existing/../helper.rvn", "not_existing/../*.rvn"] {
            let source = format!("use \"{raw}\"\nlabel start\njump imported\n");
            fs::write(&path, &source).unwrap();
            assert_eq!(
                SourceProject::open_at(&path, &source, &[]).is_ok(),
                rvn_parser::parse_file_with_uses(&path).is_ok(),
                "do not erase a missing intermediate directory: {raw}"
            );
        }
        fs::create_dir(fixture.0.join("empty")).unwrap();
        let source = "use \"empty/../helper.rvn\"\nlabel start\njump imported\n";
        fs::write(&path, source).unwrap();
        let project = SourceProject::open_at(&path, source, &[]).unwrap();
        assert_eq!(
            project.resolved_script().unwrap(),
            rvn_parser::parse_file_with_uses(&path).unwrap()
        );
    }

    fn edited(workspace: &SourceWorkspace) -> Vec<GraphDocument> {
        let mut graphs = workspace.project().graphs().to_vec();
        let function = graphs
            .iter_mut()
            .find(|graph| matches!(graph.kind, GraphKind::Function { .. }))
            .unwrap();
        let id = function
            .nodes
            .values()
            .find(|node| {
                node.kind == NodeKind::Literal
                    && node.properties.get("value") == Some(&PropertyValue::Int(2))
            })
            .unwrap()
            .id;
        function
            .set_property(id, "value", PropertyValue::Int(7))
            .unwrap();
        function.nodes.get_mut(&id).unwrap().position = [9000.0, -1000.0];
        graphs
    }

    #[test]
    fn matching_source_snapshot_still_requires_valid_source() {
        let fixture = Fixture::new();
        SourceWorkspace::link(&fixture.0, &fixture.0.join("story.rvn"), &[]).unwrap();
        let mut snapshot: Snapshot =
            serde_json::from_str(&fs::read_to_string(fixture.0.join(SIDECAR)).unwrap()).unwrap();
        snapshot.source_snapshot = "function invalid(".into();
        fs::write(fixture.0.join("story.rvn"), &snapshot.source_snapshot).unwrap();
        let sidecar = serde_json::to_string(&snapshot).unwrap();
        fs::write(fixture.0.join(SIDECAR), &sidecar).unwrap();
        assert!(SourceWorkspace::open(&fixture.0).is_err());
        assert_eq!(
            fs::read_to_string(fixture.0.join("story.rvn")).unwrap(),
            snapshot.source_snapshot
        );
        assert_eq!(
            fs::read_to_string(fixture.0.join(SIDECAR)).unwrap(),
            sidecar
        );
    }

    #[test]
    fn changed_valid_source_reimports_logic_without_replacing_presentation() {
        let fixture = Fixture::new();
        SourceWorkspace::link(&fixture.0, &fixture.0.join("story.rvn"), &[]).unwrap();
        let sidecar = fs::read(fixture.0.join(SIDECAR)).unwrap();
        let source = fs::read_to_string(fixture.0.join("story.rvn"))
            .unwrap()
            .replace("n + 2", "n + 7");
        fs::write(fixture.0.join("story.rvn"), &source).unwrap();
        let reopened = SourceWorkspace::open(&fixture.0).unwrap().unwrap();
        assert!(reopened.source_error.is_none());
        assert_eq!(reopened.project().source(), source);
        let function = reopened
            .project()
            .graphs()
            .iter()
            .find(|graph| matches!(graph.kind, GraphKind::Function { .. }))
            .unwrap();
        assert!(function
            .nodes
            .values()
            .any(|node| node.kind == NodeKind::Literal
                && node.properties.get("value") == Some(&PropertyValue::Int(7))));
        assert_eq!(fs::read(fixture.0.join(SIDECAR)).unwrap(), sidecar);
        assert_eq!(
            fs::read_to_string(fixture.0.join("story.rvn")).unwrap(),
            source
        );
    }

    #[test]
    fn unchanged_source_retains_layout_and_external_conflict_checks() {
        let fixture = Fixture::new();
        let mut workspace =
            SourceWorkspace::link(&fixture.0, &fixture.0.join("story.rvn"), &[]).unwrap();
        let mut graphs = workspace.project().graphs().to_vec();
        for graph in &mut graphs {
            for node in graph.nodes.values_mut() {
                node.position = [9876.0, -1234.0];
            }
        }
        workspace.save(&graphs).unwrap();
        let sidecar = fs::read(fixture.0.join(SIDECAR)).unwrap();
        let mut reopened = SourceWorkspace::open(&fixture.0).unwrap().unwrap();
        assert_eq!(reopened.project().graphs(), graphs);
        assert_eq!(fs::read(fixture.0.join(SIDECAR)).unwrap(), sidecar);
        let external = format!("{}// external change\n", reopened.project().source());
        fs::write(fixture.0.join("story.rvn"), &external).unwrap();
        assert!(reopened.save(&graphs).is_err());
        assert_eq!(
            fs::read_to_string(fixture.0.join("story.rvn")).unwrap(),
            external
        );
        assert_eq!(fs::read(fixture.0.join(SIDECAR)).unwrap(), sidecar);
    }

    #[test]
    fn clean_save_keeps_validated_source_presentation_and_mtimes() {
        let fixture = Fixture::new();
        SourceWorkspace::link(&fixture.0, &fixture.0.join("story.rvn"), &[]).unwrap();
        let mut workspace = SourceWorkspace::open(&fixture.0).unwrap().unwrap();
        assert!(workspace.snapshot_matches_project);
        let paths = [
            fixture.0.join("story.rvn"),
            fixture.0.join(SIDECAR),
            fixture.0.clone(),
        ];
        let modified = paths
            .each_ref()
            .map(|path| fs::metadata(path).unwrap().modified().unwrap());
        let source = fs::read(&paths[0]).unwrap();
        let sidecar = fs::read(&paths[1]).unwrap();
        workspace
            .save(&workspace.project().graphs().to_vec())
            .unwrap();
        assert_eq!(fs::read(&paths[0]).unwrap(), source);
        assert_eq!(fs::read(&paths[1]).unwrap(), sidecar);
        for (path, time) in paths.iter().zip(modified) {
            assert_eq!(fs::metadata(path).unwrap().modified().unwrap(), time);
        }
        assert!(!fixture.0.join(JOURNAL).exists());
    }

    #[test]
    fn clean_save_still_refuses_external_source_presentation_and_recovery_changes() {
        for target in ["story.rvn", SIDECAR, RECOVERY] {
            let fixture = Fixture::new();
            SourceWorkspace::link(&fixture.0, &fixture.0.join("story.rvn"), &[]).unwrap();
            let mut workspace = SourceWorkspace::open(&fixture.0).unwrap().unwrap();
            assert!(workspace.snapshot_matches_project);
            let external = match target {
                "story.rvn" => format!(
                    "{}// external whitespace-only change\n",
                    workspace.project().source()
                ),
                SIDECAR => format!("{}\n", fs::read_to_string(fixture.0.join(SIDECAR)).unwrap()),
                _ => "another session's recovery".into(),
            };
            fs::write(fixture.0.join(target), &external).unwrap();
            let source = fs::read(fixture.0.join("story.rvn")).unwrap();
            let sidecar = fs::read(fixture.0.join(SIDECAR)).unwrap();
            assert!(workspace
                .save(&workspace.project().graphs().to_vec())
                .is_err());
            assert_eq!(
                fs::read_to_string(fixture.0.join(target)).unwrap(),
                external
            );
            assert_eq!(fs::read(fixture.0.join("story.rvn")).unwrap(), source);
            assert_eq!(fs::read(fixture.0.join(SIDECAR)).unwrap(), sidecar);
            assert!(!fixture.0.join(JOURNAL).exists());
        }
    }

    #[test]
    fn clean_save_archives_owned_recovery_without_rewriting_baseline() {
        let fixture = Fixture::new();
        let mut workspace =
            SourceWorkspace::link(&fixture.0, &fixture.0.join("story.rvn"), &[]).unwrap();
        let source = fs::read(fixture.0.join("story.rvn")).unwrap();
        let sidecar = fs::read(fixture.0.join(SIDECAR)).unwrap();
        let source_time = fs::metadata(fixture.0.join("story.rvn"))
            .unwrap()
            .modified()
            .unwrap();
        let sidecar_time = fs::metadata(fixture.0.join(SIDECAR))
            .unwrap()
            .modified()
            .unwrap();
        workspace.write_recovery(&edited(&workspace)).unwrap();
        let recovery = fs::read(fixture.0.join(RECOVERY)).unwrap();
        workspace
            .save(&workspace.project().graphs().to_vec())
            .unwrap();
        assert!(workspace.expected_recovery.is_none());
        assert!(!fixture.0.join(RECOVERY).exists());
        assert!(fs::read_dir(fixture.0.join(".rvn-backups"))
            .unwrap()
            .any(
                |entry| fs::read(entry.unwrap().path().join("recovery.json"))
                    .is_ok_and(|bytes| bytes == recovery)
            ));
        assert_eq!(fs::read(fixture.0.join("story.rvn")).unwrap(), source);
        assert_eq!(fs::read(fixture.0.join(SIDECAR)).unwrap(), sidecar);
        assert_eq!(
            fs::metadata(fixture.0.join("story.rvn"))
                .unwrap()
                .modified()
                .unwrap(),
            source_time
        );
        assert_eq!(
            fs::metadata(fixture.0.join(SIDECAR))
                .unwrap()
                .modified()
                .unwrap(),
            sidecar_time
        );
    }

    #[test]
    fn clean_save_refreshes_externally_changed_whitespace_snapshot() {
        let fixture = Fixture::new();
        let linked = SourceWorkspace::link(&fixture.0, &fixture.0.join("story.rvn"), &[]).unwrap();
        let original_graphs = linked.project().graphs().to_vec();
        let external = format!(
            "{}// external whitespace-only change\n",
            linked.project().source()
        );
        fs::write(fixture.0.join("story.rvn"), &external).unwrap();
        let mut workspace = SourceWorkspace::open(&fixture.0).unwrap().unwrap();
        assert_eq!(workspace.project().graphs(), original_graphs);
        assert!(!workspace.snapshot_matches_project);
        workspace.save(&original_graphs).unwrap();
        let snapshot: Snapshot =
            serde_json::from_str(&fs::read_to_string(fixture.0.join(SIDECAR)).unwrap()).unwrap();
        assert_eq!(snapshot.source_snapshot, external);
        assert_eq!(
            fs::read_to_string(fixture.0.join("story.rvn")).unwrap(),
            external
        );
        assert!(workspace.snapshot_matches_project);
    }

    #[test]
    fn clean_save_publishes_pending_project_reference_reconciliation() {
        let fixture = Fixture::new();
        SourceWorkspace::link(&fixture.0, &fixture.0.join("story.rvn"), &[]).unwrap();
        let mut snapshot: Snapshot =
            serde_json::from_str(&fs::read_to_string(fixture.0.join(SIDECAR)).unwrap()).unwrap();
        let graph = snapshot
            .graphs
            .iter_mut()
            .find(|graph| matches!(&graph.kind,GraphKind::Label{name} if name=="start"))
            .unwrap();
        let target = serde_json::to_string(&graph.graph_id).unwrap();
        let node = graph.add_node(NodeKind::LabelValue, [123.0, 456.0]);
        graph
            .nodes
            .get_mut(&node)
            .unwrap()
            .properties
            .insert("target_graph".into(), PropertyValue::String(target));
        graph
            .nodes
            .get_mut(&node)
            .unwrap()
            .properties
            .insert("label".into(), PropertyValue::String("stale_cache".into()));
        fs::write(
            fixture.0.join(SIDECAR),
            serde_json::to_string_pretty(&snapshot).unwrap(),
        )
        .unwrap();
        let mut workspace = SourceWorkspace::open(&fixture.0).unwrap().unwrap();
        assert!(!workspace.snapshot_matches_project);
        let graphs = workspace.project().graphs().to_vec();
        workspace.save(&graphs).unwrap();
        let resolved = workspace
            .project()
            .graphs()
            .iter()
            .find(|graph| matches!(&graph.kind,GraphKind::Label{name} if name=="start"))
            .unwrap();
        assert_eq!(
            resolved.nodes[&node].properties.get("label"),
            Some(&PropertyValue::String("start".into()))
        );
        assert!(workspace.snapshot_matches_project);
    }

    #[test]
    fn unresolved_reference_is_not_an_optimization_eligibility_error() {
        let fixture = Fixture::new();
        let linked = SourceWorkspace::link(&fixture.0, &fixture.0.join("story.rvn"), &[]).unwrap();
        let mut graphs = linked.project().graphs().to_vec();
        let graph = graphs
            .iter_mut()
            .find(|graph| matches!(graph.kind, GraphKind::Label { .. }))
            .unwrap();
        let node = graph.add_node(NodeKind::LabelValue, [123.0, 456.0]);
        graph.nodes.get_mut(&node).unwrap().properties.insert(
            "target_graph".into(),
            PropertyValue::String("999999".into()),
        );
        let before = graphs.clone();
        assert!(!references_are_current(&graphs));
        assert_eq!(graphs, before);
        // Full validation/Save still reports the original reference error.
        let mut project = linked.project().clone();
        assert!(project
            .apply_visual(linked.project().source(), &graphs)
            .is_err());
        assert_eq!(project.graphs(), linked.project().graphs());
    }

    #[test]
    fn linked_save_keeps_legacy_files_comments_backups_and_layout() {
        let fixture = Fixture::new();
        fs::write(fixture.0.join("old.rvngraph"), "legacy file, never replace").unwrap();
        let mut workspace =
            SourceWorkspace::link(&fixture.0, &fixture.0.join("story.rvn"), &[]).unwrap();
        let graphs = edited(&workspace);
        workspace.save(&graphs).unwrap();
        assert!(fs::read_to_string(fixture.0.join("story.rvn"))
            .unwrap()
            .starts_with("// keep this note"));
        assert_eq!(
            fs::read_to_string(fixture.0.join("old.rvngraph")).unwrap(),
            "legacy file, never replace"
        );
        assert_eq!(
            SourceWorkspace::open(&fixture.0)
                .unwrap()
                .unwrap()
                .project()
                .graphs(),
            graphs
        );
        assert_eq!(
            fs::read_dir(fixture.0.join(".rvn-backups"))
                .unwrap()
                .count(),
            2
        );
        assert!(!fixture.0.join(JOURNAL).exists());
    }

    #[test]
    fn presentation_only_migration_backs_up_exact_old_sidecar_before_explicit_save() {
        let fixture = Fixture::new();
        let linked = SourceWorkspace::link(&fixture.0, &fixture.0.join("story.rvn"), &[]).unwrap();
        let source = fs::read_to_string(fixture.0.join("story.rvn")).unwrap();
        let mut snapshot = Snapshot {
            asset_placements:None,
            graph_id_high_water: None,
            version: 1,
            source: "story.rvn".into(),
            source_snapshot: source.clone(),
            graphs: linked.project().graphs().to_vec(),
            imports: linked.project().imports_snapshot(),
        };
        for graph in &mut snapshot.graphs {
            graph.schema_version = 2;
        }
        let old = serde_json::to_string_pretty(&snapshot).unwrap();
        fs::write(fixture.0.join(SIDECAR), &old).unwrap();
        let mut reopened = SourceWorkspace::open(&fixture.0).unwrap().unwrap();
        assert!(reopened
            .project()
            .graphs()
            .iter()
            .all(|graph| graph.schema_version == crate::GRAPH_SCHEMA_VERSION));
        assert!(!reopened.snapshot_matches_project);
        // Opening never overwrites the original presentation.
        assert_eq!(fs::read_to_string(fixture.0.join(SIDECAR)).unwrap(), old);
        let graphs = reopened.project().graphs().to_vec();
        reopened.save(&graphs).unwrap();
        assert_eq!(
            fs::read_to_string(fixture.0.join("story.rvn")).unwrap(),
            source
        );
        let backups: Vec<_> = fs::read_dir(fixture.0.join(".rvn-backups"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        let backup = backups
            .iter()
            .find(|path| {
                fs::read_to_string(path.join("presentation.json"))
                    .is_ok_and(|contents| contents == old)
            })
            .unwrap();
        assert_eq!(
            fs::read_to_string(backup.join("source.rvn")).unwrap(),
            source
        );
        assert_eq!(
            SourceWorkspace::open(&fixture.0)
                .unwrap()
                .unwrap()
                .project()
                .graphs(),
            graphs
        );
    }

    #[test]
    fn stale_session_and_external_source_conflicts_do_not_overwrite_files() {
        let fixture = Fixture::new();
        let mut first =
            SourceWorkspace::link(&fixture.0, &fixture.0.join("story.rvn"), &[]).unwrap();
        let mut stale = SourceWorkspace::open(&fixture.0).unwrap().unwrap();
        first.save(&edited(&first)).unwrap();
        let saved = fs::read_to_string(fixture.0.join("story.rvn")).unwrap();
        assert!(stale.save(&edited(&stale)).is_err());
        assert_eq!(
            fs::read_to_string(fixture.0.join("story.rvn")).unwrap(),
            saved
        );
        fs::write(
            fixture.0.join("story.rvn"),
            format!("{saved}// external edit\n"),
        )
        .unwrap();
        assert!(first
            .save(first.project().graphs().to_vec().as_slice())
            .is_err());
        assert!(fs::read_to_string(fixture.0.join("story.rvn"))
            .unwrap()
            .ends_with("// external edit\n"));
    }

    #[test]
    fn invalid_source_keeps_last_valid_graphs_and_requires_explicit_repair() {
        let fixture = Fixture::new();
        let workspace =
            SourceWorkspace::link(&fixture.0, &fixture.0.join("story.rvn"), &[]).unwrap();
        let original = workspace.project().graphs().to_vec();
        fs::write(fixture.0.join("story.rvn"), "function invalid(").unwrap();
        let mut reopened = SourceWorkspace::open(&fixture.0).unwrap().unwrap();
        assert!(reopened.source_error.is_some());
        assert_eq!(reopened.project().graphs(), original);
        assert!(reopened.save(&original).is_err());
        assert_eq!(
            fs::read_to_string(fixture.0.join("story.rvn")).unwrap(),
            "function invalid("
        );
    }

    #[test]
    fn missing_source_keeps_last_valid_graphs_without_recreating_the_file() {
        let fixture = Fixture::new();
        let workspace =
            SourceWorkspace::link(&fixture.0, &fixture.0.join("story.rvn"), &[]).unwrap();
        let original = workspace.project().graphs().to_vec();
        fs::remove_file(fixture.0.join("story.rvn")).unwrap();
        let mut reopened = SourceWorkspace::open(&fixture.0).unwrap().unwrap();
        assert!(reopened.source_error.is_some());
        assert_eq!(reopened.project().graphs(), original);
        assert!(reopened.save(&original).is_err());
        assert!(!fixture.0.join("story.rvn").exists());
    }

    #[cfg(unix)]
    #[test]
    fn absent_source_cannot_escape_through_a_symlinked_parent() {
        let fixture = Fixture::new();
        let outside = Fixture::new();
        std::os::unix::fs::symlink(&outside.0, fixture.0.join("outside")).unwrap();
        assert!(source_path(&fixture.0, "outside/absent.rvn").is_err());
        std::os::unix::fs::symlink(outside.0.join("absent.rvn"), fixture.0.join("broken.rvn"))
            .unwrap();
        assert!(source_path(&fixture.0, "broken.rvn").is_err());
    }

    #[test]
    fn autosave_recovers_unsaved_logic_and_layout_without_writing_source() {
        let fixture = Fixture::new();
        let mut workspace =
            SourceWorkspace::link(&fixture.0, &fixture.0.join("story.rvn"), &[]).unwrap();
        let before = workspace.project().source().to_string();
        let mut graphs = edited(&workspace);
        // Incomplete input is intentionally recoverable, not compilable.
        let graph = &mut graphs[0];
        graph.add_node(NodeKind::Dialogue, [123.0, 456.0]);
        workspace.write_recovery(&graphs).unwrap();
        assert_eq!(
            fs::read_to_string(fixture.0.join("story.rvn")).unwrap(),
            before
        );
        let mut reopened = SourceWorkspace::open(&fixture.0).unwrap().unwrap();
        assert_eq!(reopened.recovery().unwrap().unwrap(), graphs);
        reopened.archive_recovery().unwrap();
        assert!(reopened.recovery().unwrap().is_none());
        assert!(fs::read_dir(fixture.0.join(".rvn-backups"))
            .unwrap()
            .any(|folder| folder.unwrap().path().join("recovery.json").is_file()));
    }

    #[test]
    fn conflicting_autosave_and_external_edits_never_replace_a_recovery_cache() {
        let fixture = Fixture::new();
        let mut first =
            SourceWorkspace::link(&fixture.0, &fixture.0.join("story.rvn"), &[]).unwrap();
        let mut second = SourceWorkspace::open(&fixture.0).unwrap().unwrap();
        first.write_recovery(&edited(&first)).unwrap();
        let cache = fs::read_to_string(fixture.0.join(RECOVERY)).unwrap();
        assert!(second.write_recovery(&edited(&second)).is_err());
        assert!(second.archive_recovery().is_err());
        fs::write(
            fixture.0.join("story.rvn"),
            "label start\n\"external edit\"\n",
        )
        .unwrap();
        let mut reopened = SourceWorkspace::open(&fixture.0).unwrap().unwrap();
        assert!(reopened.recovery().is_err());
        assert!(reopened.write_recovery(&edited(&first)).is_err());
        assert_eq!(fs::read_to_string(fixture.0.join(RECOVERY)).unwrap(), cache);
    }

    #[test]
    fn interrupted_transaction_finishes_without_accepting_third_party_changes() {
        for conflicting in [false, true] {
            let fixture = Fixture::new();
            let workspace =
                SourceWorkspace::link(&fixture.0, &fixture.0.join("story.rvn"), &[]).unwrap();
            let before_source = workspace.project().source().to_string();
            let mut next = workspace.project().clone();
            next.apply_visual(&before_source, &edited(&workspace))
                .unwrap();
            let after_sidecar = serde_json::to_string(&Snapshot {
            asset_placements:None,
            graph_id_high_water: None,
                version: 1,
                source: "story.rvn".into(),
                source_snapshot: next.source().into(),
                graphs: next.graphs().to_vec(),
                imports: next.imports_snapshot(),
            })
            .unwrap();
            let transaction = Transaction {
            library_readset1027:None,
            reservation_archive:None,editorial_publication:None,
                source: "story.rvn".into(),
                before_source,
                after_source: next.source().into(),
                before_sidecar: workspace.expected_sidecar.clone(),
                after_sidecar,
            };
            fs::write(
                fixture.0.join(JOURNAL),
                serde_json::to_string(&transaction).unwrap(),
            )
            .unwrap();
            fs::write(
                fixture.0.join("story.rvn"),
                if conflicting {
                    "label changed\n\"external\"\n"
                } else {
                    next.source()
                },
            )
            .unwrap();
            if conflicting {
                assert!(SourceWorkspace::open(&fixture.0).is_err());
                assert!(fixture.0.join(JOURNAL).exists());
                assert_eq!(
                    fs::read_to_string(fixture.0.join("story.rvn")).unwrap(),
                    "label changed\n\"external\"\n"
                );
            } else {
                let recovered = SourceWorkspace::open(&fixture.0).unwrap().unwrap();
                assert_eq!(recovered.project().source(), next.source());
                assert_eq!(recovered.project().graphs(), next.graphs());
                assert!(!fixture.0.join(JOURNAL).exists());
            }
        }
    }
}

#[cfg(test)]
#[path = "source_widget_recovery_tests.rs"]
mod widget_recovery_tests;

#[cfg(test)]
#[path = "source_widget_batch_failure_tests_0938.rs"]
mod widget_batch_failure_tests_0938;

impl SourceWorkspace {
    /// Identity reservation belongs to the existing source sidecar, including
    /// identities removed by a later Undo. Preparation itself reserves nothing.
    pub fn graph_id_high_water(&self) -> u64 { self.graph_id_high_water.max(self.project.graphs().iter().map(|g|g.graph_id.get()).max().unwrap_or(0)) }
    pub fn has_pending_graph_identity_reservation(&self)->Result<bool,String>{
        let persisted=match self.expected_sidecar.as_deref(){
            Some(bytes)=>validated_snapshot_high_water(&serde_json::from_str::<Snapshot>(bytes).map_err(|error|error.to_string())?)?,
            None=>0,
        };
        Ok(self.graph_id_high_water()>persisted)
    }
    pub fn next_member_graph_id(&self, graphs:&[GraphDocument]) -> Result<crate::GraphId,String> {
        self.graph_id_high_water().max(graphs.iter().map(|g|g.graph_id.get()).max().unwrap_or(0))
            .checked_add(1).map(crate::GraphId::new).ok_or_else(||"Les identités source sont épuisées.".into())
    }
    /// Admit fresh IDs and actual model publication under the existing writer
    /// lock. The callback must finish its guarded model commit before success;
    /// a refusal consumes no identity. No reservation is published by prepare.
    pub fn with_graph_creation_admission<T>(&mut self, prepared:&[crate::GraphId],
        commit:impl FnOnce(&Self)->Result<T,String>)->Result<T,String>{
        let _lock=lock(&self.root)?;
        self.check_recovery_baseline()?;
        self.check_recovery_owner()?;
        let mut previous=self.graph_id_high_water();
        for (index,id) in prepared.iter().enumerate(){
            if id.get()<=previous || (index>0 && previous.checked_add(1)!=Some(id.get())){
                return Err("Prepared graph identities are reserved, unordered or noncontiguous".into());
            }
            previous=id.get();
        }
        if prepared.is_empty(){return Err("A graph creation requires its actual fresh identities".into());}
        let result=commit(self)?;
        self.graph_id_high_water=previous;
        self.snapshot_matches_project=false;
        Ok(result)
    }
    pub fn commit_member_graph_id(&mut self, prepared:crate::GraphId) -> Result<(),String> {
        self.with_graph_creation_admission(&[prepared], |_|Ok(()))
    }

}

#[cfg(test)]
#[path="source_member_identity_tests1003.rs"]
mod source_member_identity_tests1003;

fn validated_snapshot_high_water(snapshot:&Snapshot)->Result<u64,String>{
    validate_snapshot_editorial(snapshot)?;
    let used=snapshot.graphs.iter().map(|g|g.graph_id.get()).max().unwrap_or(0);
    if snapshot.graph_id_high_water.is_some_and(|reserved|reserved<used){return Err("La réserve d’identités source est inférieure à un membre sauvegardé.".into());}
    Ok(snapshot.graph_id_high_water.unwrap_or(used))
}

// Only an absent field uses the legacy default. Explicit null is not an ID.
fn strict_graph_id_high_water<'de,D:serde::Deserializer<'de>>(deserializer:D)->Result<Option<u64>,D::Error>{
    u64::deserialize(deserializer).map(Some)
}

#[cfg(test)]
#[path="source_widget_membership_recovery_tests1003.rs"]
mod widget_membership_recovery_tests1003;

fn validated_recovery_high_water(recovery:&Recovery,baseline:u64)->Result<u64,String>{
    validate_recovery_editorial(recovery)?;
    let used=baseline.max(recovery.graphs.iter().map(|graph|graph.graph_id.get()).max().unwrap_or(0));
    if recovery.graph_id_high_water.is_some_and(|reserved|reserved<used){return Err("Recovery identity reservation regressed below its saved or live graph identities".into());}
    Ok(recovery.graph_id_high_water.unwrap_or(used))
}

fn validate_published_snapshot_project(root:&Path,snapshot:&Snapshot)->Result<SourceProject,String>{
    validate_snapshot_editorial(snapshot)?;
    let source=source_path(root,&snapshot.source)?;
    let native = rvn_parser::parse_for_context_discovery(&snapshot.source_snapshot)
        .map_err(|error| error.to_string())?;
    let imported_native = snapshot
        .imports
        .as_ref()
        .map(|imports| imports.contains_native_plan())
        .transpose()?
        .unwrap_or(false);
    let declared_native=crate::empty_native_init_0984::source_has_declarations(&snapshot.source_snapshot.clone(),rvn_parser::NarrativeContext::from_script(&native))?;
    let project = if crate::native_source_binding::contains_native(&native) || imported_native || declared_native {
        SourceProject::open_native_snapshot(
            &source,
            snapshot.source_snapshot.clone(),
            &snapshot.graphs,
            snapshot.imports.clone(),
        )?
    } else {
        SourceProject::open_snapshot(
            &source,
            snapshot.source_snapshot.clone(),
            &snapshot.graphs,
            snapshot.imports.clone(),
        )?
    };
    Ok(project)
}

#[cfg(test)]
#[path="source_asset_placement_source_tests1023.rs"]
mod asset_placement_source_tests1023;

fn validate_recovery_graph_shapes(graphs:&[GraphDocument])->Result<(),String>{
    let mut ids=std::collections::HashSet::new();
    for graph in graphs{
        graph.validate_narrative_metadata()?;
        if !ids.insert(graph.graph_id){return Err("Duplicate graph identity in recovery; cache retained".into());}
        let mut checked=graph.clone();checked.reconcile_import(graph)?;
    }
    Ok(())
}

#[path="source_metadata_save1025.rs"]
mod metadata_save1025;

#[cfg(test)]
#[path="source_content_enum_recovery_tests1027.rs"]
mod content_enum_recovery_tests1027;
