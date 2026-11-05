//! Audio rollback prepared before the real host broker/Source fence.
use super::*;
use crate::{audio_checkpoint::{AudioRollbackSource,AudioCheckpointSnapshot},save::SaveError};

/// A retained real history visit, bound to the entire unchanged live Engine.
/// Not Clone/Serialize and never constructible from a state or client DTO.
pub struct PreparedAudioRollback{source:DebugStateSource,visit:AudioRollbackSource,batch:crate::renderer::SceneRenderBatch,legacy_preview:bool,reveal_source:Option<crate::display_reveal::DisplayRevealSource>,choice_backdrop:Option<Interaction>}
impl PreparedAudioRollback{
    /// Data-only projection for actual Source/World preflight, not a commit permit.
    pub fn render_batch(&self)->&crate::renderer::SceneRenderBatch{&self.batch}
    pub fn display_reveal_source(&self)->Option<&crate::display_reveal::DisplayRevealSource>{self.reveal_source.as_ref()}
    pub fn choice_backdrop(&self)->Option<&Interaction>{self.choice_backdrop.as_ref()}
}
/// Exclusive borrow after pure preflight. Commit cannot reenter a renderer's
/// validation or broker while the host holds broker -> gate -> cursor locks.
pub struct AudioRollbackCommitGuard<'a,R:Renderer>{engine:&'a mut Engine<R>,prepared:PreparedAudioRollback}
impl<R:Renderer> AudioRollbackCommitGuard<'_,R>{
    pub fn render_batch(&self)->&crate::renderer::SceneRenderBatch{&self.prepared.batch}
    pub fn commit_with_audio_checkpoint(mut self,snapshot:AudioCheckpointSnapshot)->Result<(),SaveError>{
        let next=crate::audio_checkpoint::checkpointed_state(&self.prepared.batch.state,self.engine.instance_id(),self.prepared.batch.runtime_epoch,&snapshot).map_err(SaveError::Incompatible)?;
        // A restored, not-yet-submitted Source must start at the exact saved
        // cursor. A well-shaped later/earlier cursor is not the retained visit.
        for expected in crate::audio_playback::describe(&self.prepared.batch.state,self.prepared.batch.runtime_epoch).map_err(SaveError::Incompatible)?.into_iter().filter(|v|!matches!(v.token.task.key.owner,crate::scene_tasks::TaskOwner::Host{..})){
            let row=snapshot.rows.iter().find(|row|row.token==expected.token).ok_or_else(||SaveError::Incompatible("Rollback Source disappeared at commit".into()))?;
            if expected.playback.checkpoint.as_ref()!=Some(&row.checkpoint){return Err(SaveError::Incompatible("Rollback Source cursor differs from the exact retained visit".into()));}
        }
        for expected in crate::audio_checkpoint::music_views(&self.prepared.batch.state,self.prepared.batch.runtime_epoch).map_err(SaveError::Incompatible)?.into_iter().filter(|v|!matches!(v.owner,crate::scene_tasks::TaskOwner::Host{..})){
            let row=snapshot.music.iter().find(|row|row.target==expected.authority.target&&row.file==expected.file).ok_or_else(||SaveError::Incompatible("Rollback Music Source disappeared at commit".into()))?;
            if expected.checkpoint.as_ref()!=Some(&row.checkpoint){return Err(SaveError::Incompatible("Rollback Music cursor differs from the exact retained visit".into()));}
        }
        self.prepared.batch.state=next;
        if self.prepared.visit.skip_current_display{self.engine.history.pop();}
        self.engine.history.pop();
        self.engine.publish_scene_restore(self.prepared.batch);
        if self.prepared.legacy_preview{self.engine.restore_legacy_preview_visit(self.engine.display_generation);}
        Ok(())
    }
}
impl<R:Renderer> Engine<R>{
    pub fn prepare_audio_rollback(&self,visit:&AudioRollbackSource)->Result<PreparedAudioRollback,SaveError>{
        let next=self.audio_rollback_state_for_warming(visit)?;
        if self.narrative.is_none()&&next.game_audio.is_none()&&next.game_tasks.is_none(){return Err(SaveError::Incompatible("Prepared audio rollback requires an actual owned Game/Scene registry".into()));}
        let source=self.debug_state_source().map_err(|e|SaveError::Incompatible(e.to_string()))?;
        // Descriptions and bindings may populate a cache. Keep all such work
        // on the real Engine's private planning copy, not its live source.
        let mut candidate=self.debugger_candidate(crate::eval::debugger_eval::Probe::shared());candidate.functions.debug_probe=None;
        let batch=if self.narrative.is_some(){candidate.scene_restore_batch(next)?}else{candidate.standalone_audio_restore_batch(next).map_err(|e|SaveError::Incompatible(e.to_string()))?};
        candidate.state=batch.state.clone();candidate.runtime_epoch=batch.runtime_epoch;candidate.interface_epoch=batch.interface_epoch;candidate.video_epoch=batch.video_epoch;
        candidate.display_generation=self.display_generation.checked_add(1).ok_or_else(||SaveError::Incompatible("Display generation exhausted".into()))?;
        if visit.skip_current_display{candidate.history.pop();}candidate.history.pop();
        if candidate.debugger_anchor_measured().map_err(|e|SaveError::Incompatible(e.to_string()))?.1>16*1024*1024-4096{return Err(SaveError::Incompatible("Prepared audio rollback lacks bounded anchor headroom".into()));}
        candidate.display_owner=batch.state.scene_execution.as_ref().and_then(|execution|execution.owner());candidate.display_history_entry=None;candidate.legacy_preview_visit=None;
        let reveal_source=candidate.display_reveal_source();
        let choice_backdrop=if matches!(batch.display,Some(Interaction::Choice{..})){candidate.last_dialogue_interaction().map_err(|e|SaveError::Incompatible(e.to_string()))?}else{None};
        Ok(PreparedAudioRollback{source,visit:visit.clone(),batch,legacy_preview:self.narrative.is_none(),reveal_source,choice_backdrop})
    }
    pub fn accepts_prepared_audio_rollback(&self,prepared:&PreparedAudioRollback)->bool{self.accepts_debug_state_source(&prepared.source)&&self.accepts_audio_rollback_source(&prepared.visit)}
    /// All fallible renderer preflights occur before the host takes its fence.
    pub fn begin_prepared_audio_rollback_commit(&mut self,prepared:PreparedAudioRollback)->Result<AudioRollbackCommitGuard<'_,R>,SaveError>{
        if !self.accepts_prepared_audio_rollback(&prepared){return Err(SaveError::Incompatible("Live source/history changed after audio rollback preparation".into()));}
        self.renderer.validate_audio_restore(&prepared.batch.state).map_err(SaveError::Incompatible)?;
        self.renderer.validate_scene_batch(&prepared.batch).map_err(SaveError::Incompatible)?;
        // Renderer validation is read-only by contract; still revoke a plan if
        // an observing host changed an authenticated source during preflight.
        if !self.accepts_prepared_audio_rollback(&prepared){return Err(SaveError::Incompatible("Audio rollback source changed during final preflight".into()));}
        Ok(AudioRollbackCommitGuard{engine:self,prepared})
    }
}
