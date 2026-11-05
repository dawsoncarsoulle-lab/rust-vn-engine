//! World policy only re-admits an observation issued by the actual Init owner.
use bevy::prelude::*;
use super::DebugBootstrap;
pub(crate) fn video_clock_allowed(world:&World,view:&rvn_core::video::VideoView)->bool{
    if world.contains_resource::<crate::resources::VnEngine>(){return false;}
    let Some(startup)=world.get_resource::<DebugBootstrap>()else{return false;};
    if !startup.accepts_actual_audio_broker()||startup.publication_pending||startup.failed.is_some()
        ||startup.mode!=Some(rvn_core::editor_debugger::StepKind::Continue)
        ||crate::editor_preview::paused(world)||crate::checkpoint_requests::world_blocked(world)||super::driver::pending(world){return false;}
    if world.get_resource::<crate::accessibility::Accessibility>().is_some_and(|access|access.blocked)
        ||world.get_resource::<crate::source_menus::SourceMenus>().is_some_and(|menus|menus.modal()){return false;}
    if !world.get_resource::<State<crate::resources::VnState>>().is_some_and(|state|matches!(state.get(),crate::resources::VnState::Stepping|crate::resources::VnState::Waiting|crate::resources::VnState::Animating)){return false;}
    let focused=world.iter_entities().filter(|entity|entity.contains::<bevy::window::PrimaryWindow>())
        .find_map(|entity|entity.get::<Window>()).map(|window|window.focused)
        .unwrap_or_else(||world.iter_entities().filter_map(|entity|entity.get::<Window>()).all(|window|window.focused));
    if !focused||startup.broker.gate().try_read(|paused|!paused)!=Some(true){return false;}
    startup.owner.video_clock_source(view).ok().flatten().is_some_and(|source|startup.owner.accepts_video_clock_source(&source))
}
/// Called by the actual First driver even when the Native tick is not scheduled.
/// Closing delivery never seeks, consumes a right sample or recreates a worker.
pub(crate) fn suspend_blocked_video_clock(world:&World)->Result<(),String>{
    let Some(startup)=world.get_resource::<DebugBootstrap>()else{return Ok(());};
    if !startup.accepts_actual_audio_broker(){return Err("Startup actual renderer broker was replaced before clock admission".into());}
    #[cfg(all(feature="video",not(target_arch="wasm32")))]
    if !startup.owner.video_views().map_err(|error|error.to_string())?.iter().any(|view|video_clock_allowed(world,view)){
        crate::video_audio::pause_sources(world)?;
    }
    Ok(())
}
