//! Owns the same completed candidate across actual World/AssetServer frames.
//! No engine is fabricated, and no probe, producer or renderer is replayed.
use bevy::prelude::*;
use rvn_core::editor_debugger::StepKind;
use super::{DebugBootstrap,PreparedBootstrapStep};

#[derive(Resource,Default)]
pub(crate) struct Driver {
    prepared:Option<Box<PreparedBootstrapStep>>,
    suspended:bool,
    handoff_ready:bool,
    entry_preparing:bool,
    handoff_mode:Option<StepKind>,
    terminal:Option<crate::scene_publication::StartupTerminalTicket>,
    identity:Option<(u64,u64,crate::checkpoint_pcm::CheckpointGate)>,
}
pub(crate) fn suspended(world:&World)->bool{world.get_resource::<Driver>().is_some_and(|driver|driver.suspended)}
pub(crate) fn pending(world:&World)->bool {
    world.get_resource::<Driver>().is_some_and(|driver|driver.prepared.is_some()||driver.handoff_ready)
}
pub(crate) fn pause(world:&mut World)->Result<(),String>{
    crate::editor_preview::set_paused(world,true)?;
    world.init_resource::<Driver>();world.resource_mut::<Driver>().suspended=true;Ok(())
}
pub(crate) fn run(world:&mut World,kind:StepKind)->Result<(),String>{
    let actual=world.get_resource::<DebugBootstrap>().ok_or("Actual startup owner is absent")?;
    if actual.failed.is_some()||actual.publication_pending{return Err("Startup publication is not ready".into());}
    actual.owner.validate_driver_run(kind).map_err(|error|error.to_string())?;
    if world.get_resource::<Driver>().is_some_and(|driver|driver.prepared.is_some()&&!driver.entry_preparing)&&actual.mode!=Some(kind){return Err("Actual startup source staging retains another armed command".into());}
    let complete=actual.owner.is_complete();
    crate::editor_preview::set_paused_for_debugger(world,kind!=StepKind::Continue)?;
    world.init_resource::<Driver>();{let mut driver=world.resource_mut::<Driver>();driver.suspended=false;if complete{driver.handoff_mode=(kind==StepKind::Continue).then_some(kind);}}
    world.resource_mut::<DebugBootstrap>().mode=Some(kind);Ok(())
}
pub(crate) fn resume(world:&mut World)->Result<(),String>{
    let kind=world.get_resource::<DebugBootstrap>().ok_or("Actual startup owner is absent")?.owner.driver_armed_step_kind().or_else(||world.resource::<DebugBootstrap>().mode).unwrap_or(StepKind::Continue);
    run(world,kind)
}
pub(crate) fn retain_terminal(world:&mut World,ticket:crate::scene_publication::StartupTerminalTicket){
    world.init_resource::<Driver>();
    let actual=world.resource::<DebugBootstrap>();let identity=(actual.owner.snapshot().instance,actual.owner.snapshot().epoch,actual.broker.gate());
    let mut driver=world.resource_mut::<Driver>();assert!(driver.terminal.is_none(),"Only one genuine callback can be active");driver.terminal=Some(ticket);driver.identity=Some(identity);
}
pub(crate) fn retain_imagemap_selection(world:&mut World,prepared:Box<PreparedBootstrapStep>)->Result<(),String>{
    let actual=world.get_resource::<DebugBootstrap>().ok_or("Actual Init input owner vanished")?;
    if actual.publication_pending||actual.failed.is_some()||!actual.owner.waiting_for_imagemap(){return Err("Actual Init input is no longer waiting".into());}
    actual.owner.validate_prepared_step(&prepared.core).map_err(|error|error.to_string())?;
    world.init_resource::<Driver>();
    let mut driver=world.resource_mut::<Driver>();
    if driver.prepared.is_some()||driver.handoff_ready||driver.terminal.is_some(){return Err("Another actual Startup transition is pending".into());}
    driver.prepared=Some(prepared);driver.entry_preparing=false;Ok(())
}
pub(crate) fn terminal_pending(world:&World)->bool{world.get_resource::<Driver>().is_some_and(|driver|driver.terminal.is_some())}
pub(crate) fn report_failure(world:&mut World,error:String){fail(world,error)}
fn fail(world:&mut World,error:String){
    if crate::runtime_restart::restore_before_publication(world,&error){world.remove_resource::<Driver>();return;}
    if let Some(mut message)=world.get_resource_mut::<crate::resources::ScriptErrorMessage>(){message.0=error.clone();}
    if let Some(mut startup)=world.get_resource_mut::<DebugBootstrap>(){startup.failed=Some(error);startup.mode=None;}
    if let Some(mut driver)=world.get_resource_mut::<Driver>(){driver.prepared=None;driver.handoff_ready=false;}
    let _=crate::editor_preview::set_paused_for_debugger(world,true);
    if let Some(mut next)=world.get_resource_mut::<NextState<crate::resources::VnState>>(){next.set(crate::resources::VnState::Error);}
}
fn world_open(world:&World)->bool{
    if world.get_resource::<DebugBootstrap>().is_some_and(|startup|startup.profile!=super::BootstrapProfile::Debug)
        && (crate::editor_preview::paused(world)||(!crate::runtime_restart::parked_restart_can_drive(world)&&world.get_resource::<State<crate::resources::VnState>>().is_some_and(|state|!matches!(state.get(),crate::resources::VnState::Stepping|crate::resources::VnState::Waiting|crate::resources::VnState::Animating)))){return false;}
    if world.get_resource::<crate::accessibility::Accessibility>().is_some_and(|access|access.blocked)||world.get_resource::<crate::source_menus::SourceMenus>().is_some_and(|menus|menus.modal()){return false;}
    world.iter_entities().filter(|entity|entity.contains::<bevy::window::PrimaryWindow>()).find_map(|entity|entity.get::<Window>()).map(|window|window.focused).unwrap_or_else(||world.iter_entities().filter_map(|entity|entity.get::<Window>()).all(|window|window.focused))
}
pub(crate) fn drain(world:&mut World){
    if !world.contains_resource::<DebugBootstrap>(){world.remove_resource::<Driver>();return;}
    if world.contains_resource::<crate::resources::VnEngine>(){fail(world,"Startup cannot share a gameplay owner".into());return;}
    if let Err(error)=crate::runtime_restart::validate_parked_restart(world){fail(world,error);return;}
    if let Err(error)=super::suspend_blocked_video_clock(world){fail(world,error);return;}
    world.init_resource::<Driver>();
    let actual=world.resource::<DebugBootstrap>();
    let identity=(actual.owner.snapshot().instance,actual.owner.snapshot().epoch,actual.broker.gate());
    let changed=world.resource::<Driver>().identity.as_ref().is_some_and(|old|old.0!=identity.0||old.1!=identity.1||!old.2.same_gate(&identity.2));
    if changed{let mut driver=world.resource_mut::<Driver>();driver.prepared=None;driver.terminal=None;driver.handoff_ready=false;}
    world.resource_mut::<Driver>().identity=Some(identity);
    let actual=world.resource::<DebugBootstrap>();
    if actual.failed.is_some()||actual.publication_pending||world.resource::<Driver>().suspended||crate::checkpoint_requests::world_blocked(world)||!world_open(world){return;}
    // Readiness is not debugger authority. Keep the original armed command
    // while real AssetServer/text events drain, for every startup profile.
    // A retained candidate must still follow its existing exact refresh path.
    if world.resource::<Driver>().prepared.is_none(){
        match crate::portable_text_resources::startup_loads_ready(world){Ok(true)=>{},Ok(false)=>return,Err(error)=>{fail(world,error);return;}}
    }
    let actual=world.resource::<DebugBootstrap>();
    let complete=actual.owner.is_complete();
    let kind=match actual.mode{Some(kind)=>kind,None if complete=>StepKind::Continue,None=>return};
    // An executed Wait is not another evaluator command. Genuine queue reports
    // are handled in VideoFeedbackSet and may install the actual callback.
    if actual.owner.waiting_for_video()||actual.owner.waiting_for_imagemap()&&world.resource::<Driver>().prepared.is_none(){return;}
    let mut driver=world.remove_resource::<Driver>().expect("Actual driver was initialized");
    let result=advance(world,&mut driver,complete,kind);
    world.insert_resource(driver);if let Err(error)=result{fail(world,error);}else if let Err(error)=super::suspend_blocked_video_clock(world){fail(world,error);}
}

// Each stage receives references or a unique heap token. In particular, the
// large Core commit and the gameplay Engine transfer cannot occupy the same
// driver closure frame. This is a transport boundary, not another scheduler.
#[inline(never)]
fn stage(world:&World,driver:&mut Driver,complete:bool,kind:StepKind)->Result<bool,String>{
    if let Some(prepared)=driver.prepared.as_mut(){
        super::refresh_world_step(world,prepared)
    }else{
        driver.entry_preparing=complete;
        let prepared=if complete{super::prepare_world_entry_boxed(world)?}else{super::prepare_world_step_boxed(world,kind)?};
        let ready=prepared.ready();driver.prepared=Some(prepared);Ok(ready)
    }
}
#[inline(never)]
fn advance(world:&mut World,driver:&mut Driver,complete:bool,kind:StepKind)->Result<(),String>{
    if !stage(world,driver,complete,kind)?{return Ok(());}
    publish(world,driver)?;
    finish_step(world,driver,kind)
}
#[inline(never)]
fn publish(world:&mut World,driver:&mut Driver)->Result<(),String>{
    let committed=super::commit_world_step_boxed(world,driver.prepared.take().expect("Ready candidate exists"))?;
    let next=committed.next_step_kind;
    let terminal=committed.terminal.clone();
    super::install_committed_world_step_boxed(world,committed);
    if let Some(receipt)=terminal{if let Some(ticket)=driver.terminal.take(){
        crate::scene_publication::acknowledge_startup_terminal(world,ticket,&receipt)?;
    }}
    let actual=world.resource::<DebugBootstrap>();
    driver.identity=Some((actual.owner.snapshot().instance,actual.owner.snapshot().epoch,actual.broker.gate()));
    let debug=actual.profile==super::BootstrapProfile::Debug;
    world.resource_mut::<DebugBootstrap>().mode=if debug{next}else{Some(StepKind::Continue)};
    driver.handoff_ready=world.resource::<DebugBootstrap>().owner.is_complete();
    Ok(())
}
#[inline(never)]
fn finish_step(world:&mut World,driver:&mut Driver,kind:StepKind)->Result<(),String>{
    if driver.entry_preparing{return handoff(world,driver);}
    if driver.handoff_ready{driver.handoff_mode=(kind==StepKind::Continue).then_some(kind);}
    let actual=world.resource::<DebugBootstrap>();
    if actual.profile==super::BootstrapProfile::Debug{
        let next=actual.mode;
        crate::editor_preview::set_paused_for_debugger(world,next!=Some(StepKind::Continue))?;
    }
    Ok(())
}
#[inline(never)]
fn handoff(world:&mut World,driver:&mut Driver)->Result<(),String>{
    if driver.terminal.is_some(){return Err("Gameplay handoff cannot discard an unacknowledged actual callback".into());}
    if world.resource::<DebugBootstrap>().retirement_intent().is_some(){return Err("Actual restart retirement has not installed its first publication".into());}
    crate::editor_preview::finish_startup_handoff(world)?;
    let startup=world.remove_resource::<DebugBootstrap>().expect("Actual completed owner remains installed");
    let (breakpoints,watches)=startup.owner.driver_configuration();
    let engine=startup.owner.into_engine().expect("Gameplay entry was committed only for a completed actual owner");
    let engine=crate::resources::VnEngine(engine);
    let debug=startup.profile==super::BootstrapProfile::Debug;
    if debug{crate::editor_debugger::inherit_startup(&engine,breakpoints,watches,driver.handoff_mode)?;}
    let initial=startup.profile==super::BootstrapProfile::RuntimeInitial;
    world.insert_resource(engine);
    driver.handoff_ready=false;driver.entry_preparing=false;
    if debug{crate::editor_preview::set_paused_for_debugger(world,driver.handoff_mode!=Some(StepKind::Continue))?;}
    else if let Some(mut next)=world.get_resource_mut::<NextState<crate::resources::VnState>>(){next.set(if initial{crate::resources::VnState::TitleScreen}else{crate::resources::VnState::Stepping});}
    Ok(())
}

pub(crate) fn drain_runtime(world:&mut World){
    if world.get_resource::<DebugBootstrap>().is_some_and(|startup|startup.profile!=super::BootstrapProfile::Debug){drain(world);}
}

#[cfg(all(test,not(target_arch="wasm32")))]
#[path="debugger_startup_driver_stack_tests_0924.rs"]
mod stack_tests_0924;

#[cfg(all(test,not(target_arch="wasm32")))]
#[path="debugger_startup_text_readiness_tests1027.rs"]
mod text_readiness_tests1027;
