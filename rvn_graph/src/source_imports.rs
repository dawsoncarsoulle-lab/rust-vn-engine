//! Validation context for a single authored source, never writable graph scopes.
use rvn_parser::{Script, Statement, NarrativeContext, ParsedProgram, ProgramSource, StatementOrigin};
use serde::{Deserialize, Serialize};
use std::{collections::{BTreeMap, BTreeSet}, fs, path::{Component, Path, PathBuf}};

/// Relative keys make an authoring snapshot movable together with its source.
/// Aliases retain canonical identities for symlinked or multiply-used imports.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ImportSnapshot {
    files: BTreeMap<String, String>,
    aliases: BTreeMap<String, String>,
    wildcards: BTreeMap<String, Vec<String>>,
}

impl ImportSnapshot {
    /// These are captured source bytes, never executable graph-cache payloads.
    /// Recovery still reparses the closure and checks actual files before write.
    pub(crate) fn contains_native_plan(&self) -> Result<bool, String> {
        let mut found=false;
        for source in self.files.values() {
            let script=rvn_parser::parse_for_context_discovery(source).map_err(|error|error.to_string())?;
            let declared=crate::empty_native_init_0984::source_has_declarations(source,NarrativeContext::from_script(&script))?;
            found|=crate::native_source_binding::contains_native(&script)||declared;
        }
        Ok(found)
    }
}

#[derive(Debug, Clone)]
pub(super) struct Context {
    path: PathBuf,
    snapshot: ImportSnapshot,
    narrative: NarrativeContext,
}

impl Context {
    pub(super) fn live(path: &Path, source: &str) -> Result<(Self, Script), String> {
        let (context, program) = Self::live_program(path, source)?;
        Ok((context, program.script))
    }

    fn live_program(path: &Path, source: &str) -> Result<(Self, ParsedProgram), String> {
        // Bound the first Graph loader, before its canonicalization/source copy.
        if path.as_os_str().len() > LIVE_PATH || source.len() > LIVE_BYTES {
            return Err("Graph live import source/path budget".into());
        }
        let path = match fs::canonicalize(path) {
            Ok(path) => path,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let parent = path.parent().ok_or("Missing source origin parent")?.canonicalize().map_err(|error| error.to_string())?;
                parent.join(path.file_name().ok_or("Missing source origin filename")?)
            }
            Err(error) => return Err(format!("{}: {error}", path.display())),
        };
        if path.as_os_str().len() > LIVE_PATH { return Err("Graph live import path budget".into()); }
        let mut loader = Loader::new(&path, None);
        let discovered = loader.load(&path, Some(source))?;
        let mut program = loader.finish(&discovered)?;
        // Only this live frontier can mint filesystem-resolution evidence.
        // Cached/recovery finish deliberately leaves resolution == None.
        rvn_parser::capture_file_program_resolution(&path, source, &mut program)?;
        Ok((Self { path, snapshot: loader.snapshot, narrative: program.context }, program))
    }

    pub(super) fn cached(path: &Path, source: &str, snapshot: ImportSnapshot) -> Result<(Self, Script), String> {
        let path = normalize(path);
        let mut loader = Loader::new(&path, Some(&snapshot));
        let discovered = loader.load(&path, Some(source))?;
        let program = loader.finish(&discovered)?;
        if loader.snapshot != snapshot {
            return Err("Import context does not match the authored source snapshot".into());
        }
        Ok((Self { path, snapshot, narrative: program.context }, program.script))
    }

    pub(super) fn empty(path: &Path) -> Self {
        Self { path: normalize(path), snapshot: ImportSnapshot::default(), narrative: NarrativeContext::Legacy }
    }

    pub(super) fn snapshot(&self) -> Option<ImportSnapshot> {
        (!self.snapshot.files.is_empty() || !self.snapshot.wildcards.is_empty())
            .then(|| self.snapshot.clone())
    }

    pub(super) fn resolve(&self, source: &str) -> Result<(Self, Script), String> {
        Self::live(&self.path, source)
    }

    pub(super) fn check(&self, source: &str) -> Result<Script, String> {
        Ok(self.check_program(source)?.script)
    }

    pub(super) fn narrative_context(&self) -> NarrativeContext { self.narrative }

    pub(super) fn check_program(&self, source: &str) -> Result<ParsedProgram, String> {
        let (current, program) = Self::live_program(&self.path, source)?;
        if current.snapshot() != self.snapshot() {
            return Err("An imported RVN source changed externally. Reopen or resolve the conflict; no files were overwritten.".into());
        }
        Ok(program)
    }

    pub(super) fn source_files(&self, source: &str) -> Result<Vec<(PathBuf, String)>, String> {
        self.check(source)?;
        let base = self.path.parent().ok_or("Missing source origin parent")?;
        let mut files = vec![(self.path.clone(), source.to_owned())];
        files.extend(self.snapshot.files.iter().map(|(path, source)| (normalize(&base.join(path)), source.clone())));
        Ok(files)
    }

    // Use only the checked/cached dependency bytes. Ownership queries must
    // work when an import changed or disappeared and recovery uses its cache.
    pub(super) fn imported_script(&self) -> Result<Script, String> {
        let mut script = Vec::new();
        for source in self.snapshot.files.values() {
            script.extend(rvn_parser::parse_with_context(source, self.narrative).map_err(|error| error.to_string())?);
        }
        Ok(script)
    }
}

// Same logical limits as Parser CapturedUseResolution. These guard the earlier
// authoring loader too; cached recovery keeps its historical no-I/O contract.
const LIVE_BYTES: usize = 16 * 1024 * 1024;
const LIVE_ITEMS: usize = 100_000;
const LIVE_DEPTH: usize = 256;
const LIVE_PATH: usize = 4096;
#[derive(Default)]
struct LiveBudget { bytes: usize, occurrences: usize, targets: usize, roots: usize }
impl LiveBudget {
    fn bytes(&mut self, amount: usize) -> Result<(), String> {
        self.bytes = self.bytes.checked_add(amount).filter(|n| *n <= LIVE_BYTES)
            .ok_or("Graph live import source/metadata byte budget")?;
        Ok(())
    }
    fn count(counter: &mut usize, amount: usize, error: &str) -> Result<(), String> {
        *counter = counter.checked_add(amount).filter(|n| *n <= LIVE_ITEMS).ok_or(error)?;
        Ok(())
    }
}

struct Loader<'a> {
    base: PathBuf,
    live: Option<LiveBudget>,
    cached: Option<&'a ImportSnapshot>,
    snapshot: ImportSnapshot,
    stack: Vec<String>,
    loaded: BTreeSet<String>,
    files: BTreeMap<String, String>,
    file_order: Vec<String>,
    statement_order: Vec<(String, usize)>,
}

impl<'a> Loader<'a> {
    fn new(path: &Path, cached: Option<&'a ImportSnapshot>) -> Self {
        Self { base: path.parent().unwrap_or_else(|| Path::new(".")).to_owned(), live: cached.is_none().then(LiveBudget::default), cached,
            snapshot: ImportSnapshot::default(), stack: Vec::new(), loaded: BTreeSet::new(),
            files: BTreeMap::new(), file_order: Vec::new(), statement_order: Vec::new() }
    }

    fn load(&mut self, path: &Path, source: Option<&str>) -> Result<Script, String> {
        if self.live.is_some() && path.as_os_str().len() > LIVE_PATH {
            return Err("Graph live import path budget".into());
        }
        let requested = relative(&self.base, path);
        let mut identity_units = path.as_os_str().len();
        let canonical = if let Some(cached) = self.cached {
            if source.is_some() { requested.clone() }
            else { cached.aliases.get(&requested).cloned().ok_or_else(|| format!("Missing cached import: {requested}"))? }
        } else if source.is_some() {
            requested.clone()
        } else {
            let canonical = fs::canonicalize(path).map_err(|error| format!("RVN import {}: {error}", path.display()))?;
            identity_units = canonical.as_os_str().len();
            if identity_units > LIVE_PATH { return Err("Graph live import path budget".into()); }
            relative(&self.base, &canonical)
        };
        if let Some(budget) = &mut self.live {
            if requested.len() > LIVE_PATH || canonical.len() > LIVE_PATH {
                return Err("Graph live import path budget".into());
            }
            // Include full source identities too, before finish clones them
            // into ProgramSource/origins (relative snapshot keys can be tiny).
            budget.bytes(identity_units)?;
            budget.bytes(requested.len())?;
            budget.bytes(canonical.len())?;
        }
        self.snapshot.aliases.insert(requested, canonical.clone());
        if let Some(index) = self.stack.iter().position(|file| file == &canonical) {
            return Err(format!("RVN use cycle: {} -> {canonical}", self.stack[index..].join(" -> ")));
        }
        if self.loaded.contains(&canonical) { return Ok(Vec::new()); }
        if self.live.is_some() && (self.stack.len() >= LIVE_DEPTH || self.loaded.len() >= LIVE_ITEMS) {
            return Err("Graph live import file/depth budget".into());
        }
        self.loaded.insert(canonical.clone());
        self.stack.push(canonical.clone());
        let contents = if let Some(source) = source {
            if let Some(budget) = &mut self.live { budget.bytes(source.len())?; }
            source.to_owned()
        } else if let Some(cached) = self.cached {
            cached.files.get(&canonical).cloned().ok_or_else(|| format!("Missing cached RVN source: {canonical}"))?
        } else {
            use std::io::Read;
            let budget = self.live.as_mut().ok_or("Graph live import budget absent")?;
            let remaining = LIVE_BYTES.saturating_sub(budget.bytes);
            let file = fs::File::open(self.base.join(&canonical)).map_err(|error| format!("RVN import {canonical}: {error}"))?;
            if file.metadata().map_err(|error| error.to_string())?.len() > remaining as u64 {
                return Err("Graph live import source/metadata byte budget".into());
            }
            let mut contents = String::new();
            file.take(remaining as u64 + 1).read_to_string(&mut contents).map_err(|error| format!("RVN import {canonical}: {error}"))?;
            budget.bytes(contents.len())?;
            contents
        };
        if source.is_none() { self.snapshot.files.insert(canonical.clone(), contents.clone()); }
        let script = rvn_parser::parse_for_context_discovery(&contents).map_err(|error| format!("RVN source {canonical}: {error}"))?;
        if self.live.is_some() && script.len() > LIVE_ITEMS { return Err("Graph live import physical root budget".into()); }
        self.file_order.push(canonical.clone());
        self.files.insert(canonical.clone(), contents);
        let base = self.base.join(&canonical).parent().ok_or("Missing RVN source parent")?.to_owned();
        let mut resolved = Vec::new();
        for (index, statement) in script.into_iter().enumerate() {
            if let Statement::Use { paths } = statement {
                for raw in paths {
                    if let Some(budget) = &mut self.live {
                        if raw.len() > LIVE_PATH { return Err("Graph live import locator budget".into()); }
                        LiveBudget::count(&mut budget.occurrences, 1, "Graph live import occurrence budget")?;
                        budget.bytes(raw.len())?;
                    }
                    for target in self.targets(&base, &raw)? {
                        resolved.extend(self.load(&target, None)?);
                    }
                }
            } else {
                if let Some(budget) = &mut self.live {
                    LiveBudget::count(&mut budget.roots, 1, "Graph live import resolved root budget")?;
                }
                self.statement_order.push((canonical.clone(), index));
                resolved.push(statement);
            }
        }
        self.stack.pop();
        Ok(resolved)
    }

    // Discovery reads each file once. Validation and origins use exactly those
    // frozen bytes, including cached recovery, rather than reopening imports.
    fn finish(&self, discovered: &Script) -> Result<ParsedProgram, String> {
        let context = NarrativeContext::from_script(discovered);
        let mut parsed = BTreeMap::new();
        for (file, source) in &self.files {
            parsed.insert(file.clone(), rvn_parser::parse_with_origins(source, context)
                .map_err(|error| format!("RVN source {file}: {error}"))?);
        }
        let mut script = Vec::new();
        let mut origins = Vec::new();
        for (file, index) in &self.statement_order {
            let source = &parsed[file];
            script.push(source.script[*index].clone());
            origins.push(StatementOrigin { source: Some(normalize(&self.base.join(file))), span: source.spans[*index].clone() });
        }
        let sources = self.file_order.iter().map(|file| ProgramSource {
            path: Some(normalize(&self.base.join(file))), source: self.files[file].clone(),
        }).collect();
        let program=ParsedProgram { script, context, origins, sources, resolution: None };
        // Included declaration comments are validated on the same frozen
        // closure before editor open/check/publish; no cache grants identity.
        crate::empty_native_init_0984::program_has_declarations(&program)?;
        Ok(program)
    }

    fn charge_targets(&mut self, paths: &[PathBuf]) -> Result<(), String> {
        if let Some(budget) = &mut self.live {
            LiveBudget::count(&mut budget.targets, paths.len(), "Graph live import target budget")?;
            for path in paths {
                if path.as_os_str().len() > LIVE_PATH { return Err("Graph live import path budget".into()); }
                budget.bytes(path.as_os_str().len())?;
            }
        }
        Ok(())
    }

    fn targets(&mut self, base: &Path, raw: &str) -> Result<Vec<PathBuf>, String> {
        let Some(directory) = raw.strip_suffix("/*.rvn").or_else(|| raw.strip_suffix("/*")) else {
            let path = base.join(raw);
            self.charge_targets(std::slice::from_ref(&path))?;
            return Ok(vec![path]);
        };
        let directory = base.join(directory);
        let key = relative(&self.base, &directory);
        if let Some(paths) = self.snapshot.wildcards.get(&key) {
            // Check before cloning the previously observed directory membership.
            if let Some(budget) = &mut self.live {
                LiveBudget::count(&mut budget.targets, paths.len(), "Graph live import target budget")?;
                for path in paths {
                    if path.len() > LIVE_PATH { return Err("Graph live import path budget".into()); }
                    budget.bytes(path.len())?;
                }
            }
            return Ok(paths.iter().map(|path| self.base.join(path)).collect());
        }
        if let Some(budget) = &mut self.live {
            if directory.as_os_str().len() > LIVE_PATH { return Err("Graph live import path budget".into()); }
            budget.bytes(directory.as_os_str().len())?;
        }
        let paths = if let Some(cached) = self.cached {
            cached.wildcards.get(&key).cloned().ok_or_else(|| format!("Missing cached use wildcard: {key}"))?
                .into_iter().map(|path| self.base.join(path)).collect::<Vec<_>>()
        } else {
            let mut paths = Vec::new();
            for entry in fs::read_dir(&directory).map_err(|error| format!("RVN use wildcard {}: {error}", directory.display()))? {
                let path = entry.map_err(|error| error.to_string())?.path();
                if path.extension().and_then(|extension| extension.to_str()) == Some("rvn") {
                    self.charge_targets(std::slice::from_ref(&path))?;
                    paths.push(path);
                }
            }
            paths.sort();
            paths
        };
        if paths.is_empty() { return Err(format!("No .rvn file found for use wildcard: {key}")); }
        self.snapshot.wildcards.insert(key, paths.iter().map(|path| relative(&self.base, path)).collect());
        Ok(paths)
    }
}

fn normalize(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {},
            Component::ParentDir => { result.pop(); },
            other => result.push(other.as_os_str()),
        }
    }
    result
}

fn relative(base: &Path, path: &Path) -> String {
    let original = path;
    let base: Vec<_> = base.components().collect();
    let path: Vec<_> = path.components().collect();
    let common = base.iter().zip(&path).take_while(|(left, right)| left == right).count();
    let mut result = PathBuf::new();
    if common == 0 { return original.to_string_lossy().into_owned(); }
    else {
        for _ in common..base.len() { result.push(".."); }
        for component in &path[common..] { result.push(component.as_os_str()); }
    }
    result.to_string_lossy().replace('\\', "/")
}


#[cfg(test)]
mod captured_resolution_tests1021 {
    use super::*;
    #[test]
    fn cached_import_snapshot_never_mints_live_use_resolution() {
        let nonce=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root=std::env::temp_dir().join(format!("rvn-cached-use-proof-{}-{nonce}",std::process::id()));
        fs::create_dir(&root).unwrap();fs::create_dir(root.join("parts")).unwrap();
        let path=root.join("main.rvn");let source="use \"parts/*.rvn\"\nscene label start\nreturn\n";
        fs::write(&path,source).unwrap();fs::write(root.join("parts/a.rvn"),"init {set order=1}\n").unwrap();
        let(context,live)=Context::live_program(&path,source).unwrap();assert!(live.resolution.is_some());
        let snapshot=context.snapshot().unwrap();
        // Serialized source claims remain recoverable without files, but cannot
        // become the opaque filesystem capture retained by the live loader.
        let json=serde_json::to_string(&snapshot).unwrap();assert!(!json.contains("resolution"));
        let snapshot:ImportSnapshot=serde_json::from_str(&json).unwrap();
        fs::remove_file(root.join("parts/a.rvn")).unwrap();fs::remove_file(&path).unwrap();
        let mut loader=Loader::new(&context.path,Some(&snapshot));
        let discovered=loader.load(&context.path,Some(source)).unwrap();
        let cached=loader.finish(&discovered).unwrap();assert!(cached.resolution.is_none());assert_eq!(cached,live);
        assert!(rvn_parser::capture_file_program_resolution(&context.path,source,&mut cached.clone()).is_err());
        live.resolution.as_ref().unwrap().verify_for(&live).unwrap();
        fs::remove_dir(root.join("parts")).unwrap();fs::remove_dir(root).unwrap();
    }
    #[test]
    fn live_import_bounds_precede_source_copies_reads_and_recursive_descent() {
        let nonce=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root=std::env::temp_dir().join(format!("rvn-live-import-bounds-{}-{nonce}",std::process::id()));
        fs::create_dir(&root).unwrap(); let root=fs::canonicalize(root).unwrap();
        let path=root.join("main.rvn");
        let source="scene label start\nreturn\n";fs::write(&path,source).unwrap();
        let (context, original)=Context::live_program(&path,source).unwrap();
        let oversized=" ".repeat(LIVE_BYTES+1);
        // No parent exists: this must fail before even trying canonicalization.
        assert!(Context::live_program(&root.join("absent/main.rvn"),&oversized).unwrap_err().contains("Graph live import source/path budget"));
        let big=root.join("big.rvn");let file=fs::File::create(&big).unwrap();file.set_len((LIVE_BYTES+1) as u64).unwrap();drop(file);
        let import="use \"big.rvn\"\nscene label start\nreturn\n";
        fs::write(&path,import).unwrap();
        assert!(Context::live_program(&path,import).unwrap_err().contains("Graph live import source/metadata byte budget"));
        assert_eq!(fs::metadata(&big).unwrap().len(),(LIVE_BYTES+1) as u64);
        fs::remove_file(&big).unwrap();
        // The 257th actual file is never parsed/read by the first Graph loader.
        for depth in 0..=LIVE_DEPTH {
            let text=if depth==LIVE_DEPTH {"not valid source @@@".to_owned()}
                else {format!("use \"depth{}.rvn\"\n",depth+1)};
            fs::write(root.join(format!("depth{depth}.rvn")),text).unwrap();
        }
        let first=fs::read_to_string(root.join("depth0.rvn")).unwrap();
        assert!(Context::live_program(&root.join("depth0.rvn"),&first).unwrap_err().contains("Graph live import file/depth budget"));
        // Item/path limits fail before target copies or recursive entry.
        let mut loader=Loader::new(&path,None);
        loader.live.as_mut().unwrap().targets=LIVE_ITEMS;
        assert!(loader.targets(&root,"depth0.rvn").unwrap_err().contains("Graph live import target budget"));
        fs::create_dir(root.join("parts")).unwrap();fs::write(root.join("parts/a.rvn"),"return\n").unwrap();
        let mut loader=Loader::new(&path,None);
        loader.live.as_mut().unwrap().targets=LIVE_ITEMS;
        assert!(loader.targets(&root,"parts/*.rvn").unwrap_err().contains("Graph live import target budget"));
        let mut loader=Loader::new(&path,None);
        loader.live.as_mut().unwrap().occurrences=LIVE_ITEMS;
        assert!(loader.load(&path,Some(import)).unwrap_err().contains("Graph live import occurrence budget"));
        let mut loader=Loader::new(&path,None);
        loader.live.as_mut().unwrap().roots=LIVE_ITEMS;
        assert!(loader.load(&path,Some(source)).unwrap_err().contains("Graph live import resolved root budget"));
        let mut loader=Loader::new(&path,None);
        assert!(loader.targets(&root,&"x".repeat(LIVE_PATH+1)).unwrap_err().contains("Graph live import path budget"));
        fs::write(&path,source).unwrap();
        assert_eq!(context.check_program(source).unwrap(),original,"failed reads do not publish a new import snapshot");
        assert_eq!(fs::read_to_string(&path).unwrap(),source);
        fs::remove_dir_all(root).unwrap();
    }

}
