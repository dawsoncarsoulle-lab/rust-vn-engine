//! Source-first authoring transaction. No disk writes: callers publish the
//! source and presentation together only after this loss-checked operation.
use crate::{
    import_script, reimport_script, transpile, transpile_project, validate_project_script,
    GraphDocument, GraphKind,
};
use rvn_parser::{SourceDocument, Statement};
use std::path::Path;
#[path = "source_imports.rs"]
mod imports;
pub(crate) use imports::ImportSnapshot;

#[derive(Debug, Clone)]
pub struct SourceProject {
    source: SourceDocument,
    graphs: Vec<GraphDocument>,
    context: Option<imports::Context>,
    imported_globals: std::collections::BTreeSet<String>,
}

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
        let (context, resolved) = match context {
            Some((context, resolved)) => (Some(context), Some(resolved)),
            None => (None, None),
        };
        let source = SourceDocument::parse(source).map_err(|error| error.to_string())?;
        let mut script: rvn_parser::Script = source
            .statements()
            .iter()
            .map(|statement| statement.statement.clone())
            .collect();
        let imported_globals = if let Some(context) = &context {
            let imported = crate::import::source_global_names(&context.imported_script()?);
            let authored_init = script.iter().filter(|statement| matches!(statement, Statement::Init { .. }))
                .cloned().collect();
            let owned = crate::import::source_global_names(&authored_init);
            imported.difference(&owned).cloned().collect()
        } else { std::collections::BTreeSet::new() };
        let graphs = if let Some(resolved) = &resolved {
            let mut narrative_started = false;
            for statement in &script {
                if matches!(statement, Statement::Label { .. }) { narrative_started = true; }
                if narrative_started && matches!(statement, Statement::Use { .. }) {
                    return Err("For visual editing, move all use imports before the first narrative label. No source was changed.".into());
                }
            }
            // Keep `use` and its trivia in SourceDocument. Only the physically
            // authored scopes are projected; resolved imports were validated
            // strictly before entering here, and remain read-only context.
            script.retain(|statement| !matches!(statement, Statement::Use { .. }));
            if presentation.is_empty() {
                crate::import::import_source_scopes(&script, resolved)?
            } else {
                crate::import::reimport_source_scopes(&script, resolved, presentation)?
            }
        } else {
            validate_project_script(&script, false)?;
            if presentation.is_empty() {
                import_script(&script)?
            } else {
                reimport_script(&script, presentation)?
            }
        };
        Ok(Self { source, graphs, context, imported_globals })
    }

    pub fn source(&self) -> &str {
        self.source.source()
    }
    pub fn graphs(&self) -> &[GraphDocument] {
        &self.graphs
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
        let script = if let Some(context) = &self.context { context.check(self.source())? }
            else { rvn_parser::parse(self.source()).map_err(|error| error.to_string())? };
        validate_project_script(&script, false)?;
        Ok(script)
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
        if let Some(context) = &self.context { context.check(self.source())?; }
        Ok(())
    }

    /// Invalid source leaves the last valid graph and its layout untouched.
    pub fn refresh(&mut self, source: impl Into<String>) -> Result<(), String> {
        let source = source.into();
        let next = if let Some(context) = &self.context {
            let (context, resolved) = context.resolve(&source)?;
            validate_project_script(&resolved, false)?;
            Self::open_context(source, &self.graphs, Some((context, resolved)))?
        } else { Self::open(source, &self.graphs)? };
        *self = next;
        Ok(())
    }

    /// Reconcile changed visual logic back into its original RVN scopes.
    /// Unsupported structural reorganizations are refused, never regenerated.
    pub fn apply_visual(
        &mut self,
        current_source: &str,
        edited: &[GraphDocument],
    ) -> Result<(), String> {
        if current_source != self.source() {
            return Err(
                "RVN source changed: reload or explicitly resolve the conflict before saving."
                    .into(),
            );
        }
        self.check_imports()?;
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
        if self.context.is_none() {
            transpile_project(edited)?;
        } else {
            // A scope's pins and code generation must still be valid. Its
            // external references are checked against the real resolved AST
            // below, rather than treating local scopes as a whole project.
            for graph in edited {
                transpile(graph).map_err(|error| error.to_string())?;
            }
        }
        let statements = self.source.statements();
        let mut edits = Vec::new();
        let mut declarations = String::new();
        let mut labels = String::new();
        let compile = |graph: &GraphDocument| {
            let mut graph = graph.clone();
            graph.characters.clear();
            transpile(&graph).map_err(|error| error.to_string())
        };
        let scope_range = |kind: &GraphKind| -> Result<std::ops::Range<usize>, String> {
            let index = statements
                .iter()
                .position(|statement| match (kind, &statement.statement) {
                    (GraphKind::Init, Statement::Init { .. }) => true,
                    (GraphKind::Function { name }, Statement::Function { name: other, .. }) => {
                        name == other
                    }
                    (GraphKind::Screen { name }, Statement::Screen { name: other, .. }) => {
                        name == other
                    }
                    (GraphKind::Handler { name }, Statement::Handler { name: other, .. }) => {
                        name == other
                    }
                    (GraphKind::Label { name }, Statement::Label { name: other }) => name == other,
                    _ => false,
                })
                .ok_or("Source scope not found")?;
            let mut end = index + 1;
            if matches!(kind, GraphKind::Label { .. }) {
                while end < statements.len()
                    && !matches!(statements[end].statement, Statement::Label { .. })
                {
                    end += 1;
                }
                if statements[index + 1..end].iter().any(|statement| {
                    matches!(
                        statement.statement,
                        Statement::Function { .. }
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
                let source = compile(graph)?.source;
                match graph.kind {
                    GraphKind::Function { .. }
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
                (GraphKind::Function{..},GraphKind::Function{..})|
                (GraphKind::Screen{..},GraphKind::Screen{..})|
                (GraphKind::Handler{..},GraphKind::Handler{..})) {
                return Err("Changing a source scope’s family is not a rename; no source was changed.".into());
            }
            let before = compile(old)?;
            let after = compile(graph)?;
            if before.ast == after.ast {
                continue;
            }
            let indices: Vec<_> = statements
                .iter()
                .enumerate()
                .filter_map(|(index, statement)| {
                    let matches = match (&old.kind, &statement.statement) {
                        (GraphKind::Init, Statement::Init { .. }) => true,
                        (GraphKind::Function { name }, Statement::Function { name: other, .. }) => {
                            name == other
                        }
                        (GraphKind::Screen { name }, Statement::Screen { name: other, .. }) => {
                            name == other
                        }
                        (GraphKind::Handler { name }, Statement::Handler { name: other, .. }) => {
                            name == other
                        }
                        (GraphKind::Label { name }, Statement::Label { name: other }) => {
                            name == other
                        }
                        _ => false,
                    };
                    matches.then_some(index)
                })
                .collect();
            let start = *indices.first().ok_or("Source scope not found")?;
            let mut end = start + 1;
            let mut replacement = after.source;
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
                        && !matches!(statements[end].statement, Statement::Label { .. })
                    {
                        end += 1;
                    }
                    if statements[start + 1..end].iter().any(|statement| {
                        matches!(
                            statement.statement,
                            Statement::Function { .. }
                                | Statement::Screen { .. }
                                | Statement::Handler { .. }
                                | Statement::Init { .. }
                                | Statement::Use { .. }
                        )
                    }) {
                        return Err("Move interleaved declarations outside this label before editing its visual flow; no source was changed.".into());
                    }
                    // Import makes implicit fallthrough explicit. Do not add
                    // that synthetic jump to otherwise untouched author code.
                    let original_last = &statements[end - 1].statement;
                    if before.ast.last() != Some(original_last)
                        && after.ast.last() == before.ast.last()
                    {
                        let generated = rvn_parser::parse_spanned(&replacement)
                            .map_err(|error| error.to_string())?;
                        if let Some(last) = generated.last() {
                            replacement.replace_range(last.range.clone(), "");
                        }
                    } else if end < statements.len()
                        && !matches!(
                            after.ast.last(),
                            Some(Statement::Jump { .. } | Statement::Return)
                        )
                    {
                        return Err("Connect an explicit Jump or Return before ending a source-linked label that has a following label.".into());
                    }
                }
                GraphKind::Function { .. }
                | GraphKind::Screen { .. }
                | GraphKind::Handler { .. } => {}
                _ => return Err("Unsupported source scope".into()),
            }
            // Scope replacements are sorted back-to-front to retain indices.
            edits.push((start..end, Some(before.source), replacement));
        }
        let mut trial = self.source.clone();
        edits.sort_by_key(|(range, _, _)| std::cmp::Reverse(range.start));
        for (range, baseline, replacement) in edits {
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
                .position(|statement| matches!(statement.statement, Statement::Label { .. }))
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
                .any(|statement| matches!(statement.statement, Statement::Label { .. }))
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
        trial
            .edit_character_declarations(&current, &changes)
            .map_err(|error| error.to_string())?;
        let added: String = characters
            .iter()
            .filter(|(id, _)| !original_characters.contains_key(*id))
            .map(|(id, name)| format!("\n    {}", declaration(id, name)))
            .collect();
        if !added.is_empty() {
            let current = trial.source().to_owned();
            trial
                .insert_init_characters(&current, &added)
                .map_err(|error| error.to_string())?;
        }
        let ast: Vec<Statement> = trial
            .statements()
            .iter()
            .map(|statement| statement.statement.clone())
            .collect();
        let removed: std::collections::BTreeSet<_> = original_characters
            .keys()
            .filter(|id| !characters.contains_key(*id))
            .map(String::as_str)
            .collect();
        let (context, checked_ast) = if let Some(context) = &self.context {
            let (context, resolved) = context.resolve(trial.source())?;
            (Some(context), resolved)
        } else { (None, ast) };
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
        let imported_globals = if let Some(context) = &context {
            let imported = crate::import::source_global_names(&context.imported_script()?);
            // The resolved AST contains imported Init as well. Ownership must
            // instead follow only Init physically present in the trial source.
            let authored_init = trial.statements().iter()
                .filter(|statement| matches!(statement.statement, Statement::Init { .. }))
                .map(|statement| statement.statement.clone()).collect();
            let owned = crate::import::source_global_names(&authored_init);
            imported.difference(&owned).cloned().collect()
        } else { std::collections::BTreeSet::new() };
        self.source = trial;
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
            | Statement::Function { body, .. }
            | Statement::Screen { body, .. }
            | Statement::Handler { body, .. }
            | Statement::While { body, .. }
            | Statement::ForEach { body, .. } => check_removed_character_references(body, removed)?,
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
