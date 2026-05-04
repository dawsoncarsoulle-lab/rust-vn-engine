use bevy::prelude::*;

use crate::resources::PersistentDataResource;
use crate::vn_command::VnCommand;

pub fn persistent_unlock_system(
    mut persistent: ResMut<PersistentDataResource>,
    mut vn_events: EventReader<VnCommand>,
) {
    for cmd in vn_events.read() {
        match cmd {
            VnCommand::ShowCinematic { id, .. } => {
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
}
