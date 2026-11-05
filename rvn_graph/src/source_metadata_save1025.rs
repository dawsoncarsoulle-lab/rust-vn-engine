//! Authored metadata-only publication through the existing refactor journal.
//! Source, presentation and recovery remain byte-exact observers. No graph is
//! imported/projected, no recovery is restored/archived, no second writer exists.
use super::*;

impl SourceWorkspace {
    pub fn save_metadata_files_preserving_authoring1025(&mut self,
        graphs: &[GraphDocument], additional: &[SourceRefactorFile],
    ) -> Result<(), String> {
        let _lock = lock(&self.root)?;
        recover(&self.root)?;
        if additional.is_empty() || graphs != self.project.graphs() {
            return Err("A metadata-only save requires files and the exact published graph baseline".into());
        }
        if let Some(error) = &self.source_error { return Err(error.clone()); }
        self.check_recovery_baseline()?;
        self.check_recovery_owner()?;
        // Validate the complete existing recovery, including reservations and
        // Widget/editorial participants, but never publish their authored data.
        if let Some(batch)=self.recovery_batch()?{
            enum_recovery1025::check_legacy_metadata1025(&self.root,&batch.enums,additional)?;
        }
        let path = source_path(&self.root, &self.relative_source)?;
        let current = fs::read_to_string(&path).map_err(|error| error.to_string())?;
        let sidecar = read_optional(&self.root.join(SIDECAR))?;
        let recovery = read_optional(&self.root.join(RECOVERY))?;
        if current != self.project.source() || sidecar != self.expected_sidecar || recovery != self.expected_recovery {
            return Err("Source, presentation or recovery changed before metadata-only save".into());
        }
        let snapshot: Snapshot = serde_json::from_str(sidecar.as_deref().ok_or("Metadata-only save requires its published sidecar")?)
            .map_err(|error| error.to_string())?;
        validate_snapshot_editorial(&snapshot)?;
        if snapshot.source != self.relative_source || snapshot.source_snapshot != current || snapshot.graphs != graphs {
            return Err("Metadata-only save requires the exact admitted published presentation; refresh Source first".into());
        }
        self.check_save_inputs(&path, &current, &sidecar)?;
        let mut files = vec![
            SourceRefactorFile { path: self.relative_source.clone(), before: Some(current.clone()), after: Some(current.clone()) },
            SourceRefactorFile { path: SIDECAR.into(), before: sidecar.clone(), after: sidecar.clone() },
        ];
        self.append_publication_files(&mut files, additional)?;
        // Existing target validation rejects authoring journals, recovery,
        // locks, imports and duplicate CAS destinations before publication.
        refactor::check_files(&self.root, &files, false)?;
        self.check_recovery_owner()?;
        self.publish_refactor(self.project.clone(), &files)?;
        // The same lock still protects all normal authoring participants.
        // An external writer bypassing it is reported, never silently ACKed.
        self.check_save_inputs(&path, &current, &sidecar)?;
        if read_optional(&self.root.join(RECOVERY))? != recovery {
            return Err("Recovery changed externally during metadata-only publication; inspect the retained transaction backups".into());
        }
        Ok(())
    }
}

#[cfg(test)]
#[path="source_metadata_save_tests1025.rs"]
mod tests;
