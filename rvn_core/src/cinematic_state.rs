//! PRIVATE: CG identity/schema rules; no renderer assets or decoder clocks.
use crate::scene_resources::TargetKind;
use crate::scene_tasks::TaskOwner;
use crate::{GameState, save::SaveData};

pub(crate) fn validate_state(state: &GameState) -> Result<(), String> {
    let Some(execution) = &state.scene_execution else { return Ok(()); };
    if let Some(id) = &state.cinematic.current {
        if !rvn_ui::programmable::safe_asset_path(id) {
            return Err("Invalid CG asset reference".into());
        }
    }
    if state.cinematic.transition.as_ref().is_some_and(|name| name.len() > 4096 || name.chars().any(char::is_control)) {
        return Err("Invalid CG transition name".into());
    }
    let records: Vec<_> = execution.resources.targets.iter()
        .filter(|record| record.identity.kind == TargetKind::Cinematic).collect();
    if records.len() != usize::from(state.cinematic.current.is_some())
        || records.iter().any(|record| record.owner != TaskOwner::Game || record.identity.alias != "cinematic") {
        return Err("Current CG and its exact Game incarnation disagree".into());
    }
    Ok(())
}

/// Inspect the actual portable Story registry, including parked descriptors.
/// Host-only outgoing CG is filtered by the existing Story-only writer.
pub(crate) fn extended_schema(execution: &crate::scene::SceneExecution) -> bool {
    execution.resources.targets.iter().any(|record|
        !matches!(record.owner, TaskOwner::Host { .. }) && record.identity.kind == TargetKind::Cinematic)
        || execution.resources.tasks.iter().any(|task|
            !matches!(task.key.owner, TaskOwner::Host { .. }) && (task.target.kind == TargetKind::Cinematic
                || task.effect.as_ref().and_then(|effect|effect.presentation.as_ref()).is_some_and(|payload|
                    payload.original_slot == crate::retired_presentations::PresentationSlot::Cinematic)))
}

/// No old fade clock exists in version10. Refuse it before touching Engine.
/// Old immediate CG is restored as the visible desired image without a fade.
pub(crate) fn prepare_legacy_scene_save(data: &mut SaveData) -> Result<(), String> {
    let Some(execution) = &mut data.scene_execution else { return Ok(()); };
    if extended_schema(execution) {
        return Err("Extended CG representation requires save format11".into());
    }
    if let Some(id) = &data.cinematic.current {
        if !rvn_ui::programmable::safe_asset_path(id) {
            return Err("Invalid legacy CG asset reference".into());
        }
        if matches!(data.cinematic.transition.as_deref(), Some("fade" | "dissolve")) {
            return Err("Version10 CG fade has no trustworthy saved phase; original preserved".into());
        }
        execution.resources.ensure_target(TaskOwner::Game, TargetKind::Cinematic, "cinematic")?;
    }
    Ok(())
}
