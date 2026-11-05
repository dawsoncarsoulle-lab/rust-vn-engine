//! Pure birth data from the caller's complete captured Current cohort.
//! No Source observer, compiler, disk lookup, identity reservation or IO grant.
use crate::{GraphDocument,GraphId,GraphKind,VariableScope};
use rvn_parser::function_library_asset1023::FunctionLibraryId;
use std::collections::{BTreeMap,BTreeSet};

/// Construct a fresh member with the original project's structural registries.
/// Init cannot contain activation-local/parameter declarations in legal RVN;
/// reject invalid graphical Current instead of treating a local as a global.
/// Other scopes may shadow a global, so only their genuine Global rows join.
/// Conflicting Current registries refuse; no neighbour is normalized or saved.
/// Without Init, pure source-less compositions may supply consistent globals,
/// but no function's character map can substitute for the absent Init owner.
pub fn prepare_initial_library_member_in_current1027(owner:FunctionLibraryId,id:GraphId,name:&str,current:&[GraphDocument])->Result<GraphDocument,String>{
 let mut ids=BTreeSet::new();
 for graph in current{if !ids.insert(graph.graph_id){return Err("Library birth Current repeats a GraphId".into())}}
 if ids.contains(&id){return Err("Library birth identity already belongs to Current".into())}
 let inits=current.iter().filter(|graph|graph.kind==GraphKind::Init).collect::<Vec<_>>();
 let init=match inits.as_slice(){[]=>None,[init]=>Some(*init),_=>return Err("Library birth requires one unambiguous Current Init".into())};
 if let Some(init)=init{
  if init.library1027.as_ref().and_then(|metadata|metadata.owner.as_ref()).is_some()||!init.local_variable_names().is_empty(){return Err("Library birth Current Init cannot have nominal, parameter or local ownership".into())}
 }
 let characters=init.map(|graph|graph.characters.clone()).unwrap_or_default();
 let mut variables=BTreeMap::new();
 for graph in current{
  // Scope getters are intentionally tolerant for editor display. Birth must
  // not inherit a parameter row if its declaration is structurally ambiguous.
  match graph.kind{
   GraphKind::UnitFunction{..}=>{crate::unit_graph_source1003::parameters(graph).map_err(|error|format!("Library birth Current parameter declarations: {error}"))?;},
   GraphKind::Function{..}|GraphKind::Screen{..}|GraphKind::Handler{..}=>{
    let kind=match graph.kind{GraphKind::Function{..}=>crate::NodeKind::FunctionEntry,GraphKind::Screen{..}=>crate::NodeKind::ScreenEntry,_=>crate::NodeKind::HandlerEntry};
    let entries=graph.nodes.values().filter(|node|node.kind==kind).collect::<Vec<_>>();
    if !matches!(entries.as_slice(),[entry] if matches!(entry.properties.get("parameters"),Some(crate::PropertyValue::StringList(_)))){return Err("Library birth Current parameter declarations are ambiguous".into())}
   },_=>{}
  }
  if graph.characters!=characters{return Err("Library birth Current character registry differs from its original Init".into())}
  for(key,definition)in &graph.variables{
   if key!=&definition.name{return Err("Library birth Current variable key differs from its definition".into())}
   if graph.variable_scope(key)!=Some(VariableScope::Global){continue}
   if variables.get(key).is_some_and(|old|old!=definition){return Err(format!("Library birth Current global {key} has contradictory definitions"))}
   variables.insert(key.clone(),definition.clone());
  }
 }
 let mut fresh=super::prepare_initial_library_member1027(owner,id,name)?;
 fresh.variables=variables;fresh.characters=characters;
 Ok(fresh)
}

#[cfg(test)]
#[path="library_birth_context_tests1031.rs"]
mod tests;
