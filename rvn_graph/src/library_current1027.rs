//! Current authoring comes from actual graphs/descriptors, not Saved bodies.
//! Candidate emission is private and never an executable or publication API.
use crate::*;
use rvn_parser::{Statement,function_library_asset1023::{FunctionLibraryAsset,FunctionLibraryId,QualifiedLibraryFunction,LibraryFunctionAccess},function_library_program1023::{FunctionLibraryScope,LibraryMemberSource,LibraryParameterId,LibraryCallMode}};
use std::collections::BTreeSet;
#[path="library_birth_context1031.rs"]
mod birth_context1031;
pub use birth_context1031::prepare_initial_library_member_in_current1027;

/// Immutable validated projection. Its constructor compiles the whole actual
/// emitted cohort strictly before returning. It confers no owner or IO grant.
pub struct PreparedLibraryCurrentCohort1027{
    graphs:Vec<GraphDocument>,assets:Vec<FunctionLibraryAsset>,scopes:Vec<FunctionLibraryScope>,source:String,
}
impl PreparedLibraryCurrentCohort1027{
    pub fn graphs(&self)->&[GraphDocument]{&self.graphs}
    pub fn assets(&self)->&[FunctionLibraryAsset]{&self.assets}
    pub fn scopes(&self)->&[FunctionLibraryScope]{&self.scopes}
    pub fn source(&self)->&str{&self.source}
    pub(crate) fn with_authored_source1031(mut self, source: String, scopes: Vec<FunctionLibraryScope>) -> Self {
        self.source = source;
        self.scopes = scopes;
        self
    }
}

/// Borrowed structural signatures of this exact authoring candidate. No names
/// resolve nominal calls and no return type is guessed from a Saved body.
pub(crate) struct LibraryCurrentMembers1027<'a>{graphs:&'a[GraphDocument],assets:&'a[FunctionLibraryAsset]}
impl<'a> LibraryCurrentMembers1027<'a>{
    fn prepare_structural(graphs:&'a[GraphDocument],assets:&'a[FunctionLibraryAsset])->Result<Self,String>{
        let mut ids=BTreeSet::new();let mut graph_ids=BTreeSet::new();
        for graph in graphs{if !graph_ids.insert(graph.graph_id){return Err("Current Library cohort repeats a project GraphId".into())}graph.validate_library_metadata1027()?;}
        let current=Self{graphs,assets};
        for asset in assets{asset.encode()?;if !ids.insert(asset.id()){return Err("Current Library cohort repeats a descriptor UUID".into())}let(mut names,mut parameters)=(BTreeSet::new(),BTreeSet::new());for member in asset.members(){let graph=current.member(&QualifiedLibraryFunction{library:asset.id().clone(),graph_id:member.graph_id})?;let name=match &graph.kind{GraphKind::Function{name}|GraphKind::UnitFunction{name}=>name,_=>unreachable!("validated real member family")};if !names.insert(name)||graph.library1027.as_ref().unwrap().entry_parameter_ids.iter().any(|id|!parameters.insert(id)){return Err("Current Library repeats a local member name or parameter identity".into())}}}
        for graph in graphs{if let Some(owner)=graph.library1027.as_ref().and_then(|m|m.owner.as_ref()){current.member(&QualifiedLibraryFunction{library:owner.clone(),graph_id:graph.graph_id.get()})?;}}crate::validate_text_owners(graphs)?;Ok(current)
    }
    fn prepare(graphs:&'a[GraphDocument],assets:&'a[FunctionLibraryAsset])->Result<Self,String>{
        let current=Self::prepare_structural(graphs,assets)?;for graph in graphs{current.validate_graph1027(graph)?;}Ok(current)
    }
    pub(crate) fn member(&self,target:&QualifiedLibraryFunction)->Result<&'a GraphDocument,String>{
        target.validate()?;let asset=self.assets.iter().find(|a|a.id()==&target.library).ok_or("Current Library target descriptor is absent")?;let metadata=asset.members().iter().find(|m|m.graph_id==target.graph_id).ok_or("Current Library target member is retired")?;
        let matches=self.graphs.iter().filter(|g|g.graph_id.get()==target.graph_id).collect::<Vec<_>>();let[graph]=matches.as_slice()else{return Err("Current Library target has no unique actual graph".into())};
        let binding=graph.library1027.as_ref().ok_or("Current Library member has no actual nominal owner metadata")?;if binding.owner.as_ref()!=Some(&target.library){return Err("Current Library GraphId belongs to another original owner".into())}
        if !matches!((&graph.kind,metadata.pure),(GraphKind::Function{..},true)|(GraphKind::UnitFunction{..},false)){return Err("Current Library descriptor purity differs from the actual graph family".into())}
        let entry=graph.nodes.values().filter(|n|n.kind==NodeKind::FunctionEntry).collect::<Vec<_>>();let[entry]=entry.as_slice()else{return Err("Current Library member needs one actual entry".into())};
        let names=if matches!(graph.kind,GraphKind::UnitFunction{..}){crate::unit_graph_source1003::parameters(graph)?.iter().map(|p|p.label.clone()).collect::<Vec<_>>()}else{match entry.properties.get("parameters"){Some(PropertyValue::StringList(names))=>names.clone(),_=>return Err("Current value member entry parameter names are absent".into())}};
        if names.len()!=binding.entry_parameter_ids.len()||names.iter().collect::<BTreeSet<_>>().len()!=names.len()||names.iter().any(|name|rvn_parser::variable_binding_source_name(name).is_none()){return Err("Current Library entry signature differs from its original actual parameter identities".into())}Ok(graph)
    }
    pub(crate) fn parameter_ids(&self,target:&QualifiedLibraryFunction)->Result<&'a[LibraryParameterId],String>{Ok(&self.member(target)?.library1027.as_ref().unwrap().entry_parameter_ids)}
    pub(crate) fn validate_graph1027(&self,graph:&GraphDocument)->Result<(),String>{
        graph.validate_library_metadata1027()?;if graph.library1027.as_ref().and_then(|m|m.owner.as_ref()).is_some(){let owner=graph.library1027.as_ref().unwrap().owner.as_ref().unwrap();self.member(&QualifiedLibraryFunction{library:owner.clone(),graph_id:graph.graph_id.get()})?;}
        if let Some(meta)=&graph.library1027{for(node,call)in &meta.calls{let mode=match graph.nodes[node].kind{NodeKind::FunctionCall=>LibraryCallMode::Value,NodeKind::UnitCall=>LibraryCallMode::Completion,_=>return Err("Current nominal binding is not attached to an actual call node".into())};self.validate_call1027(graph,&call.target,mode)?;}}
        Ok(())
    }
    pub(crate) fn validate_call1027(&self,caller:&GraphDocument,target:&QualifiedLibraryFunction,mode:LibraryCallMode)->Result<(),String>{
        let graph=self.member(target)?;let asset=self.assets.iter().find(|a|a.id()==&target.library).unwrap();let member=asset.members().iter().find(|m|m.graph_id==target.graph_id).unwrap();
        if member.access==LibraryFunctionAccess::Private&&caller.library1027.as_ref().and_then(|m|m.owner.as_ref())!=Some(&target.library){return Err("Current Library private member is outside the actual caller's owner scope".into())}
        if !matches!((&graph.kind,mode),(GraphKind::Function{..},LibraryCallMode::Value)|(GraphKind::UnitFunction{..},LibraryCallMode::Completion)){return Err("Current Library call mode differs from the actual member family".into())}Ok(())
    }
}

/// Structural authoring admission only: retired calls/incompatible wires remain
/// real diagnostics and do not erase recovery. Compile/Save stay strict.
pub fn validate_library_membership_shape1027(graphs:&[GraphDocument],assets:&[FunctionLibraryAsset])->Result<(),String>{LibraryCurrentMembers1027::prepare_structural(graphs,assets).map(|_|())}

/// A new completion-only/PureOff member from real existing Unit/entry primitives.
/// The Source writer must still admit this proposed numeric ID atomically with
/// the descriptor/graphs. No reservation, Return, zero value or StepBirth here.
pub fn prepare_initial_library_member1027(owner:FunctionLibraryId,graph_id:GraphId,name:&str)->Result<GraphDocument,String>{
    if graph_id.get()==0{return Err("Initial Library member needs its actual fresh Source GraphId".into())}
    let kind=GraphKind::UnitFunction{name:name.into()};crate::validate_callable_name(&kind,name)?;let mut graph=GraphDocument::new(graph_id,kind);graph.add_catalog_node(NodeKind::FunctionEntry,[0.,0.]).map_err(|e|e.to_string())?;crate::unit_graph_source1003::parameters(&graph)?;
    graph.schema_version=crate::GRAPH_QUALIFIED_LIBRARY_SCHEMA_VERSION;graph.library1027=Some(LibraryGraphMetadata1027{owner:Some(owner),entry_parameter_ids:Vec::new(),calls:Default::default()});graph.validate_library_metadata1027()?;Ok(graph)
}

pub fn prepare_library_current_cohort1027(graphs:&[GraphDocument],assets:&[FunctionLibraryAsset])->Result<PreparedLibraryCurrentCohort1027,String>{
    let current=LibraryCurrentMembers1027::prepare(graphs,assets)?;let mut scopes=Vec::new();
    for asset in assets{let mut members=Vec::new();for metadata in asset.members(){let target=QualifiedLibraryFunction{library:asset.id().clone(),graph_id:metadata.graph_id};let graph=current.member(&target)?;let emitted=crate::codegen::emit_library_current_member_candidate1027(graph,graphs,&current).map_err(|e|e.to_string())?;let[declaration]=emitted.ast.as_slice()else{return Err("Current Library member did not emit exactly its actual declaration".into())};members.push(LibraryMemberSource::new(metadata.graph_id,current.parameter_ids(&target)?.to_vec(),declaration.clone())?);}let scope=FunctionLibraryScope::new(asset.id().clone(),members)?;scope.validate_asset(asset)?;scopes.push(scope);}
    let resources=LibraryGraphResources1027::prepare(&scopes,assets)?;
    // This is the only return route. It revalidates exact output types,
    // parameter bindings, access, owner and actual emitted Source once more.
    let compiled=transpile_library_project1027(graphs,&resources)?;let emitted_scopes=compiled.ast.iter().filter_map(|s|if let Statement::FunctionLibrary{scope}=s{Some(scope.clone())}else{None}).collect::<Vec<_>>();
    Ok(PreparedLibraryCurrentCohort1027{graphs:graphs.to_vec(),assets:assets.to_vec(),scopes:emitted_scopes,source:compiled.source})
}
#[cfg(test)]
#[path="library_current_tests1027.rs"]
mod tests;

/// Current member code/signatures only. Ordinary caller diagnostics are neither
/// compiled nor erased, so a real picker may repair an incomplete caller.
/// This immutable view grants no full-project Compile/Save/preview admission.
pub struct PreparedLibraryCurrentSignatures1027 {
    scopes:Vec<FunctionLibraryScope>,
    assets:Vec<FunctionLibraryAsset>,
}
impl PreparedLibraryCurrentSignatures1027 {
    pub fn resources(&self)->Result<LibraryGraphResources1027<'_>,String>{LibraryGraphResources1027::prepare(&self.scopes,&self.assets)}
    pub fn scopes(&self)->&[FunctionLibraryScope]{&self.scopes}
    pub fn assets(&self)->&[FunctionLibraryAsset]{&self.assets}
}
/// Emit all actual Library members against the complete original graph context,
/// and revalidate those members against those SAME freshly emitted scopes. No
/// Saved body, global-name alias, missing reference or guessed result is used.
/// A broken Library member still refuses; only unrelated caller errors are out
/// of this projection's compile scope, not silently declared valid.
pub fn prepare_library_current_signatures1027(graphs:&[GraphDocument],assets:&[FunctionLibraryAsset])->Result<PreparedLibraryCurrentSignatures1027,String>{
    let current=LibraryCurrentMembers1027::prepare_structural(graphs,assets)?;
    let mut scopes=Vec::new();
    for asset in assets {
        let mut members=Vec::new();
        for metadata in asset.members(){
            let target=QualifiedLibraryFunction{library:asset.id().clone(),graph_id:metadata.graph_id};
            let graph=current.member(&target)?;
            current.validate_graph1027(graph)?;
            let emitted=crate::codegen::emit_library_current_member_candidate1027(graph,graphs,&current).map_err(|e|e.to_string())?;
            let [declaration]=emitted.ast.as_slice() else{return Err("Current signature member did not emit its unique actual declaration".into())};
            members.push(LibraryMemberSource::new(metadata.graph_id,current.parameter_ids(&target)?.to_vec(),declaration.clone())?);
        }
        let scope=FunctionLibraryScope::new(asset.id().clone(),members)?;scope.validate_asset(asset)?;scopes.push(scope);
    }
    let resources=LibraryGraphResources1027::prepare(&scopes,assets)?;
    for asset in assets{for metadata in asset.members(){
        let graph=current.member(&QualifiedLibraryFunction{library:asset.id().clone(),graph_id:metadata.graph_id})?;
        // This second real emission catches stale result pins and changed
        // dependencies using the ACTUAL newly emitted member bodies/signatures.
        let check=crate::codegen::transpile_library_graph1027(graph,graphs,&resources).map_err(|e|e.to_string())?;
        let original=scopes.iter().find(|s|s.library()==asset.id()).and_then(|s|s.members().iter().find(|m|m.graph_id()==metadata.graph_id)).ok_or("Current signature member vanished during revalidation")?;
        if check.ast.as_slice()!=std::slice::from_ref(original.declaration()){return Err("Current signature revalidation changed the actual emitted member".into())}
    }}
    Ok(PreparedLibraryCurrentSignatures1027{scopes,assets:assets.to_vec()})
}
#[cfg(test)]
#[path="library_current_signature_tests1027.rs"]
mod signature_tests1027;
