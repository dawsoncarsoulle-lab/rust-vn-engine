//! Explicit debugger command boundary; ordinary player stepping is unchanged.
use super::*;
#[derive(Clone,Debug)]
pub(super) struct LegacyPreviewVisit {
    pc:usize,
    interactive_pc:usize,
    random_pc:Option<usize>,
    history_entry:Option<u64>,
}
impl LegacyPreviewVisit {
    pub(super) fn capture(state:&GameState,history_entry:Option<u64>)->Self {
        Self{pc:state.pc,interactive_pc:state.current_interactive_pc,random_pc:state.display_random_pc,history_entry}
    }
    fn current(&self,state:&GameState)->bool {
        self.pc==state.pc&&self.interactive_pc==state.current_interactive_pc&&self.random_pc==state.display_random_pc
    }
}
impl<R: Renderer> Engine<R> {
    pub(super) fn restore_legacy_preview_visit(&mut self,generation:u64){
        self.display_generation=generation;
        self.legacy_preview_visit=self.current_interaction().ok().flatten()
            .map(|_|LegacyPreviewVisit::capture(&self.state,None));
    }
    pub fn preview_input_source(&self) -> Option<crate::editor_preview::InteractionSource> {
        let (position, generation, owner, entry) =
            if let Some(execution) = &self.state.scene_execution {
                let display = execution.display.as_ref()?;
                (
                    display.position.clone(),
                    Some(self.display_generation),
                    self.display_owner,
                    self.display_history_entry,
                )
            } else {
                let visit=self.legacy_preview_visit.as_ref().filter(|visit|visit.current(&self.state))?;
                (
                    crate::scene::ResumePosition {
                        pc: self.state.pc,
                        origin: None,
                    },
                    Some(self.display_generation),
                    None,
                    visit.history_entry,
                )
            };
        Some(crate::editor_preview::InteractionSource {
            instance: self.instance_id(),
            epoch: self.runtime_epoch(),
            position,
            generation,
            history_entry: entry,
            choice_indices:self.active_choice_indices().ok()?,
            owner,
        })
    }
    pub fn accepts_preview_input_source(
        &self,
        source: &crate::editor_preview::InteractionSource,
    ) -> bool {
        self.preview_input_source().as_ref() == Some(source)
    }
    /// Executes one prepared top-level statement through the existing engine.
    /// An already displayed interaction or media wait remains an input boundary;
    /// this command never chooses a default answer or advances a saved clock.
    pub fn preview_step_statement(&mut self) -> Result<Option<Interaction>, RuntimeError> {
        if self.state.motions.waiting.is_some()
            || self.state.videos.waiting.is_some()
            || self.is_finished()
        {
            return Ok(None);
        }
        if self
            .state
            .scene_execution
            .as_ref()
            .is_some_and(|execution| execution.display.as_ref().is_some_and(|display|
                display.position.pc == self.state.pc && self.state.display_random_pc == Some(self.state.pc)))
        {
            return self.displayed_interaction();
        }
        let stmt = self.script[self.state.pc].clone();
        if Self::is_interactive(&stmt) && self.state.display_random_pc == Some(self.state.pc)
            && self.preview_input_source().is_some() {
            return self.current_interaction();
        }
        if matches!(&stmt,Statement::Choice{options}if !options.is_empty())
            && self.state.display_random_pc != Some(self.state.pc)
        {
            self.state.display_random =
                crate::random::RandomState::seeded(self.state.random.next_u64());
            self.state.display_random_pc = Some(self.state.pc);
        }
        match &stmt {
            Statement::Choice { options } if self.visible_choice_indices(options)?.is_empty() => {
                self.state.pc += 1;
                self.sync_narrative_position();
                Ok(None)
            }
            Statement::Imagemap { hotspots, .. } if hotspots.is_empty() => {
                self.state.pc += 1;
                self.sync_narrative_position();
                Ok(None)
            }
            _ if Self::is_interactive(&stmt) => {
                let pc=self.state.pc;
                self.record_interaction_snapshot(&stmt)?;
                if self.state.pc!=pc{return Ok(None);}
                self.current_interaction()
            }
            _ => {
                self.exec_silent(stmt)?;
                Ok(None)
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rendered_input_keeps_the_existing_visit_and_rejects_an_identical_other_engine() {
        let source = "label start\n\"Same\"\njump start\n";
        let mut first = Engine::new(
            rvn_parser::parse(source).unwrap(),
            crate::renderer::TerminalRenderer,
            8,
        )
        .unwrap();
        let mut other = Engine::new(
            rvn_parser::parse(source).unwrap(),
            crate::renderer::TerminalRenderer,
            8,
        )
        .unwrap();
        first.step_until_interaction().unwrap();
        other.step_until_interaction().unwrap();
        let captured = first.preview_input_source().unwrap();
        assert!(first.accepts_preview_input_source(&captured));
        assert!(!other.accepts_preview_input_source(&captured));
        first.advance_dialogue().unwrap();
        first.step_until_interaction().unwrap();
        let revisit = first.preview_input_source().unwrap();
        assert_eq!(captured.position, revisit.position);
        assert_eq!(captured.epoch, revisit.epoch);
        assert_ne!(captured.history_entry, revisit.history_entry);
        assert!(!first.accepts_preview_input_source(&captured));
    }
    #[test]
    fn one_statement_step_does_not_run_the_following_command_or_answer_a_choice() {
        let script = rvn_parser::parse(
            "set value = 1\nset value = 2\nchoice {\"Remain\" => {set value = 99}}\n",
        )
        .unwrap();
        let mut engine = Engine::new(script, crate::renderer::TerminalRenderer, 8).unwrap();
        assert!(engine.preview_step_statement().unwrap().is_none());
        assert_eq!(engine.state.vars["value"], Value::Int(1));
        assert!(engine.preview_step_statement().unwrap().is_none());
        assert_eq!(engine.state.vars["value"], Value::Int(2));
        assert!(matches!(
            engine.preview_step_statement().unwrap(),
            Some(Interaction::Choice { .. })
        ));
        assert_eq!(engine.state.vars["value"], Value::Int(2));
        let entries = engine.history.entries().len();
        let source = engine.preview_input_source().unwrap();
        let random = engine.state.random;
        assert!(matches!(
            engine.preview_step_statement().unwrap(),
            Some(Interaction::Choice { .. })
        ));
        assert_eq!(engine.history.entries().len(), entries);
        assert_eq!(engine.preview_input_source(), Some(source));
        assert_eq!(engine.state.random, random);
    }
}

#[cfg(test)]
mod choice_image_authority_contracts {
 use super::*;
 #[test]
 fn conditional_destinations_revoke_same_visit_image_without_evaluating_random_into_live_state(){
  let mut engine=Engine::new(rvn_parser::parse("set flag=true\nchoice {\"First\" if flag=>{set chosen=1} \"Second\"=>{set chosen=2}}\n").unwrap(),crate::renderer::TerminalRenderer,8).unwrap();
  engine.step_until_interaction().unwrap();let source=engine.preview_input_source().unwrap();assert_eq!(source.choice_indices,vec![0,1]);
  let before=serde_json::to_vec(&engine.state).unwrap();assert!(engine.accepts_preview_input_source(&source));assert_eq!(serde_json::to_vec(&engine.state).unwrap(),before);
  engine.state.vars.insert("flag".into(),Value::Bool(false));let changed=serde_json::to_vec(&engine.state).unwrap();
  let latest=engine.preview_input_source().unwrap();assert_eq!(source.position,latest.position);assert_eq!(source.history_entry,latest.history_entry);assert_eq!(latest.choice_indices,vec![1]);
  assert!(!engine.accepts_preview_input_source(&source));assert_eq!(serde_json::to_vec(&engine.state).unwrap(),changed);
 }
}
