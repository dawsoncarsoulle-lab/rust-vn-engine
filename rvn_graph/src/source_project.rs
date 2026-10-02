//! Source-first authoring transaction. No disk writes: callers publish the
//! source and presentation together only after this loss-checked operation.
use crate::{
    import_script, reimport_script, transpile, transpile_project, validate_project_script,
    GraphDocument, GraphKind,
};
use rvn_parser::{SourceDocument, Statement};

#[derive(Debug, Clone)]
pub struct SourceProject {
    source: SourceDocument,
    graphs: Vec<GraphDocument>,
}

impl SourceProject {
    pub fn open(source: impl Into<String>, presentation: &[GraphDocument]) -> Result<Self, String> {
        let source = SourceDocument::parse(source).map_err(|error| error.to_string())?;
        let script = source
            .statements()
            .iter()
            .map(|statement| statement.statement.clone())
            .collect();
        validate_project_script(&script, false)?;
        let graphs = if presentation.is_empty() {
            import_script(&script)?
        } else {
            reimport_script(&script, presentation)?
        };
        Ok(Self { source, graphs })
    }

    pub fn source(&self) -> &str {
        self.source.source()
    }
    pub fn graphs(&self) -> &[GraphDocument] {
        &self.graphs
    }

    /// Invalid source leaves the last valid graph and its layout untouched.
    pub fn refresh(&mut self, source: impl Into<String>) -> Result<(), String> {
        let next = Self::open(source, &self.graphs)?;
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
        transpile_project(edited)?;
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
            if old.kind != graph.kind {
                return Err("Rename a source-linked scope in RVN first; existing references must be reconciled together.".into());
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
                    let matches = match (&graph.kind, &statement.statement) {
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
        check_removed_character_references(&ast, &removed)?;
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
        validate_project_script(&ast, false)?;
        self.source = trial;
        self.graphs = resolved;
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
