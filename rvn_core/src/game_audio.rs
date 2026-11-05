//! Explicit audio-only Game registry, independent of narrative Scene activations.
use crate::{GameState,scene_resources::{ResourceRegistry,TargetKind,ResourceTaskKind,EffectParameters},scene_tasks::TaskOwner};
impl GameState {
    /// Actual registry used by audio hosts. A Scene story keeps its existing registry.
    pub fn audio_resource_registry(&self)->Option<&ResourceRegistry>{
        self.scene_execution.as_ref().map(|e|&e.resources).or(self.game_audio.as_ref())
    }
    pub(crate) fn audio_resource_registry_mut(&mut self)->Option<&mut ResourceRegistry>{
        if let Some(e)=&mut self.scene_execution{Some(&mut e.resources)}else{self.game_audio.as_mut()}
    }
    pub(crate) fn audio_active_owner(&self)->Option<crate::scene::ActivationId>{self.scene_execution.as_ref().and_then(|e|e.owner())}
    pub(crate) fn ensure_game_audio(&mut self)->Result<(),String>{
        if self.scene_execution.is_some(){if self.game_audio.is_some(){return Err("Scene and standalone Game audio registries cannot coexist".into());}}
        else{self.game_audio.get_or_insert_with(ResourceRegistry::default);}
        Ok(())
    }
}
pub fn validate(state:&GameState)->Result<(),String>{
 let Some(r)=&state.game_audio else{return Ok(())};
 if state.scene_execution.is_some(){return Err("Scene and standalone Game audio registries cannot coexist".into());}
 if !r.waits.is_empty()||!r.cancelled_waits.is_empty(){return Err("Game audio cannot manufacture a narrative wait".into());}
 for t in &r.targets{if t.owner!=TaskOwner::Game||!matches!(t.identity.kind,TargetKind::Music|TargetKind::SfxPlayback|TargetKind::VoicePlayback|TargetKind::PresentationRetired){return Err("Standalone Game audio has a foreign owner or target family".into());}}
 for t in &r.tasks{
  if t.key.owner!=TaskOwner::Game||t.parked||t.wait.is_some()||!t.dependencies.is_empty()||!matches!(t.kind,ResourceTaskKind::Sfx|ResourceTaskKind::Voice|ResourceTaskKind::Effect){return Err("Standalone Game audio has a foreign or parked task".into());}
  if let Some(e)=&t.effect{if !matches!(e.parameters,EffectParameters::AudioFade{..})||!matches!(t.target.kind,TargetKind::Music|TargetKind::PresentationRetired)||e.presentation.as_ref().is_some_and(|p|!matches!(p.model,crate::retired_presentations::RetiredModel::Music{..})){return Err("Game audio effect must retain an exact Music presentation".into());}}
 }
 r.validate_game_audio()?;
 if !state.music.volume.is_finite()||!(0.0..=1.0).contains(&state.music.volume){return Err("Invalid Game audio volume".into());}
 Ok(())
}

pub(crate) fn is_audio_command(s:&rvn_parser::Statement)->bool{use rvn_parser::Statement::*;matches!(s,MusicPlay{..}|MusicStop{..}|MusicVolume{..}|MusicVolumeValue{..}|SfxPlay{..}|SfxStop{..}|VoicePlay{..}|VoiceStop)}

pub(crate) fn snapshot_music(state:&GameState)->Result<Option<crate::retired_presentations::RetiredPresentation>,String>{
 use crate::retired_presentations::{PresentationSlot,RetiredPresentation,RetiredModel,FrozenAudioSample};
 let r=state.audio_resource_registry().ok_or("Game Music has no actual registry")?;
 let Some(file)=&state.music.current_file else{return Ok(None)};
 let target=r.targets.iter().find(|t|t.owner==TaskOwner::Game&&t.identity.kind==TargetKind::Music&&t.identity.alias=="music").ok_or("Current Game Music has no exact incarnation")?.clone();
 let mut fade=None;
 for t in r.active_effects(None).into_iter().filter(|t|t.target==target.identity){
  if let Some(e)=&t.effect{if let EffectParameters::AudioFade{from,to,..}=e.parameters{if fade.is_some(){return Err("Ambiguous source audio fade".into());}fade=Some(FrozenAudioSample{from,to,clock:e.clock.clone()});}}
 }
 let p=RetiredPresentation{original_slot:PresentationSlot::Music,original_target:target,model:RetiredModel::Music{file:file.clone(),volume:state.music.volume,fade}};p.validate_structure()?;Ok(Some(p))
}

pub(crate) fn has_native_audio(script:&[rvn_parser::Statement])->bool{
 let mut blocks=vec![script];while let Some(block)=blocks.pop(){for s in block{
  if matches!(s,rvn_parser::Statement::MusicVolumeValue{..}){return true;}
  if matches!(s,rvn_parser::Statement::ScheduledConsumer{version:2,body,..} if body.first().is_some_and(is_audio_command)){return true;}
  blocks.extend(crate::consumer_schedule::statement_children(s));
 }}false
}
