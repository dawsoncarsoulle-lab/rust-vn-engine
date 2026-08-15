use bevy::prelude::*;
use rvn_core::parse_text_tags;

use crate::components::{CharacterNameText, ChoiceButton, DialogueBox, DialogueText};
use crate::resources::{CharacterRegistry, DialogueHistory, TypewriterConfig, TypewriterState};
use crate::systems::typewriter::apply_visible_sections;
use crate::vn_command::VnCommand;

pub fn dialogue_system(
    mut vn_events: EventReader<VnCommand>,
    tw_config: Res<TypewriterConfig>,
    mut tw_state: ResMut<TypewriterState>,
    registry: Res<CharacterRegistry>,
    mut history: ResMut<DialogueHistory>,
    mut dialogue_box_query: Query<&mut Style, With<DialogueBox>>,
    mut name_query: Query<&mut Text, (With<CharacterNameText>, Without<DialogueText>)>,
    mut text_query: Query<&mut Text, (With<DialogueText>, Without<CharacterNameText>)>,
    mut commands: Commands,
    choice_buttons: Query<Entity, With<ChoiceButton>>,
) {
    for cmd in vn_events.read() {
        let VnCommand::ShowDialogue { character, text } = cmd else {
            continue;
        };

        for entity in choice_buttons.iter() {
            commands.entity(entity).despawn_recursive();
        }

        if let Some(mut style) = dialogue_box_query.iter_mut().next() {
            style.display = Display::Flex;
        }

        if let Some(mut t) = name_query.iter_mut().next() {
            let display = match character.as_deref() {
                None => "",
                Some(id) => registry.display_name(id),
            };
            t.sections[0].value = display.to_string();
        }

        let display_name_for_history = match character.as_deref() {
            None => "".to_string(),
            Some(id) => registry.display_name(id).to_string(),
        };
        let rich_text = match parse_text_tags(text) {
            Ok(rich_text) => rich_text,
            Err(e) => {
                error!("[text_tags] {e}");
                parse_text_tags(&escape_as_plain_text(text)).unwrap_or_else(|_| {
                    rvn_core::RichText {
                        segments: vec![rvn_core::RichTextSegment {
                            text: text.clone(),
                            color: None,
                            speed: None,
                            shake: false,
                            pause_after: None,
                            bold: false,
                            italic: false,
                            underline: false,
                            size: None,
                            alpha: None,
                        }],
                    }
                })
            }
        };
        history.add(display_name_for_history, rich_text.plain_text());

        if tw_config.enabled && tw_config.chars_per_sec > 0.0 {
            tw_state.start_segments(rich_text.segments, tw_config.chars_per_sec);
            if let Some(mut t) = text_query.iter_mut().next() {
                apply_visible_sections(&mut t, &tw_state);
            }
        } else {
            tw_state.start_segments(rich_text.segments, 0.0);
            if let Some(mut t) = text_query.iter_mut().next() {
                apply_visible_sections(&mut t, &tw_state);
            }
        }
    }
}

fn escape_as_plain_text(text: &str) -> String {
    text.replace('{', "{{").replace('}', "}}")
}
