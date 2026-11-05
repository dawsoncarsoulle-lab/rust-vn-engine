// PRIVATE NOT_RUN. Future sibling integration test, no dependency additions.
use rvn_parser::{parse, parse_program, parse_with_origins, NarrativeContext, Statement};

fn level(source: &str) -> f32 {
    let script = parse(source).unwrap();
    let [Statement::MusicVolume { level }] = script.as_slice() else {
        panic!("Expected exactly one MusicVolume, got {script:?}");
    };
    *level
}

#[test]
fn music_volume_numeric_literals_keep_the_quoted_ast_debug_and_f32_bits() {
    for literal in ["0.4", "0.8", "0.9", "0", "1", "2", "-0.25", "-0.0"] {
        let numeric = format!("music.volume({literal})\n");
        let quoted = format!("music.volume(\"{literal}\")\n");
        let a = parse(&numeric).unwrap();
        let b = parse(&quoted).unwrap();
        assert_eq!(a, b, "literal {literal}");
        assert_eq!(format!("{a:?}"), format!("{b:?}"));
        assert_eq!(level(&numeric).to_bits(), level(&quoted).to_bits());
    }
    assert_eq!(level("music.volume(0.4)\n").to_bits(), 0.4f32.to_bits());
}

#[test]
fn music_volume_numeric_integers_and_signed_literals_use_the_existing_unit_clamp() {
    for (literal, expected) in [("0", 0.0f32), ("1", 1.0), ("2", 1.0),
        ("-3", 0.0), ("-0.25", 0.0), ("1.25", 1.0),
        ("9223372036854775807", 1.0)] {
        assert_eq!(level(&format!("music.volume({literal})")).to_bits(), expected.to_bits());
    }
    assert_eq!(level("music.volume(-0.0)").to_bits(), (-0.0f32).to_bits());
}

#[test]
fn music_volume_numeric_arity_expressions_and_other_types_are_refused_at_source() {
    for arguments in ["0.4, 0.5", "0.4,", "0.4 0.5", "0.4 + 0.1",
        "true", "amount", "[]", "{}", "-amount", "+0.4", "(0.4)"] {
        let source = format!("music.volume({arguments})\n");
        let error = parse(&source).unwrap_err();
        assert_eq!(error.location.line, 1);
        assert!(error.location.col >= 14);
        assert_eq!(error.source_line, source.trim_end());
    }
    let error = parse("music.volume(0.4, 0.5)").unwrap_err();
    assert_eq!(error.location.col, 17);
    assert_eq!(error.location.len, 1);
}

#[test]
fn music_volume_numeric_nonfinite_f32_is_refused_before_clamp_with_exact_line() {
    let source = "// 🌿\r\nmusic.volume(3402823669209384634633746074317682114560.0)\r\n";
    let error = parse(source).unwrap_err();
    assert_eq!((error.location.line, error.location.col), (2, 14));
    assert!(error.to_string().contains("volume non fini"));
    assert!(error.source_line.starts_with("music.volume("));
    assert!(parse("music.volume(-3402823669209384634633746074317682114560.0)").is_err());
}

#[test]
fn music_volume_support_does_not_widen_generic_method_arguments_or_legacy_quoted_path() {
    for source in ["music.play(0.4)", "music.stop(1)", "sfx.play(0.4)",
        "hero.show(0.4)", "hero.volume(0.4)", "music.unknown(0.4)"] {
        assert!(parse(source).is_err(), "{source}");
    }
    assert!(matches!(parse("music.play(\"theme.ogg\")").unwrap().as_slice(),
        [Statement::MusicPlay { file, .. }] if file == "theme.ogg"));
    // Existing empty, conversion fallback, quoted clamp, and string-list
    // behavior are intentionally preserved; this patch does not repair them.
    assert_eq!(level("music.volume()"), 1.0);
    assert_eq!(level("music.volume(\"not_a_number\")"), 1.0);
    assert_eq!(level("music.volume(\"2.5\")"), 1.0);
    assert_eq!(level("music.volume(\"-2.5\")"), 0.0);
    assert_eq!(level("music.volume(\"0.4\", \"0.8\")").to_bits(), 0.4f32.to_bits());
}

#[test]
fn music_volume_numeric_program_and_source_origins_keep_original_utf8_crlf_ranges() {
    let source = "// 🌿\r\nscene label A\r\nmusic.volume(0.4) // 🎵\r\n\"hold\"\r\n";
    let program = parse_program(source).unwrap();
    assert_eq!(program.context, NarrativeContext::ExplicitScenes);
    assert_eq!(program.sources[0].source, source);
    assert_eq!(program.origins.len(), program.script.len());
    assert_eq!(program.origins.len(), 3);
    assert_eq!(&source[program.origins[1].span.range.clone()], "music.volume(0.4)");
    assert!(program.origins[1].span.children.is_empty());
    assert_eq!(program.origins[1].source, None);
    let quoted = source.replace("music.volume(0.4)", "music.volume(\"0.4\")");
    assert_eq!(program.script, parse_program(&quoted).unwrap().script);
    let fragment = "if true { music.volume(0.8) }\r\n";
    let parsed = parse_with_origins(fragment, NarrativeContext::ExplicitScenes).unwrap();
    assert_eq!(parsed.spans.len(), 1);
    let [Statement::If { then_branch, .. }] = parsed.script.as_slice() else { panic!("narrative if"); };
    assert!(matches!(then_branch.as_slice(), [Statement::MusicVolume { level }] if level.to_bits() == 0.8f32.to_bits()));
    assert_eq!(&fragment[parsed.spans[0].children[0].range.clone()], "music.volume(0.8)");
    assert!(parse_with_origins(
        "handler gain() { music.volume(0.8) }\r\n",
        NarrativeContext::ExplicitScenes,
    ).is_err());
}
