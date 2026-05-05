use bevy::color::Srgba;
use bevy::prelude::*;

use super::CHOICE_MARGIN_ABOVE_BOX;
use crate::components::{ChoiceButton, ChoiceContainer, DialogueBox};
use crate::resources::{ChoiceFocus, Theme, VnRenderState};
use crate::vn_command::{PlayerInput, VnCommand};

pub fn choice_system(
    mut vn_events: EventReader<VnCommand>,
    mut render_state: ResMut<VnRenderState>,
    mut dialogue_box_query: Query<&mut Style, With<DialogueBox>>,
) {
    for cmd in vn_events.read() {
        let VnCommand::ShowChoice { options } = cmd else {
            continue;
        };
        if let Some(mut style) = dialogue_box_query.iter_mut().next() {
            style.display = Display::Flex;
        }
        render_state.choice_options = options.clone();
    }
}

pub fn update_choice_buttons(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    render_state: Res<VnRenderState>,
    theme: Res<Theme>,
    choice_focus: Res<ChoiceFocus>,
    existing: Query<Entity, With<ChoiceButton>>,
    container_query: Query<Entity, With<ChoiceContainer>>,
) {
    if !render_state.is_changed() && !theme.is_changed() && !choice_focus.is_changed() {
        return;
    }
    for entity in existing.iter() {
        commands.entity(entity).despawn_recursive();
    }
    if render_state.choice_options.is_empty() {
        return;
    }
    let Ok(container) = container_query.get_single() else {
        return;
    };

    let btn_w = 640.0_f32;
    let btn_h = 52.0_f32;
    let gap = 10.0_f32;
    let base_bottom = theme.textbox.height + CHOICE_MARGIN_ABOVE_BOX;

    // Couleurs normales
    let bg_normal = Srgba::hex(&theme.choice.background_color)
        .map(Color::from)
        .unwrap_or(Color::srgba(0.05, 0.05, 0.18, 0.95));
    // Couleur focus : légèrement plus clair + saturé
    let bg_focus = Color::srgba(0.20, 0.20, 0.50, 0.98);
    let num_color = Srgba::hex(&theme.choice.number_color)
        .map(Color::from)
        .unwrap_or(Color::WHITE);
    let txt_color = Srgba::hex(&theme.choice.text_color)
        .map(Color::from)
        .unwrap_or(Color::WHITE);

    let choice_font: Handle<Font> = theme
        .choice
        .font_path
        .as_ref()
        .map(|p| asset_server.load(p.clone()))
        .unwrap_or_default();

    for (i, label) in render_state.choice_options.iter().enumerate() {
        let bottom = base_bottom + i as f32 * (btn_h + gap);
        let focused = choice_focus.0 == Some(i);
        let bg_color = if focused { bg_focus } else { bg_normal };

        let btn = commands
            .spawn((
                ButtonBundle {
                    style: Style {
                        position_type: PositionType::Absolute,
                        width: Val::Px(btn_w),
                        height: Val::Px(btn_h),
                        left: Val::Percent(50.0),
                        bottom: Val::Px(bottom),
                        margin: UiRect::left(Val::Px(-btn_w / 2.0)),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        padding: UiRect::horizontal(Val::Px(22.0)),
                        column_gap: Val::Px(4.0),
                        border: if focused {
                            UiRect::all(Val::Px(2.0))
                        } else {
                            UiRect::all(Val::Px(0.0))
                        },
                        ..default()
                    },
                    background_color: bg_color.into(),
                    border_color: if focused {
                        Color::srgba(0.7, 0.7, 1.0, 1.0).into()
                    } else {
                        Color::NONE.into()
                    },
                    ..default()
                },
                ChoiceButton(i),
            ))
            .with_children(|p| {
                p.spawn(TextBundle::from_section(
                    format!("{}. ", i + 1),
                    TextStyle {
                        font: choice_font.clone(),
                        font_size: theme.choice.font_size,
                        color: num_color,
                    },
                ));
                p.spawn(TextBundle::from_section(
                    label.clone(),
                    TextStyle {
                        font: choice_font.clone(),
                        font_size: theme.choice.font_size,
                        color: txt_color,
                    },
                ));
            })
            .id();
        commands.entity(container).add_child(btn);
    }
}

pub fn choice_interaction_system(
    mut interaction_query: Query<
        (&Interaction, &ChoiceButton),
        (Changed<Interaction>, With<Button>, With<ChoiceButton>),
    >,
    mut choice_focus: ResMut<ChoiceFocus>,
    mut player_events: EventWriter<PlayerInput>,
) {
    for (interaction, choice_button) in interaction_query.iter_mut() {
        if *interaction == Interaction::Pressed {
            choice_focus.clear();
            player_events.send(PlayerInput::Choose(choice_button.0));
        }
    }
}
