//! Host project index and bootstrap for authored VN string tables. Gameplay
//! rendering still receives immutable resources and performs no filesystem I/O.
use crate::{string_table::StringTableResources, string_table_file::{StringTableFile, MAX_STRING_TABLE_FILE_BYTES}};
use std::collections::{BTreeMap, BTreeSet};

pub type StringTableDeclarations = BTreeMap<String, String>;
const MAX_DECLARED_TABLES: usize = 4_096;
pub const MAX_PROJECT_TABLE_BYTES: usize = 64 * 1_048_576;

pub fn validate_declarations(declarations: &StringTableDeclarations) -> Result<(), String> {
    if declarations.len() > MAX_DECLARED_TABLES {
        return Err("A project declares more than 4,096 string tables".into());
    }
    let mut paths = BTreeSet::new();
    let mut bytes: usize = 0;
    for (id, path) in declarations {
        rvn_parser::localized_text::validate_text_asset_id(id)?;
        validate_asset_path(path)?;
        if !paths.insert(path) { return Err("Distinct table IDs claim the same asset path".into()); }
        bytes = bytes.checked_add(id.len()).and_then(|size| size.checked_add(path.len()))
            .ok_or("String-table declaration size overflow")?;
        if bytes > 2 * 1_048_576 { return Err("String-table declarations exceed 2 MiB".into()); }
    }
    Ok(())
}

pub fn validate_asset_path(path: &str) -> Result<(), String> {
    if path.is_empty() || path.len() > 4_096 || path.chars().any(|c| c.is_control() || matches!(c, '\\' | ':' | '#' | '?'))
        || path.split('/').any(|part| part.is_empty() || part == "." || part == "..")
        || path.split('/').count() > 64
        || !path.rsplit_once('.').is_some_and(|(_, extension)| extension.eq_ignore_ascii_case("rvnstrings")) {
        return Err("A table asset path must be a bounded relative .rvnstrings path inside the configured assets directory".into());
    }
    Ok(())
}

pub fn validate_web_manifest(declarations: &StringTableDeclarations, manifest: Option<&StringTableDeclarations>) -> Result<(), String> {
    validate_declarations(declarations)?;
    if manifest.is_some_and(|manifest| manifest != declarations) {
        return Err("Web string-table manifest differs from the project's declared table index".into());
    }
    Ok(())
}

/// One unpublished batch. Every supplied file must match its declared identity;
/// an incomplete or failed batch never reaches authored initialization.
pub struct StringTablePreparation {
    declarations: StringTableDeclarations,
    loaded: BTreeSet<String>,
    bytes: usize,
    resources: StringTableResources,
    source_hashes: BTreeMap<String, [u8; 32]>,
}

/// Validated resources and the exact supplied bytes' identity, useful to a
/// host's export transaction without serializing runtime loading state.
pub struct PreparedStringTables {
    resources: StringTableResources,
    source_hashes: BTreeMap<String, [u8; 32]>,
}
impl PreparedStringTables {
    pub fn into_resources(self) -> StringTableResources { self.resources }
    pub fn source_hashes(&self) -> &BTreeMap<String, [u8; 32]> { &self.source_hashes }
}

impl StringTablePreparation {
    pub fn new(declarations: &StringTableDeclarations) -> Result<Self, String> {
        validate_declarations(declarations)?;
        Ok(Self { declarations: declarations.clone(), loaded: BTreeSet::new(), bytes: 0, resources: StringTableResources::default(), source_hashes: BTreeMap::new() })
    }
    pub fn supply(&mut self, id: &str, bytes: &[u8]) -> Result<(), String> {
        if !self.declarations.contains_key(id) { return Err("An undeclared string-table asset was supplied".into()); }
        if self.loaded.contains(id) { return Err("A string-table asset was supplied more than once".into()); }
        let file = StringTableFile::decode(bytes)?;
        let table = file.table();
        if table.id != id { return Err("String-table file identity differs from its project declaration".into()); }
        let size = table.content_bytes()?;
        let total = self.bytes.checked_add(size).ok_or("Project string-table content size overflow")?;
        if total > MAX_PROJECT_TABLE_BYTES { return Err("Project string-table content exceeds 64 MiB".into()); }
        // All fallible checks precede insertion into this batch.
        self.resources.publish(file.into_table())?;
        use sha2::{Digest, Sha256};
        self.source_hashes.insert(id.to_string(), Sha256::digest(bytes).into());
        self.bytes = total;
        self.loaded.insert(id.to_string());
        Ok(())
    }
    pub fn finish(self) -> Result<StringTableResources, String> {
        self.finish_prepared().map(PreparedStringTables::into_resources)
    }
    pub fn finish_prepared(self) -> Result<PreparedStringTables, String> {
        if self.loaded.len() != self.declarations.len() {
            return Err("Not every declared string-table asset was loaded".into());
        }
        Ok(PreparedStringTables { resources: self.resources, source_hashes: self.source_hashes })
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn load_native(asset_root: &std::path::Path, declarations: &StringTableDeclarations) -> Result<StringTableResources, String> {
    prepare_native(asset_root, declarations).map(PreparedStringTables::into_resources)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn prepare_native(asset_root: &std::path::Path, declarations: &StringTableDeclarations) -> Result<PreparedStringTables, String> {
    use std::{fs, io::Read};
    let mut preparation = StringTablePreparation::new(declarations)?;
    if declarations.is_empty() { return preparation.finish_prepared(); }
    let root = asset_root.canonicalize().map_err(|error| format!("String-table asset root: {error}"))?;
    for (id, relative) in declarations {
        let requested = root.join(relative);
        let metadata = fs::symlink_metadata(&requested).map_err(|error| format!("String table {relative}: {error}"))?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(format!("String table {relative} must be a regular asset file"));
        }
        let path = requested.canonicalize().map_err(|error| format!("String table {relative}: {error}"))?;
        if !path.starts_with(&root) { return Err(format!("String table {relative} escaped its configured asset root")); }
        if metadata.len() > MAX_STRING_TABLE_FILE_BYTES as u64 { return Err(format!("String table {relative} exceeds its wire byte budget")); }
        let file = fs::File::open(&path).map_err(|error| format!("String table {relative}: {error}"))?;
        if !file.metadata().map_err(|error| error.to_string())?.is_file() { return Err(format!("String table {relative} is not a regular file")); }
        let mut bytes = Vec::new();
        file.take((MAX_STRING_TABLE_FILE_BYTES + 1) as u64).read_to_end(&mut bytes)
            .map_err(|error| format!("String table {relative}: {error}"))?;
        preparation.supply(id, &bytes).map_err(|error| format!("String table {relative}: {error}"))?;
    }
    preparation.finish_prepared()
}

/// Encode literal path bytes for HTTP, including UTF-8 and percent signs.
/// Slashes remain separators; the caller supplies its trusted project URL root.
pub fn url_path(path: &str) -> String {
    let mut encoded = String::new();
    use std::fmt::Write;
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~' | b'/') {
            encoded.push(byte as char);
        } else { write!(encoded, "%{byte:02X}").expect("writing to a String cannot fail"); }
    }
    encoded
}
