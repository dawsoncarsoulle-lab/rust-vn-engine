use rvn_parser::{parse, SourceDocument};

#[test]
fn edits_inside_functions_preserve_comments_and_unchanged_code_exactly() {
    let source = "// top\nfunction score(items) {\n  // À conserver\n  set total=0 // starting score\n  for item in items {\n    set total = total + item // reward\n  }\n  return total\n}\n\nlabel start // end\n\"hello\"\n";
    let mut document = SourceDocument::parse(source).unwrap();
    let replacement = "function score(items) { set total = 5 for item in items { set total = total + item * 2 } return total }";
    document
        .replace_statement_preserving_trivia(0, source, replacement)
        .unwrap();
    for preserved in [
        "// À conserver",
        "// starting score",
        "// reward",
        "// top\n",
        "\n\nlabel start // end\n\"hello\"\n",
        "  for item in items {\n",
        "  return total\n",
    ] {
        assert!(
            document.source().contains(preserved),
            "{preserved:?}: {}",
            document.source()
        );
    }
    assert_eq!(
        document.statements()[0].statement,
        parse(replacement).unwrap()[0]
    );
    let changed = document.source().to_owned();
    document
        .replace_statement_preserving_trivia(0, &changed, replacement)
        .unwrap();
    assert_eq!(document.source(), changed);
}

#[test]
fn deleting_a_commented_statement_retains_its_comment() {
    let source = "function f() {\n  set unused = 9 // rationale\n  return 1\n}";
    let mut document = SourceDocument::parse(source).unwrap();
    document
        .replace_statement_preserving_trivia(0, source, "function f() { return 2 }")
        .unwrap();
    assert!(document.source().contains("// rationale"));
    assert_eq!(
        parse(document.source()).unwrap(),
        parse("function f() { return 2 }").unwrap()
    );
}

#[test]
fn comments_and_invalid_edits_cannot_corrupt_surrounding_code() {
    let source = "init { set n = 1 // essential\n}\nlabel start\n";
    let mut document = SourceDocument::parse(source).unwrap();
    for replacement in ["init { set n = ( }", "init { set n = 2 } set hacked = 1"] {
        assert!(document
            .replace_statement_preserving_trivia(0, source, replacement)
            .is_err());
        assert_eq!(document.source(), source);
    }
    assert!(document
        .replace_statement_preserving_trivia(0, "changed", "init { set n = 2 }")
        .is_err());
    assert_eq!(document.source(), source);
}
