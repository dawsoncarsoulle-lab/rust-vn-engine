//! Additional all-Source checkpoint contract for an actual stopped frame.
//! Historical Story-only audio_checkpoint APIs remain untouched.
use crate::audio_checkpoint::{AudioCheckpointSnapshot, MusicCheckpoint};
use crate::scene_resources::TargetIdentity;
use crate::GameState;
use std::collections::HashSet;

pub fn checkpointed_frame_state(
    original: &GameState,
    instance: u64,
    epoch: u64,
    snapshot: &AudioCheckpointSnapshot,
) -> Result<GameState, String> {
    if snapshot.rows.len() > crate::audio_playback::MAX_AUDIO_PLAYBACKS
        || snapshot.music.len() > crate::scene_resources::MAX_MOTIONS + 1
    {
        return Err("Frame audio Source count exceeds the real registry bound".into());
    }
    if snapshot.instance_id != instance || snapshot.runtime_epoch != epoch {
        return Err("Frame PCM capture belongs to another Engine/epoch".into());
    }
    let wanted = crate::audio_playback::describe(original, epoch)?;
    let music = crate::audio_checkpoint::music_views(original, epoch)?;
    if wanted.len() != snapshot.rows.len() || music.len() != snapshot.music.len() {
        return Err(
            "Every real Story and Host reader requires one shared-fence frame checkpoint".into(),
        );
    }
    let mut seen: HashSet<TargetIdentity> = HashSet::new();
    let mut next = original.clone();
    if let Some(registry) = next.audio_resource_registry_mut() {
        for row in &snapshot.rows {
            if !seen.insert(row.token.target.clone())
                || !wanted.iter().any(|view| view.token == row.token)
            {
                return Err("Frame PCM row is stale, duplicate or foreign".into());
            }
            row.checkpoint.validate()?;
            let task = registry
                .tasks
                .iter_mut()
                .find(|t| {
                    t.key == row.token.task.key
                        && t.revision == row.token.task.revision
                        && t.target == row.token.target
                })
                .ok_or("Frame PCM task disappeared")?;
            task.audio
                .as_mut()
                .ok_or("Frame checkpoint targets another resource family")?
                .checkpoint = Some(row.checkpoint.clone());
        }
        registry.music_checkpoints.clear();
        for row in &snapshot.music {
            if !seen.insert(row.target.clone())
                || !music
                    .iter()
                    .any(|v| v.authority.target == row.target && v.file == row.file)
            {
                return Err("Frame Music row is stale, duplicate, foreign or replaced".into());
            }
            row.checkpoint.validate()?;
            registry.music_checkpoints.push(MusicCheckpoint {
                target: row.target.clone(),
                file: row.file.clone(),
                checkpoint: row.checkpoint.clone(),
            });
        }
    }
    crate::scene_resources::validate_current_resource_state(&next)?;
    require_frame_checkpoints(&next)?;
    Ok(next)
}
pub fn require_frame_checkpoints(state: &GameState) -> Result<(), String> {
    crate::audio_checkpoint::require_saved_checkpoints(state)?;
    for view in crate::audio_playback::describe(state, 0)? {
        view.playback
            .checkpoint
            .as_ref()
            .ok_or("Host or Story one-shot has no exact saved Source checkpoint")?
            .validate()?;
    }
    for view in crate::audio_checkpoint::music_views(state, 0)? {
        view.checkpoint
            .as_ref()
            .ok_or("Host or Story Music has no exact saved Source checkpoint")?
            .validate()?;
    }
    Ok(())
}
/// No decoder/sink estimate: the immutable Source/PCM receipt is checked by the
/// real renderer before accepting state. This helper validates Core identities.
pub fn has_host_readers(state: &GameState) -> Result<bool, String> {
    Ok(crate::audio_playback::describe(state, 0)?.iter().any(|v| {
        matches!(
            v.token.task.key.owner,
            crate::scene_tasks::TaskOwner::Host { .. }
        )
    }) || crate::audio_checkpoint::music_views(state, 0)?
        .iter()
        .any(|v| matches!(v.owner, crate::scene_tasks::TaskOwner::Host { .. })))
}
