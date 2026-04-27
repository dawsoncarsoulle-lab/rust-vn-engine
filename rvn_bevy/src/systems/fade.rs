// rvn_bevy/src/systems/fade.rs

use bevy::prelude::*;

use crate::components::FadeAnim;
use crate::resources::{VnRenderState, VnState};

pub fn fade_system(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut FadeAnim, &mut Sprite)>,
    mut next_state: ResMut<NextState<VnState>>,
    mut render_state: ResMut<VnRenderState>,
) {
    let mut any_running = false;

    for (entity, mut anim, mut sprite) in query.iter_mut() {
        anim.elapsed_secs += time.delta_seconds();
        let t = (anim.elapsed_secs / anim.duration_secs).clamp(0.0, 1.0);
        let alpha = anim.from + (anim.to - anim.from) * t;
        sprite.color.set_alpha(alpha);

        if t >= 1.0 {
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
