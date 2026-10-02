use rvn_parser::{SourceDocument, SourceEditError};

#[test]
fn shared_cast_edits_ignore_comment_braces_preserve_trivia_and_are_atomic() {
    let source="init {\n // cast { note\n character.create(\"iris\",\"Iris\") // display } keep\n set hint=\"{ not syntax }\"\n}\nlabel start\n\"same\"\nreturn\n";
    let mut document = SourceDocument::parse(source).unwrap();
    document
        .insert_init_characters(source, "character.create(\"noe\",\"Noé 🍃\")")
        .unwrap();
    assert!(document.source().contains("// cast { note\n character.create(\"iris\",\"Iris\") // display } keep\n set hint=\"{ not syntax }\""));
    let saved = document.source().to_owned();
    let changes = std::collections::BTreeMap::from([
        (
            "iris".into(),
            Some("character.create(\"iris\",\"New Iris\")".into()),
        ),
        ("noe".into(), None),
    ]);
    document
        .edit_character_declarations(&saved, &changes)
        .unwrap();
    assert!(document.source().contains("// cast { note"));
    assert!(document
        .source()
        .contains("// display } keep\n set hint=\"{ not syntax }\""));
    assert!(document.source().contains("New Iris"));
    assert!(!document.source().contains("Noé 🍃"));
    let saved = document.source().to_owned();
    let invalid = std::collections::BTreeMap::from([("iris".into(), Some("set score=99".into()))]);
    assert!(document
        .edit_character_declarations(&saved, &invalid)
        .is_err());
    assert!(document
        .insert_init_characters(&saved, "set score=99")
        .is_err());
    assert!(document
        .edit_character_declarations("stale", &changes)
        .is_err());
    assert_eq!(document.source(), saved);
}

#[test]
fn one_edit_preserves_every_surrounding_byte_and_can_be_repeated() {
    let source = "// Été\nlabel start // entry\n  set n=1   // keep\n\n narrator \"https://example.test\" // ending\n";
    let mut document = SourceDocument::parse(source).unwrap();
    document.replace_statement(1, source, "set n = 2").unwrap();
    assert_eq!(document.source(), source.replace("set n=1", "set n = 2"));
    let current = document.source().to_owned();
    document
        .replace_statement(1, &current, "set n = 3")
        .unwrap();
    assert_eq!(document.source(), source.replace("set n=1", "set n = 3"));
}

#[test]
fn unchanged_graph_preserves_original_formatting_and_internal_comments() {
    let source = "init {\n // important\n set n=1\n}\n";
    let mut document = SourceDocument::parse(source).unwrap();
    document
        .replace_statement(0, source, "init { set n = 1 }")
        .unwrap();
    assert_eq!(document.source(), source);
}

#[test]
fn internal_comments_are_protected_until_a_lossless_edit_is_provided() {
    let source = "init {\n // important\n set n=1\n}\n";
    let mut document = SourceDocument::parse(source).unwrap();
    assert_eq!(
        document.replace_statement(0, source, "init { set n = 2 }"),
        Err(SourceEditError::CommentWouldBeLost)
    );
    assert_eq!(document.source(), source);
    document
        .replace_statement(0, source, "init {\n // important\n set n = 2\n}")
        .unwrap();
    assert!(document.source().contains("// important"));
}

#[test]
fn conflicts_invalid_code_and_statement_injection_never_change_the_snapshot() {
    let source = "label start\n set n = 1\n";
    let mut document = SourceDocument::parse(source).unwrap();
    assert_eq!(
        document.replace_statement(1, "label start\n set n = 9\n", "set n = 2"),
        Err(SourceEditError::Conflict)
    );
    assert!(document.replace_statement(1, source, "set n = (").is_err());
    assert_eq!(
        document.replace_statement(1, source, "set n = 2\nset x = 3"),
        Err(SourceEditError::ExpectedSingleStatement)
    );
    assert_eq!(document.source(), source);
}

#[test]
fn urls_and_comment_like_strings_do_not_block_valid_edits() {
    let source = "label start\nnarrator \"https://example.test\"\n";
    let mut document = SourceDocument::parse(source).unwrap();
    document
        .replace_statement(1, source, "narrator \"// new text\"")
        .unwrap();
    assert!(document.source().contains("// new text"));
}

#[test]
fn trailing_comment_cannot_swallow_a_neighboring_statement() {
    let source = "set n = 1 set x = 2\n";
    let mut document = SourceDocument::parse(source).unwrap();
    assert_eq!(
        document.replace_statement(0, source, "set n = 3 //"),
        Err(SourceEditError::SurroundingCodeWouldChange)
    );
    assert_eq!(document.source(), source);
}
