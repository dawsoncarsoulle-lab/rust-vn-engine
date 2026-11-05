//! Producer-captured portable save/restore requests. Cold preparation never
//! commits Core or discards an admitted request before the real operation.
use bevy::prelude::*;
use crate::resources::{VnEngine,VnState};
use rvn_core::{audio_checkpoint::{AudioRequestSource,AudioRollbackSource},save::{SaveData,SaveManager,ResumeSlot},persistent::LastResumeTarget};
use std::collections::VecDeque;

const MAX_REQUESTS:usize=8;
const MAX_REQUEST_BYTES:usize=16*1024*1024;

#[derive(Clone,Debug,PartialEq)]
pub(crate) enum SaveDestination {Manual(u32),Quick,Auto}
#[derive(Clone,Debug)]
enum Operation {
    Save {destination:SaveDestination,label:String,script:String,expected:Option<SaveData>,previous_menu:Option<rvn_core::GameState>,history_previous:Option<rvn_core::audio_checkpoint::HistoricalGameSnapshot>},
    Load {data:SaveData,resume:Option<LastResumeTarget>,close_menus:bool},
    Rollback {source:AudioRollbackSource,preview:SaveData},
    OwnedRollback {source:AudioRollbackSource},
}
#[derive(Clone,Debug)]
struct Request {source:AudioRequestSource,operation:Operation,bytes:usize}
#[derive(Resource,Default)]
pub(crate) struct CheckpointRequests {requests:VecDeque<Request>,failed:Option<String>}
impl CheckpointRequests {
    pub(crate) fn blocked(&self)->bool {!self.requests.is_empty()}
    fn admit(&mut self,request:Request)->Result<(),String> {
        if self.failed.is_some(){return Err("A checkpoint request is refused; cancel it explicitly before another request".into());}
        let used=self.requests.iter().map(|request|request.bytes).fold(0usize,usize::saturating_add);
        if self.requests.len()>=MAX_REQUESTS || request.bytes>MAX_REQUEST_BYTES.saturating_sub(used) {
            return Err("Portable checkpoint request queue capacity exceeded; no request was admitted".into());
        }
        self.requests.push_back(request);Ok(())
    }
    /// Internal explicit cancellation; no UI action is exposed by this package.
    /// A failed exact request otherwise remains latched until its source is revoked.
    pub(crate) fn cancel(&mut self) {self.requests.clear();self.failed=None;}
    pub(crate) fn menu_history_retained(&self,engine:&VnEngine)->Result<bool,String> {
        let Some(request)=self.requests.back() else{return Ok(false);};
        if !engine.0.accepts_audio_request_source(&request.source){return Err("Menu checkpoint request lost its producer source before history attachment".into());}
        Ok(matches!(&request.operation,Operation::Save{previous_menu:Some(_),..}))
    }
}
pub(crate) struct CheckpointRequestsPlugin;
impl Plugin for CheckpointRequestsPlugin {
    fn build(&self,app:&mut App) {
        app.init_resource::<CheckpointRequests>().add_systems(PostUpdate,process);
    }
}
pub(crate) fn unblocked(requests:Option<Res<CheckpointRequests>>)->bool {
    requests.is_none_or(|requests|!requests.blocked())
}
pub(crate) fn world_blocked(world:&World)->bool {
    world.get_resource::<CheckpointRequests>().is_some_and(CheckpointRequests::blocked)
}
fn has_audio(state:&rvn_core::GameState)->Result<bool,String> {
    Ok(!rvn_core::audio_playback::describe(state,0)?.is_empty()
        || !rvn_core::audio_checkpoint::music_views(state,0)?.is_empty())
}
fn saved_audio(data:&SaveData)->Result<bool,String> {has_audio(&data.clone().into_game_state())}
fn encoded_bytes(data:&SaveData)->Result<usize,String> {serde_json::to_vec(data).map(|bytes|bytes.len()).map_err(|error|error.to_string())}
pub(crate) fn defer_save(engine:&VnEngine,queue:Option<&mut CheckpointRequests>,destination:SaveDestination,
    label:String,script:String,expected:Option<SaveData>)->Result<bool,String> {
    defer_save_with_history(engine,queue,destination,label,script,expected,None,None)
}
pub(crate) fn defer_menu_save(engine:&VnEngine,queue:Option<&mut CheckpointRequests>,destination:SaveDestination,
    label:String,script:String,expected:Option<SaveData>,previous:&rvn_core::GameState,proof:Option<&rvn_core::audio_checkpoint::HistoricalGameSnapshot>)->Result<bool,String> {
    if engine.0.state.scene_execution.is_some() {
        let proof=proof.ok_or("Missing pre-callback historical menu witness")?;
        if !engine.0.accepts_source_menu_snapshot(previous,proof).map_err(|error|error.to_string())? {return Err("Stale historical menu witness before save admission".into());}
    }
    defer_save_with_history(engine,queue,destination,label,script,expected,Some(previous),proof)
}
fn defer_save_with_history(engine:&VnEngine,queue:Option<&mut CheckpointRequests>,destination:SaveDestination,
    label:String,script:String,expected:Option<SaveData>,previous:Option<&rvn_core::GameState>,proof:Option<&rvn_core::audio_checkpoint::HistoricalGameSnapshot>)->Result<bool,String> {
    if engine.0.state.scene_execution.is_none(){return Ok(false);}
    // Capture authority before examining the delayed file/history payload.
    let source=engine.0.audio_request_source().map_err(|error|error.to_string())?;
    if !has_audio(&engine.0.state)? && !engine.0.renderer.scene_batch_pending(){return Ok(false);}
    let queue=queue.ok_or("Portable audio saving requires the checkpoint request consumer")?;
    let extra=previous.map(|state|serde_json::to_vec(state).map(|bytes|bytes.len()).map_err(|error|error.to_string())).transpose()?.unwrap_or(0);
    let proof_bytes=proof.map(|proof|proof.payload_bytes()).transpose()?.unwrap_or(0);
    let bytes=label.len().saturating_add(script.len()).saturating_add(expected.as_ref().map(encoded_bytes).transpose()?.unwrap_or(0)).saturating_add(extra).saturating_add(proof_bytes);
    queue.admit(Request{source,operation:Operation::Save{destination,label,script,expected,previous_menu:previous.cloned(),history_previous:proof.cloned()},bytes})?;Ok(true)
}
pub(crate) fn defer_load(engine:&VnEngine,queue:Option<&mut CheckpointRequests>,data:&SaveData,
    resume:Option<LastResumeTarget>,close_menus:bool)->Result<bool,String> {
    let source=engine.0.audio_request_source().map_err(|error|error.to_string())?;
    if !saved_audio(data)? {return Ok(false);}
    let queue=queue.ok_or("Portable audio loading requires cold checkpoint preparation")?;
    data.validate_checkpoint_shape().map_err(|error|error.to_string())?;
    let bytes=encoded_bytes(data)?;
    queue.admit(Request{source,operation:Operation::Load{data:data.clone(),resume,close_menus},bytes})?;Ok(true)
}
pub(crate) fn defer_rollback(engine:&VnEngine,queue:Option<&mut CheckpointRequests>)->Result<bool,String> {
    if engine.0.state.audio_resource_registry().is_none(){return Ok(false);}
    let request_source=engine.0.audio_request_source().map_err(|error|error.to_string())?;
    let Some(owned)=engine.0.preview_game_audio_rollback_source().map_err(|error|error.to_string())?else{return Ok(false);};
    let actual=engine.0.audio_rollback_state_for_warming(&owned).map_err(|error|error.to_string())?;
    if !has_audio(&actual)?{return Ok(false);}
    // Native frames are intentionally outside the historical SaveData1-14 wire.
    // Retain the real opaque visit, never import its data-only preview as authority.
    if engine.0.state.game_audio.is_some()||actual.consumer_frame.is_some()||actual.scene_execution.as_ref().is_some_and(|execution|execution.active.as_ref().is_some_and(|activation|activation.consumer_frame.is_some())||execution.frames.iter().any(|frame|frame.parked.as_ref().is_some_and(|activation|activation.consumer_frame.is_some()))){
        let queue=queue.ok_or("Owned audio rollback requires the real checkpoint consumer")?;
        let bytes=serde_json::to_vec(&actual).map_err(|error|error.to_string())?.len();
        queue.admit(Request{source:request_source,operation:Operation::OwnedRollback{source:owned},bytes})?;return Ok(true);
    }
    let Some((source,preview))=engine.0.preview_game_audio_rollback().map_err(|error|error.to_string())? else{return Ok(false);};
    if !saved_audio(&preview)? {return Ok(false);}
    let queue=queue.ok_or("Portable audio rollback requires cold checkpoint preparation")?;
    let bytes=encoded_bytes(&preview)?;
    queue.admit(Request{source:request_source,operation:Operation::Rollback{source,preview},bytes})?;Ok(true)
}
fn dialogue_presented(world:&World)->Result<bool,String> {
    let engine=world.resource::<VnEngine>();
    let Some(rvn_core::Interaction::Dialogue{text,..})=engine.0.displayed_interaction().map_err(|error|error.to_string())? else{return Ok(true);};
    if engine.0.state.scene_execution.is_none(){return Ok(true);}
    let Some(source)=engine.0.display_reveal_source() else{return Ok(false);};
    Ok(world.get_resource::<crate::display_reveal::DialogueRevealBinding>().is_some_and(|binding|
        binding.source.as_ref()==Some(&source)&&binding.text==text)
        && world.get_resource::<crate::resources::TypewriterState>().is_some_and(|typing|
            typing.full_text==rvn_core::display_reveal::rendered_rich_text(&text).plain_text()))
}
fn manager(world:&World)->Result<SaveManager,String> {
    let paths=world.get_resource::<crate::project_paths::ProjectPaths>().ok_or("Checkpoint requests have no project save directory")?;
    SaveManager::new(&paths.saves,crate::systems::save_menu::SUPPORTED_SLOTS).map_err(|error|error.to_string())
}
fn read_destination(manager:&SaveManager,destination:&SaveDestination)->Option<SaveData> {
    match destination {SaveDestination::Manual(slot)=>manager.load(*slot),SaveDestination::Quick=>manager.load_quicksave(),SaveDestination::Auto=>manager.load_autosave()}.ok()
}
fn unchanged_file(actual:&Option<SaveData>,expected:&Option<SaveData>)->Result<bool,String> {
    Ok(serde_json::to_value(actual).map_err(|error|error.to_string())?==serde_json::to_value(expected).map_err(|error|error.to_string())?)
}
fn process(world:&mut World) { process_with_warmer(world,crate::checkpoint_sources::warm_restore); }
fn process_with_warmer(world:&mut World,mut warmer:impl FnMut(&World,&SaveData)->Result<bool,String>) {
    if !world.contains_resource::<CheckpointRequests>() || !world.contains_resource::<VnEngine>() {return;}
    loop {
        let Some(request)=world.resource::<CheckpointRequests>().requests.front().cloned() else{return;};
        // Revocation is decided before DTO, metadata, assets or a failed latch.
        if !world.resource::<VnEngine>().0.accepts_audio_request_source(&request.source) {
            let mut queue=world.resource_mut::<CheckpointRequests>();queue.requests.pop_front();queue.failed=None;continue;
        }
        if world.resource::<CheckpointRequests>().failed.is_some(){return;}
        let focused=world.iter_entities().filter(|entity|entity.contains::<bevy::window::PrimaryWindow>())
            .find_map(|entity|entity.get::<Window>()).map(|window|window.focused)
            .unwrap_or_else(||world.iter_entities().filter_map(|entity|entity.get::<Window>()).all(|window|window.focused));
        let engine=world.resource::<VnEngine>();
        if !focused || engine.0.renderer.scene_frame_failed || engine.0.renderer.scene_batch_pending()
            ||world.get_resource::<crate::accessibility::Accessibility>().is_some_and(|access|access.blocked){return;}
        let result=(||->Result<bool,String>{
            match &request.operation {
                Operation::Save{destination,label,script,expected,previous_menu,history_previous}=>{
                    if !dialogue_presented(world)? {return Ok(false);}
                    // Manual/quick producers flush before admission; autosave
                    // waits for the approved dialogue's initial feedback. No
                    // observation/history is changed by a failed writer here.
                    if !world.resource::<VnEngine>().0.accepts_audio_request_source(&request.source){return Err("Save source changed before the decoder fence".into());}
                    if let Some(previous)=previous_menu {
                        if world.resource::<VnEngine>().0.state.scene_execution.is_some() {
                            let proof=history_previous.as_ref().ok_or("Missing historical witness before portable save")?;
                            if !world.resource::<VnEngine>().0.accepts_source_menu_snapshot(previous,proof).map_err(|error|error.to_string())? {return Err("Historical menu save witness was revoked before writing".into());}
                        }
                    }
                    let manager=manager(world)?;
                    if !unchanged_file(&read_destination(&manager,destination),expected)? {return Err("The saved destination changed while checkpoint preparation was pending".into());}
                    let engine=world.resource::<VnEngine>();
                    match destination {
                        SaveDestination::Manual(slot)=>engine.0.save(&manager,*slot,label.clone(),script.clone()),
                        SaveDestination::Quick=>engine.0.save_quicksave(&manager,label.clone(),script.clone()),
                        SaveDestination::Auto=>engine.0.save_autosave(&manager,label.clone(),script.clone()),
                    }.map_err(|error|error.to_string())?;
                    let saved=read_destination(&manager,destination).ok_or("Written portable checkpoint could not be read back")?;
                    let (target,slot)=match destination {
                        SaveDestination::Manual(slot)=>(LastResumeTarget::manual(*slot,saved.timestamp),None),
                        SaveDestination::Quick=>(LastResumeTarget::quicksave(saved.timestamp),Some(ResumeSlot::Quick)),
                        SaveDestination::Auto=>(LastResumeTarget::autosave(saved.timestamp),Some(ResumeSlot::Auto)),
                    };
                    record_resume(world,target);
                    if let Some(previous)=previous_menu {
                        if let Some(proof)=history_previous {world.resource_mut::<VnEngine>().0.finish_source_menu_snapshot(previous.clone(),proof).map_err(|error|error.to_string())?;}
                        else {world.resource_mut::<VnEngine>().0.finish_source_menu_transaction(previous.clone());}
                    }
                    if world.contains_resource::<crate::save_thumbnails::SaveThumbnails>() {
                        world.resource_scope(|world,mut thumbnails:Mut<crate::save_thumbnails::SaveThumbnails>| {
                            if let Some(slot)=slot {thumbnails.request_resume(slot,world.resource::<VnEngine>(),saved);}
                            else if let SaveDestination::Manual(slot)=destination {
                                let directory=&world.resource::<crate::project_paths::ProjectPaths>().saves;
                                if let Err(error)=thumbnails.persist(world.resource::<VnEngine>(),&manager,directory,*slot){warn!("Portable save thumbnail: {error}");}
                            }
                        });
                    }
                    if matches!(destination,SaveDestination::Manual(_)) {
                        if let Some(mut save)=world.get_resource_mut::<crate::systems::save_menu::SaveMenuState>() {save.close();}
                    }
                }
                Operation::Load{data,resume,close_menus}=>{
                    if !warmer(world,data)? {return Ok(false);}
                    // Recheck after actual cold decoding, still before load or UI.
                    if !world.resource::<VnEngine>().0.accepts_audio_request_source(&request.source){return Err("Load source changed during cold checkpoint preparation".into());}
                    world.resource_mut::<VnEngine>().0.load_data(data.clone()).map_err(|error|error.to_string())?;
                    if let Some(target)=resume {record_resume(world,target.clone());}
                    finish_restore(world,*close_menus);
                }
                Operation::OwnedRollback{source}=>{
                    if !world.resource::<VnEngine>().0.accepts_audio_rollback_source(source){return Err("Owned rollback visit changed during preparation".into());}
                    let Some(plan)=crate::audio_rollback_restore::prepare(world,source)?else{return Ok(false);};
                    crate::audio_rollback_restore::commit(world,plan)?;
                    finish_restore(world,false);
                }
                Operation::Rollback{source,preview}=>{
                    if !world.resource::<VnEngine>().0.accepts_audio_rollback_source(source){return Err("Rollback target was revoked before cold preparation".into());}
                    if !warmer(world,preview)? {return Ok(false);}
                    let restored=world.resource_mut::<VnEngine>().0.rollback_from_audio_source(source).map_err(|error|error.to_string())?;
                    if !restored {return Err("Rollback did not restore its exact visit".into());}
                    finish_restore(world,false);
                }
            }
            Ok(true)
        })();
        match result {
            Ok(false)=>return,
            Ok(true)=>{world.resource_mut::<CheckpointRequests>().requests.pop_front();},
            Err(message)=>{
                world.resource_mut::<CheckpointRequests>().failed=Some(message.clone());
                world.resource_mut::<VnEngine>().0.renderer.scene_frame_failed=true;
                crate::checkpoint_sources::pause_after_failure(world);
                if let Some(mut errors)=world.get_resource_mut::<crate::resources::ScriptErrorMessage>(){errors.0=message;}
                if let Some(mut next)=world.get_resource_mut::<NextState<VnState>>(){next.set(VnState::Error);}
                // The request and pending renderer/ECS publications remain intact.
                return;
            }
        }
    }
}
fn record_resume(world:&mut World,target:LastResumeTarget) {
    if let Some(mut persistent)=world.get_resource_mut::<crate::resources::PersistentDataResource>() {
        crate::systems::save_menu::record_resume_target(&mut persistent,target);
    }
}
fn finish_restore(world:&mut World,close_menus:bool) {
    // Requests are modern. Display and metrics are restored together by the
    // approved batch; no old text is flushed under the newly committed Core.
    if close_menus {
        if let Some(mut menu)=world.get_resource_mut::<crate::resources::MenuState>(){menu.return_to=None;}
        if let Some(mut save)=world.get_resource_mut::<crate::systems::save_menu::SaveMenuState>(){save.close();}
        if let Some(mut settings)=world.get_resource_mut::<crate::systems::settings_menu::SettingsMenuState>(){settings.active=false;}
        if let Some(mut skip)=world.get_resource_mut::<crate::resources::SkipMode>(){skip.active=false;}
        if let Some(mut menus)=world.get_resource_mut::<crate::menu_documents::Menus>(){menus.finish_checkpoint_restore();}
    }
    if let Some(mut next)=world.get_resource_mut::<NextState<VnState>>(){next.set(VnState::Waiting);}
}

#[cfg(test)]
#[path="checkpoint_requests_tests.rs"]
mod tests;
