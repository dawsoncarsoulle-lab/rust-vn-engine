//! Install a complete Core publication before any gameplay clock or input.
//! This exclusive replacement contains no fallible event fan-out. Asset jobs
//! are reconciled afterwards and report failures with their original epoch.
use bevy::prelude::*;
use crate::{resources::{VnEngine, VnRenderState, ChoiceFocus, ImagemapState, TypewriterState}, vn_command::{VnCommand, PlayerInput}};
use rvn_core::renderer::SceneRenderBatch;
use crate::programmable_ui::owner_hosts_0943::{self as owner_hosts,PreparedOwnerProjection};

#[derive(Resource, Default)]
pub(crate) struct ScenePresentation {
    pub(crate) batch: Option<SceneRenderBatch>,
}
#[derive(SystemSet,Debug,Clone,Copy,PartialEq,Eq,Hash)]
pub(crate) struct HostClockSet;
#[derive(SystemSet,Debug,Clone,Copy,PartialEq,Eq,Hash)]
pub(crate) struct MotionFeedbackSet;
#[derive(SystemSet,Debug,Clone,Copy,PartialEq,Eq,Hash)]
pub(crate) struct CanvasFeedbackSet;
#[derive(SystemSet,Debug,Clone,Copy,PartialEq,Eq,Hash)]
pub(crate) struct HostPublicationSet;
#[derive(SystemSet,Debug,Clone,Copy,PartialEq,Eq,Hash)]
pub(crate) struct VideoFeedbackSet;
#[derive(SystemSet,Debug,Clone,Copy,PartialEq,Eq,Hash)]
pub(crate) struct SceneTaskClockSet;
#[derive(SystemSet,Debug,Clone,Copy,PartialEq,Eq,Hash)]
pub(crate) struct GameFrameSet;
#[derive(Resource,Default)]
pub(crate) struct VideoFeedbackQueue(Vec<(rvn_core::video::VideoView,VideoFeedback)>,VideoQueueNonces);
#[derive(Clone,Debug)]
pub(crate) enum VideoFeedback {Report(rvn_core::video::Feedback),Resume}
#[derive(Default)]
pub(crate) struct PendingVideoTerminal {
    pub(crate) feedback:Option<rvn_core::video::Feedback>,
    pub(crate) admitted:bool,
}
impl PendingVideoTerminal {
    pub(crate) fn retain(&mut self,feedback:rvn_core::video::Feedback) {
        if self.feedback.is_none(){self.feedback=Some(feedback);}
    }
    pub(crate) fn submit(&mut self,world:&mut World,view:&rvn_core::video::VideoView)->bool {
        if self.admitted{return true;}
        let Some(feedback)=self.feedback.clone() else{return true;};
        self.admitted=queue_video_feedback(world,view,feedback);self.admitted
    }
}
#[cfg(test)]
pub(crate) fn video_reports_for_test(world:&World)->Vec<(rvn_core::video::VideoView,VideoFeedback)> {
    world.resource::<VideoFeedbackQueue>().0.clone()
}
#[cfg(test)]
pub(crate) fn video_nonces_for_test(world:&World)->Vec<u64>{world.resource::<VideoFeedbackQueue>().1.values.clone()}
#[cfg(test)]
pub(crate) fn startup_feedback_schedule_for_test(world:&mut World){video_feedback(world);}
// PRIVATE top-level additions to scene_publication.rs.
#[derive(Default)]
struct VideoQueueNonces { next:u64, values:Vec<u64> }
#[derive(Clone,Debug)]
struct VideoQueueSnapshot { rows:Vec<(rvn_core::video::VideoView,VideoFeedback)>,nonces:Vec<u64> }
impl VideoFeedbackQueue {
    fn snapshot(&self)->Result<VideoQueueSnapshot,String> {
        if self.0.len()!=self.1.values.len() {return Err("Video report queue identity is inconsistent".into());}
        Ok(VideoQueueSnapshot{rows:self.0.clone(),nonces:self.1.values.clone()})
    }
    fn acknowledge(&mut self,snapshot:&VideoQueueSnapshot,receipts:&[rvn_core::VideoReportReceipt])->Result<(),String> {
        use rvn_core::VideoReportDisposition;
        self.snapshot()?;
        if snapshot.rows.len()!=snapshot.nonces.len() {return Err("Invalid video queue snapshot".into());}
        let mut ordinals=std::collections::BTreeSet::new();
        let mut remove=std::collections::BTreeSet::new();
        for receipt in receipts {
            if receipt.ordinal>=snapshot.nonces.len()||!ordinals.insert(receipt.ordinal) {
                return Err("Invalid duplicate or out-of-bounds video receipt ordinal".into());
            }
            if receipt.disposition!=VideoReportDisposition::Deferred {
                remove.insert(snapshot.nonces[receipt.ordinal]);
            }
        }
        let mut index=0;
        self.0.retain(|_|{let keep=!remove.contains(&self.1.values[index]);index+=1;keep});
        self.1.values.retain(|nonce|!remove.contains(nonce));
        Ok(())
    }
    fn retain_epoch(&mut self,epoch:u64) {
        let mut index=0;
        let mut keep_nonces=Vec::new();
        self.0.retain(|(view,_)|{
            let keep=view.resource_token.as_ref().map(|token|token.task.runtime_epoch).unwrap_or(view.epoch)==epoch;
            if keep {keep_nonces.push(self.1.values[index]);}
            index+=1;keep
        });
        self.1.values=keep_nonces;
    }
    fn take_legacy(&mut self)->Vec<(rvn_core::video::VideoView,VideoFeedback)> {
        self.1.values.clear();std::mem::take(&mut self.0)
    }
}

fn queued_reports(snapshot:&VideoQueueSnapshot)->Result<rvn_core::GameFrameReports,String> {
    let mut reports=rvn_core::GameFrameReports::default();
    for (view,feedback) in &snapshot.rows {
        let token=view.resource_token.clone().ok_or("Narrative video report is missing its exact resource token")?;
        reports.videos.push(rvn_core::GameVideoReport{token,feedback:match feedback {
            VideoFeedback::Report(feedback)=>rvn_core::GameVideoFeedback::Report(feedback.clone()),
            VideoFeedback::Resume=>rvn_core::GameVideoFeedback::Resume,
        }});
    }
    Ok(reports)
}

// Revocation is pure and precedes payload/gates. Live parked descriptors remain
// queued. We do not run a blocked handler by submitting a zero-delta frame.
fn snapshot_after_stale_ack(world:&mut World)->Result<VideoQueueSnapshot,String> {
    let snapshot=world.resource::<VideoFeedbackQueue>().snapshot()?;
    let reports=queued_reports(&snapshot)?;
    let engine=&world.resource::<VnEngine>().0;
    let mut receipts=Vec::new();
    for (ordinal,report) in reports.videos.iter().enumerate() {
        if engine.video_report_authority(&report.token).map_err(|error|error.to_string())?==rvn_core::VideoReportAuthority::Stale {
            receipts.push(rvn_core::VideoReportReceipt{ordinal,disposition:rvn_core::VideoReportDisposition::RejectedStale});
        }
    }
    world.resource_mut::<VideoFeedbackQueue>().acknowledge(&snapshot,&receipts)?;
    world.resource::<VideoFeedbackQueue>().snapshot()
}

fn route_failed(world:&World)->bool {
    world.resource::<VnEngine>().0.renderer.scene_frame_failed
        ||world.get_resource::<State<crate::resources::VnState>>().is_some_and(|state|*state.get()==crate::resources::VnState::Error)
}

pub(crate) fn video_clock_allowed(world:&World,view:&rvn_core::video::VideoView)->bool {
    if world.contains_resource::<crate::debugger_startup::DebugBootstrap>(){return crate::debugger_startup::video_clock_allowed(world,view);}
    let Some(engine)=world.get_resource::<VnEngine>() else{return false;};
    if engine.0.state.scene_execution.is_none() {return game_active(world);}
    let Some(token)=&view.resource_token else{return false;};
    if route_failed(world) {return false;}
    match engine.0.video_report_authority(token) {
        Ok(rvn_core::VideoReportAuthority::Current(rvn_core::scene_tasks::TaskOwner::Host{..}))=>host_active(world),
        Ok(rvn_core::VideoReportAuthority::Current(_))=>game_active(world)&&!engine.0.renderer.scene_batch_pending(),
        _=>false,
    }
}

// Called only by the existing CustomCanvas producer in CanvasFeedbackSet.
// No second Host producer/schedule is installed. All Host roles share delta.
pub(crate) fn host_frame(world:&mut World) {
    if !world.get_resource::<VnEngine>().is_some_and(|engine|engine.0.state.scene_execution.is_some()) {return;}
    // Standalone compatibility callers previously needed no video plugin.
    world.init_resource::<VideoFeedbackQueue>();
    let snapshot=match snapshot_after_stale_ack(world) {
        Ok(snapshot)=>snapshot,Err(message)=>{frame_failure(world,message);return;}
    };
    let audio_snapshot=match crate::scene_audio::snapshot_after_stale_ack(world) {Ok(snapshot)=>snapshot,Err(error)=>{frame_failure(world,error);return;}};
    if route_failed(world)||!host_active(world) {return;}
    let reports=match queued_reports(&snapshot) {
        Ok(reports)=>rvn_core::HostFrameReports{videos:reports.videos},
        Err(message)=>{frame_failure(world,message);return;}
    };
    let delta=crate::editor_debugger::host_seconds(world.resource::<VnEngine>()).unwrap_or_else(||world.get_resource::<Time>().map(|time|time.delta_seconds_f64().min(0.25)).unwrap_or(0.0));
    let state_before=world.resource::<VnEngine>().0.state.clone();
    if let Err(message)=crate::scene_effects::capture_bases(world) {
        world.resource_mut::<VnEngine>().0.state=state_before;frame_failure(world,message);return;
    }
    if crate::editor_debugger::enabled(world.resource::<VnEngine>()){
        let operation=rvn_core::engine::DebugOperation::HostFrame{seconds:delta,reports,audio:audio_snapshot.reports()};
        let retained=match crate::editor_debugger::retained_payload_bytes(&(&state_before,&snapshot,&audio_snapshot)){
            Ok(cost)=>cost,Err(error)=>{world.resource_mut::<VnEngine>().0.state=state_before;frame_failure(world,error);return;}};
        let result=crate::editor_debugger::enqueue_frame(world,operation,state_before,retained,Box::new(move|world,outcome|{
            let rvn_core::engine::DebugOutcome::Host((outcome,audio_receipts))=outcome else{return Err("Wrong debugger Host frame outcome".into());};
            world.resource_mut::<crate::scene_audio::AudioFeedbackQueue>().acknowledge(&audio_snapshot,&audio_receipts)?;
            world.resource_mut::<VideoFeedbackQueue>().acknowledge(&snapshot,&outcome.receipts)?;
            crate::editor_debugger::clear_host_instant(world);Ok(())
        }));
        if let Err(error)=result{frame_failure(world,error);}
        return;
    }
    let result=world.resource_mut::<VnEngine>().0.tick_host_frame_with_audio(delta,reports,audio_snapshot.reports(),true);
    match result {
        Err(error)=>{world.resource_mut::<VnEngine>().0.state=state_before;frame_failure(world,error.to_string());},
        Ok((outcome,audio_receipts))=>{
            if let Err(message)=world.resource_mut::<crate::scene_audio::AudioFeedbackQueue>().acknowledge(&audio_snapshot,&audio_receipts) {frame_failure(world,message);return;}
            // Core supplied checked ordinals from exactly this snapshot. A
            // newly enqueued identical token cannot be removed by these acks.
            if let Err(message)=world.resource_mut::<VideoFeedbackQueue>().acknowledge(&snapshot,&outcome.receipts) {
                frame_failure(world,message);
            }
            // HostPublicationSet owns the existing drain after this producer.
        }
    }
}

const MAX_QUEUED_VIDEO_REPORTS:usize=rvn_core::scene_resources::MAX_RESOURCES;
fn frame_failure(world:&mut World,message:String) {
    if let Some(mut engine)=world.get_resource_mut::<VnEngine>() {if engine.0.state.audio_resource_registry().is_some(){engine.0.renderer.scene_frame_failed=true;}}
    if let Some(mut diagnostic)=world.get_resource_mut::<crate::resources::ScriptErrorMessage>() {diagnostic.0=message;}
    if let Some(mut next)=world.get_resource_mut::<NextState<crate::resources::VnState>>() {next.set(crate::resources::VnState::Error);}
}
fn admit_video_report(world:&mut World,view:&rvn_core::video::VideoView,feedback:VideoFeedback)->bool {
    let mut queue=world.resource_mut::<VideoFeedbackQueue>();
    if queue.0.len()>=MAX_QUEUED_VIDEO_REPORTS {
        drop(queue);
        frame_failure(world,"Video report queue is full; the new report was not accepted".into());
        return false;
    }
    let Some(nonce)=queue.1.next.checked_add(1) else {
        drop(queue);frame_failure(world,"Video report queue nonce exhausted; report not accepted".into());return false;
    };
    queue.1.next=nonce;queue.1.values.push(nonce);
    queue.0.push((view.clone(),feedback));
    true
}
pub(crate) fn queue_video_feedback(world:&mut World,view:&rvn_core::video::VideoView,feedback:rvn_core::video::Feedback)->bool {
    admit_video_report(world,view,VideoFeedback::Report(feedback))
}
pub(crate) fn queue_video_resume(world:&mut World,view:&rvn_core::video::VideoView)->bool {
    admit_video_report(world,view,VideoFeedback::Resume)
}
pub(crate) fn game_active(world:&World)->bool {
    if world.get_resource::<VnEngine>().is_some_and(|engine|crate::editor_debugger::host_seconds(engine).is_some()){return false;}
    if crate::editor_preview::paused(world)||world.get_resource::<VnEngine>().is_some_and(crate::editor_debugger::pending){return false;}
    if crate::checkpoint_requests::world_blocked(world){return false;}
    use crate::resources::VnState;
    let Some(engine)=world.get_resource::<VnEngine>() else {return false;};
    if engine.0.renderer.scene_frame_failed {return false;}
    let focused=world.iter_entities().filter(|entity|entity.contains::<bevy::window::PrimaryWindow>())
        .find_map(|entity|entity.get::<Window>()).map(|window|window.focused)
        .unwrap_or_else(||world.iter_entities().filter_map(|entity|entity.get::<Window>()).all(|window|window.focused));
    clock_policy(world.get_resource::<State<VnState>>().map(|state|state.get()),focused,
        world.get_resource::<crate::accessibility::Accessibility>().is_some_and(|access|access.blocked),
        world.get_resource::<crate::source_menus::SourceMenus>().is_some_and(|menus|menus.modal()),engine.0.renderer.scene_frame_failed)
}
// PRIVATE NOT_RUN. One capability policy for the real Host route and Web.
pub(crate) fn host_clock_allowed(engine: &VnEngine, focused: bool, blocked: bool) -> bool {
    if !focused || blocked || engine.0.renderer.scene_frame_failed
        || engine.0.renderer.scene_batch_pending() {
        return false;
    }
    let Some(execution) = &engine.0.state.scene_execution else { return false; };
    execution.resources.tasks.iter().any(|task| {
        use rvn_core::{scene_resources::{ResourceTaskKind, ResourceToken, TargetKind},
            scene_tasks::{TaskOwner, TaskToken}};
        let TaskOwner::Host { role } = &task.key.owner else { return false; };
        if task.parked || !matches!(task.kind, ResourceTaskKind::Effect | ResourceTaskKind::Canvas | ResourceTaskKind::Motion | ResourceTaskKind::Video | ResourceTaskKind::Sfx | ResourceTaskKind::Voice) {
            return false;
        }
        let token = ResourceToken { task: TaskToken { runtime_epoch: engine.0.runtime_epoch(),
            key: task.key.clone(), revision: task.revision }, target: task.target.clone() };
        if !execution.resources.accepts(&token, engine.0.runtime_epoch(), execution.owner())
            || task.dependencies.iter().any(|dependency|
                !execution.resources.targets.iter().any(|record| record.identity == *dependency)) {
            return false;
        }
        if matches!(task.kind, ResourceTaskKind::Effect | ResourceTaskKind::Sfx | ResourceTaskKind::Voice) { return true; }
        if task.kind==ResourceTaskKind::Motion {
            return engine.0.state.motions.tracks.contains_key(task.alias());
        }
        if task.kind==ResourceTaskKind::Video {
            return engine.0.state.videos.tracks.contains_key(task.alias());
        }
        task.target.kind == TargetKind::Canvas && engine.0.state.ui.screens.iter().any(|screen| {
            screen.host_role.is_some_and(|current| current.id() == role.as_str())
                && screen.canvas_states.keys().any(|element|
                    format!("{}/{element}", screen.name) == task.target.alias)
        })
    })
}

// All Host roles share this instant. A removed menu is not required for an
// independent exact Host Retired effect; its old UI dependencies stay revoked.
pub(crate) fn host_active(world: &World) -> bool {
    let retained=world.get_resource::<VnEngine>().is_some_and(|engine|crate::editor_debugger::host_seconds(engine).is_some());
    if (!retained&&crate::editor_preview::paused(world))||world.get_resource::<VnEngine>().is_some_and(crate::editor_debugger::pending){return false;}
    if crate::checkpoint_requests::world_blocked(world){return false;}
    let Some(engine) = world.get_resource::<VnEngine>() else { return false; };
    let focused = world.iter_entities()
        .filter(|entity| entity.contains::<bevy::window::PrimaryWindow>())
        .find_map(|entity| entity.get::<Window>()).map(|window| window.focused)
        .unwrap_or_else(|| world.iter_entities().filter_map(|entity| entity.get::<Window>())
            .all(|window| window.focused));
    host_clock_allowed(engine, focused, world.get_resource::<crate::accessibility::Accessibility>()
        .is_some_and(|access| access.blocked))
}

fn clock_policy(state:Option<&crate::resources::VnState>,focused:bool,blocked:bool,modal:bool,failed:bool)->bool {
    focused && !blocked && !modal && !failed && state.is_some_and(|state|matches!(state,crate::resources::VnState::Stepping|crate::resources::VnState::Waiting|crate::resources::VnState::Animating))
}
pub(crate) fn story_reveal_allowed(engine:Res<VnEngine>,state:Res<State<crate::resources::VnState>>,
    windows:Query<(&Window,Option<&bevy::window::PrimaryWindow>)>,access:Res<crate::accessibility::Accessibility>,source:Option<Res<crate::source_menus::SourceMenus>>,requests:Option<Res<crate::checkpoint_requests::CheckpointRequests>>,preview:Option<Res<crate::editor_preview::PreviewState>>)->bool {
    if preview.is_some_and(|preview|preview.paused){return false;}
    if requests.is_some_and(|requests|requests.blocked()){return false;}
    if engine.0.state.scene_execution.is_none() {return true;}
    let focused=windows.iter().find_map(|(window,primary)|primary.is_some().then_some(window.focused)).unwrap_or_else(||windows.iter().all(|(window,_)|window.focused));
    clock_policy(Some(state.get()),focused,access.blocked,source.is_some_and(|source|source.modal()),engine.0.renderer.scene_frame_failed)
}
fn owns_game_frame(state:&rvn_core::GameState)->bool{state.audio_resource_registry().is_some()||state.game_tasks.is_some()}
fn game_frame(world:&mut World) {
    use crate::resources::VnState;
    let Some(game)=world.get_resource::<VnEngine>() else{return;};
    let scene=game.0.state.scene_execution.is_some();
    if !owns_game_frame(&game.0.state){return;}
    consume(world);
    // Standalone audio owns no modern video tokens. The existing legacy video
    // route retains and consumes those descriptors after this Game frame.
    let snapshot=if scene{match snapshot_after_stale_ack(world) {
        Ok(snapshot)=>snapshot,Err(message)=>{frame_failure(world,message);return;}
    }}else{VideoQueueSnapshot{rows:Vec::new(),nonces:Vec::new()}};
    let audio_snapshot=match crate::scene_audio::snapshot_after_stale_ack(world) {Ok(snapshot)=>snapshot,Err(error)=>{frame_failure(world,error);return;}};
    if route_failed(world)||!game_active(world)||world.resource::<VnEngine>().0.renderer.scene_batch_pending() {return;}
    let reports=match queued_reports(&snapshot) {
        Ok(reports)=>reports,Err(message)=>{frame_failure(world,message);return;}
    };
    let delta=world.get_resource::<Time>().map(|time|time.delta_seconds_f64().min(3600.0)).unwrap_or(0.0);
    let state_before=world.resource::<VnEngine>().0.state.clone();
    if let Err(error)=crate::scene_effects::capture_bases(world) {world.resource_mut::<VnEngine>().0.state=state_before;frame_failure(world,error);return;}
    if crate::editor_debugger::enabled(world.resource::<VnEngine>()){
        let operation=rvn_core::engine::DebugOperation::GameFrame{seconds:delta,reports,audio:audio_snapshot.reports()};
        let retained=match crate::editor_debugger::retained_payload_bytes(&(&state_before,&snapshot,&audio_snapshot)){
            Ok(cost)=>cost,Err(error)=>{world.resource_mut::<VnEngine>().0.state=state_before;frame_failure(world,error);return;}};
        let result=crate::editor_debugger::enqueue_frame(world,operation,state_before,retained,Box::new(move|world,outcome|{
            let rvn_core::engine::DebugOutcome::Game((outcome,audio_receipts))=outcome else{return Err("Wrong debugger Game frame outcome".into());};
            world.resource_mut::<crate::scene_audio::AudioFeedbackQueue>().acknowledge(&audio_snapshot,&audio_receipts)?;
            world.resource_mut::<VideoFeedbackQueue>().acknowledge(&snapshot,&outcome.receipts)?;
            crate::editor_debugger::retain_host_instant(world,delta);
            consume(world);
            if outcome.frame.scene_changed||outcome.frame.waiting_released{
                if world.get_resource::<State<VnState>>().is_some_and(|state|*state.get()==VnState::Menu){
                    if let Some(mut menu)=world.get_resource_mut::<crate::resources::MenuState>(){menu.return_to=Some(VnState::Stepping);}
                }else if let Some(mut next)=world.get_resource_mut::<NextState<VnState>>(){next.set(VnState::Stepping);}
            }
            Ok(())
        }));
        if let Err(error)=result{frame_failure(world,error);}
        return;
    }
    let result=world.resource_mut::<VnEngine>().0.tick_game_frame_with_receipts_and_audio(delta,reports,audio_snapshot.reports(),true);
    match result {
        Err(error)=>{world.resource_mut::<VnEngine>().0.state=state_before;frame_failure(world,error.to_string());},
        Ok((outcome,audio_receipts))=>{
            if let Err(message)=world.resource_mut::<crate::scene_audio::AudioFeedbackQueue>().acknowledge(&audio_snapshot,&audio_receipts) {frame_failure(world,message);return;}
            if let Err(message)=world.resource_mut::<VideoFeedbackQueue>().acknowledge(&snapshot,&outcome.receipts) {
                frame_failure(world,message);return;
            }
            consume(world);
            if outcome.frame.scene_changed||outcome.frame.waiting_released {
                if world.get_resource::<State<VnState>>().is_some_and(|state|*state.get()==VnState::Menu) {
                    if let Some(mut menu)=world.get_resource_mut::<crate::resources::MenuState>() {menu.return_to=Some(VnState::Stepping);}
                } else if let Some(mut next)=world.get_resource_mut::<NextState<VnState>>() {next.set(VnState::Stepping);}
            }
        }
    }
}
pub(crate) struct ScenePublicationPlugin;
impl Plugin for ScenePublicationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ScenePresentation>().init_resource::<VideoFeedbackQueue>().add_systems(First, consume)
            .configure_sets(Update,(HostClockSet.before(GameFrameSet),GameFrameSet.before(MotionFeedbackSet),MotionFeedbackSet.before(CanvasFeedbackSet),CanvasFeedbackSet.before(VideoFeedbackSet),VideoFeedbackSet.before(SceneTaskClockSet)))
            .add_systems(Update, (
                game_frame.in_set(GameFrameSet).before(crate::systems::stepping_system),
                consume_after_motions.after(MotionFeedbackSet).before(CanvasFeedbackSet),
                consume_after_canvases.after(CanvasFeedbackSet).in_set(HostPublicationSet).before(VideoFeedbackSet),
                video_feedback.in_set(VideoFeedbackSet),
                consume_after_video.after(VideoFeedbackSet).before(SceneTaskClockSet),
                tasks_clock.run_if(resource_exists::<VnEngine>.and_then(ready)).in_set(SceneTaskClockSet).before(crate::systems::stepping_system),
                consume_after_tasks.after(SceneTaskClockSet).before(crate::systems::stepping_system),
                consume_after_story.after(crate::systems::stepping_system).before(crate::systems::dialogue_system)
                    .before(crate::systems::sprite_system).before(crate::systems::background_system)
                    .before(crate::systems::audio_system).before(crate::systems::cinematic_system)
                    .before(crate::systems::choice_system).before(crate::systems::imagemap_system)
                    .before(crate::systems::typewriter_config_system).before(crate::source_menus::SourceMenuSyncSet),
                consume_after_presenters.after(crate::source_menus::SourceMenuSyncSet)
                    .before(crate::programmable_ui::InterfaceReceiveSet),
                consume_after_input.after(crate::systems::player_input_system)
                    .after(crate::programmable_ui::InterfaceInputSet).after(crate::custom_canvas::CanvasInputSet)
                    .after(crate::source_menus::SourceMenuActionSet).before(crate::programmable_ui::InterfaceRenderSet),
            ));
    }
}
fn consume_after_tasks(world:&mut World) { consume(world); }
fn consume_after_motions(world:&mut World) { consume(world); }
fn consume_after_canvases(world:&mut World) { consume(world); }
fn consume_after_video(world:&mut World) { consume(world); }
fn consume_after_story(world:&mut World) { consume(world); }
fn consume_after_presenters(world:&mut World) { consume(world); }
fn consume_after_input(world:&mut World) { consume(world); }
#[path="startup_video_queue.rs"]
mod startup_queue;
pub(crate) use startup_queue::{StartupTerminalTicket,startup_video_feedback,acknowledge_startup_terminal};
fn video_feedback(world:&mut World) {
    if world.contains_resource::<crate::debugger_startup::DebugBootstrap>(){
        if let Err(error)=startup_video_feedback(world){crate::debugger_startup::driver::report_failure(world,error);}
        return;
    }
    if crate::editor_preview::paused(world){return;}
    if world.get_resource::<VnEngine>().is_some_and(|engine|engine.0.state.scene_execution.is_some()) {return;}
    let mut reports=world.resource_mut::<VideoFeedbackQueue>().take_legacy();
    // Capture every player's clock before applying a completion/error handler.
    // Stable sorting retains each class's original producer order.
    reports.sort_by_key(|(_,feedback)|!matches!(feedback,VideoFeedback::Report(rvn_core::video::Feedback::Position{..})));
    for (view,feedback) in reports {
        consume(world);
        let waiting=world.resource::<VnEngine>().0.state.videos.waiting.is_some();
        let result=match feedback {
            VideoFeedback::Report(feedback)=>world.resource_mut::<VnEngine>().0.video_feedback_from_view(&view,feedback),
            VideoFeedback::Resume=>world.resource_mut::<VnEngine>().0.resume_video_from_view(&view),
        };
        if let Err(error)=result {
            if let Some(mut diagnostic)=world.get_resource_mut::<crate::resources::ScriptErrorMessage>() {diagnostic.0=error.to_string();}
            if let Some(mut next)=world.get_resource_mut::<NextState<crate::resources::VnState>>() {next.set(crate::resources::VnState::Error);}
        }
        consume(world);
        for command in world.resource_mut::<VnEngine>().0.renderer.take_pending() {world.send_event(command);}
        if waiting && world.resource::<VnEngine>().0.state.videos.waiting.is_none() {
            if world.get_resource::<State<crate::resources::VnState>>().is_some_and(|state|*state.get()==crate::resources::VnState::Menu) {
                if let Some(mut menu)=world.get_resource_mut::<crate::resources::MenuState>() {menu.return_to=Some(crate::resources::VnState::Stepping);}
            } else if let Some(mut next)=world.get_resource_mut::<NextState<crate::resources::VnState>>() {next.set(crate::resources::VnState::Stepping);}
        }
    }
}

fn tasks_clock(time: Res<Time>, state: Res<State<crate::resources::VnState>>,
    mut engine: ResMut<VnEngine>, accessibility: Res<crate::accessibility::Accessibility>,
    mut errors: ResMut<crate::resources::ScriptErrorMessage>, mut next: ResMut<NextState<crate::resources::VnState>>,
    mut output: EventWriter<VnCommand>) {
    use crate::resources::VnState;
    if engine.0.state.scene_execution.is_some() {return;}
    if accessibility.blocked || !matches!(state.get(),VnState::Stepping|VnState::Waiting|VnState::Animating) { return; }
    let previous_pc = engine.0.state.pc;
    let previous_owner = owner(&engine.0.state);
    let previous_stack = engine.0.state.call_stack.clone();
    match engine.0.tick_scene_tasks(time.delta_seconds_f64().min(3600.0)) {
        Ok(progressed) => {
            for command in engine.0.renderer.take_pending() { output.send(command); }
            if progressed && (previous_pc != engine.0.state.pc || previous_owner != owner(&engine.0.state)
                || previous_stack != engine.0.state.call_stack) { next.set(VnState::Stepping); }
        }
        Err(error) => { errors.0 = error.to_string(); next.set(VnState::Error); }
    }
}

// Attach this condition to each mutating system, not a cached whole set:
// a transfer earlier in this frame immediately blocks the next clock/input.
pub(crate) fn projection_ready(engine: Option<Res<VnEngine>>,requests:Option<Res<crate::checkpoint_requests::CheckpointRequests>>) -> bool {
    requests.is_none_or(|requests|!requests.blocked()) && engine.is_none_or(|engine| !engine.0.renderer.scene_batch_pending())
}
pub(crate) fn host_frame_ready(engine:Option<Res<VnEngine>>,requests:Option<Res<crate::checkpoint_requests::CheckpointRequests>>,preview:Option<Res<crate::editor_preview::PreviewState>>)->bool{
    if engine.as_deref().is_some_and(|engine|engine.0.has_pending_widget_materialization()||engine.0.widget_materialization_waits_for_saved_operation()){return false;}
    let retained=engine.as_deref().is_some_and(|engine|crate::editor_debugger::host_seconds(engine).is_some());
    !engine.as_deref().is_some_and(crate::editor_debugger::pending)
        &&(retained||preview.is_none_or(|preview|!preview.paused))&&projection_ready(engine,requests)
}
pub(crate) fn ready(engine: Option<Res<VnEngine>>,requests:Option<Res<crate::checkpoint_requests::CheckpointRequests>>,preview:Option<Res<crate::editor_preview::PreviewState>>) -> bool {
    if engine.as_deref().is_some_and(|engine|crate::editor_debugger::pending(engine)||engine.0.has_pending_widget_materialization()||engine.0.widget_materialization_waits_for_saved_operation()){return false;}
    preview.is_none_or(|preview|!preview.paused) && projection_ready(engine,requests)
}
pub(crate) fn projection_story_ready(engine:Option<Res<VnEngine>>,requests:Option<Res<crate::checkpoint_requests::CheckpointRequests>>)->bool {
    requests.is_none_or(|requests|!requests.blocked()) && engine.is_none_or(|engine|!engine.0.renderer.scene_batch_pending() && !engine.0.renderer.scene_frame_failed)
}
pub(crate) fn preview_statement_ready(engine:Option<Res<VnEngine>>,requests:Option<Res<crate::checkpoint_requests::CheckpointRequests>>,preview:Option<Res<crate::editor_preview::PreviewState>>)->bool {
    let debugger_result=engine.as_deref().is_some_and(crate::editor_debugger::story_ready);
    if !debugger_result&&engine.as_deref().is_some_and(|engine|engine.0.has_pending_widget_materialization()||engine.0.widget_materialization_waits_for_saved_operation()){return false;}
    (debugger_result||preview.is_none_or(|preview|!preview.paused||preview.step_requested)) && projection_story_ready(engine,requests)
}
pub(crate) fn story_ready(engine:Option<Res<VnEngine>>,requests:Option<Res<crate::checkpoint_requests::CheckpointRequests>>,preview:Option<Res<crate::editor_preview::PreviewState>>)->bool {
    if preview.is_some_and(|preview|preview.paused)||engine.as_deref().is_some_and(|engine|engine.0.has_pending_widget_materialization()||engine.0.widget_materialization_waits_for_saved_operation()){return false;}
    requests.is_none_or(|requests|!requests.blocked()) && engine.is_none_or(|engine|!engine.0.renderer.scene_batch_pending() && !engine.0.renderer.scene_frame_failed)
}

pub(crate) fn consume(world: &mut World) {
    consume_with_music_installer(world, |world, state, epoch| {
        #[cfg(target_arch = "wasm32")]
        {
            let plan = crate::web_music::prepare(world, state, epoch,
                |owner| crate::web_music::enabled(world, owner))?;
            crate::web_music::install(world, plan)
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = (world, state, epoch);
            Ok(())
        }
    });
}

// One real publication path. The browser installer is a platform boundary;
// native uses no operation, CPU contracts can exercise a synchronous refusal.
fn consume_with_music_installer(
    world: &mut World,
    install_music: impl FnOnce(&mut World, &rvn_core::GameState, u64) -> Result<(), String>,
) {
    // Dialogue/Choice/Imagemap can be reached after the last Core model
    // publication in the same step. State and frozen display must describe
    // that same instant before the old pending batch is removed or installed.
    let current = {
        let Some(engine) = world.get_resource::<VnEngine>() else { return };
        if !engine.0.renderer.scene_batch_pending() { return; }
        // A reached statement is not an executed, authorized presentation.
        // Silent stepping and a transfer may stop before the new Dialogue.
        let display = if engine.0.state.scene_execution.as_ref().is_some_and(|execution| execution.display.is_none()) {
            Ok(None)
        } else {
            engine.0.displayed_interaction()
        };
        display.map(|display| (engine.0.state.clone(), display, engine.0.runtime_epoch()))
    };
    let (current_state, current_display, current_epoch) = match current {
        Ok(current) => current,
        Err(error) => { frame_failure(world, error.to_string()); return; }
    };
    if let Err(error)=crate::display_reveal::preflight_scene(world,&current_display){frame_failure(world,error);return;}
    let choice_backdrop = if matches!(&current_display, Some(rvn_core::Interaction::Choice { .. })) {
        match crate::systems::dialogue::prepare_choice_backdrop(world.resource::<VnEngine>()) {
            Ok(backdrop) => Some(backdrop),
            Err(error) => { frame_failure(world, error); return; }
        }
    } else { None };
    if let Err(error)=crate::systems::audio::current_music_gain_from_state(&current_state,current_epoch).and_then(|logical|crate::music_mix::in_world(world,logical)) {frame_failure(world,error);return;}
    let retired=match crate::scene_retired::prepare(world,&current_state,current_epoch) {
        Ok(plan)=>plan,Err(error)=>{frame_failure(world,error);return;}
    };
    let audio=match crate::scene_audio::prepare(world,&current_state,current_epoch) {
        Ok(plan)=>plan,Err(error)=>{frame_failure(world,error);return;}
    };
    if let Err(error)=crate::systems::cinematic::preflight_cg(world,&current_state) {frame_failure(world,error);return;}
    let sources=match crate::checkpoint_sources::prepare(world,&current_state,current_epoch) {Ok(Some(plan))=>plan,Ok(None)=>return,Err(error)=>{frame_failure(world,error);return;}};
    // Complete browser projection can fail synchronously. Do it before
    // taking pending/output or installing any other ECS presentation.
    // Playback admission ignores transient pending and clock gates; the
    // after-HostPublicationSet refresh reads new gains without advancing clocks.
    if let Err(error) = install_music(world, &current_state, current_epoch) {
        #[cfg(target_arch = "wasm32")]
        crate::web_music::failure(world, error);
        #[cfg(not(target_arch = "wasm32"))]
        frame_failure(world, error);
        return;
    }
    let owners=match (||{
        let engine=world.resource::<VnEngine>();let batch=engine.0.renderer.scene_batch.as_ref().ok_or("Actual pending scene disappeared")?;
        let children=engine.0.renderer.pending.iter().rev().find_map(|command|if let VnCommand::Interfaces(views)=command{Some(views.as_slice())}else{None}).unwrap_or(&batch.interfaces);
        owner_hosts::prepare(&current_state,children,owner_hosts::OwnerPublicationSource{instance:engine.0.instance_id(),epoch:current_epoch,interface_epoch:engine.0.interface_epoch()})
    })(){Ok(owners)=>owners,Err(error)=>{frame_failure(world,error);return;}};
    if let Err(error)=crate::checkpoint_sources::publish(world,&sources){frame_failure(world,error);return;}
    let speech=match crate::accessibility::retired_speech_0975::PreparedRetiredSpeech::prepare(world,&world.resource::<VnEngine>().0.renderer.scene_batch.as_ref().expect("validated pending batch").retired_widget_speech){Ok(plan)=>plan,Err(error)=>{frame_failure(world,error);return;}};
    install_preflighted(world,sources,audio,retired,choice_backdrop,current_display,owners,speech,
        #[cfg(all(feature="video",not(target_arch="wasm32")))] None);
}

pub(crate) struct PreparedSceneInstallation {
    speech:crate::accessibility::retired_speech_0975::PreparedRetiredSpeech,
    owners:PreparedOwnerProjection,
    pub(crate) sources:crate::checkpoint_sources::PreparedSources,
    audio:crate::scene_audio::PreparedAudio,
    retired:crate::scene_retired::PreparedRetired,
    choice:Option<crate::systems::dialogue::ChoiceDialogueBackdrop>,
    display:Option<rvn_core::Interaction>,
    #[cfg(all(feature="video",not(target_arch="wasm32")))]
    pub(crate) native:crate::native_video::PreparedNativeRestore,
}
/// Early runtime-only preflight. Full admission requires opaque Core preparation,
/// actual decoded media, source receipts and the shared reader fence below.
pub(crate) fn preflight_restore_media(world:&World,candidate:&rvn_core::video::VideoState)->Result<(),String>{
    #[cfg(all(feature="video",not(target_arch="wasm32")))]
    {let _=candidate;
     // video_views legitimately populates the live interface projection cache.
     // Warm this actual read before Core captures its full source authority.
     let engine=world.get_resource::<VnEngine>().ok_or("Actual Engine is missing")?;
     let _=engine.0.video_views().map_err(|e|e.to_string())?;Ok(())}
    #[cfg(not(all(feature="video",not(target_arch="wasm32"))))]
    {let engine=world.get_resource::<VnEngine>().ok_or("Actual Engine is missing")?;
     if !candidate.tracks.is_empty()||!engine.0.video_views().map_err(|e|e.to_string())?.is_empty(){return Err("Atomic Save15 requires the actual native video runtime".into());}Ok(())}
}
pub(crate) fn prepare_restore(world:&World,core:&rvn_core::engine::PreparedSave15)->Result<PreparedSceneInstallation,String> {
    if cfg!(target_arch="wasm32"){return Err("Atomic Save15 browser audio installation has not been qualified".into());}
    if world.get_resource::<ScenePresentation>().is_none() || world.get_resource::<Events<VnCommand>>().is_none(){return Err("Actual scene presentation/event runtime is missing".into());}
    let engine=world.get_resource::<VnEngine>().ok_or("Actual Engine is missing")?;
    if engine.0.renderer.scene_batch_pending() || !engine.0.renderer.pending.is_empty() || engine.0.renderer.menu_pending{return Err("Actual renderer has unsettled outgoing publication".into());}
    let batch=core.render_batch();
    preflight_restore_media(world,&batch.state.videos)?;
    if !batch.menu_requests.is_empty(){return Err("Restore cannot replay a menu producer".into());}
    crate::display_reveal::preflight_prepared(core)?;
    let choice=if matches!(batch.display,Some(rvn_core::Interaction::Choice{..})){
        Some(crate::systems::dialogue::prepare_choice_backdrop_from_interaction(core.choice_backdrop().cloned())?)
    }else{None};
    crate::systems::audio::current_music_gain_from_state(&batch.state,batch.runtime_epoch).and_then(|logical|crate::music_mix::in_world(world,logical))?;
    let retired=crate::scene_retired::prepare(world,&batch.state,batch.runtime_epoch)?;
    let audio=crate::scene_audio::prepare(world,&batch.state,batch.runtime_epoch)?;
    crate::systems::cinematic::preflight_cg(world,&batch.state)?;
    let sources=crate::checkpoint_sources::prepare_restore(world,core)?.ok_or("Candidate audio sources are still loading")?;
    crate::checkpoint_sources::verify_restore_assets(world,&sources)?;
    #[cfg(all(feature="video",not(target_arch="wasm32")))]
    let native=crate::native_video::prepare_restore(world,core)?.ok_or("Native media sources are still staging")?;
    let owners=owner_hosts::prepare_scene(batch,engine.0.instance_id())?;
    let speech=crate::accessibility::retired_speech_0975::PreparedRetiredSpeech::prepare(world,&batch.retired_widget_speech)?;
    Ok(PreparedSceneInstallation{sources,audio,retired,choice,display:batch.display.clone(),owners,speech,
        #[cfg(all(feature="video",not(target_arch="wasm32")))] native})
}
/// Repeat all pure preflights while retaining the owned ready native workers.
pub(crate) fn refresh_restore(world:&World,core:&rvn_core::engine::PreparedSave15,mut plan:PreparedSceneInstallation)->Result<PreparedSceneInstallation,String>{
    let batch=core.render_batch();
    plan.speech.verify(world,&batch.retired_widget_speech)?;
    let engine=world.get_resource::<VnEngine>().ok_or("Actual Engine vanished")?;
    if !engine.0.accepts_prepared_save15(core){return Err("Actual Core source changed before restore".into());}
    if engine.0.renderer.scene_batch_pending()||!engine.0.renderer.pending.is_empty()||engine.0.renderer.menu_pending{return Err(format!("Actual renderer changed before restore: scene={}, pending={}, menu={}",engine.0.renderer.scene_batch_pending(),engine.0.renderer.pending.len(),engine.0.renderer.menu_pending));}
    if world.get_resource::<ScenePresentation>().is_none()||world.get_resource::<Events<VnCommand>>().is_none(){return Err("Actual scene presentation/event runtime vanished".into());}
    if !batch.menu_requests.is_empty(){return Err("Restore cannot replay a menu producer".into());}
    crate::display_reveal::preflight_prepared(core)?;
    plan.choice=if matches!(batch.display,Some(rvn_core::Interaction::Choice{..})){Some(crate::systems::dialogue::prepare_choice_backdrop_from_interaction(core.choice_backdrop().cloned())?)}else{None};
    crate::systems::audio::current_music_gain_from_state(&batch.state,batch.runtime_epoch).and_then(|logical|crate::music_mix::in_world(world,logical))?;
    plan.retired=crate::scene_retired::prepare(world,&batch.state,batch.runtime_epoch)?;
    plan.audio=crate::scene_audio::prepare(world,&batch.state,batch.runtime_epoch)?;
    crate::systems::cinematic::preflight_cg(world,&batch.state)?;
    crate::checkpoint_sources::verify_restore_assets(world,&plan.sources)?;
    plan.owners=owner_hosts::prepare_scene(batch,engine.0.instance_id())?;
    plan.display=batch.display.clone();
    #[cfg(all(feature="video",not(target_arch="wasm32")))] crate::native_video::verify_restore(world,core,&plan.native)?;
    Ok(plan)
}
pub(crate) fn install_restore(world:&mut World,plan:PreparedSceneInstallation){
    install_preflighted(world,plan.sources,plan.audio,plan.retired,plan.choice,plan.display,plan.owners,plan.speech,
        #[cfg(all(feature="video",not(target_arch="wasm32")))] Some(plan.native));
}
pub(crate) fn install_preflighted(world:&mut World,sources:crate::checkpoint_sources::PreparedSources,audio:crate::scene_audio::PreparedAudio,retired:crate::scene_retired::PreparedRetired,choice_backdrop:Option<crate::systems::dialogue::ChoiceDialogueBackdrop>,current_display:Option<rvn_core::Interaction>,owners:PreparedOwnerProjection,speech:crate::accessibility::retired_speech_0975::PreparedRetiredSpeech,#[cfg(all(feature="video",not(target_arch="wasm32")))] native:Option<crate::native_video::PreparedNativeRestore>){
    let current_state=world.resource::<VnEngine>().0.state.clone();
    let Some(mut batch) = world.get_resource_mut::<VnEngine>()
        .and_then(|mut engine| engine.0.renderer.scene_batch.take()) else { return };
    let (mut subsequent, stepped) = {
        let mut engine = world.resource_mut::<VnEngine>();
        // Ordinary target-scene statements can follow the transfer in the
        // same step. Their committed deltas belong to this same runtime epoch.
        debug_assert_eq!(batch.runtime_epoch, engine.0.runtime_epoch());
        batch.state = current_state;
        batch.display = current_display;
        let stepped = std::mem::take(&mut engine.0.renderer.scene_stepped);
        (engine.0.renderer.take_pending(), stepped)
    };
    subsequent.retain(|command| {
        match command {
            VnCommand::Interfaces(views) => batch.interfaces = views.clone(),
            VnCommand::Motions(views) => batch.motions = views.clone(),
            VnCommand::LayeredCharacters(views) => batch.layered = views.clone(),
            VnCommand::Videos(views) => {
                batch.videos = views.clone();
                if let Some(view) = views.first().filter(|view|view.resource_token.is_none()) { batch.video_epoch = view.epoch; }
            }
            _ => return true,
        }
        false
    });
    batch.interface_epoch = world.resource::<VnEngine>().0.interface_epoch();
    let instance_id=world.resource::<VnEngine>().0.instance_id();
    let display_source_changed = world.resource::<VnEngine>().0.display_reveal_source().is_some_and(|source|
        world.get_resource::<crate::display_reveal::DialogueRevealBinding>()
            .is_some_and(|binding|binding.source.as_ref()!=Some(&source)));
    let display_instance_changed=world.get_resource::<crate::display_reveal::DialogueRevealBinding>()
        .and_then(|binding|binding.source.as_ref()).is_some_and(|source|source.engine_instance!=world.resource::<VnEngine>().0.instance_id());
    let display_command=batch.display.clone().map(|display|display_command(world.resource::<VnEngine>(),display));
    install_owned(world,*batch,speech.commands(),subsequent,stepped,display_source_changed,display_instance_changed,display_command,None,instance_id,owners,sources,audio,retired,choice_backdrop,
        #[cfg(all(feature="video",not(target_arch="wasm32")))] native);
}
/// Consumes the uniquely transferred real startup publication. All host/source
/// preflights ran before Core mutation; this installer cannot execute an AST,
/// construct a gameplay owner, load a Save15 or retry a producer.
pub(crate) fn install_startup(world:&mut World,publication:crate::bevy_renderer::StartupPublication,owners:PreparedOwnerProjection,sources:crate::checkpoint_sources::PreparedSources,audio:crate::scene_audio::PreparedAudio,retired:crate::scene_retired::PreparedRetired,
    #[cfg(all(feature="video",not(target_arch="wasm32")))] native:Option<crate::native_video::PreparedNativeRestore>){
    let actual=world.resource::<crate::debugger_startup::DebugBootstrap>().owner.snapshot();
    assert_eq!(actual.instance,publication.source.instance_id());assert_eq!(actual.epoch,publication.source.runtime_epoch());
    let input=world.resource::<crate::debugger_startup::DebugBootstrap>().owner.imagemap_interaction().expect("Admitted input Startup keeps its source-owned projection");
    assert_eq!(publication.batch.display,input,"Startup display must be its actual source-owned input wait");
    owner_hosts::install_startup_backend(world,owner_hosts::OwnerPublicationSource{instance:actual.instance,epoch:actual.epoch,interface_epoch:publication.batch.interface_epoch},publication.widget_backend);
    let display_command=match &publication.batch.display{
        Some(rvn_core::Interaction::Imagemap{background,hover_image,hotspots})=>Some(VnCommand::ShowImagemap{background:background.clone(),hover_image:hover_image.clone(),hotspots:hotspots.iter().map(crate::vn_command::ImagemapZone::from).collect()}),
        None=>None,
        _=>unreachable!("Core Startup publishes no narrative Dialogue/Choice"),
    };
    // Selecting a branch retires the actual old map even at the same Init PC.
    // The generic configuration-only installer deliberately retains a None
    // display, so Startup performs its own genuine input retirement here.
    if publication.batch.display.is_none(){if let Some(mut map)=world.get_resource_mut::<ImagemapState>(){map.clear();}}
    let display_instance_changed=world.get_resource::<crate::display_reveal::DialogueRevealBinding>().and_then(|binding|binding.source.as_ref()).is_some_and(|source|source.engine_instance!=actual.instance);
    crate::runtime_restart::reset_projection_before_install(world);
    install_owned(world,publication.batch,Vec::new(),publication.commands,false,false,display_instance_changed,display_command,Some(Ok(vec![])),actual.instance,owners,sources,audio,retired,None,
        #[cfg(all(feature="video",not(target_arch="wasm32")))] native);
    world.resource_mut::<crate::debugger_startup::DebugBootstrap>().publication_pending=false;
    crate::runtime_restart::publication_installed(world,&publication.source);
}
fn install_owned(world:&mut World,mut batch:SceneRenderBatch,mut commands:Vec<VnCommand>,mut subsequent:Vec<VnCommand>,stepped:bool,display_source_changed:bool,display_instance_changed:bool,display_command:Option<VnCommand>,receipt_result:Option<Result<Vec<VnCommand>,String>>,instance_id:u64,owners:PreparedOwnerProjection,
    sources:crate::checkpoint_sources::PreparedSources,audio:crate::scene_audio::PreparedAudio,retired:crate::scene_retired::PreparedRetired,choice_backdrop:Option<crate::systems::dialogue::ChoiceDialogueBackdrop>,
    #[cfg(all(feature="video",not(target_arch="wasm32")))] native:Option<crate::native_video::PreparedNativeRestore>){
    let installed = world.resource::<ScenePresentation>().batch.as_ref();
    let runtime_replaced = installed.is_none_or(|old|old.runtime_epoch != batch.runtime_epoch);
    let owner_changed = installed.is_none_or(|old|owner(&old.state) != owner(&batch.state));
    // A configuration-only publication with no produced display keeps the
    // pixels/cursor until a new interaction supplies its exact authority.
    let display_changed = runtime_replaced || display_instance_changed || (batch.display.is_some()
        && (display_source_changed || installed.is_none_or(|old|old.display != batch.display)));
    commands.extend(presentation_difference(installed.map(|old|&old.state),&batch.state,&subsequent,runtime_replaced));
    batch.retired_widget_speech.clear();
    if world.get_resource::<crate::systems::cinematic::CinematicPublicationSession>().is_none_or(|session|session.instance_id!=Some(instance_id)) {
        commands.retain(|command|!matches!(command,VnCommand::ShowCinematic{..}|VnCommand::HideCinematic{..}));
        commands.push(match &batch.state.cinematic.current {Some(id)=>VnCommand::ShowCinematic{id:id.clone(),transition:None},None=>VnCommand::HideCinematic{transition:None}});
    }
    if runtime_replaced || display_instance_changed {
        if let Some(mut history)=world.get_resource_mut::<crate::resources::DialogueHistory>(){history.clear();}
    }
    if runtime_replaced {
        if let Some(mut game)=world.get_resource_mut::<VnEngine>(){game.0.renderer.scene_frame_failed=false;}
        // Load/NewGame revoke old producer capabilities before fresh players
        // can be admitted. Call/Return retain both epoch and queued reports.
        if let Some(mut queue)=world.get_resource_mut::<VideoFeedbackQueue>() {
            queue.retain_epoch(batch.runtime_epoch);
        }
    }
    if runtime_replaced || owner_changed {
        if let Some(mut events) = world.get_resource_mut::<Events<VnCommand>>() { events.clear(); }
        if let Some(mut inputs) = world.get_resource_mut::<Events<PlayerInput>>() { inputs.clear(); }
    }
    crate::systems::setup::install_character_names(world, &batch.state);
    crate::programmable_ui::install_scene_with_owners(world, batch.interfaces.clone(),owners);
    crate::composed_motion::install_scene(world, batch.motions.clone());
    crate::layered_characters::install_scene(world, batch.layered.clone());
    #[cfg(all(feature="video", not(target_arch="wasm32")))]
    if let Some(native)=native{crate::native_video::install_restore(world,native);}else{crate::native_video::install_scene(world,batch.videos.clone());}
    #[cfg(target_arch = "wasm32")]
    crate::web_video::install_scene(world, batch.videos.clone());
    // Transfer the old exact MusicEntity role before Current event handlers
    // or stale-lease restoration can stop/modify that outgoing reader.
    crate::checkpoint_sources::install(world,sources);
    crate::scene_audio::install(world,audio);
    crate::scene_retired::install(world,retired);
    crate::scene_effects::install(world,batch.effects.clone(),batch.runtime_epoch);
    if display_changed { if let Some(mut render) = world.get_resource_mut::<VnRenderState>() {
        let after = if stepped { render.state_after_anim.take() } else { None };
        *render = default(); render.state_after_anim = after;
    }
    if let Some(mut focus) = world.get_resource_mut::<ChoiceFocus>() { focus.clear(); }
    if let Some(mut imagemap) = world.get_resource_mut::<ImagemapState>() { imagemap.clear(); }
    if let Some(mut typing) = world.get_resource_mut::<TypewriterState>() { *typing = default(); }
    }
    let choice_backdrop_changed = choice_backdrop.as_ref().is_some_and(|backdrop|
        crate::systems::dialogue::choice_backdrop_changed(world, backdrop));
    // Visual/audio assets are reconciled after the authoritative replacement.
    // Resource errors belong to async feedback, not the Core transaction.
    if display_changed || choice_backdrop_changed {
        if let Some(backdrop) = choice_backdrop {
            crate::systems::dialogue::install_choice_backdrop(world, backdrop);
        } else if display_changed {
            crate::systems::dialogue::clear_choice_backdrop_content(world);
        }
    }
    if (!stepped || batch.state.scene_execution.is_some()) && display_changed { if let Some(display) = display_command { commands.push(display); } }
    for request in batch.speech_requests.drain(..) { commands.push(VnCommand::Speech(request)); }
    let receipt_result=receipt_result.unwrap_or_else(||world.resource_mut::<VnEngine>().0.renderer.take_scene_menu_receipts());
    match receipt_result {
        Ok(mut receipts) => commands.append(&mut receipts),
        Err(error) => {
            if let Some(mut diagnostic) = world.get_resource_mut::<crate::resources::ScriptErrorMessage>() {diagnostic.0=error;}
            if let Some(mut next) = world.get_resource_mut::<NextState<crate::resources::VnState>>() {next.set(crate::resources::VnState::Error);}
        }
    }
    commands.append(&mut subsequent);
    world.insert_resource(crate::systems::cinematic::CinematicPublicationSession{instance_id:Some(instance_id)});
    crate::editor_preview::installed(world,&batch.state,batch.runtime_epoch);
    world.resource_mut::<ScenePresentation>().batch = Some(batch);
    for command in commands { world.send_event(command); }
}
fn owner(state:&rvn_core::GameState)->Option<rvn_core::scene::ActivationId> {
    state.scene_execution.as_ref().and_then(|execution|execution.owner())
}
// Shared Game presentation is reconciled against the last model actually
// installed, never against an intermediate Init candidate or parked parent.
fn presentation_difference(old:Option<&rvn_core::GameState>,new:&rvn_core::GameState,pending:&[VnCommand],reset:bool)->Vec<VnCommand> {
    use rvn_parser::Transition;
    let old = if reset {None} else {old};
    let transition=if reset {Transition::None} else {new.last_transition.clone()};
    let mut commands=Vec::new();
    if reset {commands.push(VnCommand::ClearSprites);}
    if old.is_none_or(|old|old.background_image != new.background_image
        || game_target(old,rvn_core::scene_resources::TargetKind::Background,"background") != game_target(new,rvn_core::scene_resources::TargetKind::Background,"background"))
        && !pending.iter().any(|command|matches!(command,VnCommand::SetBackground{..})) {
        commands.push(VnCommand::SetBackground{path:new.background_image.clone(),transition:transition.clone()});
    }
    let mut ids:std::collections::BTreeSet<_>=new.sprites.keys().cloned().collect();
    if let Some(old)=old {ids.extend(old.sprites.keys().cloned());}
    for id in ids {
        if pending.iter().any(|command|matches!(command,VnCommand::ShowSprite{id:target,..}|VnCommand::HideSprite{id:target,..}|VnCommand::MoveSprite{id:target,..} if target==&id)) {continue;}
        let previous=old.and_then(|old|old.sprites.get(&id));
        match new.sprites.get(&id) {
            Some(current) if current.visible => {
                if previous.is_none_or(|old|!old.visible || old.emotion!=current.emotion)
                    || old.is_some_and(|old|game_target(old,rvn_core::scene_resources::TargetKind::Sprite,&id)!=game_target(new,rvn_core::scene_resources::TargetKind::Sprite,&id)) {commands.push(VnCommand::ShowSprite{id,emotion:current.emotion.clone(),position:current.position.clone(),transition:transition.clone()});}
                else if previous.is_some_and(|old|old.position!=current.position) {commands.push(VnCommand::MoveSprite{id,position:current.position.clone(),transition:transition.clone()});}
            }
            _ if previous.is_some_and(|old|old.visible) => commands.push(VnCommand::HideSprite{id,transition:transition.clone()}),
            _=>{}
        }
    }
    if old.is_none_or(|old|old.cinematic!=new.cinematic
        || game_target(old,rvn_core::scene_resources::TargetKind::Cinematic,"cinematic")!=game_target(new,rvn_core::scene_resources::TargetKind::Cinematic,"cinematic")) && !pending.iter().any(|command|matches!(command,VnCommand::ShowCinematic{..}|VnCommand::HideCinematic{..})) {
        commands.push(match &new.cinematic.current {Some(id)=>VnCommand::ShowCinematic{id:id.clone(),transition:None},None=>VnCommand::HideCinematic{transition:None}});
    }
    if old.is_none_or(|old|old.music.current_file!=new.music.current_file
        || game_target(old,rvn_core::scene_resources::TargetKind::Music,"music")!=game_target(new,rvn_core::scene_resources::TargetKind::Music,"music")) && !pending.iter().any(|command|matches!(command,VnCommand::MusicPlay{..}|VnCommand::MusicStop)) {
        commands.push(match &new.music.current_file {Some(file)=>VnCommand::MusicPlay{file:file.clone(),transition:transition.clone()},None=>VnCommand::MusicStop});
    }
    if old.is_none_or(|old|old.music.volume!=new.music.volume) && !pending.iter().any(|command|matches!(command,VnCommand::MusicSetVolume{..})) {commands.push(VnCommand::MusicSetVolume{level:new.music.volume});}
    if old.is_none_or(|old|old.typewriter!=new.typewriter) && !pending.iter().any(|command|matches!(command,VnCommand::SetTypewriterConfig{..})) {commands.push(VnCommand::SetTypewriterConfig{speed_cps:new.typewriter.effective_speed()});}
    if old.is_none_or(|old|old.accessibility!=new.accessibility) && !pending.iter().any(|command|matches!(command,VnCommand::Accessibility(_))) {commands.push(VnCommand::Accessibility(new.accessibility.clone()));}
    commands
}
fn game_target<'a>(state:&'a rvn_core::GameState,kind:rvn_core::scene_resources::TargetKind,alias:&str)->Option<&'a rvn_core::scene_resources::TargetIdentity> {
    state.audio_resource_registry()?.target(&rvn_core::scene_tasks::TaskOwner::Game,kind,alias)
}

pub(crate) fn display_command(engine:&VnEngine, display: rvn_core::Interaction) -> VnCommand {
    match display {
        rvn_core::Interaction::Dialogue { character, text } => crate::display_reveal::command(engine,character,text),
        rvn_core::Interaction::Choice { options } => VnCommand::ShowChoice { options },
        rvn_core::Interaction::Imagemap { background, hover_image, hotspots } => VnCommand::ShowImagemap {
            background, hover_image, hotspots: hotspots.iter().map(crate::vn_command::ImagemapZone::from).collect(),
        },
    }
}

#[cfg(test)]
#[path = "character_publication_tests.rs"]
mod character_publication_tests;

#[cfg(test)]
mod tests {
// Private proposal inside scene_publication::tests, not installed or executed.
// It checks real rollback/presentation authority, not toolbar clicking or GPU.
#[test]
fn editor_preview_0031_real_child_rollback_installs_an_admissible_parent_dialogue() {
    let mut game = engine("init{set score=3}\nscene label Parent\nlocal value=11\n\"Parent [value].\"\ncall Child\n\"Returned [value].\"\nscene label Child\nlocal value=201\n\"Child [value].\"\nreturn\n");
    game.enter_label("Parent").unwrap();
    assert!(matches!(game.step_until_interaction().unwrap(),Some(rvn_core::Interaction::Dialogue{text,..}) if text=="Parent 11."));
    let parent_position = game.preview_input_source().unwrap().position.clone();
    let mut app = app(game);
    app.init_resource::<crate::editor_preview::PreviewState>();
    consume(app.world_mut());
    {
        let mut game = app.world_mut().resource_mut::<VnEngine>();
        game.0.advance_dialogue().unwrap();
        assert!(matches!(game.0.step_until_interaction().unwrap(),Some(rvn_core::Interaction::Dialogue{text,..}) if text=="Child 201."));
    }
    consume(app.world_mut());
    let child = app.world().resource::<crate::editor_preview::InstalledScope>().0.clone();
    assert_eq!(child.parked.len(),1);
    assert_ne!(child.position,parent_position);
    assert!(crate::systems::input::rollback_game(&mut app.world_mut().resource_mut::<VnEngine>().0));
    assert!(app.world().resource::<VnEngine>().0.renderer.scene_batch_pending());
    // Pending restored Core must not relabel the installed child's old image.
    assert_eq!(app.world().resource::<crate::editor_preview::InstalledScope>().0.position,child.position);
    consume(app.world_mut());
    let game = &app.world().resource::<VnEngine>().0;
    assert!(matches!(game.displayed_interaction().unwrap(),Some(rvn_core::Interaction::Dialogue{text,..}) if text=="Parent 11."));
    let source = game.preview_input_source().expect("restored parent must retain genuine interaction authority");
    assert!(game.accepts_preview_input_source(&source));
    let installed = &app.world().resource::<crate::editor_preview::InstalledScope>().0;
    assert_eq!(installed.position,parent_position);
    assert_eq!(installed.interaction.as_ref(),Some(&source));
    assert!(installed.parked.is_empty());
    assert_eq!(installed.instance,game.instance_id());
    assert_eq!(installed.epoch,game.runtime_epoch());
    assert!(!game.renderer.scene_batch_pending() && !game.renderer.scene_frame_failed);
    assert!(app.world().resource::<crate::resources::ScriptErrorMessage>().0.is_empty());
}

    use super::*;
    #[test]
    fn editor_preview_explicit_dialogue_followed_directly_by_choice_publishes_actual_new_position() {
        use bevy::ecs::system::RunSystemOnce;
        struct OwnedRoot(std::path::PathBuf);
        impl Drop for OwnedRoot {
            fn drop(&mut self) {
                if let Ok(root) = self.0.canonicalize() {
                    let temp = std::env::temp_dir().canonicalize().unwrap();
                    assert!(root.starts_with(&temp) && root != temp);
                    std::fs::remove_dir_all(root).unwrap();
                }
            }
        }
        let root = OwnedRoot(std::env::temp_dir().join(format!("rvn-explicit-presentation-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos())));
        let manager = rvn_core::persistent::PersistentDataManager::new(&root.0).unwrap();
        let mut game = engine("init {set result=0}\nscene label P\n\"first\"\nchoice {\"A\"=>{set result=10} \"B\"=>{set result=20}}\n\"after\"\n");
        game.enter_label("P").unwrap();
        game.step_until_interaction().unwrap();
        let mut app = app(game);
        app.init_resource::<crate::editor_preview::PreviewState>()
            .init_resource::<crate::systems::debug_overlay::DebugOverlayState>()
            .init_resource::<crate::systems::debug_overlay::DebugStepRequest>()
            .init_resource::<crate::save_thumbnails::SaveThumbnails>()
            .insert_resource(crate::project_paths::ProjectPaths::new(root.0.clone(), root.0.clone(), root.0.clone(), root.0.clone(), root.0.clone()))
            .insert_resource(crate::resources::PersistentDataResource { manager, data: rvn_core::persistent::PersistentData::default() });
        consume(app.world_mut());
        let before = app.world().resource::<crate::editor_preview::InstalledScope>().0.clone();
        app.world_mut().resource_mut::<VnEngine>().0.advance_dialogue().unwrap();
        app.world_mut().run_system_once(crate::systems::stepping_system);
        let current = &app.world().resource::<VnEngine>().0;
        let expected = current.state.scene_execution.as_ref().unwrap().live_position.clone();
        let source = current.preview_input_source().unwrap();
        assert_ne!(expected, before.position);
        assert!(matches!(current.displayed_interaction().unwrap(), Some(rvn_core::Interaction::Choice { options }) if options == vec!["A", "B"]));
        // Pending Core state must not label the previously installed pixels.
        assert_eq!(app.world().resource::<crate::editor_preview::InstalledScope>().0.position, before.position);
        consume(app.world_mut());
        let installed = &app.world().resource::<crate::editor_preview::InstalledScope>().0;
        assert_eq!(installed.position, expected, "a direct choice must replace the previous dialogue's installed preview position");
        assert_eq!(installed.instance, before.instance);
        assert_eq!(installed.epoch, before.epoch);
        assert_eq!(installed.interaction.as_ref(), Some(&source));
        let published = app.world().resource::<ScenePresentation>().batch.as_ref().unwrap();
        assert_eq!(published.state.scene_execution.as_ref().unwrap().live_position, expected);
        assert!(matches!(&published.display, Some(rvn_core::Interaction::Choice { options }) if options == &vec!["A", "B"]));
        assert!(app.world().resource::<Events<VnCommand>>().iter_current_update_events().any(|event|matches!(event,VnCommand::ShowChoice { options } if options == &vec!["A", "B"])));
        assert!(!app.world().resource::<VnEngine>().0.renderer.scene_batch_pending());
        assert!(!app.world().resource::<VnEngine>().0.renderer.scene_frame_failed);
    }

    fn engine(source: &str) -> rvn_core::Engine<crate::bevy_renderer::BevyRenderer> {
        rvn_core::Engine::from_program(rvn_parser::parse_program(source).unwrap(),crate::bevy_renderer::BevyRenderer::new(),32).unwrap()
    }
    fn app(engine: rvn_core::Engine<crate::bevy_renderer::BevyRenderer>) -> App {
        let mut app = App::new();
        app.insert_resource(VnEngine(engine)).insert_resource(State::new(crate::resources::VnState::Waiting))
            .init_resource::<NextState<crate::resources::VnState>>().insert_resource(Time::<()>::default())
            .init_resource::<crate::accessibility::Accessibility>().init_resource::<crate::resources::ScriptErrorMessage>()
            .init_resource::<VnRenderState>().init_resource::<ChoiceFocus>().init_resource::<ImagemapState>().init_resource::<TypewriterState>()
            .add_event::<VnCommand>().add_event::<PlayerInput>().add_plugins(ScenePublicationPlugin);
        app
    }

    #[test]
    fn editor_pause_keeps_actual_game_frame_timer_and_story_snapshot_unchanged_until_resume(){
        let mut game=engine("scene label P\ntimer 10 => jump End\n\"Waiting\"\nscene label End\n\"Done\"\n");
        game.enter_label("P").unwrap();game.step_until_interaction().unwrap();
        let mut app=app(game);app.insert_resource(Time::<bevy::time::Virtual>::default());
        app.init_resource::<crate::editor_preview::PreviewState>();app.update();
        let instance=app.world().resource::<VnEngine>().0.instance_id();
        let epoch=app.world().resource::<VnEngine>().0.runtime_epoch();
        let before=serde_json::to_vec(&app.world().resource::<VnEngine>().0.state).unwrap();
        crate::editor_preview::set_paused(app.world_mut(),true).unwrap();
        for _ in 0..8{
            app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_millis(250));app.update();
            assert!(!game_active(app.world()));assert!(!host_active(app.world()));
            assert_eq!(serde_json::to_vec(&app.world().resource::<VnEngine>().0.state).unwrap(),before);
        }
        assert_eq!(app.world().resource::<VnEngine>().0.instance_id(),instance);
        assert_eq!(app.world().resource::<VnEngine>().0.runtime_epoch(),epoch);
        crate::editor_preview::set_paused(app.world_mut(),false).unwrap();app.update();
        assert!(game_active(app.world()));
        assert_ne!(serde_json::to_vec(&app.world().resource::<VnEngine>().0.state).unwrap(),before);
        assert!(matches!(app.world().resource::<VnEngine>().0.displayed_interaction().unwrap(),Some(rvn_core::Interaction::Dialogue{text,..})if text=="Waiting"));
    }
    #[test]
    fn authored_typewriter_publication_reaches_actual_config_consumer_without_resetting_partial_text() {
        let mut game = engine("scene label P\n\"before\"\ntypewriter.speed(17)\n\"speed published\"\ntypewriter = false\n\"after\"\n");
        game.enter_label("P").unwrap();
        game.step_until_interaction().unwrap();
        let mut app = app(game);
        app.init_resource::<crate::resources::TypewriterConfig>();
        app.add_systems(Update, crate::systems::typewriter_config_system);
        app.update();
        // Observe the next publication by ordinal, independent of First event-buffer rotation.
        let mut publication_reader=app.world().resource::<Events<VnCommand>>().get_reader_current();
        {
            let mut typing = app.world_mut().resource_mut::<TypewriterState>();
            typing.full_text = "before".into();
            typing.visible_chars = 3;
            typing.char_progress = 0.375;
        }
        {
            let mut engine = app.world_mut().resource_mut::<VnEngine>();
            engine.0.advance_dialogue().unwrap();
            engine.0.step_silent().unwrap();
            assert!(engine.0.renderer.scene_batch_pending());
            assert!(engine.0.state.typewriter.enabled);
            assert_eq!(engine.0.state.typewriter.chars_per_sec, 17);
        }
        app.update();
        assert!(!app.world().resource::<VnEngine>().0.renderer.scene_frame_failed,
            "{}", app.world().resource::<crate::resources::ScriptErrorMessage>().0);
        assert!(app.world().resource::<ScenePresentation>().batch.as_ref().unwrap().display.is_none());
        let config = app.world().resource::<crate::resources::TypewriterConfig>();
        assert!(config.enabled);
        assert_eq!(config.chars_per_sec.to_bits(), 17.0f32.to_bits());
        assert_eq!(publication_reader.read(app.world().resource::<Events<VnCommand>>())
            .filter(|event| matches!(event, VnCommand::SetTypewriterConfig { speed_cps } if *speed_cps == 17.0)).count(), 1);
        let typing = app.world().resource::<TypewriterState>();
        assert_eq!(typing.full_text, "before");
        assert_eq!(typing.visible_chars, 3);
        assert_eq!(typing.char_progress.to_bits(), 0.375f32.to_bits());
        {
            let mut engine = app.world_mut().resource_mut::<VnEngine>();
            engine.0.advance_dialogue().unwrap();
            engine.0.step_silent().unwrap();
            assert!(!engine.0.state.typewriter.enabled);
            assert!(engine.0.renderer.scene_batch_pending());
        }
        app.update();
        let config = app.world().resource::<crate::resources::TypewriterConfig>();
        assert!(!config.enabled);
        assert_eq!(config.chars_per_sec.to_bits(), 0.0f32.to_bits());
        let typing = app.world().resource::<TypewriterState>();
        assert_eq!(typing.visible_chars, 3);
        assert_eq!(typing.char_progress.to_bits(), 0.375f32.to_bits());
    }
    #[test]
    fn same_file_game_replacements_emit_one_new_presentation_for_each_new_incarnation() {
        use rvn_core::{scene_resources::TargetKind,scene_tasks::TaskOwner};
        let mut old=engine("scene label start\n\"line\"").state;
        old.background_image="room.png".into();old.music.current_file=Some("song.ogg".into());
        old.sprites.insert("hero".into(),rvn_core::SpriteState::new(None,rvn_parser::Position::Left));
        let registry=&mut old.scene_execution.as_mut().unwrap().resources;
        for (kind,alias) in [(TargetKind::Background,"background"),(TargetKind::Music,"music"),(TargetKind::Sprite,"hero")] {
            registry.ensure_target(TaskOwner::Game,kind,alias).unwrap();
        }
        assert!(presentation_difference(Some(&old),&old,&[],false).is_empty());
        let mut replaced=old.clone();
        for (kind,alias) in [(TargetKind::Background,"background"),(TargetKind::Music,"music"),(TargetKind::Sprite,"hero")] {
            replaced.scene_execution.as_mut().unwrap().resources.replace_target(TaskOwner::Game,kind,alias).unwrap();
        }
        let commands=presentation_difference(Some(&old),&replaced,&[],false);
        assert_eq!(commands.len(),3);
        assert_eq!(commands.iter().filter(|command|matches!(command,VnCommand::SetBackground{path,..} if path=="room.png")).count(),1);
        assert_eq!(commands.iter().filter(|command|matches!(command,VnCommand::MusicPlay{file,..} if file=="song.ogg")).count(),1);
        assert_eq!(commands.iter().filter(|command|matches!(command,VnCommand::ShowSprite{id,..} if id=="hero")).count(),1);
        assert!(presentation_difference(Some(&old),&replaced,&commands,false).is_empty());
    }
    #[test]
    fn a_real_scene_publication_replaces_models_and_revokes_queued_input_before_clocks() {
        let mut engine = engine("scene label start\nlocal score = 2\n\"[score]\"\nscene label next\nlocal score = 7\n\"[score]\"");
        engine.enter_label("start").unwrap();
        let mut app = app(engine);
        app.world_mut().send_event(PlayerInput::Choose(0));
        app.world_mut().send_event(VnCommand::ShowChoice { options: vec!["old".into()] });
        app.update();
        assert!(!app.world().resource::<VnEngine>().0.renderer.scene_batch_pending());
        assert_eq!(app.world().resource::<Events<PlayerInput>>().len(),0);
        assert!(!app.world().resource::<Events<VnCommand>>().iter_current_update_events().any(|event|matches!(event,VnCommand::ShowChoice{..})));
        let snapshot = app.world().resource::<ScenePresentation>().batch.as_ref().unwrap();
        assert_eq!(snapshot.runtime_epoch,app.world().resource::<VnEngine>().0.runtime_epoch());
        assert_eq!(serde_json::to_value(&snapshot.state).unwrap(),serde_json::to_value(&app.world().resource::<VnEngine>().0.state).unwrap());
        assert_eq!(snapshot.interfaces,app.world().resource::<crate::programmable_ui::Screens>().views);
        assert!(snapshot.motions.is_empty() && snapshot.layered.is_empty() && snapshot.videos.is_empty());
    }
    #[derive(Resource,Default)]
    struct Mutations(usize);
    #[derive(Resource,Default)]
    struct Transferred(bool);
    #[derive(Resource,Default)]
    struct PendingProbe(bool);
    fn probe_pending(engine:Res<VnEngine>,mut probe:ResMut<PendingProbe>) {probe.0=engine.0.renderer.scene_batch_pending();}
    fn transfer(mut engine: ResMut<VnEngine>,mut transferred:ResMut<Transferred>) { if !transferred.0 {engine.0.enter_label("next").unwrap();transferred.0=true;} }
    fn mutate(mut mutations: ResMut<Mutations>) { mutations.0 += 1; }
    #[derive(Resource,Default)]
    struct ClockSamples(Vec<(Option<rvn_core::scene::ActivationId>,f64)>);
    fn record_game_clock(time:Res<Time>,engine:Res<VnEngine>,mut samples:ResMut<ClockSamples>) {
        samples.0.push((owner(&engine.0.state),time.delta_seconds_f64()));
    }
    #[test]
    fn every_host_clock_observes_the_same_frame_owner_and_delta_before_a_feedback_transfer() {
        let mut game=engine("scene label start\n\"parent\"\nscene label next\n\"child\"");
        game.enter_label("start").unwrap();game.step_until_interaction().unwrap();
        let entry=owner(&game.state);
        let mut app=app(game);app.init_resource::<ClockSamples>().init_resource::<Transferred>();
        app.add_systems(Update,(
            record_game_clock.run_if(ready).in_set(HostClockSet),
            record_game_clock.run_if(ready).in_set(HostClockSet),
            transfer.in_set(MotionFeedbackSet),
        ));
        app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs(1));
        app.update();
        assert_eq!(app.world().resource::<ClockSamples>().0,vec![(entry,1.0),(entry,1.0)]);
        assert_ne!(owner(&app.world().resource::<VnEngine>().0.state),entry);
        assert!(!app.world().resource::<VnEngine>().0.renderer.scene_batch_pending());
        assert_eq!(owner(&app.world().resource::<ScenePresentation>().batch.as_ref().unwrap().state),owner(&app.world().resource::<VnEngine>().0.state));
    }
    #[test]
    fn a_transfer_mid_frame_blocks_the_next_mutating_system_until_exclusive_consumption() {
        let mut engine = engine("scene label start\n\"old\"\nscene label next\n\"new\"");
        engine.enter_label("start").unwrap();
        let mut app = app(engine); app.init_resource::<Mutations>().init_resource::<Transferred>().init_resource::<PendingProbe>();
        // The probe and guarded mutation must not have a scheduled consumer
        // between them. Ordering after input alone leaves the other exclusive
        // consumers free to install the transfer before the condition runs.
        app.add_systems(Update,(transfer,probe_pending,mutate.run_if(story_ready)).chain()
            .after(consume_after_input).after(consume_after_motions)
            .after(consume_after_canvases).after(consume_after_video)
            .after(consume_after_tasks).after(consume_after_story)
            .after(consume_after_presenters));
        app.update();
        assert_eq!(app.world().resource::<Mutations>().0,0);
        assert!(app.world().resource::<PendingProbe>().0);
        consume(app.world_mut());
        app.update();
        assert_eq!(app.world().resource::<Mutations>().0,1);
    }
    #[test]
    fn same_owner_publications_preserve_the_live_screen_focus_and_typewriter_clock() {
        let mut game=engine("screen HUD(){return component(\"act\",\"button\",{\"events\":{\"click\":\"refresh\"}},[])}\nhandler refresh(event){set update=4}\ninit {set update=0}\nscene label start\nui.open(\"HUD\",[],false,0)\n\"waiting\"\n");
        game.enter_label("start").unwrap();game.step_until_interaction().unwrap();
        let owner=game.state.scene_execution.as_ref().unwrap().owner();
        let mut app=app(game);app.update();assert_live_display(app.world());
        app.world_mut().resource_mut::<crate::programmable_ui::Screens>().keyboard_focus=Some(("HUD".into(),"act".into()));
        {let mut typing=app.world_mut().resource_mut::<TypewriterState>();typing.full_text="waiting".into();typing.visible_chars=3;typing.char_progress=0.25;}
        let order=app.world().resource::<crate::programmable_ui::Screens>().views[0].order;
        app.world_mut().resource_mut::<VnEngine>().0.interface_event(rvn_core::ui::UiInput{screen:"HUD".into(),element:"act".into(),kind:rvn_ui::programmable::ScreenEventKind::Click,value:None,key:None}).unwrap();
        app.update();
        assert_eq!(app.world().resource::<VnEngine>().0.state.scene_execution.as_ref().unwrap().owner(),owner);
        assert_eq!(app.world().resource::<VnEngine>().0.state.vars["update"],rvn_parser::Value::Int(4));
        assert_live_display(app.world());
        let screens=app.world().resource::<crate::programmable_ui::Screens>();assert_eq!(screens.views[0].order,order);
        assert_eq!(screens.keyboard_focus,Some(("HUD".into(),"act".into())));
        let typing=app.world().resource::<TypewriterState>();assert_eq!(typing.full_text,"waiting");assert_eq!(typing.visible_chars,3);assert_eq!(typing.char_progress,0.25);
    }
    #[test]
    fn actual_scene_timers_pause_in_the_game_menu_and_resume_on_the_same_owner() {
        let mut engine = engine("scene label start\ntimer 10 => jump next\n\"Waiting\"\nscene label next\n\"Done\"");
        engine.enter_label("start").unwrap(); engine.step_until_interaction().unwrap();
        let owner = engine.state.scene_execution.as_ref().unwrap().owner();
        let mut app = app(engine);
        app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs(1));
        app.update();
        let elapsed = |app:&App| app.world().resource::<VnEngine>().0.state.scene_execution.as_ref().unwrap().active.as_ref().unwrap().tasks.timer.as_ref().unwrap().elapsed;
        let first = elapsed(&app); assert_eq!(first,1.0);
        app.world_mut().insert_resource(State::new(crate::resources::VnState::Menu));
        app.update(); app.update(); assert_eq!(elapsed(&app),first);
        app.world_mut().insert_resource(State::new(crate::resources::VnState::Waiting));
        app.update(); assert_eq!(elapsed(&app),2.0);
        assert_eq!(app.world().resource::<VnEngine>().0.state.scene_execution.as_ref().unwrap().owner(),owner);
    }

    fn assert_live_display(world:&World) {
        assert_eq!(world.resource::<ScenePresentation>().batch.as_ref().unwrap().display,
            world.resource::<VnEngine>().0.displayed_interaction().unwrap());
    }
    fn prepared_app(source:&str)->App {
        let mut game=engine(source);game.step_until_interaction().unwrap();
        let mut app=app(game);consume(app.world_mut());assert_live_display(app.world());app
    }
    fn state_and_history(world:&World)->serde_json::Value {
        let game=&world.resource::<VnEngine>().0;
        serde_json::json!({"state":&game.state,"history":game.history.entries().iter().map(|entry|(&entry.state,&entry.display)).collect::<Vec<_>>(),
            "runtime_epoch":game.runtime_epoch(),"interface_epoch":game.interface_epoch(),"video_views":format!("{:?}",game.video_views().unwrap())})
    }
    fn models_and_output(world:&World)->String {
        format!("{:?}|{:?}|{:?}|{:?}",world.resource::<ScenePresentation>().batch,
            world.resource::<crate::programmable_ui::Screens>().views,world.resource::<VnEngine>().0.renderer.scene_batch,
            world.resource::<VnEngine>().0.renderer.pending)
    }
    fn queue_bits(world:&World)->Vec<(rvn_core::video::VideoView,u8,u64,u64,String)> {
        world.resource::<VideoFeedbackQueue>().0.iter().map(|(view,report)|{
            let (kind,a,b,text)=match report {
                VideoFeedback::Report(rvn_core::video::Feedback::Position{seconds,duration})=>(0,seconds.to_bits(),duration.to_bits(),String::new()),
                VideoFeedback::Report(rvn_core::video::Feedback::End)=>(1,0,0,String::new()),
                VideoFeedback::Report(rvn_core::video::Feedback::Error(error))=>(2,0,0,error.clone()),
                VideoFeedback::Report(rvn_core::video::Feedback::AutoplayBlocked(error))=>(3,0,0,error.clone()),
                VideoFeedback::Resume=>(4,0,0,String::new()),
            };(view.clone(),kind,a,b,text)
        }).collect()
    }
    fn view(world:&World,id:&str)->rvn_core::video::VideoView {
        world.resource::<VnEngine>().0.video_views().unwrap().into_iter().find(|view|view.id==id).unwrap()
    }
    const FRAME_CANVAS:&str=r#"
function paint(state,props,frame){return []}
screen custom(){return component("root","column",{},[component("one","canvas",{"draw":"paint","state":{"ticks":0}},[]),component("open","button",{"events":{"click":"open_modal"}},[])])}
screen host(ctx){return component("host_canvas","canvas",{"draw":"paint","state":{"ticks":0}},[])}
screen overlay(){return component("modal","text",{"text":"modal"},[])}
handler open_modal(event){ui.open("overlay",[],true,10)}
scene "parent.png"
scene label P
ui.open("custom",[],false,0)
eileen.show("smile") at left
eileen.animate("pulse",duration:10)
motion.play("background",motion_pause(10))
timer 5 => jump finish
"parent"
jump finish
scene label C
"child"
return
label finish
"finished"
"#;
    #[test]
    fn coordinator_advances_each_entry_clock_once_before_game_timer_parks_the_parent() {
        let mut app=prepared_app(FRAME_CANVAS);
        let parent=owner(&app.world().resource::<VnEngine>().0.state).unwrap();
        app.world_mut().resource_mut::<VnEngine>().0.state.scene_execution.as_mut().unwrap().game_tasks.set_timer(1.0,"call C".into()).unwrap();
        app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs(1));
        game_frame(app.world_mut());
        let game=&app.world().resource::<VnEngine>().0;let execution=game.state.scene_execution.as_ref().unwrap();
        assert_ne!(execution.owner(),Some(parent));
        let parked=execution.frames.last().unwrap().parked.as_ref().unwrap();
        assert_eq!(parked.tasks.timer.as_ref().unwrap().elapsed,1.0);
        assert_eq!(parked.tasks.suspended_motions.tracks["background"].elapsed,1.0);
        assert_eq!(parked.tasks.suspended_ui.screens[0].canvas_states["one"].elapsed,0.25);
        let effect=execution.resources.tasks.iter().find(|task|task.key.owner==rvn_core::scene_tasks::TaskOwner::Scene{activation:parent} && task.effect.is_some()).unwrap();
        assert!(effect.parked);assert_eq!(effect.effect.as_ref().unwrap().clock.elapsed,1.0);
        assert!(execution.active.as_ref().unwrap().tasks.timer.is_none());
        assert!(!game.renderer.scene_batch_pending());
        assert_eq!(owner(&app.world().resource::<ScenePresentation>().batch.as_ref().unwrap().state),execution.owner());
    }
    #[test]
    fn coordinator_validates_all_video_positions_before_ordered_terminal_callbacks() {
        use rvn_core::video::Feedback;
        let mut app=prepared_app("init {set ends=0}\nhandler done(event){set ends=ends+1 video.stop(\"b\")}\nscene label P\nvideo.play(\"a\",video_clip(\"a.webm\",{\"on_end\":\"done\"}))\nvideo.play(\"b\",video_clip(\"b.webm\",{}))\n\"P\"\n");
        let a=view(app.world(),"a");let b=view(app.world(),"b");
        assert!(queue_video_feedback(app.world_mut(),&a,Feedback::End));
        assert!(queue_video_feedback(app.world_mut(),&b,Feedback::Position{seconds:0.7,duration:2.0}));
        assert!(queue_video_feedback(app.world_mut(),&a,Feedback::Position{seconds:0.4,duration:2.0}));
        game_frame(app.world_mut());
        let game=&app.world().resource::<VnEngine>().0;
        assert_eq!(game.state.vars["ends"],rvn_parser::Value::Int(1));
        assert!(!game.state.videos.tracks.contains_key("b"));assert_eq!(game.state.videos.tracks["a"].position,2.0);
        assert!(app.world().resource::<VideoFeedbackQueue>().0.is_empty());
        assert!(!game.renderer.scene_frame_failed && !game.renderer.scene_batch_pending());
        assert_eq!(serde_json::to_value(&app.world().resource::<ScenePresentation>().batch.as_ref().unwrap().state).unwrap(),serde_json::to_value(&game.state).unwrap());
    }
    #[test]
    fn coordinator_pause_focus_accessibility_and_host_modal_gates_leave_story_modal_clocks_live() {
        let mut app=prepared_app(FRAME_CANVAS);
        let window=app.world_mut().spawn((Window::default(),bevy::window::PrimaryWindow)).id();
        app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_millis(100));
        for role in [rvn_ui::PageRole::Dialogue,rvn_ui::PageRole::Choices,rvn_ui::PageRole::QuickActions] {
            app.world_mut().insert_resource(crate::source_menus::SourceMenus::for_test_role(role));assert!(game_active(app.world()));
        }
        for role in [rvn_ui::PageRole::Pause,rvn_ui::PageRole::Confirm] {
            app.world_mut().insert_resource(crate::source_menus::SourceMenus::for_test_role(role));assert!(!game_active(app.world()));
            let before=state_and_history(app.world());game_frame(app.world_mut());assert_eq!(state_and_history(app.world()),before);
        }
        app.world_mut().remove_resource::<crate::source_menus::SourceMenus>();
        app.world_mut().insert_resource(State::new(crate::resources::VnState::Menu));
        let before=state_and_history(app.world());game_frame(app.world_mut());assert_eq!(state_and_history(app.world()),before);
        app.world_mut().insert_resource(State::new(crate::resources::VnState::Waiting));
        app.world_mut().resource_mut::<crate::accessibility::Accessibility>().blocked=true;
        game_frame(app.world_mut());assert_eq!(state_and_history(app.world()),before);
        app.world_mut().resource_mut::<crate::accessibility::Accessibility>().blocked=false;
        app.world_mut().get_mut::<Window>(window).unwrap().focused=false;
        game_frame(app.world_mut());assert_eq!(state_and_history(app.world()),before);
        app.world_mut().get_mut::<Window>(window).unwrap().focused=true;
        app.world_mut().resource_mut::<VnEngine>().0.interface_event(rvn_core::ui::UiInput{screen:"custom".into(),element:"open".into(),kind:rvn_ui::programmable::ScreenEventKind::Click,value:None,key:None}).unwrap();consume(app.world_mut());
        assert!(game_active(app.world()));game_frame(app.world_mut());
        let game=&app.world().resource::<VnEngine>().0;let active=game.state.scene_execution.as_ref().unwrap().active.as_ref().unwrap();
        assert!((active.tasks.timer.as_ref().unwrap().elapsed-0.1).abs()<1e-12);
        assert!((game.state.motions.tracks["background"].elapsed-0.1).abs()<1e-12);
    }
    #[test]
    fn coordinator_host_canvas_ticks_in_menu_but_failure_latch_stops_automatic_host_mutations() {
        let mut app=prepared_app(FRAME_CANVAS);
        let context=rvn_core::ui::value_from_json(&rvn_ui::source_menus::preview_context(rvn_ui::PageRole::Pause,true)).unwrap();
        app.world_mut().resource_mut::<VnEngine>().0.synchronize_source_menu(rvn_ui::PageRole::Pause,Some("host"),context,true,500).unwrap();consume(app.world_mut());
        app.world_mut().insert_resource(crate::source_menus::SourceMenus::for_test_role(rvn_ui::PageRole::Pause));
        app.world_mut().insert_resource(State::new(crate::resources::VnState::Menu));
        app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_millis(100));
        app.add_systems(Update,crate::custom_canvas::tick.in_set(HostClockSet));app.update();
        let game=&app.world().resource::<VnEngine>().0;
        let canvas=|name:&str,element:&str|game.state.ui.screens.iter().find(|screen|screen.name==name).unwrap().canvas_states[element].elapsed;
        assert!((canvas("host","host_canvas")-0.1).abs()<1e-8);assert_eq!(canvas("custom","one"),0.0);
        assert_eq!(game.state.motions.tracks["background"].elapsed,0.0);
        app.world_mut().resource_mut::<VnEngine>().0.renderer.scene_frame_failed=true;
        let before=state_and_history(app.world());app.update();assert_eq!(state_and_history(app.world()),before);
        // The guard is on automatic ticks, not the explicit recovery API.
        let save=rvn_core::save::SaveData::try_from_state(&app.world().resource::<VnEngine>().0.state,1,"recover".into(),"main.rvn".into()).unwrap();
        app.world_mut().resource_mut::<VnEngine>().0.load_data(save).unwrap();consume(app.world_mut());
        assert!(!app.world().resource::<VnEngine>().0.renderer.scene_frame_failed);
    }
    fn attach_unresolved_animation(app:&mut App,id:&str,nan:bool)->Entity {
        use crate::components::{AnimationKind,SpriteAnimation,SpriteBaseTransform,VnSprite};
        let token=app.world().resource::<VnEngine>().0.scene_resource_token(rvn_core::scene_resources::ResourceTaskKind::Effect,&format!("sprite/{id}/animation")).unwrap();
        let mut pose=Transform::from_xyz(12.0,23.0,0.0);if nan {pose.translation.x=f32::NAN;}
        app.world_mut().spawn((VnSprite{id:id.into()},SpriteBaseTransform{translation:pose.translation,scale:pose.scale},pose,
            SpriteAnimation{kind:AnimationKind::Pulse{scale:1.1},duration_secs:10.0,elapsed_secs:0.0,looping:false},crate::scene_effects::AnimationLease(token))).id()
    }
    fn unresolved_app()->App {
        prepared_app("scene label P\neileen.show(\"smile\") at left\neileen.animate(\"pulse\",duration:10)\nlucy.show(\"smile\") at right\nlucy.animate(\"pulse\",duration:10)\nvideo.play(\"a\",video_clip(\"a.webm\",{}))\n\"P\"\n")
    }
    #[test]
    fn coordinator_rolls_back_first_measured_base_when_second_current_base_is_invalid() {
        use rvn_core::scene_resources::{EffectProjectionBase,TransformSnapshot};
        let mut app=unresolved_app();let first=attach_unresolved_animation(&mut app,"eileen",false);let second=attach_unresolved_animation(&mut app,"lucy",true);
        let token=app.world().get::<crate::scene_effects::AnimationLease>(first).unwrap().0.clone();
        let original=app.world().resource::<VnEngine>().0.state.clone();
        assert!(app.world_mut().resource_mut::<VnEngine>().0.capture_scene_effect_base(&token,EffectProjectionBase::SpriteTransform(TransformSnapshot{translation:[12.0,23.0,0.0],..default()})).unwrap());
        assert_ne!(serde_json::to_value(&app.world().resource::<VnEngine>().0.state).unwrap(),serde_json::to_value(&original).unwrap());
        app.world_mut().resource_mut::<VnEngine>().0.state=original;
        let before=state_and_history(app.world());let models=models_and_output(app.world());let queue=queue_bits(app.world());
        let nan_bits=app.world().get::<Transform>(second).unwrap().translation.x.to_bits();
        game_frame(app.world_mut());
        assert_eq!(state_and_history(app.world()),before);assert_eq!(models_and_output(app.world()),models);assert_eq!(queue_bits(app.world()),queue);
        assert_eq!(app.world().get::<Transform>(second).unwrap().translation.x.to_bits(),nan_bits);
        assert_eq!(app.world().get::<crate::components::SpriteAnimation>(first).unwrap().elapsed_secs,0.0);
        assert!(app.world().resource::<VnEngine>().0.renderer.scene_frame_failed);
        assert!(!app.world().resource::<crate::resources::ScriptErrorMessage>().0.is_empty());
        game_frame(app.world_mut());assert_eq!(state_and_history(app.world()),before);
    }
    #[test]
    fn coordinator_retains_nan_report_bits_and_rolls_back_measured_bases_until_new_epoch_load() {
        use rvn_core::video::Feedback;
        let mut app=unresolved_app();attach_unresolved_animation(&mut app,"eileen",false);attach_unresolved_animation(&mut app,"lucy",false);
        let a=view(app.world(),"a");let payload=f64::from_bits(0x7ff8_0000_0000_0321);
        assert!(queue_video_feedback(app.world_mut(),&a,Feedback::Position{seconds:payload,duration:2.0}));
        let before=state_and_history(app.world());let models=models_and_output(app.world());let queued=queue_bits(app.world());
        game_frame(app.world_mut());assert_eq!(state_and_history(app.world()),before);assert_eq!(models_and_output(app.world()),models);assert_eq!(queue_bits(app.world()),queued);
        assert!(app.world().resource::<VnEngine>().0.renderer.scene_frame_failed);
        app.world_mut().insert_resource(State::new(crate::resources::VnState::Waiting));
        game_frame(app.world_mut());assert_eq!(state_and_history(app.world()),before);assert_eq!(queue_bits(app.world()),queued);
        let save=rvn_core::save::SaveData::try_from_state(&app.world().resource::<VnEngine>().0.state,1,"restore".into(),"main.rvn".into()).unwrap();
        app.world_mut().resource_mut::<VnEngine>().0.load_data(save).unwrap();consume(app.world_mut());
        assert!(!app.world().resource::<VnEngine>().0.renderer.scene_frame_failed);assert!(queue_bits(app.world()).is_empty());
        // A delayed report is ignored by capability before its NaN payload.
        assert!(queue_video_feedback(app.world_mut(),&a,Feedback::Position{seconds:payload,duration:f64::NAN}));
        game_frame(app.world_mut());assert!(!app.world().resource::<VnEngine>().0.renderer.scene_frame_failed);assert!(queue_bits(app.world()).is_empty());
    }
    #[test]
    fn coordinator_queue_overflow_refuses_terminal_without_eviction_and_load_admits_fresh_token() {
        use rvn_core::video::Feedback;
        let mut app=prepared_app("scene label P\nvideo.play(\"a\",video_clip(\"a.webm\",{}))\n\"P\"\n");let old=view(app.world(),"a");
        for index in 0..MAX_QUEUED_VIDEO_REPORTS {assert!(queue_video_feedback(app.world_mut(),&old,Feedback::Position{seconds:(index as f64)/4096.0,duration:2.0}));}
        let before=queue_bits(app.world());let state=state_and_history(app.world());
        assert!(!queue_video_feedback(app.world_mut(),&old,Feedback::Error("exact pending error".into())));
        assert!(!queue_video_resume(app.world_mut(),&old));assert_eq!(queue_bits(app.world()),before);assert_eq!(state_and_history(app.world()),state);
        assert_eq!(before.len(),rvn_core::scene_resources::MAX_RESOURCES);assert!(app.world().resource::<VnEngine>().0.renderer.scene_frame_failed);
        let save=rvn_core::save::SaveData::try_from_state(&app.world().resource::<VnEngine>().0.state,1,"recover".into(),"main.rvn".into()).unwrap();
        app.world_mut().resource_mut::<VnEngine>().0.load_data(save).unwrap();consume(app.world_mut());
        let fresh=view(app.world(),"a");assert_ne!(fresh.resource_token,old.resource_token);assert!(queue_bits(app.world()).is_empty());
        assert!(queue_video_feedback(app.world_mut(),&fresh,Feedback::Position{seconds:0.2,duration:2.0}));game_frame(app.world_mut());
        assert!(!app.world().resource::<VnEngine>().0.renderer.scene_frame_failed);assert_eq!(app.world().resource::<VnEngine>().0.state.videos.tracks["a"].position,0.2);
    }
    #[test]
    fn coordinator_same_owner_clock_publication_preserves_keyboard_focus_and_partial_typewriter() {
        let mut app=prepared_app(FRAME_CANVAS);let entry_owner=owner(&app.world().resource::<VnEngine>().0.state);
        app.world_mut().resource_mut::<crate::programmable_ui::Screens>().keyboard_focus=Some(("custom".into(),"one".into()));
        {let mut typing=app.world_mut().resource_mut::<TypewriterState>();typing.full_text="parent".into();typing.visible_chars=2;typing.char_progress=0.375;}
        let order=app.world().resource::<crate::programmable_ui::Screens>().views[0].order;
        app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_millis(100));game_frame(app.world_mut());
        assert_eq!(owner(&app.world().resource::<VnEngine>().0.state),entry_owner);
        let screens=app.world().resource::<crate::programmable_ui::Screens>();assert_eq!(screens.views[0].order,order);assert_eq!(screens.keyboard_focus,Some(("custom".into(),"one".into())));
        let typing=app.world().resource::<TypewriterState>();assert_eq!(typing.full_text,"parent");assert_eq!(typing.visible_chars,2);assert_eq!(typing.char_progress,0.375);
        assert!((app.world().resource::<VnEngine>().0.state.motions.tracks["background"].elapsed-0.1).abs()<1e-12);
        assert_live_display(app.world());
    }
    fn live_reveal(app:&mut App) {
        let mut typing=app.world_mut().resource_mut::<TypewriterState>();
        typing.full_text="abcdefghijklmnop".into();typing.visible_chars=1;typing.char_progress=0.25;typing.chars_per_sec=10.0;typing.typing=true;
        drop(typing);
        app.add_systems(Update,crate::systems::typewriter_system.run_if(story_ready).run_if(story_reveal_allowed)
            .run_if(in_state(crate::resources::VnState::Waiting)).after(GameFrameSet).before(MotionFeedbackSet));
        app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_millis(100));
    }
    fn reveal_bits(world:&World)->(String,String,usize,u32,bool,u32,u32) {
        let typing=world.resource::<TypewriterState>();
        (typing.full_text.clone(),format!("{:?}",typing.segments),typing.visible_chars,typing.chars_per_sec.to_bits(),typing.typing,typing.pause_remaining.to_bits(),typing.char_progress.to_bits())
    }
    #[test]
    fn coordinator_refusal_precedes_story_reveal_and_error_state_change_cannot_retry_the_frame() {
        let mut app=unresolved_app();live_reveal(&mut app);
        let a=view(app.world(),"a");assert!(queue_video_feedback(app.world_mut(),&a,rvn_core::video::Feedback::Position{seconds:f64::NAN,duration:2.0}));
        let state=state_and_history(app.world());let typing=reveal_bits(app.world());let queued=queue_bits(app.world());
        app.update();assert_eq!(state_and_history(app.world()),state);assert_eq!(reveal_bits(app.world()),typing);assert_eq!(queue_bits(app.world()),queued);
        assert!(app.world().resource::<VnEngine>().0.renderer.scene_frame_failed);
        app.world_mut().insert_resource(State::new(crate::resources::VnState::Waiting));app.update();
        assert_eq!(reveal_bits(app.world()),typing);assert_eq!(state_and_history(app.world()),state);
        let save=rvn_core::save::SaveData::try_from_state(&app.world().resource::<VnEngine>().0.state,1,"recovered".into(),"main.rvn".into()).unwrap();
        app.world_mut().resource_mut::<VnEngine>().0.load_data(save).unwrap();consume(app.world_mut());
        assert_eq!(app.world().resource::<TypewriterState>().visible_chars,0);
        {let mut typing=app.world_mut().resource_mut::<TypewriterState>();typing.full_text="abcdefghijklmnop".into();typing.visible_chars=1;typing.char_progress=0.25;typing.chars_per_sec=10.0;typing.typing=true;}
        app.update();assert!(!app.world().resource::<VnEngine>().0.renderer.scene_frame_failed);
        assert_eq!(app.world().resource::<TypewriterState>().visible_chars,2);
    }
    #[test]
    fn opted_story_reveal_shares_focus_accessibility_and_host_modal_pause_without_catching_up() {
        let mut app=prepared_app(FRAME_CANVAS);live_reveal(&mut app);
        let window=app.world_mut().spawn((Window::default(),bevy::window::PrimaryWindow)).id();
        let before=reveal_bits(app.world());
        app.world_mut().get_mut::<Window>(window).unwrap().focused=false;app.update();assert_eq!(reveal_bits(app.world()),before);
        app.world_mut().get_mut::<Window>(window).unwrap().focused=true;
        app.world_mut().resource_mut::<crate::accessibility::Accessibility>().blocked=true;app.update();assert_eq!(reveal_bits(app.world()),before);
        app.world_mut().resource_mut::<crate::accessibility::Accessibility>().blocked=false;
        app.world_mut().insert_resource(crate::source_menus::SourceMenus::for_test_role(rvn_ui::PageRole::Pause));app.update();assert_eq!(reveal_bits(app.world()),before);
        app.world_mut().insert_resource(State::new(crate::resources::VnState::Menu));app.update();assert_eq!(reveal_bits(app.world()),before);
        app.world_mut().insert_resource(State::new(crate::resources::VnState::Waiting));
        app.world_mut().insert_resource(crate::source_menus::SourceMenus::for_test_role(rvn_ui::PageRole::Dialogue));app.update();
        assert_eq!(app.world().resource::<TypewriterState>().visible_chars,2);
        assert_eq!(app.world().resource::<TypewriterState>().char_progress,0.25);
    }

    #[test]
    fn invalid_current_display_refuses_consumption_before_pending_slot_and_output_are_removed() {
        let mut game=engine("scene label P\n\"parent\"\nscene label C\n\"child\"\n");
        game.enter_label("P").unwrap();game.step_until_interaction().unwrap();
        let mut app=app(game);consume(app.world_mut());assert_live_display(app.world());
        app.world_mut().resource_mut::<VnEngine>().0.enter_label("C").unwrap();
        app.world_mut().resource_mut::<VnEngine>().0.step_until_interaction().unwrap();
        // Adversarial current-state corruption must not consume the previously
        // preflighted slot or silently substitute its older display.
        app.world_mut().resource_mut::<VnEngine>().0.state.scene_execution.as_mut().unwrap()
            .display.as_mut().unwrap().position.pc=usize::MAX;
        let before=state_and_history(app.world());let models=models_and_output(app.world());
        let commands=app.world().resource::<Events<VnCommand>>().len();
        assert!(app.world().resource::<VnEngine>().0.renderer.scene_batch_pending());
        consume(app.world_mut());
        assert_eq!(state_and_history(app.world()),before);
        assert_eq!(models_and_output(app.world()),models);
        assert_eq!(app.world().resource::<Events<VnCommand>>().len(),commands);
        assert!(app.world().resource::<VnEngine>().0.renderer.scene_batch_pending());
        assert!(app.world().resource::<VnEngine>().0.renderer.scene_frame_failed);
        assert!(!app.world().resource::<crate::resources::ScriptErrorMessage>().0.is_empty());
    }
}

// PRIVATE NOT_RUN. Append this complete child module to scene_publication.rs.
// Requires the C12 Engine/Retired/math modules and the staged Current handlers.
#[cfg(all(test, not(target_arch = "wasm32")))]
mod c12_complete_publication_contracts {
    use super::*;
    use bevy::reflect as bevy_reflect;
    use crate::components::{MusicMarker, VnSprite};
    use crate::resources::{MusicAssetRegistry, MusicEntity, MusicPlaybackState, MusicVolume};
    use crate::scene_retired::RetiredProjection;
    use rvn_core::scene_resources::{
        EffectClock, EffectParameters, ResourceTaskKind, SavedEffect, TargetKind,
    };
    use rvn_core::scene_tasks::TaskOwner;

    const SOURCE: &str = r#"
scene label A
music.volume(0.4)
music.play("old.ogg")
eileen.show("old") at left
"before"
music.play("new.ogg") with fade(4000)
eileen.show("new") at right with fade(4000)
"after"
"later"
"#;

    // The fixture uses actual local PCM decoding and typed images. It adds
    // no AudioPlugin/native sink or render/GPU plugin.
    #[derive(Resource)]
    struct CompleteFixtureImages(Vec<Handle<Image>>);
    #[derive(Default, bevy::reflect::TypePath)]
    struct FixtureImageLoader;
    impl bevy::asset::AssetLoader for FixtureImageLoader {
        type Asset = Image;
        type Settings = ();
        type Error = std::io::Error;
        async fn load<'a>(
            &'a self,
            _reader: &'a mut bevy::asset::io::Reader<'_>,
            _settings: &'a Self::Settings,
            _context: &'a mut bevy::asset::LoadContext<'_>,
        ) -> Result<Image, Self::Error> {
            Ok(Image::default())
        }
        fn extensions(&self) -> &[&str] {
            &["png"]
        }
    }

    pub(super) fn fixture() -> App {
        let mut game = rvn_core::Engine::from_program(
            rvn_parser::parse_program(SOURCE).unwrap(),
            crate::bevy_renderer::BevyRenderer::new(),
            32,
        )
        .unwrap();
        game.step_until_interaction().unwrap();
        let directory = bevy::asset::io::memory::Dir::default();
        for path in [
            "sprites/eileen/old.png",
            "sprites/eileen/new.png",
            "music/old.ogg",
            "music/new.ogg",
        ] {
            directory.insert_asset(std::path::Path::new(path), if path.starts_with("music/"){crate::checkpoint_sources::fixture_wave_for_test()}else{vec![0u8]});
        }
        let reader = bevy::asset::io::memory::MemoryAssetReader { root: directory };
        let mut app = App::new();
        app.register_asset_source(
            bevy::asset::io::AssetSourceId::Default,
            bevy::asset::io::AssetSource::build().with_reader(move || Box::new(reader.clone())),
        );
        app.add_plugins((
            MinimalPlugins,
            bevy::asset::AssetPlugin {
                watch_for_changes_override: Some(false),
                ..default()
            },
        ));
        app.init_asset::<Image>()
            .init_asset::<AudioSource>()
            .init_asset::<crate::checkpoint_pcm::CheckpointPcm>()
            .init_asset_loader::<FixtureImageLoader>()
            .init_asset_loader::<bevy::audio::AudioLoader>()
            .insert_resource(VnEngine(game))
            .insert_resource(State::new(crate::resources::VnState::Waiting))
            .init_resource::<NextState<crate::resources::VnState>>()
            .init_resource::<crate::resources::ScriptErrorMessage>()
            .init_resource::<crate::accessibility::Accessibility>()
            .init_resource::<MusicEntity>()
            .init_resource::<MusicAssetRegistry>()
            .init_resource::<MusicPlaybackState>()
            .init_resource::<MusicVolume>()
            .init_resource::<crate::systems::settings_menu::Settings>()
            .init_resource::<ScenePresentation>()
            .init_resource::<VideoFeedbackQueue>()
            .init_resource::<VnRenderState>()
            .init_resource::<ChoiceFocus>()
            .init_resource::<ImagemapState>()
            .init_resource::<TypewriterState>()
            .add_event::<VnCommand>()
            .add_event::<PlayerInput>()
            .add_systems(
                Update,
                (
                    crate::systems::sprite_system,
                    crate::systems::audio_system,
                    crate::scene_effects::reconcile_for_test,
                )
                    .chain(),
            );
        app.world_mut().spawn((
            crate::systems::sprite::SpriteStage,
            SpatialBundle::default(),
        ));
        let mut images=Vec::new();
        for path in ["sprites/eileen/old.png","sprites/eileen/new.png"] {
            let handle=app.world().resource::<AssetServer>().load::<Image>(path);
            app.world_mut().resource_mut::<Assets<Image>>().insert(handle.id(),Image::default());images.push(handle);
        }
        app.insert_resource(CompleteFixtureImages(images));
        crate::checkpoint_sources::prime_assets_for_test(app.world_mut()).unwrap();
        consume(app.world_mut());
        app.world_mut().run_schedule(Update);
        assert!(!app
            .world()
            .resource::<VnEngine>()
            .0
            .renderer
            .scene_batch_pending());
        assert!(app.world().resource::<MusicEntity>().0.is_some());
        assert_eq!(current_sprites(app.world()).len(), 1);
        app.world_mut().resource_mut::<Events<VnCommand>>().clear();
        app
    }

    // Only actual reader checkpoint metadata can be enriched by publication;
    // all authored state, targets, owners, UI, RNG and effect clocks stay exact.
    fn without_music_reader_metadata(state:&rvn_core::GameState)->serde_json::Value {
        let mut logical=state.clone();logical.scene_execution.as_mut().unwrap().resources.music_checkpoints.clear();
        serde_json::to_value(logical).unwrap()
    }
    fn assert_music_metadata_from_real_sources(world:&World) {
        let game=&world.resource::<VnEngine>().0;
        let rows=rvn_core::audio_checkpoint::music_views(&game.state,game.runtime_epoch()).unwrap();assert_eq!(rows.len(),2);
        for row in rows {
            let entity=crate::checkpoint_sources::music_entity_for_test(world,&row.authority.target).unwrap();
            let reader=world.resource::<Assets<crate::checkpoint_pcm::CheckpointPcm>>().get(world.get::<Handle<crate::checkpoint_pcm::CheckpointPcm>>(entity).unwrap()).unwrap();
            let observed=reader.gate().snapshot(&[reader],|mut values|Ok(values.remove(0))).unwrap();
            assert_eq!(row.checkpoint.as_ref(),Some(&observed));assert_eq!(observed.next_sample,0);
        }
    }
    fn current_sprites(world: &World) -> Vec<Entity> {
        world
            .iter_entities()
            .filter(|entity| {
                entity.contains::<VnSprite>() && !entity.contains::<RetiredProjection>()
            })
            .map(|entity| entity.id())
            .collect()
    }

    pub(super) fn next_pending(app: &mut App, seconds: f64) {
        let mut game = app.world_mut().resource_mut::<VnEngine>();
        game.0.advance_dialogue().unwrap();
        game.0.step_until_interaction().unwrap();
        game.0
            .tick_game_frame(seconds, rvn_core::GameFrameReports::default())
            .unwrap();
        assert!(game.0.renderer.scene_batch_pending());
        assert_eq!(
            game.0.state.music.current_file.as_deref(),
            Some("music/new.ogg")
        );
        drop(game);crate::checkpoint_sources::prime_assets_for_test(app.world_mut()).unwrap();
    }

    // These snapshots are taken BEFORE consume. The failure latch/diagnostic
    // are deliberately checked separately: they are the allowed failure output.
    #[derive(Debug, PartialEq)]
    pub(super) struct FrozenPublication {
        core: serde_json::Value,
        models_output: String,
        events: Vec<String>,
        inputs: Vec<String>,
        ecs: Vec<String>,
    }
    pub(super) fn freeze(world: &World) -> FrozenPublication {
        let game = &world.resource::<VnEngine>().0;
        let render = world.resource::<VnRenderState>();
        let typing = world.resource::<TypewriterState>();
        let core = serde_json::json!({"state":game.state,
            "history":game.history.entries().iter().map(|entry|(&entry.state,&entry.display)).collect::<Vec<_>>(),
            "runtime":game.runtime_epoch(), "interface":game.interface_epoch()});
        let models_output = format!(
            "{:?}|{:?}|{:?}|{}|{:?}|{}|{}|{}|{:?}|{:?}|{:?}|{:?}|{:?}|{}|{}|{}|{}",
            game.renderer.scene_batch,
            game.renderer.pending,
            world.resource::<ScenePresentation>().batch,
            game.renderer.scene_stepped,
            game.renderer.choice_result,
            game.renderer.waiting_for_input,
            game.renderer.menu_pending,
            game.renderer.menu_can_rollback,
            world.resource::<crate::programmable_ui::Screens>().views,
            render.background,
            render.choice_options,
            render.state_after_anim,
            world.resource::<ChoiceFocus>().0,
            typing.full_text,
            typing.visible_chars,
            typing.char_progress.to_bits(),
            typing.typing
        );
        let events = world
            .resource::<Events<VnCommand>>()
            .iter_current_update_events()
            .map(|event| format!("{event:?}"))
            .collect();
        let inputs = world
            .resource::<Events<PlayerInput>>()
            .iter_current_update_events()
            .map(|event| format!("{event:?}"))
            .collect();
        let mut ecs: Vec<_> = world
            .iter_entities()
            .map(|entity| {
                format!(
                    "{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{}|{}|{}|{}|{:?}|{:?}|{:?}|{:?}",
                    entity.id(),
                    entity.get::<Transform>(),
                    entity.get::<Sprite>(),
                    entity.get::<Visibility>(),
                    entity.get::<Parent>().map(Parent::get),
                    entity
                        .get::<Children>()
                        .map(|children| children.iter().copied().collect::<Vec<_>>()),
                    entity.get::<Handle<Image>>().map(Handle::id),
                    (entity.get::<Handle<AudioSource>>().map(Handle::id),entity.get::<Handle<crate::checkpoint_pcm::CheckpointPcm>>().map(Handle::id)),
                    entity.get::<VnSprite>().map(|sprite| &sprite.id),
                    entity.contains::<MusicMarker>(),
                    entity.contains::<crate::components::FadeAnim>(),
                    entity.contains::<crate::components::AudioFade>(),
                    entity.contains::<AudioSink>(),
                    entity.get::<RetiredProjection>(),
                    entity.get::<crate::scene_effects::FadeLease>(),
                    entity.get::<crate::scene_effects::AudioLease>(),
                    entity
                        .get::<PlaybackSettings>()
                        .map(|settings| (settings.volume.get().to_bits(), settings.paused))
                )
            })
            .collect();
        ecs.sort();
        FrozenPublication {
            core,
            models_output,
            events,
            inputs,
            ecs,
        }
    }
    pub(super) fn sentinel_output(world: &mut World) {
        world
            .resource_mut::<VnEngine>()
            .0
            .renderer
            .pending
            .push(VnCommand::SfxPlay {
                file: "preserve-on-refusal.wav".into(),
            });
        world.send_event(VnCommand::ShowChoice {
            options: vec!["preserve-existing-output".into()],
        });
        world.send_event(PlayerInput::Choose(1));
        let mut typing = world.resource_mut::<TypewriterState>();
        typing.full_text = "keep-partial".into();
        typing.visible_chars = 4;
        typing.char_progress = 0.375;
    }
    fn rejected_without_consumption(app: &mut App, expected_error: &str) {
        sentinel_output(app.world_mut());
        let before = freeze(app.world());
        consume(app.world_mut());
        assert_eq!(freeze(app.world()), before);
        assert!(app
            .world()
            .resource::<VnEngine>()
            .0
            .renderer
            .scene_batch_pending());
        assert!(
            app.world()
                .resource::<VnEngine>()
                .0
                .renderer
                .scene_frame_failed
        );
        assert!(app
            .world()
            .resource::<crate::resources::ScriptErrorMessage>()
            .0
            .contains(expected_error));
        assert!(matches!(
            app.world()
                .resource::<NextState<crate::resources::VnState>>(),
            NextState::Pending(crate::resources::VnState::Error)
        ));
    }

    #[test]
    fn complete_publication_ambiguous_current_gain_refuses_before_batch_pending_events_or_ecs_are_taken(
    ) {
        let mut app = fixture();
        next_pending(&mut app, 1.0);
        let mut game = app.world_mut().resource_mut::<VnEngine>();
        let target = game
            .0
            .state
            .scene_execution
            .as_ref()
            .unwrap()
            .resources
            .target(&TaskOwner::Game, TargetKind::Music, "music")
            .unwrap()
            .clone();
        // The authored replacement already has an incoming fade. A distinct
        // valid Effect channel on the same incarnation is ambiguous in gain.
        game.0
            .register_scene_effect(
                rvn_core::scene_resources::resource_key(
                    TaskOwner::Game,
                    ResourceTaskKind::Effect,
                    "music/additional-current-gain",
                ),
                target,
                SavedEffect {
                    presentation: None,
                    base_resolved: true,
                    clock: EffectClock {
                        duration: 4.0,
                        elapsed: 1.0,
                        looping: false,
                    },
                    parameters: EffectParameters::AudioFade {
                        from: 0.0,
                        to: 0.4,
                        despawn_on_done: false,
                    },
                },
            )
            .unwrap();
        drop(game);
        rejected_without_consumption(&mut app, "Ambiguous Current music fade");
    }

    #[test]
    fn complete_publication_retired_refusal_preserves_the_previous_models_pending_outputs_and_current_roles(
    ) {
        let mut app = fixture();
        next_pending(&mut app, 1.0);
        {
            let mut game = app.world_mut().resource_mut::<VnEngine>();
            let task = game
                .0
                .state
                .scene_execution
                .as_mut()
                .unwrap()
                .resources
                .tasks
                .iter_mut()
                .find(|task| {
                    task.target.kind == TargetKind::PresentationRetired
                        && matches!(
                            task.effect
                                .as_ref()
                                .and_then(|effect| effect.presentation.as_ref())
                                .map(|presentation| &presentation.model),
                            Some(rvn_core::retired_presentations::RetiredModel::Sprite { .. })
                        )
                })
                .unwrap();
            let rvn_core::retired_presentations::RetiredModel::Sprite { style, .. } = &mut task
                .effect
                .as_mut()
                .unwrap()
                .presentation
                .as_mut()
                .unwrap()
                .model
            else {
                unreachable!()
            };
            // Finite authored scale, but its resolved historical sprite pose
            // overflows. Refusal must happen before any physical publication.
            style.scale = f32::MAX;
        }
        // Check the exact preflight independently to avoid accepting a refusal
        // from a different, earlier guard as evidence for the Retired branch.
        let game = &app.world().resource::<VnEngine>().0;
        let error = crate::scene_retired::prepare(app.world(), &game.state, game.runtime_epoch())
            .err()
            .expect("invalid historical projection must refuse");
        rejected_without_consumption(&mut app, &error);
    }

    #[test]
    fn complete_publication_hands_off_old_music_before_current_handlers_and_keeps_current_and_retired_roles_independent(
    ) {
        let mut app = fixture();
        let old_music = app.world().resource::<MusicEntity>().0.unwrap();
        let old_source = app
            .world()
            .get::<Handle<crate::checkpoint_pcm::CheckpointPcm>>(old_music)
            .unwrap()
            .clone();
        let current_sprite = current_sprites(app.world())[0];
        next_pending(&mut app, 1.0);
        let authoritative =
            without_music_reader_metadata(&app.world().resource::<VnEngine>().0.state);
        consume(app.world_mut());
        assert!(!app
            .world()
            .resource::<VnEngine>()
            .0
            .renderer
            .scene_batch_pending());
        let installed_current=app.world().resource::<MusicEntity>().0.unwrap();assert_ne!(installed_current,old_music);
        assert!(app.world().get::<RetiredProjection>(old_music).is_some());
        assert!(app.world().get::<MusicMarker>(old_music).is_none());
        assert!(app
            .world()
            .get::<crate::scene_effects::AudioLease>(old_music)
            .is_none());
        assert!(app
            .world()
            .get::<crate::components::AudioFade>(old_music)
            .is_none());
        assert_eq!(
            app.world().get::<Handle<crate::checkpoint_pcm::CheckpointPcm>>(old_music).unwrap(),
            &old_source
        );
        assert_eq!(
            app.world()
                .get::<PlaybackSettings>(old_music)
                .unwrap()
                .volume
                .get(),
            (f64::from(0.4f32) * 0.75) as f32
        );
        let retired_sprite_leaves: Vec<_> = app
            .world()
            .iter_entities()
            .filter(|entity| entity.contains::<RetiredProjection>() && entity.contains::<Sprite>())
            .map(|entity| entity.id())
            .collect();
        assert_eq!(retired_sprite_leaves.len(), 1);
        let retired_leaf = retired_sprite_leaves[0];
        assert!(app.world().get::<VnSprite>(retired_leaf).is_none());
        assert!(app
            .world()
            .get::<crate::layered_characters::LayerTarget>(retired_leaf)
            .is_none());
        let expected_token = app
            .world()
            .get::<RetiredProjection>(retired_leaf)
            .unwrap()
            .token
            .clone();
        let old_pose = *app.world().get::<Transform>(retired_leaf).unwrap();
        assert_eq!(
            app.world()
                .get::<Sprite>(retired_leaf)
                .unwrap()
                .color
                .alpha(),
            0.75
        );
        app.world_mut().run_schedule(Update);
        let new_music = app.world().resource::<MusicEntity>().0.unwrap();
        assert_eq!(new_music,installed_current);assert_ne!(new_music, old_music);
        assert!(app.world().get::<MusicMarker>(new_music).is_some());
        assert!(app.world().get::<RetiredProjection>(new_music).is_none());
        assert_eq!(
            app.world()
                .get::<PlaybackSettings>(new_music)
                .unwrap()
                .volume
                .get(),
            (f64::from(0.4f32) * 0.25) as f32
        );
        assert_eq!(current_sprites(app.world()), vec![current_sprite]);
        let texture = app.world().get::<Handle<Image>>(current_sprite).unwrap();
        assert_eq!(
            app.world()
                .resource::<AssetServer>()
                .get_path(texture.id())
                .unwrap()
                .path(),
            std::path::Path::new("sprites/eileen/new.png")
        );
        assert_eq!(
            app.world()
                .get::<Sprite>(current_sprite)
                .unwrap()
                .color
                .alpha(),
            0.25
        );
        assert_eq!(
            without_music_reader_metadata(
                &app.world()
                    .resource::<ScenePresentation>()
                    .batch
                    .as_ref()
                    .unwrap()
                    .state
            ),
            authoritative
        );
        assert_eq!(
            without_music_reader_metadata(&app.world().resource::<VnEngine>().0.state),
            authoritative
        );
        assert_music_metadata_from_real_sources(app.world());
        // Raw legacy MusicStop is neutral for modern exact players; visual
        // ClearSprites can remove its Current projection, never historical leaves.
        app.world_mut().resource_mut::<Events<VnCommand>>().clear();
        app.world_mut().send_event(VnCommand::ClearSprites);
        app.world_mut().send_event(VnCommand::MusicStop);
        app.world_mut().run_schedule(Update);
        assert!(app.world().get_entity(new_music).is_some());assert_eq!(app.world().resource::<MusicEntity>().0,Some(new_music));
        assert!(app.world().get_entity(current_sprite).is_none());
        assert!(app.world().get_entity(old_music).is_some());
        assert_eq!(
            &app.world()
                .get::<RetiredProjection>(retired_leaf)
                .unwrap()
                .token,
            &expected_token
        );
        assert_eq!(
            *app.world().get::<Transform>(retired_leaf).unwrap(),
            old_pose
        );
        assert_eq!(
            app.world()
                .get::<Sprite>(retired_leaf)
                .unwrap()
                .color
                .alpha(),
            0.75
        );
        assert_eq!(
            app.world()
                .get::<PlaybackSettings>(old_music)
                .unwrap()
                .volume
                .get(),
            (f64::from(0.4f32) * 0.75) as f32
        );
        assert_eq!(
            without_music_reader_metadata(&app.world().resource::<VnEngine>().0.state),
            authoritative
        );
    }
}

// PRIVATE NOT_RUN. Actual capabilities and the existing producer; no device.
#[cfg(test)]
mod host_gate_contracts {
    use super::*;
    use rvn_core::{scene_resources::{resource_key, EffectClock, EffectParameters,
        ResourceTaskKind, SavedEffect, TargetKind, TransformSnapshot, VisualTransition},
        scene_tasks::TaskOwner};

    fn engine() -> crate::resources::VnEngine {
        let mut game = rvn_core::Engine::from_program(rvn_parser::parse_program(
            "scene \"parent.png\"\nscene label P\n\"parent\"\n").unwrap(),
            crate::bevy_renderer::BevyRenderer::new(), 32).unwrap();
        game.step_until_interaction().unwrap();
        game.renderer.scene_batch.take();
        crate::resources::VnEngine(game)
    }
    fn effect(engine: &mut crate::resources::VnEngine, owner: TaskOwner) {
        let target = engine.0.state.scene_execution.as_ref().unwrap().resources
            .target(&TaskOwner::Game, TargetKind::Background, "background").unwrap().clone();
        engine.0.register_scene_effect(resource_key(owner, ResourceTaskKind::Effect, "gate"),
            target, SavedEffect { presentation: None, base_resolved: false,
                clock: EffectClock { duration: 4.0, elapsed: 0.0, looping: false },
                parameters: EffectParameters::VisualFade { from: 0.0, to: 1.0,
                    despawn_on_finish: false, transition: VisualTransition::Fade,
                    base: TransformSnapshot::default(), layered: false } }).unwrap();
    }
    fn world() -> World {
        let mut engine = engine();
        effect(&mut engine, TaskOwner::Host { role: rvn_ui::PageRole::Pause.id().into() });
        let mut world = World::new();
        world.insert_resource(engine);
        world.init_resource::<crate::accessibility::Accessibility>();
        world
    }

    #[test]
    fn autonomous_host_gate_survives_menu_absence_and_refuses_pending_failed_focus_or_block() {
        let mut world = world();
        assert!(world.resource::<VnEngine>().0.state.ui.screens.is_empty());
        assert!(world.get_resource::<crate::source_menus::SourceMenus>().is_none());
        assert!(host_active(&world));
        world.resource_mut::<VnEngine>().0.tick_host_frame(0.125).unwrap();
        assert!(!host_active(&world)); // exact new publication must be drained
        world.resource_mut::<VnEngine>().0.renderer.scene_batch.take();
        assert!(host_active(&world));
        world.resource_mut::<VnEngine>().0.renderer.scene_frame_failed = true;
        assert!(!host_active(&world));
        world.resource_mut::<VnEngine>().0.renderer.scene_frame_failed = false;
        world.resource_mut::<crate::accessibility::Accessibility>().blocked = true;
        assert!(!host_active(&world));
        world.resource_mut::<crate::accessibility::Accessibility>().blocked = false;
        world.spawn((Window { focused: false, ..default() }, bevy::window::PrimaryWindow));
        assert!(!host_active(&world));
    }

    #[test]
    fn host_gate_uses_primary_focus_and_never_adopts_a_story_effect_as_a_host_clock() {
        let mut story = engine();
        let activation = story.0.state.scene_execution.as_ref().unwrap().owner().unwrap();
        effect(&mut story, TaskOwner::Scene { activation });
        let mut empty = World::new(); empty.insert_resource(story);
        assert!(!host_active(&empty));
        let mut world = world();
        let primary = world.spawn((Window { focused: true, ..default() }, bevy::window::PrimaryWindow)).id();
        let secondary = world.spawn(Window { focused: false, ..default() }).id();
        assert!(host_active(&world));
        world.get_mut::<Window>(primary).unwrap().focused = false;
        world.get_mut::<Window>(secondary).unwrap().focused = true;
        assert!(!host_active(&world));
    }

    #[test]
    fn existing_canvas_feedback_producer_advances_one_autonomous_host_in_menu_without_canvas() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = world();
        let mut time: Time = default(); time.advance_by(std::time::Duration::from_secs_f64(0.125));
        world.insert_resource(time);
        world.insert_resource(State::new(crate::resources::VnState::Menu));
        world.init_resource::<crate::programmable_ui::Screens>();
        world.init_resource::<crate::resources::ScriptErrorMessage>();
        world.init_resource::<NextState<crate::resources::VnState>>();
        world.init_resource::<Events<crate::vn_command::VnCommand>>();
        let pc = world.resource::<VnEngine>().0.state.pc;
        world.run_system_once(crate::custom_canvas::tick);
        let descriptor = &world.resource::<VnEngine>().0.state.scene_execution.as_ref()
            .unwrap().resources.tasks.iter().find(|task| task.kind == ResourceTaskKind::Effect)
            .unwrap().effect.as_ref().unwrap().clock;
        assert_eq!(descriptor.elapsed, 0.125);
        assert_eq!(world.resource::<VnEngine>().0.state.pc, pc);
        assert!(world.resource::<VnEngine>().0.renderer.scene_batch_pending());
        let before = serde_json::to_vec(&world.resource::<VnEngine>().0.state).unwrap();
        world.run_system_once(crate::custom_canvas::tick);
        assert_eq!(serde_json::to_vec(&world.resource::<VnEngine>().0.state).unwrap(), before);
    }
}

// PRIVATE child appended to scene_publication.rs. Actual C12 Current/Retired
// fixture and the production publication path; no browser/device claim.
#[cfg(all(test, not(target_arch = "wasm32")))]
mod c13_web_publication_boundary_contracts {
    use super::*;
    use super::c12_complete_publication_contracts::{fixture, freeze, next_pending, sentinel_output};

    #[test]
    fn web_sync_install_refusal_preserves_complete_pending_outputs_core_history_and_ecs() {
        let mut app = fixture();
        next_pending(&mut app, 1.0);
        sentinel_output(app.world_mut());
        let before = freeze(app.world());
        let mut calls = 0;
        consume_with_music_installer(app.world_mut(), |world, state, epoch| {
            calls += 1;
            assert_eq!(epoch, world.resource::<VnEngine>().0.runtime_epoch());
            assert_eq!(state.music.current_file.as_deref(), Some("music/new.ogg"));
            assert!(state.scene_execution.as_ref().unwrap().resources.tasks.iter()
                .any(|task| task.target.kind == rvn_core::scene_resources::TargetKind::PresentationRetired));
            assert!(world.resource::<VnEngine>().0.renderer.scene_batch_pending());
            assert_eq!(world.resource::<ScenePresentation>().batch.as_ref().unwrap()
                .state.music.current_file.as_deref(), Some("music/old.ogg"));
            assert_eq!(freeze(world), before); // BEFORE any simulated external refusal.
            Err("synchronous indexed Audio constructor refused".into())
        });
        assert_eq!(calls, 1);
        assert_eq!(freeze(app.world()), before);
        assert!(app.world().resource::<VnEngine>().0.renderer.scene_batch_pending());
        assert!(app.world().resource::<VnEngine>().0.renderer.scene_frame_failed);
        assert!(app.world().resource::<crate::resources::ScriptErrorMessage>().0
            .contains("synchronous indexed Audio constructor refused"));
        assert!(matches!(app.world().resource::<NextState<crate::resources::VnState>>(),
            NextState::Pending(crate::resources::VnState::Error)));
    }
}

// PRIVATE NOT_RUN. Append after the immutable C13 Host publication stage.
// Uses actual App/Update plugins and producer; no renderer, audio device or UI.
#[cfg(all(test, not(target_arch = "wasm32")))]
mod host_schedule_contracts {
    use super::*;
    use rvn_core::scene_resources::{
        resource_key, EffectClock, EffectParameters, ResourceTaskKind, ResourceToken,
        SavedEffect, TargetKind, TransformSnapshot, VisualTransition,
    };
    use rvn_core::scene_tasks::TaskOwner;

    #[derive(Resource)]
    struct Trace {
        token: ResourceToken,
        background: Entity,
        expected_elapsed: f64,
        phases: Vec<&'static str>,
    }

    fn exact_clock(world: &World) -> f64 {
        let trace = world.resource::<Trace>();
        let game = &world.resource::<VnEngine>().0;
        let execution = game.state.scene_execution.as_ref().unwrap();
        assert!(execution.resources.accepts(
            &trace.token, game.runtime_epoch(), execution.owner(),
        ));
        let task = execution.resources.tasks.iter().find(|task| {
            task.key == trace.token.task.key && task.revision == trace.token.task.revision
                && task.target == trace.token.target
        }).unwrap();
        task.effect.as_ref().unwrap().clock.elapsed
    }

    fn observe_producer(world: &mut World) {
        let expected = world.resource::<Trace>().expected_elapsed;
        assert_eq!(exact_clock(world).to_bits(), expected.to_bits());
        assert!(world.resource::<VnEngine>().0.renderer.scene_batch_pending());
        // The observation runs inside the real producer's set. Only the
        // existing Host publication drain is allowed to install this batch.
        world.resource_mut::<Trace>().phases.push("producer");
    }

    fn observe_feedback(world: &mut World) {
        let trace = world.resource::<Trace>();
        assert!(!world.resource::<VnEngine>().0.renderer.scene_batch_pending());
        let installed = world.resource::<ScenePresentation>().batch.as_ref().unwrap();
        assert_eq!(installed.runtime_epoch, trace.token.task.runtime_epoch);
        let effect = installed.effects.iter().find(|task| {
            task.key == trace.token.task.key && task.revision == trace.token.task.revision
                && task.target == trace.token.target
        }).unwrap().effect.as_ref().unwrap();
        assert_eq!(effect.clock.elapsed.to_bits(), trace.expected_elapsed.to_bits());
        assert_eq!(exact_clock(world).to_bits(), trace.expected_elapsed.to_bits());
        world.resource_mut::<Trace>().phases.push("feedback");
    }

    fn observe_input_after_current_reconciliation(world: &mut World) {
        let trace = world.resource::<Trace>();
        assert!(!world.resource::<VnEngine>().0.renderer.scene_batch_pending());
        assert_eq!(world.get::<crate::scene_effects::FadeLease>(trace.background).unwrap().0,
            trace.token);
        let alpha = world.get::<Sprite>(trace.background).unwrap().color.alpha();
        // Independently known input: duration4, fade0->1, delta1/8. A stale
        // frame's alpha (including default1) cannot satisfy this assertion.
        let expected_alpha = (trace.expected_elapsed / 4.0) as f32;
        assert_eq!(alpha.to_bits(), expected_alpha.to_bits());
        assert_eq!(exact_clock(world).to_bits(), trace.expected_elapsed.to_bits());
        world.resource_mut::<Trace>().phases.push("input");
    }

    #[test]
    fn actual_update_publishes_host_before_feedback_and_current_reconciliation_in_menu() {
        let mut game = rvn_core::Engine::from_program(
            rvn_parser::parse_program(
                "scene \"parent.png\"\nscene label P\ntimer 5 => jump done\n\"parent\"\nlabel done\n",
            ).unwrap(), crate::bevy_renderer::BevyRenderer::new(), 32,
        ).unwrap();
        game.step_until_interaction().unwrap();
        game.renderer.scene_batch.take();
        let target = game.state.scene_execution.as_ref().unwrap().resources
            .target(&TaskOwner::Game, TargetKind::Background, "background").unwrap().clone();
        let token = game.register_scene_effect(
            resource_key(TaskOwner::Host { role: rvn_ui::PageRole::Pause.id().into() },
                ResourceTaskKind::Effect, "scheduled-fade"), target,
            SavedEffect { presentation: None, base_resolved: false,
                clock: EffectClock { duration: 4.0, elapsed: 0.0, looping: false },
                parameters: EffectParameters::VisualFade { from: 0.0, to: 1.0,
                    despawn_on_finish: false, transition: VisualTransition::Fade,
                    base: TransformSnapshot::default(), layered: false } },
        ).unwrap();
        let original_pc = game.state.pc;
        let original_stack = game.state.call_stack.clone();
        let original_random = game.state.random.clone();
        let original_timer = game.state.scene_execution.as_ref().unwrap()
            .active.as_ref().unwrap().tasks.timer.clone();
        assert!(original_timer.is_some());

        let mut app = App::new();
        app.insert_resource(VnEngine(game))
            .insert_resource(State::new(crate::resources::VnState::Menu))
            .insert_resource(Time::<()>::default())
            .init_resource::<NextState<crate::resources::VnState>>()
            .init_resource::<crate::accessibility::Accessibility>()
            .init_resource::<crate::resources::ScriptErrorMessage>()
            .init_resource::<VnRenderState>()
            .init_resource::<ChoiceFocus>()
            .init_resource::<ImagemapState>()
            .init_resource::<TypewriterState>()
            .init_resource::<crate::programmable_ui::Screens>()
            .add_event::<VnCommand>().add_event::<PlayerInput>()
            .add_plugins((ScenePublicationPlugin, crate::scene_effects::SceneEffectsPlugin,
                crate::scene_retired::SceneRetiredPlugin));
        let background = app.world_mut().spawn((
            crate::components::CurrentMotionBackground, Sprite::default(), Transform::default(),
        )).id();
        app.insert_resource(Trace { token, background, expected_elapsed: 0.125, phases: vec![] });
        // Exactly the existing CustomCanvasPlugin producer registration.
        // No chain or substitute drain imposes the ordering being tested.
        app.add_systems(Update, crate::custom_canvas::tick.run_if(ready).in_set(CanvasFeedbackSet));
        app.add_systems(Update, observe_producer.after(crate::custom_canvas::tick).in_set(CanvasFeedbackSet));
        app.add_systems(Update, observe_feedback.in_set(VideoFeedbackSet));
        app.add_systems(Update, observe_input_after_current_reconciliation
            .in_set(crate::programmable_ui::InterfaceInputSet));
        assert!(app.world().resource::<VnEngine>().0.state.ui.screens.is_empty());
        assert!(app.world().get_resource::<crate::source_menus::SourceMenus>().is_none());

        for expected_elapsed in [0.125, 0.25] {
            app.world_mut().resource_mut::<Trace>().expected_elapsed = expected_elapsed;
            app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f64(0.125));
            app.update();
            assert!(!app.world().resource::<VnEngine>().0.renderer.scene_frame_failed);
            assert!(app.world().resource::<crate::resources::ScriptErrorMessage>().0.is_empty());
        }
        let game = &app.world().resource::<VnEngine>().0;
        assert_eq!(game.state.pc, original_pc);
        assert_eq!(game.state.call_stack, original_stack);
        assert_eq!(game.state.random, original_random);
        assert_eq!(game.state.scene_execution.as_ref().unwrap().active.as_ref().unwrap().tasks.timer,
            original_timer);
        let trace = app.world().resource::<Trace>();
        assert_eq!(trace.phases.len(), 6);
        for instant in trace.phases.chunks_exact(3) {
            assert_eq!(instant[0], "producer");
            assert_eq!(instant.iter().filter(|phase| **phase == "feedback").count(), 1);
            assert_eq!(instant.iter().filter(|phase| **phase == "input").count(), 1);
        }
    }
}

// PRIVATE child of scene_publication.rs, appended after its immutable tests.
// Actual App/Update and Core authority; no decoder, assets, audio device or UI.
#[cfg(all(test, not(target_arch = "wasm32")))]
mod host_media_update_contracts {
    use super::*;
    use rvn_core::scene_resources::{resource_key, EffectClock, EffectParameters,
        ResourceTaskKind, SavedEffect, TargetKind, TransformSnapshot, VisualTransition};
    use rvn_core::scene_tasks::TaskOwner;
    use rvn_core::video::{Feedback, VideoView};
    use rvn_parser::Value;

    const SOURCE: &str = r#"
screen host(ctx) {return component("root","column",{},[
 component("launch","button",{"events":{"click":"launch_host"}},[]),
 component("player","image",{"image":"player.png"},[]),component("picture","image",{"image":"picture.png"},[])])}
handler launch_host(event) {
 video.play("hostclip",video_clip("h.webm",{"target":"ui:host/player","on_end":"host_end","on_error":"host_error"}))
 motion.play("ui:host/picture",motion_pause(1))
}
handler host_end(event) {set host_ends=host_ends+1}
handler host_error(event) {set host_errors=host_errors+1}
handler game_end(event) {set game_ends=game_ends+1}
init {set host_ends=0 set host_errors=0 set game_ends=0}
scene "parent.png"
scene label P
video.play("gameclip",video_clip("g.webm",{"on_end":"game_end"}))
timer 5 => jump done
"parent"
call C
"returned"
jump done
scene label C
"child"
return
label done
"done"
"#;

    fn fixture(source:&str) -> App {
        let mut game=rvn_core::Engine::from_program(rvn_parser::parse_program(source).unwrap(),
            crate::bevy_renderer::BevyRenderer::new(),32).unwrap();
        game.step_until_interaction().unwrap();
        let context=rvn_core::ui::value_from_json(&rvn_ui::source_menus::preview_context(
            rvn_ui::PageRole::Pause,true)).unwrap();
        game.synchronize_source_menu(rvn_ui::PageRole::Pause,Some("host"),context,true,500).unwrap();
        game.interface_event(rvn_core::ui::UiInput{screen:"host".into(),element:"launch".into(),
            kind:rvn_ui::programmable::ScreenEventKind::Click,value:None,key:None}).unwrap();
        let mut app=App::new();
        app.insert_resource(VnEngine(game)).insert_resource(State::new(crate::resources::VnState::Waiting))
            .insert_resource(Time::<()>::default()).init_resource::<NextState<crate::resources::VnState>>()
            .init_resource::<crate::accessibility::Accessibility>().init_resource::<crate::resources::ScriptErrorMessage>()
            .init_resource::<VnRenderState>().init_resource::<ChoiceFocus>().init_resource::<ImagemapState>()
            .init_resource::<TypewriterState>().init_resource::<crate::programmable_ui::Screens>()
            .add_event::<VnCommand>().add_event::<PlayerInput>()
            .add_plugins((ScenePublicationPlugin,crate::scene_effects::SceneEffectsPlugin,crate::scene_retired::SceneRetiredPlugin));
        // Production's existing producer/set and drain, no substitute chain.
        app.add_systems(Update,crate::custom_canvas::tick.run_if(ready).in_set(CanvasFeedbackSet));
        consume(app.world_mut());
        assert!(!app.world().resource::<VnEngine>().0.renderer.scene_batch_pending());
        app
    }
    fn view(world:&World,id:&str) -> VideoView {
        world.resource::<VnEngine>().0.video_views().unwrap().into_iter().find(|view|view.id==id).unwrap()
    }
    fn story(world:&World)->serde_json::Value {
        let game=&world.resource::<VnEngine>().0;
        let execution=game.state.scene_execution.as_ref().unwrap();
        serde_json::json!({"pc":game.state.pc,"stack":game.state.call_stack,"random":game.state.random,
            "active_timer":game.active_timer,"scene_timer":execution.active.as_ref().unwrap().tasks.timer,
            "game_timer":execution.game_tasks.timer,"locals":execution.active.as_ref().unwrap().locals})
    }
    fn full(world:&World)->serde_json::Value {
        let game=&world.resource::<VnEngine>().0;
        serde_json::json!({"state":game.state,"runtime":game.runtime_epoch(),"interface":game.interface_epoch(),
            "history":game.history.entries().iter().map(|entry|(&entry.state,&entry.display)).collect::<Vec<_>>()})
    }
    fn output(world:&World)->String {
        format!("{:?}|{:?}|{:?}|{:?}|{:?}",world.resource::<ScenePresentation>().batch,
            world.resource::<crate::programmable_ui::Screens>().views,
            world.resource::<VnEngine>().0.renderer.scene_batch,world.resource::<VnEngine>().0.renderer.pending,
            world.resource::<Events<VnCommand>>().iter_current_update_events().collect::<Vec<_>>())
    }
    fn queue(world:&World)->String {
        let queue=world.resource::<VideoFeedbackQueue>();
        format!("{:?}|{:?}|{}",queue.0,queue.1.values,queue.1.next)
    }
    fn delta(app:&mut App,seconds:f64) {
        app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f64(seconds));
    }

    #[test]
    fn actual_update_routes_game_and_host_terminals_once_and_advances_host_motion_without_canvas() {
        let mut app=fixture(SOURCE);
        let game=view(app.world(),"gameclip");let host=view(app.world(),"hostclip");
        assert!(queue_video_feedback(app.world_mut(),&host,Feedback::End));
        assert!(queue_video_feedback(app.world_mut(),&game,Feedback::Position{seconds:0.4,duration:2.0}));
        assert!(queue_video_feedback(app.world_mut(),&game,Feedback::End));
        assert!(queue_video_feedback(app.world_mut(),&host,Feedback::Position{seconds:0.5,duration:2.0}));
        delta(&mut app,0.125);app.update();
        let engine=&app.world().resource::<VnEngine>().0;
        assert_eq!(engine.state.vars["game_ends"],Value::Int(1));
        assert_eq!(engine.state.vars["host_ends"],Value::Int(1));
        assert_eq!(engine.state.videos.tracks["gameclip"].position,2.0);
        assert_eq!(engine.state.videos.tracks["hostclip"].position,2.0);
        assert_eq!(engine.state.motions.tracks["ui:host/picture"].elapsed,0.125);
        assert_eq!(engine.state.scene_execution.as_ref().unwrap().active.as_ref().unwrap().tasks.timer.as_ref().unwrap().elapsed,0.125);
        assert!(app.world().resource::<VideoFeedbackQueue>().0.is_empty());
        assert!(!engine.renderer.scene_frame_failed&&!engine.renderer.scene_batch_pending());
        assert_eq!(serde_json::to_value(&app.world().resource::<ScenePresentation>().batch.as_ref().unwrap().state).unwrap(),serde_json::to_value(&engine.state).unwrap());
        app.update();
        assert_eq!(app.world().resource::<VnEngine>().0.state.vars["host_ends"],Value::Int(1));
        assert_eq!(app.world().resource::<VnEngine>().0.state.vars["game_ends"],Value::Int(1));
    }

    #[test]
    fn actual_update_admits_autonomous_host_video_after_menu_close_without_canvas_or_story_clock() {
        let source=SOURCE.replace("\"target\":\"ui:host/player\",","");
        let mut app=fixture(&source);
        let context=rvn_core::ui::value_from_json(&rvn_ui::source_menus::preview_context(rvn_ui::PageRole::Pause,true)).unwrap();
        app.world_mut().resource_mut::<VnEngine>().0.synchronize_source_menu(rvn_ui::PageRole::Pause,None,context,true,500).unwrap();
        consume(app.world_mut());app.world_mut().insert_resource(State::new(crate::resources::VnState::Menu));
        assert!(app.world().resource::<VnEngine>().0.state.ui.screens.is_empty());
        assert!(app.world().get_resource::<crate::source_menus::SourceMenus>().is_none());
        let before=story(app.world());let host=view(app.world(),"hostclip");
        assert!(video_clock_allowed(app.world(),&host));
        assert!(!video_clock_allowed(app.world(),&view(app.world(),"gameclip")));
        assert!(queue_video_feedback(app.world_mut(),&host,Feedback::End));
        delta(&mut app,0.125);app.update();
        assert_eq!(app.world().resource::<VnEngine>().0.state.vars["host_ends"],Value::Int(1));
        assert_eq!(story(app.world()),before);
        assert!(app.world().resource::<VideoFeedbackQueue>().0.is_empty());
    }

    #[test]
    fn current_terminal_queue_survives_focus_access_pending_and_failure_until_exact_route_admission() {
        use bevy::ecs::system::RunSystemOnce;
        let mut app=fixture(SOURCE);app.world_mut().insert_resource(State::new(crate::resources::VnState::Menu));
        let host=view(app.world(),"hostclip");
        assert!(queue_video_feedback(app.world_mut(),&host,Feedback::Error("retained exact error".into())));
        assert!(queue_video_resume(app.world_mut(),&host));
        assert!(queue_video_feedback(app.world_mut(),&host,Feedback::End));
        let before=full(app.world());let queued=queue(app.world());
        let primary=app.world_mut().spawn((Window{focused:false,..default()},bevy::window::PrimaryWindow)).id();
        app.update();assert_eq!(full(app.world()),before);assert_eq!(queue(app.world()),queued);
        app.world_mut().get_mut::<Window>(primary).unwrap().focused=true;
        app.world_mut().resource_mut::<crate::accessibility::Accessibility>().blocked=true;
        app.update();assert_eq!(full(app.world()),before);assert_eq!(queue(app.world()),queued);
        app.world_mut().resource_mut::<crate::accessibility::Accessibility>().blocked=false;
        // An accepted unrelated Core publication is pending: standalone tick
        // must wait for its consumer, not run a zero-delta Error callback.
        app.world_mut().resource_mut::<VnEngine>().0.interface_event(rvn_core::ui::UiInput{
            screen:"host".into(),element:"launch".into(),kind:rvn_ui::programmable::ScreenEventKind::Focus,value:None,key:None}).unwrap();
        let pending=full(app.world());let outputs=output(app.world());
        assert!(app.world().resource::<VnEngine>().0.renderer.scene_batch_pending());
        app.world_mut().run_system_once(crate::custom_canvas::tick);
        assert_eq!(full(app.world()),pending);assert_eq!(output(app.world()),outputs);assert_eq!(queue(app.world()),queued);
        consume(app.world_mut());app.world_mut().resource_mut::<VnEngine>().0.renderer.scene_frame_failed=true;
        app.update();assert_eq!(queue(app.world()),queued);assert_eq!(app.world().resource::<VnEngine>().0.state.vars["host_errors"],Value::Int(0));
        app.world_mut().resource_mut::<VnEngine>().0.renderer.scene_frame_failed=false;
        app.update();assert_eq!(app.world().resource::<VnEngine>().0.state.vars["host_errors"],Value::Int(1));
        assert_eq!(app.world().resource::<VnEngine>().0.state.vars["host_ends"],Value::Int(0));
        assert!(app.world().resource::<VideoFeedbackQueue>().0.is_empty());
    }

    #[test]
    fn host_invalid_current_position_restores_pre_capture_state_outputs_ecs_and_exact_queue() {
        let mut app=fixture(SOURCE);app.world_mut().insert_resource(State::new(crate::resources::VnState::Menu));
        let token={
        let mut engine=app.world_mut().resource_mut::<VnEngine>();
        let game=&mut engine.0;
        let target=game.state.scene_execution.as_ref().unwrap().resources.target(&TaskOwner::Game,TargetKind::Background,"background").unwrap().clone();
        game.register_scene_effect(resource_key(TaskOwner::Host{role:rvn_ui::PageRole::Pause.id().into()},ResourceTaskKind::Effect,"host-measurement"),target,
            SavedEffect{presentation:None,base_resolved:false,clock:EffectClock{duration:4.0,elapsed:0.0,looping:false},
                parameters:EffectParameters::VisualFade{from:0.0,to:1.0,despawn_on_finish:false,transition:VisualTransition::Fade,base:TransformSnapshot::default(),layered:false}}).unwrap()
        };
        consume(app.world_mut());
        let pose=Transform::from_xyz(12.0,23.0,-100.0);
        let entity=app.world_mut().spawn((pose,crate::scene_effects::FadeLease(token),
            crate::components::FadeAnim{from:0.0,to:1.0,kind:crate::components::TransitionKind::Fade,duration_secs:4.0,elapsed_secs:0.0,despawn_on_finish:false})).id();
        let host=view(app.world(),"hostclip");
        assert!(queue_video_feedback(app.world_mut(),&host,Feedback::Position{seconds:f64::from_bits(0x7ff8000000000402),duration:2.0}));
        let before=full(app.world());let outputs=output(app.world());let queued=queue(app.world());
        delta(&mut app,0.125);host_frame(app.world_mut());
        assert_eq!(full(app.world()),before);assert_eq!(output(app.world()),outputs);assert_eq!(queue(app.world()),queued);
        assert_eq!(*app.world().get::<Transform>(entity).unwrap(),pose);
        assert_eq!(app.world().get::<crate::components::FadeAnim>(entity).unwrap().elapsed_secs,0.0);
        assert!(app.world().resource::<VnEngine>().0.renderer.scene_frame_failed);
        assert!(!app.world().resource::<crate::resources::ScriptErrorMessage>().0.is_empty());
        host_frame(app.world_mut());assert_eq!(full(app.world()),before);assert_eq!(queue(app.world()),queued);
    }

    #[test]
    fn exact_queue_nonce_ack_preserves_duplicate_tokens_new_rows_and_order_and_refuses_bad_receipts() {
        let mut app=fixture(SOURCE);let host=view(app.world(),"hostclip");
        assert!(queue_video_feedback(app.world_mut(),&host,Feedback::End));
        assert!(queue_video_feedback(app.world_mut(),&host,Feedback::End));
        let snapshot=app.world().resource::<VideoFeedbackQueue>().snapshot().unwrap();
        assert!(queue_video_feedback(app.world_mut(),&host,Feedback::End));
        let newest=app.world().resource::<VideoFeedbackQueue>().1.values[2];
        let before=queue(app.world());
        let invalid=[rvn_core::VideoReportReceipt{ordinal:0,disposition:rvn_core::VideoReportDisposition::Consumed},
            rvn_core::VideoReportReceipt{ordinal:0,disposition:rvn_core::VideoReportDisposition::Consumed}];
        assert!(app.world_mut().resource_mut::<VideoFeedbackQueue>().acknowledge(&snapshot,&invalid).is_err());assert_eq!(queue(app.world()),before);
        assert!(app.world_mut().resource_mut::<VideoFeedbackQueue>().acknowledge(&snapshot,&[
            rvn_core::VideoReportReceipt{ordinal:2,disposition:rvn_core::VideoReportDisposition::Consumed}]).is_err());assert_eq!(queue(app.world()),before);
        app.world_mut().resource_mut::<VideoFeedbackQueue>().acknowledge(&snapshot,&[
            rvn_core::VideoReportReceipt{ordinal:0,disposition:rvn_core::VideoReportDisposition::Consumed},
            rvn_core::VideoReportReceipt{ordinal:1,disposition:rvn_core::VideoReportDisposition::Deferred}]).unwrap();
        let queue=app.world().resource::<VideoFeedbackQueue>();
        assert_eq!(queue.0.len(),2);assert_eq!(queue.1.values,vec![snapshot.nonces[1],newest]);
        assert_eq!(queue.0[0].0,host);assert_eq!(queue.0[1].0,host);
    }

    #[test]
    fn queue_nonce_exhaustion_refuses_without_eviction_and_stale_nan_is_purely_acked_while_blocked() {
        let mut app=fixture(SOURCE);let old=view(app.world(),"hostclip");
        assert!(queue_video_feedback(app.world_mut(),&old,Feedback::End));
        app.world_mut().resource_mut::<VideoFeedbackQueue>().1.next=u64::MAX;
        let before=queue(app.world());let state=full(app.world());
        assert!(!queue_video_feedback(app.world_mut(),&old,Feedback::Error("not admitted".into())));
        assert_eq!(queue(app.world()),before);assert_eq!(full(app.world()),state);
        assert!(app.world().resource::<VnEngine>().0.renderer.scene_frame_failed);
        app.world_mut().resource_mut::<VideoFeedbackQueue>().1.next=1;
        app.world_mut().resource_mut::<VnEngine>().0.interface_event(rvn_core::ui::UiInput{
            screen:"host".into(),element:"launch".into(),kind:rvn_ui::programmable::ScreenEventKind::Click,value:None,key:None}).unwrap();
        consume(app.world_mut());
        assert_ne!(view(app.world(),"hostclip").resource_token,old.resource_token);
        assert!(queue_video_feedback(app.world_mut(),&old,Feedback::Position{seconds:f64::NAN,duration:f64::NAN}));
        app.world_mut().resource_mut::<crate::accessibility::Accessibility>().blocked=true;
        let before=full(app.world());host_frame(app.world_mut());
        assert_eq!(full(app.world()),before);assert!(app.world().resource::<VideoFeedbackQueue>().0.is_empty());
    }

    #[test]
    fn load_epoch_purge_keeps_fresh_report_and_real_parked_descriptor_remains_deferred() {
        let mut app=fixture(SOURCE);let old=view(app.world(),"gameclip");
        let save=rvn_core::save::SaveData::try_from_state(&app.world().resource::<VnEngine>().0.state,1,"load".into(),"main.rvn".into()).unwrap();
        app.world_mut().resource_mut::<VnEngine>().0.load_data(save).unwrap();
        let fresh=view(app.world(),"gameclip");assert_ne!(fresh.resource_token,old.resource_token);
        assert!(queue_video_feedback(app.world_mut(),&old,Feedback::Position{seconds:f64::NAN,duration:f64::NAN}));
        assert!(queue_video_feedback(app.world_mut(),&fresh,Feedback::Position{seconds:0.25,duration:2.0}));
        consume(app.world_mut());
        assert_eq!(app.world().resource::<VideoFeedbackQueue>().0.len(),1);
        assert_eq!(app.world().resource::<VideoFeedbackQueue>().0[0].0,fresh);
        // Capture a capability from the actual parked descriptor, not from an
        // invented identity. The pre-Park revision has been superseded.
        app.world_mut().resource_mut::<VnEngine>().0.execute_timer_action("call C").unwrap();
        app.world_mut().resource_mut::<VnEngine>().0.step_until_interaction().unwrap();
        consume(app.world_mut());
        let game=&app.world().resource::<VnEngine>().0;
        let task=game.state.scene_execution.as_ref().unwrap().resources.tasks.iter()
            .find(|task|task.kind==ResourceTaskKind::Video&&task.alias()=="gameclip").unwrap();
        assert!(task.parked);
        let mut parked=fresh.clone();parked.resource_token=Some(rvn_core::scene_resources::ResourceToken{
            task:rvn_core::scene_tasks::TaskToken{runtime_epoch:game.runtime_epoch(),key:task.key.clone(),revision:task.revision},target:task.target.clone()});
        assert!(queue_video_feedback(app.world_mut(),&parked,Feedback::Position{seconds:f64::NAN,duration:f64::NAN}));
        let before=full(app.world());host_frame(app.world_mut());
        assert_eq!(full(app.world()),before);
        let queue=app.world().resource::<VideoFeedbackQueue>();assert_eq!(queue.0.len(),1);assert_eq!(queue.0[0].0,parked);
    }
}

#[cfg(test)]
mod standalone_game_tasks_0915 {
 use super::*;
 fn engine(source:&str)->VnEngine{VnEngine(rvn_core::Engine::from_program(rvn_parser::parse_program(source).unwrap(),crate::bevy_renderer::BevyRenderer::new(),8).unwrap())}
 #[test]fn real_timer_game_without_audio_enters_existing_frame_gate(){
  let mut game=engine("label start\nflow.consume_v2(\"scope\",\"timer\"){timer 0.5 => jump done}\n\"hold\"\nlabel done\n\"done\"\n");
  game.0.step_until_interaction().unwrap();assert!(game.0.state.scene_execution.is_none());assert!(game.0.state.game_audio.is_none());assert!(owns_game_frame(&game.0.state));
 }
 #[test]fn raw_legacy_keeps_no_owned_frame_gate(){let game=engine("label start\ntimer 0.5 => jump start\n\"hold\"\n");assert!(!owns_game_frame(&game.0.state));assert!(game.0.state.game_tasks.is_none());}
 #[test]fn timer_clock_publication_does_not_reissue_existing_story_sprite_commands(){
  let mut game=engine("label start\nhero.show(\"hero.png\") at center\nhero.animate(\"bounce\")\nflow.consume_v2(\"scope\",\"timer\"){timer 0.5 => jump done}\n\"hold\"\nlabel done\n");game.0.step_until_interaction().unwrap();let before=game.0.state.clone();game.0.tick_game_frame(0.125,Default::default()).unwrap();
  let commands=presentation_difference(Some(&before),&game.0.state,&[],false);
  assert!(!commands.iter().any(|c|matches!(c,VnCommand::ShowSprite{..}|VnCommand::HideSprite{..}|VnCommand::MoveSprite{..}|VnCommand::ClearSprites)));
 }
 #[test]fn real_timer_game_keeps_existing_focus_and_state_policy(){
  let mut world=World::new();world.insert_resource(engine("label start\nflow.consume_v2(\"scope\",\"timer\"){timer 0.5 => jump start}\n\"hold\"\n"));world.insert_resource(State::new(crate::resources::VnState::Waiting));
  assert!(game_active(&world));world.insert_resource(State::new(crate::resources::VnState::Menu));assert!(!game_active(&world));
  world.insert_resource(State::new(crate::resources::VnState::Waiting));world.spawn((Window{focused:false,..Default::default()},bevy::window::PrimaryWindow));assert!(!game_active(&world));
 }
}
