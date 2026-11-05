//! One immutable Engine instant, captured before any writer can publish bytes.
use super::*;
impl<R:Renderer> Engine<R> {
    pub(super) fn arm_interactive_audio_publication(&mut self)->Result<(),RuntimeError> {
        if self.display_history_entry.is_some() && self.display_history_entry==self.history.latest_entry_id() {
            self.history.arm_audio_publication(&self.state);
        }
        if (self.state.scene_execution.is_some()||self.state.game_audio.is_some()) && (!crate::audio_playback::describe(&self.state,self.runtime_epoch()).map_err(|message|self.scene_resource_error(message))?.is_empty()
            || !crate::audio_checkpoint::music_views(&self.state,self.runtime_epoch()).map_err(|message|self.scene_resource_error(message))?.is_empty()) {
            let batch=self.build_scene_batch(self.state.clone(),self.runtime_epoch(),self.interface_epoch(),self.video_epoch)?;
            self.renderer.validate_scene_batch(&batch).map_err(|message|self.scene_resource_error(message))?;
            self.renderer.publish_scene_batch(batch);
        }
        Ok(())
    }
    /// The publication producer supplies a snapshot while all real readers are
    /// fenced. It may fill the awaiting last entry once, never a later snapshot.
    pub fn audio_publication_source(&self)->Result<crate::audio_checkpoint::AudioPublicationSource,RuntimeError> {
        let entry=self.history.pending_audio_entry().filter(|id|Some(*id)==self.display_history_entry);
        let logical=if entry.is_some(){self.history.pending_audio_anchor().unwrap_or(&self.state)}else{&self.state};
        Ok(crate::audio_checkpoint::AudioPublicationSource{engine_instance:self.instance_id(),runtime_epoch:self.runtime_epoch(),history_entry:entry,logical_sha256:crate::audio_checkpoint::logical_digest(logical).map_err(|message|self.scene_resource_error(message))?})
    }
    pub fn record_audio_publication(&mut self,source:&crate::audio_checkpoint::AudioPublicationSource,snapshot:crate::audio_checkpoint::AudioCheckpointSnapshot)->Result<bool,RuntimeError> {
        if *source!=self.audio_publication_source()? || source.logical_sha256!=crate::audio_checkpoint::logical_digest(&self.state).map_err(|message|self.scene_resource_error(message))? {return Err(self.scene_resource_error("Audio publication source is stale before cursor admission".into()));}
        let next=crate::audio_checkpoint::checkpointed_state(&self.state,self.instance_id(),self.runtime_epoch(),&snapshot)
            .map_err(|message|self.scene_resource_error(message))?;
        let recorded=if source.history_entry.is_some(){self.history.record_audio_publication(source.history_entry,&self.state,&next).map_err(|message|self.scene_resource_error(message))?}else{false};
        self.state=next;Ok(recorded)
    }
    pub fn snapshot_for_save(&self)->Result<crate::audio_checkpoint::PortableGameSnapshot,crate::save::SaveError> {
        self.functions.reject_unit_portable_owner(&self.script).map_err(|error|crate::save::SaveError::Incompatible(error.to_string()))?;
        self.validate_standalone_timer_target(&self.state).map_err(|e|crate::save::SaveError::Incompatible(e.to_string()))?;
        let instance=self.instance_id();let epoch=self.runtime_epoch();
        let captured=self.renderer.capture_audio_checkpoint(&self.state,instance,epoch).map_err(crate::save::SaveError::Incompatible)?;
        let state=crate::audio_checkpoint::checkpointed_state(&self.state,instance,epoch,&captured).map_err(crate::save::SaveError::Incompatible)?;
        crate::audio_checkpoint::PortableGameSnapshot::captured(state).map_err(crate::save::SaveError::Incompatible)
    }
    pub fn save_autosave(&self,manager:&SaveManager,label:String,script_name:String)->Result<(),crate::save::SaveError> {
        let snapshot=self.snapshot_for_save()?;manager.save_autosave_snapshot(&snapshot,label,script_name)
    }
    pub fn save_quicksave(&self,manager:&SaveManager,label:String,script_name:String)->Result<(),crate::save::SaveError> {
        let snapshot=self.snapshot_for_save()?;manager.save_quicksave_snapshot(&snapshot,label,script_name)
    }
    pub fn audio_rollback_preview(&self)->Result<Option<(crate::audio_checkpoint::AudioRollbackSource,crate::save::SaveData)>,crate::save::SaveError> {
        self.audio_rollback_preview_offset(false)
    }
    /// Match the existing Bevy game facade's skip of a displayed current anchor.
    /// Both the skipped visit and selected earlier visit remain intact until all
    /// cold source checks and the real scene restore preflight have succeeded.
    pub fn preview_game_audio_rollback(&self)->Result<Option<(crate::audio_checkpoint::AudioRollbackSource,crate::save::SaveData)>,crate::save::SaveError> {
        self.audio_rollback_preview_offset(true)
    }
    /// Exact non-serialized visit receipt. Native frames stay outside Save1-14.
    pub fn preview_game_audio_rollback_source(&self)->Result<Option<crate::audio_checkpoint::AudioRollbackSource>,crate::save::SaveError>{self.audio_rollback_source_offset(true)}
    /// Data-only description of the actual retained visit. This copy can warm
    /// assets but grants no restore authority. Commit must revalidate `source`.
    pub fn audio_rollback_state_for_warming(&self,source:&crate::audio_checkpoint::AudioRollbackSource)->Result<GameState,crate::save::SaveError>{
        use crate::save::SaveError;
        if !self.accepts_audio_rollback_source(source){return Err(SaveError::Incompatible("Rollback warming source is stale before history description".into()));}
        let entries=self.history.entries();
        let index=entries.len().checked_sub(1+usize::from(source.skip_current_display)).ok_or_else(||SaveError::Incompatible("Rollback warming visit is absent".into()))?;
        let entry=entries.get(index).filter(|entry|entry.entry_id==Some(source.entry_id)).ok_or_else(||SaveError::Incompatible("Rollback warming visit identity differs".into()))?;
        Ok(entry.state.clone())
    }
    fn audio_rollback_source_offset(&self,game_facade:bool)->Result<Option<crate::audio_checkpoint::AudioRollbackSource>,crate::save::SaveError>{
        use crate::save::SaveError;
        let entries=self.history.entries();
        let skip_current_display=game_facade && entries.last().is_some_and(|entry|entry.display.is_some() && entry.state.pc==self.state.pc);
        let Some(index)=entries.len().checked_sub(1+usize::from(skip_current_display)) else{return Ok(None);};
        let entry=&entries[index];
        let entry_id=entry.entry_id.ok_or_else(||SaveError::Incompatible("Rollback visit identity was exhausted; no older visit may be adopted".into()))?;
        crate::audio_checkpoint::require_saved_checkpoints(&entry.state).map_err(SaveError::Incompatible)?;
        crate::display_reveal::validate_state(&entry.state).map_err(SaveError::Incompatible)?;
        let source=crate::audio_checkpoint::AudioRollbackSource{engine_instance:self.instance_id(),runtime_epoch:self.runtime_epoch(),entry_id,
            skip_current_display,latest_entry_id:self.history.latest_entry_id(),
            current_sha256:crate::audio_checkpoint::logical_digest(&self.state).map_err(SaveError::Incompatible)?,
            historical_sha256:crate::audio_checkpoint::logical_digest(&entry.state).map_err(SaveError::Incompatible)?};
        self.consumer_program.validate_emitted(&self.script).and_then(|_|self.consumer_program.validate_state_binding(&entry.state)).map_err(|e|SaveError::Incompatible(e.to_string()))?;
        Ok(Some(source))
    }
    fn audio_rollback_preview_offset(&self,game_facade:bool)->Result<Option<(crate::audio_checkpoint::AudioRollbackSource,crate::save::SaveData)>,crate::save::SaveError>{
        let Some(source)=self.audio_rollback_source_offset(game_facade)?else{return Ok(None);};
        let index=self.history.entries().len()-1-usize::from(source.skip_current_display);let entry=&self.history.entries()[index];
        let preview=crate::save::SaveData::from_state(&entry.state,0,"Rollback preparation".into(),String::new());preview.validate_checkpoint_shape()?;
        Ok(Some((source,preview)))
    }
    pub fn accepts_audio_rollback_source(&self,source:&crate::audio_checkpoint::AudioRollbackSource)->bool {
        self.audio_rollback_source_offset(source.skip_current_display).ok().flatten().is_some_and(|current|current==*source)
    }
    pub fn rollback_from_audio_source(&mut self,source:&crate::audio_checkpoint::AudioRollbackSource)->Result<bool,crate::save::SaveError> {
        if !self.accepts_audio_rollback_source(source){return Err(crate::save::SaveError::Incompatible("Rollback request belongs to a stale exact visit or logical instant".into()));}
        if self.state.game_audio.is_some()||self.state.game_tasks.is_some(){
            let entries=self.history.entries();let index=entries.len()-1-usize::from(source.skip_current_display);
            let batch=self.standalone_audio_restore_batch(entries[index].state.clone()).map_err(|e|crate::save::SaveError::Incompatible(e.to_string()))?;
            self.commit_widget_history_replacement(batch,1+usize::from(source.skip_current_display),true)?;return Ok(true);
        }
        if self.narrative.is_some() {
            let entries=self.history.entries();let index=entries.len()-1-usize::from(source.skip_current_display);
            let batch=self.scene_restore_batch(entries[index].state.clone())?;
            self.commit_widget_history_replacement(batch,1+usize::from(source.skip_current_display),false)?;return Ok(true);
        }
        if source.skip_current_display {return Err(crate::save::SaveError::Incompatible("The PCM game facade does not replace the legacy rollback route".into()));}
        Ok(self.rollback())
    }
    /// Request identity permits only observed Reveal/PCM fields to be filled at
    /// the same producer instant. PC/vars/RNG/targets/clocks/revisions remain exact.
    pub fn audio_request_source(&self)->Result<crate::audio_checkpoint::AudioRequestSource,crate::save::SaveError> {
        Ok(crate::audio_checkpoint::AudioRequestSource{engine_instance:self.instance_id(),runtime_epoch:self.runtime_epoch(),display_generation:self.display_generation,
            latest_entry_id:self.history.latest_entry_id(),logical_sha256:crate::audio_checkpoint::request_digest(&self.state).map_err(crate::save::SaveError::Incompatible)?})
    }
    pub fn accepts_audio_request_source(&self,source:&crate::audio_checkpoint::AudioRequestSource)->bool {
        self.audio_request_source().is_ok_and(|current|current==*source)
    }
}


impl<R:Renderer> Engine<R> {
    pub(super) fn capture_historical_audio(&self)->Result<crate::audio_checkpoint::HistoricalGameSnapshot,String> {
        let snapshot=self.renderer.capture_audio_checkpoint(&self.state,self.instance_id(),self.runtime_epoch())?;
        crate::audio_checkpoint::HistoricalGameSnapshot::captured(&self.state,snapshot,self.history.latest_entry_id())
    }
    fn callback_historical_audio(&self)->Result<Option<crate::audio_checkpoint::HistoricalGameSnapshot>,RuntimeError> {
        let planning=self.host_random_planning.lock().map_err(|_|self.scene_resource_error("Historical PCM producer context is unavailable".into()))?;
        // An absent actual reader is explicit non-measurement. No saved Some,
        // post-callback source, or alias can turn it into a historical witness.
        Ok(planning.historical_audio.as_ref().and_then(|capture|capture.as_ref().ok()).cloned())
    }
    pub(super) fn historical_ui_previous(&self,raw:GameState)->Result<GameState,RuntimeError> {
        match self.callback_historical_audio()? {
            Some(proof)=>proof.previous(&raw,self.instance_id(),self.runtime_epoch(),self.history.latest_entry_id()).map_err(|message|self.scene_resource_error(message)),
            None=>Ok(crate::audio_checkpoint::unmeasured_historical_state(raw)),
        }
    }
    pub(super) fn historical_menu_snapshot(&self,next:&GameState)->Result<Option<crate::audio_checkpoint::HistoricalGameSnapshot>,RuntimeError> {
        if next.menu_requests.is_empty(){return Ok(None);}
        let Some(proof)=self.callback_historical_audio()? else{return Ok(None);};
        proof.previous(&self.state,self.instance_id(),self.runtime_epoch(),self.history.latest_entry_id()).map_err(|message|self.scene_resource_error(message))?;
        proof.seal(next).map(Some).map_err(|message|self.scene_resource_error(message))
    }
    /// Pure admission before an external writer; metadata/reveal feedback may
    /// fill this same accepted instant, but another visit or mutation may not.
    pub fn accepts_source_menu_snapshot(&self,previous:&GameState,proof:&crate::audio_checkpoint::HistoricalGameSnapshot)->Result<bool,RuntimeError> {
        proof.accepts(previous,&self.state,self.instance_id(),self.runtime_epoch(),self.history.latest_entry_id()).map_err(|message|self.scene_resource_error(message))
    }
    /// Finish only the exact accepted producer. Raw cancellation state remains
    /// untouched; only fresh PCM metadata is installed into its history clone.
    pub fn finish_source_menu_snapshot(&mut self,previous:GameState,proof:&crate::audio_checkpoint::HistoricalGameSnapshot)->Result<bool,RuntimeError> {
        let measured=proof.previous(&previous,self.instance_id(),self.runtime_epoch(),self.history.latest_entry_id()).map_err(|message|self.scene_resource_error(message))?;
        let changed=previous.pc==self.state.pc && previous.story_identity==self.state.story_identity && (previous.vars!=self.state.vars || previous.random!=self.state.random || previous.ui.story_only()!=self.state.ui.story_only());
        if changed {self.history.push(measured,None);}
        Ok(changed)
    }
}

#[cfg(test)]
mod rollback_boundary_tests {
    use super::*;
    use crate::renderer::SceneRenderBatch;
    use crate::types::SpriteState;
    use rvn_parser::{Hotspot,Position,Transition};
    #[derive(Default)]
    struct Host {reject:bool,published:usize}
    impl Renderer for Host {
        fn validate_scene_batch(&self,_:&SceneRenderBatch)->Result<(),String> {if self.reject {Err("Exact renderer preflight refusal".into())}else{Ok(())}}
        fn publish_scene_batch(&mut self,_:SceneRenderBatch){self.published+=1;}
        fn set_background(&mut self,_:&str,_:&Transition){}
        fn show_sprite(&mut self,_:&str,_:Option<&str>,_:&Position,_:&Transition,_:Option<&SpriteState>){}
        fn hide_sprite(&mut self,_:&str,_:&Transition,_:&SpriteState){}
        fn move_sprite(&mut self,_:&str,_:&Position,_:&Transition,_:&SpriteState){}
        fn show_dialogue(&mut self,_:Option<&str>,_:&str){}
        fn show_choice(&mut self,_:&[String])->usize{0}
        fn show_imagemap(&mut self,_:&str,_:Option<&str>,_:&[Hotspot])->usize{0}
        fn music_play(&mut self,_:&str,_:&Transition,_:Option<&str>){}
        fn music_stop(&mut self,_:&Transition){}
        fn music_set_volume(&mut self,_:f32){}
        fn sfx_play(&mut self,_:&str,_:&Transition){}
        fn sfx_stop(&mut self,_:&str,_:&Transition){}
    }
    fn game()->Engine<Host> {
        let mut game=Engine::from_program(rvn_parser::parse_program("scene label P\n\"first\"\n\"second\"\n\"third\"\nreturn").unwrap(),Host::default(),8).unwrap();
        game.step_until_interaction().unwrap();game.advance_dialogue().unwrap();game.step_until_interaction().unwrap();game
    }
    fn exact(game:&Engine<Host>)->serde_json::Value {
        serde_json::json!({"state":game.state,"history":game.history.audio_identity_snapshot_for_test(),
            "association":game.display_history_entry,"display_generation":game.display_generation,
            "display_owner":game.display_owner,"display_source":format!("{:?}",game.display_reveal_source()),
            "audio_source":format!("{:?}",game.audio_publication_source()),"instance":game.instance_id(),
            "runtime":game.runtime_epoch(),"interface":game.interface_epoch(),"video":game.video_epoch,
            "published":game.renderer.published})
    }
    #[test]
    fn modern_game_rollback_refuses_before_pop_or_id_reallocation_with_and_without_current_anchor() {
        for skip_current in [false,true] {
            let mut game=game();
            // Produce the real continuation without a current displayed anchor.
            if !skip_current {game.advance_dialogue().unwrap();}
            if skip_current {game.history.push(game.state.clone(),Some(HistoryDisplay::Dialogue{character:None,text:"second".into()}));}
            let (source,preview)=game.preview_game_audio_rollback().unwrap().unwrap();
            assert_eq!(source.skip_current_display,skip_current);
            assert_eq!(preview.pc,game.history.entries()[game.history.len()-1-usize::from(skip_current)].state.pc);
            let expected=preview.clone().into_game_state();
            let old_len=game.history.len();let old_counter=game.history.audio_identity_snapshot_for_test()["next"].clone();
            game.renderer.reject=true;let before=exact(&game);
            assert!(game.rollback_from_audio_source(&source).is_err());assert_eq!(exact(&game),before);
            assert!(game.accepts_audio_rollback_source(&source));
            game.renderer.reject=false;assert!(game.rollback_from_audio_source(&source).unwrap());
            assert_eq!(game.history.len(),old_len-1-usize::from(skip_current));
            assert_eq!(game.history.audio_identity_snapshot_for_test()["next"],old_counter);
            assert_eq!(game.state.pc,preview.pc);assert_eq!(game.state.random,preview.random);
            assert_eq!(game.state.vars,expected.vars);assert_eq!(game.display_history_entry,None);
            assert!(!game.accepts_audio_rollback_source(&source));
        }
    }
    #[test]
    fn rollback_source_cannot_adopt_trimmed_repeated_visit_or_a_new_engine_with_equal_epochs() {
        let mut game=game();let (source,_)=game.preview_game_audio_rollback().unwrap().unwrap();
        game.history.push(game.state.clone(),None);let before=exact(&game);
        assert!(!game.accepts_audio_rollback_source(&source));assert!(game.rollback_from_audio_source(&source).is_err());assert_eq!(exact(&game),before);
        let mut independent=Engine::from_program(rvn_parser::parse_program("scene label P\n\"first\"\n\"second\"\n\"third\"\nreturn").unwrap(),Host::default(),8).unwrap();
        independent.step_until_interaction().unwrap();independent.advance_dialogue().unwrap();independent.step_until_interaction().unwrap();
        assert_eq!(independent.runtime_epoch(),source.runtime_epoch);assert_ne!(independent.instance_id(),source.engine_instance);
        let before=exact(&independent);assert!(!independent.accepts_audio_rollback_source(&source));assert!(independent.rollback_from_audio_source(&source).is_err());assert_eq!(exact(&independent),before);
    }
}
