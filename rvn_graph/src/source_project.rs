//! Source-first authoring transaction. No disk writes: callers publish the
//! source and presentation together only after this loss-checked operation.
use crate::{
    import_script, reimport_script, transpile_project, validate_project_script,
    GraphDocument, GraphKind,
};
use rvn_parser::{NarrativeContext, ParsedProgram, SourceDocument, Statement};
use std::path::Path;
#[path = "source_imports.rs"]
mod imports;
pub(crate) use imports::ImportSnapshot;
#[path = "source_variable_declarations.rs"]
mod variable_declarations;
#[path = "source_variable_fragments.rs"]
mod variable_fragments;
#[path="source_library_project1027.rs"]
mod library_project1027;
#[path="source_project_presentation_order1031.rs"]
mod presentation_order1031;

#[derive(Debug, Clone)]
pub struct SourceProject {
    source: SourceDocument,
    graphs: Vec<GraphDocument>,
    context: Option<imports::Context>,
    imported_globals: std::collections::BTreeSet<String>,
    native_preparation: bool,
    library_assets1027:Option<std::sync::Arc<[rvn_parser::function_library_asset1023::FunctionLibraryAsset]>>,
    #[cfg(not(target_arch="wasm32"))]
    library_inputs1027:Option<std::sync::Arc<crate::SourceLibraryInputs1027>>,
    #[cfg(not(target_arch="wasm32"))]
    library_fences1027:Option<std::sync::Arc<crate::SourceLibraryInputs1027>>,
    #[cfg(not(target_arch="wasm32"))]
    library_root1027:Option<std::path::PathBuf>,
}

/// A failed compiler scope retains its reconciled model and stable endpoints.
/// The editor supplies its authored origin separately when routing a result.
#[derive(Debug)]
pub struct SourceGraphError {
    pub document: GraphDocument,
    pub error: crate::TranspileError,
}

/// Source/ownership conflicts have no graph errors and take precedence over
/// compilation. No source or presentation is published on either failure.
#[derive(Debug)]
pub struct SourceVisualError {
    pub message: String,
    pub graph_errors: Vec<SourceGraphError>,
}

impl std::fmt::Display for SourceVisualError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}
impl std::error::Error for SourceVisualError {}

impl SourceProject {
    pub fn open(source: impl Into<String>, presentation: &[GraphDocument]) -> Result<Self, String> {
        Self::open_context(source.into(), presentation, None)
    }

    /// The file's `use` statements are validated against their real source
    /// context. Only scopes physically authored in this file become graphs.
    pub fn open_at(path: &Path, source: impl Into<String>, presentation: &[GraphDocument]) -> Result<Self, String> {
        let source = source.into();
        let (context, resolved) = imports::Context::live(path, &source)?;
        validate_project_script(&resolved, false)?;
        Self::open_context(source, presentation, Some((context, resolved)))
    }

    pub(crate) fn open_snapshot(path: &Path, source: String, presentation: &[GraphDocument], imports: Option<ImportSnapshot>) -> Result<Self, String> {
        if let Some(imports) = imports {
            let (context, resolved) = imports::Context::cached(path, &source, imports)?;
            validate_project_script(&resolved, false)?;
            Self::open_context(source, presentation, Some((context, resolved)))
        } else {
            // Old snapshots passed the isolated strict validation before they
            // were written. Keep that baseline if an external import broke.
            let mut project = Self::open(source, presentation)?;
            project.context = Some(imports::Context::empty(path));
            Ok(project)
        }
    }

    fn open_context(source: String, presentation: &[GraphDocument], context: Option<(imports::Context, rvn_parser::Script)>) -> Result<Self, String> {
        Self::open_context_mode(source,presentation,context,false)
    }
    pub fn open_native_preparation_at(path:&Path,source:impl Into<String>,presentation:&[GraphDocument])->Result<Self,String> {
        let source=source.into();let (context,resolved)=imports::Context::live(path,&source)?;
        validate_project_script(&resolved,false)?;
        Self::open_context_mode(source,presentation,Some((context,resolved)),true)
    }
    pub(crate) fn open_native_snapshot(path:&Path,source:String,presentation:&[GraphDocument],snapshot:Option<ImportSnapshot>)->Result<Self,String> {
        let (context,resolved)=if let Some(snapshot)=snapshot {imports::Context::cached(path,&source,snapshot)?}
            else {imports::Context::live(path,&source)?};
        validate_project_script(&resolved,false)?;
        Self::open_context_mode(source,presentation,Some((context,resolved)),true)
    }
    fn open_context_mode(source: String, presentation: &[GraphDocument], context: Option<(imports::Context, rvn_parser::Script)>,native_preparation:bool) -> Result<Self, String> {
        Self::open_context_mode_libraries1027(source,presentation,context,native_preparation,None)
    }
    fn open_context_mode_libraries1027(source:String,presentation:&[GraphDocument],context:Option<(imports::Context,rvn_parser::Script)>,native_preparation:bool,library_assets1027:Option<std::sync::Arc<[rvn_parser::function_library_asset1023::FunctionLibraryAsset]>>)->Result<Self,String>{
        let (context, resolved) = match context {
            Some((context, resolved)) => (Some(context), Some(resolved)),
            None => (None, None),
        };
        let narrative = if let Some(context) = &context {
            context.narrative_context()
        } else {
            rvn_parser::parse_program(&source)?.context
        };
        let source = SourceDocument::parse_with_context(source, narrative).map_err(|error| error.to_string())?;
        let mut script: rvn_parser::Script = source
            .statements()
            .iter()
            .map(|statement| statement.statement.clone())
            .collect();
        let declared_native=crate::empty_native_init_0984::source_has_declarations(source.source(),narrative)?;
        let native_preparation = native_preparation || declared_native || crate::native_source_binding::contains_native(resolved.as_ref().unwrap_or(&script));
        let imported_globals = if let Some(context) = &context {
            let imported = crate::import::source_global_names_with_context(&context.imported_script()?, context.narrative_context());
            let authored_init = script.iter().filter(|statement| matches!(statement, Statement::Init { .. }))
                .cloned().collect();
            let owned = crate::import::source_global_names(&authored_init);
            imported.difference(&owned).cloned().collect()
        } else { std::collections::BTreeSet::new() };
        let actual_library=script.iter().any(|s|matches!(s,Statement::FunctionLibrary{..}));
        let graphs = if actual_library {
            let assets=library_assets1027.as_ref().ok_or("Library Source requires its original admitted manifest/descriptor input channel")?;
            if context.as_ref().is_some_and(|c|c.imported_script().map(|s|!s.is_empty()).unwrap_or(true)){return Err("Qualified Library imported scopes need their original read-only owner mapping; no writable projection was guessed".into())}
            let scopes=script.iter().filter_map(|s|if let Statement::FunctionLibrary{scope}=s{Some(scope.clone())}else{None}).collect::<Vec<_>>();
            let resources=crate::LibraryGraphResources1027::prepare(&scopes,assets)?;
            crate::import_library_program1027(&script,&resources,presentation)?
        }else if let Some(resolved) = &resolved {
            let mut narrative_started = false;
            for statement in &script {
                if matches!(statement, Statement::Label { .. } | Statement::SceneLabel { .. }) { narrative_started = true; }
                if narrative_started && matches!(statement, Statement::Use { .. }) {
                    return Err("For visual editing, move all use imports before the first narrative label. No source was changed.".into());
                }
            }
            // Keep `use` and its trivia in SourceDocument. Only the physically
            // authored scopes are projected; resolved imports were validated
            // strictly before entering here, and remain read-only context.
            script.retain(|statement| !matches!(statement, Statement::Use { .. }));
            if native_preparation && (declared_native||crate::native_source_binding::contains_native(&script)) {
                crate::empty_native_init_0984::reimport_in_context(source.source(),&script,presentation,Some(resolved))?
            } else if presentation.is_empty() {
                crate::import::import_source_scopes(&script, resolved)?
            } else {
                crate::import::reimport_source_scopes(&script, resolved, presentation)?
            }
        } else {
            validate_project_script(&script, false)?;
            if native_preparation && (declared_native||crate::native_source_binding::contains_native(&script)) {
                crate::empty_native_init_0984::reimport_in_context(source.source(),&script,presentation,None)?
            } else if presentation.is_empty() {
                import_script(&script)?
            } else {
                reimport_script(&script, presentation)?
            }
        };
        // Source reconstruction above owns every field and all executable
        // order. Only restore the existing editor-document presentation Vec.
        let graphs=Self::retain_graph_presentation_order(graphs,presentation)?;
        Ok(Self { source, graphs, context, imported_globals, native_preparation,library_assets1027,
            #[cfg(not(target_arch="wasm32"))]library_inputs1027:None,
            #[cfg(not(target_arch="wasm32"))]library_fences1027:None,
            #[cfg(not(target_arch="wasm32"))]library_root1027:None,
        })
    }

    pub fn source(&self) -> &str {
        self.source.source()
    }
    pub fn graphs(&self) -> &[GraphDocument] {
        &self.graphs
    }

    /// Presentation-only ordering of already reconstructed inert documents.
    /// Moves only `fresh` objects using exact original GraphId/kind/Library
    /// owner ranks; never restores an old graph field or changes Source/AST.
    /// This does not validate semantics, admit resources, publish, or return
    /// an authoring/IO/runtime capability. Callers retain all original guards.
    pub fn retain_graph_presentation_order(
        fresh: Vec<GraphDocument>, original: &[GraphDocument],
    ) -> Result<Vec<GraphDocument>, String> {
        presentation_order1031::retain_order(fresh, original)
    }
    pub fn narrative_context(&self) -> NarrativeContext {
        self.source.narrative_context()
    }

    /// A global supplied by read-only imported sources rather than an authored
    /// initialization. Runtime Set remains legal; definition editors should
    /// protect it unless the current graph has a local/parameter shadow.
    pub fn is_imported_global(&self, name: &str) -> bool {
        self.imported_globals.contains(name)
    }

    /// Compile the complete real import tree without writing the main source
    /// or projecting imported files into writable Blueprint documents.
    pub fn resolved_script(&self) -> Result<rvn_parser::Script, String> {
        Ok(self.resolved_program()?.script)
    }

    /// Complete frozen source tree and exact UTF-8 origins. Imported scopes
    /// remain read-only; this operation creates no graph or node identities.
    pub fn resolved_program(&self) -> Result<ParsedProgram, String> {
        let program = if let Some(context) = &self.context { context.check_program(self.source())? }
            else { rvn_parser::parse_program(self.source())? };
        validate_project_script(&program.script, false)?;
        Ok(program)
    }

    /// Prepare current visual edits on an isolated candidate. Neither source,
    /// presentation, identities nor diagnostics are published by this query.
    /// Imports retain their real frozen source authority and read-only status.
    pub fn global_declaration_values(&self, edited: &[GraphDocument])
        -> Result<std::collections::BTreeMap<String, crate::GlobalDeclarationValue>, String>
    {
        let mut candidate = self.clone();
        candidate.apply_visual(self.source(), edited)?;
        let program = candidate.resolved_program()?;
        Ok(crate::variable_initializers::global_declaration_values(&program.script))
    }

    /// Exact, baseline-checked sources for an isolated preview/export. Callers
    /// choose their portable staging boundary; this function writes nothing.
    pub fn resolved_source_files(&self) -> Result<Vec<(std::path::PathBuf, String)>, String> {
        self.context.as_ref().ok_or("Source file origin is required to collect RVN imports")?.source_files(self.source())
    }

    pub(crate) fn has_imports(&self) -> bool {
        self.source.statements().iter().any(|statement| matches!(statement.statement, Statement::Use { .. }))
    }

    pub(crate) fn imports_snapshot(&self) -> Option<ImportSnapshot> {
        self.context.as_ref().and_then(imports::Context::snapshot)
    }

    pub(crate) fn check_imports(&self) -> Result<(), String> {
        #[cfg(not(target_arch="wasm32"))]self.check_library_inputs1027()?;
        if let Some(context) = &self.context { context.check(self.source())?; }
        Ok(())
    }

    /// Pure preflight for an inline definition edit. Dependencies retain their
    /// existing owners; no source, presentation, history or file is changed.
    pub fn check_variable_dependencies(&self, edited: &[GraphDocument]) -> Result<(), String> {
        self.check_imports()?;
        let (imported, readonly_scopes) = self.readonly_dependency_views()?;
        variable_fragments::check(&self.graphs, edited, &imported, &readonly_scopes)
    }

    fn readonly_dependency_views(&self) -> Result<(Vec<GraphDocument>, Vec<variable_declarations::ReadonlyScope>), String> {
        let mut readonly_scopes = Vec::new();
        let imported = if let Some(context) = &self.context {
            let mut script = context.imported_script()?;
            // Validate the complete resolved closure before choosing a dependency
            // representation. Foreign parameters have no authoring owner here.
            let complete = self.resolved_script()?;
            crate::empty_native_init_0984::validate_frozen_unit_macro_data(&complete)?;
            for statement in &script {
                if let Statement::UnitFunction { body, .. } = statement {
                    if crate::native_source_binding::contains_native(body) {
                        return Err("Read-only source cannot promote a schema14 Unit definition to Native".into());
                    }
                }
            }
            crate::import::validate_frozen_program_operations(&complete, false)?;
            let (script, scopes) = variable_declarations::readonly_dependencies(&script, &complete)?;
            readonly_scopes = scopes;
            crate::import::import_source_scopes(&script, &complete)?
        } else { Vec::new() };
        Ok((imported, readonly_scopes))
    }

    /// Pure local reconstruction with complete narrative target evidence.
    /// Imports stay read-only and are resolved by labels, never GraphIds.
    /// Fragment/shared binding migrations still need a multi-owner operation.
    pub fn reconstruct_local_variable_type(&self, proposal: &crate::LocalVariableTypeProposal,
        graph: &GraphDocument, edited: &[GraphDocument]) -> Result<GraphDocument, String> {
        self.check_imports()?;
        if edited.iter().filter(|candidate| candidate.graph_id == graph.graph_id).count() != 1
            || edited.iter().find(|candidate| candidate.graph_id == graph.graph_id) != Some(graph) {
            return Err("The local type owner is absent or changed in the project snapshot".into());
        }
        let mut labels = std::collections::BTreeMap::new();
        for candidate in edited {
            if let GraphKind::Label { name } = &candidate.kind {
                if labels.insert(name.clone(), candidate.narrative == Some(crate::NarrativeMetadata::SceneEntry)).is_some() {
                    return Err(format!("Duplicate narrative target '{name}' in the project snapshot"));
                }
            }
        }
        if let Some(context) = &self.context {
            for statement in context.imported_script()? {
                let (name, scene) = match statement {
                    Statement::Label { name } => (name, false),
                    Statement::SceneLabel { name } => (name, true),
                    _ => continue,
                };
                if labels.insert(name.clone(), scene).is_some() {
                    return Err(format!("Duplicate narrative target '{name}' across authored/imported sources"));
                }
            }
        }
        let scenes = labels.into_iter().filter_map(|(name, scene)| scene.then_some(name)).collect();
        proposal.reconstruct_in_project_with_scene_entries(graph, &scenes, edited)
    }

    /// Invalid source leaves the last valid graph and its layout untouched.
    pub fn refresh(&mut self, source: impl Into<String>) -> Result<(), String> {
        // Refresh may adopt new authored/import source bytes through its old
        // path, but cannot silently adopt a different nominal asset index.
        #[cfg(not(target_arch="wasm32"))]self.check_library_inputs1027()?;
        let source = source.into();
        let mut next = if let Some(context) = &self.context {
            let (context, resolved) = context.resolve(&source)?;
            validate_project_script(&resolved, false)?;
            Self::open_context_mode_libraries1027(source, &self.graphs, Some((context, resolved)),self.native_preparation,self.library_assets1027.clone())?
        } else { Self::open_context_mode_libraries1027(source,&self.graphs,None,self.native_preparation,self.library_assets1027.clone())? };
        #[cfg(not(target_arch="wasm32"))]next.copy_library_observation1027(self);
        *self = next;
        Ok(())
    }

    /// Refresh an existing source authority without recycling a graph that was
    /// committed and later removed. This floor is a value supplied by its one
    /// SourceWorkspace, never another persistent allocator.
    pub(crate) fn refresh_with_graph_id_floor(&mut self, source:impl Into<String>, floor:u64)->Result<(),String>{
        let mut next=self.clone();
        next.refresh(source)?;
        let known:std::collections::BTreeSet<_>=self.graphs.iter().map(|graph|graph.graph_id).collect();
        let mut high=floor.max(next.graphs.iter().map(|graph|graph.graph_id.get()).max().unwrap_or(0));
        let mut moved=std::collections::BTreeMap::new();
        for graph in &mut next.graphs {
            if !known.contains(&graph.graph_id) && graph.graph_id.get()<=floor {
                if graph.library1027.as_ref().and_then(|m|m.owner.as_ref()).is_some(){return Err("A new scoped Library member reuses a committed Source identity; no nominal GraphId was rewritten".into())}
                if !graph.inspector_evaluation.is_legacy(){
                    return Err("A new Native source scope reuses an already reserved graph identity".into());
                }
                high=high.checked_add(1).ok_or("Graph identity space exhausted")?;
                let old=graph.graph_id;
                graph.graph_id=crate::GraphId::new(high);
                moved.insert(old,graph.graph_id);
            }
        }
        // Only Legacy label-reference presentation carries these JSON IDs;
        // Native source identities above are immutable and never rewritten.
        for graph in &mut next.graphs {
            for node in graph.nodes.values_mut().filter(|node|node.kind==crate::NodeKind::LabelValue){
                if let Some(crate::PropertyValue::String(target))=node.properties.get_mut("target_graph"){
                    if let Ok(old)=serde_json::from_str::<crate::GraphId>(target){
                        if let Some(new)=moved.get(&old){*target=serde_json::to_string(new).map_err(|error|error.to_string())?;}
                    }
                }
            }
        }
        crate::validate_text_owners(&next.graphs)?;
        *self=next;
        Ok(())
    }

    /// Reconcile changed visual logic back into its original RVN scopes.
    /// Unsupported structural reorganizations are refused, never regenerated.
    pub fn apply_visual(
        &mut self,
        current_source: &str,
        edited: &[GraphDocument],
    ) -> Result<(), String> {
        self.apply_visual_with_diagnostics(current_source, edited)
            .map_err(|error| error.message)
    }

    pub fn apply_visual_with_diagnostics(
        &mut self,
        current_source: &str,
        edited: &[GraphDocument],
    ) -> Result<(), SourceVisualError> {
        let mut graph_errors = Vec::new();
        self.apply_visual_inner(current_source, edited, &mut graph_errors)
            .map_err(|message| SourceVisualError { message, graph_errors })
    }

    fn apply_visual_inner(
        &mut self,
        current_source: &str,
        edited: &[GraphDocument],
        graph_errors: &mut Vec<SourceGraphError>,
    ) -> Result<(), String> {
        self.apply_visual_inner_library1027(current_source,edited,graph_errors,None)
    }
    fn apply_visual_inner_library1027(&mut self,current_source:&str,edited:&[GraphDocument],graph_errors:&mut Vec<SourceGraphError>,library:Option<&library_project1027::LibraryVisual1027<'_>>)->Result<(),String>{
        if library.is_none()&&(self.graphs.iter().any(|g|g.library1027.is_some())||edited.iter().any(|g|g.library1027.is_some())){return self.apply_library_visual1027(current_source,edited,graph_errors)}
        if current_source != self.source() {
            return Err(
                "RVN source changed: reload or explicitly resolve the conflict before saving."
                    .into(),
            );
        }
        self.check_imports()?;
        crate::text_owner::validate_text_owner_edit(&self.graphs, edited)?;
        crate::entry_parameter_identity1003::validate_parameter_identity_edit(&self.graphs, edited)?;
        for graph in edited {
            let Some(previous) = self.graphs.iter().find(|old| old.graph_id == graph.graph_id) else { continue; };
            for name in &self.imported_globals {
                if previous.variable_scope(name) == Some(crate::VariableScope::Global)
                    && !matches!(graph.variable_scope(name), Some(crate::VariableScope::Local | crate::VariableScope::Parameter))
                    && graph.variables.get(name) != previous.variables.get(name)
                {
                    return Err(format!("Imported global '{name}' has a read-only definition. Edit its own RVN source; runtime Set remains allowed. No source was changed."));
                }
            }
        }
        let mut identities = std::collections::HashSet::new();
        if edited
            .iter()
            .any(|graph| !identities.insert(graph.graph_id))
        {
            return Err("Duplicate source-linked graph identity".into());
        }
        if edited
            .iter()
            .filter(|graph| graph.kind == GraphKind::Init)
            .count()
            > 1
        {
            return Err("Source-linked projects have one shared initialization graph.".into());
        }
        let mut resolved = edited.to_vec();
        // Read-only dependencies participate in deletion checks, but never
        // become writable graph owners or acquire serialized presentation.
        let (imported, readonly_scopes) = self.readonly_dependency_views()?;
        // Declarations belong to executable source across all scope contracts.
        // Each graph retains its own compiler; a Native neighbour cannot turn
        // an unrelated Legacy scope into a Native graph.
        variable_fragments::check(&self.graphs, &resolved, &imported, &readonly_scopes)?;
        variable_declarations::reconcile(&self.graphs, &mut resolved, &imported, &readonly_scopes)?;
        // The editor can create a project character from any open graph. Its
        // declaration belongs to the project, not to that graph's label.
        let mut characters = std::collections::BTreeMap::new();
        for graph in &self.graphs {
            characters.extend(graph.characters.clone());
        }
        let original_characters = characters.clone();
        if !characters.is_empty()
            && self
                .graphs
                .iter()
                .any(|graph| graph.kind == GraphKind::Init)
            && !resolved.iter().any(|graph| graph.kind == GraphKind::Init)
        {
            return Err("Project initialization contains shared character declarations. Remove it explicitly in RVN first; no source was changed.".into());
        }
        let mut character_changes = std::collections::BTreeMap::new();
        for graph in &resolved {
            if let Some(old) = self
                .graphs
                .iter()
                .find(|old| old.graph_id == graph.graph_id)
            {
                let ids: std::collections::BTreeSet<_> = old
                    .characters
                    .keys()
                    .chain(graph.characters.keys())
                    .collect();
                for id in ids {
                    if graph.characters.get(id) == old.characters.get(id) {
                        continue;
                    }
                    let change = graph.characters.get(id).cloned();
                    if character_changes
                        .get(id)
                        .is_some_and(|existing| existing != &change)
                    {
                        return Err(format!("Conflicting character declaration: {id}"));
                    }
                    character_changes.insert(id.clone(), change);
                }
            } else {
                for (id, name) in &graph.characters {
                    if characters.get(id).is_some_and(|existing| existing != name) {
                        return Err(format!("Conflicting character declaration: {id}"));
                    }
                    characters.insert(id.clone(), name.clone());
                }
            }
        }
        for (id, name) in &character_changes {
            if let Some(name) = name {
                characters.insert(id.clone(), name.clone());
            } else {
                characters.remove(id);
            }
        }
        for graph in &mut resolved {
            graph.characters = characters.clone();
        }
        crate::resolve_label_references(&mut resolved)?;
        let edited = resolved.as_slice();
        if self.context.is_none() || library.is_some() {
            // Preserve the established project validation order and message.
            // Typed scope failures are collected only after that check fails.
            let executable = edited.iter().map(crate::document::native_authoring_execution_projection)
                .collect::<Result<Vec<_>,_>>()?;
            let validation=if let Some(context)=library{crate::transpile_library_project1027(context.after_graphs,context.after).map(|_|())}else{transpile_project(&executable).map(|_|())};
            if let Err(message) = validation {
                for graph in edited {
                    if let Err(error) = crate::unit_graph_source1003::compile_scope(graph,edited) {
                        graph_errors.push(SourceGraphError { document: graph.clone(), error });
                    }
                }
                return Err(message);
            }
        } else {
            // A scope's pins and code generation must still be valid. Its
            // external references are checked against the real resolved AST
            // below, rather than treating local scopes as a whole project.
            for graph in edited {
                if let Err(error) = crate::unit_graph_source1003::compile_scope(graph,edited) {
                    graph_errors.push(SourceGraphError { document: graph.clone(), error });
                }
            }
            if let Some(first) = graph_errors.first() {
                return Err(first.error.to_string());
            }
        }
        let statements = self.source.statements();
        let original_ast: rvn_parser::Script = statements.iter().map(|s|s.statement.clone()).collect();
        let transport_end = crate::project::project_transport_end(&original_ast).map(str::to_owned);
        let mut original_transport_verified = false;
        // The single visual Init can have a separate shared character prelude.
        // A declarative root is selected by its genuine lexical span, never by
        // the first Init or by a cache which is absent from the source.
        let parsed_init = rvn_parser::parse_with_origins(self.source(), self.narrative_context())
            .map_err(|error| error.to_string())?;
        let empty_init = crate::empty_native_init_0984::declarations(self.source(), &parsed_init)?;
        let declared_init = empty_init.first().map(|declaration| {
            statements.iter().position(|statement| statement.range == declaration.span.range)
                .ok_or("Declared Init lexical span disappeared")
        }).transpose()?;
        let native_init = self.graphs.iter().chain(edited.iter()).any(|graph|
            graph.kind == GraphKind::Init && !graph.inspector_evaluation.is_legacy());
        let authored_inits: Vec<_> = statements.iter().enumerate().filter_map(|(index, statement)| {
            let Statement::Init { body } = &statement.statement else { return None; };
            // Nonempty CharacterCreate-only blocks are the existing shared
            // registry, not the authored Native flow being edited.
            (!(native_init && !body.is_empty() && body.iter().all(|s|
                matches!(s, Statement::CharacterCreate { .. })))).then_some(index)
        }).collect();
        let selected_init = declared_init.or_else(|| match authored_inits.as_slice() {
            [index] => Some(*index), _ => None,
        });
        let mut edits = Vec::new();
        let mut declarations = String::new();
        let mut labels = String::new();
        let mut compile = |graph: &GraphDocument, project:&[GraphDocument]| {
            let mut graph = graph.clone();
            graph.characters.clear();
            let before=std::ptr::eq(project,self.graphs.as_slice());
            (if let Some(context)=library{context.compile(&graph,before)}else{crate::unit_graph_source1003::compile_scope(&graph,project)}).map_err(|error| {
                let message = error.to_string();
                graph_errors.push(SourceGraphError { document: graph, error });
                message
            })
        };
        let scope_range = |kind: &GraphKind| -> Result<std::ops::Range<usize>, String> {
            let index = if *kind == GraphKind::Init && selected_init.is_some() {
                selected_init
            } else { statements
                .iter()
                .position(|statement| match (kind, &statement.statement) {
                    (GraphKind::Init, Statement::Init { .. }) => true,
                    (GraphKind::UnitFunction { name }, Statement::UnitFunction { name: other, .. }) => { name == other }
                    (GraphKind::Function { name }, Statement::Function { name: other, .. }) => {
                        name == other
                    }
                    (GraphKind::Screen { name }, Statement::Screen { name: other, .. }) => {
                        name == other
                    }
                    (GraphKind::Handler { name }, Statement::Handler { name: other, .. }) => {
                        name == other
                    }
                    (GraphKind::Label { name }, Statement::Label { name: other } | Statement::SceneLabel { name: other }) => name == other,
                    _ => false,
                }) }
                .ok_or("Source scope not found")?;
            let mut end = index + 1;
            if matches!(kind, GraphKind::Label { .. }) {
                while end < statements.len()
                    && !matches!(statements[end].statement, Statement::Label { .. } | Statement::SceneLabel { .. })
                {
                    end += 1;
                }
                if statements[index + 1..end].iter().any(|statement| {
                    matches!(
                        statement.statement,
                        Statement::UnitFunction { .. } | Statement::Function { .. }
                            | Statement::Screen { .. }
                            | Statement::Handler { .. }
                            | Statement::Init { .. }
                            | Statement::Use { .. }
                    )
                }) {
                    return Err("Move interleaved declarations outside this label before editing its visual flow; no source was changed.".into());
                }
            }
            Ok(index..end)
        };
        for old in &self.graphs {
            if !edited.iter().any(|graph| graph.graph_id == old.graph_id) {
                edits.push((scope_range(&old.kind)?, None, String::new()));
            }
        }
        for graph in edited {
            let Some(old) = self
                .graphs
                .iter()
                .find(|old| old.graph_id == graph.graph_id)
            else {
                let source = compile(graph,edited)?.source;
                match graph.kind {
                    GraphKind::UnitFunction { .. } | GraphKind::Function { .. }
                    | GraphKind::Screen { .. }
                    | GraphKind::Handler { .. }
                    | GraphKind::Init => {
                        declarations.push_str(&source);
                        declarations.push('\n');
                    }
                    GraphKind::Label { .. } => {
                        labels.push_str(&source);
                        labels.push('\n');
                    }
                    _ => return Err("Unsupported new source scope".into()),
                }
                continue;
            };
            if old.kind != graph.kind && !matches!((&old.kind,&graph.kind),
                (GraphKind::UnitFunction{..},GraphKind::UnitFunction{..})|
                (GraphKind::Function{..},GraphKind::Function{..})|
                (GraphKind::Screen{..},GraphKind::Screen{..})|
                (GraphKind::Handler{..},GraphKind::Handler{..})|
                (GraphKind::Label{..},GraphKind::Label{..})) {
                return Err("Changing a source scopeâ€™s family is not a rename; no source was changed.".into());
            }
            let before = compile(old,&self.graphs)?;
            let after = compile(graph,edited)?;
            let declarative_contract = |graph: &GraphDocument| -> Result<Option<String>, String> {
                if crate::empty_native_init_0984::empty_graph(graph)
                    && !graph.inspector_evaluation.is_legacy() {
                    crate::empty_native_init_0984::emit(graph).map(Some)
                } else { Ok(None) }
            };
            // Ordinary trivia still does not trigger a rewrite. An explicit
            // empty-root opt-in/out changes a source contract despite AST[].
            if before.ast == after.ast
                && declarative_contract(old)? == declarative_contract(graph)? {
                continue;
            }
            let indices: Vec<_> = statements
                .iter()
                .enumerate()
                .filter_map(|(index, statement)| {
                    let matches = match (&old.kind, &statement.statement) {
                        (GraphKind::Init, Statement::Init { .. }) => selected_init.map_or(true, |selected| selected == index),
                        (GraphKind::UnitFunction { name }, Statement::UnitFunction { name: other, .. }) => { name == other }
                    (GraphKind::Function { name }, Statement::Function { name: other, .. }) => {
                            name == other
                        }
                        (GraphKind::Screen { name }, Statement::Screen { name: other, .. }) => {
                            name == other
                        }
                        (GraphKind::Handler { name }, Statement::Handler { name: other, .. }) => {
                            name == other
                        }
                        (GraphKind::Label { name }, Statement::Label { name: other } | Statement::SceneLabel { name: other }) => {
                            name == other
                        }
                        _ => false,
                    };
                    matches.then_some(index)
                })
                .collect();
            // Projection includes an empty initialization graph even when RVN
            // has no init block. Its first executable declaration creates one.
            if indices.is_empty() && old.kind == GraphKind::Init {
                declarations.push_str(&after.source);
                declarations.push('\n');
                continue;
            }
            let start = *indices.first().ok_or("Source scope not found")?;
            let mut end = start + 1;
            let mut replacement = after.source;
            let mut baseline = before.source.clone();
            match &graph.kind {
                GraphKind::Init => {
                    if indices.len() != 1 {
                        return Err("Multiple initialization blocks must be edited separately in RVN; no source was changed.".into());
                    }
                    // Reinsert immutable character declarations inside the same
                    // init block, then preserve their original token/trivia.
                    let Statement::Init { body } = &statements[start].statement else {
                        unreachable!()
                    };
                    let characters: Vec<_> = body
                        .iter()
                        .filter(|statement| matches!(statement, Statement::CharacterCreate { .. }))
                        .collect();
                    if !characters.is_empty() {
                        let mut declarations = String::new();
                        for statement in characters {
                            let Statement::CharacterCreate { id, display_name } = statement else {
                                unreachable!()
                            };
                            declarations.push_str(&format!(
                                "character.create({}, {})\n",
                                serde_json::to_string(id).unwrap(),
                                serde_json::to_string(display_name).unwrap()
                            ));
                        }
                        replacement.insert_str(
                            replacement
                                .find('{')
                                .ok_or("Initialization header missing")?
                                + 1,
                            &declarations,
                        );
                    }
                }
                GraphKind::Label { .. } => {
                    while end < statements.len()
                        && !matches!(statements[end].statement, Statement::Label { .. } | Statement::SceneLabel { .. })
                    {
                        end += 1;
                    }
                    if statements[start + 1..end].iter().any(|statement| {
                        matches!(
                            statement.statement,
                            Statement::UnitFunction { .. } | Statement::Function { .. }
                                | Statement::Screen { .. }
                                | Statement::Handler { .. }
                                | Statement::Init { .. }
                                | Statement::Use { .. }
                        )
                    }) {
                        return Err("Move interleaved declarations outside this label before editing its visual flow; no source was changed.".into());
                    }
                    let original_scope: rvn_parser::Script = statements[start..end].iter()
                        .map(|s|s.statement.clone()).collect();
                    let native_transport = !old.inspector_evaluation.is_legacy()
                        && !graph.inspector_evaluation.is_legacy()
                        && transport_end.as_ref().is_some_and(|target|
                            original_scope.len()==before.ast.len()+1
                            && original_scope[..before.ast.len()]==before.ast[..]
                            && matches!(original_scope.last(),Some(Statement::Jump{target:actual})if actual==target));
                    if native_transport {
                        // A bare project footer is outside this Native graph.
                        // Prove the WHOLE original project before preserving it;
                        // final reimport retains its complete re-export proof.
                        if !original_transport_verified {
                            let original_executable = self.graphs.iter()
                                .map(crate::document::native_authoring_execution_projection)
                                .collect::<Result<Vec<_>,_>>()?;
                            if transpile_project(&original_executable)?.ast != original_ast {
                                return Err("Native project transport requires the exact complete original project export; no source was changed.".into());
                            }
                            original_transport_verified = true;
                        }
                        let footer = format!("    jump {}\n",transport_end.as_ref().unwrap());
                        baseline.push_str(&footer);
                        // Match the actual project exporter: scheduled exits
                        // still have transport; only bare exits suppress it.
                        if !matches!(after.ast.last(),Some(Statement::Jump{..}|Statement::Return)) {
                            replacement.push_str(&footer);
                        }
                    } else {
                        // Only an actual bare Jump to the next label, absent
                        // from the original scope, can be an implicit Legacy
                        // fallthrough. A Scheduled Dialogue is never that Jump.
                        let implicit_jump = before.ast.last().filter(|last|
                            matches!((last,statements.get(end).map(|s|&s.statement)),
                                (Statement::Jump{target},Some(Statement::Label{name}|Statement::SceneLabel{name}))if target==name)
                            && original_scope.as_slice()==&before.ast[..before.ast.len()-1]);
                        if implicit_jump.is_some() && after.ast.last()==implicit_jump {
                            let generated = rvn_parser::parse_spanned_with_context(&replacement, graph.narrative_context())
                                .map_err(|error| error.to_string())?;
                            let last = generated.last().ok_or("Implicit fallthrough source Jump missing")?;
                            if Some(&last.statement)!=implicit_jump {
                                return Err("Implicit fallthrough replacement does not end in its actual Jump; no source was changed.".into());
                            }
                            replacement.replace_range(last.range.clone(), "");
                        } else if end < statements.len()
                            && !after.ast.last().is_some_and(crate::project::is_unconditional_graph_exit)
                        {
                            return Err("Connect an explicit Jump or Return before ending a source-linked label that has a following label.".into());
                        }
                    }
                }
                GraphKind::UnitFunction { .. } | GraphKind::Function { .. }
                | GraphKind::Screen { .. }
                | GraphKind::Handler { .. } => {}
                _ => return Err("Unsupported source scope".into()),
            }
            // Scope replacements are sorted back-to-front to retain indices.
            edits.push((start..end, Some(baseline), replacement));
        }
        // A visual opt-in is validated on a disposable contextual candidate.
        // The resulting complete source must still contain a real SceneLabel;
        // no synthetic marker is inserted and the original remains untouched.
        let editing_context = if self.narrative_context() == NarrativeContext::ExplicitScenes
            || edited.iter().any(|graph| graph.narrative.is_some()) {
            NarrativeContext::ExplicitScenes
        } else { NarrativeContext::Legacy };
        let mut trial = SourceDocument::parse_with_context(self.source(), editing_context)
            .map_err(|error| error.to_string())?;
        edits.sort_by_key(|(range, _, _)| std::cmp::Reverse(range.start));
        for (range, baseline, replacement) in edits {
            if crate::empty_native_init_0984::replace_definition(&mut trial, range.clone(),
                baseline.as_deref(), &replacement)? { continue; }
            let current = trial.source().to_owned();
            trial
                .replace_statement_range_from_baseline(
                    range,
                    &current,
                    baseline.as_deref(),
                    &replacement,
                )
                .map_err(|error| error.to_string())?;
        }
        if !declarations.is_empty() {
            let index = trial
                .statements()
                .iter()
                .position(|statement| matches!(statement.statement, Statement::Label { .. } | Statement::SceneLabel { .. }))
                .unwrap_or(trial.statements().len());
            let current = trial.source().to_owned();
            trial
                .replace_statement_range_preserving_trivia(index..index, &current, &declarations)
                .map_err(|error| error.to_string())?;
        }
        if !labels.is_empty() {
            if trial
                .statements()
                .iter()
                .any(|statement| matches!(statement.statement, Statement::Label { .. } | Statement::SceneLabel { .. }))
                && !matches!(
                    trial
                        .statements()
                        .last()
                        .map(|statement| &statement.statement),
                    Some(Statement::Jump { .. } | Statement::Return)
                )
            {
                return Err("Connect Jump or Return at the end of the existing story before adding a source-linked label; implicit fallthrough would change its behavior.".into());
            }
            let index = trial.statements().len();
            let current = trial.source().to_owned();
            trial
                .replace_statement_range_preserving_trivia(index..index, &current, &labels)
                .map_err(|error| error.to_string())?;
        }
        let declaration = |id: &str, name: &str| {
            format!(
                "character.create({}, {})",
                serde_json::to_string(id).unwrap(),
                serde_json::to_string(name).unwrap()
            )
        };
        let changes = character_changes
            .iter()
            .filter(|(id, _)| original_characters.contains_key(*id))
            .map(|(id, name)| (id.clone(), name.as_ref().map(|name| declaration(id, name))))
            .collect();
        let current = trial.source().to_owned();
        let parsed = rvn_parser::parse_with_origins(&current, editing_context)
            .map_err(|error| error.to_string())?;
        let declared = crate::empty_native_init_0984::declarations(&current, &parsed)?;
        let character_preludes: Vec<_> = if declared.is_empty() { Vec::new() } else {
            trial.statements().iter().enumerate().filter_map(|(index, statement)| {
                match &statement.statement {
                    Statement::Init { body } if !body.is_empty()
                        && body.iter().all(|s| matches!(s, Statement::CharacterCreate { .. })) => Some(index),
                    _ => None,
                }
            }).collect()
        };
        trial
            .edit_character_declarations(&current, &changes)
            .map_err(|error| error.to_string())?;
        // Removing the last character from a known registry prelude must not
        // create an ambiguous second empty Init. Do not touch authored roots.
        for index in character_preludes.into_iter().rev() {
            if matches!(&trial.statements()[index].statement, Statement::Init { body } if body.is_empty()) {
                let current = trial.source().to_owned();
                trial.replace_statement_range_preserving_trivia(index..index+1, &current, "")
                    .map_err(|error| error.to_string())?;
            }
        }
        let added: String = characters
            .iter()
            .filter(|(id, _)| !original_characters.contains_key(*id))
            .map(|(id, name)| format!("\n    {}", declaration(id, name)))
            .collect();
        if !added.is_empty() {
            let current = trial.source().to_owned();
            let parsed = rvn_parser::parse_with_origins(&current, editing_context)
                .map_err(|error| error.to_string())?;
            let declared = crate::empty_native_init_0984::declarations(&current, &parsed)?;
            if let Some(declaration) = declared.first() {
                let index = trial.statements().iter().position(|statement|
                    statement.range == declaration.span.range).ok_or("Declared Init lexical span disappeared")?;
                // Use the existing statement-range writer, creating the actual
                // shared prelude outside the still genuinely empty root.
                trial.replace_statement_range_preserving_trivia(index..index, &current,
                    &format!("init {{\n{added}\n}}\n"))
                    .map_err(|error| error.to_string())?;
            } else {
                trial.insert_init_characters(&current, &added)
                    .map_err(|error| error.to_string())?;
            }
        }
        let removed: std::collections::BTreeSet<_> = original_characters
            .keys()
            .filter(|id| !characters.contains_key(*id))
            .map(String::as_str)
            .collect();
        let (context, checked_ast) = if let Some(context) = &self.context {
            let (context, resolved) = context.resolve(trial.source())?;
            (Some(context), resolved)
        } else {
            // Re-derive context from the final complete source, so removing
            // its last explicit entry cannot silently authorize helper locals.
            let program = rvn_parser::parse_program(trial.source())?;
            trial = SourceDocument::parse_with_context(trial.source(), program.context)
                .map_err(|error| error.to_string())?;
            (None, program.script)
        };
        check_removed_character_references(&checked_ast, &removed)?;
        for graph in &resolved {
            for node in graph
                .nodes
                .values()
                .filter(|node| node.kind == crate::NodeKind::CharacterValue)
            {
                if let Some(crate::PropertyValue::String(id)) = node.properties.get("character") {
                    if removed.contains(id.as_str()) {
                        return Err(format!("Character '{id}' is still referenced by Blueprint node {}. Remove its references before deleting the declaration; no source was changed.", node.id.get()));
                    }
                }
            }
        }
        validate_project_script(&checked_ast, false)?;
        let final_context = NarrativeContext::from_script(&checked_ast);
        trial = SourceDocument::parse_with_context(trial.source(), final_context)
            .map_err(|error| error.to_string())?;
        let entries: std::collections::BTreeSet<_> = checked_ast.iter().filter_map(|statement| {
            if let Statement::SceneLabel { name } = statement { Some(name.as_str()) } else { None }
        }).collect();
        for graph in &mut resolved {
            if let GraphKind::Label { name } = &graph.kind {
                let metadata = if final_context == NarrativeContext::Legacy { None }
                    else { Some(if entries.contains(name.as_str()) { crate::NarrativeMetadata::SceneEntry }
                        else { crate::NarrativeMetadata::SceneFragment }) };
                graph.set_narrative_metadata(metadata)?;
            }
        }
        let imported_globals = if let Some(context) = &context {
            let imported = crate::import::source_global_names_with_context(&context.imported_script()?, context.narrative_context());
            // The resolved AST contains imported Init as well. Ownership must
            // instead follow only Init physically present in the trial source.
            let authored_init = trial.statements().iter()
                .filter(|statement| matches!(statement.statement, Statement::Init { .. }))
                .map(|statement| statement.statement.clone()).collect();
            let owned = crate::import::source_global_names(&authored_init);
            imported.difference(&owned).cloned().collect()
        } else { std::collections::BTreeSet::new() };
        // Variable definitions are derived from the authored bindings. A pin
        // default edit changes the LocalVar source without necessarily updating
        // the editor's cached definition. Refresh that registry using the same
        // contextual projection as reopen, retaining the submitted nodes,
        // pins, edges and layout rather than replacing the documents.
        let authored: rvn_parser::Script = trial.statements().iter()
            .filter(|statement| !matches!(statement.statement, Statement::Use { .. }))
            .map(|statement| statement.statement.clone()).collect();
        let declared_native=crate::empty_native_init_0984::source_has_declarations(trial.source(),final_context)?;
        let native_preparation = self.native_preparation || declared_native || crate::native_source_binding::contains_native(&authored);
        let refreshed = if let Some(context)=library{context.refresh(&authored,&checked_ast,&resolved)?}else if native_preparation && (declared_native||crate::native_source_binding::contains_native(&authored)) {
            crate::empty_native_init_0984::reimport_in_context(trial.source(),&authored,&resolved,Some(&checked_ast))?
        } else if context.is_some() {
            crate::import::reimport_source_scopes(&authored, &checked_ast, &resolved)?
        } else {
            reimport_script(&authored, &resolved)?
        };
        for graph in resolved.iter().filter(|graph|!graph.inspector_evaluation.is_legacy()) {
            // Validate the executable authoring projection against the genuine
            // resolved source. The exact full graph/sidecar identity comparison
            // below independently includes the isolated non-executable macros.
            crate::native_frame_preparation::prepare_native_authoring_graph_in_context(graph,Some(&checked_ast))
                .map_err(|error|error.to_string())?;
        }
        for graph in &mut resolved {
            let projected = refreshed.iter().find(|projected|
                projected.graph_id == graph.graph_id && projected.kind == graph.kind)
                .ok_or_else(||format!("Source variable projection lost submitted graph {:?}/{:?}; reconstructed {:?}; no source was changed.",graph.graph_id,graph.kind,refreshed.iter().map(|g|(g.graph_id,&g.kind)).collect::<Vec<_>>()))?;
            if !graph.inspector_evaluation.is_legacy() {
                let exact = projected.inspector_evaluation == graph.inspector_evaluation
                    && (!crate::empty_native_init_0984::empty_graph(graph) || projected.schema_version == graph.schema_version)
                    && crate::native_frame_preparation::native_scope(projected).map_err(|e|e.to_string())?
                        == crate::native_frame_preparation::native_scope(graph).map_err(|e|e.to_string())?
                    && projected.nodes.keys().eq(graph.nodes.keys())
                    && projected.pins.keys().eq(graph.pins.keys())
                    && projected.edges == graph.edges
                    && compile(projected,&refreshed)?.ast == compile(graph,&refreshed)?.ast;
                if !exact {
                    return Err("Final source did not reconstruct the submitted Native contract and identities; no source was changed.".into());
                }
            }
            graph.variables = projected.variables.clone();
            if let Some(context)=library {graph.refresh_narrative_binding_types_library1027(&projected.source_global_binding_types()?,&refreshed,context.after)?;}
            else{graph.refresh_narrative_binding_types_in_project(&projected.source_global_binding_types()?, &refreshed)?;}
        }
        self.source = trial;
        self.native_preparation = native_preparation;
        self.graphs = resolved;
        self.context = context;
        self.imported_globals = imported_globals;
        Ok(())
    }
}

// Check only explicitly removed declarations, so existing scripts using legacy
// implicit speakers are not rejected. Dynamic identifiers remain runtime-checked.
fn check_removed_character_references(
    script: &[Statement],
    removed: &std::collections::BTreeSet<&str>,
) -> Result<(), String> {
    for statement in script {
        let character = match statement {
            Statement::Dialogue { character_id, .. } => character_id.as_deref(),
            Statement::ShowSprite { character_id, .. }
            | Statement::HideSprite { character_id, .. }
            | Statement::MoveSprite { character_id, .. }
            | Statement::SpriteAnimate { character_id, .. }
            | Statement::SpriteStopAnimation { character_id }
            | Statement::SpriteEffect { character_id, .. } => Some(character_id.as_str()),
            Statement::CharacterCompose {
                character: rvn_parser::Expr::Str(id),
                ..
            }
            | Statement::CharacterAttributes {
                character: rvn_parser::Expr::Str(id),
                ..
            } => Some(id.as_str()),
            Statement::MethodCall { target, .. } => Some(target.as_str()),
            _ => None,
        };
        if let Some(id) = character.filter(|id| removed.contains(id)) {
            return Err(format!("Character '{id}' is still used by the story. Remove its references before deleting the declaration; no source was changed."));
        }
        match statement {
            Statement::Init { body }
            | Statement::UnitFunction { body, .. }
            | Statement::Function { body, .. }
            | Statement::Screen { body, .. }
            | Statement::Handler { body, .. }
            | Statement::While { body, .. }
            | Statement::ForEach { body, .. } | Statement::ForLoopWithBreak{body,..}=>check_removed_character_references(body,removed)?,
            Statement::Sequence{branches}=>for b in branches{check_removed_character_references(b,removed)?;},
            Statement::If {
                then_branch,
                else_branch,
                ..
            } => {
                check_removed_character_references(then_branch, removed)?;
                check_removed_character_references(else_branch, removed)?;
            }
            Statement::Choice { options } => {
                for option in options {
                    check_removed_character_references(&option.body, removed)?;
                }
            }
            Statement::Imagemap { hotspots, .. } => {
                for hotspot in hotspots {
                    check_removed_character_references(&hotspot.body, removed)?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}


#[cfg(test)]
mod native_label_exit_tests1002 {
    use crate::*;
    use rvn_parser::Statement;

    fn start(graphs:&[GraphDocument])->&GraphDocument {
        graphs.iter().find(|graph|matches!(&graph.kind,GraphKind::Label{name}if name=="start")).unwrap()
    }

    #[test]
    fn source_exit1002_real_native_return_and_jump_before_next_label_keep_source_ids_and_execution() {
        for header in ["label start","scene label start"] {
            for exit in ["return","jump other"] {
                let source=format!("{header}\n\"Before\"\n{exit}\nlabel other\n\"After\"\nreturn\n");
                let mut project=SourceProject::open(&source,&[]).unwrap();
                let mut edited=project.graphs().to_vec();
                let graph=edited.iter_mut().find(|graph|matches!(&graph.kind,GraphKind::Label{name}if name=="start")).unwrap();
                graph.enable_native_frame_preparation().unwrap();
                let authored=graph.clone();
                project.apply_visual(&source,&edited).unwrap();
                let actual=start(project.graphs());
                assert_eq!(actual.graph_id,authored.graph_id);
                assert!(actual.nodes.keys().eq(authored.nodes.keys()));
                assert!(actual.pins.keys().eq(authored.pins.keys()));
                assert_eq!(actual.edges,authored.edges);
                let compiled=prepare_native_frame_graph(actual).unwrap();
                assert!(matches!(compiled.ast.last(),Some(Statement::ScheduledConsumer{version:2,body,..})
                    if matches!(body.as_slice(),[Statement::Return])&&exit=="return"
                    || matches!(body.as_slice(),[Statement::Jump{target}]if target=="other")&&exit=="jump other"));
                let cold=SourceProject::open(project.source(),&[]).unwrap();
                let reopened=start(cold.graphs());
                assert_eq!(reopened.graph_id,authored.graph_id);
                assert!(reopened.nodes.keys().eq(authored.nodes.keys()));
                assert!(reopened.pins.keys().eq(authored.pins.keys()));
                assert_eq!(reopened.edges,authored.edges);
                // Both source forms preserve the established VM contract: a
                // top-level Return reports ReturnWithoutCall; it is not EOF.
                let raw=SourceProject::open(&source,&[]).unwrap();
                for program in [raw.resolved_program().unwrap(),cold.resolved_program().unwrap()] {
                let mut engine=rvn_core::Engine::from_program(program,rvn_core::TerminalRenderer,16).unwrap();
                assert_eq!(engine.step_until_interaction().unwrap(),Some(rvn_core::Interaction::Dialogue{character:None,text:"Before".into()}));
                engine.advance_dialogue().unwrap();
                if exit=="jump other" {
                    assert_eq!(engine.step_until_interaction().unwrap(),Some(rvn_core::Interaction::Dialogue{character:None,text:"After".into()}));
                    engine.advance_dialogue().unwrap();
                }
                assert_eq!(engine.step_until_interaction().unwrap_err().kind,
                    rvn_core::error::RuntimeErrorKind::ReturnWithoutCall);
                }
            }
        }
    }

    #[test]
    fn source_exit1002_disconnected_native_label_before_next_label_refuses_without_publication() {
        let source="label start\n\"Before\"\nreturn\nlabel other\n\"After\"\nreturn\n";
        let mut project=SourceProject::open(source,&[]).unwrap();
        let original=project.graphs().to_vec();let mut edited=original.clone();
        let graph=edited.iter_mut().find(|graph|matches!(&graph.kind,GraphKind::Label{name}if name=="start")).unwrap();
        graph.enable_native_frame_preparation().unwrap();
        let terminal=graph.nodes.values().find(|node|node.kind==NodeKind::Return).unwrap().id;
        graph.remove_node(terminal).unwrap();
        let error=project.apply_visual(source,&edited).unwrap_err();
        assert!(error.contains("explicit Jump or Return"),"{error}");
        assert_eq!(project.source(),source);assert_eq!(project.graphs(),original);
    }

    #[test]
    fn source_exit1002_current_source_conflict_precedes_native_terminal_publication() {
        let source="label start\n\"Before\"\nreturn\nlabel other\n\"After\"\nreturn\n";
        let mut project=SourceProject::open(source,&[]).unwrap();
        let original=project.graphs().to_vec();let mut edited=original.clone();
        edited.iter_mut().find(|graph|matches!(&graph.kind,GraphKind::Label{name}if name=="start")).unwrap().enable_native_frame_preparation().unwrap();
        let external=format!("{source}// External edit\n");
        let error=project.apply_visual(&external,&edited).unwrap_err();
        assert!(error.contains("RVN source changed"),"{error}");
        assert_eq!(project.source(),source);assert_eq!(project.graphs(),original);
    }
}
