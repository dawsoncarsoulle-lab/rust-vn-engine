//! Independent string-table resource contract. Hosts perform I/O and publish
//! validated snapshots; resolving a reference never loads a file or caches text.
use crate::locale::LocaleManager;
use rvn_parser::localized_text::validate_string_table_id as validate_table_id;
pub use rvn_parser::localized_text::StringTableReference;
use rvn_parser::localized_text::{validate_text_identifier, TextIdentity, TextLocalization};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

/// Stored data keys are independent of editor identity-entry validation.
/// Local Unreal0332 CSV import and compiled Blueprint VM retain empty keys.
/// The 4096-byte/NUL bound is the existing VN Name Text storage contract.
pub fn validate_data_key(key: &str) -> Result<(), String> {
    if key.len() > 4096 || key.contains('\0') {
        return Err("A stored string-table key exceeds 4096 bytes or contains NUL".into());
    }
    Ok(())
}

const MAX_TABLE_BYTES: usize = 8 * 1_048_576;
const MAX_TABLE_ENTRIES: usize = 65_536;
// Bound JSON field overhead as well as decoded bytes, keeping every valid
// table representable within the established 64 MiB wire budget.
const MAX_TABLE_METADATA_NAMES: usize = 65_536;
const MAX_REDIRECTS: usize = 64;

/// Asset identity is a typed description, separate from an ordinary ID string.
/// It owns neither table content nor a host handle. Existing Value serialization
/// preserves it across saves; the empty string remains the engine's null value.
pub(crate) fn asset_reference(id: &str) -> rvn_parser::Value {
    use rvn_parser::Value;
    Value::Dict(BTreeMap::from([
        ("kind".into(), Value::Str("string_table_asset".into())),
        ("id".into(), Value::Str(id.into())),
    ]))
}

pub(crate) fn asset_reference_id(value: &rvn_parser::Value) -> Result<&str, String> {
    use rvn_parser::Value;
    if matches!(value, Value::Str(id) if id.is_empty()) { return Ok(""); }
    if let Value::Dict(fields) = value {
        if fields.len() == 2 && matches!(fields.get("kind"), Some(Value::Str(kind)) if kind == "string_table_asset") {
            if let Some(Value::Str(id)) = fields.get("id") {
                validate_table_id(id)?;
                return Ok(id);
            }
        }
    }
    Err("Expected a string-table asset reference or null".into())
}

/// Installed reference editor's visible unavailable-entry marker. This is
/// presentation text, never a replacement source stored in the asset or save.
pub const MISSING_STRING_TABLE_ENTRY: &str = "<MISSING STRING TABLE ENTRY>";

#[derive(Debug, Clone)]
pub struct StringTableRevision {
    authority: Arc<()>,
    revision: u64,
}
impl PartialEq for StringTableRevision {
    fn eq(&self, other: &Self) -> bool {
        self.revision == other.revision && Arc::ptr_eq(&self.authority, &other.authority)
    }
}
impl Eq for StringTableRevision {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StringTableEntry {
    pub source: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub developer_notes: String,
    /// Authored context, separate from visible text and localization identity.
    /// Names preserve display spelling; equivalence folds ASCII only.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty", deserialize_with = "unique_metadata")]
    pub metadata: BTreeMap<String, String>,
}

impl StringTableEntry {
    pub fn metadata_value(&self, name: &str) -> Option<&str> {
        self.metadata.iter().find(|(key, _)| key.eq_ignore_ascii_case(name)).map(|(_, value)| value.as_str())
    }

    /// Metadata writes retain the most recent supplied display spelling.
    pub fn set_metadata(&mut self, name: String, value: String) -> Result<(), String> {
        validate_metadata_name(&name)?;
        if let Some(old) = self.metadata.keys().find(|key| key.eq_ignore_ascii_case(&name)).cloned() {
            self.metadata.remove(&old);
        }
        self.metadata.insert(name, value);
        Ok(())
    }
}

/// Independent name contract for the installed reference. An empty base with
/// a valid numeric suffix represents the absent name; Unicode case and
/// normalization remain distinct. No global name pool is required.
pub fn is_absent_metadata_name(name: &str) -> bool {
    name.is_empty() || name.eq_ignore_ascii_case("None") || name.strip_prefix('_').is_some_and(|suffix| {
        !suffix.is_empty() && suffix.bytes().all(|b| b.is_ascii_digit())
            && (suffix.len() == 1 || !suffix.starts_with('0'))
            && suffix.parse::<u32>().is_ok_and(|n| n < i32::MAX as u32)
    })
}

pub fn validate_metadata_name(name: &str) -> Result<(), String> {
    if is_absent_metadata_name(name) || name.contains('\0') || name.encode_utf16().count() >= 1024 {
        return Err("Invalid string-table metadata name".into());
    }
    Ok(())
}

pub(crate) fn unique_metadata<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<BTreeMap<String, String>, D::Error> {
    struct Metadata;
    impl<'de> serde::de::Visitor<'de> for Metadata {
        type Value = BTreeMap<String, String>;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { f.write_str("distinct string-table metadata names") }
        fn visit_map<M: serde::de::MapAccess<'de>>(self, mut map: M) -> Result<Self::Value, M::Error> {
            let mut result = BTreeMap::new();
            let mut names = BTreeSet::new();
            while let Some((name, value)) = map.next_entry::<String, String>()? {
                validate_metadata_name(&name).map_err(serde::de::Error::custom)?;
                if !names.insert(name.to_ascii_lowercase()) { return Err(serde::de::Error::custom("Repeated string-table metadata name")); }
                result.insert(name, value);
                if result.len() > 65_536 { return Err(serde::de::Error::custom("String table metadata exceeds 65,536 names")); }
            }
            Ok(result)
        }
    }
    deserializer.deserialize_map(Metadata)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StringTableAsset {
    pub id: String,
    pub namespace: String,
    pub entries: BTreeMap<String, StringTableEntry>,
}

impl StringTableAsset {
    pub fn content_bytes(&self) -> Result<usize, String> {
        self.entries.iter().try_fold(self.id.len().checked_add(self.namespace.len()).ok_or("String table size overflow")?, |bytes, (key, entry)| {
            let bytes = bytes.checked_add(key.len()).and_then(|n| n.checked_add(entry.source.len()))
                .and_then(|n| n.checked_add(entry.developer_notes.len())).ok_or("String table size overflow")?;
            entry.metadata.iter().try_fold(bytes, |bytes, (name, value)| bytes.checked_add(name.len())
                .and_then(|n| n.checked_add(value.len())).ok_or_else(|| "String table size overflow".into()))
        })
    }

    pub fn validate(&self) -> Result<(), String> {
        validate_table_id(&self.id)?;
        validate_text_identifier(&self.namespace)?;
        if self.entries.len() > MAX_TABLE_ENTRIES {
            return Err("A string table exceeds 65,536 entries".into());
        }
        let mut metadata_names = 0usize;
        let mut bytes = self
            .id
            .len()
            .checked_add(self.namespace.len())
            .ok_or("String table size overflow")?;
        for (key, entry) in &self.entries {
            validate_data_key(key)?;
            let mut names = BTreeSet::new();
            metadata_names = metadata_names.checked_add(entry.metadata.len()).ok_or("String table metadata count overflow")?;
            if metadata_names > MAX_TABLE_METADATA_NAMES { return Err("String table metadata exceeds 65,536 names across all entries".into()); }
            for (name, value) in &entry.metadata {
                validate_metadata_name(name)?;
                if !names.insert(name.to_ascii_lowercase()) { return Err("Repeated string-table metadata name".into()); }
                bytes = bytes.checked_add(name.len()).and_then(|n| n.checked_add(value.len())).ok_or("String table size overflow")?;
            }
            bytes = bytes
                .checked_add(key.len())
                .and_then(|n| n.checked_add(entry.source.len()))
                .and_then(|n| n.checked_add(entry.developer_notes.len()))
                .ok_or("String table size overflow")?;
            if bytes > MAX_TABLE_BYTES {
                return Err("A string table exceeds 8 MiB".into());
            }
        }
        if bytes > MAX_TABLE_BYTES {
            return Err("A string table exceeds 8 MiB".into());
        }
        Ok(())
    }

    /// Data APIs allow empty sources. Both the reference editor's Add command
    /// and its row source commit impose a separate nonempty-source requirement.
    pub fn localization(&self, key: &str) -> Result<TextLocalization, String> {
        let entry = self
            .entries
            .get(key)
            .ok_or("String table entry is missing")?;
        let identity = TextIdentity {
            owner: format!("string-table:{}", self.id),
            namespace: self.namespace.clone(),
            key: key.into(),
        };
        validate_table_id(&self.id)?;
        validate_text_identifier(&self.namespace)?;
        validate_data_key(key)?;
        Ok(TextLocalization::Localizable {
            identity,
            developer_notes: entry.developer_notes.clone(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct StringTableLoadToken {
    table: String,
    revision: u64,
    authority: Arc<()>,
    load_identity: Arc<()>,
}

impl PartialEq for StringTableLoadToken {
    fn eq(&self, other: &Self) -> bool {
        self.table == other.table
            && self.revision == other.revision
            && Arc::ptr_eq(&self.authority, &other.authority)
            && Arc::ptr_eq(&self.load_identity, &other.load_identity)
    }
}
impl Eq for StringTableLoadToken {}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TableState {
    Loading,
    Ready(Arc<StringTableAsset>),
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TableSlot {
    revision: u64,
    state: TableState,
    load_identity: Option<Arc<()>>,
}

/// Runtime/editor resource state is deliberately separate from persistent Text.
/// A generation token prevents an old asynchronous load from replacing a newer
/// publication, failure or unload. No shared process-wide registry is required.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StringTableResources {
    load_authority: Arc<()>,
    revision_identity: Arc<()>,
    revision: u64,
    tables: BTreeMap<String, TableSlot>,
    redirects: BTreeMap<StringTableReference, StringTableReference>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StringTableResolution<'a> {
    Ready {
        table: &'a StringTableAsset,
        key: &'a str,
        entry: &'a StringTableEntry,
        revision: u64,
    },
    MissingTable,
    Loading,
    Failed(&'a str),
    MissingEntry,
    RedirectLimit,
}


/// Path-independent installed content. Live revision/load tokens are deliberately
/// absent: this is a comparison receipt, never an authority to finish a load.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StringTableContentSnapshot {
    pub version: u32,
    pub lookup_policy: u32,
    pub redirect_limit: usize,
    pub missing_entry_marker: String,
    pub tables: BTreeMap<String, StringTableContentState>,
    // JSON object keys cannot represent compound references; sorted pairs can.
    pub redirects: Vec<(StringTableReference, StringTableReference)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StringTableContentState {
    Loading,
    Ready(StringTableAsset),
    Failed(String),
}

impl StringTableContentSnapshot {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 || self.lookup_policy != 1 || self.redirect_limit != MAX_REDIRECTS
            || self.missing_entry_marker != MISSING_STRING_TABLE_ENTRY {
            return Err("Unsupported string-table content policy".into());
        }
        if self.tables.len() > crate::scene_resources::MAX_RESOURCES || self.redirects.len() > MAX_TABLE_ENTRIES {
            return Err("String-table content snapshot count budget exceeded".into());
        }
        crate::locale::check_portable_text_items([self.tables.len(),self.redirects.len()].into_iter().chain(
            self.tables.values().filter_map(|state|match state {StringTableContentState::Ready(table)=>Some(table),_=>None})
                .flat_map(|table|std::iter::once(table.entries.len()).chain(table.entries.values().map(|entry|entry.metadata.len())))))?;
        let mut bytes = 0usize;
        for (id, state) in &self.tables {
            validate_table_id(id)?;
            bytes = bytes.checked_add(id.len()).ok_or("Content snapshot size overflow")?;
            let size = match state {
                StringTableContentState::Loading => 0,
                StringTableContentState::Failed(message) => {
                    if message.len() > 4096 { return Err("String-table diagnostic budget exceeded".into()); }
                    message.len()
                }
                StringTableContentState::Ready(table) => {
                    if &table.id != id { return Err("String-table snapshot key differs from asset ID".into()); }
                    table.validate()?;
                    table.content_bytes()?
                }
            };
            bytes = bytes.checked_add(size).ok_or("Content snapshot size overflow")?;
            if bytes > crate::locale::MAX_PORTABLE_TEXT_BYTES { return Err("Portable text content exceeds 64 MiB".into()); }
        }
        let mut previous = None;
        for (from, to) in &self.redirects {
            from.validate()?;to.validate()?;
            if previous.is_some_and(|key| key >= from) { return Err("String-table redirects are not distinct and canonical".into()); }
            previous = Some(from);
            for value in [&from.table, &from.key, &to.table, &to.key] {
                bytes = bytes.checked_add(value.len()).ok_or("Content snapshot size overflow")?;
            }
            if bytes > crate::locale::MAX_PORTABLE_TEXT_BYTES { return Err("Portable text content exceeds 64 MiB".into()); }
        }
        Ok(())
    }

    /// Canonical UTF-8 JSON with fixed field order and sorted maps/pairs, SHA-256.
    /// No Debug formatting, paths, process addresses, generations, or tokens.
    pub fn content_digest(&self) -> Result<[u8; 32], String> {
        self.validate()?;
        crate::locale::canonical_text_digest(self)
    }

    pub fn restore_digest(&self) -> Result<[u8; 32], String> {
        if self.tables.values().any(|state| matches!(state, StringTableContentState::Loading)) {
            return Err("Pending string-table load requires a verified host publication boundary".into());
        }
        self.content_digest()
    }
}

impl StringTableResources {
    /// Freeze the installed observable snapshot at an explicit checkpoint boundary.
    /// Ordinary lookups continue to use the existing O(1) live revision stamp.
    pub fn portable_content_snapshot(&self) -> Result<StringTableContentSnapshot, String> {
        // Preflight borrowed content before making the owned receipt.
        crate::locale::check_portable_text_items([self.tables.len(),self.redirects.len()].into_iter().chain(
            self.tables.values().filter_map(|slot|match &slot.state {TableState::Ready(table)=>Some(table.as_ref()),_=>None})
                .flat_map(|table|std::iter::once(table.entries.len()).chain(table.entries.values().map(|entry|entry.metadata.len())))))?;
        let mut bytes = 0usize;
        for (id, slot) in &self.tables {
            let size = match &slot.state {
                TableState::Loading => 0,
                TableState::Failed(message) => message.len(),
                TableState::Ready(table) => { table.validate()?; table.content_bytes()? }
            };
            bytes = bytes.checked_add(id.len()).and_then(|sum| sum.checked_add(size)).ok_or("Content snapshot size overflow")?;
            if bytes > crate::locale::MAX_PORTABLE_TEXT_BYTES { return Err("Portable text content exceeds 64 MiB".into()); }
        }
        for (from, to) in &self.redirects {
            for value in [&from.table, &from.key, &to.table, &to.key] {
                bytes = bytes.checked_add(value.len()).ok_or("Content snapshot size overflow")?;
            }
            if bytes > crate::locale::MAX_PORTABLE_TEXT_BYTES { return Err("Portable text content exceeds 64 MiB".into()); }
        }
        let snapshot = StringTableContentSnapshot {
            version: 1, lookup_policy: 1, redirect_limit: MAX_REDIRECTS,
            missing_entry_marker: MISSING_STRING_TABLE_ENTRY.into(),
            tables: self.tables.iter().map(|(id, slot)| (id.clone(), match &slot.state {
                TableState::Loading => StringTableContentState::Loading,
                TableState::Failed(message) => StringTableContentState::Failed(message.clone()),
                TableState::Ready(table) => StringTableContentState::Ready(table.as_ref().clone()),
            })).collect(),
            redirects: self.redirects.iter().map(|(from,to)| (from.clone(),to.clone())).collect(),
        };
        snapshot.validate()?;
        // Charge canonical wire overhead before returning the receipt as usable.
        snapshot.content_digest()?;
        Ok(snapshot)
    }

    pub fn portable_content_digest(&self) -> Result<[u8; 32], String> {
        self.portable_content_snapshot()?.content_digest()
    }

    /// Complete installed registry receipt. Host queues/providers are separate:
    /// no live loading token can be restored or accepted by content equality.
    pub fn portable_receipt(&self) -> Result<StringTableContentSnapshot, String> {
        let receipt = self.portable_content_snapshot()?;
        receipt.restore_digest()?;
        Ok(receipt)
    }

    pub fn accepts_portable_receipt(&self, expected: &StringTableContentSnapshot) -> Result<bool, String> {
        expected.restore_digest()?;
        Ok(self.portable_receipt()? == *expected)
    }

    pub fn revision_stamp(&self) -> StringTableRevision {
        StringTableRevision {
            authority: self.revision_identity.clone(),
            revision: self.revision,
        }
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }

    fn next_revision(&self) -> Result<u64, String> {
        self.revision
            .checked_add(1)
            .ok_or_else(|| "String table revision exhausted".into())
    }

    fn commit_revision(&mut self, revision: u64) {
        self.revision = revision;
        // Copy-on-write snapshots can diverge at the same numeric revision.
        // Each publication therefore owns an independent cache/trace identity.
        self.revision_identity = Arc::new(());
    }

    pub fn begin_load(&mut self, table: &str) -> Result<StringTableLoadToken, String> {
        validate_table_id(table)?;
        self.check_capacity(table)?;
        let revision = self.next_revision()?;
        let load_identity = Arc::new(());
        self.tables.insert(
            table.into(),
            TableSlot {
                revision,
                state: TableState::Loading,
                load_identity: Some(load_identity.clone()),
            },
        );
        self.commit_revision(revision);
        Ok(StringTableLoadToken {
            table: table.into(),
            revision,
            authority: self.load_authority.clone(),
            load_identity,
        })
    }

    pub fn publish(&mut self, table: StringTableAsset) -> Result<u64, String> {
        self.publish_shared(Arc::new(table))
    }

    pub fn ready_table(&self, id: &str) -> Option<&StringTableAsset> {
        match &self.tables.get(id)?.state {
            TableState::Ready(table) => Some(table),
            _ => None,
        }
    }

    /// Queries enumerate published tables in this engine's immutable snapshot.
    /// Pending/failed loads and redirects are not registered table assets.
    pub fn registered_tables(&self) -> impl Iterator<Item = &StringTableAsset> {
        self.tables.values().filter_map(|slot| match &slot.state {
            TableState::Ready(table) => Some(table.as_ref()),
            _ => None,
        })
    }

    /// Hosts may deliver Added and Loaded events for the same decoded asset.
    /// A semantic no-op preserves cache, debugger and continuation identities.
    pub fn publish_if_changed(&mut self, table: Arc<StringTableAsset>) -> Result<Option<u64>, String> {
        table.validate()?;
        if self.ready_table(&table.id) == Some(table.as_ref()) { return Ok(None); }
        self.publish_shared(table).map(Some)
    }

    pub fn publish_shared(&mut self, table: Arc<StringTableAsset>) -> Result<u64, String> {
        table.validate()?;
        self.check_capacity(&table.id)?;
        let revision = self.next_revision()?;
        self.tables.insert(
            table.id.clone(),
            TableSlot {
                revision,
                state: TableState::Ready(table),
                load_identity: None,
            },
        );
        self.commit_revision(revision);
        Ok(revision)
    }

    fn accepts(&self, token: &StringTableLoadToken) -> bool {
        Arc::ptr_eq(&self.load_authority, &token.authority)
            && self.tables.get(&token.table).is_some_and(|slot| {
                slot.revision == token.revision
                    && matches!(slot.state, TableState::Loading)
                    && slot
                        .load_identity
                        .as_ref()
                        .is_some_and(|identity| Arc::ptr_eq(identity, &token.load_identity))
            })
    }

    fn check_capacity(&self, id: &str) -> Result<(), String> {
        if !self.tables.contains_key(id)
            && self.tables.len() >= crate::scene_resources::MAX_RESOURCES
        {
            return Err("String table resources exceed 4,096 tables".into());
        }
        Ok(())
    }

    /// False means stale or already completed; neither resource state nor the
    /// revision changes. Validation also happens before any publication.
    pub fn finish_load(
        &mut self,
        token: &StringTableLoadToken,
        table: StringTableAsset,
    ) -> Result<bool, String> {
        if !self.accepts(token) {
            return Ok(false);
        }
        if table.id != token.table {
            return Err("Loaded string table does not match its token".into());
        }
        self.publish(table)?;
        Ok(true)
    }

    pub fn fail_load(
        &mut self,
        token: &StringTableLoadToken,
        diagnostic: String,
    ) -> Result<bool, String> {
        if !self.accepts(token) {
            return Ok(false);
        }
        if diagnostic.len() > 4096 {
            return Err("String table load diagnostic exceeds 4,096 bytes".into());
        }
        let revision = self.next_revision()?;
        self.tables.insert(
            token.table.clone(),
            TableSlot {
                revision,
                state: TableState::Failed(diagnostic),
                load_identity: None,
            },
        );
        self.commit_revision(revision);
        Ok(true)
    }

    pub fn unload(&mut self, table: &str) -> Result<bool, String> {
        if !self.tables.contains_key(table) {
            return Ok(false);
        }
        let revision = self.next_revision()?;
        self.tables.remove(table);
        self.commit_revision(revision);
        Ok(true)
    }

    /// Redirections belong to the asset host. They never rewrite the original
    /// persisted reference; resolution follows a bounded chain each time.
    pub fn redirect(
        &mut self,
        from: StringTableReference,
        to: StringTableReference,
    ) -> Result<bool, String> {
        from.validate()?;
        to.validate()?;
        if self.redirects.get(&from) == Some(&to) {
            return Ok(false);
        }
        if !self.redirects.contains_key(&from) && self.redirects.len() >= MAX_TABLE_ENTRIES {
            return Err("String table resources exceed 65,536 redirects".into());
        }
        let mut cursor = &to;
        for _ in 0..MAX_REDIRECTS {
            if cursor == &from {
                return Err("String table redirect would create a cycle".into());
            }
            match self.redirects.get(cursor) {
                Some(next) => cursor = next,
                None => {
                    let revision = self.next_revision()?;
                    self.redirects.insert(from, to);
                    self.commit_revision(revision);
                    return Ok(true);
                }
            }
        }
        Err("String table redirect chain exceeds 64 references".into())
    }

    pub fn remove_redirect(&mut self, from: &StringTableReference) -> Result<bool, String> {
        if !self.redirects.contains_key(from) {
            return Ok(false);
        }
        let revision = self.next_revision()?;
        self.redirects.remove(from);
        self.commit_revision(revision);
        Ok(true)
    }

    pub fn resolve<'a>(
        &'a self,
        reference: &'a StringTableReference,
    ) -> Result<StringTableResolution<'a>, String> {
        reference.validate()?;
        let mut current = reference;
        for _ in 0..=MAX_REDIRECTS {
            if let Some(next) = self.redirects.get(current) {
                current = next;
                continue;
            }
            return Ok(match self.tables.get(&current.table) {
                None => StringTableResolution::MissingTable,
                Some(slot) => match &slot.state {
                    TableState::Loading => StringTableResolution::Loading,
                    TableState::Failed(diagnostic) => StringTableResolution::Failed(diagnostic),
                    TableState::Ready(table) => match table.entries.get(&current.key) {
                        Some(entry) => StringTableResolution::Ready {
                            table,
                            key: &current.key,
                            entry,
                            revision: slot.revision,
                        },
                        None => StringTableResolution::MissingEntry,
                    },
                },
            });
        }
        Ok(StringTableResolution::RedirectLimit)
    }

    pub fn display(
        &self,
        reference: &StringTableReference,
        locale: Option<&LocaleManager>,
    ) -> Result<String, String> {
        self.display_with_lookup_work(reference, locale).map(|(display, _)| display)
    }

    pub(crate) fn display_with_lookup_work(
        &self,
        reference: &StringTableReference,
        locale: Option<&LocaleManager>,
    ) -> Result<(String, usize), String> {
        self.lookup_display(reference, locale, false)
    }

    pub(crate) fn project_with_lookup_work(
        &self,
        reference: &StringTableReference,
        locale: Option<&LocaleManager>,
    ) -> Result<(String, usize), String> {
        self.lookup_display(reference, locale, true)
    }

    /// Name semantics are opt-in. Legacy exact references and stored UUIDs are
    /// never rewritten; lookup uses only this published host snapshot.
    pub(crate) fn project_name_with_lookup_work(
        &self,
        reference: &StringTableReference,
        locale: Option<&LocaleManager>,
    ) -> Result<(String, usize, usize), String> {
        let (resolution, copies) = self.resolve_name(reference)?;
        let (display, locale_work) = self.lookup_resolution(resolution, locale, true)?;
        Ok((display, locale_work, copies))
    }

    fn resolve_name<'a>(&'a self, reference: &'a StringTableReference)
        -> Result<(StringTableResolution<'a>, usize), String> {
        use rvn_parser::Name;
        reference.validate_name()?;
        fn name(raw: &str, copies: &mut usize) -> Result<Name, String> {
            *copies = copies.checked_add(1 + raw.len().checked_mul(2).ok_or("Name lookup work overflow")? / 32)
                .ok_or("Name lookup work overflow")?;
            if *copies > 1_000_000 { return Err("Name lookup exceeds 1,000,000 value-copy units".into()); }
            Name::new(raw).map_err(|error| error.to_string())
        }
        fn scan_key(a: &str, b: &str, copies: &mut usize) -> Result<(), String> {
            let units = a.len().checked_add(b.len()).and_then(|bytes| (bytes / 32).checked_add(1))
                .ok_or("Name key lookup work overflow")?;
            *copies = copies.checked_add(units).ok_or("Name key lookup work overflow")?;
            if *copies > 1_000_000 { return Err("Name lookup exceeds 1,000,000 value-copy units".into()); }
            Ok(())
        }
        let mut copies = 0;
        // Store borrowed ASCII identities rather than clones of every Name.
        #[derive(Clone, Copy)] struct Identity<'a>(&'a str);
        impl PartialEq for Identity<'_> { fn eq(&self, other: &Self) -> bool { self.0.eq_ignore_ascii_case(other.0) } }
        impl Eq for Identity<'_> {}
        impl std::hash::Hash for Identity<'_> {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                state.write_usize(self.0.len());
                for byte in self.0.bytes() { state.write_u8(byte.to_ascii_lowercase()); }
            }
        }
        let mut registered = std::collections::HashSet::new();
        for table in self.registered_tables() {
            let id = name(&table.id, &mut copies)?;
            if id.is_none() { return Err("Legacy table ID resolves to None at the Name Text boundary".into()); }
            if !registered.insert(Identity(&table.id)) { return Err("Ambiguous Name table identity at the Name Text boundary".into()); }
        }
        let mut current = reference;
        for _ in 0..=MAX_REDIRECTS {
            let id = name(&current.table, &mut copies)?;
            let mut redirected = None;
            if !id.is_none() {
                for (from, to) in &self.redirects {
                    scan_key(&from.key, &current.key, &mut copies)?;
                    if from.key != current.key { continue; }
                    if name(&from.table, &mut copies)? == id {
                        if redirected.is_some() { return Err("Ambiguous Name table redirect".into()); }
                        redirected = Some(to);
                    }
                }
            }
            if let Some(next) = redirected {
                next.validate_name()?;
                current = next;
                continue;
            }
            let mut found = None;
            if !id.is_none() {
                for (stored, slot) in &self.tables {
                    // Unrepresentable pending/failed legacy handles remain
                    // isolated; published invalid IDs were diagnosed above.
                    let candidate = match name(stored, &mut copies) { Ok(id) => id, Err(error) if copies > 1_000_000 => return Err(error), Err(_) => continue };
                    if candidate == id {
                        if found.is_some() { return Err("Ambiguous Name table load identity".into()); }
                        found = Some(slot);
                    }
                }
            }
            let resolution = match found {
                None => StringTableResolution::MissingTable,
                Some(slot) => match &slot.state {
                    TableState::Loading => StringTableResolution::Loading,
                    TableState::Failed(reason) => StringTableResolution::Failed(reason),
                    TableState::Ready(table) => match table.entries.get(&current.key) {
                        Some(entry) => StringTableResolution::Ready { table, key: &current.key, entry, revision: slot.revision },
                        None => StringTableResolution::MissingEntry,
                    },
                },
            };
            return Ok((resolution, copies));
        }
        Ok((StringTableResolution::RedirectLimit, copies))
    }

    fn lookup_display(
        &self,
        reference: &StringTableReference,
        locale: Option<&LocaleManager>,
        present_unavailable: bool,
    ) -> Result<(String, usize), String> {
        let resolution = self.resolve(reference)?;
        self.lookup_resolution(resolution, locale, present_unavailable)
    }

    fn lookup_resolution(&self, resolution: StringTableResolution<'_>, locale: Option<&LocaleManager>, present_unavailable: bool) -> Result<(String, usize), String> {
        if present_unavailable && matches!(&resolution, StringTableResolution::MissingTable | StringTableResolution::Loading | StringTableResolution::Failed(_) | StringTableResolution::MissingEntry) {
            return Ok((MISSING_STRING_TABLE_ENTRY.into(), 0));
        }
        match resolution {
            StringTableResolution::Ready {
                table, key, entry, ..
            } => {
                let metadata = TextLocalization::Localizable {
                    identity: TextIdentity {
                        owner: format!("string-table:{}", table.id),
                        namespace: table.namespace.clone(),
                        key: key.into(),
                    },
                    developer_notes: String::new(),
                };
                let (display, lookup_work) = locale
                    .map(|l| l.resolve_text_with_lookup_work(&entry.source, Some(&metadata)))
                    .unwrap_or((&entry.source, 0));
                if display.len() > crate::value_limits::MAX_VALUE_BYTES {
                    return Err("String table display exceeds 1 MiB".into());
                }
                Ok((display.into(), lookup_work))
            }
            StringTableResolution::MissingTable => Err("String table is missing".into()),
            StringTableResolution::Loading => Err("String table is loading".into()),
            StringTableResolution::Failed(reason) => {
                Err(format!("String table load failed: {reason}"))
            }
            StringTableResolution::MissingEntry => Err("String table entry is missing".into()),
            StringTableResolution::RedirectLimit => {
                Err("String table redirect chain exceeds its limit".into())
            }
        }
    }
}
