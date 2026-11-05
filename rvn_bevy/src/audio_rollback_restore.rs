//! Actual Story rollback under the existing broker/Source/World boundary.
//! New Native-video rollback needs its own authenticated frame history; this
//! audio bridge refuses such a plan before any Core or Source publication.
use bevy::prelude::*;
use crate::resources::VnEngine;
pub(crate) struct PreparedAudioRollbackWorld{core:rvn_core::engine::PreparedAudioRollback,sources:crate::checkpoint_sources::PreparedSources}
fn preflight(world:&World,core:&rvn_core::engine::PreparedAudioRollback)->Result<(crate::scene_audio::PreparedAudio,crate::scene_retired::PreparedRetired,Option<crate::systems::dialogue::ChoiceDialogueBackdrop>),String>{
 if cfg!(target_arch="wasm32"){return Err("Atomic owned audio rollback browser installation is not qualified".into());}
 let game=world.get_resource::<VnEngine>().ok_or("Actual Engine is missing")?;
 if !game.0.accepts_prepared_audio_rollback(core){return Err("Rollback Core source changed".into());}
 if game.0.renderer.scene_batch_pending()||!game.0.renderer.pending.is_empty()||game.0.renderer.menu_pending{return Err("Rollback has unsettled outgoing publication".into());}
 if world.get_resource::<crate::scene_publication::ScenePresentation>().is_none()||world.get_resource::<Events<crate::vn_command::VnCommand>>().is_none(){return Err("Rollback presentation runtime is absent".into());}
 let batch=core.render_batch();
 let _=crate::programmable_ui::owner_hosts_0943::prepare_scene(batch,game.0.instance_id())?;
 if !game.0.state.videos.tracks.is_empty()||!batch.state.videos.tracks.is_empty(){return Err("Audio rollback requires a separately authenticated native-media history plan".into());}
 if !batch.menu_requests.is_empty(){return Err("Rollback cannot replay a menu producer".into());}
 if let Some(rvn_core::Interaction::Dialogue{text,..})=&batch.display{
  if text.len()>rvn_core::display_reveal::MAX_REVEAL_TEXT_BYTES{return Err("Rollback dialogue exceeds its portable bound".into());}
  if batch.state.scene_execution.is_some(){core.display_reveal_source().ok_or("Rollback dialogue lacks future display authority")?;}
  if let Some(reveal)=batch.state.scene_execution.as_ref().and_then(|e|e.display.as_ref()).and_then(|d|d.reveal.as_ref()){reveal.for_text(text)?;}
 }
 let choice=if matches!(batch.display,Some(rvn_core::Interaction::Choice{..})){Some(crate::systems::dialogue::prepare_choice_backdrop_from_interaction(core.choice_backdrop().cloned())?)}else{None};
 crate::systems::audio::current_music_gain_from_state(&batch.state,batch.runtime_epoch).and_then(|logical|crate::music_mix::in_world(world,logical))?;
 crate::systems::cinematic::preflight_cg(world,&batch.state)?;
 Ok((crate::scene_audio::prepare(world,&batch.state,batch.runtime_epoch)?,crate::scene_retired::prepare(world,&batch.state,batch.runtime_epoch)?,choice))
}
pub(crate) fn prepare(world:&World,source:&rvn_core::audio_checkpoint::AudioRollbackSource)->Result<Option<PreparedAudioRollbackWorld>,String>{
 if cfg!(target_arch="wasm32"){return Err("Atomic owned audio rollback browser installation is not qualified".into());}
 let game=world.get_resource::<VnEngine>().ok_or("Actual Engine is missing")?;
 let warming=game.0.audio_rollback_state_for_warming(source).map_err(|e|e.to_string())?;
 if !crate::checkpoint_sources::warm_state(world,&warming)?{return Ok(None);}
 let core=game.0.prepare_audio_rollback(source).map_err(|e|e.to_string())?;
 let _=preflight(world,&core)?;
 let Some(sources)=crate::checkpoint_sources::prepare_audio_rollback(world,&core)?else{return Ok(None);};
 crate::checkpoint_sources::verify_restore_assets(world,&sources)?;
 Ok(Some(PreparedAudioRollbackWorld{core,sources}))
}
pub(crate) fn commit(world:&mut World,plan:PreparedAudioRollbackWorld)->Result<(),String>{
 let(audio,retired,choice)=preflight(world,&plan.core)?;
 crate::checkpoint_sources::verify_restore_assets(world,&plan.sources)?;
 let display=plan.core.render_batch().display.clone();
 let speech=crate::accessibility::retired_speech_0975::PreparedRetiredSpeech::prepare(world,&plan.core.render_batch().retired_widget_speech)?;
 let owners=crate::programmable_ui::owner_hosts_0943::prepare_scene(plan.core.render_batch(),world.resource::<VnEngine>().0.instance_id())?;
 world.resource_scope(|_world,mut game:Mut<VnEngine>|->Result<(),String>{let guard=game.0.begin_prepared_audio_rollback_commit(plan.core).map_err(|e|e.to_string())?;crate::checkpoint_sources::publish_audio_rollback(&plan.sources,guard)})?;
 crate::scene_publication::install_preflighted(world,plan.sources,audio,retired,choice,display,owners,speech,
  #[cfg(all(feature="video",not(target_arch="wasm32")))] None);
 Ok(())
}

#[cfg(test)]
impl PreparedAudioRollbackWorld{pub(crate) fn future_display_source(&self)->Option<rvn_core::display_reveal::DisplayRevealSource>{self.core.display_reveal_source().cloned()}pub(crate) fn future_choice_backdrop(&self)->Option<rvn_core::Interaction>{self.core.choice_backdrop().cloned()}}
