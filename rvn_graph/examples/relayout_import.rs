//! Explicit layout-only maintenance for generated projects; preserves script semantics.
use rvn_graph::*;
use std::{fs, path::Path};
fn visit(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            visit(&path)?;
        } else if path.extension().and_then(|s| s.to_str()) == Some("rvngraph") {
            let mut graph = GraphDocument::from_json(&fs::read_to_string(&path)?)?;
            let before = transpile(&graph).map_err(|e| e.to_string())?.ast;
            graph
                .materialize_visible_defaults()
                .map_err(|e| e.to_string())?;
            layout_imported_values(&mut graph);
            assert_eq!(transpile(&graph).map_err(|e| e.to_string())?.ast, before);
            fs::write(&path, graph.to_pretty_json()?)?;
        }
    }
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 2 || args[0] != "--apply" {
        return Err(
            "Usage: relayout_import --apply GRAPH_DIRECTORY (changes positions only)".into(),
        );
    }
    visit(Path::new(&args[1]))
}
