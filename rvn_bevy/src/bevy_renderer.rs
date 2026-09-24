use rvn_core::{GameState, Renderer, SpriteState};
use rvn_parser::{AnimationParam, Hotspot, Position, Transition};

use crate::vn_command::VnCommand;

pub struct BevyRenderer {
    pub pending: Vec<VnCommand>,
    pub choice_result: Option<usize>,
    pub waiting_for_input: bool,
}

impl BevyRenderer {
    pub fn new() -> Self {
        Self {
            pending: Vec::new(),
            choice_result: None,
            waiting_for_input: false,
        }
    }

    pub fn take_pending(&mut self) -> Vec<VnCommand> {
        std::mem::take(&mut self.pending)
    }
}

#[cfg(test)]
mod restore_tests {
    use super::*;
    #[test]
    fn restoring_an_empty_cast_discards_previous_visual_and_music_commands() {
        let mut engine = rvn_core::Engine::new(vec![], BevyRenderer::new(), 10).unwrap();
        engine.renderer.pending.push(VnCommand::ShowSprite {
            id: "old_character".into(),
            emotion: None,
            position: rvn_parser::Position::Left,
            transition: Transition::None,
        });
        engine.renderer.restore_screen(&engine.state);
        let commands = engine.renderer.take_pending();
        assert!(matches!(commands.first(), Some(VnCommand::ClearSprites)));
        assert!(!commands
            .iter()
            .any(|cmd| matches!(cmd, VnCommand::ShowSprite { .. })));
        assert!(commands
            .iter()
            .any(|cmd| matches!(cmd, VnCommand::MusicStop)));
    }
}

impl Renderer for BevyRenderer {
    fn set_background(&mut self, path: &str, transition: &Transition) {
        self.pending.push(VnCommand::SetBackground {
            path: path.to_string(),
            transition: transition.clone(),
        });
    }

    fn show_cinematic(&mut self, id: &str, transition: Option<&str>) {
        self.pending.push(VnCommand::ShowCinematic {
            id: id.to_string(),
            transition: transition.map(str::to_string),
        });
    }

    fn hide_cinematic(&mut self, transition: Option<&str>) {
        self.pending.push(VnCommand::HideCinematic {
            transition: transition.map(str::to_string),
        });
    }

    fn unlock_ending(&mut self, id: &str) {
        self.pending
            .push(VnCommand::UnlockEnding { id: id.to_string() });
    }

    fn show_sprite(
        &mut self,
        id: &str,
        emotion: Option<&str>,
        position: &Position,
        transition: &Transition,
        _from: Option<&SpriteState>,
    ) {
        self.pending.push(VnCommand::ShowSprite {
            id: id.to_string(),
            emotion: emotion.map(str::to_string),
            position: position.clone(),
            transition: transition.clone(),
        });
    }

    fn hide_sprite(&mut self, id: &str, transition: &Transition, _from: &SpriteState) {
        self.pending.push(VnCommand::HideSprite {
            id: id.to_string(),
            transition: transition.clone(),
        });
    }

    fn move_sprite(
        &mut self,
        id: &str,
        position: &Position,
        transition: &Transition,
        _from: &SpriteState,
    ) {
        self.pending.push(VnCommand::MoveSprite {
            id: id.to_string(),
            position: position.clone(),
            transition: transition.clone(),
        });
    }

    fn animate_sprite(&mut self, id: &str, animation: &str, params: &[AnimationParam]) {
        self.pending.push(VnCommand::AnimateSprite {
            id: id.to_string(),
            animation: animation.to_string(),
            params: params.to_vec(),
        });
    }

    fn voice_play(&mut self, file: &str) {
        self.pending.push(VnCommand::VoicePlay {
            file: file.to_string(),
        });
    }

    fn voice_stop(&mut self) {
        self.pending.push(VnCommand::VoiceStop);
    }

    fn stop_sprite_animation(&mut self, id: &str) {
        self.pending
            .push(VnCommand::StopSpriteAnimation { id: id.to_string() });
    }

    fn show_dialogue(&mut self, character: Option<&str>, text: &str) {
        self.pending.push(VnCommand::ShowDialogue {
            character: character.map(str::to_string),
            text: text.to_string(),
        });
    }

    fn show_choice(&mut self, options: &[String]) -> usize {
        if let Some(idx) = self.choice_result.take() {
            self.waiting_for_input = false;
            idx
        } else {
            self.pending.push(VnCommand::ShowChoice {
                options: options.to_vec(),
            });
            self.waiting_for_input = true;
            0
        }
    }

    fn music_play(&mut self, file: &str, transition: &Transition, _previous: Option<&str>) {
        self.pending.push(VnCommand::MusicPlay {
            file: file.to_string(),
            transition: transition.clone(),
        });
    }

    fn music_stop(&mut self, _transition: &Transition) {
        self.pending.push(VnCommand::MusicStop);
    }

    fn music_set_volume(&mut self, level: f32) {
        self.pending.push(VnCommand::MusicSetVolume { level });
    }

    fn sfx_play(&mut self, file: &str, _transition: &Transition) {
        self.pending.push(VnCommand::SfxPlay {
            file: file.to_string(),
        });
    }

    fn sfx_stop(&mut self, file: &str, _transition: &Transition) {
        self.pending.push(VnCommand::SfxStop {
            file: file.to_string(),
        });
    }

    fn show_imagemap(
        &mut self,
        background: &str,
        hover_image: Option<&str>,
        hotspots: &[Hotspot],
    ) -> usize {
        if let Some(idx) = self.choice_result.take() {
            self.waiting_for_input = false;
            idx
        } else {
            let hs = hotspots
                .iter()
                .map(crate::vn_command::ImagemapZone::from)
                .collect();
            self.pending.push(VnCommand::ShowImagemap {
                background: background.to_string(),
                hover_image: hover_image.map(str::to_string),
                hotspots: hs,
            });
            self.waiting_for_input = true;
            0
        }
    }

    fn restore_screen(&mut self, state: &GameState) {
        self.pending.clear();
        self.pending.push(VnCommand::ClearSprites);
        self.pending.push(VnCommand::SetBackground {
            path: state.background_image.clone(),
            transition: Transition::None,
        });
        for (id, sprite) in &state.sprites {
            if sprite.visible {
                self.pending.push(VnCommand::ShowSprite {
                    id: id.clone(),
                    emotion: sprite.emotion.clone(),
                    position: sprite.position.clone(),
                    transition: Transition::None,
                });
            } else {
                self.pending.push(VnCommand::HideSprite {
                    id: id.clone(),
                    transition: Transition::None,
                });
            }
        }
        if let Some(id) = &state.cinematic.current {
            self.pending.push(VnCommand::ShowCinematic {
                id: id.clone(),
                transition: None,
            });
        } else {
            self.pending
                .push(VnCommand::HideCinematic { transition: None });
        }
        if let Some(file) = &state.music.current_file {
            self.pending.push(VnCommand::MusicPlay {
                file: file.clone(),
                transition: Transition::None,
            });
        } else {
            self.pending.push(VnCommand::MusicStop);
        }
    }

    fn set_typewriter_config(&mut self, speed_cps: f32) {
        self.pending
            .push(VnCommand::SetTypewriterConfig { speed_cps });
    }
}
