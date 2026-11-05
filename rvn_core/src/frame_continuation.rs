//! Logical frame facts are data. Only this Engine child rebuilds transient
//! reports after matching the actual restored resource/source registries.
use super::*;
use crate::consumer_schedule::HostDefinitionKind;
use crate::save::SaveError;
use crate::scene_resources::{ResourceTaskKind, ResourceToken, TargetIdentity};
use crate::scene_tasks::{TaskKey, TaskOwner, TaskToken};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[path = "frame_host.rs"]
mod host;
pub use host::HostLifecycleRestoreContext;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FrameClock {
    Game,
    Host,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PortableResource {
    key: TaskKey,
    revision: u64,
    target: TargetIdentity,
    kind: ResourceTaskKind,
    dependencies: Vec<TargetIdentity>,
    parked: bool,
    payload: [u8; 32],
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(in crate::engine) enum VideoPayload {
    Position { seconds: f64, duration: f64 },
    End,
    Error { message: String },
    AutoplayBlocked { message: String },
    Resume,
}
impl VideoPayload {
    pub(in crate::engine) fn capture(feedback: &GameVideoFeedback) -> Self {
        match feedback {
            GameVideoFeedback::Resume => Self::Resume,
            GameVideoFeedback::Report(crate::video::Feedback::Position { seconds, duration }) => {
                Self::Position {
                    seconds: *seconds,
                    duration: *duration,
                }
            }
            GameVideoFeedback::Report(crate::video::Feedback::End) => Self::End,
            GameVideoFeedback::Report(crate::video::Feedback::Error(message)) => Self::Error {
                message: message.clone(),
            },
            GameVideoFeedback::Report(crate::video::Feedback::AutoplayBlocked(message)) => {
                Self::AutoplayBlocked {
                    message: message.clone(),
                }
            }
        }
    }
    pub(in crate::engine) fn restore(&self) -> Result<GameVideoFeedback, SaveError> {
        Ok(match self {
            Self::Position { seconds, duration } => {
                if !seconds.is_finite()
                    || !duration.is_finite()
                    || !(0.0..=86400.0).contains(seconds)
                    || !(0.001..=86400.0).contains(duration)
                    || seconds > duration
                {
                    return Err(invalid(
                        "Portable video position exceeds its real bounded payload",
                    ));
                }
                GameVideoFeedback::Report(crate::video::Feedback::Position {
                    seconds: *seconds,
                    duration: *duration,
                })
            }
            Self::End => GameVideoFeedback::Report(crate::video::Feedback::End),
            Self::Resume => GameVideoFeedback::Resume,
            Self::Error { message } | Self::AutoplayBlocked { message } => {
                if message.is_empty() || message.len() > 4096 || message.contains('\0') {
                    return Err(invalid(
                        "Portable video diagnostic exceeds its bounded payload",
                    ));
                }
                GameVideoFeedback::Report(if matches!(self, Self::Error { .. }) {
                    crate::video::Feedback::Error(message.clone())
                } else {
                    crate::video::Feedback::AutoplayBlocked(message.clone())
                })
            }
        })
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum PortableVideoReport {
    RejectedStale,
    Resident {
        source: PortableResource,
        payload: VideoPayload,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum AudioPayload {
    End,
    Error { message: String },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum PortableAudioReport {
    RejectedStale,
    Resident {
        source: PortableResource,
        payload: AudioPayload,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PortableInterface {
    target: TargetIdentity,
    order: u64,
    role: Option<rvn_ui::PageRole>,
    definition: String,
    source_ordinal: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    host: Option<PortableHostData>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PortableHostData {
    root: bool,
    instance: crate::ui::ScreenInstance,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct HostTaskWitness {
    key: TaskKey,
    shape: [u8; 32],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    descriptor: Option<crate::scene_resources::ResourceTask>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct HostVideoData {
    alias: String,
    track: crate::video::Track,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct HostMotionData {
    alias: String,
    track: crate::motion::Track,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct HostWaitData {
    owner: Option<crate::scene::ActivationId>,
    key: TaskKey,
    revision: u64,
    target: TargetIdentity,
    pc: usize,
    parked: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct HostCancelledWaitData {
    owner: Option<crate::scene::ActivationId>,
    key: TaskKey,
    revision: u64,
    target: TargetIdentity,
    pc: usize,
    reason: crate::scene_resources::CancellationReason,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PortableFrameOperation {
    clock: FrameClock,
    seconds: f64,
    videos: Vec<PortableVideoReport>,
    audio: Vec<PortableAudioReport>,
    interfaces: Vec<PortableInterface>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    host_tasks: Vec<HostTaskWitness>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    host_videos: Vec<HostVideoData>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    host_motions: Vec<HostMotionData>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    host_music: Vec<crate::audio_checkpoint::MusicCheckpoint>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    host_waits: Vec<HostWaitData>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    host_cancelled: Vec<HostCancelledWaitData>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    host_motion_wait: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    host_video_wait: Option<String>,
}
fn invalid(message: &str) -> SaveError {
    SaveError::Incompatible(message.into())
}
fn digest(value: &impl Serialize) -> Result<[u8; 32], SaveError> {
    use sha2::{Digest, Sha256};
    let bytes = serde_json::to_vec(value)?;
    if bytes.len() > 1024 * 1024 {
        return Err(invalid("Frame resource payload exceeds 1MiB"));
    }
    Ok(Sha256::digest(bytes).into())
}

/// Opaque in-process replay wrapper. Saved DTOs cannot create this capability.
#[derive(Clone, Debug)]
pub struct RestoredFrameOperation {
    clock: FrameClock,
    seconds: f64,
    videos: Vec<GameVideoReport>,
    audio: Vec<crate::audio_playback::AudioReport>,
    video_order: Vec<Option<usize>>,
    audio_order: Vec<Option<usize>>,
}
impl RestoredFrameOperation {
    pub(super) fn is_game_frame(&self) -> bool { self.clock == FrameClock::Game }
    pub(super) fn execute<R: Renderer>(
        &self,
        engine: &mut Engine<R>,
    ) -> Result<DebugOutcome, RuntimeError> {
        fn video_receipts(
            order: &[Option<usize>],
            receipts: Vec<VideoReportReceipt>,
        ) -> Vec<VideoReportReceipt> {
            order
                .iter()
                .enumerate()
                .map(|(ordinal, index)| VideoReportReceipt {
                    ordinal,
                    disposition: index
                        .map(|index| receipts[index].disposition)
                        .unwrap_or(VideoReportDisposition::RejectedStale),
                })
                .collect()
        }
        fn audio_receipts(
            order: &[Option<usize>],
            receipts: Vec<AudioReportReceipt>,
        ) -> Vec<AudioReportReceipt> {
            order
                .iter()
                .enumerate()
                .map(|(ordinal, index)| AudioReportReceipt {
                    ordinal,
                    disposition: index
                        .map(|index| receipts[index].disposition)
                        .unwrap_or(AudioReportDisposition::RejectedStale),
                })
                .collect()
        }
        match self.clock {
            FrameClock::Game => {
                let (mut frame, audio) = engine.tick_game_frame_with_receipts_and_audio(
                    self.seconds,
                    GameFrameReports {
                        videos: self.videos.clone(),
                    },
                    self.audio.clone(),
                    true,
                )?;
                frame.receipts = video_receipts(&self.video_order, frame.receipts);
                Ok(DebugOutcome::Game((
                    frame,
                    audio_receipts(&self.audio_order, audio),
                )))
            }
            FrameClock::Host => {
                let (mut frame, audio) = engine.tick_host_frame_with_audio(
                    self.seconds,
                    HostFrameReports {
                        videos: self.videos.clone(),
                    },
                    self.audio.clone(),
                    true,
                )?;
                frame.receipts = video_receipts(&self.video_order, frame.receipts);
                Ok(DebugOutcome::Host((
                    frame,
                    audio_receipts(&self.audio_order, audio),
                )))
            }
        }
    }
}
impl PortableFrameOperation {
    pub(crate) fn is_game_frame(&self) -> bool { self.clock == FrameClock::Game }
}
impl<R: Renderer> Engine<R> {
    pub(super) fn validate_standalone_frame_owner(&self, state: &GameState) -> Result<(), SaveError> {
        if self.narrative.is_some() || state.scene_execution.is_some() || self.startup.is_some()
            || (state.game_tasks.is_none() && state.game_audio.is_none()) {
            return Err(invalid("Game frame has no actual standalone Game clock owner"));
        }
        self.consumer_program.validate_emitted(&self.script)
            .and_then(|_| self.consumer_program.validate_state_binding(state))
            .map_err(|e| invalid(&e.to_string()))?;
        self.validate_standalone_timer_target(state).map_err(|e| invalid(&e.to_string()))?;
        crate::scene_resources::validate_current_resource_state(state).map_err(|e| invalid(&e))?;
        Ok(())
    }
    fn frame_resource_facts(
        &self,
        state: &GameState,
        key: &TaskKey,
        clock: &FrameClock,
    ) -> Result<PortableResource, SaveError> {
        // Scene retains its existing registry; standalone reports are audio-only
        // and use the actual Game audio owner, never the Sprite task authority.
        let registry = state.scene_execution.as_ref().map(|e| &e.resources)
            .or_else(|| (*clock == FrameClock::Game).then_some(state.game_audio.as_ref()).flatten())
            .ok_or_else(|| invalid("Frame report needs the real resource registry"))?;
        let task = registry
            .tasks
            .iter()
            .find(|task| &task.key == key)
            .ok_or_else(|| invalid("Frame report resource is absent"))?;
        let payload = match task.kind {
            ResourceTaskKind::Video => digest(
                &state
                    .videos
                    .tracks
                    .get(task.alias())
                    .ok_or_else(|| invalid("Frame video has no actual track"))?
                    .clip,
            )?,
            ResourceTaskKind::Sfx | ResourceTaskKind::Voice => {
                let audio = task
                    .audio
                    .as_ref()
                    .ok_or_else(|| invalid("Frame audio has no actual playback"))?;
                digest(&(audio.family, &audio.file))?
            }
            _ => return Err(invalid("Frame report targets a foreign resource family")),
        };
        Ok(PortableResource {
            key: task.key.clone(),
            revision: task.revision,
            target: task.target.clone(),
            kind: task.kind,
            dependencies: task.dependencies.clone(),
            parked: task.parked,
            payload,
        })
    }
    fn frame_interfaces(&self) -> Result<Vec<PortableInterface>, SaveError> {
        let mut result = Vec::new();
        for instance in &self.state.ui.screens {
            let Some(definition) = self
                .consumer_program
                .host_definition(&HostDefinitionKind::Screen, instance.definition())
            else {
                continue;
            };
            let target = crate::ui_residency::visible_interface_target(&self.state, &instance.name)
                .map_err(SaveError::Incompatible)?;
            let resident = crate::ui_residency::resident_interface(&self.state, &target)
                .map_err(SaveError::Incompatible)?;
            result.push(PortableInterface {
                target: resident.identity,
                order: instance.order,
                role: instance.host_role,
                definition: instance.definition().to_owned(),
                source_ordinal: definition.source_ordinal,
                host: instance.host_role.map(|_| {
                    let mut data = instance.clone();
                    for bound in data.consumer_frames.values_mut() {
                        bound.program = [0; 32];
                    }
                    if let Some(projection) = &mut data.consumer_projection {
                        projection.program = [0; 32];
                    }
                    PortableHostData {
                        root: instance.host_root,
                        instance: data,
                    }
                }),
            });
        }
        Ok(result)
    }
    pub(super) fn capture_frame_operation_from_state(
        &self,
        operation: &DebugOperation,
        captured: &GameState,
    ) -> Result<Option<PortableFrameOperation>, SaveError> {
        let (clock, seconds, videos, audio, orders) = match operation {
            DebugOperation::GameFrame {
                seconds,
                reports,
                audio,
            } => (FrameClock::Game, *seconds, &reports.videos, audio, None),
            DebugOperation::HostFrame {
                seconds,
                reports,
                audio,
            } => (FrameClock::Host, *seconds, &reports.videos, audio, None),
            DebugOperation::RestoredFrame(frame) => (
                frame.clock.clone(),
                frame.seconds,
                &frame.videos,
                &frame.audio,
                Some((&frame.video_order, &frame.audio_order)),
            ),
            _ => return Ok(None),
        };
        if !seconds.is_finite()
            || !(0.0..=if clock == FrameClock::Game {
                3600.0
            } else {
                0.25
            })
                .contains(&seconds)
        {
            return Err(invalid(
                "Saved frame delta differs from actual clock contract",
            ));
        }
        if clock == FrameClock::Game && self.state.scene_execution.is_none() {
            self.validate_standalone_frame_owner(&self.state)?;
            self.validate_standalone_frame_owner(captured)?;
        }
        if videos.len() > crate::scene_resources::MAX_RESOURCES
            || audio.len() > crate::audio_playback::MAX_AUDIO_PLAYBACKS
        {
            return Err(invalid("Saved frame report count exceeds real bounds"));
        }
        let mut saved_videos = Vec::new();
        for report in videos {
            match self
                .video_report_authority(&report.token)
                .map_err(|e| invalid(&e.to_string()))?
            {
                VideoReportAuthority::Stale => {
                    saved_videos.push(PortableVideoReport::RejectedStale)
                }
                _ => {
                    let source = self.frame_resource_facts(&self.state, &report.token.task.key, &clock)?;
                    let payload = VideoPayload::capture(&report.feedback);
                    payload.restore()?;
                    saved_videos.push(PortableVideoReport::Resident { source, payload });
                }
            }
        }
        let mut saved_audio = Vec::new();
        for report in audio {
            match self.audio_report_admission(report.instance_id, &report.token) {
                crate::audio_playback::AudioAdmission::Stale => {
                    saved_audio.push(PortableAudioReport::RejectedStale)
                }
                _ => {
                    let source = self.frame_resource_facts(&self.state, &report.token.task.key, &clock)?;
                    let payload = match &report.feedback {
                        crate::audio_playback::AudioFeedback::End => AudioPayload::End,
                        crate::audio_playback::AudioFeedback::Error(message) => {
                            if message.is_empty() || message.len() > 4096 || message.contains('\0')
                            {
                                return Err(invalid("Saved audio diagnostic exceeds bound"));
                            }
                            AudioPayload::Error {
                                message: message.clone(),
                            }
                        }
                    };
                    saved_audio.push(PortableAudioReport::Resident { source, payload });
                }
            }
        }
        if let Some((video_order, audio_order)) = orders {
            saved_videos = video_order
                .iter()
                .map(|index| {
                    index
                        .map(|i| saved_videos[i].clone())
                        .unwrap_or(PortableVideoReport::RejectedStale)
                })
                .collect();
            saved_audio = audio_order
                .iter()
                .map(|index| {
                    index
                        .map(|i| saved_audio[i].clone())
                        .unwrap_or(PortableAudioReport::RejectedStale)
                })
                .collect();
        }
        let (host_tasks, host_videos) = self.capture_frame_hosts_from_state(captured)?;
        Ok(Some(PortableFrameOperation {
            clock,
            seconds,
            videos: saved_videos,
            audio: saved_audio,
            interfaces: self.frame_interfaces()?,
            host_tasks,
            host_videos,
            host_motions: self.capture_host_motions(captured)?,
            host_music: self.capture_host_music(captured)?,
            host_waits: self.capture_host_waits(captured)?,
            host_cancelled: self.capture_host_cancelled(captured)?,
            host_motion_wait: captured
                .motions
                .waiting
                .as_ref()
                .filter(|alias| self.frame_task_is_host(captured, ResourceTaskKind::Motion, alias))
                .cloned(),
            host_video_wait: captured
                .videos
                .waiting
                .as_ref()
                .filter(|alias| self.frame_task_is_host(captured, ResourceTaskKind::Video, alias))
                .cloned(),
        }))
    }
    pub(super) fn frame_story_snapshot(&self, state: &GameState) -> GameState {
        let mut story = state.clone();
        story.ui = story.ui.story_only();
        if let Some(execution) = &mut story.scene_execution {
            let host_video: Vec<_> = execution
                .resources
                .tasks
                .iter()
                .filter(|t| {
                    matches!(t.key.owner, TaskOwner::Host { .. })
                        && t.kind == ResourceTaskKind::Video
                })
                .map(|t| t.alias().to_owned())
                .collect();
            for alias in host_video {
                story.videos.stop(&alias);
            }
            let host_motion: Vec<_> = execution
                .resources
                .tasks
                .iter()
                .filter(|t| {
                    matches!(t.key.owner, TaskOwner::Host { .. })
                        && t.kind == ResourceTaskKind::Motion
                })
                .map(|t| t.alias().to_owned())
                .collect();
            for alias in host_motion {
                story.motions.tracks.remove(&alias);
                if story.motions.waiting.as_deref() == Some(&alias) {
                    story.motions.waiting = None;
                }
            }
            execution.resources = execution.resources.story_only();
            execution.resources.music_checkpoints.retain(|row|execution.resources.targets.iter().any(|record|record.identity==row.target));
            execution.resources.cancelled_waits.retain(|wait| !matches!(wait.key.owner, TaskOwner::Host { .. }));
            for active in execution.active.iter_mut().chain(
                execution
                    .frames
                    .iter_mut()
                    .filter_map(|f| f.parked.as_mut()),
            ) {
                active.tasks.suspended_ui = active.tasks.suspended_ui.story_only();
            }
        }
        story
    }
    fn capture_frame_hosts(&self) -> Result<(Vec<HostTaskWitness>, Vec<HostVideoData>), SaveError> {
        self.capture_frame_hosts_from_state(&self.state)
    }
    fn capture_frame_hosts_from_state(
        &self,
        captured: &GameState,
    ) -> Result<(Vec<HostTaskWitness>, Vec<HostVideoData>), SaveError> {
        let registry = captured
            .scene_execution
            .as_ref()
            .map(|e| &e.resources)
            .or(captured.ui.instance_registry.as_ref());
        let mut tasks = Vec::new();
        let mut videos = Vec::new();
        if let Some(registry) = registry {
            for task in registry
                .tasks
                .iter()
                .filter(|t| matches!(t.key.owner, TaskOwner::Host { .. }))
            {
                if let Some(effect) = &task.effect {
                    effect.validate().map_err(SaveError::Incompatible)?;
                }
                if let Some(audio) = &task.audio {
                    audio.validate().map_err(SaveError::Incompatible)?;
                }
                tasks.push(HostTaskWitness {
                    key: task.key.clone(),
                    shape: digest(task)?,
                    descriptor: Some(task.clone()),
                });
                if task.kind == ResourceTaskKind::Video {
                    videos.push(HostVideoData {
                        alias: task.alias().into(),
                        track: captured
                            .videos
                            .tracks
                            .get(task.alias())
                            .ok_or_else(|| invalid("Host video has no actual track"))?
                            .clone(),
                    });
                }
            }
        }
        Ok((tasks, videos))
    }
    /// Host definitions are admitted from an already established live Host
    /// context. Saved names/targets never open a screen or allocate a capability.
    pub(super) fn merge_frame_hosts(
        &self,
        next: &mut GameState,
        frame: &PortableFrameOperation,
    ) -> Result<(), SaveError> {
        let actual = self.frame_interfaces()?;
        let saved: Vec<_> = frame
            .interfaces
            .iter()
            .filter(|s| s.role.is_some())
            .collect();
        let current: Vec<_> = actual.iter().filter(|s| s.role.is_some()).collect();
        if saved.len() != current.len() || saved.len() > crate::scene_resources::MAX_INTERFACES {
            return Err(invalid(
                "Stopped frame Host context must first be recreated through the real lifecycle",
            ));
        }
        let (actual_tasks, _) = self.capture_frame_hosts()?;
        if frame.host_tasks.len() != actual_tasks.len() {
            return Err(invalid(
                "Saved Host resource count differs from actual recreated lifecycle",
            ));
        }
        for (old, actual) in frame.host_tasks.iter().zip(&actual_tasks) {
            if old.key != actual.key {
                return Err(invalid("Saved Host resource birth/order differs"));
            }
            if let Some(descriptor) = &old.descriptor {
                let current = actual.descriptor.as_ref().unwrap();
                if descriptor.key != old.key
                    || digest(descriptor)? != old.shape
                    || descriptor.target != current.target
                    || descriptor.revision != current.revision
                    || !host::compatible(descriptor,current)?
                {
                    return Err(invalid(
                        "Host descriptor identity/source differs from current admitted registry",
                    ));
                }
            } else if old.shape != actual.shape {
                return Err(invalid(
                    "Legacy Host witness differs from current actual registry",
                ));
            }
        }
        for source in saved {
            let live = current
                .iter()
                .find(|s| s.target == source.target)
                .ok_or_else(|| {
                    invalid("Saved Host target is absent or replaced in the actual registry")
                })?;
            let saved = source
                .host
                .as_ref()
                .ok_or_else(|| invalid("Saved Host interface lacks its typed data"))?;
            let live_data = live
                .host
                .as_ref()
                .ok_or_else(|| invalid("Current interface is not a Host lifecycle context"))?;
            if source.order != live.order
                || source.role != live.role
                || source.definition != live.definition
                || source.source_ordinal != live.source_ordinal
                || saved.root != live_data.root
                || saved.instance.name != live_data.instance.name
                || saved.instance.order != live_data.instance.order
                || saved.instance.arguments != live_data.instance.arguments
                || saved.instance.modal != live_data.instance.modal
                || saved.instance.layer != live_data.instance.layer
            {
                return Err(invalid(
                    "Saved Host invocation/source differs from current real lifecycle",
                ));
            }
            if next.ui.screens.iter().any(|i| i.name == source.definition) {
                return Err(invalid("Saved story aliases a current Host interface"));
            }
            let mut instance = saved.instance.clone();
            instance.host_role = live.role;
            instance.host_root = live_data.root;
            let program = self
                .consumer_program
                .identity()
                .map_err(|e| invalid(&e.to_string()))?;
            for (key, bound) in &mut instance.consumer_frames {
                if bound.program != [0; 32]
                    || bound.owner.target != live.target
                    || key != &bound.owner.storage_key()
                {
                    return Err(invalid(
                        "Host frame belongs to a different actual target/definition",
                    ));
                }
                bound.program = program;
                self.consumer_program
                    .validate_instance_binding(bound)
                    .map_err(|e| invalid(&e.to_string()))?;
            }
            if let Some(projection) = &mut instance.consumer_projection {
                if projection.program != [0; 32]
                    || projection.owner.target != live.target
                    || projection.owner.kind != HostDefinitionKind::Screen
                    || projection.owner.definition != live.definition
                    || projection.owner.source_ordinal != live.source_ordinal
                {
                    return Err(invalid(
                        "Host projection differs from current actual source owner",
                    ));
                }
                projection.program = program;
                projection.host.validate().map_err(SaveError::Incompatible)?;
                if let Some(root) = &projection.root {
                    root.validate().map_err(SaveError::Incompatible)?;
                }
            }
            next.ui.screens.push(instance);
        }
        next.ui.host_next_order = self.state.ui.host_next_order;
        if !current.is_empty() {
            let live = self
                .state
                .scene_execution
                .as_ref()
                .map(|e| &e.resources)
                .or(self.state.ui.instance_registry.as_ref())
                .ok_or_else(|| invalid("Actual Host context lacks a registry"))?;
            let registry = if let Some(scene) = &mut next.scene_execution {
                &mut scene.resources
            } else {
                next.ui
                    .instance_registry
                    .get_or_insert_with(Default::default)
            };
            for target in live
                .targets
                .iter()
                .filter(|t| matches!(t.owner, TaskOwner::Host { .. }))
            {
                if registry
                    .targets
                    .iter()
                    .any(|t| t.identity.incarnation == target.identity.incarnation)
                {
                    return Err(invalid("Actual Host incarnation collides with saved story"));
                }
                registry.targets.push(target.clone());
            }
            for task in live
                .tasks
                .iter()
                .filter(|t| matches!(t.key.owner, TaskOwner::Host { .. }))
            {
                if registry.tasks.iter().any(|t| t.key == task.key) {
                    return Err(invalid("Actual Host task collides with saved story"));
                }
                registry.tasks.push(task.clone());
            }
            registry.next_incarnation = registry.next_incarnation.max(live.next_incarnation);
            registry.next_revision = registry.next_revision.max(live.next_revision);
        }
        let mut aliases = std::collections::BTreeSet::new();
        for video in &frame.host_videos {
            if !aliases.insert(&video.alias)
                || !frame
                    .host_tasks
                    .iter()
                    .any(|t| t.key.local_id == format!("video:{}", video.alias))
            {
                return Err(invalid(
                    "Host video lacks an actual admitted resource witness",
                ));
            }
            video.track.validate().map_err(SaveError::Incompatible)?;
            let current = self
                .state
                .videos
                .tracks
                .get(&video.alias)
                .ok_or_else(|| invalid("Host video is absent from actual lifecycle"))?;
            if current.clip != video.track.clip || next.videos.tracks.contains_key(&video.alias) {
                return Err(invalid(
                    "Saved Host video source differs or collides with story",
                ));
            }
            next.videos
                .tracks
                .insert(video.alias.clone(), video.track.clone());
        }
        self.install_host_payloads(next, frame)?;
        self.consumer_program
            .validate_state_binding(next)
            .map_err(|e| invalid(&e.to_string()))?;
        Ok(())
    }
    pub(super) fn frame_namespaces(
        &self,
        frame: &PortableFrameOperation,
    ) -> Result<BTreeMap<String, BTreeMap<String, String>>, SaveError> {
        let mut result = BTreeMap::new();
        if frame.interfaces.is_empty() {
            return Ok(result);
        }
        let source = self
            .consumer_program
            .source
            .as_ref()
            .ok_or_else(|| invalid("Frame has no frozen definition catalogue"))?;
        for (root, definition) in &source.definitions {
            let scopes = definition
                .scopes
                .keys()
                .map(|scope| {
                    (
                        scope.clone(),
                        format!(
                            "host-source:{}:{}:{}:{}",
                            definition.source_ordinal,
                            root.len(),
                            root,
                            scope
                        ),
                    )
                })
                .collect();
            result.insert(root.clone(), scopes);
        }
        Ok(result)
    }
    pub(super) fn frame_permit(
        &self,
        frame: &PortableFrameOperation,
    ) -> Result<Option<crate::ui_consumer::HostContinuationPermit>, SaveError> {
        frame
            .interfaces
            .first()
            .map(|source| {
                crate::ui_consumer::HostContinuationPermit::from_resident(
                    &self.consumer_program,
                    &self.state,
                    &source.target,
                )
                .map_err(|e| invalid(&e.to_string()))
            })
            .transpose()
    }
    pub(super) fn restore_frame_operation(
        &self,
        portable: &PortableFrameOperation,
        logical: &GameState,
    ) -> Result<RestoredFrameOperation, SaveError> {
        if !portable.seconds.is_finite()
            || !(0.0..=if portable.clock == FrameClock::Game {
                3600.0
            } else {
                0.25
            })
                .contains(&portable.seconds)
            || portable.videos.len() > crate::scene_resources::MAX_RESOURCES
            || portable.audio.len() > crate::audio_playback::MAX_AUDIO_PLAYBACKS
            || portable.interfaces.len() > crate::scene_resources::MAX_INTERFACES
        {
            return Err(invalid(
                "Portable frame exceeds actual delta/report/interface bounds",
            ));
        }
        let actual_interfaces = self.frame_interfaces()?;
        let mut targets = std::collections::HashSet::new();
        if actual_interfaces.len() != portable.interfaces.len()
            || portable.interfaces.iter().any(|source| {
                !targets.insert(source.target.clone())
                    || (source.role.is_some() != source.host.is_some())
            })
        {
            return Err(invalid(
                "Portable frame interface inventory differs from actual unique residents",
            ));
        }
        if portable.clock == FrameClock::Game && self.state.scene_execution.is_none() {
            self.validate_standalone_frame_owner(&self.state)?;
            self.validate_standalone_frame_owner(logical)?;
        }
        for source in &portable.interfaces {
            let actual = crate::ui_residency::resident_interface(&self.state, &source.target)
                .map_err(SaveError::Incompatible)?;
            let definition = self
                .consumer_program
                .host_definition(&HostDefinitionKind::Screen, &source.definition)
                .ok_or_else(|| invalid("Portable frame Screen definition is absent"))?;
            if actual.instance.definition() != source.definition.as_str()
                || actual.instance.order != source.order
                || actual.instance.host_role != source.role
                || definition.source_ordinal != source.source_ordinal
            {
                return Err(invalid(
                    "Portable frame interface differs from actual restored definition/lifetime",
                ));
            }
        }
        fn check(expected: &PortableResource, actual: PortableResource) -> Result<(), SaveError> {
            if expected.key != actual.key
                || expected.revision != actual.revision
                || expected.target != actual.target
                || expected.kind != actual.kind
                || expected.dependencies != actual.dependencies
                || expected.parked != actual.parked
                || expected.payload != actual.payload
            {
                return Err(invalid(
                    "Portable report differs from saved actual resource/source facts",
                ));
            }
            Ok(())
        }
        let rebuild = |source: &PortableResource| -> Result<ResourceToken, SaveError> {
            check(source, self.frame_resource_facts(logical, &source.key, &portable.clock)?)?;
            let actual = self.frame_resource_facts(&self.state, &source.key, &portable.clock)?;
            if actual.target != source.target
                || actual.kind != source.kind
                || actual.dependencies != source.dependencies
                || actual.parked != source.parked
                || actual.payload != source.payload
            {
                return Err(invalid(
                    "Restored report resource/source differs after reconciliation",
                ));
            }
            Ok(ResourceToken {
                task: TaskToken {
                    runtime_epoch: self.runtime_epoch,
                    key: actual.key,
                    revision: actual.revision,
                },
                target: actual.target,
            })
        };
        let mut videos = Vec::new();
        let mut video_order = Vec::new();
        for report in &portable.videos {
            match report {
                PortableVideoReport::RejectedStale => video_order.push(None),
                PortableVideoReport::Resident { source, payload } => {
                    if source.kind != ResourceTaskKind::Video {
                        return Err(invalid("Saved video report belongs to another family"));
                    }
                    let token = rebuild(source)?;
                    video_order.push(Some(videos.len()));
                    videos.push(GameVideoReport {
                        token,
                        feedback: payload.restore()?,
                    });
                }
            }
        }
        let mut audio = Vec::new();
        let mut audio_order = Vec::new();
        for report in &portable.audio {
            match report {
                PortableAudioReport::RejectedStale => audio_order.push(None),
                PortableAudioReport::Resident { source, payload } => {
                    if !matches!(source.kind, ResourceTaskKind::Sfx | ResourceTaskKind::Voice) {
                        return Err(invalid("Saved audio report belongs to another family"));
                    }
                    let token = rebuild(source)?;
                    let feedback = match payload {
                        AudioPayload::End => crate::audio_playback::AudioFeedback::End,
                        AudioPayload::Error { message } => {
                            if message.is_empty() || message.len() > 4096 || message.contains('\0')
                            {
                                return Err(invalid("Saved audio diagnostic exceeds bound"));
                            }
                            crate::audio_playback::AudioFeedback::Error(message.clone())
                        }
                    };
                    audio_order.push(Some(audio.len()));
                    audio.push(crate::audio_playback::AudioReport {
                        instance_id: self.instance_id(),
                        token,
                        feedback,
                    });
                }
            }
        }
        Ok(RestoredFrameOperation {
            clock: portable.clock.clone(),
            seconds: portable.seconds,
            videos,
            audio,
            video_order,
            audio_order,
        })
    }
}

impl PortableFrameOperation {
    pub(crate) fn visit_widget_value_roots_mut(&mut self, visit:&mut impl FnMut(&mut Value)->Result<(),String>)->Result<(),String>{
        for interface in &mut self.interfaces { if let Some(host)=&mut interface.host { crate::widget_reference_values_0974::visit_instance_values_mut(&mut host.instance,visit)?; } }
        Ok(())
    }
}
