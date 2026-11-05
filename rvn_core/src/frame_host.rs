//! Real in-process Host recreation proof and candidate registry migration.
//! A saved alias or serialized token cannot construct this context.
use super::*;
use crate::eval::debugger_eval::Probe;
use crate::scene_resources::{resource_key, ResourceRegistry, ResourceTask};

#[derive(Clone)]
pub struct HostLifecycleRestoreContext {
    source: std::sync::Arc<DebugStateSource>,
    program: [u8; 32],
}
impl std::fmt::Debug for HostLifecycleRestoreContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HostLifecycleRestoreContext")
            .finish_non_exhaustive()
    }
}
pub(in crate::engine) struct RebasedHostContext {
    pub(in crate::engine) state: GameState,
    pub(in crate::engine) frame: PortableFrameOperation,
}
fn registry(state: &GameState) -> Result<&ResourceRegistry, SaveError> {
    state
        .scene_execution
        .as_ref()
        .map(|e| &e.resources)
        .or(state.ui.instance_registry.as_ref())
        .ok_or_else(|| invalid("Host lifecycle has no true registry"))
}
fn registry_mut(state: &mut GameState) -> Result<&mut ResourceRegistry, SaveError> {
    if let Some(scene) = &mut state.scene_execution {
        Ok(&mut scene.resources)
    } else {
        state
            .ui
            .instance_registry
            .as_mut()
            .ok_or_else(|| invalid("Host lifecycle has no true registry"))
    }
}
fn host(owner: &TaskOwner) -> bool {
    matches!(owner, TaskOwner::Host { .. })
}
/// Comparison of real immutable descriptors for the presently admitted families.
/// Runtime identity is purposefully excluded only after live context validation.
pub(super) fn semantic(task: &ResourceTask) -> Result<[u8; 32], SaveError> {
    let mut logical = task.clone();
    logical.revision = 0;
    logical.target.incarnation = 0;
    logical.wait = None;
    for dependency in &mut logical.dependencies {
        dependency.incarnation = 0;
    }
    if let Some(effect) = &mut logical.effect {
        effect.validate().map_err(SaveError::Incompatible)?;
        effect.clock.elapsed = 0.0;
        effect.base_resolved = false;
        match &mut effect.parameters {
            crate::scene_resources::EffectParameters::SpriteAnimation { base, .. }
            | crate::scene_resources::EffectParameters::VisualFade { base, .. } => {
                *base = Default::default()
            }
            _ => {}
        }
    }
    if let Some(audio) = &mut logical.audio {
        audio.validate().map_err(SaveError::Incompatible)?;
        audio.checkpoint = None;
        if let Some(envelope) = &mut audio.envelope {
            envelope.clock.elapsed = 0.0;
        }
        logical.key.local_id = format!("{:?}:birth", logical.kind);
        logical.target.alias = format!("{:?}:birth", logical.target.kind);
    }
    if logical.target.kind == crate::scene_resources::TargetKind::PresentationRetired {
        logical.key.local_id = "effect:retired:birth".into();
        logical.target.alias = "retired:birth".into();
        if let Some(presentation) = logical
            .effect
            .as_mut()
            .and_then(|e| e.presentation.as_mut())
        {
            // Historical provenance remains data, never a newly admitted target.
            presentation.original_target.identity.incarnation = 1;
            match &mut presentation.model {
                crate::retired_presentations::RetiredModel::Cinematic { sample, .. }
                | crate::retired_presentations::RetiredModel::Background { sample, .. }
                | crate::retired_presentations::RetiredModel::Sprite { sample, .. } => {
                    for layer in sample.layers.values_mut() {
                        layer.original_target.identity.incarnation = 1;
                    }
                }
                crate::retired_presentations::RetiredModel::Music { .. } => {}
            }
        }
    }
    digest(&logical)
}
pub(super) fn compatible(saved:&ResourceTask,current:&ResourceTask)->Result<bool,SaveError> {
    if semantic(saved)?==semantic(current)? { return Ok(true); }
    // The real audio clock discards a finished canonical incoming envelope.
    // This terminal runtime state does not change its file, owner or birth.
    if saved.audio.as_ref().is_some_and(|a| !a.stopping&&a.envelope.is_none())
        && current.audio.as_ref().is_some_and(|a| !a.stopping&&a.envelope.is_some()) {
        let mut terminal=current.clone();terminal.audio.as_mut().unwrap().envelope=None;
        return Ok(semantic(saved)?==semantic(&terminal)?);
    }
    Ok(false)
}

impl<R: Renderer> Engine<R> {
    pub(super) fn capture_host_music(
        &self,
        state: &GameState,
    ) -> Result<Vec<crate::audio_checkpoint::MusicCheckpoint>, SaveError> {
        crate::audio_checkpoint::music_views(state, 0)
            .map_err(SaveError::Incompatible)?
            .into_iter()
            .filter(|v| host(&v.owner))
            .map(|v| {
                Ok(crate::audio_checkpoint::MusicCheckpoint {
                    target: v.authority.target,
                    file: v.file,
                    checkpoint: v.checkpoint.ok_or_else(|| {
                        invalid("Host Music has no exact fresh Source checkpoint")
                    })?,
                })
            })
            .collect()
    }
    /// Asset preparation data only. The returned clone is never a commit permit;
    /// full preparation must recheck this live context and the actual source.
    pub fn save15_state_for_frame_audio_warming(
        &self,
        program: &rvn_parser::ParsedProgram,
        save: &crate::save::consumer_checkpoint::Save15,
        context: &HostLifecycleRestoreContext,
    ) -> Result<GameState, SaveError> {
        self.save15_state_for_frame_audio_warming_with_locale(
            program,
            save,
            context,
            self.locale.clone(),
        )
    }
    pub fn save15_state_for_frame_audio_warming_with_locale(
        &self,
        program: &rvn_parser::ParsedProgram,
        save: &crate::save::consumer_checkpoint::Save15,
        context: &HostLifecycleRestoreContext,
        locale: Option<crate::locale::LocaleManager>,
    ) -> Result<GameState, SaveError> {
        self.validate_save15_program(program)?;
        save.source.validate_current(program)?;
        save.encode()?;
        save.validate_enum_resources1025(&self.functions.enum_assets1025)?;
        if !self
            .functions
            .text_resources()
            .accepts_portable_receipt(&save.text_tables)
            .map_err(SaveError::Incompatible)?
        {
            return Err(invalid(
                "Save15 StringTable content/provider differs from current registry",
            ));
        }
        match (&locale, &save.locale) {
            (None, None) => {}
            (Some(current), Some(expected))
                if current
                    .accepts_portable_receipt(expected)
                    .map_err(SaveError::Incompatible)? => {}
            _ => {
                return Err(invalid(
                    "Save15 locale/catalog/provider differs from frozen provider",
                ))
            }
        }
        if self.persistent_vars.len() != save.persistent_context.len()
            || self
                .persistent_vars
                .iter()
                .any(|(k, v)| save.persistent_context.get(k) != Some(v))
        {
            return Err(invalid(
                "Save15 persistent context differs from stopped transaction",
            ));
        }
        let mut next = save
            .story
            .restore(program, &self.script, &self.consumer_program)?;
        if next.story_identity != self.state.story_identity {
            return Err(invalid(
                "Save15 story identity differs from frozen programme",
            ));
        }
        let frame = save
            .debugger
            .as_ref()
            .and_then(|d| d.frame_operation.as_ref())
            .ok_or_else(|| invalid("Frame audio warming requires an actual stopped frame"))?;
        let plan = self.rebase_frame_host_context(program, &next, frame, context)?;
        let mut source = self.debugger_candidate(Probe::shared());
        source.functions.debug_probe = None;
        source.state = plan.state;
        source.locale = locale;
        source.merge_frame_hosts(&mut next, &plan.frame)?;
        crate::frame_audio_checkpoint::require_frame_checkpoints(&next)
            .map_err(SaveError::Incompatible)?;
        crate::scene_resources::validate_current_resource_state(&next)
            .map_err(SaveError::Incompatible)?;
        Ok(next)
    }
    pub(super) fn frame_task_is_host(
        &self,
        state: &GameState,
        kind: ResourceTaskKind,
        alias: &str,
    ) -> bool {
        registry(state).is_ok_and(|r| {
            r.tasks
                .iter()
                .any(|t| host(&t.key.owner) && t.kind == kind && t.alias() == alias)
        })
    }
    pub(super) fn capture_host_motions(
        &self,
        state: &GameState,
    ) -> Result<Vec<HostMotionData>, SaveError> {
        let mut result = Vec::new();
        if let Ok(r) = registry(state) {
            for t in r
                .tasks
                .iter()
                .filter(|t| host(&t.key.owner) && t.kind == ResourceTaskKind::Motion)
            {
                let track = state
                    .motions
                    .tracks
                    .get(t.alias())
                    .ok_or_else(|| invalid("Actual Host motion has no exact track"))?;
                track.validate().map_err(SaveError::Incompatible)?;
                result.push(HostMotionData {
                    alias: t.alias().into(),
                    track: track.clone(),
                });
            }
        }
        Ok(result)
    }
    pub(super) fn capture_host_waits(
        &self,
        state: &GameState,
    ) -> Result<Vec<HostWaitData>, SaveError> {
        Ok(registry(state)
            .map(|r| {
                r.waits
                    .iter()
                    .filter(|w| host(&w.key.owner))
                    .map(|w| HostWaitData {
                        owner: w.owner,
                        key: w.key.clone(),
                        revision: w.revision,
                        target: w.target.clone(),
                        pc: w.position.pc,
                        parked: w.parked,
                    })
                    .collect()
            })
            .unwrap_or_default())
    }
    pub(super) fn capture_host_cancelled(
        &self,
        state: &GameState,
    ) -> Result<Vec<HostCancelledWaitData>, SaveError> {
        Ok(registry(state)
            .map(|r| {
                r.cancelled_waits
                    .iter()
                    .filter(|w| host(&w.key.owner))
                    .map(|w| HostCancelledWaitData {
                        owner: w.owner,
                        key: w.key.clone(),
                        revision: w.revision,
                        target: w.target.clone(),
                        pc: w.position.pc,
                        reason: w.reason,
                    })
                    .collect()
            })
            .unwrap_or_default())
    }
    fn host_wait_position(
        &self,
        pc: usize,
        kind: ResourceTaskKind,
    ) -> Result<crate::scene::ResumePosition, SaveError> {
        let p = self.narrative.as_ref().ok_or_else(|| {
            invalid("Host wait must keep a real Narrative caller, never a fake Game/Handler wait")
        })?;
        let instruction = Self::consumer_leaf(pc.checked_sub(1).and_then(|pc| self.script.get(pc)));
        if !matches!(
            (kind, instruction),
            (ResourceTaskKind::Motion, Some(Statement::MotionWait { .. }))
                | (ResourceTaskKind::Video, Some(Statement::VideoWait { .. }))
        ) {
            return Err(invalid(
                "Host wait has no matching frozen MotionWait/VideoWait instruction",
            ));
        }
        Ok(p.position(pc))
    }
    pub(super) fn install_host_payloads(
        &self,
        next: &mut GameState,
        frame: &PortableFrameOperation,
    ) -> Result<(), SaveError> {
        if frame.host_motions.len() > crate::scene_resources::MAX_MOTIONS
            || frame.host_music.len() > crate::scene_resources::MAX_MOTIONS
            || frame.host_waits.len() > crate::scene_resources::MAX_RESOURCES
            || frame.host_cancelled.len() > crate::scene_resources::MAX_RESOURCES
        {
            return Err(invalid(
                "Host payload count exceeds the real resource budget",
            ));
        }
        for witness in &frame.host_tasks {
            if let Some(saved) = &witness.descriptor {
                let task = registry_mut(next)?
                    .tasks
                    .iter_mut()
                    .find(|t| {
                        t.key == witness.key
                            && t.revision == saved.revision
                            && t.target == saved.target
                    })
                    .ok_or_else(|| invalid("Host payload has no exact admitted task"))?;
                if let Some(effect) = &saved.effect {
                    effect.validate().map_err(SaveError::Incompatible)?;
                    task.effect = Some(effect.clone());
                }
                if let Some(audio) = &saved.audio {
                    audio.validate().map_err(SaveError::Incompatible)?;
                    audio
                        .checkpoint
                        .as_ref()
                        .ok_or_else(|| invalid("Host reader has no fresh shared-fence checkpoint"))?
                        .validate()
                        .map_err(SaveError::Incompatible)?;
                    task.audio = Some(audio.clone());
                }
                task.wait = saved
                    .wait
                    .as_ref()
                    .map(|w| self.host_wait_position(w.pc, saved.kind))
                    .transpose()?;
            }
        }
        let music =
            crate::audio_checkpoint::music_views(next, 0).map_err(SaveError::Incompatible)?;
        let mut seen = std::collections::HashSet::new();
        for checkpoint in &frame.host_music {
            if !seen.insert(checkpoint.target.clone())
                || !music.iter().any(|v| {
                    host(&v.owner)
                        && v.authority.target == checkpoint.target
                        && v.file == checkpoint.file
                })
            {
                return Err(invalid(
                    "Host Music checkpoint has no exact admitted typed payload",
                ));
            }
            checkpoint
                .checkpoint
                .validate()
                .map_err(SaveError::Incompatible)?;
            registry_mut(next)?
                .music_checkpoints
                .push(checkpoint.clone());
        }
        let mut aliases = std::collections::HashSet::new();
        for motion in &frame.host_motions {
            if !aliases.insert(&motion.alias)
                || !frame
                    .host_tasks
                    .iter()
                    .any(|w| w.key.local_id == format!("motion:{}", motion.alias))
            {
                return Err(invalid("Host motion lacks a unique admitted source task"));
            }
            motion.track.validate().map_err(SaveError::Incompatible)?;
            let actual = self
                .state
                .motions
                .tracks
                .get(&motion.alias)
                .ok_or_else(|| invalid("Recreated Host has no true motion definition"))?;
            if actual.definition != motion.track.definition
                || actual.target != motion.track.target
                || next.motions.tracks.contains_key(&motion.alias)
            {
                return Err(invalid(
                    "Host motion source differs or collides with restored Story",
                ));
            }
            next.motions
                .tracks
                .insert(motion.alias.clone(), motion.track.clone());
        }
        if let Some(wait) = &frame.host_motion_wait {
            if next.motions.waiting.is_some() || !next.motions.tracks.contains_key(wait) {
                return Err(invalid("Host motion wait collides or has no track"));
            }
            next.motions.waiting = Some(wait.clone());
        }
        if let Some(wait) = &frame.host_video_wait {
            if next.videos.waiting.is_some() || !next.videos.tracks.contains_key(wait) {
                return Err(invalid("Host video wait collides or has no track"));
            }
            next.videos.waiting = Some(wait.clone());
        }
        for wait in &frame.host_waits {
            let task = registry(next)?
                .tasks
                .iter()
                .find(|t| {
                    t.key == wait.key && t.revision == wait.revision && t.target == wait.target
                })
                .ok_or_else(|| invalid("Host wait lease references another admitted operation"))?;
            let position = self.host_wait_position(wait.pc, task.kind)?;
            registry_mut(next)?
                .waits
                .push(crate::scene_resources::WaitLease {
                    owner: wait.owner,
                    key: wait.key.clone(),
                    revision: wait.revision,
                    target: wait.target.clone(),
                    position,
                    parked: wait.parked,
                });
        }
        for wait in &frame.host_cancelled {
            let kind = if wait.key.local_id.starts_with("motion:") {
                ResourceTaskKind::Motion
            } else if wait.key.local_id.starts_with("video:") {
                ResourceTaskKind::Video
            } else {
                return Err(invalid("No invented Host/audio wait family"));
            };
            let position = self.host_wait_position(wait.pc, kind)?;
            let r = registry_mut(next)?;
            r.next_incarnation = r.next_incarnation.max(wait.target.incarnation);
            r.next_revision = r.next_revision.max(wait.revision);
            r.cancelled_waits
                .push(crate::scene_resources::CancelledWait {
                    owner: wait.owner,
                    key: wait.key.clone(),
                    revision: wait.revision,
                    target: wait.target.clone(),
                    position,
                    reason: wait.reason,
                });
        }
        Ok(())
    }
    /// Caller establishes its real menu through the existing lifecycle first.
    /// This records current source/body/instance/registry, never saved labels.
    pub fn capture_host_lifecycle_restore_context(
        &self,
        program: &rvn_parser::ParsedProgram,
    ) -> Result<HostLifecycleRestoreContext, SaveError> {
        self.validate_save15_program(program)?;
        let contexts = self.frame_interfaces()?;
        let roles: std::collections::BTreeSet<_> = contexts.iter().filter_map(|i| i.role).collect();
        if roles.is_empty() {
            return Err(invalid("No real source Host lifecycle was recreated"));
        }
        let resources = registry(&self.state)?;
        for task in resources.tasks.iter().filter(|t| host(&t.key.owner)) {
            let TaskOwner::Host { role } = &task.key.owner else {
                unreachable!()
            };
            let role =
                rvn_ui::PageRole::from_id(role).ok_or_else(|| invalid("Host role is invalid"))?;
            if !roles.contains(&role) {
                return Err(invalid(
                    "Host resource lacks a real frozen source presenter",
                ));
            }
        }
        Ok(HostLifecycleRestoreContext {
            source: std::sync::Arc::new(
                self.debug_state_source()
                    .map_err(|e| invalid(&e.to_string()))?,
            ),
            program: self
                .consumer_program
                .identity()
                .map_err(|e| invalid(&e.to_string()))?,
        })
    }
    pub(in crate::engine) fn rebase_frame_host_context(
        &self,
        program: &rvn_parser::ParsedProgram,
        story: &GameState,
        saved: &PortableFrameOperation,
        context: &HostLifecycleRestoreContext,
    ) -> Result<RebasedHostContext, SaveError> {
        self.validate_save15_program(program)?;
        if !self.accepts_debug_state_source(&context.source)
            || context.program
                != self
                    .consumer_program
                    .identity()
                    .map_err(|e| invalid(&e.to_string()))?
        {
            return Err(invalid(
                "Host recreation context is stale, foreign, replaced or rolled back",
            ));
        }
        let actual = self.frame_interfaces()?;
        let current: Vec<_> = actual.iter().filter(|i| i.role.is_some()).collect();
        let old: Vec<_> = saved
            .interfaces
            .iter()
            .filter(|i| i.role.is_some())
            .collect();
        if current.len() != old.len() || old.is_empty() {
            return Err(invalid(
                "Saved Host set differs from actual recreated source lifecycle",
            ));
        }
        let live_registry = registry(&self.state)?;
        let current_tasks: Vec<_> = live_registry
            .tasks
            .iter()
            .filter(|t| host(&t.key.owner))
            .collect();
        if saved.host_tasks.len() != current_tasks.len() {
            return Err(invalid(
                "Host births/tasks differ from actual recreated lifecycle",
            ));
        }
        let mut task_pairs = Vec::new();
        let mut claimed = std::collections::HashSet::new();
        for (birth, witness) in saved.host_tasks.iter().enumerate() {
            let descriptor = witness.descriptor.as_ref().ok_or_else(|| {
                invalid("Old Host witness has no typed descriptor required for identity migration")
            })?;
            if descriptor.key != witness.key
                || digest(descriptor)? != witness.shape
                || !host(&descriptor.key.owner)
            {
                return Err(invalid(
                    "Saved Host witness differs from its logical descriptor",
                ));
            }
            let live = *current_tasks
                .get(birth)
                .ok_or_else(|| invalid("Saved birth ordinal has no real recreated operation"))?;
            let dynamic = matches!(
                descriptor.kind,
                ResourceTaskKind::Sfx | ResourceTaskKind::Voice
            ) || descriptor.target.kind
                == crate::scene_resources::TargetKind::PresentationRetired;
            if descriptor.kind != live.kind
                || descriptor.key.owner != live.key.owner
                || (!dynamic && descriptor.key != live.key)
            {
                return Err(invalid(
                    "Saved birth differs from actual owner/family/declaration ordering",
                ));
            }
            if !claimed.insert(live.key.clone()) || !compatible(descriptor,live)? {
                return Err(invalid(
                    "Host descriptor/source dependencies differ from actual lifecycle",
                ));
            }
            task_pairs.push((descriptor.clone(), live.clone()));
        }
        let mut state = story.clone();
        state.ui.screens.extend(
            self.state
                .ui
                .screens
                .iter()
                .filter(|i| i.host_role.is_some())
                .cloned(),
        );
        state.ui.host_next_order = self.state.ui.host_next_order;
        for task in &current_tasks {
            if task.kind == ResourceTaskKind::Video {
                let track = self
                    .state
                    .videos
                    .tracks
                    .get(task.alias())
                    .ok_or_else(|| invalid("Recreated video has no true track"))?;
                if state
                    .videos
                    .tracks
                    .insert(task.alias().into(), track.clone())
                    .is_some()
                {
                    return Err(invalid(
                        "Current Host video collides with actual restored Story",
                    ));
                }
            } else if task.kind == ResourceTaskKind::Motion {
                let track = self
                    .state
                    .motions
                    .tracks
                    .get(task.alias())
                    .ok_or_else(|| invalid("Recreated motion has no true track"))?;
                if state
                    .motions
                    .tracks
                    .insert(task.alias().into(), track.clone())
                    .is_some()
                {
                    return Err(invalid(
                        "Current Host motion collides with actual restored Story",
                    ));
                }
            }
        }
        let story_high = story
            .scene_execution
            .as_ref()
            .map(|e| &e.resources)
            .or(story.ui.instance_registry.as_ref());
        let resources = registry_mut(&mut state)?;
        *resources = resources.story_only();
        resources.next_incarnation = resources
            .next_incarnation
            .max(live_registry.next_incarnation);
        resources.next_revision = resources.next_revision.max(live_registry.next_revision);
        if let Some(story) = story_high {
            resources.next_incarnation = resources.next_incarnation.max(story.next_incarnation);
            resources.next_revision = resources.next_revision.max(story.next_revision);
        }
        let mut actual_to_new = std::collections::HashMap::new();
        for record in live_registry.targets.iter().filter(|r| host(&r.owner)) {
            let next_id = resources
                .next_incarnation
                .checked_add(1)
                .ok_or_else(|| invalid("Host target allocator exhausted"))?;
            let alias = match record.identity.kind {
                crate::scene_resources::TargetKind::SfxPlayback => format!("sfx_{next_id}"),
                crate::scene_resources::TargetKind::VoicePlayback => format!("voice_{next_id}"),
                crate::scene_resources::TargetKind::PresentationRetired => {
                    format!("retired_{next_id}")
                }
                _ => record.identity.alias.clone(),
            };
            let target = resources
                .ensure_target(record.owner.clone(), record.identity.kind, &alias)
                .map_err(SaveError::Incompatible)?;
            actual_to_new.insert(record.identity.clone(), target);
        }
        let mut rebuilt_tasks = std::collections::HashMap::new();
        for live in &current_tasks {
            let target = if let Some(target) = actual_to_new.get(&live.target) {
                target.clone()
            } else {
                let record = live_registry
                    .targets
                    .iter()
                    .find(|r| r.identity == live.target)
                    .ok_or_else(|| invalid("Host operation has no real target record"))?;
                let saved_registry = story_high.ok_or_else(|| {
                    invalid("Host effect requires its authentic restored Story target")
                })?;
                let target = saved_registry
                    .target(&record.owner, record.identity.kind, &record.identity.alias)
                    .ok_or_else(|| {
                        invalid(
                            "Current Host effect target is not in the real restored Story registry",
                        )
                    })?;
                target.clone()
            };
            let alias = if matches!(live.kind, ResourceTaskKind::Sfx | ResourceTaskKind::Voice)
                || target.kind == crate::scene_resources::TargetKind::PresentationRetired
            {
                target.alias.as_str()
            } else {
                live.alias()
            };
            let key = resource_key(live.key.owner.clone(), live.kind, alias);
            resources
                .register(key.clone(), target.clone(), live.kind, None)
                .map_err(SaveError::Incompatible)?;
            let task = resources.tasks.iter_mut().find(|t| t.key == key).unwrap();
            task.dependencies = live
                .dependencies
                .iter()
                .map(|d| actual_to_new.get(d).cloned().unwrap_or_else(|| d.clone()))
                .collect();
            task.effect = live.effect.clone();
            task.audio = live.audio.clone();
            rebuilt_tasks.insert(live.key.clone(), task.clone());
        }
        for instance in state
            .ui
            .screens
            .iter_mut()
            .filter(|i| i.host_role.is_some())
        {
            for bound in instance.consumer_frames.values_mut() {
                bound.owner.target = actual_to_new
                    .get(&bound.owner.target)
                    .ok_or_else(|| invalid("Recreated Host frame refers to another lifecycle"))?
                    .clone();
            }
            if let Some(projection) = &mut instance.consumer_projection {
                projection.owner.target = actual_to_new
                    .get(&projection.owner.target)
                    .ok_or_else(|| {
                        invalid("Recreated Host projection refers to another lifecycle")
                    })?
                    .clone();
            }
        }
        self.consumer_program
            .validate_state_binding(&state)
            .map_err(|e| invalid(&e.to_string()))?;
        let rebuilt_by_saved: std::collections::HashMap<_, _> = task_pairs
            .iter()
            .map(|(old, live)| {
                (
                    old.key.clone(),
                    rebuilt_tasks.get(&live.key).unwrap().clone(),
                )
            })
            .collect();
        let mut frame = saved.clone();
        let mut saved_to_new = std::collections::HashMap::new();
        for (descriptor, live) in &task_pairs {
            let target = rebuilt_tasks.get(&live.key).unwrap().target.clone();
            if let Some(old) = saved_to_new.insert(descriptor.target.clone(), target.clone()) {
                if old != target {
                    return Err(invalid(
                        "Saved Host target maps to contradictory real births",
                    ));
                }
            }
        }
        for source in frame.interfaces.iter_mut().filter(|i| i.role.is_some()) {
            let live = current
                .iter()
                .find(|i| {
                    i.role == source.role
                        && i.definition == source.definition
                        && i.source_ordinal == source.source_ordinal
                })
                .ok_or_else(|| invalid("Host source kind/definition/ordinal/role differs"))?;
            let data = source
                .host
                .as_mut()
                .ok_or_else(|| invalid("Host source lacks typed saved instance data"))?;
            let actual_data = live.host.as_ref().unwrap();
            if data.root != actual_data.root
                || data.instance.arguments != actual_data.instance.arguments
                || data.instance.modal != actual_data.instance.modal
                || data.instance.layer != actual_data.instance.layer
                || data.instance.definition() != live.definition.as_str()
            {
                return Err(invalid(
                    "Recreated Host invocation is not the actual saved definition contract",
                ));
            }
            let old_target = source.target.clone();
            let target = saved_to_new
                .get(&old_target)
                .ok_or_else(|| invalid("Saved interface has no proven actual lifecycle birth"))?
                .clone();
            for bound in data.instance.consumer_frames.values_mut() {
                if bound.owner.target != old_target {
                    return Err(invalid("Saved Host frame belongs to another instance"));
                }
                bound.owner.target = target.clone();
            }
            if let Some(projection) = &mut data.instance.consumer_projection {
                if projection.owner.target != old_target {
                    return Err(invalid("Saved projection belongs to another instance"));
                }
                projection.owner.target = target.clone();
            }
            source.target = target;
            source.order = live.order;
            data.instance.order = live.order;
        }
        for witness in &mut frame.host_tasks {
            let actual = rebuilt_by_saved
                .get(&witness.key)
                .ok_or_else(|| invalid("Rebased witness lacks a real candidate task"))?;
            let old = witness.descriptor.as_ref().unwrap();
            let mut data = actual.clone();
            data.effect = old.effect.clone();
            data.audio = old.audio.clone();
            data.wait = old.wait.clone();
            witness.key = actual.key.clone();
            witness.shape = digest(&data)?;
            witness.descriptor = Some(data);
        }
        for wait in &mut frame.host_waits {
            let pair = task_pairs
                .iter()
                .find(|(old, _)| old.key == wait.key)
                .ok_or_else(|| invalid("Wait lacks its actual lifecycle operation"))?;
            if wait.revision != pair.0.revision || wait.target != pair.0.target {
                return Err(invalid(
                    "Saved wait points at another logical resource generation",
                ));
            }
            let actual = rebuilt_by_saved.get(&wait.key).unwrap();
            wait.key = actual.key.clone();
            wait.target = actual.target.clone();
            wait.revision = actual.revision;
        }
        for music in &mut frame.host_music {
            music.target = saved_to_new
                .get(&music.target)
                .ok_or_else(|| invalid("Saved Host Music has no actual lifecycle birth"))?
                .clone();
        }
        let rebase = |source: &mut PortableResource| -> Result<(), SaveError> {
            if !host(&source.key.owner) {
                return Ok(());
            }
            let pair = task_pairs
                .iter()
                .find(|(old, _)| old.key == source.key)
                .ok_or_else(|| invalid("Host report has no actual lifecycle task"))?;
            if source.revision != pair.0.revision
                || source.target != pair.0.target
                || source.kind != pair.0.kind
                || source.dependencies != pair.0.dependencies
                || source.parked != pair.0.parked
            {
                return Err(invalid(
                    "Saved Host report differs from typed original resource descriptor",
                ));
            }
            let actual = rebuilt_by_saved.get(&source.key).unwrap();
            source.key = actual.key.clone();
            source.target = actual.target.clone();
            source.revision = actual.revision;
            source.dependencies = actual.dependencies.clone();
            source.parked = actual.parked;
            Ok(())
        };
        for report in &mut frame.videos {
            if let PortableVideoReport::Resident { source, .. } = report {
                rebase(source)?;
            }
        }
        for report in &mut frame.audio {
            if let PortableAudioReport::Resident { source, .. } = report {
                rebase(source)?;
            }
        }
        Ok(RebasedHostContext { state, frame })
    }
}
