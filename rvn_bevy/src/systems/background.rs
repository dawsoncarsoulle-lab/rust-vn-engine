use bevy::prelude::*;
use rvn_parser::Transition;

use super::{ensure_extension, make_fade_in, make_fade_out, WIN_H, WIN_W};
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

        render_state.background = Some(path.clone());
        let full_path = ensure_extension(&path);
        let texture: Handle<Image> = asset_server.load(full_path);
        let initial_alpha = if has_anim { 0.0 } else { 1.0 };

        let mut ec = commands.spawn((
            VnBackground,
            SpriteBundle {
                sprite: Sprite {
                    custom_size: Some(Vec2::new(WIN_W, WIN_H)),
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
