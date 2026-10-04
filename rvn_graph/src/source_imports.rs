//! Validation context for a single authored source, never writable graph scopes.
use rvn_parser::{Script, Statement};
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

#[derive(Debug, Clone)]
pub(super) struct Context {
    path: PathBuf,
    snapshot: ImportSnapshot,
}

impl Context {
    pub(super) fn live(path: &Path, source: &str) -> Result<(Self, Script), String> {
        let path = match fs::canonicalize(path) {
            Ok(path) => path,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let parent = path.parent().ok_or("Missing source origin parent")?.canonicalize().map_err(|error| error.to_string())?;
                parent.join(path.file_name().ok_or("Missing source origin filename")?)
            }
            Err(error) => return Err(format!("{}: {error}", path.display())),
        };
        let mut loader = Loader::new(&path, None);
        let script = loader.load(&path, Some(source))?;
        Ok((Self { path, snapshot: loader.snapshot }, script))
    }

    pub(super) fn cached(path: &Path, source: &str, snapshot: ImportSnapshot) -> Result<(Self, Script), String> {
        let path = normalize(path);
        let mut loader = Loader::new(&path, Some(&snapshot));
        let script = loader.load(&path, Some(source))?;
        if loader.snapshot != snapshot {
            return Err("Import context does not match the authored source snapshot".into());
        }
        Ok((Self { path, snapshot }, script))
    }

    pub(super) fn empty(path: &Path) -> Self {
        Self { path: normalize(path), snapshot: ImportSnapshot::default() }
    }

    pub(super) fn snapshot(&self) -> Option<ImportSnapshot> {
        (!self.snapshot.files.is_empty() || !self.snapshot.wildcards.is_empty())
            .then(|| self.snapshot.clone())
    }

    pub(super) fn resolve(&self, source: &str) -> Result<(Self, Script), String> {
        Self::live(&self.path, source)
    }

    pub(super) fn check(&self, source: &str) -> Result<Script, String> {
        let (current, script) = self.resolve(source)?;
        if current.snapshot() != self.snapshot() {
            return Err("An imported RVN source changed externally. Reopen or resolve the conflict; no files were overwritten.".into());
        }
        Ok(script)
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
            script.extend(rvn_parser::parse(source).map_err(|error| error.to_string())?);
        }
        Ok(script)
    }
}

struct Loader<'a> {
    base: PathBuf,
    cached: Option<&'a ImportSnapshot>,
    snapshot: ImportSnapshot,
    stack: Vec<String>,
    loaded: BTreeSet<String>,
}

impl<'a> Loader<'a> {
    fn new(path: &Path, cached: Option<&'a ImportSnapshot>) -> Self {
        Self { base: path.parent().unwrap_or_else(|| Path::new(".")).to_owned(), cached,
            snapshot: ImportSnapshot::default(), stack: Vec::new(), loaded: BTreeSet::new() }
    }

    fn load(&mut self, path: &Path, source: Option<&str>) -> Result<Script, String> {
        let requested = relative(&self.base, path);
        let canonical = if let Some(cached) = self.cached {
            if source.is_some() { requested.clone() }
            else { cached.aliases.get(&requested).cloned().ok_or_else(|| format!("Missing cached import: {requested}"))? }
        } else if source.is_some() {
            requested.clone()
        } else {
            let canonical = fs::canonicalize(path).map_err(|error| format!("RVN import {}: {error}", path.display()))?;
            relative(&self.base, &canonical)
        };
        self.snapshot.aliases.insert(requested, canonical.clone());
        if let Some(index) = self.stack.iter().position(|file| file == &canonical) {
            return Err(format!("RVN use cycle: {} -> {canonical}", self.stack[index..].join(" -> ")));
        }
        if !self.loaded.insert(canonical.clone()) { return Ok(Vec::new()); }
        self.stack.push(canonical.clone());
        let contents = if let Some(source) = source { source.to_owned() }
            else if let Some(cached) = self.cached { cached.files.get(&canonical).cloned().ok_or_else(|| format!("Missing cached RVN source: {canonical}"))? }
            else { fs::read_to_string(self.base.join(&canonical)).map_err(|error| format!("RVN import {canonical}: {error}"))? };
        if source.is_none() { self.snapshot.files.insert(canonical.clone(), contents.clone()); }
        let script = rvn_parser::parse(&contents).map_err(|error| format!("RVN source {canonical}: {error}"))?;
        let base = self.base.join(&canonical).parent().ok_or("Missing RVN source parent")?.to_owned();
        let mut resolved = Vec::new();
        for statement in script {
            if let Statement::Use { paths } = statement {
                for raw in paths {
                    for target in self.targets(&base, &raw)? {
                        resolved.extend(self.load(&target, None)?);
                    }
                }
            } else { resolved.push(statement); }
        }
        self.stack.pop();
        Ok(resolved)
    }

    fn targets(&mut self, base: &Path, raw: &str) -> Result<Vec<PathBuf>, String> {
        let Some(directory) = raw.strip_suffix("/*.rvn").or_else(|| raw.strip_suffix("/*")) else {
            return Ok(vec![base.join(raw)]);
        };
        let directory = base.join(directory);
        let key = relative(&self.base, &directory);
        let paths = if let Some(cached) = self.cached {
            cached.wildcards.get(&key).cloned().ok_or_else(|| format!("Missing cached use wildcard: {key}"))?
                .into_iter().map(|path| self.base.join(path)).collect::<Vec<_>>()
        } else {
            let mut paths = Vec::new();
            for entry in fs::read_dir(&directory).map_err(|error| format!("RVN use wildcard {}: {error}", directory.display()))? {
                let path = entry.map_err(|error| error.to_string())?.path();
                if path.extension().and_then(|extension| extension.to_str()) == Some("rvn") { paths.push(path); }
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
