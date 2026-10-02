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
