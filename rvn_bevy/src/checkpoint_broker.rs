//! PRIVATE: one renderer fence for exact Source cursors. No Entity or clock is saved.
use bevy::prelude::*;
use rvn_core::audio_checkpoint::{self,AudioCheckpointSnapshot,AudioCheckpointRow,MusicCheckpoint,MusicSourceAuthority};
use rvn_core::audio_playback::AudioFamily;
use rvn_core::scene_resources::ResourceToken;
use rvn_core::scene_tasks::TaskOwner;
use std::{collections::{HashMap,HashSet},sync::{Arc,Mutex}};
use crate::checkpoint_pcm::{CheckpointGate,CheckpointPcm,PcmClip};

const MAX_SOURCE_ROWS:usize=rvn_core::audio_playback::MAX_AUDIO_PLAYBACKS+rvn_core::scene_resources::MAX_MOTIONS+1;
const MAX_DECODED_CACHE_BYTES:u64=256*1024*1024;
const MAX_CACHED_CLIPS:usize=rvn_core::scene_resources::MAX_RESOURCES;

#[derive(Clone,Debug,PartialEq,Eq,Hash)]
pub(crate) enum CheckpointAssetKey {Sfx(String),Voice(String),Music(String)}
impl CheckpointAssetKey {
    pub(crate) fn one_shot(family:AudioFamily,file:&str)->Self {
        match family {AudioFamily::Sfx=>Self::Sfx(file.into()),AudioFamily::Voice=>Self::Voice(file.into())}
    }
}

#[derive(Clone)]
pub(crate) struct BoundOneShot {
    pub token:ResourceToken,pub file:String,pub family:AudioFamily,pub parked:bool,pub reader:CheckpointPcm,
}
#[derive(Clone)]
pub(crate) struct BoundMusic {
    pub authority:MusicSourceAuthority,pub file:String,pub owner:TaskOwner,pub parked:bool,pub reader:CheckpointPcm,
}
#[derive(Default)]
struct BrokerState {
    instance_id:Option<u64>,epoch:Option<u64>,one_shots:Vec<BoundOneShot>,music:Vec<BoundMusic>,
    clips:HashMap<CheckpointAssetKey,Arc<PcmClip>>,
    preparation_assets:HashMap<String,Handle<bevy::audio::AudioSource>>,
}

#[derive(Clone,Resource,Default)]
pub(crate) struct AudioCheckpointBroker {gate:CheckpointGate,state:Arc<Mutex<BrokerState>>}
impl AudioCheckpointBroker {
    pub(crate) fn gate(&self)->CheckpointGate {self.gate.clone()}
    /// A loading Handle must survive between preflight attempts. This is only
    /// a bounded asset-retention cache: it grants no Source authority, creates
    /// no player and changes neither Core nor decoder cursors. Cold restore
    /// and publication use the same cache and replace its requested path set.
    pub(crate) fn retain_preparation_assets(&self,assets:Option<&AssetServer>,paths:&[String])->Result<(),String> {
        let wanted:HashSet<&String>=paths.iter().collect();
        if wanted.len()>MAX_SOURCE_ROWS || wanted.iter().any(|path|path.len()>4096 || !rvn_ui::programmable::safe_asset_path(path)) {
            return Err("Prepared audio asset retention exceeds its path bounds".into());
        }
        let assets=if wanted.is_empty(){None}else{Some(assets.ok_or("Portable audio requires the actual asset runtime")?)};
        let mut state=self.state.lock().map_err(|_|"Audio checkpoint broker is poisoned")?;
        let mut handles=HashMap::with_capacity(wanted.len());
        for path in wanted {
            let handle=state.preparation_assets.get(path).cloned()
                .unwrap_or_else(||assets.unwrap().load::<bevy::audio::AudioSource>(path.clone()));
            handles.insert(path.clone(),handle);
        }
        state.preparation_assets=handles;Ok(())
    }
    pub(crate) fn preparation_asset(&self,path:&str)->Result<Handle<bevy::audio::AudioSource>,String> {
        self.state.lock().map_err(|_|"Audio checkpoint broker is poisoned")?.preparation_assets.get(path).cloned()
            .ok_or_else(||"Audio asset was not retained by its preparation preflight".into())
    }
    /// Remaining space while every old cache/source allocation is still alive.
    /// The decoder reserves from this before its Vec and final Arc can coexist.
    pub(crate) fn available_decode_bytes(&self)->Result<u64,String> {
        let state=self.state.lock().map_err(|_|"Audio checkpoint broker is poisoned")?;
        let mut allocations=HashMap::new();
        for clip in state.clips.values(){allocations.insert(clip.allocation_key(),clip.pcm_bytes());}
        for reader in state.one_shots.iter().map(|row|&row.reader).chain(state.music.iter().map(|row|&row.reader)){allocations.insert(reader.clip().allocation_key(),reader.clip().pcm_bytes());}
        let used=allocations.values().try_fold(0u64,|sum,bytes|sum.checked_add(*bytes).ok_or("PCM buffer budget overflow"))?;
        MAX_DECODED_CACHE_BYTES.checked_sub(used).ok_or_else(||"Retained PCM buffers exceed the preparation budget".into())
    }

    /// Preflight/cache decoded authorized asset bytes before publication. Unique
    /// immutable clips share their PCM allocation; a cache miss is never cursor 0.
    pub(crate) fn cache_clip(&self,key:CheckpointAssetKey,clip:Arc<PcmClip>)->Result<(),String> {
        self.cache_clip_with_budget(key,clip,MAX_DECODED_CACHE_BYTES)
    }
    fn cache_clip_with_budget(&self,key:CheckpointAssetKey,clip:Arc<PcmClip>,limit:u64)->Result<(),String> {
        self.cache_clip_with_limits(key,clip,limit,MAX_CACHED_CLIPS)
    }
    fn cache_clip_with_limits(&self,key:CheckpointAssetKey,clip:Arc<PcmClip>,limit:u64,key_limit:usize)->Result<(),String> {
        let mut state=self.state.lock().map_err(|_|"Audio checkpoint broker is poisoned")?;
        if !state.clips.contains_key(&key) && state.clips.len()>=key_limit {return Err("Prepared PCM asset metadata exceeds its explicit cache capacity".into());}
        let mut sizes=HashMap::new();
        for (existing_key,existing) in &state.clips {
            if *existing_key!=key {sizes.insert(existing.allocation_key(),existing.pcm_bytes());}
        }
        // A replaced cache key may still be retained by an exact historical
        // source. Count real shared allocations, never equal digests alone.
        for reader in state.one_shots.iter().map(|row|&row.reader).chain(state.music.iter().map(|row|&row.reader)) {
            sizes.insert(reader.clip().allocation_key(),reader.clip().pcm_bytes());
        }
        sizes.insert(clip.allocation_key(),clip.pcm_bytes());
        let bytes=sizes.values().try_fold(0u64,|sum,size|sum.checked_add(*size).ok_or("PCM cache budget overflow"))?;
        if bytes>limit {return Err("Decoded PCM cache exceeds its explicit memory budget".into());}
        state.clips.insert(key,clip);Ok(())
    }
    pub(crate) fn clip(&self,key:&CheckpointAssetKey)->Result<Option<Arc<PcmClip>>,String> {
        Ok(self.state.lock().map_err(|_|"Audio checkpoint broker is poisoned")?.clips.get(key).cloned())
    }

    /// Publication is the producer fence for a new interactive history entry.
    /// All actual Sources are already prepared but newly created ones have not
    /// been submitted to AudioOutput. Existing readers cannot advance while
    /// Core captures the same logical state and exact history entry.
    pub(crate) fn publish<T>(&self,instance_id:u64,epoch:u64,state:&rvn_core::GameState,
        one_shots:Vec<BoundOneShot>,music:Vec<BoundMusic>,commit:impl FnOnce(AudioCheckpointSnapshot)->Result<T,String>)->Result<T,String> {
        if one_shots.len()+music.len()>MAX_SOURCE_ROWS {return Err("Too many exact audio Sources".into());}
        let wanted=rvn_core::audio_playback::describe(state,epoch)?;let wanted_music=audio_checkpoint::music_views(state,epoch)?;
        if wanted.len()!=one_shots.len() || wanted_music.len()!=music.len() {return Err("Audio publication lacks a real prepared Source".into());}
        let mut identities=HashSet::new();
        for row in &one_shots {
            if !identities.insert(row.token.target.clone()) || !wanted.iter().any(|view|view.token==row.token && view.playback.file==row.file && view.playback.family==row.family && view.parked==row.parked) {return Err("Stale or duplicate one-shot publication".into());}
        }
        for row in &music {
            if !identities.insert(row.authority.target.clone()) || !wanted_music.iter().any(|view|view.authority==row.authority && view.file==row.file && view.owner==row.owner && view.parked==row.parked) {return Err("Stale or duplicate Music publication".into());}
        }
        // Consistent lock order with capture(): broker, then shared Source gate.
        let mut published=self.state.lock().map_err(|_|"Audio checkpoint broker is poisoned")?;
        let mut readers:Vec<_>=one_shots.iter().map(|row|(&row.reader,row.parked)).chain(music.iter().map(|row|(&row.reader,row.parked))).collect();
        // Readers removed by this publication are paused by this same gate;
        // they cannot advance between authoritative commit and ECS retirement.
        for old in published.one_shots.iter().map(|row|&row.reader).chain(published.music.iter().map(|row|&row.reader)) {
            if !readers.iter().any(|(current,_)|current.same_reader(old)){readers.push((old,true));}
        }
        let result=self.gate.publish(&readers,|rows| {
            let mut snapshot=AudioCheckpointSnapshot{instance_id,runtime_epoch:epoch,rows:Vec::new(),music:Vec::new()};
            for (index,checkpoint) in rows.into_iter().take(one_shots.len()+music.len()).enumerate() {
                if index<one_shots.len() {
                    let row=&one_shots[index];if !matches!(row.token.task.key.owner,TaskOwner::Host{..}) {snapshot.rows.push(AudioCheckpointRow{token:row.token.clone(),checkpoint});}
                } else {
                    let row=&music[index-one_shots.len()];if !matches!(row.owner,TaskOwner::Host{..}) {snapshot.music.push(MusicCheckpoint{target:row.authority.target.clone(),file:row.file.clone(),checkpoint});}
                }
            }
            audio_checkpoint::checkpointed_state(state,instance_id,epoch,&snapshot)?;
            commit(snapshot)
        })?;
        published.instance_id=Some(instance_id);published.epoch=Some(epoch);published.one_shots=one_shots;published.music=music;
        // The accepted PreparedSources plan owns every raw strong handle until
        // its exact SourceEntry cache is installed. Cold requests keep these
        // handles until this publication succeeds; no loading asset is orphaned.
        published.preparation_assets.clear();
        Ok(result)
    }

    /// Atomic authority publication after all renderer preflights. Working plans
    /// hold clones of real readers; this method never seeks, starts or retags one.
    pub(crate) fn install(&self,instance_id:u64,epoch:u64,state:&rvn_core::GameState,
        one_shots:Vec<BoundOneShot>,music:Vec<BoundMusic>)->Result<(),String> {
        if one_shots.len()+music.len()>MAX_SOURCE_ROWS {return Err("Too many exact audio sources".into());}
        let wanted=rvn_core::audio_playback::describe(state,epoch)?;
        let wanted_music=audio_checkpoint::music_views(state,epoch)?;
        if wanted.len()!=one_shots.len() || wanted_music.len()!=music.len() {return Err("Audio publication lacks an exact source binding".into());}
        let mut targets=HashSet::new();
        for row in &one_shots {
            if !targets.insert(row.token.target.clone()) || !wanted.iter().any(|view|view.token==row.token && view.playback.file==row.file && view.playback.family==row.family && view.parked==row.parked) {
                return Err("One-shot source binding is stale, duplicated or foreign".into());
            }
        }
        for row in &music {
            if !targets.insert(row.authority.target.clone()) || !wanted_music.iter().any(|view|view.authority==row.authority && view.file==row.file && view.owner==row.owner && view.parked==row.parked) {
                return Err("Music source binding is stale, duplicated or foreign".into());
            }
        }
        let readers:Vec<_>=one_shots.iter().map(|row|&row.reader).chain(music.iter().map(|row|&row.reader)).collect();
        self.gate.snapshot_with_status(&readers,|rows| {
            for (index,(_,paused)) in rows.iter().enumerate() {
                let expected=if index<one_shots.len(){one_shots[index].parked}else{music[index-one_shots.len()].parked};
                if *paused!=expected {return Err("Source pause has not reached its exact publication residence".into());}
            }
            Ok(())
        })?;
        let mut published=self.state.lock().map_err(|_|"Audio checkpoint broker is poisoned")?;
        published.instance_id=Some(instance_id);published.epoch=Some(epoch);
        published.one_shots=one_shots;published.music=music;Ok(())
    }

    pub(crate) fn capture(&self,state:&rvn_core::GameState,instance_id:u64,epoch:u64)->Result<AudioCheckpointSnapshot,String> {
        let published=self.state.lock().map_err(|_|"Audio checkpoint broker is poisoned")?;
        let wants_one_shots=rvn_core::audio_playback::describe(state,epoch)?;
        let wants_music=audio_checkpoint::music_views(state,epoch)?;
        let wanted_story:Vec<_>=wants_one_shots.iter().filter(|view|!matches!(view.token.task.key.owner,TaskOwner::Host{..})).collect();
        let wanted_music_story:Vec<_>=wants_music.iter().filter(|view|!matches!(view.owner,TaskOwner::Host{..})).collect();
        if wanted_story.is_empty() && wanted_music_story.is_empty() {
            return Ok(AudioCheckpointSnapshot{instance_id,runtime_epoch:epoch,rows:Vec::new(),music:Vec::new()});
        }
        if published.instance_id!=Some(instance_id) || published.epoch!=Some(epoch) {return Err("Exact Engine audio publication is not settled".into());}
        let mut bound=Vec::new();
        for view in wanted_story {
            let row=published.one_shots.iter().find(|row|row.token==view.token && row.file==view.playback.file && row.family==view.playback.family && row.parked==view.parked)
                .ok_or("A live or parked audio capability has not been published")?;
            bound.push(row);
        }
        let mut bound_music=Vec::new();
        for view in wanted_music_story {
            let row=published.music.iter().find(|row|row.authority==view.authority && row.file==view.file && row.owner==view.owner && row.parked==view.parked)
                .ok_or("A live or parked Music capability has not been published")?;
            bound_music.push(row);
        }
        let readers:Vec<_>=bound.iter().map(|row|&row.reader).chain(bound_music.iter().map(|row|&row.reader)).collect();
        self.gate.snapshot_with_status(&readers,|rows| {
            let mut snapshot=AudioCheckpointSnapshot{instance_id,runtime_epoch:epoch,rows:Vec::new(),music:Vec::new()};
            for (index,(checkpoint,paused)) in rows.into_iter().enumerate() {
                if index<bound.len() {
                    let row=bound[index];if paused!=row.parked {return Err("Unsettled one-shot pause during Save".into());}
                    snapshot.rows.push(AudioCheckpointRow{token:row.token.clone(),checkpoint});
                } else {
                    let row=bound_music[index-bound.len()];if paused!=row.parked {return Err("Unsettled Music pause during Save".into());}
                    snapshot.music.push(MusicCheckpoint{target:row.authority.target.clone(),file:row.file.clone(),checkpoint});
                }
            }
            // The immutable Engine state cannot change while this capture runs.
            // Clone/validate it under the same source fence, before any writer I/O.
            audio_checkpoint::checkpointed_state(state,instance_id,epoch,&snapshot)?;
            Ok(snapshot)
        })
    }

    /// Called before Core Load commits any PC/RNG/state/epoch or publication.
    /// Missing assets stay a preparation refusal; no decoder is reset to zero.
    pub(crate) fn capture_with<T>(&self,state:&rvn_core::GameState,instance_id:u64,epoch:u64,build:impl FnOnce(AudioCheckpointSnapshot)->Result<T,String>)->Result<T,String> {
        let published=self.state.lock().map_err(|_|"Audio checkpoint broker is poisoned")?;
        let wants_one_shots=rvn_core::audio_playback::describe(state,epoch)?;
        let wants_music=audio_checkpoint::music_views(state,epoch)?;
        let wanted_story:Vec<_>=wants_one_shots.iter().filter(|view|!matches!(view.token.task.key.owner,TaskOwner::Host{..})).collect();
        let wanted_music_story:Vec<_>=wants_music.iter().filter(|view|!matches!(view.owner,TaskOwner::Host{..})).collect();
        if wanted_story.is_empty() && wanted_music_story.is_empty() {
            return self.gate.snapshot(&[],|_|build(AudioCheckpointSnapshot{instance_id,runtime_epoch:epoch,rows:Vec::new(),music:Vec::new()}));
        }
        if published.instance_id!=Some(instance_id) || published.epoch!=Some(epoch) {return Err("Exact Engine audio publication is not settled".into());}
        let mut bound=Vec::new();
        for view in wanted_story {
            let row=published.one_shots.iter().find(|row|row.token==view.token && row.file==view.playback.file && row.family==view.playback.family && row.parked==view.parked)
                .ok_or("A live or parked audio capability has not been published")?;
            bound.push(row);
        }
        let mut bound_music=Vec::new();
        for view in wanted_music_story {
            let row=published.music.iter().find(|row|row.authority==view.authority && row.file==view.file && row.owner==view.owner && row.parked==view.parked)
                .ok_or("A live or parked Music capability has not been published")?;
            bound_music.push(row);
        }
        let readers:Vec<_>=bound.iter().map(|row|&row.reader).chain(bound_music.iter().map(|row|&row.reader)).collect();
        self.gate.snapshot_with_status(&readers,|rows| {
            let mut snapshot=AudioCheckpointSnapshot{instance_id,runtime_epoch:epoch,rows:Vec::new(),music:Vec::new()};
            for (index,(checkpoint,paused)) in rows.into_iter().enumerate() {
                if index<bound.len() {
                    let row=bound[index];if paused!=row.parked {return Err("Unsettled one-shot pause during Save".into());}
                    snapshot.rows.push(AudioCheckpointRow{token:row.token.clone(),checkpoint});
                } else {
                    let row=bound_music[index-bound.len()];if paused!=row.parked {return Err("Unsettled Music pause during Save".into());}
                    snapshot.music.push(MusicCheckpoint{target:row.authority.target.clone(),file:row.file.clone(),checkpoint});
                }
            }
            // The immutable Engine state cannot change while this capture runs.
            // Clone/validate it under the same source fence, before any writer I/O.
            audio_checkpoint::checkpointed_state(state,instance_id,epoch,&snapshot)?;
            build(snapshot)
        })
    }

    /// Called before Core Load commits any PC/RNG/state/epoch or publication.
    /// Missing assets stay a preparation refusal; no decoder is reset to zero.
    pub(crate) fn validate_restore(&self,state:&rvn_core::GameState)->Result<(),String> {
        audio_checkpoint::require_saved_checkpoints(state)?;
        let published=self.state.lock().map_err(|_|"Audio checkpoint broker is poisoned")?;
        for view in rvn_core::audio_playback::describe(state,0)? {
            if matches!(view.token.task.key.owner,TaskOwner::Host{..}) {continue;}
            let saved=view.playback.checkpoint.as_ref().ok_or("Missing saved one-shot cursor")?;
            let clip=published.clips.get(&CheckpointAssetKey::one_shot(view.playback.family,&view.playback.file)).ok_or("Saved one-shot asset is not decoded and verified yet")?;
            clip.validate_checkpoint(saved)?;
        }
        for view in audio_checkpoint::music_views(state,0)? {
            if matches!(view.owner,TaskOwner::Host{..}) {continue;}
            let saved=view.checkpoint.as_ref().ok_or("Missing saved Music cursor")?;
            let clip=published.clips.get(&CheckpointAssetKey::Music(view.file)).ok_or("Saved Music asset is not decoded and verified yet")?;
            clip.validate_checkpoint(saved)?;
        }
        Ok(())
    }

    // Additional authenticated Frame contract; historical Story methods above remain exact.
    pub(crate) fn capture_frame(&self,state:&rvn_core::GameState,instance_id:u64,epoch:u64)->Result<AudioCheckpointSnapshot,String> {
        let published=self.state.lock().map_err(|_|"Audio checkpoint broker is poisoned")?;
        let wants_one_shots=rvn_core::audio_playback::describe(state,epoch)?;
        let wants_music=audio_checkpoint::music_views(state,epoch)?;
        let wanted_story:Vec<_>=wants_one_shots.iter().collect();
        let wanted_music_story:Vec<_>=wants_music.iter().collect();
        if wanted_story.is_empty() && wanted_music_story.is_empty() {
            return Ok(AudioCheckpointSnapshot{instance_id,runtime_epoch:epoch,rows:Vec::new(),music:Vec::new()});
        }
        if published.instance_id!=Some(instance_id) || published.epoch!=Some(epoch) {return Err("Exact Engine audio publication is not settled".into());}
        let mut bound=Vec::new();
        for view in wanted_story {
            let row=published.one_shots.iter().find(|row|row.token==view.token && row.file==view.playback.file && row.family==view.playback.family && row.parked==view.parked)
                .ok_or("A live or parked audio capability has not been published")?;
            bound.push(row);
        }
        let mut bound_music=Vec::new();
        for view in wanted_music_story {
            let row=published.music.iter().find(|row|row.authority==view.authority && row.file==view.file && row.owner==view.owner && row.parked==view.parked)
                .ok_or("A live or parked Music capability has not been published")?;
            bound_music.push(row);
        }
        let readers:Vec<_>=bound.iter().map(|row|&row.reader).chain(bound_music.iter().map(|row|&row.reader)).collect();
        self.gate.snapshot_with_status(&readers,|rows| {
            let mut snapshot=AudioCheckpointSnapshot{instance_id,runtime_epoch:epoch,rows:Vec::new(),music:Vec::new()};
            for (index,(checkpoint,paused)) in rows.into_iter().enumerate() {
                if index<bound.len() {
                    let row=bound[index];if paused!=row.parked {return Err("Unsettled one-shot pause during Save".into());}
                    snapshot.rows.push(AudioCheckpointRow{token:row.token.clone(),checkpoint});
                } else {
                    let row=bound_music[index-bound.len()];if paused!=row.parked {return Err("Unsettled Music pause during Save".into());}
                    snapshot.music.push(MusicCheckpoint{target:row.authority.target.clone(),file:row.file.clone(),checkpoint});
                }
            }
            // The immutable Engine state cannot change while this capture runs.
            // Clone/validate it under the same source fence, before any writer I/O.
            rvn_core::frame_audio_checkpoint::checkpointed_frame_state(state,instance_id,epoch,&snapshot)?;
            Ok(snapshot)
        })
    }

    pub(crate) fn capture_frame_with<T>(&self,state:&rvn_core::GameState,instance_id:u64,epoch:u64,build:impl FnOnce(AudioCheckpointSnapshot)->Result<T,String>)->Result<T,String> {
        let published=self.state.lock().map_err(|_|"Audio checkpoint broker is poisoned")?;
        let wants_one_shots=rvn_core::audio_playback::describe(state,epoch)?;
        let wants_music=audio_checkpoint::music_views(state,epoch)?;
        let wanted_story:Vec<_>=wants_one_shots.iter().collect();
        let wanted_music_story:Vec<_>=wants_music.iter().collect();
        if wanted_story.is_empty() && wanted_music_story.is_empty() {
            return self.gate.snapshot(&[],|_|build(AudioCheckpointSnapshot{instance_id,runtime_epoch:epoch,rows:Vec::new(),music:Vec::new()}));
        }
        if published.instance_id!=Some(instance_id) || published.epoch!=Some(epoch) {return Err("Exact Engine audio publication is not settled".into());}
        let mut bound=Vec::new();
        for view in wanted_story {
            let row=published.one_shots.iter().find(|row|row.token==view.token && row.file==view.playback.file && row.family==view.playback.family && row.parked==view.parked)
                .ok_or("A live or parked audio capability has not been published")?;
            bound.push(row);
        }
        let mut bound_music=Vec::new();
        for view in wanted_music_story {
            let row=published.music.iter().find(|row|row.authority==view.authority && row.file==view.file && row.owner==view.owner && row.parked==view.parked)
                .ok_or("A live or parked Music capability has not been published")?;
            bound_music.push(row);
        }
        let readers:Vec<_>=bound.iter().map(|row|&row.reader).chain(bound_music.iter().map(|row|&row.reader)).collect();
        self.gate.snapshot_with_status(&readers,|rows| {
            let mut snapshot=AudioCheckpointSnapshot{instance_id,runtime_epoch:epoch,rows:Vec::new(),music:Vec::new()};
            for (index,(checkpoint,paused)) in rows.into_iter().enumerate() {
                if index<bound.len() {
                    let row=bound[index];if paused!=row.parked {return Err("Unsettled one-shot pause during Save".into());}
                    snapshot.rows.push(AudioCheckpointRow{token:row.token.clone(),checkpoint});
                } else {
                    let row=bound_music[index-bound.len()];if paused!=row.parked {return Err("Unsettled Music pause during Save".into());}
                    snapshot.music.push(MusicCheckpoint{target:row.authority.target.clone(),file:row.file.clone(),checkpoint});
                }
            }
            // The immutable Engine state cannot change while this capture runs.
            // Clone/validate it under the same source fence, before any writer I/O.
            rvn_core::frame_audio_checkpoint::checkpointed_frame_state(state,instance_id,epoch,&snapshot)?;
            build(snapshot)
        })
    }

    pub(crate) fn validate_frame_restore(&self,state:&rvn_core::GameState)->Result<(),String> {
        rvn_core::frame_audio_checkpoint::require_frame_checkpoints(state)?;
        let published=self.state.lock().map_err(|_|"Audio checkpoint broker is poisoned")?;
        for view in rvn_core::audio_playback::describe(state,0)? {
            let saved=view.playback.checkpoint.as_ref().ok_or("Missing saved one-shot cursor")?;
            let clip=published.clips.get(&CheckpointAssetKey::one_shot(view.playback.family,&view.playback.file)).ok_or("Saved one-shot asset is not decoded and verified yet")?;
            clip.validate_checkpoint(saved)?;
        }
        for view in audio_checkpoint::music_views(state,0)? {
            let saved=view.checkpoint.as_ref().ok_or("Missing saved Music cursor")?;
            let clip=published.clips.get(&CheckpointAssetKey::Music(view.file)).ok_or("Saved Music asset is not decoded and verified yet")?;
            clip.validate_checkpoint(saved)?;
        }
        Ok(())
    }

    pub(crate) fn publish_frame<T>(&self,instance_id:u64,epoch:u64,state:&rvn_core::GameState,
        one_shots:Vec<BoundOneShot>,music:Vec<BoundMusic>,commit:impl FnOnce(AudioCheckpointSnapshot)->Result<T,String>)->Result<T,String> {
        if one_shots.len()+music.len()>MAX_SOURCE_ROWS {return Err("Too many exact audio Sources".into());}
        let wanted=rvn_core::audio_playback::describe(state,epoch)?;let wanted_music=audio_checkpoint::music_views(state,epoch)?;
        if wanted.len()!=one_shots.len() || wanted_music.len()!=music.len() {return Err("Audio publication lacks a real prepared Source".into());}
        let mut identities=HashSet::new();
        for row in &one_shots {
            if !identities.insert(row.token.target.clone()) || !wanted.iter().any(|view|view.token==row.token && view.playback.file==row.file && view.playback.family==row.family && view.parked==row.parked) {return Err("Stale or duplicate one-shot publication".into());}
        }
        for row in &music {
            if !identities.insert(row.authority.target.clone()) || !wanted_music.iter().any(|view|view.authority==row.authority && view.file==row.file && view.owner==row.owner && view.parked==row.parked) {return Err("Stale or duplicate Music publication".into());}
        }
        // Consistent lock order with capture(): broker, then shared Source gate.
        let mut published=self.state.lock().map_err(|_|"Audio checkpoint broker is poisoned")?;
        let mut readers:Vec<_>=one_shots.iter().map(|row|(&row.reader,row.parked)).chain(music.iter().map(|row|(&row.reader,row.parked))).collect();
        // Readers removed by this publication are paused by this same gate;
        // they cannot advance between authoritative commit and ECS retirement.
        for old in published.one_shots.iter().map(|row|&row.reader).chain(published.music.iter().map(|row|&row.reader)) {
            if !readers.iter().any(|(current,_)|current.same_reader(old)){readers.push((old,true));}
        }
        let result=self.gate.publish(&readers,|rows| {
            let mut snapshot=AudioCheckpointSnapshot{instance_id,runtime_epoch:epoch,rows:Vec::new(),music:Vec::new()};
            for (index,checkpoint) in rows.into_iter().take(one_shots.len()+music.len()).enumerate() {
                if index<one_shots.len() {
                    let row=&one_shots[index];snapshot.rows.push(AudioCheckpointRow{token:row.token.clone(),checkpoint});
                } else {
                    let row=&music[index-one_shots.len()];snapshot.music.push(MusicCheckpoint{target:row.authority.target.clone(),file:row.file.clone(),checkpoint});
                }
            }
            rvn_core::frame_audio_checkpoint::checkpointed_frame_state(state,instance_id,epoch,&snapshot)?;
            commit(snapshot)
        })?;
        published.instance_id=Some(instance_id);published.epoch=Some(epoch);published.one_shots=one_shots;published.music=music;
        // The accepted PreparedSources plan owns every raw strong handle until
        // its exact SourceEntry cache is installed. Cold requests keep these
        // handles until this publication succeeds; no loading asset is orphaned.
        published.preparation_assets.clear();
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::audio::Decodable;
    fn clip()->Arc<PcmClip> {
        let samples=[11i16,12,21,22,31,32];let mut wav=Vec::new();
        wav.extend_from_slice(b"RIFF");wav.extend_from_slice(&(36u32+samples.len() as u32*2).to_le_bytes());wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());wav.extend_from_slice(&1u16.to_le_bytes());wav.extend_from_slice(&2u16.to_le_bytes());
        wav.extend_from_slice(&48_000u32.to_le_bytes());wav.extend_from_slice(&192_000u32.to_le_bytes());wav.extend_from_slice(&4u16.to_le_bytes());wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");wav.extend_from_slice(&(samples.len() as u32*2).to_le_bytes());for sample in samples {wav.extend_from_slice(&sample.to_le_bytes());}
        PcmClip::decode("sfx/same.wav",&wav).unwrap()
    }
    fn game()->rvn_core::Engine<crate::bevy_renderer::BevyRenderer> {
        let mut game=rvn_core::Engine::from_program(rvn_parser::parse_program("scene label P\nsfx.play(\"same.wav\")\n\"parent\"\ncall C\n\"returned\"\nscene label C\n\"child\"\nreturn").unwrap(),crate::bevy_renderer::BevyRenderer::new(),32).unwrap();
        game.step_until_interaction().unwrap();game
    }
    fn binding(game:&rvn_core::Engine<crate::bevy_renderer::BevyRenderer>,reader:CheckpointPcm)->BoundOneShot {
        let view=game.audio_playback_views().unwrap().remove(0);
        BoundOneShot{token:view.token,file:view.playback.file,family:view.playback.family,parked:view.parked,reader}
    }
    #[test]
    fn cache_metadata_capacity_refuses_new_suffix_but_allows_exact_key_replacement_without_double_counting_arc() {
        let broker=AudioCheckpointBroker::default();let clip=clip();let bytes=clip.pcm_bytes();
        let first=CheckpointAssetKey::Sfx("first.wav".into());let second=CheckpointAssetKey::Voice("second.wav".into());
        broker.cache_clip_with_limits(first.clone(),clip.clone(),bytes,2).unwrap();
        broker.cache_clip_with_limits(second.clone(),clip.clone(),bytes,2).unwrap();
        let before=broker.available_decode_bytes().unwrap();
        assert!(broker.cache_clip_with_limits(CheckpointAssetKey::Music("third.wav".into()),clip.clone(),bytes,2).is_err());
        assert_eq!(broker.available_decode_bytes().unwrap(),before);
        assert!(broker.clip(&CheckpointAssetKey::Music("third.wav".into())).unwrap().is_none());
        assert_eq!(broker.clip(&first).unwrap().unwrap().allocation_key(),clip.allocation_key());
        broker.cache_clip_with_limits(second,clip,bytes,2).unwrap();assert_eq!(broker.available_decode_bytes().unwrap(),before);
    }
    #[test]
    fn real_pcm_cursor_and_core_clone_are_captured_at_one_fence_without_mutating_state() {
        let game=game();let broker=AudioCheckpointBroker::default();let reader=clip().start(broker.gate(),false);let mut source=reader.decoder();
        assert_eq!(source.by_ref().take(3).collect::<Vec<_>>(),vec![11,12,21]);
        broker.install(game.instance_id(),game.runtime_epoch(),&game.state,vec![binding(&game,reader)],Vec::new()).unwrap();
        let before=serde_json::to_value(&game.state).unwrap();let snap=broker.capture(&game.state,game.instance_id(),game.runtime_epoch()).unwrap();
        assert_eq!((snap.rows[0].checkpoint.next_sample,snap.rows[0].checkpoint.next_channel),(3,1));
        let frozen=audio_checkpoint::checkpointed_state(&game.state,game.instance_id(),game.runtime_epoch(),&snap).unwrap();
        assert_eq!(frozen.pc,game.state.pc);assert_eq!(frozen.random,game.state.random);assert_eq!(serde_json::to_value(&game.state).unwrap(),before);
        assert_eq!(source.next(),Some(22));assert_eq!(broker.capture(&game.state,game.instance_id(),game.runtime_epoch()).unwrap().rows[0].checkpoint.next_sample,4);
    }
    #[test]
    fn save_between_call_and_publication_refuses_instead_of_inventing_a_parked_position() {
        let mut game=game();let broker=AudioCheckpointBroker::default();let reader=clip().start(broker.gate(),false);let mut source=reader.decoder();assert_eq!(source.next(),Some(11));
        broker.install(game.instance_id(),game.runtime_epoch(),&game.state,vec![binding(&game,reader.clone())],Vec::new()).unwrap();
        game.advance_dialogue().unwrap();game.step_until_interaction().unwrap();let before=serde_json::to_value(&game.state).unwrap();
        assert!(broker.capture(&game.state,game.instance_id(),game.runtime_epoch()).is_err());assert_eq!(serde_json::to_value(&game.state).unwrap(),before);
        reader.set_paused(true).unwrap();broker.install(game.instance_id(),game.runtime_epoch(),&game.state,vec![binding(&game,reader.clone())],Vec::new()).unwrap();
        let snap=broker.capture(&game.state,game.instance_id(),game.runtime_epoch()).unwrap();assert_eq!(snap.rows[0].checkpoint.next_sample,1);
        assert_eq!(source.by_ref().take(3).collect::<Vec<_>>(),vec![0,0,0]);assert_eq!(broker.capture(&game.state,game.instance_id(),game.runtime_epoch()).unwrap(),snap);
        game.advance_dialogue().unwrap();game.step_until_interaction().unwrap();assert!(broker.capture(&game.state,game.instance_id(),game.runtime_epoch()).is_err());
        reader.set_paused(false).unwrap();broker.install(game.instance_id(),game.runtime_epoch(),&game.state,vec![binding(&game,reader)],Vec::new()).unwrap();
        assert_eq!(broker.capture(&game.state,game.instance_id(),game.runtime_epoch()).unwrap().rows[0].checkpoint.next_sample,1);
        assert_eq!(source.next(),Some(0));assert_eq!(source.next(),Some(12));
    }
    #[test]
    fn fresh_engine_with_identical_epoch_and_targets_cannot_reuse_a_reader_capability() {
        let first=game();let second=game();let broker=AudioCheckpointBroker::default();let row=binding(&first,clip().start(broker.gate(),false));
        assert_eq!(binding(&second,row.reader.clone()).token,row.token);assert_ne!(first.instance_id(),second.instance_id());
        broker.install(first.instance_id(),first.runtime_epoch(),&first.state,vec![row],Vec::new()).unwrap();
        assert!(broker.capture(&second.state,second.instance_id(),second.runtime_epoch()).is_err());
    }
    #[test]
    fn load_verifies_saved_pcm_metadata_and_real_cached_asset_before_state_publication() {
        let game=game();let broker=AudioCheckpointBroker::default();let clip=clip();let reader=clip.start(broker.gate(),false);let mut source=reader.decoder();assert_eq!(source.next(),Some(11));
        let row=binding(&game,reader);let key=CheckpointAssetKey::one_shot(row.family,&row.file);
        broker.install(game.instance_id(),game.runtime_epoch(),&game.state,vec![row],Vec::new()).unwrap();
        let snap=broker.capture(&game.state,game.instance_id(),game.runtime_epoch()).unwrap();let frozen=audio_checkpoint::checkpointed_state(&game.state,game.instance_id(),game.runtime_epoch(),&snap).unwrap();
        let before=serde_json::to_value(&game.state).unwrap();assert!(broker.validate_restore(&frozen).is_err());broker.cache_clip(key,clip).unwrap();broker.validate_restore(&frozen).unwrap();
        let mut bad=frozen.clone();bad.scene_execution.as_mut().unwrap().resources.tasks.iter_mut().find(|task|task.audio.is_some()).unwrap().audio.as_mut().unwrap().checkpoint.as_mut().unwrap().pcm_sha256[0]^=1;
        assert!(broker.validate_restore(&bad).is_err());assert_eq!(serde_json::to_value(&game.state).unwrap(),before);
    }
    #[test]
    fn cache_budget_counts_real_allocations_and_preserves_originals_on_refusal() {
        let broker=AudioCheckpointBroker::default();let first=clip();let duplicate_bytes=clip();
        assert_eq!(first.pcm_digest(),duplicate_bytes.pcm_digest());assert_ne!(first.allocation_key(),duplicate_bytes.allocation_key());
        let left=CheckpointAssetKey::Music("left.wav".into());let right=CheckpointAssetKey::Music("right.wav".into());
        broker.cache_clip_with_budget(left.clone(),first.clone(),12).unwrap();
        assert!(broker.cache_clip_with_budget(right.clone(),duplicate_bytes,12).is_err());
        assert!(broker.clip(&right).unwrap().is_none());assert!(Arc::ptr_eq(&broker.clip(&left).unwrap().unwrap(),&first));
        broker.cache_clip_with_budget(right,first.clone(),12).unwrap();
        assert_eq!(broker.state.lock().unwrap().clips.len(),2);
    }
    #[test]
    fn replacement_pauses_actual_outgoing_reader_under_same_publication_gate(){
        let game=game();let broker=AudioCheckpointBroker::default();let clip=clip();
        let old=clip.start(broker.gate(),false);let mut decoder=old.decoder();assert_eq!(decoder.next(),Some(11));
        broker.install(game.instance_id(),game.runtime_epoch(),&game.state,vec![binding(&game,old.clone())],vec![]).unwrap();
        let incoming=clip.start(broker.gate(),false);
        let observed=broker.publish(game.instance_id(),game.runtime_epoch(),&game.state,vec![binding(&game,incoming.clone())],vec![],|snapshot|Ok(snapshot)).unwrap();
        assert_eq!(observed.rows.len(),1);assert_eq!(observed.rows[0].checkpoint.next_sample,0);
        let statuses=broker.gate().snapshot_with_status(&[&old,&incoming],Ok).unwrap();assert!(statuses[0].1);assert!(!statuses[1].1);
        assert_eq!(decoder.next(),Some(0));assert_eq!(statuses[0].0.next_sample,1);
    }
    #[test]
    fn refused_fenced_commit_keeps_outgoing_pause_and_broker_authority_unchanged(){
        let game=game();let broker=AudioCheckpointBroker::default();let clip=clip();
        let old=clip.start(broker.gate(),false);let mut decoder=old.decoder();assert_eq!(decoder.next(),Some(11));
        broker.install(game.instance_id(),game.runtime_epoch(),&game.state,vec![binding(&game,old.clone())],vec![]).unwrap();
        let before=broker.capture(&game.state,game.instance_id(),game.runtime_epoch()).unwrap();
        let incoming=clip.start(broker.gate(),false);
        let result:Result<(),String>=broker.publish(game.instance_id(),game.runtime_epoch(),&game.state,vec![binding(&game,incoming.clone())],vec![],|_|Err("actual precommit refusal".into()));
        assert!(result.is_err());assert_eq!(broker.capture(&game.state,game.instance_id(),game.runtime_epoch()).unwrap(),before);
        let statuses=broker.gate().snapshot_with_status(&[&old,&incoming],Ok).unwrap();assert!(!statuses[0].1);assert!(!statuses[1].1);
        assert_eq!(decoder.next(),Some(12));
    }

}
