//! Logical one-shot playback identities. No decoder, entity or guessed playhead.
use crate::scene_resources::{resource_key, EffectClock, ResourceTaskKind, ResourceToken, TargetKind};
use crate::scene_tasks::{TaskOwner, TaskToken};
use crate::types::GameState;
use serde::{Deserialize, Serialize};

pub const MAX_AUDIO_PLAYBACKS: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioFamily { Sfx, Voice }
impl AudioFamily {
    pub fn task_kind(self) -> ResourceTaskKind { match self { Self::Sfx => ResourceTaskKind::Sfx, Self::Voice => ResourceTaskKind::Voice } }
    pub fn target_kind(self) -> TargetKind { match self { Self::Sfx => TargetKind::SfxPlayback, Self::Voice => TargetKind::VoicePlayback } }
    fn prefix(self) -> &'static str { match self { Self::Sfx => "sfx", Self::Voice => "voice" } }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioEnvelope { pub clock: EffectClock, pub from: f32, pub to: f32 }
impl AudioEnvelope {
    pub fn gain(&self) -> f32 { crate::effect_sampling::sample_audio(self.from, self.to, &self.clock) }
    fn validate(&self) -> Result<(), String> {
        self.clock.validate()?;
        if self.clock.looping || ![self.from,self.to].iter().all(|gain|gain.is_finite() && (0.0..=1.0).contains(gain)) {
            return Err("Invalid owned audio envelope".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioPlayback {
    pub family: AudioFamily,
    /// The immutable authored path is payload, never the reader's identity.
    pub file: String,
    pub envelope: Option<AudioEnvelope>,
    pub stopping: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkpoint: Option<crate::audio_checkpoint::AudioCheckpoint>,
}
impl AudioPlayback {
    pub fn new(family: AudioFamily, file: String, fade_seconds: f64) -> Result<Self,String> {
        if !fade_seconds.is_finite() || !(0.0..=86400.0).contains(&fade_seconds) { return Err("Invalid owned audio fade duration".into()); }
        let value = Self { family, file, envelope: (fade_seconds>0.0).then_some(AudioEnvelope {
            clock:EffectClock{duration:fade_seconds,elapsed:0.0,looping:false}, from:0.0,to:1.0,
        }), stopping:false, checkpoint:None };
        value.validate()?; Ok(value)
    }
    pub fn gain(&self)->f32 { self.envelope.as_ref().map(AudioEnvelope::gain).unwrap_or(1.0) }
    pub fn validate(&self)->Result<(),String> {
        if self.file.is_empty() || self.file.len()>4096 || self.file.chars().any(char::is_control) { return Err("Invalid owned audio source path".into()); }
        if let Some(checkpoint)=&self.checkpoint {
            checkpoint.validate()?;
            if checkpoint.looping {return Err("One-shot SFX/Voice cannot adopt a looping decoder checkpoint".into());}
        }
        if self.family==AudioFamily::Voice && (self.envelope.is_some() || self.stopping) { return Err("Voice cannot invent an authored fade".into()); }
        if let Some(envelope)=&self.envelope {
            envelope.validate()?;
            if envelope.clock.duration<=0.0 || (!self.stopping && (envelope.from.to_bits()!=0.0f32.to_bits() || envelope.to.to_bits()!=1.0f32.to_bits())) {return Err("An incoming SFX requires its canonical 0-to-1 envelope".into());}
        }
        if self.stopping && !self.envelope.as_ref().is_some_and(|fade|fade.to.to_bits()==0.0f32.to_bits()) { return Err("An outgoing SFX requires its exact zero destination".into()); }
        Ok(())
    }
}

/// This includes parked Scene residents so consumers pause an existing reader
/// rather than treating a hidden parent as revoked. They must not start it.
#[derive(Clone, Debug, PartialEq)]
pub struct AudioPlaybackView { pub token:ResourceToken, pub playback:AudioPlayback, pub parked:bool }

pub fn describe(state:&GameState,epoch:u64)->Result<Vec<AudioPlaybackView>,String> {
    crate::scene_resources::validate_current_resource_state(state)?;
    let Some(registry)=state.audio_resource_registry() else {return Ok(Vec::new());};
    Ok(registry.tasks.iter().filter_map(|task|task.audio.as_ref().map(|audio|AudioPlaybackView {
        token:ResourceToken{task:TaskToken{runtime_epoch:epoch,key:task.key.clone(),revision:task.revision},target:task.target.clone()},
        playback:audio.clone(),parked:task.parked,
    })).collect())
}

/// Registration is transactional even when called from an already-cloned
/// Engine candidate. Allocation cannot leak on cap, owner or payload refusal.
pub fn register(state:&mut GameState,epoch:u64,owner:TaskOwner,playback:AudioPlayback)->Result<ResourceToken,String> {
    crate::scene_resources::validate_current_resource_state(state)?;
    playback.validate()?;
    let mut next=state.clone();
    let active=next.audio_active_owner();
    let registry=next.audio_resource_registry_mut().ok_or("Owned audio requires an actual Scene/Game audio registry")?;
    if registry.tasks.iter().filter(|task|task.audio.is_some()).count()>=MAX_AUDIO_PLAYBACKS {return Err("Owned audio playback limit: 64".into());}
    let id=registry.next_incarnation.checked_add(1).ok_or("Audio playback incarnation exhausted")?;
    let alias=format!("{}_{}",playback.family.prefix(),id);
    let kind=playback.family.task_kind();
    let target=registry.ensure_target(owner.clone(),playback.family.target_kind(),&alias)?;
    let key=resource_key(owner,kind,&alias);
    registry.register(key.clone(),target.clone(),kind,None)?;
    registry.tasks.iter_mut().find(|task|task.key==key).unwrap().audio=Some(playback);
    let token=registry.effect_token(epoch,&key,active).ok_or("Owned audio cannot start under an unavailable owner")?;
    crate::scene_resources::validate_current_resource_state(&next)?;
    *state=next; Ok(token)
}

fn remove_exact(state:&mut GameState,token:&ResourceToken)->Result<bool,String> {
    let registry=state.audio_resource_registry_mut().ok_or("Owned audio requires an actual Scene/Game audio registry")?;
    let Some(task)=registry.tasks.iter().find(|task|task.key==token.task.key && task.revision==token.task.revision && task.target==token.target).cloned() else {return Ok(false);};
    if task.audio.is_none() || task.wait.is_some() || !task.dependencies.is_empty() {return Err("Exact audio cleanup cannot adopt another task".into());}
    if registry.tasks.iter().any(|other|other.key!=task.key && (other.target==task.target || other.dependencies.contains(&task.target))) {return Err("A playback target has foreign dependents".into());}
    registry.supersede(&task.key)?;
    registry.targets.retain(|record|record.identity!=task.target);
    Ok(true)
}

/// Story Stop is a command selection over payloads, after ownership admission.
/// Host reads are never included. Parked Story reads are deliberately included.
pub fn stop_story(state:&mut GameState,family:AudioFamily,file:Option<&str>,fade_seconds:f64)->Result<usize,String> {
    crate::scene_resources::validate_current_resource_state(state)?;
    if !fade_seconds.is_finite() || !(0.0..=86400.0).contains(&fade_seconds) || (family==AudioFamily::Voice && fade_seconds!=0.0) {return Err("Invalid Story audio stop fade".into());}
    let mut next=state.clone();
    let Some(registry)=next.audio_resource_registry() else {return Ok(0);};
    let keys=registry.tasks.iter().filter(|task|!matches!(task.key.owner,TaskOwner::Host{..}) && task.audio.as_ref().is_some_and(|audio|audio.family==family && file.is_none_or(|file|file==audio.file)))
        .map(|task|task.key.clone()).collect::<Vec<_>>();
    let mut changed=0;
    for key in keys {
        let registry=next.audio_resource_registry_mut().unwrap();
        let task=registry.tasks.iter().find(|task|task.key==key).unwrap().clone();
        let audio=task.audio.as_ref().unwrap();
        if audio.stopping && fade_seconds>0.0 {continue;}
        if fade_seconds==0.0 {
            let token=ResourceToken{task:TaskToken{runtime_epoch:0,key:task.key,revision:task.revision},target:task.target};
            changed+=usize::from(remove_exact(&mut next,&token)?);
        } else {
            let gain=audio.gain();
            registry.renew_task(&key)?;
            let audio=registry.tasks.iter_mut().find(|task|task.key==key).unwrap().audio.as_mut().unwrap();
            audio.stopping=true;audio.envelope=Some(AudioEnvelope{clock:EffectClock{duration:fade_seconds,elapsed:0.0,looping:false},from:gain,to:0.0});
            changed+=1;
        }
    }
    crate::scene_resources::validate_current_resource_state(&next)?;
    *state=next; Ok(changed)
}

#[derive(Clone, Copy, Debug)]
pub enum AudioClockRoute { Story, Host }

/// Advance every eligible fade on the clone before any callback. This never
/// measures decoder time, runs a handler or modifies narrative continuation.
pub fn advance_clocks(state:&mut GameState,epoch:u64,route:AudioClockRoute,seconds:f64)->Result<bool,String> {
    if !seconds.is_finite() || !(0.0..=3600.0).contains(&seconds) {return Err("Audio frame delta must be finite in 0..3600".into());}
    crate::scene_resources::validate_current_resource_state(state)?;
    let mut next=state.clone();
    let Some(registry)=next.audio_resource_registry_mut() else {return Ok(false);};
    let captured=registry.tasks.iter().filter(|task|!task.parked && task.audio.is_some() && match route {
        AudioClockRoute::Host=>matches!(task.key.owner,TaskOwner::Host{..}),AudioClockRoute::Story=>!matches!(task.key.owner,TaskOwner::Host{..}),
    }).map(|task|ResourceToken{task:TaskToken{runtime_epoch:epoch,key:task.key.clone(),revision:task.revision},target:task.target.clone()}).collect::<Vec<_>>();
    let mut changed=false;
    for token in captured {
        let active=next.audio_active_owner();let registry=next.audio_resource_registry_mut().unwrap();
        if !registry.accepts(&token,epoch,active) {continue;}
        let audio=registry.tasks.iter_mut().find(|task|task.key==token.task.key).unwrap().audio.as_mut().unwrap();
        if let Some(envelope)=&mut audio.envelope {
            let clock=envelope.clock.advanced(seconds)?;changed|=clock!=envelope.clock;envelope.clock=clock;
            if envelope.clock.is_due() {
                if !audio.stopping {audio.envelope=None;changed=true;}
            }
        }
    }
    crate::scene_resources::validate_current_resource_state(&next)?;
    *state=next; Ok(changed)
}

/// Terminal fade cleanup is phase two, after captured decoder deliveries and
/// callbacks. A current Error cannot be hidden by advancing its fade clock.
/// If a callback parks the owner, its terminal descriptor remains resident.
pub fn complete_due_clocks(state:&mut GameState,epoch:u64,route:AudioClockRoute)->Result<bool,String> {
    crate::scene_resources::validate_current_resource_state(state)?;
    let mut next=state.clone();
    let Some(registry)=next.audio_resource_registry() else{return Ok(false);};
    let active=next.audio_active_owner();
    let due=registry.tasks.iter().filter(|task|!task.parked && match route {
        AudioClockRoute::Host=>matches!(task.key.owner,TaskOwner::Host{..}),
        AudioClockRoute::Story=>!matches!(task.key.owner,TaskOwner::Host{..}),
    }).filter(|task|task.audio.as_ref().is_some_and(|audio|audio.stopping && audio.envelope.as_ref().is_some_and(|fade|fade.clock.is_due())))
        .filter_map(|task|registry.effect_token(epoch,&task.key,active)).collect::<Vec<_>>();
    let mut changed=false;
    for token in due {changed|=remove_exact(&mut next,&token)?;}
    crate::scene_resources::validate_current_resource_state(&next)?;
    *state=next;Ok(changed)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AudioFeedback { End, Error(String) }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioReport {
    /// Process-local Engine lineage, not a registry token or saved identity.
    pub instance_id:u64,
    pub token:ResourceToken,
    pub feedback:AudioFeedback,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioAdmission { Stale, Deferred, Current }

pub fn admission(state:&GameState,epoch:u64,token:&ResourceToken)->AudioAdmission {
    if crate::scene_resources::validate_current_resource_state(state).is_err(){return AudioAdmission::Stale;}
    let active=state.audio_active_owner();
    if token.task.runtime_epoch!=epoch {return AudioAdmission::Stale;}
    let Some(registry)=state.audio_resource_registry() else {return AudioAdmission::Stale;};
    let Some(task)=registry.tasks.iter().find(|task|task.key==token.task.key && task.revision==token.task.revision && task.target==token.target && task.audio.is_some()) else {return AudioAdmission::Stale;};
    if task.parked {AudioAdmission::Deferred} else if registry.accepts(token,epoch,active) {AudioAdmission::Current} else {AudioAdmission::Stale}
}
/// Error is a transaction refusal retaining its exact queued record; End is
/// cleanup only. There is deliberately no callback or invented AudioWait.
pub fn apply_report(state:&mut GameState,epoch:u64,report:&AudioReport)->Result<AudioAdmission,String> {
    let admitted=admission(state,epoch,&report.token);
    if admitted!=AudioAdmission::Current {return Ok(admitted);}
    match &report.feedback {
        AudioFeedback::Error(message)=>{
            if message.is_empty() || message.len()>4096 || message.contains('\0') {return Err("Invalid current audio decoder error".into());}
            Err(format!("Owned audio decoder: {message}"))
        },
        AudioFeedback::End=>{
            let mut next=state.clone();remove_exact(&mut next,&report.token)?;
            crate::scene_resources::validate_current_resource_state(&next)?;
            *state=next;Ok(admitted)
        },
    }
}

pub fn validate_task(task:&crate::scene_resources::ResourceTask)->Result<(),String> {
    let audio_kind=matches!(task.kind,ResourceTaskKind::Sfx|ResourceTaskKind::Voice);
    if audio_kind!=task.audio.is_some() {return Err("Owned audio descriptor and task kind disagree".into());}
    let Some(audio)=&task.audio else {return Ok(());};audio.validate()?;
    if task.kind!=audio.family.task_kind() || task.target.kind!=audio.family.target_kind() || task.target.alias!=format!("{}_{}",audio.family.prefix(),task.target.incarnation)
        || task.wait.is_some() || task.effect.is_some() || !task.dependencies.is_empty() || task.key!=resource_key(task.key.owner.clone(),task.kind,&task.target.alias) {return Err("Invalid exact audio playback identity or ownership".into());}
    Ok(())
}

/// Temporary refusal until a real decoder checkpoint adapter is approved.
/// This preserves originals and explicitly does not close portable audio.
pub fn require_portable_checkpoint(state:&GameState)->Result<(),String> {
    let Some(registry)=state.audio_resource_registry() else {return Ok(());};
    let audio_target=|target:&crate::scene_resources::TargetIdentity|matches!(target.kind,TargetKind::SfxPlayback|TargetKind::VoicePlayback);
    let story_task=registry.tasks.iter().any(|task|!matches!(task.key.owner,TaskOwner::Host{..}) && (task.audio.is_some() || matches!(task.kind,ResourceTaskKind::Sfx|ResourceTaskKind::Voice) || audio_target(&task.target) || task.dependencies.iter().any(audio_target)));
    let story_target=registry.targets.iter().any(|record|!matches!(record.owner,TaskOwner::Host{..}) && audio_target(&record.identity));
    let continuation=registry.waits.iter().any(|wait|audio_target(&wait.target))
        || registry.cancelled_waits.iter().any(|receipt|audio_target(&receipt.target));
    if story_task || story_target || continuation {
        Err("Story audio descriptors require a trustworthy decoder checkpoint and a validated version barrier; original save is preserved".into())
    } else {Ok(())}
}
