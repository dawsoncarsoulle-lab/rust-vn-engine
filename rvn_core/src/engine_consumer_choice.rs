use super::*;
use crate::consumer_schedule::{BoundConsumerChoice,BoundConsumerFrame,ConsumerOwner};
impl<R:Renderer> Engine<R>{
    pub(super) fn consumer_choice_options(statement:&Statement)->Option<&[rvn_parser::ChoiceOption]>{match statement{Statement::ScheduledConsumer{version:2,body,..}=>match body.as_slice(){[Statement::Choice{options}]=>Some(options),_=>None},_=>None}}
    fn actual_consumer_owner(&self)->ConsumerOwner{self.state.scene_execution.as_ref().and_then(|e|e.owner()).map(|activation|ConsumerOwner::Scene{activation}).unwrap_or(ConsumerOwner::Game)}
    pub(super) fn cached_consumer_choice(&self)->Option<&BoundConsumerChoice>{
        if self.state.display_random_pc!=Some(self.state.pc){return None;}
        let choice=self.state.consumer_choice.as_ref().or_else(||self.state.scene_execution.as_ref()?.display.as_ref()?.consumer_choice.as_ref())?;
        (choice.pc==self.state.pc&&choice.owner==self.actual_consumer_owner()).then_some(choice)
    }
    pub(super) fn retained_choice_labels(&self,choice:&BoundConsumerChoice)->Result<Vec<String>,RuntimeError>{
        self.consumer_program.validate_emitted(&self.script).map_err(|e|self.eval_err(e,"Frozen Choice programme"))?;
        self.consumer_program.validate_choice(choice).map_err(|e|self.eval_err(e,"Retained Choice"))?;
        let options=self.script.get(choice.pc).and_then(Self::consumer_choice_options).ok_or_else(||self.scene_resource_error("Retained Choice has no actual authored consumer".into()))?;
        let mut labels=Vec::new();for(row,option)in choice.rows.iter().zip(options){if row.visible{let key=text_to_locale_key(&option.label);let translated=self.translate(&key);labels.push(self.functions.render_retained_choice_label(&option.label,&row.values,self.locale.as_ref(),(translated!=key).then_some(translated)).map_err(|e|self.eval_err(e,"Frozen Choice label ports"))?);}}Ok(labels)
    }
    pub(super) fn record_consumer_choice(&mut self,statement:&Statement)->Result<(),RuntimeError>{self.record_consumer_selection(statement)}
    pub(super) fn record_consumer_choice_inner(&mut self,statement:&Statement)->Result<(),RuntimeError>{
        if let Some(choice)=self.cached_consumer_choice(){self.consumer_program.validate_choice(choice).map_err(|e|self.eval_err(e,"Frozen Choice visit"))?;if self.history.latest_entry_id().is_none(){let labels=self.retained_choice_labels(choice)?;self.history.push_audio_entry(self.state.clone(),Some(HistoryDisplay::Choice{options:labels}));self.display_history_entry=if self.narrative.is_some()||self.state.game_audio.is_some(){self.history.latest_entry_id()}else{None};if self.state.game_audio.is_some(){self.history.arm_audio_publication(&self.state);}self.legacy_preview_visit=if self.narrative.is_none(){Some(LegacyPreviewVisit::capture(&self.state,self.history.latest_entry_id()))}else{None};}return Ok(());}
        self.consumer_program.validate_plan_binding(statement,self.state.pc,false).map_err(|e|self.eval_err(e,"Scheduled Choice binding"))?;
        let generation=self.display_generation.checked_add(1).ok_or_else(||self.eval_err(EvalError::NumericOverflow,"Choice display generation"))?;
        let Statement::ScheduledConsumer{scope,..}=statement else{unreachable!()};let owner=self.actual_consumer_owner();let identity=self.consumer_program.identity().map_err(|e|self.eval_err(e,"Choice programme"))?;
        let scope=self.consumer_program.bound_scope(scope,self.state.pc).map_err(|e|self.eval_err(e,"Choice source scope"))?;
        let environment=self.vars_for_eval();let globals=self.lexical_vars_for_eval();let mut next=self.state.clone();
        let slot=match next.scene_execution.as_mut().and_then(|e|e.active.as_mut()){Some(a)=>&mut a.consumer_frame,None=>&mut next.consumer_frame};let frame=slot.get_or_insert_with(||BoundConsumerFrame::new(identity,owner));frame.validate_binding(identity,owner).map_err(|e|self.eval_err(e,"Choice activation"))?;
        let value=self.functions.eval_engine_consumer_with_widget_objects(self.widget_object_context(&self.state)?.as_ref(), statement,&environment,&globals,&mut next.random,&mut frame.frame,&scope,self.locale.as_ref()).map_err(|e|self.eval_err(e,"Choice direct inputs"))?;
        let choice=BoundConsumerChoice::from_value(identity,owner,self.state.pc,value).map_err(|e|self.eval_err(e,"Choice row tuple"))?;let labels=self.retained_choice_labels(&choice)?;
        if labels.is_empty(){next.pc+=1;next.consumer_choice=None;next.display_random_pc=None;return self.commit_scheduled_state(next);}
        next.consumer_imagemap=None;
        let display=HistoryDisplay::Choice{options:labels.clone()};next.current_interactive_pc=next.pc;next.display_random_pc=Some(next.pc);next.display_random=next.random;
        if let Some(program)=&self.narrative{let indices=choice.indices();let checkpoint=crate::scene::DisplayCheckpoint{consumer_dialogue:None,
consumer_choice:Some(choice),consumer_imagemap:None,position:program.position(next.pc),variables:environment,lexical_globals:globals,random:next.display_random,typewriter:next.typewriter.clone(),display:display.clone(),choice_indices:indices,reveal:None};next.scene_execution.as_mut().unwrap().display=Some(checkpoint);}
        else{next.consumer_choice=Some(choice);}
        if self.narrative.is_some(){self.commit_scene_state(next)?;}
        else{let cache=next.game_audio.is_some().then(||self.audio_interface_cache_transaction()).transpose()?;let mut batch=self.build_scene_batch(next.clone(),self.runtime_epoch,self.interface_epoch,self.video_epoch)?;batch.display=Some(Interaction::Choice{options:labels});self.renderer.validate_scene_batch(&batch).map_err(|e|self.scene_resource_error(e))?;cache.map(|transaction|transaction.commit()).unwrap_or(());self.state=next;self.renderer.publish_scene_batch(batch);}
        self.display_generation=generation;self.display_owner=self.state.scene_execution.as_ref().and_then(|e|e.owner());self.history.push_audio_entry(self.state.clone(),Some(display));self.display_history_entry=if self.narrative.is_some()||self.state.game_audio.is_some(){self.history.latest_entry_id()}else{None};if self.state.game_audio.is_some(){self.history.arm_audio_publication(&self.state);}self.legacy_preview_visit=if self.narrative.is_none(){Some(LegacyPreviewVisit::capture(&self.state,self.history.latest_entry_id()))}else{None};Ok(())
    }
    pub(super) fn submit_consumer_choice(&mut self,selected:usize)->Result<(),RuntimeError>{
        if self.state.motions.waiting.is_some()||self.state.videos.waiting.is_some(){return Ok(());}
        let choice=self.cached_consumer_choice().ok_or_else(||self.scene_resource_error("Choice input requires its actual reached consumer visit".into()))?;self.consumer_program.validate_choice(choice).map_err(|e|self.eval_err(e,"Choice input binding"))?;let indices=choice.indices();
        if indices.is_empty(){return Err(self.scene_resource_error("Reached Choice has no visible input".into()));}
        let options=self.script.get(self.state.pc).and_then(Self::consumer_choice_options).unwrap();let body=options[indices[selected.min(indices.len()-1)]].body.clone();
        self.submit_lowered_consumer_branch(&body)
    }
}
