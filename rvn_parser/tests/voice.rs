use rvn_parser::{parse, ParseErrorKind, Statement};

#[test]
fn voice_filename_and_stop_keep_the_existing_authored_forms() {
    let script = parse("voice \"qa-voice.wav\"\nvoice stop\nvoice \"é🌟.ogg\"\n").unwrap();
    assert!(matches!(&script[0], Statement::VoicePlay { file } if file == "qa-voice.wav"));
    assert!(matches!(&script[1], Statement::VoiceStop));
    assert!(matches!(&script[2], Statement::VoicePlay { file } if file == "é🌟.ogg"));
    assert_eq!(script.len(), 3);
}

#[test]
fn invalid_non_string_voice_argument_returns_a_located_error_without_panicking() {
    for (argument, token_len) in [
        (".play(\"qa-voice.wav\")", 1),
        ("1", 1),
        ("0.5", 3),
        ("()", 1),
        ("true", 4),
    ] {
        let source = format!("// é🌟\r\nvoice {argument}\r\n");
        let parsed = std::panic::catch_unwind(|| parse(&source));
        assert!(
            parsed.is_ok(),
            "invalid voice argument must not panic: {argument}"
        );
        let error = parsed.unwrap().unwrap_err();
        assert!(matches!(
            error.kind,
            ParseErrorKind::UnexpectedToken {
                expected: "string filename for voice",
                ..
            }
        ));
        assert_eq!(error.location.line, 2);
        assert_eq!(error.location.col, 7);
        assert_eq!(error.location.len, token_len);
        assert_eq!(error.source_line, format!("voice {argument}"));
    }
}

#[test]
fn voice_missing_argument_retains_the_structured_eof_error() {
    let error = parse("voice").unwrap_err();
    assert!(matches!(
        error.kind,
        ParseErrorKind::UnexpectedEof {
            expected: "string filename for voice"
        }
    ));
    assert_eq!(error.location.line, 1);
    assert_eq!(error.location.col, 6);
}
