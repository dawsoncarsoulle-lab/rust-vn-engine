use bevy::prelude::*;

use crate::components::{CharacterNameText, DialogueBox, DialogueText};
use crate::resources::{Theme, ThemeWatcher};

pub fn theme_reload_system(mut theme: ResMut<Theme>, mut watcher: ResMut<ThemeWatcher>) {
    // Only attempt to reload the theme when the file on disk has been modified.
    // We use the path stored in the ThemeWatcher resource rather than a hard‑coded
    // relative path so that the engine can run from arbitrary project directories.
    if let Ok(metadata) = std::fs::metadata(&watcher.path) {
        if let Ok(modified) = metadata.modified() {
            if modified > watcher.last_modified {
                watcher.last_modified = modified;
                if let Ok(content) = std::fs::read_to_string(&watcher.path) {
                    if let Ok(new_theme) = toml::from_str::<Theme>(&content) {
                        *theme = new_theme;
                        info!("[Theme] Hot‑reloaded avec succès !");
                    }
                }
            }
        }
    }
}

pub fn apply_theme_system(
    theme: Res<Theme>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
    mut box_query: Query<(Entity, &mut BackgroundColor, &mut Style), With<DialogueBox>>,
    mut name_query: Query<&mut Text, (With<CharacterNameText>, Without<DialogueText>)>,
    mut text_query: Query<&mut Text, (With<DialogueText>, Without<CharacterNameText>)>,
) {
    if !theme.is_changed() {
        return;
    }

    if let Ok((entity, mut bg, mut style)) = box_query.get_single_mut() {
        *bg = bevy::color::Srgba::hex(&theme.textbox.background_color)
            .map(Color::from)
            .unwrap_or(Color::srgba(0.0, 0.0, 0.0, 0.85))
            .into();
        style.height = Val::Px(theme.textbox.height);
        style.padding = UiRect::all(Val::Px(theme.textbox.padding));

        commands.entity(entity).remove::<UiImage>();
        if let Some(path) = &theme.textbox.image_path {
            commands
                .entity(entity)
                .insert(UiImage::new(asset_server.load::<Image>(path.clone())));
        }
    }

    if let Ok(mut text) = name_query.get_single_mut() {
        text.sections[0].style.font_size = theme.text.name.font_size;
        text.sections[0].style.color = bevy::color::Srgba::hex(&theme.text.name.color)
            .map(Color::from)
            .unwrap_or(Color::WHITE);
        text.sections[0].style.font = theme
            .text
            .name
            .font_path
            .as_ref()
            .map(|p| asset_server.load(p.clone()))
            .unwrap_or_default();
    }

    if let Ok(mut text) = text_query.get_single_mut() {
        text.sections[0].style.font_size = theme.text.dialogue.font_size;
        text.sections[0].style.color = bevy::color::Srgba::hex(&theme.text.dialogue.color)
            .map(Color::from)
            .unwrap_or(Color::WHITE);
        text.sections[0].style.font = theme
            .text
            .dialogue
            .font_path
            .as_ref()
            .map(|p| asset_server.load(p.clone()))
            .unwrap_or_default();
    }
}
