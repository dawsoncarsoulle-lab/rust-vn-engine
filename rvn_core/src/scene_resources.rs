//! Saved resource identities and pure, transactional scene suspension.
//! No entity, decoder, texture, pointer capture or renderer handle belongs here.
use crate::scene::{ActivationId, NarrativeBudget, ResumePosition, SceneExecution};
use crate::scene_tasks::{TaskKey, TaskOwner, TaskToken};
use crate::types::GameState;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashSet};

pub const MAX_MOTIONS: usize = 64;
pub const MAX_RESOURCES: usize = 4096;
pub const MAX_VIDEOS: usize = 8;
pub const MAX_INTERFACES: usize = 32;
pub const MAX_CANVASES: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetKind {
    Background,
    Sprite,
    Layer,
    Music,
    Video,
    Interface,
    Canvas,
    PresentationRetired,
    SfxPlayback,
    VoicePlayback,
    Cinematic,
}

/// An alias is an address. Incarnation is the saved identity of its object.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetIdentity {
    pub kind: TargetKind,
    pub alias: String,
    pub incarnation: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetRecord {
    pub owner: TaskOwner,
    pub identity: TargetIdentity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceTaskKind {
    Motion,
    Video,
    Interface,
    Canvas,
    Effect,
    Sfx,
    Voice,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CancellationReason {
    TargetRemoved,
    TargetReplaced,
    OperationSuperseded,
    OwnerAbandoned,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CancelledWait {
    pub owner: Option<ActivationId>,
    pub key: TaskKey,
    pub revision: u64,
    pub target: TargetIdentity,
    pub position: ResumePosition,
    pub reason: CancellationReason,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WaitLease {
    pub owner: Option<ActivationId>,
    pub key: TaskKey,
    pub revision: u64,
    pub target: TargetIdentity,
    pub position: ResumePosition,
    pub parked: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ResourceTransition {
    pub cancelled_waits: Vec<CancelledWait>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourceToken {
    pub task: TaskToken,
    pub target: TargetIdentity,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectClock {
    pub duration: f64,
    pub elapsed: f64,
    pub looping: bool,
}
impl EffectClock {
    pub fn validate(&self) -> Result<(), String> {
        if !self.duration.is_finite()
            || !(0.0..=86400.0).contains(&self.duration)
            || !self.elapsed.is_finite()
            || self.elapsed < 0.0
            || (!self.looping && self.elapsed > self.duration)
            || (self.looping && (self.duration <= 0.0 || self.elapsed > 1.0e12))
        {
            return Err("Invalid saved effect clock".into());
        }
        Ok(())
    }

    /// Core owns the saved clock. Projection may derive a cyclic phase but
    /// must not wrap, round or advance this cumulative time itself.
    pub fn advanced(&self, delta: f64) -> Result<Self, String> {
        self.validate()?;
        if !delta.is_finite() || delta < 0.0 {
            return Err("An effect delta must be finite and nonnegative".into());
        }
        if delta == 0.0 {
            return Ok(self.clone());
        }
        let elapsed = if self.looping {
            self.elapsed + delta
        } else {
            (self.elapsed + delta.min(self.duration - self.elapsed)).min(self.duration)
        };
        let next = Self {
            duration: self.duration,
            elapsed,
            looping: self.looping,
        };
        next.validate()?;
        Ok(next)
    }

    /// Test a clock already validated by its candidate transaction.
    pub fn is_due(&self) -> bool {
        !self.looping && self.elapsed >= self.duration
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransformSnapshot {
    pub translation: [f32; 3],
    pub scale: [f32; 3],
    pub rotation: [f32; 4],
}
impl Default for TransformSnapshot {
    fn default() -> Self {
        Self {
            translation: [0.0; 3],
            scale: [1.0; 3],
            rotation: [0.0, 0.0, 0.0, 1.0],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SpriteEffectKind {
    Shake { intensity: f32 },
    Bounce { height: f32 },
    Pulse { scale: f32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VisualTransition {
    Fade,
    Dissolve,
    SlideLeft,
    SlideRight,
    SlideUp,
    SlideDown,
    ZoomIn,
    ZoomOut,
    Wipe,
    Blur,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum EffectParameters {
    SpriteAnimation {
        animation: SpriteEffectKind,
        base: TransformSnapshot,
    },
    VisualFade {
        from: f32,
        to: f32,
        despawn_on_finish: bool,
        transition: VisualTransition,
        base: TransformSnapshot,
        layered: bool,
    },
    AudioFade {
        from: f32,
        to: f32,
        despawn_on_done: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SavedEffect {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presentation: Option<crate::retired_presentations::RetiredPresentation>,
    #[serde(default)]
    pub base_resolved: bool,
    pub clock: EffectClock,
    pub parameters: EffectParameters,
}
impl SavedEffect {
    pub fn validate(&self) -> Result<(), String> {
        let clock = &self.clock;
        clock.validate()?;
        let base = match &self.parameters {
            EffectParameters::SpriteAnimation { animation, base } => {
                let parameter = match animation {
                    SpriteEffectKind::Shake { intensity } => *intensity,
                    SpriteEffectKind::Bounce { height } => *height,
                    SpriteEffectKind::Pulse { scale } => *scale,
                };
                if !parameter.is_finite() || parameter.abs() > 1.0e6 {
                    return Err("Invalid saved sprite effect parameter".into());
                }
                Some(base)
            }
            EffectParameters::VisualFade { from, to, base, .. } => {
                if !valid_unit(*from) || !valid_unit(*to) || clock.looping {
                    return Err("Invalid saved visual fade".into());
                }
                Some(base)
            }
            EffectParameters::AudioFade { from, to, .. } => {
                if !valid_unit(*from) || !valid_unit(*to) || clock.looping {
                    return Err("Invalid saved audio fade".into());
                }
                None
            }
        };
        if let Some(base) = base {
            let norm: f64 = base
                .rotation
                .iter()
                .map(|value| f64::from(*value).powi(2))
                .sum();
            if base
                .translation
                .iter()
                .chain(&base.scale)
                .chain(&base.rotation)
                .any(|value| !value.is_finite() || value.abs() > 1.0e6)
                || (norm - 1.0).abs() > 0.001
            {
                return Err("Invalid saved effect base transform".into());
            }
        }
        if let Some(presentation) = &self.presentation {
            presentation.validate_outgoing(self)?;
        }
        Ok(())
    }
}

/// The host may resolve or rebase a projection without owning any authored
/// parameters or logical clock. Its variant must match the original effect.
#[derive(Clone, Debug, PartialEq)]
pub enum EffectProjectionBase {
    SpriteTransform(TransformSnapshot),
    VisualTransform(TransformSnapshot),
    AudioResolved,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EffectCompletion {
    pub task: ResourceTask,
    /// Reserved for the distinct retired-presentation model. Current Game
    /// targets are never removed by an effect's terminal clock.
    pub retired_target: Option<TargetIdentity>,
}

fn same_authored_effect(left: &SavedEffect, right: &SavedEffect) -> bool {
    if !match (&left.presentation, &right.presentation) {
        (None, None) => true,
        (Some(left), Some(right)) => left.bit_exact(right),
        _ => false,
    } {
        return false;
    }
    if left.clock.duration.to_bits() != right.clock.duration.to_bits()
        || left.clock.looping != right.clock.looping
    {
        return false;
    }
    match (&left.parameters, &right.parameters) {
        (
            EffectParameters::SpriteAnimation {
                animation: left, ..
            },
            EffectParameters::SpriteAnimation {
                animation: right, ..
            },
        ) => match (left, right) {
            (
                SpriteEffectKind::Shake { intensity: a },
                SpriteEffectKind::Shake { intensity: b },
            )
            | (SpriteEffectKind::Bounce { height: a }, SpriteEffectKind::Bounce { height: b })
            | (SpriteEffectKind::Pulse { scale: a }, SpriteEffectKind::Pulse { scale: b }) => {
                a.to_bits() == b.to_bits()
            }
            _ => false,
        },
        (
            EffectParameters::VisualFade {
                from: a,
                to: b,
                despawn_on_finish: c,
                transition: d,
                layered: e,
                ..
            },
            EffectParameters::VisualFade {
                from: f,
                to: g,
                despawn_on_finish: h,
                transition: i,
                layered: j,
                ..
            },
        ) => a.to_bits() == f.to_bits() && b.to_bits() == g.to_bits() && c == h && d == i && e == j,
        (
            EffectParameters::AudioFade {
                from: a,
                to: b,
                despawn_on_done: c,
            },
            EffectParameters::AudioFade {
                from: d,
                to: e,
                despawn_on_done: f,
            },
        ) => a.to_bits() == d.to_bits() && b.to_bits() == e.to_bits() && c == f,
        _ => false,
    }
}
fn valid_unit(value: f32) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceTask {
    pub key: TaskKey,
    pub revision: u64,
    pub target: TargetIdentity,
    #[serde(default)]
    pub dependencies: Vec<TargetIdentity>,
    pub kind: ResourceTaskKind,
    pub parked: bool,
    pub effect: Option<SavedEffect>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio: Option<crate::audio_playback::AudioPlayback>,
    pub wait: Option<ResumePosition>,
}

impl ResourceTask {
    pub fn alias(&self) -> &str {
        self.key
            .local_id
            .strip_prefix(task_prefix(self.kind))
            .unwrap_or(&self.key.local_id)
    }
}
fn task_prefix(kind: ResourceTaskKind) -> &'static str {
    match kind {
        ResourceTaskKind::Motion => "motion:",
        ResourceTaskKind::Video => "video:",
        ResourceTaskKind::Interface => "ui:",
        ResourceTaskKind::Canvas => "canvas:",
        ResourceTaskKind::Effect => "effect:",
        ResourceTaskKind::Sfx => "sfx:",
        ResourceTaskKind::Voice => "voice:",
    }
}
pub fn resource_key(owner: TaskOwner, kind: ResourceTaskKind, alias: &str) -> TaskKey {
    TaskKey {
        owner,
        local_id: format!("{}{alias}", task_prefix(kind)),
    }
}

/// Vecs make the saved wire format explicit: structured keys are not JSON keys.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceRegistry {
    pub next_incarnation: u64,
    pub next_revision: u64,
    pub targets: Vec<TargetRecord>,
    pub tasks: Vec<ResourceTask>,
    pub cancelled_waits: Vec<CancelledWait>,
    #[serde(default)]
    pub waits: Vec<WaitLease>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub music_checkpoints: Vec<crate::audio_checkpoint::MusicCheckpoint>,
}

impl ResourceRegistry {
    /// Existing registry used as a lifetime-only subset by native Legacy UI.
    /// This creates no SceneActivation, clock, wait, playback or projection task.
    pub(crate) fn validate_interface_registry(&self,ui:&crate::ui::UiState)->Result<(),String>{
        if self.targets.len()!=ui.screens.len()||self.tasks.len()!=ui.screens.len()||self.targets.len()>MAX_RESOURCES||!self.cancelled_waits.is_empty()||!self.waits.is_empty()||!self.music_checkpoints.is_empty(){return Err("Native interface lifetime registry has invalid cardinality or non-UI tasks".into());}
        let mut incarnations=HashSet::new();let mut names=HashSet::new();let mut keys=HashSet::new();
        for record in &self.targets{
            validate_owner(&record.owner)?;validate_alias(record.identity.kind,&record.identity.alias)?;
            if matches!(record.owner,TaskOwner::Scene{..})||record.identity.kind!=TargetKind::Interface||record.identity.incarnation==0||record.identity.incarnation>self.next_incarnation||!incarnations.insert(record.identity.incarnation)||!names.insert(record.identity.alias.clone()){return Err("Invalid native interface incarnation/owner".into());}
            crate::ui_residency::resident_interface_parts(ui,self,None,&[],&record.identity)?;
        }
        for task in &self.tasks{
            validate_key(&task.key)?;
            if task.kind!=ResourceTaskKind::Interface||task.key!=resource_key(task.key.owner.clone(),ResourceTaskKind::Interface,&task.target.alias)||task.revision==0||task.revision>self.next_revision||!keys.insert(task.key.clone())||task.parked||!task.dependencies.is_empty()||task.wait.is_some()||task.effect.is_some()||task.audio.is_some()||!self.targets.iter().any(|record|record.identity==task.target&&record.owner==task.key.owner){return Err("Invalid native interface task/revision".into());}
        }
        Ok(())
    }
    fn prune_music_checkpoints(&mut self) {
        self.music_checkpoints.retain(|row|self.targets.iter().any(|record|record.identity==row.target));
    }
    pub fn target(
        &self,
        owner: &TaskOwner,
        kind: TargetKind,
        alias: &str,
    ) -> Option<&TargetIdentity> {
        self.targets
            .iter()
            .find(|record| {
                record.owner == *owner
                    && record.identity.kind == kind
                    && record.identity.alias == alias
            })
            .map(|record| &record.identity)
    }
    pub fn ensure_target(
        &mut self,
        owner: TaskOwner,
        kind: TargetKind,
        alias: &str,
    ) -> Result<TargetIdentity, String> {
        validate_owner(&owner)?;
        validate_alias(kind, alias)?;
        if let Some(target) = self.target(&owner, kind, alias) {
            return Ok(target.clone());
        }
        if self.targets.len() >= MAX_RESOURCES {
            return Err("Resource identity limit: 4096".into());
        }
        let incarnation = self
            .next_incarnation
            .checked_add(1)
            .ok_or("Resource incarnation exhausted")?;
        let target = TargetIdentity {
            kind,
            alias: alias.into(),
            incarnation,
        };
        self.next_incarnation = incarnation;
        self.targets.push(TargetRecord {
            owner,
            identity: target.clone(),
        });
        Ok(target)
    }
    fn revision(&mut self) -> Result<u64, String> {
        self.next_revision = self
            .next_revision
            .checked_add(1)
            .ok_or("Resource task revision exhausted")?;
        Ok(self.next_revision)
    }
    /// Revoke callbacks without replacing the operation, its clock or its waiters.
    /// The caller decides whether this exact live or parked owner may be renewed.
    pub fn renew_task(&mut self, key: &TaskKey) -> Result<bool, String> {
        let Some(index) = self.tasks.iter().position(|task| task.key == *key) else {
            return Ok(false);
        };
        let revision = self
            .next_revision
            .checked_add(1)
            .ok_or("Resource task revision exhausted")?;
        let old_revision = self.tasks[index].revision;
        self.next_revision = revision;
        self.tasks[index].revision = revision;
        for wait in self
            .waits
            .iter_mut()
            .filter(|wait| wait.key == *key && wait.revision == old_revision)
        {
            wait.revision = revision;
        }
        Ok(true)
    }
    pub fn register(
        &mut self,
        key: TaskKey,
        target: TargetIdentity,
        kind: ResourceTaskKind,
        wait: Option<ResumePosition>,
    ) -> Result<u64, String> {
        validate_key(&key)?;
        if !self.targets.iter().any(|record| record.identity == target) {
            return Err("Task target is not a live incarnation".into());
        }
        validate_kind(kind, target.kind)?;
        if let Some(task) = self.tasks.iter_mut().find(|task| task.key == key) {
            if task.target != target || task.kind != kind {
                return Err("An operation must be superseded before changing its target".into());
            }
            if task.parked {
                return Err("A parked operation cannot be adopted by a live alias".into());
            }
            task.wait = wait;
            return Ok(task.revision);
        }
        if self.tasks.len() >= MAX_RESOURCES {
            return Err("Resource task limit: 4096".into());
        }
        let revision = self.revision()?;
        self.tasks.push(ResourceTask {
            key,
            revision,
            target,
            dependencies: Vec::new(),
            kind,
            parked: false,
            effect: None,
            audio: None,
            wait,
        });
        Ok(revision)
    }
    pub fn effect_token(
        &self,
        runtime_epoch: u64,
        key: &TaskKey,
        active: Option<ActivationId>,
    ) -> Option<ResourceToken> {
        let task = self
            .tasks
            .iter()
            .find(|task| task.key == *key && !task.parked && eligible(&task.key.owner, active))?;
        Some(ResourceToken {
            task: TaskToken {
                runtime_epoch,
                key: key.clone(),
                revision: task.revision,
            },
            target: task.target.clone(),
        })
    }
    pub fn accepts(
        &self,
        token: &ResourceToken,
        runtime_epoch: u64,
        active: Option<ActivationId>,
    ) -> bool {
        token.task.runtime_epoch == runtime_epoch
            && self.tasks.iter().any(|task| {
                !task.parked
                    && eligible(&task.key.owner, active)
                    && task.key == token.task.key
                    && task.revision == token.task.revision
                    && task.target == token.target
                    && self
                        .targets
                        .iter()
                        .any(|record| record.identity == task.target)
            })
    }
    pub fn task(
        &self,
        kind: ResourceTaskKind,
        alias: &str,
        active: Option<ActivationId>,
    ) -> Option<&ResourceTask> {
        self.tasks.iter().find(|task| {
            task.kind == kind
                && task.alias() == alias
                && !task.parked
                && eligible(&task.key.owner, active)
        })
    }
    pub fn complete_effect(
        &mut self,
        token: &ResourceToken,
        epoch: u64,
        active: Option<ActivationId>,
    ) -> bool {
        if !self.accepts(token, epoch, active)
            || !self.tasks.iter().any(|task| {
                task.key == token.task.key
                    && task.kind == ResourceTaskKind::Effect
                    && task
                        .effect
                        .as_ref()
                        .is_some_and(|effect| effect.validate().is_ok() && effect.clock.is_due())
            })
        {
            return false;
        }
        self.tasks.retain(|task| task.key != token.task.key);
        self.prune_unreferenced_retired_targets();
        true
    }
    pub fn active_effects(&self, active: Option<ActivationId>) -> Vec<&ResourceTask> {
        self.tasks
            .iter()
            .filter(|task| {
                !task.parked
                    && eligible(&task.key.owner, active)
                    && task.kind == ResourceTaskKind::Effect
            })
            .collect()
    }
    pub fn capture_effect(
        &mut self,
        token: &ResourceToken,
        runtime_epoch: u64,
        active: Option<ActivationId>,
        effect: SavedEffect,
    ) -> Result<bool, String> {
        if !self.accepts(token, runtime_epoch, active) {
            return Ok(false);
        }
        effect.validate()?;
        let task = self
            .tasks
            .iter()
            .find(|task| task.key == token.task.key)
            .unwrap();
        if task.kind != ResourceTaskKind::Effect {
            return Err("A non-effect task cannot accept an ECS effect snapshot".into());
        }
        let current = task
            .effect
            .as_ref()
            .ok_or("An effect task has no descriptor")?;
        if !same_authored_effect(current, &effect) {
            return Err("A projection cannot change authored effect parameters or duration".into());
        }
        if current.clock.elapsed.to_bits() != effect.clock.elapsed.to_bits() {
            return Err("A projection cannot change the Core effect clock".into());
        }
        if current.base_resolved && !effect.base_resolved {
            return Err("A resolved effect base cannot become unresolved".into());
        }
        if !effect.base_resolved {
            let same_transform = |left: &TransformSnapshot, right: &TransformSnapshot| {
                left.translation
                    .iter()
                    .chain(&left.scale)
                    .chain(&left.rotation)
                    .zip(
                        right
                            .translation
                            .iter()
                            .chain(&right.scale)
                            .chain(&right.rotation),
                    )
                    .all(|(left, right)| left.to_bits() == right.to_bits())
            };
            let same_base = match (&current.parameters, &effect.parameters) {
                (
                    EffectParameters::SpriteAnimation { base: left, .. },
                    EffectParameters::SpriteAnimation { base: right, .. },
                )
                | (
                    EffectParameters::VisualFade { base: left, .. },
                    EffectParameters::VisualFade { base: right, .. },
                ) => same_transform(left, right),
                (EffectParameters::AudioFade { .. }, EffectParameters::AudioFade { .. }) => true,
                _ => false,
            };
            if !same_base {
                return Err("An unmeasured effect snapshot cannot change its base".into());
            }
            return Ok(false);
        }
        let base = match effect.parameters {
            EffectParameters::SpriteAnimation { base, .. } => {
                EffectProjectionBase::SpriteTransform(base)
            }
            EffectParameters::VisualFade { base, .. } => {
                EffectProjectionBase::VisualTransform(base)
            }
            EffectParameters::AudioFade { .. } => EffectProjectionBase::AudioResolved,
        };
        self.capture_effect_base(token, runtime_epoch, active, base)
    }

    pub fn capture_effect_base(
        &mut self,
        token: &ResourceToken,
        runtime_epoch: u64,
        active: Option<ActivationId>,
        base: EffectProjectionBase,
    ) -> Result<bool, String> {
        if !self.accepts(token, runtime_epoch, active) {
            return Ok(false);
        }
        let index = self
            .tasks
            .iter()
            .position(|task| task.key == token.task.key)
            .unwrap();
        let task = &self.tasks[index];
        if task.kind != ResourceTaskKind::Effect {
            return Err("A non-effect task cannot accept a projection base".into());
        }
        let mut effect = task
            .effect
            .clone()
            .ok_or("An effect task has no descriptor")?;
        match (&mut effect.parameters, base) {
            (
                EffectParameters::SpriteAnimation { base, .. },
                EffectProjectionBase::SpriteTransform(next),
            )
            | (
                EffectParameters::VisualFade { base, .. },
                EffectProjectionBase::VisualTransform(next),
            ) => {
                *base = next;
            }
            (EffectParameters::AudioFade { .. }, EffectProjectionBase::AudioResolved) => {}
            _ => {
                return Err("Projection base does not belong to the authored effect family".into())
            }
        }
        effect.base_resolved = true;
        effect.validate()?;
        validate_effect_target(&effect, task.target.kind)?;
        self.tasks[index].effect = Some(effect);
        Ok(true)
    }
    fn cancel_where(
        &mut self,
        predicate: impl Fn(&ResourceTask) -> bool,
        reason: CancellationReason,
    ) -> Result<Vec<CancelledWait>, String> {
        let removed: Vec<_> = self
            .tasks
            .iter()
            .filter(|task| predicate(task))
            .map(|task| (task.key.clone(), task.revision))
            .collect();
        let cancelled: Vec<_> = self
            .waits
            .iter()
            .filter(|wait| removed.contains(&(wait.key.clone(), wait.revision)))
            .map(|wait| CancelledWait {
                owner: wait.owner,
                key: wait.key.clone(),
                revision: wait.revision,
                target: wait.target.clone(),
                position: wait.position.clone(),
                reason,
            })
            .collect();
        if self
            .cancelled_waits
            .len()
            .checked_add(cancelled.len())
            .is_none_or(|count| count > MAX_RESOURCES)
        {
            return Err("Cancelled wait limit: 4096".into());
        }
        self.tasks.retain(|task| !predicate(task));
        self.waits
            .retain(|wait| !removed.contains(&(wait.key.clone(), wait.revision)));
        self.cancelled_waits.extend(cancelled.iter().cloned());
        self.prune_unreferenced_retired_targets();
        Ok(cancelled)
    }
    pub fn supersede(&mut self, key: &TaskKey) -> Result<Vec<CancelledWait>, String> {
        self.cancel_where(
            |task| task.key == *key,
            CancellationReason::OperationSuperseded,
        )
    }
    pub fn remove_target(
        &mut self,
        owner: &TaskOwner,
        kind: TargetKind,
        alias: &str,
        reason: CancellationReason,
    ) -> Result<Vec<CancelledWait>, String> {
        let targets: Vec<_> = self
            .targets
            .iter()
            .filter(|record| {
                record.owner == *owner
                    && (record.identity.kind == kind && record.identity.alias == alias
                        || kind == TargetKind::Interface
                            && record.identity.kind == TargetKind::Canvas
                            && record.identity.alias.starts_with(&format!("{alias}/"))
                        || kind == TargetKind::Sprite
                            && record.identity.kind == TargetKind::Layer
                            && record.identity.alias.starts_with(&format!("{alias}/")))
            })
            .map(|record| record.identity.clone())
            .collect();
        let cancelled = self.cancel_where(
            |task| {
                targets.contains(&task.target)
                    || task
                        .dependencies
                        .iter()
                        .any(|dependency| targets.contains(dependency))
            },
            reason,
        )?;
        self.targets
            .retain(|record| !targets.contains(&record.identity));
        self.prune_music_checkpoints();
        Ok(cancelled)
    }
    pub fn replace_target(
        &mut self,
        owner: TaskOwner,
        kind: TargetKind,
        alias: &str,
    ) -> Result<TargetIdentity, String> {
        // Validate allocation before invalidating the old object.
        validate_owner(&owner)?;
        validate_alias(kind, alias)?;
        self.next_incarnation
            .checked_add(1)
            .ok_or("Resource incarnation exhausted")?;
        self.remove_target(&owner, kind, alias, CancellationReason::TargetReplaced)?;
        self.ensure_target(owner, kind, alias)
    }
    fn park_owner(&mut self, owner: &TaskOwner, parked: bool) -> Result<(), String> {
        let count = self
            .tasks
            .iter()
            .filter(|task| task.key.owner == *owner)
            .count();
        self.next_revision
            .checked_add(count as u64)
            .ok_or("Resource task revision exhausted")?;
        for task in self
            .tasks
            .iter_mut()
            .filter(|task| task.key.owner == *owner)
        {
            self.next_revision += 1;
            let old_revision = task.revision;
            task.revision = self.next_revision;
            for wait in self
                .waits
                .iter_mut()
                .filter(|wait| wait.key == task.key && wait.revision == old_revision)
            {
                wait.revision = task.revision;
            }
            task.parked = parked;
        }
        Ok(())
    }
    fn drain_waits(&mut self, owner: &TaskOwner) -> Vec<CancelledWait> {
        let context = match owner {
            TaskOwner::Scene { activation } => Some(*activation),
            _ => None,
        };
        let drained: Vec<_> = self
            .cancelled_waits
            .iter()
            .filter(|wait| wait.owner == context)
            .cloned()
            .collect();
        self.cancelled_waits.retain(|wait| wait.owner != context);
        drained
    }
    fn bind_wait(
        &mut self,
        key: &TaskKey,
        owner: Option<ActivationId>,
        position: ResumePosition,
    ) -> Result<(), String> {
        let task = self
            .tasks
            .iter()
            .find(|task| task.key == *key)
            .ok_or("A wait has no operation")?;
        if let Some(wait) = self
            .waits
            .iter_mut()
            .find(|wait| wait.key == *key && wait.owner == owner)
        {
            if wait.parked {
                return Err("A parked wait cannot be rebound by an active caller".into());
            }
            wait.position = position;
            wait.revision = task.revision;
            wait.target = task.target.clone();
            return Ok(());
        }
        if self.waits.len() >= MAX_RESOURCES {
            return Err("Wait lease limit: 4096".into());
        }
        self.waits.push(WaitLease {
            owner,
            key: key.clone(),
            revision: task.revision,
            target: task.target.clone(),
            position,
            parked: false,
        });
        Ok(())
    }
    fn park_wait_owner(&mut self, owner: Option<ActivationId>, parked: bool) {
        for wait in self.waits.iter_mut().filter(|wait| wait.owner == owner) {
            wait.parked = parked;
        }
    }
    /// Historical payloads do not keep their original Game incarnation alive.
    /// Only full current target/dependency references retain an outgoing object.
    fn prune_unreferenced_retired_targets(&mut self) -> Vec<TargetIdentity> {
        let removed: Vec<_> = self
            .targets
            .iter()
            .filter(|record| {
                record.identity.kind == TargetKind::PresentationRetired
                    && !self.tasks.iter().any(|task| {
                        task.target == record.identity
                            || task.dependencies.contains(&record.identity)
                    })
            })
            .map(|record| record.identity.clone())
            .collect();
        self.targets
            .retain(|record| !removed.contains(&record.identity));
        self.prune_music_checkpoints();
        removed
    }
    pub fn story_only(&self) -> Self {
        let mut registry = self.clone();
        let host_targets: Vec<_> = registry
            .targets
            .iter()
            .filter(|record| matches!(record.owner, TaskOwner::Host { .. }))
            .map(|record| record.identity.clone())
            .collect();
        registry.tasks.retain(|task| {
            !host_targets.contains(&task.target)
                && !task
                    .dependencies
                    .iter()
                    .any(|target| host_targets.contains(target))
        });
        registry
            .waits
            .retain(|wait| registry.tasks.iter().any(|task| task.key == wait.key));
        registry
            .tasks
            .retain(|task| !matches!(task.key.owner, TaskOwner::Host { .. }));
        registry
            .targets
            .retain(|record| !matches!(record.owner, TaskOwner::Host { .. }));
        // A cancellation receipt is continuation data, not a Host object. Keep
        // it until its exact narrative caller consumes it after Return/load.
        registry
            .waits
            .retain(|wait| registry.tasks.iter().any(|task| task.key == wait.key));
        registry.prune_unreferenced_retired_targets();
        registry
    }
    pub fn validate(&self,execution:&SceneExecution)->Result<(),String>{self.validate_with_context(Some(execution))}
    pub(crate) fn validate_game_audio(&self)->Result<(),String>{self.validate_with_context(None)}
    pub(crate) fn validate_game_resources(&self)->Result<(),String>{self.validate_with_context(None)}
    fn validate_with_context(&self, execution: Option<&SceneExecution>) -> Result<(), String> {
        crate::audio_checkpoint::validate_music_registry(self)?;
        if self.waits.len() > MAX_RESOURCES
            || self.targets.len() > MAX_RESOURCES
            || self.tasks.len() > MAX_RESOURCES
            || self.cancelled_waits.len() > MAX_RESOURCES
        {
            return Err("Resource registry exceeds 4096 entries".into());
        }
        let mut identities = HashSet::new();
        let mut addresses = HashSet::new();
        let mut keys = HashSet::new();
        for record in &self.targets {
            validate_owner(&record.owner)?;
            validate_alias(record.identity.kind, &record.identity.alias)?;
            validate_registry_owner(&record.owner,execution)?;
            if record.identity.incarnation == 0
                || record.identity.incarnation > self.next_incarnation
                || !identities.insert(record.identity.incarnation)
                || !addresses.insert((
                    record.owner.clone(),
                    record.identity.kind,
                    record.identity.alias.clone(),
                ))
            {
                return Err("Resource incarnation or address is invalid or duplicated".into());
            }
            if matches!(record.identity.kind, TargetKind::SfxPlayback | TargetKind::VoicePlayback) {
                let attached: Vec<_> = self.tasks.iter().filter(|task| task.target == record.identity).collect();
                if attached.len() != 1 || attached[0].key.owner != record.owner {
                    return Err("A playback target requires one exact owned reader".into());
                }
                crate::audio_playback::validate_task(attached[0])?;
            }
            if record.identity.kind == TargetKind::PresentationRetired {
                if record.identity.alias != format!("retired_{}", record.identity.incarnation) {
                    return Err("Retired address is not its allocated identity".into());
                }
                let attached: Vec<_> = self
                    .tasks
                    .iter()
                    .filter(|task| task.target == record.identity)
                    .collect();
                if attached.len() != 1
                    || attached[0].key.owner != record.owner
                    || attached[0].kind != ResourceTaskKind::Effect
                    || attached[0].key
                        != resource_key(
                            record.owner.clone(),
                            ResourceTaskKind::Effect,
                            &record.identity.alias,
                        )
                    || !attached[0].dependencies.is_empty()
                    || attached[0].wait.is_some()
                {
                    return Err("Retired target must have one exact owned outgoing effect".into());
                }
                attached[0]
                    .effect
                    .as_ref()
                    .and_then(|effect| effect.presentation.as_ref())
                    .ok_or("Retired target has no logical payload")?
                    .validate_for_target(&record.identity, self.next_incarnation)?;
            }
        }
        let mut live_addresses = HashSet::new();
        let mut motions = 0;
        let mut videos = 0;
        let mut screens = 0;
        let mut canvases = 0;
        let mut audios = 0;
        for task in &self.tasks {
            validate_key(&task.key)?;
            if !task.key.local_id.starts_with(task_prefix(task.kind)) {
                return Err("Resource task namespace is invalid".into());
            }
            validate_registry_owner(&task.key.owner,execution)?;
            validate_kind(task.kind, task.target.kind)?;
            if task.revision == 0
                || task.revision > self.next_revision
                || !keys.insert(task.key.clone())
                || !self
                    .targets
                    .iter()
                    .any(|record| record.identity == task.target)
            {
                return Err("Resource task key, revision or target is incoherent".into());
            }
            if matches!(
                task.kind,
                ResourceTaskKind::Video | ResourceTaskKind::Interface | ResourceTaskKind::Canvas
                    | ResourceTaskKind::Sfx | ResourceTaskKind::Voice
            ) && !self
                .targets
                .iter()
                .any(|record| record.identity == task.target && record.owner == task.key.owner)
            {
                return Err("Scoped instance ownership was retagged to another visit".into());
            }
            if !task.parked
                && eligible(&task.key.owner, execution.and_then(|e|e.owner()))
                && !live_addresses.insert((task.kind, task.alias().to_string()))
            {
                return Err("Two live operations share one descriptor address".into());
            }
            if task.parked != execution.is_some_and(|e|owner_parked(&task.key.owner,e)) {
                return Err("Resource task eligibility does not match its visit".into());
            }
            if task
                .dependencies
                .iter()
                .any(|dependency| dependency.kind == TargetKind::PresentationRetired)
            {
                return Err("Outgoing appearances cannot become live task dependencies".into());
            }
            if task.dependencies.len() > 32
                || task.dependencies.iter().any(|dependency| {
                    !self
                        .targets
                        .iter()
                        .any(|record| record.identity == *dependency)
                })
            {
                return Err("Resource task dependency incarnation is absent or excessive".into());
            }
            if let Some(wait) = &task.wait {
                if task.kind != ResourceTaskKind::Motion && task.kind != ResourceTaskKind::Video {
                    return Err("Only motion and video tasks can hold a narrative wait".into());
                }
                validate_wait_shape(wait)?;
            }
            if (task.kind == ResourceTaskKind::Effect) != task.effect.is_some() {
                return Err(
                    "Effect task descriptor is missing or belongs to another task kind".into(),
                );
            }
            if let Some(effect) = &task.effect {
                effect.validate()?;
                validate_effect_target(effect, task.target.kind)?;
            }
            crate::audio_playback::validate_task(task)?;
            match task.kind {
                ResourceTaskKind::Sfx | ResourceTaskKind::Voice => audios += 1,
                ResourceTaskKind::Motion | ResourceTaskKind::Effect => motions += 1,
                ResourceTaskKind::Video => videos += 1,
                ResourceTaskKind::Interface => screens += 1,
                ResourceTaskKind::Canvas => canvases += 1,
            }
        }
        if motions > MAX_MOTIONS
            || videos > MAX_VIDEOS
            || screens > MAX_INTERFACES
            || canvases > MAX_CANVASES
            || audios > crate::audio_playback::MAX_AUDIO_PLAYBACKS
        {
            return Err(
                "Aggregate task cap exceeded: 64 motions/effects, 8 videos, 32 screens or canvases"
                    .into(),
            );
        }
        let mut leases = HashSet::new();
        for wait in &self.waits {
            validate_wait_owner(wait.owner,execution.ok_or("Standalone Game audio cannot own waits")?)?;
            validate_wait_shape(&wait.position)?;
            let task = self
                .tasks
                .iter()
                .find(|task| {
                    task.key == wait.key
                        && task.revision == wait.revision
                        && task.target == wait.target
                })
                .ok_or("Wait lease refers to another or absent operation")?;
            if !matches!(
                task.kind,
                ResourceTaskKind::Motion | ResourceTaskKind::Video
            ) || !leases.insert((wait.owner, wait.key.clone()))
                || wait.parked != wait_owner_parked(wait.owner,execution.ok_or("Standalone Game audio cannot park waits")?)
            {
                return Err("Wait lease ownership or eligibility is invalid".into());
            }
        }
        let mut waits = HashSet::new();
        for wait in &self.cancelled_waits {
            validate_wait_owner(wait.owner,execution.ok_or("Standalone Game audio cannot own waits")?)?;
            validate_key(&wait.key)?;
            validate_registry_owner(&wait.key.owner,execution)?;
            validate_wait_shape(&wait.position)?;
            validate_alias(wait.target.kind, &wait.target.alias)?;
            if wait.target.incarnation == 0
                || wait.target.incarnation > self.next_incarnation
                || wait.revision == 0
                || wait.revision > self.next_revision
                || !waits.insert((wait.owner, wait.key.clone(), wait.revision))
            {
                return Err("Cancelled wait receipt is invalid or duplicated".into());
            }
        }
        Ok(())
    }
}

fn validate_registry_owner(owner:&TaskOwner,execution:Option<&SceneExecution>)->Result<(),String>{
 match execution{Some(e)=>validate_resident_owner(owner,e),None if *owner==TaskOwner::Game=>Ok(()),None=>Err("Standalone audio requires the actual Game lifetime".into())}
}

fn validate_wait_owner(
    owner: Option<ActivationId>,
    execution: &SceneExecution,
) -> Result<(), String> {
    if let Some(activation) = owner {
        return validate_resident_owner(&TaskOwner::Scene { activation }, execution);
    }
    if execution.owner().is_none()
        || execution
            .frames
            .iter()
            .any(|frame| frame.kind == crate::scene::FrameKind::SceneCall && frame.owner.is_none())
    {
        Ok(())
    } else {
        Err("A previsit wait has no live or suspended continuation".into())
    }
}
fn wait_owner_parked(owner: Option<ActivationId>, execution: &SceneExecution) -> bool {
    match owner {
        Some(activation) => owner_parked(&TaskOwner::Scene { activation }, execution),
        None => {
            execution.game_tasks.resources_parked
                || execution.frames.iter().any(|frame| {
                    frame.kind == crate::scene::FrameKind::SceneCall && frame.owner.is_none()
                })
        }
    }
}

fn validate_wait_shape(wait: &ResumePosition) -> Result<(), String> {
    // Exact authored/generated origin and PC membership is checked by PreparedNarrative.
    if wait
        .origin
        .as_ref()
        .is_some_and(|origin| origin.route.len() > 4096)
    {
        return Err("Resource wait provenance is too deep".into());
    }
    Ok(())
}
fn eligible(owner: &TaskOwner, active: Option<ActivationId>) -> bool {
    match owner {
        TaskOwner::Scene { activation } => Some(*activation) == active,
        _ => true,
    }
}
fn owner_parked(owner: &TaskOwner, execution: &SceneExecution) -> bool {
    let TaskOwner::Scene { activation } = owner else {
        return false;
    };
    execution
        .active
        .as_ref()
        .filter(|active| active.id == *activation)
        .map(|active| active.tasks.resources_parked)
        .unwrap_or_else(|| {
            execution.frames.iter().any(|frame| {
                frame
                    .parked
                    .as_ref()
                    .is_some_and(|parent| parent.id == *activation)
            })
        })
}
fn validate_resident_owner(owner: &TaskOwner, execution: &SceneExecution) -> Result<(), String> {
    if let TaskOwner::Scene { activation } = owner {
        if execution.owner() != Some(*activation)
            && !execution.frames.iter().any(|frame| {
                frame
                    .parked
                    .as_ref()
                    .is_some_and(|parent| parent.id == *activation)
            })
        {
            return Err("Resource task belongs to an absent visit".into());
        }
    }
    Ok(())
}
fn validate_owner(owner: &TaskOwner) -> Result<(), String> {
    match owner {
        TaskOwner::Scene { activation } if activation.0 == 0 => {
            Err("Invalid task activation identity".into())
        }
        TaskOwner::Host { role } if rvn_ui::PageRole::from_id(role).is_none() => {
            Err("Invalid host task role".into())
        }
        _ => Ok(()),
    }
}
fn validate_key(key: &TaskKey) -> Result<(), String> {
    validate_owner(&key.owner)?;
    if key.local_id.is_empty()
        || key.local_id.len() > 4096
        || key.local_id.chars().any(char::is_control)
    {
        return Err("Invalid task local identity".into());
    }
    Ok(())
}
fn validate_alias(kind: TargetKind, alias: &str) -> Result<(), String> {
    let valid = match kind {
        TargetKind::Background => alias == "background",
        TargetKind::Music => alias == "music",
        TargetKind::Layer | TargetKind::Canvas => {
            alias.split_once('/').is_some_and(|(first, second)| {
                rvn_ui::composition::valid_name(first) && rvn_ui::composition::valid_name(second)
            })
        }
        _ => rvn_ui::composition::valid_name(alias),
    };
    if valid {
        Ok(())
    } else {
        Err("Invalid logical resource alias".into())
    }
}
fn validate_kind(kind: ResourceTaskKind, target: TargetKind) -> Result<(), String> {
    let valid = match kind {
        ResourceTaskKind::Motion => matches!(
            target,
            TargetKind::Background | TargetKind::Sprite | TargetKind::Layer | TargetKind::Interface
        ),
        ResourceTaskKind::Video => target == TargetKind::Video,
        ResourceTaskKind::Sfx => target == TargetKind::SfxPlayback,
        ResourceTaskKind::Voice => target == TargetKind::VoicePlayback,
        ResourceTaskKind::Interface => target == TargetKind::Interface,
        ResourceTaskKind::Canvas => target == TargetKind::Canvas,
        ResourceTaskKind::Effect => matches!(
            target,
            TargetKind::Background
                | TargetKind::Sprite
                | TargetKind::Layer
                | TargetKind::Music
                | TargetKind::Cinematic
                | TargetKind::PresentationRetired
        ),
    };
    if valid {
        Ok(())
    } else {
        Err("Task kind and resource target disagree".into())
    }
}
fn validate_effect_target(effect: &SavedEffect, target: TargetKind) -> Result<(), String> {
    if target == TargetKind::PresentationRetired {
        return effect
            .presentation
            .as_ref()
            .ok_or("Outgoing target has no logical payload")?
            .validate_outgoing(effect);
    }
    if effect.presentation.is_some() {
        return Err("Current Game target cannot adopt a retired payload".into());
    }
    let valid = match &effect.parameters {
        EffectParameters::AudioFade { .. } => target == TargetKind::Music,
        EffectParameters::SpriteAnimation { .. } => {
            matches!(target, TargetKind::Sprite | TargetKind::Layer)
        }
        EffectParameters::VisualFade { layered, .. } => {
            if *layered {
                matches!(target, TargetKind::Sprite | TargetKind::Layer)
            } else {
                matches!(target, TargetKind::Background | TargetKind::Sprite | TargetKind::Cinematic)
            }
        }
    };
    if valid {
        Ok(())
    } else {
        Err("Effect parameters do not belong to this target kind".into())
    }
}

fn validate_snapshot_ownership(
    execution: &SceneExecution,
    owner: Option<ActivationId>,
    parked: bool,
    motions: &crate::motion::MotionState,
    videos: &crate::video::VideoState,
    ui: &crate::ui::UiState,
) -> Result<(), String> {
    let registry = &execution.resources;
    let find = |kind, alias: &str| {
        registry.tasks.iter().find(|task| {
            task.kind == kind
                && task.alias() == alias
                && task.parked == parked
                && if parked {
                    task.key.owner
                        == owner
                            .map(|activation| TaskOwner::Scene { activation })
                            .unwrap_or(TaskOwner::Game)
                } else {
                    eligible(&task.key.owner, owner)
                }
        })
    };
    for (alias, track) in &motions.tracks {
        let task = find(ResourceTaskKind::Motion, alias)
            .ok_or("Saved motion is not owned by its exact task")?;
        let (kind, expected) = match &track.target {
            crate::motion::MotionTarget::Background => {
                (TargetKind::Background, "background".to_string())
            }
            crate::motion::MotionTarget::Sprite { id } => (TargetKind::Sprite, id.clone()),
            crate::motion::MotionTarget::Layer { id, layer } => {
                (TargetKind::Layer, format!("{id}/{layer}"))
            }
            crate::motion::MotionTarget::Interface { screen, .. } => {
                (TargetKind::Interface, screen.clone())
            }
        };
        if task.target.kind != kind || task.target.alias != expected {
            return Err("Saved motion belongs to another object incarnation".into());
        }
    }
    for alias in videos.tracks.keys() {
        let task = find(ResourceTaskKind::Video, alias)
            .ok_or("Saved video is not owned by its exact player task")?;
        if task.target.alias != *alias {
            return Err("Saved player alias differs from its resource identity".into());
        }
    }
    for screen in &ui.screens {
        let task = find(ResourceTaskKind::Interface, &screen.name)
            .ok_or("Saved interface is not owned by its exact instance task")?;
        if task.target.alias != screen.name
            || screen
                .host_role
                .map(|role| TaskOwner::Host {
                    role: role.id().into(),
                })
                .is_some_and(|host| host != task.key.owner)
        {
            return Err("Saved interface ownership differs from its presenter".into());
        }
        for element in screen.canvas_states.keys() {
            let alias = format!("{}/{element}", screen.name);
            let canvas = find(ResourceTaskKind::Canvas, &alias)
                .ok_or("Saved canvas has no exact instance task")?;
            if canvas.key.owner != task.key.owner || canvas.target.alias != alias {
                return Err("Saved canvas belongs to another interface owner".into());
            }
        }
    }
    for (kind, alias) in [
        (ResourceTaskKind::Motion, motions.waiting.as_deref()),
        (ResourceTaskKind::Video, videos.waiting.as_deref()),
    ] {
        if let Some(alias) = alias {
            if !registry.waits.iter().any(|wait| {
                wait.owner == owner
                    && wait.parked == parked
                    && registry.tasks.iter().any(|task| {
                        task.key == wait.key
                            && task.revision == wait.revision
                            && task.kind == kind
                            && task.alias() == alias
                    })
            }) {
                return Err("Saved wait has lost its exact continuation lease".into());
            }
        }
    }
    Ok(())
}

pub(crate) fn validate_suspended_ownership(execution: &SceneExecution) -> Result<(), String> {
    if let Some(active) = &execution.active {
        validate_snapshot_ownership(
            execution,
            Some(active.id),
            true,
            &active.tasks.suspended_motions,
            &active.tasks.suspended_videos,
            &active.tasks.suspended_ui,
        )?;
    }
    for frame in &execution.frames {
        if let Some(parent) = &frame.parked {
            validate_snapshot_ownership(
                execution,
                Some(parent.id),
                true,
                &parent.tasks.suspended_motions,
                &parent.tasks.suspended_videos,
                &parent.tasks.suspended_ui,
            )?;
        }
        if let Some(previsit) = &frame.pre_scene_resources {
            if !previsit.suspended_motions.tracks.is_empty()
                || !previsit.suspended_videos.tracks.is_empty()
                || !previsit.suspended_ui.screens.is_empty()
                || previsit.timer.is_some()
            {
                return Err("Previsit continuation cannot park Game resources or its timer".into());
            }
            validate_snapshot_ownership(
                execution,
                None,
                true,
                &previsit.suspended_motions,
                &previsit.suspended_videos,
                &previsit.suspended_ui,
            )?;
        }
    }
    validate_snapshot_ownership(
        execution,
        None,
        true,
        &execution.game_tasks.suspended_motions,
        &execution.game_tasks.suspended_videos,
        &execution.game_tasks.suspended_ui,
    )
}

fn current_owner(execution: &SceneExecution) -> TaskOwner {
    execution
        .owner()
        .map(|activation| TaskOwner::Scene { activation })
        .unwrap_or(TaskOwner::Game)
}
fn transaction(
    state: &mut GameState,
    operation: impl FnOnce(&mut GameState) -> Result<ResourceTransition, String>,
) -> Result<ResourceTransition, String> {
    let mut candidate = state.clone();
    if let Some(execution) = &candidate.scene_execution {
        execution.resources.validate(execution)?;
        validate_suspended_ownership(execution)?;
    }
    let transition = operation(&mut candidate)?;
    validate_current_resource_state(&candidate)?;
    *state = candidate;
    Ok(transition)
}

/// Overlay only live descriptors. A parent homonym is never adopted by the child.
pub fn sync_current(state: &mut GameState) -> Result<ResourceTransition, String> {
    transaction(state, sync_in_place)
}

fn sync_in_place(state: &mut GameState) -> Result<ResourceTransition, String> {
    let Some(mut execution) = state.scene_execution.take() else {
        return Ok(ResourceTransition::default());
    };
    let owner = current_owner(&execution);
    let active_id = execution.owner();
    let registry = &mut execution.resources;
    let mut live_global = Vec::new();
    if !state.background_image.is_empty() {
        live_global.push((TargetKind::Background, "background".to_string()));
    }
    if state.music.current_file.is_some() {
        live_global.push((TargetKind::Music, "music".to_string()));
    }
    if state.cinematic.current.is_some() {
        live_global.push((TargetKind::Cinematic, "cinematic".to_string()));
    }
    for (alias, sprite) in &state.sprites {
        if sprite.visible {
            live_global.push((TargetKind::Sprite, alias.clone()));
        }
    }
    for (alias, composition) in &state.layered.characters {
        if state
            .sprites
            .get(alias)
            .is_some_and(|sprite| sprite.visible)
        {
            for layer in &composition.definition.layers {
                live_global.push((TargetKind::Layer, format!("{alias}/{}", layer.id)));
            }
        }
    }
    let removed: Vec<_> = registry
        .targets
        .iter()
        .filter(|target| {
            target.owner == TaskOwner::Game
                && matches!(
                    target.identity.kind,
                    TargetKind::Background
                        | TargetKind::Sprite
                        | TargetKind::Layer
                        | TargetKind::Music
                        | TargetKind::Cinematic
                )
                && !live_global.contains(&(target.identity.kind, target.identity.alias.clone()))
        })
        .map(|target| (target.identity.kind, target.identity.alias.clone()))
        .collect();
    for (kind, alias) in removed {
        registry.remove_target(
            &TaskOwner::Game,
            kind,
            &alias,
            CancellationReason::TargetRemoved,
        )?;
    }
    for (kind, alias) in live_global {
        registry.ensure_target(TaskOwner::Game, kind, &alias)?;
    }
    for screen in &state.ui.screens {
        let default = screen
            .host_role
            .map(|role| TaskOwner::Host {
                role: role.id().into(),
            })
            .unwrap_or_else(|| owner.clone());
        let screen_owner = live_task_owner(
            registry,
            ResourceTaskKind::Interface,
            &screen.name,
            &default,
            active_id,
        );
        let target =
            registry.ensure_target(screen_owner.clone(), TargetKind::Interface, &screen.name)?;
        registry.register(
            resource_key(
                screen_owner.clone(),
                ResourceTaskKind::Interface,
                &screen.name,
            ),
            target,
            ResourceTaskKind::Interface,
            None,
        )?;
        for element in screen.canvas_states.keys() {
            let alias = format!("{}/{element}", screen.name);
            let target =
                registry.ensure_target(screen_owner.clone(), TargetKind::Canvas, &alias)?;
            registry.register(
                resource_key(screen_owner.clone(), ResourceTaskKind::Canvas, &alias),
                target,
                ResourceTaskKind::Canvas,
                None,
            )?;
        }
    }
    for (alias, track) in &state.videos.tracks {
        let task_owner =
            live_task_owner(registry, ResourceTaskKind::Video, alias, &owner, active_id);
        let key = resource_key(task_owner.clone(), ResourceTaskKind::Video, alias);
        let existing = registry.tasks.iter().find(|task| task.key == key).cloned();
        let target = registry.ensure_target(task_owner.clone(), TargetKind::Video, alias)?;
        let wait =
            (state.videos.waiting.as_ref() == Some(alias)).then(|| execution.live_position.clone());
        registry.register(key.clone(), target, ResourceTaskKind::Video, wait)?;
        // Raw detached descriptors can be discovered once. Registered Engine
        // producers must explicitly bind their visible UI before final sync.
        if existing.is_none() {
            if let Some(target) = &track.clip.target {
                let (screen, _) = target
                    .strip_prefix("ui:")
                    .and_then(|text| text.split_once('/'))
                    .ok_or("Invalid embedded video target")?;
                let identity = crate::ui_residency::visible_interface_target_parts(
                    &state.ui,
                    registry,
                    execution.active.as_ref(),
                    &execution.frames,
                    screen,
                )?;
                registry
                    .tasks
                    .iter_mut()
                    .find(|task| task.key == key)
                    .unwrap()
                    .dependencies = vec![identity];
            }
        }
        let task = registry.tasks.iter().find(|task| task.key == key).unwrap();
        crate::ui_residency::video_binding_parts(
            &state.ui,
            registry,
            execution.active.as_ref(),
            &execution.frames,
            task,
            track,
        )?;
        if state.videos.waiting.as_ref() == Some(alias) {
            registry.bind_wait(
                &resource_key(task_owner, ResourceTaskKind::Video, alias),
                active_id,
                execution.live_position.clone(),
            )?;
        }
        track.validate()?;
    }
    for (alias, track) in &state.motions.tracks {
        let task_owner =
            live_task_owner(registry, ResourceTaskKind::Motion, alias, &owner, active_id);
        let key = resource_key(task_owner.clone(), ResourceTaskKind::Motion, alias);
        let existing = registry.tasks.iter().find(|task| task.key == key).cloned();
        let target = match &track.target {
            crate::motion::MotionTarget::Interface { screen, .. } => {
                if let Some(task) = &existing {
                    // Captured incarnation is authoritative; never resolve a
                    // Game operation against a child alias at this point.
                    crate::ui_residency::motion_binding_parts(
                        &state.ui,
                        registry,
                        execution.active.as_ref(),
                        &execution.frames,
                        task,
                        track,
                    )?;
                    task.target.clone()
                } else {
                    crate::ui_residency::visible_interface_target_parts(
                        &state.ui,
                        registry,
                        execution.active.as_ref(),
                        &execution.frames,
                        screen,
                    )?
                }
            }
            _ => {
                let (kind, name) = match &track.target {
                    crate::motion::MotionTarget::Background => {
                        (TargetKind::Background, "background".to_owned())
                    }
                    crate::motion::MotionTarget::Sprite { id } => (TargetKind::Sprite, id.clone()),
                    crate::motion::MotionTarget::Layer { id, layer } => {
                        (TargetKind::Layer, format!("{id}/{layer}"))
                    }
                    crate::motion::MotionTarget::Interface { .. } => unreachable!(),
                };
                registry
                    .target(&TaskOwner::Game, kind, &name)
                    .cloned()
                    .ok_or("Motion addresses an absent logical object")?
            }
        };
        let wait = (state.motions.waiting.as_ref() == Some(alias))
            .then(|| execution.live_position.clone());
        registry.register(key, target, ResourceTaskKind::Motion, wait)?;
    }
    if let Some(alias) = &state.motions.waiting {
        let task_owner =
            live_task_owner(registry, ResourceTaskKind::Motion, alias, &owner, active_id);
        registry.bind_wait(
            &resource_key(task_owner, ResourceTaskKind::Motion, alias),
            active_id,
            execution.live_position.clone(),
        )?;
    }
    registry.waits.retain(|wait| {
        wait.parked
            || wait.owner != active_id
            || match registry
                .tasks
                .iter()
                .find(|task| task.key == wait.key)
                .map(|task| task.kind)
            {
                Some(ResourceTaskKind::Motion) => {
                    state.motions.waiting.as_deref()
                        == Some(wait.key.local_id.strip_prefix("motion:").unwrap_or(""))
                }
                Some(ResourceTaskKind::Video) => {
                    state.videos.waiting.as_deref()
                        == Some(wait.key.local_id.strip_prefix("video:").unwrap_or(""))
                }
                _ => false,
            }
    });
    // Retired UI/video/canvas operations are explicit removals, not reusable aliases.
    let obsolete: Vec<_> = registry
        .tasks
        .iter()
        .filter(|task| {
            !task.parked
                && eligible(&task.key.owner, active_id)
                && match task.kind {
                    ResourceTaskKind::Motion => !state.motions.tracks.contains_key(task.alias()),
                    ResourceTaskKind::Video => !state.videos.tracks.contains_key(task.alias()),
                    ResourceTaskKind::Interface => !state
                        .ui
                        .screens
                        .iter()
                        .any(|screen| screen.name == task.alias()),
                    ResourceTaskKind::Canvas => !state.ui.screens.iter().any(|screen| {
                        screen
                            .canvas_states
                            .keys()
                            .any(|element| format!("{}/{element}", screen.name) == task.alias())
                    }),
                    ResourceTaskKind::Effect | ResourceTaskKind::Sfx | ResourceTaskKind::Voice => false,
                }
        })
        .map(|task| task.key.clone())
        .collect();
    for key in obsolete {
        registry.cancel_where(|task| task.key == key, CancellationReason::TargetRemoved)?;
    }
    prune_suspended(&mut execution);
    let cancelled = execution.resources.drain_waits(&owner);
    state.scene_execution = Some(execution);
    Ok(ResourceTransition {
        cancelled_waits: cancelled,
    })
}
fn live_task_owner(
    registry: &ResourceRegistry,
    kind: ResourceTaskKind,
    alias: &str,
    default: &TaskOwner,
    active: Option<ActivationId>,
) -> TaskOwner {
    registry
        .tasks
        .iter()
        .find(|task| {
            task.kind == kind
                && task.alias() == alias
                && !task.parked
                && eligible(&task.key.owner, active)
        })
        .map(|task| task.key.owner.clone())
        .unwrap_or_else(|| default.clone())
}

/// Called before call_scene clones its parent. Game and Host remain live.
pub fn park_current(state: &mut GameState) -> Result<ResourceTransition, String> {
    transaction(state, |candidate| {
        sync_in_place(candidate)?;
        let Some(execution) = &mut candidate.scene_execution else {
            return Ok(ResourceTransition::default());
        };
        let Some(active) = &mut execution.active else {
            if execution.game_tasks.resources_parked {
                return Err("This previsit continuation is already parked".into());
            }
            execution.game_tasks.suspended_motions.waiting = candidate.motions.waiting.take();
            execution.game_tasks.suspended_videos.waiting = candidate.videos.waiting.take();
            execution.game_tasks.resources_parked = true;
            execution.resources.park_wait_owner(None, true);
            return Ok(ResourceTransition::default());
        };
        if active.tasks.resources_parked {
            return Err("This visit is already parked".into());
        }
        let owner = TaskOwner::Scene {
            activation: active.id,
        };
        let owned =
            |kind, alias: &str| {
                execution.resources.tasks.iter().any(|task| {
                    task.key.owner == owner && task.kind == kind && task.alias() == alias
                })
            };
        let motion_keys: Vec<_> = candidate
            .motions
            .tracks
            .keys()
            .filter(|key| owned(ResourceTaskKind::Motion, key))
            .cloned()
            .collect();
        for key in motion_keys {
            active
                .tasks
                .suspended_motions
                .tracks
                .insert(key.clone(), candidate.motions.tracks.remove(&key).unwrap());
        }
        if candidate.motions.waiting.as_ref().is_some_and(|key| {
            execution.resources.waits.iter().any(|wait| {
                wait.owner == Some(active.id) && wait.key.local_id == format!("motion:{key}")
            })
        }) {
            active.tasks.suspended_motions.waiting = candidate.motions.waiting.take();
        }
        let video_keys: Vec<_> = candidate
            .videos
            .tracks
            .keys()
            .filter(|key| owned(ResourceTaskKind::Video, key))
            .cloned()
            .collect();
        for key in video_keys {
            active
                .tasks
                .suspended_videos
                .tracks
                .insert(key.clone(), candidate.videos.tracks.remove(&key).unwrap());
        }
        if candidate.videos.waiting.as_ref().is_some_and(|key| {
            execution.resources.waits.iter().any(|wait| {
                wait.owner == Some(active.id) && wait.key.local_id == format!("video:{key}")
            })
        }) {
            active.tasks.suspended_videos.waiting = candidate.videos.waiting.take();
        }
        let names: Vec<_> = candidate
            .ui
            .screens
            .iter()
            .filter(|screen| owned(ResourceTaskKind::Interface, &screen.name))
            .map(|screen| screen.name.clone())
            .collect();
        active.tasks.suspended_ui.screens = candidate
            .ui
            .screens
            .iter()
            .filter(|screen| names.contains(&screen.name))
            .cloned()
            .collect();
        active.tasks.suspended_ui.next_order = candidate.ui.next_order;
        candidate
            .ui
            .screens
            .retain(|screen| !names.contains(&screen.name));
        active.tasks.resources_parked = true;
        execution.resources.park_owner(&owner, true)?;
        execution.resources.park_wait_owner(Some(active.id), true);
        Ok(ResourceTransition::default())
    })
}

pub fn abandon_current(state: &mut GameState) -> Result<ResourceTransition, String> {
    transaction(state, |candidate| {
        sync_in_place(candidate)?;
        let Some(mut execution) = candidate.scene_execution.take() else {
            return Ok(ResourceTransition::default());
        };
        let Some(id) = execution.owner() else {
            let mut cancelled = Vec::new();
            for wait in execution
                .resources
                .waits
                .iter()
                .filter(|wait| wait.owner.is_none())
            {
                cancelled.push(CancelledWait {
                    owner: None,
                    key: wait.key.clone(),
                    revision: wait.revision,
                    target: wait.target.clone(),
                    position: wait.position.clone(),
                    reason: CancellationReason::OwnerAbandoned,
                });
                match wait.key.local_id.split_once(':') {
                    Some(("motion", alias))
                        if candidate.motions.waiting.as_deref() == Some(alias) =>
                    {
                        candidate.motions.waiting = None
                    }
                    Some(("video", alias))
                        if candidate.videos.waiting.as_deref() == Some(alias) =>
                    {
                        candidate.videos.waiting = None
                    }
                    _ => {}
                }
            }
            execution
                .resources
                .waits
                .retain(|wait| wait.owner.is_some());
            execution.resources.drain_waits(&TaskOwner::Game);
            execution.game_tasks.suspended_motions = Default::default();
            execution.game_tasks.suspended_videos = Default::default();
            execution.game_tasks.suspended_ui = Default::default();
            execution.game_tasks.resources_parked = false;
            candidate.scene_execution = Some(execution);
            return Ok(ResourceTransition {
                cancelled_waits: cancelled,
            });
        };
        let owner = TaskOwner::Scene { activation: id };
        let removed_targets: HashSet<_> = execution
            .resources
            .targets
            .iter()
            .filter(|record| record.owner == owner)
            .map(|record| record.identity.clone())
            .collect();
        let affected = |task: &ResourceTask| {
            task.key.owner == owner
                || removed_targets.contains(&task.target)
                || task
                    .dependencies
                    .iter()
                    .any(|target| removed_targets.contains(target))
        };
        let tasks: Vec<_> = execution
            .resources
            .tasks
            .iter()
            .filter(|task| affected(task))
            .cloned()
            .collect();
        for task in &tasks {
            remove_live_task(candidate, task, execution.owner());
        }
        let mut cancelled = execution
            .resources
            .cancel_where(|task| affected(task), CancellationReason::OwnerAbandoned)?;
        // Scoped player instances whose operations were cancelled do not
        // leave a reusable incarnation for a later player of the same alias.
        let retired_players: HashSet<_> = tasks
            .iter()
            .filter(|task| task.kind == ResourceTaskKind::Video)
            .map(|task| task.target.clone())
            .collect();
        execution.resources.targets.retain(|record| {
            record.owner != owner
                && (!retired_players.contains(&record.identity)
                    || execution.resources.tasks.iter().any(|task| {
                        task.target == record.identity
                            || task.dependencies.contains(&record.identity)
                    }))
        });
        execution.resources.prune_music_checkpoints();
        prune_suspended(&mut execution);
        for wait in execution
            .resources
            .waits
            .iter()
            .filter(|wait| wait.owner == Some(id))
        {
            cancelled.push(CancelledWait {
                owner: wait.owner,
                key: wait.key.clone(),
                revision: wait.revision,
                target: wait.target.clone(),
                position: wait.position.clone(),
                reason: CancellationReason::OwnerAbandoned,
            });
            match wait.key.local_id.split_once(':') {
                Some(("motion", alias)) if candidate.motions.waiting.as_deref() == Some(alias) => {
                    candidate.motions.waiting = None
                }
                Some(("video", alias)) if candidate.videos.waiting.as_deref() == Some(alias) => {
                    candidate.videos.waiting = None
                }
                _ => {}
            }
        }
        execution
            .resources
            .waits
            .retain(|wait| wait.owner != Some(id));
        // No continuation of an abandoned owner can subsequently receive these receipts.
        execution.resources.drain_waits(&owner);
        if let Some(active) = &mut execution.active {
            active.tasks.suspended_motions = Default::default();
            active.tasks.suspended_videos = Default::default();
            active.tasks.suspended_ui = Default::default();
            active.tasks.timer = None;
            active.tasks.resources_parked = false;
        }
        candidate.scene_execution = Some(execution);
        Ok(ResourceTransition {
            cancelled_waits: cancelled,
        })
    })
}

/// Called only after a real SceneCall frame returned, never for a helper.
pub fn resume_current(state: &mut GameState) -> Result<ResourceTransition, String> {
    transaction(state, |candidate| {
        let Some(execution) = &mut candidate.scene_execution else {
            return Ok(ResourceTransition::default());
        };
        prune_suspended(execution);
        let Some(active) = &mut execution.active else {
            let tasks = &mut execution.game_tasks;
            if !tasks.resources_parked {
                return Ok(ResourceTransition::default());
            }
            if candidate.motions.waiting.is_some() && tasks.suspended_motions.waiting.is_some()
                || candidate.videos.waiting.is_some() && tasks.suspended_videos.waiting.is_some()
            {
                return Err("A returning previsit wait conflicts with a live continuation".into());
            }
            if tasks.suspended_motions.waiting.is_some() {
                candidate.motions.waiting = tasks.suspended_motions.waiting.take();
            }
            if tasks.suspended_videos.waiting.is_some() {
                candidate.videos.waiting = tasks.suspended_videos.waiting.take();
            }
            tasks.resources_parked = false;
            execution.resources.park_wait_owner(None, false);
            return Ok(ResourceTransition {
                cancelled_waits: execution.resources.drain_waits(&TaskOwner::Game),
            });
        };
        if !active.tasks.resources_parked {
            return Err("This visit has no parked resources to resume".into());
        }
        let owner = TaskOwner::Scene {
            activation: active.id,
        };
        for key in active.tasks.suspended_motions.tracks.keys() {
            if candidate.motions.tracks.contains_key(key) {
                return Err("A live motion would be overwritten by a returning visit".into());
            }
        }
        for key in active.tasks.suspended_videos.tracks.keys() {
            if candidate.videos.tracks.contains_key(key) {
                return Err("A live player would be overwritten by a returning visit".into());
            }
        }
        for screen in &active.tasks.suspended_ui.screens {
            if candidate
                .ui
                .screens
                .iter()
                .any(|live| live.name == screen.name)
            {
                return Err("A live interface would be overwritten by a returning visit".into());
            }
        }
        if candidate.motions.waiting.is_some() && active.tasks.suspended_motions.waiting.is_some()
            || candidate.videos.waiting.is_some() && active.tasks.suspended_videos.waiting.is_some()
        {
            return Err("A returning wait conflicts with another live owner".into());
        }
        candidate
            .motions
            .tracks
            .append(&mut active.tasks.suspended_motions.tracks);
        if active.tasks.suspended_motions.waiting.is_some() {
            candidate.motions.waiting = active.tasks.suspended_motions.waiting.take();
        }
        candidate
            .videos
            .tracks
            .append(&mut active.tasks.suspended_videos.tracks);
        if active.tasks.suspended_videos.waiting.is_some() {
            candidate.videos.waiting = active.tasks.suspended_videos.waiting.take();
        }
        candidate.ui.next_order = candidate
            .ui
            .next_order
            .max(active.tasks.suspended_ui.next_order);
        candidate
            .ui
            .screens
            .append(&mut active.tasks.suspended_ui.screens);
        active.tasks.resources_parked = false;
        execution.resources.park_owner(&owner, false)?;
        execution.resources.park_wait_owner(Some(active.id), false);
        let cancelled = execution.resources.drain_waits(&owner);
        Ok(ResourceTransition {
            cancelled_waits: cancelled,
        })
    })
}

fn remove_live_task(state: &mut GameState, task: &ResourceTask, active: Option<ActivationId>) {
    if task.parked || !eligible(&task.key.owner, active) {
        return;
    }
    match task.kind {
        ResourceTaskKind::Motion => {
            state.motions.tracks.remove(task.alias());
            if state.motions.waiting.as_deref() == Some(task.alias()) {
                state.motions.waiting = None;
            }
        }
        ResourceTaskKind::Video => {
            state.videos.tracks.remove(task.alias());
            if state.videos.waiting.as_deref() == Some(task.alias()) {
                state.videos.waiting = None;
            }
        }
        ResourceTaskKind::Interface => state
            .ui
            .screens
            .retain(|screen| screen.name != task.alias()),
        ResourceTaskKind::Canvas => {
            if let Some((name, element)) = task.alias().split_once('/') {
                if let Some(screen) = state
                    .ui
                    .screens
                    .iter_mut()
                    .find(|screen| screen.name == name)
                {
                    screen.canvas_states.remove(element);
                }
            }
        }
        ResourceTaskKind::Effect | ResourceTaskKind::Sfx | ResourceTaskKind::Voice => {}
    }
}
fn prune_suspended(execution: &mut SceneExecution) {
    fn prune(
        tasks: &mut crate::scene_tasks::SceneTasks,
        id: Option<ActivationId>,
        registry: &ResourceRegistry,
    ) {
        let valid = |kind, key: &str| {
            registry.tasks.iter().any(|task| {
                id.is_some_and(|activation| task.key.owner == TaskOwner::Scene { activation })
                    && task.kind == kind
                    && task.alias() == key
            })
        };
        tasks
            .suspended_motions
            .tracks
            .retain(|key, _| valid(ResourceTaskKind::Motion, key));
        if tasks.suspended_motions.waiting.as_ref().is_some_and(|key| {
            !registry
                .waits
                .iter()
                .any(|wait| wait.owner == id && wait.key.local_id == format!("motion:{key}"))
        }) {
            tasks.suspended_motions.waiting = None;
        }
        tasks
            .suspended_videos
            .tracks
            .retain(|key, _| valid(ResourceTaskKind::Video, key));
        if tasks.suspended_videos.waiting.as_ref().is_some_and(|key| {
            !registry
                .waits
                .iter()
                .any(|wait| wait.owner == id && wait.key.local_id == format!("video:{key}"))
        }) {
            tasks.suspended_videos.waiting = None;
        }
        tasks
            .suspended_ui
            .screens
            .retain(|screen| valid(ResourceTaskKind::Interface, &screen.name));
        for screen in &mut tasks.suspended_ui.screens {
            screen.canvas_states.retain(|element, _| {
                valid(
                    ResourceTaskKind::Canvas,
                    &format!("{}/{element}", screen.name),
                )
            });
        }
    }
    if let Some(active) = &mut execution.active {
        prune(&mut active.tasks, Some(active.id), &execution.resources);
    }
    prune(&mut execution.game_tasks, None, &execution.resources);
    for frame in &mut execution.frames {
        if let Some(previsit) = &mut frame.pre_scene_resources {
            prune(previsit, None, &execution.resources);
        }
        if let Some(parent) = &mut frame.parked {
            prune(&mut parent.tasks, Some(parent.id), &execution.resources);
        }
    }
}

/// Normal completion releases every continuation waiting for this exact
/// operation, including suspended callers. It keeps the terminal descriptor
/// and target; it never substitutes cancellation or runs an event handler.
pub fn release_waiters(
    state: &mut GameState,
    token: &ResourceToken,
    runtime_epoch: u64,
) -> Result<bool, String> {
    let Some(execution) = &state.scene_execution else {
        return Ok(false);
    };
    if !execution
        .resources
        .accepts(token, runtime_epoch, execution.owner())
    {
        return Ok(false);
    }
    let task = execution
        .resources
        .tasks
        .iter()
        .find(|task| task.key == token.task.key)
        .unwrap();
    let terminal = match task.kind {
        ResourceTaskKind::Motion => state
            .motions
            .tracks
            .get(task.alias())
            .is_some_and(|track| !track.running),
        ResourceTaskKind::Video => state
            .videos
            .tracks
            .get(task.alias())
            .is_some_and(|track| track.finished()),
        _ => return Err("Only a motion or video can complete a narrative wait".into()),
    };
    if !terminal {
        return Err("The exact waited operation has not completed".into());
    }
    let mut candidate = state.clone();
    let execution = candidate.scene_execution.as_mut().unwrap();
    let active = execution.owner();
    let live_wait = execution.resources.waits.iter().any(|wait| {
        wait.owner == active
            && !wait.parked
            && wait.key == token.task.key
            && wait.revision == token.task.revision
            && wait.target == token.target
    });
    execution.resources.waits.retain(|wait| {
        wait.key != token.task.key
            || wait.revision != token.task.revision
            || wait.target != token.target
    });
    let task = execution
        .resources
        .tasks
        .iter_mut()
        .find(|task| task.key == token.task.key)
        .unwrap();
    task.wait = None;
    if live_wait {
        match task.kind {
            ResourceTaskKind::Motion
                if candidate.motions.waiting.as_deref() == Some(task.alias()) =>
            {
                candidate.motions.waiting = None;
            }
            ResourceTaskKind::Video
                if candidate.videos.waiting.as_deref() == Some(task.alias()) =>
            {
                candidate.videos.waiting = None;
            }
            _ => {}
        }
    }
    prune_suspended(execution);
    validate_current_resource_state(&candidate)?;
    *state = candidate;
    Ok(true)
}

pub fn replace_target(
    state: &mut GameState,
    owner: TaskOwner,
    kind: TargetKind,
    alias: &str,
) -> Result<ResourceTransition, String> {
    transaction(state, |candidate| {
        mutate_targets(candidate, |registry| {
            registry.replace_target(owner, kind, alias)?;
            Ok(())
        })
    })
}
pub fn remove_target(
    state: &mut GameState,
    owner: &TaskOwner,
    kind: TargetKind,
    alias: &str,
) -> Result<ResourceTransition, String> {
    if state.scene_execution.is_none()&&kind==TargetKind::Interface{
        if let Some(registry)=&state.ui.instance_registry{let mut next=registry.clone();next.remove_target(owner,kind,alias,CancellationReason::TargetRemoved)?;state.ui.screens.retain(|screen|screen.name!=alias);state.ui.instance_registry=Some(next);return Ok(ResourceTransition::default());}
    }
    transaction(state, |candidate| {
        mutate_targets(candidate, |registry| {
            registry.remove_target(owner, kind, alias, CancellationReason::TargetRemoved)?;
            Ok(())
        })
    })
}
pub fn supersede_task(state: &mut GameState, key: &TaskKey) -> Result<ResourceTransition, String> {
    transaction(state, |candidate| {
        mutate_targets(candidate, |registry| {
            registry.supersede(key)?;
            Ok(())
        })
    })
}
fn mutate_targets(
    state: &mut GameState,
    mutation: impl FnOnce(&mut ResourceRegistry) -> Result<(), String>,
) -> Result<ResourceTransition, String> {
    let Some(mut execution) = state.scene_execution.take() else {
        if let Some(registry)=&mut state.game_audio {mutation(registry)?;}
        return Ok(ResourceTransition::default());
    };
    let before = execution.resources.tasks.clone();
    mutation(&mut execution.resources)?;
    for task in &before {
        if !execution
            .resources
            .tasks
            .iter()
            .any(|live| live.key == task.key && live.revision == task.revision)
        {
            remove_live_task(state, task, execution.owner());
        }
    }
    prune_suspended(&mut execution);
    let owner = current_owner(&execution);
    let cancelled = execution.resources.drain_waits(&owner);
    state.scene_execution = Some(execution);
    Ok(ResourceTransition {
        cancelled_waits: cancelled,
    })
}

pub fn supersede_target_tasks(
    state: &mut GameState,
    target: &TargetIdentity,
    kind: ResourceTaskKind,
) -> Result<ResourceTransition, String> {
    transaction(state, |candidate| {
        mutate_targets(candidate, |registry| {
            registry.cancel_where(
                |task| task.target == *target && task.kind == kind,
                CancellationReason::OperationSuperseded,
            )?;
            Ok(())
        })
    })
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EffectChannel {
    SpriteAnimation,
    VisualFade,
    AudioFade,
}
pub fn supersede_effect_channel(
    state: &mut GameState,
    target: &TargetIdentity,
    channel: EffectChannel,
) -> Result<ResourceTransition, String> {
    transaction(state, |candidate| {
        if candidate.scene_execution.is_none() && target.kind==TargetKind::Sprite {
            if let Some(registry)=candidate.effect_resource_registry_mut(target.kind){registry.cancel_where(|task|task.target==*target&&task.effect.as_ref().is_some_and(|e|matches!((&e.parameters,channel),(EffectParameters::SpriteAnimation{..},EffectChannel::SpriteAnimation))),CancellationReason::OperationSuperseded)?;}
            return Ok(ResourceTransition::default());
        }
        mutate_targets(candidate, |registry| {
            registry.cancel_where(
                |task| {
                    task.target == *target
                        && task.effect.as_ref().is_some_and(|effect| {
                            matches!(
                                (&effect.parameters, channel),
                                (
                                    EffectParameters::SpriteAnimation { .. },
                                    EffectChannel::SpriteAnimation
                                ) | (
                                    EffectParameters::VisualFade { .. },
                                    EffectChannel::VisualFade
                                ) | (EffectParameters::AudioFade { .. }, EffectChannel::AudioFade)
                            )
                        })
                },
                CancellationReason::OperationSuperseded,
            )?;
            Ok(())
        })
    })
}

pub fn register_effect(
    state: &mut GameState,
    runtime_epoch: u64,
    key: TaskKey,
    target: TargetIdentity,
    effect: SavedEffect,
) -> Result<ResourceToken, String> {
    effect.validate()?;
    validate_effect_target(&effect, target.kind)?;
    let mut candidate = state.clone();
    let active=candidate.audio_active_owner();
    let registry=candidate.effect_resource_registry_mut(target.kind).ok_or("Resource effect requires its actual owned target family")?;
    if !eligible(&key.owner, active) {
        return Err("Cannot register an effect for a parked visit".into());
    }
    let replacing = registry.tasks.iter().any(|task| task.key == key);
    let registered_revision = registry.register(
        key.clone(),
        target.clone(),
        ResourceTaskKind::Effect,
        None,
    )?;
    // Register first enforces the same live target/kind and rejects parked
    // operations. Every accepted effect launch is a fresh operation, even
    // when its authored descriptor is identical. Generic task registration
    // keeps its existing idempotent behavior for the other resource families.
    let revision = if replacing {
        registry.revision()?
    } else {
        registered_revision
    };
    let task = registry
        .tasks
        .iter_mut()
        .find(|task| task.key == key)
        .unwrap();
    task.revision = revision;
    task.effect = Some(effect);
    validate_current_resource_state(&candidate)?;
    *state = candidate;
    Ok(ResourceToken {
        task: TaskToken {
            runtime_epoch,
            key,
            revision,
        },
        target,
    })
}

/// Complete the exact terminal operation as one pure candidate transaction.
/// Incoming Game objects already hold their desired model: an effect's end
/// does not hide, replace or remove a global object sharing its old address.
pub fn complete_due_effect(
    state: &mut GameState,
    token: &ResourceToken,
    runtime_epoch: u64,
) -> Result<Option<EffectCompletion>, String> {
    let Some(registry) = state.effect_resource_registry(token.target.kind) else {
        return Ok(None);
    };
    if !registry
        .accepts(token, runtime_epoch, state.audio_active_owner())
    {
        return Ok(None);
    }
    validate_current_resource_state(state)?;
    let task = registry
        .tasks
        .iter()
        .find(|task| task.key == token.task.key)
        .unwrap();
    if task.kind != ResourceTaskKind::Effect {
        return Err("Only an effect can complete its presentation clock".into());
    }
    let effect = task
        .effect
        .as_ref()
        .ok_or("An effect task has no descriptor")?;
    if !effect.clock.is_due() {
        return Ok(None);
    }
    let completed = task.clone();
    let mut candidate = state.clone();
    candidate.effect_resource_registry_mut(token.target.kind).unwrap()
        .tasks
        .retain(|task| {
            task.key != token.task.key
                || task.revision != token.task.revision
                || task.target != token.target
        });
    let removed = candidate.effect_resource_registry_mut(token.target.kind).unwrap()
        .prune_unreferenced_retired_targets();
    let retired_target = removed.into_iter().find(|target| *target == token.target);
    validate_current_resource_state(&candidate)?;
    *state = candidate;
    Ok(Some(EffectCompletion {
        task: completed,
        retired_target,
    }))
}

#[derive(Default)]
pub(crate) struct ResourceBudget {
    motions: usize,
    videos: usize,
    screens: usize,
    canvases: usize,
    paths: BTreeSet<String>,
}
impl ResourceBudget {
    pub(crate) fn retired(
        &mut self,
        registry: &ResourceRegistry,
        values: &mut NarrativeBudget,
    ) -> Result<(), String> {
        for presentation in registry
            .tasks
            .iter()
            .filter_map(|task| task.effect.as_ref()?.presentation.as_ref())
        {
            presentation.validate_structure()?;
            presentation.budget(values)?;
            self.paths.extend(presentation.resource_paths());
        }
        for audio in registry.tasks.iter().filter_map(|task| task.audio.as_ref()) {
            audio.validate()?;
            self.paths.insert(audio.file.clone());
            values.binding("@audio_source", &rvn_parser::Value::Str(audio.file.clone()))?;
        }
        self.check()
    }
    pub(crate) fn snapshot(
        &mut self,
        motions: &crate::motion::MotionState,
        videos: &crate::video::VideoState,
        ui: &crate::ui::UiState,
        values: &mut NarrativeBudget,
    ) -> Result<(), String> {
        validate_snapshot_clocks(motions, videos)?;
        validate_ui_state(ui, values)?;
        self.motions = self
            .motions
            .checked_add(motions.tracks.len())
            .ok_or("Motion budget overflow")?;
        self.videos = self
            .videos
            .checked_add(videos.tracks.len())
            .ok_or("Video budget overflow")?;
        self.screens = self
            .screens
            .checked_add(ui.screens.len())
            .ok_or("Interface budget overflow")?;
        self.canvases = self
            .canvases
            .checked_add(
                ui.screens
                    .iter()
                    .map(|screen| screen.canvas_states.len())
                    .sum(),
            )
            .ok_or("Canvas budget overflow")?;
        for track in motions.tracks.values() {
            self.paths.extend(track.definition.frame_paths());
        }
        for track in videos.tracks.values() {
            self.paths.extend(track.clip.resources());
        }
        self.check()
    }
    pub(crate) fn check(&self) -> Result<(), String> {
        if self.motions > MAX_MOTIONS
            || self.videos > MAX_VIDEOS
            || self.screens > MAX_INTERFACES
            || self.canvases > MAX_CANVASES
            || self.paths.len() > MAX_RESOURCES
        {
            return Err("Aggregate saved resources exceed 64 motions, 4096 assets, 8 players, 32 screens or canvases".into());
        }
        Ok(())
    }
}

pub(crate) fn validate_snapshot_clocks(
    motions: &crate::motion::MotionState,
    videos: &crate::video::VideoState,
) -> Result<(), String> {
    motions.validate()?;
    let mut video_snapshot = videos.clone();
    if let Some(alias) = &videos.waiting {
        if !rvn_ui::composition::valid_name(alias) {
            return Err("Invalid saved video wait identity".into());
        }
        if !videos.tracks.contains_key(alias) {
            video_snapshot.waiting = None;
        }
    }
    video_snapshot.validate()
}

pub(crate) fn validate_ui_state(
    ui: &crate::ui::UiState,
    budget: &mut NarrativeBudget,
) -> Result<(), String> {
    let mut names = HashSet::new();
    let mut orders = HashSet::new();
    for screen in &ui.screens {
        screen.owner_properties.validate()?;
        if let Some(request)=&screen.owner_focus_request{request.validate()?;}
        if !rvn_ui::composition::valid_name(&screen.name)
            || !names.insert(&screen.name)
            || !orders.insert((screen.host_role.is_some(), screen.order))
            || screen.order
                >= if screen.host_role.is_some() {
                    ui.host_next_order
                } else {
                    ui.next_order
                }
            || screen.host_root && screen.host_role.is_none()
            || !crate::ui::valid_instance_layer(screen)
            || screen
                .focus
                .as_ref()
                .is_some_and(|name| !rvn_ui::composition::valid_name(name))
            || screen.arguments.len() > 128
            || screen.values.len() > 512
        {
            return Err("Invalid saved interface identity, order or metadata".into());
        }
        for argument in &screen.arguments {
            budget.binding("@ui_argument", argument)?;
        }
        for (name, value) in &screen.values {
            if !rvn_ui::composition::valid_name(name)
                || matches!(
                    value,
                    rvn_parser::Value::Enum(_) | rvn_parser::Value::Text(_) | rvn_parser::Value::Widget(_) | rvn_parser::Value::WidgetClass(_) | rvn_parser::Value::List(_) | rvn_parser::Value::Dict(_)
                )
                || matches!(value, rvn_parser::Value::Str(text) if text.len() > 65536)
            {
                return Err("Invalid saved interface control value".into());
            }
            budget.binding(name, value)?;
        }
        for (name, canvas) in &screen.canvas_states {
            if !rvn_ui::composition::valid_name(name)
                || !canvas.elapsed.is_finite()
                || !(0.0..=1.0e12).contains(&canvas.elapsed)
            {
                return Err("Invalid saved canvas identity or elapsed time".into());
            }
            crate::ui::validate_canvas_state(&canvas.state).map_err(|error| error.to_string())?;
            budget.binding(name, &canvas.state)?;
        }
    }
    if ui.screens.len() > MAX_INTERFACES
        || ui
            .screens
            .iter()
            .map(|screen| screen.canvas_states.len())
            .sum::<usize>()
            > MAX_CANVASES
    {
        return Err("Interface/canvas aggregate limit exceeded".into());
    }
    Ok(())
}

pub fn validate_current_resource_state(state: &GameState) -> Result<(), String> {
    crate::game_tasks::validate(state)?;
    crate::game_audio::validate(state)?;
    crate::display_reveal::validate_state(state)?;
    crate::cinematic_state::validate_state(state)?;
    crate::audio_checkpoint::music_views(state,0)?;
    let Some(execution) = &state.scene_execution else {
        return Ok(());
    };
    execution.resources.validate(execution)?;
    let mut resources = ResourceBudget::default();
    let mut values = NarrativeBudget::default();
    resources.retired(&execution.resources, &mut values)?;
    values.values(&state.vars)?;
    execution.validate_sprite_styles(&mut values)?;
    values.values(&execution.pre_scene_temporaries)?;
    resources.snapshot(&state.motions, &state.videos, &state.ui, &mut values)?;
    if let Some(active) = &execution.active {
        values.values(&active.locals)?;
        values.values(&active.temporaries)?;
        resources.snapshot(
            &active.tasks.suspended_motions,
            &active.tasks.suspended_videos,
            &active.tasks.suspended_ui,
            &mut values,
        )?;
    }
    resources.snapshot(
        &execution.game_tasks.suspended_motions,
        &execution.game_tasks.suspended_videos,
        &execution.game_tasks.suspended_ui,
        &mut values,
    )?;
    for frame in &execution.frames {
        if let Some(previsit) = &frame.pre_scene_resources {
            resources.snapshot(
                &previsit.suspended_motions,
                &previsit.suspended_videos,
                &previsit.suspended_ui,
                &mut values,
            )?;
        }
        if let Some(parent) = &frame.parked {
            values.values(&parent.locals)?;
            values.values(&parent.temporaries)?;
            resources.snapshot(
                &parent.tasks.suspended_motions,
                &parent.tasks.suspended_videos,
                &parent.tasks.suspended_ui,
                &mut values,
            )?;
        }
        if let Some(temporaries) = &frame.pre_scene_temporaries {
            values.values(temporaries)?;
        }
    }
    if !state.background_image.is_empty() {
        resources.paths.insert(state.background_image.clone());
    }
    if let Some(path) = &state.music.current_file {
        resources.paths.insert(path.clone());
    }
    if let Some(id) = &state.cinematic.current {
        resources.paths.insert(id.clone());
        values.binding("@cinematic", &rvn_parser::Value::Str(id.clone()))?;
    }
    state.layered.validate()?;
    for composition in state.layered.characters.values() {
        resources.paths.extend(composition.definition.image_paths());
    }
    resources.check()?;
    validate_suspended_ownership(execution)?;
    validate_snapshot_ownership(
        execution,
        execution.owner(),
        false,
        &state.motions,
        &state.videos,
        &state.ui,
    )?;
    crate::ui_residency::validate_resident_projections(state)
}

/// Prepare a portable snapshot without detaching a narrative continuation from
/// the exact Host operation it is still waiting for. No live state is changed.
pub(crate) fn portable_story_state(state: &GameState) -> Result<GameState, String> {
    crate::audio_playback::require_portable_checkpoint(state)?;
    if crate::audio_checkpoint::music_views(state,0)?.iter().any(|view|!matches!(view.owner,TaskOwner::Host{..})) {
        return Err("Story Music requires a fresh fenced decoder snapshot, not a saved cursor copy".into());
    }
    portable_story_state_captured_audio(state)
}

/// Called only through the opaque fresh audio snapshot boundary.
pub(crate) fn portable_story_state_captured_audio(state:&GameState)->Result<GameState,String> {
    let Some(execution) = &state.scene_execution else {
        return Ok(state.clone());
    };
    validate_current_resource_state(state)?;
    let original = &execution.resources;
    crate::audio_checkpoint::require_saved_checkpoints(state)?;
    let portable = original.story_only();
    let retained = |key: &TaskKey, revision, target: &TargetIdentity| {
        portable
            .tasks
            .iter()
            .any(|task| task.key == *key && task.revision == revision && task.target == *target)
    };
    for wait in &original.waits {
        if !retained(&wait.key, wait.revision, &wait.target) {
            return Err(format!(
                "Narrative continuation depends on a non-portable Host operation: {}",
                wait.key.local_id
            ));
        }
    }
    fn filter_snapshot(
        owner: Option<ActivationId>,
        parked: bool,
        original: &ResourceRegistry,
        portable: &ResourceRegistry,
        motions: &mut crate::motion::MotionState,
        videos: &mut crate::video::VideoState,
        ui: &mut crate::ui::UiState,
    ) {
        let keep = |kind, alias: &str| {
            original.tasks.iter().any(|task| {
                task.kind == kind
                    && task.alias() == alias
                    && task.parked == parked
                    && if parked {
                        task.key.owner
                            == owner
                                .map(|activation| TaskOwner::Scene { activation })
                                .unwrap_or(TaskOwner::Game)
                    } else {
                        eligible(&task.key.owner, owner)
                    }
                    && portable.tasks.iter().any(|saved| {
                        saved.key == task.key
                            && saved.revision == task.revision
                            && saved.target == task.target
                    })
            })
        };
        motions
            .tracks
            .retain(|alias, _| keep(ResourceTaskKind::Motion, alias));
        videos
            .tracks
            .retain(|alias, _| keep(ResourceTaskKind::Video, alias));
        *ui = ui.story_only();
        ui.screens
            .retain(|screen| keep(ResourceTaskKind::Interface, &screen.name));
        for screen in &mut ui.screens {
            screen.canvas_states.retain(|element, _| {
                keep(
                    ResourceTaskKind::Canvas,
                    &format!("{}/{element}", screen.name),
                )
            });
        }
    }
    let mut candidate = state.clone();
    filter_snapshot(
        execution.owner(),
        false,
        original,
        &portable,
        &mut candidate.motions,
        &mut candidate.videos,
        &mut candidate.ui,
    );
    let saved = candidate.scene_execution.as_mut().unwrap();
    let filter_tasks = |owner, tasks: &mut crate::scene_tasks::SceneTasks| {
        filter_snapshot(
            owner,
            true,
            original,
            &portable,
            &mut tasks.suspended_motions,
            &mut tasks.suspended_videos,
            &mut tasks.suspended_ui,
        );
    };
    if let Some(active) = &mut saved.active {
        filter_tasks(Some(active.id), &mut active.tasks);
    }
    filter_tasks(None, &mut saved.game_tasks);
    for frame in &mut saved.frames {
        if let Some(parent) = &mut frame.parked {
            filter_tasks(Some(parent.id), &mut parent.tasks);
        }
        if let Some(previsit) = &mut frame.pre_scene_resources {
            filter_tasks(None, previsit);
        }
    }
    saved.resources = portable;
    validate_current_resource_state(&candidate)?;
    Ok(candidate)
}

/// Allocate the outgoing object only on a fully validated candidate. The caller
/// must commit this together with its desired Game model and supersession.
pub fn register_retired_effect(
    state: &mut GameState,
    runtime_epoch: u64,
    owner: TaskOwner,
    presentation: crate::retired_presentations::RetiredPresentation,
    mut effect: SavedEffect,
) -> Result<Option<ResourceToken>, String> {
    validate_current_resource_state(state)?;
    if effect.presentation.is_some() || effect.clock.elapsed.to_bits() != 0.0f64.to_bits() {
        return Err("A new outgoing operation needs an uncaptured zero clock".into());
    }
    let active=state.audio_active_owner();
    if state.audio_resource_registry().is_none(){return Err("Outgoing appearances require an actual registry".into());}
    validate_owner(&owner)?;
    if !eligible(&owner, active) {
        return Err("A parked visit cannot create a new outgoing appearance".into());
    }
    effect.presentation = Some(presentation);
    effect.validate()?;
    if effect.clock.is_due() {
        return Ok(None);
    }
    let mut candidate = state.clone();
    let registry = candidate.audio_resource_registry_mut().unwrap();
    let next = registry
        .next_incarnation
        .checked_add(1)
        .ok_or("Resource incarnation exhausted")?;
    let alias = format!("retired_{next}");
    let target = registry.ensure_target(owner.clone(), TargetKind::PresentationRetired, &alias)?;
    let key = resource_key(owner, ResourceTaskKind::Effect, &alias);
    let revision =
        registry.register(key.clone(), target.clone(), ResourceTaskKind::Effect, None)?;
    registry
        .tasks
        .iter_mut()
        .find(|task| task.key == key)
        .unwrap()
        .effect = Some(effect);
    validate_current_resource_state(&candidate)?;
    *state = candidate;
    Ok(Some(ResourceToken {
        task: TaskToken {
            runtime_epoch,
            key,
            revision,
        },
        target,
    }))
}

/// A replacement of a global slot invalidates all its outgoing appearances,
/// including parked owners. Authored Story Stop uses the narrower sibling below.
pub fn cancel_retired_slot(
    state: &mut GameState,
    slot: &crate::retired_presentations::PresentationSlot,
    reason: CancellationReason,
) -> Result<ResourceTransition, String> {
    cancel_retired_slot_matching(state, slot, reason, false)
}
pub fn cancel_retired_story_slot(
    state: &mut GameState,
    slot: &crate::retired_presentations::PresentationSlot,
    reason: CancellationReason,
) -> Result<ResourceTransition, String> {
    cancel_retired_slot_matching(state, slot, reason, true)
}
fn cancel_retired_slot_matching(
    state: &mut GameState,
    slot: &crate::retired_presentations::PresentationSlot,
    reason: CancellationReason,
    story_only: bool,
) -> Result<ResourceTransition, String> {
    transaction(state, |candidate| {
        mutate_targets(candidate, |registry| {
            let targets: Vec<_> = registry
                .tasks
                .iter()
                .filter(|task| {
                    (!story_only || !matches!(task.key.owner, TaskOwner::Host { .. }))
                        && task
                            .effect
                            .as_ref()
                            .and_then(|effect| effect.presentation.as_ref())
                            .is_some_and(|payload| payload.original_slot == *slot)
                })
                .map(|task| task.target.clone())
                .collect();
            registry.cancel_where(
                |task| {
                    targets.contains(&task.target)
                        || task
                            .dependencies
                            .iter()
                            .any(|target| targets.contains(target))
                },
                reason,
            )?;
            Ok(())
        })
    })
}
