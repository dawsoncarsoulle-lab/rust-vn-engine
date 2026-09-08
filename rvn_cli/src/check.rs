#![allow(
    clippy::too_many_arguments,
    clippy::needless_borrow,
    clippy::needless_borrows_for_generic_args
)]
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use rvn_core::parse_text_tags;
use rvn_parser::{
    parse_recovering, AnimationParam, AnimationValue, Expr, Hotspot, InterpolatedText, ParseError,
    ParseErrorKind, Rect, Script, Statement, TextSegment,
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
    pub span_len: Option<usize>,
    pub source_line: Option<String>,
    pub kind: &'static str,
    pub message: String,
    pub primary_label: Option<String>,
    pub suggestion: Option<String>,
    pub code_suggestion: Option<CodeSuggestion>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeSuggestion {
    pub help: String,
    pub kind: SuggestionKind,
    pub line: usize,
    pub column: usize,
    pub span_len: usize,
    pub source_line: String,
    pub replacement: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SuggestionKind {
    Insertion,
    Replacement,
}

impl Diagnostic {
    fn error(kind: &'static str, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            file: None,
            line: None,
            column: None,
            span_len: None,
            source_line: None,
            kind,
            message: message.into(),
            primary_label: None,
            suggestion: None,
            code_suggestion: None,
            notes: Vec::new(),
        }
    }

    fn warning(kind: &'static str, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warning,
            file: None,
            line: None,
            column: None,
            span_len: None,
            source_line: None,
            kind,
            message: message.into(),
            primary_label: None,
            suggestion: None,
            code_suggestion: None,
            notes: Vec::new(),
        }
    }

    fn at(mut self, loc: Option<Location>) -> Self {
        if let Some(loc) = loc {
            self.file = Some(loc.file);
            self.line = Some(loc.line);
            self.column = Some(loc.column);
            self.source_line = loc.source_line;
            self.span_len = loc.span_len;
        }
        self
    }

    fn suggest(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }

    fn label(mut self, label: impl Into<String>) -> Self {
        self.primary_label = Some(label.into());
        self
    }

    fn code_suggestion(mut self, suggestion: CodeSuggestion) -> Self {
        self.code_suggestion = Some(suggestion);
        self
    }

    fn with_source_line(mut self, source_line: impl Into<String>, span_len: usize) -> Self {
        self.source_line = Some(source_line.into());
        self.span_len = Some(span_len.max(1));
        self
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sev = match self.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        writeln!(f, "{sev}[{}]: {}", self.kind, self.message)?;
        if let Some(file) = &self.file {
            write!(f, " --> {}", file.display())?;
            if let Some(line) = self.line {
                write!(f, ":{line}")?;
                if let Some(column) = self.column {
                    write!(f, ":{column}")?;
                }
            }
            writeln!(f)?;
        }
        if let (Some(line), Some(column), Some(source_line)) =
            (self.line, self.column, &self.source_line)
        {
            let line_no = line.to_string();
            let gutter = line_no.len();
            let pad = " ".repeat(gutter);
            let col0 = column.saturating_sub(1);
            let carets = "^".repeat(self.span_len.unwrap_or(1).max(1));
            writeln!(f, "{pad} |")?;
            writeln!(f, "{line_no} | {source_line}")?;
            write!(f, "{pad} | {}{carets}", " ".repeat(col0))?;
            if let Some(label) = &self.primary_label {
                write!(f, " {label}")?;
            }
            writeln!(f)?;
        }
        for note in &self.notes {
            writeln!(f, "  = {note}")?;
        }
        if let Some(suggestion) = &self.code_suggestion {
            render_code_suggestion(f, suggestion)?;
        }
        if let Some(suggestion) = &self.suggestion {
            writeln!(f, "  = suggestion: {suggestion}")?;
        }
        Ok(())
    }
}

fn render_code_suggestion(f: &mut fmt::Formatter<'_>, suggestion: &CodeSuggestion) -> fmt::Result {
    let corrected = corrected_source_line(suggestion);
    let line_no = suggestion.line.to_string();
    let gutter = line_no.len();
    let pad = " ".repeat(gutter);
    let col0 = suggestion.column.saturating_sub(1);
    let marker_char = match suggestion.kind {
        SuggestionKind::Insertion => '+',
        SuggestionKind::Replacement => '~',
    };
    let marker_len = match suggestion.kind {
        SuggestionKind::Insertion => suggestion.replacement.trim_end().chars().count().max(1),
        SuggestionKind::Replacement => suggestion
            .replacement
            .chars()
            .count()
            .max(suggestion.span_len)
            .max(1),
    };
    writeln!(f, "help: {}", suggestion.help)?;
    writeln!(f, "{pad} |")?;
    writeln!(f, "{line_no} | {corrected}")?;
    writeln!(
        f,
        "{pad} | {}{}",
        " ".repeat(col0),
        marker_char.to_string().repeat(marker_len)
    )?;
    Ok(())
}

fn corrected_source_line(suggestion: &CodeSuggestion) -> String {
    let start = char_to_byte_index(&suggestion.source_line, suggestion.column.saturating_sub(1));
    let end = match suggestion.kind {
        SuggestionKind::Insertion => start,
        SuggestionKind::Replacement => char_to_byte_index(
            &suggestion.source_line,
            suggestion.column.saturating_sub(1) + suggestion.span_len,
        ),
    };
    let mut corrected = String::new();
    corrected.push_str(&suggestion.source_line[..start]);
    corrected.push_str(&suggestion.replacement);
    corrected.push_str(&suggestion.source_line[end..]);
    corrected
}

fn char_to_byte_index(text: &str, char_index: usize) -> usize {
    text.char_indices()
        .nth(char_index)
        .map(|(idx, _)| idx)
        .unwrap_or(text.len())
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
    source_line: Option<String>,
    span_len: Option<usize>,
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
    cinematics: Vec<(String, Location)>,
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
                        source_line: None,
                        span_len: None,
                    })));
                return;
            }
        };

        let main_script = project_dir.join(&cfg.project.main_script);
        let scripts = match load_scripts(&main_script, &mut self.diagnostics) {
            Ok(scripts) => scripts,
            Err(e) => {
                self.diagnostics
                    .push(Diagnostic::error("script-load", e).at(Some(Location {
                        file: main_script,
                        line: 1,
                        column: 1,
                        source_line: None,
                        span_len: None,
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
        suppress_noisy_warnings(&mut self.diagnostics);
    }
}

fn load_scripts(
    main_script: &Path,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<ProjectScripts, String> {
    let mut files = Vec::new();
    let mut units = Vec::new();
    let mut loaded = HashSet::new();
    let mut stack = Vec::new();
    load_script_inner(
        main_script,
        &mut files,
        &mut units,
        &mut loaded,
        &mut stack,
        diagnostics,
    )?;
    Ok(ProjectScripts { files, units })
}

fn load_script_inner(
    path: &Path,
    files: &mut Vec<SourceFile>,
    units: &mut Vec<ScriptUnit>,
    loaded: &mut HashSet<PathBuf>,
    stack: &mut Vec<PathBuf>,
    diagnostics: &mut Vec<Diagnostic>,
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
    let recovered = match parse_recovering(&source) {
        Ok(recovered) => recovered,
        Err(error) => {
            if let Some(diag) = parser_diagnostic(&canonical, &error) {
                diagnostics.push(diag);
            }
            let file_index = files.len();
            files.push(SourceFile {
                path: canonical.clone(),
                source,
            });
            units.push(ScriptUnit {
                file_index,
                script: Vec::new(),
            });
            stack.pop();
            return Ok(());
        }
    };
    for error in &recovered.errors {
        if let Some(diag) = parser_diagnostic(&canonical, error) {
            diagnostics.push(diag);
        }
    }
    let file_index = files.len();
    files.push(SourceFile {
        path: canonical.clone(),
        source,
    });
    let base_dir = canonical.parent().unwrap_or_else(|| Path::new("."));
    let mut own = Vec::new();
    for stmt in recovered.script {
        match stmt {
            Statement::Use { paths } => {
                for use_path in paths {
                    for target in expand_use_path(base_dir, &use_path)? {
                        load_script_inner(&target, files, units, loaded, stack, diagnostics)?;
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

fn parser_diagnostic(path: &Path, error: &ParseError) -> Option<Diagnostic> {
    let loc = Location {
        file: path.to_path_buf(),
        line: error.location.line.max(1),
        column: error.location.col.max(1),
        source_line: Some(error.source_line.clone()),
        span_len: Some(error.location.len),
    };
    let diag = match &error.kind {
        ParseErrorKind::InvalidAssignment { .. } => {
            let code_suggestion = CodeSuggestion {
                help: "prefix the assignment with `set`".to_string(),
                kind: SuggestionKind::Insertion,
                line: loc.line,
                column: loc.column,
                span_len: 0,
                source_line: error.source_line.clone(),
                replacement: "set ".to_string(),
            };
            Diagnostic::error("invalid-assignment", "invalid assignment")
                .at(Some(loc.clone()))
                .with_source_line(error.source_line.clone(), error.location.len)
                .label("variables must be assigned with `set`")
                .code_suggestion(code_suggestion)
        }
        ParseErrorKind::UnexpectedToken { got, expected } => Diagnostic::error(
            "parse",
            format!("syntaxe invalide: token inattendu `{got}`, attendu : {expected}"),
        )
        .at(Some(loc))
        .with_source_line(error.source_line.clone(), error.location.len),
        ParseErrorKind::UnexpectedEof { expected } => Diagnostic::error(
            "parse-eof",
            format!("fin de fichier inattendue, attendu : {expected}"),
        )
        .at(Some(loc))
        .with_source_line(error.source_line.clone(), error.location.len),
        ParseErrorKind::LexError { slice } => {
            Diagnostic::error("lex", format!("caractère non reconnu : `{slice}`"))
                .at(Some(loc))
                .with_source_line(error.source_line.clone(), error.location.len)
        }
    };
    Some(diag)
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
                let locale_key = text_to_locale_key(text);
                validate_text_tags(&locale_key, &loc, diagnostics);
                symbols.locale_keys.insert(locale_key);
            }
            Statement::Choice { options } => {
                validate_duplicate_choice_text(options, &loc, diagnostics);
                for opt in options {
                    collect_text_vars(&opt.label, &mut symbols.used_vars, &loc);
                    let locale_key = text_to_locale_key(&opt.label);
                    validate_text_tags(&locale_key, &loc, diagnostics);
                    symbols.locale_keys.insert(locale_key);
                    collect_block(&opt.body, source, symbols, diagnostics);
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
            Statement::CinematicShow { id, transition } => {
                symbols.cinematics.push((id.clone(), loc.clone()));
                validate_cinematic_transition(transition.as_deref(), &loc, diagnostics);
            }
            Statement::CinematicHide { transition } => {
                validate_cinematic_transition(transition.as_deref(), &loc, diagnostics);
            }
            Statement::UnlockEnding { .. } => {}
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
            Statement::SfxPlay { file, .. }
            | Statement::SfxStop { file, .. }
            | Statement::VoicePlay { file, .. } => {
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
            | Statement::TypewriterSpeed { .. }
            | Statement::VoiceStop
            | Statement::SpriteEffect { .. }
            | Statement::Timer { .. }
            | Statement::TimerCancel => {}
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
                match *kind {
                    "jump" => format!("unknown jump target `{target}`"),
                    "call" => format!("unknown call target `{target}`"),
                    _ => format!("cible de {kind} introuvable"),
                },
            )
            .at(Some(loc.clone()))
            .label("unknown label target");
            if let Some(suggestion) = nearest(target, symbols.labels.keys()) {
                if let Some(code_suggestion) = replacement_code_suggestion(
                    loc,
                    target,
                    &suggestion,
                    "a similarly named label exists",
                ) {
                    diag = diag.code_suggestion(code_suggestion);
                } else {
                    diag = diag.suggest(format!("use `{suggestion}`"));
                }
            }
            diagnostics.push(diag);
        }
    }

    for (id, loc) in &symbols.used_characters {
        if !symbols.declared_characters.contains_key(id) {
            let mut diag = Diagnostic::error(
                "undefined-character",
                format!("undefined character id `{id}`"),
            )
            .at(Some(loc.clone()))
            .label("undefined character id");
            if let Some(suggestion) = nearest(id, symbols.declared_characters.keys()) {
                if let Some(code_suggestion) = replacement_code_suggestion(
                    loc,
                    id,
                    &suggestion,
                    "a similarly named character exists",
                ) {
                    diag = diag.code_suggestion(code_suggestion);
                } else {
                    diag = diag.suggest(format!("use `{suggestion}`"));
                }
            } else {
                diag = diag.suggest(format!(
                    "declare it in init with `character.create(\"{id}\", \"Display Name\")`"
                ));
            }
            diagnostics.push(diag);
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

fn suppress_noisy_warnings(diagnostics: &mut Vec<Diagnostic>) {
    let has_structural_script_error = diagnostics.iter().any(|d| {
        d.severity == Severity::Error
            && matches!(
                d.kind,
                "invalid-assignment"
                    | "parse"
                    | "parse-eof"
                    | "lex"
                    | "unknown-label-target"
                    | "duplicate-label"
                    | "missing-start-label"
            )
    });
    if !has_structural_script_error {
        return;
    }

    diagnostics.retain(|d| {
        !(d.severity == Severity::Warning
            && matches!(
                d.kind,
                "dead-code"
                    | "unreachable-label"
                    | "unreferenced-label"
                    | "missing-locale-key"
                    | "unused-locale-key"
                    | "orphan-script"
            ))
    });
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
                for opt in options {
                    collect_nested_flow_targets(&opt.body, targets);
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
                for opt in options {
                    collect_nested_flow_targets(&opt.body, targets);
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
    options: &[rvn_parser::ChoiceOption],
    loc: &Location,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut seen = HashSet::new();
    for opt in options {
        let text = text_to_locale_key(&opt.label);
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
    let cg_exts = ["png", "jpg", "jpeg", "webp"];
    for (id, loc) in &symbols.cinematics {
        let path = format!("cgs/{id}");
        if !asset_exists(&assets_dir, &path, &cg_exts) {
            diagnostics.push(
                Diagnostic::error(
                    "missing-cinematic-asset",
                    format!(
                        "Erreur: cinematic asset introuvable: '{path}' under '{}'",
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
            Some(path) if path.contains('/') => path.strip_prefix("assets/").unwrap_or(path).to_owned(),
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
    } else {
        validate_title_screen_theme_assets(project_dir, paths, diagnostics);
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
}

fn validate_title_screen_theme_assets(
    project_dir: &Path,
    paths: &crate::PathsSection,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let theme_path = project_dir.join(&paths.theme);
    let Ok(source) = fs::read_to_string(&theme_path) else {
        return;
    };
    let Ok(value) = source.parse::<toml::Value>() else {
        return;
    };
    let Some(title_screen) = value.get("title_screen").and_then(|v| v.as_table()) else {
        return;
    };
    let assets_dir = project_dir.join(&paths.assets);

    if let Some(background) = title_screen.get("background") {
        match background {
            toml::Value::String(path) => {
                push_theme_warning(
                    diagnostics,
                    &theme_path,
                    &source,
                    "legacy-title-config",
                    "title_screen.background is legacy; prefer [title_screen.background].path",
                    Some("background"),
                    Some("use `[title_screen.background] path = \"...\" mode = \"cover\"`"),
                );
                validate_title_asset(
                    diagnostics,
                    &theme_path,
                    &source,
                    &assets_dir,
                    "missing-title-background",
                    "title_screen.background",
                    path,
                    &["png", "jpg", "jpeg", "webp"],
                );
            }
            toml::Value::Table(table) => {
                if let Some(path) = table.get("path").and_then(|v| v.as_str()) {
                    validate_title_asset(
                        diagnostics,
                        &theme_path,
                        &source,
                        &assets_dir,
                        "missing-title-background",
                        "title_screen.background.path",
                        path,
                        &["png", "jpg", "jpeg", "webp"],
                    );
                }
                if let Some(mode) = table.get("mode").and_then(|v| v.as_str()) {
                    validate_one_of(
                        diagnostics,
                        &theme_path,
                        &source,
                        "invalid-title-background-mode",
                        "title_screen.background.mode",
                        mode,
                        &["cover", "contain", "stretch"],
                    );
                }
            }
            _ => {}
        }
    }

    if let Some(logo) = title_screen.get("logo") {
        match logo {
            toml::Value::String(path) => {
                push_theme_warning(
                    diagnostics,
                    &theme_path,
                    &source,
                    "legacy-title-config",
                    "title_screen.logo is legacy; prefer [title_screen.logo].path",
                    Some("logo"),
                    Some("use `[title_screen.logo] path = \"...\" anchor = \"top_center\"`"),
                );
                validate_title_asset(
                    diagnostics,
                    &theme_path,
                    &source,
                    &assets_dir,
                    "missing-title-logo",
                    "title_screen.logo",
                    path,
                    &["png", "jpg", "jpeg", "webp"],
                );
            }
            toml::Value::Table(table) => {
                if let Some(path) = table.get("path").and_then(|v| v.as_str()) {
                    validate_title_asset(
                        diagnostics,
                        &theme_path,
                        &source,
                        &assets_dir,
                        "missing-title-logo",
                        "title_screen.logo.path",
                        path,
                        &["png", "jpg", "jpeg", "webp"],
                    );
                }
                validate_anchor_field(
                    diagnostics,
                    &theme_path,
                    &source,
                    table,
                    "title_screen.logo.anchor",
                );
                validate_positive_field(
                    diagnostics,
                    &theme_path,
                    &source,
                    table,
                    "scale",
                    "title_screen.logo.scale",
                    false,
                );
            }
            _ => {}
        }
    }

    if let Some(path) = title_screen.get("music").and_then(|v| v.as_str()) {
        validate_title_asset(
            diagnostics,
            &theme_path,
            &source,
            &assets_dir,
            "missing-title-music",
            "title_screen.music",
            path,
            &["ogg", "mp3", "wav", "flac"],
        );
    }

    if title_screen.contains_key("button_x")
        || title_screen.contains_key("button_y")
        || title_screen.contains_key("button_spacing")
    {
        push_theme_warning(
            diagnostics,
            &theme_path,
            &source,
            "legacy-title-config",
            "title_screen.button_x/button_y/button_spacing are legacy; prefer [title_screen.buttons] anchor + offsets",
            Some("button_"),
            Some("use `[title_screen.buttons] anchor = \"center\" offset_x = 0.0 offset_y = 80.0 spacing = 12.0`"),
        );
    }
    for key in [
        "show_continue",
        "show_new_game",
        "show_load",
        "show_gallery",
        "show_settings",
        "show_quit",
    ] {
        if title_screen.contains_key(key) {
            push_theme_warning(
                diagnostics,
                &theme_path,
                &source,
                "legacy-title-config",
                format!("title_screen.{key} is legacy; prefer [title_screen.buttons.visibility]")
                    .as_str(),
                Some(key),
                Some("move visibility flags under `[title_screen.buttons.visibility]`"),
            );
        }
    }

    if let Some(title) = title_screen.get("title").and_then(|v| v.as_table()) {
        if title.contains_key("x") || title.contains_key("y") {
            push_theme_warning(
                diagnostics,
                &theme_path,
                &source,
                "legacy-title-config",
                "title_screen.title.x/y are legacy; prefer anchor + offset_x/offset_y",
                Some("title"),
                Some("use `anchor = \"top_center\"`, `offset_x = 0.0`, `offset_y = 96.0`"),
            );
        }
        validate_anchor_field(
            diagnostics,
            &theme_path,
            &source,
            title,
            "title_screen.title.anchor",
        );
        validate_positive_field(
            diagnostics,
            &theme_path,
            &source,
            title,
            "font_size",
            "title_screen.title.font_size",
            false,
        );
        validate_color_field(
            diagnostics,
            &theme_path,
            &source,
            title,
            "color",
            "title_screen.title.color",
        );
    }

    if let Some(buttons) = title_screen.get("buttons").and_then(|v| v.as_table()) {
        validate_anchor_field(
            diagnostics,
            &theme_path,
            &source,
            buttons,
            "title_screen.buttons.anchor",
        );
        validate_positive_field(
            diagnostics,
            &theme_path,
            &source,
            buttons,
            "width",
            "title_screen.buttons.width",
            false,
        );
        validate_positive_field(
            diagnostics,
            &theme_path,
            &source,
            buttons,
            "height",
            "title_screen.buttons.height",
            false,
        );
        validate_positive_field(
            diagnostics,
            &theme_path,
            &source,
            buttons,
            "font_size",
            "title_screen.buttons.font_size",
            false,
        );
        validate_positive_field(
            diagnostics,
            &theme_path,
            &source,
            buttons,
            "spacing",
            "title_screen.buttons.spacing",
            true,
        );
        if let Some(style) = buttons.get("style").and_then(|v| v.as_table()) {
            for key in [
                "background_color",
                "hover_color",
                "pressed_color",
                "text_color",
            ] {
                validate_color_field(
                    diagnostics,
                    &theme_path,
                    &source,
                    style,
                    key,
                    &format!("title_screen.buttons.style.{key}"),
                );
            }
        }
    }

    if let Some(order) = title_screen.get("button_order").and_then(|v| v.as_array()) {
        let mut seen = HashSet::new();
        for item in order.iter().filter_map(|v| v.as_str()) {
            if !matches!(
                item,
                "continue" | "new_game" | "load" | "gallery" | "settings" | "quit"
            ) {
                diagnostics.push(
                    Diagnostic::warning(
                        "unknown-title-button",
                        format!("unknown title_screen.button_order entry '{item}'"),
                    )
                    .at(find_location(&theme_path, &source, item).or(Some(Location {
                        file: theme_path.clone(),
                        line: 1,
                        column: 1,
                        source_line: None,
                        span_len: None,
                    })))
                    .suggest(
                        "supported entries: continue, new_game, load, gallery, settings, quit",
                    ),
                );
            } else if !seen.insert(item) {
                diagnostics.push(
                    Diagnostic::warning(
                        "duplicate-title-button",
                        format!("duplicate title_screen.button_order entry '{item}'"),
                    )
                    .at(find_location(&theme_path, &source, item).or(Some(Location {
                        file: theme_path.clone(),
                        line: 1,
                        column: 1,
                        source_line: None,
                        span_len: None,
                    })))
                    .suggest("keep each button id at most once"),
                );
            }
        }
    }
}

fn validate_title_asset(
    diagnostics: &mut Vec<Diagnostic>,
    theme_path: &Path,
    source: &str,
    assets_dir: &Path,
    kind: &'static str,
    field: &str,
    path: &str,
    extensions: &[&str],
) {
    if asset_exists(assets_dir, path, extensions) {
        return;
    }
    diagnostics.push(
        Diagnostic::warning(
            kind,
            format!(
                "{field} asset '{path}' not found under '{}'",
                assets_dir.display()
            ),
        )
        .at(find_location(theme_path, source, path).or(Some(Location {
            file: theme_path.to_path_buf(),
            line: 1,
            column: 1,
            source_line: None,
            span_len: None,
        }))),
    );
}

fn validate_anchor_field(
    diagnostics: &mut Vec<Diagnostic>,
    theme_path: &Path,
    source: &str,
    table: &toml::map::Map<String, toml::Value>,
    field: &str,
) {
    let key = field.rsplit('.').next().unwrap_or(field);
    if let Some(anchor) = table.get(key).and_then(|v| v.as_str()) {
        validate_one_of(
            diagnostics,
            theme_path,
            source,
            "invalid-title-anchor",
            field,
            anchor,
            &[
                "top_left",
                "top_center",
                "top_right",
                "center_left",
                "center",
                "center_right",
                "bottom_left",
                "bottom_center",
                "bottom_right",
            ],
        );
    }
}

fn validate_one_of(
    diagnostics: &mut Vec<Diagnostic>,
    theme_path: &Path,
    source: &str,
    kind: &'static str,
    field: &str,
    value: &str,
    allowed: &[&str],
) {
    if allowed.contains(&value) {
        return;
    }
    diagnostics.push(
        Diagnostic::warning(kind, format!("{field} has unsupported value '{value}'"))
            .at(find_theme_key_location(
                theme_path,
                source,
                field.rsplit('.').next().unwrap_or(field),
                Some(value),
            )
            .or_else(|| find_location(theme_path, source, value))
            .or(Some(Location {
                file: theme_path.to_path_buf(),
                line: 1,
                column: 1,
                source_line: None,
                span_len: None,
            })))
            .suggest(format!("supported values: {}", allowed.join(", "))),
    );
}

fn validate_positive_field(
    diagnostics: &mut Vec<Diagnostic>,
    theme_path: &Path,
    source: &str,
    table: &toml::map::Map<String, toml::Value>,
    key: &str,
    field: &str,
    allow_zero: bool,
) {
    let Some(value) = table
        .get(key)
        .and_then(|v| v.as_float().or_else(|| v.as_integer().map(|n| n as f64)))
    else {
        return;
    };
    let valid = if allow_zero {
        value >= 0.0
    } else {
        value > 0.0
    };
    if valid {
        return;
    }
    diagnostics.push(
        Diagnostic::warning(
            "invalid-title-size",
            format!(
                "{field} must be {}",
                if allow_zero { ">= 0" } else { "> 0" }
            ),
        )
        .at(
            find_theme_key_location(theme_path, source, key, Some(&format!("{value}"))).or(Some(
                Location {
                    file: theme_path.to_path_buf(),
                    line: 1,
                    column: 1,
                    source_line: None,
                    span_len: None,
                },
            )),
        ),
    );
}

fn validate_color_field(
    diagnostics: &mut Vec<Diagnostic>,
    theme_path: &Path,
    source: &str,
    table: &toml::map::Map<String, toml::Value>,
    key: &str,
    field: &str,
) {
    let Some(color) = table.get(key).and_then(|v| v.as_str()) else {
        return;
    };
    if is_valid_hex_color(color) {
        return;
    }
    diagnostics.push(
        Diagnostic::warning(
            "invalid-title-color",
            format!("{field} must be #RRGGBB or #RRGGBBAA"),
        )
        .at(
            find_theme_key_location(theme_path, source, key, Some(color)).or(Some(Location {
                file: theme_path.to_path_buf(),
                line: 1,
                column: 1,
                source_line: None,
                span_len: None,
            })),
        ),
    );
}

fn find_theme_key_location(
    path: &Path,
    source: &str,
    key: &str,
    value: Option<&str>,
) -> Option<Location> {
    for (line_idx, line) in source.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') {
            continue;
        }
        let Some((lhs, rhs)) = trimmed.split_once('=') else {
            continue;
        };
        if lhs.trim() != key {
            continue;
        }
        if let Some(value) = value {
            let rhs = rhs.trim_start();
            let numeric = value
                .chars()
                .next()
                .map(|c| c == '-' || c.is_ascii_digit())
                .unwrap_or(false);
            if !(rhs.starts_with(value) || (!numeric && rhs.contains(value))) {
                continue;
            }
        }
        let indent = line.len() - trimmed.len();
        return Some(Location {
            file: path.to_path_buf(),
            line: line_idx + 1,
            column: indent + 1,
            source_line: Some(line.to_string()),
            span_len: Some(key.chars().count().max(1)),
        });
    }
    None
}

fn is_valid_hex_color(value: &str) -> bool {
    let Some(hex) = value.strip_prefix('#') else {
        return false;
    };
    matches!(hex.len(), 6 | 8) && hex.chars().all(|c| c.is_ascii_hexdigit())
}

fn push_theme_warning(
    diagnostics: &mut Vec<Diagnostic>,
    theme_path: &Path,
    source: &str,
    kind: &'static str,
    message: &str,
    needle: Option<&str>,
    suggestion: Option<&str>,
) {
    let mut diag = Diagnostic::warning(kind, message).at(needle
        .and_then(|needle| {
            find_theme_key_location(theme_path, source, needle, None)
                .or_else(|| find_location(theme_path, source, needle))
        })
        .or(Some(Location {
            file: theme_path.to_path_buf(),
            line: 1,
            column: 1,
            source_line: None,
            span_len: None,
        })));
    if let Some(suggestion) = suggestion {
        diag = diag.suggest(suggestion);
    }
    diagnostics.push(diag);
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
                        source_line: None,
                        span_len: None,
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
                        source_line: None,
                        span_len: None,
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
                        source_line: None,
                        span_len: None,
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

fn validate_cinematic_transition(
    transition: Option<&str>,
    loc: &Location,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(transition) = transition else {
        return;
    };
    if !matches!(
        transition,
        "fade"
            | "dissolve"
            | "slideleft"
            | "slideright"
            | "slideup"
            | "slidedown"
            | "zoomin"
            | "zoomout"
            | "wipe"
            | "blur"
    ) {
        diagnostics.push(
            Diagnostic::error(
                "invalid-transition",
                format!("cinematic transition '{transition}' is not supported"),
            )
            .at(Some(loc.clone()))
            .suggest("use `with fade`, `with dissolve`, `with slideleft`, `with zoomin`, etc."),
        );
    }
}

fn collect_text_vars(text: &InterpolatedText, out: &mut Vec<(String, Location)>, loc: &Location) {
    for segment in &text.0 {
        if let TextSegment::Interp(expr) = segment {
            collect_expr_vars(expr, out, loc);
        }
    }
}

fn validate_text_tags(text: &str, loc: &Location, diagnostics: &mut Vec<Diagnostic>) {
    if let Err(e) = parse_text_tags(text) {
        diagnostics.push(Diagnostic::error("invalid-text-tag", e.message).at(Some(loc.clone())));
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
        Expr::Call { args, .. } => {
            for arg in args {
                collect_expr_vars(arg, out, loc);
            }
        }
        Expr::ListLit(items) => {
            for item in items {
                collect_expr_vars(item, out, loc);
            }
        }
        Expr::Index { target, index } => {
            collect_expr_vars(target, out, loc);
            collect_expr_vars(index, out, loc);
        }
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
        Statement::CinematicShow { id, .. } => vec![format!("cinematic \"{id}\"")],
        Statement::CinematicHide { .. } => vec!["cinematic hide".to_string()],
        Statement::UnlockEnding { id } => vec![format!("unlock_ending \"{id}\"")],
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
        Statement::SfxPlay { .. }
        | Statement::SfxStop { .. }
        | Statement::VoicePlay { .. }
        | Statement::VoiceStop => vec!["audio".to_string()],
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
        source_line: None,
        span_len: None,
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
        source_line: source
            .lines()
            .nth(line.saturating_sub(1))
            .map(str::to_string),
        span_len: Some(needle.chars().count().max(1)),
    })
}

fn replacement_code_suggestion(
    loc: &Location,
    old_text: &str,
    replacement: &str,
    help: &str,
) -> Option<CodeSuggestion> {
    let source_line = loc.source_line.clone()?;
    let column = source_line
        .find(old_text)
        .map(|byte| source_line[..byte].chars().count() + 1)
        .unwrap_or(loc.column);
    Some(CodeSuggestion {
        help: help.to_string(),
        kind: SuggestionKind::Replacement,
        line: loc.line,
        column,
        span_len: old_text.chars().count().max(1),
        source_line,
        replacement: replacement.to_string(),
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
    fn reports_invalid_text_tags() {
        let root = fixture("label start\n    \"{color=red}bad{/color}\"\n    return\n");
        let report = check_project(root.to_str().unwrap(), CheckOptions::default());
        assert!(kinds(&report).contains(&"invalid-text-tag"));
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

    #[test]
    fn recovers_after_invalid_assignment_and_reports_unknown_jump() {
        let root = fixture("label start\n    varible = 5\n    jump ixi\n    \"Bonjour.\"\n");
        let report = check_project(root.to_str().unwrap(), CheckOptions::default());
        let kinds = kinds(&report);
        assert!(kinds.contains(&"invalid-assignment"));
        assert!(kinds.contains(&"unknown-label-target"));
        let assignment = report
            .diagnostics
            .iter()
            .find(|d| d.kind == "invalid-assignment")
            .unwrap();
        assert!(assignment.suggestion.is_none());
        let rendered = assignment.to_string();
        assert!(rendered.contains("help: prefix the assignment with `set`"));
        assert!(rendered.contains("set varible = 5"));
        assert!(rendered.contains("+++"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn renders_structured_replacement_suggestions() {
        let root = fixture(
            "init { character.create(\"eileen\", \"Eileen\") }\nlabel start\n    eiileen \"Bonjour\"\n    jump strat\n",
        );
        let report = check_project(root.to_str().unwrap(), CheckOptions::default());

        let label = report
            .diagnostics
            .iter()
            .find(|d| d.kind == "unknown-label-target")
            .unwrap()
            .to_string();
        assert!(label.contains("help: a similarly named label exists"));
        assert!(label.contains("jump start"));
        assert!(label.contains("~~~~~"));

        let character = report
            .diagnostics
            .iter()
            .find(|d| d.kind == "undefined-character")
            .unwrap()
            .to_string();
        assert!(character.contains("help: a similarly named character exists"));
        assert!(character.contains("eileen \"Bonjour\""));
        assert!(character.contains("~~~~~~"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn unknown_jump_target_suppresses_flow_noise() {
        let root = fixture("label start\n    jump ixi\n    \"dead\"\nlabel later\n    \"later\"\n");
        let report = check_project(root.to_str().unwrap(), CheckOptions::default());
        let kinds = kinds(&report);
        assert!(kinds.contains(&"unknown-label-target"));
        assert!(!kinds.contains(&"dead-code"));
        assert!(!kinds.contains(&"unreachable-label"));
        assert!(!kinds.contains(&"unreferenced-label"));
        let _ = fs::remove_dir_all(root);
    }

    // ── Helpers for asset/locale fixtures ─────────────────────────────

    fn write_asset(root: &Path, rel: &str) {
        let p = root.join("assets").join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, b"").unwrap();
    }

    fn write_locale(root: &Path, name: &str, body: &str) {
        fs::write(root.join("locales").join(name), body).unwrap();
    }

    // ── Missing assets ────────────────────────────────────────────────

    #[test]
    fn reports_missing_background_asset() {
        let root = fixture("label start\n    scene \"backgrounds/forest.png\"\n    return\n");
        let report = check_project(root.to_str().unwrap(), CheckOptions::default());
        assert!(kinds(&report).contains(&"missing-asset"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn reports_missing_sprite_asset() {
        let root = fixture(
            "init { character.create(\"eileen\", \"Eileen\") }\nlabel start\n    eileen.show(\"happy\") at left\n    return\n",
        );
        let report = check_project(root.to_str().unwrap(), CheckOptions::default());
        assert!(kinds(&report).contains(&"missing-asset"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn reports_missing_cinematic_asset() {
        let root = fixture("label start\n    cinematic \"intro\"\n    return\n");
        let report = check_project(root.to_str().unwrap(), CheckOptions::default());
        assert!(kinds(&report).contains(&"missing-cinematic-asset"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn no_missing_asset_when_file_exists() {
        let root = fixture("label start\n    scene \"backgrounds/forest\"\n    return\n");
        write_asset(&root, "backgrounds/forest.png");
        let report = check_project(root.to_str().unwrap(), CheckOptions::default());
        assert!(!kinds(&report).contains(&"missing-asset"));
        let _ = fs::remove_dir_all(root);
    }

    // ── Locales ───────────────────────────────────────────────────────

    #[test]
    fn reports_missing_locale_key() {
        let root = fixture("label start\n    \"Hello world\"\n    return\n");
        write_locale(&root, "en.toml", "");
        let report = check_project(root.to_str().unwrap(), CheckOptions::default());
        assert!(kinds(&report).contains(&"missing-locale-key"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn reports_unused_locale_key() {
        let root = fixture("label start\n    \"Hello world\"\n    return\n");
        write_locale(&root, "en.toml", "[strings]\n\"Stale key\" = \"...\"\n");
        let report = check_project(root.to_str().unwrap(), CheckOptions::default());
        assert!(kinds(&report).contains(&"unused-locale-key"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn no_locale_diagnostics_when_covered() {
        let root = fixture("label start\n    \"Hello world\"\n    return\n");
        write_locale(
            &root,
            "en.toml",
            "[strings]\n\"Hello world\" = \"Bonjour\"\n",
        );
        let report = check_project(root.to_str().unwrap(), CheckOptions::default());
        assert!(!kinds(&report).contains(&"missing-locale-key"));
        assert!(!kinds(&report).contains(&"unused-locale-key"));
        let _ = fs::remove_dir_all(root);
    }

    // ── Undefined character / unassigned variable ────────────────────

    #[test]
    fn reports_undefined_character_without_suggestion() {
        // Declare a character so the parser treats unknown ids as character refs.
        // "zzzzzzz" has no nearby declared character, so no fuzzy suggestion.
        let root = fixture(
            "init { character.create(\"eileen\", \"Eileen\") }\nlabel start\n    zzzzzzz \"Boo\"\n    return\n",
        );
        let report = check_project(root.to_str().unwrap(), CheckOptions::default());
        let diag = report
            .diagnostics
            .iter()
            .find(|d| d.kind == "undefined-character")
            .unwrap();
        // No fuzzy match → the suggestion is the "declare it" hint, not "use <name>".
        let suggestion = diag.suggestion.as_deref().unwrap_or("");
        assert!(
            !suggestion.starts_with("use `"),
            "unexpected fuzzy suggestion: {suggestion}"
        );
        assert!(
            suggestion.contains("declare"),
            "expected declare hint, got: {suggestion}"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn reports_unassigned_variable() {
        let root = fixture("label start\n    set bonus = score + 1\n    return\n");
        let report = check_project(root.to_str().unwrap(), CheckOptions::default());
        assert!(kinds(&report).contains(&"unassigned-variable"));
        let _ = fs::remove_dir_all(root);
    }

    // ── Duplicate labels ──────────────────────────────────────────────

    #[test]
    fn reports_duplicate_label() {
        let root = fixture("label start\n    return\nlabel start\n    return\n");
        let report = check_project(root.to_str().unwrap(), CheckOptions::default());
        assert!(kinds(&report).contains(&"duplicate-label"));
        let _ = fs::remove_dir_all(root);
    }

    // ── Orphan scripts ────────────────────────────────────────────────

    #[test]
    fn reports_orphan_script() {
        let root = fixture("label start\n    return\n");
        fs::write(
            root.join("scripts/lonely.rvn"),
            "label lonely\n    return\n",
        )
        .unwrap();
        let report = check_project(root.to_str().unwrap(), CheckOptions::default());
        assert!(kinds(&report).contains(&"orphan-script"));
        let _ = fs::remove_dir_all(root);
    }

    // ── Empty choice / imagemap ───────────────────────────────────────

    #[test]
    fn reports_empty_choice() {
        let root = fixture("label start\n    choice { }\n    return\n");
        let report = check_project(root.to_str().unwrap(), CheckOptions::default());
        assert!(kinds(&report).contains(&"empty-choice"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn reports_empty_imagemap() {
        let root = fixture(
            "label start\n    imagemap { background: \"backgrounds/map.png\" }\n    return\n",
        );
        let report = check_project(root.to_str().unwrap(), CheckOptions::default());
        assert!(kinds(&report).contains(&"empty-imagemap"));
        let _ = fs::remove_dir_all(root);
    }

    // ── Imagemap overlap ──────────────────────────────────────────────

    #[test]
    fn reports_imagemap_overlap() {
        let script = "label start\n    imagemap {\n        background: \"backgrounds/map.png\"\n        hotspot { area: (0,0,100,100) } => { return }\n        hotspot { area: (50,50,100,100) } => { return }\n    }\n    return\n";
        let root = fixture(script);
        let report = check_project(root.to_str().unwrap(), CheckOptions::default());
        assert!(kinds(&report).contains(&"imagemap-overlap"));
        let _ = fs::remove_dir_all(root);
    }

    // ── Missing project dir / config ──────────────────────────────────

    #[test]
    fn reports_missing_project_dir() {
        let root = fixture("label start\n    return\n");
        fs::remove_dir_all(root.join("locales")).unwrap();
        let report = check_project(root.to_str().unwrap(), CheckOptions::default());
        assert!(kinds(&report).contains(&"missing-project-dir"));
        let _ = fs::remove_dir_all(root);
    }

    // ── Missing start label ───────────────────────────────────────────

    #[test]
    fn reports_missing_start_label() {
        let root = fixture("label other\n    return\n");
        let report = check_project(root.to_str().unwrap(), CheckOptions::default());
        assert!(kinds(&report).contains(&"missing-start-label"));
        let _ = fs::remove_dir_all(root);
    }

    // ── Clean project: no diagnostics ─────────────────────────────────

    #[test]
    fn clean_project_has_no_errors() {
        let root = fixture("label start\n    \"Hello world\"\n    return\n");
        write_locale(&root, "en.toml", "\"Hello world\" = \"Hello world\"\n");
        let report = check_project(root.to_str().unwrap(), CheckOptions::default());
        let errors: Vec<_> = report
            .diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .collect();
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        let _ = fs::remove_dir_all(root);
    }
}
