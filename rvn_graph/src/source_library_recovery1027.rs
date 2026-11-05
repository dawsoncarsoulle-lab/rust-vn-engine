//! Library records inside the ONE Source recovery, not another model/cache.
use super::*;
use crate::source_function_library_recovery1023::{FunctionLibraryRecovery as Record,FunctionLibraryBaseline,LibraryRecoveryState};
use rvn_parser::function_library_asset1023::{FunctionLibraryAsset,FunctionLibraryId};
use std::collections::BTreeSet;

#[derive(Clone,Debug,PartialEq)]
pub struct SourceRecoveryCohortLibrary1027{pub batch:SourceRecoveryBatch,pub libraries:Vec<Record>,pub(super) reserved_high1027:u64}
impl SourceRecoveryCohortLibrary1027{pub fn high_water1027(&self)->u64{self.reserved_high1027}}
/// Warm original-Source identity receipt, never an editor/IO/runtime capability.
/// A cancelled fresh plan stays uncommitted; only the real writer can admit it.
#[derive(Clone,Debug)]
pub struct SourceLibraryMemberIdentity1027{
    origin:std::sync::Weak<()>,owner:FunctionLibraryId,graph_id:GraphId,
    committed:std::sync::Arc<std::sync::atomic::AtomicBool>,
}
impl SourceLibraryMemberIdentity1027{
    pub(crate) fn is_committed1027(&self)->bool{self.committed.load(std::sync::atomic::Ordering::SeqCst)}
    pub fn owner(&self)->&FunctionLibraryId{&self.owner}
    pub fn graph_id(&self)->GraphId{self.graph_id}
    fn matches(&self,workspace:&SourceWorkspace,owner:&FunctionLibraryId,id:GraphId)->bool{
        std::sync::Weak::ptr_eq(&self.origin,&std::sync::Arc::downgrade(&workspace.library_origin1027))&&&self.owner==owner&&self.graph_id==id
    }
}
#[derive(Clone,Debug,PartialEq,Serialize,Deserialize)]
#[serde(tag="kind",rename_all="snake_case",deny_unknown_fields)]
pub enum LibraryRecoveryChange1027{
    Upsert{#[serde(deserialize_with="required_before1027")]before:Option<Record>,after:Record},
    Remove{before:Record},
}
fn required_before1027<'de,D:serde::Deserializer<'de>>(d:D)->Result<Option<Record>,D::Error>{Option::<Record>::deserialize(d)}
impl LibraryRecoveryChange1027{fn id(&self)->&FunctionLibraryId{match self{Self::Upsert{after,..}=>after.id(),Self::Remove{before}=>before.id()}}}
pub(super) fn validate_records1027(records:&[Record],widgets:&[WidgetRecovery],enums:&[rvn_parser::enum_recovery1025::EnumRecoveryRecord1025])->Result<(),String>{
    let mut ids=BTreeSet::new();let mut bytes=0usize;
    if records.len()>rvn_parser::function_library_asset1023::MAX_FUNCTION_LIBRARY_MEMBERS{return Err("Library recovery owner count exceeds its existing bound".into())}
    for record in records{let wire=record.encode()?;bytes=bytes.checked_add(wire.len()).ok_or("Library recovery byte count overflow")?;if bytes>4*crate::MAX_LIBRARY_SOURCE_INPUT_BYTES1027{return Err("Library recovery exceeds its bounded original/current wire closure".into())}if !ids.insert(record.id().as_str().to_owned()){return Err("Library recovery repeats an original UUID".into())}
    }
    let mut paths=std::collections::BTreeMap::new();for record in records{for state in record.before().into_iter().chain(record.after()){let key=path_key1027(state.locator());if paths.insert(key,record.id()).is_some_and(|old|old!=record.id()){return Err("Library recovery UUIDs alias one physical locator".into())}}}
    for widget in widgets{let value=widget.blueprint()?;if ids.contains(&value.uuid)||paths.contains_key(&path_key1027(&widget.locator)){return Err("Library recovery aliases a Widget identity/locator".into())}}
    for value in enums{if ids.contains(&value.asset_id().to_hex())||paths.contains_key(&path_key1027(value.locator())){return Err("Library recovery aliases an Enum identity/locator".into())}}
    Ok(())
}
fn path_key1027(path:&str)->String{if cfg!(windows){path.replace('\\',"/").to_lowercase()}else{path.to_owned()}}
pub(super) fn apply_changes1027(previous:&[Record],changes:&[LibraryRecoveryChange1027])->Result<Vec<Record>,String>{
    validate_records1027(previous,&[],&[])?;let mut next=previous.to_vec();let mut ids=BTreeSet::new();
    for change in changes{if !ids.insert(change.id()){return Err("Library recovery change set repeats an identity".into())}let index=next.iter().position(|r|r.id()==change.id());match change{
        LibraryRecoveryChange1027::Upsert{before,after}=>{after.encode()?;match (before,index){(None,None)=>next.push(after.clone()),(Some(before),Some(index))if next[index]==*before=>{if before.id()!=after.id()||before.before()!=after.before(){return Err("Dirty Library maintenance cannot replace the original Saved baseline".into())}next[index]=after.clone()},_=>return Err("Library recovery changed before its exact mutation".into())}},
        LibraryRecoveryChange1027::Remove{before}=>{let index=index.filter(|i|next[*i]==*before).ok_or("Library recovery removal lost its original record")?;next.remove(index);},
    }}validate_records1027(&next,&[],&[])?;Ok(next)
}

pub(super) fn saved_baselines_for_project1027(project:&SourceProject)->Result<Vec<FunctionLibraryBaseline>,String>{
    let Some(input)=project.library_inputs1027()else{return Ok(Vec::new())};let mut out=Vec::new();
    for asset in input.assets(){let locator=input.manifest().libraries.get(asset.id()).ok_or("Saved Library descriptor has no original index locator")?;let locator=format!("{}/{locator}",input.manifest().assets_relative);let wire=input.source_references().iter().find(|f|path_key1027(&f.path)==path_key1027(&locator)).and_then(|f|f.before.clone()).ok_or("Saved Library original descriptor wire is absent")?;out.push(FunctionLibraryBaseline::new(asset.id().clone(),LibraryRecoveryState::new(locator,crate::source_function_library_recovery1023::FunctionLibraryWire::read(wire)?)?)?);}
    Ok(out)
}
fn saved_baselines1027(workspace:&SourceWorkspace)->Result<Vec<FunctionLibraryBaseline>,String>{workspace.project.check_imports()?;saved_baselines_for_project1027(&workspace.project)}
pub(super) fn validate_snapshot_records1027(project:&SourceProject,records:&[Record],graphs:&[GraphDocument],widgets:&[WidgetRecovery],memberships:&[WidgetMembershipRecovery],enums:&[rvn_parser::enum_recovery1025::EnumRecoveryRecord1025])->Result<(),String>{
    if records.is_empty()&&!graphs.iter().any(|g|g.library1027.is_some())&&project.library_inputs1027().is_none_or(|input|input.assets().is_empty()){return Ok(())}
    validate_records1027(records,widgets,enums)?;let baseline=saved_baselines_for_project1027(project)?;let current=crate::source_function_library_recovery1023::validate_set(&baseline,records)?;let assets=current.iter().map(|r|r.state().wire().asset()).collect::<Result<Vec<_>,_>>()?;crate::validate_library_membership_shape1027(graphs,&assets)?;
    let ids=assets.iter().map(|a|a.id().as_str()).collect::<BTreeSet<_>>();let members=assets.iter().flat_map(|a|a.members()).map(|m|m.graph_id).collect::<BTreeSet<_>>();for value in enums{if ids.contains(value.asset_id().to_hex().as_str()){return Err("Recovery Enum aliases a Library UUID".into())}}for value in widgets.iter().map(|w|w.blueprint()).chain(memberships.iter().map(|m|m.after())){let value=value?;if ids.contains(value.uuid.as_str())||value.members.iter().any(|m|members.contains(&m.get())){return Err("Recovery Widget aliases a Library UUID or member GraphId".into())}}Ok(())
}
pub(super) fn current_assets1027(workspace:&SourceWorkspace,records:&[Record])->Result<Vec<FunctionLibraryAsset>,String>{
    let baseline=saved_baselines1027(workspace)?;let next=crate::source_function_library_recovery1023::validate_set(&baseline,records)?;next.iter().map(|row|row.state().wire().asset()).collect()
}
pub(super) fn check_baselines1027(workspace:&SourceWorkspace,records:&[Record],graphs:&[GraphDocument],widgets:&[WidgetRecovery],memberships:&[WidgetMembershipRecovery],enums:&[rvn_parser::enum_recovery1025::EnumRecoveryRecord1025])->Result<(),String>{
    if records.is_empty()&&!graphs.iter().any(|g|g.library1027.is_some())&&workspace.project.library_inputs1027().is_none_or(|input|input.assets().is_empty()){return Ok(())}
    validate_records1027(records,widgets,enums)?;let assets=current_assets1027(workspace,records)?;crate::validate_library_membership_shape1027(graphs,&assets)?;
    let ids=assets.iter().map(|a|a.id().as_str()).chain(records.iter().map(|r|r.id().as_str())).collect::<BTreeSet<_>>();let members=assets.iter().flat_map(|a|a.members()).map(|m|m.graph_id).collect::<BTreeSet<_>>();
    for value in enums{if ids.contains(value.asset_id().to_hex().as_str()){return Err("Current Enum aliases a Saved/Current Library UUID".into())}}
    for value in widgets.iter().map(|w|w.blueprint()).chain(memberships.iter().map(|m|m.after())){let value=value?;if ids.contains(value.uuid.as_str())||value.members.iter().any(|m|members.contains(&m.get())){return Err("Current Widget aliases a Library UUID or member GraphId".into())}}
    for graph in graphs{if let Some(saved)=workspace.project.graphs().iter().find(|g|g.graph_id==graph.graph_id){let before=saved.library1027.as_ref().and_then(|m|m.owner.as_ref());let after=graph.library1027.as_ref().and_then(|m|m.owner.as_ref());if before!=after{return Err("Recovery cannot transfer an original Source GraphId to another nominal parent".into())}}}
    for record in records{if let Some(state)=record.before(){let target=refactor::target(&workspace.root,state.locator())?;if read_optional(&target)?.as_deref()!=Some(state.wire().as_str()){return Err("Original Saved Library descriptor changed before recovery publication".into())}}if let Some(state)=record.after(){if record.before().is_none_or(|before|path_key1027(before.locator())!=path_key1027(state.locator())){let target=refactor::target(&workspace.root,state.locator())?;if read_optional(&target)?.is_some(){return Err("New Library target was created outside its original recovery owner".into())}}}}Ok(())
}
impl SourceWorkspace{
    // Internal preparation values only. Callers use with_authoring_admission;
    // these expose neither a publisher nor any runtime/native capability.
    pub(crate) fn library_origin_weak1027(&self)->std::sync::Weak<()>{std::sync::Arc::downgrade(&self.library_origin1027)}
    pub(crate) fn library_current_assets_in_admission1027(&self,records:&[Record])->Result<Vec<FunctionLibraryAsset>,String>{current_assets1027(self,records)}
    pub(crate) fn library_saved_baselines_in_admission1027(&self)->Result<Vec<FunctionLibraryBaseline>,String>{saved_baselines1027(self)}
    /// Restore THIS exact complete recovery on its original Source. The caller
    /// installs actual documents in its guarded callback; no history or cached
    /// model is fabricated and Saved Source remains untouched.
    pub fn restore_library_recovery_with1027<T>(&mut self,expected:&SourceRecoveryCohortLibrary1027,install:impl FnOnce(&[GraphDocument],&SourceRecoveryCohortLibrary1027)->Result<T,String>)->Result<T,String>{
        let _lock=lock(&self.root)?;self.check_recovery_baseline()?;self.check_recovery_owner()?;let actual=self.recovery_cohort_libraries1027()?.ok_or("Original Library recovery disappeared before restore")?;if actual!=*expected{return Err("Library recovery changed before its complete guarded restore".into())}let result=install(self.project.graphs(),&actual)?;let high=actual.high_water1027();self.asset_placements=actual.batch.asset_placements;self.graph_id_high_water=self.graph_id_high_water.max(high);self.snapshot_matches_project=false;Ok(result)
    }
    pub fn next_library_member_graph_id1027(&self,expected:Option<&SourceRecoveryCohortLibrary1027>)->Result<GraphId,String>{
        let _lock=lock(&self.root)?;self.check_recovery_baseline()?;self.check_recovery_owner()?;let actual=self.recovery_cohort_libraries1027()?;if actual.as_ref()!=expected{return Err("Next Library identity lost its original recovery frontier".into())}let current=actual.as_ref().map(|c|c.batch.graphs.as_slice()).unwrap_or(self.project.graphs());self.graph_id_high_water().max(actual.as_ref().map(|c|c.high_water1027()).unwrap_or(0)).max(current.iter().map(|g|g.graph_id.get()).max().unwrap_or(0)).checked_add(1).map(GraphId::new).ok_or_else(||"Source graph identities are exhausted".into())
    }
    pub fn prepare_library_member_identity1027(&self,owner:&FunctionLibraryId,graph_id:GraphId,expected:Option<&SourceRecoveryCohortLibrary1027>)->Result<SourceLibraryMemberIdentity1027,String>{
        let _lock=lock(&self.root)?;self.check_recovery_baseline()?;self.check_recovery_owner()?;
        if !self.project.observes_library_inputs1027(){return Err("Library identity requires its original scoped Source observation".into())}
        let actual=self.recovery_cohort_libraries1027()?;if actual.as_ref()!=expected{return Err("Library identity preparation lost its actual recovery cohort".into())}
        let records=actual.as_ref().map(|c|c.libraries.as_slice()).unwrap_or(&[]);let assets=current_assets1027(self,records)?;if !assets.iter().any(|a|a.id()==owner){return Err("Library identity parent is absent from the original Current assets".into())}
        let current=actual.as_ref().map(|c|c.batch.graphs.as_slice()).unwrap_or(self.project.graphs());
        let known=current.iter().chain(self.project.graphs()).find(|g|g.graph_id==graph_id);
        let committed=if let Some(graph)=known{
            if graph.library1027.as_ref().and_then(|m|m.owner.as_ref())!=Some(owner){return Err("Source GraphId belongs to another original parent".into())}true
        }else{let high=self.graph_id_high_water().max(actual.as_ref().map(|c|c.high_water1027()).unwrap_or(0)).max(current.iter().map(|g|g.graph_id.get()).max().unwrap_or(0));if graph_id.get()==0||graph_id.get()<=high{return Err("Retired unpublished member identity requires its original committed receipt".into())}false};
        Ok(SourceLibraryMemberIdentity1027{origin:std::sync::Arc::downgrade(&self.library_origin1027),owner:owner.clone(),graph_id,committed:std::sync::Arc::new(std::sync::atomic::AtomicBool::new(committed))})
    }
    /// Capture Library resource dependencies on THIS Source authority. Graphs,
    /// sidecar, recovery, journal, IDs and authored files do not change. The App
    /// must advance its guarded observation epoch before using a new ticket.
    pub fn adopt_scoped_libraries1027(&mut self)->Result<bool,String>{
        let _lock=lock(&self.root)?;self.check_recovery_baseline()?;self.check_recovery_owner()?;
        if self.project.observes_library_inputs1027(){self.project.check_library_inputs1027()?;return Ok(false)}
        let inputs=SourceLibraryInputs1027::read(&self.root)?.map(std::sync::Arc::new);let next=SourceProject::open_library_at1027(&self.source_path(),self.project.source().into(),self.project.graphs(),inputs.clone(),inputs,&self.root,self.project.imports_snapshot(),false)?;
        if next.source()!=self.project.source()||next.graphs()!=self.project.graphs(){return Err("Adopting Library resources would replace original Source models; explicit source reconciliation required".into())}next.check_imports()?;self.check_recovery_baseline()?;self.project=next;Ok(true)
    }
    /// Compile actual Current data without publishing or changing Saved inputs.
    pub fn prepare_library_recovery_current1027(&self)->Result<crate::PreparedLibraryCurrentCohort1027,String>{
        let cohort = self.recovery_cohort_libraries1027()?;
        let (graphs, records) = if let Some(cohort) = &cohort {
            (cohort.batch.graphs.as_slice(), cohort.libraries.as_slice())
        } else {
            (self.project.graphs(), &[][..])
        };
        let assets = current_assets1027(self, records)?;
        let prepared = crate::prepare_library_current_cohort1027(graphs, &assets)?;
        let retired = records.iter().filter(|record| record.before().is_some() && record.after().is_none())
            .map(|record| record.id()).collect::<Vec<_>>();
        let (authored, scopes) = self.project.prepare_library_authored_current1031(&prepared, &retired)?;
        Ok(prepared.with_authored_source1031(authored, scopes))
    }
}

pub(super) fn validate_identity_changes1027(workspace:&SourceWorkspace,previous:Option<&SourceRecoveryCohortLibrary1027>,graphs:&[GraphDocument],fresh:&[GraphId],plans:&[SourceLibraryMemberIdentity1027])->Result<Vec<SourceLibraryMemberIdentity1027>,String>{
    let before=previous.map(|c|c.batch.graphs.as_slice()).unwrap_or(workspace.project.graphs());let high=workspace.graph_id_high_water().max(previous.map(|c|c.high_water1027()).unwrap_or(0)).max(before.iter().map(|g|g.graph_id.get()).max().unwrap_or(0));
    for graph in graphs{if let Some(old)=before.iter().chain(workspace.project.graphs()).find(|g|g.graph_id==graph.graph_id){if old.library1027.as_ref().and_then(|m|m.owner.as_ref())!=graph.library1027.as_ref().and_then(|m|m.owner.as_ref()){return Err("A Library edit cannot retarget an original Source GraphId to another parent".into())}}}
    let mut admitted=Vec::new();let mut expected_fresh=Vec::new();
    for graph in graphs.iter().filter(|g|!before.iter().chain(workspace.project.graphs()).any(|old|old.graph_id==g.graph_id)){
        let owner=graph.library1027.as_ref().and_then(|m|m.owner.as_ref()).ok_or("New unrelated Source graphs require their actual owning authoring admission")?;
        let matching=plans.iter().filter(|p|p.matches(workspace,owner,graph.graph_id)).collect::<Vec<_>>();let[plan]=matching.as_slice()else{return Err("Library member identity has no unique original Source admission receipt".into())};
        if graph.graph_id.get()>high{if plan.committed.load(std::sync::atomic::Ordering::SeqCst){return Err("A committed member receipt cannot become a different fresh identity".into())}expected_fresh.push(graph.graph_id);admitted.push((**plan).clone());}
        else if !plan.committed.load(std::sync::atomic::Ordering::SeqCst){return Err("Retired unpublished identity cannot be restored by a cancelled fresh plan".into())}
    }
    expected_fresh.sort();if expected_fresh.as_slice()!=fresh{return Err("Library fresh identity list differs from its actual new Source models".into())}
    for pair in fresh.windows(2){if pair[0].get().checked_add(1)!=Some(pair[1].get()){return Err("Library fresh Source identities must retain their exact prepared order".into())}}
    for plan in plans{if !std::sync::Weak::ptr_eq(&plan.origin,&std::sync::Arc::downgrade(&workspace.library_origin1027)){return Err("Library identity receipt belongs to another or retired Source incarnation".into())}}
    Ok(admitted)
}
pub(super) fn admit_identities1027(identities:&[SourceLibraryMemberIdentity1027]){for value in identities{value.committed.store(true,std::sync::atomic::Ordering::SeqCst);}}
#[cfg(test)]
#[path="source_library_recovery_tests1027.rs"]
mod tests;

impl SourceWorkspace {
    /// Explicit full typed decision only. Retains the original Source writer
    /// lock across final physical preflight, existing archive journal and
    /// infallible installation of all prepared Saved participants.
    pub fn discard_library_recovery_with1030<T>(&mut self,expected:&SourceRecoveryCohortLibrary1027,
        preflight:impl FnOnce()->Result<(),String>,install:impl FnOnce()->T)->Result<T,String>{
        let _lock=lock(&self.root)?;self.check_recovery_baseline()?;self.check_recovery_owner()?;
        if self.root.join(JOURNAL).try_exists().map_err(|error|error.to_string())?||self.root.join(refactor::REFACTOR_JOURNAL).try_exists().map_err(|error|error.to_string())?{return Err("Finish the existing Source transaction before full Library discard".into())}
        self.discard_library_cohort_locked1030(expected,preflight)?;Ok(install())
    }
}
