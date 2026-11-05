use bevy::prelude::*;

use crate::resources::PersistentDataResource;
use crate::vn_command::VnCommand;

// Keep authored progression in the existing store after accepted publication.
pub fn persistent_unlock_system(
    mut persistent: ResMut<PersistentDataResource>,
    mut vn_events: EventReader<VnCommand>,
    engine: Option<Res<crate::resources::VnEngine>>,
    mut error: Option<ResMut<crate::resources::ScriptErrorMessage>>,
    mut next: Option<ResMut<NextState<crate::resources::VnState>>>,
    state: Option<Res<State<crate::resources::VnState>>>,
    requests: Option<Res<crate::checkpoint_requests::CheckpointRequests>>,
) {
    for cmd in vn_events.read() {
        match cmd {
            VnCommand::ShowCinematic { id, .. } | VnCommand::UnlockCinematic { id } => {
                let manager = persistent.manager.clone();
                if let Err(e) = manager.mark_cg_seen(&mut persistent.data, id) {
                    error!("[persistent] impossible de marquer la CG `{id}` : {e}");
                }
            }
            VnCommand::UnlockEnding { id } => {
                let manager = persistent.manager.clone();
                if let Err(e) = manager.mark_ending_seen(&mut persistent.data, id) {
                    error!("[persistent] impossible de marquer l'ending `{id}` : {e}");
                }
            }
            _ => {}
        }
    }
    let Some(engine) = engine else { return; };
    // A failed store write stays visible until an explicit error recovery.
    // Checkpoint preparation cannot publish a different owner concurrently.
    if state.as_ref().is_some_and(|state| *state.get() == crate::resources::VnState::Error)
        || next.as_ref().is_some_and(|next| matches!(&**next, NextState::Pending(crate::resources::VnState::Error)))
        || requests.as_ref().is_some_and(|requests| requests.blocked())
    { return; }
    if engine.0.renderer.scene_batch_pending() || engine.0.renderer.scene_frame_failed {
        return;
    }
    let vars = engine.0.export_persistent_vars();
    if vars == &persistent.data.script_vars {
        return;
    }
    let mut candidate = persistent.data.clone();
    candidate.script_vars = vars.clone();
    match persistent.manager.save(&candidate) {
        Ok(()) => persistent.data = candidate,
        Err(failure) => {
            let message = format!("[persistent] impossible d’enregistrer les variables : {failure}");
            error!("{message}");
            if let Some(error) = error.as_deref_mut() { error.0 = message; }
            if let Some(next) = next.as_deref_mut() { next.set(crate::resources::VnState::Error); }
        }
    }
}
