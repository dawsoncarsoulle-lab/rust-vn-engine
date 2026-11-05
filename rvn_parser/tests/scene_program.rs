use rvn_parser::{
    parse, parse_file_program_with_uses, parse_file_with_uses, parse_for_context_discovery,
    parse_program, parse_recovering, parse_recovering_with_context, parse_spanned,
    parse_spanned_with_context, parse_with_context, parse_with_origins, Expr, NarrativeContext,
    SourceDocument, Statement, StatementSpan,
};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(1);

struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "rvn-c1-étoile-{}-{nonce}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn write(&self, name: &str, source: &str) -> PathBuf {
        let path = self.0.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, source).unwrap();
        path
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn scene_entry_helper_and_background_are_distinct_without_implicit_legacy_opt_in() {
    let source = "scene label intro\nlabel helper\nscene \"label\"\nreturn\n";
    let script = parse(source).unwrap();
    assert!(matches!(&script[0], Statement::SceneLabel { name } if name == "intro"));
    assert!(matches!(&script[1], Statement::Label { name } if name == "helper"));
    assert!(matches!(&script[2], Statement::Scene { background, .. } if background == "label"));
    assert_eq!(
        NarrativeContext::from_script(&script),
        NarrativeContext::ExplicitScenes
    );
    assert_eq!(parse_program(source).unwrap().script, script);
    assert_eq!(
        NarrativeContext::from_script(&parse("label scene_intro\nscene forest\n").unwrap()),
        NarrativeContext::Legacy
    );
    assert!(parse("scene label intro\nlocal count = 1\n").is_err());
}

#[test]
fn scene_entries_are_root_only_in_every_structured_body() {
    for source in [
        "init { scene label bad }",
        "function f() { scene label bad }",
        "screen f() { scene label bad }",
        "handler f() { scene label bad }",
        "if true { scene label bad }",
        "if true {} else { scene label bad }",
        "while true { scene label bad }",
        "for item in [1] { scene label bad }",
        "choice { \"go\" => { scene label bad } }",
        "imagemap { background: \"map.png\" hotspot { area: (0,0,20,20) } => { scene label bad } }",
    ] {
        let full = format!("scene label good\n{source}\n");
        assert!(
            parse_with_context(&full, NarrativeContext::ExplicitScenes).is_err(),
            "{source}"
        );
        assert!(parse_program(&full).is_err(), "{source}");
    }
    assert!(parse("scene label").is_err());
    assert!(parse("scene label \"not_an_identifier\"").is_err());
}

#[test]
fn legacy_apis_stay_strict_and_lexical_function_screen_handler_locals_stay_valid() {
    let helper = "label helper\nlocal count = 1\nreturn\n";
    assert!(parse(helper).is_err());
    assert!(parse_spanned(helper).is_err());
    assert!(!parse_recovering(helper).unwrap().errors.is_empty());
    assert!(SourceDocument::parse(helper).is_err());
    let contextual = parse_with_context(helper, NarrativeContext::ExplicitScenes).unwrap();
    assert!(
        matches!(&contextual[1], Statement::LocalVar { name, value: Expr::Int(1) } if name == "count")
    );
    assert_eq!(
        parse_spanned_with_context(helper, NarrativeContext::ExplicitScenes)
            .unwrap()
            .into_iter()
            .map(|s| s.statement)
            .collect::<Vec<_>>(),
        contextual
    );
    assert!(
        parse_recovering_with_context(helper, NarrativeContext::ExplicitScenes)
            .unwrap()
            .errors
            .is_empty()
    );
    let lexical = "function f(a) { local x = a return x }\nscreen s() { local x = 2 return x }\nhandler h() { local x = 3 return x }\n";
    let legacy = parse(lexical).unwrap();
    assert_eq!(
        parse_with_context(lexical, NarrativeContext::ExplicitScenes).unwrap(),
        legacy
    );
    for statement in &legacy {
        let body = match statement {
            Statement::Function { body, .. }
            | Statement::Screen { body, .. }
            | Statement::Handler { body, .. } => body,
            _ => panic!("lexical declaration"),
        };
        assert!(matches!(body[0], Statement::LocalVar { .. }));
        assert!(matches!(body[1], Statement::FunctionReturn { .. }));
    }
}

#[test]
fn resolved_context_accepts_helpers_before_entry_but_never_init_locals() {
    let source = "label helper\nlocal n = 4\nif n > 1 { local nested = n }\nreturn\nscene label intro\ncall helper\n";
    assert!(parse(source).is_err());
    let program = parse_program(source).unwrap();
    assert_eq!(program.context, NarrativeContext::ExplicitScenes);
    assert!(matches!(&program.script[1], Statement::LocalVar {name,..} if name == "n"));
    for body in [
        "local n = 1",
        "if true { local n = 1 }",
        "while false { local n = 1 }",
        "for x in [1] { local n = x }",
        "choice { \"go\" => { local n = 1 } }",
    ] {
        let invalid = format!("scene label intro\ninit {{ {body} }}\n");
        assert!(parse_program(&invalid).is_err(), "{body}");
    }
    assert!(parse_program("label helper\nlocal n = 4\nreturn\n").is_err());
    assert!(parse_for_context_discovery("label helper\nlocal n = 4\nreturn\n").is_ok());
}

#[test]
fn global_context_is_derived_from_frozen_imports_with_helper_and_real_file_origins() {
    let files = Files::new();
    let root_source = "// racine été\nlabel helper\nlocal n = 1\nreturn\nuse \"entries.rvn\"\nlabel end\nreturn\n";
    let entry_source = "// entrée réelle\nscene label intro\ncall helper\n";
    let root = files.write("root.rvn", root_source);
    let entry = files.write("entries.rvn", entry_source);
    let program = parse_file_program_with_uses(&root).unwrap();
    assert_eq!(program.context, NarrativeContext::ExplicitScenes);
    assert_eq!(program.sources.len(), 2);
    assert_eq!(program.script.len(), 7);
    assert!(matches!(&program.script[3], Statement::SceneLabel {name} if name == "intro"));
    assert_eq!(program.origins.len(), program.script.len());
    assert_eq!(
        program.origins[3].source,
        Some(entry.canonicalize().unwrap())
    );
    assert_eq!(
        &entry_source[program.origins[3].span.range.clone()],
        "scene label intro"
    );
    assert_eq!(
        program.origins[1].source,
        Some(root.canonicalize().unwrap())
    );
    assert_eq!(
        &root_source[program.origins[1].span.range.clone()],
        "local n = 1"
    );
    assert_eq!(program.sources[0].source, root_source);
    assert_eq!(program.sources[1].source, entry_source);
    assert_eq!(fs::read_to_string(&root).unwrap(), root_source);
    assert_eq!(fs::read_to_string(&entry).unwrap(), entry_source);
    assert!(parse_file_with_uses(&root).is_err());
    assert!(parse_program(root_source).unwrap_err().contains("use"));
}

#[test]
fn imported_helper_without_marker_uses_project_context_and_no_marker_program_refuses() {
    let files = Files::new();
    let helper_source = "label utility\nlocal n = 1\nreturn\n";
    files.write("helper.rvn", helper_source);
    let root = files.write(
        "root.rvn",
        "use \"helper.rvn\"\nscene label intro\ncall utility\n",
    );
    let program = parse_file_program_with_uses(&root).unwrap();
    assert!(matches!(&program.script[1], Statement::LocalVar {name,..} if name == "n"));
    assert!(program.sources.iter().any(|s| s.source == helper_source));
    fs::write(&root, "use \"helper.rvn\"\nlabel intro\ncall utility\n").unwrap();
    let error = parse_file_program_with_uses(&root).unwrap_err();
    assert!(error.contains("helper.rvn"));
    assert!(error.contains("local"));
    assert_eq!(
        fs::read_to_string(files.0.join("helper.rvn")).unwrap(),
        helper_source
    );
}

#[test]
fn resolved_imports_preserve_wildcard_order_alias_dedup_and_cycle_diagnostics() {
    let files = Files::new();
    files.write("parts/b.rvn", "label b\nreturn\n");
    files.write("parts/a.rvn", "scene label a\nreturn\n");
    files.write("parts/ignored.txt", "not an rvn source");
    let root = files.write("root.rvn", "label before\nuse \"parts/*.rvn\"\nuse { \"./parts/a.rvn\", \"parts/../parts/b.rvn\", \"parts/*\" }\nlabel after\nreturn\n");
    let program = parse_file_program_with_uses(&root).unwrap();
    let names: Vec<_> = program
        .script
        .iter()
        .filter_map(|s| match s {
            Statement::Label { name } | Statement::SceneLabel { name } => Some(name.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(names, ["before", "a", "b", "after"]);
    assert_eq!(program.sources.len(), 3);
    assert_eq!(program.script, parse_file_with_uses(&root).unwrap());
    fs::write(
        files.0.join("parts/a.rvn"),
        "use \"../root.rvn\"\nscene label a\n",
    )
    .unwrap();
    let error = parse_file_program_with_uses(&root).unwrap_err();
    assert!(error.contains("cycle de `use`"));
    assert!(error.contains("root.rvn") && error.contains("a.rvn"));
    let missing = files.write("missing.rvn", "use \"empty/*\"\nscene label intro\n");
    fs::create_dir(files.0.join("empty")).unwrap();
    assert!(parse_file_program_with_uses(missing)
        .unwrap_err()
        .contains("aucun fichier"));
}

fn text<'a>(source: &'a str, span: &StatementSpan) -> &'a str {
    &source[span.range.clone()]
}
fn check_containment(span: &StatementSpan) {
    for child in &span.children {
        assert!(span.range.start <= child.range.start && child.range.end <= span.range.end);
        check_containment(child);
    }
    for adjacent in span.children.windows(2) {
        assert!(adjacent[0].range.end <= adjacent[1].range.start);
    }
}

#[test]
fn source_origin_tree_retains_exact_utf8_and_repeated_branch_occurrences() {
    let source = "// été 🍃\nscene label intro\nif true {\n local n = 1 // premier\n choice {\n \"oui\" => { set n = 2 }\n \"non\" => { set n = 2 }\n }\n} else {\n local n = 1 // second\n}\nlabel helper\nfor item in [1,2] { while false { local x = item } }\nreturn\n";
    let parsed = parse_with_origins(source, NarrativeContext::ExplicitScenes).unwrap();
    assert_eq!(parsed.script.len(), parsed.spans.len());
    let branch = &parsed.spans[1];
    assert_eq!(branch.children.len(), 3);
    assert_eq!(text(source, &branch.children[0]), "local n = 1");
    assert_eq!(text(source, &branch.children[2]), "local n = 1");
    assert_ne!(branch.children[0].range, branch.children[2].range);
    let choice = &branch.children[1];
    assert_eq!(choice.children.len(), 2);
    assert_eq!(text(source, &choice.children[0]), "set n = 2");
    assert_eq!(text(source, &choice.children[1]), "set n = 2");
    assert_ne!(choice.children[0].range, choice.children[1].range);
    let loop_span = &parsed.spans[3];
    assert_eq!(
        text(source, &loop_span.children[0]),
        "while false { local x = item }"
    );
    assert_eq!(
        text(source, &loop_span.children[0].children[0]),
        "local x = item"
    );
    for span in &parsed.spans {
        check_containment(span);
    }
    let legacy_spans =
        parse_spanned_with_context(source, NarrativeContext::ExplicitScenes).unwrap();
    assert_eq!(
        legacy_spans.iter().map(|s| &s.range).collect::<Vec<_>>(),
        parsed.spans.iter().map(|s| &s.range).collect::<Vec<_>>()
    );
}

#[test]
fn origin_children_cover_lexical_declarations_init_and_imagemap_hotspots() {
    let source = "init { set shared = 0 }\nfunction f() { local x = 1 return x }\nscreen s() { local x = 2 return x }\nhandler h() { local x = 3 return x }\nscene label intro\nimagemap { background: \"map.png\" hotspot { area: (0,0,20,20) } => { local x = 4 jump helper } hotspot { area: (20,0,40,20) } => { local x = 5 call helper } }\nlabel helper\nreturn\n";
    let parsed = parse_with_origins(source, NarrativeContext::ExplicitScenes).unwrap();
    assert_eq!(text(source, &parsed.spans[0].children[0]), "set shared = 0");
    for (root, value) in [(1, 1), (2, 2), (3, 3)] {
        assert_eq!(parsed.spans[root].children.len(), 2);
        assert_eq!(
            text(source, &parsed.spans[root].children[0]),
            format!("local x = {value}")
        );
        assert_eq!(text(source, &parsed.spans[root].children[1]), "return x");
    }
    let hotspots = &parsed.spans[5].children;
    assert_eq!(
        hotspots.iter().map(|s| text(source, s)).collect::<Vec<_>>(),
        ["local x = 4", "jump helper", "local x = 5", "call helper"]
    );
    assert!(parse_program("if true { use \"unresolved.rvn\" }")
        .unwrap_err()
        .contains("use"));
}

#[test]
fn contextual_source_edits_keep_helper_trivia_and_refuse_invalid_trials_atomically() {
    let source = "// Helper été\nlabel helper\nlocal n = 1 // note 🍃\nif n > 0 { local nested = n }\nreturn\n";
    let mut doc =
        SourceDocument::parse_with_context(source, NarrativeContext::ExplicitScenes).unwrap();
    doc.replace_statement_range_from_baseline(1..2, source, Some("local n = 1"), "local n = 2")
        .unwrap();
    assert_eq!(
        doc.source(),
        source.replacen("local n = 1", "local n = 2", 1)
    );
    assert_eq!(doc.narrative_context(), NarrativeContext::ExplicitScenes);
    let saved = doc.source().to_owned();
    let ast: Vec<_> = doc
        .statements()
        .iter()
        .map(|s| s.statement.clone())
        .collect();
    assert!(doc
        .replace_statement(2, &saved, "if true { scene label forbidden }")
        .is_err());
    assert!(doc
        .replace_statement(1, "external modification", "local n = 3")
        .is_err());
    assert!(doc.replace_statement(1, &saved, "local n = (").is_err());
    assert_eq!(doc.source(), saved);
    assert_eq!(
        doc.statements()
            .iter()
            .map(|s| s.statement.clone())
            .collect::<Vec<_>>(),
        ast
    );
    doc.replace_statement_range_from_baseline(1..2, &saved, Some("local n = 2"), "local n = 7")
        .unwrap();
    assert!(doc.source().contains("local n = 7 // note 🍃"));
    assert!(doc
        .source()
        .ends_with("if n > 0 { local nested = n }\nreturn\n"));
}

#[test]
fn contextual_full_document_edits_preserve_init_and_scene_entry_and_revalidate_demotion() {
    let source = "scene label intro\nlocal n = 1\nlabel helper\nreturn\n";
    let mut doc =
        SourceDocument::parse_with_context(source, NarrativeContext::ExplicitScenes).unwrap();
    doc.insert_init_characters(source, "character.create(\"iris\",\"Iris\")")
        .unwrap();
    assert!(matches!(
        doc.statements()[0].statement,
        Statement::Init { .. }
    ));
    assert!(matches!(
        doc.statements()[1].statement,
        Statement::SceneLabel { .. }
    ));
    let current = doc.source().to_owned();
    doc.replace_statement(1, &current, "label intro").unwrap();
    // A fragment context intentionally survives editing; complete-program
    // validation must reject the now-unscoped local before any publication.
    assert!(parse_program(doc.source()).is_err());
    assert_eq!(doc.narrative_context(), NarrativeContext::ExplicitScenes);
    let current = doc.source().to_owned();
    doc.replace_statement(1, &current, "scene label intro")
        .unwrap();
    assert!(parse_program(doc.source()).is_ok());
}

#[test]
fn recovering_context_restores_depth_after_errors_and_keeps_legacy_diagnostics() {
    let invalid = "init { local bad = 1 }\nlocal good = 2\nscene label intro\n";
    let recovered =
        parse_recovering_with_context(invalid, NarrativeContext::ExplicitScenes).unwrap();
    assert_eq!(recovered.errors.len(), 1);
    assert!(recovered
        .script
        .iter()
        .any(|s| matches!(s, Statement::LocalVar {name,..} if name == "good")));
    assert!(recovered
        .script
        .iter()
        .any(|s| matches!(s, Statement::SceneLabel {name} if name == "intro")));
    let legacy = "init { set n = 0 }\nlabel start\nif n == 0 { set n = 1 } else { set n = 2 }\nscene forest\nchoice { \"go\" => { jump next } }\nlabel next\nreturn\n";
    let expected = parse(legacy).unwrap();
    assert_eq!(
        parse_program(legacy).unwrap().context,
        NarrativeContext::Legacy
    );
    assert_eq!(parse_program(legacy).unwrap().script, expected);
    assert_eq!(
        format!(
            "{:?}",
            parse_with_context(legacy, NarrativeContext::Legacy).unwrap()
        ),
        format!("{expected:?}")
    );
    assert_eq!(
        parse_with_origins(legacy, NarrativeContext::Legacy)
            .unwrap()
            .script,
        expected
    );
    assert!(parse_recovering(legacy).unwrap().errors.is_empty());
}
