use bevy::prelude::*;

use super::{ensure_extension, WIN_H, WIN_W};
use crate::components::{ImagemapBackground, ImagemapHover};
use crate::resources::ImagemapState;
use crate::vn_command::VnCommand;

pub fn imagemap_system(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut vn_events: EventReader<VnCommand>,
    mut imagemap_state: ResMut<ImagemapState>,
    imagemap_bg_query: Query<Entity, With<ImagemapBackground>>,
    imagemap_hover_query: Query<Entity, With<ImagemapHover>>,
) {
    let cmds: Vec<VnCommand> = vn_events.read().cloned().collect();
    for cmd in cmds {
        let VnCommand::ShowImagemap {
            background,
            hover_image,
            hotspots,
        } = cmd
        else {
            continue;
        };

        for entity in imagemap_bg_query.iter() {
            commands.entity(entity).despawn();
        }
        for entity in imagemap_hover_query.iter() {
            commands.entity(entity).despawn();
        }

        let bg_path = ensure_extension(&background);
        let bg_handle: Handle<Image> = asset_server.load(bg_path.clone());

        commands.spawn((
            ImagemapBackground,
            SpriteBundle {
                sprite: Sprite {
                    custom_size: Some(Vec2::new(WIN_W, WIN_H)),
                    ..default()
                },
                texture: bg_handle.clone(),
                transform: Transform::from_xyz(0.0, 0.0, -50.0),
                ..default()
            },
        ));

        if let Some(hover_path) = hover_image {
            let hover_path = ensure_extension(&hover_path);
            let hover_texture: Handle<Image> = asset_server.load(hover_path.clone());
            commands.spawn((
                ImagemapHover,
                SpriteBundle {
                    sprite: Sprite {
                        custom_size: Some(Vec2::new(WIN_W, WIN_H)),
                        color: Color::srgba(1.0, 1.0, 1.0, 0.0),
                        ..default()
                    },
                    texture: hover_texture,
                    transform: Transform::from_xyz(0.0, 0.0, -49.0),
                    visibility: Visibility::Hidden,
                    ..default()
                },
            ));
        }

        imagemap_state.hotspots = hotspots
            .iter()
            .enumerate()
            .map(|(i, (_, (x1, y1, x2, y2)))| (i, *x1, *y1, *x2, *y2))
            .collect();
        imagemap_state.bg_handle = Some(bg_handle);
        imagemap_state.active = true;
    }
}

pub fn imagemap_dimensions_system(
    mut imagemap_state: ResMut<ImagemapState>,
    images: Res<Assets<Image>>,
) {
    if !imagemap_state.active || imagemap_state.source_w > 0.0 {
        return;
    }
    if let Some(handle) = &imagemap_state.bg_handle {
        if let Some(image) = images.get(handle) {
            let size = image.size();
            imagemap_state.source_w = size.x as f32;
            imagemap_state.source_h = size.y as f32;
        }
    }
}

pub fn imagemap_hover_system(
    windows: Query<&Window>,
    imagemap_state: Res<ImagemapState>,
    mut hover_query: Query<(&mut Sprite, &mut Visibility), With<ImagemapHover>>,
) {
    if !imagemap_state.active {
        return;
    }
    let cursor_pos = windows.get_single().ok().and_then(|w| w.cursor_position());
    let Ok((mut sprite, mut visibility)) = hover_query.get_single_mut() else {
        return;
    };

    match cursor_pos {
        Some(pos) => {
            let hovered = imagemap_state.hit_test(pos.x, pos.y, WIN_W, WIN_H);
            if hovered.is_some() {
                *visibility = Visibility::Visible;
                sprite.color.set_alpha(0.7);
            } else {
                *visibility = Visibility::Hidden;
            }
        }
        None => {
            *visibility = Visibility::Hidden;
        }
    }
}

pub fn imagemap_cleanup_system(
    mut commands: Commands,
    imagemap_state: Res<ImagemapState>,
    imagemap_bg_query: Query<Entity, With<ImagemapBackground>>,
    imagemap_hover_query: Query<Entity, With<ImagemapHover>>,
) {
    if !imagemap_state.active {
        for entity in imagemap_bg_query.iter() {
            commands.entity(entity).despawn();
        }
        for entity in imagemap_hover_query.iter() {
            commands.entity(entity).despawn();
        }
    }
}
