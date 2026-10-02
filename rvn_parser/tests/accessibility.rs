use rvn_parser::parse;

#[test]
fn event_handlers_accept_portable_controls_but_calculation_functions_do_not() {
    let commands = [
        "accessibility.configure({\"text_scale\":1.5})",
        "accessibility.speak(\"Éloïse\")",
        "accessibility.stop()",
        "video.play(\"intro\",video_clip(\"intro.webm\",{}))",
        "video.pause(\"intro\")",
        "video.resume(\"intro\")",
        "video.seek(\"intro\",0.5)",
        "video.volume(\"intro\",0.25)",
        "video.stop(\"intro\")",
    ];
    for command in commands {
        assert!(
            parse(&format!("handler click(event) {{if true {{{command}}}}}")).is_ok(),
            "{command}"
        );
        assert!(
            parse(&format!("function calculation() {{{command} return 1}}")).is_err(),
            "{command}"
        );
        assert!(
            parse(&format!("screen view() {{{command} return {{}}}}")).is_err(),
            "{command}"
        );
    }
}

#[test]
fn accessibility_arity_is_checked_before_execution() {
    for invalid in [
        "accessibility.configure()",
        "accessibility.speak()",
        "accessibility.stop(1)",
        "accessibility.speak(1,2)",
        "accessibility.unknown()",
    ] {
        assert!(parse(invalid).is_err(), "{invalid}");
    }
}
