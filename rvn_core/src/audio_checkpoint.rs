//! PRIVATE: portable source cursor. No Engine lineage, Entity or hardware clock is saved.
use serde::{Deserialize, Serialize};

pub const CHECKPOINT_VERSION: u32 = 1;
pub const MAX_ENCODED_BYTES: usize = 64 * 1024 * 1024;
/// Explicit preparation memory limit, not a legacy authored duration limit.
/// The renderer also counts all retained shared allocations against 256 MiB.
pub const MAX_PCM_SAMPLES: u64 = 128 * 1024 * 1024;
pub const MAX_CHANNELS: u16 = 8;
pub const MAX_SAMPLE_RATE: u32 = 384_000;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioCheckpoint {
    pub version: u32,
    pub asset_sha256: [u8; 32],
    /// SHA256 of little-endian rate/channels followed by the exact i16 PCM samples.
    pub pcm_sha256: [u8; 32],
    pub channels: u16,
    pub sample_rate: u32,
    pub total_samples: u64,
    /// Next non-consumed interleaved sample, not elapsed time or an audible position.
    pub next_sample: u64,
    /// Explicit channel of next_sample. A restored Source starts at channel zero.
    pub next_channel: u16,
    pub looping: bool,
    pub loops_completed: u64,
}

impl AudioCheckpoint {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != CHECKPOINT_VERSION
            || self.channels == 0 || self.channels > MAX_CHANNELS
            || self.sample_rate == 0 || self.sample_rate > MAX_SAMPLE_RATE
            || self.total_samples == 0 || self.total_samples > MAX_PCM_SAMPLES
            || self.total_samples % u64::from(self.channels) != 0
            || self.next_sample > self.total_samples
            || u64::from(self.next_channel) != self.next_sample % u64::from(self.channels)
            || (!self.looping && self.loops_completed != 0)
        {
            return Err("Invalid exact audio decoder checkpoint".into());
        }
        Ok(())
    }

    pub fn same_source(&self, other: &Self) -> bool {
        self.version == other.version && self.asset_sha256 == other.asset_sha256
            && self.pcm_sha256 == other.pcm_sha256 && self.channels == other.channels
            && self.sample_rate == other.sample_rate && self.total_samples == other.total_samples
    }
}


/// A small transient witness from the producer's real decoder fence. It owns
/// metadata only; raw previous remains the sole cancellation/history state.
/// Neither a saved cursor nor a caller-supplied state can mint this authority.
#[derive(Clone,Debug)]
pub struct HistoricalGameSnapshot {
    snapshot: AudioCheckpointSnapshot,
    latest_entry_id: Option<u64>,
    previous_sha256: [u8;32],
    accepted_sha256: Option<[u8;32]>,
}
impl HistoricalGameSnapshot {
    /// Streaming bound for receipt/backlog/request accounting, without a second
    /// GameState, allocated formatting buffer, or a saved transient identity.
    pub fn payload_bytes(&self)->Result<usize,String> {
        use std::fmt::Write;
        struct Count(usize);
        impl std::fmt::Write for Count {
            fn write_str(&mut self,text:&str)->std::fmt::Result {
                self.0=self.0.checked_add(text.len()).filter(|n|*n<=16*1024*1024).ok_or(std::fmt::Error)?;Ok(())
            }
        }
        let mut count=Count(0);write!(&mut count,"{self:?}").map_err(|_|"Historical PCM metadata exceeds the receipt budget".to_string())?;Ok(count.0)
    }
    pub(crate) fn captured(previous:&crate::GameState,snapshot:AudioCheckpointSnapshot,entry:Option<u64>)->Result<Self,String> {
        // None is valid before the first interaction and after Load/rollback.
        // Instance/epoch/raw/candidate digests still bind the real fence;
        // receipt/token admission supplies one-shot consumption independently
        // of whether bounded history retains a newly allocated visit ID.
        // Verify the complete fence now, without retaining the temporary clone.
        let checked=checkpointed_state(previous,snapshot.instance_id,snapshot.runtime_epoch,&snapshot)?;
        require_saved_checkpoints(&checked)?;
        let proof=Self{snapshot,latest_entry_id:entry,previous_sha256:raw_digest(previous)?,accepted_sha256:None};
        proof.payload_bytes()?;Ok(proof)
    }
    pub(crate) fn previous(&self,raw:&crate::GameState,instance:u64,epoch:u64,entry:Option<u64>)->Result<crate::GameState,String> {
        if self.snapshot.instance_id!=instance || self.snapshot.runtime_epoch!=epoch || self.latest_entry_id!=entry || self.previous_sha256!=raw_digest(raw)? {
            return Err("Historical PCM witness belongs to a different Engine, visit or raw previous".into());
        }
        checkpointed_state(raw,instance,epoch,&self.snapshot)
    }
    pub(crate) fn seal(mut self,accepted:&crate::GameState)->Result<Self,String> {
        self.accepted_sha256=Some(accepted_digest(accepted)?);Ok(self)
    }
    pub(crate) fn accepts(&self,raw:&crate::GameState,current:&crate::GameState,instance:u64,epoch:u64,entry:Option<u64>)->Result<bool,String> {
        if self.snapshot.instance_id!=instance || self.snapshot.runtime_epoch!=epoch || self.latest_entry_id!=entry {return Ok(false);}
        if self.previous_sha256!=raw_digest(raw)? || self.accepted_sha256!=Some(accepted_digest(current)?) {return Ok(false);}
        self.previous(raw,instance,epoch,entry)?;Ok(true)
    }
}
fn raw_digest(state:&crate::GameState)->Result<[u8;32],String> {
    use sha2::{Digest,Sha256};
    use std::fmt::Write;
    struct Hash {value:Sha256,bytes:usize}
    impl std::fmt::Write for Hash {
        fn write_str(&mut self,text:&str)->std::fmt::Result {
            self.bytes=self.bytes.checked_add(text.len()).filter(|n|*n<=16*1024*1024).ok_or(std::fmt::Error)?;
            self.value.update(text.as_bytes());Ok(())
        }
    }
    // Debug is derived on GameState/UiState and includes runtime-only fields
    // skipped by Save serde, notably host_next_order. Hash directly, without a
    // formatting buffer or second retained GameState; this is not a save wire.
    let mut hash=Hash{value:Sha256::new(),bytes:0};
    write!(&mut hash,"{state:?}").map_err(|_|"Raw historical state exceeds its bounded streaming digest".to_string())?;
    Ok(hash.value.finalize().into())
}
fn accepted_digest(state:&crate::GameState)->Result<[u8;32],String> {
    let mut installed=state.clone();installed.menu_requests.clear();installed.speech_requests.clear();request_digest(&installed)
}
pub(crate) fn unmeasured_historical_state(mut raw:crate::GameState)->crate::GameState {
    use crate::scene_tasks::TaskOwner;
    if let Some(registry)=raw.audio_resource_registry_mut() {
        for task in &mut registry.tasks {
            if !matches!(task.key.owner,TaskOwner::Host{..}) {
                if let Some(audio)=task.audio.as_mut(){audio.checkpoint=None;}
            }
        }
        let targets=&registry.targets;
        registry.music_checkpoints.retain(|row|targets.iter().any(|record|record.identity==row.target && matches!(record.owner,TaskOwner::Host{..})));
    }
    raw
}

/// Transient exact authority. This envelope is never serialized into GameSave.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioCheckpointRow {
    pub token: crate::scene_resources::ResourceToken,
    pub checkpoint: AudioCheckpoint,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioCheckpointSnapshot {
    pub instance_id: u64,
    pub runtime_epoch: u64,
    pub rows: Vec<AudioCheckpointRow>,
    pub music: Vec<MusicCheckpoint>,
}

/// Transient first-publication capability. Private fields prevent a consumer
/// from manufacturing an entry ID or selecting a later state by alias/index.
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct AudioPublicationSource {
    pub(crate) engine_instance:u64,pub(crate) runtime_epoch:u64,
    pub(crate) history_entry:Option<u64>,pub(crate) logical_sha256:[u8;32],
}
/// Producer-captured rollback capability. An index, PC or alias is insufficient
/// after bounded-history trim/pop. The only visit ID is RollbackHistory.entry_id.
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct AudioRollbackSource {
    pub(crate) engine_instance:u64,pub(crate) runtime_epoch:u64,
    pub(crate) entry_id:u64,pub(crate) current_sha256:[u8;32],pub(crate) historical_sha256:[u8;32],
    pub(crate) skip_current_display:bool,pub(crate) latest_entry_id:Option<u64>,
}
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct AudioRequestSource {
    pub(crate) engine_instance:u64,pub(crate) runtime_epoch:u64,pub(crate) display_generation:u64,
    pub(crate) latest_entry_id:Option<u64>,pub(crate) logical_sha256:[u8;32],
}
pub(crate) fn request_digest(state:&crate::GameState)->Result<[u8;32],String> {
    use sha2::{Digest,Sha256};
    let mut comparable=state.clone();
    if let Some(display)=comparable.scene_execution.as_mut().and_then(|execution|execution.display.as_mut()){display.reveal=None;}
    Ok(Sha256::digest(logical_bytes(&comparable)?).into())
}
pub(crate) fn logical_digest(state:&crate::GameState)->Result<[u8;32],String> {
    use sha2::{Digest,Sha256};
    let bytes=logical_bytes(state)?;Ok(Sha256::digest(bytes).into())
}

/// Saved cursor of one physical Current/Retired Music source. The registry's
/// existing target owns its lifetime; this record adds no clock or owner.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MusicCheckpoint {
    pub target: crate::scene_resources::TargetIdentity,
    pub file: String,
    pub checkpoint: AudioCheckpoint,
}

/// Transient capability: Current Music is Game-owned independently of the fade
/// owner. A historical source additionally requires its exact outgoing task.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MusicSourceAuthority {
    pub target: crate::scene_resources::TargetIdentity,
    pub outgoing: Option<crate::scene_tasks::TaskToken>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MusicCheckpointView {
    pub authority: MusicSourceAuthority,
    pub file: String,
    pub parked: bool,
    pub owner: crate::scene_tasks::TaskOwner,
    pub checkpoint: Option<AudioCheckpoint>,
}

pub fn music_views(state: &crate::GameState, epoch: u64) -> Result<Vec<MusicCheckpointView>, String> {
    use crate::{retired_presentations::RetiredModel, scene_resources::TargetKind, scene_tasks::{TaskOwner,TaskToken}};
    let Some(registry)=state.audio_resource_registry() else {return Ok(Vec::new());};
    validate_music_registry(registry)?;
    let mut result=Vec::new();
    let saved=|target:&crate::scene_resources::TargetIdentity|registry.music_checkpoints.iter().find(|row|row.target==*target).map(|row|row.checkpoint.clone());
    if let Some(file)=&state.music.current_file {
        let target=registry.target(&TaskOwner::Game,TargetKind::Music,"music").ok_or("Current Music has no exact Game target")?.clone();
        if registry.music_checkpoints.iter().any(|row|row.target==target && row.file!=*file) {return Err("Current Music checkpoint refers to a replaced file".into());}
        result.push(MusicCheckpointView{authority:MusicSourceAuthority{target:target.clone(),outgoing:None},file:file.clone(),parked:false,owner:TaskOwner::Game,checkpoint:saved(&target)});
    }
    for task in &registry.tasks {
        let Some(presentation)=task.effect.as_ref().and_then(|effect|effect.presentation.as_ref()) else {continue;};
        let RetiredModel::Music{file,..}=&presentation.model else {continue;};
        result.push(MusicCheckpointView{authority:MusicSourceAuthority{target:task.target.clone(),outgoing:Some(TaskToken{runtime_epoch:epoch,key:task.key.clone(),revision:task.revision})},file:file.clone(),parked:task.parked,owner:task.key.owner.clone(),checkpoint:saved(&task.target)});
    }
    if result.len()>crate::scene_resources::MAX_MOTIONS+1 {return Err("Music sources exceed the Current plus owned-effect budget".into());}
    Ok(result)
}

pub fn validate_music_registry(registry:&crate::scene_resources::ResourceRegistry)->Result<(),String> {
    use crate::{retired_presentations::RetiredModel,scene_resources::TargetKind,scene_tasks::TaskOwner};
    if registry.music_checkpoints.len()>crate::scene_resources::MAX_MOTIONS+1 {return Err("Too many music decoder checkpoints".into());}
    let mut seen=std::collections::HashSet::new();
    for row in &registry.music_checkpoints {
        row.checkpoint.validate()?;
        if !row.checkpoint.looping || !rvn_ui::programmable::safe_asset_path(&row.file)
            || !seen.insert(row.target.clone()) {return Err("Invalid, duplicate or non-looping Music checkpoint".into());}
        let record=registry.targets.iter().find(|record|record.identity==row.target).ok_or("Orphan music checkpoint")?;
        match row.target.kind {
            TargetKind::Music if record.owner==TaskOwner::Game && row.target.alias=="music"=>{},
            TargetKind::PresentationRetired=>{
                let matching:Vec<_>=registry.tasks.iter().filter(|task|task.target==row.target).collect();
                if matching.len()!=1 || !matching[0].effect.as_ref().and_then(|effect|effect.presentation.as_ref())
                    .is_some_and(|presentation|matches!(&presentation.model,RetiredModel::Music{file,..} if file==&row.file)) {
                    return Err("A historical music checkpoint has no exact typed Music payload".into());
                }
            },
            _=>return Err("Music checkpoint target is neither Game Music nor typed Retired Music".into()),
        }
    }
    Ok(())
}

/// Saved-source metadata can be validated before Load. Only a fresh captured
/// snapshot may bypass the raw writer's guard; an old Some is never freshness.
pub fn require_saved_checkpoints(state:&crate::GameState)->Result<(),String> {
    let Some(registry)=state.audio_resource_registry() else {return Ok(());};
    crate::scene_resources::validate_current_resource_state(state)?;
    for task in &registry.tasks {
        if matches!(task.key.owner,crate::scene_tasks::TaskOwner::Host{..}) {continue;}
        if let Some(audio)=&task.audio {audio.checkpoint.as_ref().ok_or("A Story audio reader has no saved decoder checkpoint")?.validate()?;}
    }
    for view in music_views(state,0)? {
        if !matches!(view.owner,crate::scene_tasks::TaskOwner::Host{..}) && view.checkpoint.is_none() {
            return Err("Current and parked Story Music require their real source checkpoint".into());
        }
    }
    Ok(())
}

pub fn extended_schema(state:&crate::GameState)->bool {
    state.audio_resource_registry().is_some_and(|registry|registry.music_checkpoints.iter().any(|row|
        registry.targets.iter().any(|record|record.identity==row.target && !matches!(record.owner,crate::scene_tasks::TaskOwner::Host{..})))
        || registry.tasks.iter().any(|task|!matches!(task.key.owner,crate::scene_tasks::TaskOwner::Host{..}) && task.audio.as_ref().is_some_and(|audio|audio.checkpoint.is_some())))
}

/// An immutable state sampled while all participating decoders were fenced.
/// Fields are private: copying a saved Some checkpoint cannot mint this witness.
#[derive(Clone,Debug)]
pub struct PortableGameSnapshot {state:crate::GameState}
impl PortableGameSnapshot {
    pub fn state(&self)->&crate::GameState {&self.state}
    pub(crate) fn captured(state:crate::GameState)->Result<Self,String> {
        require_saved_checkpoints(&state)?;Ok(Self{state})
    }
}

/// Build a portable clone at the same instant as a renderer's decoder fence.
/// No live state, token, clock, UI, PC, RNG or history is mutated here.
pub fn checkpointed_state(
    original: &crate::GameState,
    instance_id: u64,
    runtime_epoch: u64,
    snapshot: &AudioCheckpointSnapshot,
) -> Result<crate::GameState, String> {
    if snapshot.instance_id != instance_id || snapshot.runtime_epoch != runtime_epoch
        || snapshot.rows.len() > crate::audio_playback::MAX_AUDIO_PLAYBACKS
        || snapshot.music.len() > crate::scene_resources::MAX_MOTIONS+1
    {
        return Err("Audio checkpoint does not belong to this exact Engine instant".into());
    }
    crate::scene_resources::validate_current_resource_state(original)?;
    let mut next = original.clone();
    let Some(registry) = next.audio_resource_registry_mut() else {
        if snapshot.rows.is_empty() && snapshot.music.is_empty() { return Ok(next); }
        return Err("Audio checkpoint has no actual Scene/Game audio registry".into());
    };
    let wanted: Vec<_> = registry.tasks.iter().filter(|task|
        task.audio.is_some() && !matches!(task.key.owner, crate::scene_tasks::TaskOwner::Host { .. })
    ).map(|task| crate::scene_resources::ResourceToken {
        task: crate::scene_tasks::TaskToken { runtime_epoch, key: task.key.clone(), revision: task.revision },
        target: task.target.clone(),
    }).collect();
    if wanted.len() != snapshot.rows.len() {
        return Err("Every live and parked Story audio reader needs its own fresh checkpoint".into());
    }
    let mut seen = std::collections::HashSet::new();
    for row in &snapshot.rows {
        if !seen.insert(row.token.target.clone()) || !wanted.contains(&row.token) {
            return Err("Duplicate, stale or foreign audio checkpoint capability".into());
        }
        row.checkpoint.validate()?;
        let task = registry.tasks.iter_mut().find(|task|
            task.key == row.token.task.key && task.revision == row.token.task.revision
                && task.target == row.token.target
        ).ok_or("Checkpointed audio task disappeared")?;
        task.audio.as_mut().unwrap().checkpoint = Some(row.checkpoint.clone());
    }
    let music_wanted:Vec<_>=music_views(original,runtime_epoch)?.into_iter().filter(|view|!matches!(view.owner,crate::scene_tasks::TaskOwner::Host{..})).collect();
    if music_wanted.len()!=snapshot.music.len() {return Err("Every Current and parked Story Music reader needs its fresh checkpoint".into());}
    for row in &snapshot.music {
        if !seen.insert(row.target.clone()) || !music_wanted.iter().any(|view|view.authority.target==row.target && view.file==row.file) {
            return Err("Duplicate, stale or foreign Music checkpoint capability".into());
        }
        row.checkpoint.validate()?;
    }
    // Replace only Story cursors. Non-portable Host readers remain in the live
    // clone until the existing story-only preflight removes them coherently.
    registry.music_checkpoints.retain(|row|!music_wanted.iter().any(|view|view.authority.target==row.target));
    registry.music_checkpoints.extend(snapshot.music.iter().cloned());
    crate::scene_resources::validate_current_resource_state(&next)?;
    Ok(next)
}

/// Conservative same-instant comparison for first publication. Only previous
/// decoder metadata and the already-excluded physical Host UI are removed.
/// PC/variables/RNG, position/owner/targets, all clocks and frames stay exact.
pub(crate) fn same_logical_instant(left:&crate::GameState,right:&crate::GameState)->Result<bool,String> {
    Ok(logical_bytes(left)?==logical_bytes(right)?)
}
fn logical_bytes(state:&crate::GameState)->Result<Vec<u8>,String> {
    fn comparable(state:&crate::GameState)->crate::GameState {
        let mut state=state.clone();state.ui=state.ui.story_only();
        if let Some(registry)=state.audio_resource_registry_mut() {
            registry.music_checkpoints.clear();
            for task in &mut registry.tasks {if let Some(audio)=&mut task.audio {audio.checkpoint=None;}}
        }
        state
    }
    serde_json::to_vec(&comparable(state)).map_err(|error|error.to_string())
}

/// Copy only exact decoder fields. The historical state itself is never replaced
/// by a later live state, even when a reader became ready after an async load.
pub(crate) fn install_metadata(destination:&mut crate::GameState,captured:&crate::GameState)->Result<(),String> {
    if !same_logical_instant(destination,captured)? {return Err("Audio history no longer belongs to the captured logical instant".into());}
    let (Some(to),Some(from))=(destination.audio_resource_registry_mut(),captured.audio_resource_registry()) else {return Ok(());};
    for task in &mut to.tasks {
        if let Some(audio)=&mut task.audio {
            let exact=from.tasks.iter().find(|source|source.key==task.key && source.revision==task.revision && source.target==task.target)
                .and_then(|source|source.audio.as_ref()).ok_or("Exact historical audio task disappeared")?;
            audio.checkpoint=exact.checkpoint.clone();
        }
    }
    to.music_checkpoints=from.music_checkpoints.clone();Ok(())
}

/// History is intentionally before the interactive instruction, while its
/// producer anchor is after it. That established PC/RNG contract stays intact.
/// The producer must have checked the full anchor first; only resource clocks,
/// targets, owners and revisions are compared here, then only cursors copied.
pub(crate) fn install_historical_metadata(destination:&mut crate::GameState,captured:&crate::GameState)->Result<(),String> {
    let (Some(to),Some(from))=(destination.audio_resource_registry_mut(),captured.audio_resource_registry()) else{return Ok(());};
    let comparable=|resources:&crate::scene_resources::ResourceRegistry| {
        let mut resources=resources.clone();resources.music_checkpoints.clear();
        for task in &mut resources.tasks {if let Some(audio)=&mut task.audio {audio.checkpoint=None;}}resources
    };
    if serde_json::to_vec(&comparable(&to)).map_err(|error|error.to_string())? != serde_json::to_vec(&comparable(&from)).map_err(|error|error.to_string())? {
        return Err("Historical resources differ from the exact interactive producer".into());
    }
    for task in &mut to.tasks {
        if let Some(audio)=&mut task.audio {audio.checkpoint=from.tasks.iter().find(|source|source.key==task.key && source.revision==task.revision && source.target==task.target).and_then(|source|source.audio.as_ref()).ok_or("Historical audio task disappeared")?.checkpoint.clone();}
    }
    to.music_checkpoints=from.music_checkpoints.clone();Ok(())
}
