//! Only the SourceProject presentation Vec is ordered here. Every document is
//! the fresh source/resource-reconstructed object; no old executable payload,
//! AST, Source observation, identity allocator or admission is restored.
use crate::{GraphDocument,GraphId};
use rvn_parser::function_library_asset1023::FunctionLibraryId;
use std::collections::{BTreeMap,BTreeSet};

fn nominal_owner(graph:&GraphDocument)->Option<&FunctionLibraryId>{
    graph.library1027.as_ref().and_then(|metadata|metadata.owner.as_ref())
}
pub(super) fn retain_order(mut fresh:Vec<GraphDocument>,previous:&[GraphDocument])->Result<Vec<GraphDocument>,String>{
    let mut original=BTreeMap::new();
    for(rank,graph)in previous.iter().enumerate(){
        if original.insert(graph.graph_id,(rank,&graph.kind,nominal_owner(graph))).is_some(){
            return Err("Original Source presentation repeats a GraphId".into());
        }
    }
    let mut identities=BTreeSet::<GraphId>::new();
    for graph in &fresh{
        if !identities.insert(graph.graph_id){return Err("Fresh Source projection repeats a GraphId".into());}
    }
    // The unique exact pair plus nominal owner establishes one old rank for
    // one fresh object. Changed kind/name/owner is not an old-slot match: the
    // existing importer remains its only semantic authority. Removed objects
    // stay absent; genuinely new/unmatched objects retain their fresh relative
    // source order through the stable sort. Never sort by GraphId globally.
    fresh.sort_by_key(|graph|{
        original.get(&graph.graph_id).filter(|(_,kind,owner)|
            *kind==&graph.kind&&*owner==nominal_owner(graph))
            .map(|(rank,_,_)|*rank).unwrap_or(usize::MAX)
    });
    Ok(fresh)
}

#[cfg(test)]
#[path="source_project_presentation_order_tests1031.rs"]
mod tests;
