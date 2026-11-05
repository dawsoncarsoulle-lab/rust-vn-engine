use bevy::color::Srgba;
use bevy::prelude::*;

use crate::components::{CharacterNameText, ChoiceContainer, DialogueBox, DialogueText};
use crate::resources::{CharacterRegistry, Theme, VnEngine};

pub fn setup_ui(mut commands: Commands, theme: Res<Theme>, asset_server: Res<AssetServer>) {
    let name_font: Handle<Font> = theme
        .text
        .name
        .font_path
        .as_ref()
        .map(|p| asset_server.load(p.clone()))
        .unwrap_or_default();

    let dialogue_font: Handle<Font> = theme
        .text
        .dialogue
        .font_path
        .as_ref()
        .map(|p| asset_server.load(p.clone()))
        .unwrap_or_default();

    let textbox_bg = Srgba::hex(&theme.textbox.background_color)
        .map(Color::from)
        .unwrap_or(Color::srgba(0.0, 0.0, 0.0, 0.85));

    let textbox_image = theme
        .textbox
        .image_path
        .as_ref()
        .map(|p| UiImage::new(asset_server.load::<Image>(p.clone())));

    let mut textbox_entity = commands.spawn((
        NodeBundle {
            style: Style {
                position_type: PositionType::Absolute,
                bottom: Val::Px(0.0),
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                height: Val::Px(theme.textbox.height),
                padding: UiRect::all(Val::Px(theme.textbox.padding)),
                flex_direction: FlexDirection::Column,
                display: Display::None,
                ..default()
            },
            background_color: textbox_bg.into(),
            ..default()
        },
        DialogueBox,
    ));
    if let Some(img) = textbox_image {
        textbox_entity.insert(img);
    }
    textbox_entity.with_children(|parent| {
        parent.spawn((
            TextBundle::from_section(
                "",
                TextStyle {
                    font: name_font,
                    font_size: theme.text.name.font_size,
                    color: Srgba::hex(&theme.text.name.color)
                        .map(Color::from)
                        .unwrap_or(Color::WHITE),
                },
            ),
            CharacterNameText,
        ));
        parent.spawn((
            TextBundle::from_section(
                "",
                TextStyle {
                    font: dialogue_font,
                    font_size: theme.text.dialogue.font_size,
                    color: Srgba::hex(&theme.text.dialogue.color)
                        .map(Color::from)
                        .unwrap_or(Color::WHITE),
                },
            ),
            DialogueText,
        ));
    });

    commands.spawn((
        NodeBundle {
            style: Style {
                position_type: PositionType::Absolute,
                bottom: Val::Px(0.0),
                left: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            background_color: Color::NONE.into(),
            ..default()
        },
        ChoiceContainer,
    ));
}

/// Data-only projection of an already published Core state. Call only after
/// the real Core/World publication fence has accepted the candidate.
pub fn install_character_names(world: &mut World, state: &rvn_core::GameState) {
    world.init_resource::<CharacterRegistry>();
    world.resource_mut::<CharacterRegistry>().project_character_state(&state.layered);
}

pub fn build_character_registry(engine: Option<Res<VnEngine>>, mut registry: ResMut<CharacterRegistry>) {
    if let Some(engine) = engine { registry.project_character_state(&engine.0.state.layered); }
    else { registry.0.clear(); }
}

/// Historical Game driver: schedule before dialogue/history/accessibility.
/// Startup has no VnEngine; its installer supplies the accepted batch instead.
pub fn synchronize_character_registry(engine: Option<Res<VnEngine>>, mut registry: ResMut<CharacterRegistry>) {
    if let Some(engine) = engine {
        // Core can be ahead of an unaccepted World publication. Its candidate
        // names cannot relabel the old display or history before installation.
        if !engine.0.renderer.scene_batch_pending() && !engine.0.renderer.scene_frame_failed {
            registry.project_character_state(&engine.0.state.layered);
        }
    }
}
