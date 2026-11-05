//! A coherent source/descriptor/graph candidate on the original Source writer.
//! No alias lookup, second transaction authority or separate Library journal.
use super::*;
impl SourceWorkspace {
    /// Publish a namespace/member/signature change together with the actual
    /// manifest and descriptor participants. Fresh IDs are admitted by this
    /// transaction itself; callers must not pre-consume a separate reservation.
    pub fn refactor_library_source1027(
        &mut self,
        source: &str,
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
            return Err(
                "Resolve original recovered edits before publishing a Library source cohort".into(),
            );
        }
        if !self.project.observes_library_inputs1027() {
            return Err(
                "Library publication requires the original scoped Source resource capture".into(),
            );
        }
        let path = self.source_path();
        let current = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        self.check_save_inputs(&path, self.project.source(), &self.expected_sidecar)?;
        let old = self
            .project
            .graphs()
            .iter()
            .map(|g| g.graph_id)
            .collect::<std::collections::BTreeSet<_>>();
        let mut fresh = graphs
            .iter()
            .filter(|g| !old.contains(&g.graph_id))
            .map(|g| g.graph_id.get())
            .collect::<Vec<_>>();
        fresh.sort_unstable();
        if fresh.windows(2).any(|pair| pair[0] == pair[1])
            || fresh
                .first()
                .is_some_and(|id| *id <= self.graph_id_high_water())
        {
            return Err(
                "Library candidate attempts to reuse an original or retired Source identity".into(),
            );
        }
        let mut files = vec![SourceRefactorFile {
            path: self.relative_source.clone(),
            before: Some(current),
            after: Some(source.into()),
        }];
        self.append_publication_files(&mut files, additional)?;
        refactor::check_files(&self.root, &files, false)?;
        let inputs = self.project.library_candidate_inputs1027(&self.root,&files)?;
        let mut next = SourceProject::open_library_at1027(
            &path,
            source.into(),
            graphs,
            inputs,
            self.project.library_inputs1027().cloned(),
            &self.root,
            None,
            false,
        )?;
        if next.graphs() != graphs {
            return Err("Library candidate graphs differ from their actual source/member/parameter projection".into());
        }
        let snapshot = Snapshot {
            asset_placements: self.placements_for_graphs(next.graphs())?,
            graph_id_high_water: Some(
                self.graph_id_high_water().max(
                    next.graphs()
                        .iter()
                        .map(|g| g.graph_id.get())
                        .max()
                        .unwrap_or(0),
                ),
            ),
            version: if self.asset_placements.is_some() {
                2
            } else {
                1
            },
            source: self.relative_source.clone(),
            source_snapshot: next.source().into(),
            graphs: next.graphs().to_vec(),
            imports: next.imports_snapshot(),
        };
        let sidecar = serde_json::to_string_pretty(&snapshot).map_err(|e| e.to_string())?;
        files.push(SourceRefactorFile {
            path: SIDECAR.into(),
            before: self.expected_sidecar.clone(),
            after: Some(sidecar.clone()),
        });
        let backup = self.publish_refactor(next.clone(), &files)?;
        let receipt = SourceRefactorReceipt {
            library_before1027:self.library_journal_readset1027(),
            library_after1027:library_inputs1027::project_readset1027(&next),
            files,
            before_graphs: self.project.graphs().to_vec(),
            after_graphs: next.graphs().to_vec(),
            backup,
        };
        self.graph_id_high_water = self
            .graph_id_high_water
            .max(snapshot.graph_id_high_water.unwrap_or(0));
        self.asset_placements = snapshot.asset_placements.clone();
        self.published_asset_placements = snapshot.asset_placements;
        next.commit_library_inputs1027();
        self.project = next;
        self.expected_sidecar = Some(sidecar);
        self.snapshot_matches_project = true;
        Ok(receipt)
    }
}
