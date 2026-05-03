use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use rvn_parser::{
    parse, AnimationParam, AnimationValue, Expr, Hotspot, InterpolatedText, Rect, Script,
    Statement, TextSegment,
};

use crate::load_project_config;

#[derive(Debug, Clone, Copy, Default)]
pub struct CheckOptions {
    pub strict: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub severity: Severity,
    pub file: Option<PathBuf>,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub kind: &'static str,
    pub message: String,
    pub suggestion: Option<String>,
}

impl Diagnostic {
    fn error(kind: &'static str, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            file: None,
            line: None,
            column: None,
            kind,
            message: message.into(),
            suggestion: None,
        }
    }

    fn warning(kind: &'static str, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warning,
            file: None,
            line: None,
            column: None,
            kind,
            message: message.into(),
            suggestion: None,
        }
    }

    fn at(mut self, loc: Option<Location>) -> Self {
        if let Some(loc) = loc {
            self.file = Some(loc.file);
            self.line = Some(loc.line);
            self.column = Some(loc.column);
        }
        self
    }

    fn suggest(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sev = match self.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        write!(f, "{sev}[{}]: {}", self.kind, self.message)?;
        if let Some(file) = &self.file {
            write!(f, " in {}", file.display())?;
            if let Some(line) = self.line {
                write!(f, ":{line}")?;
                if let Some(column) = self.column {
                    write!(f, ":{column}")?;
                }
            }
        }
        if let Some(suggestion) = &self.suggestion {
            write!(f, "\n  suggestion: {suggestion}")?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct CheckReport {
    pub diagnostics: Vec<Diagnostic>,
    pub failed: bool,
}

impl CheckReport {
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error)
    }

    pub fn error_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .count()
    }

    pub fn warning_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Warning)
            .count()
    }
}

#[derive(Debug, Clone)]
struct Location {
    file: PathBuf,
    line: usize,
    column: usize,
}

#[derive(Debug)]
struct SourceFile {
    path: PathBuf,
    source: String,
}

#[derive(Debug)]
struct ProjectScripts {
    files: Vec<SourceFile>,
    units: Vec<ScriptUnit>,
}

#[derive(Debug)]
struct ScriptUnit {
    file_index: usize,
    script: Script,
}

#[derive(Debug, Default)]
struct Symbols {
    labels: HashMap<String, Location>,
    label_order: Vec<String>,
    duplicate_labels: Vec<(String, Location)>,
    explicit_label_refs: Vec<(String, Location, &'static str)>,
    declared_characters: HashMap<String, Location>,
    used_characters: Vec<(String, Location)>,
    backgrounds: Vec<(String, Location)>,
    sprites: Vec<(String, Option<String>, Location)>,
    music_files: Vec<(String, Location)>,
    sfx_files: Vec<(String, Location)>,
    assigned_vars: HashMap<String, Location>,
    used_vars: Vec<(String, Location)>,
    locale_keys: HashSet<String>,
}

pub fn check_project(project: &str, options: CheckOptions) -> CheckReport {
    let mut cx = CheckContext::new(project);
    cx.run();
    let failed = cx.diagnostics.iter().any(|d| {
        d.severity == Severity::Error || (options.strict && d.severity == Severity::Warning)
    });
    CheckReport {
        diagnostics: cx.diagnostics,
        failed,
    }
}

struct CheckContext<'a> {
    project: &'a str,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> CheckContext<'a> {
    fn new(project: &'a str) -> Self {
        Self {
            project,
            diagnostics: Vec::new(),
        }
    }

    fn run(&mut self) {
        let project_dir = Path::new(self.project);
        let cfg = match load_project_config(project_dir) {
            Ok(cfg) => cfg,
            Err(e) => {
                self.diagnostics
                    .push(Diagnostic::error("config", e).at(Some(Location {
                        file: project_dir.join("rvn.toml"),
                        line: 1,
                        column: 1,
                    })));
                return;
            }
        };

        let main_script = project_dir.join(&cfg.project.main_script);
        let scripts = match load_scripts(&main_script) {
            Ok(scripts) => scripts,
            Err(e) => {
                self.diagnostics
                    .push(Diagnostic::error("script-load", e).at(Some(Location {
                        file: main_script,
                        line: 1,
                        column: 1,
                    })));
                return;
            }
        };

        let symbols = collect_symbols(&scripts, &mut self.diagnostics);
        validate_symbols(
            &symbols,
            cfg.project.start_label.as_deref(),
            &mut self.diagnostics,
        );
        analyze_control_flow(
            &scripts,
            &symbols,
            cfg.project.start_label.as_deref(),
            &mut self.diagnostics,
        );
        validate_assets(project_dir, &cfg.paths, &symbols, &mut self.diagnostics);
        validate_locales(
            project_dir,
            &cfg.paths.locales,
            &symbols.locale_keys,
            &mut self.diagnostics,
        );
        validate_orphan_scripts(
            project_dir,
            &cfg.project.main_script,
            &scripts,
            &mut self.diagnostics,
        );
    }
}

fn load_scripts(main_script: &Path) -> Result<ProjectScripts, String> {
    let mut files = Vec::new();
    let mut units = Vec::new();
    let mut loaded = HashSet::new();
    let mut stack = Vec::new();
    load_script_inner(main_script, &mut files, &mut units, &mut loaded, &mut stack)?;
    Ok(ProjectScripts { files, units })
}

fn load_script_inner(
    path: &Path,
    files: &mut Vec<SourceFile>,
    units: &mut Vec<ScriptUnit>,
    loaded: &mut HashSet<PathBuf>,
    stack: &mut Vec<PathBuf>,
) -> Result<(), String> {
    let canonical = fs::canonicalize(path)
        .map_err(|e| format!("fichier RVN introuvable `{}`: {e}", path.display()))?;
    if let Some(pos) = stack.iter().position(|p| p == &canonical) {
        let mut cycle: Vec<String> = stack[pos..]
            .iter()
            .map(|p| p.display().to_string())
            .collect();
        cycle.push(canonical.display().to_string());
        return Err(format!("cycle de `use` détecté: {}", cycle.join(" -> ")));
    }
    if !loaded.insert(canonical.clone()) {
        return Ok(());
    }

    stack.push(canonical.clone());
    let source = fs::read_to_string(&canonical)
        .map_err(|e| format!("impossible de lire `{}`: {e}", canonical.display()))?;
    let parsed = parse(&source)
        .map_err(|e| format!("Erreur de parsing dans `{}`:\n{}", canonical.display(), e))?;
    let file_index = files.len();
    files.push(SourceFile {
        path: canonical.clone(),
        source,
    });
    let base_dir = canonical.parent().unwrap_or_else(|| Path::new("."));
    let mut own = Vec::new();
    for stmt in parsed {
        match stmt {
            Statement::Use { paths } => {
                for use_path in paths {
                    for target in expand_use_path(base_dir, &use_path)? {
                        load_script_inner(&target, files, units, loaded, stack)?;
                    }
                }
            }
            other => own.push(other),
        }
    }
    units.push(ScriptUnit {
        file_index,
        script: own,
    });
    stack.pop();
    Ok(())
}

fn expand_use_path(base_dir: &Path, raw: &str) -> Result<Vec<PathBuf>, String> {
    if raw.ends_with("/*") || raw.ends_with("/*.rvn") {
        let dir_part = raw
            .strip_suffix("/*.rvn")
            .or_else(|| raw.strip_suffix("/*"))
            .unwrap_or(raw);
        let dir = base_dir.join(dir_part);
        let mut files = Vec::new();
        for entry in fs::read_dir(&dir).map_err(|e| {
            format!(
                "impossible de lire le dossier `use` `{}`: {e}",
                dir.display()
            )
        })? {
            let path = entry
                .map_err(|e| format!("erreur de lecture dans `{}`: {e}", dir.display()))?
                .path();
            if path.extension().and_then(|s| s.to_str()) == Some("rvn") {
                files.push(path);
            }
        }
        files.sort();
        if files.is_empty() {
            return Err(format!(
                "aucun fichier `.rvn` trouvé pour le `use` wildcard `{}`",
                dir.display()
            ));
        }
        return Ok(files);
    }
    Ok(vec![base_dir.join(raw)])
}

fn collect_symbols(scripts: &ProjectScripts, diagnostics: &mut Vec<Diagnostic>) -> Symbols {
    let mut symbols = Symbols::default();
    for unit in &scripts.units {
        let source = &scripts.files[unit.file_index];
        collect_block(&unit.script, source, &mut symbols, diagnostics);
    }
    symbols
}

fn collect_block(
    stmts: &[Statement],
    source: &SourceFile,
    symbols: &mut Symbols,
    diagnostics: &mut Vec<Diagnostic>,
) {
    detect_dead_code(stmts, source, diagnostics);
    for stmt in stmts {
        let loc = locate_stmt(source, stmt);
        match stmt {
            Statement::Use { .. } => {}
            Statement::Init { body } => collect_block(body, source, symbols, diagnostics),
            Statement::CharacterCreate { id, .. } => {
                if let Some(prev) = symbols.declared_characters.insert(id.clone(), loc.clone()) {
                    diagnostics.push(
                        Diagnostic::warning(
                            "duplicate-character",
                            format!("character '{id}' is declared more than once"),
                        )
                        .at(Some(loc.clone()))
                        .suggest(format!(
                            "previous declaration is in {}",
                            prev.file.display()
                        )),
                    );
                }
            }
            Statement::Dialogue { character_id, text } => {
                if let Some(id) = character_id {
                    symbols.used_characters.push((id.clone(), loc.clone()));
                }
                collect_text_vars(text, &mut symbols.used_vars, &loc);
                symbols.locale_keys.insert(text_to_locale_key(text));
            }
            Statement::Choice { options } => {
                validate_duplicate_choice_text(options, &loc, diagnostics);
                for (label, body) in options {
                    collect_text_vars(label, &mut symbols.used_vars, &loc);
                    symbols.locale_keys.insert(text_to_locale_key(label));
                    collect_block(body, source, symbols, diagnostics);
                }
            }
            Statement::SetVar { name, value } => {
                collect_expr_vars(value, &mut symbols.used_vars, &loc);
                symbols
                    .assigned_vars
                    .entry(name.clone())
                    .or_insert(loc.clone());
            }
            Statement::If {
                condition,
                then_branch,
                else_branch,
            } => {
                collect_expr_vars(condition, &mut symbols.used_vars, &loc);
                collect_block(then_branch, source, symbols, diagnostics);
                collect_block(else_branch, source, symbols, diagnostics);
            }
            Statement::Label { name } => {
                if symbols.labels.contains_key(name) {
                    symbols.duplicate_labels.push((name.clone(), loc.clone()));
                } else {
                    symbols.labels.insert(name.clone(), loc.clone());
                    symbols.label_order.push(name.clone());
                }
            }
            Statement::Jump { target } => {
                symbols
                    .explicit_label_refs
                    .push((target.clone(), loc.clone(), "jump"))
            }
            Statement::Call { target } => {
                symbols
                    .explicit_label_refs
                    .push((target.clone(), loc.clone(), "call"))
            }
            Statement::Scene { background, .. } => {
                symbols.backgrounds.push((background.clone(), loc.clone()))
            }
            Statement::ShowSprite {
                character_id,
                emotion,
                ..
            } => {
                symbols
                    .used_characters
                    .push((character_id.clone(), loc.clone()));
                symbols
                    .sprites
                    .push((character_id.clone(), emotion.clone(), loc.clone()));
            }
            Statement::HideSprite { character_id, .. }
            | Statement::MoveSprite { character_id, .. }
            | Statement::SpriteAnimate { character_id, .. }
            | Statement::SpriteStopAnimation { character_id } => {
                symbols
                    .used_characters
                    .push((character_id.clone(), loc.clone()));
            }
            Statement::MethodCall {
                target,
                method,
                arg,
                ..
            } => {
                if method == "show" {
                    symbols.used_characters.push((target.clone(), loc.clone()));
                    symbols
                        .sprites
                        .push((target.clone(), arg.clone(), loc.clone()));
                }
            }
            Statement::MusicPlay { file, .. } => {
                symbols.music_files.push((file.clone(), loc.clone()))
            }
            Statement::SfxPlay { file, .. } | Statement::SfxStop { file, .. } => {
                symbols.sfx_files.push((file.clone(), loc.clone()))
            }
            Statement::Imagemap {
                background,
                hover_image,
                hotspots,
            } => {
                symbols.backgrounds.push((background.clone(), loc.clone()));
                if let Some(hover_image) = hover_image {
                    symbols.backgrounds.push((hover_image.clone(), loc.clone()));
                }
                validate_imagemap_overlaps(hotspots, &loc, diagnostics);
                for hotspot in hotspots {
                    collect_block(&hotspot.body, source, symbols, diagnostics);
                }
            }
            Statement::Config { .. }
            | Statement::Return
            | Statement::MusicStop { .. }
            | Statement::MusicVolume { .. }
            | Statement::TypewriterSet { .. }
            | Statement::TypewriterSpeed { .. } => {}
        }
        if let Statement::SpriteAnimate {
            animation, params, ..
        } = stmt
        {
            validate_animation(animation, params, &loc, diagnostics);
        }
        if let Statement::Choice { options } = stmt {
            if options.is_empty() {
                diagnostics.push(
                    Diagnostic::error("empty-choice", "`choice` block has no option")
                        .at(Some(loc.clone()))
                        .suggest("add at least one `\"Label\" => { ... }` entry"),
                );
            }
        }
        if let Statement::Imagemap { hotspots, .. } = stmt {
            if hotspots.is_empty() {
                diagnostics.push(
                    Diagnostic::error("empty-imagemap", "`imagemap` has no hotspot")
                        .at(Some(loc.clone()))
                        .suggest("add at least one `hotspot { area: (...) } => { ... }` block"),
                );
            }
        }
    }
}

fn validate_symbols(
    symbols: &Symbols,
    start_label: Option<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (label, loc) in &symbols.duplicate_labels {
        diagnostics.push(
            Diagnostic::error("duplicate-label", format!("duplicate label '{label}'"))
                .at(Some(loc.clone())),
        );
    }
    if let Some(start_label) = start_label {
        if !symbols.labels.contains_key(start_label) {
            diagnostics.push(
                Diagnostic::error(
                    "missing-start-label",
                    format!("start_label '{start_label}' does not exist in the resolved project scripts"),
                )
                .suggest(format!("add `label {start_label}` or update rvn.toml")),
            );
        }
    }

    for (target, loc, kind) in &symbols.explicit_label_refs {
        if !symbols.labels.contains_key(target) {
            let mut diag = Diagnostic::error(
                "unknown-label-target",
                format!("unknown {kind} target '{target}'"),
            )
            .at(Some(loc.clone()));
            if let Some(suggestion) = nearest(target, symbols.labels.keys()) {
                diag = diag.suggest(format!("did you mean '{suggestion}'?"));
            }
            diagnostics.push(diag);
        }
    }

    for (id, loc) in &symbols.used_characters {
        if !symbols.declared_characters.contains_key(id) {
            diagnostics.push(
                Diagnostic::error(
                    "undefined-character",
                    format!("undefined character id '{id}'"),
                )
                .at(Some(loc.clone()))
                .suggest(format!(
                    "declare it in init with `character.create(\"{id}\", \"Display Name\")`"
                )),
            );
        }
    }
    for (id, loc) in &symbols.declared_characters {
        if !symbols.used_characters.iter().any(|(used, _)| used == id) {
            diagnostics.push(
                Diagnostic::warning(
                    "unused-character",
                    format!("character '{id}' is declared but never used"),
                )
                .at(Some(loc.clone())),
            );
        }
    }

    let used_vars: HashSet<&str> = symbols.used_vars.iter().map(|(v, _)| v.as_str()).collect();
    for (var, loc) in &symbols.used_vars {
        if !var.starts_with("__") && !symbols.assigned_vars.contains_key(var) {
            diagnostics.push(
                Diagnostic::error(
                    "unassigned-variable",
                    format!("variable '{var}' is used before being assigned"),
                )
                .at(Some(loc.clone()))
                .suggest(format!("assign it earlier with `set {var} = ...`")),
            );
        }
    }
    for (var, loc) in &symbols.assigned_vars {
        if !used_vars.contains(var.as_str()) {
            diagnostics.push(
                Diagnostic::warning(
                    "unused-variable",
                    format!("variable '{var}' is assigned but never used"),
                )
                .at(Some(loc.clone())),
            );
        }
    }
}

fn analyze_control_flow(
    scripts: &ProjectScripts,
    symbols: &Symbols,
    start_label: Option<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if symbols.labels.is_empty() {
        return;
    }
    let entry = start_label
        .filter(|s| symbols.labels.contains_key(*s))
        .map(str::to_string)
        .or_else(|| symbols.label_order.first().cloned());
    let Some(entry) = entry else { return };

    let mut blocks: HashMap<String, Vec<&Statement>> = HashMap::new();
    for unit in &scripts.units {
        let mut current: Option<String> = None;
        for stmt in &unit.script {
            if let Statement::Label { name } = stmt {
                current = Some(name.clone());
                blocks.entry(name.clone()).or_default();
            } else if let Some(label) = &current {
                blocks.entry(label.clone()).or_default().push(stmt);
            }
        }
    }

    let mut edges: HashMap<String, HashSet<String>> = HashMap::new();
    for (idx, label) in symbols.label_order.iter().enumerate() {
        let mut targets = HashSet::new();
        let mut terminates = false;
        if let Some(stmts) = blocks.get(label) {
            collect_flow_targets(stmts, &mut targets, &mut terminates);
        }
        if !terminates {
            if let Some(next) = symbols.label_order.get(idx + 1) {
                targets.insert(next.clone());
            }
        }
        edges.insert(label.clone(), targets);
    }

    let mut reachable = HashSet::new();
    let mut queue = VecDeque::from([entry.clone()]);
    while let Some(label) = queue.pop_front() {
        if !reachable.insert(label.clone()) {
            continue;
        }
        if let Some(targets) = edges.get(&label) {
            for target in targets {
                if symbols.labels.contains_key(target) {
                    queue.push_back(target.clone());
                }
            }
        }
    }

    let explicit_refs: HashSet<&str> = symbols
        .explicit_label_refs
        .iter()
        .map(|(target, _, _)| target.as_str())
        .collect();
    for label in &symbols.label_order {
        if label == &entry {
            continue;
        }
        let loc = symbols.labels.get(label).cloned();
        if !reachable.contains(label) {
            diagnostics.push(
                Diagnostic::warning(
                    "unreachable-label",
                    format!("label '{label}' is unreachable"),
                )
                .at(loc),
            );
        } else if !explicit_refs.contains(label.as_str()) {
            diagnostics.push(
                Diagnostic::warning(
                    "unreferenced-label",
                    format!("label '{label}' is never explicitly referenced"),
                )
                .at(loc)
                .suggest("this label is reachable only by fallthrough"),
            );
        }
    }
}

fn collect_flow_targets(
    stmts: &[&Statement],
    targets: &mut HashSet<String>,
    terminates: &mut bool,
) {
    for stmt in stmts {
        match stmt {
            Statement::Jump { target } => {
                targets.insert(target.clone());
                *terminates = true;
                return;
            }
            Statement::Call { target } => {
                targets.insert(target.clone());
            }
            Statement::Return => {
                *terminates = true;
                return;
            }
            Statement::Choice { options } => {
                for (_, body) in options {
                    collect_nested_flow_targets(body, targets);
                }
            }
            Statement::If {
                then_branch,
                else_branch,
                ..
            } => {
                collect_nested_flow_targets(then_branch, targets);
                collect_nested_flow_targets(else_branch, targets);
            }
            Statement::Imagemap { hotspots, .. } => {
                for hotspot in hotspots {
                    collect_nested_flow_targets(&hotspot.body, targets);
                }
            }
            _ => {}
        }
    }
}

fn collect_nested_flow_targets(stmts: &[Statement], targets: &mut HashSet<String>) {
    for stmt in stmts {
        match stmt {
            Statement::Jump { target } | Statement::Call { target } => {
                targets.insert(target.clone());
            }
            Statement::Choice { options } => {
                for (_, body) in options {
                    collect_nested_flow_targets(body, targets);
                }
            }
            Statement::If {
                then_branch,
                else_branch,
                ..
            } => {
                collect_nested_flow_targets(then_branch, targets);
                collect_nested_flow_targets(else_branch, targets);
            }
            Statement::Imagemap { hotspots, .. } => {
                for hotspot in hotspots {
                    collect_nested_flow_targets(&hotspot.body, targets);
                }
            }
            _ => {}
        }
    }
}

fn detect_dead_code(stmts: &[Statement], source: &SourceFile, diagnostics: &mut Vec<Diagnostic>) {
    let mut terminated_by: Option<&'static str> = None;
    for stmt in stmts {
        if matches!(stmt, Statement::Label { .. }) {
            terminated_by = None;
        }
        if let Some(kind) = terminated_by {
            diagnostics.push(
                Diagnostic::warning(
                    "dead-code",
                    format!("statement is unreachable after `{kind}`"),
                )
                .at(Some(locate_stmt(source, stmt))),
            );
            continue;
        }
        match stmt {
            Statement::Jump { .. } => terminated_by = Some("jump"),
            Statement::Return => terminated_by = Some("return"),
            _ => {}
        }
    }
}

fn validate_duplicate_choice_text(
    options: &[(InterpolatedText, Vec<Statement>)],
    loc: &Location,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut seen = HashSet::new();
    for (label, _) in options {
        let text = text_to_locale_key(label);
        if !seen.insert(text.clone()) {
            diagnostics.push(
                Diagnostic::warning(
                    "duplicate-choice-text",
                    format!("duplicate choice text '{text}'"),
                )
                .at(Some(loc.clone())),
            );
        }
    }
}

fn validate_imagemap_overlaps(
    hotspots: &[Hotspot],
    loc: &Location,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (i, a) in hotspots.iter().enumerate() {
        for b in hotspots.iter().skip(i + 1) {
            if rects_overlap(&a.area, &b.area) {
                let left = a.name.as_deref().unwrap_or("<unnamed>");
                let right = b.name.as_deref().unwrap_or("<unnamed>");
                diagnostics.push(
                    Diagnostic::warning(
                        "imagemap-overlap",
                        format!("imagemap hotspots '{left}' and '{right}' overlap"),
                    )
                    .at(Some(loc.clone())),
                );
            }
        }
    }
}

fn rects_overlap(a: &Rect, b: &Rect) -> bool {
    a.x1 < b.x2 && a.x2 > b.x1 && a.y1 < b.y2 && a.y2 > b.y1
}

fn validate_animation(
    animation: &str,
    params: &[AnimationParam],
    loc: &Location,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let allowed_params: &[&str] = match animation {
        "shake" => &["loop", "duration", "intensity"],
        "bounce" => &["loop", "duration", "height"],
        "pulse" => &["loop", "duration", "scale"],
        other => {
            diagnostics.push(
                Diagnostic::error("unknown-animation", format!("unknown animation '{other}'"))
                    .at(Some(loc.clone()))
                    .suggest("supported animations: shake, bounce, pulse"),
            );
            return;
        }
    };
    for param in params {
        if !allowed_params.contains(&param.name.as_str()) {
            diagnostics.push(
                Diagnostic::error(
                    "unknown-animation-param",
                    format!(
                        "unknown parameter '{}' for animation '{}'",
                        param.name, animation
                    ),
                )
                .at(Some(loc.clone()))
                .suggest(format!("allowed parameters: {}", allowed_params.join(", "))),
            );
            continue;
        }
        match (param.name.as_str(), &param.value) {
            ("loop", AnimationValue::Bool(_)) => {}
            ("duration" | "intensity" | "height" | "scale", AnimationValue::Int(_)) => {}
            ("duration" | "intensity" | "height" | "scale", AnimationValue::Float(_)) => {}
            ("loop", _) => diagnostics.push(
                Diagnostic::error(
                    "invalid-animation-param",
                    format!("parameter `loop` for animation `{animation}` must be a boolean"),
                )
                .at(Some(loc.clone())),
            ),
            (name, _) => diagnostics.push(
                Diagnostic::error(
                    "invalid-animation-param",
                    format!("parameter `{name}` for animation `{animation}` must be a number"),
                )
                .at(Some(loc.clone())),
            ),
        }
    }
}

fn validate_assets(
    project_dir: &Path,
    paths: &crate::PathsSection,
    symbols: &Symbols,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let assets_dir = project_dir.join(&paths.assets);
    let bg_exts = ["png", "jpg", "jpeg", "webp"];
    for (bg, loc) in &symbols.backgrounds {
        if !asset_exists(&assets_dir, bg, &bg_exts) {
            diagnostics.push(
                Diagnostic::error(
                    "missing-asset",
                    format!(
                        "background '{bg}' not found under '{}'",
                        assets_dir.display()
                    ),
                )
                .at(Some(loc.clone())),
            );
        }
    }
    let sprite_exts = ["png", "jpg", "jpeg", "webp"];
    for (character_id, emotion, loc) in &symbols.sprites {
        let sprite_path = match emotion {
            Some(emotion) => format!("sprites/{character_id}/{emotion}"),
            None => format!("sprites/{character_id}/default"),
        };
        if !asset_exists(&assets_dir, &sprite_path, &sprite_exts) {
            diagnostics.push(
                Diagnostic::error(
                    "missing-asset",
                    format!(
                        "sprite '{sprite_path}' not found under '{}'",
                        assets_dir.display()
                    ),
                )
                .at(Some(loc.clone())),
            );
        }
    }
    let audio_exts = ["ogg", "mp3", "wav", "flac"];
    for (music, loc) in &symbols.music_files {
        let path = if music.starts_with("music/") {
            music.clone()
        } else {
            format!("music/{music}")
        };
        if !asset_exists(&assets_dir, &path, &audio_exts) {
            diagnostics.push(
                Diagnostic::error(
                    "missing-asset",
                    format!("music '{path}' not found under '{}'", assets_dir.display()),
                )
                .at(Some(loc.clone())),
            );
        }
    }
    for (sfx, loc) in &symbols.sfx_files {
        let path = if sfx.starts_with("sfx/") {
            sfx.clone()
        } else {
            format!("sfx/{sfx}")
        };
        if !asset_exists(&assets_dir, &path, &audio_exts) {
            diagnostics.push(
                Diagnostic::error(
                    "missing-asset",
                    format!(
                        "sound effect '{path}' not found under '{}'",
                        assets_dir.display()
                    ),
                )
                .at(Some(loc.clone())),
            );
        }
    }
    if !project_dir.join(&paths.theme).exists() {
        diagnostics.push(Diagnostic::error(
            "missing-project-file",
            format!(
                "theme file '{}' not found",
                project_dir.join(&paths.theme).display()
            ),
        ));
    }
    if !project_dir.join(&paths.locales).exists() {
        diagnostics.push(Diagnostic::error(
            "missing-project-dir",
            format!(
                "locales directory '{}' not found",
                project_dir.join(&paths.locales).display()
            ),
        ));
    }
    if !project_dir.join(&paths.saves).exists() {
        diagnostics.push(Diagnostic::error(
            "missing-project-dir",
            format!(
                "saves directory '{}' not found",
                project_dir.join(&paths.saves).display()
            ),
        ));
    }
}

fn validate_locales(
    project_dir: &Path,
    locales_path: &str,
    used_keys: &HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let dir = project_dir.join(locales_path);
    let Ok(entries) = fs::read_dir(&dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("toml") {
            continue;
        }
        let Ok(source) = fs::read_to_string(&path) else {
            continue;
        };
        let keys = parse_locale_keys(&source);
        for key in used_keys {
            if !keys.contains(key) {
                diagnostics.push(
                    Diagnostic::warning(
                        "missing-locale-key",
                        format!(
                            "locale key '{key}' is used but missing from '{}'",
                            path.display()
                        ),
                    )
                    .at(Some(Location {
                        file: path.clone(),
                        line: 1,
                        column: 1,
                    })),
                );
            }
        }
        for key in keys {
            if !used_keys.contains(&key) {
                diagnostics.push(
                    Diagnostic::warning(
                        "unused-locale-key",
                        format!(
                            "locale key '{key}' is present in '{}' but never used",
                            path.display()
                        ),
                    )
                    .at(Some(Location {
                        file: path.clone(),
                        line: 1,
                        column: 1,
                    })),
                );
            }
        }
    }
}

fn parse_locale_keys(source: &str) -> HashSet<String> {
    match source.parse::<toml::Value>() {
        Ok(toml::Value::Table(table)) => table
            .get("strings")
            .and_then(|v| v.as_table())
            .map(|strings| strings.keys().cloned().collect())
            .unwrap_or_default(),
        _ => HashSet::new(),
    }
}

fn validate_orphan_scripts(
    project_dir: &Path,
    main_script: &str,
    scripts: &ProjectScripts,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let scripts_dir = project_dir
        .join(main_script)
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| project_dir.join("scripts"));
    let loaded: HashSet<PathBuf> = scripts.files.iter().map(|f| f.path.clone()).collect();
    let mut all = Vec::new();
    collect_rvn_files(&scripts_dir, &mut all);
    for file in all {
        if let Ok(canonical) = fs::canonicalize(&file) {
            if !loaded.contains(&canonical) {
                diagnostics.push(
                    Diagnostic::warning(
                        "orphan-script",
                        format!(
                            "script '{}' is not included by the main script graph",
                            file.display()
                        ),
                    )
                    .at(Some(Location {
                        file,
                        line: 1,
                        column: 1,
                    })),
                );
            }
        }
    }
}

fn collect_rvn_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rvn_files(&path, out);
        } else if path.extension().and_then(|s| s.to_str()) == Some("rvn") {
            out.push(path);
        }
    }
}

fn asset_exists(assets_dir: &Path, path_without_or_with_ext: &str, extensions: &[&str]) -> bool {
    let direct = assets_dir.join(path_without_or_with_ext);
    if direct.extension().is_some() {
        return direct.exists();
    }
    extensions.iter().any(|ext| {
        assets_dir
            .join(format!("{path_without_or_with_ext}.{ext}"))
            .exists()
    })
}

fn collect_text_vars(text: &InterpolatedText, out: &mut Vec<(String, Location)>, loc: &Location) {
    for segment in &text.0 {
        if let TextSegment::Interp(expr) = segment {
            collect_expr_vars(expr, out, loc);
        }
    }
}

fn collect_expr_vars(expr: &Expr, out: &mut Vec<(String, Location)>, loc: &Location) {
    match expr {
        Expr::Var(name) => out.push((name.clone(), loc.clone())),
        Expr::BinOp { left, right, .. } | Expr::And(left, right) | Expr::Or(left, right) => {
            collect_expr_vars(left, out, loc);
            collect_expr_vars(right, out, loc);
        }
        Expr::Neg(inner) | Expr::Not(inner) => collect_expr_vars(inner, out, loc),
        Expr::Int(_) | Expr::Float(_) | Expr::Bool(_) | Expr::Str(_) => {}
    }
}

fn text_to_locale_key(text: &InterpolatedText) -> String {
    text.0
        .iter()
        .map(|segment| match segment {
            TextSegment::Lit(s) => s.clone(),
            TextSegment::Interp(expr) => format!("[{}]", expr_to_display(expr)),
        })
        .collect()
}

fn expr_to_display(expr: &Expr) -> String {
    match expr {
        Expr::Int(n) => n.to_string(),
        Expr::Float(f) => f.to_string(),
        Expr::Bool(b) => b.to_string(),
        Expr::Str(s) => s.clone(),
        Expr::Var(n) => n.clone(),
        Expr::Neg(e) => format!("-{}", expr_to_display(e)),
        Expr::Not(e) => format!("not {}", expr_to_display(e)),
        Expr::And(l, r) => format!("{} and {}", expr_to_display(l), expr_to_display(r)),
        Expr::Or(l, r) => format!("{} or {}", expr_to_display(l), expr_to_display(r)),
        Expr::BinOp { op, left, right } => format!(
            "{} {:?} {}",
            expr_to_display(left),
            op,
            expr_to_display(right)
        ),
    }
}

fn locate_stmt(source: &SourceFile, stmt: &Statement) -> Location {
    let needles: Vec<String> = match stmt {
        Statement::Label { name } => vec![format!("label {name}")],
        Statement::Jump { target } => vec![format!("jump {target}")],
        Statement::Call { target } => vec![format!("call {target}")],
        Statement::SetVar { name, .. } => vec![format!("set {name}")],
        Statement::Dialogue {
            character_id: Some(id),
            ..
        } => vec![format!("{id} \"")],
        Statement::Choice { .. } => vec!["choice".to_string()],
        Statement::If { .. } => vec!["if ".to_string()],
        Statement::Return => vec!["return".to_string()],
        Statement::Scene { background, .. } => vec![
            format!("scene {background}"),
            format!("scene \"{background}\""),
        ],
        Statement::Imagemap { .. } => vec!["imagemap".to_string()],
        Statement::CharacterCreate { id, .. } => vec![format!("character.create(\"{id}\"")],
        Statement::ShowSprite { character_id, .. } => vec![format!("{character_id}.show")],
        Statement::HideSprite { character_id, .. } => vec![format!("{character_id}.hide")],
        Statement::MoveSprite { character_id, .. } => vec![format!("{character_id}.move")],
        Statement::SpriteAnimate { character_id, .. } => vec![format!("{character_id}.animate")],
        Statement::SpriteStopAnimation { character_id } => {
            vec![format!("{character_id}.stop_animation")]
        }
        Statement::MusicPlay { .. }
        | Statement::MusicStop { .. }
        | Statement::MusicVolume { .. } => vec!["music.".to_string()],
        Statement::SfxPlay { .. } | Statement::SfxStop { .. } => vec!["sfx.".to_string()],
        _ => vec![],
    };
    for needle in needles {
        if let Some(loc) = find_location(&source.path, &source.source, &needle) {
            return loc;
        }
    }
    Location {
        file: source.path.clone(),
        line: 1,
        column: 1,
    }
}

fn find_location(path: &Path, source: &str, needle: &str) -> Option<Location> {
    let offset = source.find(needle)?;
    let prefix = &source[..offset];
    let line = prefix.bytes().filter(|b| *b == b'\n').count() + 1;
    let column = prefix.rsplit('\n').next().unwrap_or(prefix).chars().count() + 1;
    Some(Location {
        file: path.to_path_buf(),
        line,
        column,
    })
}

fn nearest<'a>(target: &str, candidates: impl Iterator<Item = &'a String>) -> Option<String> {
    candidates
        .map(|candidate| (levenshtein(target, candidate), candidate))
        .filter(|(distance, _)| *distance <= 3)
        .min_by_key(|(distance, _)| *distance)
        .map(|(_, candidate)| candidate.clone())
}

fn levenshtein(a: &str, b: &str) -> usize {
    let mut costs: Vec<usize> = (0..=b.chars().count()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut prev = i;
        costs[0] = i + 1;
        for (j, cb) in b.chars().enumerate() {
            let old = costs[j + 1];
            costs[j + 1] = if ca == cb {
                prev
            } else {
                1 + prev.min(costs[j]).min(old)
            };
            prev = old;
        }
    }
    *costs.last().unwrap_or(&0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn fixture(script: &str) -> PathBuf {
        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("rvn_check_test_{id}"));
        fs::create_dir_all(root.join("scripts")).unwrap();
        fs::create_dir_all(root.join("assets")).unwrap();
        fs::create_dir_all(root.join("locales")).unwrap();
        fs::create_dir_all(root.join("saves")).unwrap();
        fs::write(root.join("theme.toml"), "").unwrap();
        fs::write(
            root.join("rvn.toml"),
            "[project]\nmain_script = \"scripts/main.rvn\"\nstart_label = \"start\"\n[paths]\nassets = \"assets\"\nlocales = \"locales\"\ntheme = \"theme.toml\"\nsaves = \"saves\"\n",
        )
        .unwrap();
        fs::write(root.join("scripts/main.rvn"), script).unwrap();
        root
    }

    fn kinds(report: &CheckReport) -> Vec<&'static str> {
        report.diagnostics.iter().map(|d| d.kind).collect()
    }

    #[test]
    fn reports_unreachable_labels() {
        let root =
            fixture("label start\n    jump end\nlabel dead\n    \"x\"\nlabel end\n    return\n");
        let report = check_project(root.to_str().unwrap(), CheckOptions::default());
        assert!(kinds(&report).contains(&"unreachable-label"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn reports_dead_code_after_jump() {
        let root = fixture("label start\n    jump end\n    \"dead\"\nlabel end\n    return\n");
        let report = check_project(root.to_str().unwrap(), CheckOptions::default());
        assert!(kinds(&report).contains(&"dead-code"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn reports_duplicate_choice_text() {
        let root =
            fixture("label start\nchoice { \"Open\" => { return } \"Open\" => { return } }\n");
        let report = check_project(root.to_str().unwrap(), CheckOptions::default());
        assert!(kinds(&report).contains(&"duplicate-choice-text"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn reports_unused_variables_and_characters() {
        let root = fixture("init { character.create(\"eileen\", \"Eileen\") }\nlabel start\n    set score = 1\n    return\n");
        let report = check_project(root.to_str().unwrap(), CheckOptions::default());
        assert!(kinds(&report).contains(&"unused-variable"));
        assert!(kinds(&report).contains(&"unused-character"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn strict_fails_on_warnings() {
        let root = fixture("label start\n    set score = 1\n    return\n");
        let report = check_project(root.to_str().unwrap(), CheckOptions { strict: true });
        assert!(report.failed);
        assert_eq!(report.error_count(), 0);
        assert!(report.warning_count() > 0);
        let _ = fs::remove_dir_all(root);
    }
}
