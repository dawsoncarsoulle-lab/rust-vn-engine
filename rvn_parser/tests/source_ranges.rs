use rvn_parser::{parse, parse_spanned, Statement};

#[test]
fn ranges_retain_original_utf8_offsets_and_exclude_surrounding_trivia() {
    let source = "// Été : ne pas perdre ce commentaire\n\n  label start // entrée\n  set n = max(1, 2) // valeur\n  narrator \"été // dans du texte\"\n";
    let statements = parse_spanned(source).unwrap();
    assert_eq!(
        statements
            .iter()
            .map(|s| s.statement.clone())
            .collect::<Vec<_>>(),
        parse(source).unwrap()
    );
    assert_eq!(&source[statements[0].range.clone()], "label start");
    assert_eq!(&source[statements[1].range.clone()], "set n = max(1, 2)");
    assert_eq!(
        &source[statements[2].range.clone()],
        "narrator \"été // dans du texte\""
    );
}

#[test]
fn compound_ranges_include_the_whole_body_not_the_next_statement() {
    let source = "init {\n // declaration\n set n = 1\n}\nlabel start\nchoice {\n \"Yes\" => {\n set n = n + 1\n }\n}\nreturn\n";
    let statements = parse_spanned(source).unwrap();
    assert!(matches!(statements[0].statement, Statement::Init { .. }));
    assert_eq!(
        &source[statements[0].range.clone()],
        "init {\n // declaration\n set n = 1\n}"
    );
    assert!(matches!(statements[2].statement, Statement::Choice { .. }));
    assert!(source[statements[2].range.clone()].ends_with('}'));
    assert_eq!(&source[statements[3].range.clone()], "return");
    for pair in statements.windows(2) {
        assert!(pair[0].range.end <= pair[1].range.start);
    }
}

#[test]
fn spans_do_not_turn_invalid_sources_into_a_partial_editable_document() {
    assert!(parse_spanned("label start\nset n = (\n").is_err());
    assert!(parse_spanned("// nothing\n").unwrap().is_empty());
}


#[test]
fn quoted_variable_keys_keep_exact_bytes_and_utf8_crlf_spans() {
    use rvn_parser::{Expr,variable_source_name};
    for name in ["flag","Score","score","Confianceété","Confiance été","true","1er","日本語","é","e\u{301}","valeur]été","prix`été","chemin\\été","prix\"été","💠"] {
        let spelling=variable_source_name(name).unwrap();
        let statement=format!("set {spelling} = {spelling}");
        let source=format!("// Été 💠\r\nlabel start\r\n{statement}\r\n");
        let parsed=parse_spanned(&source).unwrap();
        assert_eq!(parsed[1].statement,Statement::SetVar{name:name.into(),value:Expr::Var(name.into())});
        assert_eq!(&source[parsed[1].range.clone()],statement);
        assert!(source.is_char_boundary(parsed[1].range.start)&&source.is_char_boundary(parsed[1].range.end));
    }
    assert_eq!(variable_source_name("flag").as_deref(),Some("flag"));
    assert_eq!(variable_source_name("persistent.__rvn_legacy").as_deref(),Some("persistent.__rvn_legacy"));
}

#[test]
fn quoted_variable_names_do_not_broaden_callable_or_reserved_binding_syntax() {
    for name in ["","__rvn_private","a.b","a\nété","a\u{0}b"] {
        assert!(rvn_parser::variable_binding_source_name(name).is_none());
        assert!(!rvn_parser::is_variable_binding_name(name));
    }
    for source in ["set `` = 1", "set `__rvn_private` = 1", "set `a.b` = 1", r"set `a\q` = 1", "function `été`() { return 1 }", "label start\nset flag = `été`()", "label start\nset flag = persistent.`été`()"] {
        assert!(parse(source).is_err(),"Unexpected admission: {source}");
    }
    assert!(rvn_parser::is_binding_name("reward"));
    assert!(!rvn_parser::is_binding_name("Confiance été"));
    assert!(!rvn_parser::is_binding_name("true"));
}

#[test]
fn interpolation_keeps_brackets_backticks_and_backslashes_inside_variable_tokens() {
    use rvn_parser::{Expr,TextSegment};
    let source=r#"label start
"Valeur [`état]été`] / [`prix\`été`] / [`chemin\\\\été`]"
"#;
    let parsed=parse(source).unwrap();
    let Statement::Dialogue{text,..}=&parsed[1] else {panic!("Expected dialogue")};
    let keys=text.0.iter().filter_map(|segment|match segment {TextSegment::Interp(Expr::Var(name))=>Some(name.as_str()),_=>None}).collect::<Vec<_>>();
    assert_eq!(keys,["état]été","prix`été","chemin\\été"]);
}
