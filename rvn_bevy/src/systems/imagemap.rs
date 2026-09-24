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

        imagemap_state.clear();
        imagemap_state.hotspots = hotspots;
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
    cameras: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    images: Res<Assets<Image>>,
    windows: Query<&Window>,
    imagemap_state: Res<ImagemapState>,
    mut hover_query: Query<
        (&mut Sprite, &mut Visibility, &mut Transform, &Handle<Image>),
        With<ImagemapHover>,
    >,
) {
    if !imagemap_state.active {
        return;
    }
    let cursor_pos = windows.get_single().ok().and_then(|w| w.cursor_position());
    let Ok((mut sprite, mut visibility, mut transform, texture)) = hover_query.get_single_mut()
    else {
        return;
    };

    match cursor_pos {
        Some(pos) => {
            let world = cameras
                .get_single()
                .ok()
                .and_then(|(camera, transform)| camera.viewport_to_world_2d(transform, pos));
            let hovered = world.and_then(|p| imagemap_state.hit_test_world(p));
            if let Some(index) = hovered {
                let zone = &imagemap_state.hotspots[index];
                let r = &zone.hover_area;
                let valid = images.get(texture).is_some_and(|image| {
                    r.x1 >= 0
                        && r.y1 >= 0
                        && r.x2 > r.x1
                        && r.y2 > r.y1
                        && r.x2 <= image.width() as i32
                        && r.y2 <= image.height() as i32
                });
                if !valid {
                    *visibility = Visibility::Hidden;
                    return;
                }
                let sx = WIN_W / imagemap_state.source_w;
                let sy = WIN_H / imagemap_state.source_h;
                sprite.rect = Some(Rect::new(
                    r.x1 as f32,
                    r.y1 as f32,
                    r.x2 as f32,
                    r.y2 as f32,
                ));
                sprite.custom_size = Some(Vec2::new(
                    zone.area.width() as f32 * sx,
                    zone.area.height() as f32 * sy,
                ));
                transform.translation.x =
                    (zone.area.x1 as f32 + zone.area.width() as f32 * 0.5) * sx - WIN_W * 0.5;
                transform.translation.y =
                    WIN_H * 0.5 - (zone.area.y1 as f32 + zone.area.height() as f32 * 0.5) * sy;
                *visibility = Visibility::Visible;
                sprite.color = Color::WHITE;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vn_command::ImagemapZone;
    fn state() -> ImagemapState {
        ImagemapState {
            source_w: 1920.0,
            source_h: 1080.0,
            active: true,
            hotspots: vec![
                ImagemapZone {
                    area: rvn_parser::Rect::new(0, 0, 960, 1080),
                    hover_area: rvn_parser::Rect::new(100, 200, 300, 500),
                },
                ImagemapZone {
                    area: rvn_parser::Rect::new(960, 0, 1920, 1080),
                    hover_area: rvn_parser::Rect::new(0, 0, 100, 100),
                },
            ],
            ..default()
        }
    }
    #[test]
    fn boundaries_are_half_open_and_outside_is_ignored() {
        let s = state();
        assert_eq!(s.hit_test_world(Vec2::new(-640.0, 360.0)), Some(0));
        assert_eq!(s.hit_test_world(Vec2::ZERO), Some(1));
        assert_eq!(s.hit_test_world(Vec2::new(640.0, 0.0)), None);
        assert_eq!(s.hit_test_world(Vec2::new(-641.0, 0.0)), None);
        assert_eq!(s.hit_test_world(Vec2::new(0.0, -360.0)), None);
    }
    #[test]
    fn unloaded_images_and_reset_cannot_be_clicked() {
        let mut s = state();
        s.source_w = 0.0;
        assert_eq!(s.hit_test_world(Vec2::ZERO), None);
        s.clear();
        assert!(!s.active);
        assert_eq!(s.source_h, 0.0);
        assert!(s.hotspots.is_empty());
    }
    #[test]
    fn overlapping_zones_keep_declaration_order() {
        let mut s = state();
        s.hotspots[0].area = rvn_parser::Rect::new(0, 0, 1920, 1080);
        assert_eq!(s.hit_test_world(Vec2::ZERO), Some(0));
    }
    #[test]
    fn renderer_keeps_independent_hover_coordinates() {
        let h = rvn_parser::Hotspot {
            name: None,
            area: rvn_parser::Rect::new(0, 0, 100, 200),
            hover_area: Some(rvn_parser::Rect::new(500, 100, 700, 400)),
            body: vec![],
        };
        let z = ImagemapZone::from(&h);
        assert_eq!(z.hover_area, h.hover_area.unwrap());
        assert_eq!(z.area, h.area);
    }
}
