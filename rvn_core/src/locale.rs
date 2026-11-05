use std::collections::{HashMap, BTreeMap};
use serde::{Serialize, Deserialize};
use std::path::{Path, PathBuf};

// ─── ERREURS ─────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum LocaleError {
    Io(std::io::Error),
    Parse(String),
}

impl std::fmt::Display for LocaleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "erreur disque locale : {e}"),
            Self::Parse(e) => write!(f, "erreur parsing locale : {e}"),
        }
    }
}

impl From<std::io::Error> for LocaleError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

// ─── LOCALE TABLE ─────────────────────────────────────────────────────────────

/// Table de traduction pour une langue donnée.
/// Clé   = string originale (avec éventuels [var] non résolus).
/// Valeur = string traduite  (avec éventuels [var] non résolus).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct LocaleTable {
    pub lang: String,
    pub strings: HashMap<String, String>,
}

impl LocaleTable {
    /// Charge une table depuis un fichier TOML.
    pub fn load(lang: &str, path: &Path) -> Result<Self, LocaleError> {
        let content = std::fs::read_to_string(path)?;
        Self::parse(lang, &content)
    }

    /// Parse le contenu TOML d'un fichier de locale.
    pub fn parse(lang: &str, content: &str) -> Result<Self, LocaleError> {
        // On parse manuellement pour éviter une dépendance `toml` dans rvn_core.
        // Format attendu :
        //   [strings]
        //   "clé" = "valeur"
        //
        // Les clés et valeurs sont des strings TOML entre guillemets.
        let mut strings = HashMap::new();
        let mut in_strings = false;

        for (line_no, raw_line) in content.lines().enumerate() {
            let line = raw_line.trim();

            if line == "[strings]" {
                in_strings = true;
                continue;
            }
            if line.starts_with('[') {
                in_strings = false;
                continue;
            }

            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if !in_strings {
                continue;
            }

            match parse_kv_line(line) {
                Some((k, v)) => {
                    strings.insert(k, v);
                }
                None => {
                    return Err(LocaleError::Parse(format!(
                        "ligne {} mal formée : {:?}",
                        line_no + 1,
                        raw_line
                    )));
                }
            }
        }

        Ok(Self {
            lang: lang.to_string(),
            strings,
        })
    }

    /// Sérialise la table en contenu TOML.
    /// Utilisé pour générer automatiquement `fr.toml` (langue par défaut).
    pub fn to_toml(&self) -> String {
        let mut out = format!(
            "# Fichier de localisation — langue : {}\n\
             # Clés = textes originaux du script.\n\
             # Valeurs = traductions. Gardez les [variables] intactes.\n\
             \n\
             [strings]\n",
            self.lang
        );
        let mut keys: Vec<&String> = self.strings.keys().collect();
        keys.sort();
        for key in keys {
            let val = &self.strings[key];
            out.push_str(&format!("{} = {}\n", toml_quote(key), toml_quote(val)));
        }
        out
    }
}

/// A locale table with optional source witnesses for explicit Text identities.
/// Old tables remain key-only until their translation author records a witness.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct LocaleCatalog {
    table: LocaleTable,
    text_sources: HashMap<String, String>,
}

impl From<LocaleTable> for LocaleCatalog {
    fn from(table: LocaleTable) -> Self {
        Self { table, text_sources: HashMap::new() }
    }
}

impl LocaleCatalog {
    pub fn with_text_sources(table: LocaleTable, text_sources: HashMap<String, String>) -> Result<Self, LocaleError> {
        let catalog = Self { table, text_sources };
        catalog.validate_sources()?;
        Ok(catalog)
    }

    pub fn table(&self) -> &LocaleTable { &self.table }
    pub fn text_sources(&self) -> &HashMap<String, String> { &self.text_sources }

    pub fn load(lang: &str, path: &Path) -> Result<Self, LocaleError> {
        Self::parse(lang, &std::fs::read_to_string(path)?)
    }

    pub fn parse(lang: &str, content: &str) -> Result<Self, LocaleError> {
        let table = LocaleTable::parse(lang, content)?;
        let mut text_sources = HashMap::new();
        let (mut in_sources, mut seen_section) = (false, false);
        for (line_no, raw_line) in content.lines().enumerate() {
            let line = raw_line.trim();
            if line == "[text_sources]" {
                if seen_section { return Err(LocaleError::Parse("duplicate text_sources section".into())); }
                seen_section = true;
                in_sources = true;
                continue;
            }
            if line.starts_with('[') { in_sources = false; continue; }
            if !in_sources || line.is_empty() || line.starts_with('#') { continue; }
            let (key, source) = parse_kv_line(line)
                .ok_or_else(|| LocaleError::Parse(format!("invalid text source at line {}", line_no + 1)))?;
            if text_sources.insert(key, source).is_some() {
                return Err(LocaleError::Parse("duplicate text source witness".into()));
            }
        }
        Self::with_text_sources(table, text_sources)
    }

    pub fn to_toml(&self) -> String {
        let mut out = self.table.to_toml();
        if !self.text_sources.is_empty() {
            out.push_str("\n[text_sources]\n");
            let mut keys: Vec<_> = self.text_sources.keys().collect();
            keys.sort();
            for key in keys {
                out.push_str(&format!("{} = {}\n", toml_quote(key), toml_quote(&self.text_sources[key])));
            }
        }
        out
    }

    /// Explicitly migrate a translation whose source has been reviewed.
    /// A different existing witness requires a reviewed replacement catalog.
    pub fn record_text_source_witness(&mut self, identity: &rvn_parser::localized_text::TextIdentity, source: &str) -> Result<bool, LocaleError> {
        identity.validate().map_err(LocaleError::Parse)?;
        self.record_source_at_key(identity.locale_key(), source)
    }

    /// Record a reviewed translation against the actual bounded table entry.
    /// A free-form owner prefix is never evidence of table origin or authority.
    /// Old authored Text identity admission remains unchanged in the API above.
    pub fn record_string_table_source_witness(&mut self, table: &crate::string_table::StringTableAsset, key: &str) -> Result<bool, LocaleError> {
        table.validate().map_err(LocaleError::Parse)?;
        let entry = table.entries.get(key).ok_or_else(|| LocaleError::Parse("string-table witness requires an actual entry".into()))?;
        let localization = table.localization(key).map_err(LocaleError::Parse)?;
        let identity = localization.identity().ok_or_else(|| LocaleError::Parse("string-table witness needs an identity".into()))?;
        self.record_source_at_key(identity.locale_key(), &entry.source)
    }

    fn record_source_at_key(&mut self, key: String, source: &str) -> Result<bool, LocaleError> {
        Self::validate_source(&key, source)?;
        if !self.table.strings.contains_key(&key) {
            return Err(LocaleError::Parse("a source witness requires an existing identity translation".into()));
        }
        if let Some(previous) = self.text_sources.get(&key) {
            return if previous == source { Ok(false) }
                else { Err(LocaleError::Parse("source witness differs; review the translation before replacing the catalog".into())) };
        }
        let bytes = self.text_sources.iter().try_fold(key.len() + source.len(), |sum, (key, source)|
            sum.checked_add(key.len()).and_then(|sum| sum.checked_add(source.len())))
            .ok_or_else(|| LocaleError::Parse("text source witness budget exceeded".into()))?;
        if self.text_sources.len() >= 65_536 || bytes > 8 * 1024 * 1024 {
            return Err(LocaleError::Parse("text source witness budget exceeded".into()));
        }
        self.text_sources.insert(key, source.to_string());
        Ok(true)
    }

    fn validate_source(key: &str, source: &str) -> Result<(), LocaleError> {
        if key.len() > 1024 * 1024 || source.len() > 1024 * 1024 || !valid_text_locale_key(key) {
            return Err(LocaleError::Parse("invalid or oversized text source witness".into()));
        }
        Ok(())
    }

    fn validate_sources(&self) -> Result<(), LocaleError> {
        if self.text_sources.len() > 65_536 {
            return Err(LocaleError::Parse("too many text source witnesses".into()));
        }
        let mut bytes: usize = 0;
        for (key, source) in &self.text_sources {
            Self::validate_source(key, source)?;
            if !self.table.strings.contains_key(key) {
                return Err(LocaleError::Parse("orphan text source witness".into()));
            }
            bytes = bytes.checked_add(key.len()).and_then(|sum| sum.checked_add(source.len()))
                .ok_or_else(|| LocaleError::Parse("text source witness budget exceeded".into()))?;
            if bytes > 8 * 1024 * 1024 {
                return Err(LocaleError::Parse("text source witness budget exceeded".into()));
            }
        }
        Ok(())
    }

    fn resolve_identity<'a>(&'a self, key: &str, source: &str) -> (Option<&'a str>, usize) {
        let Some(display) = self.table.strings.get(key) else { return (None, 0); };
        if !source.is_empty() {
            if let Some(witness) = self.text_sources.get(key) {
                // Charge the whole supplied source conservatively, including
                // failed comparisons, instead of letting a tiny display hide
                // the work of comparing a large table entry on every lookup.
                return ((witness == source).then_some(display.as_str()), source.len());
            }
        }
        (Some(display.as_str()), 0)
    }
}

fn valid_text_locale_key(key: &str) -> bool {
    let Some(mut remaining) = key.strip_prefix("@rvn.text:") else { return false; };
    let mut parts = Vec::with_capacity(3);
    for index in 0..3 {
        let Some((length, value)) = remaining.split_once(':') else { return false; };
        let Ok(length) = length.parse::<usize>() else { return false; };
        let Some(part) = value.get(..length) else { return false; };
        parts.push(part.to_string());
        remaining = &value[length..];
        if index < 2 {
            let Some(rest) = remaining.strip_prefix(':') else { return false; };
            remaining = rest;
        }
    }
    if !remaining.is_empty() { return false; }
    let identity = rvn_parser::localized_text::TextIdentity {
        owner: parts.remove(0), namespace: parts.remove(0), key: parts.remove(0),
    };
    // This parses a catalog address, not an authored identifier or an owner.
    // Length framing and canonical roundtrip still reject malformed addresses.
    !identity.owner.trim().is_empty()
        && !identity.owner.contains('\0')
        && rvn_parser::localized_text::validate_text_identifier(&identity.namespace).is_ok()
        && crate::string_table::validate_data_key(&identity.key).is_ok()
        && identity.locale_key() == key
}



pub(crate) const MAX_PORTABLE_TEXT_BYTES: usize = 64 * 1024 * 1024;
pub(crate) fn check_portable_text_items(counts: impl IntoIterator<Item=usize>) -> Result<(), String> {
    let mut total=0usize;
    for count in counts {
        total=total.checked_add(count).filter(|sum|*sum<=1_000_000).ok_or("Portable text content exceeds 1,000,000 container items")?;
    }
    Ok(())
}
const MAX_PORTABLE_LOCALES: usize = 4096;
const MAX_PORTABLE_LOCALE_ENTRIES: usize = 65_536;

pub(crate) fn canonical_text_digest(value: &impl Serialize) -> Result<[u8;32], String> {
    use sha2::{Digest, Sha256};
    struct Sink { digest: Sha256, bytes: usize }
    impl std::io::Write for Sink {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            let next = self.bytes.checked_add(bytes.len()).filter(|size| *size <= MAX_PORTABLE_TEXT_BYTES)
                .ok_or_else(|| std::io::Error::other("Portable text wire exceeds 64 MiB"))?;
            self.digest.update(bytes);self.bytes=next;Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
    }
    let mut sink=Sink {digest:Sha256::new(),bytes:0};
    sink.digest.update(b"rvn.portable-text-content.v1\0");
    serde_json::to_writer(&mut sink,value).map_err(|error| error.to_string())?;
    Ok(sink.digest.finalize().into())
}

/// Provider policy is observable and separate from installed translation content.
/// Legacy constructors permit an uncached locale to load from the live directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LocaleProviderPolicy { ExternalFilesystem, FrozenCatalogs }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocaleCatalogSnapshot {
    pub lang: String,
    pub strings: BTreeMap<String, String>,
    pub text_sources: BTreeMap<String, String>,
}

impl LocaleCatalogSnapshot {
    fn borrowed_preflight(catalog: &LocaleCatalog, bytes: &mut usize) -> Result<(), String> {
        catalog.validate_sources().map_err(|error| error.to_string())?;
        if catalog.table.strings.len() > MAX_PORTABLE_LOCALE_ENTRIES { return Err("Portable locale exceeds 65,536 entries".into()); }
        add_portable_bytes(bytes, catalog.table.lang.len())?;
        for (key,value) in catalog.table.strings.iter().chain(catalog.text_sources.iter()) {
            add_portable_bytes(bytes,key.len())?;add_portable_bytes(bytes,value.len())?;
        }
        Ok(())
    }
    fn from_catalog(catalog: &LocaleCatalog) -> Self {
        Self {lang:catalog.table.lang.clone(),strings:catalog.table.strings.iter().map(|(k,v)|(k.clone(),v.clone())).collect(),
            text_sources:catalog.text_sources.iter().map(|(k,v)|(k.clone(),v.clone())).collect()}
    }
    fn validate(&self, bytes:&mut usize) -> Result<(),String> {
        if self.strings.len()>MAX_PORTABLE_LOCALE_ENTRIES || self.text_sources.len()>MAX_PORTABLE_LOCALE_ENTRIES {
            return Err("Portable locale entry count budget exceeded".into());
        }
        add_portable_bytes(bytes,self.lang.len())?;
        let mut witness_bytes=0usize;
        for (key,value) in &self.strings {add_portable_bytes(bytes,key.len())?;add_portable_bytes(bytes,value.len())?;}
        for (key,value) in &self.text_sources {
            LocaleCatalog::validate_source(key,value).map_err(|error|error.to_string())?;
            if !self.strings.contains_key(key) {return Err("Orphan portable text source witness".into());}
            witness_bytes=witness_bytes.checked_add(key.len()).and_then(|n|n.checked_add(value.len())).ok_or("Locale size overflow")?;
            if witness_bytes>8*1024*1024 {return Err("Text source witness budget exceeded".into());}
            add_portable_bytes(bytes,key.len())?;add_portable_bytes(bytes,value.len())?;
        }
        Ok(())
    }
}

fn add_portable_bytes(total:&mut usize,size:usize)->Result<(),String> {
    *total=total.checked_add(size).filter(|sum|*sum<=MAX_PORTABLE_TEXT_BYTES).ok_or("Portable text content exceeds 64 MiB")?;Ok(())
}

/// Current/fallback/cache values are separate because Legacy edits can make the
/// selected catalogue differ from the cached copy. Directory paths are excluded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocaleContentSnapshot {
    pub version:u32,
    pub lookup_policy:u32,
    pub provider:LocaleProviderPolicy,
    pub default_lang:String,
    pub current_lang:String,
    pub available_langs:Vec<String>,
    pub current:LocaleCatalogSnapshot,
    pub fallback:LocaleCatalogSnapshot,
    pub cache:BTreeMap<String,LocaleCatalogSnapshot>,
}

impl LocaleContentSnapshot {
    pub fn validate(&self)->Result<(),String> {
        if self.version!=1 || self.lookup_policy!=1 {return Err("Unsupported locale content policy".into());}
        if self.available_langs.len()>MAX_PORTABLE_LOCALES || self.cache.len()>MAX_PORTABLE_LOCALES {return Err("Portable locale count budget exceeded".into());}
        check_portable_text_items([self.available_langs.len(),self.cache.len()].into_iter().chain(
            [&self.current,&self.fallback].into_iter().chain(self.cache.values()).flat_map(|catalog|[catalog.strings.len(),catalog.text_sources.len()])))?;
        let mut bytes=0;add_portable_bytes(&mut bytes,self.default_lang.len())?;add_portable_bytes(&mut bytes,self.current_lang.len())?;
        for lang in &self.available_langs {add_portable_bytes(&mut bytes,lang.len())?;}
        self.current.validate(&mut bytes)?;self.fallback.validate(&mut bytes)?;
        for (lang,catalog) in &self.cache {add_portable_bytes(&mut bytes,lang.len())?;catalog.validate(&mut bytes)?;}
        if self.provider==LocaleProviderPolicy::FrozenCatalogs {
            let available:std::collections::BTreeSet<_>=self.available_langs.iter().collect();
            if available.len()!=self.available_langs.len() || self.available_langs.iter().any(|lang|!self.cache.contains_key(lang))
                || available.len()!=self.cache.len() || !available.contains(&self.default_lang) || !available.contains(&self.current_lang)
                || self.cache.iter().any(|(lang,catalog)|lang!=&catalog.lang)
                || self.cache.get(&self.default_lang)!=Some(&self.fallback) || self.cache.get(&self.current_lang)!=Some(&self.current) {
                return Err("Frozen locale provider must own every declared catalogue and selected value".into());
            }
            for lang in &self.available_langs {
                if lang.is_empty() || lang.len()>256 || !lang.bytes().all(|byte|byte.is_ascii_alphanumeric() || matches!(byte,b'-'|b'_')) {
                    return Err("Invalid frozen locale identifier".into());
                }
            }
        }
        Ok(())
    }
    pub fn content_digest(&self)->Result<[u8;32],String> {self.validate()?;canonical_text_digest(self)}
    /// This requires an actually closed provider, never an empty claim about disk.
    pub fn restore_digest(&self)->Result<[u8;32],String> {
        if self.provider!=LocaleProviderPolicy::FrozenCatalogs {return Err("External locale provider needs a verified host closure; installed content alone is insufficient".into());}
        self.content_digest()
    }
}

// ─── LOCALE MANAGER ───────────────────────────────────────────────────────────

/// Gestionnaire central de localisation.
/// Maintient la table de la langue courante + la table de fallback.
#[derive(Debug, Clone)]
pub struct LocaleManager {
    /// Répertoire contenant les fichiers de locale (ex: assets/locales/).
    pub locale_dir: PathBuf,
    /// Langue par défaut (fallback si clé manquante).
    pub default_lang: String,
    /// Langue courante.
    pub current_lang: String,
    /// Langues disponibles.
    pub available_langs: Vec<String>,
    /// Table de la langue courante.
    current_table: LocaleCatalog,
    /// Table de fallback (langue par défaut).
    fallback_table: LocaleCatalog,
    /// Tables préchargées, utilisées par le runtime web sans accès disque.
    table_cache: HashMap<String, LocaleCatalog>,
    provider: LocaleProviderPolicy,
    revision: LocaleRevision,
}

/// Live authority only: portable receipts contain catalogue semantics, never
/// this process-local identity. Reading/comparing a stamp does no catalogue work.
#[derive(Debug, Clone)]
pub struct LocaleRevision { authority: std::sync::Arc<()>, generation: u64 }
impl Default for LocaleRevision {
    fn default() -> Self { Self { authority: std::sync::Arc::new(()), generation: 0 } }
}
impl PartialEq for LocaleRevision {
    fn eq(&self, other:&Self)->bool { self.generation==other.generation && std::sync::Arc::ptr_eq(&self.authority,&other.authority) }
}
impl Eq for LocaleRevision {}
impl PartialEq for LocaleManager {
    fn eq(&self, other:&Self)->bool {
        self.locale_dir==other.locale_dir && self.default_lang==other.default_lang && self.current_lang==other.current_lang
            && self.available_langs==other.available_langs && self.current_table==other.current_table
            && self.fallback_table==other.fallback_table && self.table_cache==other.table_cache && self.provider==other.provider
    }
}
impl Eq for LocaleManager {}

impl LocaleManager {
    pub fn revision_stamp(&self)->LocaleRevision { self.revision.clone() }
    fn invalidate_live_revision(&mut self) {
        match self.revision.generation.checked_add(1) {
            Some(generation)=>self.revision=LocaleRevision{authority:std::sync::Arc::new(()),generation},
            None=>self.revision=LocaleRevision::default(),
        }
    }
    pub(crate) fn debugger_copy(&self)->Self {
        Self{locale_dir:self.locale_dir.clone(),default_lang:self.default_lang.clone(),current_lang:self.current_lang.clone(),
            available_langs:self.available_langs.clone(),current_table:self.current_table.clone(),fallback_table:self.fallback_table.clone(),table_cache:self.table_cache.clone(),provider:self.provider,revision:self.revision.clone()}
    }
    pub(crate) fn debugger_write(&self,out:&mut impl std::fmt::Write)->std::fmt::Result {
        write!(out,"{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}",self.locale_dir,self.default_lang,self.current_lang,self.available_langs,self.current_table,self.fallback_table,self.table_cache,self.provider,self.revision.generation)
    }
    /// Crée un LocaleManager et charge les tables nécessaires.
    pub fn new(
        locale_dir: impl AsRef<Path>,
        default_lang: &str,
        current_lang: &str,
        available_langs: Vec<String>,
    ) -> Result<Self, LocaleError> {
        let dir = locale_dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&dir)?;

        let fallback_table = Self::load_or_empty(default_lang, &dir);
        let current_table = if current_lang == default_lang {
            fallback_table.clone()
        } else {
            Self::load_or_empty(current_lang, &dir)
        };

        Ok(Self {
            locale_dir: dir,
            default_lang: default_lang.to_string(),
            current_lang: current_lang.to_string(),
            available_langs,
            current_table,
            fallback_table,
            table_cache: HashMap::new(),
            provider: LocaleProviderPolicy::ExternalFilesystem,
            revision: LocaleRevision::default(),
        })
    }

    pub fn from_tables(
        locale_dir: impl AsRef<Path>, default_lang: &str, current_lang: &str,
        available_langs: Vec<String>, table_cache: HashMap<String, LocaleTable>,
    ) -> Self {
        Self::from_catalogs(locale_dir, default_lang, current_lang, available_langs,
            table_cache.into_iter().map(|(lang, table)| (lang, table.into())).collect())
    }

    pub fn from_catalogs(
        locale_dir: impl AsRef<Path>,
        default_lang: &str,
        current_lang: &str,
        available_langs: Vec<String>,
        mut table_cache: HashMap<String, LocaleCatalog>,
    ) -> Self {
        let fallback_table = table_cache
            .remove(default_lang)
            .unwrap_or_else(|| LocaleTable {
                lang: default_lang.to_string(),
                strings: HashMap::new(),
            }.into());
        let current_table = if current_lang == default_lang {
            fallback_table.clone()
        } else {
            table_cache
                .remove(current_lang)
                .unwrap_or_else(|| LocaleTable {
                    lang: current_lang.to_string(),
                    strings: HashMap::new(),
                }.into())
        };
        table_cache.insert(default_lang.to_string(), fallback_table.clone());
        table_cache.insert(current_lang.to_string(), current_table.clone());

        Self {
            locale_dir: locale_dir.as_ref().to_path_buf(),
            default_lang: default_lang.to_string(),
            current_lang: current_lang.to_string(),
            available_langs,
            current_table,
            fallback_table,
            table_cache,
            provider: LocaleProviderPolicy::ExternalFilesystem,
            revision: LocaleRevision::default(),
        }
    }

    /// Explicit complete host publication: all permitted languages are decoded,
    /// validated, and owned. No future filesystem fallback or reload is possible.
    /// Equivalent assets relocated to a new host produce the same content receipt.
    pub fn from_frozen_catalogs(
        default_lang:&str,current_lang:&str,available_langs:Vec<String>,catalogs:HashMap<String,LocaleCatalog>,
    )->Result<Self,String> {
        // Borrowed preflight precedes clone/allocation by from_catalogs.
        let mut bytes=0;
        if available_langs.len()>MAX_PORTABLE_LOCALES || catalogs.len()>MAX_PORTABLE_LOCALES {return Err("Portable locale count budget exceeded".into());}
        let selected=[catalogs.get(default_lang),catalogs.get(current_lang)];
        check_portable_text_items([available_langs.len(),catalogs.len()].into_iter().chain(
            catalogs.values().chain(selected.into_iter().flatten()).flat_map(|catalog|[catalog.table.strings.len(),catalog.text_sources.len()])))?;
        for (lang,catalog) in &catalogs {add_portable_bytes(&mut bytes,lang.len())?;LocaleCatalogSnapshot::borrowed_preflight(catalog,&mut bytes)?;}
        if !catalogs.contains_key(default_lang) || !catalogs.contains_key(current_lang) {return Err("Frozen locale provider lacks a selected catalogue".into());}
        add_portable_bytes(&mut bytes,default_lang.len())?;add_portable_bytes(&mut bytes,current_lang.len())?;
        for lang in &available_langs {add_portable_bytes(&mut bytes,lang.len())?;}
        // Active and fallback copies are represented independently in the receipt.
        LocaleCatalogSnapshot::borrowed_preflight(&catalogs[default_lang],&mut bytes)?;
        LocaleCatalogSnapshot::borrowed_preflight(&catalogs[current_lang],&mut bytes)?;
        let mut manager=Self::from_catalogs(Path::new(""),default_lang,current_lang,available_langs,catalogs);
        manager.provider=LocaleProviderPolicy::FrozenCatalogs;
        manager.portable_content_snapshot()?.restore_digest()?;
        Ok(manager)
    }

    pub fn provider_policy(&self)->LocaleProviderPolicy {self.provider}

    pub fn portable_content_snapshot(&self)->Result<LocaleContentSnapshot,String> {
        if self.available_langs.len()>MAX_PORTABLE_LOCALES || self.table_cache.len()>MAX_PORTABLE_LOCALES {return Err("Portable locale count budget exceeded".into());}
        check_portable_text_items([self.available_langs.len(),self.table_cache.len()].into_iter().chain(
            [&self.current_table,&self.fallback_table].into_iter().chain(self.table_cache.values()).flat_map(|catalog|[catalog.table.strings.len(),catalog.text_sources.len()])))?;
        let mut bytes=0;add_portable_bytes(&mut bytes,self.default_lang.len())?;add_portable_bytes(&mut bytes,self.current_lang.len())?;
        for lang in &self.available_langs {add_portable_bytes(&mut bytes,lang.len())?;}
        LocaleCatalogSnapshot::borrowed_preflight(&self.current_table,&mut bytes)?;
        LocaleCatalogSnapshot::borrowed_preflight(&self.fallback_table,&mut bytes)?;
        for (lang,catalog) in &self.table_cache {add_portable_bytes(&mut bytes,lang.len())?;LocaleCatalogSnapshot::borrowed_preflight(catalog,&mut bytes)?;}
        let snapshot=LocaleContentSnapshot {version:1,lookup_policy:1,provider:self.provider,
            default_lang:self.default_lang.clone(),current_lang:self.current_lang.clone(),available_langs:self.available_langs.clone(),
            current:LocaleCatalogSnapshot::from_catalog(&self.current_table),fallback:LocaleCatalogSnapshot::from_catalog(&self.fallback_table),
            cache:self.table_cache.iter().map(|(lang,catalog)|(lang.clone(),LocaleCatalogSnapshot::from_catalog(catalog))).collect()};
        snapshot.content_digest()?;Ok(snapshot)
    }
    pub fn portable_content_digest(&self)->Result<[u8;32],String> {self.portable_content_snapshot()?.content_digest()}
    pub fn portable_restore_digest(&self)->Result<[u8;32],String> {self.portable_content_snapshot()?.restore_digest()}

    pub fn portable_receipt(&self)->Result<LocaleContentSnapshot,String> {
        let receipt=self.portable_content_snapshot()?;receipt.restore_digest()?;Ok(receipt)
    }
    pub fn accepts_portable_receipt(&self,expected:&LocaleContentSnapshot)->Result<bool,String> {
        expected.restore_digest()?;Ok(self.portable_receipt()?==*expected)
    }

    /// Charge un fichier de locale ou retourne une table vide si absent.
    fn load_or_empty(lang: &str, dir: &Path) -> LocaleCatalog {
        let path = dir.join(format!("{lang}.toml"));
        if path.exists() {
            LocaleCatalog::load(lang, &path).unwrap_or_else(|e| {
                eprintln!("[locale] erreur chargement {lang}.toml : {e}");
                LocaleTable {
                    lang: lang.to_string(),
                    strings: HashMap::new(),
                }.into()
            })
        } else {
            LocaleTable {
                lang: lang.to_string(),
                strings: HashMap::new(),
            }.into()
        }
    }

    /// Traduit une string.
    ///
    /// Ordre de résolution :
    ///   1. Table de la langue courante
    ///   2. Table de fallback (langue par défaut)
    ///   3. La string originale elle-même
    pub fn translate<'a>(&'a self, original: &'a str) -> &'a str {
        if let Some(t) = self.current_table.table.strings.get(original) {
            return t.as_str();
        }
        if self.current_lang != self.default_lang {
            if let Some(t) = self.fallback_table.table.strings.get(original) {
                return t.as_str();
            }
        }
        original
    }

    /// Resolve an explicit text identity without falling through to unrelated
    /// source-key translations. Legacy text continues to use `translate`.
    pub fn resolve_text<'a>(
        &'a self, source: &'a str,
        localization: Option<&rvn_parser::localized_text::TextLocalization>,
    ) -> &'a str {
        self.resolve_text_with_lookup_work(source, localization).0
    }

    pub(crate) fn resolve_text_with_lookup_work<'a>(
        &'a self, source: &'a str,
        localization: Option<&rvn_parser::localized_text::TextLocalization>,
    ) -> (&'a str, usize) {
        use rvn_parser::localized_text::TextLocalization;
        match localization {
            None => (self.translate(source), 0),
            Some(TextLocalization::Empty | TextLocalization::CultureInvariant) => (source, 0),
            Some(TextLocalization::Localizable { identity, .. }) => {
                let key = identity.locale_key();
                let (display, mut work) = self.current_table.resolve_identity(&key, source);
                if let Some(display) = display { return (display, work); }
                if self.current_lang != self.default_lang {
                    let (display, fallback_work) = self.fallback_table.resolve_identity(&key, source);
                    work = work.saturating_add(fallback_work);
                    if let Some(display) = display { return (display, work); }
                }
                (source, work)
            }
        }
    }

    /// Change la langue courante et recharge la table.
    pub fn set_language(&mut self, lang: &str) -> Result<(), LocaleError> {
        if lang == self.current_lang {
            return Ok(());
        }
        if self.provider==LocaleProviderPolicy::FrozenCatalogs {
            let table=self.table_cache.get(lang).filter(|_|self.available_langs.iter().any(|value|value==lang))
                .ok_or_else(||LocaleError::Parse("Locale is outside the frozen host publication".into()))?.clone();
            self.current_lang=lang.into();self.current_table=table;self.invalidate_live_revision();return Ok(());
        }
        self.current_lang = lang.to_string();
        self.current_table = self
            .table_cache
            .get(lang)
            .cloned()
            .unwrap_or_else(|| Self::load_or_empty(lang, &self.locale_dir));
        self.invalidate_live_revision();
        Ok(())
    }

    /// Recharge la table de la langue courante depuis le disque.
    /// Utilisé par le hot-reload.
    pub fn reload_current(&mut self) {
        if self.provider==LocaleProviderPolicy::FrozenCatalogs || !self.table_cache.is_empty() {
            return;
        }
        self.invalidate_live_revision();
        self.current_table = Self::load_or_empty(&self.current_lang.clone(), &self.locale_dir);
        if self.current_lang != self.default_lang {
            self.fallback_table = Self::load_or_empty(&self.default_lang.clone(), &self.locale_dir);
        }
    }

    /// Ajoute les strings manquantes dans le fichier de la langue par défaut.
    /// Appelé après le parsing du script pour générer/compléter `fr.toml`.
    /// Ne touche pas aux traductions existantes.
    pub fn update_default_locale(&mut self, strings: &[String]) -> Result<bool, LocaleError> {
        if self.provider==LocaleProviderPolicy::FrozenCatalogs {
            if strings.len()>MAX_PORTABLE_LOCALE_ENTRIES {return Err(LocaleError::Parse("Portable locale entry count budget exceeded".into()));}
            let mut bytes=0;
            for text in strings {add_portable_bytes(&mut bytes,text.len()).map_err(LocaleError::Parse)?;}
            let mut candidate=self.clone();
            let mut changed=false;
            for text in strings {
                if !candidate.fallback_table.table.strings.contains_key(text) {
                    candidate.fallback_table.table.strings.insert(text.clone(),text.clone());changed=true;
                }
            }
            if !changed {return Ok(false);}
            candidate.table_cache.insert(candidate.default_lang.clone(),candidate.fallback_table.clone());
            if candidate.current_lang==candidate.default_lang {candidate.current_table=candidate.fallback_table.clone();}
            candidate.portable_restore_digest().map_err(LocaleError::Parse)?;
            candidate.invalidate_live_revision();*self=candidate;return Ok(true);
        }
        let mut changed = false;
        for s in strings {
            if !self.fallback_table.table.strings.contains_key(s.as_str()) {
                self.fallback_table.table.strings.insert(s.clone(), s.clone());
                self.invalidate_live_revision();
                changed = true;
            }
        }

        if changed {
            #[cfg(target_arch = "wasm32")]
            {
                return Ok(true);
            }
            #[cfg(not(target_arch = "wasm32"))]
            {
                let path = self.locale_dir.join(format!("{}.toml", self.default_lang));
                std::fs::write(&path, self.fallback_table.to_toml())?;
                eprintln!(
                    "[locale] {} mis à jour avec {} nouvelles clés",
                    path.display(),
                    strings.len()
                );
            }
        }

        Ok(changed)
    }

    pub fn current_lang(&self) -> &str {
        &self.current_lang
    }
    pub fn default_lang(&self) -> &str {
        &self.default_lang
    }
    pub fn locale_path(&self, lang: &str) -> PathBuf {
        self.locale_dir.join(format!("{lang}.toml"))
    }
}

// ─── COLLECTEUR DE STRINGS ────────────────────────────────────────────────────

/// Parcourt le script et collecte toutes les strings affichables
/// (dialogues + labels de choix) pour alimenter le fichier de locale.
pub fn collect_strings_from_script(script: &rvn_parser::Script) -> Vec<String> {
    let mut strings = Vec::new();
    collect_recursive(script, &mut strings);
    let mut seen = std::collections::HashSet::new();
    strings.retain(|s| seen.insert(s.clone()));
    strings
}

/// Collecte toutes les strings affichables depuis un script aplati (post-flatten_ast).
/// Après flatten_ast, tous les Statement::Dialogue et Statement::Choice sont dans
/// le tableau principal de façon linéaire — pas besoin de récursion.
/// C'est la version à utiliser sur engine.0.script.
pub fn collect_strings_from_flat_script(script: &rvn_parser::Script) -> Vec<String> {
    // Function and screen declarations retain their nested expressions even
    // after narrative flattening. Their translation keys must not disappear.
    collect_strings_from_script(script)
}

fn collect_expression_keys(value: &rvn_parser::Expr, out: &mut Vec<String>) {
    use rvn_parser::Expr;
    fn dictionary_keys(value: &Expr, names: &[&str], out: &mut Vec<String>) {
        if let Expr::Call { name, args } = value {
            if name == "dict" {
                for pair in args.chunks_exact(2) {
                    if matches!(&pair[0],Expr::Str(key) if names.contains(&key.as_str())) {
                        match &pair[1] {
                            Expr::Str(text) => out.push(text.clone()),
                            Expr::ListLit(items) => {
                                for item in items {
                                    if let Expr::Str(text) = item {
                                        out.push(text.clone());
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }
    match value {
        Expr::LibraryCall{call}=>for(_,value)in call.arguments(){collect_expression_keys(value,out);},
        Expr::Call { name, args } => {
            if name == "component" {
                if let Some(properties) = args.get(2) {
                    dictionary_keys(
                        properties,
                        &[
                            "text_key",
                            "placeholder_key",
                            "accessible_label_key",
                            "option_keys",
                        ],
                        out,
                    );
                }
            }
            if name == "video_clip" {
                if let Some(Expr::Call { name, args }) = args.get(1) {
                    if name == "dict" {
                        for pair in args.chunks_exact(2) {
                            if matches!(&pair[0],Expr::Str(key) if key=="subtitles") {
                                if let Expr::ListLit(cues) = &pair[1] {
                                    for cue in cues {
                                        dictionary_keys(cue, &["text"], out);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            for arg in args {
                collect_expression_keys(arg, out);
            }
        }
        Expr::BinOp { left, right, .. } | Expr::And(left, right) | Expr::Or(left, right) => {
            collect_expression_keys(left, out);
            collect_expression_keys(right, out);
        }
        Expr::Index { target, index } => {
            collect_expression_keys(target, out);
            collect_expression_keys(index, out);
        }
        Expr::Neg(value) | Expr::Not(value) => collect_expression_keys(value, out),
        Expr::ListLit(values) => {
            for value in values {
                collect_expression_keys(value, out);
            }
        }
        _ => {}
    }
}

fn collect_recursive(stmts: &[rvn_parser::Statement], out: &mut Vec<String>) {
    use rvn_parser::Statement;
    for stmt in stmts {
        match stmt {
            Statement::FunctionLibrary{scope}=>for member in scope.members(){collect_recursive(std::slice::from_ref(member.declaration()),out);},
            Statement::LibraryUnitCall{call}=>for(_,value)in call.arguments(){collect_expression_keys(value,out);},
            Statement::Dialogue { text, .. } => {
                out.push(text_to_locale_key(text));
            }
            Statement::Choice { options } => {
                for opt in options {
                    out.push(text_to_locale_key(&opt.label));
                    collect_recursive(&opt.body, out);
                }
            }
            Statement::Use { .. } => {}
            Statement::Init { body }
            | Statement::While { body, .. }
            | Statement::ForEach { body, .. }
            | Statement::ForLoopWithBreak {body,..}
            | Statement::Function { body, .. }
            | Statement::UnitFunction { body, .. }
            | Statement::Screen { body, .. }
            | Statement::Handler { body, .. } => collect_recursive(body, out),
            Statement::StandardMacroDispatch{dispatch}=>for branch in &dispatch.branches{collect_recursive(&branch.body,out);},
            Statement::Sequence{branches}=>for branch in branches{collect_recursive(branch,out);},
            Statement::FunctionReturn { value }
            | Statement::SetVar { value, .. }
            | Statement::LocalVar { value, .. } => collect_expression_keys(value, out),
            Statement::UnitCall { arguments, .. } => {
                for argument in arguments { collect_expression_keys(argument, out); }
            }
            Statement::VideoPlay { definition, .. } => collect_expression_keys(definition, out),
            Statement::UiOpen { arguments, .. } => collect_expression_keys(arguments, out),
            Statement::UiSetState { state, .. } => collect_expression_keys(state, out),
            Statement::If {
                then_branch,
                else_branch,
                ..
            } => {
                collect_recursive(then_branch, out);
                collect_recursive(else_branch, out);
            }
            Statement::Imagemap { hotspots, .. } => {
                for hs in hotspots {
                    collect_recursive(&hs.body, out);
                }
            }
            _ => {}
        }
    }
}

/// Convertit un InterpolatedText en clé de locale.
/// Les expressions interpolées sont remplacées par `[varname]` si c'est
/// une simple variable, ou `[expr]` sinon.
fn expr_to_display(expr: &rvn_parser::Expr) -> String {
    use rvn_parser::{BinOpKind, Expr};
    match expr {
        Expr::LibraryCall{call}=>format!("functions.value_v1({:?}, {:?}, {{{}}})",call.reference().library.as_str(),call.reference().graph_id.to_string(),call.arguments().iter().map(|(id,value)|format!("{:?}:{}",id.as_str(),expr_to_display(value))).collect::<Vec<_>>().join(",")),
        Expr::StandardMacroData {site,output,..} => {
            let method=match output {rvn_parser::scheduled_source::standard_macro_contract::DataOutput::Counter=>"flow.macro_counter_v1",rvn_parser::scheduled_source::standard_macro_contract::DataOutput::IsA=>"flow.macro_is_a_v1"};
            format!("{}({}, {})",method,serde_json::to_string(&site.scope_source()).expect("String serialization"),serde_json::to_string(&site.node_source()).expect("String serialization"))
        }
        Expr::Int(n) => n.to_string(),
        Expr::Float(f) => f.to_string(),
        Expr::Bool(b) => b.to_string(),
        Expr::Str(s) => s.clone(),
        Expr::Var(n) => n.clone(),
        Expr::Neg(e) => format!("-{}", expr_to_display(e)),
        Expr::Not(e) => format!("not {}", expr_to_display(e)),
        Expr::And(l, r) => format!("{} and {}", expr_to_display(l), expr_to_display(r)),
        Expr::Or(l, r) => format!("{} or {}", expr_to_display(l), expr_to_display(r)),
        Expr::BinOp { op, left, right } => {
            let op_str = match op {
                BinOpKind::Add => "+",
                BinOpKind::Sub => "-",
                BinOpKind::Mul => "*",
                BinOpKind::Div => "/",
                BinOpKind::Eq => "==",
                BinOpKind::Ne => "!=",
                BinOpKind::Lt => "<",
                BinOpKind::Le => "<=",
                BinOpKind::Gt => ">",
                BinOpKind::Ge => ">=",
            };
            format!(
                "{} {} {}",
                expr_to_display(left),
                op_str,
                expr_to_display(right)
            )
        }
        Expr::Call { name, args } => {
            let args_str: Vec<String> = args.iter().map(expr_to_display).collect();
            format!("{}({})", name, args_str.join(", "))
        }
        Expr::ListLit(items) => {
            let parts: Vec<String> = items.iter().map(expr_to_display).collect();
            format!("[{}]", parts.join(", "))
        }
        Expr::Index { target, index } => {
            format!("{}[{}]", expr_to_display(target), expr_to_display(index))
        }
    }
}

fn text_to_locale_key(text: &rvn_parser::InterpolatedText) -> String {
    use rvn_parser::TextSegment;
    text.0
        .iter()
        .map(|seg| match seg {
            TextSegment::Lit(s) => s.clone(),
            TextSegment::Interp(expr) => format!("[{}]", expr_to_display(expr)),
        })
        .collect()
}

// ─── HELPERS TOML MINIMAL ─────────────────────────────────────────────────────

/// Parse une ligne `"clé" = "valeur"` TOML.
fn parse_kv_line(line: &str) -> Option<(String, String)> {
    // Trouve le = séparateur (en dehors des strings)
    let eq_pos = find_eq(line)?;
    let key_part = line[..eq_pos].trim();
    let val_part = line[eq_pos + 1..].trim();
    let key = parse_toml_string(key_part)?;
    let val = parse_toml_string(val_part)?;
    Some((key, val))
}

fn find_eq(line: &str) -> Option<usize> {
    let mut in_str = false;
    let mut escape = false;
    for (i, c) in line.char_indices() {
        if escape {
            escape = false;
            continue;
        }
        if c == '\\' && in_str {
            escape = true;
            continue;
        }
        if c == '"' {
            in_str = !in_str;
            continue;
        }
        if c == '=' && !in_str {
            return Some(i);
        }
    }
    None
}

fn parse_toml_string(s: &str) -> Option<String> {
    // The existing line-based locale format uses quoted basic strings. Decode
    // once so a literal backslash+n cannot become an actual line break.
    let mut chars = s.trim().strip_prefix('"')?.chars();
    let mut out = String::new();
    while let Some(character) = chars.next() {
        match character {
            '"' => {
                let rest = chars.as_str().trim();
                return (rest.is_empty() || rest.starts_with('#')).then_some(out);
            }
            '\\' => match chars.next()? {
                '"' => out.push('"'), '\\' => out.push('\\'),
                'n' => out.push('\n'), 't' => out.push('\t'), 'r' => out.push('\r'),
                'b' => out.push('\u{0008}'), 'f' => out.push('\u{000c}'),
                escape @ ('u' | 'U') => {
                    let width = if escape == 'u' { 4 } else { 8 };
                    let mut value = 0u32;
                    for _ in 0..width { value = value.checked_mul(16)?.checked_add(chars.next()?.to_digit(16)?)?; }
                    out.push(char::from_u32(value)?);
                }
                _ => return None,
            },
            character if character.is_control() && character != '\t' => return None,
            character => out.push(character),
        }
    }
    None
}

fn toml_quote(s: &str) -> String {
    // JSON quoting is also valid for this TOML basic-string subset, including
    // all control escapes and literal UTF-8 source characters.
    serde_json::to_string(s).expect("serializing a string cannot fail")
}


// ─── TESTS ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_TOML: &str = r#"
# Test locale
[strings]
"Bonjour !" = "Hello!"
"Tu as [score] points." = "You have [score] points."
"Option [n]" = "Option [n]"
"Texte avec \"guillemets\"" = "Text with \"quotes\""
"#;

    #[test]
    fn test_parse_locale_table() {
        let table = LocaleTable::parse("en", SAMPLE_TOML).unwrap();
        assert_eq!(table.lang, "en");
        assert_eq!(table.strings.get("Bonjour !").unwrap(), "Hello!");
        assert_eq!(
            table.strings.get("Tu as [score] points.").unwrap(),
            "You have [score] points."
        );
        assert_eq!(table.strings.len(), 4);
    }

    #[test]
    fn test_parse_escaped_quotes() {
        let table = LocaleTable::parse("en", SAMPLE_TOML).unwrap();
        assert!(table.strings.contains_key("Texte avec \"guillemets\""));
    }

    #[test]
    fn test_translate_found() {
        let dir = std::env::temp_dir().join("rvn_locale_test_found");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("en.toml"), SAMPLE_TOML).unwrap();
        std::fs::write(
            dir.join("fr.toml"),
            "[strings]\n\"Bonjour !\" = \"Bonjour !\"\n",
        )
        .unwrap();

        let mgr = LocaleManager::new(&dir, "fr", "en", vec!["fr".into(), "en".into()]).unwrap();
        assert_eq!(mgr.translate("Bonjour !"), "Hello!");
        assert_eq!(
            mgr.translate("Tu as [score] points."),
            "You have [score] points."
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_translate_fallback_to_original() {
        let dir = std::env::temp_dir().join("rvn_locale_test_fallback");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("en.toml"), "[strings]\n").unwrap();
        std::fs::write(dir.join("fr.toml"), "[strings]\n").unwrap();

        let mgr = LocaleManager::new(&dir, "fr", "en", vec!["fr".into(), "en".into()]).unwrap();
        // String absente → retourne l'original
        assert_eq!(mgr.translate("Texte non traduit"), "Texte non traduit");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_translate_fallback_chain() {
        // fr → en → original
        let dir = std::env::temp_dir().join("rvn_locale_test_chain");
        std::fs::create_dir_all(&dir).unwrap();
        // fr.toml (default) a la clé
        std::fs::write(
            dir.join("fr.toml"),
            "[strings]\n\"Bonjour\" = \"Bonjour\"\n",
        )
        .unwrap();
        // ja.toml (current) n'a pas la clé → fallback vers fr
        std::fs::write(dir.join("ja.toml"), "[strings]\n").unwrap();

        let mgr = LocaleManager::new(&dir, "fr", "ja", vec!["fr".into(), "ja".into()]).unwrap();
        assert_eq!(mgr.translate("Bonjour"), "Bonjour"); // fallback fr
        assert_eq!(mgr.translate("Inconnu"), "Inconnu"); // fallback original
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_to_toml_roundtrip() {
        let table = LocaleTable::parse("en", SAMPLE_TOML).unwrap();
        let serialized = table.to_toml();
        let reparsed = LocaleTable::parse("en", &serialized).unwrap();
        assert_eq!(table.strings, reparsed.strings);
    }

    #[test]
    fn test_update_default_locale_adds_missing() {
        let dir = std::env::temp_dir().join("rvn_locale_test_update");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("fr.toml"),
            "[strings]\n\"Existant\" = \"Existant\"\n",
        )
        .unwrap();
        std::fs::write(dir.join("en.toml"), "[strings]\n").unwrap();

        let mut mgr = LocaleManager::new(&dir, "fr", "en", vec!["fr".into(), "en".into()]).unwrap();
        let strings = vec!["Existant".to_string(), "Nouveau".to_string()];
        let changed = mgr.update_default_locale(&strings).unwrap();
        assert!(changed);

        // Vérifie que "Nouveau" a été ajouté
        let content = std::fs::read_to_string(dir.join("fr.toml")).unwrap();
        assert!(content.contains("Nouveau"));
        // "Existant" est toujours là
        assert!(content.contains("Existant"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_toml_quote_special_chars() {
        assert_eq!(toml_quote("Hello!"), "\"Hello!\"");
        assert_eq!(toml_quote("Say \"hi\""), "\"Say \\\"hi\\\"\"");
        assert_eq!(toml_quote("line\nnew"), "\"line\\nnew\"");
    }

    #[test]
    fn screen_and_video_keys_survive_narrative_flattening() {
        let script=rvn_parser::parse("screen info(){return component(\"root\",\"text\",{\"text_key\":\"ui.info\",\"accessible_label_key\":\"ui.label\"},[])}\nfunction clip(){return video_clip(\"clip.webm\",{\"subtitles\":[{\"start\":0,\"end\":1,\"text\":\"video.cue\"}]})}\nlabel start\nvideo.play(\"intro\",clip())\n\"After\"\n").unwrap();
        let keys = collect_strings_from_flat_script(&script);
        assert_eq!(keys, vec!["ui.info", "ui.label", "video.cue", "After"]);
    }

    #[test]
    fn test_set_language() {
        let dir = std::env::temp_dir().join("rvn_locale_test_setlang");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("fr.toml"),
            "[strings]\n\"Bonjour\" = \"Bonjour\"\n",
        )
        .unwrap();
        std::fs::write(dir.join("en.toml"), "[strings]\n\"Bonjour\" = \"Hello\"\n").unwrap();

        let mut mgr = LocaleManager::new(&dir, "fr", "fr", vec!["fr".into(), "en".into()]).unwrap();
        assert_eq!(mgr.translate("Bonjour"), "Bonjour");

        mgr.set_language("en").unwrap();
        assert_eq!(mgr.translate("Bonjour"), "Hello");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
