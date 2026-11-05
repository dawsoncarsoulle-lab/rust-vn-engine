//! The same SourceProject projection with explicit admitted nominal resources.
//! No file writer, editable registry or imported-source ownership is introduced.
use super::*;
use std::sync::Arc;
pub(super) struct LibraryVisual1027<'a> {
    pub(super) before: &'a crate::LibraryGraphResources1027<'a>,
    pub(super) after: &'a crate::LibraryGraphResources1027<'a>,
    pub(super) before_graphs: &'a [GraphDocument],
    pub(super) after_graphs: &'a [GraphDocument],
}
impl LibraryVisual1027<'_> {
    pub(super) fn compile(
        &self,
        graph: &GraphDocument,
        before: bool,
    ) -> Result<crate::TranspiledScript, crate::TranspileError> {
        let resources = if before { self.before } else { self.after };
        let graphs = if before {
            self.before_graphs
        } else {
            self.after_graphs
        };
        crate::codegen::transpile_library_graph1027(graph, graphs, resources).map(|v| {
            crate::TranspiledScript {
                source: v.source,
                ast: v.ast,
            }
        })
    }
    pub(super) fn refresh(
        &self,
        authored: &rvn_parser::Script,
        complete: &rvn_parser::Script,
        ordinary: &[GraphDocument],
    ) -> Result<Vec<GraphDocument>, String> {
        if authored != complete {
            return Err(
                "Scoped Library visual import requires the original read-only import owner mapping"
                    .into(),
            );
        }
        let scopes = library_scopes1027(authored);
        let resources = crate::LibraryGraphResources1027::prepare(&scopes, self.after.assets())?;
        let mut previous = self
            .after_graphs
            .iter()
            .filter(|g| {
                g.library1027
                    .as_ref()
                    .and_then(|m| m.owner.as_ref())
                    .is_some()
            })
            .cloned()
            .collect::<Vec<_>>();
        previous.extend_from_slice(ordinary);
        crate::import_library_program1027(authored, &resources, &previous)
    }
}
fn library_scopes1027(
    script: &rvn_parser::Script,
) -> Vec<rvn_parser::function_library_program1023::FunctionLibraryScope> {
    script
        .iter()
        .filter_map(|s| {
            if let Statement::FunctionLibrary { scope } = s {
                Some(scope.clone())
            } else {
                None
            }
        })
        .collect()
}
impl SourceProject {
    pub(super) fn apply_library_visual1027(
        &mut self,
        current: &str,
        edited: &[GraphDocument],
        errors: &mut Vec<SourceGraphError>,
    ) -> Result<(), String> {
        if current != self.source() {
            return Err("RVN source changed before scoped Library preparation".into());
        }
        self.check_imports()?;
        crate::text_owner::validate_text_owner_edit(&self.graphs, edited)?;
        crate::entry_parameter_identity1003::validate_parameter_identity_edit(
            &self.graphs,
            edited,
        )?;
        if self.has_imports() {
            return Err("Scoped Library changes require the original read-only imported owner mapping; no imported file was projected writable".into());
        }
        let mut coherent = edited.to_vec();
        variable_fragments::check(&self.graphs, &coherent, &[], &[])?;
        variable_declarations::reconcile(&self.graphs, &mut coherent, &[], &[])?;
        let edited = coherent.as_slice();
        let assets = self
            .library_assets1027
            .as_ref()
            .ok_or("Scoped Library source resources are absent")?;
        let before_script = self
            .source
            .statements()
            .iter()
            .map(|s| s.statement.clone())
            .collect::<rvn_parser::Script>();
        let before_scopes = library_scopes1027(&before_script);
        let before = crate::LibraryGraphResources1027::prepare(&before_scopes, assets)?;
        let emitted = crate::transpile_library_project1027(edited, &before)?;
        let after_scopes = library_scopes1027(&emitted.ast);
        let after = crate::LibraryGraphResources1027::prepare(&after_scopes, assets)?;
        let emitted_doc = SourceDocument::parse_with_context(emitted.source, emitted.context)
            .map_err(|e| e.to_string())?;
        let mut source = self.source.clone();
        let replacements = source
            .statements()
            .iter()
            .enumerate()
            .filter_map(|(i, s)| {
                if let Statement::FunctionLibrary { scope } = &s.statement {
                    Some((i, scope.library().clone()))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        for (index, id) in replacements.into_iter().rev() {
            let actual=emitted_doc.statements().iter().find(|s|matches!(&s.statement,Statement::FunctionLibrary{scope}if scope.library()==&id)).ok_or("Visual edits may not discard an original Library owner; use its typed Source transaction")?;
            let replacement = &emitted_doc.source()[actual.range.clone()];
            let baseline = source.source()[source.statements()[index].range.clone()].to_owned();
            let captured = source.source().to_owned();
            source
                .replace_statement_range_from_baseline(
                    index..index + 1,
                    &captured,
                    Some(&baseline),
                    replacement,
                )
                .map_err(|e| e.to_string())?;
        }
        // New/retired namespace membership and descriptors are published by the
        // explicit refactor transaction, never inferred from a visual member list.
        if before_scopes
            .iter()
            .map(|s| s.library())
            .collect::<std::collections::BTreeSet<_>>()
            != after_scopes.iter().map(|s| s.library()).collect()
        {
            return Err("Library namespace creation/removal requires its original descriptor/index Source transaction".into());
        }
        let mut ordinary = self.clone();
        ordinary.source = source;
        ordinary.graphs.retain(|g| {
            g.library1027
                .as_ref()
                .and_then(|m| m.owner.as_ref())
                .is_none()
        });
        if let Some(context) = &ordinary.context {
            let (next, _) = context.resolve(ordinary.source())?;
            ordinary.context = Some(next);
        }
        let edited_ordinary = edited
            .iter()
            .filter(|g| {
                g.library1027
                    .as_ref()
                    .and_then(|m| m.owner.as_ref())
                    .is_none()
            })
            .cloned()
            .collect::<Vec<_>>();
        let context = LibraryVisual1027 {
            before: &before,
            after: &after,
            before_graphs: &self.graphs,
            after_graphs: edited,
        };
        let captured = ordinary.source().to_owned();
        ordinary.apply_visual_inner_library1027(
            &captured,
            &edited_ordinary,
            errors,
            Some(&context),
        )?;
        let final_script = ordinary
            .source
            .statements()
            .iter()
            .map(|s| s.statement.clone())
            .collect::<rvn_parser::Script>();
        let final_scopes = library_scopes1027(&final_script);
        let final_resources = crate::LibraryGraphResources1027::prepare(&final_scopes, assets)?;
        let cohort = edited
            .iter()
            .map(|graph| {
                ordinary
                    .graphs
                    .iter()
                    .find(|g| g.graph_id == graph.graph_id)
                    .unwrap_or(graph)
                    .clone()
            })
            .collect::<Vec<_>>();
        crate::transpile_library_project1027(&cohort, &final_resources)?;
        ordinary.graphs = cohort;
        *self = ordinary;
        Ok(())
    }

    /// Build an authored v5 Save candidate from original lexical ranges.
    /// The runtime exporter remains responsible for its own transport footer.
    pub(crate) fn prepare_library_authored_current1031(
        &self,
        compiled: &crate::PreparedLibraryCurrentCohort1027,
        retired: &[&rvn_parser::function_library_asset1023::FunctionLibraryId],
    ) -> Result<(String, Vec<rvn_parser::function_library_program1023::FunctionLibraryScope>), String> {
        self.check_imports()?;
        if self.has_imports() {
            return Err("Complete Library Source Save requires its original read-only imported owner mapping".into());
        }
        let edited = compiled.graphs();
        crate::text_owner::validate_text_owner_edit(&self.graphs, edited)?;
        crate::entry_parameter_identity1003::validate_parameter_identity_edit(&self.graphs, edited)?;
        let before_script = self.source.statements().iter()
            .map(|statement| statement.statement.clone()).collect::<rvn_parser::Script>();
        let before_scopes = library_scopes1027(&before_script);
        let before_assets = self.library_assets1027.as_deref().unwrap_or(&[]);
        let before = crate::LibraryGraphResources1027::prepare(&before_scopes, before_assets)?;
        let after = crate::LibraryGraphResources1027::prepare(compiled.scopes(), compiled.assets())?;
        let narrative = if edited.iter().any(|graph| graph.narrative.is_some()) {
            NarrativeContext::ExplicitScenes
        } else {
            self.narrative_context()
        };
        let generated = SourceDocument::parse_with_context(compiled.source(), narrative)
            .map_err(|error| error.to_string())?;
        let scope_text = |owner: &rvn_parser::function_library_asset1023::FunctionLibraryId| {
            generated.statements().iter().find_map(|statement| match &statement.statement {
                Statement::FunctionLibrary { scope } if scope.library() == owner => {
                    Some(&generated.source()[statement.range.clone()])
                }
                _ => None,
            })
        };
        let mut source = self.source.clone();
        for (index, statement) in self.source.statements().iter().enumerate().rev() {
            let Statement::FunctionLibrary { scope } = &statement.statement else { continue; };
            let replacement = if let Some(replacement) = scope_text(scope.library()) {
                replacement
            } else if retired.contains(&scope.library()) {
                ""
            } else {
                return Err("An original Library definition may only be removed by its typed retirement".into());
            };
            let current = source.source().to_owned();
            source.replace_statement_range_preserving_trivia(index..index + 1, &current, replacement)
                .map_err(|error| error.to_string())?;
        }
        let mut inserted = String::new();
        for scope in compiled.scopes() {
            if before_scopes.iter().any(|old| old.library() == scope.library()) { continue; }
            let text = scope_text(scope.library()).ok_or("An admitted new Library definition was not emitted")?;
            inserted.push_str(text);
            inserted.push('\n');
        }
        if !inserted.is_empty() {
            let index = source.statements().iter().position(|statement| {
                matches!(statement.statement, Statement::Label { .. } | Statement::SceneLabel { .. })
            }).unwrap_or(source.statements().len());
            let current = source.source().to_owned();
            source.replace_statement_range_preserving_trivia(index..index, &current, &inserted)
                .map_err(|error| error.to_string())?;
        }

        // Reuse the real Source visual patcher for ordinary graphs. This
        // preserves original lexical scope order and keeps implicit
        // fallthrough spelling tied to its actual authored neighbour.
        let mut ordinary = self.clone();
        ordinary.source = source;
        ordinary.graphs.retain(|graph| graph.library1027.as_ref()
            .and_then(|metadata| metadata.owner.as_ref()).is_none());
        let edited_ordinary = edited.iter().filter(|graph| graph.library1027.as_ref()
            .and_then(|metadata| metadata.owner.as_ref()).is_none()).cloned().collect::<Vec<_>>();
        if let Some(context) = &ordinary.context {
            let (next, _) = context.resolve(ordinary.source())?;
            ordinary.context = Some(next);
        }
        if ordinary.graphs != edited_ordinary {
            let resources = LibraryVisual1027 {
                before: &before,
                after: &after,
                before_graphs: &self.graphs,
                after_graphs: edited,
            };
            let current = ordinary.source().to_owned();
            ordinary.apply_visual_inner_library1027(&current, &edited_ordinary, &mut Vec::new(), Some(&resources))?;
        }
        let authored = ordinary.source.statements().iter()
            .map(|statement| statement.statement.clone()).collect::<rvn_parser::Script>();
        let scopes = library_scopes1027(&authored);
        crate::LibraryGraphResources1027::prepare(&scopes, compiled.assets())?;
        Ok((ordinary.source().to_owned(), scopes))
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn open_library_at1027(
        path: &Path,
        source: String,
        presentation: &[GraphDocument],
        inputs: Option<Arc<crate::SourceLibraryInputs1027>>,
        fences: Option<Arc<crate::SourceLibraryInputs1027>>,
        root: &Path,
        snapshot: Option<ImportSnapshot>,
        native_preparation: bool,
    ) -> Result<Self, String> {
        let (context, resolved) = if let Some(snapshot) = snapshot {
            imports::Context::cached(path, &source, snapshot)?
        } else {
            imports::Context::live(path, &source)?
        };
        validate_project_script(&resolved, false)?;
        let assets = inputs.as_ref().map(|v| Arc::from(v.assets()));
        let mut project = Self::open_context_mode_libraries1027(
            source,
            presentation,
            Some((context, resolved)),
            native_preparation,
            assets,
        )?;
        project.library_inputs1027 = inputs;
        project.library_fences1027 = fences;
        project.library_root1027 = Some(root.into());
        Ok(project)
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn check_library_inputs1027(&self) -> Result<(), String> {
        if let Some(fences) = &self.library_fences1027 {
            return fences.validate_original();
        }
        if let Some(root) = &self.library_root1027 {
            if !crate::SourceLibraryInputs1027::manifest_absent1027(root)?
            {
                return Err("The previously absent Source resource manifest changed".into());
            }
        }
        Ok(())
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn library_inputs1027(&self) -> Option<&Arc<crate::SourceLibraryInputs1027>> {
        self.library_inputs1027.as_ref()
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn observes_library_inputs1027(&self) -> bool {
        self.library_root1027.is_some()
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn stage_library_files1027(
        &self,
        root: &Path,
        path: &Path,
        files: &[crate::SourceRefactorFile],
    ) -> Result<Self, String> {
        if !self.observes_library_inputs1027() {
            return Ok(self.clone());
        }
        let inputs = self.library_candidate_inputs1027(root,files)?;
        Self::open_library_at1027(
            path,
            self.source().into(),
            &self.graphs,
            inputs,
            self.library_fences1027.clone(),
            root,
            self.imports_snapshot(),
            self.native_preparation,
        )
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn library_candidate_inputs1027(&self,root:&Path,files:&[crate::SourceRefactorFile])->Result<Option<Arc<crate::SourceLibraryInputs1027>>,String>{
        if crate::SourceLibraryInputs1027::has_manifest_participant1027(files){return crate::SourceLibraryInputs1027::from_transaction1027(root,files,true).map(|v|v.map(Arc::new))}
        if self.library_inputs1027.as_ref().is_some_and(|input|input.manifest().has_library_index){return Err("Indexed Library candidate lost its original manifest participant".into())}
        // No nominal namespace exists. Preserve the exact original readonly
        // no-index observation, including its legacy redirect policy.
        Ok(self.library_inputs1027.clone())
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn open_library_recovery1027(
        path: &Path,
        source: String,
        presentation: &[GraphDocument],
        snapshot: Option<ImportSnapshot>,
        inputs: Option<Arc<crate::SourceLibraryInputs1027>>,
    ) -> Result<Self, String> {
        let (context, resolved) = if let Some(snapshot) = snapshot {
            imports::Context::cached(path, &source, snapshot)?
        } else {
            imports::Context::live(path, &source)?
        };
        validate_project_script(&resolved, false)?;
        let assets = inputs.as_ref().map(|v| Arc::from(v.assets()));
        // This is semantic validation of already checked journal AFTER bytes only.
        // No editor/workspace is installed and no live input is recaptured here.
        let mut project=Self::open_context_mode_libraries1027(
            source,
            presentation,
            Some((context, resolved)),
            false,
            assets,
        )?;
        // Frozen journal data is retained for validating typed recovery records.
        // No root/fence is installed: this temporary projection is not a live
        // Source observation and cannot publish or recapture latest resources.
        project.library_inputs1027=inputs;Ok(project)
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn commit_library_inputs1027(&mut self) {
        self.library_fences1027 = self.library_inputs1027.clone();
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn copy_library_observation1027(&mut self, old: &Self) {
        self.library_root1027 = old.library_root1027.clone();
        self.library_inputs1027 = old.library_inputs1027.clone();
        self.library_fences1027 = old.library_fences1027.clone();
    }
}

#[cfg(test)]
#[path="source_widget_group_stage_tests1031.rs"]
mod widget_group_stage1031;
