use bevy::prelude::*;
use rvn_core::save::SaveManager;
use rvn_parser::Transition;

use crate::project_paths::ProjectPaths;
use crate::resources::{
    PersistentDataResource, ScriptErrorMessage, VnEngine, VnRenderState, VnState,
};
use crate::systems::save_menu::{record_resume_target, MAX_SLOTS};
use crate::vn_command::VnCommand;
use rvn_core::persistent::LastResumeTarget;
use rvn_core::{error::ScriptError, Interaction};

use crate::systems::debug_overlay::{DebugOverlayState, DebugStepRequest};

pub fn stepping_system(
    mut engine: ResMut<VnEngine>,
    mut next_state: ResMut<NextState<VnState>>,
    mut render_state: ResMut<VnRenderState>,
    mut vn_events: EventWriter<VnCommand>,
    mut error_msg: ResMut<ScriptErrorMessage>,
    debug_state: Res<DebugOverlayState>,
    mut step_req: ResMut<DebugStepRequest>,
    project_paths: Res<ProjectPaths>,
    mut persistent: ResMut<PersistentDataResource>,
) {
    // En mode debug, on ne progresse que si un step a été demandé (touche F10).
    if debug_state.visible {
        if !step_req.pending {
            return;
        } else {
            // Consomme la demande de step
            step_req.pending = false;
        }
    }
    if engine.0.is_finished() {
        vn_events.send(VnCommand::ScriptFinished);
        next_state.set(VnState::Finished);
        return;
    }

    let interaction = match engine.0.step_until_interaction() {
        Ok(interaction) => interaction,
        Err(e) => {
            let script_err = ScriptError::from_runtime(&e);
            error!("{script_err}");
            eprintln!("\n{script_err}");
            error_msg.0 = script_err.to_string();
            next_state.set(VnState::Error);
            return;
        }
    };

    let mut pending = engine.0.renderer.take_pending();
    let wants_to_wait = interaction.is_some();
    let should_autosave = pending
        .iter()
        .any(|cmd| matches!(cmd, VnCommand::SetBackground { .. }))
        || matches!(interaction, Some(Interaction::Choice { .. }));

    if should_autosave {
        match SaveManager::new(&project_paths.saves, MAX_SLOTS as u32) {
            Ok(mgr) => {
                if let Err(e) = mgr.save_autosave(
                    &engine.0.state,
                    "Autosave".to_string(),
                    "script.rvn".to_string(),
                ) {
                    error!("[autosave] échec: {e}");
                } else if let Ok(data) = mgr.load_autosave() {
                    record_resume_target(
                        &mut persistent,
                        LastResumeTarget::autosave(data.timestamp),
                    );
                }
            }
            Err(e) => error!("[autosave] SaveManager indisponible: {e}"),
        }
    }

    if let Some(interaction) = interaction {
        match interaction {
            Interaction::Dialogue { character, text } => {
                pending.push(VnCommand::ShowDialogue { character, text });
            }
            Interaction::Choice { options } => {
                pending.push(VnCommand::ShowChoice { options });
            }
            Interaction::Imagemap {
                background,
                hover_image,
                hotspots,
            } => {
                let hotspots = hotspots
                    .into_iter()
                    .map(|h| (h.name, (h.area.x1, h.area.y1, h.area.x2, h.area.y2)))
                    .collect();
                pending.push(VnCommand::ShowImagemap {
                    background,
                    hover_image,
                    hotspots,
                });
            }
        }
    } else if engine.0.is_finished() {
        vn_events.send(VnCommand::ScriptFinished);
        next_state.set(VnState::Finished);
        return;
    } else {
        next_state.set(VnState::Finished);
        return;
    }

    let has_anim = pending.iter().any(|cmd| {
        matches!(
            cmd,
            VnCommand::SetBackground { transition, .. }
                | VnCommand::ShowSprite { transition, .. }
                | VnCommand::HideSprite { transition, .. }
                | VnCommand::MoveSprite { transition, .. }
            if *transition != Transition::None
        ) || matches!(
            cmd,
            VnCommand::ShowCinematic {
                transition: Some(transition),
                ..
            } | VnCommand::HideCinematic {
                transition: Some(transition)
            } if transition == "fade" || transition == "dissolve"
        )
    });

    for cmd in pending {
        vn_events.send(cmd);
    }

    if has_anim {
        render_state.state_after_anim = Some(if wants_to_wait {
            VnState::Waiting
        } else {
            VnState::Stepping
        });
        next_state.set(VnState::Animating);
    } else if wants_to_wait {
        next_state.set(VnState::Waiting);
    } else {
        next_state.set(VnState::Stepping);
    }
}

pub fn script_finished_system(
    mut vn_events: EventReader<VnCommand>,
    mut next_state: ResMut<NextState<VnState>>,
) {
    for cmd in vn_events.read() {
        if matches!(cmd, VnCommand::ScriptFinished) {
            info!("[moteur] script terminé");
            next_state.set(VnState::Finished);
        }
    }
}
