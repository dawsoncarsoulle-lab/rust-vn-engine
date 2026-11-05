use super::*;
use crate::display_reveal::{DisplayRevealSource, RevealCheckpoint};

impl<R: Renderer> Engine<R> {
    /// Capture at the actual producer, never while dequeuing an old command.
    pub fn display_reveal_source(&self) -> Option<DisplayRevealSource> {
        let execution = self.state.scene_execution.as_ref()?;
        let display = execution.display.as_ref()?;
        if !matches!(display.display, HistoryDisplay::Dialogue { .. }) { return None; }
        Some(DisplayRevealSource {
            engine_instance: self.instance_id(), runtime_epoch: self.runtime_epoch,
            display_generation: self.display_generation, position: display.position.clone(),
            owner: self.display_owner,
        })
    }

    pub fn accepts_display_reveal_source(&self, source: &DisplayRevealSource) -> bool {
        self.display_reveal_source().as_ref() == Some(source)
    }

    /// Late authority is neutral before inspecting text or numeric payload.
    /// Accepted feedback only updates observed progress and its current rollback
    /// entry. It never replays RVN, advances RNG, PC or creates history entries.
    pub fn record_display_reveal(&mut self, source: &DisplayRevealSource, reveal: RevealCheckpoint) -> Result<bool, RuntimeError> {
        if !self.accepts_display_reveal_source(source) { return Ok(false); }
        reveal.validate().map_err(|message| self.eval_err(EvalError::InvalidFunction(message), "Dialogue reveal"))?;
        let current = self.displayed_interaction()?;
        if !matches!(current, Some(Interaction::Dialogue { ref text, .. }) if text == &reveal.rendered_text) {
            return Ok(false);
        }
        self.state.scene_execution.as_mut().unwrap().display.as_mut().unwrap().reveal = Some(reveal.clone());
        self.history.record_display_reveal(self.display_history_entry, &source.position, source.owner, &reveal);
        Ok(true)
    }
}

#[cfg(test)] mod generation_tests {
use super::*;
use crate::{Renderer,SpriteState};
use crate::renderer::SceneRenderBatch;
use rvn_parser::{Hotspot,Position,Transition};
#[derive(Default)]
struct Host {
    batches: Vec<SceneRenderBatch>,
    reject: bool,
}
impl Renderer for Host {
    fn supports_video(&self) -> bool {
        true
    }
    fn supports_programmable_ui(&self) -> bool {
        true
    }
    fn supports_composable_motion(&self) -> bool {
        true
    }
    fn supports_custom_canvas(&self) -> bool {
        true
    }
    fn validate_scene_batch(&self, _: &SceneRenderBatch) -> Result<(), String> {
        if self.reject {
            Err("Rejected exact resident projection".into())
        } else {
            Ok(())
        }
    }
    fn publish_scene_batch(&mut self, batch: SceneRenderBatch) {
        self.batches.push(batch);
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
    fn show_imagemap(&mut self, _: &str, _: Option<&str>, _: &[Hotspot]) -> usize {
        0
    }
    fn music_play(&mut self, _: &str, _: &Transition, _: Option<&str>) {}
    fn music_stop(&mut self, _: &Transition) {}
    fn music_set_volume(&mut self, _: f32) {}
    fn sfx_play(&mut self, _: &str, _: &Transition) {}
    fn sfx_stop(&mut self, _: &str, _: &Transition) {}
}


    #[test]
    fn reveal_generation_exhaustion_refuses_new_interaction_and_restore_before_state_history_or_publication() {
        let mut game=Engine::from_program(rvn_parser::parse_program("scene label P\n\"line\"\nreturn\n").unwrap(),Host::default(),32).unwrap();
        game.step_until_interaction().unwrap();
        let saved=crate::save::SaveData::try_from_state(&game.state,1,"saved".into(),"main.rvn".into()).unwrap();
        game.display_generation=u64::MAX;
        let before=serde_json::json!({"state":game.state,"history":game.history.entries().iter().map(|entry|(&entry.state,&entry.display)).collect::<Vec<_>>(),"source":format!("{:?}",game.display_reveal_source()),"runtime":game.runtime_epoch(),"interface":game.interface_epoch(),"batches":game.renderer.batches.len()});
        let statement=game.script[game.state.current_interactive_pc].clone();
        assert!(game.record_interaction_snapshot(&statement).is_err());assert!(game.load_data(saved).is_err());
        let after=serde_json::json!({"state":game.state,"history":game.history.entries().iter().map(|entry|(&entry.state,&entry.display)).collect::<Vec<_>>(),"source":format!("{:?}",game.display_reveal_source()),"runtime":game.runtime_epoch(),"interface":game.interface_epoch(),"batches":game.renderer.batches.len()});
        assert_eq!(after,before);assert_eq!(game.display_generation,u64::MAX);
    }

}
