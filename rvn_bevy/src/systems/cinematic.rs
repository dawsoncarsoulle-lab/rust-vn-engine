use bevy::prelude::*;

use super::{ensure_extension, make_fade_in, make_fade_out};
use crate::components::VnCinematic;
use crate::resources::CgAssetRegistry;
use crate::vn_command::VnCommand;

fn transition_name_to_fade(name: Option<&str>) -> rvn_parser::Transition {
    match name {
        Some("fade") | Some("dissolve") => rvn_parser::Transition::Fade {
            duration_ms: rvn_parser::Transition::DEFAULT_FADE_MS,
        },
        _ => rvn_parser::Transition::None,
    }
}

pub fn cinematic_system(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    cg_registry: Res<CgAssetRegistry>,
    mut vn_events: EventReader<VnCommand>,
    query: Query<Entity, With<VnCinematic>>,
) {
    let cmds: Vec<VnCommand> = vn_events.read().cloned().collect();
    for cmd in cmds {
        match cmd {
            VnCommand::ShowCinematic { id, transition } => {
                let transition = transition_name_to_fade(transition.as_deref());
                let has_anim = transition != rvn_parser::Transition::None;

                for entity in query.iter() {
                    if let Some(anim) = make_fade_out(&transition) {
                        commands.entity(entity).insert(anim);
                    } else {
                        commands.entity(entity).despawn();
                    }
                }

                let path = cg_registry
                    .0
                    .get(&id)
                    .cloned()
                    .unwrap_or_else(|| ensure_extension(&format!("cgs/{id}")));
                let texture: Handle<Image> = asset_server.load(path);
                let initial_alpha = if has_anim { 0.0 } else { 1.0 };

                let mut ec = commands.spawn((
                    VnCinematic,
                    SpriteBundle {
                        sprite: Sprite {
                            color: Color::srgba(1.0, 1.0, 1.0, initial_alpha),
                            ..default()
                        },
                        texture,
                        transform: Transform::from_xyz(0.0, 0.0, 50.0),
                        ..default()
                    },
                ));
                if let Some(anim) = make_fade_in(&transition) {
                    ec.insert(anim);
                }
            }
            VnCommand::HideCinematic { transition } => {
                let transition = transition_name_to_fade(transition.as_deref());
                for entity in query.iter() {
                    if let Some(anim) = make_fade_out(&transition) {
                        commands.entity(entity).insert(anim);
                    } else {
                        commands.entity(entity).despawn();
                    }
                }
            }
            _ => {}
        }
    }
}

pub fn cinematic_cover_resize_system(
    windows: Query<&Window>,
    images: Res<Assets<Image>>,
    mut query: Query<(&Handle<Image>, &mut Transform), With<VnCinematic>>,
) {
    let Ok(window) = windows.get_single() else {
        return;
    };

    let window_w = window.resolution.width().max(1.0);
    let window_h = window.resolution.height().max(1.0);

    for (handle, mut transform) in query.iter_mut() {
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
