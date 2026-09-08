use anyhow::{Context, Result};
use rvn_graph::{transpile, GraphDocument};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Transpile a persistent editor document. `None` means the source was written
/// to stdout; otherwise the returned path is the atomically replaced output.
pub(crate) fn transpile_blueprint(
    graph_path: &Path,
    output: Option<&Path>,
    stdout: bool,
) -> Result<Option<PathBuf>> {
    let serialized = fs::read_to_string(graph_path)
        .with_context(|| format!("unable to read Blueprint graph '{}'", graph_path.display()))?;
    let graph = GraphDocument::from_json(&serialized)
        .with_context(|| format!("unable to load Blueprint graph '{}'", graph_path.display()))?;
    let generated = transpile(&graph).with_context(|| {
        format!(
            "unable to transpile Blueprint graph '{}'",
            graph_path.display()
        )
    })?;

    if stdout {
        print!("{}", generated.source);
        return Ok(None);
    }

    let output = output
        .map(Path::to_path_buf)
        .unwrap_or_else(|| graph_path.with_extension("rvn"));
    atomic_write(&output, generated.source.as_bytes()).with_context(|| {
        format!(
            "unable to write generated RVN script '{}'",
            output.display()
        )
    })?;
    Ok(Some(output))
}

fn atomic_write(path: &Path, contents: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("output path has no file name"))?;

    let (temporary_path, mut temporary) = create_temporary_file(parent, file_name)?;
    let write_result = (|| -> std::io::Result<()> {
        temporary.write_all(contents)?;
        temporary.sync_all()?;
        drop(temporary);
        fs::rename(&temporary_path, path)
    })();
    if let Err(error) = write_result {
        let _ = fs::remove_file(&temporary_path);
        return Err(error.into());
    }
    Ok(())
}

fn create_temporary_file(parent: &Path, file_name: &std::ffi::OsStr) -> Result<(PathBuf, File)> {
    let process = std::process::id();
    for attempt in 0..100_u32 {
        let mut temporary_name = std::ffi::OsString::from(".");
        temporary_name.push(file_name);
        temporary_name.push(format!(".{process}.{attempt}.tmp"));
        let temporary_path = parent.join(temporary_name);
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary_path)
        {
            Ok(file) => return Ok((temporary_path, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
    anyhow::bail!(
        "unable to reserve a temporary file next to '{}'",
        parent.join(file_name).display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use rvn_graph::{GraphId, GraphKind, NodeKind};

    #[test]
    fn transpiles_to_the_default_sibling_path_atomically() {
        let directory = std::env::temp_dir().join(format!(
            "rvn-blueprint-cli-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        fs::create_dir_all(&directory).unwrap();
        let graph_path = directory.join("chapter.rvngraph");

        let mut graph = GraphDocument::new(
            GraphId::new(1),
            GraphKind::Label {
                name: "chapter".into(),
            },
        );
        graph.add_catalog_node(NodeKind::Label, [0.0, 0.0]).unwrap();
        fs::write(&graph_path, graph.to_pretty_json().unwrap()).unwrap();

        let output = transpile_blueprint(&graph_path, None, false)
            .unwrap()
            .unwrap();
        assert_eq!(output, directory.join("chapter.rvn"));
        assert_eq!(fs::read_to_string(output).unwrap(), "label chapter\n");
        assert!(!fs::read_dir(&directory)
            .unwrap()
            .filter_map(Result::ok)
            .any(|entry| entry.file_name().to_string_lossy().ends_with(".tmp")));

        fs::remove_dir_all(directory).unwrap();
    }
}
