//! Exact position/diagnostic compatibility of the real lexer/parser. No timing
//! thresholds, alternate grammar, synthetic validation or cached source state.
use super::*;
fn random_access_locations(source:&str)->Parser<'_>{
    // The same parser/lexer/grammar; only replace token metadata with the
    // unchanged historical public random-access location function as oracle.
    let mut parser=Parser::new(source).unwrap();
    for((_,location),span)in parser.tokens.iter_mut().zip(&parser.token_ranges){
        *location=byte_offset_to_location(source,span.start,span.len());
    }
    parser.eof_location=byte_offset_to_location(source,source.len(),1);parser
}
#[test]
fn location_cursor1022_every_utf8_boundary_length_and_lf_crlf_match_public_reference(){
    for source in ["","plain","α🙂e\u{301}\r\nβ\n中\rfin","\n\r\n\n","text\r"]{
        let mut cursor=SourceLocationCursor::new(source);
        for offset in (0..=source.len()).filter(|offset|source.is_char_boundary(*offset)){
            for len in 0..=source.len()+2{
                assert_eq!(cursor.location(offset,len),byte_offset_to_location(source,offset,len),"offset={offset},len={len},source={source:?}");
            }
        }
    }
}
#[test]
fn location_cursor1022_actual_logos_tokens_keep_unicode_columns_crlf_lengths_and_skipped_comments(){
    let source="// Été🙂\r\n\n\r\nlabel start\nset note = \"é🙂\r\n中\rfin\" // skipped π🙂\r\nreturn";
    let parser=Parser::new(source).unwrap();let old=random_access_locations(source);
    assert_eq!(parser.token_ranges,old.token_ranges);assert_eq!(parser.tokens,old.tokens);assert_eq!(parser.eof_location,old.eof_location);
    assert!(parser.tokens.iter().any(|(token,location)|matches!(token,Token::String(_))&&location.len>1));
    assert_eq!(Parser::new(source).unwrap().parse_script().unwrap(),random_access_locations(source).parse_script().unwrap());
}
#[test]
fn location_cursor1022_empty_and_trailing_newline_eof_locations_keep_one_based_scalar_policy(){
    for source in ["","//🙂","//🙂\r\n","\n\r\n","set note=\"🙂\"\r\n"]{
        let actual=Parser::new(source).unwrap();assert_eq!(actual.eof_location,byte_offset_to_location(source,source.len(),1));
        assert_eq!(actual.eof_location.len,1);
    }
    let actual=Parser::new("\r\n").unwrap();assert_eq!(actual.eof_location,SourceLocation{line:2,col:1,len:1});
}
#[test]
fn location_cursor1022_actual_lex_errors_keep_slice_unicode_position_excerpt_and_display(){
    let source="// Été🙂\r\nlabel start\r\n  @\r\n";
    let error=parse(source).unwrap_err();assert_eq!(error.kind,ParseErrorKind::LexError{slice:"@".into()});
    assert_eq!(error.location,SourceLocation{line:3,col:3,len:1});assert_eq!(error.source_line,"  @");
    let reference=ParseError::build(ParseErrorKind::LexError{slice:"@".into()},byte_offset_to_location(source,source.find('@').unwrap(),1),source);
    assert_eq!(error,reference);assert_eq!(error.to_string(),reference.to_string());
    let unicode="// Été\nset n = 🙂";let error=parse(unicode).unwrap_err();
    assert_eq!(error.kind,ParseErrorKind::LexError{slice:"🙂".into()});
    assert_eq!(error.location,SourceLocation{line:2,col:9,len:1});
    assert_eq!(error.location,byte_offset_to_location(unicode,unicode.find('🙂').unwrap(),"🙂".len()));
    let bare_cr="// Été🙂\rlabel start";let error=parse(bare_cr).unwrap_err();let offset=bare_cr.find('\r').unwrap();
    assert_eq!(error.kind,ParseErrorKind::LexError{slice:"\r".into()});assert_eq!(error.location,byte_offset_to_location(bare_cr,offset,1));
}
#[test]
fn location_cursor1022_actual_unexpected_tokens_eof_and_assignments_keep_exact_diagnostics(){
    for source in ["// Été🙂\r\nlabel start\r\nset n = )\r\n","set n =\r\n","//🙂\nlabel start\n  n = 5 // erreur\n"]{
        let actual=Parser::new(source).unwrap().parse_script().unwrap_err();
        let reference=random_access_locations(source).parse_script().unwrap_err();
        assert_eq!(actual,reference);assert_eq!(actual.to_string(),reference.to_string());
    }
    let eof=parse("set n =\r\n").unwrap_err();assert!(matches!(eof.kind,ParseErrorKind::UnexpectedEof{..}));assert_eq!(eof.location,SourceLocation{line:2,col:1,len:1});
}
#[test]
fn location_cursor1022_recovery_keeps_following_statements_and_original_error_locations(){
    let source="// Été🙂\r\nlabel start\r\n  n = 5 // erreur\r\n  jump end\r\n";
    let actual=Parser::new(source).unwrap().parse_script_recovering();let reference=random_access_locations(source).parse_script_recovering();
    assert_eq!(actual,reference);assert_eq!(actual.errors.len(),1);assert_eq!(actual.errors[0].location,SourceLocation{line:3,col:3,len:1});
    assert!(matches!(&actual.script[1],Statement::Jump{target}if target=="end"));
}
#[test]
fn location_cursor1022_spanned_source_ranges_still_index_original_utf8_and_crlf_bytes(){
    let source="// Été🙂\r\n  label start // entrée\r\n  set note=\"中🙂\" // valeur\r\n  \"fin\"\r\n";
    let actual=Parser::new(source).unwrap().parse_script_spanned().unwrap();let reference=random_access_locations(source).parse_script_spanned().unwrap();
    assert_eq!(actual,reference);for(statement,expected)in actual.iter().zip(["label start","set note=\"中🙂\"","\"fin\""]){
        assert_eq!(&source[statement.range.clone()],expected);
    }
}
#[test]
fn location_cursor1022_many_valid_tokens_keep_all_metadata_and_ast_without_a_timing_assertion(){
    let mut source=String::from("//🙂\r\nlabel start\r\n");
    for n in 0..96{source.push_str(&format!("set value_{n}=\"é🙂-{n}\" // 中\r\n"));}source.push_str("return\r\n");
    let actual=Parser::new(&source).unwrap();let reference=random_access_locations(&source);
    assert_eq!(actual.tokens,reference.tokens);assert_eq!(actual.token_ranges,reference.token_ranges);assert_eq!(actual.eof_location,reference.eof_location);
    assert_eq!(Parser::new(&source).unwrap().parse_script().unwrap(),random_access_locations(&source).parse_script().unwrap());
}
