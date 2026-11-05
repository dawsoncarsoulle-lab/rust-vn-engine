//! Native opt-in image transport for the existing player App and renderer.
use bevy::prelude::*;
use rvn_core::editor_preview::{self as wire, ControlKind, Header, Packet};
use std::sync::atomic::{AtomicBool, Ordering};
#[path="editor_preview_session.rs"]
mod sessions;
#[path="editor_preview_startup.rs"]
mod startup;
#[path="editor_preview_consumption.rs"]
mod consumption;
use consumption::{PendingConsumed,drain_consumed};
pub(crate) use startup::finish_handoff as finish_startup_handoff;
use std::{
    net::{SocketAddr, TcpStream},
    sync::{mpsc, Arc, Condvar, Mutex},
    time::Duration,
};


// Diagnostic only: nine one-shot boundaries (0-4 render,5-8 transport), disabled unless Root opts in.
pub(crate) fn trace_once(stage:u32,message:std::fmt::Arguments<'_>){
    use std::sync::{OnceLock,atomic::{AtomicU32,Ordering}};
    static ENABLED:OnceLock<bool>=OnceLock::new();
    static SEEN:AtomicU32=AtomicU32::new(0);
    if !*ENABLED.get_or_init(||std::env::var_os("RVN_PREVIEW_TRACE_0925").is_some_and(|value|value=="1")){return;}
    let Some(bit)=1u32.checked_shl(stage).filter(|_|stage<9)else{return;};
    if SEEN.fetch_or(bit,Ordering::Relaxed)&bit==0{eprintln!("[editor-preview-0925] stage={stage} {message}");}
}
fn trace_capture(world:&World){
    if let Some(scope)=world.get_resource::<RenderScope>(){
        let scope=&scope.0;
        trace_once(2,format_args!("Last capture instance={} epoch={} sequence={} paused={} pending={}",scope.instance,scope.epoch,scope.sequence,scope.paused,scope.publication_pending));
    }else{trace_once(2,format_args!("Last capture returned without RenderScope"));}
}

fn connect(options:&Options,instance:u64)->Result<(Output,mpsc::Receiver<Header>),String>{
    let mut stream = TcpStream::connect_timeout(
        &SocketAddr::from(([127, 0, 0, 1], options.port)),
        Duration::from_secs(2),
    )
    .map_err(|e| e.to_string())?;
    stream.set_nodelay(true).map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .map_err(|e| e.to_string())?;
    Packet::header(Header::Hello {
        version: wire::WIRE_VERSION,
        nonce: options.nonce,
        pid: std::process::id(),
        instance,
    })
    .write(&mut stream)?;
    let mut reader = stream.try_clone().map_err(|e| e.to_string())?;
    let (tx, rx) = mpsc::sync_channel(16);
    let output = Output {
        phase:0,
        queue: Arc::new((Mutex::new(Latest::default()), Condvar::new())),
        alive: Arc::new(AtomicBool::new(true)),
        contexts:Arc::new(Mutex::new(Default::default())),
        held:Arc::new(Mutex::new(Default::default())),
    };
    let alive = output.alive.clone();
    std::thread::spawn(move || {
        while alive.load(Ordering::Acquire) {
            match Packet::read(&mut reader) {
                Ok(Packet {
                    header: header @ (Header::Control { .. }|Header::Input { .. }|Header::Release { .. }|Header::Debugger{..}|Header::SessionGrant{..}|Header::FrameConsumed{..}),
                    ..
                }) => {
                    if tx.try_send(header).is_err() {
                        break;
                    }
                }
                _ => break,
            }
        }
        alive.store(false, Ordering::Release);
    });
    let writer = output.clone();
    std::thread::spawn(move || {
        while writer.alive.load(Ordering::Acquire) {
            let packet = {
                let Ok(queue) = writer.queue.0.lock() else {
                    break;
                };
                let Ok((mut queue, _)) =
                    writer
                        .queue
                        .1
                        .wait_timeout_while(queue, Duration::from_millis(30), |q| {
                            !q.can_write()
                        })
                else {
                    break;
                };
                writer.take_for_write(&mut queue)
            };
            if let Some((packet, token)) = packet {
                let result=packet.write(&mut stream);
                writer.finish_write(token, result.is_ok());
                if let Header::Frame{scope,..}=&packet.header {
                    trace_once(7,format_args!("Actual writer Packet.write returned success={} instance={} epoch={} sequence={}",result.is_ok(),scope.instance,scope.epoch,scope.sequence));
                }
                if result.is_err() {
                    break;
                }
            }
        }
        writer.failed();
        let _ = stream.shutdown(std::net::Shutdown::Both);
    });
    Ok((output,rx))
}
#[derive(Clone, Resource, Debug)]
pub struct Options {
    pub port: u16,
    pub nonce: u64,
    pub width: u32,
    pub height: u32,
}
impl Options {
    pub fn parse(value: &str) -> Result<Self, String> {
        let values = value.split(',').collect::<Vec<_>>();
        if values.len() != 2 {
            return Err("Expected --editor-preview=port,nonce".into());
        }
        let result = Self {
            port: values[0].parse().map_err(|_| "Invalid preview port")?,
            nonce: values[1].parse().map_err(|_| "Invalid preview nonce")?,
            width: 0,
            height: 0,
        };
        if result.port == 0 || result.nonce == 0 {
            return Err("Invalid editor connection".into());
        }
        Ok(result)
    }
}
#[derive(Resource, Default)]
pub(crate) struct PreviewState {
    pub(crate) paused: bool,
    user_paused:bool,
    pub(crate) step_requested: bool,
    sequence: u64,
    received_sequence: u64,
    control_error: Option<String>,
    frame_sequence: u64,
    time_was_paused: bool,
    legacy_playing: Vec<Entity>,
}
pub(crate) fn paused(world: &World) -> bool {
    world
        .get_resource::<PreviewState>()
        .is_some_and(|state| state.paused)
}
// A GPU completion is retained with the exact packet that can expose it.
// Only the writer's current lease and successful writes admit controls.
#[derive(Clone)]
struct CompletedImage {
    scope: wire::FrameScope,
    startup: Option<Arc<rvn_core::engine::StartupObservationSource>>,
    context: Option<Arc<crate::editor_preview_input::PhysicalFrame>>,
}
#[derive(Clone, Copy, PartialEq, Eq)]
struct WriteToken { phase: u64, instance: u64, epoch: u64, sequence: u64 }
impl WriteToken {
    fn of(phase:u64, scope:&wire::FrameScope)->Self {
        Self{phase,instance:scope.instance,epoch:scope.epoch,sequence:scope.sequence}
    }
}
#[derive(Default)]
struct Latest {
    frame: Option<Packet>,
    queued_completion: Option<CompletedImage>,
    writing: Option<(WriteToken, CompletedImage)>,
    writing_consumed: bool,
    main_consumed: Option<CompletedImage>,
    awaiting_consumption: Option<CompletedImage>,
    consumption_floor: u64,
    failure_reason: Option<String>,
    owner: Option<(u64,u64)>,
    sequence: u64,
    session_retired:bool,
    phase:u64,
    completed:std::collections::VecDeque<wire::FrameScope>,
    startup_sources:std::collections::VecDeque<(u64,u64,u64,Arc<rvn_core::engine::StartupObservationSource>)>,
    retired:std::collections::VecDeque<(u64,u64,u64)>,
}
#[derive(Clone, Resource)]
pub(crate) struct Output {
    phase:u64,
    queue: Arc<(Mutex<Latest>, Condvar)>,
    alive: Arc<AtomicBool>,
    contexts: Arc<Mutex<std::collections::VecDeque<Arc<crate::editor_preview_input::PhysicalFrame>>>>,
    held:Arc<Mutex<std::collections::HashMap<String,Arc<crate::editor_preview_input::PhysicalFrame>>>>,
}
impl Output {
    #[cfg(test)]
    fn publish_and_consume_cpu_fixture(&self){
        let pending={let mut queue=self.queue.0.lock().unwrap();self.take_for_write(&mut queue)};
        if let Some((packet,token))=pending{
            let result=packet.write(&mut Vec::new());self.finish_write(token,result.is_ok());result.unwrap();
            // Explicit CPU consumer fixture, never a GPU/native observation.
            if let Header::Frame{scope,..}=&packet.header{self.consume_frame(scope.instance,scope.epoch,scope.sequence).unwrap();}
        }
    }
    pub(crate) fn frame(&self, packet: Packet, context:Option<Arc<crate::editor_preview_input::PhysicalFrame>>) {
        self.frame_with_startup(packet,context,None)
    }
    pub(crate) fn frame_with_startup(&self, packet: Packet, context:Option<Arc<crate::editor_preview_input::PhysicalFrame>>,startup:Option<Arc<rvn_core::engine::StartupObservationSource>>) {
        let Header::Frame { scope, .. } = &packet.header else {
            return;
        };
        trace_once(5,format_args!("Output.frame_with_startup entered instance={} epoch={} sequence={} output_phase={}",scope.instance,scope.epoch,scope.sequence,self.phase));
        match self.queue.0.lock() {
            Ok(mut queue) => {
                if queue.failure_reason.is_some()||queue.session_retired||self.phase!=queue.phase||scope.sequence <= queue.sequence||queue.owner.is_some_and(|owner|owner!=(scope.instance,scope.epoch)) {
                    return;
                }
                // Preflight the real scope before changing queue authority.
                let mut cost=crate::editor_preview_input::PreviewCost::new(64*1024);
                if cost.serialized(scope).is_none(){self.failed();return;}
                if context.as_ref().is_some_and(|frame| frame.scope != *scope){self.failed();return;}
                let Ok(mut contexts)=self.contexts.lock() else {self.failed();return;};
                let Ok(held)=self.held.lock()else{self.failed();return;};
                let writing=queue.writing.as_ref().and_then(|(_,image)|image.context.as_ref());
                if Self::context_cost_all(&Default::default(),&held,context.as_ref(),writing,queue.main_consumed.as_ref().and_then(|v|v.context.as_ref()),queue.awaiting_consumption.as_ref().and_then(|v|v.context.as_ref()))>16*1024*1024{
                    // Pinned geometry cannot be evicted or waited on by World.
                    // Abort this preview explicitly instead of freezing Main
                    // forever on an image it cannot advance from.
                    queue.failure_reason=Some("Preview pinned image geometry exceeds 16 MiB; restart the preview after releasing held inputs or reducing UI geometry".into());
                    queue.frame=None;queue.queued_completion=None;
                    self.alive.store(false,Ordering::Release);self.queue.1.notify_one();return;
                }
                // Latest queued pixels may be dropped, but never evict an
                // actual publication just because GPU readback runs faster.
                while Self::context_cost_all(&contexts,&held,context.as_ref(),writing,queue.main_consumed.as_ref().and_then(|v|v.context.as_ref()),queue.awaiting_consumption.as_ref().and_then(|v|v.context.as_ref()))>16*1024*1024 {
                    if contexts.pop_front().is_none(){return;}
                }
                queue.queued_completion=Some(CompletedImage{scope:scope.clone(),startup,context});
                let admitted=(scope.instance,scope.epoch,scope.sequence);
                queue.owner.get_or_insert((scope.instance,scope.epoch));
                queue.sequence = scope.sequence;
                queue.frame = Some(packet);
                self.queue.1.notify_one();
                trace_once(6,format_args!("Actual Output queue admission and notify completed instance={} epoch={} sequence={}",admitted.0,admitted.1,admitted.2));
            }
            Err(_) => self.failed(),
        }
    }
    fn take_for_write(&self,queue:&mut Latest)->Option<(Packet,Option<WriteToken>)>{
        // There is one socket writer. A second take cannot replace its pin.
        if !queue.can_write(){return None;}
        let packet=queue.frame.take()?;
        let token=if let Header::Frame{scope,..}=&packet.header {
            let image=queue.queued_completion.take()?;
            if queue.session_retired||image.scope!=*scope{return None;}
            let token=WriteToken::of(queue.phase,scope);
            queue.writing_consumed=false;queue.writing=Some((token,image));Some(token)
        }else{queue.queued_completion=None;None};
        Some((packet,token))
    }
    fn finish_write(&self,token:Option<WriteToken>,success:bool){
        let Some(token)=token else{return;};
        let Ok(mut queue)=self.queue.0.lock()else{self.failed();return;};
        if queue.writing.as_ref().is_none_or(|(actual,_)|*actual!=token){return;}
        let (_,image)=queue.writing.take().expect("the exact writer lease was checked");
        let consumed=std::mem::take(&mut queue.writing_consumed);
        // The connection writer spans startup/gameplay phases; its Output
        // clone's extraction phase is not publication authority. The token is.
        if !success||queue.session_retired||token.phase!=queue.phase{return;}
        let Ok(mut contexts)=self.contexts.lock()else{self.failed();return;};
        let Ok(held)=self.held.lock()else{self.failed();return;};
        if consumed{queue.consumption_floor=image.scope.sequence;queue.main_consumed=Some(image.clone());}
        else{queue.awaiting_consumption=Some(image.clone());}
        queue.completed.push_back(image.scope.clone());
        while queue.completed.len()>8{queue.completed.pop_front();}
        if let Some(source)=image.startup{queue.startup_sources.push_back((token.instance,token.epoch,token.sequence,source));}
        // Keep metadata on precisely the same successful publication frontier.
        let keep=queue.completed.iter().map(|scope|(scope.instance,scope.epoch,scope.sequence)).collect::<std::collections::HashSet<_>>();
        queue.startup_sources.retain(|(i,e,s,_)|keep.contains(&(*i,*e,*s)));
        if let Some(context)=image.context{contexts.push_back(context);}
        contexts.retain(|frame|keep.contains(&(frame.scope.instance,frame.scope.epoch,frame.scope.sequence)));
        while contexts.len()>8||Self::context_cost_all(&contexts,&held,queue.queued_completion.as_ref().and_then(|image|image.context.as_ref()),None,queue.main_consumed.as_ref().and_then(|v|v.context.as_ref()),queue.awaiting_consumption.as_ref().and_then(|v|v.context.as_ref()))>16*1024*1024{
            if contexts.pop_front().is_none(){break;}
        }
        if consumed{self.queue.1.notify_one();}
    }
    fn completed(&self,instance:u64,epoch:u64,sequence:u64)->Option<wire::FrameScope>{
        let queue=self.queue.0.lock().ok()?;
        if queue.session_retired||self.phase!=queue.phase{return None;}
        queue.main_consumed.as_ref().map(|image|&image.scope).into_iter().chain(queue.writing.as_ref().map(|(_,image)|&image.scope)).chain(queue.completed.iter())
            .find(|scope|scope.instance==instance&&scope.epoch==epoch&&scope.sequence==sequence).cloned()
    }
    fn completed_startup_source(&self,instance:u64,epoch:u64,sequence:u64)->Option<Arc<rvn_core::engine::StartupObservationSource>>{
        self.completed_startup(instance,epoch,sequence)?.1
    }
    fn completed_startup(&self,instance:u64,epoch:u64,sequence:u64)->Option<(wire::FrameScope,Option<Arc<rvn_core::engine::StartupObservationSource>>)>{
        let queue=self.queue.0.lock().ok()?;
        if queue.session_retired||self.phase!=queue.phase{return None;}
        for image in queue.main_consumed.iter().chain(queue.awaiting_consumption.iter()).chain(queue.writing.as_ref().map(|(_,image)|image)){
            if (image.scope.instance,image.scope.epoch,image.scope.sequence)==(instance,epoch,sequence){return Some((image.scope.clone(),image.startup.clone()));}
        }
        let scope=queue.completed.iter().find(|scope|(scope.instance,scope.epoch,scope.sequence)==(instance,epoch,sequence))?.clone();
        let source=queue.startup_sources.iter().find(|(owner,generation,frame,_)|*owner==instance&&*generation==epoch&&*frame==sequence).map(|(_,_,_,source)|source.clone());
        Some((scope,source))
    }
    fn context(&self,instance:u64,epoch:u64,sequence:u64)->Option<Arc<crate::editor_preview_input::PhysicalFrame>> {
        let queue=self.queue.0.lock().ok()?;
        if queue.session_retired||self.phase!=queue.phase{return None;}
        let recent=self.contexts.lock().ok()?;
        let held=self.held.lock().ok()?;
        queue.main_consumed.as_ref().and_then(|image|image.context.as_ref()).into_iter().chain(queue.awaiting_consumption.as_ref().and_then(|image|image.context.as_ref())).chain(queue.writing.as_ref().and_then(|(_,image)|image.context.as_ref())).chain(recent.iter()).chain(held.values())
            .find(|frame|frame.scope.instance==instance&&frame.scope.epoch==epoch&&frame.scope.sequence==sequence).cloned()
    }
    fn context_cost_all(recent:&std::collections::VecDeque<Arc<crate::editor_preview_input::PhysicalFrame>>,held:&std::collections::HashMap<String,Arc<crate::editor_preview_input::PhysicalFrame>>,queued:Option<&Arc<crate::editor_preview_input::PhysicalFrame>>,writing:Option<&Arc<crate::editor_preview_input::PhysicalFrame>>,main:Option<&Arc<crate::editor_preview_input::PhysicalFrame>>,awaiting:Option<&Arc<crate::editor_preview_input::PhysicalFrame>>)->usize {
        let mut seen=std::collections::HashSet::new();
        // Deduplicate the same allocation, never contradictory captures that
        // happen to claim the same scope. Overflow is a failed budget.
        recent.iter().chain(held.values()).chain(queued).chain(writing).chain(main).chain(awaiting).filter(|frame|seen.insert(Arc::as_ptr(frame) as usize))
            .try_fold(0usize,|sum,frame|sum.checked_add(frame.cost)).unwrap_or(usize::MAX)
    }
    fn hold(&self,frame:Arc<crate::editor_preview_input::PhysicalFrame>,input:&wire::ImageInput)->Result<bool,String>{
        let hold=match input {
            wire::ImageInput::Key{key,pressed,..}=>Some((format!("key:{key}"),*pressed)),
            wire::ImageInput::Pointer{kind,button,..}if matches!(kind,wire::PointerKind::Down|wire::PointerKind::Up)=>Some((format!("pointer:{button}"),*kind==wire::PointerKind::Down)),
            _=>None,
        };
        let queue=self.queue.0.lock().map_err(|_|"Image queue poisoned")?;
        let recent=self.contexts.lock().map_err(|_|"Image context cache poisoned")?;
        let mut held=self.held.lock().map_err(|_|"Image gesture cache poisoned")?;
        if let Some((key,pressed))=hold {
            if !pressed{held.remove(&key);return Ok(false);}
            if held.contains_key(&key){return Ok(false);}
            if held.len()>=64{return Err("Image held-input budget".into());}
            held.insert(key.clone(),frame);
            let count=held.values().map(|frame|frame.scope.sequence).collect::<std::collections::HashSet<_>>().len();
            if count>8 || Self::context_cost_all(&recent,&held,queue.queued_completion.as_ref().and_then(|image|image.context.as_ref()),queue.writing.as_ref().and_then(|(_,image)|image.context.as_ref()),queue.main_consumed.as_ref().and_then(|v|v.context.as_ref()),queue.awaiting_consumption.as_ref().and_then(|v|v.context.as_ref()))>16*1024*1024{held.remove(&key);return Err("Image held-geometry budget".into());}
            drop(recent);return Ok(true);
        }else if matches!(input,wire::ImageInput::FocusLost){held.clear();}
        drop(recent);Ok(false)
    }
    fn release_held(&self){if let Ok(mut held)=self.held.lock(){held.clear();}}
// Private independent replacement of Output::discard_foreign, not installed.
    pub(crate) fn discard_foreign(&self, instance: u64, epoch: u64) {
        // Use the same lock order as Output::frame: queue, contexts, held.
        let (Ok(mut queue), Ok(mut frames), Ok(mut held)) =
            (self.queue.0.lock(), self.contexts.lock(), self.held.lock()) else { self.failed(); return; };
        queue.retired.retain(|(owner, old_epoch, _)| *owner == instance && *old_epoch < epoch);
        let identities=frames.iter().chain(held.values())
            .chain(queue.queued_completion.as_ref().and_then(|image|image.context.as_ref()))
            .chain(queue.writing.as_ref().and_then(|(_,image)|image.context.as_ref()))
            .chain(queue.main_consumed.as_ref().and_then(|image|image.context.as_ref()))
            .chain(queue.awaiting_consumption.as_ref().and_then(|image|image.context.as_ref()))
            .map(|frame|(frame.scope.instance,frame.scope.epoch,frame.scope.sequence)).collect::<Vec<_>>();
        for id in identities {
            if id.0 == instance && id.1 < epoch && !queue.retired.contains(&id) {
                queue.retired.push_back(id);
                // Keep the existing sixteen identity-only bound. Main and its
                // successor are appended after the recent frontier, so active
                // release sources survive before expendable recent entries.
                while queue.retired.len() > 16 { queue.retired.pop_front(); }
            }
        }
        queue.owner=Some((instance,epoch));
        if queue.main_consumed.as_ref().is_some_and(|v|(v.scope.instance,v.scope.epoch)!=(instance,epoch)){queue.main_consumed=None;}
        if queue.awaiting_consumption.as_ref().is_some_and(|v|(v.scope.instance,v.scope.epoch)!=(instance,epoch)){queue.awaiting_consumption=None;self.queue.1.notify_one();}
        if queue.queued_completion.as_ref().is_some_and(|image|(image.scope.instance,image.scope.epoch)!=(instance,epoch)){
            queue.queued_completion=None;queue.frame=None;
        }
        if queue.writing.as_ref().is_some_and(|(_,image)|(image.scope.instance,image.scope.epoch)!=(instance,epoch)){queue.writing=None;queue.writing_consumed=false;}
        queue.completed.retain(|scope|(scope.instance,scope.epoch)==(instance,epoch));
        queue.startup_sources.retain(|(i,e,_,_)|(*i,*e)==(instance,epoch));
        frames.retain(|frame| frame.scope.instance == instance && frame.scope.epoch == epoch);
        held.retain(|_, frame| frame.scope.instance == instance && frame.scope.epoch == epoch);
    }
    fn retired(&self, instance: u64, epoch: u64, sequence: u64) -> bool {
        self.queue.0.lock().is_ok_and(|queue| !queue.session_retired && queue.retired.contains(&(instance, epoch, sequence)))
    }

    pub(crate) fn retire_startup_phase(&mut self)->Result<(),String>{
        // Same lock order as frame(). Existing extracted Output clones retain
        // the old phase and can no longer publish even a pending/no-source image.
        let(mut queue,mut contexts,mut held)=(self.queue.0.lock().map_err(|_|"Image queue poisoned")?,self.contexts.lock().map_err(|_|"Image contexts poisoned")?,self.held.lock().map_err(|_|"Image inputs poisoned")?);
        let next=queue.phase.checked_add(1).ok_or("Image phase exhausted")?;
        queue.consumption_floor=queue.sequence;queue.main_consumed=None;queue.awaiting_consumption=None;queue.writing_consumed=false;
        queue.phase=next;self.phase=next;queue.frame=None;queue.queued_completion=None;queue.writing=None;queue.completed.clear();queue.startup_sources.clear();contexts.clear();held.clear();
        Ok(())
    }
    pub(crate) fn failed(&self) {
        self.alive.store(false, Ordering::Release);
    }
}
#[derive(Resource)]
struct Controls(Mutex<mpsc::Receiver<Header>>);
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn install(app: &mut App, options: Options, instance: u64) -> Result<(), String> {
    if crate::text_runtime_owner::current(app.world())?.instance!=instance{return Err("Preview owner changed before authentication".into());}
    if app.world().contains_resource::<crate::resources::VnEngine>(){
    let sidecar=app.world().resource::<crate::project_paths::ProjectPaths>().root.join(rvn_core::debugger_provenance::SIDECAR);
    match std::fs::symlink_metadata(&sidecar){
        Ok(metadata)=>{
            if !metadata.is_file()||metadata.file_type().is_symlink()||metadata.len()>rvn_core::debugger_provenance::MAX_BYTES as u64{return Err("Invalid bounded compiler debugger sidecar".into());}
            use std::io::Read;
            let mut bytes=Vec::new();std::fs::File::open(&sidecar).map_err(|e|e.to_string())?
                .take((rvn_core::debugger_provenance::MAX_BYTES+1)as u64).read_to_end(&mut bytes).map_err(|e|e.to_string())?;
            if bytes.len()>rvn_core::debugger_provenance::MAX_BYTES{return Err("Compiler debugger sidecar grew beyond its bound".into());}
            let overlay=rvn_core::debugger_provenance::Overlay::decode(&bytes)?;
            app.world_mut().resource_mut::<crate::resources::VnEngine>().0.install_debug_source_map(overlay,options.nonce)?;
        },
        Err(error)if error.kind()==std::io::ErrorKind::NotFound=>{},Err(error)=>return Err(error.to_string()),
    }
    }
    let (output,rx)=connect(&options,instance)?;
    app.insert_resource(options)
        .insert_resource(output)
        .insert_resource(Controls(Mutex::new(rx)))
        .init_resource::<PreviewState>()
        .add_systems(First,(crate::editor_debugger::drive_widget_materialization,crate::editor_debugger::drain).chain().after(control).before(crate::scene_publication::consume).before(bevy::time::TimeSystem))
        .init_resource::<PendingInput>()
        .init_resource::<PendingConsumed>()
        .init_resource::<crate::editor_preview_input::Receipts>()
        .add_systems(PreUpdate,(drain_input,drain_consumed).chain().before(bevy::input::InputSystem))
        .add_systems(PreUpdate,crate::editor_preview_input::project_confirmation
            .after(bevy::ui::UiSystem::Focus))
        .add_systems(Update,crate::editor_preview_input::stamp
            .after(crate::systems::input_system)
            .after(crate::systems::input::finished_quick_load_system)
            .after(crate::systems::choice_interaction_system)
            .after(crate::systems::menu_input_system)
            .after(crate::systems::history_input_system)
            .before(crate::systems::player_input_system))
        .add_systems(
            First,
            control
                .before(crate::scene_publication::consume)
                .before(bevy::time::TimeSystem),
        )
        // AudioPlaySet is private in Bevy0.14.2. Last follows its entire
        // actual PostUpdate playback phase and any deferred sink creation.
        .add_systems(Last, (enforce_audio_pause, capture_scope).chain())
        .add_plugins(crate::editor_preview_image::ImagePlugin);
    let nonce=app.world().resource::<Options>().nonce;
    sessions::attach(app.world_mut(),instance,nonce);
    Ok(())
}
#[derive(Resource,Default)]
struct PendingInput(Vec<Header>);
// Private independent replacement of admission/drain; not installed.
enum ImageAdmission {
    Current(Arc<crate::editor_preview_input::PhysicalFrame>, wire::ImageInput),
    Retired,
}
fn admitted_image_input(world: &World, header: &Header) -> Result<ImageAdmission, String> {
    let engine = &world.resource::<crate::resources::VnEngine>().0;
    let (instance, epoch, frame_sequence) = match header {
        Header::Input { instance, epoch, frame_sequence, input, .. } => {
            if input.len() > wire::MAX_INPUT_BYTES { return Err("Oversized image input envelope".into()); }
            // Opaque positive/legacy input retains admission BEFORE JSON.
            if *instance != engine.instance_id() || *epoch != engine.runtime_epoch() { return Err("The rendered engine session changed".into()); }
            (*instance, *epoch, *frame_sequence)
        }
        Header::Release { instance, epoch, frame_sequence, release, .. } => {
            release.validate()?;
            if *instance != engine.instance_id() || *epoch > engine.runtime_epoch() { return Err("The rendered engine session changed".into()); }
            if *epoch < engine.runtime_epoch() {
                let output = world.resource::<Output>();
                // Capture invalidation can precede the release by several images.
                // No new UI/physical input is dispatched for an obsolete scope.
                output.discard_foreign(engine.instance_id(), engine.runtime_epoch());
                return if output.retired(*instance, *epoch, *frame_sequence) { Ok(ImageAdmission::Retired) }
                    else { Err("The completed retired input image expired".into()) };
            }
            (*instance, *epoch, *frame_sequence)
        }
        _ => return Err("Expected image input".into()),
    };
    let frame = world.resource::<Output>().context(instance, epoch, frame_sequence).ok_or("The completed input image expired")?;
    let input = match header {
        Header::Input { input, .. } => wire::ImageInput::decode(input)?,
        Header::Release { release, .. } => release.clone().into_input(),
        _ => unreachable!(),
    };
    if !input.is_passive() {
        if paused(world) || engine.renderer.scene_frame_failed || engine.renderer.scene_batch_pending() || crate::checkpoint_requests::world_blocked(world) || world.get_resource::<crate::accessibility::Accessibility>().is_some_and(|access| access.blocked) { return Err("The preview input publication is not ready".into()); }
        if frame.scope.paused || frame.scope.publication_pending { return Err("The completed input image was not interactive".into()); }
    }
    Ok(ImageAdmission::Current(frame, input))
}
fn image_input_sequence(header: &Header) -> Option<u64> {
    match header { Header::Input { sequence, .. } | Header::Release { sequence, .. } => Some(*sequence), _ => None }
}
fn drain_input(world: &mut World) {
    let headers = std::mem::take(&mut world.resource_mut::<PendingInput>().0);
    if world.contains_resource::<crate::debugger_startup::DebugBootstrap>(){return;}
    let focus_lost = headers.iter().filter_map(|header| {
        let sequence = image_input_sequence(header)?;
        if sequence <= world.resource::<PreviewState>().received_sequence { return None; }
        matches!(admitted_image_input(world, header), Ok(ImageAdmission::Current(_, wire::ImageInput::FocusLost))).then_some(sequence)
    }).max();
    for header in headers {
        let Some(sequence) = image_input_sequence(&header) else { continue; };
        if sequence <= world.resource::<PreviewState>().received_sequence { continue; }
        let result = (|| -> Result<(), String> {
            let ImageAdmission::Current(frame, input) = admitted_image_input(world, &header)? else {
                // Real backend receipt of a known retirement, no world mutation.
                return Ok(());
            };
            if focus_lost.is_some_and(|lost| sequence < lost) { return Err("Input discarded by a later focus loss in the same producer batch".into()); }
            let neutral_only = input.is_passive() && (matches!(input, wire::ImageInput::FocusLost)
                || paused(world) || world.resource::<crate::resources::VnEngine>().0.renderer.scene_batch_pending()
                || world.resource::<crate::resources::VnEngine>().0.renderer.scene_frame_failed
                || crate::checkpoint_requests::world_blocked(world)
                || world.get_resource::<crate::accessibility::Accessibility>().is_some_and(|access| access.blocked)
                || frame.scope.paused || frame.scope.publication_pending);
            let output = world.resource::<Output>().clone();
            let newly_held = output.hold(frame.clone(), &input)?;
            let result = if neutral_only {
                crate::editor_preview_input::capture_widget_button_neutral_release1030(world,&frame,&input);
                crate::editor_preview_input::neutralize_passive(world, &input)
            } else { crate::editor_preview_input::inject(world, frame.clone(), input.clone()) };
            if result.is_err() && newly_held {
                let release = match input {
                    wire::ImageInput::Key { key, logical, modifiers, .. } => Some(wire::ImageInput::Key { key, logical, pressed: false, modifiers }),
                    wire::ImageInput::Pointer { button, point, wheel, .. } => Some(wire::ImageInput::Pointer { kind: wire::PointerKind::Up, button, point, wheel }),
                    _ => None,
                };
                if let Some(release) = release { let _ = output.hold(frame, &release); }
            }
            result
        })();
        let mut state = world.resource_mut::<PreviewState>();
        state.received_sequence = sequence;
        state.control_error = result.as_ref().err().cloned();
        if result.is_ok() { state.sequence = sequence; }
    }
}

#[cfg(test)]
pub(crate) fn image_input_status_for_test(world:&World)->(u64,u64,Option<String>){
    let state=world.resource::<PreviewState>();
    (state.received_sequence,state.sequence,state.control_error.clone())
}
#[cfg(test)]
pub(crate) fn drain_images_for_test(world:&mut World,frame:Arc<crate::editor_preview_input::PhysicalFrame>,inputs:Vec<(u64,wire::ImageInput)>){
    // CPU cache/admission fixture only; this creates no GPU readback proof.
    if !world.contains_resource::<Output>() {world.insert_resource(Output{phase:0,queue:Arc::new((Mutex::new(Latest::default()),Condvar::new())),alive:Arc::new(AtomicBool::new(true)),contexts:Arc::new(Mutex::new(Default::default())),held:Arc::new(Mutex::new(Default::default()))});}
    world.init_resource::<PreviewState>();world.init_resource::<PendingInput>();
    let scope=frame.scope.clone();world.resource::<Output>().frame(Packet{header:Header::Frame{scope:scope.clone(),width:1,height:1,debugger:None,inspection:None},pixels:vec![0;4]},Some(frame));
    world.resource::<Output>().publish_and_consume_cpu_fixture();
    for (sequence,input) in inputs {world.resource_mut::<PendingInput>().0.push(wire::image_input_header(&scope,sequence,input).unwrap());}
    drain_input(world);
}
fn stop(world: &mut World, error: Option<String>) {
    let exit = if let Some(error) = error {
        eprintln!("[editor-preview] {error}");
        bevy::app::AppExit::Error(std::num::NonZeroU8::new(1).unwrap())
    } else {
        bevy::app::AppExit::Success
    };
    world.send_event(exit);
}
fn control(world: &mut World) {
    trace_once(1,format_args!("First/control entered on actual World"));
    if !world.resource::<Output>().alive.load(Ordering::Acquire) {
        let error=world.resource::<Output>().queue.0.lock().ok().and_then(|queue|queue.failure_reason.clone());
        if error.is_some() || !sessions::blocked(world){stop(world,error);return;}
    }
    let headers = world.resource::<Controls>().0.lock()
        .map(|rx| rx.try_iter().take(16).collect::<Vec<_>>())
        .map_err(|_| ());
    let headers = match headers {
        Ok(headers) => headers,
        Err(_) => {
            stop(world, Some("Editor control receiver poisoned".into()));
            return;
        }
    };
    match sessions::controls(world,&headers){Ok(true)=>return,Ok(false)=>{},Err(error)=>{stop(world,Some(error));return;}}
    if !world.contains_resource::<crate::debugger_startup::DebugBootstrap>(){crate::editor_preview_input::begin(world);}
    for header in headers {
        if matches!(&header,Header::FrameConsumed{..}){
            world.init_resource::<PendingConsumed>();world.resource_mut::<PendingConsumed>().0.push(header);continue;
        }
        if startup::control(world,&header){continue;}
        if let Header::Debugger{instance,epoch,sequence,frame_sequence,frame_control_sequence,command}=&header{
            if *sequence<=world.resource::<PreviewState>().received_sequence{continue;}
            let result=(||{
                let engine=world.resource::<crate::resources::VnEngine>();
                // Session and completed-image admission precede JSON commands.
                if *instance!=engine.0.instance_id()||*epoch!=engine.0.runtime_epoch(){return Err("The rendered debugger Engine changed".to_string());}
                if *frame_control_sequence!=world.resource::<PreviewState>().sequence{return Err("The preceding control image is no longer current".into());}
                let frame=world.resource::<Output>().completed(*instance,*epoch,*frame_sequence).ok_or("The completed debugger image expired")?;
                if frame.control_sequence!=*frame_control_sequence{return Err("The completed debugger image has another control source".into());}
                if crate::checkpoint_requests::world_blocked(world)||engine.0.renderer.scene_frame_failed||engine.0.renderer.scene_batch_pending(){return Err("The debugger publication is not ready".into());}
                let command=rvn_core::editor_debugger::Command::decode(command)?;
                match command{
                    rvn_core::editor_debugger::Command::Configure{breakpoints,watches}=>crate::editor_debugger::configure(world,breakpoints,watches),
                    rvn_core::editor_debugger::Command::Sites{offset}=>crate::editor_debugger::sites(world,offset),
                    rvn_core::editor_debugger::Command::Run(kind)=>{
                        if !crate::editor_debugger::pending(world.resource::<crate::resources::VnEngine>())&&kind!=rvn_core::editor_debugger::StepKind::Continue{
                            let engine=world.resource::<crate::resources::VnEngine>();
                            let position=engine.0.state.scene_execution.as_ref().map(|execution|execution.live_position.clone()).unwrap_or(rvn_core::scene::ResumePosition{pc:engine.0.state.pc,origin:None});
                            if !frame.paused||frame.position!=position{return Err("The completed instruction is no longer current".into());}
                        }
                        crate::editor_debugger::run(world,kind)
                    },
                    rvn_core::editor_debugger::Command::Disable=>crate::editor_debugger::cancel(world),
                }
            })();
            let mut state=world.resource_mut::<PreviewState>();state.received_sequence=*sequence;
            state.control_error=result.as_ref().err().cloned();if result.is_ok(){state.sequence=*sequence;}
            continue;
        }
        if matches!(&header,Header::Input{..}|Header::Release{..}) {
            world.init_resource::<PendingInput>();world.resource_mut::<PendingInput>().0.push(header);continue;
        }
        let Header::Control {
            instance,
            epoch,
            sequence,
            frame_control_sequence,
            position,
            interaction,
            kind,
        } = header
        else {
            continue;
        };
        if sequence <= world.resource::<PreviewState>().received_sequence {
            continue;
        }
        let engine = &world.resource::<crate::resources::VnEngine>().0;
        let refusal = if instance != engine.instance_id() || epoch != engine.runtime_epoch() {
            Some("The rendered engine session changed")
        } else if kind == ControlKind::Stop {
            stop(world, None);
            return;
        } else if frame_control_sequence != world.resource::<PreviewState>().sequence {
            Some("The preceding control image is no longer current")
        } else if kind == ControlKind::Pause || kind == ControlKind::Resume {
            None
        } else if engine.renderer.scene_frame_failed
            || engine.renderer.scene_batch_pending()
            || crate::checkpoint_requests::world_blocked(world)
        {
            Some("The player publication is not ready")
        } else if kind == ControlKind::Step {
            let current = engine
                .state
                .scene_execution
                .as_ref()
                .map(|execution| execution.live_position.clone())
                .unwrap_or(rvn_core::scene::ResumePosition {
                    pc: engine.state.pc,
                    origin: None,
                });
            if position != current {
                Some("The displayed instruction changed")
            } else if !paused(world)
                || *world.resource::<State<crate::resources::VnState>>().get()
                    != crate::resources::VnState::Stepping
                || engine.state.motions.waiting.is_some()
                || engine.state.videos.waiting.is_some()
            {
                Some("This boundary cannot execute a paused statement")
            } else {
                None
            }
        } else if paused(world)
            || !interaction
                .as_ref()
                .is_some_and(|source| engine.accepts_preview_input_source(source))
            || !matches!(
                engine.displayed_interaction(),
                Ok(Some(rvn_core::Interaction::Dialogue { .. }))
            )
        {
            Some("The captured Dialogue is no longer an admissible input")
        } else {
            None
        };
        // A refusal must release the editor controls, without accepting the
        // command or manufacturing an image. Its receipt travels with the next
        // actual Camera frame just like an accepted command.
        {
            let mut state = world.resource_mut::<PreviewState>();
            state.received_sequence = sequence;
            state.control_error = refusal.map(str::to_owned);
        }
        if refusal.is_some() {
            continue;
        }
        if kind==ControlKind::Step&&crate::editor_debugger::enabled(world.resource::<crate::resources::VnEngine>()){
            if let Err(error)=crate::editor_debugger::run(world,rvn_core::editor_debugger::StepKind::Into){world.resource_mut::<PreviewState>().control_error=Some(error);continue;}
        }else if kind==ControlKind::Pause&&crate::editor_debugger::enabled(world.resource::<crate::resources::VnEngine>()){
            if let Err(error)=crate::editor_debugger::pause(world){world.resource_mut::<PreviewState>().control_error=Some(error);continue;}
        }else if kind==ControlKind::Resume&&crate::editor_debugger::enabled(world.resource::<crate::resources::VnEngine>()){
            if let Err(error)=crate::editor_debugger::resume(world){world.resource_mut::<PreviewState>().control_error=Some(error);continue;}
        }else if kind == ControlKind::Step {
            world.resource_mut::<PreviewState>().step_requested = true;
            world
                .resource_mut::<NextState<crate::resources::VnState>>()
                .set(crate::resources::VnState::Stepping);
        } else if kind == ControlKind::Advance {
            crate::editor_preview_input::queue_player(world,crate::vn_command::PlayerInput::Advance,interaction);
        } else if let Err(error) = set_paused(world, kind == ControlKind::Pause) {
            stop(world, Some(error));
            return;
        }
        world.resource_mut::<PreviewState>().sequence = sequence;
    }
}

/// One transient pause boundary for editor controls and the later debugger.
/// It does not change any Scene lease, saved clock or PCM residence flag.
pub(crate) fn set_paused(world:&mut World,desired:bool)->Result<(),String>{set_pause(world,desired,true)}
pub(crate) fn set_paused_for_debugger(world:&mut World,desired:bool)->Result<(),String>{set_pause(world,desired,false)}
fn set_pause(world: &mut World, desired: bool, release_inputs:bool) -> Result<(), String> {
    world.init_resource::<PreviewState>();
    let gate = if let Some(engine)=world.get_resource::<crate::resources::VnEngine>(){engine.0.renderer.audio_checkpoints.gate()}
        else{world.get_resource::<crate::debugger_startup::DebugBootstrap>().ok_or("Actual pause owner is missing")?.broker.gate()};
    gate.set_preview_paused(desired)?;
    #[cfg(all(feature="video",not(target_arch="wasm32")))]
    if desired {crate::video_audio::pause_sources(world)?;}
    if release_inputs{world.resource_mut::<PreviewState>().user_paused=desired;}
    let previous = paused(world);
    if desired && release_inputs{
        crate::editor_preview_input::release_held(world);
        if let Some(output)=world.get_resource::<Output>(){output.release_held();}
    }
    if desired && !previous {
        let prior = world.resource::<Time<bevy::time::Virtual>>().is_paused();
        world.resource_mut::<PreviewState>().time_was_paused = prior;
        world.resource_mut::<Time<bevy::time::Virtual>>().pause();
    } else if !desired && previous && !world.resource::<PreviewState>().time_was_paused {
        world.resource_mut::<Time<bevy::time::Virtual>>().unpause();
    }
    {
        let mut state = world.resource_mut::<PreviewState>();
        state.paused = desired;
        if !desired {
            state.step_requested = false;
        }
    }
    // Existing sinks observe Pause immediately at the First command boundary;
    // Last also covers readers created by this frame's ordinary audio phase.
    enforce_audio_pause(world);
    Ok(())
}
fn enforce_audio_pause(world: &mut World) {
    use bevy::audio::AudioSinkPlayback;
    if paused(world) {
        let mut query = world.query::<(Entity, &AudioSink)>();
        let playing: Vec<_> = query
            .iter(world)
            .filter_map(|(entity, sink)| {
                let playing = !sink.is_paused();
                sink.pause();
                playing.then_some(entity)
            })
            .collect();
        let mut state = world.resource_mut::<PreviewState>();
        for entity in playing {
            if !state.legacy_playing.contains(&entity) {
                state.legacy_playing.push(entity);
            }
        }
    } else {
        let playing = std::mem::take(&mut world.resource_mut::<PreviewState>().legacy_playing);
        // Modern consumers have already reapplied their exact residence. Only
        // legacy sources need this restoration of their previous playing state.
        if world.get_resource::<crate::resources::VnEngine>().is_some_and(|engine|engine.0.state.scene_execution.is_none())
        {
            for entity in playing {
                if let Some(sink) = world.get::<AudioSink>(entity) {
                    sink.play();
                }
            }
        }
    }
}
#[derive(Clone, Resource)]
pub(crate) struct InstalledScope(pub wire::FrameScope);
// Called at the existing atomic ScenePresentation installation, after the real
// state was accepted. Render extraction never labels old pixels with a pending PC.
pub(crate) fn installed(world: &mut World, state: &rvn_core::GameState, epoch: u64) {
    if !world.contains_resource::<PreviewState>() {
        return;
    }
    let engine = world.get_resource::<crate::resources::VnEngine>().map(|engine|&engine.0);
    let startup=world.get_resource::<crate::debugger_startup::DebugBootstrap>().map(|startup|startup.owner.snapshot());
    let actual=crate::text_runtime_owner::current(world).expect("Preflighted actual installed owner");
    assert_eq!(actual.epoch,epoch);
    let execution = state.scene_execution.as_ref();
    let scope = wire::FrameScope {
        instance: actual.instance,
        epoch,
        sequence: 0,
        control_sequence: 0,
        control_ack: 0,
        control_error: None,
        paused: false,
        publication_pending: false, finished: None,
        step_allowed: false,
        interaction: engine.and_then(|engine|engine.preview_input_source()),
        position: startup.map(|snapshot|snapshot.position).or_else(||execution.map(|e| e.live_position.clone())).unwrap_or(
            rvn_core::scene::ResumePosition {
                pc: state.pc,
                origin: None,
            },
        ),
        active: execution
            .and_then(|e| e.active.as_ref())
            .map(|a| wire::ActivationScope {
                activation: a.id.0,
                scene: a.scene.clone(),
            }),
        parked: execution
            .map(|e| {
                e.frames
                    .iter()
                    .filter_map(|f| f.parked.as_ref())
                    .map(|a| wire::ActivationScope {
                        activation: a.id.0,
                        scene: a.scene.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default(),
    };
    let inspection = rvn_core::editor_inspection::capture(state, rvn_core::editor_inspection::FrameIdentity::of(&scope));
    world.insert_resource(InstalledInspection(Arc::new(inspection)));
    world.insert_resource(InstalledScope(scope));
}
#[derive(Clone, Resource)]
pub(crate) struct RenderScope(pub wire::FrameScope);
#[derive(Clone,Resource)]
pub(crate) struct RenderStartupSource(pub Option<Arc<rvn_core::engine::StartupObservationSource>>);
#[derive(Clone,Resource)]
pub(crate) struct RenderInputFrame(pub Option<Arc<crate::editor_preview_input::PhysicalFrame>>);
#[derive(Clone,Resource)]
pub(crate) struct RenderDebugger(pub Option<String>);
#[derive(Clone, Resource)]
pub(crate) struct InstalledInspection(pub Arc<rvn_core::editor_inspection::Inspection>);
#[derive(Clone, Resource)]
pub(crate) struct RenderInspection(pub Option<Arc<str>>);

fn capture_inspection(world: &World, scope: &wire::FrameScope) -> Option<Arc<str>> {
    use rvn_core::editor_inspection::{FrameIdentity, Inspection};
    let logical = world.get_resource::<InstalledInspection>()?;
    let engine = &world.get_resource::<crate::resources::VnEngine>()?.0;
    let position = engine.state.scene_execution.as_ref().map(|execution| &execution.live_position);
    if scope.publication_pending || engine.renderer.scene_frame_failed || crate::checkpoint_requests::world_blocked(world)
        || scope.instance != engine.instance_id() || scope.epoch != engine.runtime_epoch()
        || position.map_or(scope.position.pc != engine.state.pc || scope.position.origin.is_some(), |position| position != &scope.position) { return None; }
    let image = crate::programmable_ui::inspect_rendered_ui(world, &logical.0, scope)
        .and_then(|image| crate::editor_preview_inspection::project(world, &image, scope))
        .unwrap_or_else(|error| Inspection::unavailable(FrameIdentity::of(scope), scope.position.clone(), &error));
    // Keep all bytes attached to this scope before render extraction. The GPU
    // slot owns the same Arc until its own mapped copy is delivered.
    image.encode_for(FrameIdentity::of(scope)).ok()
        .and_then(|bytes| String::from_utf8(bytes).ok()).map(Arc::from)
}

fn capture_scope(world: &mut World) {
    match sessions::observe(world){Ok(true)=>{trace_capture(world);return;},Ok(false)=>{},Err(error)=>{trace_capture(world);stop(world,Some(error));return;}}
    if startup::capture(world){trace_capture(world);return;}
    world.insert_resource(RenderStartupSource(None));
    if !world.contains_resource::<InstalledScope>()
        || world
            .resource::<crate::resources::VnEngine>()
            .0
            .state
            .scene_execution
            .is_none()
    {
        let engine = &world.resource::<crate::resources::VnEngine>().0;
        let state = engine.state.clone();
        let epoch = engine.runtime_epoch();
        installed(world, &state, epoch);
    }
    let Some(next) = world
        .resource::<PreviewState>()
        .frame_sequence
        .checked_add(1)
    else {
        stop(world, Some("Editor frame sequence exhausted".into()));
        return;
    };
    world.resource_mut::<PreviewState>().frame_sequence = next;
    let mut scope = world.resource::<InstalledScope>().0.clone();
    scope.sequence = next;
    scope.control_sequence = world.resource::<PreviewState>().sequence;
    scope.control_ack = world.resource::<PreviewState>().received_sequence;
    scope.control_error = world.resource::<PreviewState>().control_error.clone();
    scope.paused = paused(world);
    scope.publication_pending = world
        .resource::<crate::resources::VnEngine>()
        .0
        .renderer
        .scene_batch_pending();
    let engine = &world.resource::<crate::resources::VnEngine>().0;
    scope.step_allowed = scope.paused
        && !scope.publication_pending
        && !engine.renderer.scene_frame_failed
        && !crate::checkpoint_requests::world_blocked(world)
        && *world.resource::<State<crate::resources::VnState>>().get()
            == crate::resources::VnState::Stepping
        && engine.state.motions.waiting.is_none()
        && engine.state.videos.waiting.is_none();
    let engine=&world.resource::<crate::resources::VnEngine>().0;
    let position=engine.state.scene_execution.as_ref().map(|execution|execution.live_position.clone()).unwrap_or(rvn_core::scene::ResumePosition{pc:engine.state.pc,origin:None});
    let debugger_current=!scope.publication_pending&&!engine.renderer.scene_frame_failed&&!crate::checkpoint_requests::world_blocked(world)
        &&scope.instance==engine.instance_id()&&scope.epoch==engine.runtime_epoch()&&scope.position==position;
    // Same installed image fence as debugger/inspection; never relabel a pending or refused image.
    scope.finished = debugger_current.then(|| engine.is_finished());
    let snapshot=if debugger_current{match crate::editor_debugger::encoded_snapshot(world.resource::<crate::resources::VnEngine>()){
        Ok(snapshot)=>snapshot,Err(error)=>{world.resource_mut::<PreviewState>().control_error=Some(error);None}
    }}else{None};
    world.insert_resource(RenderDebugger(snapshot));
    world.insert_resource(RenderInspection(capture_inspection(world, &scope)));
    let physical=crate::editor_preview_input::capture(world,scope.clone());
    world.insert_resource(RenderInputFrame(physical));
    world.insert_resource(RenderScope(scope));
    trace_capture(world);
}

#[cfg(test)]
mod control_contracts {
// Private proposed additions inside editor_preview::control_contracts.
// No test here has been installed, typechecked or executed.
fn terminal_image_wire_0027(world: &mut World) -> serde_json::Value {
    let before = serde_json::to_vec(&world.resource::<crate::resources::VnEngine>().0.state).unwrap();
    capture_scope(world);
    assert_eq!(serde_json::to_vec(&world.resource::<crate::resources::VnEngine>().0.state).unwrap(), before);
    serde_json::to_value(&world.resource::<RenderScope>().0).unwrap()
}

#[test]
fn terminal_image_0027_real_core_end_is_observed_without_changing_the_story() {
    let (mut world, _) = fixture();
    {
        let mut engine = world.resource_mut::<crate::resources::VnEngine>();
        engine.0.step_until_interaction().unwrap();
        assert!(!engine.0.is_finished());
        engine.0.advance_dialogue().unwrap();
        engine.0.step_until_interaction().unwrap();
        assert!(engine.0.is_finished());
    }
    assert_eq!(terminal_image_wire_0027(&mut world)["finished"], serde_json::json!(true));
}

#[test]
fn terminal_image_0027_waiting_dialogue_is_not_a_terminal_observation() {
    let (mut world, _) = fixture();
    world.resource_mut::<crate::resources::VnEngine>().0.step_until_interaction().unwrap();
    assert!(!world.resource::<crate::resources::VnEngine>().0.is_finished());
    assert_eq!(terminal_image_wire_0027(&mut world)["finished"], serde_json::json!(false));
}

fn terminal_restore_fixture_0027() -> (World, rvn_core::save::SaveData) {
    let (mut world, _) = fixture();
    let mut game = rvn_core::Engine::from_program(
        rvn_parser::parse_program("init{set value=0}\nscene label A\nset value=1\n\"First\"\nscene label Empty\nset value=2\n").unwrap(),
        crate::bevy_renderer::BevyRenderer::new(), 16).unwrap();
    game.enter_label("A").unwrap();
    game.step_until_interaction().unwrap();
    world.insert_resource(crate::resources::VnEngine(game));
    world.init_resource::<crate::scene_publication::ScenePresentation>();
    world.init_resource::<crate::resources::VnRenderState>();
    world.init_resource::<crate::resources::ChoiceFocus>();
    world.init_resource::<crate::resources::ImagemapState>();
    world.init_resource::<crate::resources::TypewriterState>();
    world.init_resource::<crate::resources::ScriptErrorMessage>();
    world.init_resource::<crate::accessibility::Accessibility>();
    world.init_resource::<crate::programmable_ui::Screens>();
    world.init_resource::<bevy::ui::UiStack>();
    world.init_resource::<Events<crate::vn_command::VnCommand>>();
    world.init_resource::<Time<()>>();
    crate::scene_publication::consume(&mut world);
    assert!(!world.resource::<crate::resources::VnEngine>().0.renderer.scene_batch_pending());
    let snapshot = world.resource::<crate::resources::VnEngine>().0.snapshot_for_save().unwrap();
    let saved = rvn_core::save::SaveData::try_from_snapshot(&snapshot,1,"first".into(),"program.rvn".into()).unwrap();
    {
        let mut engine = world.resource_mut::<crate::resources::VnEngine>();
        engine.0.enter_label("Empty").unwrap();
        engine.0.step_until_interaction().unwrap();
        assert!(engine.0.is_finished(), "the real explicit final scene must reach Core end");
    }
    crate::scene_publication::consume(&mut world);
    assert!(!world.resource::<crate::resources::VnEngine>().0.renderer.scene_batch_pending());
    (world,saved)
}

#[test]
fn terminal_image_0027_real_restore_revokes_end_until_the_new_image_is_installed() {
    let (mut world,saved) = terminal_restore_fixture_0027();
    assert_eq!(terminal_image_wire_0027(&mut world)["finished"],serde_json::json!(true));
    let old = world.resource::<RenderScope>().0.clone();
    world.resource_mut::<crate::resources::VnEngine>().0.load_data(saved).unwrap();
    let pending = terminal_image_wire_0027(&mut world);
    assert_eq!(pending["epoch"],serde_json::json!(old.epoch));
    assert_eq!(pending["publication_pending"],serde_json::json!(true));
    assert!(pending["finished"].is_null(), "an old image cannot assert the restored engine's end state");
    crate::scene_publication::consume(&mut world);
    let installed = terminal_image_wire_0027(&mut world);
    assert_ne!(installed["epoch"],serde_json::json!(old.epoch));
    assert_eq!(installed["finished"],serde_json::json!(false));
}

#[test]
fn terminal_image_0027_rejected_restore_publication_does_not_mint_a_terminal_status() {
    let (mut world,saved) = terminal_restore_fixture_0027();
    capture_scope(&mut world);
    let old = world.resource::<RenderScope>().0.clone();
    world.resource_mut::<crate::resources::VnEngine>().0.load_data(saved).unwrap();
    world.insert_resource(crate::systems::settings_menu::Settings{music_volume:f32::NAN,..default()});
    crate::scene_publication::consume(&mut world);
    assert!(world.resource::<crate::resources::VnEngine>().0.renderer.scene_frame_failed);
    let refused = terminal_image_wire_0027(&mut world);
    assert_eq!(refused["epoch"],serde_json::json!(old.epoch));
    assert!(refused["finished"].is_null());
}

    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    pub(super) fn fixture() -> (World, mpsc::SyncSender<Header>) {
        let game = rvn_core::Engine::new(
            rvn_parser::parse("set value = 1\n\"Boundary\"\n").unwrap(),
            crate::bevy_renderer::BevyRenderer::new(),
            8,
        )
        .unwrap();
        let mut world = World::new();
        let (tx, rx) = mpsc::sync_channel(16);
        world.insert_resource(crate::resources::VnEngine(game));
        world.insert_resource(Controls(Mutex::new(rx)));
        world.insert_resource(Output {
            phase:0,
            queue: Arc::new((Mutex::new(Latest::default()), Condvar::new())),
            alive: Arc::new(AtomicBool::new(true)),
            contexts:Arc::new(Mutex::new(Default::default())),
            held:Arc::new(Mutex::new(Default::default())),
        });
        world.init_resource::<PreviewState>();
        world.init_resource::<Time<bevy::time::Virtual>>();
        world.insert_resource(State::new(crate::resources::VnState::Stepping));
        world.init_resource::<NextState<crate::resources::VnState>>();
        (world, tx)
    }
    pub(super) fn image_context(world:&mut World,sequence:u64)->Arc<crate::editor_preview_input::PhysicalFrame>{
        world.init_resource::<crate::programmable_ui::Screens>();world.init_resource::<bevy::ui::UiStack>();
        capture_scope(world);let mut scope=world.resource::<RenderScope>().0.clone();scope.sequence=sequence;
        crate::editor_preview_input::capture(world,scope).unwrap()
    }
    fn cache_packet(frame:&crate::editor_preview_input::PhysicalFrame)->Packet{
        // This cache fixture has no GPU proof: only the owned context eviction
        // contract is exercised, independently of real GPU readback tests.
        Packet{header:Header::Frame{scope:frame.scope.clone(),width:1,height:1,debugger:None,inspection:None},pixels:vec![0;4]}
    }
    #[test]
    fn image_gesture_retains_only_bounded_contexts_and_releases_the_old_frame(){
        let (mut world,_)=fixture();let output=world.resource::<Output>().clone();let first=image_context(&mut world,1);
        output.frame(cache_packet(&first),Some(first.clone()));output.publish_and_consume_cpu_fixture();
        let press=wire::ImageInput::Key{key:"KeyA".into(),logical:Some("a".into()),pressed:true,modifiers:[false;4]};
        assert!(output.hold(first.clone(),&press).unwrap());assert!(!output.hold(first.clone(),&press).unwrap());
        for sequence in 2..=20{let frame=image_context(&mut world,sequence);output.frame(cache_packet(&frame),Some(frame));output.publish_and_consume_cpu_fixture();}
        assert_eq!(output.contexts.lock().unwrap().len(),8);
        assert!(output.context(first.scope.instance,first.scope.epoch,1).is_some());
        let release=wire::ImageInput::Key{key:"KeyA".into(),logical:Some("a".into()),pressed:false,modifiers:[false;4]};
        assert!(!output.hold(first.clone(),&release).unwrap());assert!(output.context(first.scope.instance,first.scope.epoch,1).is_none());
        for sequence in 21..=28 {let frame=image_context(&mut world,sequence);let key=wire::ImageInput::Key{key:format!("Key{sequence}"),logical:None,pressed:true,modifiers:[false;4]};output.hold(frame,&key).unwrap();}
        let frame=image_context(&mut world,29);let key=wire::ImageInput::Key{key:"ninth".into(),logical:None,pressed:true,modifiers:[false;4]};
        assert!(output.hold(frame,&key).is_err());assert_eq!(output.held.lock().unwrap().len(),8);
        output.discard_foreign(first.scope.instance+1,first.scope.epoch);
        assert!(output.held.lock().unwrap().is_empty());assert!(output.contexts.lock().unwrap().is_empty());
    }
    #[test]
    fn stale_image_envelope_is_refused_before_its_malformed_payload_is_decoded(){
        let (mut world,_)=fixture();world.init_resource::<PendingInput>();
        let engine=&world.resource::<crate::resources::VnEngine>().0;let instance=engine.instance_id();let epoch=engine.runtime_epoch();let before=serde_json::to_vec(&engine.state).unwrap();
        world.resource_mut::<PendingInput>().0.push(Header::Input{instance:instance+1,epoch,sequence:1,frame_sequence:1,input:"{not JSON".into()});
        drain_input(&mut world);assert_eq!(world.resource::<PreviewState>().control_error.as_deref(),Some("The rendered engine session changed"));
        assert_eq!(serde_json::to_vec(&world.resource::<crate::resources::VnEngine>().0.state).unwrap(),before);
        world.resource_mut::<PendingInput>().0.push(Header::Input{instance,epoch,sequence:2,frame_sequence:999,input:"{not JSON".into()});
        drain_input(&mut world);assert_eq!(world.resource::<PreviewState>().control_error.as_deref(),Some("The completed input image expired"));
        assert_eq!(serde_json::to_vec(&world.resource::<crate::resources::VnEngine>().0.state).unwrap(),before);
        world.resource_mut::<PendingInput>().0.push(Header::Input{instance:instance+1,epoch,sequence:3,frame_sequence:999,input:" ".repeat(wire::MAX_INPUT_BYTES+1)});
        drain_input(&mut world);assert_eq!(world.resource::<PreviewState>().control_error.as_deref(),Some("Oversized image input envelope"));
        assert!(world.resource::<Output>().held.lock().unwrap().is_empty());
    }
    #[test]
    fn pause_releases_physical_keys_and_geometry_without_opening_normal_story_input(){
        let (mut world,_)=fixture();world.init_resource::<Events<bevy::input::keyboard::KeyboardFocusLost>>();world.init_resource::<ButtonInput<MouseButton>>();
        let frame=image_context(&mut world,1);let output=world.resource::<Output>().clone();
        output.hold(frame,&wire::ImageInput::Pointer{kind:wire::PointerKind::Down,button:0,point:[1.0,1.0],wheel:[0.0;2]}).unwrap();
        world.resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Left);
        set_paused(&mut world,true).unwrap();assert!(!world.resource::<ButtonInput<MouseButton>>().pressed(MouseButton::Left));
        assert!(output.held.lock().unwrap().is_empty());assert_eq!(world.resource::<Events<bevy::input::keyboard::KeyboardFocusLost>>().len(),1);
        assert!(!(&mut world).run_system_once(crate::scene_publication::ready));assert!(!(&mut world).run_system_once(crate::scene_publication::story_ready));
    }

    #[test]
    fn debugger_command_admits_the_actual_completed_producer_before_json_and_never_reuses_a_preceding_control_image(){
        let(mut world,tx)=fixture();let physical=image_context(&mut world,7);let frame=physical.scope.clone();
        world.resource::<Output>().frame(cache_packet(&physical),Some(physical));world.resource::<Output>().publish_and_consume_cpu_fixture();
        let before=serde_json::to_vec(&world.resource::<crate::resources::VnEngine>().0.state).unwrap();
        let history=format!("{:?}",world.resource::<crate::resources::VnEngine>().0.history.entries());
        let header=|instance,sequence,frame_sequence,command:String|Header::Debugger{instance,epoch:frame.epoch,sequence,frame_sequence,frame_control_sequence:frame.control_sequence,command};
        tx.send(header(frame.instance+1,1,7,"{malformed".into())).unwrap();control(&mut world);
        assert_eq!(world.resource::<PreviewState>().control_error.as_deref(),Some("The rendered debugger Engine changed"));
        tx.send(header(frame.instance,2,999,"{malformed".into())).unwrap();control(&mut world);
        assert_eq!(world.resource::<PreviewState>().control_error.as_deref(),Some("The completed debugger image expired"));
        tx.send(header(frame.instance,3,7,"{malformed".into())).unwrap();control(&mut world);
        assert!(world.resource::<PreviewState>().control_error.is_some());assert_eq!(world.resource::<PreviewState>().sequence,0);
        assert!(!crate::editor_debugger::enabled(world.resource::<crate::resources::VnEngine>()));
        let configure=rvn_core::editor_debugger::Command::Configure{breakpoints:vec![],watches:vec![]}.encode().unwrap();
        tx.send(header(frame.instance,4,7,configure)).unwrap();control(&mut world);
        assert!(crate::editor_debugger::enabled(world.resource::<crate::resources::VnEngine>()));assert_eq!(world.resource::<PreviewState>().sequence,4);
        tx.send(header(frame.instance,5,7,"{malformed".into())).unwrap();control(&mut world);
        assert_eq!(world.resource::<PreviewState>().control_error.as_deref(),Some("The preceding control image is no longer current"));
        assert_eq!(world.resource::<PreviewState>().received_sequence,5);assert_eq!(world.resource::<PreviewState>().sequence,4);
        assert_eq!(serde_json::to_vec(&world.resource::<crate::resources::VnEngine>().0.state).unwrap(),before);
        assert_eq!(format!("{:?}",world.resource::<crate::resources::VnEngine>().0.history.entries()),history);
    }

    #[test]
    fn debugger_image_snapshot_waits_for_real_load_publication_and_never_labels_the_previous_scene_on_failure(){
        use rvn_core::editor_debugger::{Snapshot,Watch,WatchScope,WatchValue};
        let (mut world,_)=fixture();
        let mut game=rvn_core::Engine::from_program(rvn_parser::parse_program("init{set value=0}\nscene label A\nset value=1\n\"First\"\nscene label B\nset value=2\n\"Second\"\n").unwrap(),crate::bevy_renderer::BevyRenderer::new(),16).unwrap();
        game.enter_label("A").unwrap();game.step_until_interaction().unwrap();world.insert_resource(crate::resources::VnEngine(game));
        world.init_resource::<crate::scene_publication::ScenePresentation>();
        world.init_resource::<crate::resources::VnRenderState>();world.init_resource::<crate::resources::ChoiceFocus>();
        world.init_resource::<crate::resources::ImagemapState>();world.init_resource::<crate::resources::TypewriterState>();
        world.init_resource::<crate::resources::ScriptErrorMessage>();world.init_resource::<crate::accessibility::Accessibility>();
        world.init_resource::<crate::programmable_ui::Screens>();world.init_resource::<bevy::ui::UiStack>();
        world.init_resource::<Events<crate::vn_command::VnCommand>>();world.init_resource::<Time<()>>();
        crate::editor_debugger::configure(&mut world,vec![],vec![Watch{scope:WatchScope::Globals,name:"value".into()}]).unwrap();
        crate::scene_publication::consume(&mut world);assert!(!world.resource::<crate::resources::VnEngine>().0.renderer.scene_batch_pending());
        let snapshot=world.resource::<crate::resources::VnEngine>().0.snapshot_for_save().unwrap();
        let saved=rvn_core::save::SaveData::try_from_snapshot(&snapshot,1,"first".into(),"program.rvn".into()).unwrap();
        {let mut engine=world.resource_mut::<crate::resources::VnEngine>();engine.0.enter_label("B").unwrap();engine.0.step_until_interaction().unwrap();}
        crate::scene_publication::consume(&mut world);capture_scope(&mut world);
        let previous=world.resource::<RenderScope>().0.clone();
        let observed=Snapshot::decode(world.resource::<RenderDebugger>().0.as_ref().unwrap(),previous.instance,previous.epoch).unwrap();
        assert_eq!(observed.position,previous.position);assert_eq!(observed.watches[0].value,WatchValue::Value(rvn_parser::Value::Int(2)));
        let installed=format!("{:?}",world.resource::<crate::scene_publication::ScenePresentation>().batch);
        world.resource_mut::<crate::resources::VnEngine>().0.load_data(saved).unwrap();
        let loaded=serde_json::to_vec(&world.resource::<crate::resources::VnEngine>().0.state).unwrap();
        assert_ne!(previous.epoch,world.resource::<crate::resources::VnEngine>().0.runtime_epoch());
        for _ in 0..3{capture_scope(&mut world);assert!(world.resource::<RenderScope>().0.publication_pending);assert_eq!(world.resource::<RenderScope>().0.epoch,previous.epoch);assert!(world.resource::<RenderDebugger>().0.is_none());}
        world.insert_resource(crate::systems::settings_menu::Settings{music_volume:f32::NAN,..default()});
        crate::scene_publication::consume(&mut world);assert!(world.resource::<crate::resources::VnEngine>().0.renderer.scene_frame_failed);
        capture_scope(&mut world);assert!(world.resource::<RenderDebugger>().0.is_none());
        assert_eq!(format!("{:?}",world.resource::<crate::scene_publication::ScenePresentation>().batch),installed);
        assert_eq!(serde_json::to_vec(&world.resource::<crate::resources::VnEngine>().0.state).unwrap(),loaded);
        world.resource_mut::<crate::systems::settings_menu::Settings>().music_volume=1.0;
        crate::scene_publication::consume(&mut world);assert!(!world.resource::<crate::resources::VnEngine>().0.renderer.scene_batch_pending());
        assert!(!world.resource::<crate::resources::VnEngine>().0.renderer.scene_frame_failed);capture_scope(&mut world);
        let current=&world.resource::<RenderScope>().0;let observed=Snapshot::decode(world.resource::<RenderDebugger>().0.as_ref().unwrap(),current.instance,current.epoch).unwrap();
        assert_eq!(current.epoch,world.resource::<crate::resources::VnEngine>().0.runtime_epoch());assert_eq!(observed.position,current.position);
        assert_eq!(observed.watches[0].value,WatchValue::Value(rvn_parser::Value::Int(1)));assert!(observed.site.is_none());
        // This exercises the actual publication and extraction metadata path.
        // GPU pixels/readback remain a separate native/render contract.
    }

    fn command(world: &World, sequence: u64, kind: ControlKind) -> Header {
        let engine = &world.resource::<crate::resources::VnEngine>().0;
        Header::Control {
            instance: engine.instance_id(),
            epoch: engine.runtime_epoch(),
            sequence,
            frame_control_sequence: world.resource::<PreviewState>().sequence,
            position: rvn_core::scene::ResumePosition {
                pc: engine.state.pc,
                origin: None,
            },
            interaction: engine.preview_input_source(),
            kind,
        }
    }
    #[test]
    fn refused_statement_is_acknowledged_without_state_mutation_and_paused_step_keeps_regular_input_closed(
    ) {
        let (mut world, tx) = fixture();
        tx.send(command(&world, 1, ControlKind::Pause)).unwrap();
        control(&mut world);
        capture_scope(&mut world);
        assert!(world.resource::<RenderScope>().0.paused);
        let before =
            serde_json::to_vec(&world.resource::<crate::resources::VnEngine>().0.state).unwrap();
        let mut wrong = command(&world, 2, ControlKind::Step);
        let Header::Control { position, .. } = &mut wrong else {
            unreachable!()
        };
        position.pc += 1;
        tx.send(wrong).unwrap();
        control(&mut world);
        capture_scope(&mut world);
        let scope = &world.resource::<RenderScope>().0;
        assert_eq!(scope.control_sequence, 1);
        assert_eq!(scope.control_ack, 2);
        assert!(scope.control_error.is_some());
        assert!(!world.resource::<PreviewState>().step_requested);
        assert_eq!(
            serde_json::to_vec(&world.resource::<crate::resources::VnEngine>().0.state).unwrap(),
            before
        );
        tx.send(command(&world, 3, ControlKind::Step)).unwrap();
        control(&mut world);
        capture_scope(&mut world);
        assert_eq!(world.resource::<RenderScope>().0.control_sequence, 3);
        assert_eq!(world.resource::<RenderScope>().0.control_ack, 3);
        assert!(world.resource::<RenderScope>().0.control_error.is_none());
        assert!(world.resource::<PreviewState>().step_requested);
        assert!(!(&mut world).run_system_once(crate::scene_publication::story_ready));
        assert!(!(&mut world).run_system_once(crate::scene_publication::ready));
        assert!((&mut world).run_system_once(crate::scene_publication::preview_statement_ready));
        assert!(world.resource::<Time<bevy::time::Virtual>>().is_paused());
        tx.send(command(&world, 4, ControlKind::Resume)).unwrap();
        control(&mut world);
        assert!(!paused(&world));
        assert!(!world.resource::<Time<bevy::time::Virtual>>().is_paused());
    }
    #[test]
    fn preview_resume_keeps_a_prior_virtual_pause_and_rejects_the_other_engine_instance() {
        let (mut world, tx) = fixture();
        world.resource_mut::<Time<bevy::time::Virtual>>().pause();
        set_paused(&mut world, true).unwrap();
        set_paused(&mut world, false).unwrap();
        assert!(world.resource::<Time<bevy::time::Virtual>>().is_paused());
        let before =
            serde_json::to_vec(&world.resource::<crate::resources::VnEngine>().0.state).unwrap();
        let foreign = rvn_core::Engine::new(
            rvn_parser::parse("set value = 1\n\"Boundary\"\n").unwrap(),
            crate::bevy_renderer::BevyRenderer::new(),
            8,
        )
        .unwrap();
        let mut header = command(&world, 1, ControlKind::Pause);
        let Header::Control { instance, .. } = &mut header else {
            unreachable!()
        };
        *instance = foreign.instance_id();
        tx.send(header).unwrap();
        control(&mut world);
        capture_scope(&mut world);
        assert_eq!(world.resource::<RenderScope>().0.control_sequence, 0);
        assert_eq!(world.resource::<RenderScope>().0.control_ack, 1);
        assert!(world.resource::<RenderScope>().0.control_error.is_some());
        assert!(!paused(&world));
        assert_eq!(
            serde_json::to_vec(&world.resource::<crate::resources::VnEngine>().0.state).unwrap(),
            before
        );
    }

// Inside editor_preview::control_contracts, private candidate only.
#[test]
fn retired_image_0028_registry_retains_only_sixteen_actual_capture_identities_and_drops_geometry() {
    let (mut world, _) = fixture();
    let mut game = rvn_core::Engine::from_program(rvn_parser::parse_program(
        "init{set value=1}\nscene label start\n\"Ready\"\n").unwrap(),
        crate::bevy_renderer::BevyRenderer::new(), 16).unwrap();
    game.enter_label("start").unwrap(); game.step_until_interaction().unwrap();
    world.insert_resource(crate::resources::VnEngine(game));
    world.init_resource::<crate::scene_publication::ScenePresentation>();
    world.init_resource::<crate::resources::VnRenderState>(); world.init_resource::<crate::resources::ChoiceFocus>();
    world.init_resource::<crate::resources::ImagemapState>(); world.init_resource::<crate::resources::TypewriterState>();
    world.init_resource::<crate::resources::ScriptErrorMessage>(); world.init_resource::<crate::accessibility::Accessibility>();
    world.init_resource::<crate::programmable_ui::Screens>(); world.init_resource::<bevy::ui::UiStack>();
    world.init_resource::<Events<crate::vn_command::VnCommand>>(); world.init_resource::<Time<()>>();
    crate::scene_publication::consume(&mut world);
    assert!(!world.resource::<crate::resources::VnEngine>().0.renderer.scene_batch_pending());
    let saved = rvn_core::save::SaveData::try_from_snapshot(
        &world.resource::<crate::resources::VnEngine>().0.snapshot_for_save().unwrap(),1,"ready".into(),"story.rvn".into()).unwrap();
    let output = world.resource::<Output>().clone();
    let mut ids = Vec::new(); let mut weak = Vec::new();
    for generation in 0..3 {
        for offset in 0..8 {
            let frame = image_context(&mut world, 1 + generation*8 + offset);
            ids.push((frame.scope.instance, frame.scope.epoch, frame.scope.sequence));
            weak.push(Arc::downgrade(&frame)); output.frame(cache_packet(&frame), Some(frame));output.publish_and_consume_cpu_fixture();
        }
        let previous = world.resource::<crate::resources::VnEngine>().0.runtime_epoch();
        world.resource_mut::<crate::resources::VnEngine>().0.load_data(saved.clone()).unwrap();
        let engine = &world.resource::<crate::resources::VnEngine>().0;
        assert!(engine.runtime_epoch() > previous);
        output.discard_foreign(engine.instance_id(), engine.runtime_epoch());
        assert!(output.contexts.lock().unwrap().is_empty()); assert!(output.held.lock().unwrap().is_empty());
        crate::scene_publication::consume(&mut world);
    }
    assert_eq!(output.queue.0.lock().unwrap().retired.len(), 16);
    assert!(weak.iter().all(|frame| frame.upgrade().is_none()), "retirement metadata must retain no old geometry Arc");
    for (index, (instance, epoch, sequence)) in ids.into_iter().enumerate() {
        assert_eq!(output.retired(instance, epoch, sequence), index >= 8);
    }
    let engine = &world.resource::<crate::resources::VnEngine>().0;
    output.discard_foreign(engine.instance_id()+1, engine.runtime_epoch());
    assert!(output.queue.0.lock().unwrap().retired.is_empty());
}
#[test]
fn retired_image_0028_release_refuses_unknown_future_and_foreign_identity_without_decoding_old_opaque_inputs() {
    let (mut world, _) = fixture(); world.init_resource::<PendingInput>();
    let engine = &world.resource::<crate::resources::VnEngine>().0;
    let instance = engine.instance_id(); let epoch = engine.runtime_epoch();
    let before = serde_json::to_vec(&engine.state).unwrap();
    for (sequence, instance, epoch, expected) in [
        (1, instance+1, epoch, "The rendered engine session changed"),
        (2, instance, epoch+1, "The rendered engine session changed"),
        (3, instance, epoch, "The completed input image expired")] {
        world.resource_mut::<PendingInput>().0.push(Header::Release {instance, epoch, sequence,
            frame_sequence:999,release:wire::ImageRelease::FocusLost});
        drain_input(&mut world);
        assert_eq!(world.resource::<PreviewState>().control_error.as_deref(), Some(expected));
        assert_eq!(world.resource::<PreviewState>().sequence,0);
        assert_eq!(world.resource::<PreviewState>().received_sequence,sequence);
    }
    assert_eq!(serde_json::to_vec(&world.resource::<crate::resources::VnEngine>().0.state).unwrap(), before);
    assert!(world.resource::<Output>().held.lock().unwrap().is_empty());
}

}

#[cfg(test)]
#[path="editor_preview_publication_tests_0933.rs"]
mod publication_tests_0933;

#[cfg(test)]
#[path="editor_preview_consumption_tests_0938.rs"]
mod consumption_tests_0938;

#[cfg(test)]
mod actual_owner_scope_budget_0968{
 use super::*;
 fn output()->Output{Output{phase:0,queue:Arc::new((Mutex::new(Latest::default()),Condvar::new())),alive:Arc::new(AtomicBool::new(true)),contexts:Default::default(),held:Default::default()}}
 fn frame(app:&App,sequence:u64,size:usize)->Arc<crate::editor_preview_input::PhysicalFrame>{
  let scope=crate::programmable_ui::owner_hosts_0943::tests::actual_scope(app,sequence);
  let baseline=crate::editor_preview_input::capture(app.world(),scope.clone()).unwrap().cost;assert!(baseline<size);
  let mut scope=scope;scope.control_error=Some(String::with_capacity(size-baseline));
  let frame=crate::editor_preview_input::capture(app.world(),scope).unwrap();assert_eq!(frame.cost,size,"actual retained spare String capacity is counted without editing frame.cost");assert!(frame.owner_snapshot().is_some());frame
 }
 fn packet(frame:&crate::editor_preview_input::PhysicalFrame)->Packet{Packet{header:Header::Frame{scope:frame.scope.clone(),width:1,height:1,debugger:None,inspection:None},pixels:vec![31,63,127,255]}}
 #[test]
 fn real_owner_scope_heap_capacity_reaches_exact_union_boundary_with_main_and_held_pins_and_rejects_successor_atomically(){
  let app=crate::programmable_ui::owner_hosts_0943::tests::actual_owner_image_world();let out=output();let mut last=None;
  for sequence in 1..=8{
   let frame=frame(&app,sequence,2*1024*1024);out.frame(packet(&frame),Some(frame.clone()));out.publish_and_consume_cpu_fixture();let key=wire::ImageInput::Key{key:format!("Key{sequence}"),logical:None,pressed:true,modifiers:[false;4]};key.encode().unwrap();assert!(out.hold(frame.clone(),&key).unwrap());last=Some(frame);
  }
  let last=last.unwrap();{let queue=out.queue.0.lock().unwrap();let recent=out.contexts.lock().unwrap();let held=out.held.lock().unwrap();assert_eq!(held.len(),8);assert_eq!(Output::context_cost_all(&recent,&held,None,None,queue.main_consumed.as_ref().and_then(|image|image.context.as_ref()),None),16*1024*1024);assert!(Arc::ptr_eq(queue.main_consumed.as_ref().unwrap().context.as_ref().unwrap(),&last));}
  let next=crate::editor_preview_input::capture(app.world(),crate::programmable_ui::owner_hosts_0943::tests::actual_scope(&app,9)).unwrap();assert!(next.cost>0&&next.owner_snapshot().is_some());out.frame(packet(&next),Some(next));assert!(!out.alive.load(Ordering::Acquire));let queue=out.queue.0.lock().unwrap();assert!(queue.failure_reason.as_deref().unwrap().contains("16 MiB"));assert!(queue.frame.is_none());assert_eq!(queue.main_consumed.as_ref().unwrap().scope.sequence,8);assert!(Arc::ptr_eq(queue.main_consumed.as_ref().unwrap().context.as_ref().unwrap(),&last));assert_eq!(out.held.lock().unwrap().len(),8);
 }
 #[test]
 fn actual_owner_capture_counts_spare_scope_vectors_and_nested_original_provenance_before_cloning_and_refuses_oversize(){
  let app=crate::programmable_ui::owner_hosts_0943::tests::actual_owner_image_world();let mut scope=crate::programmable_ui::owner_hosts_0943::tests::actual_scope(&app,1);scope.interaction=app.world().resource::<crate::resources::VnEngine>().0.preview_input_source();let baseline=crate::editor_preview_input::capture(app.world(),scope.clone()).unwrap().cost;
  let mut extra=0usize;scope.control_error=Some(String::with_capacity(4096));extra+=scope.control_error.as_ref().unwrap().capacity();scope.parked.reserve_exact(1000);extra+=scope.parked.capacity()*std::mem::size_of::<wire::ActivationScope>();
  if let Some(interaction)=&mut scope.interaction{let before=interaction.choice_indices.capacity();interaction.choice_indices.reserve_exact(1000);extra+=(interaction.choice_indices.capacity()-before)*std::mem::size_of::<usize>();}
  if let Some(origin)=&mut scope.position.origin{let before=origin.route.capacity();origin.route.reserve_exact(1000);extra+=(origin.route.capacity()-before)*std::mem::size_of::<rvn_core::program::AstRoutePart>();}
  let frame=crate::editor_preview_input::capture(app.world(),scope).unwrap();assert!(extra>=4096+1000*std::mem::size_of::<wire::ActivationScope>());assert!(frame.cost>=baseline+extra);assert!(frame.owner_snapshot().unwrap().current(app.world()));
  let source=app.world().resource::<crate::resources::VnEngine>().0.debug_state_source().unwrap();let mut excessive=crate::programmable_ui::owner_hosts_0943::tests::actual_scope(&app,2);excessive.control_error=Some(String::with_capacity(2*1024*1024+1));assert!(crate::editor_preview_input::capture(app.world(),excessive).is_none());assert!(app.world().resource::<crate::resources::VnEngine>().0.accepts_debug_state_source(&source));
 }
}
