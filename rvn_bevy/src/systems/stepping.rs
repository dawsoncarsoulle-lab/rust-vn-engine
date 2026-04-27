use bevy::prelude::*;
use rvn_parser::Transition;

use crate::resources::{ScriptErrorMessage, VnEngine, VnRenderState, VnState};
use crate::vn_command::VnCommand;
use rvn_core::error::ScriptError;

use crate::systems::debug_overlay::{DebugOverlayState, DebugStepRequest};

pub fn stepping_system(
    mut engine: ResMut<VnEngine>,
    mut next_state: ResMut<NextState<VnState>>,
    mut render_state: ResMut<VnRenderState>,
    mut vn_events: EventWriter<VnCommand>,
    mut error_msg: ResMut<ScriptErrorMessage>,
    debug_state: Res<DebugOverlayState>,
    mut step_req: ResMut<DebugStepRequest>,
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

    if let Err(e) = engine.0.step_silent() {
        let script_err = ScriptError::from_runtime(&e);
        error!("{script_err}");
        eprintln!("\n{script_err}");
        error_msg.0 = script_err.to_string();
        next_state.set(VnState::Error);
        return;
    }

    if engine.0.is_finished() {
        for cmd in engine.0.renderer.take_pending() {
            vn_events.send(cmd);
        }
        vn_events.send(VnCommand::ScriptFinished);
        next_state.set(VnState::Finished);
        return;
    }

    let wants_to_wait = match engine.0.peek_statement().cloned() {
        Some(rvn_parser::Statement::Dialogue { .. }) => {
            if let Err(e) = engine.0.step() {
                let script_err = ScriptError::from_runtime(&e);
                error!("{script_err}");
                eprintln!("\n{script_err}");
                error_msg.0 = script_err.to_string();
                next_state.set(VnState::Error);
                return;
            }
            true
        }

        Some(rvn_parser::Statement::Choice { options }) => {
            if engine.0.renderer.choice_result.is_some() {
                if let Err(e) = engine.0.step() {
                    let script_err = ScriptError::from_runtime(&e);
                    error!("{script_err}");
                    eprintln!("\n{script_err}");
                    error_msg.0 = script_err.to_string();
                    next_state.set(VnState::Error);
                    return;
                }
                engine
                    .0
                    .renderer
                    .pending
                    .iter()
                    .any(|c| matches!(c, VnCommand::ShowDialogue { .. }))
            } else {
                let labels: Vec<String> = options
                    .into_iter()
                    .map(|(label, _)| label.to_string())
                    .collect();
                engine
                    .0
                    .renderer
                    .pending
                    .push(VnCommand::ShowChoice { options: labels });
                true
            }
        }

        Some(rvn_parser::Statement::Imagemap {
            background,
            hover_image,
            hotspots,
        }) => {
            if engine.0.renderer.choice_result.is_some() {
                if let Err(e) = engine.0.step() {
                    let script_err = ScriptError::from_runtime(&e);
                    error!("{script_err}");
                    eprintln!("\n{script_err}");
                    error_msg.0 = script_err.to_string();
                    next_state.set(VnState::Error);
                    return;
                }
                engine
                    .0
                    .renderer
                    .pending
                    .iter()
                    .any(|c| matches!(c, VnCommand::ShowDialogue { .. }))
            } else {
                let hs = hotspots
                    .into_iter()
                    .map(|h| (h.name, (h.area.x1, h.area.y1, h.area.x2, h.area.y2)))
                    .collect();
                engine.0.renderer.pending.push(VnCommand::ShowImagemap {
                    background,
                    hover_image,
                    hotspots: hs,
                });
                true
            }
        }

        _ => {
            next_state.set(VnState::Finished);
            return;
        }
    };

    let pending = engine.0.renderer.take_pending();

    let has_anim = pending.iter().any(|cmd| {
        matches!(
            cmd,
            VnCommand::SetBackground { transition, .. }
                | VnCommand::ShowSprite { transition, .. }
                | VnCommand::HideSprite { transition, .. }
                | VnCommand::MoveSprite { transition, .. }
            if *transition != Transition::None
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
