//! Reached Dialogue consumers retain real input ports across redraw/save/next.
use super::*;
use crate::consumer_schedule::{BoundConsumerDialogue, BoundConsumerFrame, ConsumerOwner};
impl<R: Renderer> Engine<R> {
    pub(super) fn consumer_dialogue_parts(
        statement: &Statement,
    ) -> Option<(&Option<String>, &rvn_parser::InterpolatedText)> {
        match statement {
            Statement::ScheduledConsumer {
                version: 2, body, ..
            } => match body.as_slice() {
                [Statement::Dialogue { character_id, text }] => Some((character_id, text)),
                _ => None,
            },
            _ => None,
        }
    }
    pub(super) fn dialogue_statement(statement: &Statement) -> bool {
        matches!(statement, Statement::Dialogue { .. })
            || Self::consumer_dialogue_parts(statement).is_some()
    }
    fn dialogue_owner(&self) -> ConsumerOwner {
        self.state
            .scene_execution
            .as_ref()
            .and_then(|e| e.owner())
            .map(|activation| ConsumerOwner::Scene { activation })
            .unwrap_or(ConsumerOwner::Game)
    }
    pub(super) fn cached_consumer_dialogue(&self) -> Option<&BoundConsumerDialogue> {
        if self.state.display_random_pc != Some(self.state.pc) {
            return None;
        }
        let data = self.state.consumer_dialogue.as_ref().or_else(|| {
            self.state
                .scene_execution
                .as_ref()?
                .display
                .as_ref()?
                .consumer_dialogue
                .as_ref()
        })?;
        (data.pc == self.state.pc && data.owner == self.dialogue_owner()).then_some(data)
    }
    pub(super) fn retained_dialogue(
        &self,
        data: &BoundConsumerDialogue,
    ) -> Result<Interaction, RuntimeError> {
        self.consumer_program
            .validate_emitted(&self.script)
            .map_err(|e| self.eval_err(e, "Frozen Dialogue programme"))?;
        self.consumer_program
            .validate_dialogue(data)
            .map_err(|e| self.eval_err(e, "Frozen Dialogue ports"))?;
        let (character, text) = self
            .script
            .get(data.pc)
            .and_then(Self::consumer_dialogue_parts)
            .ok_or_else(|| {
                self.scene_resource_error("Retained Dialogue has no actual source site".into())
            })?;
        let key = text_to_locale_key(text);
        let translated = self.translate(&key);
        let text = self
            .functions
            .render_retained_choice_label(
                text,
                &data.values,
                self.locale.as_ref(),
                (translated != key).then_some(translated),
            )
            .map_err(|e| self.eval_err(e, "Frozen Dialogue render"))?;
        Ok(Interaction::Dialogue {
            character: character.clone(),
            text,
        })
    }
    pub(super) fn record_consumer_dialogue(
        &mut self,
        statement: &Statement,
    ) -> Result<(), RuntimeError> {
        if let Some(data) = self.cached_consumer_dialogue() {
            self.consumer_program
                .validate_dialogue(data)
                .map_err(|e| self.eval_err(e, "Frozen Dialogue visit"))?;
            if self.history.latest_entry_id().is_none() {
                let Interaction::Dialogue { character, text } = self.retained_dialogue(data)?
                else {
                    unreachable!()
                };
                self.history.push_audio_entry(
                    self.state.clone(),
                    Some(HistoryDisplay::Dialogue { character, text }),
                );
                self.display_history_entry = if self.narrative.is_some()||self.state.game_audio.is_some() {
                    self.history.latest_entry_id()
                } else {
                    None
                };
                if self.state.game_audio.is_some(){self.history.arm_audio_publication(&self.state);}
                self.legacy_preview_visit = if self.narrative.is_none() {
                    Some(LegacyPreviewVisit::capture(
                        &self.state,
                        self.history.latest_entry_id(),
                    ))
                } else {
                    None
                };
            }
            return Ok(());
        }
        self.consumer_program
            .validate_plan_binding(statement, self.state.pc, false)
            .map_err(|e| self.eval_err(e, "Scheduled Dialogue binding"))?;
        let generation = self.display_generation.checked_add(1).ok_or_else(|| {
            self.eval_err(EvalError::NumericOverflow, "Dialogue display generation")
        })?;
        let Statement::ScheduledConsumer { scope, .. } = statement else {
            unreachable!()
        };
        let owner = self.dialogue_owner();
        let identity = self
            .consumer_program
            .identity()
            .map_err(|e| self.eval_err(e, "Dialogue programme"))?;
        let scope = self
            .consumer_program
            .bound_scope(scope, self.state.pc)
            .map_err(|e| self.eval_err(e, "Dialogue source scope"))?;
        let environment = self.vars_for_eval();
        let globals = self.lexical_vars_for_eval();
        let mut next = self.state.clone();
        let slot = match next
            .scene_execution
            .as_mut()
            .and_then(|e| e.active.as_mut())
        {
            Some(a) => &mut a.consumer_frame,
            None => &mut next.consumer_frame,
        };
        let frame = slot.get_or_insert_with(|| BoundConsumerFrame::new(identity, owner));
        frame
            .validate_binding(identity, owner)
            .map_err(|e| self.eval_err(e, "Dialogue activation"))?;
        let values = self
            .functions
            .eval_engine_consumer(
                statement,
                &environment,
                &globals,
                &mut next.random,
                &mut frame.frame,
                &scope,
                self.locale.as_ref(),
            )
            .map_err(|e| self.eval_err(e, "Dialogue direct inputs"))?;
        let data = BoundConsumerDialogue::from_value(identity, owner, self.state.pc, values)
            .map_err(|e| self.eval_err(e, "Dialogue typed ports"))?;
        let interaction = self.retained_dialogue(&data)?;
        let Interaction::Dialogue { character, text } = interaction.clone() else {
            unreachable!()
        };
        let display = HistoryDisplay::Dialogue { character, text };
        next.current_interactive_pc = next.pc;
        next.display_random_pc = Some(next.pc);
        next.display_random = next.random;
        next.consumer_choice = None;
        next.consumer_imagemap = None;
        next.last_dialogue = Some(crate::types::DialogueSnapshot {
            consumer_dialogue: Some(data.clone()),
            pc: next.pc,
            vars: environment.clone(),
            lexical_globals: self.narrative.as_ref().map(|_| globals.clone()),
            random: next.display_random,
        });
        if let Some(program) = &self.narrative {
            next.consumer_dialogue = None;
            next.scene_execution.as_mut().unwrap().display =
                Some(crate::scene::DisplayCheckpoint {
                    consumer_dialogue: Some(data),
                    consumer_choice: None, consumer_imagemap:None,
                    position: program.position(next.pc),
                    variables: environment,
                    lexical_globals: globals,
                    random: next.display_random,
                    typewriter: next.typewriter.clone(),
                    display: display.clone(),
                    choice_indices: vec![],
                    reveal: None,
                });
            self.commit_scene_state(next)?;
        } else {
            next.consumer_dialogue = Some(data);
            let cache=next.game_audio.is_some().then(||self.audio_interface_cache_transaction()).transpose()?;
            let mut batch = self.build_scene_batch(next.clone(),self.runtime_epoch,self.interface_epoch,self.video_epoch)?;
            batch.display = Some(interaction);
            self.renderer
                .validate_scene_batch(&batch)
                .map_err(|e| self.scene_resource_error(e))?;
            cache.map(|transaction|transaction.commit()).unwrap_or(());
            self.state = next;
            self.renderer.publish_scene_batch(batch);
        }
        self.display_generation = generation;
        self.display_owner = self.state.scene_execution.as_ref().and_then(|e| e.owner());
        self.history
            .push_audio_entry(self.state.clone(), Some(display));
        self.display_history_entry = if self.narrative.is_some()||self.state.game_audio.is_some() {
            self.history.latest_entry_id()
        } else {
            None
        };
        if self.state.game_audio.is_some(){self.history.arm_audio_publication(&self.state);}
        self.legacy_preview_visit = if self.narrative.is_none() {
            Some(LegacyPreviewVisit::capture(
                &self.state,
                self.history.latest_entry_id(),
            ))
        } else {
            None
        };
        Ok(())
    }
    pub(super) fn advance_consumer_dialogue(&mut self) -> Result<(), RuntimeError> {
        let mut next = self.state.clone();
        let displayed = next
            .scene_execution
            .as_ref()
            .and_then(|e| e.display.as_ref())
            .map(|d| d.position.pc);
        if displayed.is_some_and(|pc| pc != next.pc) {
            next.scene_execution.as_mut().unwrap().display = None;
            return self.commit_scheduled_state(next);
        }
        let data = self.cached_consumer_dialogue().ok_or_else(|| {
            self.scene_resource_error("Dialogue input requires its actual reached visit".into())
        })?;
        self.consumer_program
            .validate_dialogue(data)
            .map_err(|e| self.eval_err(e, "Dialogue input binding"))?;
        if let Some(execution) = &mut next.scene_execution {
            execution.display = None;
        }
        next.consumer_dialogue = None;
        next.display_random_pc = None;
        next.pc += 1;
        self.commit_scheduled_state(next)
    }
}
