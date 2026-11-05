//! A callable refactor publishes source, presentation and menu references as
//! one recoverable transaction. Its receipt also supports exact CAS Undo.
use super::*;
pub(super) const REFACTOR_JOURNAL: &str = ".rvn-refactor.transaction.json";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SourceRefactorFile {
    pub path: String,
    pub before: Option<String>,
    pub after: Option<String>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct SourceRefactorReceipt {
    pub(super) library_before1027:Option<library_inputs1027::LibraryReadset1027>,
    pub(super) library_after1027:Option<library_inputs1027::LibraryReadset1027>,
    pub files: Vec<SourceRefactorFile>,
    pub before_graphs: Vec<GraphDocument>,
    pub after_graphs: Vec<GraphDocument>,
    pub backup: PathBuf,
}
#[path="source_recovery_transition1025.rs"]
mod recovery_transition1025;
use recovery_transition1025::JournalRecovery1025;
pub use recovery_transition1025::{CauseBeforeCommit1031,CleanupWarning1031,CleanupStatus1031,VerifiedCommitted1031,SourceLibraryCompletePublication1031,SourceLibraryCompleteCommitted1031};

#[derive(Serialize, Deserialize)]
struct RefactorJournal {
    #[serde(default,skip_serializing_if="Option::is_none")]
    library_before1027:Option<library_inputs1027::LibraryReadset1027>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    library_after1027:Option<library_inputs1027::LibraryReadset1027>,
    source: String,
    graphs: Vec<GraphDocument>,
    files: Vec<SourceRefactorFile>,
    // Exact owned bytes move through the existing journal, never a second
    // recovery writer or editable tree. Absent for every legacy transaction.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    recovery: Option<JournalRecovery1025>,
}

fn reserved_target(relative: &str) -> bool {
    let key = if cfg!(windows) {
        relative.replace('\\', "/").to_lowercase()
    } else {
        relative.into()
    };
    key == JOURNAL
        || key == REFACTOR_JOURNAL
        || key == RECOVERY
        || key == ".rvn-authoring.lock"
        || key.starts_with(".rvn-backups")
}
pub(super) fn target(root: &Path, relative: &str) -> Result<PathBuf, String> {
    if reserved_target(relative) {
        return Err("A refactor cannot replace an authoring journal or recovery.".into());
    }
    let path = Path::new(relative);
    if relative.is_empty()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err("Unsafe file path in refactor metadata".into());
    }
    let path = root.join(path);
    let parent = path
        .parent()
        .ok_or("Missing refactor parent")?
        .canonicalize()
        .map_err(|error| error.to_string())?;
    if !parent.starts_with(root) {
        return Err("A refactor target escapes the project folder.".into());
    }
    if fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        return Err("A refactor does not follow symbolic links.".into());
    }
    let path = parent.join(path.file_name().ok_or("Missing refactor file name")?);
    let resolved = path.canonicalize().unwrap_or(path);
    let relative = resolved
        .strip_prefix(root)
        .map_err(|_| "A refactor target escapes the project folder.")?
        .to_string_lossy();
    if reserved_target(&relative) {
        return Err("A refactor cannot replace an authoring journal, lock or recovery.".into());
    }
    Ok(resolved)
}
pub(super) fn check_files(
    root: &Path,
    files: &[SourceRefactorFile],
    recovering: bool,
) -> Result<(), String> {
    let mut paths = std::collections::BTreeSet::new();
    for file in files {
        let path = target(root, &file.path)?;
        let key = if cfg!(windows) {
            path.to_string_lossy().to_lowercase()
        } else {
            path.to_string_lossy().into_owned()
        };
        if !paths.insert(key) {
            return Err("A refactor lists the same file more than once.".into());
        }
        if fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
            return Err("A refactor does not follow symbolic links.".into());
        }
        let current = read_optional(&path)?;
        if current != file.before && (!recovering || current != file.after) {
            return Err(format!(
                "Refactor conflict on {}. Open changes and backups were retained.",
                path.display()
            ));
        }
    }
    Ok(())
}
fn replace(path: &Path, text: &Option<String>) -> Result<(), String> {
    match text {
        Some(text) => atomic_write(path, text),
        None => match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.to_string()),
        },
    }
}
fn validate_apply_header1031(root:&Path,journal:&RefactorJournal,recovering:bool)->Result<(),String>{
    check_files(root, &journal.files, recovering)?;
    if journal.library_before1027.is_some()||journal.library_after1027.is_some(){library_inputs1027::journal_inputs1027(root,&journal.files,journal.library_before1027.as_ref(),journal.library_after1027.as_ref())?;}
    let after_sidecar=journal.files.iter().find(|file|file.path==SIDECAR).and_then(|file|file.after.as_deref()).ok_or("Source group lost its after-sidecar")?;
    let after_snapshot:Snapshot=serde_json::from_str(after_sidecar).map_err(|error|error.to_string())?;
    validate_snapshot_editorial(&after_snapshot)?;
    if let Some(before)=journal.files.iter().find(|file|file.path==SIDECAR).and_then(|file|file.before.as_deref()){
        let old:Snapshot=serde_json::from_str(before).map_err(|error|error.to_string())?;
        validate_snapshot_editorial(&old)?;
        if old.asset_placements.is_some()&&after_snapshot.asset_placements.is_none(){return Err("Source group may not downgrade adopted asset placement metadata".into());}
    }
    if after_snapshot.version==2&&after_snapshot.graphs!=journal.graphs{return Err("Source group graph snapshot differs from its sidecar".into());}
    Ok(())
}
fn apply_typed_verified1031(root:&Path,journal:&RefactorJournal,recovering:bool,original:Box<recovery_transition1025::PreparedCommit1031>,execution:&recovery_transition1025::Execution1031,cleanup_only:bool)->Result<VerifiedCommitted1031,String>{
    validate_apply_header1031(root,journal,recovering)?;
    let Some(JournalRecovery1025::Transition(transition))=&journal.recovery else{return Err("Verified publication requires the actual typed journal, never a legacy archive".into())};
    recovery_transition1025::execute_verified1031(root,journal,transition,recovering,original,execution,cleanup_only)
}
fn cleanup_typed_journal1031(root:&Path,expected:&VerifiedCommitted1031,execution:&recovery_transition1025::Execution1031)->Result<VerifiedCommitted1031,String>{
    if root!=expected.root1031(){return Err("Cleanup proof belongs to another original Source root".into());}
    let wire=read_optional(&root.join(REFACTOR_JOURNAL))?.ok_or("The original typed journal is absent")?;
    if wire!=expected.original_journal1031(){return Err("The exact original committed journal was replaced; cleanup refused".into());}
    let journal:RefactorJournal=serde_json::from_str(&wire).map_err(|e|e.to_string())?;
    let original=Box::new(recovery_transition1025::PreparedCommit1031::prepare(root,&journal.source,wire));
    apply_typed_verified1031(root,&journal,true,original,execution,true)
}
fn apply(root: &Path, journal: &RefactorJournal, recovering: bool) -> Result<(), String> {
    validate_apply_header1031(root,journal,recovering)?;
    if let Some(JournalRecovery1025::Transition(transition))=&journal.recovery{
        return recovery_transition1025::execute1025(root,journal,transition,recovering);
    }
    if let Some(expected) = journal.recovery.as_ref().and_then(JournalRecovery1025::legacy) {
        let owned: Recovery = serde_json::from_str(expected)
            .map_err(|error| format!("Invalid Widget recovery in group journal: {error}"))?;
        if !owned.enums.is_empty(){return Err("Legacy archive cannot discard typed Enum recovery".into());}
        if !owned.libraries.is_empty(){return Err("Legacy archive cannot discard typed Library recovery".into());}
        let before_source = journal
            .files
            .iter()
            .find(|file| file.path == journal.source)
            .and_then(|file| file.before.as_deref())
            .ok_or("Widget journal has no source baseline")?;
        let before_sidecar = journal
            .files
            .iter()
            .find(|file| file.path == SIDECAR)
            .and_then(|file| file.before.as_deref())
            .ok_or("Widget journal has no presentation baseline")?;
        let baseline: Snapshot =
            serde_json::from_str(before_sidecar).map_err(|error| error.to_string())?;
        if validate_recovery_editorial(&owned).is_err()
            || owned.source != journal.source
            || owned.baseline_source != before_source
            || owned.baseline_sidecar.as_deref() != Some(before_sidecar)
            || owned.widgets.is_empty() && owned.widget_memberships.is_empty() && owned.asset_placements_after.is_none()
                && validated_recovery_high_water(&owned,validated_snapshot_high_water(&baseline)?)?<=validated_snapshot_high_water(&baseline)?
        {
            return Err("Widget group journal does not own its original recovery baseline".into());
        }
        let reserved=validated_recovery_high_water(&owned,validated_snapshot_high_water(&baseline)?)?;
        let published=journal.files.iter().find(|file|file.path==SIDECAR).and_then(|file|file.after.as_deref()).ok_or("Source publication lost its real after-sidecar")?;
        let published:Snapshot=serde_json::from_str(published).map_err(|error|error.to_string())?;
        if validated_snapshot_high_water(&published)?<reserved{return Err("Source publication would drop an owned reserved identity".into());}
        widget_recovery::validate(
            &owned.widgets,
            &owned.graphs,
            &baseline.graphs,
            &journal.source,
        )?;
        widget_recovery::validate(
            &owned.widgets,
            &journal.graphs,
            &baseline.graphs,
            &journal.source,
        )?;
        widget_recovery::validate_memberships(&owned.widget_memberships,&owned.widgets,&owned.graphs,&baseline.graphs,&journal.source)?;
        widget_recovery::validate_memberships(&owned.widget_memberships,&owned.widgets,&journal.graphs,&baseline.graphs,&journal.source)?;
        if let Some(placements)=owned.asset_placements_after.as_ref(){
            let ids=journal.graphs.iter().map(|graph|graph.graph_id).collect::<std::collections::BTreeSet<_>>();
            let retained=SourceAssetPlacements::from_entries(placements.entries().filter(|row|ids.contains(&row.graph_id)).cloned())?;
            if Some(&retained)!=published.asset_placements.as_ref(){return Err("Source publication would drop an owned asset placement participant".into());}
        }
        widget_recovery::publication_participants(
            &SourceRecoveryBatch {
                graphs: owned.graphs,
                widgets: owned.widgets,
                asset_placements:owned.asset_placements_after,
                widget_memberships: owned.widget_memberships,
                enums: owned.enums,
            },
            &journal.files,
        )?;
        let current = read_optional(&root.join(RECOVERY))?;
        if current.as_ref().is_some_and(|current| current != expected) {
            return Err(
                "Another writer changed Widget recovery; durable journal and all files retained"
                    .into(),
            );
        }
        // Absence is legal ONLY after all files reached their after state:
        // this is a resumed archive-complete / journal-cleanup interruption.
        if current.is_none()
            && journal.files.iter().any(|file| {
                target(root, &file.path)
                    .and_then(|path| read_optional(&path))
                    .ok()
                    != Some(file.after.clone())
            })
        {
            return Err("Widget recovery disappeared before its complete publication".into());
        }
    }
    let mut published = Vec::<&SourceRefactorFile>::new();
    for file in &journal.files {
        let path = target(root, &file.path)?;
        let current = read_optional(&path)?;
        if current == file.after {
            continue;
        }
        let result = if current != file.before {
            Err(format!("Refactor conflict on {}", path.display()))
        } else {
            replace(&path, &file.after)
        };
        if let Err(error) = result {
            // Compensate only bytes still owned by this exact transaction.
            // If another writer intervened, retain its edit and the journal.
            let mut failures = Vec::new();
            for previous in published.iter().rev() {
                let path = target(root, &previous.path)?;
                if read_optional(&path)? != previous.after {
                    failures.push(path.display().to_string());
                    continue;
                }
                if let Err(reason) = replace(&path, &previous.before) {
                    failures.push(format!("{}: {reason}", path.display()));
                }
            }
            if failures.is_empty() && !recovering {
                let _ = fs::remove_file(root.join(REFACTOR_JOURNAL));
            }
            return Err(format!(
                "{error}. Refactor backups retained. {}",
                if failures.is_empty() {
                    "Published files restored.".into()
                } else {
                    format!("Recovery needed: {}", failures.join("; "))
                }
            ));
        }
        published.push(file);
    }
    if let Some(expected) = journal.recovery.as_ref().and_then(JournalRecovery1025::legacy) {
        let current = read_optional(&root.join(RECOVERY))?;
        if let Some(current) = &current {
            if current != expected {
                return Err(
                    "Widget recovery ownership changed before archive; journal retained".into(),
                );
            }
        }
        // The journal itself still owns the exact bytes when an archive-done
        // restart finds no active cache. Preserve them before deleting the
        // durable journal even if the preceding archive receipt was lost.
        let backup = root.join(".rvn-backups").join(unique_name());
        fs::create_dir_all(&backup).map_err(|error| error.to_string())?;
        atomic_write(&backup.join("recovery.json"), expected)?;
        if current.is_some() {
            // Recheck ownership immediately before removing this exact copy.
            if read_optional(&root.join(RECOVERY))?.as_ref() != Some(expected) {
                return Err("Widget recovery changed during archive; journal retained".into());
            }
            fs::remove_file(root.join(RECOVERY)).map_err(|error| error.to_string())?;
        }
    }
    fs::remove_file(root.join(REFACTOR_JOURNAL)).map_err(|error| error.to_string())?;
    Ok(())
}
pub(super) fn recover_refactor(root: &Path) -> Result<(), String> {
    let Some(contents) = read_optional(&root.join(REFACTOR_JOURNAL))? else {
        return Ok(());
    };
    let journal: RefactorJournal = serde_json::from_str(&contents)
        .map_err(|error| format!("Invalid refactor journal: {error}"))?;
    let source = journal
        .files
        .iter()
        .find(|file| file.path == journal.source)
        .and_then(|file| file.after.clone())
        .ok_or("Refactor journal has no source")?;
    let snapshot: Snapshot = serde_json::from_str(
        journal
            .files
            .iter()
            .find(|file| file.path == SIDECAR)
            .and_then(|file| file.after.as_deref())
            .ok_or("Refactor journal has no presentation")?,
    )
    .map_err(|error| error.to_string())?;
    validate_snapshot_editorial(&snapshot)?;
    // Prove every real target is still one of this journal's exact states
    // before deriving semantic resources from its AFTER participants. Partial
    // application must never reread a mixture of latest Library descriptors.
    check_files(root, &journal.files, true)?;
    let project = if journal.library_before1027.is_some()||journal.library_after1027.is_some()||SourceLibraryInputs1027::has_manifest_participant1027(&journal.files) {
        let inputs=library_inputs1027::journal_inputs1027(root,&journal.files,journal.library_before1027.as_ref(),journal.library_after1027.as_ref())?;
        SourceProject::open_library_recovery1027(&source_path(root,&journal.source)?,source,&journal.graphs,snapshot.imports,inputs)?
    } else { SourceProject::open_snapshot(
        &source_path(root, &journal.source)?,source,&journal.graphs,snapshot.imports,
    )? };
    project.check_imports()?;
    // All targets are checked before touching a single file on recovery.
    apply(root, &journal, true)
}
impl SourceWorkspace {
    pub fn refactor(
        &mut self,
        graphs: &[GraphDocument],
        additional: &[SourceRefactorFile],
    ) -> Result<SourceRefactorReceipt, String> {
        let _lock = lock(&self.root)?;
        recover(&self.root)?;
        if let Some(error) = &self.source_error {
            return Err(error.clone());
        }
        self.check_recovery_owner()?;
        if self.expected_recovery.is_some() {
            return Err("Save or discard recovered edits before renaming a declaration.".into());
        }
        let current = fs::read_to_string(self.source_path()).map_err(|error| error.to_string())?;
        self.check_save_inputs(
            &self.source_path(),
            self.project.source(),
            &self.expected_sidecar,
        )?;
        let mut next = self.project.clone();
        next.apply_visual(&current, graphs)?;
        let snapshot = Snapshot {
            asset_placements:self.placements_for_graphs(next.graphs())?,
            graph_id_high_water: Some(self.graph_id_high_water().max(next.graphs().iter().map(|g|g.graph_id.get()).max().unwrap_or(0))),
            version:if self.asset_placements.is_some(){2}else{1},
            source: self.relative_source.clone(),
            source_snapshot: next.source().into(),
            graphs: next.graphs().to_vec(),
            imports: next.imports_snapshot(),
        };
        let sidecar = serde_json::to_string_pretty(&snapshot).map_err(|error| error.to_string())?;
        let mut files = vec![
            SourceRefactorFile {
                path: self.relative_source.clone(),
                before: Some(current),
                after: Some(next.source().into()),
            },
            SourceRefactorFile {
                path: SIDECAR.into(),
                before: self.expected_sidecar.clone(),
                after: Some(sidecar.clone()),
            },
        ];
        self.append_publication_files(&mut files, additional)?;
        let staged=next.stage_library_files1027(&self.root,&self.source_path(),&files)?;
        if staged.source()!=next.source()||staged.graphs()!=next.graphs(){return Err("New Library resources require a coherent original Source/graph candidate before publication".into())}next=staged;
        let backup = self.publish_refactor(next.clone(), &files)?;
        let receipt = SourceRefactorReceipt {
            library_before1027:self.library_journal_readset1027(),
            library_after1027:library_inputs1027::project_readset1027(&next),
            files,
            before_graphs: self.project.graphs().to_vec(),
            after_graphs: next.graphs().to_vec(),
            backup,
        };
        self.graph_id_high_water=self.graph_id_high_water.max(snapshot.graph_id_high_water.unwrap_or(0));
        self.published_asset_placements=snapshot.asset_placements.clone();
        self.asset_placements=snapshot.asset_placements;
        next.commit_library_inputs1027();self.project = next;
        self.expected_sidecar = Some(sidecar);
        self.snapshot_matches_project = true;
        Ok(receipt)
    }
    pub fn replay_refactor(
        &mut self,
        receipt: &SourceRefactorReceipt,
        redo: bool,
    ) -> Result<(), String> {
        let _lock = lock(&self.root)?;
        recover(&self.root)?;
        self.check_recovery_owner()?;
        if self.expected_recovery.is_some() {
            return Err("Recovered edits must be resolved before replaying a refactor.".into());
        }
        self.check_save_inputs(
            &self.source_path(),
            self.project.source(),
            &self.expected_sidecar,
        )?;
        let mut files: Vec<_> = receipt
            .files
            .iter()
            .map(|file| {
                if redo {
                    file.clone()
                } else {
                    SourceRefactorFile {
                        path: file.path.clone(),
                        before: file.after.clone(),
                        after: file.before.clone(),
                    }
                }
            })
            .collect();
        let (original_readset,destination_readset)=if redo{(&receipt.library_before1027,&receipt.library_after1027)}else{(&receipt.library_after1027,&receipt.library_before1027)};
        if self.library_journal_readset1027()!=*original_readset{return Err("Refactor replay lost its exact original Library resource observation".into())}
        if let Some(original)=original_readset{original.validate(&self.root)?;}
        let mut replayed_high_water=self.graph_id_high_water;
        // Refactor history may predate a committed member identity. Only its
        // monotone reservation is rebased; every other sidecar field must still
        // equal the authentic historical participant. Physical CAS uses the
        // exact current bytes under this same source writer lock.
        if let Some(file)=files.iter_mut().find(|file|file.path==SIDECAR) {
            let actual=self.expected_sidecar.as_deref().ok_or("Le sidecar source est absent.")?;
            let historical=file.before.as_deref().ok_or("Le participant historique du sidecar est absent.")?;
            let mut current:serde_json::Value=serde_json::from_str(actual).map_err(|e|e.to_string())?;
            let mut expected:serde_json::Value=serde_json::from_str(historical).map_err(|e|e.to_string())?;
            let current_high=validated_snapshot_high_water(&serde_json::from_str::<Snapshot>(actual).map_err(|e|e.to_string())?)?;
            let expected_high=validated_snapshot_high_water(&serde_json::from_str::<Snapshot>(historical).map_err(|e|e.to_string())?)?;
            current.as_object_mut().ok_or("Sidecar source invalide.")?.remove("graph_id_high_water");
            expected.as_object_mut().ok_or("Sidecar historique invalide.")?.remove("graph_id_high_water");
            if current!=expected{return Err("Le sidecar a changé en dehors de sa réserve d’identités ; aucun refactor rejoué.".into());}
            let mut destination:serde_json::Value=serde_json::from_str(file.after.as_deref().ok_or("La destination du sidecar est absente.")?).map_err(|e|e.to_string())?;
            let destination_high=validated_snapshot_high_water(&serde_json::from_value::<Snapshot>(destination.clone()).map_err(|e|e.to_string())?)?;
            let history_high=receipt.before_graphs.iter().chain(&receipt.after_graphs).map(|g|g.graph_id.get()).max().unwrap_or(0);
            replayed_high_water=self.graph_id_high_water.max(current_high).max(expected_high).max(destination_high).max(history_high);
            // Without a real reservation increase, replay the historical
            // destination bytes exactly (including absent legacy fields and
            // JSON key order). Re-encoding Value would reformat an old receipt.
            if replayed_high_water>destination_high {
                destination["graph_id_high_water"]=serde_json::json!(replayed_high_water);
                file.after=Some(serde_json::to_string_pretty(&destination).map_err(|e|e.to_string())?);
            }
            file.before=Some(actual.into());
        }
        let source = files
            .iter()
            .find(|file| file.path == self.relative_source)
            .and_then(|file| file.after.clone())
            .ok_or("Missing refactor source")?;
        let graphs = if redo {
            &receipt.after_graphs
        } else {
            &receipt.before_graphs
        };
        let destination_snapshot:Snapshot=serde_json::from_str(files.iter().find(|file|file.path==SIDECAR).and_then(|file|file.after.as_deref()).ok_or("Replayed Source lost its sidecar")?).map_err(|error|error.to_string())?;
        validate_snapshot_editorial(&destination_snapshot)?;
        if self.asset_placements.is_some()&&destination_snapshot.asset_placements.is_none(){return Err("A historical refactor may not downgrade adopted Source asset placement metadata".into());}
        let mut next = if self.project.observes_library_inputs1027(){
            let inputs=library_inputs1027::journal_inputs1027(&self.root,&files,original_readset.as_ref(),destination_readset.as_ref())?;
            SourceProject::open_library_at1027(&self.source_path(),source,graphs,inputs,self.project.library_inputs1027().cloned(),&self.root,destination_snapshot.imports.clone(),false)?
        }else{SourceProject::open_at(&self.source_path(), source, graphs)?};
        self.publish_refactor(next.clone(), &files)?;
        self.expected_sidecar = files
            .iter()
            .find(|file| file.path == SIDECAR)
            .and_then(|file| file.after.clone());
        self.asset_placements=destination_snapshot.asset_placements.clone();
        self.published_asset_placements=destination_snapshot.asset_placements;
        self.graph_id_high_water=replayed_high_water;
        next.commit_library_inputs1027();self.project = next;
        self.snapshot_matches_project = self.expected_sidecar.is_some();
        Ok(())
    }
    pub(super) fn append_publication_files(
        &self,
        files: &mut Vec<SourceRefactorFile>,
        additional: &[SourceRefactorFile],
    ) -> Result<(), String> {
        let imported: std::collections::BTreeSet<_> = self
            .project
            .resolved_source_files()?
            .into_iter()
            .skip(1)
            .map(|(path, _)| path)
            .collect();
        for file in additional {
            if file.path == self.relative_source || file.path == SIDECAR {
                return Err(
                    "Additional publication files cannot replace source or presentation.".into(),
                );
            }
            let target = target(&self.root, &file.path)?;
            let target = target.canonicalize().unwrap_or(target);
            if imported.contains(&target) {
                return Err("Imported RVN sources are read-only in this authoring workspace; refactor their own source explicitly.".into());
            }
            files.push(file.clone());
        }
        self.append_library_readsets1027(files)?;
        Ok(())
    }
    pub(super) fn publish_refactor(
        &self,
        next: SourceProject,
        files: &[SourceRefactorFile],
    ) -> Result<PathBuf, String> {
        self.publish_refactor_retaining_recovery(next, files, None)
    }
    pub(super) fn publish_refactor_retaining_recovery(
        &self,
        next: SourceProject,
        files: &[SourceRefactorFile],
        recovery: Option<String>,
    ) -> Result<PathBuf, String> {
        self.project.check_imports()?;
        next.check_imports()?;
        check_files(&self.root, files, false)?;
        let parent = self.root.join(".rvn-backups");
        fs::create_dir_all(&parent).map_err(|error| error.to_string())?;
        let backup = parent.join(format!("refactor-{}", unique_name()));
        fs::create_dir(&backup).map_err(|error| error.to_string())?;
        for file in files {
            if let Some(before) = &file.before {
                let path = backup.join(&file.path);
                fs::create_dir_all(path.parent().ok_or("Missing backup folder")?)
                    .map_err(|error| error.to_string())?;
                atomic_write(&path, before)?;
            }
        }
        check_files(&self.root, files, false)?;
        let journal = RefactorJournal {
            library_before1027:self.library_journal_readset1027(),
            library_after1027:library_inputs1027::project_readset1027(&next),
            source: self.relative_source.clone(),
            graphs: next.graphs().to_vec(),
            files: files.to_vec(),
            recovery:recovery.map(JournalRecovery1025::LegacyArchive),
        };
        self.project.check_imports()?;
        next.check_imports()?;
        atomic_write(
            &self.root.join(REFACTOR_JOURNAL),
            &serde_json::to_string(&journal).map_err(|error| error.to_string())?,
        )?;
        apply(&self.root, &journal, false)?;
        Ok(backup)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let root =
                std::env::temp_dir().join(format!("rvn-callable-refactor-{}", unique_name()));
            fs::create_dir(&root).unwrap();
            fs::write(root.join("story.rvn"),"// unrelated project note\nfunction caption(value) {\n // calculation note\n return value\n}\nscreen form(){return component(\"root\",\"text\",{\"text\":caption(\"caption\")},[])}\nlabel start\n\"Value [caption(4)]\"\nreturn\nlabel untouched\n // leave this tail exact\n \"caption is ordinary text\"\nreturn\n").unwrap();
            fs::write(
                root.join("menus.rvnui"),
                "{\"source_screens\":{\"title\":\"form\"}}\n",
            )
            .unwrap();
            Self(root)
        }
        fn linked(&self) -> SourceWorkspace {
            SourceWorkspace::link(&self.0, &self.0.join("story.rvn"), &[]).unwrap()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn changed(workspace: &SourceWorkspace) -> Vec<GraphDocument> {
        let id = workspace
            .project()
            .graphs()
            .iter()
            .find(|graph| crate::callable_name(&graph.kind) == Some("caption"))
            .unwrap()
            .graph_id;
        crate::rename_callable_graphs(workspace.project().graphs(), id, "format_caption").unwrap()
    }
    #[test]
    fn publication_and_project_undo_restore_source_sidecar_and_menus_byte_exact() {
        let fixture = Fixture::new();
        let mut workspace = fixture.linked();
        let before_source = fs::read(fixture.0.join("story.rvn")).unwrap();
        let before_sidecar = fs::read(fixture.0.join(SIDECAR)).unwrap();
        let before_menu = fs::read_to_string(fixture.0.join("menus.rvnui")).unwrap();
        let graphs = changed(&workspace);
        let receipt = workspace
            .refactor(
                &graphs,
                &[SourceRefactorFile {
                    path: "menus.rvnui".into(),
                    before: Some(before_menu.clone()),
                    after: Some(
                        "{\"source_screens\":{\"title\":\"form\"},\"refactor\":true}\n".into(),
                    ),
                }],
            )
            .unwrap();
        let after_source = fs::read(fixture.0.join("story.rvn")).unwrap();
        let after_sidecar = fs::read(fixture.0.join(SIDECAR)).unwrap();
        let after_menu = fs::read(fixture.0.join("menus.rvnui")).unwrap();
        let source = std::str::from_utf8(&after_source).unwrap();
        assert!(source.contains("format_caption"));
        assert!(source.contains("// calculation note"));
        assert!(source.ends_with(
            "label untouched\n // leave this tail exact\n \"caption is ordinary text\"\nreturn\n"
        ));
        assert!(receipt.backup.join("story.rvn").exists());
        assert!(!fixture.0.join(REFACTOR_JOURNAL).exists());
        workspace.replay_refactor(&receipt, false).unwrap();
        assert_eq!(
            fs::read(fixture.0.join("story.rvn")).unwrap(),
            before_source
        );
        assert_eq!(fs::read(fixture.0.join(SIDECAR)).unwrap(), before_sidecar);
        assert_eq!(
            fs::read_to_string(fixture.0.join("menus.rvnui")).unwrap(),
            before_menu
        );
        workspace.replay_refactor(&receipt, true).unwrap();
        assert_eq!(fs::read(fixture.0.join("story.rvn")).unwrap(), after_source);
        assert_eq!(fs::read(fixture.0.join(SIDECAR)).unwrap(), after_sidecar);
        assert_eq!(fs::read(fixture.0.join("menus.rvnui")).unwrap(), after_menu);
        let reopened = SourceWorkspace::open(&fixture.0).unwrap().unwrap();
        assert!(reopened
            .project()
            .graphs()
            .iter()
            .any(|graph| crate::callable_name(&graph.kind) == Some("format_caption")));
    }
    #[test]
    fn menu_conflict_refuses_all_files_and_undo_conflict_keeps_the_published_project() {
        let fixture = Fixture::new();
        let mut workspace = fixture.linked();
        let before = fs::read(fixture.0.join("story.rvn")).unwrap();
        let sidecar = fs::read(fixture.0.join(SIDECAR)).unwrap();
        let graphs = changed(&workspace);
        let file = SourceRefactorFile {
            path: "menus.rvnui".into(),
            before: Some("stale menu".into()),
            after: Some("new menu".into()),
        };
        assert!(workspace.refactor(&graphs, &[file]).is_err());
        assert_eq!(fs::read(fixture.0.join("story.rvn")).unwrap(), before);
        assert_eq!(fs::read(fixture.0.join(SIDECAR)).unwrap(), sidecar);
        assert!(!fixture.0.join(REFACTOR_JOURNAL).exists());
        let before_menu = fs::read_to_string(fixture.0.join("menus.rvnui")).unwrap();
        let receipt = workspace
            .refactor(
                &graphs,
                &[SourceRefactorFile {
                    path: "menus.rvnui".into(),
                    before: Some(before_menu),
                    after: Some("our menu".into()),
                }],
            )
            .unwrap();
        let published = fs::read(fixture.0.join("story.rvn")).unwrap();
        fs::write(fixture.0.join("menus.rvnui"), "external writer").unwrap();
        assert!(workspace.replay_refactor(&receipt, false).is_err());
        assert_eq!(fs::read(fixture.0.join("story.rvn")).unwrap(), published);
        assert_eq!(
            fs::read_to_string(fixture.0.join("menus.rvnui")).unwrap(),
            "external writer"
        );
    }
    #[test]
    fn an_interrupted_refactor_checks_all_targets_before_recovery_and_keeps_external_edits() {
        let fixture = Fixture::new();
        let mut workspace = fixture.linked();
        let receipt = workspace.refactor(&changed(&workspace), &[]).unwrap();
        // Simulate source already replaced but presentation not yet replaced.
        let sidecar = receipt
            .files
            .iter()
            .find(|file| file.path == SIDECAR)
            .unwrap();
        replace(&fixture.0.join(SIDECAR), &sidecar.before).unwrap();
        let journal = RefactorJournal {
            source: "story.rvn".into(),
            library_before1027:receipt.library_before1027.clone(),
            library_after1027:receipt.library_after1027.clone(),
            graphs: receipt.after_graphs.clone(),
            files: receipt.files.clone(),
            recovery: None,
        };
        atomic_write(
            &fixture.0.join(REFACTOR_JOURNAL),
            &serde_json::to_string(&journal).unwrap(),
        )
        .unwrap();
        let reopened = SourceWorkspace::open(&fixture.0).unwrap().unwrap();
        assert_eq!(reopened.project().graphs(), receipt.after_graphs);
        assert!(!fixture.0.join(REFACTOR_JOURNAL).exists());
        replace(&fixture.0.join(SIDECAR), &sidecar.before).unwrap();
        fs::write(
            fixture.0.join("story.rvn"),
            "// competing writer\nlabel start\nreturn\n",
        )
        .unwrap();
        atomic_write(
            &fixture.0.join(REFACTOR_JOURNAL),
            &serde_json::to_string(&journal).unwrap(),
        )
        .unwrap();
        let expected = fs::read(fixture.0.join(SIDECAR)).unwrap();
        assert!(SourceWorkspace::open(&fixture.0).is_err());
        assert_eq!(fs::read(fixture.0.join(SIDECAR)).unwrap(), expected);
        assert!(fixture.0.join(REFACTOR_JOURNAL).exists());
        assert!(fs::read_to_string(fixture.0.join("story.rvn"))
            .unwrap()
            .contains("competing writer"));
    }
}
