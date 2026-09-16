use rvn_core::locale::LocaleTable;
use rvn_core::save::SaveData;
use rvn_core::types::SpriteState;
use rvn_core::{Engine, Interaction, LocaleManager, Renderer};
use rvn_parser::{Hotspot, Position, Transition};
#[derive(Default)]
struct Silent {
    dialogues: Vec<String>,
    choices: Vec<Vec<String>>,
}
impl Renderer for Silent {
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
    fn show_dialogue(&mut self, _: Option<&str>, text: &str) {
        self.dialogues.push(text.into());
    }
    fn show_choice(&mut self, options: &[String]) -> usize {
        self.choices.push(options.into());
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

#[test]
fn choice_save_keeps_dialogue_and_snapshot_values_in_both_languages() {
    let script=rvn_parser::parse("init { set n = 3 }\nlabel start\nnarrator \"Nombre [n]\"\nset n = 9\nchoice { \"Suite\" => { narrator \"Fin\" } }\n").unwrap();
    let locale = || {
        LocaleManager::from_tables(
            "unused",
            "fr",
            "fr",
            vec!["fr".into(), "en".into()],
            [(
                "en".into(),
                LocaleTable::parse(
                    "en",
                    "[strings]\n\"Nombre [n]\" = \"Number [n]\"\n\"Suite\" = \"Next\"\n",
                )
                .unwrap(),
            )]
            .into_iter()
            .collect(),
        )
    };
    let mut engine = Engine::new(script.clone(), Silent::default(), 16).unwrap();
    engine.locale = Some(locale());
    assert!(matches!(
        engine.step_until_interaction().unwrap(),
        Some(Interaction::Dialogue { .. })
    ));
    engine.advance_dialogue().unwrap();
    assert!(matches!(
        engine.step_until_interaction().unwrap(),
        Some(Interaction::Choice { .. })
    ));
    let saved = SaveData::from_state(&engine.state, 1, "test".into(), "story.rvn".into());
    let json = serde_json::to_string(&saved).unwrap();
    engine.locale.as_mut().unwrap().set_language("en").unwrap();
    assert!(engine.rollback());
    assert_eq!(engine.renderer.dialogues.last().unwrap(), "Number 3");
    assert_eq!(
        engine.renderer.choices.last().unwrap(),
        &vec!["Next".to_string()]
    );
    assert!(engine.rollback());
    assert_eq!(engine.renderer.dialogues.last().unwrap(), "Number 3");
    let mut restored = Engine::new(script, Silent::default(), 16).unwrap();
    restored.locale = Some(locale());
    restored
        .locale
        .as_mut()
        .unwrap()
        .set_language("en")
        .unwrap();
    restored.load_data(serde_json::from_str(&json).unwrap());
    assert!(
        matches!(restored.last_dialogue_interaction().unwrap(),Some(Interaction::Dialogue{text,..}) if text=="Number 3")
    );
    assert!(
        matches!(restored.current_interaction().unwrap(),Some(Interaction::Choice{options}) if options==vec!["Next"])
    );
    restored
        .locale
        .as_mut()
        .unwrap()
        .set_language("fr")
        .unwrap();
    assert!(
        matches!(restored.last_dialogue_interaction().unwrap(),Some(Interaction::Dialogue{text,..}) if text=="Nombre 3")
    );
    let mut old: serde_json::Value = serde_json::from_str(&json).unwrap();
    old.as_object_mut().unwrap().remove("last_dialogue");
    let old: SaveData = serde_json::from_value(old).unwrap();
    assert!(old.last_dialogue.is_none());
}
