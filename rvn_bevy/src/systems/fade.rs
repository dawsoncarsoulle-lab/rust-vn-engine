// rvn_bevy/src/systems/fade.rs

use bevy::prelude::*;

use crate::components::{FadeAnim, TransitionKind};
use crate::resources::{VnRenderState, VnState};

/// Slide distance in pixels for slide transitions.
const SLIDE_DISTANCE: f32 = 1920.0;

pub fn fade_system(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut FadeAnim, &mut Sprite, &mut Transform,Option<&crate::scene_effects::FadeLease>,Option<&crate::scene_effects::OwnedFadeBase>),Without<crate::scene_retired::RetiredProjection>>,
    mut next_state: ResMut<NextState<VnState>>,
    mut render_state: ResMut<VnRenderState>,
    layered: Query<(), (With<crate::layered_characters::LayeredFade>,Without<crate::scene_retired::RetiredProjection>)>,
    accessibility: Res<crate::accessibility::Accessibility>,
    engine:Option<Res<crate::resources::VnEngine>>,
    state:Option<Res<State<VnState>>>,
) {
    if accessibility.blocked {
        return;
    }
    let mut any_running = false;

    for (entity, mut anim, mut sprite, mut transform,lease,base) in query.iter_mut() {
        // Narrative effects are sampled after the single Core frame. This
        // legacy system owns neither their elapsed nor their completion.
        if lease.is_some() {any_running=true;continue;}
        if !crate::scene_effects::allowed(engine.as_deref(),state.as_deref(),lease.map(|lease|&lease.0)) {any_running=true;continue;}
        anim.elapsed_secs += time.delta_seconds();
        let t = if accessibility.settings.reduced_motion {
            1.0
        } else {
            (anim.elapsed_secs / anim.duration_secs).clamp(0.0, 1.0)
        };
        let alpha = anim.from + (anim.to - anim.from) * t;
        sprite.color.set_alpha(base.map(|base|base.alpha).unwrap_or(1.0)*alpha);

        // Apply transform-based effects for non-fade transitions.
        // On fade-in (from=0 → to=1): entity starts offset/zoomed and settles.
        // On fade-out (from=1 → to=0): entity starts normal and drifts out.
        let is_in = anim.to > anim.from;
        let progress = if is_in { 1.0 - t } else { t };
        if let Some(base)=base {crate::scene_effects::apply_visual_pose(&anim,&base.transform,&mut transform,t);} else {match anim.kind {
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
        }}

        if t >= 1.0 {
            // Reset transform to identity position/scale on completion.
            if base.is_none() && !matches!(anim.kind, TransitionKind::Fade | TransitionKind::Dissolve) {
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

    if !any_running && layered.is_empty() {
        let next = render_state
            .state_after_anim
            .take()
            .unwrap_or(VnState::Stepping);
        next_state.set(next);
    }
}
