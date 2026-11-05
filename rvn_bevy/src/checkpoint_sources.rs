//! PRIVATE Source-ready boundary. Decode actual authorized AssetServer bytes
//! before taking a pending publication; missing assets defer the same instant.
use bevy::{audio::{AudioSourceBundle,AudioSinkPlayback,Volume},asset::LoadState,prelude::*};
use rvn_core::{audio_checkpoint::{MusicCheckpointView,MusicSourceAuthority},scene_resources::TargetIdentity};
use crate::checkpoint_broker::{AudioCheckpointBroker,CheckpointAssetKey,BoundMusic,BoundOneShot};
use crate::checkpoint_pcm::{CheckpointPcm,PcmClip};
use std::{collections::{HashMap,HashSet},sync::Arc};

#[derive(Clone)]
struct SourceEntry {file:String,raw:Handle<AudioSource>,reader:CheckpointPcm,handle:Option<Handle<CheckpointPcm>>,entity:Option<Entity>}
#[derive(Resource,Default)]
struct PreparedSourceCache {instance:Option<u64>,epoch:Option<u64>,entries:HashMap<TargetIdentity,SourceEntry>}
pub(crate) struct PreparedSources {
    instance:u64,epoch:u64,state:rvn_core::GameState,broker:AudioCheckpointBroker,
    publication:Option<rvn_core::audio_checkpoint::AudioPublicationSource>,
    frame_audio:bool,
    entries:HashMap<TargetIdentity,SourceEntry>,one_shots:Vec<BoundOneShot>,music:Vec<(BoundMusic,f32)>,
}
#[derive(Component,Clone)]
pub(crate) struct ExactMusicSource {pub authority:MusicSourceAuthority,pub instance:u64,pub epoch:u64}

fn ready_clip(world:&World,broker:&AudioCheckpointBroker,key:CheckpointAssetKey,path:&str)->Result<Option<(Handle<AudioSource>,Arc<PcmClip>)>,String> {
    if !rvn_ui::programmable::safe_asset_path(path) || path.len()>4096 {return Err("Unsafe prepared audio asset locator".into());}
    let assets=world.get_resource::<AssetServer>().ok_or("Portable audio requires the actual asset runtime")?;
    let raw=broker.preparation_asset(path)?;
    if let Some(LoadState::Failed(error))=assets.get_load_state(raw.id()) {return Err(format!("Audio asset preparation failed: {error}"));}
    let Some(source)=world.get_resource::<Assets<AudioSource>>().and_then(|sources|sources.get(&raw)) else {return Ok(None);};
    if let Some(clip)=broker.clip(&key)? {
        use sha2::{Digest,Sha256};
        let digest:[u8;32]=Sha256::digest(&source.bytes).into();
        if clip.asset_digest()==digest {return Ok(Some((raw,clip)));}
    }
    let clip=PcmClip::decode_shared(path,source.bytes.clone(),broker.available_decode_bytes()?)?;broker.cache_clip(key,clip.clone())?;Ok(Some((raw,clip)))
}
fn previous<'a>(cache:Option<&'a PreparedSourceCache>,instance:u64,epoch:u64,target:&TargetIdentity)->Option<&'a SourceEntry> {
    cache.filter(|cache|cache.instance==Some(instance) && cache.epoch==Some(epoch))?.entries.get(target)
}
/// Cold admission performs actual asset decoding and exact metadata comparison.
/// It does not change Engine state/history/epochs, pause existing readers, take
/// pending outputs, create an Entity or submit an audio decoder. The caller keeps
/// its producer-captured Load request pending while this returns false.
pub(crate) fn warm_restore(world:&World,data:&rvn_core::save::SaveData)->Result<bool,String> {
    data.validate_checkpoint_shape().map_err(|error|error.to_string())?;
    let state=data.clone().into_game_state();
    warm_state(world,&state)
}
pub(crate) fn warm_state(world:&World,state:&rvn_core::GameState)->Result<bool,String> {
    rvn_core::audio_checkpoint::require_saved_checkpoints(state)?;
    let engine=&world.get_resource::<crate::resources::VnEngine>().ok_or("Cold audio admission requires its actual Engine")?.0;
    let broker=&engine.renderer.audio_checkpoints;
    let next_epoch=engine.runtime_epoch().checked_add(1).ok_or("Runtime generation exhausted before cold audio admission")?;
    // Settings and authored envelope ambiguity are checked before decoding and,
    // critically, before Core Load can commit. The same pure helpers are used
    // by the real current/outgoing publication; no sink or clock is modified.
    let _one_shot_gain_preflight=crate::scene_audio::prepare(world,&state,next_epoch)?;
    for view in rvn_core::audio_checkpoint::music_views(&state,next_epoch)? {
        let logical=music_logical_gain(&state,&view,next_epoch)?;
        let _mixed_gain=crate::music_mix::in_world(world,logical)?;
    }
    let shots=rvn_core::audio_playback::describe(&state,0)?;
    let music=rvn_core::audio_checkpoint::music_views(&state,0)?;
    let mut paths:Vec<_>=shots.iter().filter(|view|!matches!(view.token.task.key.owner,rvn_core::scene_tasks::TaskOwner::Host{..}))
        .map(|view|crate::scene_audio::source_path(view.playback.family,&view.playback.file)).collect();
    paths.extend(music.iter().filter(|view|!matches!(view.owner,rvn_core::scene_tasks::TaskOwner::Host{..}))
        .map(|view|world.get_resource::<crate::resources::MusicAssetRegistry>().map(|registry|registry.resolve(&view.file)).unwrap_or(view.file.clone())));
    broker.retain_preparation_assets(world.get_resource::<AssetServer>(),&paths)?;
    let mut missing=false;
    for view in shots {
        if matches!(view.token.task.key.owner,rvn_core::scene_tasks::TaskOwner::Host{..}){continue;}
        let path=crate::scene_audio::source_path(view.playback.family,&view.playback.file);
        let key=CheckpointAssetKey::one_shot(view.playback.family,&view.playback.file);
        let Some((_,clip))=ready_clip(world,broker,key,&path)? else{missing=true;continue;};
        clip.validate_checkpoint(view.playback.checkpoint.as_ref().ok_or("Cold one-shot admission has no saved cursor")?)?;
    }
    for view in music {
        if matches!(view.owner,rvn_core::scene_tasks::TaskOwner::Host{..}){continue;}
        let path=world.get_resource::<crate::resources::MusicAssetRegistry>().map(|registry|registry.resolve(&view.file)).unwrap_or(view.file.clone());
        let Some((_,clip))=ready_clip(world,broker,CheckpointAssetKey::Music(view.file.clone()),&path)? else{missing=true;continue;};
        clip.validate_checkpoint(view.checkpoint.as_ref().ok_or("Cold Music admission has no saved cursor")?)?;
    }
    if missing{return Ok(false);}
    broker.validate_restore(&state)?;Ok(true)
}
pub(crate) fn warm_frame_state(world:&World,state:&rvn_core::GameState)->Result<bool,String> {
    rvn_core::frame_audio_checkpoint::require_frame_checkpoints(state)?;
    let engine=&world.get_resource::<crate::resources::VnEngine>().ok_or("Cold audio admission requires its actual Engine")?.0;
    let broker=&engine.renderer.audio_checkpoints;
    let next_epoch=engine.runtime_epoch().checked_add(1).ok_or("Runtime generation exhausted before cold audio admission")?;
    // Settings and authored envelope ambiguity are checked before decoding and,
    // critically, before Core Load can commit. The same pure helpers are used
    // by the real current/outgoing publication; no sink or clock is modified.
    let _one_shot_gain_preflight=crate::scene_audio::prepare(world,&state,next_epoch)?;
    for view in rvn_core::audio_checkpoint::music_views(&state,next_epoch)? {
        let logical=music_logical_gain(&state,&view,next_epoch)?;
        let _mixed_gain=crate::music_mix::in_world(world,logical)?;
    }
    let shots=rvn_core::audio_playback::describe(&state,0)?;
    let music=rvn_core::audio_checkpoint::music_views(&state,0)?;
    let mut paths:Vec<_>=shots.iter()
        .map(|view|crate::scene_audio::source_path(view.playback.family,&view.playback.file)).collect();
    paths.extend(music.iter()
        .map(|view|world.get_resource::<crate::resources::MusicAssetRegistry>().map(|registry|registry.resolve(&view.file)).unwrap_or(view.file.clone())));
    broker.retain_preparation_assets(world.get_resource::<AssetServer>(),&paths)?;
    let mut missing=false;
    for view in shots {
        let path=crate::scene_audio::source_path(view.playback.family,&view.playback.file);
        let key=CheckpointAssetKey::one_shot(view.playback.family,&view.playback.file);
        let Some((_,clip))=ready_clip(world,broker,key,&path)? else{missing=true;continue;};
        clip.validate_checkpoint(view.playback.checkpoint.as_ref().ok_or("Cold one-shot admission has no saved cursor")?)?;
    }
    for view in music {
        let path=world.get_resource::<crate::resources::MusicAssetRegistry>().map(|registry|registry.resolve(&view.file)).unwrap_or(view.file.clone());
        let Some((_,clip))=ready_clip(world,broker,CheckpointAssetKey::Music(view.file.clone()),&path)? else{missing=true;continue;};
        clip.validate_checkpoint(view.checkpoint.as_ref().ok_or("Cold Music admission has no saved cursor")?)?;
    }
    if missing{return Ok(false);}
    broker.validate_frame_restore(&state)?;Ok(true)
}
pub(crate) fn prepare(world:&World,state:&rvn_core::GameState,epoch:u64)->Result<Option<PreparedSources>,String> {
    prepare_inner(world,state,epoch,false)
}
pub(crate) fn prepare_restore(world:&World,prepared:&rvn_core::engine::PreparedSave15)->Result<Option<PreparedSources>,String> {
    let batch=prepared.render_batch();
    let mut plan=prepare_inner(world,&batch.state,batch.runtime_epoch,true)?;
    if let Some(plan)=&mut plan{plan.frame_audio=prepared.requires_frame_audio_checkpoint();}
    Ok(plan)
}
fn prepare_inner(world:&World,state:&rvn_core::GameState,epoch:u64,restore:bool)->Result<Option<PreparedSources>,String> {
    let engine=&world.get_resource::<crate::resources::VnEngine>().ok_or("Source preparation requires its actual Engine")?.0;
    let instance=engine.instance_id();let broker=engine.renderer.audio_checkpoints.clone();
    let publication=if restore {None}else{Some(engine.audio_publication_source().map_err(|error|error.to_string())?)};
    prepare_owned(world,state,epoch,instance,broker,publication)
}
/// The opaque Core plan supplies state; the actual startup supplies owner/broker.
pub(crate) fn prepare_startup(world:&World,core:&rvn_core::engine::PreparedStartupStep)->Result<Option<PreparedSources>,String>{
    if world.contains_resource::<crate::resources::VnEngine>(){return Err("Startup cannot share a gameplay owner".into());}
    let startup=world.get_resource::<crate::debugger_startup::DebugBootstrap>().ok_or("Actual startup owner is missing")?;
    startup.owner.validate_prepared_step(core).map_err(|error|error.to_string())?;
    let actual=startup.owner.snapshot();
    if core.snapshot().instance!=actual.instance||core.snapshot().epoch!=actual.epoch{return Err("Startup source owner changed before media preparation".into());}
    let batch=core.render_batch().ok_or("Computation-only startup has no media publication")?;
    if batch.runtime_epoch!=actual.epoch{return Err("Startup batch belongs to another generation".into());}
    prepare_owned(world,&batch.state,batch.runtime_epoch,actual.instance,startup.broker.clone(),None)
}
fn prepare_owned(world:&World,state:&rvn_core::GameState,epoch:u64,instance:u64,broker:AudioCheckpointBroker,publication:Option<rvn_core::audio_checkpoint::AudioPublicationSource>)->Result<Option<PreparedSources>,String>{
    let shots=rvn_core::audio_playback::describe(state,epoch)?;let music=rvn_core::audio_checkpoint::music_views(state,epoch)?;
    if (!shots.is_empty() || !music.is_empty()) && world.get_resource::<Assets<CheckpointPcm>>().is_none() {return Err("CheckpointPcm asset runtime has not been registered".into());}
    // All gains are validated before issuing even an asynchronous asset request.
    let shot_paths:Vec<_>=shots.iter().map(|view|crate::scene_audio::source_path(view.playback.family,&view.playback.file)).collect();
    let mut music_gains=Vec::with_capacity(music.len());
    for view in &music {
        let logical=music_logical_gain(state,view,epoch)?;
        music_gains.push(crate::music_mix::in_world(world,logical)?);
    }
    let mut paths=shot_paths.clone();
    paths.extend(music.iter().map(|view|world.get_resource::<crate::resources::MusicAssetRegistry>().map(|registry|registry.resolve(&view.file)).unwrap_or(view.file.clone())));
    broker.retain_preparation_assets(world.get_resource::<AssetServer>(),&paths)?;
    let cache=world.get_resource::<PreparedSourceCache>();let mut entries=HashMap::new();let mut one_shots=Vec::new();let mut bound_music=Vec::new();let mut missing=false;
    for (view,path) in shots.into_iter().zip(shot_paths) {
        let identity=view.token.target.clone();
        let entry=if let Some(old)=previous(cache,instance,epoch,&identity) {
            if old.file!=view.playback.file {return Err("Exact one-shot target cannot adopt a changed source".into());}old.clone()
        } else {
            let key=CheckpointAssetKey::one_shot(view.playback.family,&view.playback.file);
            let Some((raw,clip))=ready_clip(world,&broker,key,&path)? else {missing=true;continue;};
            let reader=match &view.playback.checkpoint {Some(saved)=>clip.restore(broker.gate(),saved,view.parked)?,None=>clip.start(broker.gate(),view.parked)};
            SourceEntry{file:view.playback.file.clone(),raw,reader,handle:None,entity:None}
        };
        one_shots.push(BoundOneShot{token:view.token,file:view.playback.file,family:view.playback.family,parked:view.parked,reader:entry.reader.clone()});
        entries.insert(identity,entry);
    }
    let mut handed_off=HashSet::new();
    for (view,gain) in music.into_iter().zip(music_gains) {
        let identity=view.authority.target.clone();
        let entry=if let Some(old)=previous(cache,instance,epoch,&identity) {
            if old.file!=view.file {return Err("Exact Music target cannot adopt a changed file".into());}old.clone()
        } else {
            let historical=state.audio_resource_registry().and_then(|resources|resources.tasks.iter().find(|task|task.target==identity))
                .and_then(|task|task.effect.as_ref()).and_then(|effect|effect.presentation.as_ref());
            let original=historical.and_then(|presentation|previous(cache,instance,epoch,&presentation.original_target.identity))
                .filter(|entry|entry.file==view.file);
            if let Some(old)=original {
                let original_id=&historical.unwrap().original_target.identity;
                let retained=state.audio_resource_registry().ok_or("Historical Music has no actual registry")?.targets.iter().any(|record|record.identity==*original_id);
                if !retained && handed_off.insert(original_id.clone()) {old.clone()} else {
                    // An autonomous copy is initialized from a real fenced
                    // cursor, never a source restart inferred from an alias.
                    let observed=broker.gate().snapshot(&[&old.reader],|mut rows|Ok(rows.remove(0)))?;
                    SourceEntry{file:view.file.clone(),raw:old.raw.clone(),reader:old.reader.clip().restore(broker.gate(),&observed,view.parked)?,handle:None,entity:None}
                }
            } else {
                let path=world.get_resource::<crate::resources::MusicAssetRegistry>().map(|registry|registry.resolve(&view.file)).unwrap_or(view.file.clone());
                let Some((raw,clip))=ready_clip(world,&broker,CheckpointAssetKey::Music(view.file.clone()),&path)? else {missing=true;continue;};
                let reader=match &view.checkpoint {
                    Some(saved)=>clip.restore(broker.gate(),saved,view.parked)?,
                    None if view.authority.outgoing.is_none() || (cache.is_none() && epoch==0)=>clip.start_looped(broker.gate(),view.parked),
                    None=>return Err("Historical Music has neither a real resident Source nor an exact saved checkpoint".into()),
                };
                SourceEntry{file:view.file.clone(),raw,reader,handle:None,entity:None}
            }
        };
        bound_music.push((BoundMusic{authority:view.authority,file:view.file,owner:view.owner,parked:view.parked,reader:entry.reader.clone()},gain));entries.insert(identity,entry);
    }
    if missing {return Ok(None);}
    Ok(Some(PreparedSources{frame_audio:false,instance,epoch,state:state.clone(),broker,publication,entries,one_shots,music:bound_music}))
}
fn music_logical_gain(state:&rvn_core::GameState,view:&MusicCheckpointView,epoch:u64)->Result<f32,String> {
    if view.authority.outgoing.is_none() {return crate::systems::audio::current_music_gain_from_state(state,epoch);}
    let resources=state.audio_resource_registry().ok_or("Historical Music has no registry")?;
    let effect=resources.tasks.iter().find(|task|task.target==view.authority.target).and_then(|task|task.effect.as_ref()).ok_or("Historical Music effect disappeared")?;
    let presentation=effect.presentation.as_ref().ok_or("Historical Music payload disappeared")?;
    let frozen=rvn_core::retired_sampling::freeze_appearance(presentation,None,false)?;
    match rvn_core::retired_sampling::sample_outgoing(&frozen,effect,false)? {rvn_core::retired_sampling::RetiredFrame::Music{gain,..}=>Ok(gain),_=>Err("Music target contains a visual payload".into())}
}
/// Last fallible publication step, before pending/output ownership is taken.
/// Actual cursor capture + Core/history metadata + pause residence share one
/// fence. The plan has not spawned an entity or submitted a new decoder yet.
pub(crate) fn publish(world:&mut World,plan:&PreparedSources)->Result<(),String> {
    let one_shots=plan.one_shots.clone();let music=plan.music.iter().map(|(row,_)|row.clone()).collect();
    plan.broker.publish(plan.instance,plan.epoch,&plan.state,one_shots,music,|snapshot| {
        world.resource_mut::<crate::resources::VnEngine>().0.record_audio_publication(plan.publication.as_ref().ok_or("Restore plan requires its exclusive Core commit guard")?,snapshot).map_err(|error|error.to_string())?;Ok(())
    })
}
pub(crate) fn verify_restore_assets(world:&World,plan:&PreparedSources)->Result<(),String> {
    if plan.publication.is_some(){return Err("Ordinary audio plan cannot authorize candidate restore".into());}
    if !plan.entries.is_empty() && world.get_resource::<Assets<CheckpointPcm>>().is_none(){return Err("CheckpointPcm assets vanished before commit".into());}
    if !plan.music.is_empty() && world.get_resource::<crate::resources::MusicEntity>().is_none(){return Err("Music entity registry is missing".into());}
    let server=world.get_resource::<AssetServer>().ok_or("Actual asset runtime vanished")?;
    for entry in plan.entries.values(){
        if matches!(server.get_load_state(entry.raw.id()),Some(LoadState::Failed(_))){return Err("Prepared audio asset failed before commit".into());}
        let raw=world.get_resource::<Assets<AudioSource>>().and_then(|assets|assets.get(&entry.raw)).ok_or("Prepared raw audio asset vanished")?;
        use sha2::{Digest,Sha256};
        let digest:[u8;32]=Sha256::digest(&raw.bytes).into();
        if digest!=entry.reader.clip().asset_digest(){return Err("Prepared audio bytes changed before commit".into());}
    }
    for row in &plan.one_shots {
        let entry=plan.entries.get(&row.token.target).ok_or("Prepared one-shot source disappeared")?;
        let path=crate::scene_audio::source_path(row.family,&row.file);
        if server.get_path(entry.raw.id()).is_none_or(|actual|actual.path()!=std::path::Path::new(&path)){return Err("Prepared one-shot locator changed".into());}
    }
    for (row,_) in &plan.music {
        let entry=plan.entries.get(&row.authority.target).ok_or("Prepared music source disappeared")?;
        let path=world.get_resource::<crate::resources::MusicAssetRegistry>().map(|r|r.resolve(&row.file)).unwrap_or(row.file.clone());
        if server.get_path(entry.raw.id()).is_none_or(|actual|actual.path()!=std::path::Path::new(&path)){return Err("Prepared music locator changed".into());}
    }
    Ok(())
}
pub(crate) fn publish_restore<R:rvn_core::renderer::Renderer>(plan:&PreparedSources,guard:rvn_core::engine::Save15CommitGuard<'_,R>,settings:&mut crate::systems::settings_menu::Settings,selected:String)->Result<Option<rvn_core::engine::DebugSession>,String> {
    publish_restore_with_retirement(plan,guard,settings,selected,&mut ||{})
}
pub(crate) fn publish_restore_with_retirement<R:rvn_core::renderer::Renderer>(plan:&PreparedSources,guard:rvn_core::engine::Save15CommitGuard<'_,R>,settings:&mut crate::systems::settings_menu::Settings,selected:String,retire:&mut dyn FnMut())->Result<Option<rvn_core::engine::DebugSession>,String>{
    if plan.publication.is_some(){return Err("Ordinary plan cannot commit Save15".into());}
    let commit=|snapshot| {
        let session=guard.commit_with_audio_checkpoint(snapshot).map_err(|error|error.to_string())?;
        settings.language=selected;
        retire(); // Same gate: old cached stereo halves cannot escape publication.
        Ok(session)
    };
    if plan.frame_audio{plan.broker.publish_frame(plan.instance,plan.epoch,&plan.state,plan.one_shots.clone(),plan.music.iter().map(|(row,_)|row.clone()).collect(),commit)}
    else{plan.broker.publish(plan.instance,plan.epoch,&plan.state,plan.one_shots.clone(),plan.music.iter().map(|(row,_)|row.clone()).collect(),commit)}
}
/// After all preflights succeed. Only real prepared Sources are installed.
pub(crate) fn install(world:&mut World,mut plan:PreparedSources) {
    let mut old=world.remove_resource::<PreparedSourceCache>().unwrap_or_default();
    let wanted_entities:HashSet<_>=plan.entries.values().filter_map(|entry|entry.entity).collect();
    for entry in old.entries.values() {
        if !plan.entries.values().any(|current|current.reader.same_reader(&entry.reader)) {let _=entry.reader.stop();}
        if let Some(entity)=entry.entity.filter(|entity|!wanted_entities.contains(entity)) {
            if world.get_resource::<crate::resources::MusicEntity>().is_some_and(|current|current.0==Some(entity)){world.resource_mut::<crate::resources::MusicEntity>().0=None;}
            if let Some(sink)=world.get::<AudioSink>(entity){sink.stop();}if let Some(entity)=world.get_entity_mut(entity){entity.despawn();}
        }
    }
    for entry in plan.entries.values_mut() {
        if entry.handle.is_none(){entry.handle=Some(world.resource_mut::<Assets<CheckpointPcm>>().add(entry.reader.clone()));}
    }
    // Current and outgoing Music each keep exactly one player. Retired may
    // receive an old Current entity without recreating its decoder.
    for (binding,gain) in &plan.music {
        let entry=plan.entries.get_mut(&binding.authority.target).unwrap();
        let entity=entry.entity.filter(|entity|world.get_entity(*entity).is_some()).unwrap_or_else(||world.spawn(AudioSourceBundle::<CheckpointPcm>{source:entry.handle.clone().unwrap(),settings:PlaybackSettings::ONCE.with_volume(Volume::new(*gain)),..default()}).id());
        entry.entity=Some(entity);
        world.entity_mut(entity).insert(ExactMusicSource{authority:binding.authority.clone(),instance:plan.instance,epoch:plan.epoch});
        if let Some(task)=&binding.authority.outgoing {
            if world.get_resource::<crate::resources::MusicEntity>().is_some_and(|current|current.0==Some(entity)){world.resource_mut::<crate::resources::MusicEntity>().0=None;}
            world.entity_mut(entity).remove::<(crate::components::MusicMarker,crate::scene_effects::AudioLease,crate::components::AudioFade)>();
            world.entity_mut(entity).insert(crate::scene_retired::RetiredProjection{token:rvn_core::scene_resources::ResourceToken{task:task.clone(),target:binding.authority.target.clone()}});
        } else {
            world.entity_mut(entity).remove::<crate::scene_retired::RetiredProjection>();world.entity_mut(entity).insert(crate::components::MusicMarker);world.resource_mut::<crate::resources::MusicEntity>().0=Some(entity);
        }
        if let Some(mut settings)=world.get_mut::<PlaybackSettings>(entity){settings.volume=Volume::new(*gain);settings.paused=binding.parked;}
        if let Some(sink)=world.get::<AudioSink>(entity){sink.set_volume(*gain);if binding.parked {sink.pause();}else{sink.play();}}
    }
    old.instance=Some(plan.instance);old.epoch=Some(plan.epoch);old.entries=plan.entries;world.insert_resource(old);
}
pub(crate) fn publish_startup(plan:&PreparedSources,guard:rvn_core::engine::StartupHostStepGuard<'_,crate::bevy_renderer::BevyRenderer>)->Result<rvn_core::engine::CommittedStartupStep<crate::bevy_renderer::StartupPublication>,String>{
    publish_startup_with_retirement(plan,guard,&mut||{})
}
pub(crate) fn publish_startup_with_retirement(plan:&PreparedSources,guard:rvn_core::engine::StartupHostStepGuard<'_,crate::bevy_renderer::BevyRenderer>,retire:&mut dyn FnMut())->Result<rvn_core::engine::CommittedStartupStep<crate::bevy_renderer::StartupPublication>,String>{
    if plan.publication.is_some()||plan.frame_audio{return Err("Other publication plans cannot authorize startup".into());}
    if guard.host_publication().is_none_or(|prepared|!prepared.accepts_broker(&plan.broker)){return Err("Startup publication must use its actual renderer broker gate".into());}
    let batch=guard.render_batch().ok_or("Computation-only startup has no media fence")?;
    if guard.snapshot().instance!=plan.instance||batch.runtime_epoch!=plan.epoch||serde_json::to_vec(&batch.state).map_err(|e|e.to_string())?!=serde_json::to_vec(&plan.state).map_err(|e|e.to_string())?{
        return Err("Prepared startup media does not match its exclusive Core guard".into());
    }
    plan.broker.publish_frame(plan.instance,plan.epoch,&plan.state,plan.one_shots.clone(),plan.music.iter().map(|(row,_)|row.clone()).collect(),|snapshot|{let committed=guard.commit_with_audio_checkpoint(snapshot).map_err(|error|error.to_string())?;retire();Ok(committed)})
}
pub(crate) fn one_shot_source(world:&World,target:&TargetIdentity)->Option<(CheckpointPcm,Handle<CheckpointPcm>)> {
    let cache=world.get_resource::<PreparedSourceCache>()?;
    let (instance,epoch)=if let Some(engine)=world.get_resource::<crate::resources::VnEngine>(){(engine.0.instance_id(),engine.0.runtime_epoch())}else{let startup=world.get_resource::<crate::debugger_startup::DebugBootstrap>()?;let snapshot=startup.owner.snapshot();(snapshot.instance,snapshot.epoch)};
    let entry=previous(Some(cache),instance,epoch,target)?;
    Some((entry.reader.clone(),entry.handle.clone()?))
}
pub(crate) fn checkpointed_music(world:&World)->bool {
    world.get_resource::<crate::resources::VnEngine>().is_some_and(|engine|engine.0.state.audio_resource_registry().is_some())
}
pub(crate) fn pause_after_failure(world:&World) {
    let Some(cache)=world.get_resource::<PreparedSourceCache>() else{return;};
    let Some(engine)=world.get_resource::<crate::resources::VnEngine>() else{return;};
    if cache.instance!=Some(engine.0.instance_id()) || cache.epoch!=Some(engine.0.runtime_epoch()){return;}
    let readers:Vec<_>=cache.entries.values().map(|entry|(&entry.reader,true)).collect();
    // Existing failure already refuses all writers. A poisoned/stopped reader
    // cannot become valid again through this cleanup path.
    let _=engine.0.renderer.audio_checkpoints.gate().publish(&readers,|_|Ok(()));
}

#[cfg(test)]
#[path="checkpoint_source_tests.rs"]
mod source_contract_tests;

#[cfg(test)]
#[path="checkpoint_async_source_tests.rs"]
mod async_source_contract_tests;

#[cfg(test)]
pub(crate) fn fixture_wave_for_test()->Vec<u8>{source_contract_tests::wave()}

// Existing consumer fixtures use the same actual Source preflight/fence/install.
// No fake cursor, elapsed clock, sink or native-device acceptance is supplied.
#[cfg(test)]
pub(crate) fn source_app_for_test(game:rvn_core::Engine<crate::bevy_renderer::BevyRenderer>)->App {
    source_contract_tests::app_from_game(game)
}
#[cfg(test)]
pub(crate) fn install_ready_sources_for_test(world:&mut World)->Result<(),String> {
    prime_assets_for_test(world)?;
    let game=&world.resource::<crate::resources::VnEngine>().0;
    let plan=prepare(world,&game.state,game.runtime_epoch())?.ok_or("Fixture real WAV Source is not ready")?;
    publish(world,&plan)?;install(world,plan);Ok(())
}
#[cfg(test)]
pub(crate) fn music_entity_for_test(world:&World,target:&TargetIdentity)->Option<Entity> {
    world.iter_entities().find(|entity|entity.get::<ExactMusicSource>()
        .is_some_and(|source|source.authority.target==*target)).map(|entity|entity.id())
}

/// Only old consumer fixtures use deterministic readiness. The new source
/// contracts above exercise real pending/missing assets without this helper.
#[cfg(test)]
pub(crate) fn prime_assets_for_test(world:&mut World)->Result<(),String> {
    let engine=&world.resource::<crate::resources::VnEngine>().0;let state=engine.state.clone();let epoch=engine.runtime_epoch();
    let _actual_gain_preflight=crate::scene_audio::prepare(world,&state,epoch)?;
    let mut paths:Vec<_>=rvn_core::audio_playback::describe(&state,epoch)?.iter().map(|view|crate::scene_audio::source_path(view.playback.family,&view.playback.file)).collect();
    paths.extend(rvn_core::audio_checkpoint::music_views(&state,epoch)?.iter().map(|view|world.resource::<crate::resources::MusicAssetRegistry>().resolve(&view.file)));paths.sort();paths.dedup();
    for path in paths {let handle=world.resource::<AssetServer>().load::<AudioSource>(path);world.resource_mut::<Assets<AudioSource>>().insert(handle.id(),AudioSource{bytes:Arc::from(fixture_wave_for_test())});}
    Ok(())
}

/// Only an opaque retained Core visit supplies this future state/epoch.
pub(crate) fn prepare_audio_rollback(world:&World,prepared:&rvn_core::engine::PreparedAudioRollback)->Result<Option<PreparedSources>,String>{let batch=prepared.render_batch();prepare_inner(world,&batch.state,batch.runtime_epoch,true)}
pub(crate) fn publish_audio_rollback<R:rvn_core::Renderer>(plan:&PreparedSources,guard:rvn_core::engine::AudioRollbackCommitGuard<'_,R>)->Result<(),String>{
 if plan.publication.is_some()||plan.frame_audio{return Err("Wrong Source plan cannot authorize audio rollback".into());}
 let batch=guard.render_batch();
 if batch.runtime_epoch!=plan.epoch||serde_json::to_vec(&batch.state).map_err(|e|e.to_string())?!=serde_json::to_vec(&plan.state).map_err(|e|e.to_string())?{return Err("Rollback Source plan differs from its exclusive Core guard".into());}
 plan.broker.publish(plan.instance,plan.epoch,&plan.state,plan.one_shots.clone(),plan.music.iter().map(|(row,_)|row.clone()).collect(),|snapshot|guard.commit_with_audio_checkpoint(snapshot).map_err(|e|e.to_string()))
}

#[cfg(all(test, not(target_arch="wasm32")))]
#[path="music_policy_audio_tests1003.rs"]
mod music_policy_audio1003;
