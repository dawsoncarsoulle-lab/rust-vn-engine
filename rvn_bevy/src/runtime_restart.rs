//! Prepared normal restart: one actual source owner and a pre-publication anchor.
use bevy::prelude::*;
use crate::{resources::{VnEngine,VnState},debugger_startup::{DebugBootstrap,BootstrapProfile}};
struct PreparedRestart { source:rvn_core::audio_checkpoint::AudioRequestSource, startup:DebugBootstrap }
/// Opaque real old-owner admission, retained by the incoming owner until its
/// actual Source publication is installed; no portable or generic constructor.
pub(crate) struct RestartIntent{old:rvn_core::audio_checkpoint::AudioRequestSource,incoming:u64,gate:crate::checkpoint_pcm::CheckpointGate}
impl RestartIntent{
 pub(crate) fn accepts_incoming(&self,incoming:u64,gate:&crate::checkpoint_pcm::CheckpointGate)->bool{self.incoming==incoming&&self.gate.same_gate(gate)}
 fn accepts_retained(&self,incoming:u64,gate:&crate::checkpoint_pcm::CheckpointGate,old:&rvn_core::audio_checkpoint::AudioRequestSource)->bool{self.accepts_incoming(incoming,gate)&&self.old==*old}
}
#[derive(Resource,Default)]
pub(crate) struct Requests { pending:Option<PreparedRestart> }
#[derive(Resource)]
struct Anchor { engine:VnEngine, source:rvn_core::audio_checkpoint::AudioRequestSource, menus:Option<crate::source_menus::SourceMenus>, state:VnState, projection_reset:bool }
impl Requests {
    pub(crate) fn admit(&mut self,engine:&VnEngine,label:Option<String>)->Result<(),String>{
        if self.pending.is_some(){return Err("A normal restart already owns an admitted request".into());}
        if engine.0.renderer.scene_batch_pending()||engine.0.renderer.scene_frame_failed{return Err("The actual old publication must settle before normal restart admission".into());}
        if crate::editor_debugger::pending(engine)||crate::editor_debugger::enabled(engine){return Err("Finish or detach the actual debugger before a normal runtime restart".into());}
        let source=engine.0.audio_request_source().map_err(|error|error.to_string())?;
        let mut renderer=crate::bevy_renderer::BevyRenderer::new();
        // One real output fence survives replacement; only commit changes its published owner/readers.
        renderer.audio_checkpoints=engine.0.renderer.audio_checkpoints.clone();let broker=renderer.audio_checkpoints.clone();
        let owner=engine.0.prepare_fresh_startup(renderer,64).map_err(|error|error.to_string())?;
        let intent=RestartIntent{old:source.clone(),incoming:owner.snapshot().instance,gate:broker.gate()};
        let startup=crate::debugger_startup::from_restart_owner(owner,broker,label,intent)?;
        self.pending=Some(PreparedRestart{source,startup});Ok(())
    }
}
pub(crate) fn activate(world:&mut World){
    if world.contains_resource::<DebugBootstrap>()||world.contains_resource::<Anchor>(){return;}
    // The ordinary consumer runs later in First. Keep the exact request until
    // its already accepted old batch is installed; never discard it in Anchor.
    if world.get_resource::<VnEngine>().is_some_and(|engine|engine.0.renderer.scene_batch_pending()){return;}
    let Some(request)=world.get_resource_mut::<Requests>().and_then(|mut requests|requests.pending.take())else{return;};
    let accepts=world.get_resource::<VnEngine>().is_some_and(|engine|engine.0.accepts_audio_request_source(&request.source)&&!engine.0.renderer.scene_frame_failed&&!crate::editor_debugger::pending(engine)&&!crate::editor_debugger::enabled(engine));
    if !accepts||crate::checkpoint_requests::world_blocked(world){report(world,"The actual player changed before normal restart admission".into());return;}
    let state=world.get_resource::<State<VnState>>().map(|state|state.get().clone()).unwrap_or(VnState::Stepping);
    let engine=world.remove_resource::<VnEngine>().expect("Accepted restart retains its unique old owner");
    let menus=world.remove_resource::<crate::source_menus::SourceMenus>();world.init_resource::<crate::source_menus::SourceMenus>();
    world.insert_resource(Anchor{engine,source:request.source,menus,state,projection_reset:false});world.insert_resource(request.startup);
    crate::programmable_ui::cancel_runtime_input(world);
    crate::custom_canvas::cancel_runtime_input(world);
    crate::menu_documents::cancel_runtime_input(world);
    discard_story_presses(world);
    // The old State is a parked presentation until actual publication; real OnExit runs afterward.
}
fn report(world:&mut World,error:String){
    if let Some(mut message)=world.get_resource_mut::<crate::resources::ScriptErrorMessage>(){message.0=error.clone();}
    if let Some(mut menus)=world.get_resource_mut::<crate::source_menus::SourceMenus>(){menus.diagnostic=error.clone();}
    warn!("Normal restart refused: {error}");
}
/// Failure before the first accepted publication preserves the prior player.
pub(crate) fn restore_before_publication(world:&mut World,error:&str)->bool{
    if !world.contains_resource::<Anchor>(){return false;}
    // The failure can happen in First, before this frame's input quarantine.
    discard_story_presses(world);
    let anchor=world.remove_resource::<Anchor>().expect("Checked retained owner");
    world.remove_resource::<DebugBootstrap>();world.insert_resource(anchor.engine);
    if let Some(menus)=anchor.menus{world.insert_resource(menus);}
    world.resource_mut::<NextState<VnState>>().set(anchor.state);report(world,error.into());true
}
/// The actual scene installer has accepted/installed the new epoch. Earlier
/// Init publications cannot be rolled back by a later failed step.
pub(crate) fn publication_installed(world:&mut World,source:&rvn_core::engine::StartupPublicationSource){
    if !world.contains_resource::<Anchor>(){
        assert!(world.get_resource::<DebugBootstrap>().is_none_or(|startup|startup.retirement_intent().is_none()),"Actual first retirement publication lost its retained owner");return;
    }
    world.resource_mut::<DebugBootstrap>().consume_installed_retirement(source);
    discard_story_presses(world);
    reset_projection_before_install(world);world.remove_resource::<Anchor>();
    world.resource_mut::<NextState<VnState>>().set(VnState::Stepping);
    if let Some(mut menus)=world.get_resource_mut::<crate::menu_documents::Menus>(){menus.finish_checkpoint_restore();}
    if let Some(mut menu)=world.get_resource_mut::<crate::resources::MenuState>(){menu.return_to=None;}
    if let Some(mut save)=world.get_resource_mut::<crate::systems::save_menu::SaveMenuState>(){save.active=false;}
    if let Some(mut settings)=world.get_resource_mut::<crate::systems::settings_menu::SettingsMenuState>(){settings.active=false;}
    if let Some(mut skip)=world.get_resource_mut::<crate::resources::SkipMode>(){skip.active=false;}
}

/// Bevy UiFocus may retain Pressed while the mouse stays down. Frozen story
/// handlers must never consume that press after failure or accepted cutover.
/// PanelButton is issued only by the actual host accessibility presenter.
#[derive(Resource)]
struct QuarantinedPointerFrame(u32);
fn discard_story_presses(world:&mut World){
    input::reject_current_events(world);
    // Negative suppression only, tied to the real Main frame. First can
    // restore/install before InputSystem sees the queued physical event.
    if let Some(frame)=world.get_resource::<bevy::core::FrameCount>().map(|frame|frame.0){
        world.insert_resource(QuarantinedPointerFrame(frame));
    }
    let mut query=world.query_filtered::<&mut Interaction,Without<crate::accessibility::PanelButton>>();
    for mut interaction in query.iter_mut(world){
        if *interaction==Interaction::Pressed{*interaction=Interaction::None;}
    }
}
/// Runs after real UiFocus, before any story focus/input consumer. Host input
/// events, ButtonInput, hover/focus and the actual host panel remain available.
pub(crate) fn quarantine_pointer_presses(world:&mut World){
    let this_frame=world.get_resource::<bevy::core::FrameCount>().map(|frame|frame.0);
    let suppressed=world.get_resource::<QuarantinedPointerFrame>().is_some_and(|frame|Some(frame.0)==this_frame);
    if world.contains_resource::<Anchor>()||suppressed{discard_story_presses(world);}
    else{world.remove_resource::<QuarantinedPointerFrame>();}
}
pub(crate) fn presentation_available(world:&World)->bool{!world.contains_resource::<Anchor>()}
/// A per-system read of the actual retained owner for mixed host/story handlers.
/// No mirror flag or cached permission; host controls can remain usable.
#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct PresentationAccess<'w>{anchor:Option<Res<'w,Anchor>>}
impl PresentationAccess<'_>{pub(crate) fn available(&self)->bool{self.anchor.is_none()}}


/// Real replacement intent permits driving while the old display State is parked.
/// Existing focus, preview pause, checkpoint and current-modal checks still apply.
pub(crate) fn parked_restart_can_drive(world:&World)->bool{validate_parked_restart(world).unwrap_or(false)}
pub(crate) fn validate_parked_restart(world:&World)->Result<bool,String>{
    let Some(startup)=world.get_resource::<DebugBootstrap>()else{return Ok(false);};
    if startup.profile!=BootstrapProfile::RuntimeRestart{if world.contains_resource::<Anchor>()||startup.retirement_intent().is_some(){return Err("Parked restart retirement lost its runtime profile".into());}return Ok(false);}
    Ok(retirement_source(world,startup.owner.snapshot().instance,&startup.broker.gate())?.is_some())
}
/// Restore the previous applied base once after accepted Core commit, before
/// the new installer writes Transform/effects. Ordinary First reset is fenced.
pub(crate) fn reset_projection_before_install(world:&mut World){
    if !world.get_resource::<Anchor>().is_some_and(|anchor|!anchor.projection_reset){return;}
    crate::composed_motion::reset_for_owner_replacement(world);
    crate::accessibility::reset_for_owner_replacement(world);
    world.resource_mut::<Anchor>().projection_reset=true;
}
/// Inert bridge backed by the actual retained Engine, never a portable grant.
#[derive(Clone)]
pub(crate) struct RetirementSource {
    old:rvn_core::audio_checkpoint::AudioRequestSource,
    outgoing_instance:u64,
    incoming_instance:u64,
    gate:crate::checkpoint_pcm::CheckpointGate,
}
impl RetirementSource {
    #[cfg(all(feature="video",not(target_arch="wasm32")))]
    pub(crate) fn admits_native_buffer(&self,buffer:&crate::video_audio::Buffer)->bool {
        buffer.verifies_owner(self.outgoing_instance,&self.gate)
    }
}
pub(crate) fn retirement_source(world:&World,incoming:u64,gate:&crate::checkpoint_pcm::CheckpointGate)->Result<Option<RetirementSource>,String>{
    let Some(anchor)=world.get_resource::<Anchor>()else{
        if world.get_resource::<DebugBootstrap>().is_some_and(|startup|startup.retirement_intent().is_some()){return Err("Replacement retirement lost its admitted retained owner before first publication".into());}
        return Ok(None);
    };
    let startup=world.get_resource::<DebugBootstrap>().ok_or("Replacement retirement lost its actual Startup owner")?;
    let intent=startup.retirement_intent().ok_or("Replacement retirement lost its actual admission intent")?;
    if world.contains_resource::<VnEngine>()||startup.profile!=BootstrapProfile::RuntimeRestart
        ||startup.owner.snapshot().instance!=incoming||!startup.accepts_actual_audio_broker()
        ||!intent.accepts_retained(incoming,gate,&anchor.source)||!startup.broker.gate().same_gate(gate)||!anchor.engine.0.renderer.audio_checkpoints.gate().same_gate(gate)
        ||anchor.engine.0.renderer.scene_batch_pending()||anchor.engine.0.renderer.scene_frame_failed
        ||!anchor.engine.0.accepts_audio_request_source(&anchor.source){return Err("Replacement retirement owner/source/gate changed".into());}
    Ok(Some(RetirementSource{old:anchor.source.clone(),outgoing_instance:anchor.engine.0.instance_id(),incoming_instance:incoming,gate:gate.clone()}))
}
pub(crate) fn accepts_retirement(world:&World,incoming:u64,gate:&crate::checkpoint_pcm::CheckpointGate,expected:&Option<RetirementSource>)->Result<bool,String>{
    let current=retirement_source(world,incoming,gate)?;
    Ok(match(current,expected){
        (None,None)=>true,
        (Some(current),Some(expected))=>current.old==expected.old&&current.outgoing_instance==expected.outgoing_instance
            &&current.incoming_instance==expected.incoming_instance&&current.gate.same_gate(&expected.gate),
        _=>false,
    })
}

/// All direct World preparation/refresh/commit APIs share this boundary even
/// without Native video. A missing old owner cannot be replaced by same-gate data.
pub(crate) fn validate_prepared_retirement(world:&World,expected:&Option<RetirementSource>)->Result<(),String>{
    if expected.is_none()&&!world.contains_resource::<Anchor>(){
        if world.get_resource::<DebugBootstrap>().is_some_and(|startup|startup.retirement_intent().is_some()){return Err("Prepared retirement lost its admitted retained owner before first publication".into());}
        return Ok(());
    }
    let startup=world.get_resource::<DebugBootstrap>().ok_or("Prepared retirement lost its actual Startup owner")?;
    if !accepts_retirement(world,startup.owner.snapshot().instance,&startup.broker.gate(),expected)?{
        return Err("Prepared replacement retirement source changed".into());
    }Ok(())
}

#[cfg(all(test,not(target_arch="wasm32")))]
#[path="runtime_restart_contract_tests.rs"]
mod contract_tests;

#[path="runtime_restart_input.rs"]
mod input;
pub(crate) use input::{InputAccess,input_available,capture_suppressed_events,accepts_event};

/// Stage old Widget lifecycle only after incoming Core and host resources are
/// validated. The real retained Anchor is the sole old-source authority.
pub(crate) fn prepare_widget_retirement(world:&World,expected:&Option<RetirementSource>)->Result<Option<rvn_core::engine::PreparedWidgetRuntimeRetirement>,String>{
 validate_prepared_retirement(world,expected)?;
 if expected.is_none(){return Ok(None);}
 let anchor=world.get_resource::<Anchor>().ok_or("Widget retirement lost its actual retained Engine")?;
 anchor.engine.0.prepare_runtime_widget_retirement().map(Some).map_err(|error|error.to_string())
}
pub(crate) fn validate_widget_retirement(world:&World,expected:&Option<RetirementSource>,prepared:&Option<rvn_core::engine::PreparedWidgetRuntimeRetirement>)->Result<(),String>{
 validate_prepared_retirement(world,expected)?;
 match(expected,prepared){
  (None,None)=>Ok(()),
  (Some(_),Some(prepared))=>world.get_resource::<Anchor>().ok_or("Widget retirement lost retained owner")?.engine.0.validate_runtime_widget_retirement(prepared).map_err(|error|error.to_string()),
  _=>Err("Prepared Widget retirement does not cover the actual replacement source".into()),
 }
}
pub(crate) fn reserve_widget_retirement(world:&World,expected:&Option<RetirementSource>,prepared:Option<rvn_core::engine::PreparedWidgetRuntimeRetirement>)->Result<Option<Box<dyn rvn_core::widget_backend_0956::WidgetViewportCommitReservation>>,String>{
 validate_widget_retirement(world,expected,&prepared)?;
 match prepared{Some(prepared)=>world.get_resource::<Anchor>().ok_or("Widget retirement lost retained owner before final fence")?.engine.0.reserve_runtime_widget_retirement(prepared).map_err(|error|error.to_string()),None=>Ok(None)}
}
