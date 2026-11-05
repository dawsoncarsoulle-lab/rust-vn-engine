//! Reached selection consumers prepare without inventing an input response.
//! Choice and Imagemap publish through the same existing candidate boundary.
use super::*;
use crate::consumer_schedule::{BoundConsumerFrame,BoundConsumerImagemap,ConsumerOwner};
use std::{rc::Rc,cell::RefCell};
enum InputOperation<'a>{Record(&'a Statement),Select(&'a [Statement])}
impl<R:Renderer> Engine<R>{
    pub(super) fn consumer_imagemap_parts(statement:&Statement)->Option<(&str,Option<&str>,&[Hotspot])>{
        match statement {Statement::ScheduledConsumer{version:2,body,..}=>match body.as_slice(){
            [Statement::Imagemap{background,hover_image,hotspots}]=>Some((background,hover_image.as_deref(),hotspots)),_=>None},_=>None}
    }
    fn imagemap_owner(&self)->Result<ConsumerOwner,RuntimeError>{
        if self.narrative.is_some(){
            return self.state.scene_execution.as_ref().and_then(|execution|execution.owner())
                .map(|activation|ConsumerOwner::Scene{activation})
                .ok_or_else(||self.scene_resource_error("Reached input has no actual active Scene owner".into()));
        }
        if self.state.scene_execution.is_some(){return Err(self.scene_resource_error("Standalone Game input cannot borrow a Scene declaration".into()));}
        Ok(ConsumerOwner::Game)
    }
    pub(super) fn cached_consumer_imagemap(&self)->Option<&BoundConsumerImagemap>{
        if self.state.display_random_pc!=Some(self.state.pc){return None;}
        let data=self.state.consumer_imagemap.as_ref().or_else(||self.state.scene_execution.as_ref()?.display.as_ref()?.consumer_imagemap.as_ref())?;
        (data.pc==self.state.pc&&data.owner==self.imagemap_owner().ok()?).then_some(data)
    }
    pub(super) fn retained_imagemap(&self,data:&BoundConsumerImagemap)->Result<Interaction,RuntimeError>{
        self.consumer_program.validate_emitted(&self.script).map_err(|e|self.eval_err(e,"Frozen Imagemap program"))?;
        self.consumer_program.validate_imagemap(data).map_err(|e|self.eval_err(e,"Retained Imagemap visit"))?;
        let(background,hover,hotspots)=self.script.get(data.pc).and_then(Self::consumer_imagemap_parts)
            .ok_or_else(||self.scene_resource_error("Imagemap has no actual frozen input site".into()))?;
        Ok(Interaction::Imagemap{background:background.into(),hover_image:hover.map(str::to_owned),hotspots:hotspots.to_vec()})
    }
    /// Shared Choice/Imagemap transaction. Planning records model publications;
    /// no show_choice/show_imagemap, default selection, or callback is called.
    pub(super) fn record_consumer_selection(&mut self,statement:&Statement)->Result<(),RuntimeError>{
        self.commit_consumer_input_candidate(InputOperation::Record(statement))
    }
    fn commit_consumer_input_candidate(&mut self,operation:InputOperation<'_>)->Result<(),RuntimeError>{
        self.consumer_program.validate_emitted(&self.script).map_err(|e|self.eval_err(e,"Frozen selection program"))?;
        self.consumer_program.validate_state_binding(&self.state).map_err(|e|self.eval_err(e,"Actual selection owner"))?;
        let source=self.debug_state_source()?;
        let publication=Rc::new(RefCell::new(None));
        let probe=self.functions.debug_probe.clone().unwrap_or_else(crate::eval::debugger_eval::Probe::shared);
        let mut candidate=self.debugger_candidate(probe);
        candidate.functions.debug_probe=self.functions.debug_probe.clone();
        candidate.renderer.1=Some(publication.clone());
        match operation{
            InputOperation::Record(statement)if Self::consumer_choice_options(statement).is_some()=>candidate.record_consumer_choice_inner(statement)?,
            InputOperation::Record(statement)if Self::consumer_imagemap_parts(statement).is_some()=>candidate.record_consumer_imagemap_inner(statement)?,
            InputOperation::Record(_)=>return Err(self.scene_resource_error("Expected actual selection consumer".into())),
            InputOperation::Select(body)=>candidate.submit_lowered_consumer_branch_inner(body)?,
        }
        candidate.consumer_program.validate_state_binding(&candidate.state).map_err(|e|self.eval_err(e,"Prepared selection ownership"))?;
        // Refuse a candidate whose complete bounded source cannot be represented.
        candidate.debug_state_source()?;
        let state=candidate.state.clone();let history=candidate.history.debugger_copy();
        let fields=(candidate.display_generation,candidate.display_owner,candidate.display_history_entry,candidate.legacy_preview_visit.clone(),candidate.runtime_epoch,candidate.interface_epoch,candidate.video_epoch);
        let cache=candidate.interface_view_cache.lock().map_err(|_|self.scene_resource_error("Selection description cache unavailable".into()))?.iter().map(|entry|InterfaceViewSnapshot{locale:entry.locale.as_ref().map(LocaleManager::debugger_copy),text_resources:entry.text_resources.clone(),ui:entry.ui.clone(),globals:entry.globals.clone(),views:entry.views.clone()}).collect();
        drop(candidate);
        if !self.accepts_debug_state_source(&source){return Err(self.scene_resource_error("Selection source changed before publication".into()));}
        // All fallible operations precede adoption, including history/cache.
        self.state=state;self.history=history;
        (self.display_generation,self.display_owner,self.display_history_entry,self.legacy_preview_visit,self.runtime_epoch,self.interface_epoch,self.video_epoch)=fields;
        *self.interface_view_cache.lock().unwrap_or_else(|poison|poison.into_inner())=cache;
        if let Some(batch)=publication.borrow_mut().take(){self.renderer.publish_scene_batch(batch);}
        Ok(())
    }
    pub(super) fn record_consumer_imagemap_inner(&mut self,statement:&Statement)->Result<(),RuntimeError>{
        if let Some(data)=self.cached_consumer_imagemap(){
            let interaction=self.retained_imagemap(data)?;
            if self.history.latest_entry_id().is_none(){
                let Interaction::Imagemap{background,hover_image,hotspots}=interaction else{unreachable!()};
                self.history.push_audio_entry(self.state.clone(),Some(HistoryDisplay::Imagemap{background,hover_image,hotspots}));
                self.display_history_entry=if self.narrative.is_some()||self.state.game_audio.is_some(){self.history.latest_entry_id()}else{None};
                if self.state.game_audio.is_some(){self.history.arm_audio_publication(&self.state);}
                self.legacy_preview_visit=if self.narrative.is_none(){Some(LegacyPreviewVisit::capture(&self.state,self.history.latest_entry_id()))}else{None};
            }
            return Ok(());
        }
        self.consumer_program.validate_plan_binding(statement,self.state.pc,false).map_err(|e|self.eval_err(e,"Scheduled Imagemap binding"))?;
        let generation=self.display_generation.checked_add(1).ok_or_else(||self.eval_err(EvalError::NumericOverflow,"Imagemap display generation"))?;
        let Statement::ScheduledConsumer{scope,..}=statement else{unreachable!()};
        let owner=self.imagemap_owner()?;let identity=self.consumer_program.identity().map_err(|e|self.eval_err(e,"Imagemap program"))?;
        let scope=self.consumer_program.bound_scope(scope,self.state.pc).map_err(|e|self.eval_err(e,"Imagemap source scope"))?;
        let environment=self.vars_for_eval();let globals=self.lexical_vars_for_eval();let mut next=self.state.clone();
        let slot=match next.scene_execution.as_mut().and_then(|e|e.active.as_mut()){Some(active)=>&mut active.consumer_frame,None=>&mut next.consumer_frame};
        let frame=slot.get_or_insert_with(||BoundConsumerFrame::new(identity,owner));frame.validate_binding(identity,owner).map_err(|e|self.eval_err(e,"Imagemap activation"))?;
        let values=self.functions.eval_engine_consumer_with_widget_objects(self.widget_object_context(&self.state)?.as_ref(), statement,&environment,&globals,&mut next.random,&mut frame.frame,&scope,self.locale.as_ref()).map_err(|e|self.eval_err(e,"Imagemap header gather"))?;
        if !matches!(values,Value::List(ref fields)if fields.is_empty()){return Err(self.scene_resource_error("Imagemap static header has unexpected computed fields".into()));}
        let data=BoundConsumerImagemap{program:identity,owner,pc:self.state.pc};
        let interaction=self.retained_imagemap(&data)?;
        let Interaction::Imagemap{background,hover_image,hotspots}=interaction.clone()else{unreachable!()};
        if hotspots.is_empty(){next.pc+=1;next.consumer_imagemap=None;next.display_random_pc=None;return self.commit_scheduled_state(next);}
        let indices=(0..hotspots.len()).collect();
        let display=HistoryDisplay::Imagemap{background,hover_image,hotspots};
        next.current_interactive_pc=next.pc;next.display_random_pc=Some(next.pc);next.display_random=next.random;
        next.consumer_choice=None;next.consumer_dialogue=None;
        if let Some(program)=&self.narrative{
            next.consumer_imagemap=None;
            next.scene_execution.as_mut().unwrap().display=Some(crate::scene::DisplayCheckpoint{
                consumer_dialogue:None,consumer_choice:None,consumer_imagemap:Some(data),
                position:program.position(next.pc),variables:environment,lexical_globals:globals,random:next.display_random,
                typewriter:next.typewriter.clone(),display:display.clone(),choice_indices:indices,reveal:None});
            self.commit_scene_state(next)?;
        }else{
            next.consumer_imagemap=Some(data);
            let mut batch=self.build_scene_batch(next.clone(),self.runtime_epoch,self.interface_epoch,self.video_epoch)?;batch.display=Some(interaction);
            self.renderer.validate_scene_batch(&batch).map_err(|e|self.scene_resource_error(e))?;
            self.state=next;self.renderer.publish_scene_batch(batch);
        }
        self.display_generation=generation;self.display_owner=self.state.scene_execution.as_ref().and_then(|e|e.owner());
        self.history.push_audio_entry(self.state.clone(),Some(display));
        self.display_history_entry=if self.narrative.is_some()||self.state.game_audio.is_some(){self.history.latest_entry_id()}else{None};
        if self.state.game_audio.is_some(){self.history.arm_audio_publication(&self.state);}
        self.legacy_preview_visit=if self.narrative.is_none(){Some(LegacyPreviewVisit::capture(&self.state,self.history.latest_entry_id()))}else{None};
        Ok(())
    }
    pub(super) fn submit_consumer_imagemap(&mut self,selected:usize)->Result<(),RuntimeError>{
        if self.state.videos.waiting.is_some()||self.state.motions.waiting.is_some(){return Ok(());}
        let data=self.cached_consumer_imagemap().ok_or_else(||self.scene_resource_error("Imagemap input requires its actual reached visit".into()))?;
        self.consumer_program.validate_imagemap(data).map_err(|e|self.eval_err(e,"Imagemap input binding"))?;
        let(_,_,hotspots)=self.script.get(data.pc).and_then(Self::consumer_imagemap_parts).unwrap();
        let body=hotspots.get(selected).ok_or_else(||self.scene_resource_error("Imagemap input is outside the actual reached hotspot set".into()))?.body.clone();
        self.submit_lowered_consumer_branch(&body)
    }
    /// The only selected child is an actual prepared Call/empty continuation.
    /// Neither this helper nor preparation evaluates any unselected branch.
    pub(super) fn submit_lowered_consumer_branch(&mut self,body:&[Statement])->Result<(),RuntimeError>{
        self.commit_consumer_input_candidate(InputOperation::Select(body))
    }
    fn submit_lowered_consumer_branch_inner(&mut self,body:&[Statement])->Result<(),RuntimeError>{
        let mut next=self.state.clone();next.display_random_pc=None;next.consumer_imagemap=None;
        if body.is_empty(){next.pc+=1;return self.commit_scheduled_state(next);}
        let[Statement::Call{target}]=body else{return Err(self.scene_resource_error("Selected input branch was not lowered from its actual authored route".into()));};
        let destination=self.resolve(target)?;
        if self.narrative.is_some(){return self.transfer_narrative_from(next,destination,Some(self.state.pc+1),true);}
        if next.call_stack.len()>=128{return Err(self.eval_err(EvalError::ExecutionLimit{limit:"128 appels narratifs imbriqués"},"Imagemap selected branch"));}
        next.call_stack.push(self.state.pc+1);next.pc=destination;self.commit_scheduled_state(next)
    }
}
