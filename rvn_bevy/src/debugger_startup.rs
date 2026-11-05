//! Authenticated debug bootstrap admission before initialization effects.
use std::path::Path;
use rvn_core::engine::EngineStartup;
#[path="debugger_enum_startup1025.rs"]
mod enum_resources1025;
#[path="debugger_library_startup1027.rs"]
mod library_resources1027;
pub(crate) use library_resources1027::{prepare as prepare_with_library_resources1027,prepare_runtime as prepare_runtime_with_library_resources1027};
pub(crate) use enum_resources1025::{prepare as prepare_with_enum_assets1025,prepare_runtime as prepare_runtime_with_enum_assets1025};
use crate::bevy_renderer::BevyRenderer;
#[cfg(all(test,not(target_arch="wasm32")))]
#[path="debugger_startup_world_tests.rs"]
pub(crate) mod tests;
#[cfg(all(test,not(target_arch="wasm32")))]
#[path="startup_imagemap_world_tests_0998.rs"]
mod imagemap_world_tests;
#[path="debugger_startup_overlay.rs"]
mod overlay;
#[path="debugger_startup_clock.rs"]
mod clock;
#[path="debugger_startup_driver.rs"]
pub(crate) mod driver;
#[path="startup_imagemap_images_0997.rs"]
mod imagemap_images;
#[path="startup_imagemap_native_0998.rs"]
pub(crate) mod native_imagemap;
pub(crate) fn validate_installed_imagemap_images(world:&bevy::prelude::World)->Result<(),String>{imagemap_images::verify_installed(world)}
pub(crate) use clock::{video_clock_allowed,suspend_blocked_video_clock};
pub(crate) use overlay::read_overlay;

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub(crate) enum BootstrapProfile { Debug, RuntimeInitial, RuntimeRestart }

/// The real pending owner. No ordinary VnEngine is fabricated during Init.
#[derive(bevy::prelude::Resource)]
pub(crate) struct DebugBootstrap {
    pub(crate) profile:BootstrapProfile,
    pub(crate) owner:EngineStartup<BevyRenderer>,
    pub(crate) start_label:Option<String>,
    pub(crate) broker:crate::checkpoint_broker::AudioCheckpointBroker,
    // Captured only while the actual renderer transfers into this owner.
    // A later replacement of the mutable host broker cannot replace this proof.
    audio_owner:StartupAudioOwner,
    retirement_intent:Option<crate::runtime_restart::RestartIntent>,
    pub(crate) publication_pending:bool,
    pub(crate) failed:Option<String>,
    pub(crate) mode:Option<rvn_core::editor_debugger::StepKind>,
    pub(crate) sites:Option<rvn_core::editor_debugger::SitePage>,
    pub(crate) intent:u64,
}
struct StartupAudioOwner{
    instance:u64,
    gate:crate::checkpoint_pcm::CheckpointGate,
}
impl DebugBootstrap{
    pub(crate) fn retirement_intent(&self)->Option<&crate::runtime_restart::RestartIntent>{self.retirement_intent.as_ref()}
    /// Called after the real Source batch installation, never on computation or handoff.
    pub(crate) fn consume_installed_retirement(&mut self,source:&rvn_core::engine::StartupPublicationSource){
        let intent=self.retirement_intent.as_ref().expect("Retained restart owns its actual admission intent");
        assert!(self.profile==BootstrapProfile::RuntimeRestart&&self.accepts_actual_audio_broker()
            &&self.owner.snapshot().instance==source.instance_id()&&self.owner.snapshot().epoch==source.runtime_epoch()
            &&intent.accepts_incoming(source.instance_id(),&self.broker.gate()),"Installed publication differs from actual retirement admission");
        self.retirement_intent=None;
    }
    /// Pure owner admission before any Source/metadata access, never a step.
    pub(crate) fn accepts_actual_audio_broker(&self)->bool{
        self.owner.snapshot().instance==self.audio_owner.instance
            &&self.broker.gate().same_gate(&self.audio_owner.gate)
    }
}
pub(crate) fn prepare(
    root:&Path,program:rvn_parser::ParsedProgram,nonce:u64,
    persistent:std::collections::HashMap<String,rvn_parser::Value>,
    context:rvn_core::TextRuntimeContext,start_label:Option<String>,
)->Result<DebugBootstrap,String>{
    prepare_with_classes(root,program,Default::default(),nonce,persistent,context,start_label)
}
pub(crate) fn prepare_with_classes(
 root:&Path,program:rvn_parser::ParsedProgram,classes:rvn_core::widget_class_source_0950::AdmittedWidgetClasses,nonce:u64,
 persistent:std::collections::HashMap<String,rvn_parser::Value>,context:rvn_core::TextRuntimeContext,start_label:Option<String>,
)->Result<DebugBootstrap,String>{
    if nonce==0{return Err("Debugger startup requires its authenticated preview nonce".into());}
    let overlay=read_overlay(root)?.ok_or("Debug startup requires the actual captured compiler observation")?;
    let renderer=BevyRenderer::new();let broker=renderer.audio_checkpoints.clone();
    let owner=rvn_core::Engine::prepare_widget_class_input_startup(program,classes,renderer,64,persistent,context,Some((overlay,nonce))).map_err(|error|error.to_string())?;
    if let Some(label)=&start_label{owner.validate_start_label(label).map_err(|error|error.to_string())?;}
    let audio_owner=StartupAudioOwner{instance:owner.snapshot().instance,gate:broker.gate()};
    Ok(DebugBootstrap{profile:BootstrapProfile::Debug,owner,start_label,broker,audio_owner,retirement_intent:None,publication_pending:false,failed:None,mode:None,sites:None,intent:0})
}

/// Host inputs stay opaque and are recalculated from the current actual World.
pub(crate) struct PreparedBootstrapStep{
    intent:u64,
    gate:crate::checkpoint_pcm::CheckpointGate,
    core:Box<rvn_core::engine::PreparedStartupStep>,
    text:crate::portable_text_resources::LiveTextHostReceipt,
    retirement:Option<crate::runtime_restart::RetirementSource>,
    widget_retirement:Option<rvn_core::engine::PreparedWidgetRuntimeRetirement>,
    presentation:StartupPresentationReceipt,
    sources:Option<crate::checkpoint_sources::PreparedSources>,
    imagemap_images:Option<imagemap_images::PreparedImages>,
    owners:Option<crate::programmable_ui::owner_hosts_0943::PreparedOwnerProjection>,
    audio:Option<crate::scene_audio::PreparedAudio>,
    retired:Option<crate::scene_retired::PreparedRetired>,
    #[cfg(all(feature="video",not(target_arch="wasm32")))]
    native:Option<crate::native_video::PreparedNativeRestore>,
    ready:bool,
}
#[derive(PartialEq,Eq)]
struct StartupPresentationReceipt{music:u32,sfx:u32,reduced:bool,cg:Option<String>,focused:bool}
fn presentation_receipt(world:&bevy::prelude::World,state:&rvn_core::GameState)->Result<StartupPresentationReceipt,String>{
    let focused=world.iter_entities().filter(|entity|entity.contains::<bevy::window::PrimaryWindow>())
        .find_map(|entity|entity.get::<bevy::prelude::Window>()).map(|window|window.focused)
        .unwrap_or_else(||world.iter_entities().filter_map(|entity|entity.get::<bevy::prelude::Window>()).all(|window|window.focused));
    if !focused{return Err("Actual startup publication lost World focus".into());}
    let music=crate::music_mix::user_gain(world)?;
    let sfx=world.get_resource::<crate::systems::settings_menu::Settings>().map(|settings|settings.sfx_volume).unwrap_or(1.0);
    if !sfx.is_finite()||!(0.0..=1.0).contains(&sfx){return Err("Invalid actual startup SFX preference".into());}
    Ok(StartupPresentationReceipt{music:music.to_bits(),sfx:sfx.to_bits(),focused,reduced:world.get_resource::<crate::accessibility::Accessibility>().is_some_and(|access|access.settings.reduced_motion),cg:state.cinematic.current.as_deref().map(|id|crate::systems::cinematic::resolve_cg_asset(world,id)).transpose()?})
}
impl PreparedBootstrapStep{
    pub(crate) fn core(&self)->&rvn_core::engine::PreparedStartupStep{&self.core}
    pub(crate) fn ready(&self)->bool{self.ready}
}
/// A uniquely transferred publication. The World remains blocked until its
/// existing scene installer consumes these owned batches/readers.
pub(crate) struct CommittedBootstrapStep{
    pub(crate) progress:rvn_core::engine::DebugProgress,
    pub(crate) next_step_kind:Option<rvn_core::editor_debugger::StepKind>,
    pub(crate) terminal:Option<rvn_core::engine::AcceptedStartupVideoTerminal>,
    pub(crate) publication:Option<crate::bevy_renderer::StartupPublication>,
    pub(crate) sources:Option<crate::checkpoint_sources::PreparedSources>,
    imagemap_images:Option<imagemap_images::PreparedImages>,
    pub(crate) owners:Option<crate::programmable_ui::owner_hosts_0943::PreparedOwnerProjection>,
    pub(crate) audio:Option<crate::scene_audio::PreparedAudio>,
    pub(crate) retired:Option<crate::scene_retired::PreparedRetired>,
    #[cfg(all(feature="video",not(target_arch="wasm32")))]
    pub(crate) native:Option<crate::native_video::PreparedNativeRestore>,
}
#[inline(never)]
pub(crate) fn prepare_world_step(world:&bevy::prelude::World,kind:rvn_core::editor_debugger::StepKind)->Result<PreparedBootstrapStep,String>{
    prepare_world_step_boxed(world,kind).map(|prepared|*prepared)
}
#[inline(never)]
pub(crate) fn prepare_world_step_boxed(world:&bevy::prelude::World,kind:rvn_core::editor_debugger::StepKind)->Result<Box<PreparedBootstrapStep>,String>{
    if world.get_resource::<crate::scene_publication::ScenePresentation>().is_none()||world.get_resource::<bevy::prelude::Events<crate::vn_command::VnCommand>>().is_none(){return Err("Actual startup presentation/event runtime is missing".into());}
    let startup=world.get_resource::<DebugBootstrap>().ok_or("Actual startup owner is missing")?;
    if !startup.accepts_actual_audio_broker(){return Err("Startup actual renderer broker was replaced".into());}
    if startup.publication_pending||startup.failed.is_some(){return Err("Previous startup publication is unsettled".into());}
    let retirement=crate::runtime_restart::retirement_source(world,startup.owner.snapshot().instance,&startup.broker.gate())?;
    let text=crate::portable_text_resources::live_receipt(world)?;
    let core=startup.owner.prepare_step_boxed(kind).map_err(|error|error.to_string())?;
    prepare_world_core(world,core,text,retirement)
}
/// Input source comes exclusively from an admitted completed physical image.
/// Retain this exact Core candidate through the existing AssetServer/PCM driver.
#[inline(never)]
pub(crate) fn prepare_world_imagemap_selection(world:&bevy::prelude::World,input:&rvn_core::engine::StartupImagemapSource,selected:usize)->Result<Box<PreparedBootstrapStep>,String>{
    let startup=world.get_resource::<DebugBootstrap>().ok_or("Actual input Startup owner is missing")?;
    if !startup.accepts_actual_audio_broker()||startup.publication_pending||startup.failed.is_some()||driver::pending(world){return Err("Actual Init input publication is unsettled".into());}
    let retirement=crate::runtime_restart::retirement_source(world,startup.owner.snapshot().instance,&startup.broker.gate())?;
    let text=crate::portable_text_resources::live_receipt(world)?;
    let core=startup.owner.prepare_imagemap_selection_boxed(input,selected).map_err(|error|error.to_string())?;
    prepare_world_core(world,core,text,retirement)
}
#[inline(never)]
pub(crate) fn prepare_world_entry(world:&bevy::prelude::World)->Result<PreparedBootstrapStep,String>{
    prepare_world_entry_boxed(world).map(|prepared|*prepared)
}
#[inline(never)]
pub(crate) fn prepare_world_entry_boxed(world:&bevy::prelude::World)->Result<Box<PreparedBootstrapStep>,String>{
    let startup=world.get_resource::<DebugBootstrap>().ok_or("Actual completed Startup owner is missing")?;
    if !startup.accepts_actual_audio_broker()||startup.publication_pending||startup.failed.is_some(){return Err("Actual completed Startup publication is unsettled".into());}
    let retirement=crate::runtime_restart::retirement_source(world,startup.owner.snapshot().instance,&startup.broker.gate())?;
    let text=crate::portable_text_resources::live_receipt(world)?;
    let core=startup.owner.prepare_gameplay_entry_boxed(startup.start_label.as_deref()).map_err(|error|error.to_string())?;
    prepare_world_core(world,core,text,retirement)
}
#[inline(never)]
fn prepare_world_core(world:&bevy::prelude::World,core:Box<rvn_core::engine::PreparedStartupStep>,text:crate::portable_text_resources::LiveTextHostReceipt,retirement:Option<crate::runtime_restart::RetirementSource>)->Result<Box<PreparedBootstrapStep>,String>{
    if world.get_resource::<crate::scene_publication::ScenePresentation>().is_none()||world.get_resource::<bevy::prelude::Events<crate::vn_command::VnCommand>>().is_none(){return Err("Actual startup presentation/event runtime is missing".into());}
    crate::runtime_restart::validate_prepared_retirement(world,&retirement)?;
    if !crate::portable_text_resources::accepts_live_receipt(world,&text)?{return Err("Actual startup text host changed during Core preparation".into());}
    let startup=world.get_resource::<DebugBootstrap>().ok_or("Actual startup owner is missing")?;
    let presentation=presentation_receipt(world,core.render_batch().map(|batch|&batch.state).unwrap_or_else(||startup.owner.state()))?;
    let mut prepared=Box::new(PreparedBootstrapStep{intent:startup.intent,gate:startup.broker.gate(),core,text,retirement,widget_retirement:None,presentation,sources:None,imagemap_images:None,owners:None,audio:None,retired:None,
        #[cfg(all(feature="video",not(target_arch="wasm32")))] native:None,
        ready:false});
    refresh_world_step(world,&mut prepared)?;Ok(prepared)
}
/// Loading retains the exact completed Core calculation. Retries validate and
/// prepare assets only; they cannot run another producer or advance its Probe.
pub(crate) fn refresh_world_step(world:&bevy::prelude::World,prepared:&mut PreparedBootstrapStep)->Result<bool,String>{
    crate::runtime_restart::validate_prepared_retirement(world,&prepared.retirement)?;
    if !crate::portable_text_resources::accepts_live_receipt(world,&prepared.text)?{return Err("Actual startup text host changed during source staging".into());}
    let startup=world.get_resource::<DebugBootstrap>().ok_or("Actual startup owner is missing")?;
    if !startup.accepts_actual_audio_broker(){return Err("Startup actual renderer broker was replaced".into());}
    if startup.publication_pending||startup.failed.is_some()||startup.intent!=prepared.intent||!startup.broker.gate().same_gate(&prepared.gate){return Err("Startup owner/intent/broker changed during source staging".into());}
    startup.owner.validate_prepared_step(&prepared.core).map_err(|error|error.to_string())?;
    if presentation_receipt(world,prepared.core.render_batch().map(|batch|&batch.state).unwrap_or_else(||startup.owner.state()))?!=prepared.presentation{return Err("Actual startup presentation host changed during staging".into());}
    if let Some(batch)=prepared.core.render_batch(){
        if batch.display.is_some()&&prepared.core.imagemap_interaction()!=batch.display.as_ref(){return Err("Startup display is not its actual prepared input wait".into());}
        if prepared.imagemap_images.is_none(){prepared.imagemap_images=Some(imagemap_images::begin(world,batch.display.as_ref())?);}
        if !imagemap_images::refresh(world,batch.display.as_ref(),prepared.imagemap_images.as_mut().unwrap())?{return Ok(false);}
        prepared.owners=Some(crate::programmable_ui::owner_hosts_0943::prepare_scene(batch,startup.owner.snapshot().instance)?);
        crate::systems::audio::current_music_gain_from_state(&batch.state,batch.runtime_epoch).and_then(|gain|crate::music_mix::in_world(world,gain))?;
        crate::systems::cinematic::preflight_cg(world,&batch.state)?;
        prepared.retired=Some(crate::scene_retired::prepare(world,&batch.state,batch.runtime_epoch)?);
        if prepared.sources.is_none(){prepared.sources=crate::checkpoint_sources::prepare_startup(world,&prepared.core)?;}
        let Some(sources)=&prepared.sources else{return Ok(false);};
        crate::checkpoint_sources::verify_restore_assets(world,sources)?;
        #[cfg(all(feature="video",not(target_arch="wasm32")))] {
            if prepared.native.is_none(){prepared.native=crate::native_video::prepare_startup(world,&prepared.core)?;}
            let Some(native)=&prepared.native else{return Ok(false);};
            crate::native_video::verify_startup(world,&prepared.core,native)?;
        }
        prepared.audio=Some(crate::scene_audio::prepare(world,&batch.state,batch.runtime_epoch)?);
        if prepared.widget_retirement.is_none(){prepared.widget_retirement=crate::runtime_restart::prepare_widget_retirement(world,&prepared.retirement)?;}
        crate::runtime_restart::validate_widget_retirement(world,&prepared.retirement,&prepared.widget_retirement)?;
    }
    prepared.ready=true;Ok(true)
}
#[inline(never)]
pub(crate) fn commit_world_step(world:&mut bevy::prelude::World,prepared:PreparedBootstrapStep)->Result<CommittedBootstrapStep,String>{
    commit_world_step_boxed(world,Box::new(prepared)).map(|committed|*committed)
}
/// Pointer-sized transfer for the real driver. The same opaque Core candidate
/// and staged host resources are consumed exactly once after the same guards.
#[inline(never)]
pub(crate) fn commit_world_step_boxed(world:&mut bevy::prelude::World,mut prepared:Box<PreparedBootstrapStep>)->Result<Box<CommittedBootstrapStep>,String>{
    crate::runtime_restart::validate_prepared_retirement(world,&prepared.retirement)?;
    if !prepared.ready{return Err("Startup source staging has not completed".into());}
    if world.get_resource::<crate::scene_publication::ScenePresentation>().is_none()||world.get_resource::<bevy::prelude::Events<crate::vn_command::VnCommand>>().is_none(){return Err("Actual startup presentation/event runtime vanished".into());}
    if world.get_resource::<DebugBootstrap>().is_none_or(|startup|startup.intent!=prepared.intent||!startup.broker.gate().same_gate(&prepared.gate)){return Err("Actual startup intent/broker changed after preparation".into());}
    if !crate::portable_text_resources::accepts_live_receipt(world,&prepared.text)?{return Err("Actual startup text host changed after preparation".into());}
    let actual=world.resource::<DebugBootstrap>();
    if !actual.accepts_actual_audio_broker(){return Err("Startup actual renderer broker was replaced".into());}
    if presentation_receipt(world,prepared.core.render_batch().map(|batch|&batch.state).unwrap_or_else(||actual.owner.state()))?!=prepared.presentation{return Err("Actual startup presentation host changed after preparation".into());}
    if let Some(batch)=prepared.core.render_batch(){
        imagemap_images::verify(world,batch.display.as_ref(),prepared.imagemap_images.as_ref().ok_or("Actual Startup image staging is incomplete")?)?;
    }
    if let Some(sources)=&prepared.sources{crate::checkpoint_sources::verify_restore_assets(world,sources)?;}
    #[cfg(all(feature="video",not(target_arch="wasm32")))] {
        if let Some(native)=&prepared.native{crate::native_video::verify_startup(world,&prepared.core,native)?;}
        else if prepared.core.render_batch().is_some(){return Err("Actual startup Native staging is incomplete".into());}
    }
    let widget_reservation=if prepared.core.render_batch().is_some(){crate::runtime_restart::reserve_widget_retirement(world,&prepared.retirement,prepared.widget_retirement.take())?}else{None};
    let mut startup=world.get_resource_mut::<DebugBootstrap>().ok_or("Actual startup owner vanished")?;
    commit_owned_world_step(&mut startup,prepared,widget_reservation)
}
/// All World preflight has finished. Publishers use only this real owner and
/// its captured host receipts, so no by-value World removal or owner copy is
/// needed. A rejection leaves the same owner and broker installed in World.
#[inline(never)]
fn commit_owned_world_step(startup:&mut DebugBootstrap,prepared:Box<PreparedBootstrapStep>,mut widget_reservation:Option<Box<dyn rvn_core::widget_backend_0956::WidgetViewportCommitReservation>>)->Result<Box<CommittedBootstrapStep>,String>{
        if startup.publication_pending||startup.failed.is_some(){return Err("Previous startup publication is unsettled".into());}
        let next_step_kind=prepared.core.next_step_kind();
        let guard=startup.owner.begin_prepared_host_step_commit_boxed(prepared.core).map_err(|error|error.to_string())?;
        #[cfg(all(feature="video",not(target_arch="wasm32")))]
        if let Some(native)=&prepared.native{native.verify_startup_guard(&guard,&startup.broker)?;}
        let publish=|retire:&mut dyn FnMut()|{
            let mut retire_actual=||{
                if let Some(reservation)=widget_reservation.take(){reservation.commit(retire);}else{retire();}
            };
            match &prepared.sources{
                Some(sources)=>crate::checkpoint_sources::publish_startup_with_retirement(sources,guard,&mut retire_actual),
                None=>guard.commit_computation().map_err(|error|error.to_string()),
            }
        };
        #[cfg(all(feature="video",not(target_arch="wasm32")))]
        let committed=if let Some(native)=&prepared.native{native.with_retirement(publish)?}else{publish(&mut||{})?};
        #[cfg(not(all(feature="video",not(target_arch="wasm32"))))]
        let committed=publish(&mut||{})?;
        startup.publication_pending=committed.publication.is_some();
        Ok(Box::new(CommittedBootstrapStep{progress:committed.progress,next_step_kind,terminal:committed.terminal,publication:committed.publication,sources:prepared.sources,imagemap_images:prepared.imagemap_images,owners:prepared.owners,audio:prepared.audio,retired:prepared.retired,
            #[cfg(all(feature="video",not(target_arch="wasm32")))] native:prepared.native}))
}
/// Single transfer of the batch/readers already accepted by the real owner.
/// No publisher or producer is called again during presentation installation.
#[inline(never)]
pub(crate) fn install_committed_world_step(world:&mut bevy::prelude::World,committed:CommittedBootstrapStep)->rvn_core::engine::DebugProgress{
    install_committed_world_step_boxed(world,Box::new(committed))
}
#[inline(never)]
pub(crate) fn install_committed_world_step_boxed(world:&mut bevy::prelude::World,committed:Box<CommittedBootstrapStep>)->rvn_core::engine::DebugProgress{
    if let Some(publication)=committed.publication{
        imagemap_images::install(world,publication.batch.display.as_ref(),committed.imagemap_images.expect("Publishing Startup owns its exact Image handles"));
        crate::scene_publication::install_startup(world,publication,committed.owners.expect("Ready startup owns its exact owner projection"),committed.sources.expect("Ready startup owns its Sources"),committed.audio.expect("Ready startup owns its audio installation"),committed.retired.expect("Ready startup owns its retired installation"),
            #[cfg(all(feature="video",not(target_arch="wasm32")))] committed.native);
    }
    committed.progress
}

/// Normal admission uses the real source owner without debugger credentials.
pub(crate) fn prepare_runtime(program:rvn_parser::ParsedProgram,persistent:std::collections::HashMap<String,rvn_parser::Value>,context:rvn_core::TextRuntimeContext,start_label:Option<String>)->Result<DebugBootstrap,String>{
    prepare_runtime_with_classes(program,Default::default(),persistent,context,start_label)
}
pub(crate) fn prepare_runtime_with_classes(program:rvn_parser::ParsedProgram,classes:rvn_core::widget_class_source_0950::AdmittedWidgetClasses,persistent:std::collections::HashMap<String,rvn_parser::Value>,context:rvn_core::TextRuntimeContext,start_label:Option<String>)->Result<DebugBootstrap,String>{
    let renderer=BevyRenderer::new();let broker=renderer.audio_checkpoints.clone();
    let owner=rvn_core::Engine::prepare_widget_class_input_startup(program,classes,renderer,64,persistent,context,None).map_err(|error|error.to_string())?;
    from_runtime_owner(owner,broker,start_label,BootstrapProfile::RuntimeInitial)
}
pub(crate) fn from_runtime_owner(owner:EngineStartup<BevyRenderer>,broker:crate::checkpoint_broker::AudioCheckpointBroker,start_label:Option<String>,profile:BootstrapProfile)->Result<DebugBootstrap,String>{
    if profile!=BootstrapProfile::RuntimeInitial{return Err("Generic runtime admission only creates an initial owner; restart requires its actual admitted intent".into());}
    runtime_owner(owner,broker,start_label,profile,None)
}
pub(crate) fn from_restart_owner(owner:EngineStartup<BevyRenderer>,broker:crate::checkpoint_broker::AudioCheckpointBroker,start_label:Option<String>,intent:crate::runtime_restart::RestartIntent)->Result<DebugBootstrap,String>{
    if !intent.accepts_incoming(owner.snapshot().instance,&broker.gate()){return Err("Runtime restart intent differs from its actual incoming owner/broker".into());}
    runtime_owner(owner,broker,start_label,BootstrapProfile::RuntimeRestart,Some(intent))
}
fn runtime_owner(owner:EngineStartup<BevyRenderer>,broker:crate::checkpoint_broker::AudioCheckpointBroker,start_label:Option<String>,profile:BootstrapProfile,retirement_intent:Option<crate::runtime_restart::RestartIntent>)->Result<DebugBootstrap,String>{
    if let Some(label)=&start_label{owner.validate_start_label(label).map_err(|error|error.to_string())?;}
    let audio_owner=StartupAudioOwner{instance:owner.snapshot().instance,gate:broker.gate()};
    Ok(DebugBootstrap{profile,owner,start_label,broker,audio_owner,retirement_intent,publication_pending:false,failed:None,mode:Some(rvn_core::editor_debugger::StepKind::Continue),sites:None,intent:0})
}
