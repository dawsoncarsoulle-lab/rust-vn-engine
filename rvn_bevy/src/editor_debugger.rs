//! Debugger consumers share the ordinary Engine and Preview pause boundary.
//! The queue retains producer sources and receipt closures, never recaptures a
//! later event/snapshot when an owned evaluator continuation is stopped.
use bevy::prelude::*;
use rvn_core::engine::{DebugOperation,DebugOutcome,DebugProgress,DebugSession};
use rvn_core::editor_debugger::{Breakpoint,Snapshot,StepKind,Watch};
use crate::resources::{VnEngine,VnState};
use std::collections::VecDeque;

#[path="editor_widget_materialization_0964.rs"]
mod widget_materialization_0964;
pub(crate) use widget_materialization_0964::{drive_widget_materialization,adopt_restored_widget_session};
const MAX_QUEUED:usize=32;
const MAX_QUEUE_BYTES:usize=4*1024*1024;
type Finish=Box<dyn FnOnce(&mut World,DebugOutcome)->Result<(),String>+Send+Sync>;
type Abort=Box<dyn FnOnce(&mut World)+Send+Sync>;
struct Queued { instance:u64,epoch:u64,operation:DebugOperation,cost:usize,finish:Option<Finish>,abort:Option<Abort>,soft_failure:bool }
struct Active { queued:Queued,session:DebugSession,restore_pause:bool,needs_pause:bool }
struct HostInstant {instance:u64,epoch:u64,seconds:f64}
#[derive(Default)]
pub(crate) struct Mailbox {
    enabled:bool,breakpoints:Vec<Breakpoint>,watches:Vec<Watch>,
    queue:VecDeque<Queued>,active:Option<Active>,bytes:usize,
    mode:Option<StepKind>,snapshot:Option<Snapshot>,story:Option<Result<Option<rvn_core::Interaction>,String>>,
    story_queued:bool,error:Option<String>,host_instant:Option<HostInstant>,sites:Option<rvn_core::editor_debugger::SitePage>,
}
#[derive(Debug)]
pub(crate) enum Poll<T>{Immediate,Pending,Complete(Result<T,String>)}
impl Mailbox {
    fn pending(&self)->bool {self.active.is_some()||!self.queue.is_empty()}
    fn admit(&mut self,engine:&VnEngine,operation:DebugOperation,finish:Option<Finish>,retained:usize)->Result<(),String>{
        let mut counter=Counter(0);
        // Streaming Debug visits the actual opaque source and borrowed payload
        // before accepting their queue ownership; it allocates no encoded copy.
        use std::fmt::Write;
        write!(&mut counter,"{operation:?}").map_err(|_|"Debugger operation exceeds queue budget")?;
        counter.0=counter.0.checked_add(retained).filter(|bytes|*bytes<=MAX_QUEUE_BYTES).ok_or("Debugger retained callback exceeds queue budget")?;
        if self.queue.len()+usize::from(self.active.is_some())>=MAX_QUEUED
            ||self.bytes.checked_add(counter.0).is_none_or(|n|n>MAX_QUEUE_BYTES){return Err("Debugger callback queue is full; operation not accepted".into());}
        self.bytes+=counter.0;
        let soft_failure=matches!(&operation,DebugOperation::Input(input)if input.source.host_role.is_some());
        self.queue.push_back(Queued{instance:engine.0.instance_id(),epoch:engine.0.runtime_epoch(),operation,cost:counter.0,finish,abort:None,soft_failure});
        Ok(())
    }
}
struct Counter(usize);
impl std::fmt::Write for Counter {
    fn write_str(&mut self,text:&str)->std::fmt::Result {
        self.0=self.0.checked_add(text.len()).ok_or(std::fmt::Error)?;
        if self.0>MAX_QUEUE_BYTES {return Err(std::fmt::Error);}Ok(())
    }
}
pub(crate) fn enabled(engine:&VnEngine)->bool {
    engine.0.renderer.debugger.lock().is_ok_and(|mailbox|mailbox.enabled)
}
pub(crate) fn pending(engine:&VnEngine)->bool {
    engine.0.renderer.debugger.lock().map_or(true,|mailbox|mailbox.pending())
}
pub(crate) fn story_ready(engine:&VnEngine)->bool {
    engine.0.renderer.debugger.lock().is_ok_and(|mailbox|mailbox.story.is_some())
}
pub(crate) fn snapshot(engine:&VnEngine)->Option<Snapshot>{engine.0.renderer.debugger.lock().ok()?.snapshot.clone()}
pub(crate) fn encoded_snapshot(engine:&VnEngine)->Result<Option<String>,String>{
    let mailbox=engine.0.renderer.debugger.lock().map_err(|_|"Debugger queue poisoned")?;
    if !mailbox.enabled{return Ok(None);}
    let mut snapshot=if let Some(active)=mailbox.active.as_ref().filter(|active|engine.0.accepts_debug_session(&active.session)){
        engine.0.debug_session_snapshot(&active.session)
    }else{engine.0.debug_current_snapshot(&mailbox.watches)?};
    // A completed callback is no longer an active lexical frame. Keep its
    // error/work visible, without showing an old site as the new instruction.
    if mailbox.active.is_none(){
        if let Some(previous)=&mailbox.snapshot{snapshot.steps=previous.steps;snapshot.value_work=previous.value_work;}
        snapshot.error=mailbox.error.clone();
    }
    snapshot.sites=mailbox.sites.clone();snapshot.breakpoints=mailbox.breakpoints.clone();
    snapshot.running=mailbox.mode.is_some()&&mailbox.pending();
    snapshot.armed=mailbox.active.as_ref().and_then(|active|active.session.armed_step_kind());
    snapshot.encode().map(Some)
}
pub(crate) fn sites(world:&mut World,offset:usize)->Result<(),String>{
    let engine=world.resource::<VnEngine>();let page=engine.0.debug_site_page(offset).map_err(|error|error.to_string())?;
    let mut mailbox=engine.0.renderer.debugger.lock().map_err(|_|"Debugger queue poisoned")?;
    if !mailbox.enabled{return Err("Debugger is not attached".into());}mailbox.sites=Some(page);Ok(())
}
pub(crate) fn configure(world:&mut World,breakpoints:Vec<Breakpoint>,watches:Vec<Watch>)->Result<(),String>{
    // Validation uses the existing prepared program, including real nested
    // statement sites. No breakpoint merely names a UI/filename alias.
    let engine=world.resource::<VnEngine>();
    engine.0.validate_debug_breakpoints(&breakpoints).map_err(|e|e.to_string())?;
    if watches.len()>rvn_core::editor_debugger::MAX_WATCHES || watches.iter().any(|watch|!rvn_parser::is_variable_binding_name(&watch.name)||watch.name.len()>4096){return Err("Invalid or oversized debugger watches".into());}
    let page=engine.0.debug_site_page(0).map_err(|error|error.to_string())?;
    let mut mailbox=engine.0.renderer.debugger.lock().map_err(|_|"Debugger queue poisoned")?;
    if mailbox.error.is_some(){return Err("Restart the player before configuring a failed debugger operation".into());}
    if let Some(active)=&mut mailbox.active{
        if !engine.0.accepts_debug_session(&active.session){return Err("The debugger operation changed before configuration".into());}
        engine.0.configure_debug_session(&mut active.session,breakpoints.clone(),watches.clone()).map_err(|error|error.to_string())?;
        let snapshot=engine.0.debug_session_snapshot(&active.session);mailbox.snapshot=Some(snapshot);
    }else{mailbox.snapshot=Some(engine.0.debug_current_snapshot(&watches)?);}
    mailbox.enabled=true;mailbox.breakpoints=breakpoints;mailbox.watches=watches;mailbox.error=None;
    if mailbox.sites.is_none(){mailbox.sites=Some(page);}
    Ok(())
}
pub(crate) fn inherit_startup(engine:&VnEngine,breakpoints:Vec<Breakpoint>,watches:Vec<Watch>,mode:Option<StepKind>)->Result<(),String>{
    let snapshot=engine.0.debug_current_snapshot(&watches)?;
    let mut mailbox=engine.0.renderer.debugger.lock().map_err(|_|"Actual debugger mailbox is poisoned at handoff")?;
    if mailbox.pending(){return Err("Gameplay debugger already owns a pending operation at Startup handoff".into());}
    mailbox.enabled=true;mailbox.breakpoints=breakpoints;mailbox.watches=watches;mailbox.mode=mode;mailbox.snapshot=Some(snapshot);mailbox.error=None;mailbox.host_instant=None;Ok(())
}
pub(crate) fn run(world:&mut World,kind:StepKind)->Result<(),String>{
    let engine=world.resource::<VnEngine>();
    let mut mailbox=engine.0.renderer.debugger.lock().map_err(|_|"Debugger queue poisoned")?;
    if !mailbox.enabled{return Err("Debugger is not attached to this player".into());}
    if mailbox.error.is_some(){return Err("The failed debugger operation cannot be resumed".into());}
    if let Some(active)=&mut mailbox.active {
        if active.session.armed_step_kind().is_some_and(|armed|armed!=kind){return Err("A debugger step is still running; resume the same command".into());}
        if kind==StepKind::Out&&active.session.armed_step_kind().is_none()&&active.session.snapshot().frames.last().map_or(true,|frame|frame.depth==0){return Err("No caller frame to step out to".into());}
        if kind==StepKind::Continue{active.restore_pause=false;}
    }
    else if !mailbox.pending() && world.get_resource::<State<VnState>>().is_some_and(|state|*state.get()==VnState::Stepping){
        mailbox.admit(&engine,DebugOperation::Statement,None,0)?;mailbox.story_queued=true;
    }
    let idle=!mailbox.pending();
    if idle&&kind!=StepKind::Continue{return Err("This interaction waits for an actual player event or media report; no statement can be stepped".into());}
    mailbox.mode=Some(kind);drop(mailbox);
    if idle{crate::editor_preview::set_paused_for_debugger(world,false)?;}
    Ok(())
}
pub(crate) fn pause(world:&mut World)->Result<(),String>{
    let engine=world.resource::<VnEngine>();
    {let mut mailbox=engine.0.renderer.debugger.lock().map_err(|_|"Debugger queue poisoned")?;
        mailbox.mode=None;if let Some(active)=&mut mailbox.active{active.restore_pause=true;}}
    crate::editor_preview::set_paused(world,true)
}
pub(crate) fn resume(world:&mut World)->Result<(),String>{
    let kind={let engine=world.resource::<VnEngine>();let mailbox=engine.0.renderer.debugger.lock().map_err(|_|"Debugger queue poisoned")?;
        mailbox.active.as_ref().and_then(|active|active.session.armed_step_kind()).unwrap_or(StepKind::Continue)};
    run(world,kind)
}
/// Counts the payload retained by a receipt closure while it is still borrowed.
/// The queue later adds its owned operation, rather than counting each in isolation.
pub(crate) fn retained_payload_bytes(value:&impl std::fmt::Debug)->Result<usize,String>{
    use std::fmt::Write;let mut count=Counter(0);write!(&mut count,"{value:?}").map_err(|_|"Debugger retained callback exceeds queue budget")?;Ok(count.0)
}
pub(crate) fn cancel(world:&mut World)->Result<(),String>{
    let (restore,instance,epoch,mut discarded,abandoned)={
        let engine=world.resource::<VnEngine>();
        let mut mailbox=engine.0.renderer.debugger.lock().map_err(|_|"Debugger queue poisoned")?;
        let restore=if mailbox.error.is_some(){None}else{mailbox.active.as_ref().map(|active|active.restore_pause)};
        let mut discarded=std::mem::take(&mut mailbox.queue);
        let abandoned=if let Some(active)=mailbox.active.take(){discarded.push_front(active.queued);Some(active.session)}else{None};
        let fatal=mailbox.error.take();
        *mailbox=Mailbox::default();
        if let Some(error)=fatal{mailbox.enabled=true;mailbox.error=Some(error);}
        (restore,engine.0.instance_id(),engine.0.runtime_epoch(),discarded,abandoned)
    };
    if let Some(session)=abandoned{
        if world.resource::<VnEngine>().0.accepts_debug_session(&session){world.resource_mut::<VnEngine>().0.abandon_restored_widget_boundary(session).map_err(|error|error.to_string())?;}
    }
    // Queued media acknowledgements are dropped, leaving their real queues
    // intact. Cancelling preparation never pushes compensating history.
    for mut queued in discarded.drain(..){if let Some(abort)=queued.abort.take(){abort(world);}}
    if let Some(restore)=restore {
        let engine=world.resource::<VnEngine>();
        if engine.0.instance_id()==instance&&engine.0.runtime_epoch()==epoch{crate::editor_preview::set_paused_for_debugger(world,restore)?;}
    }
    Ok(())
}
pub(crate) fn statement(engine:&VnEngine)->Poll<Option<rvn_core::Interaction>>{
    let Ok(mut mailbox)=engine.0.renderer.debugger.lock()else{return Poll::Complete(Err("Debugger queue poisoned".into()));};
    if !mailbox.enabled{return Poll::Immediate;}
    if let Some(result)=mailbox.story.take(){return Poll::Complete(result);}
    if mailbox.error.is_some(){return Poll::Complete(Err(mailbox.error.clone().unwrap()));}
    if !mailbox.story_queued && !mailbox.pending(){
        if let Err(error)=mailbox.admit(engine,DebugOperation::Statement,None,0){return Poll::Complete(Err(error));}
        mailbox.story_queued=true;
    }
    Poll::Pending
}
pub(crate) fn enqueue(world:&mut World,operation:DebugOperation,finish:Finish)->Result<bool,String>{
    let engine=world.resource::<VnEngine>();
    let mut mailbox=engine.0.renderer.debugger.lock().map_err(|_|"Debugger queue poisoned")?;
    if !mailbox.enabled{return Ok(false);}
    if let Some(error)=&mailbox.error{return Err(error.clone());}
    mailbox.admit(engine,operation,Some(finish),0)?;Ok(true)
}
/// Retains the original frame instant until the existing Canvas Host producer
/// has accepted it. No new Host producer or alternate clock is introduced.
pub(crate) fn retain_host_instant(world:&World,seconds:f64){
    if world.get_resource::<crate::custom_canvas::DebuggerHostProducer>().is_none(){return;}
    let engine=world.resource::<VnEngine>();
    if let Ok(mut mailbox)=engine.0.renderer.debugger.lock(){
        mailbox.host_instant=Some(HostInstant{instance:engine.0.instance_id(),epoch:engine.0.runtime_epoch(),seconds:seconds.min(0.25)});
    }
}
pub(crate) fn host_seconds(engine:&VnEngine)->Option<f64>{
    let mailbox=engine.0.renderer.debugger.lock().ok()?;
    mailbox.host_instant.as_ref().filter(|instant|instant.instance==engine.0.instance_id()&&instant.epoch==engine.0.runtime_epoch()).map(|instant|instant.seconds)
}
pub(crate) fn clear_host_instant(world:&World){
    if let Ok(mut mailbox)=world.resource::<VnEngine>().0.renderer.debugger.lock(){mailbox.host_instant=None;}
}
pub(crate) fn enqueue_frame(world:&mut World,operation:DebugOperation,before:rvn_core::GameState,retained:usize,finish:Finish)->Result<bool,String>{
    let engine=world.resource::<VnEngine>();
    if !enabled(engine){return Ok(false);}
    let source=engine.0.debug_state_source().map_err(|error|error.to_string())?;
    let abort:Abort=Box::new(move|world:&mut World|{
        let mut engine=world.resource_mut::<VnEngine>();
        // Never overwrite a Load/rollback or any newly committed source.
        if engine.0.accepts_debug_state_source(&source){engine.0.state=before;}
    });
    let result=(||{let mut mailbox=engine.0.renderer.debugger.lock().map_err(|_|"Debugger queue poisoned")?;
        if let Some(error)=&mailbox.error{return Err(error.clone());}
        mailbox.admit(engine,operation,Some(finish),retained)?;
        mailbox.queue.back_mut().ok_or("Debugger frame queue missing")?.abort=None;Ok(())})();
    if let Err(error)=result{abort(world);return Err(error);}
    {let engine=world.resource::<VnEngine>();let mut mailbox=engine.0.renderer.debugger.lock().map_err(|_|"Debugger queue poisoned")?;
        mailbox.queue.back_mut().ok_or("Debugger frame queue missing")?.abort=Some(abort);}
    // Ordinary frames without an evaluator suspension complete at their real
    // producer boundary, retaining the same delta for the later Host phase.
    drain(world);Ok(true)
}
pub(crate) fn player(engine:&VnEngine,operation:DebugOperation,choice:bool)->Result<bool,String>{
    let mut mailbox=engine.0.renderer.debugger.lock().map_err(|_|"Debugger queue poisoned")?;
    if !mailbox.enabled{return Ok(false);}
    if let Some(error)=&mailbox.error{return Err(error.clone());}
    mailbox.admit(engine,operation,Some(Box::new(move|world,outcome|{
        if !matches!(outcome,DebugOutcome::Interaction(None)){return Err("Wrong debugger player input outcome".into());}
        if choice{
            if let Some(mut state)=world.get_resource_mut::<crate::resources::VnRenderState>(){state.choice_options.clear();}
            if let Some(mut state)=world.get_resource_mut::<crate::resources::ImagemapState>(){state.clear();}
            if let Some(mut state)=world.get_resource_mut::<crate::resources::ChoiceFocus>(){state.clear();}
        }
        world.resource_mut::<NextState<VnState>>().set(VnState::Stepping);Ok(())
    })),0)?;Ok(true)
}
pub(crate) fn input(engine:&VnEngine,input:rvn_core::ui_input::ScopedUiInput)->Result<bool,String>{
    let mut mailbox=engine.0.renderer.debugger.lock().map_err(|_|"Debugger queue poisoned")?;
    if !mailbox.enabled{return Ok(false);}
    if let Some(error)=&mailbox.error{return Err(error.clone());}
    // Source before payload. A consumed physical event keeps this source for
    // every deferred handler; neither drain nor Continue scopes it again.
    if !engine.0.ui_input_source_current(&input.source){return Err("Captured debugger UI source changed".into());}
    mailbox.admit(engine,DebugOperation::Input(input),Some(Box::new(|world,outcome|{
        let DebugOutcome::Input(admitted)=outcome else{return Err("Wrong debugger UI outcome".into());};
        if !admitted{return Ok(());}
        crate::scene_publication::consume(world);
        let views=world.resource::<VnEngine>().0.interface_views().map_err(|e|e.to_string())?;
        if let Some(mut screens)=world.get_resource_mut::<crate::programmable_ui::Screens>(){crate::programmable_ui::update_views(&mut screens,views);}
        for command in world.resource_mut::<VnEngine>().0.renderer.take_pending(){world.send_event(command);}
        Ok(())
    })),0)?;Ok(true)
}
fn warning(world:&mut World,error:String){
    bevy::log::warn!("Debugger callback refused: {error}");
    if let Some(mut diagnostic)=world.get_resource_mut::<crate::resources::ScriptErrorMessage>(){diagnostic.0=error;}
}
fn failure(world:&mut World,error:String){
    if let Some(engine)=world.get_resource::<VnEngine>(){
        if let Ok(mut mailbox)=engine.0.renderer.debugger.try_lock(){
            mailbox.error=Some(error.clone());mailbox.mode=None;
        }
    }
    if let Some(mut diagnostic)=world.get_resource_mut::<crate::resources::ScriptErrorMessage>(){diagnostic.0=error;}
    if let Some(mut next)=world.get_resource_mut::<NextState<VnState>>(){next.set(VnState::Error);}
}
/// Exclusive First drain: pause audio/time before creating a candidate, then
/// keep the same owned evaluator and cumulative budget between real frames.
pub(crate) fn drain(world:&mut World){
    if world.contains_resource::<crate::debugger_startup::DebugBootstrap>(){
        if world.resource::<crate::debugger_startup::DebugBootstrap>().profile==crate::debugger_startup::BootstrapProfile::Debug{crate::debugger_startup::driver::drain(world);}
        return;
    }
    let Some(engine)=world.get_resource::<VnEngine>()else{return;};
    let shared=engine.0.renderer.debugger.clone();
    let identity=(engine.0.instance_id(),engine.0.runtime_epoch());
    let (mut active,mode,breakpoints,watches)={
        let Ok(mut mailbox)=shared.lock()else{failure(world,"Debugger queue poisoned".into());return;};
        if !mailbox.enabled||mailbox.error.is_some(){return;}
        (mailbox.active.take(),mailbox.mode,mailbox.breakpoints.clone(),mailbox.watches.clone())
    };
    // Restore/rollback revokes old requests normally. It is not a runtime error
    // and never executes or acknowledges an old receipt against the new visit.
    if active.as_ref().is_some_and(|active|(active.queued.instance,active.queued.epoch)!=identity){
        let stale=active.take().unwrap();let restore=stale.restore_pause;
        if let Ok(mut mailbox)=shared.lock(){mailbox.bytes=mailbox.bytes.saturating_sub(stale.queued.cost);mailbox.story_queued=false;mailbox.story=None;mailbox.snapshot=None;mailbox.mode=None;mailbox.host_instant=None;}
        if let Err(error)=crate::editor_preview::set_paused_for_debugger(world,restore){failure(world,error);}return;
    }
    if active.is_none(){
        let queued={let Ok(mut mailbox)=shared.lock()else{failure(world,"Debugger queue poisoned".into());return;};mailbox.queue.pop_front()};
        let Some(mut queued)=queued else{return;};
        if (queued.instance,queued.epoch)!=identity{
            if let Ok(mut mailbox)=shared.lock(){mailbox.bytes=mailbox.bytes.saturating_sub(queued.cost);mailbox.story_queued=false;mailbox.story=None;mailbox.snapshot=None;mailbox.mode=None;mailbox.host_instant=None;}
            return;
        }
        let restore_pause=crate::editor_preview::paused(world);
        if let Err(error)=crate::editor_preview::set_paused_for_debugger(world,true){
            if let Some(abort)=queued.abort.take(){abort(world);}failure(world,error);return;
        }
        let mut soft=queued.soft_failure;
        let result=world.resource::<VnEngine>().0.debug_begin(queued.operation.clone(),breakpoints).map_err(|e|{soft|=e.is_menu_request_rejection();e.to_string()}).and_then(|mut session|{
            session.set_watches(watches)?;Ok(session)
        });
        match result {
            Ok(session)=>active=Some(Active{queued,session,restore_pause,needs_pause:false}),
            Err(error)=>{
                if let Some(abort)=queued.abort.take(){abort(world);}
                if let Ok(mut mailbox)=shared.lock(){mailbox.bytes=mailbox.bytes.saturating_sub(queued.cost);mailbox.error=Some(error.clone());
                    if mailbox.story_queued{mailbox.story=Some(Err(error.clone()));mailbox.story_queued=false;}}
                if soft{warning(world,error);if let Ok(mut mailbox)=shared.lock(){mailbox.error=None;mailbox.mode=None;}if let Err(error)=crate::editor_preview::set_paused_for_debugger(world,restore_pause){failure(world,error);}}else{failure(world,error);}return;
            }
        }
    }
    let Some(mut active)=active else{return;};
    if active.needs_pause{
        if let Err(error)=crate::editor_preview::set_paused_for_debugger(world,true){
            if let Ok(mut mailbox)=shared.lock(){mailbox.active=Some(active);}failure(world,error);return;
        }
        active.needs_pause=false;
    }
    if mode==Some(StepKind::Continue){active.restore_pause=false;}
    let command=mode.or_else(||(!active.restore_pause).then_some(StepKind::Continue));
    let mut soft=active.queued.soft_failure;
    let result=if let Some(command)=command {
        world.resource_mut::<VnEngine>().0.debug_step(&mut active.session,command).map(Some).map_err(|e|{soft|=e.is_menu_request_rejection();e.to_string()})
    }else{Ok(None)};
    let observation=world.resource::<VnEngine>().0.debug_session_snapshot(&active.session);
    match result {
        Ok(Some((DebugProgress::Complete,Some(outcome))))=>{
            let restore=active.restore_pause;
            let finish=active.queued.finish.take();
            {let Ok(mut mailbox)=shared.lock()else{failure(world,"Debugger queue poisoned".into());return;};
                mailbox.bytes=mailbox.bytes.saturating_sub(active.queued.cost);mailbox.snapshot=Some(observation);
                if command!=Some(StepKind::Continue){mailbox.mode=None;}
            }
            if let Some(finish)=finish{
                if let Err(error)=finish(world,outcome){failure(world,error);return;}
            }else{
                let result=match outcome{DebugOutcome::Interaction(interaction)=>Ok(interaction),_=>Err("Wrong debugger Story outcome".into())};
                if let Ok(mut mailbox)=shared.lock(){mailbox.story=Some(result);mailbox.story_queued=false;}
            }
            // Continue restores the user's prior pause state. An explicit
            // statement step remains paused after its one real publication.
            if command==Some(StepKind::Continue){if let Err(error)=crate::editor_preview::set_paused_for_debugger(world,restore){failure(world,error);}}
        }
        Ok(progress)=>{
            let Ok(mut mailbox)=shared.lock()else{failure(world,"Debugger queue poisoned".into());return;};
            mailbox.snapshot=Some(observation);
            if matches!(progress,Some((DebugProgress::Stopped,_))){mailbox.mode=None;active.restore_pause=true;}
            mailbox.active=Some(active);
        }
        Err(error)=>{
            if let Some(abort)=active.queued.abort.take(){abort(world);}
            if let Ok(mut mailbox)=shared.lock(){mailbox.snapshot=Some(observation);mailbox.bytes=mailbox.bytes.saturating_sub(active.queued.cost);mailbox.error=Some(error.clone());
                if mailbox.story_queued{mailbox.story=Some(Err(error.clone()));mailbox.story_queued=false;}}
            if soft{warning(world,error);if let Ok(mut mailbox)=shared.lock(){mailbox.error=None;mailbox.mode=None;}if let Err(error)=crate::editor_preview::set_paused_for_debugger(world,active.restore_pause){failure(world,error);}}else{failure(world,error);}
        }
    }
}

#[cfg(test)]
#[path="editor_debugger_tests.rs"]
mod tests;
