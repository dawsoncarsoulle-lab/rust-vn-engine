use rvn_core::video::{Feedback, Playback, VideoView};
use rvn_core::{Engine, Renderer, SpriteState};
use rvn_parser::{parse, Hotspot, Position, Transition};
#[derive(Default)]
struct Media {
    views: Vec<VideoView>,
    reject: bool,
}
impl Renderer for Media {
    fn supports_video(&self) -> bool {
        true
    }
    fn update_videos(&mut self, views: &[VideoView]) -> Result<(), String> {
        if self.reject {
            Err("Renderer rejected video".into())
        } else {
            self.views = views.to_vec();
            Ok(())
        }
    }
    fn set_background(&mut self, _: &str, _: &Transition) {}
    fn show_sprite(
        &mut self,
        _: &str,
        _: Option<&str>,
        _: &Position,
        _: &Transition,
        _: Option<&SpriteState>,
    ) {
    }
    fn hide_sprite(&mut self, _: &str, _: &Transition, _: &SpriteState) {}
    fn move_sprite(&mut self, _: &str, _: &Position, _: &Transition, _: &SpriteState) {}
    fn show_dialogue(&mut self, _: Option<&str>, _: &str) {}
    fn show_choice(&mut self, _: &[String]) -> usize {
        0
    }
    fn music_play(&mut self, _: &str, _: &Transition, _: Option<&str>) {}
    fn music_stop(&mut self, _: &Transition) {}
    fn music_set_volume(&mut self, _: f32) {}
    fn sfx_play(&mut self, _: &str, _: &Transition) {}
    fn sfx_stop(&mut self, _: &str, _: &Transition) {}
    fn show_imagemap(&mut self, _: &str, _: Option<&str>, _: &[Hotspot]) -> usize {
        0
    }
}
const PROGRAM: &str = r#"
init {set ends = 0}
handler completed(event){set ends = ends+1}
label start
video.play("intro",video_clip("clip.webm",{"skippable":true,"on_end":"completed"}))
"First"
video.pause("intro")
video.seek("intro",0.5)
video.volume("intro",0.4)
"Paused"
video.resume("intro")
video.wait("intro")
"Finished"
video.stop("intro")
"Closed"
"#;
fn engine() -> Engine<Media> {
    Engine::new(parse(PROGRAM).unwrap(), Media::default(), 16).unwrap()
}
#[test]
fn controls_saves_rollback_events_and_old_decoder_callbacks() {
    let mut game = engine();
    game.step_until_interaction().unwrap();
    let epoch = game.renderer.views[0].epoch;
    game.video_feedback(
        epoch,
        "intro",
        Feedback::Position {
            seconds: 0.25,
            duration: 1.6,
        },
    )
    .unwrap();
    let save =
        rvn_core::save::SaveData::from_state(&game.state, 1, "Video".into(), "main.rvn".into());
    game.advance_dialogue().unwrap();
    game.step_until_interaction().unwrap();
    assert_eq!(game.state.videos.tracks["intro"].playback, Playback::Paused);
    assert_eq!(game.state.videos.tracks["intro"].position, 0.5);
    assert!(!game.video_feedback(epoch, "intro", Feedback::End).unwrap());
    assert_eq!(game.state.vars["ends"], rvn_parser::Value::Int(0));
    game.advance_dialogue().unwrap();
    assert!(game.step_until_interaction().unwrap().is_none());
    assert_eq!(game.state.videos.waiting.as_deref(), Some("intro"));
    let current = game.renderer.views[0].epoch;
    game.video_feedback(current, "intro", Feedback::End)
        .unwrap();
    assert!(game.state.videos.waiting.is_none());
    assert_eq!(game.state.vars["ends"], rvn_parser::Value::Int(1));
    assert!(!game
        .video_feedback(current, "intro", Feedback::End)
        .unwrap());
    game.step_until_interaction().unwrap();
    game.advance_dialogue().unwrap();
    game.step_until_interaction().unwrap();
    assert!(game.state.videos.tracks.is_empty());
    assert!(game.rollback());
    assert!(game.rollback());
    assert_eq!(game.state.videos.tracks["intro"].playback, Playback::Ended);
    game.load_data(serde_json::from_value(serde_json::to_value(save).unwrap()).unwrap())
        .unwrap();
    assert_eq!(game.state.videos.tracks["intro"].position, 0.25);
    assert_eq!(game.state.vars["ends"], rvn_parser::Value::Int(0));
    assert!(!game.video_feedback(epoch, "intro", Feedback::End).unwrap());
}
#[test]
fn cinematic_save_wait_autoplay_refusal_and_end_do_not_restart_the_clip() {
    let source="label start\nvideo.play(\"intro\",video_clip(\"clip.webm\",{\"cinematic\":true}))\n\"After\"\n";
    let mut game = Engine::new(parse(source).unwrap(), Media::default(), 8).unwrap();
    assert!(game.step_until_interaction().unwrap().is_none());
    let epoch = game.renderer.views[0].epoch;
    game.video_feedback(
        epoch,
        "intro",
        Feedback::Position {
            seconds: 0.7,
            duration: 1.6,
        },
    )
    .unwrap();
    let waiting_pc = game.state.pc;
    game.advance_dialogue().unwrap();
    game.submit_choice(0).unwrap();
    assert_eq!(game.state.pc, waiting_pc);
    game.video_feedback(
        epoch,
        "intro",
        Feedback::AutoplayBlocked("Click to start the video".into()),
    )
    .unwrap();
    assert!(game.state.videos.waiting.is_some());
    assert!(!game.is_finished());
    let save = rvn_core::save::SaveData::from_state(&game.state, 1, "".into(), "".into());
    let pc = game.state.pc;
    game.load_data(save).unwrap();
    assert_eq!(game.state.pc, pc);
    assert_eq!(game.state.videos.tracks["intro"].position, 0.7);
    let epoch = game.renderer.views[0].epoch;
    game.video_feedback(epoch, "intro", Feedback::End).unwrap();
    assert!(game.step_until_interaction().unwrap().is_some());
    assert_eq!(game.state.videos.tracks["intro"].playback, Playback::Ended);
}
#[test]
fn unsupported_renderers_invalid_descriptors_and_failures_are_atomic() {
    let mut unsupported =
        Engine::new(parse(PROGRAM).unwrap(), rvn_core::TerminalRenderer, 8).unwrap();
    assert!(unsupported.step_until_interaction().is_err());
    assert!(unsupported.state.videos.tracks.is_empty());
    let mut game = engine();
    game.renderer.reject = true;
    let before = serde_json::to_value(&game.state).unwrap();
    assert!(game.step_until_interaction().is_err());
    assert_eq!(game.state.videos.tracks.len(), 0);
    // Execution passed labels but did not commit any media or variable data.
    assert_eq!(
        serde_json::to_value(&game.state.vars).unwrap(),
        before["vars"]
    );
    game.renderer.reject = false;
    game.step_until_interaction().unwrap();
    let before = serde_json::to_value(&game.state).unwrap();
    let mut save = rvn_core::save::SaveData::from_state(&game.state, 1, "".into(), "".into());
    save.videos.tracks.get_mut("intro").unwrap().clip.source = "../secret.webm".into();
    assert!(game.load_data(save).is_err());
    assert_eq!(serde_json::to_value(&game.state).unwrap(), before);
    assert!(parse("function bad(){video.wait(\"intro\")}\nlabel start").is_err());
}
#[test]
fn prior_saves_default_to_no_video_state() {
    let game = engine();
    let mut value = serde_json::to_value(rvn_core::save::SaveData::from_state(
        &game.state,
        1,
        "".into(),
        "".into(),
    ))
    .unwrap();
    value.as_object_mut().unwrap().remove("videos");
    value["format_version"] = serde_json::json!(5);
    let save: rvn_core::save::SaveData = serde_json::from_value(value).unwrap();
    assert!(save.videos.tracks.is_empty());
}
