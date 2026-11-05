//! Scoped lifecycle preparation on the ORIGINAL Source/Recovery authority.
//! No flat binding/lowered global alias, second writer, Registry or Undo stack.
use crate::source_function_library_recovery1023::{
    publication_files, validate_set, FunctionLibraryBaseline, FunctionLibraryRecovery,
    FunctionLibraryWire, LibraryRecoveryState,
};
use crate::{
    GraphDocument, GraphId, LibraryGraphResources1027, LibraryRecoveryChange1027,
    SourceAssetPlacements, SourceLibraryMemberIdentity1027, SourceRecoveryBatch,
    SourceRecoveryCohortLibrary1027, SourceRefactorFile, SourceWorkspace,
};
use rvn_parser::function_library_asset1023::{
    FunctionLibraryAsset, FunctionLibraryId, FunctionLibraryMember, LibraryFunctionAccess,
};
use rvn_parser::function_library_program1023::FunctionLibraryScope;
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Weak},
};
#[derive(Clone, PartialEq)]
struct Captured1027 {
    path: PathBuf,
    source: String,
    saved: Vec<GraphDocument>,
    current: Vec<GraphDocument>,
    high: u64,
    cohort: Option<SourceRecoveryCohortLibrary1027>,
    inputs: Option<Arc<crate::SourceLibraryInputs1027>>,
    placements: Option<SourceAssetPlacements>,
    published_placements: Option<SourceAssetPlacements>,
    assets: Vec<FunctionLibraryAsset>,
    baseline: Vec<FunctionLibraryBaseline>,
}
#[derive(Clone)]
struct SourceCapture1026 {
    origin: Weak<()>,
    data: Captured1027,
}
impl SourceCapture1026 {
    fn capture(workspace: &SourceWorkspace) -> Result<Self, String> {
        workspace.with_authoring_admission(|source| {
            if !source.project().observes_library_inputs1027() {
                return Err(
                    "Library lifecycle needs the original scoped Source observation".into(),
                );
            }
            source.project().check_imports()?;
            let cohort = source.recovery_cohort_libraries1027()?;
            let records = cohort
                .as_ref()
                .map(|c| c.libraries.as_slice())
                .unwrap_or(&[]);
            let current = cohort
                .as_ref()
                .map(|c| c.batch.graphs.clone())
                .unwrap_or_else(|| source.project().graphs().to_vec());
            let high = source
                .graph_id_high_water()
                .max(cohort.as_ref().map(|c| c.high_water1027()).unwrap_or(0));
            Ok(Self {
                origin: source.library_origin_weak1027(),
                data: Captured1027 {
                    path: source.source_path(),
                    source: source.project().source().into(),
                    saved: source.project().graphs().to_vec(),
                    current,
                    high,
                    inputs: source.source_library_inputs1027(),
                    placements: source.asset_placements().cloned(),
                    published_placements: source.published_asset_placements().cloned(),
                    assets: source.library_current_assets_in_admission1027(records)?,
                    baseline: source.library_saved_baselines_in_admission1027()?,
                    cohort,
                },
            })
        })
    }
    fn validate(&self, workspace: &SourceWorkspace) -> Result<(), String> {
        let actual = Self::capture(workspace)?;
        if !Weak::ptr_eq(&self.origin, &actual.origin) || self.data != actual.data {
            return Err(
                "Library preparation lost its original complete Source/Current frontier".into(),
            );
        }
        Ok(())
    }
    fn validate_saved(&self, actual: &Self) -> Result<(), String> {
        if !Weak::ptr_eq(&self.origin, &actual.origin)
            || self.data.path != actual.data.path
            || self.data.source != actual.data.source
            || self.data.saved != actual.data.saved
            || self.data.inputs != actual.data.inputs
            || self.data.published_placements != actual.data.published_placements
        {
            return Err(
                "Library lifecycle changed original Source incarnation or Saved resources".into(),
            );
        }
        Ok(())
    }
    fn expected_batch(&self) -> SourceRecoveryBatch {
        self.data
            .cohort
            .as_ref()
            .map(|c| c.batch.clone())
            .unwrap_or_else(|| SourceRecoveryBatch {
                graphs: self.data.current.clone(),
                widgets: Vec::new(),
                widget_memberships: Vec::new(),
                asset_placements: self.data.placements.clone(),
                enums: Vec::new(),
            })
    }
    fn validate_installed(
        &self,
        workspace: &SourceWorkspace,
        record: &FunctionLibraryRecovery,
        graphs: &[GraphDocument],
        high: u64,
    ) -> Result<Self, String> {
        let actual = Self::capture(workspace)?;
        self.validate_saved(&actual)?;
        let cohort =
            actual.data.cohort.as_ref().ok_or(
                "Library first-open/Save requires its actually committed dirty participant",
            )?;
        let mut batch = self.expected_batch();
        batch.graphs = graphs.to_vec();
        let mut records = self
            .data
            .cohort
            .as_ref()
            .map(|c| c.libraries.clone())
            .unwrap_or_default();
        if let Some(old) = records.iter_mut().find(|r| r.id() == record.id()) {
            if old.before() != record.before() {
                return Err("Library mutation replaced the immutable Saved baseline".into());
            }
            *old = record.clone()
        } else {
            records.push(record.clone())
        }
        if cohort.batch != batch
            || cohort.libraries != records
            || cohort.high_water1027() != high
            || actual.data.high != high
            || actual.data.placements != self.data.placements
        {
            return Err("Library lifecycle lost its exact installed member/descriptor or unrelated Current participants".into());
        }
        Ok(actual)
    }
}
/// Empty asset preparation. Birth itself is nontransactional editor behavior;
/// this value grants no automatic UndoBirth or implicit cold-open seed.
pub struct LibraryBirth1026 {
    source: SourceCapture1026,
    locator: String,
    asset: FunctionLibraryAsset,
    scope: FunctionLibraryScope,
    graphs: Vec<GraphDocument>,
    fresh: Vec<GraphId>,
    identities: Vec<SourceLibraryMemberIdentity1027>,
    first_open: bool,
    newborn_consumed: Arc<std::sync::atomic::AtomicBool>,
}
impl LibraryBirth1026 {
    pub fn asset(&self) -> &FunctionLibraryAsset {
        &self.asset
    }
    pub fn locator(&self) -> &str {
        &self.locator
    }
    pub fn graphs(&self) -> &[GraphDocument] {
        &self.graphs
    }
    pub fn fresh_graphs(&self) -> &[GraphId] {
        &self.fresh
    }
    pub fn identity_plans1027(&self) -> &[SourceLibraryMemberIdentity1027] {
        &self.identities
    }
    pub fn source_path(&self) -> &Path {
        &self.source.data.path
    }
    pub fn validate_original_source(&self, workspace: &SourceWorkspace) -> Result<(), String> {
        self.source.validate(workspace)
    }
    pub fn expected_cohort1027(&self) -> Option<&SourceRecoveryCohortLibrary1027> {
        self.source.data.cohort.as_ref()
    }
    pub fn recovery(&self) -> Result<FunctionLibraryRecovery, String> {
        let before = self
            .source
            .data
            .baseline
            .iter()
            .find(|b| b.id() == self.asset.id())
            .map(|b| b.state().clone());
        FunctionLibraryRecovery::new(
            self.asset.id().clone(),
            before,
            Some(LibraryRecoveryState::new(
                self.locator.clone(),
                FunctionLibraryWire::from_asset(&self.asset)?,
            )?),
        )
    }
    pub fn recovery_change1027(&self) -> Result<LibraryRecoveryChange1027, String> {
        let before = self
            .source
            .data
            .cohort
            .as_ref()
            .and_then(|c| c.libraries.iter().find(|r| r.id() == self.asset.id()))
            .cloned();
        Ok(LibraryRecoveryChange1027::Upsert {
            before,
            after: self.recovery()?,
        })
    }
    /// A NEW real Open intent must be guarded by the original App SourceRc,
    /// Registry incarnation and current epoch before calling this method. This
    /// never refreshes an old pending callback: it returns a separate frozen plan.
    /// Only the original warm token can do this; codecs/cold loaders cannot mint it.
    pub fn recapture_first_editor_open_intent1027(
        &self,
        workspace: &SourceWorkspace,
    ) -> Result<Self, String> {
        if self.first_open
            || self
                .newborn_consumed
                .load(std::sync::atomic::Ordering::SeqCst)
            || !self.asset.members().is_empty()
        {
            return Err("Original newborn intent has already been consumed".into());
        }
        let source = SourceCapture1026::capture(workspace)?;
        if !Weak::ptr_eq(&self.source.origin, &source.origin)
            || self.source.data.path != source.data.path
        {
            return Err("New Open belongs to another or cold Source lineage".into());
        }
        if !source.data.assets.iter().any(|a| a == &self.asset) {
            return Err(
                "Original newborn Library is no longer the actual empty Current asset".into(),
            );
        }
        let state = source
            .data
            .cohort
            .as_ref()
            .and_then(|c| c.libraries.iter().find(|r| r.id() == self.asset.id()))
            .and_then(|r| r.after())
            .or_else(|| {
                source
                    .data
                    .baseline
                    .iter()
                    .find(|b| b.id() == self.asset.id())
                    .map(|b| b.state())
            })
            .ok_or("Original newborn descriptor owner disappeared")?;
        if state.wire().asset()? != self.asset {
            return Err("Original newborn UUID/empty descriptor changed".into());
        }
        // A genuine new Open may follow an admitted rename/move. The original
        // App Registry incarnation guard remains mandatory; UUID and this warm
        // token do not authorize the old callback to follow a changed locator.
        let locator = state.locator().to_owned();
        let graphs = source.data.current.clone();
        Ok(Self {
            source,
            locator,
            asset: self.asset.clone(),
            scope: self.scope.clone(),
            graphs,
            fresh: Vec::new(),
            identities: Vec::new(),
            first_open: false,
            newborn_consumed: self.newborn_consumed.clone(),
        })
    }
    /// Prepare against THIS captured Open frontier. On refusal the borrowed token
    /// survives, so a later genuine Open can capture its own current frontier.
    pub fn prepare_first_editor_open(
        &self,
        workspace: &SourceWorkspace,
        name: &str,
    ) -> Result<Self, String> {
        if self.first_open
            || self
                .newborn_consumed
                .load(std::sync::atomic::Ordering::SeqCst)
            || !self.asset.members().is_empty()
        {
            return Err("Library first-editor initialization was already prepared".into());
        }
        self.source.validate(workspace)?;
        if !self.source.data.assets.iter().any(|a| a == &self.asset) {
            return Err("First editor requires the actually admitted empty Library birth".into());
        }
        let source = self.source.clone();
        let actual = source.data.cohort.as_ref();
        let graph_id = workspace.next_library_member_graph_id1027(actual)?;
        let identity =
            workspace.prepare_library_member_identity1027(self.asset.id(), graph_id, actual)?;
        let graph =
            crate::prepare_initial_library_member_in_current1027(self.asset.id().clone(), graph_id, name, &self.graphs)?;
        let asset = FunctionLibraryAsset::new(
            self.asset.id().clone(),
            vec![FunctionLibraryMember {
                graph_id: graph_id.get(),
                access: LibraryFunctionAccess::Public,
                pure: false,
            }],
        )?;
        let mut graphs = self.graphs.clone();
        graphs.push(graph);
        let (scope, _) = own_scope(&asset, &graphs)?;
        source.validate(workspace)?;
        Ok(Self {
            source,
            locator: self.locator.clone(),
            asset,
            scope,
            graphs,
            fresh: vec![graph_id],
            identities: vec![identity],
            first_open: true,
            newborn_consumed: self.newborn_consumed.clone(),
        })
    }
    /// Only the successfully admitted Source/member + actual editor-open model
    /// callback calls this infallible move. Native Created is not synthesized.
    /// A later native-window failure reopens the existing member, never a new seed.
    pub fn consume_first_editor_open1027(&self) -> bool {
        if self.first_open && self.identities.len() == 1 && self.identities[0].is_committed1027() {
            self.newborn_consumed
                .store(true, std::sync::atomic::Ordering::SeqCst);
            true
        } else {
            false
        }
    }
    /// Only an actually installed member/descriptor cohort can prepare Save.
    /// This never publishes an uncommitted first-open proposal behind Recovery.
    pub fn prepare_save(&self, workspace: &SourceWorkspace) -> Result<LibrarySavePlan1026, String> {
        let recovery = self.recovery()?;
        let high = self
            .source
            .data
            .high
            .max(self.fresh.iter().map(|id| id.get()).max().unwrap_or(0));
        let actual = self
            .source
            .validate_installed(workspace, &recovery, &self.graphs, high)?;
        let (scope, scope_source) = own_scope(&self.asset, &self.graphs)?;
        if scope != self.scope {
            return Err("Library scope changed after its newborn member intent".into());
        }
        let next_index = validate_set(&actual.data.baseline, std::slice::from_ref(&recovery))?;
        let files = publication_files(std::slice::from_ref(&recovery))?;
        actual.validate(workspace)?;
        Ok(LibrarySavePlan1026 {
            source: actual,
            asset: self.asset.clone(),
            scope,
            scope_source,
            files,
            next_index,
            recovery,
            fresh: self.fresh.clone(),
        })
    }
}
/// Descriptor/index plans only; caller still prepares coherent scoped Source
/// and invokes the real partial Save API with every original participant.
pub struct LibrarySavePlan1026 {
    source: SourceCapture1026,
    asset: FunctionLibraryAsset,
    scope: FunctionLibraryScope,
    scope_source: String,
    files: Vec<SourceRefactorFile>,
    next_index: Vec<FunctionLibraryBaseline>,
    recovery: FunctionLibraryRecovery,
    fresh: Vec<GraphId>,
}
impl LibrarySavePlan1026 {
    pub fn validate_original_source(&self, workspace: &SourceWorkspace) -> Result<(), String> {
        self.source.validate(workspace)
    }
    pub fn asset(&self) -> &FunctionLibraryAsset {
        &self.asset
    }
    pub fn scope(&self) -> &FunctionLibraryScope {
        &self.scope
    }
    pub fn scope_source(&self) -> &str {
        &self.scope_source
    }
    pub fn files(&self) -> &[SourceRefactorFile] {
        &self.files
    }
    pub fn index(&self) -> &[FunctionLibraryBaseline] {
        &self.next_index
    }
    pub fn recovery(&self) -> &FunctionLibraryRecovery {
        &self.recovery
    }
    pub fn fresh_graphs(&self) -> &[GraphId] {
        &self.fresh
    }
}
pub fn prepare_library_birth1026(
    workspace: &SourceWorkspace,
    baseline: &[FunctionLibraryBaseline],
    id: FunctionLibraryId,
    locator: &str,
) -> Result<LibraryBirth1026, String> {
    let source = SourceCapture1026::capture(workspace)?;
    if source.data.baseline.as_slice() != baseline {
        return Err("Library birth baseline is not the exact original Saved resource set".into());
    }
    if source.data.assets.iter().any(|a| a.id() == &id) {
        return Err("Library birth UUID already has an actual Current owner".into());
    }
    let asset = FunctionLibraryAsset::new(id.clone(), Vec::new())?;
    let state =
        LibraryRecoveryState::new(locator.into(), FunctionLibraryWire::from_asset(&asset)?)?;
    let recovery = FunctionLibraryRecovery::new(id.clone(), None, Some(state))?;
    let records = source
        .data
        .cohort
        .as_ref()
        .map(|c| c.libraries.as_slice())
        .unwrap_or(&[]);
    let mut changes = records.to_vec();
    changes.push(recovery);
    let current = validate_set(baseline, &changes)?;
    let assets = current
        .iter()
        .map(|r| r.state().wire().asset())
        .collect::<Result<Vec<_>, _>>()?;
    crate::validate_library_membership_shape1027(&source.data.current, &assets)?;
    let scope = FunctionLibraryScope::new(id, Vec::new())?;
    let graphs = source.data.current.clone();
    Ok(LibraryBirth1026 {
        source,
        locator: locator.into(),
        asset,
        scope,
        graphs,
        fresh: Vec::new(),
        identities: Vec::new(),
        first_open: false,
        newborn_consumed: Arc::new(std::sync::atomic::AtomicBool::new(false)),
    })
}
fn own_scope(
    asset: &FunctionLibraryAsset,
    graphs: &[GraphDocument],
) -> Result<(FunctionLibraryScope, String), String> {
    let owned = graphs
        .iter()
        .filter(|g| g.library1027.as_ref().and_then(|m| m.owner.as_ref()) == Some(asset.id()))
        .cloned()
        .collect::<Vec<_>>();
    let prepared = crate::prepare_library_current_cohort1027(&owned, std::slice::from_ref(asset))?;
    let [scope] = prepared.scopes() else {
        return Err("One original Library must emit exactly its own qualified scope".into());
    };
    scope.validate_asset(asset)?;
    Ok((scope.clone(), prepared.source().into()))
}
pub struct LoadedLibrary1026 {
    asset: FunctionLibraryAsset,
    scope: FunctionLibraryScope,
    locator: String,
    wire: String,
}
impl LoadedLibrary1026 {
    pub fn asset(&self) -> &FunctionLibraryAsset {
        &self.asset
    }
    pub fn scope(&self) -> &FunctionLibraryScope {
        &self.scope
    }
    pub fn locator(&self) -> &str {
        &self.locator
    }
    pub fn exact_wire(&self) -> &str {
        &self.wire
    }
}
fn read_cold_scope(
    asset: &FunctionLibraryAsset,
    source: &str,
) -> Result<(FunctionLibraryScope, bool), String> {
    if source.trim().is_empty() && asset.members().is_empty() {
        return Ok((
            FunctionLibraryScope::new(asset.id().clone(), Vec::new())?,
            true,
        ));
    }
    let scope = rvn_parser::parser::function_libraries1023::parse_library_scope(source)
        .map_err(|e| e.to_string())?;
    scope.validate_asset(asset)?;
    Ok((scope, false))
}
pub fn admit_saved_library_scoped1027(
    expected: &FunctionLibraryId,
    locator: &str,
    wire: &str,
    scope_source: &str,
    graphs: &[GraphDocument],
    resources: &LibraryGraphResources1027<'_>,
) -> Result<LoadedLibrary1026, String> {
    let asset = FunctionLibraryAsset::decode(wire.as_bytes())?;
    if asset.id() != expected || !resources.assets().iter().any(|a| a == &asset) {
        return Err(
            "Cold Library descriptor differs from the original admitted UUID/resources".into(),
        );
    }
    LibraryRecoveryState::new(locator.into(), FunctionLibraryWire::read(wire.into())?)?;
    let (scope, absent_empty) = read_cold_scope(&asset, scope_source)?;
    if if absent_empty {
        resources.scopes().iter().any(|s| s.library() == expected)
    } else {
        !resources.scopes().iter().any(|s| s == &scope)
    } {
        return Err(
            "Cold Library Source presence/scope differs from its exact admitted source".into(),
        );
    }
    let emitted = crate::transpile_library_project1027(graphs, resources)?;
    let matches = emitted
        .ast
        .iter()
        .any(|s| matches!(s,rvn_parser::Statement::FunctionLibrary{scope:actual}if actual==&scope));
    if matches == absent_empty {
        return Err(
            "Cold Library models differ from actual Source presence/bodies/identities".into(),
        );
    }
    Ok(LoadedLibrary1026 {
        asset,
        scope,
        locator: locator.into(),
        wire: wire.into(),
    })
}
pub fn admit_saved_library1026(
    expected: &FunctionLibraryId,
    locator: &str,
    wire: &str,
    scope_source: &str,
    graphs: &[GraphDocument],
) -> Result<LoadedLibrary1026, String> {
    let asset = FunctionLibraryAsset::decode(wire.as_bytes())?;
    let (scope, absent_empty) = read_cold_scope(&asset, scope_source)?;
    let assets = [asset];
    let scopes = if absent_empty {
        Vec::new()
    } else {
        vec![scope]
    };
    let resources = LibraryGraphResources1027::prepare(&scopes, &assets)?;
    admit_saved_library_scoped1027(expected, locator, wire, scope_source, graphs, &resources)
}
#[cfg(test)]
#[path = "function_library_creation_tests1027.rs"]
mod tests;
