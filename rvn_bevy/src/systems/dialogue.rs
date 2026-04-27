use bevy::prelude::*;

use crate::components::{CharacterNameText, ChoiceButton, DialogueBox, DialogueText};
use crate::resources::{CharacterRegistry, DialogueHistory, TypewriterConfig, TypewriterState};
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
        history.add(display_name_for_history, text.clone());

        if tw_config.enabled && tw_config.chars_per_sec > 0.0 {
            tw_state.start(text.clone(), tw_config.chars_per_sec);
        } else {
            tw_state.start(text.clone(), 0.0);
            if let Some(mut t) = text_query.iter_mut().next() {
                t.sections[0].value = text.clone();
            }
        }
    }
}
