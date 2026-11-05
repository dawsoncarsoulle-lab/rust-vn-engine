//! Compiler capture for the existing source-linked editor. Graph documents are
//! compared with reconstructed RVN semantics before their view paths are used.
use rvn_core::debugger_provenance::SourceMap;
use rvn_graph::GraphDocument;
use rvn_parser::{ParsedProgram,Statement};
use std::{collections::BTreeMap,path::{Path,PathBuf}};

pub fn source_compiler_map(program:&ParsedProgram,owned_source:&Path,graphs:&[(PathBuf,GraphDocument)]) -> Result<SourceMap,String> {
    let frozen=crate::FrozenNativeSource::from_frozen_program(program.clone())?;
    let source=program.sources.iter().find(|source|source.path.as_deref()==Some(owned_source)).ok_or("Owned compiler source absent")?;
    let parsed=rvn_parser::parse_with_origins(&source.source,program.context).map_err(|e|e.to_string())?;
    let script=parsed.script.into_iter().filter(|s|!matches!(s,Statement::Use{..})).collect();
    let has_native=frozen.sites().iter().any(|site|site.source==owned_source);
    let mut paths=BTreeMap::new();
    if has_native {
        let restored=rvn_graph::import_native_frame_script(&script)?;
        for (path,graph) in graphs.iter().filter(|(_,graph)|!graph.inspector_evaluation.is_legacy()) {
            let candidates:Vec<_>=restored.iter().filter(|actual|actual.graph_id==graph.graph_id).collect();
            let [actual]=candidates.as_slice()else{return Err("Captured Native graph has no unique source owner".into());};
            if actual.kind!=graph.kind||actual.nodes.keys().ne(graph.nodes.keys())||actual.pins.keys().ne(graph.pins.keys())
                ||actual.edges!=graph.edges||rvn_graph::transpile_with_origins(actual).map_err(|e|e.to_string())?.ast!=rvn_graph::transpile_with_origins(graph).map_err(|e|e.to_string())?.ast {
                return Err("Captured Native graph semantics differ from authoritative RVN".into());
            }
            if paths.insert(graph.graph_id.get(),path.clone()).is_some(){return Err("Captured graph owner is ambiguous".into());}
        }
    }
    let mut map=frozen.compiler_map().clone();
    for origin in map.origins.iter_mut().chain(map.sites.iter_mut().flat_map(|site|[&mut site.origin,&mut site.declaration])) {
        if origin.source.as_deref()==Some(owned_source) {
            if let Some(graph)=&mut origin.graph {
                if let Some(path)=paths.get(&graph.graph) {graph.source=Some(path.clone());}
            }
        }
    }
    Ok(map)
}
