use bevy::prelude::*;
use rvn_parser::Transition;

use super::{ensure_extension, make_fade_in, make_fade_out};
use crate::components::VnBackground;
use crate::resources::VnRenderState;
use crate::vn_command::VnCommand;

pub fn background_system(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut render_state: ResMut<VnRenderState>,
    mut vn_events: EventReader<VnCommand>,
    bg_query: Query<Entity, With<VnBackground>>,
) {
    let cmds: Vec<VnCommand> = vn_events.read().cloned().collect();
    for cmd in cmds {
        let VnCommand::SetBackground { path, transition } = cmd else {
            continue;
        };

        let has_anim = transition != Transition::None;

        for entity in bg_query.iter() {
            if has_anim {
                if let Some(anim) = make_fade_out(&transition) {
                    commands.entity(entity).insert(anim);
                }
            } else {
                commands.entity(entity).despawn();
            }
        }

        if path.is_empty(){render_state.background=None;continue;}
        render_state.background = Some(path.clone());
        let full_path = ensure_extension(&path);
        let texture: Handle<Image> = asset_server.load(full_path);
        let initial_alpha = if has_anim { 0.0 } else { 1.0 };

        let mut ec = commands.spawn((
            VnBackground,
            SpriteBundle {
                sprite: Sprite {
                    color: Color::srgba(1.0, 1.0, 1.0, initial_alpha),
                    ..default()
                },
                texture,
                transform: Transform::from_xyz(0.0, 0.0, -100.0),
                ..default()
            },
        ));
        if let Some(anim) = make_fade_in(&transition) {
            ec.insert(anim);
        }
    }
}

/// Recalcule le scale du background pour couvrir toute la fenêtre.
///
/// Le comportement attendu pour un VN est un mode "cover" : l'image garde son
/// ratio, remplit toute la fenêtre, et peut être légèrement rognée si le ratio
/// de l'image ne correspond pas au ratio de la fenêtre.
pub fn background_cover_resize_system(
    windows: Query<&Window>,
    images: Res<Assets<Image>>,
    mut bg_query: Query<(&Handle<Image>, &mut Transform), With<VnBackground>>,
) {
    let Ok(window) = windows.get_single() else {
        return;
    };

    let window_w = window.resolution.width().max(1.0);
    let window_h = window.resolution.height().max(1.0);

    for (handle, mut transform) in bg_query.iter_mut() {
        let Some(image) = images.get(handle) else {
            continue;
        };

        let size = image.size();
        let image_w = (size.x as f32).max(1.0);
        let image_h = (size.y as f32).max(1.0);

        let scale = (window_w / image_w).max(window_h / image_h);
        transform.scale = Vec3::new(scale, scale, 1.0);
        transform.translation.x = 0.0;
        transform.translation.y = 0.0;
    }
}
