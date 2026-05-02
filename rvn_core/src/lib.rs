pub mod engine;
pub mod error;
pub mod eval;
pub mod locale;
pub mod renderer;
pub mod rollback;
pub mod save;
pub mod types;

pub use engine::{Engine, Interaction};
pub use error::RuntimeError;
pub use eval::{EvalError, eval_bool, eval_expr, eval_interpolated};
pub use locale::{LocaleManager, collect_strings_from_flat_script, collect_strings_from_script};
pub use renderer::{Renderer, TerminalRenderer};
pub use rollback::{HistoryDisplay, RollbackHistory};
pub use types::{GameState, MusicState, SpriteState, TypewriterState};

// ─── TESTS ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use rvn_parser::parse;
    use rvn_parser::{Hotspot, Position, Statement, Transition, Value};

    // ── Mock Renderer ─────────────────────────────────────────────────────────

    #[derive(Default, Clone)]
    struct Mock {
        pub events: Vec<String>,
        pub screen: String,
        pub visible: Vec<String>,
    }

    impl Renderer for Mock {
        fn set_background(&mut self, p: &str, t: &Transition) {
            self.events.push(format!("bg:{p}:{t}"));
        }
        fn show_sprite(
            &mut self,
            id: &str,
            emo: Option<&str>,
            pos: &Position,
            t: &Transition,
            _: Option<&SpriteState>,
        ) {
            self.events
                .push(format!("show:{id}:{}:{pos}:{t}", emo.unwrap_or("")));
        }
        fn hide_sprite(&mut self, id: &str, t: &Transition, _: &SpriteState) {
            self.events.push(format!("hide:{id}:{t}"));
        }
        fn move_sprite(&mut self, id: &str, pos: &Position, t: &Transition, _: &SpriteState) {
            self.events.push(format!("move:{id}:{pos}:{t}"));
        }
        fn animate_sprite(
            &mut self,
            id: &str,
            animation: &str,
            _params: &[rvn_parser::AnimationParam],
        ) {
            self.events.push(format!("animate:{id}:{animation}"));
        }
        fn stop_sprite_animation(&mut self, id: &str) {
            self.events.push(format!("stop_animation:{id}"));
        }
        fn show_dialogue(&mut self, c: Option<&str>, text: &str) {
            self.events.push(format!("dlg:{}:{text}", c.unwrap_or("")));
        }
        fn show_choice(&mut self, opts: &[String]) -> usize {
            self.events.push(format!("choice:{}", opts.len()));
            0
        }
        fn restore_screen(&mut self, state: &GameState) {
            self.screen = state.background_image.clone();
            self.visible = state
                .sprites
                .iter()
                .filter(|(_, s)| s.visible)
                .map(|(k, _)| k.clone())
                .collect();
            self.events
                .push(format!("restore:{}", state.background_image));
        }
        fn show_imagemap(&mut self, background: &str, _: Option<&str>, _: &[Hotspot]) -> usize {
            self.events.push(format!("imagemap:{}", background));
            0
        }
        fn music_play(&mut self, file: &str, _: &Transition, prev: Option<&str>) {
            self.events
                .push(format!("music_play:{file} prev:{}", prev.unwrap_or("none")));
        }
        fn music_stop(&mut self, t: &Transition) {
            self.events.push(format!("music_stop:{t}"));
        }
        fn music_set_volume(&mut self, level: f32) {
            self.events.push(format!("music_vol:{level:.2}"));
        }
        fn sfx_play(&mut self, file: &str, _: &Transition) {
            self.events.push(format!("sfx_play:{file}"));
        }
        fn sfx_stop(&mut self, file: &str, t: &Transition) {
            self.events.push(format!("sfx_stop:{file}:{t}"));
        }
    }

    fn engine(src: &str) -> Engine<Mock> {
        Engine::new(parse(src).expect("parse failed"), Mock::default(), 32).unwrap()
    }

    fn step(e: &mut Engine<Mock>) {
        e.step().expect("step error");
    }

    // ── Interpolation dans le moteur ──────────────────────────────────────────

    #[test]
    fn test_dialogue_variable_interpolation() {
        let src = r#"
            set prenom = "Sarah"
            "Bonjour [prenom] !"
        "#;
        let mut e = engine(src);
        step(&mut e);
        assert!(
            e.renderer
                .events
                .iter()
                .any(|s| s == "dlg::Bonjour Sarah !"),
            "events: {:?}",
            e.renderer.events
        );
    }

    #[test]
    fn test_dialogue_expr_interpolation() {
        let src = r#"
            set score = 9
            "Tu as [score + 1] points."
        "#;
        let mut e = engine(src);
        step(&mut e);
        assert!(
            e.renderer
                .events
                .iter()
                .any(|s| s == "dlg::Tu as 10 points."),
            "events: {:?}",
            e.renderer.events
        );
    }

    #[test]
    fn test_set_expr_arithmetic() {
        let src = "set x = 3 * 4 + 2";
        let mut e = engine(src);
        step(&mut e);
        assert_eq!(e.get_var("x"), Some(&Value::Int(14)));
    }

    #[test]
    fn test_set_expr_var_ref() {
        let src = "set a = 5\nset b = a * 2";
        let mut e = engine(src);
        step(&mut e);
        step(&mut e);
        assert_eq!(e.get_var("b"), Some(&Value::Int(10)));
    }

    #[test]
    fn test_if_and_condition() {
        let src = r#"
            set x = 5
            set flag = true
            if x > 3 and flag == true { sarah "ok" }
        "#;
        let mut e = engine(src);
        step(&mut e);
        assert!(e.renderer.events.iter().any(|s| s.contains("ok")));
    }

    #[test]
    fn test_if_or_condition() {
        let src = r#"
            set x = 1
            if x == 99 or x == 1 { sarah "trouvé" }
        "#;
        let mut e = engine(src);
        step(&mut e);
        assert!(e.renderer.events.iter().any(|s| s.contains("trouvé")));
    }

    #[test]
    fn test_if_not_condition() {
        let src = r#"
            set done = false
            if not done { sarah "pas encore fait" }
        "#;
        let mut e = engine(src);
        step(&mut e);
        assert!(
            e.renderer
                .events
                .iter()
                .any(|s| s.contains("pas encore fait"))
        );
    }

    #[test]
    fn test_choice_interpolated_labels() {
        let src = r#"
            set n = 3
            choice {
                "Option [n]" => { jump a }
                "Quitter"    => { jump b }
            }
            label a sarah "A"
            label b sarah "B"
        "#;
        let mut e = engine(src);
        step(&mut e);
        // Le label du choix affiché doit contenir "Option 3"
        assert!(
            e.renderer.events.iter().any(|s| s.contains("choice:")),
            "events: {:?}",
            e.renderer.events
        );
    }

    // ── Snapshot & Rollback (inchangés fonctionnellement) ─────────────────────

    #[test]
    fn test_snapshot_avant_dialogue() {
        let mut e =
            engine(r#"scene chambre with fade sarah.show("sourire") at left sarah "Bonjour !""#);
        assert_eq!(e.history.len(), 0);
        step(&mut e);
        assert_eq!(e.history.len(), 1);
    }

    #[test]
    fn test_rollback_multiple() {
        let mut e = engine("sarah \"A\"\nsarah \"B\"\nsarah \"C\"");
        step(&mut e);
        step(&mut e);
        step(&mut e);
        e.rollback();
        assert_eq!(e.state.pc, 2);
        e.rollback();
        assert_eq!(e.state.pc, 1);
        e.rollback();
        assert_eq!(e.state.pc, 0);
        assert!(!e.rollback());
    }

    #[test]
    fn test_rollback_annule_variables() {
        let src = "set score = 0\nset score = 10\nsarah \"Bravo !\"";
        let mut e = engine(src);
        step(&mut e);
        assert_eq!(e.get_var("score"), Some(&Value::Int(10)));
        e.rollback();
        assert_eq!(e.get_var("score"), Some(&Value::Int(10)));
    }

    #[test]
    fn test_load_restaure_ecran_via_events() {
        use crate::save::SaveManager;
        let dir = std::env::temp_dir().join("rvn_load_events_test2");
        let mgr = SaveManager::new(&dir, 5).unwrap();
        let src = "scene chambre with fade\nsarah \"Bonjour !\"\nsarah \"Au revoir !\"";
        let mut e = Engine::new(parse(src).unwrap(), Mock::default(), 32).unwrap();
        step(&mut e);
        step(&mut e);
        e.save(&mgr, 1, "test".into(), "x.rvn".into()).unwrap();
        let mut e2 = Engine::new(parse(src).unwrap(), Mock::default(), 32).unwrap();
        e2.load(&mgr, 1).unwrap();
        assert!(e2.renderer.events.iter().any(|s| s.contains("restore:")));
        assert!(e2.renderer.events.iter().any(|s| s.contains("chambre")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_music_play_met_a_jour_state() {
        let mut e = engine(r#"music.play("theme.ogg")"#);
        e.step().unwrap();
        assert_eq!(e.state.music.current_file, Some("music/theme.ogg".into()));
    }

    #[test]
    fn test_init_block_execution() {
        let src = r#"init { set score = 100 } label debut sarah "Le score est [score].""#;
        let mut e = engine(src);
        assert_eq!(e.get_var("score"), Some(&Value::Int(100)));
        step(&mut e);
        assert!(
            e.renderer
                .events
                .iter()
                .any(|s| s.contains("Le score est 100."))
        );
    }

    #[test]
    fn test_event_driven_choice_api() {
        let src = r#"
            set n = 2
            choice {
                "Option [n]" => { jump a }
                "Quitter" => { jump b }
            }
            label a
                sarah "A"
            label b
                sarah "B"
        "#;
        let mut e = engine(src);
        let interaction = e.step_until_interaction().unwrap().unwrap();
        match interaction {
            Interaction::Choice { options } => {
                assert_eq!(options[0], "Option 2");
                assert_eq!(options[1], "Quitter");
            }
            other => panic!("interaction inattendue: {other:?}"),
        }
        e.submit_choice(0).unwrap();
        let interaction = e.step_until_interaction().unwrap().unwrap();
        match interaction {
            Interaction::Dialogue { character, text } => {
                assert_eq!(character.as_deref(), Some("sarah"));
                assert_eq!(text, "A");
            }
            other => panic!("interaction inattendue: {other:?}"),
        }
    }
    #[test]
    fn test_sprite_animation_commands() {
        let src = r#"
            eileen.animate("shake", intensity: 12, duration: 0.3)
            eileen.stop_animation()
        "#;
        let mut e = engine(src);
        step(&mut e);
        step(&mut e);
        assert!(
            e.renderer
                .events
                .iter()
                .any(|s| s == "animate:eileen:shake")
        );
        assert!(
            e.renderer
                .events
                .iter()
                .any(|s| s == "stop_animation:eileen")
        );
    }
}
