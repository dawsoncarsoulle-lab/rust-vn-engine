use bevy::color::Srgba;
use bevy::prelude::*;

use crate::components::DialogueText;
use crate::resources::{TypewriterConfig, TypewriterState};
use crate::vn_command::VnCommand;

// Story configuration remains saved; player preferences compose only at presentation.
pub(crate) fn narrative_text_speed(authored: f32, settings: Option<&super::settings_menu::Settings>) -> f32 {
    let enabled = settings.is_none_or(|settings| settings.typewriter);
    if !enabled || authored <= 0.0 {
        return 0.0;
    }
    let multiplier = settings.map(|settings| settings.text_speed).unwrap_or(1.0);
    authored * multiplier
}

pub fn typewriter_config_system(
    mut vn_events: EventReader<VnCommand>,
    mut tw_config: ResMut<TypewriterConfig>,
    mut engine: Option<ResMut<crate::resources::VnEngine>>,
    reveal_binding: Option<Res<crate::display_reveal::DialogueRevealBinding>>,
    settings: Option<Res<super::settings_menu::Settings>>,
    mut tw_state: Option<ResMut<TypewriterState>>,
    mut text_query: Query<&mut Text, With<DialogueText>>,
) {
    for cmd in vn_events.read() {
        if let VnCommand::SetTypewriterConfig { speed_cps } = cmd {
            let speed = if engine.as_ref().is_some_and(|engine| engine.0.state.scene_execution.is_some()) {
                narrative_text_speed(*speed_cps, settings.as_deref())
            } else {
                *speed_cps
            };
            tw_config.enabled = speed > 0.0;
            tw_config.chars_per_sec = speed;
            // Only an active modern reveal consumes this presentation change.
            // Inactive cursors and the historical legacy command policy remain intact.
            if engine.as_ref().is_some_and(|engine| engine.0.state.scene_execution.is_some()) {
                if let Some(typing) = tw_state.as_deref_mut().filter(|typing| typing.typing) {
                    if speed > 0.0 {
                        typing.chars_per_sec = speed;
                    } else {
                        typing.skip();
                        if let Ok(mut text) = text_query.get_single_mut() {
                            apply_visible_sections(&mut text, typing);
                        }
                    }
                    crate::display_reveal::feedback(engine.as_deref_mut(),typing,reveal_binding.as_deref());
                }
            }
        }
    }
}

pub fn typewriter_system(
    time: Res<Time>,
    mut tw_state: ResMut<TypewriterState>,
    mut text_query: Query<&mut Text, With<DialogueText>>,
    accessibility: Res<crate::accessibility::Accessibility>,
    mut engine: Option<ResMut<crate::resources::VnEngine>>,
    binding: Option<Res<crate::display_reveal::DialogueRevealBinding>>,
) {
    if accessibility.blocked {
        return;
    }
    if accessibility.settings.reduced_motion && tw_state.typing {
        // Reduced motion also removes character-by-character/shaking reveal.
        for segment in &mut tw_state.segments {
            segment.shake = false;
        }
        tw_state.skip();
        if let Ok(mut text) = text_query.get_single_mut() {
            apply_visible_sections(&mut text, &tw_state);
        }
        crate::display_reveal::feedback(engine.as_deref_mut(),&tw_state,binding.as_deref());
        return;
    }
    if !tw_state.typing {
        return;
    }

    let total_chars = tw_state.full_text.chars().count();
    let previous_visible = tw_state.visible_chars;
    let mut remaining_dt = time.delta_seconds();

    while remaining_dt > 0.0 && tw_state.visible_chars < total_chars {
        if tw_state.pause_remaining > 0.0 {
            let consumed = tw_state.pause_remaining.min(remaining_dt);
            tw_state.pause_remaining -= consumed;
            remaining_dt -= consumed;
            if tw_state.pause_remaining > 0.0 {
                break;
            }
        }

        let speed = tw_state.speed_for_next_char();
        if speed <= 0.0 {
            break;
        }
        tw_state.char_progress += remaining_dt * speed;
        remaining_dt = 0.0;

        while tw_state.char_progress >= 1.0 && tw_state.visible_chars < total_chars {
            tw_state.visible_chars += 1;
            tw_state.char_progress -= 1.0;

            if let Some(pause) = tw_state.pause_after_visible_char() {
                tw_state.pause_remaining = pause;
                break;
            }
        }
    }

    if tw_state.visible_chars != previous_visible {
        if let Ok(mut text) = text_query.get_single_mut() {
            apply_visible_sections(&mut text, &tw_state);
        }
    }

    if tw_state.visible_chars >= total_chars {
        tw_state.typing = false;
    }
    crate::display_reveal::feedback(engine.as_deref_mut(),&tw_state,binding.as_deref());
}

pub fn apply_visible_sections(text: &mut Text, tw_state: &TypewriterState) {
    let base_style = text
        .sections
        .first()
        .map(|section| section.style.clone())
        .unwrap_or_default();
    text.sections.clear();

    let mut remaining = tw_state.visible_chars;
    for segment in &tw_state.segments {
        if remaining == 0 {
            break;
        }

        let char_count = segment.text.chars().count();
        let visible = remaining.min(char_count);
        remaining -= visible;

        let byte_idx = segment
            .text
            .char_indices()
            .nth(visible)
            .map(|(idx, _)| idx)
            .unwrap_or(segment.text.len());
        let mut value = segment.text[..byte_idx].to_string();
        if segment.shake && !value.is_empty() && tw_state.typing {
            value = shake_text(&value, tw_state.visible_chars);
        }

        let mut style = base_style.clone();
        if let Some(color) = &segment.color {
            style.color = Srgba::hex(color).map(Color::from).unwrap_or(style.color);
        }
        text.sections.push(TextSection { value, style });
    }

    if text.sections.is_empty() {
        text.sections.push(TextSection {
            value: String::new(),
            style: base_style,
        });
    }
}

fn shake_text(value: &str, tick: usize) -> String {
    let prefix = match tick % 3 {
        0 => "",
        1 => " ",
        _ => "",
    };
    format!("{prefix}{value}")
}
