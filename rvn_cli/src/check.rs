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
    parse_recovering_with_context, NarrativeContext, AnimationParam, AnimationValue, Expr, Hotspot, InterpolatedText, ParseError,
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

#[derive(Debug, Default, Clone)]
struct Symbols {
    functions: HashMap<String, Location>,
    labels: HashMap<String, Location>,
    label_order: Vec<String>,
    scenes: HashSet<String>,
    duplicate_labels: Vec<(String, Location)>,
    explicit_label_refs: Vec<(String, Location, &'static str)>,
    declared_characters: HashMap<String, Location>,
    used_characters: Vec<(String, Location)>,
    backgrounds: Vec<(String, Location)>,
    cinematics: Vec<(String, Location)>,
    sprites: Vec<(String, Option<String>, Location)>,
    composed_characters: HashSet<String>,
    music_files: Vec<(String, Location)>,
    sfx_files: Vec<(String, Location)>,
    assigned_vars: HashMap<String, Location>,
    local_vars: HashSet<String>,
    lexical_body: bool,
    lexical_source_start: usize,
    used_vars: Vec<(String, Location)>,
    locale_keys: HashSet<String>,
    locale_prefixes: HashSet<String>,
    interface_bindings: Vec<(String, Location)>,
    interface_images: Vec<(String, Location)>,
    video_files: Vec<(String, Location)>,
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

        if let Err(error)=rvn_bevy::enum_runtime_loader1025::check_project(project_dir,&cfg.paths.assets){
            self.diagnostics.push(Diagnostic::error("enum-assets",error).at(Some(Location{file:project_dir.join("rvn.toml"),line:1,column:1,source_line:None,span_len:None})));
        }
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
        let complete_script = scripts
            .units
            .iter()
            .flat_map(|unit| unit.script.clone())
            .collect::<Vec<_>>();
        for error in rvn_parser::validate_logic(&complete_script, false) {
            if error.code == "duplicate-function" {
                continue;
            }
            let location = scripts.files.iter().find_map(|file| {
                find_location(&file.path, &file.source, &format!("{}(", error.name))
            });
            self.diagnostics
                .push(Diagnostic::error(error.code, error.english).at(location));
        }
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
            &symbols.locale_prefixes,
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
    let diagnostic_start = diagnostics.len();
    if let Ok(program) = rvn_parser::parse_file_program_with_uses(main_script) {
        // Reuse the runtime's canonical frozen-source/root/pin admission before
        // the checker separates imported roots into per-file lexical units.
        // These source identities are metadata, never user variable bindings.
        if let Err(error) = rvn_parser::scheduled_source::scheduled_step_sources(&program) {
            diagnostics.push(Diagnostic::error("scheduled-source", error).at(Some(Location {
                file: main_script.to_path_buf(), line: 1, column: 1,
                source_line: None, span_len: None,
            })));
        }
        let files: Vec<_> = program.sources.into_iter().map(|source| SourceFile {
            path: source.path.expect("file loader supplies actual origins"), source: source.source,
        }).collect();
        let mut units: Vec<ScriptUnit> = Vec::new();
        for (statement, origin) in program.script.into_iter().zip(program.origins) {
            let file_index = files.iter().position(|file| Some(&file.path) == origin.source.as_ref())
                .expect("resolved statement belongs to a frozen source");
            if let Some(last) = units.last_mut().filter(|unit| unit.file_index == file_index) { last.script.push(statement); }
            else { units.push(ScriptUnit { file_index, script: vec![statement] }); }
        }
        return Ok(ProjectScripts { files, units });
    }
    // Recovery still reports independent syntax errors. Discover the entire
    // imported closure first; only then decide whether root locals are legal.
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
    let context = NarrativeContext::from_script(&units.iter().flat_map(|unit| unit.script.clone()).collect());
    diagnostics.truncate(diagnostic_start);
    for unit in &mut units {
        let file = &files[unit.file_index];
        match parse_recovering_with_context(&file.source, context) {
            Ok(recovered) => {
                for error in &recovered.errors { if let Some(diag) = parser_diagnostic(&file.path,error) { diagnostics.push(diag); } }
                unit.script = recovered.script.into_iter().filter(|statement| !matches!(statement,Statement::Use{..})).collect();
            }
            Err(error) => { unit.script.clear(); if let Some(diag) = parser_diagnostic(&file.path,&error) { diagnostics.push(diag); } }
        }
    }
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
    let recovered = match parse_recovering_with_context(&source, NarrativeContext::ExplicitScenes) {
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

// Static metadata reads borrow the actual current consumer's immutable AST.
// They do not evaluate calls, flatten producer DAGs or grant runtime authority.
type InterfaceCaptures<'a> = HashMap<&'a str, &'a Expr>;

fn collect_block(
    stmts: &[Statement],
    source: &SourceFile,
    symbols: &mut Symbols,
    diagnostics: &mut Vec<Diagnostic>,
) {
    collect_block_with_captures(stmts, source, symbols, diagnostics, &InterfaceCaptures::new());
}

fn collect_block_with_captures<'a>(
    stmts: &'a [Statement],
    source: &SourceFile,
    symbols: &mut Symbols,
    diagnostics: &mut Vec<Diagnostic>,
    captures: &InterfaceCaptures<'a>,
) {
    detect_dead_code(stmts, source, diagnostics);
    for stmt in stmts {
        let previous_locals = symbols.local_vars.clone();
        let first_use = symbols.used_vars.len();
        let loc = locate_stmt_from(source, stmt, symbols.lexical_source_start);
        collect_interface_metadata_with_captures(stmt, symbols, &loc, captures);
        match stmt {
            Statement::FunctionLibrary{..}=>{
                diagnostics.push(Diagnostic::error("qualified-library-admission-required","Function Libraries require their original membership index and qualified compiler; legacy CLI checking cannot admit this scope").at(Some(loc.clone())));
            },
            Statement::LibraryUnitCall{call}=>{
                for(_,value)in call.arguments(){collect_expr_vars(value,&mut symbols.used_vars,&loc);}
                diagnostics.push(Diagnostic::error("qualified-library-admission-required","Qualified Library calls require their original membership index and compiler").at(Some(loc.clone())));
            },
            Statement::Use { .. } => {}
            Statement::Init { body } => collect_block_with_captures(body, source, symbols, diagnostics, captures),
            Statement::ScheduledConsumer { steps, body, .. } => {
                // Captures and inspections are ordered expression dependencies,
                // not variable declarations or a new lexical definition scope.
                for step in steps {
                    let value = match step {
                        rvn_parser::ConsumerStep::Capture { value, .. }
                        | rvn_parser::ConsumerStep::InspectTextTable { value, .. } => value,
                    };
                    collect_expr_vars(value, &mut symbols.used_vars, &loc);
                }
                let current: InterfaceCaptures<'_> = steps.iter().filter_map(|step| match step {
                    rvn_parser::ConsumerStep::Capture { producer, value } => Some((producer.as_str(), value)),
                    rvn_parser::ConsumerStep::InspectTextTable { .. } => None,
                }).collect();
                collect_block_with_captures(body, source, symbols, diagnostics, &current);
            }
            Statement::UnitFunction { name, parameters, body }
            | Statement::Function {
                name,
                parameters,
                body,
            }
            | Statement::Screen {
                name,
                parameters,
                body,
            }
            | Statement::Handler {
                name,
                parameters,
                body,
            } => {
                if symbols
                    .functions
                    .insert(name.clone(), loc.clone())
                    .is_some()
                    || rvn_core::eval::is_builtin(name)
                {
                    diagnostics.push(
                        Diagnostic::error(
                            "duplicate-function",
                            format!("function '{name}' is already defined"),
                        )
                        .at(Some(loc.clone())),
                    );
                }
                let mut local = Symbols::default();
                local.lexical_body=true;
                local.lexical_source_start=definition_offset(source,&loc);
                for parameter in parameters {
                    local.assigned_vars.insert(parameter.clone(), loc.clone());
                }
                local.local_vars.extend(parameters.iter().cloned());
                collect_block(body, source, &mut local, diagnostics);
                if matches!(stmt, Statement::Handler { .. } | Statement::UnitFunction { .. }) {
                    // Handler/Unit `set` updates game state, unless the binding
                    // was explicitly local (including parameters/loop items).
                    symbols.used_vars.extend(
                        local
                            .used_vars
                            .into_iter()
                            .filter(|(name, _)| !local.local_vars.contains(name)),
                    );
                    symbols.assigned_vars.extend(
                        local
                            .assigned_vars
                            .into_iter()
                            .filter(|(name, _)| !local.local_vars.contains(name)),
                    );
                } else {
                    symbols.used_vars.extend(
                        local
                            .used_vars
                            .into_iter()
                            .filter(|(name, _)| !local.assigned_vars.contains_key(name)),
                    );
                }
                symbols.backgrounds.extend(local.backgrounds);
                symbols.cinematics.extend(local.cinematics);
                symbols.sprites.extend(local.sprites);
                symbols.music_files.extend(local.music_files);
                symbols.sfx_files.extend(local.sfx_files);
                symbols.interface_bindings.extend(local.interface_bindings);
                symbols.interface_images.extend(local.interface_images);
                symbols.video_files.extend(local.video_files);
                symbols
                    .composed_characters
                    .extend(local.composed_characters);
                symbols.used_characters.extend(local.used_characters);
                symbols.locale_keys.extend(local.locale_keys);
                symbols.locale_prefixes.extend(local.locale_prefixes);
            }
            Statement::UnitCall { arguments, .. } => {
                for value in arguments { collect_expr_vars(value, &mut symbols.used_vars, &loc); }
            }
            Statement::UnitReturn => {}
            Statement::StandardMacroDispatch { dispatch } => {
                // Immutable expression dependencies are inspected, never evaluated.
                // The parser/Core still own source and instance admission.
                for value in dispatch.operand_expressions() {
                    collect_expr_vars(value, &mut symbols.used_vars, &loc);
                }
                for branch in &dispatch.branches { collect_block_with_captures(&branch.body, source, symbols, diagnostics, captures); }
            }
            Statement::FunctionReturn { value } => {
                collect_expr_vars(value, &mut symbols.used_vars, &loc)
            }
            Statement::MenuExecute { request } => {
                collect_expr_vars(request, &mut symbols.used_vars, &loc)
            }
            Statement::UiOpen {
                name,
                arguments,
                modal,
                layer,
                ..
            } => {
                for value in [name, arguments, modal, layer] {
                    collect_expr_vars(value, &mut symbols.used_vars, &loc);
                }
            }
            Statement::WidgetAddToViewport { object, z_order } => {
                collect_expr_vars(object, &mut symbols.used_vars, &loc);
                collect_expr_vars(z_order, &mut symbols.used_vars, &loc);
            }
            Statement::WidgetRemoveFromParent { object } => collect_expr_vars(object, &mut symbols.used_vars, &loc),
            Statement::UiClose { name } => collect_expr_vars(name, &mut symbols.used_vars, &loc),
            Statement::UiFocus { name, element } => {
                collect_expr_vars(name, &mut symbols.used_vars, &loc);
                collect_expr_vars(element, &mut symbols.used_vars, &loc);
            }
            Statement::UiSetState {
                name,
                element,
                state,
            } => {
                for value in [name, element, state] {
                    collect_expr_vars(value, &mut symbols.used_vars, &loc);
                }
            }
            Statement::MotionPlay { target, definition } => {
                collect_expr_vars(target, &mut symbols.used_vars, &loc);
                collect_expr_vars(definition, &mut symbols.used_vars, &loc);
            }
            Statement::CharacterCompose {
                character,
                definition,
            } => {
                collect_expr_vars(character, &mut symbols.used_vars, &loc);
                collect_expr_vars(definition, &mut symbols.used_vars, &loc);
                if let Expr::Str(id) = character {
                    symbols.used_characters.push((id.clone(), loc.clone()));
                    symbols.composed_characters.insert(id.clone());
                }
            }
            Statement::CharacterAttributes {
                character,
                attributes,
            } => {
                collect_expr_vars(character, &mut symbols.used_vars, &loc);
                collect_expr_vars(attributes, &mut symbols.used_vars, &loc);
                if let Expr::Str(id) = character {
                    symbols.used_characters.push((id.clone(), loc.clone()));
                }
            }
            Statement::MotionStop { target } | Statement::MotionWait { target } => {
                collect_expr_vars(target, &mut symbols.used_vars, &loc)
            }
            Statement::VideoPlay { name, definition } => {
                collect_expr_vars(name, &mut symbols.used_vars, &loc);
                collect_expr_vars(definition, &mut symbols.used_vars, &loc);
            }
            Statement::VideoSeek { name, seconds } => {
                collect_expr_vars(name, &mut symbols.used_vars, &loc);
                collect_expr_vars(seconds, &mut symbols.used_vars, &loc);
            }
            Statement::AccessibilityConfigure { settings } => {
                collect_expr_vars(settings, &mut symbols.used_vars, &loc)
            }
            Statement::AccessibilitySpeak { text } => {
                collect_expr_vars(text, &mut symbols.used_vars, &loc)
            }
            Statement::AccessibilityStop => {}
            Statement::VideoVolume { name, volume } => {
                collect_expr_vars(name, &mut symbols.used_vars, &loc);
                collect_expr_vars(volume, &mut symbols.used_vars, &loc);
            }
            Statement::VideoPause { name }
            | Statement::VideoResume { name }
            | Statement::VideoStop { name }
            | Statement::VideoSkip { name }
            | Statement::VideoWait { name } => {
                collect_expr_vars(name, &mut symbols.used_vars, &loc)
            }
            Statement::Sequence{branches}=>for b in branches{collect_block_with_captures(b,source,symbols,diagnostics, captures);},
            Statement::ForLoopWithBreak{name,first,last,body}=>{
                collect_expr_vars(first,&mut symbols.used_vars,&loc);collect_expr_vars(last,&mut symbols.used_vars,&loc);
                if symbols.lexical_body{symbols.local_vars.insert(name.clone());}
                if !symbols.local_vars.contains(name){symbols.assigned_vars.entry(name.clone()).or_insert(loc.clone());}
                collect_block_with_captures(body,source,symbols,diagnostics, captures);
            }
            Statement::While { condition, body } => {
                collect_expr_vars(condition, &mut symbols.used_vars, &loc);
                collect_block_with_captures(body, source, symbols, diagnostics, captures);
            }
            Statement::ForEach {
                name,
                collection,
                body,
            } => {
                collect_expr_vars(collection, &mut symbols.used_vars, &loc);
                if symbols.lexical_body {symbols.local_vars.insert(name.clone());}
                if !symbols.local_vars.contains(name) {symbols.assigned_vars.entry(name.clone()).or_insert(loc.clone());}
                collect_block_with_captures(body, source, symbols, diagnostics, captures);
            }
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
                    collect_block_with_captures(&opt.body, source, symbols, diagnostics, captures);
                }
            }
            Statement::SetVar { name, value } => {
                collect_expr_vars(value, &mut symbols.used_vars, &loc);
                if !symbols.local_vars.contains(name) { symbols
                    .assigned_vars
                    .entry(name.clone())
                    .or_insert(loc.clone()); }
            }
            Statement::LocalVar { name, value } => {
                collect_expr_vars(value, &mut symbols.used_vars, &loc);
                symbols.local_vars.insert(name.clone());
            }
            Statement::If {
                condition,
                then_branch,
                else_branch,
            } => {
                collect_expr_vars(condition, &mut symbols.used_vars, &loc);
                collect_block_with_captures(then_branch, source, symbols, diagnostics, captures);
                collect_block_with_captures(else_branch, source, symbols, diagnostics, captures);
            }
            Statement::Label { name } | Statement::SceneLabel { name } => {
                if matches!(stmt,Statement::SceneLabel { .. }) { symbols.local_vars.clear(); symbols.scenes.insert(name.clone()); }
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
            Statement::Timer { action, .. } => {
                if let Some((kind,target))=timer_target(action) {
                    symbols.explicit_label_refs.push((target.to_string(),loc.clone(),kind));
                }
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
            Statement::MusicVolumeValue { level } => {
                collect_expr_vars(level, &mut symbols.used_vars, &loc);
            }
            Statement::TypewriterSpeedValue { chars_per_sec } => {
                collect_expr_vars(chars_per_sec, &mut symbols.used_vars, &loc);
            }
            Statement::TypewriterSetValue { enabled } => {
                collect_expr_vars(enabled, &mut symbols.used_vars, &loc);
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
                    collect_block_with_captures(&hotspot.body, source, symbols, diagnostics, captures);
                }
            }
            Statement::Config { .. }
            | Statement::LoopBreak
            | Statement::Return
            | Statement::MusicStop { .. }
            | Statement::MusicVolume { .. }
            | Statement::TypewriterSet { .. }
            | Statement::TypewriterSpeed { .. }
            | Statement::VoiceStop
            | Statement::SpriteEffect { .. }
            | Statement::TimerCancel => {}
        }
        // Definition bodies have already resolved their own lexical bindings.
        // An outer scene local cannot satisfy a global read in such a body.
        if !matches!(stmt, Statement::Function { .. } | Statement::UnitFunction { .. } | Statement::Screen { .. } | Statement::Handler { .. }) {
            let new_uses: Vec<_> = symbols.used_vars.drain(first_use..).collect();
            symbols.used_vars.extend(new_uses.into_iter().filter(|(name,_)| !previous_locals.contains(name)));
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
                    _ => format!("cible de {kind} `{target}` introuvable"),
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

    let used_vars: HashSet<&str> = symbols
        .used_vars
        .iter()
        .chain(&symbols.interface_bindings)
        .map(|(v, _)| v.as_str())
        .collect();
    for (var, loc) in symbols.used_vars.iter().chain(&symbols.interface_bindings) {
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
    // Units retain resolved source order; an import can split one label's
    // statement stream without starting a new control-flow block.
    let mut current: Option<String> = None;
    for unit in &scripts.units {
        for stmt in &unit.script {
            if let Statement::Label { name } | Statement::SceneLabel { name } = stmt {
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
            Statement::ScheduledConsumer { body, .. } => {
                let body: Vec<_> = body.iter().collect();
                collect_flow_targets(&body, targets, terminates);
                if *terminates { return; }
            }
            Statement::StandardMacroDispatch { dispatch } => {
                for branch in &dispatch.branches { collect_nested_flow_targets(&branch.body, targets); }
            }
            Statement::Sequence { branches } => {
                for branch in branches { collect_nested_flow_targets(branch, targets); }
            }
            Statement::While { body, .. } | Statement::ForEach { body, .. }
                | Statement::ForLoopWithBreak { body, .. } => collect_nested_flow_targets(body, targets),
            Statement::Jump { target } => {
                targets.insert(target.clone());
                *terminates = true;
                return;
            }
            Statement::Call { target } => {
                targets.insert(target.clone());
            }
            Statement::Timer { action, .. } => {
                if let Some((_,target))=timer_target(action) {targets.insert(target.to_string());}
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
            Statement::ScheduledConsumer { body, .. } => {
                collect_nested_flow_targets(body, targets);
            }
            Statement::StandardMacroDispatch { dispatch } => {
                for branch in &dispatch.branches { collect_nested_flow_targets(&branch.body, targets); }
            }
            Statement::Sequence { branches } => {
                for branch in branches { collect_nested_flow_targets(branch, targets); }
            }
            Statement::While { body, .. } | Statement::ForEach { body, .. }
                | Statement::ForLoopWithBreak { body, .. } => collect_nested_flow_targets(body, targets),
            Statement::Jump { target } | Statement::Call { target } => {
                targets.insert(target.clone());
            }
            Statement::Timer { action, .. } => {
                if let Some((_,target))=timer_target(action) {targets.insert(target.to_string());}
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
fn timer_target(action:&str)->Option<(&'static str,&str)> {
    let (kind,target)=action.trim().split_once(' ')?;
    let kind=match kind {"jump"=>"timer jump","call"=>"timer call",_=>return None};
    let target=target.trim();
    (!target.is_empty()).then_some((kind,target))
}

fn detect_dead_code(stmts: &[Statement], source: &SourceFile, diagnostics: &mut Vec<Diagnostic>) {
    let mut terminated_by: Option<&'static str> = None;
    for stmt in stmts {
        if matches!(stmt, Statement::Label { .. } | Statement::SceneLabel { .. }) {
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
        let executed = if let Statement::ScheduledConsumer { body, .. } = stmt {
            body.first().unwrap_or(stmt)
        } else { stmt };
        match executed {
            Statement::Jump { .. } => terminated_by = Some("jump"),
            Statement::Return | Statement::UnitReturn | Statement::FunctionReturn { .. } => terminated_by = Some("return"),
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
    for (path, loc) in &symbols.video_files {
        let result = (|| -> Result<(), String> {
            use std::io::Read;
            if !rvn_ui::programmable::safe_asset_path(path)
                || !path.to_ascii_lowercase().ends_with(".webm")
            {
                return Err("expected a relative .webm resource inside the asset directory".into());
            }
            let root = assets_dir
                .canonicalize()
                .map_err(|error| error.to_string())?;
            let file = assets_dir
                .join(path)
                .canonicalize()
                .map_err(|error| error.to_string())?;
            if !file.starts_with(&root) {
                return Err("resource resolves outside the asset directory".into());
            }
            let metadata = std::fs::metadata(&file).map_err(|error| error.to_string())?;
            if !metadata.is_file() || metadata.len() > 1_073_741_824 {
                return Err("expected a regular video file of at most 1 GiB".into());
            }
            let mut bytes = Vec::new();
            std::fs::File::open(file)
                .map_err(|error| error.to_string())?
                .take(rvn_ui::webm::HEADER_LIMIT as u64)
                .read_to_end(&mut bytes)
                .map_err(|error| error.to_string())?;
            rvn_ui::webm::validate_header(&bytes, false)
        })();
        if let Err(error) = result {
            diagnostics.push(
                Diagnostic::error("invalid-video-resource", format!("video '{path}': {error}"))
                    .at(Some(loc.clone())),
            );
        }
    }
    for (image, loc) in &symbols.interface_images {
        if !rvn_ui::programmable::safe_asset_path(image) {
            diagnostics.push(Diagnostic::error("invalid-interface-image",format!("interface image must be a relative resource inside the asset directory: '{image}'")).at(Some(loc.clone())));
        } else if !assets_dir.join(image).is_file() {
            diagnostics.push(
                Diagnostic::error(
                    "missing-interface-image",
                    format!(
                        "interface image '{image}' not found under '{}'",
                        assets_dir.display()
                    ),
                )
                .at(Some(loc.clone())),
            );
        }
    }
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
        // A composition supplies its own ordered resources, checked above.
        // Do not demand an unused conventional sprites/<id>/default image.
        if emotion.is_none() && symbols.composed_characters.contains(character_id) {
            continue;
        }
        let sprite_path = match emotion {
            Some(path) if path.contains('/') => {
                path.strip_prefix("assets/").unwrap_or(path).to_owned()
            }
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
    prefixes: &HashSet<String>,
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
            if !used_keys.contains(&key) && !prefixes.iter().any(|prefix| key.starts_with(prefix)) {
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
    collect_rvn_files(&scripts_dir, project_dir, &mut all);
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

fn collect_rvn_files(dir: &Path, project_dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        // Exported games and recovery snapshots are not project source. Do
        // not follow directory symlinks into other projects or recursive loops.
        if kind.is_dir() {
            let name = entry.file_name();
            if name == ".git"
                || name == ".rvn-backups"
                || (dir == project_dir && (name == "dist" || name == "dist-web"))
            {
                continue;
            }
            collect_rvn_files(&path, project_dir, out);
        } else if kind.is_file() && path.extension().and_then(|s| s.to_str()) == Some("rvn") {
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

fn collect_interface_metadata(statement: &Statement, symbols: &mut Symbols, loc: &Location) {
    collect_interface_metadata_with_captures(statement, symbols, loc, &InterfaceCaptures::new());
}

// Resolve only a chain of captured references to its borrowed source shape.
// Unresolved, malformed and cyclic chains remain unknown; arbitrary calls are
// never executed, and producer subtrees are never cloned or expanded.
fn interface_metadata_value<'a>(mut value: &'a Expr, captures: &InterfaceCaptures<'a>) -> Option<&'a Expr> {
    for _ in 0..=captures.len() {
        match value {
            Expr::Call { name, args } if name == "__rvn_captured_v1" => {
                let [Expr::Str(id)] = args.as_slice() else { return None; };
                value = *captures.get(id.as_str())?;
            }
            _ => return Some(value),
        }
    }
    None
}

fn collect_interface_metadata_with_captures<'a>(
    statement: &'a Statement, symbols: &mut Symbols, loc: &Location, captures: &InterfaceCaptures<'a>,
) {
    fn key<'a>(value: &'a Expr, symbols: &mut Symbols, captures: &InterfaceCaptures<'a>) {
        let Some(value) = interface_metadata_value(value, captures) else { symbols.locale_prefixes.insert(String::new()); return; };
        fn prefix<'a>(mut value: &'a Expr, captures: &InterfaceCaptures<'a>) -> String {
            let mut seen = HashSet::new();
            loop {
                let Some(resolved) = interface_metadata_value(value, captures) else { return String::new(); };
                // Cycle guard over borrowed syntax only, never a source identity.
                if !seen.insert(resolved as *const Expr) { return String::new(); }
                match resolved {
                    Expr::Str(text) => return text.clone(),
                    Expr::BinOp { op: rvn_parser::BinOpKind::Add, left, .. } => value = left,
                    _ => return String::new(),
                }
            }
        }
        if let Expr::Str(text) = value {
            symbols.locale_keys.insert(text.clone());
        } else {
            symbols.locale_prefixes.insert(prefix(value, captures));
        }
    }
    fn properties<'a>(value: &'a Expr, symbols: &mut Symbols, loc: &Location, captures: &InterfaceCaptures<'a>) {
        let Some(value) = interface_metadata_value(value, captures) else { symbols.locale_prefixes.insert(String::new()); return; };
        let Expr::Call { name, args } = value else {
            symbols.locale_prefixes.insert(String::new());
            return;
        };
        if name != "dict" {
            symbols.locale_prefixes.insert(String::new());
            return;
        }
        for pair in args.chunks_exact(2) {
            let Some(Expr::Str(name)) = interface_metadata_value(&pair[0], captures) else {
                continue;
            };
            let value = interface_metadata_value(&pair[1], captures).unwrap_or(&pair[1]);
            match (name.as_str(), value) {
                ("binding", Expr::Str(name)) => {
                    symbols.interface_bindings.push((name.clone(), loc.clone()))
                }
                ("image", Expr::Str(path)) => {
                    symbols.interface_images.push((path.clone(), loc.clone()))
                }
                ("text_key" | "placeholder_key" | "accessible_label_key", value) => {
                    key(value, symbols, captures)
                }
                ("option_keys", Expr::ListLit(values)) => {
                    for value in values {
                        key(value, symbols, captures);
                    }
                }
                ("option_keys", _) => {
                    symbols.locale_prefixes.insert(String::new());
                }
                ("text" | "placeholder" | "accessible_label", Expr::Str(text)) => {
                    if let Ok(text) = rvn_parser::parse_interpolated_str(text) {
                        collect_text_vars(&text, &mut symbols.interface_bindings, loc);
                    }
                }
                _ => {}
            }
        }
    }
    fn walk<'a>(value: &'a Expr, symbols: &mut Symbols, loc: &Location, captures: &InterfaceCaptures<'a>) {
        match value {
            Expr::LibraryCall{call}=>for(_,value)in call.arguments(){walk(value,symbols,loc,captures);},
            Expr::Call { name, args } => {
                if name == "component" && args.len() == 4 {
                    properties(&args[2], symbols, loc, captures);
                }
                if name == "canvas_image" {
                    if let Some(Expr::Str(path)) = args.first().and_then(|value| interface_metadata_value(value, captures)) {
                        symbols.interface_images.push((path.clone(), loc.clone()));
                    }
                }
                if name == "motion_frames" {
                    if let Some(Expr::ListLit(images)) = args.first().and_then(|value| interface_metadata_value(value, captures)) {
                        for image in images {
                            if let Some(Expr::Str(path)) = interface_metadata_value(image, captures) {
                                symbols.interface_images.push((path.clone(), loc.clone()));
                            }
                        }
                    }
                }
                if name == "image_layer" {
                    if let Some(Expr::Str(path)) = args.get(1).and_then(|value| interface_metadata_value(value, captures)) {
                        symbols.interface_images.push((path.clone(), loc.clone()));
                    }
                }
                if name == "image_layers" {
                    if let Some(Expr::ListLit(images)) = args.get(1).and_then(|value| interface_metadata_value(value, captures)) {
                        let marker = match args.first().and_then(|value| interface_metadata_value(value, captures)) {
                            Some(Expr::Str(prefix)) => Some(format!("{prefix}__")),
                            _ => None,
                        };
                        for image in images {
                            if let Some(Expr::Str(path)) = interface_metadata_value(image, captures) {
                                if marker.as_ref().is_none_or(|marker| {
                                    path.rsplit('/')
                                        .next()
                                        .is_some_and(|name| name.starts_with(marker))
                                }) {
                                    symbols.interface_images.push((path.clone(), loc.clone()));
                                }
                            }
                        }
                    }
                }
                if name == "video_clip" {
                    if let Some(Expr::Str(path)) = args.first().and_then(|value| interface_metadata_value(value, captures)) {
                        symbols.video_files.push((path.clone(), loc.clone()));
                    }
                    if let Some(Expr::Call { name, args }) = args.get(1).and_then(|value| interface_metadata_value(value, captures)) {
                        if name == "dict" {
                            for pair in args.chunks_exact(2) {
                                if let Some(Expr::Str(property)) = interface_metadata_value(&pair[0], captures) {
                                    let value = interface_metadata_value(&pair[1], captures).unwrap_or(&pair[1]);
                                    match (property.as_str(), value) {
                                        ("poster" | "fallback", Expr::Str(path)) => symbols
                                            .interface_images
                                            .push((path.clone(), loc.clone())),
                                        ("mask", Expr::Str(path)) => {
                                            symbols.video_files.push((path.clone(), loc.clone()))
                                        }
                                        ("subtitles", Expr::ListLit(cues)) => {
                                            for cue in cues {
                                                if let Some(Expr::Call { name, args }) = interface_metadata_value(cue, captures) {
                                                    if name == "dict" {
                                                        for field in args.chunks_exact(2) {
                                                            if matches!(interface_metadata_value(&field[0], captures),Some(Expr::Str(name)) if name=="text")
                                                            {
                                                                key(&field[1], symbols, captures);
                                                            }
                                                        }
                                                    }
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
                for value in args {
                    walk(value, symbols, loc, captures);
                }
            }
            Expr::BinOp { left, right, .. } | Expr::And(left, right) | Expr::Or(left, right) => {
                walk(left, symbols, loc, captures);
                walk(right, symbols, loc, captures);
            }
            Expr::Index { target, index } => {
                walk(target, symbols, loc, captures);
                walk(index, symbols, loc, captures);
            }
            Expr::Neg(value) | Expr::Not(value) => walk(value, symbols, loc, captures),
            Expr::StandardMacroData { .. } => {} // A live cell identity has no nested expression/variable metadata.
            Expr::ListLit(values) => {
                for value in values {
                    walk(value, symbols, loc, captures);
                }
            }
            _ => {}
        }
    }
    match statement {
        Statement::ScheduledConsumer { steps, .. } => {
            let mut active = InterfaceCaptures::new();
            for step in steps {
                let (rvn_parser::ConsumerStep::Capture { value, .. }
                    | rvn_parser::ConsumerStep::InspectTextTable { value, .. }) = step;
                walk(value, symbols, loc, &active);
                if let rvn_parser::ConsumerStep::Capture { producer, value } = step { active.insert(producer.as_str(), value); }
            }
        }
        Statement::StandardMacroDispatch { dispatch } => {
            let mut active = InterfaceCaptures::new();
            for step in dispatch.operand_steps() {
                let (rvn_parser::ConsumerStep::Capture { value, .. }
                    | rvn_parser::ConsumerStep::InspectTextTable { value, .. }) = step;
                walk(value, symbols, loc, &active);
                if let rvn_parser::ConsumerStep::Capture { producer, value } = step { active.insert(producer.as_str(), value); }
            }
            if let Some(value) = &dispatch.operand { walk(value, symbols, loc, &active); }
        }
        Statement::UnitCall { arguments, .. } => {
            for value in arguments { walk(value, symbols, loc, captures); }
        }
        Statement::FunctionReturn { value }
        | Statement::SetVar { value, .. }
        | Statement::LocalVar { value, .. } => walk(value, symbols, loc, captures),
        Statement::MenuExecute { request } => walk(request, symbols, loc, captures),
        Statement::UiOpen {
            name,
            arguments,
            modal,
            layer,
                ..
        } => {
            for value in [name, arguments, modal, layer] {
                walk(value, symbols, loc, captures);
            }
        }
        Statement::WidgetAddToViewport { object, z_order } => { walk(object, symbols, loc, captures); walk(z_order, symbols, loc, captures); }
        Statement::WidgetRemoveFromParent { object } => walk(object, symbols, loc, captures),
        Statement::UiClose { name } => walk(name, symbols, loc, captures),
        Statement::UiFocus { name, element } => {
            walk(name, symbols, loc, captures);
            walk(element, symbols, loc, captures);
        }
        Statement::UiSetState {
            name,
            element,
            state,
        } => {
            for value in [name, element, state] {
                walk(value, symbols, loc, captures);
            }
        }
        Statement::MotionPlay { target, definition } => {
            walk(target, symbols, loc, captures);
            walk(definition, symbols, loc, captures);
        }
        Statement::CharacterCompose {
            character,
            definition,
        } => {
            walk(character, symbols, loc, captures);
            walk(definition, symbols, loc, captures);
        }
        Statement::CharacterAttributes {
            character,
            attributes,
        } => {
            walk(character, symbols, loc, captures);
            walk(attributes, symbols, loc, captures);
        }
        Statement::MotionStop { target } | Statement::MotionWait { target } => {
            walk(target, symbols, loc, captures)
        }
        Statement::VideoPlay { name, definition } => {
            walk(name, symbols, loc, captures);
            walk(definition, symbols, loc, captures);
        }
        Statement::AccessibilityConfigure { settings } => walk(settings, symbols, loc, captures),
        Statement::AccessibilitySpeak { text } => walk(text, symbols, loc, captures),
        Statement::If { condition, .. } | Statement::While { condition, .. } => {
            walk(condition, symbols, loc, captures)
        }
        Statement::ForLoopWithBreak{first,last,..}=>{walk(first,symbols,loc,captures);walk(last,symbols,loc,captures);},
        Statement::ForEach { collection, .. } => walk(collection, symbols, loc, captures),
        _ => {}
    }
}

fn validate_text_tags(text: &str, loc: &Location, diagnostics: &mut Vec<Diagnostic>) {
    if let Err(e) = parse_text_tags(text) {
        diagnostics.push(Diagnostic::error("invalid-text-tag", e.message).at(Some(loc.clone())));
    }
}

fn collect_expr_vars(expr: &Expr, out: &mut Vec<(String, Location)>, loc: &Location) {
    match expr {
        Expr::LibraryCall{call}=>for(_,value)in call.arguments(){collect_expr_vars(value,out,loc);},
        Expr::Var(name) => out.push((name.clone(), loc.clone())),
        Expr::BinOp { left, right, .. } | Expr::And(left, right) | Expr::Or(left, right) => {
            collect_expr_vars(left, out, loc);
            collect_expr_vars(right, out, loc);
        }
        Expr::Neg(inner) | Expr::Not(inner) => collect_expr_vars(inner, out, loc),
        Expr::Int(_) | Expr::Float(_) | Expr::Bool(_) | Expr::Str(_)
            | Expr::StandardMacroData { .. } => {}
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
        Expr::LibraryCall{call}=>format!("functions.value_v1({:?}, {:?}, {{{}}})",call.reference().library.as_str(),call.reference().graph_id.to_string(),call.arguments().iter().map(|(id,value)|format!("{:?}:{}",id.as_str(),expr_to_display(value))).collect::<Vec<_>>().join(",")),
        Expr::StandardMacroData { site, output, .. } => {
            use rvn_parser::scheduled_source::standard_macro_contract::DataOutput;
            let method = match output { DataOutput::Counter => "flow.macro_counter_v1", DataOutput::IsA => "flow.macro_is_a_v1" };
            // Site's canonical source identifiers are bounded ASCII. Rust's
            // quoted string spelling preserves those exact parser strings.
            format!("{method}({:?}, {:?})", site.scope_source(), site.node_source())
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

fn definition_offset(source: &SourceFile, location: &Location) -> usize {
    let line_start=source.source.split_inclusive('\n')
        .take(location.line.saturating_sub(1)).map(str::len).sum::<usize>();
    let line=source.source[line_start..].split('\n').next().unwrap_or("");
    let column_offset=line.char_indices().nth(location.column.saturating_sub(1))
        .map(|(offset,_)|offset).unwrap_or(line.len());
    line_start+column_offset
}

fn locate_stmt(source: &SourceFile, stmt: &Statement) -> Location {
    locate_stmt_from(source,stmt,0)
}

fn locate_stmt_from(source: &SourceFile, stmt: &Statement, start: usize) -> Location {
    if let Statement::MenuExecute { request } = stmt {
        // Distinguish separate request handlers before falling back to the
        // command token, whose spelling alone can occur elsewhere in the file.
        if let Some(mut location)=find_location_from(&source.path,&source.source,&format!("menu.execute({}",expr_to_display(request)),start) {
            location.span_len=Some("menu.execute(".len());
            return location;
        }
    }
    let needles: Vec<String> = match stmt {
        Statement::ScheduledConsumer { version, .. } => vec![format!("flow.consume_v{version}(")],
        Statement::Function { name, .. } | Statement::UnitFunction { name, .. } => vec![format!("function {name}")],
        Statement::UnitCall { name, .. } => vec![format!("call {name}(")],
        Statement::UnitReturn => vec!["return".into()],
        Statement::StandardMacroDispatch { .. } => vec!["flow.macro_dispatch_v1(".into()],
        Statement::Screen { name, .. } => vec![format!("screen {name}")],
        Statement::Handler { name, .. } => vec![format!("handler {name}")],
        Statement::LocalVar { name, .. } => vec![format!("local {name}")],
        Statement::MenuExecute { .. } => vec!["menu.execute(".into()],
        Statement::UiOpen { story, .. } => vec![if *story { "ui.open_story(" } else { "ui.open(" }.into()],
        Statement::MotionPlay { .. } => vec!["motion.play(".into()],
        Statement::MotionStop { .. } => vec!["motion.stop(".into()],
        Statement::MotionWait { .. } => vec!["motion.wait(".into()],
        Statement::WidgetAddToViewport { .. } => vec!["widget.add_to_viewport(".into()],
        Statement::WidgetRemoveFromParent { .. } => vec!["widget.remove_from_parent(".into()],
        Statement::UiClose { .. } => vec!["ui.close(".into()],
        Statement::UiFocus { .. } => vec!["ui.focus(".into()],
        Statement::UiSetState { .. } => vec!["ui.set_state(".into()],
        Statement::FunctionReturn { .. } => vec!["return ".into()],
        Statement::Sequence{..}=>vec!["flow.sequence(".into()],
        Statement::ForLoopWithBreak{..}=>vec!["flow.for_loop(".into()],
        Statement::LoopBreak=>vec!["flow.break(".into()],
        Statement::While { .. } => vec!["while ".into()],
        Statement::ForEach { name, .. } => vec![format!("for {name}")],
        Statement::Label { name } => vec![format!("label {name}")],
        Statement::SceneLabel { name } => vec![format!("scene label {name}")],
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
        Statement::TypewriterSpeed { .. } | Statement::TypewriterSpeedValue { .. }
        | Statement::TypewriterSet { .. } | Statement::TypewriterSetValue { .. }
        | Statement::TypewriterSpeed { .. } => vec!["typewriter".to_string()],
        Statement::MusicPlay { .. }
        | Statement::MusicStop { .. }
        | Statement::MusicVolume { .. }
        | Statement::MusicVolumeValue { .. } => vec!["music.".to_string()],
        Statement::SfxPlay { .. }
        | Statement::SfxStop { .. }
        | Statement::VoicePlay { .. }
        | Statement::VoiceStop => vec!["audio".to_string()],
        _ => vec![],
    };
    for needle in needles {
        if let Some(loc) = find_location_from(&source.path, &source.source, &needle, start) {
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
    find_location_from(path,source,needle,0)
}

fn find_location_from(path: &Path, source: &str, needle: &str, start: usize) -> Option<Location> {
    let offset = start.checked_add(source.get(start..)?.find(needle)?)?;
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
    fn viewport1003_checker_collects_actual_object_and_order_variables_at_each_method_line() {
        // Syntax/diagnostics only: no Widget source/class/runtime admission.
        let root=fixture("scene label start\nlocal held=0\nwidget.add_to_viewport(held,missing_z)\nwidget.remove_from_parent(missing_object)\nreturn\n");
        let report=check_project(root.to_str().unwrap(),CheckOptions::default());
        let unknown:Vec<_>=report.diagnostics.iter().filter(|d|d.kind=="unassigned-variable").collect();
        assert_eq!(unknown.len(),2,"{report:?}");
        for(name,line)in [("missing_z",3),("missing_object",4)] {
            let diagnostic=unknown.iter().find(|d|d.message.contains(name)).unwrap();
            assert_eq!(diagnostic.line,Some(line));assert_eq!(diagnostic.column,Some(1));
        }
        assert!(unknown.iter().all(|d|!d.message.contains("'held'")&&!d.message.contains("'widget'")),"{report:?}");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn viewport1003_checker_keeps_nested_interface_metadata_without_granting_object_admission() {
        // Deliberately unadmitted object expression: syntax tooling must still
        // report nested metadata; collecting it does not create a WidgetObject.
        let source=SourceFile{path:"viewport-metadata.rvn".into(),source:"widget.remove_from_parent(component(\"probe\",\"text\",{\"binding\":\"needle\",\"image\":\"missing.png\"},[]))\n".into()};
        let script=rvn_parser::parse(&source.source).unwrap();let loc=locate_stmt(&source,&script[0]);let mut symbols=Symbols::default();
        collect_interface_metadata(&script[0],&mut symbols,&loc);
        assert_eq!(symbols.interface_bindings.iter().map(|(name,_)|name.as_str()).collect::<Vec<_>>(),vec!["needle"]);
        assert_eq!(symbols.interface_images.iter().map(|(path,_)|path.as_str()).collect::<Vec<_>>(),vec!["missing.png"]);
        assert!(symbols.interface_bindings.iter().chain(&symbols.interface_images).all(|(_,loc)|loc.line==1&&loc.column==1));
    }
    #[test]
    fn scheduled_consumer_visits_capture_inspection_and_body_variables_without_variable_ids() {
        let root=fixture("scene label start\nlocal scene_value=1\nflow.consume_v2(\"scope\",\"volume\"){\ncapture \"value\"=scene_value+missing_capture\ninspect \"inspection\"=__rvn_invariant_text(missing_inspect)\nmusic.volume(__rvn_captured_v1(\"value\")+missing_body)\n}\n\"Ready\"\n");
        let report=check_project(root.to_str().unwrap(),CheckOptions::default());
        assert!(report.diagnostics.iter().filter(|d|d.severity==Severity::Error).all(|d|d.kind=="unassigned-variable"),"{report:?}");
        let unknown:HashSet<_>=report.diagnostics.iter().filter(|d|d.kind=="unassigned-variable").map(|d|d.message.clone()).collect();
        for name in ["missing_capture","missing_inspect","missing_body"] {assert!(unknown.iter().any(|d|d.contains(name)),"{name}: {report:?}");}
        assert_eq!(unknown.len(),3,"scope/producer IDs and the actual scene local must never become unknown variables: {report:?}");
        let body=report.diagnostics.iter().find(|d|d.kind=="unassigned-variable"&&d.message.contains("missing_body")).unwrap();assert_eq!(body.line,Some(6));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn scheduled_consumer_keeps_parameter_scope_and_analyzes_calls_inside_dependencies() {
        let root=fixture("function scale(level){flow.consume_v1(\"fn\",\"return\"){capture \"sample\"=level+secret return __rvn_captured_v1(\"sample\")}}\nhandler on_event(event){flow.consume_v1(\"handler\",\"assignment\"){capture \"input\"=max(event) set result=__rvn_captured_v1(\"input\")}}\nscene label start\nlocal secret=1\nflow.consume_v2(\"story\",\"set\"){capture \"bad\"=abs(1,2) set actual=__rvn_captured_v1(\"bad\")}\n\"Ready\"\n");
        let report=check_project(root.to_str().unwrap(),CheckOptions::default());
        let unknown:Vec<_>=report.diagnostics.iter().filter(|d|d.kind=="unassigned-variable").collect();assert_eq!(unknown.len(),1,"{report:?}");assert!(unknown[0].message.contains("secret"),"{report:?}");assert_eq!(unknown[0].line,Some(1));
        assert!(report.diagnostics.iter().any(|d|d.severity==Severity::Error&&d.message.contains("abs")),"invalid captured function arity must not be hidden: {report:?}");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn speed0993_dynamic_integer_expression_reaches_actual_checker_scope_and_line() {
        let root=fixture("scene label start\ntypewriter.speed(missing_speed+1)\n\"Ready\"\n");
        let report=check_project(root.to_str().unwrap(),CheckOptions::default());let unknown:Vec<_>=report.diagnostics.iter().filter(|d|d.kind=="unassigned-variable").collect();assert_eq!(unknown.len(),1,"{report:?}");assert!(unknown[0].message.contains("missing_speed"));assert_eq!(unknown[0].line,Some(2));
        fs::write(root.join("scripts/main.rvn"),"scene label start\nlocal rate=20\ntypewriter.speed(rate+1)\ntypewriter.speed(0)\n\"Ready\"\n").unwrap();let valid=check_project(root.to_str().unwrap(),CheckOptions::default());assert!(!valid.diagnostics.iter().any(|d|d.severity==Severity::Error||d.kind=="unassigned-variable"),"{valid:?}");fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn typewriter0985_dynamic_boolean_expression_reaches_actual_checker_scope() {
        let root=fixture("scene label start\ntypewriter=not missing_bool\n\"Ready\"\n");
        let report=check_project(root.to_str().unwrap(),CheckOptions::default());
        let unknown:Vec<_>=report.diagnostics.iter().filter(|d|d.kind=="unassigned-variable").collect();
        assert_eq!(unknown.len(),1,"{report:?}");assert!(unknown[0].message.contains("missing_bool"));assert_eq!(unknown[0].line,Some(2));
        fs::write(root.join("scripts/main.rvn"),"scene label start\nlocal enabled=false\ntypewriter=not enabled\ntypewriter=true\n\"Ready\"\n").unwrap();
        let valid=check_project(root.to_str().unwrap(),CheckOptions::default());
        assert!(!valid.diagnostics.iter().any(|d|d.severity==Severity::Error||d.kind=="unassigned-variable"),"{valid:?}");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn dynamic_music_volume_visits_its_expression_and_keeps_historical_literals() {
        let root=fixture("scene label start\nmusic.volume(missing_volume+0.1)\n\"Ready\"\n");let report=check_project(root.to_str().unwrap(),CheckOptions::default());
        let unknown:Vec<_>=report.diagnostics.iter().filter(|d|d.kind=="unassigned-variable").collect();assert_eq!(unknown.len(),1,"{report:?}");assert!(unknown[0].message.contains("missing_volume"));assert_eq!(unknown[0].line,Some(2));
        fs::write(root.join("scripts/main.rvn"),"scene label start\nlocal level=0.2\nmusic.volume(level+0.1)\nmusic.volume(0.3)\nmusic.volume(\"0.4\")\n\"Ready\"\n").unwrap();let valid=check_project(root.to_str().unwrap(),CheckOptions::default());assert!(!valid.diagnostics.iter().any(|d|d.severity==Severity::Error||d.kind=="unassigned-variable"),"{valid:?}");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn scheduled_source_checker_reuses_frozen_declared_pin_contract_across_imports() {
        let root=fixture("use \"producer.rvn\"\nscene label start\n\"Ready\"\n");
        let source="function probe(){flow.consume_v1(\"g9/n1:function_entry|exec=11@0\",\"n2:function_return|exec=12@0,result=13@0\"){capture \"n3:random|a=14@0,b=15@0,result=16@0\"=random(1,9) return __rvn_captured_v1(\"n3:random|a=14@0,b=15@0,result=16@0\")}}\n";
        fs::write(root.join("scripts/producer.rvn"),source).unwrap();let valid=check_project(root.to_str().unwrap(),CheckOptions::default());assert!(!valid.diagnostics.iter().any(|d|d.severity==Severity::Error),"{valid:?}");
        fs::write(root.join("scripts/producer.rvn"),source.replace("result=16@0","result=15@0")).unwrap();let invalid=check_project(root.to_str().unwrap(),CheckOptions::default());assert!(invalid.diagnostics.iter().any(|d|d.kind=="scheduled-source"&&d.severity==Severity::Error),"colliding real declared pins must use the canonical source validator: {invalid:?}");
        fs::remove_dir_all(root).unwrap();
    }


    #[test]
    fn scheduled_control_body_keeps_real_label_edges_and_termination() {
        let root=fixture("scene label start\nflow.consume_v2(\"story\",\"jump\"){jump done}\n\"Dead\"\nscene label skipped\n\"Skipped\"\nscene label done\n\"Done\"\n");let report=check_project(root.to_str().unwrap(),CheckOptions::default());
        assert!(!report.diagnostics.iter().any(|d|d.severity==Severity::Error),"{report:?}");
        let unreachable:Vec<_>=report.diagnostics.iter().filter(|d|d.kind=="unreachable-label").collect();assert_eq!(unreachable.len(),1,"{report:?}");assert!(unreachable[0].message.contains("'skipped'"),"{report:?}");assert!(report.diagnostics.iter().any(|d|d.kind=="dead-code"&&d.message.contains("jump")),"{report:?}");
        fs::write(root.join("scripts/main.rvn"),"scene label start\nflow.consume_v2(\"story\",\"conditional\"){if true {flow.consume_v2(\"story\",\"nested\"){jump done}}}\nscene label done\n\"Done\"\n").unwrap();let nested=check_project(root.to_str().unwrap(),CheckOptions::default());assert!(!nested.diagnostics.iter().any(|d|d.severity==Severity::Error||d.kind=="unreachable-label"||d.kind=="unreferenced-label"),"{nested:?}");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn explicit_scene_entries_are_start_targets_but_locals_do_not_assign_other_scenes_or_functions() {
        let root=fixture("function read(){return scene_only}\nscene label start\nlocal scene_only=2\n\"[scene_only]\"\njump next\nscene label next\n\"[scene_only]\"\n");
        let report=check_project(root.to_str().unwrap(),CheckOptions::default());
        assert!(!kinds(&report).contains(&"missing-start-label"),"{report:?}");
        assert!(!kinds(&report).contains(&"unknown-label-target"),"{report:?}");
        let uses:Vec<_>=report.diagnostics.iter().filter(|diagnostic|diagnostic.kind=="unassigned-variable" && diagnostic.message.contains("scene_only")).collect();
        assert_eq!(uses.len(),2,"{report:?}");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn resolved_import_context_accepts_local_fragments_only_in_an_explicit_scene_program() {
        let root=fixture("scene label start\nuse \"fragment.rvn\"\n\"[score]\"\n");
        fs::write(root.join("scripts/fragment.rvn"),"local score=2\n").unwrap();
        let report=check_project(root.to_str().unwrap(),CheckOptions::default());
        assert!(!report.diagnostics.iter().any(|diagnostic|diagnostic.severity==Severity::Error),"{report:?}");
        assert!(!kinds(&report).contains(&"unassigned-variable"),"{report:?}");
        fs::write(root.join("scripts/main.rvn"),"label start\nuse \"fragment.rvn\"\n\"[score]\"\n").unwrap();
        let legacy=check_project(root.to_str().unwrap(),CheckOptions::default());
        assert!(legacy.diagnostics.iter().any(|diagnostic|diagnostic.severity==Severity::Error),"{legacy:?}");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn foreach_item_stays_lexical_and_does_not_supply_another_scene_global() {
        let root=fixture("scene label start\nlocal item=0\nfor item in [1,2]{\"[item]\"}\njump next\nscene label next\n\"[item]\"\n");
        let report=check_project(root.to_str().unwrap(),CheckOptions::default());
        assert_eq!(report.diagnostics.iter().filter(|diagnostic|diagnostic.kind=="unassigned-variable" && diagnostic.message.contains("item")).count(),1,"{report:?}");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn function_only_import_keeps_the_surrounding_label_control_flow() {
        let root=fixture("scene label start\nuse \"functions.rvn\"\njump done\nscene label unreachable\n\"dead\"\nscene label done\n\"done\"\n");
        fs::write(root.join("scripts/functions.rvn"),"function helper(){return 1}\n").unwrap();
        let report=check_project(root.to_str().unwrap(),CheckOptions::default());
        assert!(!report.diagnostics.iter().any(|diagnostic|diagnostic.severity==Severity::Error),"{report:?}");
        let unreachable:Vec<_>=report.diagnostics.iter().filter(|diagnostic|diagnostic.kind=="unreachable-label").collect();
        assert_eq!(unreachable.len(),1,"{report:?}");
        assert!(unreachable[0].message.contains("'unreachable'"),"{report:?}");
        fs::write(root.join("scripts/main.rvn"),"label start\nuse \"functions.rvn\"\nreturn\nlabel unreachable\n\"dead\"\n").unwrap();
        let returned=check_project(root.to_str().unwrap(),CheckOptions::default());
        assert!(kinds(&returned).contains(&"unreachable-label"),"{returned:?}");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn outer_scene_locals_cannot_mask_lexical_global_reads_in_definitions() {
        let source="scene label start\nlocal secret=1\nfunction probe(){return secret}\nscreen hud(){return component(\"root\",\"text\",{\"text\":secret},[])}\nhandler on_event(event){set result=secret}\n\"[secret]\"\n";
        let root=fixture(source);
        let anchors=|report:&CheckReport|report.diagnostics.iter()
            .filter(|diagnostic|diagnostic.kind=="unassigned-variable" && diagnostic.message.contains("secret"))
            .map(|diagnostic|(diagnostic.line,diagnostic.column,diagnostic.source_line.clone()))
            .collect::<Vec<_>>();
        let expected=|first_line:usize| -> Vec<(Option<usize>,Option<usize>,Option<String>)> {vec![
            (Some(first_line),Some(18),Some("function probe(){return secret}".into())),
            (Some(first_line+1),Some(14),Some("screen hud(){return component(\"root\",\"text\",{\"text\":secret},[])}".into())),
            (Some(first_line+2),Some(25),Some("handler on_event(event){set result=secret}".into())),
        ]};
        let report=check_project(root.to_str().unwrap(),CheckOptions::default());
        assert!(report.failed,"Missing lexical globals must be errors: {report:?}");
        assert!(report.diagnostics.iter().filter(|diagnostic|diagnostic.severity==Severity::Error).all(|diagnostic|diagnostic.kind=="unassigned-variable" && diagnostic.message.contains("secret")),"{report:?}");
        assert_eq!(report.diagnostics.iter().filter(|diagnostic|diagnostic.kind=="unassigned-variable" && diagnostic.message.contains("secret")).count(),3,"{report:?}");
        assert_eq!(anchors(&report),expected(3),"Each definition must own exactly one diagnostic, not three equal-name counts: {report:?}");
        // Real project bytes change both the UTF-8 byte offset and line endings.
        let unicode_source=format!("// préface 🙂\r\n{}",source.replace('\n',"\r\n"));
        fs::write(root.join("scripts/main.rvn"),&unicode_source).unwrap();
        let unicode=check_project(root.to_str().unwrap(),CheckOptions::default());
        assert!(unicode.failed,"{unicode:?}");
        assert_eq!(anchors(&unicode),expected(4),"Raw Unicode/CRLF provenance must stay exact: {unicode:?}");
        fs::write(root.join("scripts/main.rvn"),format!("// préface 🙂\r\ninit {{set secret=2}}\r\n{}",source.replace('\n',"\r\n"))).unwrap();
        let resolved=check_project(root.to_str().unwrap(),CheckOptions::default());
        assert!(!resolved.diagnostics.iter().any(|diagnostic|diagnostic.severity==Severity::Error),"A real Game global must satisfy all three lexical reads: {resolved:?}");
        assert!(!resolved.failed,"{resolved:?}");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn scene_timer_targets_participate_in_label_validation_and_async_reachability() {
        let root=fixture("scene label start\ntimer 1 => call next\njump done\nscene label next\n\"timer\"\nreturn\nlabel done\n\"waiting\"\n");
        let report=check_project(root.to_str().unwrap(),CheckOptions::default());
        assert!(!kinds(&report).contains(&"unknown-label-target") && !kinds(&report).contains(&"unreachable-label"),"{report:?}");
        fs::write(root.join("scripts/main.rvn"),"scene label start\ntimer 1 => jump missing\n\"waiting\"\n").unwrap();
        let invalid=check_project(root.to_str().unwrap(),CheckOptions::default());
        assert!(invalid.diagnostics.iter().any(|diagnostic|diagnostic.kind=="unknown-label-target" && diagnostic.message.contains("missing")),"{invalid:?}");
        fs::remove_dir_all(root).unwrap();
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
    fn handler_global_reads_and_writes_are_not_mistaken_for_local_variables() {
        let root = fixture("init {set completed = 0}\nhandler finished(event) {local temporary = 1 for item in [1] {set completed = completed + item + temporary}}\nlabel start\nreturn\n");
        let report = check_project(root.to_str().unwrap(), CheckOptions { strict: true });
        assert!(!report.failed, "{:?}", report.diagnostics);
        let root_missing =
            fixture("handler finished(event) {set result = missing + 1}\nlabel start\nreturn\n");
        let report = check_project(root_missing.to_str().unwrap(), CheckOptions::default());
        assert!(
            report
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.kind == "unassigned-variable"
                    && diagnostic.message.contains("missing")),
            "{:?}",
            report.diagnostics
        );
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(root_missing).unwrap();
    }

    #[test]
    fn story_open_diagnostic_has_the_real_crlf_method_location() {
        let root=fixture("screen inventory(data){return component(\"root\",\"text\",{},[])}\r\nhandler open_inventory(event){\r\n    ui.open_story(\"inventory\",[missing],true,60)\r\n}\r\nlabel start\r\nreturn\r\n");
        let report=check_project(root.to_str().unwrap(),CheckOptions::default());
        let diagnostic=report.diagnostics.iter().find(|diagnostic|diagnostic.kind=="unassigned-variable"&&diagnostic.message.contains("missing")).expect("Missing argument must be diagnosed");
        assert_eq!(diagnostic.line,Some(3));assert_eq!(diagnostic.column,Some(5));assert_eq!(diagnostic.span_len,Some("ui.open_story(".len()));
        assert!(diagnostic.source_line.as_ref().is_some_and(|line|line.contains("ui.open_story")));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn menu_execute_reads_handler_globals_and_reports_missing_request_variables() {
        let root=fixture("init{set page_id=\"title\"}\r\nhandler open_page(event){\r\n    menu.execute(menu_open_page(page_id))\r\n}\r\nhandler broken(event){\r\n    menu.execute(menu_choose(missing_index))\r\n}\r\nlabel start\r\nreturn\r\n");
        let report=check_project(root.to_str().unwrap(),CheckOptions::default());
        let diagnostic=report.diagnostics.iter().find(|diagnostic|diagnostic.kind=="unassigned-variable"&&diagnostic.message.contains("missing_index")).expect("A missing request variable must be diagnosed");
        assert_eq!(diagnostic.line,Some(6));assert_eq!(diagnostic.column,Some(5));assert_eq!(diagnostic.span_len,Some("menu.execute(".len()));
        assert!(diagnostic.source_line.as_ref().is_some_and(|line|line.contains("missing_index")));
        assert!(!report.diagnostics.iter().any(|diagnostic|diagnostic.kind=="unused-variable"&&diagnostic.message.contains("page_id")),"{:?}",report.diagnostics);
        fs::remove_dir_all(root).unwrap();
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
    fn videos_validate_direct_and_function_resources_and_localized_cues() {
        let root=fixture("function clip(){return video_clip(\"clip.webm\",{\"rect\":[0,0,320,180],\"subtitles\":[{\"start\":0,\"end\":1,\"text\":\"video.cue\"}]})}\nlabel start\nvideo.play(\"intro\",clip())\nvideo.play(\"other\",video_clip(\"missing.webm\",{}))\nreturn\n");
        fs::write(
            root.join("assets/clip.webm"),
            include_bytes!("../../examples/videos/assets/clip.webm"),
        )
        .unwrap();
        write_locale(&root, "en.toml", "[strings]\n\"video.cue\"=\"A caption\"\n");
        let report = check_project(root.to_str().unwrap(), CheckOptions::default());
        let errors: Vec<_> = report
            .diagnostics
            .iter()
            .filter(|problem| problem.kind == "invalid-video-resource")
            .collect();
        assert_eq!(errors.len(), 1, "{:?}", report.diagnostics);
        assert!(errors[0].message.contains("missing.webm"));
        assert!(!kinds(&report).contains(&"unused-locale-key"));
        let mut invalid = include_bytes!("../../examples/videos/assets/clip.webm").to_vec();
        let offset = invalid
            .windows(5)
            .position(|bytes| bytes == b"V_VP8")
            .unwrap();
        invalid[offset + 4] = b'9';
        fs::write(root.join("assets/clip.webm"), invalid).unwrap();
        let report = check_project(root.to_str().unwrap(), CheckOptions::default());
        assert_eq!(
            report
                .diagnostics
                .iter()
                .filter(|problem| problem.kind == "invalid-video-resource")
                .count(),
            2
        );
        let _ = fs::remove_dir_all(root);
    }

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
    fn composed_characters_check_their_layers_without_a_legacy_default_sprite() {
        let root=fixture("function portrait(){return layered_image([600,1000],{},[image_layer(\"body\",\"body.png\",{})])}\ninit {character.create(\"iris\",\"Iris\")}\nlabel start\ncharacter.compose(\"iris\",portrait())\niris.show()\nreturn\n");
        let report = check_project(root.to_str().unwrap(), CheckOptions::default());
        assert!(kinds(&report).contains(&"missing-interface-image"));
        assert!(!kinds(&report).contains(&"missing-asset"));
        write_asset(&root, "body.png");
        let report = check_project(root.to_str().unwrap(), CheckOptions::default());
        assert!(!report.has_errors(), "{:?}", report.diagnostics);
        let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn discovered_character_layers_diagnose_missing_matching_assets() {
        let root=fixture("function portrait(){return layered_image([600,1000],{},image_layers(\"iris\",[\"iris__body.png\",\"other.png\"]))}\ninit {character.create(\"iris\",\"Iris\")}\nlabel start\ncharacter.compose(\"iris\",portrait())\niris.show()\nreturn\n");
        let report = check_project(root.to_str().unwrap(), CheckOptions::default());
        assert!(kinds(&report).contains(&"missing-interface-image"));
        write_asset(&root, "iris__body.png");
        let report = check_project(root.to_str().unwrap(), CheckOptions::default());
        assert!(!report.has_errors(), "{:?}", report.diagnostics);
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
    fn screen_bindings_translations_and_images_are_checked() {
        let root=fixture("screen inventory() { return component(\"root\",\"column\",{},[component(\"name\",\"input\",{\"binding\":\"player_name\",\"placeholder_key\":\"ui.name\"},[]),component(\"picture\",\"image\",{\"image\":\"missing.png\"},[])]) }\ninit { set player_name=\"Camille\" }\nlabel start\nui.open(\"inventory\",[],true,1)\nreturn\n");
        write_locale(&root, "en.toml", "[strings]\n\"ui.name\"=\"Name\"\n");
        let report = check_project(root.to_str().unwrap(), CheckOptions::default());
        assert!(kinds(&report).contains(&"missing-interface-image"));
        assert!(!kinds(&report).contains(&"unused-variable"));
        assert!(!kinds(&report).contains(&"unused-locale-key"));
        assert!(!kinds(&report).contains(&"missing-locale-key"));
        write_asset(&root, "missing.png");
        let report = check_project(root.to_str().unwrap(), CheckOptions::default());
        assert!(!kinds(&report).contains(&"missing-interface-image"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn dynamic_screen_keys_suppress_only_matching_unused_keys() {
        let root=fixture("screen inventory() { return component(\"root\",\"text\",{\"text_key\":\"ui.item.\"+selected},[]) }\ninit { set selected=\"key\" }\nlabel start\nui.open(\"inventory\",[],true,1)\nreturn\n");
        write_locale(
            &root,
            "en.toml",
            "[strings]\n\"ui.item.key\"=\"Key\"\n\"obsolete\"=\"Unused\"\n",
        );
        let report = check_project(root.to_str().unwrap(), CheckOptions::default());
        let unused: Vec<_> = report
            .diagnostics
            .iter()
            .filter(|problem| problem.kind == "unused-locale-key")
            .collect();
        assert_eq!(unused.len(), 1);
        assert!(unused[0].message.contains("obsolete"));
        let _ = fs::remove_dir_all(root);
    }

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

    #[test]
    fn root_source_does_not_treat_exports_or_backups_as_orphan_scripts() {
        let root = fixture("label start\nreturn\n");
        fs::write(
            root.join("rvn.toml"),
            "[project]\ntitle=\"Test\"\nmain_script=\"main.rvn\"\nstart_label=\"start\"\n",
        )
        .unwrap();
        fs::rename(root.join("scripts/main.rvn"), root.join("main.rvn")).unwrap();
        for folder in ["dist/Game/data", "dist-web/Game", ".rvn-backups/session"] {
            fs::create_dir_all(root.join(folder)).unwrap();
            fs::write(root.join(folder).join("main.rvn"), "label start\nreturn\n").unwrap();
        }
        #[cfg(unix)]
        std::os::unix::fs::symlink(&root, root.join("recursive-source-link")).unwrap();
        let report = check_project(root.to_str().unwrap(), CheckOptions::default());
        assert!(!kinds(&report).contains(&"orphan-script"), "{report:?}");
        fs::write(root.join("lonely.rvn"), "label lonely\nreturn\n").unwrap();
        let report = check_project(root.to_str().unwrap(), CheckOptions::default());
        assert!(kinds(&report).contains(&"orphan-script"));
        fs::remove_dir_all(root).unwrap();
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

    #[test]
    fn unit_cli1021_globals_parameters_locals_and_call_modes_follow_real_source() {
        let source = "init {set count=0}\nfunction effect(value)->(){\nlocal scratch=value\nfor item in [1] {set count=count+item+scratch}\nset scratch=2\nset value=3\nset observed=secret\nreturn\nset dead=missing_dead\n}\nscene label start\nlocal secret=4\ncall effect(secret+missing_arg)\nreturn\n";
        let root = fixture(source);
        let report = check_project(root.to_str().unwrap(), CheckOptions::default());
        assert!(!report.diagnostics.iter().any(|d| matches!(d.kind, "parse" | "parse-eof" | "lex" | "script-load")), "{report:?}");
        let unknown = report.diagnostics.iter().filter(|d| d.kind == "unassigned-variable").collect::<Vec<_>>();
        assert_eq!(unknown.len(), 3, "{report:?}");
        for name in ["secret", "missing_dead", "missing_arg"] {
            assert!(unknown.iter().any(|d| d.message.contains(&format!("'{name}'"))), "{report:?}");
        }
        assert!(report.diagnostics.iter().any(|d| d.kind == "dead-code" && d.message.contains("return")), "{report:?}");
        assert!(!kinds(&report).contains(&"unknown-label-target"), "Unit calls are not narrative label calls: {report:?}");
        fs::write(root.join("scripts/main.rvn"), "function effect(value)->(){return}\nfunction scalar(value){return value}\nscene label start\ncall effect()\ncall scalar(1)\nreturn\n").unwrap();
        let invalid = check_project(root.to_str().unwrap(), CheckOptions::default());
        assert!(kinds(&invalid).contains(&"unit-call-arity"), "{invalid:?}");
        assert!(kinds(&invalid).contains(&"unit-call-target"), "{invalid:?}");
        fs::remove_dir_all(root).unwrap();
    }

    // Genuine Graph->source fixture. Catalogue/Graph APIs allocate every source
    // identity; no Site literal, Widget owner, cell, PC or runtime grant is made.
    fn actual_connected_macro_cli_source(expression: &str, value: &str) -> (String, rvn_parser::scheduled_source::standard_macro_contract::Site) {
        use rvn_graph::{GraphKind, NodeKind as K, PropertyValue};
        let raw = format!("init{{set sink=0}}\nhandler allowed(event){{set operand_probe={expression} set sink={value}}}\nscene label start\nreturn\n");
        let mut graphs = rvn_graph::import_script(&rvn_parser::parse(&raw).unwrap()).unwrap();
        let graph = graphs.iter_mut().find(|g| matches!(g.kind, GraphKind::Handler { .. })).unwrap();
        let probe = graph.nodes.values().find(|n| n.kind == K::SetVariable
            && n.properties.get("name") == Some(&PropertyValue::String("operand_probe".into()))).unwrap().id;
        let input = graph.pin_by_key(probe, "value").unwrap().id;
        let producer = graph.edges.values().find(|e| e.input == input).unwrap().output;
        let exec_in = graph.pin_by_key(probe, "exec_in").unwrap().id;
        let exec_out = graph.pin_by_key(probe, "exec_out").unwrap().id;
        let previous = graph.edges.values().find(|e| e.input == exec_in).unwrap().output;
        let next = graph.edges.values().find(|e| e.output == exec_out).unwrap().input;
        graph.remove_node(probe).unwrap();
        let node = graph.add_catalog_node(K::DoN, [300.0, 0.0]).unwrap();
        graph.connect(previous, graph.pin_by_key(node, "exec_in").unwrap().id).unwrap();
        graph.connect(graph.pin_by_key(node, "exit").unwrap().id, next).unwrap();
        graph.connect(producer, graph.pin_by_key(node, "n").unwrap().id).unwrap();
        graph.enable_native_frame_preparation().unwrap();
        let compiled = rvn_graph::transpile_project(&graphs).unwrap();
        assert!(compiled.source.contains("operand_v1 {"));
        let mut pending = compiled.ast.iter().collect::<Vec<_>>();
        let site = loop {
            let statement = pending.pop().expect("genuine generated macro dispatch");
            match statement {
                Statement::StandardMacroDispatch { dispatch } => break dispatch.site.clone(),
                Statement::Handler { body, .. } | Statement::ScheduledConsumer { body, .. } => pending.extend(body),
                _ => {}
            }
        };
        (compiled.source, site)
    }

    #[test]
    fn macro_cli1021_deferred_and_nested_consumer_metadata_reach_existing_checks() {
        let (source, _) = actual_connected_macro_cli_source(
            r#"len(component("capture","image",{"image":"capture.png","text_key":"ui.capture"},[]))"#,
            r#"component("branch","text",{"text_key":"ui.branch"},[])"#);
        let root = fixture(&source);
        write_locale(&root, "en.toml", "[strings]\n");
        let report = check_project(root.to_str().unwrap(), CheckOptions::default());
        assert!(!report.diagnostics.iter().any(|d| matches!(d.kind, "parse" | "parse-eof" | "lex" | "script-load")), "{report:?}");
        assert!(report.diagnostics.iter().any(|d| d.kind == "missing-interface-image" && d.message.contains("capture.png")), "{report:?}");
        for name in ["ui.capture", "ui.branch"] {
            assert!(report.diagnostics.iter().any(|d| d.kind == "missing-locale-key" && d.message.contains(name)), "{report:?}");
        }
        assert!(!kinds(&report).contains(&"script-load"), "{report:?}");
        fs::write(root.join("assets/capture.png"), include_bytes!("../../examples/animations/assets/card-one.png")).unwrap();
        write_locale(&root, "en.toml", "[strings]\n\"ui.capture\"=\"Capture\"\n\"ui.branch\"=\"Branch\"\n");
        let complete = check_project(root.to_str().unwrap(), CheckOptions::default());
        assert!(!complete.diagnostics.iter().any(|d| matches!(d.kind, "missing-interface-image" | "missing-locale-key" | "unused-locale-key")), "{complete:?}");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn macro_cli1021_live_data_is_not_a_variable_and_uses_canonical_source_display() {
        let (_, site) = actual_connected_macro_cli_source("random(1,1)", "1");
        let source = format!("set sample=flow.macro_counter_v1({:?},{:?})", site.scope_source(), site.node_source());
        let parsed = rvn_parser::parse(&source).unwrap();
        let [Statement::SetVar { value, .. }] = parsed.as_slice() else { panic!("actual parsed live data"); };
        assert!(matches!(value, Expr::StandardMacroData { .. }));
        let location = Location { file: PathBuf::from("diagnostic.rvn"), line: 1, column: 1, source_line: Some(source), span_len: None };
        let mut variables = Vec::new();
        collect_expr_vars(value, &mut variables, &location);
        assert!(variables.is_empty());
        let displayed = expr_to_display(value);
        assert!(displayed.starts_with("flow.macro_counter_v1("));
        let reparsed = rvn_parser::parse(&format!("set sample={displayed}")).unwrap();
        let [Statement::SetVar { value: restored, .. }] = reparsed.as_slice() else { panic!("display must remain an actual expression"); };
        assert_eq!(restored, value);
    }

    #[test]
    fn macro_cli1021_definition_resources_and_unit_argument_metadata_reach_existing_checks() {
        let root = fixture("function accept(value)->(){return}\nhandler sound(event){video.play(\"probe\",video_clip(\"missing.webm\",{}))}\nscene label start\ncall accept(component(\"call\",\"image\",{\"image\":\"call.png\",\"text_key\":\"ui.call\"},[]))\nreturn\n");
        write_locale(&root, "en.toml", "[strings]\n");
        let report = check_project(root.to_str().unwrap(), CheckOptions::default());
        assert!(!report.diagnostics.iter().any(|d| matches!(d.kind, "parse" | "parse-eof" | "lex" | "script-load")), "{report:?}");
        assert!(report.diagnostics.iter().any(|d| d.kind == "invalid-video-resource" && d.message.contains("missing.webm")), "{report:?}");
        assert!(report.diagnostics.iter().any(|d| d.kind == "missing-interface-image" && d.message.contains("call.png")), "{report:?}");
        assert!(report.diagnostics.iter().any(|d| d.kind == "missing-locale-key" && d.message.contains("ui.call")), "{report:?}");
        assert!(!kinds(&report).contains(&"script-load"), "{report:?}");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn macro_cli1021_unrelated_captured_dictionary_is_not_component_metadata() {
        // Real DoN producer DAG: these ordinary dictionary keys are not UI
        // metadata merely because their names resemble component properties.
        let (source, _) = actual_connected_macro_cli_source(
            r#"len(dict("image","unrelated.png","text_key","ui.unrelated"))"#, "1");
        let root = fixture(&source);
        write_locale(&root, "en.toml", "[strings]\n");
        let report = check_project(root.to_str().unwrap(), CheckOptions::default());
        assert!(!report.diagnostics.iter().any(|d| matches!(d.kind, "parse" | "parse-eof" | "lex" | "script-load")), "{report:?}");
        assert!(!report.diagnostics.iter().any(|d| d.kind == "missing-interface-image" && d.message.contains("unrelated.png")), "{report:?}");
        assert!(!report.diagnostics.iter().any(|d| d.kind == "missing-locale-key" && d.message.contains("ui.unrelated")), "{report:?}");
        fs::remove_dir_all(root).unwrap();
    }

}

#[cfg(test)]
#[path="enum_cli_check_tests1025.rs"]
mod enum_assets1025_tests;
