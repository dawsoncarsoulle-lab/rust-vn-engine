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
    mut engine: Option<ResMut<crate::resources::VnEngine>>,
    mut queue: Option<ResMut<crate::display_reveal::DialogueRevealQueue>>,
    mut binding: Option<ResMut<crate::display_reveal::DialogueRevealBinding>>,
    mut presented: Option<ResMut<Events<crate::display_reveal::PresentedDialogue>>>,
) {
    let mut pending = if let Some(queue)=queue.as_deref_mut() {
        vn_events.clear();
        if engine.as_ref().is_some_and(|engine|engine.0.renderer.scene_batch_pending() || engine.0.renderer.scene_frame_failed) { return; }
        std::mem::take(&mut queue.commands)
    } else { vn_events.read().cloned().collect::<std::collections::VecDeque<_>>() };
    while let Some(cmd)=pending.front().cloned() {
        let payload=crate::display_reveal::payload(&cmd,engine.as_deref());
        let (character,text,source,checkpoint)=match payload {
            Ok(Some(payload))=>payload,
            Ok(None)=>{pending.pop_front();continue;},
            Err(error)=>{
                if let Some(engine)=engine.as_deref_mut(){engine.0.renderer.scene_frame_failed=true;}
                error!("Dialogue reveal refused before presentation: {error}");break;
            }
        };
        let rich_text=rvn_core::display_reveal::rendered_rich_text(text);
        // A scene publication may reset the cursor after a locale-only redraw.
        // The retained binding alone does not prove that this text is installed.
        if binding.as_ref().is_some_and(|binding|binding.source.as_ref()==source && source.is_some() && binding.text==*text)
            && tw_state.full_text==rich_text.plain_text() {
            pending.pop_front();continue;
        }
        let same_display=binding.as_ref().is_some_and(|binding|binding.source.as_ref()==source && source.is_some());
        let mut next=TypewriterState::default();
        next.start_segments(rich_text.segments.clone(),if tw_config.enabled {tw_config.chars_per_sec} else {0.0});
        if let Err(error)=crate::display_reveal::restore(&mut next,text,checkpoint) {
            if let Some(engine)=engine.as_deref_mut(){engine.0.renderer.scene_frame_failed=true;}
            error!("Dialogue checkpoint restore refused before presentation: {error}");break;
        }
        let next_binding=crate::display_reveal::DialogueRevealBinding {source:source.cloned(),text:text.clone()};
        if let Some(engine)=engine.as_deref_mut() {
            if let Err(error)=crate::display_reveal::flush(engine,&next,Some(&next_binding)) {
                engine.0.renderer.scene_frame_failed=true;error!("Dialogue checkpoint refused: {error}");break;
            }
        }

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
        if !same_display { history.add(display_name_for_history, rich_text.plain_text()); }

        *tw_state=next;
        if let Some(mut t)=text_query.iter_mut().next() {apply_visible_sections(&mut t,&tw_state);}
        if let Some(source)=source {
            if let Some(presented)=presented.as_deref_mut(){presented.send(crate::display_reveal::PresentedDialogue {source:source.clone(),character:character.clone(),text:text.clone()});}
        }
        if let Some(binding)=binding.as_deref_mut(){*binding=next_binding;}
        pending.pop_front();
    }
    if let Some(queue)=queue.as_deref_mut(){queue.commands=pending;}
}

fn escape_as_plain_text(text: &str) -> String {
    text.replace('{', "{{").replace('}', "}}")
}

// Prepare from the current engine's recorded snapshot before any ECS replacement.
// A choice backdrop is already completed text, not a live reveal authority.
pub(crate) struct ChoiceDialogueBackdrop {
    character: Option<String>,
    typing: TypewriterState,
    present: bool,
}

pub(crate) fn prepare_choice_backdrop(
    engine: &crate::resources::VnEngine,
) -> Result<ChoiceDialogueBackdrop, String> {
    let recorded = engine.0.last_dialogue_interaction().map_err(|error| error.to_string())?;
    prepare_choice_backdrop_from_interaction(recorded)
}

pub(crate) fn prepare_choice_backdrop_from_interaction(recorded:Option<rvn_core::Interaction>) -> Result<ChoiceDialogueBackdrop,String> {
    let (character, text, present) = match recorded {
        Some(rvn_core::Interaction::Dialogue { character, text }) => (character, text, true),
        None => (None, String::new(), false),
        _ => return Err("The recorded choice backdrop must be a dialogue".into()),
    };
    if text.len() > rvn_core::display_reveal::MAX_REVEAL_TEXT_BYTES {
        return Err("Choice dialogue backdrop exceeds the portable text budget".into());
    }
    let rich_text = rvn_core::display_reveal::rendered_rich_text(&text);
    let mut typing = TypewriterState::default();
    typing.start_segments(rich_text.segments, 0.0);
    Ok(ChoiceDialogueBackdrop { character, typing, present })
}

// Content installed at the last successful choice-backdrop commit. It is
// transient presentation state; never a reveal authority or save identity.
#[derive(Resource, Clone, PartialEq)]
struct InstalledChoiceBackdrop {
    name: String,
    present: bool,
    segments: Vec<rvn_core::RichTextSegment>,
}
fn choice_backdrop_content(world: &World, backdrop: &ChoiceDialogueBackdrop) -> InstalledChoiceBackdrop {
    let name = backdrop.character.as_deref().map(|id| {
        world.get_resource::<CharacterRegistry>().map(|registry| registry.display_name(id).to_owned())
            .unwrap_or_else(|| id.to_owned())
    }).unwrap_or_default();
    InstalledChoiceBackdrop { name, present: backdrop.present, segments: backdrop.typing.segments.clone() }
}
pub(crate) fn choice_backdrop_changed(world: &World, backdrop: &ChoiceDialogueBackdrop) -> bool {
    world.get_resource::<InstalledChoiceBackdrop>().is_none_or(|installed|
        *installed != choice_backdrop_content(world, backdrop))
}
pub(crate) fn clear_choice_backdrop_content(world: &mut World) {
    world.remove_resource::<InstalledChoiceBackdrop>();
}
pub(crate) fn install_choice_backdrop(world: &mut World, backdrop: ChoiceDialogueBackdrop) {
    let installed = choice_backdrop_content(world, &backdrop);
    let name = backdrop.character.as_deref().map(|id| {
        world.get_resource::<CharacterRegistry>().map(|registry| registry.display_name(id).to_owned())
            .unwrap_or_else(|| id.to_owned())
    }).unwrap_or_default();
    for mut style in world.query_filtered::<&mut Style, With<DialogueBox>>().iter_mut(world) {
        style.display = if backdrop.present { Display::Flex } else { Display::None };
    }
    for mut text in world.query_filtered::<&mut Text, (With<CharacterNameText>, Without<DialogueText>)>().iter_mut(world) {
        if let Some(section) = text.sections.first_mut() { section.value.clone_from(&name); }
    }
    for mut text in world.query_filtered::<&mut Text, (With<DialogueText>, Without<CharacterNameText>)>().iter_mut(world) {
        apply_visible_sections(&mut text, &backdrop.typing);
    }
    if let Some(mut accessibility) = world.get_resource_mut::<crate::accessibility::Accessibility>() {
        accessibility.retain_dialogue_backdrop(&name, if backdrop.present { &backdrop.typing.full_text } else { "" });
    }
    if backdrop.present {
        if let Some(mut history) = world.get_resource_mut::<DialogueHistory>() {
            history.add(name, backdrop.typing.full_text.clone());
        }
    }
    if let Some(mut binding) = world.get_resource_mut::<crate::display_reveal::DialogueRevealBinding>() {
        *binding = default();
    }
    if let Some(mut typing) = world.get_resource_mut::<TypewriterState>() { *typing = backdrop.typing; }
    world.insert_resource(installed);
}
