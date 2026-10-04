use logos::Logos;
use rvn_parser::lexer::Token;
use rvn_parser::{
    parse, parse_recovering, parse_spanned, Expr, ParseErrorKind, SourceDocument, SourceLocation,
    Statement,
};

#[test]
fn lexer_accepts_lf_crlf_and_mixed_blank_lines_without_changing_spans() {
    let source = "// heading\r\n\n\r\nlabel start\nset n = 1\r\nreturn";
    let tokens = Token::lexer(source)
        .spanned()
        .map(|(token, range)| (token.unwrap(), range))
        .collect::<Vec<_>>();
    let newlines = tokens
        .iter()
        .filter(|(token, _)| matches!(token, Token::Newline))
        .map(|(_, range)| &source[range.clone()])
        .collect::<Vec<_>>();
    assert_eq!(newlines, ["\r\n\n\r\n", "\n", "\r\n"]);
    let label = tokens
        .iter()
        .find(|(token, _)| matches!(token, Token::Label))
        .unwrap();
    assert_eq!(label.1.start, source.find("label").unwrap());
    assert_eq!(&source[label.1.clone()], "label");
}

#[test]
fn windows_source_parses_like_lf_with_inline_and_full_line_comments() {
    let lf = "// SOURCE CANONIQUE\n\nlabel start // entrée\nset score = 1\nchoice {\n // les choix restent accessibles\n \"Oui // texte\" if score == 1 => { return }\n \"Non\" => { jump start }\n}\n// commentaire final";
    let crlf = lf.replace('\n', "\r\n");
    assert_eq!(parse(&crlf).unwrap(), parse(lf).unwrap());
    assert!(parse("// commentaire seul\r\n\r\n").unwrap().is_empty());
    assert!(parse("// commentaire à EOF").unwrap().is_empty());
}

#[test]
fn standalone_carriage_returns_outside_strings_are_still_lex_errors() {
    for source in [
        "label start\rreturn",
        "// commentaire\rlabel start",
        "\r\r\nlabel start",
    ] {
        let error = parse(source).unwrap_err();
        assert_eq!(
            error.kind,
            ParseErrorKind::LexError {
                slice: "\r".to_owned()
            }
        );
        assert_eq!(error.location.line, 1);
        assert_eq!(
            error.location.col,
            source[..source.find('\r').unwrap()].chars().count() + 1
        );
        assert_eq!(error.location.len, 1);
    }
}

#[test]
fn quoted_crlf_and_standalone_cr_are_preserved_as_string_content() {
    let payload = "première\r\n// ceci reste du texte\rdernière";
    let source = format!("set note = \"{payload}\"\r\n");
    let script = parse(&source).unwrap();
    assert!(
        matches!(&script[0], Statement::SetVar { value: Expr::Str(text), .. } if text == payload)
    );
    let dialogue = parse(&format!("\"{payload}\"\r\n")).unwrap();
    assert!(
        matches!(&dialogue[0], Statement::Dialogue { text, .. } if text.as_plain() == Some(payload))
    );
}

#[test]
fn escaped_line_breaks_keep_their_existing_string_semantics() {
    let script = parse("set note = \"first\\r\\nsecond\"\r\n").unwrap();
    assert!(
        matches!(&script[0], Statement::SetVar { value: Expr::Str(text), .. } if text == "first\r\nsecond")
    );
}

#[test]
fn crlf_diagnostics_keep_original_line_columns_and_source_excerpt() {
    let source = "// Été\r\nlabel start\r\n  @\r\n";
    let error = parse(source).unwrap_err();
    assert_eq!(
        error.location,
        SourceLocation {
            line: 3,
            col: 3,
            len: 1
        }
    );
    assert_eq!(error.source_line, "  @");
    assert_eq!(
        error.kind,
        ParseErrorKind::LexError {
            slice: "@".to_owned()
        }
    );
    let eof = parse("set n =\r\n").unwrap_err();
    assert_eq!(eof.location.line, 2);
    assert_eq!(eof.location.col, 1);
    assert!(matches!(eof.kind, ParseErrorKind::UnexpectedEof { .. }));
}

#[test]
fn recovering_parser_keeps_crlf_statement_boundaries_and_error_locations() {
    let recovered = parse_recovering("label start\r\n  n = 5 // erreur\r\n  jump end\r\n").unwrap();
    assert_eq!(recovered.errors.len(), 1);
    assert_eq!(recovered.errors[0].location.line, 2);
    assert_eq!(recovered.errors[0].location.col, 3);
    assert!(matches!(
        recovered.errors[0].kind,
        ParseErrorKind::InvalidAssignment { .. }
    ));
    assert!(matches!(&recovered.script[1], Statement::Jump { target } if target == "end"));
}

#[test]
fn crlf_statement_ranges_index_the_original_utf8_source() {
    let source = "// Été\r\n  label start // entrée\r\n  set n=1 // valeur\r\n  \"fin\"\r\n";
    let statements = parse_spanned(source).unwrap();
    assert_eq!(statements.len(), 3);
    for (statement, expected) in statements.iter().zip(["label start", "set n=1", "\"fin\""]) {
        let start = source.find(expected).unwrap();
        assert_eq!(statement.range, start..start + expected.len());
        assert_eq!(&source[statement.range.clone()], expected);
    }
}

#[test]
fn editing_windows_source_retains_every_unrelated_crlf_and_comment() {
    let source = "// Été\r\nlabel start // entrée\r\n  set n=1 // conserver\r\n\r\n  \"https://example.test\"\r\n";
    let mut document = SourceDocument::parse(source).unwrap();
    assert_eq!(document.source(), source);
    document.replace_statement(1, source, "set n=2").unwrap();
    assert_eq!(document.source(), source.replace("set n=1", "set n=2"));
}
