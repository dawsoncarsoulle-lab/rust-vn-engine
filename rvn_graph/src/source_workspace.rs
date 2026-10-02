//! Recoverable source/presentation transactions. Authored RVN is authoritative.
use crate::{GraphDocument, SourceProject};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write,
    path::{Component, Path, PathBuf},
};

const SIDECAR: &str = ".rvn-authoring.json";
const JOURNAL: &str = ".rvn-authoring.transaction.json";
const RECOVERY: &str = ".rvn-authoring.recovery.json";

#[derive(Debug, Serialize, Deserialize)]
struct Recovery {
    version: u32,
    source: String,
    baseline_source: String,
    baseline_sidecar: Option<String>,
    graphs: Vec<GraphDocument>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Snapshot {
    version: u32,
    source: String,
    source_snapshot: String,
    graphs: Vec<GraphDocument>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Transaction {
    source: String,
    before_source: String,
    after_source: String,
    before_sidecar: Option<String>,
    after_sidecar: String,
}

#[derive(Debug, Clone)]
pub struct SourceWorkspace {
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
        let project = SourceProject::open(contents, presentation)?;
        let mut workspace = Self {
            root,
            relative_source,
            expected_sidecar: None,
            expected_recovery: None,
            project,
            snapshot_matches_project: false,
            source_error: None,
        };
        let graphs = workspace.project.graphs().to_vec();
        workspace.save_locked(&graphs)?;
        Ok(workspace)
    }

    /// Invalid external source retains the last valid graph, with its error.
    pub fn open(root: &Path) -> Result<Option<Self>, String> {
        let root = fs::canonicalize(root).map_err(|error| error.to_string())?;
        if !root.join(SIDECAR).exists() && !root.join(JOURNAL).exists() {
            return Ok(None);
        }
        let _lock = lock(&root)?;
        recover(&root)?;
        let Some(sidecar) = read_optional(&root.join(SIDECAR))? else {
            return Ok(None);
        };
        let snapshot: Snapshot =
            serde_json::from_str(&sidecar).map_err(|error| error.to_string())?;
        if snapshot.version != 1 {
            return Err(format!(
                "Unsupported source-authoring format {}",
                snapshot.version
            ));
        }
        let path = source_path(&root, &snapshot.source)?;
        let mut project = SourceProject::open(snapshot.source_snapshot.as_str(), &snapshot.graphs)?;
        let source_error = match fs::read_to_string(&path) {
            // The snapshot was fully parsed, validated and reconciled above.
            // Unchanged authored bytes need no second import of every graph.
            Ok(source) if source == project.source() => None,
            Ok(source) => project
                .refresh(source)
                .err()
                .map(|error| format!("{}: {error}", path.display())),
            Err(error) => Some(format!("{}: {error}", path.display())),
        };
        let snapshot_matches_project = snapshot.source_snapshot == project.source()
            && snapshot.graphs == project.graphs()
            && references_are_current(project.graphs())
            && snapshot
                .graphs
                .iter()
                .all(|graph| graph.schema_version == crate::GRAPH_SCHEMA_VERSION);
        let expected_recovery = read_optional(&root.join(RECOVERY))?;
        Ok(Some(Self {
            root,
            relative_source: snapshot.source,
            expected_sidecar: Some(sidecar),
            expected_recovery,
            project,
            snapshot_matches_project,
            source_error,
        }))
    }

    pub fn source_path(&self) -> PathBuf {
        self.root.join(&self.relative_source)
    }
    pub fn project(&self) -> &SourceProject {
        &self.project
    }

    /// Autosave presentation, including incomplete graphs, without touching
    /// authored RVN. A stale cache is retained for explicit conflict resolution.
    pub fn write_recovery(&mut self, graphs: &[GraphDocument]) -> Result<(), String> {
        let _lock = lock(&self.root)?;
        self.check_recovery_baseline()?;
        self.check_recovery_owner()?;
        if read_optional(&self.root.join(RECOVERY))?.is_some() {
            self.recovery()?;
        }
        let recovery = Recovery {
            version: 1,
            source: self.relative_source.clone(),
            baseline_source: self.project.source().into(),
            baseline_sidecar: self.expected_sidecar.clone(),
            graphs: graphs.to_vec(),
        };
        let contents = serde_json::to_string(&recovery).map_err(|error| error.to_string())?;
        atomic_write(&self.root.join(RECOVERY), &contents)?;
        self.expected_recovery = Some(contents);
        Ok(())
    }

    pub fn recovery(&self) -> Result<Option<Vec<GraphDocument>>, String> {
        let Some(contents) = read_optional(&self.root.join(RECOVERY))? else {
            return Ok(None);
        };
        let recovery: Recovery = serde_json::from_str(&contents).map_err(|error| {
            format!("Unreadable recovery; the original cache was retained: {error}")
        })?;
        if recovery.version != 1
            || recovery.source != self.relative_source
            || recovery.baseline_source != self.project.source()
            || recovery.baseline_sidecar != self.expected_sidecar
        {
            return Err("Recovery conflicts with a changed source or presentation. The cache was retained; resolve the conflict explicitly.".into());
        }
        self.check_recovery_baseline()?;
        // Recover incomplete logic, but never corrupt identities/pins/edges.
        let mut ids = std::collections::HashSet::new();
        for graph in &recovery.graphs {
            if !ids.insert(graph.graph_id) {
                return Err("Duplicate graph identity in recovery; cache retained".into());
            }
            let mut checked = graph.clone();
            checked.reconcile_import(graph)?;
        }
        Ok(Some(recovery.graphs))
    }

    /// Discarding a cache archives it first; it remains manually recoverable.
    pub fn archive_recovery(&mut self) -> Result<(), String> {
        let _lock = lock(&self.root)?;
        self.archive_recovery_locked()
    }

    fn archive_recovery_locked(&mut self) -> Result<(), String> {
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
        self.save_locked(graphs)
    }

    fn save_locked(&mut self, graphs: &[GraphDocument]) -> Result<(), String> {
        if let Some(error) = &self.source_error {
            return Err(error.clone());
        }
        self.check_recovery_owner()?;
        if self.expected_recovery.is_some() {
            self.recovery()?;
        }
        let path = source_path(&self.root, &self.relative_source)?;
        let current = fs::read_to_string(&path).map_err(|error| error.to_string())?;
        let sidecar = read_optional(&self.root.join(SIDECAR))?;
        if sidecar != self.expected_sidecar {
            return Err("Blueprint presentation was changed externally. Reopen or resolve the conflict; no files were overwritten.".into());
        }
        if self.snapshot_matches_project
            && current == self.project.source()
            && graphs == self.project.graphs()
        {
            // A clean explicit Save does not need to compile/reimport every
            // graph again. The baseline was fully checked on open/save, and
            // all disk/recovery ownership checks remain under the save lock.
            self.check_save_inputs(&path, &current, &sidecar)?;
            self.archive_recovery_locked()?;
            return Ok(());
        }
        let mut next = self.project.clone();
        next.apply_visual(&current, graphs)?;
        let snapshot = Snapshot {
            version: 1,
            source: self.relative_source.clone(),
            source_snapshot: next.source().into(),
            graphs: next.graphs().to_vec(),
        };
        let after_sidecar =
            serde_json::to_string_pretty(&snapshot).map_err(|error| error.to_string())?;
        let transaction = Transaction {
            source: self.relative_source.clone(),
            before_source: current.clone(),
            after_source: next.source().into(),
            before_sidecar: sidecar.clone(),
            after_sidecar: after_sidecar.clone(),
        };
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
        // Successful explicit save supersedes autosaved edits, but archives
        // rather than deletes them. If archiving fails, the cache stays intact.
        self.archive_recovery_locked()?;
        atomic_write(
            &self.root.join(JOURNAL),
            &serde_json::to_string(&transaction).map_err(|error| error.to_string())?,
        )?;
        recover(&self.root)?;
        self.expected_sidecar = Some(after_sidecar);
        self.snapshot_matches_project = next
            .graphs()
            .iter()
            .all(|graph| graph.schema_version == crate::GRAPH_SCHEMA_VERSION);
        self.project = next;
        Ok(())
    }

    fn check_save_inputs(
        &self,
        path: &Path,
        current: &str,
        sidecar: &Option<String>,
    ) -> Result<(), String> {
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

fn lock(root: &Path) -> Result<fs::File, String> {
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
    let Some(journal) = read_optional(&root.join(JOURNAL))? else {
        return Ok(());
    };
    let transaction: Transaction = serde_json::from_str(&journal)
        .map_err(|error| format!("Incomplete save journal: {error}"))?;
    let source = source_path(root, &transaction.source)?;
    let current = fs::read_to_string(&source).map_err(|error| error.to_string())?;
    let sidecar = read_optional(&root.join(SIDECAR))?;
    if (current != transaction.before_source && current != transaction.after_source)
        || (sidecar != transaction.before_sidecar
            && sidecar.as_deref() != Some(&transaction.after_sidecar))
    {
        return Err("An interrupted save conflicts with external edits. The journal and backups were retained; resolve the conflict explicitly.".into());
    }
    let snapshot: Snapshot =
        serde_json::from_str(&transaction.after_sidecar).map_err(|error| error.to_string())?;
    if snapshot.version != 1
        || snapshot.source != transaction.source
        || snapshot.source_snapshot != transaction.after_source
    {
        return Err("Inconsistent save journal; files were not overwritten".into());
    }
    SourceProject::open(snapshot.source_snapshot, &snapshot.graphs)?;
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
            let _ = fs::remove_dir_all(&self.0);
        }
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
            version: 1,
            source: "story.rvn".into(),
            source_snapshot: source.clone(),
            graphs: linked.project().graphs().to_vec(),
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
                version: 1,
                source: "story.rvn".into(),
                source_snapshot: next.source().into(),
                graphs: next.graphs().to_vec(),
            })
            .unwrap();
            let transaction = Transaction {
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
