//! Non-destructive source edits for visual authoring. This API performs no I/O:
//! callers must resolve conflicts and atomically persist successful results.
use crate::lexer::Token;
use crate::{parse, parse_spanned, SpannedStatement};
use logos::Logos;

#[derive(Debug, Clone)]
pub struct SourceDocument {
    source: String,
    statements: Vec<SpannedStatement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceEditError {
    InvalidSource(String),
    Conflict,
    StatementNotFound(usize),
    ExpectedSingleStatement,
    CommentWouldBeLost,
    SurroundingCodeWouldChange,
    EditTooComplex,
}

impl std::fmt::Display for SourceEditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidSource(error) => write!(f, "{error}"),
            Self::Conflict => write!(f, "The RVN source changed since this edit started; reload or resolve the conflict."),
            Self::StatementNotFound(index) => write!(f, "Source statement {index} was not found."),
            Self::ExpectedSingleStatement => write!(f, "A source edit must replace exactly one statement."),
            Self::CommentWouldBeLost => write!(f, "This edit would discard an internal comment; preserve it before applying the edit."),
            Self::SurroundingCodeWouldChange => write!(f, "This edit would change surrounding code; no changes were applied."),
            Self::EditTooComplex => write!(f, "This source edit is too large to reconcile safely; split it into smaller changes."),
        }
    }
}
impl std::error::Error for SourceEditError {}

impl SourceDocument {
    pub fn parse(source: impl Into<String>) -> Result<Self, SourceEditError> {
        let source = source.into();
        let statements = parse_spanned(&source)
            .map_err(|error| SourceEditError::InvalidSource(error.to_string()))?;
        Ok(Self { source, statements })
    }

    pub fn source(&self) -> &str {
        &self.source
    }
    pub fn statements(&self) -> &[SpannedStatement] {
        &self.statements
    }

    pub fn insert_init_characters(
        &mut self,
        current_source: &str,
        declarations: &str,
    ) -> Result<(), SourceEditError> {
        if current_source != self.source {
            return Err(SourceEditError::Conflict);
        }
        let parsed = parse(declarations)
            .map_err(|error| SourceEditError::InvalidSource(error.to_string()))?;
        if parsed.is_empty() {
            return Ok(());
        }
        if parsed
            .iter()
            .any(|statement| !matches!(statement, crate::Statement::CharacterCreate { .. }))
        {
            return Err(SourceEditError::SurroundingCodeWouldChange);
        }
        if let Some(index) = self
            .statements
            .iter()
            .position(|statement| matches!(statement.statement, crate::Statement::Init { .. }))
        {
            let mut replacement = self.source[self.statements[index].range.clone()].to_owned();
            let opening = Token::lexer(&replacement)
                .spanned()
                .find_map(|(token, range)| {
                    matches!(token, Ok(Token::BraceOpen)).then_some(range.end)
                })
                .ok_or(SourceEditError::SurroundingCodeWouldChange)?;
            replacement.insert_str(opening, &format!("\n{declarations}\n"));
            self.replace_statement(index, current_source, &replacement)
        } else {
            let index = self
                .statements
                .iter()
                .position(|statement| matches!(statement.statement, crate::Statement::Label { .. }))
                .unwrap_or(self.statements.len());
            self.replace_statement_range_preserving_trivia(
                index..index,
                current_source,
                &format!("init {{\n{declarations}\n}}\n"),
            )
        }
    }

    /// Edit existing shared character declarations without regenerating an
    /// initialization block. Strings/comments containing braces are not syntax.
    /// A None replacement removes only the declaration, retaining its comments.
    pub fn edit_character_declarations(
        &mut self,
        current_source: &str,
        changes: &std::collections::BTreeMap<String, Option<String>>,
    ) -> Result<(), SourceEditError> {
        if current_source != self.source {
            return Err(SourceEditError::Conflict);
        }
        let mut trial = self.clone();
        let mut found = std::collections::BTreeSet::new();
        for (id, replacement) in changes {
            if let Some(replacement) = replacement {
                let parsed = parse(replacement)
                    .map_err(|error| SourceEditError::InvalidSource(error.to_string()))?;
                if !matches!(parsed.as_slice(),[crate::Statement::CharacterCreate{id:actual,..}] if actual==id)
                {
                    return Err(SourceEditError::SurroundingCodeWouldChange);
                }
            }
        }
        for index in (0..trial.statements.len()).rev() {
            let original = &trial.statements[index];
            if !matches!(original.statement, crate::Statement::Init { .. }) {
                continue;
            }
            let fragment = &trial.source[original.range.clone()];
            let opening = Token::lexer(fragment)
                .spanned()
                .find_map(|(token, range)| {
                    matches!(token, Ok(Token::BraceOpen)).then_some(range.end)
                })
                .ok_or(SourceEditError::SurroundingCodeWouldChange)?;
            let closing = Token::lexer(fragment)
                .spanned()
                .filter_map(|(token, range)| {
                    matches!(token, Ok(Token::BraceClose)).then_some(range.start)
                })
                .last()
                .ok_or(SourceEditError::SurroundingCodeWouldChange)?;
            let mut body = Self::parse(&fragment[opening..closing])?;
            let mut touched = false;
            for statement in (0..body.statements.len()).rev() {
                let crate::Statement::CharacterCreate { id, .. } =
                    &body.statements[statement].statement
                else {
                    continue;
                };
                let Some(replacement) = changes.get(id) else {
                    continue;
                };
                found.insert(id.clone());
                touched = true;
                let current = body.source.clone();
                body.replace_statement_range_preserving_trivia(
                    statement..statement + 1,
                    &current,
                    replacement.as_deref().unwrap_or(""),
                )?;
            }
            if touched {
                let replacement = format!(
                    "{}{}{}",
                    &fragment[..opening],
                    body.source(),
                    &fragment[closing..]
                );
                let current = trial.source.clone();
                trial.replace_statement(index, &current, &replacement)?;
            }
        }
        if changes.keys().any(|id| !found.contains(id)) {
            return Err(SourceEditError::SurroundingCodeWouldChange);
        }
        *self = trial;
        Ok(())
    }

    /// Update tokens inside a function or structured block while retaining all
    /// original trivia and untouched token bytes. Full-file AST verification
    /// still runs before committing, so a retained comment cannot swallow code.
    pub fn replace_statement_preserving_trivia(
        &mut self,
        index: usize,
        current_source: &str,
        replacement: &str,
    ) -> Result<(), SourceEditError> {
        if parse(replacement)
            .map_err(|error| SourceEditError::InvalidSource(error.to_string()))?
            .len()
            != 1
        {
            return Err(SourceEditError::ExpectedSingleStatement);
        }
        self.replace_statement_range_preserving_trivia(
            index..index + 1,
            current_source,
            replacement,
        )
    }

    pub fn replace_statement_range_preserving_trivia(
        &mut self,
        range: std::ops::Range<usize>,
        current_source: &str,
        replacement: &str,
    ) -> Result<(), SourceEditError> {
        self.replace_statement_range_from_baseline(range, current_source, None, replacement)
    }

    /// The compiler baseline lets equivalent authored spellings (e.g. `{}`
    /// versus `dict()`) stay byte-for-byte unchanged outside the visual edit.
    pub fn replace_statement_range_from_baseline(
        &mut self,
        range: std::ops::Range<usize>,
        current_source: &str,
        baseline: Option<&str>,
        replacement: &str,
    ) -> Result<(), SourceEditError> {
        if current_source != self.source {
            return Err(SourceEditError::Conflict);
        }
        if range.start > range.end || range.end > self.statements.len() {
            return Err(SourceEditError::StatementNotFound(range.end));
        }
        let parsed = parse(replacement)
            .map_err(|error| SourceEditError::InvalidSource(error.to_string()))?;
        let original: Vec<_> = self.statements[range.clone()]
            .iter()
            .map(|statement| statement.statement.clone())
            .collect();
        if original == parsed {
            return Ok(());
        }
        let start = self
            .statements
            .get(range.start)
            .map(|statement| statement.range.start)
            .unwrap_or(self.source.len());
        let end = if range.is_empty() {
            start
        } else {
            self.statements[range.end - 1].range.end
        };
        let fragment = &self.source[start..end];
        let candidate = baseline
            .and_then(|before| reconcile_canonical_edit(fragment, before, replacement).ok())
            .filter(|candidate| parse(candidate).ok().as_deref() == Some(parsed.as_slice()))
            .map(Ok)
            .unwrap_or_else(|| reconcile_tokens(fragment, replacement))?;
        if parse(&candidate).ok().as_deref() != Some(parsed.as_slice()) {
            return Err(SourceEditError::SurroundingCodeWouldChange);
        }
        if comments(replacement)
            .iter()
            .any(|comment| !comments(&candidate).contains(comment))
        {
            return Err(SourceEditError::CommentWouldBeLost);
        }
        let mut source = self.source.clone();
        // Delimit inserted/deleted blocks without disturbing neighboring bytes.
        let delimited = if range.is_empty() {
            format!("\n{candidate}\n")
        } else {
            candidate
        };
        source.replace_range(start..end, &delimited);
        let statements = parse_spanned(&source)
            .map_err(|error| SourceEditError::InvalidSource(error.to_string()))?;
        let mut expected: Vec<_> = self
            .statements
            .iter()
            .map(|statement| statement.statement.clone())
            .collect();
        expected.splice(range, parsed);
        if statements
            .iter()
            .map(|statement| &statement.statement)
            .ne(expected.iter())
        {
            return Err(SourceEditError::SurroundingCodeWouldChange);
        }
        self.source = source;
        self.statements = statements;
        Ok(())
    }

    /// Replace one top-level statement. Unchanged bytes are retained verbatim,
    /// including comments, indentation and trailing whitespace. Failed edits
    /// leave this snapshot untouched. `current_source` is the caller's latest
    /// disk/editor content, not the content captured when the graph opened.
    pub fn replace_statement(
        &mut self,
        index: usize,
        current_source: &str,
        replacement: &str,
    ) -> Result<(), SourceEditError> {
        if current_source != self.source {
            return Err(SourceEditError::Conflict);
        }
        let original = self
            .statements
            .get(index)
            .ok_or(SourceEditError::StatementNotFound(index))?;
        let parsed = parse(replacement)
            .map_err(|error| SourceEditError::InvalidSource(error.to_string()))?;
        let [new_statement] = parsed.as_slice() else {
            return Err(SourceEditError::ExpectedSingleStatement);
        };
        // Recompiling an unchanged graph must not reformat the author source.
        if original.statement == *new_statement {
            return Ok(());
        }
        let old_fragment = &self.source[original.range.clone()];
        let old_comments = comments(old_fragment);
        let new_comments = comments(replacement);
        let mut remaining = new_comments.iter();
        for comment in old_comments {
            if !remaining.any(|candidate| *candidate == comment) {
                return Err(SourceEditError::CommentWouldBeLost);
            }
        }
        let mut candidate = self.source.clone();
        candidate.replace_range(original.range.clone(), replacement);
        let statements = parse_spanned(&candidate)
            .map_err(|error| SourceEditError::InvalidSource(error.to_string()))?;
        let mut expected = self
            .statements
            .iter()
            .map(|s| s.statement.clone())
            .collect::<Vec<_>>();
        expected[index] = new_statement.clone();
        if statements.iter().map(|s| &s.statement).ne(expected.iter()) {
            return Err(SourceEditError::SurroundingCodeWouldChange);
        }
        self.source = candidate;
        self.statements = statements;
        Ok(())
    }
}

fn token_ranges(source: &str) -> Vec<std::ops::Range<usize>> {
    Token::lexer(source)
        .spanned()
        .filter_map(|(token, range)| {
            if matches!(token, Ok(Token::Newline)) {
                None
            } else {
                Some(range)
            }
        })
        .collect()
}

fn token_matches(
    a: &str,
    before: &[std::ops::Range<usize>],
    b: &str,
    after: &[std::ops::Range<usize>],
) -> Result<Vec<(usize, usize)>, SourceEditError> {
    let equal = |x: usize, y: usize| a[before[x].clone()] == b[after[y].clone()];
    let mut prefix = 0;
    while prefix < before.len().min(after.len()) && equal(prefix, prefix) {
        prefix += 1;
    }
    let mut suffix = 0;
    while suffix < (before.len() - prefix).min(after.len() - prefix)
        && equal(before.len() - suffix - 1, after.len() - suffix - 1)
    {
        suffix += 1;
    }
    let n = before.len() - prefix - suffix;
    let m = after.len() - prefix - suffix;
    let cells = (n + 1)
        .checked_mul(m + 1)
        .filter(|cells| *cells <= 2_000_000)
        .ok_or(SourceEditError::EditTooComplex)?;
    let mut lengths = vec![0u32; cells];
    for x in (0..n).rev() {
        for y in (0..m).rev() {
            lengths[x * (m + 1) + y] = if equal(x + prefix, y + prefix) {
                1 + lengths[(x + 1) * (m + 1) + y + 1]
            } else {
                lengths[(x + 1) * (m + 1) + y].max(lengths[x * (m + 1) + y + 1])
            };
        }
    }
    let mut matches: Vec<_> = (0..prefix).map(|index| (index, index)).collect();
    let (mut x, mut y) = (0, 0);
    while x < n && y < m {
        if equal(x + prefix, y + prefix) {
            matches.push((x + prefix, y + prefix));
            x += 1;
            y += 1;
        } else if lengths[(x + 1) * (m + 1) + y] >= lengths[x * (m + 1) + y + 1] {
            x += 1;
        } else {
            y += 1;
        }
    }
    matches.extend((0..suffix).map(|offset| {
        (
            before.len() - suffix + offset,
            after.len() - suffix + offset,
        )
    }));
    Ok(matches)
}

fn reconcile_canonical_edit(
    old: &str,
    before: &str,
    after: &str,
) -> Result<String, SourceEditError> {
    let original = token_ranges(old);
    let a = token_ranges(before);
    let b = token_ranges(after);
    let retained = token_matches(before, &a, after, &b)?;
    let spelling: std::collections::BTreeMap<_, _> = token_matches(before, &a, old, &original)?
        .into_iter()
        .collect();
    let mut changes = Vec::new();
    let (mut left_a, mut left_b) = (0, 0);
    for (right_a, right_b) in retained
        .into_iter()
        .chain(std::iter::once((a.len(), b.len())))
    {
        if left_a != right_a || left_b != right_b {
            // A scalar edit has its own authored token, even when adjacent
            // dictionary punctuation is spelled {} / : instead of dict() / ,.
            // Using neighboring canonical tokens would unnecessarily rewrite
            // the entire expression when those delimiters are not mapped.
            if right_a == left_a + 1 && right_b == left_b + 1 {
                if let Some(index) = spelling.get(&left_a) {
                    changes.push((
                        original[*index].clone(),
                        after[b[left_b].clone()].to_owned(),
                    ));
                    left_a = right_a + 1;
                    left_b = right_b + 1;
                    continue;
                }
            }
            let start = if left_a == 0 {
                0
            } else {
                original[*spelling
                    .get(&(left_a - 1))
                    .ok_or(SourceEditError::SurroundingCodeWouldChange)?]
                .end
            };
            let end = if right_a == a.len() {
                old.len()
            } else {
                original[*spelling
                    .get(&right_a)
                    .ok_or(SourceEditError::SurroundingCodeWouldChange)?]
                .start
            };
            if start > end || !comments(&old[start..end]).is_empty() {
                return Err(SourceEditError::CommentWouldBeLost);
            }
            let text = if left_b == right_b {
                String::new()
            } else {
                format!(" {} ", &after[b[left_b].start..b[right_b - 1].end])
            };
            changes.push((start..end, text));
        }
        left_a = right_a + 1;
        left_b = right_b + 1;
    }
    let mut result = old.to_owned();
    for (range, text) in changes.into_iter().rev() {
        result.replace_range(range, &text);
    }
    Ok(result)
}

fn reconcile_tokens(old: &str, new: &str) -> Result<String, SourceEditError> {
    fn tokens(source: &str) -> Vec<std::ops::Range<usize>> {
        Token::lexer(source)
            .spanned()
            .filter_map(|(token, range)| {
                if matches!(token, Ok(Token::Newline)) {
                    None
                } else {
                    Some(range)
                }
            })
            .collect()
    }
    let before = tokens(old);
    let after = tokens(new);
    let equal = |a: usize, b: usize| old[before[a].clone()] == new[after[b].clone()];
    // Trim identical ends before allocating the bounded LCS table. This keeps
    // small edits in large functions inexpensive.
    let mut prefix = 0;
    while prefix < before.len().min(after.len()) && equal(prefix, prefix) {
        prefix += 1;
    }
    let mut suffix = 0;
    while suffix < (before.len() - prefix).min(after.len() - prefix)
        && equal(before.len() - suffix - 1, after.len() - suffix - 1)
    {
        suffix += 1;
    }
    let n = before.len() - prefix - suffix;
    let m = after.len() - prefix - suffix;
    let cells = (n + 1)
        .checked_mul(m + 1)
        .filter(|cells| *cells <= 2_000_000)
        .ok_or(SourceEditError::EditTooComplex)?;
    let mut lengths = vec![0u32; cells];
    for a in (0..n).rev() {
        for b in (0..m).rev() {
            lengths[a * (m + 1) + b] = if equal(a + prefix, b + prefix) {
                1 + lengths[(a + 1) * (m + 1) + b + 1]
            } else {
                lengths[(a + 1) * (m + 1) + b].max(lengths[a * (m + 1) + b + 1])
            };
        }
    }
    let mut matches: Vec<_> = (0..prefix).map(|index| (index, index)).collect();
    let (mut a, mut b) = (0, 0);
    while a < n && b < m {
        if equal(a + prefix, b + prefix) {
            matches.push((a + prefix, b + prefix));
            a += 1;
            b += 1;
        } else if lengths[(a + 1) * (m + 1) + b] >= lengths[a * (m + 1) + b + 1] {
            a += 1;
        } else {
            b += 1;
        }
    }
    matches.extend((0..suffix).map(|offset| {
        (
            before.len() - suffix + offset,
            after.len() - suffix + offset,
        )
    }));
    let retained: std::collections::BTreeSet<_> = matches.iter().map(|(old, _)| *old).collect();
    let mut insertions = std::collections::BTreeMap::new();
    let mut next = 0;
    let mut anchor = 0;
    for (old_index, new_index) in matches
        .iter()
        .copied()
        .chain(std::iter::once((before.len(), after.len())))
    {
        if next < new_index {
            let chunk = &new[after[next].start..after[new_index - 1].end];
            // Insert before the old trailing trivia, particularly inline
            // comments/newlines. Arithmetic must stay on its original line.
            insertions.insert(anchor, format!(" {chunk} "));
        }
        next = new_index + 1;
        if let Some(range) = before.get(old_index) {
            anchor = range.end;
        }
    }
    let mut result = String::new();
    if let Some(chunk) = insertions.get(&0) {
        result.push_str(chunk);
    }
    let mut cursor = 0;
    for (index, range) in before.iter().enumerate() {
        result.push_str(&old[cursor..range.start]);
        if retained.contains(&index) {
            result.push_str(&old[range.clone()]);
        }
        if let Some(chunk) = insertions.get(&range.end) {
            result.push_str(chunk);
        }
        cursor = range.end;
    }
    result.push_str(&old[cursor..]);
    Ok(result)
}

// Logos skips comments but exposes token byte ranges. Only examine the gaps,
// so a string such as "https://example.test" is never treated as a comment.
fn comments(source: &str) -> Vec<&str> {
    let mut result = Vec::new();
    let mut end = 0;
    for (_, span) in Token::lexer(source).spanned() {
        collect_comments(&source[end..span.start], &mut result);
        end = span.end;
    }
    collect_comments(&source[end..], &mut result);
    result
}
fn collect_comments<'a>(gap: &'a str, result: &mut Vec<&'a str>) {
    for line in gap.split('\n') {
        let line = line.trim_start();
        if line.starts_with("//") {
            result.push(line);
        }
    }
}
