//! Portable text identity shared by RVN, graph authoring and runtime locales.
//! The owner is an RVN asset identity, never an Unreal package or a file path.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TextIdentity {
    pub owner: String,
    pub namespace: String,
    pub key: String,
}

impl TextIdentity {
    pub fn validate(&self) -> Result<(), String> {
        if self.owner.trim().is_empty() {
            return Err("A localized text needs an asset owner".into());
        }
        validate_text_identifier(&self.namespace)?;
        validate_text_identifier(&self.key)?;
        if self.key.is_empty() {
            return Err("A localized text needs a nonempty key".into());
        }
        Ok(())
    }

    /// Length prefixes prevent collisions between owner/namespace/key tuples.
    /// This key fits the existing [strings] locale table without changing it.
    pub fn locale_key(&self) -> String {
        format!("@rvn.text:{}:{}:{}:{}:{}:{}", self.owner.len(), self.owner,
            self.namespace.len(), self.namespace, self.key.len(), self.key)
    }
}

/// Persistent authored Text asset ID shared by graph ownership and table files.
/// Allocation belongs to the editor's OS-backed asset transaction.
pub fn validate_text_asset_id(value: &str) -> Result<(), String> {
    if value.len() != 32 || !value.bytes().all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        || value.bytes().all(|c| c == b'0') {
        return Err("Text asset identity must be 32 lowercase hexadecimal digits and nonzero".into());
    }
    Ok(())
}

pub fn validate_text_identifier(value: &str) -> Result<(), String> {
    if let Some(character) = value.chars().find(|c| matches!(c,
        '"' | '\'' | ' ' | ',' | '\n' | '\r' | '\t' | '[' | ']')) {
        return Err(format!("Text identifiers cannot contain {character:?}"));
    }
    Ok(())
}

/// Absence of metadata in an old graph keeps RVN's source-key localization.
/// Explicit invariant text bypasses every translation lookup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum TextLocalization {
    /// A cleared text has neither identity nor notes; the editor can localize
    /// its next nonempty source using the owning asset's identity.
    Empty,
    CultureInvariant,
    Localizable {
        identity: TextIdentity,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        developer_notes: String,
    },
}

impl TextLocalization {
    pub fn identity(&self) -> Option<&TextIdentity> {
        match self {
            Self::Empty | Self::CultureInvariant => None,
            Self::Localizable { identity, .. } => Some(identity),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextIdentityEdit { Source, Namespace, Key, Enable }

/// Source and metadata remain separate from any rendered translation, so an
/// editor snapshot or save can be redisplayed after changing the language.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalizedText {
    pub source: String,
    pub localization: TextLocalization,
}

/// Persistent address of an independently authored VN string-table asset.
/// Neither source text, display cache nor a host loading handle belongs here.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct StringTableReference {
    pub table: String,
    pub key: String,
}

pub fn validate_string_table_id(id: &str) -> Result<(), String> {
    if id.trim().is_empty() || id.len() > 4096 || id.chars().any(char::is_control) {
        return Err("A string table needs a nonempty asset ID of at most 4,096 bytes without control characters".into());
    }
    Ok(())
}

impl StringTableReference {
    pub fn validate_name(&self) -> Result<(), String> {
        let name = crate::Name::new(&self.table).map_err(|error| error.to_string())?;
        if name.as_str() != self.table { return Err("A Name Text reference must retain its validated runtime spelling".into()); }
        if self.key.len() > 4096 || self.key.contains('\0') { return Err("A dynamic Name Text key exceeds 4096 bytes or contains NUL".into()); }
        Ok(())
    }
    pub fn validate(&self) -> Result<(), String> {
        validate_string_table_id(&self.table)?;
        validate_text_identifier(&self.key)?;
        if self.key.trim().is_empty() || self.key.len() > 4096 {
            return Err("A string-table reference needs a nonempty key of at most 4,096 bytes".into());
        }
        Ok(())
    }
}

/// A formatted text keeps its inputs, including nested localized arguments.
/// Serialization never needs to freeze a translation or evaluate RVN code.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DeferredText {
    Literal { text: LocalizedText },
    StringTable { reference: StringTableReference },
    /// Additive Name lookup semantics; the persistent address stays String/UUID.
    NameStringTable { reference: StringTableReference },
    Format { pattern: Box<DeferredText>, arguments: BTreeMap<String, TextArgument> },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum TextArgument {
    String(String),
    Text(Box<DeferredText>),
    Int(i64),
    Float(f32),
    Bool(bool),
}

impl PartialEq for DeferredText {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Literal { text: a }, Self::Literal { text: b }) => a == b,
            (Self::StringTable { reference: a }, Self::StringTable { reference: b }) => a == b,
            (Self::NameStringTable { reference: a }, Self::NameStringTable { reference: b }) => {
                a.key == b.key && match (crate::Name::new(&a.table), crate::Name::new(&b.table)) {
                    (Ok(a), Ok(b)) => a == b,
                    _ => a.table == b.table,
                }
            },
            (Self::Format { pattern: a, arguments: aa }, Self::Format { pattern: b, arguments: ba }) => a == b && aa == ba,
            _ => false,
        }
    }
}

impl DeferredText {
    pub fn has_name_string_table(&self) -> Result<bool, String> {
        self.storage_work()?;
        fn contains(text: &DeferredText) -> bool {
            match text {
                DeferredText::NameStringTable { .. } => true,
                DeferredText::Format { pattern, arguments } => contains(pattern) || arguments.values().any(|arg| matches!(arg, TextArgument::Text(text) if contains(text))),
                _ => false,
            }
        }
        Ok(contains(self))
    }
    pub fn has_string_table(&self) -> Result<bool, String> {
        self.storage_work()?;
        fn contains(text: &DeferredText) -> bool {
            match text {
                DeferredText::StringTable { .. } | DeferredText::NameStringTable { .. } => true,
                DeferredText::Literal { .. } => false,
                DeferredText::Format { pattern, arguments } => contains(pattern) || arguments.values().any(|arg|
                    matches!(arg, TextArgument::Text(text) if contains(text))),
            }
        }
        Ok(contains(self))
    }
    /// Validate the stored tree before cloning, saving or projecting it. Notes
    /// and identity strings share the same budget as source and arguments.
    pub fn storage_work(&self) -> Result<usize, String> {
        let (mut nodes, mut bytes) = (0, 0);
        self.visit_storage(0, &mut nodes, &mut bytes)?;
        Ok(nodes + bytes / 32)
    }

    /// Share bounds with an enclosing RVN collection instead of resetting the
    /// allowance for each nested Text value.
    pub fn visit_storage(&self, depth: usize, nodes: &mut usize, bytes: &mut usize) -> Result<(), String> {
        fn charge(value: &str, bytes: &mut usize) -> Result<(), String> {
            *bytes = bytes.checked_add(value.len()).ok_or("Text size overflow")?;
            if *bytes > 1_048_576 { return Err("Text storage exceeds 1 MiB".into()); }
            Ok(())
        }
        fn visit(text: &DeferredText, depth: usize, nodes: &mut usize, bytes: &mut usize) -> Result<(), String> {
            *nodes += 1;
            if depth > 64 || *nodes > 65_536 { return Err("Text storage exceeds its depth or node budget".into()); }
            match text {
                DeferredText::StringTable { reference } => {
                    reference.validate()?;
                    charge(&reference.table, bytes)?;
                    charge(&reference.key, bytes)?;
                }
                DeferredText::NameStringTable { reference } => {
                    reference.validate_name()?;
                    charge(&reference.table, bytes)?;
                    charge(&reference.key, bytes)?;
                }
                DeferredText::Literal { text } => {
                    charge(&text.source, bytes)?;
                    if matches!(text.localization, TextLocalization::Empty) && !text.source.is_empty() {
                        return Err("Empty text metadata requires an empty source".into());
                    }
                    if let TextLocalization::Localizable { identity, developer_notes } = &text.localization {
                        charge(&identity.owner, bytes)?; charge(&identity.namespace, bytes)?;
                        charge(&identity.key, bytes)?; charge(developer_notes, bytes)?;
                        identity.validate()?;
                    }
                }
                DeferredText::Format { pattern, arguments } => {
                    if arguments.len() > 4096 { return Err("Text storage exceeds 4,096 arguments".into()); }
                    visit(pattern, depth + 1, nodes, bytes)?;
                    for (name, argument) in arguments {
                        *nodes += 1;
                        if *nodes > 65_536 { return Err("Text storage exceeds its node budget".into()); }
                        charge(name, bytes)?;
                        match argument {
                            TextArgument::Text(text) => visit(text, depth + 1, nodes, bytes)?,
                            TextArgument::String(text) => charge(text, bytes)?,
                            TextArgument::Float(number) if !number.is_finite() => return Err("Text argument must be finite".into()),
                            _ => {},
                        }
                    }
                }
            }
            Ok(())
        }
        visit(self, depth, nodes, bytes)
    }

    pub fn invariant(source: impl Into<String>) -> Self {
        Self::Literal { text: LocalizedText {
            source: source.into(), localization: TextLocalization::CultureInvariant,
        } }
    }

    /// Locale-free display used by diagnostics and explicit source inspection.
    /// Runtime display supplies its locale through rvn_core::render_text.
    pub fn source_display(&self) -> Result<String, String> {
        self.storage_work()?;
        fn render(text: &DeferredText, depth: usize, work: &mut usize) -> Result<String, String> {
            if depth > 64 { return Err("Text exceeds 64 levels".into()); }
            let result = match text {
                DeferredText::StringTable { .. } | DeferredText::NameStringTable { .. } => return Err("A string-table Text needs an explicit resource context".into()),
                DeferredText::Literal { text } => {
                    if text.source.len() > 1_048_576 - *work { return Err("Text exceeds 1 MiB of work".into()); }
                    text.source.clone()
                },
                DeferredText::Format { pattern, arguments } => {
                    if arguments.len() > 4096 { return Err("Text exceeds 4,096 arguments".into()); }
                    let pattern = render(pattern, depth + 1, work)?;
                    let mut values = BTreeMap::new();
                    for (name, value) in arguments {
                        let value = match value {
                            TextArgument::String(s) => {
                                if s.len() > 1_048_576 { return Err("Text exceeds 1 MiB".into()); }
                                s.clone()
                            },
                            TextArgument::Text(t) => render(t, depth + 1, work)?,
                            TextArgument::Int(n) => n.to_string(),
                            TextArgument::Float(n) if n.is_finite() => n.to_string(),
                            TextArgument::Float(_) => return Err("Text argument must be finite".into()),
                            TextArgument::Bool(b) => b.to_string(),
                        };
                        *work = work.checked_add(value.len()).ok_or("Text size overflow")?;
                        if *work > 1_048_576 { return Err("Text exceeds 1 MiB of work".into()); }
                        values.insert(name.as_str(), value);
                    }
                    substitute_named(&pattern, &values, 1_048_576 - *work)?
                }
            };
            *work = work.checked_add(result.len()).ok_or("Text size overflow")?;
            if *work > 1_048_576 { return Err("Text exceeds 1 MiB of work".into()); }
            Ok(result)
        }
        render(self, 0, &mut 0)
    }
}

/// Token substitution shared by source inspection and localized runtime output.
/// Argument payloads are literal; no expression parsing or recursive expansion.
pub fn substitute_named(pattern: &str, values: &BTreeMap<&str, String>, max_bytes: usize) -> Result<String, String> {
    let mut result = String::new();
    let mut characters = pattern.char_indices().peekable();
    // Once there is no closing brace, none of the remaining openings can form
    // an argument. Avoid rescanning a long unfinished editor draft per brace.
    let mut no_more_closing = false;
    while let Some((start, character)) = characters.next() {
        let part: &str;
        if character == '`' && characters.peek().is_some_and(|(_, c)| matches!(c, '`' | '{' | '}' | '|')) {
            let (index, escaped) = characters.next().unwrap();
            part = &pattern[index..index + escaped.len_utf8()];
        } else if character == '{' && !no_more_closing {
            let mut scan = characters.clone();
            let mut closing = None;
            for (index, c) in scan.by_ref() {
                if c == '}' { closing = Some(index); break; }
            }
            if let Some(end) = closing {
                let name = &pattern[start + 1..end];
                if scan.peek().is_some_and(|(_, c)| *c == '|') {
                    return Err("Format argument modifiers require a qualified formatter".into());
                }
                if !name.is_empty() {
                    part = values.get(name).map(String::as_str).unwrap_or(&pattern[start..end + 1]);
                    characters = scan;
                } else { part = &pattern[start..start + 1]; }
            } else { no_more_closing = true; part = &pattern[start..start + 1]; }
        } else { part = &pattern[start..start + character.len_utf8()]; }
        if part.len() > max_bytes.saturating_sub(result.len()) {
            return Err("Text display exceeds its byte budget".into());
        }
        result.push_str(part);
    }
    Ok(result)
}

impl LocalizedText {
    /// Disabling clears identity and notes. Enabling never resurrects them.
    pub fn set_localizable(
        &mut self,
        enabled: bool,
        owner: &str,
        references: &[TextReference<'_>],
        generate_key: impl FnMut() -> String,
    ) -> Result<bool, String> {
        if enabled == !matches!(self.localization, TextLocalization::CultureInvariant) { return Ok(false); }
        let next = if enabled {
            if self.source.is_empty() { self.localization = TextLocalization::Empty; return Ok(true); }
            let identity = identity_for_edit(TextIdentity {
                owner: owner.into(), namespace: String::new(), key: String::new(),
            }, TextIdentityEdit::Enable, &self.source, references, generate_key)?;
            TextLocalization::Localizable { identity, developer_notes: String::new() }
        } else { TextLocalization::CultureInvariant };
        self.localization = next;
        Ok(true)
    }

    /// Identity decisions precede assignment. UI draft validation and document
    /// transactions belong to the editor; a failed edit leaves this value exact.
    pub fn edit_source(
        &mut self,
        source: impl Into<String>,
        owner: &str,
        references: &[TextReference<'_>],
        generate_key: impl FnMut() -> String,
    ) -> Result<bool, String> {
        let source = source.into();
        if source == self.source { return Ok(false); }
        let mut next = if source.is_empty() { TextLocalization::Empty } else { self.localization.clone() };
        if matches!(next, TextLocalization::Empty) && !source.is_empty() {
            next = TextLocalization::Localizable {
                identity: identity_for_edit(TextIdentity { owner: owner.into(),
                    namespace: String::new(), key: String::new() }, TextIdentityEdit::Enable,
                    &self.source, references, generate_key)?, developer_notes: String::new(),
            };
        } else if let TextLocalization::Localizable { identity, .. } = &mut next {
            *identity = identity_for_edit(identity.clone(), TextIdentityEdit::Source,
                &self.source, references, generate_key)?;
        }
        self.source = source;
        self.localization = next;
        Ok(true)
    }

    pub fn edit_identifier(
        &mut self,
        action: TextIdentityEdit,
        value: impl Into<String>,
        references: &[TextReference<'_>],
        generate_key: impl FnMut() -> String,
    ) -> Result<bool, String> {
        let value = value.into();
        let TextLocalization::Localizable { identity, developer_notes } = &self.localization
            else { return Err("Invariant text has no editable identity".into()); };
        let mut proposed = identity.clone();
        match action {
            TextIdentityEdit::Namespace => proposed.namespace = value,
            TextIdentityEdit::Key => proposed.key = value,
            _ => return Err("A namespace or key edit is required".into()),
        }
        if proposed == *identity { return Ok(false); }
        let identity = identity_for_edit(proposed, action, &self.source, references, generate_key)?;
        self.localization = TextLocalization::Localizable { identity,
            developer_notes: developer_notes.clone() };
        Ok(true)
    }
}

/// Borrowed references from the owning RVN asset. Source edits preserve an ID
/// only for a single reference; identifier edits may share an identical source.
pub struct TextReference<'a> {
    pub identity: &'a TextIdentity,
    pub source: &'a str,
}

/// Decide identity before mutating the document, so errors are transactional.
/// Key allocation belongs to the caller; it consumes no gameplay random state.
pub fn identity_for_edit(
    proposed: TextIdentity,
    action: TextIdentityEdit,
    previous_source: &str,
    references: &[TextReference<'_>],
    mut generate_key: impl FnMut() -> String,
) -> Result<TextIdentity, String> {
    validate_text_identifier(&proposed.namespace)?;
    validate_text_identifier(&proposed.key)?;
    if proposed.owner.trim().is_empty() {
        return Err("A localized text needs an asset owner".into());
    }
    let matching: Vec<_> = references.iter().filter(|r| r.identity == &proposed).collect();
    let keep = !proposed.key.is_empty() && match action {
        TextIdentityEdit::Source => matching.len() == 1,
        TextIdentityEdit::Namespace | TextIdentityEdit::Key =>
            matching.iter().all(|r| r.source == previous_source),
        TextIdentityEdit::Enable => false,
    };
    if keep { return Ok(proposed); }
    // A broken allocator must fail without hanging the editor or reusing an ID.
    for _ in 0..128 {
        let candidate = TextIdentity { key: generate_key(), ..proposed.clone() };
        if candidate.validate().is_ok() && !references.iter().any(|r| r.identity == &candidate) {
            return Ok(candidate);
        }
    }
    Err("Could not allocate a fresh localized text key".into())
}
