//! Resolved narrative parsing and exact source provenance. These sidecars do
//! not change the authored AST or fabricate graph/runtime identities.
use crate::{parse_with_origins, Script, Statement};
use std::{
    collections::{HashMap, HashSet},
    fs,
    ops::Range,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NarrativeContext {
    #[default]
    Legacy,
    ExplicitScenes,
}

impl NarrativeContext {
    /// Derive the context from a complete, resolved script, never from label
    /// names or background changes. Structural/runtime validation is separate.
    pub fn from_script(script: &Script) -> Self {
        if script
            .iter()
            .any(|statement| matches!(statement, Statement::SceneLabel { .. }))
        {
            Self::ExplicitScenes
        } else {
            Self::Legacy
        }
    }
}

/// Offsets refer to original UTF-8 bytes. Children follow their AST lists in
/// lexical order: then/else, choice options, imagemap hotspots, or one body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatementSpan {
    pub range: Range<usize>,
    pub children: Vec<StatementSpan>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedSource {
    pub script: Script,
    pub spans: Vec<StatementSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatementOrigin {
    pub source: Option<PathBuf>,
    pub span: StatementSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramSource {
    pub path: Option<PathBuf>,
    pub source: String,
}

#[derive(Clone)]
pub struct ParsedProgram {
    pub script: Script,
    pub context: NarrativeContext,
    /// One origin per resolved root statement, in precisely the same order.
    pub origins: Vec<StatementOrigin>,
    /// Frozen source bytes, in first-inclusion order, including their Use lines.
    pub sources: Vec<ProgramSource>,
    /// Live-loader provenance only; never restored from a client DTO or sidecar.
    pub resolution: Option<CapturedUseResolution>,
}

// Resolution is additional admission evidence, not a change to the historical
// programme identity. SourceMap hashes this exact four-field Debug rendering.
impl std::fmt::Debug for ParsedProgram {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ParsedProgram").field("script", &self.script)
            .field("context", &self.context).field("origins", &self.origins)
            .field("sources", &self.sources).finish()
    }
}
impl PartialEq for ParsedProgram {
    fn eq(&self, other: &Self) -> bool {
        self.script == other.script && self.context == other.context
            && self.origins == other.origins && self.sources == other.sources
    }
}

const RESOLUTION_BYTES: usize = 16 * 1024 * 1024;
const RESOLUTION_ITEMS: usize = 100_000;
const RESOLUTION_DEPTH: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
struct UseTarget { requested: PathBuf, canonical: PathBuf }
#[derive(Debug, Clone, PartialEq, Eq)]
struct UseOccurrence {
    source: PathBuf, root: usize, path_index: usize, raw: String,
    directory: Option<PathBuf>,
    targets: Vec<UseTarget>,
}
/// Opaque observation of an actual filesystem load. No Deserialize or public
/// constructor: serialized aliases/wildcards are source claims, not this proof.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedUseResolution {
    sources: Vec<ProgramSource>,
    occurrences: Vec<UseOccurrence>,
}
impl CapturedUseResolution {
    /// Replays only captured UTF-8 bytes and captured resolution edges.
    pub fn verify_for(&self, program: &ParsedProgram) -> Result<(), String> {
        if program.script.len() > RESOLUTION_ITEMS || program.origins.len() > RESOLUTION_ITEMS {
            return Err("Captured Use root budget".into());
        }
        if self.sources != program.sources || program.context != NarrativeContext::from_script(&program.script)
            || program.script.len() != program.origins.len() {
            return Err("Captured Use source/context binding differs".into());
        }
        check_resolution_shape(&self.sources, &self.occurrences)?;
        let mut parsed = HashMap::new();
        for source in &self.sources {
            let path = source.path.as_ref().ok_or("Captured Use source has no canonical identity")?;
            let value = parse_with_origins(&source.source, program.context).map_err(|e| e.to_string())?;
            if parsed.insert(path.clone(), value).is_some() { return Err("Duplicate captured Use source".into()); }
        }
        let mut edges = HashMap::new();
        for edge in &self.occurrences {
            let wildcard = edge.raw.ends_with("/*") || edge.raw.ends_with("/*.rvn");
            if wildcard != edge.directory.is_some() || edge.directory.as_ref().is_some_and(|p| !p.is_absolute()) {
                return Err("Captured Use directory shape differs".into());
            }
            if wildcard {
                if edge.targets.iter().any(|t| t.requested.extension().and_then(|s|s.to_str()) != Some("rvn"))
                    || edge.targets.windows(2).any(|t| t[0].requested >= t[1].requested) {
                    return Err("Captured Use wildcard target order/type differs".into());
                }
            } else if edge.targets.len() != 1 || edge.targets[0].requested != edge.source.parent().ok_or("Captured Use parent absent")?.join(&edge.raw) {
                return Err("Captured Use explicit locator differs".into());
            }
            if edges.insert((edge.source.clone(), edge.root, edge.path_index), edge).is_some() {
                return Err("Duplicate captured Use occurrence".into());
            }
        }
        enum Step { Enter(PathBuf), Root(PathBuf, usize), Leave(PathBuf) }
        let first = self.sources.first().and_then(|s|s.path.clone()).ok_or("Captured Use entry absent")?;
        let mut pending = vec![Step::Enter(first)];
        let mut active = HashSet::new(); let mut loaded = HashSet::new();
        let mut visited = Vec::new(); let mut used_edges = HashSet::new(); let mut emitted = 0;
        while let Some(step) = pending.pop() {
            if pending.len() > RESOLUTION_ITEMS { return Err("Captured Use replay step budget".into()); }
            match step {
                Step::Leave(path) => { active.remove(&path); }
                Step::Root(path, root) => {
                    let source = &parsed[&path];
                    let origin = program.origins.get(emitted).ok_or("Captured Use root count differs")?;
                    if program.script.get(emitted) != Some(&source.script[root])
                        || origin.source.as_ref() != Some(&path) || origin.span != source.spans[root] {
                        return Err("Captured Use ordered AST/origin differs".into());
                    }
                    emitted += 1;
                }
                Step::Enter(path) => {
                    // A stack hit is a cycle even if that file was already loaded.
                    if active.contains(&path) { return Err("Captured Use cycle".into()); }
                    if !loaded.insert(path.clone()) { continue; }
                    if active.len() >= RESOLUTION_DEPTH { return Err("Captured Use depth budget".into()); }
                    let source = parsed.get(&path).ok_or("Captured Use target bytes absent")?;
                    active.insert(path.clone()); visited.push(path.clone());
                    pending.push(Step::Leave(path.clone()));
                    for (root, statement) in source.script.iter().enumerate().rev() {
                        if let Statement::Use { paths } = statement {
                            for (path_index, raw) in paths.iter().enumerate().rev() {
                                let key = (path.clone(), root, path_index);
                                let edge = edges.get(&key).ok_or("Captured Use occurrence absent")?;
                                if &edge.raw != raw || edge.targets.is_empty() { return Err("Captured Use locator differs".into()); }
                                used_edges.insert(key);
                                for target in edge.targets.iter().rev() { pending.push(Step::Enter(target.canonical.clone())); }
                            }
                        } else { pending.push(Step::Root(path.clone(), root)); }
                    }
                }
            }
        }
        if emitted != program.script.len() || used_edges.len() != self.occurrences.len()
            || visited.iter().ne(self.sources.iter().filter_map(|s|s.path.as_ref())) {
            return Err("Captured Use complete source/root/edge order differs".into());
        }
        ensure_resolved(&program.script)
    }
}
fn check_resolution_shape(sources: &[ProgramSource], edges: &[UseOccurrence]) -> Result<(), String> {
    if sources.len() > RESOLUTION_ITEMS || edges.len() > RESOLUTION_ITEMS { return Err("Captured Use item budget".into()); }
    let mut bytes = 0usize; let mut targets = 0usize;
    let mut add = |n: usize| -> Result<(), String> {
        bytes = bytes.checked_add(n).filter(|n|*n <= RESOLUTION_BYTES).ok_or("Captured Use byte budget")?; Ok(())
    };
    for source in sources {
        add(source.source.len())?;
        let path = source.path.as_ref().ok_or("Captured Use source identity absent")?;
        if !path.is_absolute() { return Err("Captured Use identity is not absolute".into()); }
        add(path.as_os_str().len())?;
    }
    for edge in edges {
        add(edge.source.as_os_str().len())?; add(edge.raw.len())?;
        if let Some(directory) = &edge.directory { add(directory.as_os_str().len())?; }
        targets = targets.checked_add(edge.targets.len()).filter(|n|*n <= RESOLUTION_ITEMS).ok_or("Captured Use target budget")?;
        for target in &edge.targets { add(target.requested.as_os_str().len())?; add(target.canonical.as_os_str().len())?; }
    }
    Ok(())
}

/// Capture through the same real loader, then attach only after equality of
/// every historical field. A cached source-sidecar cannot call this without FS.
pub fn capture_file_program_resolution(path: impl AsRef<Path>, source: &str, program: &mut ParsedProgram) -> Result<(), String> {
    let mut actual = parse_file_program_with_uses_from_source(path, source)?;
    if actual != *program { return Err("Live captured Use programme differs from frozen authoring context".into()); }
    program.resolution = actual.resolution.take(); Ok(())
}

/// Explicit live frontier. Never call this from a frozen replay or save DTO.
pub fn recheck_file_program_resolution(program: &ParsedProgram) -> Result<(), String> {
    let proof = program.resolution.as_ref().ok_or("Captured Use resolution is absent")?;
    proof.verify_for(program)?;
    let entry = program.sources.first().ok_or("Captured Use entry absent")?;
    let actual = parse_file_program_with_uses_from_source(entry.path.as_ref().ok_or("Captured Use entry path absent")?, &entry.source)?;
    if actual != *program || actual.resolution.as_ref() != Some(proof) {
        return Err("Captured Use wildcard/alias/source changed during freezing".into());
    }
    Ok(())
}

/// Parse one complete in-memory program. Imports require a resolver with real
/// source origins; this entry point refuses unresolved Use rather than guessing
/// its global context. Discovery and validation read the same immutable bytes.
pub fn parse_program(source: &str) -> Result<ParsedProgram, String> {
    let discovered = parse_with_origins(source, NarrativeContext::ExplicitScenes)
        .map_err(|error| error.to_string())?;
    ensure_resolved(&discovered.script)?;
    let context = NarrativeContext::from_script(&discovered.script);
    let parsed = parse_with_origins(source, context).map_err(|error| error.to_string())?;
    let origins = parsed
        .spans
        .into_iter()
        .map(|span| StatementOrigin { source: None, span })
        .collect();
    Ok(ParsedProgram {
        script: parsed.script,
        context,
        origins,
        resolution: None,
        sources: vec![ProgramSource {
            path: None,
            source: source.to_owned(),
        }],
    })
}

/// Resolve recursive Use directives, derive the global narrative context, then
/// validate every file under that context. Each source and wildcard target list
/// is read exactly once. Import order/cycle/dedup rules match the legacy loader;
/// the legacy API itself remains strict and unchanged.
pub fn parse_file_program_with_uses(path: impl AsRef<Path>) -> Result<ParsedProgram, String> {
    let mut loader = DiscoveryLoader::default();
    loader.load(path.as_ref())?;
    loader.finish()
}

/// Same discovery rules with an actual editor-owned root buffer. Imports are
/// still read/canonicalized by the real loader; the override is not a resolver.
pub fn parse_file_program_with_uses_from_source(path: impl AsRef<Path>, source: &str) -> Result<ParsedProgram, String> {
    let mut loader = DiscoveryLoader::default();
    loader.load_with_source(path.as_ref(), Some(source))?;
    loader.finish()
}

fn validate_frozen_program(loader: DiscoveryLoader) -> Result<ParsedProgram, String> {
    let discovery: Script = loader
        .order
        .iter()
        .map(|&(file, root)| loader.files[file].discovered.script[root].clone())
        .collect();
    ensure_resolved(&discovery)?;
    let context = NarrativeContext::from_script(&discovery);
    let mut validated = Vec::with_capacity(loader.files.len());
    for file in &loader.files {
        validated.push(parse_with_origins(&file.source, context).map_err(|error| {
            format!("Erreur de parsing dans `{}`:\n{error}", file.path.display())
        })?);
    }
    let mut script = Vec::with_capacity(loader.order.len());
    let mut origins = Vec::with_capacity(loader.order.len());
    for (file, root) in loader.order {
        script.push(validated[file].script[root].clone());
        origins.push(StatementOrigin {
            source: Some(loader.files[file].path.clone()),
            span: validated[file].spans[root].clone(),
        });
    }
    let mut program = ParsedProgram {
        script,
        context,
        origins,
        resolution: None,
        sources: loader
            .files
            .into_iter()
            .map(|file| ProgramSource {
                path: Some(file.path),
                source: file.source,
            })
            .collect(),
    };
    check_resolution_shape(&program.sources, &loader.occurrences)?;
    let proof = CapturedUseResolution { sources: program.sources.clone(), occurrences: loader.occurrences };
    proof.verify_for(&program)?;
    program.resolution = Some(proof);
    Ok(program)
}

fn ensure_resolved(script: &[Statement]) -> Result<(), String> {
    for statement in script {
        match statement {
            Statement::FunctionLibrary{scope}=>for member in scope.members(){ensure_resolved(std::slice::from_ref(member.declaration()))?;},
            Statement::Use { .. } => {
                return Err("Instruction `use` non résolue : utilisez un loader de programme complet ; les imports doivent être à la racine.".into());
            }
            Statement::Init { body }
            | Statement::Function { body, .. }
            | Statement::UnitFunction { body, .. }
            | Statement::Screen { body, .. }
            | Statement::Handler { body, .. }
            | Statement::ScheduledConsumer { body, .. }
            | Statement::While { body, .. }
            | Statement::ForEach { body, .. }
            | Statement::ForLoopWithBreak { body, .. } => ensure_resolved(body)?,
            Statement::Sequence { branches } => { for branch in branches { ensure_resolved(branch)?; } },
            Statement::StandardMacroDispatch{dispatch}=>{for branch in &dispatch.branches {ensure_resolved(&branch.body)?;}},
            Statement::If {
                then_branch,
                else_branch,
                ..
            } => {
                ensure_resolved(then_branch)?;
                ensure_resolved(else_branch)?;
            }
            Statement::Choice { options } => {
                for option in options {
                    ensure_resolved(&option.body)?;
                }
            }
            Statement::Imagemap { hotspots, .. } => {
                for hotspot in hotspots {
                    ensure_resolved(&hotspot.body)?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

struct FrozenFile {
    path: PathBuf,
    source: String,
    discovered: ParsedSource,
}

#[derive(Default)]
struct DiscoveryLoader {
    files: Vec<FrozenFile>,
    loaded: HashMap<PathBuf, usize>,
    stack: Vec<PathBuf>,
    order: Vec<(usize, usize)>,
    wildcard_targets: HashMap<PathBuf, Vec<PathBuf>>,
    occurrences: Vec<UseOccurrence>,
    source_bytes: usize,
    target_count: usize,
    metadata_bytes: usize,
}

impl DiscoveryLoader {
    fn charge_metadata(&mut self, bytes: usize) -> Result<(), String> {
        self.metadata_bytes = self.metadata_bytes.checked_add(bytes)
            .filter(|n| *n <= RESOLUTION_BYTES.saturating_sub(self.source_bytes))
            .ok_or("Captured Use metadata byte budget")?;
        Ok(())
    }
    fn finish(self) -> Result<ParsedProgram, String> {
        validate_frozen_program(self)
    }

    fn targets(&mut self, base: &Path, raw: &str) -> Result<(Vec<PathBuf>, Option<PathBuf>), String> {
        let Some(directory) = raw
            .strip_suffix("/*.rvn")
            .or_else(|| raw.strip_suffix("/*"))
        else {
            return Ok((vec![base.join(raw)], None));
        };
        let directory = fs::canonicalize(base.join(directory)).map_err(|error| {
            format!(
                "impossible de lire le dossier `use` `{}`: {error}",
                base.join(directory).display()
            )
        })?;
        if let Some(targets) = self.wildcard_targets.get(&directory) {
            return Ok((targets.clone(), Some(directory)));
        }
        let targets = crate::expand_use_path_bounded(base, raw, RESOLUTION_ITEMS)?;
        self.wildcard_targets.insert(directory.clone(), targets.clone());
        Ok((targets, Some(directory)))
    }

    fn load(&mut self, path: &Path) -> Result<(), String> {
        self.load_with_source(path, None).map(|_| ())
    }
    fn load_with_source(&mut self, path: &Path, supplied: Option<&str>) -> Result<PathBuf, String> {
        let canonical = match fs::canonicalize(path) {
            Ok(path) => path,
            Err(error) if supplied.is_some() && error.kind() == std::io::ErrorKind::NotFound => {
                let parent = path.parent().ok_or("Missing supplied root parent")?.canonicalize().map_err(|e| e.to_string())?;
                parent.join(path.file_name().ok_or("Missing supplied root filename")?)
            }
            Err(error) => return Err(format!("fichier RVN introuvable `{}`: {error}", path.display())),
        };
        if let Some(index) = self.stack.iter().position(|path| path == &canonical) {
            let mut cycle: Vec<_> = self.stack[index..]
                .iter()
                .map(|path| path.display().to_string())
                .collect();
            cycle.push(canonical.display().to_string());
            return Err(format!("cycle de `use` détecté: {}", cycle.join(" -> ")));
        }
        if self.loaded.contains_key(&canonical) { return Ok(canonical); }
        if self.stack.len() >= RESOLUTION_DEPTH || self.files.len() >= RESOLUTION_ITEMS
            || canonical.as_os_str().len() > 4096 { return Err("Captured Use source/depth budget".into()); }
        let length = if let Some(source) = supplied { source.len() as u64 }
            else { fs::metadata(&canonical).map_err(|e|e.to_string())?.len() };
        if length > RESOLUTION_BYTES.saturating_sub(self.source_bytes).saturating_sub(self.metadata_bytes) as u64 { return Err("Captured Use source byte budget".into()); }
        let source = if let Some(source) = supplied { source.to_owned() } else {
            use std::io::Read;
            let remaining = RESOLUTION_BYTES.saturating_sub(self.source_bytes).saturating_sub(self.metadata_bytes);
            let file = fs::File::open(&canonical).map_err(|error| format!("impossible de lire `{}`: {error}", canonical.display()))?;
            let mut source = String::new();
            file.take(remaining as u64 + 1).read_to_string(&mut source).map_err(|e| e.to_string())?;
            if source.len() > remaining { return Err("Captured Use source byte budget".into()); }
            source
        };
        self.source_bytes = self.source_bytes.checked_add(source.len()).filter(|n|*n <= RESOLUTION_BYTES.saturating_sub(self.metadata_bytes))
            .ok_or("Captured Use source byte budget")?;
        self.charge_metadata(canonical.as_os_str().len())?;
        let discovered =
            parse_with_origins(&source, NarrativeContext::ExplicitScenes).map_err(|error| {
                format!("Erreur de parsing dans `{}`:\n{error}", canonical.display())
            })?;
        if discovered.script.len() > RESOLUTION_ITEMS { return Err("Captured Use physical root budget".into()); }
        let file = self.files.len();
        self.loaded.insert(canonical.clone(), file);
        self.files.push(FrozenFile {
            path: canonical.clone(),
            source,
            discovered,
        });
        self.stack.push(canonical.clone());
        let base = canonical.parent().unwrap_or_else(|| Path::new("."));
        for root in 0..self.files[file].discovered.script.len() {
            if let Statement::Use { paths } = &self.files[file].discovered.script[root] {
                let paths = paths.clone();
                for (path_index, raw) in paths.into_iter().enumerate() {
                    if self.occurrences.len() >= RESOLUTION_ITEMS || raw.len() > 4096 { return Err("Captured Use occurrence budget".into()); }
                    let (targets, directory) = self.targets(base, &raw)?;
                    self.target_count = self.target_count.checked_add(targets.len()).filter(|n|*n <= RESOLUTION_ITEMS)
                        .ok_or("Captured Use target budget")?;
                    self.charge_metadata(canonical.as_os_str().len() + raw.len() + directory.as_ref().map_or(0, |p| p.as_os_str().len()))?;
                    let occurrence = self.occurrences.len();
                    self.occurrences.push(UseOccurrence { source: canonical.clone(), root, path_index, raw, directory, targets: Vec::new() });
                    for requested in targets {
                        if requested.as_os_str().len() > 4096 { return Err("Captured Use locator budget".into()); }
                        let target = self.load_with_source(&requested, None)?;
                        self.charge_metadata(requested.as_os_str().len() + target.as_os_str().len())?;
                        self.occurrences[occurrence].targets.push(UseTarget { requested, canonical: target });
                    }
                }
            } else {
                if self.order.len() >= RESOLUTION_ITEMS { return Err("Captured Use resolved root budget".into()); }
                self.order.push((file, root));
            }
        }
        self.stack.pop();
        Ok(canonical)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let nonce = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path =
                std::env::temp_dir().join(format!("rvn-c1-frozen-{}-{nonce}", std::process::id()));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn validation_uses_frozen_import_bytes_after_sources_change_or_disappear() {
        let fixture = Fixture::new();
        let root = fixture.0.join("root.rvn");
        let helper = fixture.0.join("helper.rvn");
        let root_source = "use \"helper.rvn\"\nscene label intro\ncall helper\n";
        let helper_source = "// été\nlabel helper\nlocal n = 1\nreturn\n";
        fs::write(&root, root_source).unwrap();
        fs::write(&helper, helper_source).unwrap();
        let mut loader = DiscoveryLoader::default();
        loader.load(&root).unwrap();
        fs::write(&root, "init { local invalid = 9 }\n").unwrap();
        fs::remove_file(&helper).unwrap();
        let program = loader.finish().unwrap();
        assert_eq!(program.context, NarrativeContext::ExplicitScenes);
        assert_eq!(program.sources[0].source, root_source);
        assert_eq!(program.sources[1].source, helper_source);
        assert!(matches!(&program.script[1], Statement::LocalVar {name,..} if name == "n"));
        assert_eq!(
            &helper_source[program.origins[1].span.range.clone()],
            "local n = 1"
        );
        assert_eq!(
            fs::read_to_string(&root).unwrap(),
            "init { local invalid = 9 }\n"
        );
        assert!(parse_file_program_with_uses(root).is_err());
    }

    #[test]
    fn wildcard_target_snapshot_is_reused_and_not_reread_during_validation() {
        let fixture = Fixture::new();
        let parts = fixture.0.join("parts");
        fs::create_dir(&parts).unwrap();
        fs::write(parts.join("a.rvn"), "scene label intro\nreturn\n").unwrap();
        let root = fixture.0.join("root.rvn");
        fs::write(&root, "use \"parts/*.rvn\"\n").unwrap();
        let mut loader = DiscoveryLoader::default();
        loader.load(&root).unwrap();
        let before = loader.targets(&fixture.0, "parts/*.rvn").unwrap();
        fs::write(parts.join("b.rvn"), "local invalid_in_init = 2\n").unwrap();
        let after = loader.targets(&fixture.0, "./parts/*").unwrap();
        assert_eq!(after, before);
        let program = loader.finish().unwrap();
        assert_eq!(program.sources.len(), 2);
        assert_eq!(program.script.len(), 2);
        assert_eq!(program.origins.len(), 2);
        assert!(fs::read_to_string(parts.join("b.rvn"))
            .unwrap()
            .contains("local"));
    }

    #[test]
    fn captured_use1021_wildcard_alias_replay_is_frozen_and_live_recheck_is_exact() {
        let fixture=Fixture::new();let parts=fixture.0.join("parts");fs::create_dir(&parts).unwrap();
        fs::write(parts.join("a.rvn"),"init {set order=1}\n").unwrap();
        fs::write(parts.join("b.rvn"),"init {set order=2}\n").unwrap();
        let root=fixture.0.join("main.rvn");
        fs::write(&root,"use {\"parts/*.rvn\",\"parts/../parts/b.rvn\"}\nscene label start\nreturn\n").unwrap();
        let program=parse_file_program_with_uses(&root).unwrap();
        let proof=program.resolution.as_ref().unwrap();proof.verify_for(&program).unwrap();
        assert_eq!(program.sources.len(),3);assert_eq!(proof.occurrences.len(),2);
        assert_eq!(proof.occurrences[0].targets.len(),2);
        assert_eq!(proof.occurrences[0].targets[1].canonical,proof.occurrences[1].targets[0].canonical);
        // Windows verbatim PathBuf::push normalizes dot segments. Requested
        // targets may therefore coincide; the captured source spelling must not.
        assert_eq!(proof.occurrences[0].raw,"parts/*.rvn");
        assert_eq!(proof.occurrences[1].raw,"parts/../parts/b.rvn");
        assert_eq!(proof.occurrences[1].targets[0].requested,
            proof.occurrences[1].source.parent().unwrap().join("parts/../parts/b.rvn"));
        let mut forged_alias=proof.clone();forged_alias.occurrences[1].raw="parts/b.rvn".into();
        assert!(forged_alias.verify_for(&program).is_err(),"the exact authored alias remains part of the frozen proof");
        // Captured directory membership is complete, including an empty source.
        fs::write(parts.join("c.rvn"),"").unwrap();
        assert!(recheck_file_program_resolution(&program).is_err());
        proof.verify_for(&program).unwrap();
        fs::remove_file(parts.join("a.rvn")).unwrap();fs::remove_file(parts.join("b.rvn")).unwrap();
        fs::remove_file(&root).unwrap();
        proof.verify_for(&program).unwrap(); // No filesystem access on this path.
        assert!(parse_file_program_with_uses(&root).is_err());
        fs::write(&root,"use \"parts/../main.rvn\"\n").unwrap();
        let error=parse_file_program_with_uses(&root).unwrap_err();
        assert!(error.contains("cycle"),"{error}"); // Stack hit before already-loaded dedup.
    }

    #[test]
    fn captured_use1021_tampered_edges_bytes_origins_and_source_inventory_are_refused() {
        let fixture=Fixture::new();let parts=fixture.0.join("parts");fs::create_dir(&parts).unwrap();
        fs::write(parts.join("a.rvn"),"init {set order=1}\n").unwrap();
        fs::write(parts.join("b.rvn"),"init {set order=2}\n").unwrap();
        let root=fixture.0.join("main.rvn");fs::write(&root,"use \"parts/*\"\nscene label start\nreturn\n").unwrap();
        let program=parse_file_program_with_uses(&root).unwrap();let proof=program.resolution.as_ref().unwrap();
        let mut bad=proof.clone();bad.occurrences[0].targets.remove(0);assert!(bad.verify_for(&program).is_err());
        let mut bad=proof.clone();bad.occurrences[0].targets.reverse();assert!(bad.verify_for(&program).is_err());
        let mut bad=proof.clone();bad.occurrences[0].targets[0].canonical=bad.occurrences[0].targets[1].canonical.clone();assert!(bad.verify_for(&program).is_err());
        let mut bad=proof.clone();bad.occurrences.push(bad.occurrences[0].clone());assert!(bad.verify_for(&program).is_err());
        let mut bad=program.clone();bad.origins[0].span.range.start+=1;assert!(proof.verify_for(&bad).is_err());
        let mut bad=program.clone();bad.script.swap(0,1);bad.origins.swap(0,1);assert!(proof.verify_for(&bad).is_err());
        let mut bad=program.clone();bad.sources[1].source.push_str("// changed\n");assert!(proof.verify_for(&bad).is_err());
        let mut bad=program.clone();bad.sources.swap(1,2);assert!(proof.verify_for(&bad).is_err());
        let mut bad=program.clone();bad.sources.pop();assert!(proof.verify_for(&bad).is_err());
        let mut bad=proof.clone();bad.occurrences[0].raw="other/*".into();assert!(bad.verify_for(&program).is_err());
        let mut bad=proof.clone();bad.occurrences[0].targets[0].canonical=program.sources[0].path.clone().unwrap();assert!(bad.verify_for(&program).unwrap_err().contains("cycle"));
        let mut oversized=program.clone();oversized.sources[0].source=" ".repeat(RESOLUTION_BYTES+1);
        assert!(capture_file_program_resolution(&root,&oversized.sources[0].source,&mut program.clone()).is_err());
        assert!(parse_program("use \"parts/*\"\n").is_err());
    }

    #[test]
    fn captured_use1021_debug_identity_equality_and_anonymous_none_are_legacy_exact() {
        let fixture=Fixture::new();let root=fixture.0.join("main.rvn");
        let raw="scene label start\n\"Été世界\"\nreturn\n";fs::write(&root,raw).unwrap();
        let captured=parse_file_program_with_uses(&root).unwrap();let mut none=captured.clone();none.resolution=None;
        assert_eq!(captured,none);
        let historical=format!("ParsedProgram {{ script: {:?}, context: {:?}, origins: {:?}, sources: {:?} }}",captured.script,captured.context,captured.origins,captured.sources);
        assert_eq!(format!("{captured:?}"),historical);assert_eq!(format!("{none:?}"),historical);
        assert!(parse_program(raw).unwrap().resolution.is_none());
        // Real editor-owned root bytes are supported even before that file exists.
        let unpublished=fixture.0.join("unpublished.rvn");
        let p=parse_file_program_with_uses_from_source(&unpublished,raw).unwrap();
        p.resolution.as_ref().unwrap().verify_for(&p).unwrap();assert!(!unpublished.exists());
        let before=captured.clone();let mut changed=captured.clone();changed.sources[0].source.push_str("// forged\n");
        assert!(capture_file_program_resolution(&root,raw,&mut changed).is_err());
        assert_eq!(captured,before);
    }

}
