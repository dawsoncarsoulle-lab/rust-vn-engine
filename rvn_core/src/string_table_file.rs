//! Versioned, independently authored VN string-table files. The codec performs
//! no filesystem access, identity allocation, gameplay evaluation or translation.
use crate::string_table::{StringTableAsset, StringTableEntry};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const STRING_TABLE_FILE_FORMAT: u32 = 1;
pub const STRING_TABLE_METADATA_FILE_FORMAT: u32 = 2;
pub const STRING_TABLE_FILE_EXTENSION: &str = "rvnstrings";
// A fully escaped JSON string may take six wire bytes per stored byte. The
// decoded table retains its existing 8 MiB content bound. The remaining wire
// allowance covers JSON fields/indentation for up to 65,536 entries and
// 65,536 metadata names across the table.
pub const MAX_STRING_TABLE_FILE_BYTES: usize = 64 * 1_048_576;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StringTableFile {
    table: StringTableAsset,
}

impl StringTableFile {
    pub fn new(table: StringTableAsset) -> Result<Self, String> {
        rvn_parser::localized_text::validate_text_asset_id(&table.id)?;
        table.validate()?;
        Ok(Self { table })
    }

    pub fn table(&self) -> &StringTableAsset { &self.table }
    pub fn into_table(self) -> StringTableAsset { self.table }

    /// Ordinary authored content edits retain the persistent asset ID. A
    /// duplication must explicitly construct an asset with a freshly allocated
    /// ID and pass the project's independent collision/reference transaction.
    pub fn edited(&self, table: StringTableAsset) -> Result<Self, String> {
        if table.id != self.table.id {
            return Err("A content edit cannot replace the persistent string-table asset ID".into());
        }
        Self::new(table)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_STRING_TABLE_FILE_BYTES {
            return Err("String-table file exceeds its wire byte budget".into());
        }
        let wire: Wire = serde_json::from_slice(bytes).map_err(|error| format!("Invalid string-table file: {error}"))?;
        if !matches!(wire.format_version, STRING_TABLE_FILE_FORMAT | STRING_TABLE_METADATA_FILE_FORMAT) {
            return Err(format!("Unsupported string-table file format {}", wire.format_version));
        }
        if wire.format_version == STRING_TABLE_FILE_FORMAT && wire.entries.values().any(|entry| entry.metadata.is_some()) {
            return Err("Metadata requires string-table file format 2".into());
        }
        Self::new(StringTableAsset {
            id: wire.id, namespace: wire.namespace,
            entries: wire.entries.into_iter().map(|(key, entry)| (key, StringTableEntry {
                source: entry.source, developer_notes: entry.developer_notes, metadata: entry.metadata.unwrap_or_default(),
            })).collect(),
        })
    }

    pub fn encode(&self) -> Result<Vec<u8>, String> {
        rvn_parser::localized_text::validate_text_asset_id(&self.table.id)?;
        self.table.validate()?;
        #[derive(Serialize)]
        struct WireRef<'a> {
            format_version: u32,
            id: &'a str,
            namespace: &'a str,
            entries: &'a BTreeMap<String, StringTableEntry>,
        }
        let mut bytes = serde_json::to_vec_pretty(&WireRef {
            format_version: if self.table.entries.values().any(|entry| !entry.metadata.is_empty()) {
                STRING_TABLE_METADATA_FILE_FORMAT
            } else { STRING_TABLE_FILE_FORMAT }, id: &self.table.id,
            namespace: &self.table.namespace, entries: &self.table.entries,
        }).map_err(|error| format!("Cannot encode string-table file: {error}"))?;
        bytes.push(b'\n');
        if bytes.len() > MAX_STRING_TABLE_FILE_BYTES {
            return Err("String-table file exceeds its wire byte budget".into());
        }
        Ok(bytes)
    }

    /// Prepare a complete CSV replacement while retaining asset identity and
    /// namespace. The caller decides whether to publish it and journal it.
    pub fn import_csv(&self, bytes: &[u8]) -> Result<Self, String> {
        let text = decode_csv_text(bytes)?;
        let mut rows = CsvRows::new(&text);
        let header = rows.next_row()?.ok_or("CSV requires a header and at least one content row")?;
        let mut key_column = None;
        let mut source_column = None;
        let mut notes_column = None;
        let mut metadata_columns = BTreeMap::new();
        for (index, name) in header.into_iter().enumerate() {
            if name.eq_ignore_ascii_case("Key") && key_column.is_none() { key_column = Some(index); }
            else if name.eq_ignore_ascii_case("SourceString") && source_column.is_none() { source_column = Some(index); }
            else if name.eq_ignore_ascii_case("DevNotes") && notes_column.is_none() { notes_column = Some(index); }
            else if !crate::string_table::is_absent_metadata_name(&name) {
                crate::string_table::validate_metadata_name(&name)?;
                metadata_columns.insert(name.to_ascii_lowercase(), (name, index));
            }
        }
        let key_column = key_column.ok_or("CSV is missing required column Key")?;
        let source_column = source_column.ok_or("CSV is missing required column SourceString")?;
        let mut table = StringTableAsset { id: self.table.id.clone(), namespace: self.table.namespace.clone(), entries: BTreeMap::new() };
        let mut content_bytes = table.content_bytes()?;
        let mut metadata_count = 0usize;
        let mut content_rows = 0usize;
        while let Some(cells) = rows.next_row()? {
            content_rows += 1;
            let (Some(key), Some(source)) = (cells.get(key_column), cells.get(source_column)) else { continue; };
            let key = csv_unescape(key);
            let source = csv_unescape(source);
            let notes = notes_column.and_then(|index| cells.get(index)).map(|text| csv_unescape(text)).unwrap_or_default();
            crate::string_table::validate_data_key(&key)?;
            if !table.entries.contains_key(&key) && table.entries.len() >= 65_536 { return Err("CSV exceeds 65,536 entries".into()); }
            let mut entry = match table.entries.remove(&key) {
                Some(entry) => { content_bytes -= entry_bytes(&entry); entry }
                None => {
                    content_bytes = content_bytes.checked_add(key.len()).ok_or("CSV content size overflow")?;
                    StringTableEntry { source: String::new(), developer_notes: String::new(), metadata: BTreeMap::new() }
                }
            };
            let previous_metadata_count = entry.metadata.len();
            entry.source = source;
            entry.developer_notes = notes;
            for (name, index) in metadata_columns.values() {
                if let Some(value) = cells.get(*index) {
                    let value = csv_unescape(value);
                    if !value.is_empty() { entry.metadata.insert(name.clone(), value); }
                }
            }
            metadata_count += entry.metadata.len() - previous_metadata_count;
            if metadata_count > 65_536 { return Err("CSV exceeds 65,536 metadata names across all entries".into()); }
            content_bytes = content_bytes.checked_add(entry_bytes(&entry)).ok_or("CSV content size overflow")?;
            table.entries.insert(key, entry);
            if content_bytes > 8 * 1_048_576 { return Err("CSV table exceeds 8 MiB".into()); }
        }
        if content_rows == 0 { return Err("CSV requires a header and at least one content row".into()); }
        self.edited(table)
    }

    /// Installed Windows reference's CSV data contract. Column and entry order
    /// are deterministic here, rather than depending on an engine name pool.
    /// Metadata headers remain raw, as observed in the reference: this format
    /// can lose special header names on re-import. .rvnstrings remains lossless.
    pub fn export_csv(&self) -> Result<Vec<u8>, String> {
        self.table.validate()?;
        let mut columns = BTreeMap::new();
        for entry in self.table.entries.values() {
            for name in entry.metadata.keys() { columns.entry(name.to_ascii_lowercase()).or_insert(name); }
        }
        let mut text = String::from("Key,SourceString,DevNotes");
        for name in columns.values() { text.push(','); text.push_str(name); }
        text.push('\n');
        for (key, entry) in &self.table.entries {
            let metadata: BTreeMap<_, _> = entry.metadata.iter().map(|(name, value)| (name.to_ascii_lowercase(), value.as_str())).collect();
            for (index, value) in [key.as_str(), entry.source.as_str(), entry.developer_notes.as_str()].into_iter().enumerate() {
                if index != 0 { text.push(','); }
                csv_quoted(&mut text, &csv_escape(value));
            }
            for canonical in columns.keys() {
                text.push(','); csv_quoted(&mut text, metadata.get(canonical).copied().unwrap_or(""));
                if text.len() > MAX_STRING_TABLE_FILE_BYTES { return Err("CSV exceeds its wire byte budget".into()); }
            }
            text.push('\n');
            if text.len() > MAX_STRING_TABLE_FILE_BYTES { return Err("CSV exceeds its wire byte budget".into()); }
        }
        if text.contains('\0') { return Err("CSV cannot represent NUL characters; use the string-table asset format".into()); }
        if text.is_ascii() { return Ok(text.into_bytes()); }
        let mut bytes = Vec::new(); bytes.extend_from_slice(&[0xff, 0xfe]);
        for unit in text.encode_utf16() {
            if bytes.len() + 2 > MAX_STRING_TABLE_FILE_BYTES { return Err("CSV exceeds its wire byte budget".into()); }
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        Ok(bytes)
    }
}

fn entry_bytes(entry: &StringTableEntry) -> usize {
    entry.source.len() + entry.developer_notes.len() + entry.metadata.iter().map(|(name, value)| name.len() + value.len()).sum::<usize>()
}

fn decode_csv_text(bytes: &[u8]) -> Result<String, String> {
    if bytes.len() > MAX_STRING_TABLE_FILE_BYTES { return Err("CSV exceeds its wire byte budget".into()); }
    let text = if bytes.starts_with(&[0xff, 0xfe]) || bytes.starts_with(&[0xfe, 0xff]) {
        if bytes.len() % 2 != 0 { return Err("CSV has incomplete UTF-16 data".into()); }
        let little = bytes[0] == 0xff;
        let units = bytes[2..].chunks_exact(2).map(|pair| if little { u16::from_le_bytes([pair[0], pair[1]]) } else { u16::from_be_bytes([pair[0], pair[1]]) });
        let mut text = String::new();
        for ch in char::decode_utf16(units) {
            text.push(ch.map_err(|_| "CSV contains invalid UTF-16")?);
            if text.len() > MAX_STRING_TABLE_FILE_BYTES { return Err("CSV decoded text exceeds its byte budget".into()); }
        }
        text
    } else {
        std::str::from_utf8(bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes)).map_err(|_| "CSV contains invalid UTF-8")?.to_owned()
    };
    if text.contains('\0') { return Err("CSV contains a NUL character".into()); }
    Ok(text)
}

fn csv_unescape(text: &str) -> String {
    // Ordered passes reproduce the measured double/triple-backslash behavior.
    // This is intentionally different from a JSON escape parser.
    text.replace("\\\"", "\"").replace("\\'", "'").replace("\\t", "\t").replace("\\T", "\t")
        .replace("\\r", "\r").replace("\\R", "\r").replace("\\n", "\n").replace("\\N", "\n").replace("\\\\", "\\")
}
fn csv_escape(text: &str) -> String {
    let mut out = String::new();
    for ch in text.chars() {
        match ch {
            '\\' => out.push_str("\\\\"), '\n' => out.push_str("\\n"), '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"), '\'' => out.push_str("\\'"), '"' => out.push_str("\\\""),
            _ => out.push(ch),
        }
    }
    out
}
fn csv_quoted(out: &mut String, text: &str) {
    out.push('"');
    for ch in text.chars() { if ch == '"' { out.push('"'); } out.push(ch); }
    out.push('"');
}

/// Streaming rows avoid retaining a second whole CSV cell matrix. VN limits
/// bound cell overhead as well as bytes; they are not claimed as Unreal limits.
struct CsvRows<'a> { text: &'a [u8], at: usize, rows: usize, cells: usize }
impl<'a> CsvRows<'a> {
    fn new(text: &'a str) -> Self { Self { text: text.as_bytes(), at: 0, rows: 0, cells: 0 } }
    fn newline(&self) -> usize {
        match self.text.get(self.at) { Some(b'\r') if self.text.get(self.at + 1) == Some(&b'\n') => 2, Some(b'\r' | b'\n') => 1, _ => 0 }
    }
    fn next_row(&mut self) -> Result<Option<Vec<String>>, String> {
        while self.newline() != 0 { self.at += self.newline(); }
        if self.at == self.text.len() { return Ok(None); }
        self.rows += 1;
        if self.rows > 131_073 { return Err("CSV exceeds the VN row budget".into()); }
        let mut row = Vec::new(); let mut row_bytes = 0usize;
        loop {
            self.cells += 1;
            if self.cells > 1_048_576 || row.len() >= 65_539 { return Err("CSV exceeds the VN cell budget".into()); }
            let mut quoted = self.text.get(self.at) == Some(&b'"');
            if quoted { self.at += 1; }
            let mut cell = Vec::new();
            while let Some(&byte) = self.text.get(self.at) {
                if quoted && byte == b'"' {
                    self.at += 1;
                    if self.text.get(self.at) == Some(&b'"') { cell.push(b'"'); self.at += 1; }
                    else { quoted = false; }
                } else if !quoted && (byte == b',' || self.newline() != 0) { break; }
                else { cell.push(byte); self.at += 1; }
                if cell.len() > MAX_STRING_TABLE_FILE_BYTES { return Err("CSV cell exceeds its wire byte budget".into()); }
            }
            row_bytes += cell.len();
            if row_bytes > MAX_STRING_TABLE_FILE_BYTES { return Err("CSV row exceeds its wire byte budget".into()); }
            row.push(String::from_utf8(cell).map_err(|_| "CSV cell contains invalid UTF-8")?);
            if self.text.get(self.at) == Some(&b',') { self.at += 1; }
            else { self.at += self.newline(); return Ok(Some(row)); }
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    format_version: u32,
    id: String,
    namespace: String,
    #[serde(deserialize_with = "unique_entries")]
    entries: BTreeMap<String, WireEntry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireEntry {
    source: String,
    #[serde(default)]
    developer_notes: String,
    #[serde(default, deserialize_with = "optional_metadata")]
    metadata: Option<BTreeMap<String, String>>,
}

fn optional_metadata<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Option<BTreeMap<String, String>>, D::Error> {
    // Present null is rejected, as is a duplicate/equivalent name. Missing is
    // distinguishable from an explicitly authored extension in legacy format1.
    crate::string_table::unique_metadata(deserializer).map(Some)
}

fn unique_entries<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<BTreeMap<String, WireEntry>, D::Error> {
    struct Entries;
    impl<'de> serde::de::Visitor<'de> for Entries {
        type Value = BTreeMap<String, WireEntry>;
        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("distinct string-table keys")
        }
        fn visit_map<M: serde::de::MapAccess<'de>>(self, mut map: M) -> Result<Self::Value, M::Error> {
            let mut entries = BTreeMap::new();
            while let Some((key, entry)) = map.next_entry::<String, WireEntry>()? {
                if entries.len() >= 65_536 {
                    return Err(serde::de::Error::custom("String table exceeds 65,536 entries"));
                }
                if entries.insert(key, entry).is_some() {
                    return Err(serde::de::Error::custom("Repeated string-table entry key"));
                }
            }
            Ok(entries)
        }
    }
    deserializer.deserialize_map(Entries)
}
