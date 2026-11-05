//! PRIVATE: the authored CG slot reuses the narrative candidate publication.
use super::*;

impl<R: Renderer> Engine<R> {
    pub(super) fn exec_owned_cinematic(
        &mut self,
        id: Option<String>,
        authored_transition: Option<String>,
    ) -> Result<(), RuntimeError> {
        use crate::retired_presentations::{snapshot_retired_source, PresentationSlot};
        use crate::scene_resources::{CancellationReason, TargetKind};
        use crate::scene_tasks::TaskOwner;
        if let Some(id) = &id {
            if !rvn_ui::programmable::safe_asset_path(id) {
                return Err(self.scene_resource_error("Invalid CG asset reference".into()));
            }
        }
        // Keep the existing authored names and unknown-name immediate behavior.
        let transition = match authored_transition.as_deref() {
            Some("fade" | "dissolve") => Transition::Fade {
                duration_ms: Transition::DEFAULT_FADE_MS,
            },
            _ => Transition::None,
        };
        let slot = PresentationSlot::Cinematic;
        let source = snapshot_retired_source(&self.state, &slot)
            .map_err(|message| self.scene_resource_error(message))?;
        let mut next = self.state.clone();
        // A repeated Hide has no new source and must not rewind its old exit.
        if id.is_some() || source.is_some() {
            crate::scene_resources::cancel_retired_story_slot(
                &mut next, &slot, CancellationReason::TargetReplaced,
            ).map_err(|message| self.scene_resource_error(message))?;
        }
        // Resource helpers validate the completed candidate. The desired slot
        // must already match the replacement/removal when that helper returns.
        next.cinematic.current = id.clone();
        next.cinematic.transition = authored_transition;
        if id.is_some() {
            crate::scene_resources::replace_target(
                &mut next, TaskOwner::Game, TargetKind::Cinematic, "cinematic",
            ).map_err(|message| self.scene_resource_error(message))?;
        } else if source.is_some() {
            crate::scene_resources::remove_target(
                &mut next, &TaskOwner::Game, TargetKind::Cinematic, "cinematic",
            ).map_err(|message| self.scene_resource_error(message))?;
        }
        self.plan_retired_transition(&mut next, source, &transition)?;
        if next.cinematic.current.is_some() {
            self.plan_incoming_transition(&mut next, &slot, &transition)?;
        }
        next.pc += 1;
        self.commit_scene_state(next)
    }
}
