use rvn_parser::{parse, Statement};
#[test]
fn video_controls_accept_expressions_and_invalid_config_never_panics() {
    let source="label start\nvideo.play(\"intro\",video_clip(\"clip.webm\",{}))\nvideo.pause(\"intro\")\nvideo.resume(\"intro\")\nvideo.seek(\"intro\",1+0.5)\nvideo.volume(\"intro\",0.5)\nvideo.wait(\"intro\")\nvideo.skip(\"intro\")\nvideo.stop(\"intro\")\n";
    let script = parse(source).unwrap();
    assert!(matches!(script[1], Statement::VideoPlay { .. }));
    assert_eq!(script.len(), 9);
    for source in [
        "init {ends:0}",
        "video.play(\"intro\")",
        "video.seek(\"intro\")",
        "video.unknown(\"intro\")",
    ] {
        assert!(parse(source).is_err());
    }
}
