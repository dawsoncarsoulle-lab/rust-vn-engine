//! Opt-in delivery QA through the same public source-linked workspace API.
//! The default mode only checks; --save-cache requires an unchanged RVN preflight.
use std::{collections::BTreeMap, fs, path::PathBuf};
use rvn_graph::{GraphDocument, SourceWorkspace};

fn main() {
    if let Err(error) = run() {
        eprintln!("Atlas authoring-cache check refused: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut arguments=std::env::args().skip(1);
    let root=PathBuf::from(arguments.next().ok_or("Usage: atlas-authoring-cache PROJECT [--save-cache]")?);
    let save=match arguments.next().as_deref() {
        None=>false,
        Some("--save-cache")=>true,
        _=>return Err("Only the explicit --save-cache option is supported".into()),
    };
    if arguments.next().is_some(){return Err("Unexpected argument".into());}
    let root=fs::canonicalize(root).map_err(|error|error.to_string())?;
    for name in [".rvn-authoring.transaction.json",".rvn-authoring.recovery.json"] {
        if root.join(name).exists(){return Err(format!("Pending {name}; resolve the authoring session first. No source/cache was changed."));}
    }
    let sidecar=root.join(".rvn-authoring.json");
    let cache_before=fs::read(&sidecar).map_err(|error|error.to_string())?;
    let old:serde_json::Value=serde_json::from_slice(&cache_before).map_err(|error|error.to_string())?;
    let relative=old["source"].as_str().ok_or("Source path missing from cache")?;
    // The real workspace validates its linked path; restrict this delivery tool
    // to this demo's canonical main before reading or preparing any transaction.
    if relative!="main.rvn"{return Err("This Atlas delivery check expects canonical main.rvn".into());}
    let source_path=root.join(relative);
    let source_before=fs::read(&source_path).map_err(|error|error.to_string())?;
    let source_modified=fs::metadata(&source_path).and_then(|metadata|metadata.modified()).map_err(|error|error.to_string())?;
    let previous:Vec<GraphDocument>=serde_json::from_value(old["graphs"].clone()).map_err(|error|error.to_string())?;
    let mut workspace=SourceWorkspace::open(&root)?.ok_or("Project is not source-linked")?;
    if let Some(error)=&workspace.source_error{return Err(format!("Linked source error: {error}"));}
    if workspace.source_path()!=source_path||workspace.project().source().as_bytes()!=source_before {
        return Err("Reopening did not retain canonical main.rvn exactly".into());
    }
    let graphs=workspace.project().graphs().to_vec();
    let mut surviving_nodes=0;
    for old_graph in &previous {
        let current=graphs.iter().find(|graph|graph.kind==old_graph.kind).ok_or_else(||format!("Scope disappeared while reopening: {:?}",old_graph.kind))?;
        if current.graph_id!=old_graph.graph_id{return Err(format!("Graph identity changed for {:?}",old_graph.kind));}
        for (id,node) in &old_graph.nodes {
            if let Some(current_node)=current.nodes.get(id) {
                if current_node.position!=node.position{return Err(format!("Surviving node {id:?} moved in {:?}",old_graph.kind));}
                surviving_nodes+=1;
            }
        }
    }
    let before_ids:BTreeMap<_,_>=graphs.iter().map(|graph|(graph.graph_id,graph.nodes.iter().map(|(id,node)|(*id,node.position)).collect::<BTreeMap<_,_>>())).collect();
    let mut preflight=workspace.project().clone();
    preflight.apply_visual(workspace.project().source(),&graphs)?;
    if preflight.source().as_bytes()!=source_before{return Err("Saving these graphs would change RVN; cache was not saved".into());}
    if fs::read(&source_path).map_err(|error|error.to_string())?!=source_before||fs::read(&sidecar).map_err(|error|error.to_string())?!=cache_before {
        return Err("Source/cache changed during preflight; no save was attempted".into());
    }
    if save {
        workspace.save(&graphs)?;
        if fs::read(&source_path).map_err(|error|error.to_string())?!=source_before{return Err("Source bytes unexpectedly changed during save".into());}
        let after:serde_json::Value=serde_json::from_slice(&fs::read(&sidecar).map_err(|error|error.to_string())?).map_err(|error|error.to_string())?;
        if after["source_snapshot"].as_str().map(str::as_bytes)!=Some(source_before.as_slice()){return Err("Saved cache snapshot does not match canonical main".into());}
        let reopened=SourceWorkspace::open(&root)?.ok_or("Saved project lost its link")?;
        if reopened.source_error.is_some(){return Err("Saved project failed its second reopen".into());}
        let after_ids:BTreeMap<_,_>=reopened.project().graphs().iter().map(|graph|(graph.graph_id,graph.nodes.iter().map(|(id,node)|(*id,node.position)).collect::<BTreeMap<_,_>>())).collect();
        if before_ids!=after_ids{return Err("Node identities/positions changed during cache save/reopen".into());}
    }
    if fs::read(&source_path).map_err(|error|error.to_string())?!=source_before||fs::metadata(&source_path).and_then(|metadata|metadata.modified()).map_err(|error|error.to_string())?!=source_modified {
        return Err("Canonical main was touched during the check".into());
    }
    println!("{}",serde_json::json!({
        "result":"passed","mode":if save{"save-cache"}else{"check"},"project":root,
        "source_error":null,"source_bytes_unchanged":true,"source_mtime_unchanged":true,
        "graphs":graphs.len(),"previous_graph_ids_preserved":previous.len(),
        "surviving_node_positions_preserved":surviving_nodes,
        "source_snapshot_current":save||old["source_snapshot"].as_str().map(str::as_bytes)==Some(source_before.as_slice())
    }));
    Ok(())
}
