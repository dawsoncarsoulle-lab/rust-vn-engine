//! Owned Save15/text preparation from the actual verified AssetServer host.
//! Publication happens under exclusive World access; ECS frame installation
//! remains the existing scene-publication pipeline, separately to be qualified.
use bevy::prelude::*;
use crate::{portable_text_resources::{self, PortableTextHostReceipt, PreparedSelectedLanguage}, resources::VnEngine, systems::settings_menu::Settings};

#[derive(PartialEq, Eq)]
struct SettingsWitness {
    music:u32, sfx:u32, speed:u32, auto:u32,
    typewriter:bool, fullscreen:bool, language:String,
}
impl SettingsWitness {
    fn from_settings(value:&Settings)->Self {
        Self { music:value.music_volume.to_bits(), sfx:value.sfx_volume.to_bits(), speed:value.text_speed.to_bits(), auto:value.auto_speed.to_bits(), typewriter:value.typewriter, fullscreen:value.fullscreen, language:value.language.clone() }
    }
}
/// Fields are private and not serializable. A save supplies desired semantic
/// receipts; only this actual owner can derive a live publication capability.
pub(crate) struct PreparedTextRestore {
    host:PreparedSelectedLanguage,
    core:rvn_core::engine::PreparedSave15,
    settings:SettingsWitness,
    scene:crate::scene_publication::PreparedSceneInstallation,
}
pub(crate) fn prepare(world:&World, program:&rvn_parser::ParsedProgram, save:rvn_core::save::consumer_checkpoint::Save15, expected:&PortableTextHostReceipt)->Result<PreparedTextRestore,String> {
    prepare_inner(world,program,save,expected,None)
}
/// Real current host lifecycle, after legitimate projection caches are warm.
pub(crate) fn capture_host_context(world:&World,program:&rvn_parser::ParsedProgram)->Result<rvn_core::engine::HostLifecycleRestoreContext,String>{
    let game=&world.get_resource::<VnEngine>().ok_or("Actual Engine is missing")?.0;
    crate::scene_publication::preflight_restore_media(world,&game.state.videos)?;
    game.capture_host_lifecycle_restore_context(program).map_err(|e|e.to_string())
}
/// Caller retains this same opaque context throughout pending preparation;
/// stale lifecycle/source is rejected, never re-created on a warming clone.
pub(crate) fn prepare_with_host_context(world:&World,program:&rvn_parser::ParsedProgram,save:rvn_core::save::consumer_checkpoint::Save15,expected:&PortableTextHostReceipt,context:&rvn_core::engine::HostLifecycleRestoreContext)->Result<PreparedTextRestore,String>{
    prepare_inner(world,program,save,expected,Some(context))
}
fn prepare_inner(world:&World,program:&rvn_parser::ParsedProgram,save:rvn_core::save::consumer_checkpoint::Save15,expected:&PortableTextHostReceipt,context:Option<&rvn_core::engine::HostLifecycleRestoreContext>)->Result<PreparedTextRestore,String>{
    let settings=world.get_resource::<Settings>().ok_or("Selected-language restore requires actual Settings")?;
    let engine=world.get_resource::<VnEngine>().ok_or("Selected-language restore requires actual Engine")?;
    if settings.language!=engine.0.locale.as_ref().ok_or("Actual selected locale is missing")?.current_lang() {
        return Err("Actual settings and Engine locale disagree before restore".into());
    }
    let host=portable_text_resources::prepare_selected_language(world,expected)?;
    if save.text_tables!=host.receipt().registry || save.locale.as_ref()!=Some(&host.receipt().locales) {
        return Err("Save15 resources differ from the actual host-derived candidate".into());
    }
    let warming=if let Some(context)=context{
        engine.0.save15_state_for_frame_audio_warming_with_locale(program,&save,context,Some(host.locale().clone()))
    }else{engine.0.save15_state_for_audio_warming(program,&save)}.map_err(|error|error.to_string())?;
    crate::scene_publication::preflight_restore_media(world,&warming.videos)?;
    let ready=if context.is_some(){crate::checkpoint_sources::warm_frame_state(world,&warming)?}else{crate::checkpoint_sources::warm_state(world,&warming)?};
    if !ready{return Err("Save15 PCM sources are still loading".into());}
    let core=if let Some(context)=context{engine.0.prepare_save15_with_locale_and_host_context(program,save,Some(host.locale().clone()),context)}else{engine.0.prepare_save15_with_locale(program,save,Some(host.locale().clone()))}.map_err(|error|error.to_string())?;
    host.verify_host(world)?;
    let scene=crate::scene_publication::prepare_restore(world,&core)?;
    Ok(PreparedTextRestore {host,core,settings:SettingsWitness::from_settings(settings),scene:scene})
}
pub(crate) fn commit(world:&mut World, prepared:PreparedTextRestore)->Result<Option<rvn_core::engine::DebugSession>,String> {
    // The exclusive borrow prevents intervening ECS/system publication. This
    // authenticates observed live assets, queues, epochs, provider and locale
    // mutation generation immediately before the Core publication point.
    prepared.host.verify_host(world)?;
    let current=world.get_resource::<Settings>().ok_or("Actual Settings vanished before restore")?;
    if SettingsWitness::from_settings(current)!=prepared.settings {
        return Err("Actual settings changed after restore preparation".into());
    }
    let selected=prepared.host.locale().current_lang().to_owned();
    // Repeat pure ECS/asset/gain preflights at the last exclusive boundary.
    // The accepted Core plan remains the only authority for the candidate.
    crate::checkpoint_sources::verify_restore_assets(world,&prepared.scene.sources)?;
    let scene=crate::scene_publication::refresh_restore(world,&prepared.core,prepared.scene)?;
    crate::checkpoint_sources::verify_restore_assets(world,&scene.sources)?;
    let session=world.resource_scope(|world,mut engine:Mut<VnEngine>|->Result<_,String>{
        #[cfg(all(feature="video",not(target_arch="wasm32")))]
        {engine.0.renderer.native_admission=Some(scene.native.admission());}
        let result=(||->Result<_,String>{
        let guard=engine.0.begin_prepared_save15_commit(prepared.core).map_err(|error|error.to_string())?;
        let mut settings=world.resource_mut::<Settings>();
        #[cfg(all(feature="video",not(target_arch="wasm32")))]
        {scene.native.with_retirement(|retire|crate::checkpoint_sources::publish_restore_with_retirement(&scene.sources,guard,&mut settings,selected,retire))}
        #[cfg(not(all(feature="video",not(target_arch="wasm32"))))]
        {crate::checkpoint_sources::publish_restore(&scene.sources,guard,&mut settings,selected)}
        })();
        #[cfg(all(feature="video",not(target_arch="wasm32")))]
        {engine.0.renderer.native_admission=None;}
        result
    })?;
    crate::scene_publication::install_restore(world,scene);
    Ok(session)
}
