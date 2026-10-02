use rvn_core::{Engine, Renderer, SpriteState};
use rvn_parser::{parse, Hotspot, Position, Transition};
use rvn_ui::accessibility::{AccessibilitySettings, SpeechRequest};
#[derive(Default)]
struct Accessible {
    settings: AccessibilitySettings,
    speech: Vec<SpeechRequest>,
    unsupported: bool,
    reject_speech: bool,
    fail_speech: bool,
}
impl Renderer for Accessible {
    fn supports_programmable_ui(&self) -> bool {
        true
    }
    fn update_interfaces(&mut self, _: &[rvn_ui::programmable::ScreenView]) -> Result<(), String> {
        Ok(())
    }
    fn supports_accessibility(&self) -> bool {
        !self.unsupported
    }
    fn update_accessibility(&mut self, settings: &AccessibilitySettings) -> Result<(), String> {
        if self.unsupported && settings != &Default::default() {
            Err("Unsupported accessibility".into())
        } else {
            self.settings = settings.clone();
            Ok(())
        }
    }
    fn validate_accessibility_speech(&self, request: &SpeechRequest) -> Result<(), String> {
        if self.unsupported || self.reject_speech {
            Err("Speech unavailable".into())
        } else {
            if let SpeechRequest::Speak(text) = request {
                rvn_ui::accessibility::validate_speech(text)?;
            }
            Ok(())
        }
    }
    fn accessibility_speech(&mut self, request: &SpeechRequest) -> Result<(), String> {
        if self.fail_speech && matches!(request, SpeechRequest::Speak(_)) {
            return Err("Speech queue failed".into());
        }
        self.speech.push(request.clone());
        Ok(())
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
const SOURCE:&str="label start\naccessibility.configure({\"self_voicing\":true,\"text_scale\":1.5})\naccessibility.speak(\"Welcome\")\n\"First\"\naccessibility.configure({\"text_scale\":2})\naccessibility.stop()\n\"Second\"\n";
#[test]
fn failed_speech_never_commits_handler_variables_or_policy() {
    for reject in [true, false] {
        let source="init {set count = 0}\nhandler change(event) {set count = 1 accessibility.configure({\"text_scale\":2}) accessibility.speak(\"Welcome\")}\nscreen panel(){return component(\"button\",\"button\",{\"events\":{\"click\":\"change\"}},[])}\nlabel start\nui.open(\"panel\",[],false,0)\n\"Wait\"\n";
        let mut game = Engine::new(parse(source).unwrap(), Accessible::default(), 8).unwrap();
        game.step_until_interaction().unwrap();
        let before = game.state.pc;
        game.renderer.reject_speech = reject;
        game.renderer.fail_speech = !reject;
        assert!(game
            .interface_event(rvn_core::ui::UiInput {
                screen: "panel".into(),
                element: "button".into(),
                kind: rvn_ui::programmable::ScreenEventKind::Click,
                value: None,
                key: None
            })
            .is_err());
        assert_eq!(game.state.pc, before);
        assert_eq!(game.get_var("count"), Some(&rvn_parser::Value::Int(0)));
        assert_eq!(game.state.accessibility, AccessibilitySettings::default());
        assert_eq!(game.renderer.settings, AccessibilitySettings::default());
        assert!(game
            .renderer
            .speech
            .iter()
            .all(|request| matches!(request, SpeechRequest::Stop)));
    }
}
#[test]
fn load_and_rollback_restore_policy_without_replaying_ephemeral_speech() {
    let mut game = Engine::new(parse(SOURCE).unwrap(), Accessible::default(), 16).unwrap();
    game.step_until_interaction().unwrap();
    assert_eq!(
        game.renderer.speech,
        vec![SpeechRequest::Speak("Welcome".into())]
    );
    let data = rvn_core::save::SaveData::from_state(&game.state, 1, "".into(), "main.rvn".into());
    let serialized = serde_json::to_string(&data).unwrap();
    assert!(!serialized.contains("Welcome"));
    game.advance_dialogue().unwrap();
    game.step_until_interaction().unwrap();
    assert_eq!(game.state.accessibility.text_scale, 2.0);
    assert!(game.rollback());
    assert!(game.rollback());
    assert_eq!(game.state.accessibility.text_scale, 1.5);
    game.load_data(data.clone()).unwrap();
    assert_eq!(game.renderer.settings.text_scale, 1.5);
    assert_eq!(
        game.renderer
            .speech
            .iter()
            .filter(|request| matches!(request, SpeechRequest::Speak(_)))
            .count(),
        1
    );
    let mut invalid = data;
    invalid.accessibility.text_scale = 100.0;
    let before = game.state.pc;
    assert!(game.load_data(invalid).is_err());
    assert_eq!(game.state.pc, before);
}
#[test]
fn old_saves_default_and_unsupported_renderers_do_not_silently_accept_requests() {
    let mut game = Engine::new(
        parse("label start\n\"First\"\n").unwrap(),
        Accessible::default(),
        8,
    )
    .unwrap();
    game.step_until_interaction().unwrap();
    let mut json = serde_json::to_value(rvn_core::save::SaveData::from_state(
        &game.state,
        1,
        "".into(),
        "".into(),
    ))
    .unwrap();
    json.as_object_mut().unwrap().remove("accessibility");
    json["format_version"] = serde_json::json!(6);
    game.load_data(serde_json::from_value(json).unwrap())
        .unwrap();
    assert_eq!(game.state.accessibility, Default::default());
    for source in [
        "accessibility.configure({})",
        "accessibility.speak(\"Hello\")",
        "accessibility.stop()",
    ] {
        let mut game = Engine::new(
            parse(source).unwrap(),
            Accessible {
                unsupported: true,
                ..Default::default()
            },
            8,
        )
        .unwrap();
        assert!(game.step_until_interaction().is_err());
        assert_eq!(game.state.pc, 0);
        assert!(game.renderer.speech.is_empty());
    }
    for source in [
        "accessibility.configure({\"speech_rate\":20})",
        "accessibility.speak(\"  \")",
    ] {
        let mut game = Engine::new(parse(source).unwrap(), Accessible::default(), 8).unwrap();
        assert!(game.step_until_interaction().is_err());
        assert_eq!(game.state.pc, 0);
    }
}
