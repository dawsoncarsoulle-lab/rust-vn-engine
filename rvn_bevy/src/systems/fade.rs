// rvn_bevy/src/systems/fade.rs

use bevy::prelude::*;

use crate::components::{FadeAnim, TransitionKind};
use crate::resources::{VnRenderState, VnState};

/// Slide distance in pixels for slide transitions.
const SLIDE_DISTANCE: f32 = 1920.0;

pub fn fade_system(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut FadeAnim, &mut Sprite, &mut Transform)>,
    mut next_state: ResMut<NextState<VnState>>,
    mut render_state: ResMut<VnRenderState>,
) {
    let mut any_running = false;

    for (entity, mut anim, mut sprite, mut transform) in query.iter_mut() {
        anim.elapsed_secs += time.delta_seconds();
        let t = (anim.elapsed_secs / anim.duration_secs).clamp(0.0, 1.0);
        let alpha = anim.from + (anim.to - anim.from) * t;
        sprite.color.set_alpha(alpha);

        // Apply transform-based effects for non-fade transitions.
        // On fade-in (from=0 → to=1): entity starts offset/zoomed and settles.
        // On fade-out (from=1 → to=0): entity starts normal and drifts out.
        let is_in = anim.to > anim.from;
        let progress = if is_in { 1.0 - t } else { t };
        match anim.kind {
            TransitionKind::SlideLeft => {
                transform.translation.x = SLIDE_DISTANCE * progress;
            }
            TransitionKind::SlideRight => {
                transform.translation.x = -SLIDE_DISTANCE * progress;
            }
            TransitionKind::SlideUp => {
                transform.translation.y = -SLIDE_DISTANCE * progress;
            }
            TransitionKind::SlideDown => {
                transform.translation.y = SLIDE_DISTANCE * progress;
            }
            TransitionKind::ZoomIn => {
                let scale = 1.0 - 0.5 * progress;
                transform.scale = Vec3::splat(scale);
            }
            TransitionKind::ZoomOut => {
                let scale = 1.0 + 0.5 * progress;
                transform.scale = Vec3::splat(scale);
            }
            // Fade, Dissolve, Wipe, Blur use alpha only (no transform change).
            // Wipe and Blur could be enhanced with custom shaders in the future.
            _ => {}
        }

        if t >= 1.0 {
            // Reset transform to identity position/scale on completion.
            if !matches!(anim.kind, TransitionKind::Fade | TransitionKind::Dissolve) {
                transform.scale = Vec3::splat(1.0);
            }
            commands.entity(entity).remove::<FadeAnim>();
            if anim.despawn_on_finish {
                commands.entity(entity).despawn();
            }
        } else {
            any_running = true;
        }
    }

    if !any_running {
        let next = render_state
            .state_after_anim
            .take()
            .unwrap_or(VnState::Stepping);
        next_state.set(next);
    }
}
