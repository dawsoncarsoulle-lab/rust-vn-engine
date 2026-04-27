use bevy::prelude::*;

use crate::components::DialogueText;
use crate::resources::{TypewriterConfig, TypewriterState};
use crate::vn_command::VnCommand;

pub fn typewriter_config_system(
    mut vn_events: EventReader<VnCommand>,
    mut tw_config: ResMut<TypewriterConfig>,
) {
    for cmd in vn_events.read() {
        if let VnCommand::SetTypewriterConfig { speed_cps } = cmd {
            tw_config.enabled = *speed_cps > 0.0;
            tw_config.chars_per_sec = *speed_cps;
        }
    }
}

pub fn typewriter_system(
    time: Res<Time>,
    mut tw_state: ResMut<TypewriterState>,
    mut text_query: Query<&mut Text, With<DialogueText>>,
) {
    if !tw_state.typing {
        return;
    }

    tw_state.elapsed += time.delta_seconds();

    let target_chars = (tw_state.elapsed * tw_state.chars_per_sec) as usize;
    let total_chars = tw_state.full_text.chars().count();

    if target_chars != tw_state.visible_chars {
        tw_state.visible_chars = target_chars.min(total_chars);

        if let Ok(mut text) = text_query.get_single_mut() {
            text.sections[0].value = tw_state.current_slice().to_string();
        }

        if tw_state.visible_chars >= total_chars {
            tw_state.typing = false;
        }
    }
}
