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

pub fn build_character_registry(engine: Res<VnEngine>, mut registry: ResMut<CharacterRegistry>) {
    fn scan(stmts: &[rvn_parser::Statement], registry: &mut CharacterRegistry) {
        for stmt in stmts {
            match stmt {
                rvn_parser::Statement::CharacterCreate { id, display_name } => {
                    registry.0.insert(id.clone(), display_name.clone());
                    info!(
                        "[registry] personnage enregistré : {} → \"{}\"",
                        id, display_name
                    );
                }
                rvn_parser::Statement::Init { body } => scan(body, registry),
                _ => {}
            }
        }
    }

    scan(&engine.0.script, &mut registry);
    info!(
        "[registry] {} personnage(s) enregistré(s)",
        registry.0.len()
    );
}
