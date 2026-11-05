//! Child of engine: exact audio commands on one checked narrative candidate.
use super::*;
use crate::audio_playback::{self, AudioAdmission, AudioClockRoute, AudioFamily, AudioPlayback, AudioReport};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioReportDisposition { Consumed, RejectedStale, Deferred }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioReportReceipt { pub ordinal:usize, pub disposition:AudioReportDisposition }

impl<R: Renderer> Engine<R> {
    pub(super) fn exec_owned_audio_play(&mut self,family:AudioFamily,file:String,fade_seconds:f64)->Result<(),RuntimeError> {
        self.exec_owned_audio_play_from(self.state.clone(),family,file,fade_seconds)
    }
    pub(super) fn exec_owned_audio_play_from(&mut self,mut next:GameState,family:AudioFamily,file:String,fade_seconds:f64)->Result<(),RuntimeError>{
        let playback=AudioPlayback::new(family,file,fade_seconds).map_err(|message|self.scene_resource_error(message))?;
        let owner=Self::command_owner(&next);
        audio_playback::register(&mut next,self.runtime_epoch,owner,playback).map_err(|message|self.scene_resource_error(message))?;
        next.pc+=1;
        self.commit_audio_state(next)
    }
    pub(super) fn exec_owned_audio_stop(&mut self,family:AudioFamily,file:Option<String>,fade_seconds:f64)->Result<(),RuntimeError> {
        self.exec_owned_audio_stop_from(self.state.clone(),family,file,fade_seconds)
    }
    pub(super) fn exec_owned_audio_stop_from(&mut self,mut next:GameState,family:AudioFamily,file:Option<String>,fade_seconds:f64)->Result<(),RuntimeError>{
        audio_playback::stop_story(&mut next,family,file.as_deref(),fade_seconds).map_err(|message|self.scene_resource_error(message))?;
        next.pc+=1;
        self.commit_audio_state(next)
    }
    pub fn audio_playback_views(&self)->Result<Vec<audio_playback::AudioPlaybackView>,RuntimeError> {
        audio_playback::describe(&self.state,self.runtime_epoch).map_err(|message|self.scene_resource_error(message))
    }
    /// Route admission is independent of decoder payload. A parked resident
    /// stays deferred; a superseded pre-Park callback is already stale.
    pub fn audio_report_admission(&self,instance_id:u64,token:&crate::scene_resources::ResourceToken)->AudioAdmission {
        if instance_id!=self.instance_id() {return AudioAdmission::Stale;}
        audio_playback::admission(&self.state,self.runtime_epoch,token)
    }
    pub(super) fn prepare_audio_reports(&self,reports:Vec<AudioReport>,route:AudioClockRoute,enabled:bool)
        ->Result<(Vec<AudioReport>,Vec<AudioReportReceipt>),RuntimeError> {
        if reports.len()>audio_playback::MAX_AUDIO_PLAYBACKS {return Err(self.scene_resource_error("Audio terminal report cap: 64".into()));}
        let mut deliveries=Vec::new();let mut receipts=Vec::with_capacity(reports.len());
        for (ordinal,report) in reports.into_iter().enumerate() {
            let disposition=match self.audio_report_admission(report.instance_id,&report.token) {
                AudioAdmission::Stale=>AudioReportDisposition::RejectedStale,
                AudioAdmission::Deferred=>AudioReportDisposition::Deferred,
                AudioAdmission::Current=>{
                    let host=matches!(report.token.task.key.owner,crate::scene_tasks::TaskOwner::Host{..});
                    let own_route=match route {AudioClockRoute::Host=>host,AudioClockRoute::Story=>!host};
                    if enabled && own_route {deliveries.push(report);AudioReportDisposition::Consumed}
                    else {AudioReportDisposition::Deferred}
                }
            };
            receipts.push(AudioReportReceipt{ordinal,disposition});
        }
        Ok((deliveries,receipts))
    }
    pub(super) fn apply_audio_deliveries(&self,next:&mut GameState,reports:&[AudioReport])->Result<bool,RuntimeError> {
        let mut changed=false;
        for report in reports {
            if report.instance_id!=self.instance_id() {continue;}
            // A preceding Canvas/Video/timer action may already revoke the
            // entry-time capability. Never rebind its file to a new playback.
            changed|=audio_playback::apply_report(next,self.runtime_epoch,report)
                .map_err(|message|self.scene_resource_error(message))?==AudioAdmission::Current;
        }
        Ok(changed)
    }
}

impl<R:Renderer> Engine<R>{
 pub(super) fn commit_audio_state(&mut self,next:GameState)->Result<(),RuntimeError>{
  self.commit_audio_state_with_timer(next,self.active_timer.clone())
 }
 // One existing publication path. A Timer replacement carries both candidate
 // owners through preflight; neither live owner changes on renderer refusal.
 pub(super) fn commit_audio_state_with_timer(&mut self,mut next:GameState,timer_context:Option<(f32,String,f32)>)->Result<(),RuntimeError>{
  crate::game_tasks::sync_current(&mut next).map_err(|m|self.scene_resource_error(m))?;
  if self.narrative.is_some(){return self.commit_scene_state(next);}
  self.consumer_program.validate_state_binding(&next).map_err(|e|self.eval_err(e,"Game audio frame"))?;
  crate::scene_resources::validate_current_resource_state(&next).map_err(|m|self.scene_resource_error(m))?;
  // Describing UI can populate its cache. Keep that observation on a private
  // candidate until the actual renderer has accepted the entire publication.
  let cache=self.audio_interface_cache_transaction()?;
  let batch=self.build_scene_batch_with_timer_context(next,self.runtime_epoch,self.interface_epoch,self.video_epoch,timer_context.as_ref())?;
  self.renderer.validate_scene_batch(&batch).map_err(|m|self.scene_resource_error(m))?;cache.commit();
  self.state=batch.state.clone();self.active_timer=timer_context;self.renderer.publish_scene_batch(batch);Ok(())
 }
 /// Normalize a real evaluated numeric VN volume; this is not a source permit.
 pub(super) fn music_volume_value(&self,value:rvn_parser::Value)->Result<f32,RuntimeError>{
  let level=match value {rvn_parser::Value::Float(v)=>v,rvn_parser::Value::Int(v)=>v as f32,
   _=>return Err(self.scene_resource_error("Music volume requires a numeric value".into()))};
  if !level.is_finite(){return Err(self.scene_resource_error("Music volume requires a finite value".into()));}
  Ok(level.clamp(0.0,1.0))
 }
 pub(super) fn exec_scheduled_audio(&mut self,next:GameState,command:Statement)->Result<(),RuntimeError>{
  let sfx=|file:String|if file.starts_with("sfx/"){file}else{format!("sfx/{file}")};
  match &command {
   Statement::MusicPlay{file,..}|Statement::SfxPlay{file,..}|Statement::SfxStop{file,..}|Statement::VoicePlay{file} if !rvn_ui::programmable::safe_asset_path(file)=>return Err(self.scene_resource_error("Invalid authored Native audio source path".into())),
   Statement::MusicVolume{level} if !level.is_finite()||!(0.0..=1.0).contains(level)=>return Err(self.scene_resource_error("Invalid authored Native music volume".into())),
   _=>{}
  }
  match command{
   Statement::MusicPlay{file,transition}=>self.exec_narrative_music_play_from(next,if file.starts_with("music/"){file}else{format!("music/{file}")},transition),
   Statement::MusicStop{transition}=>self.exec_narrative_music_stop_from(next,transition),
   Statement::MusicVolume{level}=>self.exec_narrative_music_volume_from(next,level),
   Statement::SfxPlay{file,transition}=>self.exec_owned_audio_play_from(next,AudioFamily::Sfx,sfx(file),f64::from(transition.duration_ms())/1000.0),
   Statement::SfxStop{file,transition}=>self.exec_owned_audio_stop_from(next,AudioFamily::Sfx,Some(sfx(file)),f64::from(transition.duration_ms())/1000.0),
   Statement::VoicePlay{file}=>self.exec_owned_audio_play_from(next,AudioFamily::Voice,file,0.0),
   Statement::VoiceStop=>self.exec_owned_audio_stop_from(next,AudioFamily::Voice,None,0.0),
   _=>Err(self.scene_resource_error("Not an actual audio command".into()))
  }
 }
}

impl<R:Renderer> Engine<R>{
 pub(super) fn standalone_audio_restore_batch(&self,next:GameState)->Result<crate::renderer::SceneRenderBatch,RuntimeError>{
  if self.narrative.is_some()||next.scene_execution.is_some()||(next.game_audio.is_none()&&next.game_tasks.is_none())||next.story_identity!=self.state.story_identity||next.pc>self.script.len()||next.call_stack.len()>128||next.call_stack.iter().any(|pc|*pc>self.script.len()){return Err(self.scene_resource_error("Standalone audio restore has no actual Game source/position".into()));}
  self.consumer_program.validate_emitted(&self.script).and_then(|_|self.consumer_program.validate_state_binding(&next)).map_err(|e|self.eval_err(e,"Game audio restore programme"))?;
  crate::scene_resources::validate_current_resource_state(&next).map_err(|m|self.scene_resource_error(m))?;
  crate::audio_checkpoint::require_saved_checkpoints(&next).map_err(|m|self.scene_resource_error(m))?;
  self.renderer.validate_audio_restore(&next).map_err(|m|self.scene_resource_error(m))?;
  let runtime=self.runtime_epoch.checked_add(1).ok_or_else(||self.scene_resource_error("Runtime epoch exhausted".into()))?;let interface=self.interface_epoch.checked_add(1).ok_or_else(||self.scene_resource_error("Interface epoch exhausted".into()))?;let video=self.next_video_epoch()?;self.display_generation.checked_add(1).ok_or_else(||self.scene_resource_error("Display generation exhausted".into()))?;
  // This is synchronous disposable restore planning, not a DebugSession.
  // A fresh unpiloted probe would suspend the real Screen computation.
  let mut batch={let mut candidate=self.debugger_candidate(crate::eval::debugger_eval::Probe::shared());candidate.functions.debug_probe=None;candidate.build_scene_batch(next,runtime,interface,video)?};
  {let mut candidate=self.debugger_candidate(crate::eval::debugger_eval::Probe::shared());candidate.functions.debug_probe=None;candidate.state=batch.state.clone();batch.display=candidate.current_interaction()?;}
  self.renderer.validate_scene_batch(&batch).map_err(|m|self.scene_resource_error(m))?;Ok(batch)
 }
}

/// A renderer refusal must retain the exact source anchor, including its
/// private description cache. A frame copies this bounded cache, not the Script.
pub(super) struct AudioInterfaceCacheTransaction<'a>{
 cache:&'a Mutex<Vec<InterfaceViewSnapshot>>,before:Option<Vec<InterfaceViewSnapshot>>,
}
impl AudioInterfaceCacheTransaction<'_>{pub(super) fn commit(mut self){self.before=None;}}
impl Drop for AudioInterfaceCacheTransaction<'_>{fn drop(&mut self){if let Some(before)=self.before.take(){*self.cache.lock().unwrap_or_else(|poison|poison.into_inner())=before;}}}
impl<R:Renderer> Engine<R>{
 pub(super) fn audio_interface_cache_transaction(&self)->Result<AudioInterfaceCacheTransaction<'_>,RuntimeError>{
  let before=self.interface_view_cache.lock().map_err(|_|self.scene_resource_error("Game audio description cache unavailable".into()))?.iter().map(|entry|InterfaceViewSnapshot{locale:entry.locale.as_ref().map(LocaleManager::debugger_copy),text_resources:entry.text_resources.clone(),ui:entry.ui.clone(),globals:entry.globals.clone(),views:entry.views.clone()}).collect();
  Ok(AudioInterfaceCacheTransaction{cache:&self.interface_view_cache,before:Some(before)})
 }
}
