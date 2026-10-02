use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};

use rvn_parser::{parse_recovering, Expr, ParseError, ParseErrorKind, Statement, TextSegment};
use tower_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, Diagnostic, DiagnosticRelatedInformation,
    DiagnosticSeverity, Hover, HoverContents, Location, MarkupContent, MarkupKind, NumberOrString,
    Position, Range, SymbolKind, TextEdit, Url, WorkspaceEdit,
};
use walkdir::WalkDir;

#[derive(Debug, Clone, Copy)]
struct KeywordDoc {
    name: &'static str,
    summary: &'static str,
    usage: Option<&'static str>,
    snippet: Option<&'static str>,
}

#[derive(Debug, Clone, Copy)]
struct BuiltinDoc {
    name: &'static str,
    summary: &'static str,
}

#[derive(Debug, Clone, Copy)]
struct MethodDoc {
    name: &'static str,
    summary: &'static str,
    snippet: &'static str,
}

const RVN_KEYWORDS: &[KeywordDoc] = &[
    KeywordDoc {
        name:"screen",summary:"Defines a reusable, parameterized screen. Return a component tree; screens cannot change game variables or run narrative operations.",
        usage:Some("screen name(title) { return component(...) }"),snippet:Some("screen ${1:name}(${2:title}) {\n    return component(\"root\", \"column\", {}, [])\n}"),
    },
    KeywordDoc {
        name:"handler",summary:"Handles one non-blocking screen event. Global assignments update game state; local values stay inside the event handler.",
        usage:Some("handler name(event) { ... }"),snippet:Some("handler ${1:name}(event) {\n    $0\n}"),
    },
    KeywordDoc {
        name:"local",summary:"Declares a handler-local value. Parameters and iterators are also local; use set for a bound game variable.",
        usage:Some("local name = expression"),snippet:Some("local ${1:name} = ${2:expression}"),
    },
    KeywordDoc {
        name: "function",
        summary: "Defines a calculation function with local parameters. Narrative operations are not allowed inside expressions.",
        usage: Some("function name(parameter) { return expression }"),
        snippet: Some("function ${1:name}(${2:parameter}) {\n    return ${3:expression}\n}"),
    },
    KeywordDoc {
        name: "while",
        summary: "Repeats a block while a condition is true. Runtime work is bounded to prevent an infinite loop from freezing the game.",
        usage: Some("while condition { ... }"),
        snippet: Some("while ${1:condition} {\n    $0\n}"),
    },
    KeywordDoc {
        name: "for",
        summary: "Visits each value of a list. Function iterators are local; narrative iterators are saved with the story.",
        usage: Some("for item in collection { ... }"),
        snippet: Some("for ${1:item} in ${2:collection} {\n    $0\n}"),
    },
    KeywordDoc {
        name: "label",
        summary: "Defines a named location that can be used by `jump` or `call`.",
        usage: Some("label name"),
        snippet: Some("label ${1:name}\n    $0"),
    },
    KeywordDoc {
        name: "scene",
        summary: "Sets the active background scene.",
        usage: Some("scene \"path\" [with transition]"),
        snippet: Some("scene \"${1:path}\" with ${2:fade}"),
    },
    KeywordDoc {
        name: "choice",
        summary: "Presents branching options to the player.",
        usage: Some("choice { \"Text\" => { ... } }"),
        snippet: Some("choice {\n    \"${1:Option}\" => {\n        $0\n    }\n}"),
    },
    KeywordDoc {
        name: "jump",
        summary: "Transfers execution to a label.",
        usage: Some("jump label_name"),
        snippet: Some("jump ${1:label}"),
    },
    KeywordDoc {
        name: "call",
        summary: "Enters a label and returns at `return`.",
        usage: Some("call label_name"),
        snippet: Some("call ${1:label}"),
    },
    KeywordDoc {
        name: "return",
        summary: "Returns from the current narrative `call`, or returns a value from a calculation function.",
        usage: Some("return (narrative) / return expression (function)"),
        snippet: Some("return"),
    },
    KeywordDoc {
        name: "set",
        summary: "Assigns an RVN variable.",
        usage: Some("set variable = expression"),
        snippet: Some("set ${1:variable} = ${2:value}"),
    },
    KeywordDoc {
        name: "if",
        summary: "Runs a block when an expression is true.",
        usage: Some("if condition { ... }"),
        snippet: Some("if ${1:condition} {\n    $0\n}"),
    },
    KeywordDoc {
        name: "else",
        summary: "Runs the alternate branch of an `if` statement.",
        usage: Some("else { ... }"),
        snippet: Some("else {\n    $0\n}"),
    },
    KeywordDoc {
        name: "use",
        summary: "Loads one or more RVN script files.",
        usage: Some("use \"path.rvn\""),
        snippet: Some("use \"${1:path.rvn}\""),
    },
    KeywordDoc {
        name: "init",
        summary: "Declares setup statements that run before the script starts.",
        usage: Some("init { ... }"),
        snippet: Some("init {\n    $0\n}"),
    },
    KeywordDoc {
        name: "cinematic",
        summary: "Shows or hides a cinematic image.",
        usage: Some("cinematic \"id\" [with transition]"),
        snippet: Some("cinematic \"${1:id}\" with ${2:fade}"),
    },
    KeywordDoc {
        name: "unlock_ending",
        summary: "Records an unlocked ending.",
        usage: Some("unlock_ending \"ending_id\""),
        snippet: Some("unlock_ending \"${1:ending_id}\""),
    },
    KeywordDoc {
        name: "imagemap",
        summary: "Shows an interactive image map with clickable hotspots.",
        usage: Some("imagemap { background: \"path\" hotspot { ... } => { ... } }"),
        snippet: Some("imagemap {\n    background: \"${1:path}\"\n    $0\n}"),
    },
    KeywordDoc {
        name: "with",
        summary: "Adds a transition to a scene, sprite, audio, or cinematic command.",
        usage: Some("with fade"),
        snippet: None,
    },
    KeywordDoc {
        name: "at",
        summary: "Sets a sprite position.",
        usage: Some("at left"),
        snippet: None,
    },
    KeywordDoc {
        name: "hotspot",
        summary: "Defines a clickable area inside an `imagemap`.",
        usage: Some("hotspot { area: (x1, y1, x2, y2) } => { ... }"),
        snippet: None,
    },
    KeywordDoc {
        name: "and",
        summary: "Boolean AND operator used in expressions.",
        usage: None,
        snippet: None,
    },
    KeywordDoc {
        name: "or",
        summary: "Boolean OR operator used in expressions.",
        usage: None,
        snippet: None,
    },
    KeywordDoc {
        name: "not",
        summary: "Boolean NOT operator used in expressions.",
        usage: None,
        snippet: None,
    },
    KeywordDoc {
        name: "true",
        summary: "Boolean true literal.",
        usage: None,
        snippet: None,
    },
    KeywordDoc {
        name: "false",
        summary: "Boolean false literal.",
        usage: None,
        snippet: None,
    },
    KeywordDoc {
        name: "fade",
        summary: "Built-in transition name.",
        usage: None,
        snippet: None,
    },
    KeywordDoc {
        name: "dissolve",
        summary: "Built-in transition name.",
        usage: None,
        snippet: None,
    },
];

const BUILTIN_ROOTS: &[BuiltinDoc] = &[
    BuiltinDoc{name:"canvas_rect",summary:"canvas_rect(rect, color, radius) draws a rounded rectangle. Rect is [x,y,width,height], color is RGBA in [0,1]."},
    BuiltinDoc{name:"canvas_ellipse",summary:"canvas_ellipse(rect, color) draws an ellipse in reference-pixel coordinates."},
    BuiltinDoc{name:"canvas_line",summary:"canvas_line(points, color, width) draws a bounded polyline; points are [x,y] pairs."},
    BuiltinDoc{name:"canvas_polygon",summary:"canvas_polygon(points, color) draws a filled polygon from [x,y] pairs."},
    BuiltinDoc{name:"canvas_text",summary:"canvas_text(text, position, color, size) draws text in reference pixels, with a local [x,y] position."},
    BuiltinDoc{name:"canvas_image",summary:"canvas_image(asset, rect) draws a project-relative image, with a diagnostic for missing assets."},
    BuiltinDoc{name:"canvas_group",summary:"canvas_group(transform, clip, children) applies [x,y,scale_x,scale_y,rotation_degrees,opacity]. Clip is [] or a local rectangle."},
    BuiltinDoc{name:"canvas_hit",summary:"canvas_hit(id, rect) defines a stable interactive region. Pointer events receive its identity in their local payload."},
    BuiltinDoc{name:"accessibility",summary:"Portable text size, contrast, reduced motion and speech controls. Player preferences override project defaults."},
    BuiltinDoc{name:"motion",summary:"Play, stop and wait for composable animations on a background, character, layer or interface component."},
    BuiltinDoc{name:"video",summary:"Control a named WebM player. Cinematic playback blocks narration; an embedded player may run alongside it."},
    BuiltinDoc{name:"motion_tween",summary:"motion_tween(seconds, from, to, curve) interpolates specified pose channels."},
    BuiltinDoc{name:"motion_spline",summary:"motion_spline(seconds, points, curve) follows x/y control points with Catmull–Rom interpolation; coordinates are reference-pixel offsets and segment timing is uniform."},
    BuiltinDoc{name:"motion_bezier",summary:"motion_bezier(x1, y1, x2, y2) defines a cubic timing curve from (0, 0) to (1, 1); control coordinates must remain in [0, 1]."},
    BuiltinDoc{name:"motion_curve",summary:"motion_curve(function_name, samples) samples a bounded one-parameter RVN calculation function into a timing curve. Use 2–257 samples, finite progress in [0, 1] with endpoints 0 and 1, and no randomness or narrative operations."},
    BuiltinDoc{name:"motion_sequence",summary:"motion_sequence(steps) plays a list of animations in order."},
    BuiltinDoc{name:"motion_parallel",summary:"motion_parallel(branches) plays independent pose channels together; conflicting writes are rejected."},
    BuiltinDoc{name:"motion_pause",summary:"motion_pause(seconds) holds the current pose."},
    BuiltinDoc{name:"motion_repeat",summary:"motion_repeat(count, animation) repeats an animation; count 0 means endless and cannot be waited on."},
    BuiltinDoc{name:"motion_frames",summary:"motion_frames(paths, fps) plays asset-relative images in order."},
    BuiltinDoc{name:"layered_image",summary:"layered_image(canvas, defaults, layers[, options]) defines ordered, conditional character layers and exclusive attributes. The optional fourth options dictionary accepts named variants, ordered attribute rules and a one-parameter RVN selector function."},
    BuiltinDoc{name:"image_layer",summary:"image_layer(id, source, properties) describes one composition layer, with optional group, attribute, conditions and variant-specific art."},
    BuiltinDoc{name:"image_layers",summary:"image_layers(prefix, paths) discovers layers from project-relative image names: prefix__id, prefix__group__attribute, or prefix__variant__group__attribute. The explicit image-list order is drawing order; ambiguous matching names are errors."},
    BuiltinDoc{name:"video_clip",summary:"video_clip(source, properties) defines a WebM VP8/Vorbis clip, target, captions, mask, controls and events."},
    BuiltinDoc {name:"ui",summary:"Opens, closes and focuses reusable screens. Available in narrative code and non-blocking handlers, not calculation expressions."},
    BuiltinDoc { name: "len", summary: "Number of list items, dictionary entries, or Unicode characters in text." },
    BuiltinDoc { name: "contains", summary: "Tests for a list value, dictionary key, or text substring." },
    BuiltinDoc { name: "list_append", summary: "list_append(list, value) returns a new list; the input is unchanged." },
    BuiltinDoc { name: "list_insert", summary: "list_insert(list, index, value) returns a new list." },
    BuiltinDoc { name: "list_remove", summary: "list_remove(list, index) returns a new list." },
    BuiltinDoc { name: "list_set", summary: "list_set(list, index, value) returns a new list." },
    BuiltinDoc { name: "list_concat", summary: "list_concat(first, second) joins two lists without changing either." },
    BuiltinDoc { name: "list_slice", summary: "list_slice(list, start, end) returns a range, excluding end." },
    BuiltinDoc { name: "dict", summary: "dict(key, value, ...) creates a dictionary with unique text keys; also written {\"key\": value}." },
    BuiltinDoc { name: "component", summary: "component(id, kind, properties, children) creates a reusable screen component with explicit identity and child components." },
    BuiltinDoc { name: "dict_get", summary: "dict_get(dictionary, key, fallback) returns a value or the fallback." },
    BuiltinDoc { name: "dict_at", summary: "dict_at(dictionary, key) returns a value; a missing key is an error." },
    BuiltinDoc { name: "dict_set", summary: "dict_set(dictionary, key, value) returns a modified copy." },
    BuiltinDoc { name: "dict_remove", summary: "dict_remove(dictionary, key) returns a copy without the key." },
    BuiltinDoc { name: "dict_keys", summary: "dict_keys(dictionary) returns keys in stable sorted order." },
    BuiltinDoc { name: "dict_values", summary: "dict_values(dictionary) returns values in the same order as dict_keys." },
    BuiltinDoc { name: "random", summary: "random(min, max) returns an inclusive random integer. The stream is restored by loading and rollback." },
    BuiltinDoc {
        name: "music",
        summary: "Controls background music playback.",
    },
    BuiltinDoc {
        name: "sfx",
        summary: "Controls sound effect playback.",
    },
    BuiltinDoc {
        name: "typewriter",
        summary: "Configures the text typewriter effect.",
    },
    BuiltinDoc {
        name: "character",
        summary: "Declares character ids and display names.",
    },
];

#[derive(Debug, Clone, Default)]
pub struct Analysis {
    pub index: ProjectIndex,
}

#[derive(Debug, Clone, Default)]
pub struct ProjectIndex {
    pub functions: HashMap<String, FunctionSymbol>,
    pub labels: HashMap<String, LabelSymbol>,
    pub characters: HashMap<String, CharacterSymbol>,
    pub diagnostics: HashMap<Url, Vec<Diagnostic>>,
    pub document_symbols: HashMap<Url, Vec<RvnSymbol>>,
    pub label_refs: Vec<LabelRef>,
    pub character_uses: Vec<CharacterUse>,
    pub callable_refs: Vec<(Url, crate::callables::Site)>,
    unsafe_callable_renames: HashSet<String>,
}

#[derive(Debug, Clone)]
pub struct FunctionSymbol {
    pub name: String,
    pub parameters: Vec<String>,
    pub uri: Url,
    pub range: Range,
    pub declaration: &'static str,
}

#[derive(Debug, Clone)]
pub struct LabelSymbol {
    pub name: String,
    pub uri: Url,
    pub range: Range,
}

#[derive(Debug, Clone)]
pub struct CharacterSymbol {
    pub id: String,
    pub display_name: String,
    pub uri: Url,
    pub range: Range,
}

#[derive(Debug, Clone)]
pub struct RvnSymbol {
    pub name: String,
    pub detail: Option<String>,
    pub kind: SymbolKind,
    pub range: Range,
    pub selection_range: Range,
}

#[derive(Debug, Clone)]
struct SourceFile {
    uri: Url,
    text: String,
    script: Vec<Statement>,
}

#[derive(Debug, Clone, Default)]
struct SourceRanges {
    labels: HashMap<String, VecDeque<Range>>,
    label_refs: HashMap<(&'static str, String), VecDeque<Range>>,
    character_declarations: HashMap<String, VecDeque<Range>>,
    character_uses: HashMap<String, VecDeque<Range>>,
}

#[derive(Debug, Clone)]
pub struct LabelRef {
    name: String,
    kind: &'static str,
    uri: Url,
    range: Range,
}

#[derive(Debug, Clone)]
pub struct CharacterUse {
    name: String,
    uri: Url,
    range: Range,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RvnSymbolKind {
    Label,
    Character,
    Callable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSymbol {
    pub kind: RvnSymbolKind,
    pub name: String,
}

#[derive(Debug, Default)]
struct SemanticIndex {
    functions: HashMap<String, FunctionSymbol>,
    labels: HashMap<String, LabelSymbol>,
    label_refs: Vec<LabelRef>,
    characters: HashMap<String, CharacterSymbol>,
    character_uses: Vec<CharacterUse>,
}

pub fn analyze_workspace(roots: &[PathBuf], documents: &HashMap<Url, String>) -> Analysis {
    analyze_files(collect_workspace_files(roots, documents))
}

pub fn analyze_for_document(
    uri: &Url,
    roots: &[PathBuf],
    documents: &HashMap<Url, String>,
) -> Analysis {
    analyze_files(collect_document_scope(uri, roots, documents))
}

fn analyze_files(mut files: Vec<(Url, String)>) -> Analysis {
    files.sort_by(|a, b| a.0.as_str().cmp(b.0.as_str()));

    let mut index = ProjectIndex::default();
    let mut semantic = SemanticIndex::default();
    let mut sources = Vec::new();

    for (uri, text) in files {
        let mut diagnostics = Vec::new();
        let recovered = match parse_recovering(&text) {
            Ok(recovered) => recovered,
            Err(error) => {
                diagnostics.push(parse_diagnostic(&error));
                index.diagnostics.insert(uri.clone(), diagnostics);
                continue;
            }
        };
        for error in recovered.errors {
            diagnostics.push(parse_diagnostic(&error));
        }

        let source = SourceFile {
            uri: uri.clone(),
            text,
            script: recovered.script,
        };
        index.callable_refs.extend(
            crate::callables::scan(&source.text)
                .into_iter()
                .filter(|site| !site.role.ends_with("-declaration"))
                .map(|site| (uri.clone(), site)),
        );
        let mut ranges = SourceRanges::scan(&source.text);
        collect_file_symbols(&source, &mut ranges, &mut semantic, &mut diagnostics);
        index
            .document_symbols
            .insert(uri.clone(), document_symbols(&source));
        index.diagnostics.insert(uri, diagnostics);
        sources.push(source);
    }

    index.labels.clone_from(&semantic.labels);
    index.functions.clone_from(&semantic.functions);
    index.characters.clone_from(&semantic.characters);
    index.label_refs.clone_from(&semantic.label_refs);
    index.character_uses.clone_from(&semantic.character_uses);
    validate_semantics(&semantic, &mut index);
    let script: Vec<_> = sources
        .iter()
        .flat_map(|source| source.script.iter().cloned())
        .collect();
    for problem in rvn_parser::validate_logic(&script, false) {
        if problem.code == "duplicate-function" {
            continue;
        } // Already has related declaration locations.
        let role = if problem.code.contains("screen") {
            "screen"
        } else {
            "function"
        };
        let mut locations: Vec<_> = index
            .callable_refs
            .iter()
            .filter(|(_, site)| {
                if site.name != problem.name || site.role != role {
                    return false;
                }
                if problem.code == "function-arity"
                    || problem.code == "screen-arity"
                    || problem.code == "canvas-draw-arity"
                {
                    let expected = index
                        .functions
                        .get(&site.name)
                        .map(|function| function.parameters.len()..=function.parameters.len())
                        .or_else(|| rvn_parser::builtin_arity(&site.name));
                    return site
                        .arity
                        .zip(expected)
                        .is_some_and(|(count, expected)| !expected.contains(&count));
                }
                true
            })
            .map(|(uri, site)| (uri.clone(), site.range))
            .collect();
        if locations.is_empty() {
            if let Some(function) = index.functions.get(&problem.name) {
                locations.push((function.uri.clone(), function.range));
            }
        }
        for (uri, range) in locations {
            let diagnostics = index.diagnostics.entry(uri).or_default();
            if !diagnostics
                .iter()
                .any(|existing| existing.range == range && existing.message == problem.english)
            {
                diagnostics.push(simple_diagnostic(
                    range,
                    DiagnosticSeverity::ERROR,
                    problem.code,
                    problem.english.clone(),
                ));
            }
        }
    }
    // Do not offer an incomplete rename when a reference is dynamic or lies
    // inside escaped dialogue interpolation. It must never silently break code.
    fn protect(block: &[Statement], unsafe_names: &mut HashSet<String>, screens: &[String]) {
        fn expr(value: &Expr, names: &mut HashSet<String>) {
            match value {
                Expr::Call { name, args } => {
                    names.insert(name.clone());
                    for value in args {
                        expr(value, names);
                    }
                }
                Expr::BinOp { left, right, .. }
                | Expr::And(left, right)
                | Expr::Or(left, right) => {
                    expr(left, names);
                    expr(right, names);
                }
                Expr::Index { target, index } => {
                    expr(target, names);
                    expr(index, names);
                }
                Expr::Neg(value) | Expr::Not(value) => expr(value, names),
                Expr::ListLit(values) => {
                    for value in values {
                        expr(value, names);
                    }
                }
                _ => {}
            }
        }
        for statement in block {
            match statement {
                Statement::Dialogue { text, .. } => {
                    for segment in &text.0 {
                        if let TextSegment::Interp(value) = segment {
                            expr(value, unsafe_names);
                        }
                    }
                }
                Statement::Choice { options } => {
                    for option in options {
                        for segment in &option.label.0 {
                            if let TextSegment::Interp(value) = segment {
                                expr(value, unsafe_names);
                            }
                        }
                        protect(&option.body, unsafe_names, screens);
                    }
                }
                Statement::UiOpen { name, .. }
                | Statement::UiClose { name }
                | Statement::UiFocus { name, .. }
                | Statement::UiSetState { name, .. }
                    if !matches!(name,Expr::Str(name) if rvn_parser::is_binding_name(name)) =>
                {
                    unsafe_names.extend(screens.iter().cloned())
                }
                Statement::Init { body }
                | Statement::Function { body, .. }
                | Statement::Screen { body, .. }
                | Statement::Handler { body, .. }
                | Statement::While { body, .. }
                | Statement::ForEach { body, .. } => protect(body, unsafe_names, screens),
                Statement::If {
                    then_branch,
                    else_branch,
                    ..
                } => {
                    protect(then_branch, unsafe_names, screens);
                    protect(else_branch, unsafe_names, screens);
                }
                Statement::Imagemap { hotspots, .. } => {
                    for hotspot in hotspots {
                        protect(&hotspot.body, unsafe_names, screens);
                    }
                }
                _ => {}
            }
        }
    }
    let screens = index
        .functions
        .values()
        .filter(|function| function.declaration == "screen")
        .map(|function| function.name.clone())
        .collect::<Vec<_>>();
    protect(&script, &mut index.unsafe_callable_renames, &screens);
    if index
        .callable_refs
        .iter()
        .any(|(_, site)| site.role == "dynamic-draw")
    {
        index.unsafe_callable_renames.extend(
            index
                .functions
                .values()
                .filter(|function| function.declaration == "function")
                .map(|function| function.name.clone()),
        );
    }
    Analysis { index }
}

impl ProjectIndex {
    pub fn definition_for(&self, symbol: &ResolvedSymbol) -> Option<Location> {
        let (uri, range) = match symbol.kind {
            RvnSymbolKind::Label => {
                let value = self.labels.get(&symbol.name)?;
                (&value.uri, value.range)
            }
            RvnSymbolKind::Character => {
                let value = self.characters.get(&symbol.name)?;
                (&value.uri, value.range)
            }
            RvnSymbolKind::Callable => {
                let value = self.functions.get(&symbol.name)?;
                (&value.uri, value.range)
            }
        };
        Some(Location {
            uri: uri.clone(),
            range,
        })
    }
    pub fn diagnostics_for(&self, uri: &Url) -> Vec<Diagnostic> {
        self.diagnostics.get(uri).cloned().unwrap_or_default()
    }

    pub fn symbols_for(&self, uri: &Url) -> Vec<RvnSymbol> {
        self.document_symbols.get(uri).cloned().unwrap_or_default()
    }

    pub fn symbol_at(&self, uri: &Url, position: Position) -> Option<ResolvedSymbol> {
        for function in self.functions.values() {
            if function.uri == *uri && range_contains(function.range, position) {
                return Some(ResolvedSymbol {
                    kind: RvnSymbolKind::Callable,
                    name: function.name.clone(),
                });
            }
        }
        for (file, site) in &self.callable_refs {
            if file == uri && range_contains(site.range, position) {
                return Some(ResolvedSymbol {
                    kind: RvnSymbolKind::Callable,
                    name: site.name.clone(),
                });
            }
        }
        for label in self.labels.values() {
            if label.uri == *uri && range_contains(label.range, position) {
                return Some(ResolvedSymbol {
                    kind: RvnSymbolKind::Label,
                    name: label.name.clone(),
                });
            }
        }

        for reference in &self.label_refs {
            if reference.uri == *uri && range_contains(reference.range, position) {
                return Some(ResolvedSymbol {
                    kind: RvnSymbolKind::Label,
                    name: reference.name.clone(),
                });
            }
        }

        for character in self.characters.values() {
            if character.uri == *uri && range_contains(character.range, position) {
                return Some(ResolvedSymbol {
                    kind: RvnSymbolKind::Character,
                    name: character.id.clone(),
                });
            }
        }

        for character_use in &self.character_uses {
            if character_use.uri == *uri && range_contains(character_use.range, position) {
                return Some(ResolvedSymbol {
                    kind: RvnSymbolKind::Character,
                    name: character_use.name.clone(),
                });
            }
        }

        None
    }

    pub fn references_for(&self, symbol: &ResolvedSymbol) -> Vec<Location> {
        let mut locations = Vec::new();
        match symbol.kind {
            RvnSymbolKind::Callable => {
                let Some(definition) = self.definition_for(symbol) else {
                    return Vec::new();
                };
                locations.push(definition);
                locations.extend(
                    self.callable_refs
                        .iter()
                        .filter(|(_, site)| site.name == symbol.name)
                        .map(|(uri, site)| Location {
                            uri: uri.clone(),
                            range: site.range,
                        }),
                );
            }
            RvnSymbolKind::Label => {
                let Some(label) = self.labels.get(&symbol.name) else {
                    return Vec::new();
                };
                locations.push(Location {
                    uri: label.uri.clone(),
                    range: label.range,
                });
                locations.extend(
                    self.label_refs
                        .iter()
                        .filter(|reference| reference.name == symbol.name)
                        .map(|reference| Location {
                            uri: reference.uri.clone(),
                            range: reference.range,
                        }),
                );
            }
            RvnSymbolKind::Character => {
                let Some(character) = self.characters.get(&symbol.name) else {
                    return Vec::new();
                };
                locations.push(Location {
                    uri: character.uri.clone(),
                    range: character.range,
                });
                locations.extend(
                    self.character_uses
                        .iter()
                        .filter(|character_use| character_use.name == symbol.name)
                        .map(|character_use| Location {
                            uri: character_use.uri.clone(),
                            range: character_use.range,
                        }),
                );
            }
        }
        locations.sort_by(|a, b| {
            a.uri
                .as_str()
                .cmp(b.uri.as_str())
                .then(a.range.start.line.cmp(&b.range.start.line))
                .then(a.range.start.character.cmp(&b.range.start.character))
        });
        locations.dedup_by(|a, b| a.uri == b.uri && a.range == b.range);
        locations
    }

    pub fn rename_edit(&self, symbol: &ResolvedSymbol, new_name: &str) -> Option<WorkspaceEdit> {
        if !rvn_parser::is_binding_name(new_name) {
            return None;
        }
        match symbol.kind {
            RvnSymbolKind::Label if !self.labels.contains_key(&symbol.name) => return None,
            RvnSymbolKind::Character if !self.characters.contains_key(&symbol.name) => return None,
            RvnSymbolKind::Callable => {
                let declaration = self.functions.get(&symbol.name)?;
                if declaration.declaration == "handler"
                    || self.unsafe_callable_renames.contains(&symbol.name)
                    || rvn_parser::builtin_arity(new_name).is_some()
                    || (new_name != symbol.name && self.functions.contains_key(new_name))
                {
                    return None;
                }
            }
            _ => {}
        }
        if new_name != symbol.name
            && match symbol.kind {
                RvnSymbolKind::Label => self.labels.contains_key(new_name),
                RvnSymbolKind::Character => self.characters.contains_key(new_name),
                RvnSymbolKind::Callable => false,
            }
        {
            return None;
        }
        let mut changes: HashMap<Url, Vec<TextEdit>> = HashMap::new();
        for location in self.references_for(symbol) {
            changes
                .entry(location.uri)
                .or_default()
                .push(TextEdit::new(location.range, new_name.to_string()));
        }
        (!changes.is_empty()).then_some(WorkspaceEdit {
            changes: Some(changes),
            document_changes: None,
            change_annotations: None,
        })
    }

    pub fn suggested_label(&self, typo: &str) -> Option<String> {
        nearest_symbol(typo, self.labels.keys().map(String::as_str))
    }

    pub fn suggested_character(&self, typo: &str) -> Option<String> {
        nearest_symbol(typo, self.characters.keys().map(String::as_str))
    }

    pub fn completion_items(&self, text: &str, position: Position) -> Vec<CompletionItem> {
        match completion_context(text, position) {
            CompletionContext::LabelTarget => {
                let mut labels: Vec<_> = self.labels.values().collect();
                labels.sort_by(|a, b| a.name.cmp(&b.name));
                labels
                    .into_iter()
                    .map(|label| completion(&label.name, CompletionItemKind::REFERENCE, "label"))
                    .collect()
            }
            CompletionContext::LineStart { allow_characters } => {
                let mut items = keyword_completions();
                items.extend(builtin_root_completions());
                items.extend(function_completions(self));
                if allow_characters {
                    items.extend(character_completions(self));
                }
                items
            }
            CompletionContext::AfterDot(target) => method_completions(&target, self),
            CompletionContext::General => {
                let mut items = keyword_completions();
                items.extend(builtin_root_completions());
                items.extend(function_completions(self));
                items
            }
        }
    }

    pub fn hover(&self, text: &str, uri: &Url, position: Position) -> Option<Hover> {
        let word = word_at(text, position)?;
        if let Some(keyword) = keyword_doc(&word) {
            return Some(hover_markdown(keyword));
        }
        if let Some(function) = self.functions.get(&word) {
            return Some(hover_markdown(format!(
                "**{}** `{}({})`\n\nDefined in `{}`.",
                function.declaration,
                function.name,
                function.parameters.join(", "),
                display_uri(&function.uri)
            )));
        }

        if let Some(reference) = label_reference_at(text, position) {
            if let Some(label) = self.labels.get(&reference) {
                return Some(hover_markdown(format!(
                    "**label reference** `{}`\n\nDefined in `{}`.",
                    label.name,
                    display_uri(&label.uri)
                )));
            }
        }

        if let Some(label) = self.labels.get(&word) {
            return Some(hover_markdown(format!(
                "**label** `{}`\n\nDefined in `{}`.",
                label.name,
                display_uri(&label.uri)
            )));
        }

        if let Some(builtin) = builtin_doc(&word) {
            return Some(hover_markdown(builtin));
        }

        if let Some(method) = method_hover(text, position, self) {
            return Some(hover_markdown(method));
        }

        if let Some(character) = self.characters.get(&word) {
            return Some(Hover {
                contents: hover_markdown_contents(format!(
                    "**character id** `{}`\n\nDisplay name: `{}`\n\nDeclared in `{}`.",
                    character.id,
                    character.display_name,
                    display_uri(&character.uri)
                )),
                range: Some(character.range),
            });
        }

        if is_character_like_at(text, position) {
            return Some(hover_markdown(format!(
                "**unknown character** `{word}`\n\nNo character with this id is declared in the current RVN scope."
            )));
        }

        if self
            .document_symbols
            .get(uri)
            .is_some_and(|symbols| symbols.iter().any(|symbol| symbol.name == word))
        {
            return Some(hover_markdown(format!("**RVN symbol** `{word}`")));
        }

        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CompletionContext {
    LabelTarget,
    LineStart { allow_characters: bool },
    AfterDot(String),
    General,
}

fn collect_workspace_files(
    roots: &[PathBuf],
    documents: &HashMap<Url, String>,
) -> Vec<(Url, String)> {
    let mut seen = HashSet::new();
    let mut files = Vec::new();

    for (uri, text) in documents {
        if seen.insert(uri.clone()) {
            files.push((uri.clone(), text.clone()));
        }
    }

    for root in roots {
        if !root.exists() {
            continue;
        }
        for entry in WalkDir::new(root).into_iter().filter_map(Result::ok) {
            let path = entry.path();
            if !path.is_file() || path.extension().and_then(|s| s.to_str()) != Some("rvn") {
                continue;
            }
            let Ok(uri) = Url::from_file_path(path) else {
                continue;
            };
            if seen.contains(&uri) {
                continue;
            }
            if let Ok(text) = fs::read_to_string(path) {
                seen.insert(uri.clone());
                files.push((uri, text));
            }
        }
    }

    files
}

fn collect_document_scope(
    uri: &Url,
    roots: &[PathBuf],
    documents: &HashMap<Url, String>,
) -> Vec<(Url, String)> {
    let Some(current_path) = uri.to_file_path().ok() else {
        return documents
            .get(uri)
            .map(|text| vec![(uri.clone(), text.clone())])
            .unwrap_or_default();
    };

    if let Some(project_root) = find_project_root(&current_path, roots) {
        return collect_workspace_files(&[project_root], documents);
    }

    let mut seen = HashSet::new();
    let mut files = Vec::new();
    collect_use_graph(uri.clone(), current_path, documents, &mut seen, &mut files);
    files
}

fn find_project_root(path: &Path, roots: &[PathBuf]) -> Option<PathBuf> {
    let mut current = path.parent();
    while let Some(dir) = current {
        if dir.join("rvn.toml").is_file() {
            return Some(dir.to_path_buf());
        }
        current = dir.parent();
    }

    roots
        .iter()
        .find(|root| path.starts_with(root) && root.join("rvn.toml").is_file())
        .cloned()
}

fn collect_use_graph(
    uri: Url,
    path: PathBuf,
    documents: &HashMap<Url, String>,
    seen: &mut HashSet<Url>,
    files: &mut Vec<(Url, String)>,
) {
    if !seen.insert(uri.clone()) {
        return;
    }

    let text = documents
        .get(&uri)
        .cloned()
        .or_else(|| fs::read_to_string(&path).ok())
        .unwrap_or_default();
    files.push((uri.clone(), text.clone()));

    let Ok(recovered) = parse_recovering(&text) else {
        return;
    };
    let base_dir = path.parent().unwrap_or_else(|| Path::new("."));
    for statement in recovered.script {
        if let Statement::Use { paths } = statement {
            for raw in paths {
                for target in expand_use_path(base_dir, &raw) {
                    if let Ok(target_uri) = Url::from_file_path(&target) {
                        collect_use_graph(target_uri, target, documents, seen, files);
                    }
                }
            }
        }
    }
}

fn expand_use_path(base_dir: &Path, raw: &str) -> Vec<PathBuf> {
    if raw.ends_with("/*") || raw.ends_with("/*.rvn") {
        let dir_part = raw
            .strip_suffix("/*.rvn")
            .or_else(|| raw.strip_suffix("/*"))
            .unwrap_or(raw);
        let dir = base_dir.join(dir_part);
        let mut files: Vec<_> = fs::read_dir(dir)
            .ok()
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().and_then(|s| s.to_str()) == Some("rvn"))
            .collect();
        files.sort();
        return files;
    }
    vec![base_dir.join(raw)]
}

impl SourceRanges {
    fn scan(text: &str) -> Self {
        let mut ranges = SourceRanges::default();
        for (line_idx, line) in text.lines().enumerate() {
            let indent = line.len() - line.trim_start().len();
            let trimmed = &line[indent..];

            if let Some(name) = trimmed.strip_prefix("label ").and_then(first_ident) {
                ranges
                    .labels
                    .entry(name.to_string())
                    .or_default()
                    .push_back(range(
                        line_idx,
                        indent + "label ".len(),
                        indent + "label ".len() + name.len(),
                    ));
            }

            for command in ["jump", "call"] {
                if let Some(target) = trimmed
                    .strip_prefix(command)
                    .and_then(|rest| rest.strip_prefix(char::is_whitespace))
                    .and_then(|rest| first_ident(rest.trim_start()))
                {
                    let Some(col) = line.find(target) else {
                        continue;
                    };
                    ranges
                        .label_refs
                        .entry((command, target.to_string()))
                        .or_default()
                        .push_back(range(line_idx, col, col + target.len()));
                }
            }

            if let Some(id_range) = character_create_id_range(line_idx, line) {
                let id = &line[id_range.start.character as usize..id_range.end.character as usize];
                ranges
                    .character_declarations
                    .entry(id.to_string())
                    .or_default()
                    .push_back(id_range);
            }

            if let Some((id, id_range)) = character_line_head(line_idx, line) {
                ranges
                    .character_uses
                    .entry(id.to_string())
                    .or_default()
                    .push_back(id_range);
            }
        }
        ranges
    }

    fn take_label(&mut self, name: &str) -> Option<Range> {
        self.labels.get_mut(name)?.pop_front()
    }

    fn take_label_ref(&mut self, kind: &'static str, name: &str) -> Option<Range> {
        self.label_refs
            .get_mut(&(kind, name.to_string()))?
            .pop_front()
    }

    fn take_character_declaration(&mut self, id: &str) -> Option<Range> {
        self.character_declarations.get_mut(id)?.pop_front()
    }

    fn take_character_use(&mut self, id: &str) -> Option<Range> {
        self.character_uses.get_mut(id)?.pop_front()
    }
}

fn collect_file_symbols(
    source: &SourceFile,
    ranges: &mut SourceRanges,
    semantic: &mut SemanticIndex,
    diagnostics: &mut Vec<Diagnostic>,
) {
    collect_block(source, ranges, &source.script, semantic, diagnostics);
}

fn collect_block(
    source: &SourceFile,
    ranges: &mut SourceRanges,
    statements: &[Statement],
    semantic: &mut SemanticIndex,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for statement in statements {
        match statement {
            Statement::Init { body } => collect_block(source, ranges, body, semantic, diagnostics),
            Statement::Function {
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
                let keyword = match statement {
                    Statement::Screen { .. } => "screen",
                    Statement::Handler { .. } => "handler",
                    _ => "function",
                };
                let range = find_command_target_range(&source.text, keyword, name);
                if let Some(existing) = semantic.functions.get(name) {
                    diagnostics.push(diagnostic_with_related(
                        range,
                        DiagnosticSeverity::ERROR,
                        "duplicate-function",
                        format!("duplicate function `{name}`"),
                        vec![DiagnosticRelatedInformation {
                            location: Location {
                                uri: existing.uri.clone(),
                                range: existing.range,
                            },
                            message: "first declaration is here".into(),
                        }],
                    ));
                } else if rvn_parser::builtin_arity(name).is_some() {
                    diagnostics.push(simple_diagnostic(
                        range,
                        DiagnosticSeverity::ERROR,
                        "duplicate-function",
                        format!("cannot replace built-in function `{name}`"),
                    ));
                } else {
                    semantic.functions.insert(
                        name.clone(),
                        FunctionSymbol {
                            name: name.clone(),
                            parameters: parameters.clone(),
                            uri: source.uri.clone(),
                            range,
                            declaration: keyword,
                        },
                    );
                }
                collect_block(source, ranges, body, semantic, diagnostics);
            }
            Statement::While { condition, body } => {
                collect_expr(condition, source, semantic);
                collect_block(source, ranges, body, semantic, diagnostics);
            }
            Statement::ForEach {
                collection, body, ..
            } => {
                collect_expr(collection, source, semantic);
                collect_block(source, ranges, body, semantic, diagnostics);
            }
            Statement::CharacterCreate { id, display_name } => {
                semantic.characters.entry(id.clone()).or_insert_with(|| {
                    let range = ranges
                        .take_character_declaration(id)
                        .unwrap_or_else(|| find_character_declaration_range(&source.text, id));
                    CharacterSymbol {
                        id: id.clone(),
                        display_name: display_name.clone(),
                        uri: source.uri.clone(),
                        range,
                    }
                });
            }
            Statement::Dialogue { character_id, text } => {
                if let Some(character) = character_id {
                    semantic.character_uses.push(CharacterUse {
                        name: character.clone(),
                        uri: source.uri.clone(),
                        range: ranges.take_character_use(character).unwrap_or_else(|| {
                            find_dialogue_character_range(&source.text, character)
                        }),
                    });
                }
                collect_text_exprs(text, source, semantic);
            }
            Statement::Choice { options } => {
                for opt in options {
                    collect_text_exprs(&opt.label, source, semantic);
                    collect_block(source, ranges, &opt.body, semantic, diagnostics);
                }
            }
            Statement::If {
                condition,
                then_branch,
                else_branch,
            } => {
                collect_expr(condition, source, semantic);
                collect_block(source, ranges, then_branch, semantic, diagnostics);
                collect_block(source, ranges, else_branch, semantic, diagnostics);
            }
            Statement::Label { name } => {
                let range = ranges
                    .take_label(name)
                    .unwrap_or_else(|| find_label_name_range(&source.text, name));
                if let Some(existing) = semantic.labels.get(name) {
                    diagnostics.push(diagnostic_with_related(
                        range,
                        DiagnosticSeverity::ERROR,
                        "duplicate-label",
                        format!("duplicate label `{name}`"),
                        vec![DiagnosticRelatedInformation {
                            location: Location {
                                uri: existing.uri.clone(),
                                range: existing.range,
                            },
                            message: "first declaration is here".to_string(),
                        }],
                    ));
                } else {
                    semantic.labels.insert(
                        name.clone(),
                        LabelSymbol {
                            name: name.clone(),
                            uri: source.uri.clone(),
                            range,
                        },
                    );
                }
            }
            Statement::Jump { target } => semantic.label_refs.push(LabelRef {
                name: target.clone(),
                kind: "jump",
                uri: source.uri.clone(),
                range: ranges
                    .take_label_ref("jump", target)
                    .unwrap_or_else(|| find_command_target_range(&source.text, "jump", target)),
            }),
            Statement::Call { target } => semantic.label_refs.push(LabelRef {
                name: target.clone(),
                kind: "call",
                uri: source.uri.clone(),
                range: ranges
                    .take_label_ref("call", target)
                    .unwrap_or_else(|| find_command_target_range(&source.text, "call", target)),
            }),
            Statement::ShowSprite { character_id, .. }
            | Statement::HideSprite { character_id, .. }
            | Statement::MoveSprite { character_id, .. }
            | Statement::SpriteAnimate { character_id, .. }
            | Statement::SpriteStopAnimation { character_id } => {
                semantic.character_uses.push(CharacterUse {
                    name: character_id.clone(),
                    uri: source.uri.clone(),
                    range: ranges
                        .take_character_use(character_id)
                        .unwrap_or_else(|| find_character_usage_range(&source.text, character_id)),
                });
            }
            Statement::MethodCall { target, method, .. } if method == "show" => {
                semantic.character_uses.push(CharacterUse {
                    name: target.clone(),
                    uri: source.uri.clone(),
                    range: ranges
                        .take_character_use(target)
                        .unwrap_or_else(|| find_character_usage_range(&source.text, target)),
                });
            }
            Statement::SetVar { value, .. }
            | Statement::LocalVar { value, .. }
            | Statement::FunctionReturn { value } => collect_expr(value, source, semantic),
            Statement::UiOpen {
                name,
                arguments,
                modal,
                layer,
            } => {
                for value in [name, arguments, modal, layer] {
                    collect_expr(value, source, semantic);
                }
            }
            Statement::UiClose { name } => collect_expr(name, source, semantic),
            Statement::UiFocus { name, element } => {
                collect_expr(name, source, semantic);
                collect_expr(element, source, semantic);
            }
            Statement::UiSetState {
                name,
                element,
                state,
            } => {
                for value in [name, element, state] {
                    collect_expr(value, source, semantic);
                }
            }
            Statement::Imagemap { hotspots, .. } => {
                for hotspot in hotspots {
                    collect_block(source, ranges, &hotspot.body, semantic, diagnostics);
                }
            }
            _ => {}
        }
    }
}

fn validate_semantics(semantic: &SemanticIndex, index: &mut ProjectIndex) {
    for reference in &semantic.label_refs {
        if !semantic.labels.contains_key(&reference.name) {
            let suggestion =
                nearest_symbol(&reference.name, semantic.labels.keys().map(String::as_str));
            let mut message = format!(
                "{} target `{}` does not resolve to any label in the workspace",
                reference.kind, reference.name
            );
            if let Some(suggestion) = suggestion {
                message.push_str(&format!("; did you mean `{suggestion}`?"));
            }
            index
                .diagnostics
                .entry(reference.uri.clone())
                .or_default()
                .push(simple_diagnostic(
                    reference.range,
                    DiagnosticSeverity::ERROR,
                    "unknown-label-target",
                    message,
                ));
        }
    }

    for character_use in &semantic.character_uses {
        if !semantic.characters.contains_key(&character_use.name) {
            let suggestion = nearest_symbol(
                &character_use.name,
                semantic.characters.keys().map(String::as_str),
            );
            let mut message = format!("undefined character id `{}`", character_use.name);
            if let Some(suggestion) = suggestion {
                message.push_str(&format!("; did you mean `{suggestion}`?"));
            }
            message.push_str(&format!(
                "; if this is a new character, declare it with `character.create(\"{}\", \"Display Name\")`",
                character_use.name
            ));
            index
                .diagnostics
                .entry(character_use.uri.clone())
                .or_default()
                .push(simple_diagnostic(
                    character_use.range,
                    DiagnosticSeverity::ERROR,
                    "undefined-character",
                    message,
                ));
        }
    }
}

fn document_symbols(source: &SourceFile) -> Vec<RvnSymbol> {
    let mut symbols = Vec::new();
    for statement in &source.script {
        match statement {
            Statement::Function {
                name, parameters, ..
            }
            | Statement::Screen {
                name, parameters, ..
            }
            | Statement::Handler {
                name, parameters, ..
            } => {
                let keyword = match statement {
                    Statement::Screen { .. } => "screen",
                    Statement::Handler { .. } => "handler",
                    _ => "function",
                };
                let selection_range = find_command_target_range(&source.text, keyword, name);
                symbols.push(RvnSymbol {
                    name: name.clone(),
                    detail: Some(format!("{keyword}({})", parameters.join(", "))),
                    kind: SymbolKind::FUNCTION,
                    range: find_line_range(&source.text, |line| {
                        line.trim_start()
                            .strip_prefix(&format!("{keyword} "))
                            .is_some_and(|rest| {
                                rest.strip_prefix(name)
                                    .is_some_and(|rest| rest.starts_with('('))
                            })
                    }),
                    selection_range,
                });
            }
            Statement::Label { name } => {
                let range = find_label_statement_range(&source.text, name);
                let selection_range = find_label_name_range(&source.text, name);
                symbols.push(RvnSymbol {
                    name: name.clone(),
                    detail: Some("label".to_string()),
                    kind: SymbolKind::FUNCTION,
                    range,
                    selection_range,
                });
            }
            Statement::Init { .. } => {
                let range = find_keyword_statement_range(&source.text, "init");
                symbols.push(RvnSymbol {
                    name: "init".to_string(),
                    detail: Some("initialization".to_string()),
                    kind: SymbolKind::NAMESPACE,
                    range,
                    selection_range: range,
                });
            }
            Statement::Imagemap { .. } => {
                let range = find_keyword_statement_range(&source.text, "imagemap");
                symbols.push(RvnSymbol {
                    name: "imagemap".to_string(),
                    detail: Some("interactive map".to_string()),
                    kind: SymbolKind::OBJECT,
                    range,
                    selection_range: range,
                });
            }
            _ => {}
        }
    }
    symbols
}

fn parse_diagnostic(error: &ParseError) -> Diagnostic {
    let message = match &error.kind {
        ParseErrorKind::UnexpectedToken { got, expected } => {
            format!("invalid syntax: unexpected token `{got}`, expected {expected}")
        }
        ParseErrorKind::UnexpectedEof { expected } => {
            format!("unexpected end of file, expected {expected}")
        }
        ParseErrorKind::LexError { slice } => format!("unrecognized character: `{slice}`"),
        ParseErrorKind::InvalidAssignment { suggestion, .. } => {
            format!("invalid assignment; use `set`, for example `{suggestion}`")
        }
    };
    let code = match &error.kind {
        ParseErrorKind::InvalidAssignment { .. } => "invalid-assignment",
        _ => "parse",
    };
    simple_diagnostic(
        Range {
            start: Position {
                line: (error.location.line.saturating_sub(1)) as u32,
                character: (error.location.col.saturating_sub(1)) as u32,
            },
            end: Position {
                line: (error.location.line.saturating_sub(1)) as u32,
                character: (error.location.col.saturating_sub(1) + error.location.len) as u32,
            },
        },
        DiagnosticSeverity::ERROR,
        code,
        message,
    )
}

fn simple_diagnostic(
    range: Range,
    severity: DiagnosticSeverity,
    code: &'static str,
    message: String,
) -> Diagnostic {
    Diagnostic {
        range,
        severity: Some(severity),
        code: Some(NumberOrString::String(code.to_string())),
        code_description: None,
        source: Some("rvn-lsp".to_string()),
        message,
        related_information: None,
        tags: None,
        data: None,
    }
}

fn diagnostic_with_related(
    range: Range,
    severity: DiagnosticSeverity,
    code: &'static str,
    message: String,
    related_information: Vec<DiagnosticRelatedInformation>,
) -> Diagnostic {
    let mut diagnostic = simple_diagnostic(range, severity, code, message);
    diagnostic.related_information = Some(related_information);
    diagnostic
}

fn collect_text_exprs(
    text: &rvn_parser::InterpolatedText,
    source: &SourceFile,
    semantic: &mut SemanticIndex,
) {
    for segment in &text.0 {
        if let TextSegment::Interp(expr) = segment {
            collect_expr(expr, source, semantic);
        }
    }
}

fn collect_expr(_expr: &Expr, _source: &SourceFile, _semantic: &mut SemanticIndex) {
    // Variable diagnostics remain intentionally deferred to `rvn check`.
    // The parser already validates expression syntax for live LSP feedback.
}

fn completion_context(text: &str, position: Position) -> CompletionContext {
    let Some(line) = text.lines().nth(position.line as usize) else {
        return CompletionContext::General;
    };
    let character = crate::callables::byte_column(line, position.character);
    let prefix = &line[..character];
    let trimmed = prefix.trim_start();

    if trimmed.starts_with("jump ") || trimmed.starts_with("call ") {
        return CompletionContext::LabelTarget;
    }

    if let Some(dot) = prefix.rfind('.') {
        let after_dot = &prefix[dot + 1..];
        if !after_dot.chars().all(is_ident_char) {
            return CompletionContext::General;
        }
        let before_dot = &prefix[..dot];
        let target = before_dot
            .split(|ch: char| !is_ident_char(ch))
            .next_back()
            .unwrap_or_default();
        if !target.is_empty() {
            return CompletionContext::AfterDot(target.to_string());
        }
    }

    if trimmed.chars().all(is_ident_char) {
        return CompletionContext::LineStart {
            allow_characters: line.starts_with(char::is_whitespace)
                || has_previous_label(text, position.line as usize),
        };
    }

    CompletionContext::General
}

fn has_previous_label(text: &str, line_idx: usize) -> bool {
    text.lines().take(line_idx).any(|line| {
        line.trim_start()
            .strip_prefix("label ")
            .is_some_and(|rest| first_ident(rest).is_some())
    })
}

fn keyword_completions() -> Vec<CompletionItem> {
    RVN_KEYWORDS
        .iter()
        .filter_map(|keyword| {
            let snippet = keyword.snippet?;
            Some(CompletionItem {
                label: keyword.name.to_string(),
                kind: Some(CompletionItemKind::KEYWORD),
                detail: Some("RVN keyword".to_string()),
                documentation: Some(tower_lsp::lsp_types::Documentation::MarkupContent(
                    MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: keyword_markdown(*keyword),
                    },
                )),
                insert_text: Some(snippet.to_string()),
                insert_text_format: Some(tower_lsp::lsp_types::InsertTextFormat::SNIPPET),
                ..CompletionItem::default()
            })
        })
        .collect()
}

fn builtin_root_completions() -> Vec<CompletionItem> {
    BUILTIN_ROOTS
        .iter()
        .map(|builtin| CompletionItem {
            label: builtin.name.to_string(),
            kind: Some(CompletionItemKind::FUNCTION),
            detail: Some("RVN built-in".to_string()),
            documentation: Some(tower_lsp::lsp_types::Documentation::MarkupContent(
                MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: format!("**built-in** `{}`\n\n{}", builtin.name, builtin.summary),
                },
            )),
            ..CompletionItem::default()
        })
        .collect()
}

fn function_completions(index: &ProjectIndex) -> Vec<CompletionItem> {
    let mut functions: Vec<_> = index
        .functions
        .values()
        .filter(|function| function.declaration == "function")
        .collect();
    functions.sort_by(|a, b| a.name.cmp(&b.name));
    functions
        .into_iter()
        .map(|function| CompletionItem {
            label: function.name.clone(),
            kind: Some(CompletionItemKind::FUNCTION),
            detail: Some(format!(
                "{}({})",
                function.name,
                function.parameters.join(", ")
            )),
            insert_text: Some(format!(
                "{}({})",
                function.name,
                function
                    .parameters
                    .iter()
                    .enumerate()
                    .map(|(index, parameter)| format!("${{{}:{}}}", index + 1, parameter))
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
            insert_text_format: Some(tower_lsp::lsp_types::InsertTextFormat::SNIPPET),
            ..CompletionItem::default()
        })
        .collect()
}

fn character_completions(index: &ProjectIndex) -> Vec<CompletionItem> {
    let mut characters: Vec<_> = index.characters.values().collect();
    characters.sort_by(|a, b| a.id.cmp(&b.id));
    characters
        .into_iter()
        .map(|character| {
            let mut item = completion(&character.id, CompletionItemKind::VARIABLE, "character");
            item.documentation = Some(tower_lsp::lsp_types::Documentation::MarkupContent(
                MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: format!(
                        "**character id** `{}`\n\nDisplay name: `{}`",
                        character.id, character.display_name
                    ),
                },
            ));
            item
        })
        .collect()
}

fn method_completions(target: &str, index: &ProjectIndex) -> Vec<CompletionItem> {
    let Some(methods) = method_docs(target, index) else {
        return Vec::new();
    };

    methods
        .iter()
        .map(|method| CompletionItem {
            label: method.name.to_string(),
            kind: Some(CompletionItemKind::METHOD),
            detail: Some(format!("{target}.{}", method.name)),
            documentation: Some(tower_lsp::lsp_types::Documentation::MarkupContent(
                MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: format!(
                        "**method** `{}.{}`\n\n{}",
                        target, method.name, method.summary
                    ),
                },
            )),
            insert_text: Some(method.snippet.to_string()),
            insert_text_format: Some(tower_lsp::lsp_types::InsertTextFormat::SNIPPET),
            ..CompletionItem::default()
        })
        .collect()
}

fn completion(label: &str, kind: CompletionItemKind, detail: &str) -> CompletionItem {
    CompletionItem {
        label: label.to_string(),
        kind: Some(kind),
        detail: Some(format!("{label} — {detail}")),
        ..CompletionItem::default()
    }
}

fn keyword_doc(word: &str) -> Option<String> {
    RVN_KEYWORDS
        .iter()
        .copied()
        .find(|keyword| keyword.name == word)
        .map(keyword_markdown)
}

fn keyword_markdown(keyword: KeywordDoc) -> String {
    match keyword.usage {
        Some(usage) => format!(
            "**keyword** `{}`\n\n{}\n\nUsage: `{usage}`",
            keyword.name, keyword.summary
        ),
        None => format!("**keyword** `{}`\n\n{}", keyword.name, keyword.summary),
    }
}

fn builtin_doc(word: &str) -> Option<String> {
    BUILTIN_ROOTS
        .iter()
        .find(|builtin| builtin.name == word)
        .map(|builtin| format!("**built-in** `{}`\n\n{}", builtin.name, builtin.summary))
}

fn method_docs(target: &str, index: &ProjectIndex) -> Option<&'static [MethodDoc]> {
    match target {
        "accessibility"=>Some(&[
            MethodDoc{name:"configure",summary:"Sets validated project defaults; omitted properties reset to defaults. Player preferences take precedence.",snippet:"configure({\"text_scale\": ${1:1.25}, \"reduced_motion\": ${2:true}})"},
            MethodDoc{name:"speak",summary:"Speaks literal or localized text using an available system/browser voice. Transient speech is not replayed from saves.",snippet:"speak(\"${1:text}\")"},
            MethodDoc{name:"stop",summary:"Stops only this game's queued speech.",snippet:"stop()"},
        ]),
        "motion"=>Some(&[
            MethodDoc{name:"play",summary:"Starts or replaces a composable animation on an existing target.",snippet:"play(\"${1:sprite:iris}\", ${2:animation})"},
            MethodDoc{name:"stop",summary:"Removes the animation contribution and restores the target's ordinary appearance.",snippet:"stop(\"${1:sprite:iris}\")"},
            MethodDoc{name:"wait",summary:"Waits for a finite animation. Not permitted in a non-blocking handler.",snippet:"wait(\"${1:sprite:iris}\")"},
        ]),
        "video"=>Some(&[
            MethodDoc{name:"play",summary:"Starts a named video player with a validated clip definition.",snippet:"play(\"${1:intro}\", ${2:clip})"},
            MethodDoc{name:"pause",summary:"Pauses playback without losing its position.",snippet:"pause(\"${1:intro}\")"},
            MethodDoc{name:"resume",summary:"Resumes paused playback; browser autoplay restrictions may require a gesture.",snippet:"resume(\"${1:intro}\")"},
            MethodDoc{name:"stop",summary:"Closes the player and its audio.",snippet:"stop(\"${1:intro}\")"},
            MethodDoc{name:"skip",summary:"Skips only a clip that explicitly allows it.",snippet:"skip(\"${1:intro}\")"},
            MethodDoc{name:"wait",summary:"Waits for a finite player's end. Not permitted in event handlers.",snippet:"wait(\"${1:intro}\")"},
            MethodDoc{name:"seek",summary:"Seeks to a finite position while retaining the paused/playing state.",snippet:"seek(\"${1:intro}\", ${2:0.8})"},
            MethodDoc{name:"volume",summary:"Sets this clip's volume between 0 and 1.",snippet:"volume(\"${1:intro}\", ${2:0.5})"},
        ]),
        "ui"=>Some(&[
            MethodDoc{name:"open",summary:"Opens or replaces a named screen with parameters, modality and display layer.",snippet:"open(\"${1:screen}\", [${2:arguments}], ${3:true}, ${4:1})"},
            MethodDoc{name:"close",summary:"Closes a screen and invokes its close handler once.",snippet:"close(\"${1:screen}\")"},
            MethodDoc{name:"focus",summary:"Focuses an available input or button. The game reveals it in scrolling containers.",snippet:"focus(\"${1:screen}\", \"${2:component}\")"},
        ]),
        "music" => Some(&[
            MethodDoc {
                name: "play",
                summary: "Starts background music.",
                snippet: "play(\"${1:file.ogg}\")",
            },
            MethodDoc {
                name: "stop",
                summary: "Stops the current background music.",
                snippet: "stop()",
            },
            MethodDoc {
                name: "volume",
                summary: "Sets the background music volume from 0.0 to 1.0.",
                snippet: "volume(${1:0.5})",
            },
        ]),
        "sfx" => Some(&[
            MethodDoc {
                name: "play",
                summary: "Plays a sound effect.",
                snippet: "play(\"${1:file.ogg}\")",
            },
            MethodDoc {
                name: "stop",
                summary: "Stops a sound effect by file name.",
                snippet: "stop(\"${1:file.ogg}\")",
            },
        ]),
        "typewriter" => Some(&[MethodDoc {
            name: "speed",
            summary: "Sets the typewriter speed in characters per second.",
            snippet: "speed(${1:42})",
        }]),
        "character" => Some(&[MethodDoc{name:"compose",summary:"Assigns a layered image definition to an existing character.",snippet:"compose(\"${1:iris}\", ${2:composition})"},MethodDoc{name:"attributes",summary:"Changes only the specified composition attributes; unrelated groups are preserved.",snippet:"attributes(\"${1:iris}\", {\"${2:expression}\": \"${3:happy}\"})"},MethodDoc {
            name: "create",
            summary: "Declares a character id and display name.",
            snippet: "create(\"${1:id}\", \"${2:Display Name}\")",
        }]),
        other if index.characters.contains_key(other) => Some(&[
            MethodDoc {
                name: "show",
                summary: "Shows this character sprite.",
                snippet: "show(\"${1:neutral}\")",
            },
            MethodDoc {
                name: "move",
                summary: "Moves this character sprite.",
                snippet: "move()",
            },
            MethodDoc {
                name: "hide",
                summary: "Hides this character sprite.",
                snippet: "hide()",
            },
            MethodDoc {
                name: "animate",
                summary: "Starts a sprite animation for this character.",
                snippet: "animate(\"${1:shake}\", loop: ${2:true})",
            },
            MethodDoc {
                name: "stop_animation",
                summary: "Stops the current sprite animation for this character.",
                snippet: "stop_animation()",
            },
        ]),
        _ => None,
    }
}

fn method_hover(text: &str, position: Position, index: &ProjectIndex) -> Option<String> {
    let line = text.lines().nth(position.line as usize)?;
    let character = crate::callables::byte_column(line, position.character);
    let word = word_at(text, position)?;
    let word_start = character.saturating_sub(
        line[..character]
            .chars()
            .rev()
            .take_while(|ch| is_ident_char(*ch))
            .count(),
    );
    let before_word = &line[..word_start];
    let before_dot = before_word.strip_suffix('.')?;
    let target = before_dot
        .split(|ch: char| !is_ident_char(ch))
        .next_back()
        .unwrap_or_default();
    let method = method_docs(target, index)?
        .iter()
        .find(|method| method.name == word)?;
    Some(format!(
        "**method** `{}.{}`\n\n{}",
        target, method.name, method.summary
    ))
}

fn hover_markdown(markdown: impl Into<String>) -> Hover {
    Hover {
        contents: hover_markdown_contents(markdown),
        range: None,
    }
}

fn hover_markdown_contents(markdown: impl Into<String>) -> HoverContents {
    HoverContents::Markup(MarkupContent {
        kind: MarkupKind::Markdown,
        value: markdown.into(),
    })
}

fn display_uri(uri: &Url) -> String {
    uri.to_file_path()
        .ok()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| uri.to_string())
}

pub fn label_reference_at(text: &str, position: Position) -> Option<String> {
    let line = text.lines().nth(position.line as usize)?;
    let character = crate::callables::byte_column(line, position.character);
    let bytes = line.as_bytes();
    let mut start = character.min(bytes.len());
    while start > 0 && is_ident_byte(bytes[start - 1]) {
        start -= 1;
    }
    let mut end = character.min(bytes.len());
    while end < bytes.len() && is_ident_byte(bytes[end]) {
        end += 1;
    }
    if start == end {
        return None;
    }
    let word = &line[start..end];
    let prefix = line[..start].trim_end();
    if prefix.ends_with("jump") || prefix.ends_with("call") {
        Some(word.to_string())
    } else {
        None
    }
}

fn is_ident_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn is_ident_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

fn range_contains(range: Range, position: Position) -> bool {
    (position.line > range.start.line
        || (position.line == range.start.line && position.character >= range.start.character))
        && (position.line < range.end.line
            || (position.line == range.end.line && position.character <= range.end.character))
}

fn word_at(text: &str, position: Position) -> Option<String> {
    let line = text.lines().nth(position.line as usize)?;
    let character = crate::callables::byte_column(line, position.character);
    let bytes = line.as_bytes();
    let mut start = character;
    while start > 0 && is_ident_byte(bytes[start - 1]) {
        start -= 1;
    }
    let mut end = character;
    while end < bytes.len() && is_ident_byte(bytes[end]) {
        end += 1;
    }
    (start < end).then(|| line[start..end].to_string())
}

fn is_character_like_at(text: &str, position: Position) -> bool {
    let Some(line) = text.lines().nth(position.line as usize) else {
        return false;
    };
    let Some(word) = word_at(text, position) else {
        return false;
    };
    let trimmed = line.trim_start();
    if !trimmed.starts_with(&word) {
        return false;
    }

    let rest = &trimmed[word.len()..];
    if rest.trim_start().starts_with('=') {
        return false;
    }

    rest.starts_with(' ') || rest.starts_with('.')
}

fn find_label_statement_range(text: &str, name: &str) -> Range {
    find_line_range(text, |line| {
        line.trim_start()
            .strip_prefix("label ")
            .is_some_and(|rest| rest.split_whitespace().next() == Some(name))
    })
}

fn find_label_name_range(text: &str, name: &str) -> Range {
    find_token_after_keyword(text, "label", name)
        .unwrap_or_else(|| find_identifier_range(text, name))
}

fn find_command_target_range(text: &str, command: &str, target: &str) -> Range {
    find_token_after_keyword(text, command, target)
        .unwrap_or_else(|| find_identifier_range(text, target))
}

fn find_keyword_statement_range(text: &str, keyword: &str) -> Range {
    find_line_range(text, |line| line.trim_start().starts_with(keyword))
}

fn find_string_literal_range(text: &str, value: &str) -> Range {
    find_identifier_range(text, value)
}

fn find_character_declaration_range(text: &str, id: &str) -> Range {
    find_string_literal_range(text, id)
}

fn find_dialogue_character_range(text: &str, id: &str) -> Range {
    find_line_token_range(text, id, |line| {
        let trimmed = line.trim_start();
        trimmed.starts_with(id) && trimmed[id.len()..].starts_with(' ')
    })
    .unwrap_or_else(|| find_identifier_range(text, id))
}

fn find_character_usage_range(text: &str, id: &str) -> Range {
    find_line_token_range(text, id, |line| {
        let trimmed = line.trim_start();
        trimmed.starts_with(id) && trimmed[id.len()..].starts_with('.')
    })
    .unwrap_or_else(|| find_dialogue_character_range(text, id))
}

fn first_ident(text: &str) -> Option<&str> {
    let end = text
        .char_indices()
        .take_while(|(_, ch)| is_ident_char(*ch))
        .map(|(idx, ch)| idx + ch.len_utf8())
        .last()
        .unwrap_or(0);
    (end > 0).then(|| &text[..end])
}

fn character_create_id_range(line_idx: usize, line: &str) -> Option<Range> {
    let call = "character.create(";
    let start = line.find(call)? + call.len();
    let rest = &line[start..];
    let rest = rest.strip_prefix('"')?;
    let id_len = rest.find('"')?;
    Some(range(line_idx, start + 1, start + 1 + id_len))
}

fn character_line_head(line_idx: usize, line: &str) -> Option<(&str, Range)> {
    let indent = line.len() - line.trim_start().len();
    let trimmed = &line[indent..];
    let id = first_ident(trimmed)?;
    if id == "character" {
        return None;
    }
    let after = trimmed[id.len()..].chars().next()?;
    if after == ' ' || after == '.' {
        Some((id, range(line_idx, indent, indent + id.len())))
    } else {
        None
    }
}

fn find_identifier_range(text: &str, ident: &str) -> Range {
    for (line_idx, line) in text.lines().enumerate() {
        if let Some(col) = line.find(ident) {
            return range(line_idx, col, col + ident.len());
        }
    }
    Range::default()
}

fn find_line_token_range(
    text: &str,
    token: &str,
    predicate: impl Fn(&str) -> bool,
) -> Option<Range> {
    for (line_idx, line) in text.lines().enumerate() {
        if predicate(line) {
            let col = line.find(token)?;
            return Some(range(line_idx, col, col + token.len()));
        }
    }
    None
}

fn find_token_after_keyword(text: &str, keyword: &str, token: &str) -> Option<Range> {
    for (line_idx, line) in text.lines().enumerate() {
        let trimmed_start = line.len() - line.trim_start().len();
        let Some(rest) = line[trimmed_start..].strip_prefix(keyword) else {
            continue;
        };
        let token_col = trimmed_start + keyword.len() + rest.find(token)?;
        return Some(range(line_idx, token_col, token_col + token.len()));
    }
    None
}

fn nearest_symbol<'a>(needle: &str, candidates: impl Iterator<Item = &'a str>) -> Option<String> {
    let mut best: Option<(&str, usize)> = None;
    for candidate in candidates {
        let distance = levenshtein(needle, candidate);
        let max_len = needle.len().max(candidate.len()).max(1);
        if (distance <= 2 || distance * 3 <= max_len)
            && best
                .map(|(_, best_distance)| distance < best_distance)
                .unwrap_or(true)
        {
            best = Some((candidate, distance));
        }
    }
    best.map(|(candidate, _)| candidate.to_string())
}

fn levenshtein(a: &str, b: &str) -> usize {
    let b_len = b.chars().count();
    let mut costs: Vec<_> = (0..=b_len).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut previous = costs[0];
        costs[0] = i + 1;
        for (j, cb) in b.chars().enumerate() {
            let current = costs[j + 1];
            costs[j + 1] = if ca == cb {
                previous
            } else {
                1 + previous.min(current).min(costs[j])
            };
            previous = current;
        }
    }
    costs[b_len]
}

fn find_line_range(text: &str, predicate: impl Fn(&str) -> bool) -> Range {
    for (line_idx, line) in text.lines().enumerate() {
        if predicate(line) {
            return range(line_idx, 0, line.len().max(1));
        }
    }
    Range::default()
}

fn range(line: usize, start: usize, end: usize) -> Range {
    Range {
        start: Position {
            line: line as u32,
            character: start as u32,
        },
        end: Position {
            line: line as u32,
            character: end as u32,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uri(name: &str) -> Url {
        Url::from_file_path(format!("/tmp/{name}.rvn")).unwrap()
    }

    #[test]
    fn callable_definitions_references_and_safe_renames_work_across_files() {
        let main = uri("call-main");
        let helpers = uri("call-helpers");
        let documents=HashMap::from([(main.clone(),"use \"call-helpers.rvn\"\nlabel start\nset score=reward(2)\nui.open(\"inventory\",[],true,1)\n".into()),(helpers.clone(),"function reward(n) { return n+1 }\nscreen inventory() { return component(\"root\",\"text\",{},[]) }".into())]);
        let index = analyze_workspace(&[], &documents).index;
        let symbol = index.symbol_at(&main, Position::new(2, 12)).unwrap();
        assert_eq!(symbol.name, "reward");
        assert_eq!(index.definition_for(&symbol).unwrap().uri, helpers);
        assert_eq!(index.references_for(&symbol).len(), 2);
        let edits = index
            .rename_edit(&symbol, "points")
            .unwrap()
            .changes
            .unwrap();
        assert_eq!(edits.values().map(Vec::len).sum::<usize>(), 2);
        assert!(index.rename_edit(&symbol, "inventory").is_none());
        let screen = index.symbol_at(&main, Position::new(3, 12)).unwrap();
        assert_eq!(index.references_for(&screen).len(), 2);
    }

    #[test]
    fn live_logic_diagnostics_point_only_at_the_invalid_call() {
        let file = uri("call-errors");
        let documents=HashMap::from([(file.clone(),"function reward(n) { return n }\nscreen inventory() { return component(\"root\",\"text\",{},[]) }\nlabel start\nset a=reward(1)\nset b=reward(1,2)\nset c=missing()\nui.open(\"inventory\",[1],true,1)\nui.close(\"unknown\")".into())]);
        let index = analyze_workspace(&[], &documents).index;
        let errors = index.diagnostics_for(&file);
        let arity: Vec<_> = errors
            .iter()
            .filter(|problem| diagnostic_code(problem) == Some("function-arity"))
            .collect();
        assert_eq!(arity.len(), 1);
        assert_eq!(arity[0].range.start.line, 4);
        assert!(errors.iter().any(
            |problem| diagnostic_code(problem) == Some("unknown-function")
                && problem.range.start.line == 5
        ));
        assert!(errors
            .iter()
            .any(|problem| diagnostic_code(problem) == Some("screen-arity")
                && problem.range.start.line == 6));
        assert!(errors
            .iter()
            .any(|problem| diagnostic_code(problem) == Some("unknown-screen")
                && problem.range.start.line == 7));
    }

    #[test]
    fn escaped_interpolation_and_dynamic_screen_names_never_receive_partial_renames() {
        let file = uri("unsafe-renames");
        let documents=HashMap::from([(file,"function reward(n) { return n }\nscreen inventory() { return component(\"root\",\"text\",{},[]) }\nlabel start\n\"Reward [reward(1)]\"\nui.open(screen_name,[],true,1)".into())]);
        let index = analyze_workspace(&[], &documents).index;
        for name in ["reward", "inventory"] {
            assert!(index
                .rename_edit(
                    &ResolvedSymbol {
                        kind: RvnSymbolKind::Callable,
                        name: name.into()
                    },
                    "changed"
                )
                .is_none());
        }
    }

    #[test]
    fn custom_canvas_callbacks_are_renamed_completely_and_dynamic_callbacks_are_protected() {
        let file = uri("canvas-callbacks");
        let source="function paint(s,p,f){return []}\nscreen one(){return component(\"one\",\"canvas\",{\"draw\":\"paint\"},[])}\nscreen two(){return {\"id\":\"two\",\"kind\":\"canvas\",\"draw\":\"paint\"}}\nlabel start\nreturn";
        let symbol = ResolvedSymbol {
            kind: RvnSymbolKind::Callable,
            name: "paint".into(),
        };
        let documents = HashMap::from([(file.clone(), source.into())]);
        let index = analyze_workspace(&[], &documents).index;
        let edit = index.rename_edit(&symbol, "redraw").unwrap();
        assert_eq!(edit.changes.unwrap()[&file].len(), 3);
        let dynamic=format!("{source}\nscreen dynamic(callback){{return component(\"dynamic\",\"canvas\",{{\"draw\":callback}},[])}}");
        let index = analyze_workspace(&[], &HashMap::from([(file.clone(), dynamic)])).index;
        assert!(index.rename_edit(&symbol, "redraw").is_none());
        let wrong = source.replace("paint(s,p,f)", "paint(s)");
        let index = analyze_workspace(&[], &HashMap::from([(file.clone(), wrong)])).index;
        let errors = index.diagnostics_for(&file);
        let errors: Vec<_> = errors
            .iter()
            .filter(|diagnostic| diagnostic_code(diagnostic) == Some("canvas-draw-arity"))
            .collect();
        assert_eq!(errors.len(), 2);
        assert!(errors
            .iter()
            .all(|diagnostic| matches!(diagnostic.range.start.line, 1 | 2)));
    }

    #[test]
    fn reports_unknown_label_and_undefined_character() {
        let uri = uri("main");
        let mut documents = HashMap::new();
        documents.insert(
            uri.clone(),
            r#"
label start
    eileen "Bonjour"
    jump missing
"#
            .to_string(),
        );

        let analysis = analyze_workspace(&[], &documents);
        let diagnostics = analysis.index.diagnostics_for(&uri);
        assert!(diagnostics
            .iter()
            .any(|d| diagnostic_code(d) == Some("unknown-label-target")));
        assert!(diagnostics
            .iter()
            .any(|d| diagnostic_code(d) == Some("undefined-character")));
    }

    #[test]
    fn indexes_labels_and_document_symbols() {
        let uri = uri("main");
        let mut documents = HashMap::new();
        documents.insert(
            uri.clone(),
            r#"
init {
    character.create("eileen", "Eileen")
}

label start
    call next

label next
    return
"#
            .to_string(),
        );

        let analysis = analyze_workspace(&[], &documents);
        assert!(analysis.index.labels.contains_key("start"));
        assert!(analysis.index.labels.contains_key("next"));
        let symbols = analysis.index.symbols_for(&uri);
        assert_eq!(symbols.iter().filter(|s| s.name == "start").count(), 1);
        assert_eq!(symbols.iter().filter(|s| s.name == "next").count(), 1);
    }

    #[test]
    fn detects_label_reference_at_position() {
        let text = "    jump target_label\n";
        let target = label_reference_at(
            text,
            Position {
                line: 0,
                character: 12,
            },
        );
        assert_eq!(target.as_deref(), Some("target_label"));
    }

    #[test]
    fn completion_suggests_labels_characters_and_methods() {
        let uri = uri("main");
        let mut documents = HashMap::new();
        let text = r#"
init {
    character.create("eileen", "Eileen")
}

label start
    jump 
    music.
    eileen.
ei
"#;
        documents.insert(uri.clone(), text.to_string());
        let analysis = analyze_workspace(&[], &documents);

        let jump_items = analysis.index.completion_items(
            text,
            Position {
                line: 6,
                character: 9,
            },
        );
        assert!(jump_items.iter().any(|item| item.label == "start"));

        let top_level_items = analysis.index.completion_items(
            text,
            Position {
                line: 0,
                character: 0,
            },
        );
        assert!(top_level_items.iter().any(|item| item.label == "label"));
        assert!(top_level_items.iter().any(|item| item.label == "scene"));
        assert!(top_level_items.iter().any(|item| item.label == "cinematic"));
        assert!(top_level_items
            .iter()
            .any(|item| item.label == "unlock_ending"));
        assert!(top_level_items.iter().any(|item| item.label == "music"));
        assert!(!top_level_items.iter().any(|item| item.label == "eileen"));

        let statement_items = analysis.index.completion_items(
            text,
            Position {
                line: 6,
                character: 4,
            },
        );
        assert!(statement_items.iter().any(|item| item.label == "eileen"));

        let unindented_label_body_items = analysis.index.completion_items(
            text,
            Position {
                line: 9,
                character: 2,
            },
        );
        assert!(unindented_label_body_items
            .iter()
            .any(|item| item.label == "eileen"));

        let music_items = analysis.index.completion_items(
            text,
            Position {
                line: 7,
                character: 10,
            },
        );
        assert!(music_items.iter().any(|item| item.label == "play"));
        assert!(music_items.iter().any(|item| item.label == "stop"));

        let character_items = analysis.index.completion_items(
            text,
            Position {
                line: 8,
                character: 11,
            },
        );
        assert!(character_items.iter().any(|item| item.label == "show"));
        assert!(character_items.iter().any(|item| item.label == "hide"));
    }

    #[test]
    fn computation_functions_collections_and_loops_have_editor_assistance() {
        let file = uri("computation");
        let text = "function total(items) {\nset result = 0\nfor item in items { set result = result + item }\nreturn result\n}\nlabel start\nset score = total([1, 2])\n\"[score]\"\n";
        let analysis = analyze_workspace(&[], &HashMap::from([(file.clone(), text.into())]));
        assert!(analysis.index.diagnostics_for(&file).is_empty());
        let items = analysis.index.completion_items(
            text,
            Position {
                line: 6,
                character: 12,
            },
        );
        for name in [
            "total",
            "function",
            "for",
            "while",
            "dict_get",
            "list_append",
            "random",
        ] {
            assert!(
                items.iter().any(|item| item.label == name),
                "missing completion {name}"
            );
        }
        let total = items.iter().find(|item| item.label == "total").unwrap();
        assert_eq!(total.insert_text.as_deref(), Some("total(${1:items})"));
        assert!(analysis
            .index
            .symbols_for(&file)
            .iter()
            .any(|symbol| symbol.name == "total" && symbol.kind == SymbolKind::FUNCTION));
        assert!(analysis
            .index
            .hover(
                text,
                &file,
                Position {
                    line: 6,
                    character: 14
                }
            )
            .is_some());
    }

    #[test]
    fn screen_declarations_are_not_offered_as_expression_calls() {
        let file = uri("screens");
        let text="screen form() { return component(\"root\",\"column\",{},[]) }\nhandler clicked(event) {}\nfunction total(n) {return n}\nlabel start\nui.open(\"form\",[],true,1)\n";
        let analysis = analyze_workspace(&[], &HashMap::from([(file, text.into())]));
        let items = analysis.index.completion_items(text, Position::new(4, 0));
        for keyword in ["screen", "handler", "local", "ui", "total"] {
            assert!(items.iter().any(|item| item.label == keyword));
        }
        for declaration in ["form", "clicked"] {
            assert!(!items.iter().any(|item| item.label == declaration));
        }
        let methods = analysis.index.completion_items("ui.", Position::new(0, 3));
        for method in ["open", "close", "focus"] {
            assert!(methods.iter().any(|item| item.label == method));
        }
    }

    #[test]
    fn advanced_authoring_builtins_have_matching_completion_hover_and_parser_registry() {
        let index = ProjectIndex::default();
        let file = uri("advanced-authoring-help");
        let items = index.completion_items("", Position::new(0, 0));
        for (name, arity, help) in [
            ("motion_spline", 3..=3, "Catmull–Rom"),
            ("motion_bezier", 4..=4, "cubic timing"),
            ("motion_curve", 2..=2, "2–257"),
            ("image_layers", 2..=2, "drawing order"),
            ("layered_image", 3..=4, "optional fourth"),
        ] {
            assert_eq!(rvn_parser::builtin_arity(name), Some(arity));
            let matching = items
                .iter()
                .filter(|item| item.label == name)
                .collect::<Vec<_>>();
            assert_eq!(matching.len(), 1, "missing or duplicate builtin {name}");
            assert_eq!(matching[0].kind, Some(CompletionItemKind::FUNCTION));
            let Some(tower_lsp::lsp_types::Documentation::MarkupContent(documentation)) =
                &matching[0].documentation
            else {
                panic!("missing completion help for {name}");
            };
            assert!(
                documentation.value.contains(help),
                "incorrect completion help for {name}"
            );
            let hover = index.hover(name, &file, Position::new(0, 1));
            assert!(
                hover_contains(&hover, help),
                "incorrect hover help for {name}"
            );
        }
    }

    #[test]
    fn advanced_authoring_expression_calls_and_legacy_layered_image_are_known() {
        let file = uri("advanced-authoring-builtins");
        let text = r#"function progress(t) {return t}
function portrait() {
    local layers=image_layers("iris",["iris__body.png"])
    local path=motion_spline(1,[{"x":0,"y":0},{"x":100,"y":0}],motion_curve("progress",65))
    local timing=motion_bezier(0.25,0.1,0.75,0.9)
    local legacy=layered_image([600,1000],{},layers)
    return layered_image([600,1000],{},layers,{"variants":{}})
}
"#;
        let analysis = analyze_workspace(&[], &HashMap::from([(file.clone(), text.into())]));
        assert!(
            analysis.index.diagnostics_for(&file).is_empty(),
            "{:?}",
            analysis.index.diagnostics_for(&file)
        );
    }

    #[test]
    fn duplicate_calculation_functions_are_diagnosed_across_files() {
        let first = uri("first-function");
        let second = uri("second-function");
        let documents = HashMap::from([
            (first, "function twice(n) { return n * 2 }".into()),
            (second.clone(), "function twice(n) { return n + n }".into()),
        ]);
        let analysis = analyze_workspace(&[], &documents);
        assert!(analysis
            .index
            .diagnostics_for(&second)
            .iter()
            .any(|diagnostic| diagnostic.code
                == Some(NumberOrString::String("duplicate-function".into()))));
    }

    #[test]
    fn fuzzy_suggestions_are_reported() {
        let uri = uri("main");
        let mut documents = HashMap::new();
        documents.insert(
            uri.clone(),
            r#"
init {
    character.create("eileen", "Eileen")
}

label exploration
    eiileen "Bonjour"
    jump exploratoin
"#
            .to_string(),
        );

        let analysis = analyze_workspace(&[], &documents);
        let diagnostics = analysis.index.diagnostics_for(&uri);
        assert!(diagnostics.iter().any(|d| d.message.contains("eileen")));
        assert!(diagnostics
            .iter()
            .any(|d| d.message.contains("exploration")));
    }

    #[test]
    fn hover_reports_label_character_and_keyword_help() {
        let uri = uri("main");
        let mut documents = HashMap::new();
        let text = r#"
init {
    character.create("eileen", "Eileen")
}

label start
    jump start
    scene "backgrounds/title_forest.png" with fade
    music.play("theme.ogg")
    typewriter.speed(42)
    eileen "Bonjour"
    choice {
    }
"#;
        documents.insert(uri.clone(), text.to_string());
        let analysis = analyze_workspace(&[], &documents);

        let label_hover = analysis.index.hover(
            text,
            &uri,
            Position {
                line: 6,
                character: 11,
            },
        );
        assert!(hover_contains(&label_hover, "label"));

        let character_hover = analysis.index.hover(
            text,
            &uri,
            Position {
                line: 10,
                character: 6,
            },
        );
        assert!(hover_contains(&character_hover, "Display name"));

        let keyword_hover = analysis.index.hover(
            text,
            &uri,
            Position {
                line: 11,
                character: 7,
            },
        );
        assert!(hover_contains(&keyword_hover, "branching options"));

        let scene_hover = analysis.index.hover(
            text,
            &uri,
            Position {
                line: 7,
                character: 6,
            },
        );
        assert!(hover_contains(&scene_hover, "**keyword** `scene`"));
        assert!(hover_contains(
            &scene_hover,
            "Sets the active background scene"
        ));

        let music_hover = analysis.index.hover(
            text,
            &uri,
            Position {
                line: 8,
                character: 6,
            },
        );
        assert!(hover_contains(&music_hover, "**built-in** `music`"));

        let typewriter_method_hover = analysis.index.hover(
            text,
            &uri,
            Position {
                line: 9,
                character: 17,
            },
        );
        assert!(hover_contains(&typewriter_method_hover, "typewriter.speed"));
    }

    #[test]
    fn hover_does_not_treat_invalid_assignment_as_character() {
        let uri = uri("main");
        let mut documents = HashMap::new();
        let text = r#"
label start
var = 5
"#;
        documents.insert(uri.clone(), text.to_string());
        let analysis = analyze_workspace(&[], &documents);

        let hover = analysis.index.hover(
            text,
            &uri,
            Position {
                line: 2,
                character: 1,
            },
        );
        assert!(!hover_contains(&hover, "unknown character"));
    }

    #[test]
    fn references_labels_across_documents() {
        let main_uri = uri("main");
        let other_uri = uri("other");
        let mut documents = HashMap::new();
        documents.insert(
            main_uri.clone(),
            "label exploration\n    jump exploration\n".to_string(),
        );
        documents.insert(
            other_uri.clone(),
            "label cinematic_demo\n    call exploration\n".to_string(),
        );

        let analysis = analyze_workspace(&[], &documents);
        let symbol = analysis
            .index
            .symbol_at(
                &main_uri,
                Position {
                    line: 0,
                    character: 8,
                },
            )
            .unwrap();
        let references = analysis.index.references_for(&symbol);
        assert_eq!(references.len(), 3);
        assert!(references.iter().any(|location| {
            location.uri == main_uri
                && location.range == range(0, "label ".len(), "label exploration".len())
        }));
        assert!(references
            .iter()
            .any(|location| location.uri == other_uri && location.range.start.line == 1));
    }

    #[test]
    fn rename_labels_uses_semantic_ranges_only() {
        let main_uri = uri("main");
        let other_uri = uri("other");
        let mut documents = HashMap::new();
        documents.insert(
            main_uri.clone(),
            "label exploration\n    jump exploration\n".to_string(),
        );
        documents.insert(other_uri.clone(), "    call exploration\n".to_string());

        let analysis = analyze_workspace(&[], &documents);
        let symbol = ResolvedSymbol {
            kind: RvnSymbolKind::Label,
            name: "exploration".to_string(),
        };
        let edit = analysis.index.rename_edit(&symbol, "explore_area").unwrap();
        let changes = edit.changes.unwrap();
        assert_eq!(changes.values().map(Vec::len).sum::<usize>(), 3);
        assert!(changes
            .get(&main_uri)
            .unwrap()
            .iter()
            .all(|edit| edit.new_text == "explore_area"));
        assert!(analysis.index.rename_edit(&symbol, "bad-name").is_none());
    }

    #[test]
    fn references_and_rename_characters() {
        let main_uri = uri("main");
        let mut documents = HashMap::new();
        documents.insert(
            main_uri.clone(),
            r#"init {
    character.create("eileen", "Eileen")
}

label start
    eileen "Bonjour"
    eileen.show("neutral")
    eileen.move()
"#
            .to_string(),
        );

        let analysis = analyze_workspace(&[], &documents);
        let symbol = analysis
            .index
            .symbol_at(
                &main_uri,
                Position {
                    line: 1,
                    character: 23,
                },
            )
            .unwrap();
        assert_eq!(symbol.kind, RvnSymbolKind::Character);
        let references = analysis.index.references_for(&symbol);
        assert_eq!(references.len(), 4);

        let edit = analysis.index.rename_edit(&symbol, "guide").unwrap();
        assert_eq!(edit.changes.unwrap().get(&main_uri).unwrap().len(), 4);
    }

    #[test]
    fn suggestions_are_available_for_quick_fixes() {
        let main_uri = uri("main");
        let mut documents = HashMap::new();
        documents.insert(
            main_uri.clone(),
            r#"init {
    character.create("eileen", "Eileen")
}

label exploration
    eiileen "Bonjour"
    jump exploratoin
"#
            .to_string(),
        );

        let analysis = analyze_workspace(&[], &documents);
        assert_eq!(
            analysis.index.suggested_label("exploratoin").as_deref(),
            Some("exploration")
        );
        assert_eq!(
            analysis.index.suggested_character("eiileen").as_deref(),
            Some("eileen")
        );
    }

    fn diagnostic_code(diagnostic: &Diagnostic) -> Option<&str> {
        match diagnostic.code.as_ref()? {
            tower_lsp::lsp_types::NumberOrString::String(code) => Some(code.as_str()),
            tower_lsp::lsp_types::NumberOrString::Number(_) => None,
        }
    }

    fn hover_contains(hover: &Option<Hover>, needle: &str) -> bool {
        match hover.as_ref().map(|hover| &hover.contents) {
            Some(HoverContents::Markup(content)) => content.value.contains(needle),
            Some(HoverContents::Scalar(tower_lsp::lsp_types::MarkedString::String(value))) => {
                value.contains(needle)
            }
            _ => false,
        }
    }
}
