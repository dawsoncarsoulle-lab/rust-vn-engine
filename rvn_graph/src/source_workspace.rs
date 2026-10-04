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
#[path="source_refactor.rs"] mod refactor;
pub use refactor::{SourceRefactorFile,SourceRefactorReceipt};

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    imports: Option<crate::source_project::ImportSnapshot>,
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
        let project = SourceProject::open_at(&source, contents, presentation)?;
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
        if !root.join(SIDECAR).exists() && !root.join(JOURNAL).exists() && !root.join(refactor::REFACTOR_JOURNAL).exists() {
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
        let mut project = SourceProject::open_snapshot(&path, snapshot.source_snapshot.clone(), &snapshot.graphs, snapshot.imports.clone())?;
        let source_error = match fs::read_to_string(&path) {
            Ok(source) if source == project.source() && project.imports_snapshot().is_none()
                && !project.has_imports() => None,
            // Imports can change even while the linked file's bytes do not.
            // A failed refresh retains the validated, cached context/graphs.
            Ok(source) => project.refresh(source).err().map(|error| format!("{}: {error}", path.display())),
            Err(error) => Some(format!("{}: {error}", path.display())),
        };
        let snapshot_matches_project = snapshot.source_snapshot == project.source()
            && snapshot.graphs == project.graphs()
            && snapshot.imports == project.imports_snapshot()
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
        let mut recovery: Recovery = serde_json::from_str(&contents).map_err(|error| {
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
        for graph in &mut recovery.graphs {
            if !ids.insert(graph.graph_id) {
                return Err("Duplicate graph identity in recovery; cache retained".into());
            }
            let mut checked = graph.clone();
            checked.reconcile_import(graph)?;
            // Old autosaves bypass GraphDocument::from_json. Upgrade only
            // additive assignment outputs in memory, retaining the original
            // recovery file and every authored pin, wire and position.
            graph.normalize_assignment_value_outputs().map_err(|error| error.to_string())?;
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
        self.project.check_imports()?;
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
            imports: next.imports_snapshot(),
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
        next.check_imports()?;
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
        self.project.check_imports()?;
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
    refactor::recover_refactor(root)?;
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
    let project = SourceProject::open_snapshot(&source, snapshot.source_snapshot, &snapshot.graphs, snapshot.imports)?;
    project.check_imports()?;
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
            let Ok(path) = self.0.canonicalize() else { return; };
            let Ok(temporary) = std::env::temp_dir().canonicalize() else { return; };
            if path.parent() == Some(temporary.as_path())
                && path.file_name().and_then(|name| name.to_str()).is_some_and(|name| name.starts_with("rvn-source-storage-")) {
                let _ = fs::remove_dir_all(path);
            }
        }
    }

    const IMPORTED_MAIN: &str = "// main stays authored\nuse \"chapter.rvn\"\nfunction local_value(n) { return helper(n) }\nlabel start\n music.volume(\"0.2\")\n \"[local_value(3)]\"\n ui.open(\"hud\", [], false, 1)\n jump chapter\n";
    const IMPORTED_CHAPTER: &str = "// exact imported CRLF and Unicode — 雨\r\nfunction helper(n) { return n + 2 }\r\nscreen hud() { return {\"id\":\"root\",\"kind\":\"text\",\"text\":\"Imported\"} }\r\nhandler clicked(event) { return true }\r\nlabel chapter\r\n\"Imported chapter\"\r\nreturn\r\n";

    fn imported_fixture() -> (Fixture, PathBuf) {
        let fixture = Fixture::new();
        fs::create_dir(fixture.0.join("scripts")).unwrap();
        let path = fixture.0.join("scripts/main.rvn");
        fs::write(&path, IMPORTED_MAIN).unwrap();
        fs::write(fixture.0.join("scripts/chapter.rvn"), IMPORTED_CHAPTER).unwrap();
        // An identically named root file must never supply the import's scope.
        fs::write(fixture.0.join("chapter.rvn"), "label wrong_origin\nreturn\n").unwrap();
        (fixture, path)
    }

    fn imported_edits(workspace: &SourceWorkspace) -> Vec<GraphDocument> {
        let mut graphs = workspace.project().graphs().to_vec();
        let graph = graphs.iter_mut().find(|graph| matches!(&graph.kind, GraphKind::Label { name } if name == "start")).unwrap();
        let volume = graph.nodes.values().find(|node| node.kind == NodeKind::MusicVolume).unwrap().id;
        let pin = graph.pin_by_key(volume, "level").unwrap().id;
        let producer = graph.pins[&graph.edges.values().find(|edge| edge.input == pin).unwrap().output].node;
        graph.set_property(producer, "value", PropertyValue::Float(0.4)).unwrap();
        graph.nodes.get_mut(&volume).unwrap().position = [430.0, 210.0];
        graphs
    }

    #[test]
    fn linked_imports_validate_the_real_project_without_writing_or_projecting_imported_scopes() {
        let (fixture, path) = imported_fixture();
        let mut workspace = SourceWorkspace::link(&fixture.0, &path, &[]).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), IMPORTED_MAIN);
        assert!(workspace.project().graphs().iter().all(|graph| match &graph.kind {
            GraphKind::Init => true,
            GraphKind::Label { name } => name == "start",
            GraphKind::Function { name } => name == "local_value",
            _ => false,
        }));
        let compiled = workspace.project().resolved_script().unwrap();
        assert_eq!(compiled, rvn_parser::parse_file_with_uses(&path).unwrap());
        assert!(compiled.iter().any(|statement| matches!(statement, rvn_parser::Statement::Screen { name, .. } if name == "hud")));
        assert!(compiled.iter().any(|statement| matches!(statement, rvn_parser::Statement::Handler { name, .. } if name == "clicked")));
        let files = workspace.project().resolved_source_files().unwrap();
        assert_eq!(files.len(), 2);
        assert!(files.iter().any(|(file, source)| file == &fixture.0.join("scripts/chapter.rvn").canonicalize().unwrap() && source == IMPORTED_CHAPTER));
        let graphs = imported_edits(&workspace);
        workspace.save(&graphs).unwrap();
        assert!(workspace.project().source().contains("// main stays authored"));
        assert!(workspace.project().source().contains("use \"chapter.rvn\""));
        assert!(workspace.project().resolved_script().unwrap().iter().any(|statement| matches!(statement, rvn_parser::Statement::MusicVolume { level } if *level == 0.4)));
        assert_eq!(fs::read_to_string(fixture.0.join("scripts/chapter.rvn")).unwrap(), IMPORTED_CHAPTER);
        assert_eq!(fs::read_to_string(fixture.0.join("chapter.rvn")).unwrap(), "label wrong_origin\nreturn\n");
        let backups: Vec<_> = fs::read_dir(fixture.0.join(".rvn-backups")).unwrap().filter_map(Result::ok).collect();
        assert!(backups.iter().any(|entry| fs::read_to_string(entry.path().join("source.rvn")).ok().as_deref() == Some(IMPORTED_MAIN)));
        assert!(backups.iter().all(|entry| !entry.path().join("chapter.rvn").exists()));
        // Reopening exercises the same source projection with prior graph
        // identities and layout, including the cached import snapshot.
        let reopened = SourceWorkspace::open(&fixture.0).unwrap().unwrap();
        assert!(reopened.source_error.is_none());
        assert_eq!(reopened.project().graphs(), workspace.project().graphs());
        assert_eq!(reopened.project().resolved_script().unwrap(), rvn_parser::parse_file_with_uses(&path).unwrap());
        assert!(reopened.project().source().contains("use \"chapter.rvn\""));
        assert_eq!(fs::read_to_string(fixture.0.join("scripts/chapter.rvn")).unwrap(), IMPORTED_CHAPTER);
    }

    #[test]
    fn real_imports_never_relax_unknown_references_at_link_or_save() {
        let (fixture, path) = imported_fixture();
        let invalid = IMPORTED_MAIN.replace("jump chapter", "jump absent");
        fs::write(&path, &invalid).unwrap();
        assert!(SourceWorkspace::link(&fixture.0, &path, &[]).unwrap_err().contains("absent"));
        assert!(!fixture.0.join(SIDECAR).exists());
        assert_eq!(fs::read_to_string(&path).unwrap(), invalid);
        let invalid = IMPORTED_MAIN.replace("helper(n)", "missing_function(n)");
        assert!(SourceProject::open_at(&path, invalid, &[]).unwrap_err().contains("missing_function"));
        fs::write(&path, IMPORTED_MAIN).unwrap();
        let mut workspace = SourceWorkspace::link(&fixture.0, &path, &[]).unwrap();
        let original = workspace.project().graphs().to_vec();
        let mut invalid = original.clone();
        let graph = invalid.iter_mut().find(|graph| matches!(&graph.kind, GraphKind::Label { name } if name == "start")).unwrap();
        let target = graph.nodes.values().find(|node| node.kind == NodeKind::LabelValue && node.properties.get("label") == Some(&PropertyValue::String("chapter".into()))).unwrap().id;
        graph.set_property(target, "label", PropertyValue::String("absent".into())).unwrap();
        workspace.write_recovery(&invalid).unwrap();
        let sidecar = fs::read(fixture.0.join(SIDECAR)).unwrap();
        let recovery = fs::read(fixture.0.join(RECOVERY)).unwrap();
        assert!(workspace.save(&invalid).unwrap_err().contains("absent"));
        assert_eq!(workspace.project().graphs(), original);
        assert_eq!(fs::read_to_string(&path).unwrap(), IMPORTED_MAIN);
        assert_eq!(fs::read(fixture.0.join(SIDECAR)).unwrap(), sidecar);
        assert_eq!(fs::read(fixture.0.join(RECOVERY)).unwrap(), recovery);
        assert_eq!(fs::read_to_string(fixture.0.join("scripts/chapter.rvn")).unwrap(), IMPORTED_CHAPTER);
    }

    #[test]
    fn a_use_after_a_label_refuses_visual_projection_and_preserves_last_valid_cache_and_recovery() {
        let (fixture, path) = imported_fixture();
        let mut workspace = SourceWorkspace::link(&fixture.0, &path, &[]).unwrap();
        let edited = imported_edits(&workspace);
        workspace.write_recovery(&edited).unwrap();
        let graphs = workspace.project().graphs().to_vec();
        let sidecar = fs::read(fixture.0.join(SIDECAR)).unwrap();
        let recovery = fs::read(fixture.0.join(RECOVERY)).unwrap();
        let interleaved = IMPORTED_MAIN.replacen("use \"chapter.rvn\"\n", "", 1)
            .replacen("label start\n", "label start\nuse \"chapter.rvn\"\n", 1);
        fs::write(&path, &interleaved).unwrap();
        // This is a legal runtime import. Only visual ownership is unsafe:
        // the imported label would own the following narrative statements.
        assert!(rvn_parser::parse_file_with_uses(&path).is_ok());
        assert!(SourceProject::open_at(&path, &interleaved, &[]).unwrap_err().contains("before the first narrative label"));
        assert!(workspace.project.refresh(&interleaved).unwrap_err().contains("before the first narrative label"));
        assert_eq!(workspace.project().source(), IMPORTED_MAIN);
        assert_eq!(workspace.project().graphs(), graphs);
        let mut reopened = SourceWorkspace::open(&fixture.0).unwrap().unwrap();
        assert!(reopened.source_error.as_deref().unwrap().contains("before the first narrative label"));
        assert_eq!(reopened.project().source(), IMPORTED_MAIN);
        assert_eq!(reopened.project().graphs(), graphs);
        assert!(reopened.recovery().is_err());
        assert!(reopened.save(&edited).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), interleaved);
        assert_eq!(fs::read(fixture.0.join(SIDECAR)).unwrap(), sidecar);
        assert_eq!(fs::read(fixture.0.join(RECOVERY)).unwrap(), recovery);
        assert_eq!(fs::read_to_string(fixture.0.join("scripts/chapter.rvn")).unwrap(), IMPORTED_CHAPTER);
    }

    #[test]
    fn changed_or_missing_import_keeps_recovery_and_the_last_valid_portable_snapshot() {
        let (fixture, path) = imported_fixture();
        let mut workspace = SourceWorkspace::link(&fixture.0, &path, &[]).unwrap();
        let graphs = imported_edits(&workspace);
        workspace.write_recovery(&graphs).unwrap();
        let sidecar = fs::read(fixture.0.join(SIDECAR)).unwrap();
        let recovery = fs::read(fixture.0.join(RECOVERY)).unwrap();
        let external = format!("{IMPORTED_CHAPTER}// externally edited\n");
        fs::write(fixture.0.join("scripts/chapter.rvn"), &external).unwrap();
        assert!(workspace.save(&graphs).unwrap_err().contains("changed externally"));
        assert!(workspace.project().resolved_script().is_err());
        assert_eq!(fs::read_to_string(fixture.0.join("scripts/chapter.rvn")).unwrap(), external);
        assert_eq!(fs::read_to_string(&path).unwrap(), IMPORTED_MAIN);
        assert_eq!(fs::read(fixture.0.join(SIDECAR)).unwrap(), sidecar);
        assert_eq!(fs::read(fixture.0.join(RECOVERY)).unwrap(), recovery);
        fs::remove_file(fixture.0.join("scripts/chapter.rvn")).unwrap();
        let reopened = SourceWorkspace::open(&fixture.0).unwrap().unwrap();
        assert!(reopened.source_error.is_some());
        assert_eq!(reopened.project().graphs(), workspace.project().graphs());
        assert_eq!(fs::read(fixture.0.join(RECOVERY)).unwrap(), recovery);
        fs::write(fixture.0.join("scripts/chapter.rvn"), IMPORTED_CHAPTER).unwrap();

        let moved = Fixture::new();
        fs::create_dir(moved.0.join("scripts")).unwrap();
        fs::copy(&path, moved.0.join("scripts/main.rvn")).unwrap();
        fs::copy(fixture.0.join("scripts/chapter.rvn"), moved.0.join("scripts/chapter.rvn")).unwrap();
        fs::copy(fixture.0.join(SIDECAR), moved.0.join(SIDECAR)).unwrap();
        let portable = SourceWorkspace::open(&moved.0).unwrap().unwrap();
        assert!(portable.source_error.is_none());
        assert_eq!(portable.project().graphs(), workspace.project().graphs());
        assert_eq!(portable.project().resolved_script().unwrap(), rvn_parser::parse_file_with_uses(moved.0.join("scripts/main.rvn")).unwrap());
    }

    #[test]
    fn wildcards_nested_deduplicated_imports_and_cycles_match_runtime_resolution() {
        let fixture = Fixture::new();
        fs::create_dir(fixture.0.join("chapters")).unwrap();
        fs::write(fixture.0.join("chapters/a.rvn"), "use \"../helper.rvn\"\nlabel a\nreturn\n").unwrap();
        fs::write(fixture.0.join("chapters/b.rvn"), "use \"../helper.rvn\"\nlabel b\nreturn\n").unwrap();
        fs::write(fixture.0.join("helper.rvn"), "function helper(n) { return n + 1 }\n").unwrap();
        let path = fixture.0.join("story.rvn");
        let main = "use { \"chapters/*.rvn\", \"helper.rvn\" }\nlabel start\n\"[helper(3)]\"\njump a\n";
        fs::write(&path, main).unwrap();
        let mut workspace = SourceWorkspace::link(&fixture.0, &path, &[]).unwrap();
        assert_eq!(workspace.project().resolved_script().unwrap(), rvn_parser::parse_file_with_uses(&path).unwrap());
        assert_eq!(workspace.project().resolved_source_files().unwrap().len(), 4);
        let graphs = workspace.project().graphs().to_vec();
        fs::write(fixture.0.join("chapters/c.rvn"), "label c\nreturn\n").unwrap();
        assert!(workspace.save(&graphs).unwrap_err().contains("changed externally"));
        let reopened = SourceWorkspace::open(&fixture.0).unwrap().unwrap();
        assert!(reopened.source_error.is_none());
        assert!(reopened.project().resolved_script().unwrap().iter().any(|statement| matches!(statement, rvn_parser::Statement::Label { name } if name == "c")));
        fs::write(fixture.0.join("helper.rvn"), "use \"story.rvn\"\nfunction helper(n) { return n + 1 }\n").unwrap();
        assert!(SourceProject::open_at(&path, main, &[]).unwrap_err().contains("cycle"));
        assert!(rvn_parser::parse_file_with_uses(&path).is_err());
        assert!(SourceWorkspace::open(&fixture.0).unwrap().unwrap().source_error.is_some());
    }

    #[test]
    fn interrupted_save_checks_import_bytes_before_publishing_main_or_presentation() {
        let (fixture, path) = imported_fixture();
        let workspace = SourceWorkspace::link(&fixture.0, &path, &[]).unwrap();
        let before_sidecar = fs::read_to_string(fixture.0.join(SIDECAR)).unwrap();
        let mut next = workspace.project().clone();
        next.apply_visual(IMPORTED_MAIN, &imported_edits(&workspace)).unwrap();
        let after_sidecar = serde_json::to_string(&Snapshot {
            version: 1, source: "scripts/main.rvn".into(), source_snapshot: next.source().into(),
            graphs: next.graphs().to_vec(), imports: next.imports_snapshot(),
        }).unwrap();
        let transaction = Transaction {
            source: "scripts/main.rvn".into(), before_source: IMPORTED_MAIN.into(), after_source: next.source().into(),
            before_sidecar: Some(before_sidecar.clone()), after_sidecar,
        };
        let journal = serde_json::to_string(&transaction).unwrap();
        fs::write(fixture.0.join(JOURNAL), &journal).unwrap();
        let external = format!("{IMPORTED_CHAPTER}// third party\n");
        fs::write(fixture.0.join("scripts/chapter.rvn"), &external).unwrap();
        assert!(SourceWorkspace::open(&fixture.0).unwrap_err().contains("changed externally"));
        assert_eq!(fs::read_to_string(&path).unwrap(), IMPORTED_MAIN);
        assert_eq!(fs::read_to_string(fixture.0.join(SIDECAR)).unwrap(), before_sidecar);
        assert_eq!(fs::read_to_string(fixture.0.join(JOURNAL)).unwrap(), journal);
        assert_eq!(fs::read_to_string(fixture.0.join("scripts/chapter.rvn")).unwrap(), external);
        fs::write(fixture.0.join("scripts/chapter.rvn"), IMPORTED_CHAPTER).unwrap();
        let recovered = SourceWorkspace::open(&fixture.0).unwrap().unwrap();
        assert_eq!(recovered.project().source(), next.source());
        assert!(!fixture.0.join(JOURNAL).exists());
        assert_eq!(fs::read_to_string(fixture.0.join("scripts/chapter.rvn")).unwrap(), IMPORTED_CHAPTER);
    }

    #[test]
    fn refactor_and_replay_keep_imports_read_only_and_compare_their_exact_baseline() {
        let (fixture, path) = imported_fixture();
        let mut workspace = SourceWorkspace::link(&fixture.0, &path, &[]).unwrap();
        let original = workspace.project().graphs().to_vec();
        let graphs = imported_edits(&workspace);
        let extra = SourceRefactorFile { path: "scripts/chapter.rvn".into(), before: Some(IMPORTED_CHAPTER.into()), after: Some("label chapter\nreturn\n".into()) };
        assert!(workspace.refactor(&graphs, &[extra]).unwrap_err().contains("read-only"));
        assert_eq!(fs::read_to_string(&path).unwrap(), IMPORTED_MAIN);
        assert_eq!(workspace.project().graphs(), original);
        let receipt = workspace.refactor(&graphs, &[]).unwrap();
        assert_eq!(workspace.project().resolved_script().unwrap(), rvn_parser::parse_file_with_uses(&path).unwrap());
        workspace.replay_refactor(&receipt, false).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), IMPORTED_MAIN);
        let sidecar = fs::read(fixture.0.join(SIDECAR)).unwrap();
        let external = format!("{IMPORTED_CHAPTER}// external edit\n");
        fs::write(fixture.0.join("scripts/chapter.rvn"), &external).unwrap();
        assert!(workspace.replay_refactor(&receipt, true).unwrap_err().contains("changed externally"));
        assert_eq!(fs::read_to_string(&path).unwrap(), IMPORTED_MAIN);
        assert_eq!(fs::read(fixture.0.join(SIDECAR)).unwrap(), sidecar);
        assert_eq!(fs::read_to_string(fixture.0.join("scripts/chapter.rvn")).unwrap(), external);
    }

    #[test]
    fn use_path_resolution_keeps_the_same_filesystem_semantics_as_the_runtime() {
        let fixture = Fixture::new();
        fs::write(fixture.0.join("helper.rvn"), "label imported\nreturn\n").unwrap();
        let path = fixture.0.join("story.rvn");
        for raw in ["not_existing/../helper.rvn", "not_existing/../*.rvn"] {
            let source = format!("use \"{raw}\"\nlabel start\njump imported\n");
            fs::write(&path, &source).unwrap();
            assert_eq!(SourceProject::open_at(&path, &source, &[]).is_ok(), rvn_parser::parse_file_with_uses(&path).is_ok(), "do not erase a missing intermediate directory: {raw}");
        }
        fs::create_dir(fixture.0.join("empty")).unwrap();
        let source = "use \"empty/../helper.rvn\"\nlabel start\njump imported\n";
        fs::write(&path, source).unwrap();
        let project = SourceProject::open_at(&path, source, &[]).unwrap();
        assert_eq!(project.resolved_script().unwrap(), rvn_parser::parse_file_with_uses(&path).unwrap());
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
            imports: linked.project().imports_snapshot(),
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
                imports: next.imports_snapshot(),
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
