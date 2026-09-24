//! Text state colors do not mutate saved styles or rich narrative text.
use super::*;
#[derive(Component)]
pub(super) struct TextAppearance {
    normal: rvn_ui::Color,
    states: rvn_ui::TextStateColors,
    enabled: bool,
    selected: bool,
}
impl TextAppearance {
    pub(super) fn new(e: &Element, selected: bool) -> Self {
        Self {
            normal: e.foreground,
            states: e.appearance.text_states.clone(),
            enabled: e.enabled,
            selected,
        }
    }
}
pub(super) fn update(
    mut texts: Query<(Entity, &TextAppearance, &mut Text)>,
    parents: Query<&Parent>,
    interactive: Query<(
        Option<&Interaction>,
        Option<&Outline>,
        Option<&crate::components::ChoiceButton>,
    )>,
    choices: Res<ChoiceFocus>,
) {
    for (entity, appearance, mut text) in &mut texts {
        let mut node = entity;
        let mut pressed = false;
        let mut hover = false;
        let mut focus = false;
        for _ in 0..64 {
            let Ok(parent) = parents.get(node) else { break };
            node = parent.get();
            if let Ok((interaction, outline, choice)) = interactive.get(node) {
                focus |= outline.is_some() || choice.is_some_and(|c| choices.0 == Some(c.0));
                if let Some(interaction) = interaction {
                    pressed = *interaction == Interaction::Pressed;
                    hover = *interaction == Interaction::Hovered;
                    break;
                }
            }
        }
        let value = color(appearance.states.resolve(
            appearance.normal,
            appearance.enabled,
            pressed,
            hover,
            focus,
            appearance.selected,
        ));
        if text
            .sections
            .iter()
            .any(|section| section.style.color != value)
        {
            for section in &mut text.sections {
                section.style.color = value;
            }
        }
    }
}
