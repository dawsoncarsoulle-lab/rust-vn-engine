//! PRIVATE first-publication fence. No saved owner, clock or history push.
use super::*;
impl RollbackHistory {
    #[cfg(test)]
    pub(crate) fn audio_identity_snapshot_for_test(&self)->serde_json::Value {
        serde_json::json!({"next":self.next_entry_id,"entries":self.entries.iter().map(|entry|serde_json::json!({
            "id":entry.entry_id,"state":entry.state,"display":entry.display,
            "pending":entry.audio_publication_pending,"anchor":entry.audio_publication_anchor
        })).collect::<Vec<_>>()})
    }
    pub(crate) fn pending_audio_entry(&self)->Option<u64> {
        self.entries.last().filter(|entry|entry.audio_publication_pending && entry.audio_publication_anchor.is_some()).and_then(|entry|entry.entry_id)
    }
    pub(crate) fn pending_audio_anchor(&self)->Option<&GameState> {
        self.entries.last().filter(|entry|entry.audio_publication_pending && entry.entry_id.is_some()).and_then(|entry|entry.audio_publication_anchor.as_ref())
    }
    pub(crate) fn push_audio_entry(&mut self,state:GameState,display:Option<HistoryDisplay>) {
        let awaiting=state.scene_execution.is_some()||state.game_audio.is_some();self.push(state,display);
        if let Some(entry)=self.entries.last_mut() {entry.audio_publication_pending=awaiting;}
    }
    pub(crate) fn arm_audio_publication(&mut self,producer:&GameState) {
        if let Some(entry)=self.entries.last_mut().filter(|entry|entry.audio_publication_pending) {entry.audio_publication_anchor=Some(producer.clone());}
    }
    pub(crate) fn record_audio_publication(&mut self,entry_id:Option<u64>,expected:&GameState,captured:&GameState)->Result<bool,String> {
        if entry_id!=self.pending_audio_entry(){return Err("Audio publication belongs to a replaced/trimmed/rolled-back entry".into());}
        let Some(entry)=self.entries.last_mut().filter(|entry|entry.audio_publication_pending) else {return Ok(false);};
        let same=match &entry.audio_publication_anchor {Some(anchor)=>crate::audio_checkpoint::same_logical_instant(anchor,expected)?,None=>false};
        if !same {
            // Seal this unmeasured entry: a future coincident PC/alias must not
            // turn a late reader into historical authority.
            entry.audio_publication_pending=false;entry.audio_publication_anchor=None;return Ok(false);
        }
        let mut next=entry.state.clone();
        // A changed resource shape is a non-measured historical refusal, never
        // grounds for rewriting the pre-interactive entry or blocking the line.
        if crate::audio_checkpoint::install_historical_metadata(&mut next,captured).is_err(){entry.audio_publication_pending=false;entry.audio_publication_anchor=None;return Ok(false);}
        entry.state=next;entry.audio_publication_pending=false;entry.audio_publication_anchor=None;Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // Core model fixture only: accept its SFX intent; no decoder checkpoint is supplied.
    struct AudioStateFixture;
    impl crate::Renderer for AudioStateFixture {
        fn validate_scene_batch(&self,_:&crate::renderer::SceneRenderBatch)->Result<(),String>{Ok(())}
        fn set_background(&mut self,_:&str,_:&rvn_parser::Transition){}
        fn show_sprite(&mut self,_:&str,_:Option<&str>,_:&rvn_parser::Position,_:&rvn_parser::Transition,_:Option<&crate::SpriteState>){}
        fn hide_sprite(&mut self,_:&str,_:&rvn_parser::Transition,_:&crate::SpriteState){}
        fn move_sprite(&mut self,_:&str,_:&rvn_parser::Position,_:&rvn_parser::Transition,_:&crate::SpriteState){}
        fn show_dialogue(&mut self,_:Option<&str>,_:&str){}
        fn show_choice(&mut self,_:&[String])->usize{0}
        fn show_imagemap(&mut self,_:&str,_:Option<&str>,_:&[rvn_parser::Hotspot])->usize{0}
        fn music_play(&mut self,_:&str,_:&rvn_parser::Transition,_:Option<&str>){}
        fn music_stop(&mut self,_:&rvn_parser::Transition){}
        fn music_set_volume(&mut self,_:f32){}
        fn sfx_play(&mut self,_:&str,_:&rvn_parser::Transition){}
        fn sfx_stop(&mut self,_:&str,_:&rvn_parser::Transition){}
    }
    fn state()->GameState {
        let mut engine=crate::Engine::from_program(rvn_parser::parse_program("scene label P\nsfx.play(\"same.wav\")\n\"parent\"").unwrap(),AudioStateFixture,8).unwrap();
        engine.step_until_interaction().unwrap();engine.state
    }
    fn captured(state:&GameState)->GameState {
        let mut next=state.clone();let audio=next.scene_execution.as_mut().unwrap().resources.tasks.iter_mut().find_map(|task|task.audio.as_mut()).unwrap();
        audio.checkpoint=Some(crate::audio_checkpoint::AudioCheckpoint{version:1,asset_sha256:[2;32],pcm_sha256:[3;32],channels:2,sample_rate:48_000,total_samples:8,next_sample:3,next_channel:1,looping:false,loops_completed:0});next
    }
    #[test]
    fn first_ready_publication_attaches_only_metadata_to_the_same_entry_once_without_push() {
        let state=state();let mut history=RollbackHistory::new(8);history.push_audio_entry(state.clone(),None);
        history.arm_audio_publication(&state);
        let len=history.len();let frozen=captured(&state);let entry=history.pending_audio_entry();assert!(history.record_audio_publication(entry,&state,&frozen).unwrap());
        assert_eq!(history.len(),len);assert_eq!(history.entries()[0].state.pc,state.pc);assert_eq!(history.entries()[0].state.random,state.random);
        assert!(crate::audio_checkpoint::same_logical_instant(&history.entries()[0].state,&state).unwrap());
        let before=serde_json::to_vec(&history.entries()[0].state).unwrap();let mut later=frozen.clone();later.scene_execution.as_mut().unwrap().resources.tasks.iter_mut().find_map(|task|task.audio.as_mut()).unwrap().checkpoint.as_mut().unwrap().next_sample=4;
        later.scene_execution.as_mut().unwrap().resources.tasks.iter_mut().find_map(|task|task.audio.as_mut()).unwrap().checkpoint.as_mut().unwrap().next_channel=0;
        assert!(history.record_audio_publication(entry,&state,&later).is_err());assert_eq!(serde_json::to_vec(&history.entries()[0].state).unwrap(),before);
    }
    #[test]
    fn delayed_ready_after_clock_or_pc_change_never_rewrites_old_unmeasured_history() {
        let state=state();let mut history=RollbackHistory::new(8);history.push_audio_entry(state.clone(),None);
        history.arm_audio_publication(&state);
        let before=serde_json::to_vec(&history.entries()[0].state).unwrap();let mut changed=state.clone();changed.pc+=1;
        let entry=history.pending_audio_entry();assert!(!history.record_audio_publication(entry,&changed,&captured(&changed)).unwrap());assert_eq!(serde_json::to_vec(&history.entries()[0].state).unwrap(),before);
        assert!(history.record_audio_publication(entry,&state,&captured(&state)).is_err());assert_eq!(serde_json::to_vec(&history.entries()[0].state).unwrap(),before);
        assert!(crate::audio_checkpoint::require_saved_checkpoints(&history.entries()[0].state).is_err());
    }
}
