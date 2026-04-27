use bevy::prelude::*;
use rvn_parser::{Position, Transition};

use super::{make_fade_in, make_fade_out, WIN_H, WIN_W};
use crate::components::VnSprite;
use crate::vn_command::VnCommand;

fn position_to_x(pos: &Position) -> f32 {
    match pos {
        Position::Left => -WIN_W * 0.30,
        Position::Center => 0.0,
        Position::Right => WIN_W * 0.30,
        Position::Custom(n) => (n - 0.5) * WIN_W,
    }
}

pub fn sprite_system(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut vn_events: EventReader<VnCommand>,
    sprite_query: Query<(Entity, &VnSprite)>,
) {
    let cmds: Vec<VnCommand> = vn_events.read().cloned().collect();
    for cmd in cmds {
        match cmd {
            VnCommand::ShowSprite {
                id,
                emotion,
                position,
                transition,
            } => {
                let file = match &emotion {
                    Some(emo) => format!("sprites/{}/{}.png", id, emo),
                    None => format!("sprites/{}/default.png", id),
                };
                let texture: Handle<Image> = asset_server.load(file);
                let x = position_to_x(&position);
                let sprite_h = WIN_H * 0.85;
                let y = -(WIN_H / 2.0) + (sprite_h / 2.0);
                let has_anim = transition != Transition::None;
                let initial_alpha = if has_anim { 0.0 } else { 1.0 };

                let existing = sprite_query.iter().find(|(_, s)| s.id == id);
                if let Some((entity, _)) = existing {
                    commands.entity(entity).insert((
                        Sprite {
                            custom_size: Some(Vec2::new(sprite_h * 0.55, sprite_h)),
                            color: Color::srgba(1.0, 1.0, 1.0, initial_alpha),
                            ..default()
                        },
                        texture,
                        Transform::from_xyz(x, y, 10.0),
                    ));
                    if let Some(anim) = make_fade_in(&transition) {
                        commands.entity(entity).insert(anim);
                    }
                } else {
                    let mut ec = commands.spawn((
                        VnSprite { id: id.clone() },
                        SpriteBundle {
                            sprite: Sprite {
                                custom_size: Some(Vec2::new(sprite_h * 0.55, sprite_h)),
                                color: Color::srgba(1.0, 1.0, 1.0, initial_alpha),
                                ..default()
                            },
                            texture,
                            transform: Transform::from_xyz(x, y, 10.0),
                            ..default()
                        },
                    ));
                    if let Some(anim) = make_fade_in(&transition) {
                        ec.insert(anim);
                    }
                }
            }

            VnCommand::HideSprite { id, transition } => {
                for (entity, sprite) in sprite_query.iter() {
                    if sprite.id == id {
                        if let Some(anim) = make_fade_out(&transition) {
                            commands.entity(entity).insert(anim);
                        } else {
                            commands.entity(entity).despawn();
                        }
                    }
                }
            }

            VnCommand::MoveSprite { id, position, .. } => {
                let x = position_to_x(&position);
                let sprite_h = WIN_H * 0.85;
                let y = -(WIN_H / 2.0) + (sprite_h / 2.0);
                for (entity, sprite) in sprite_query.iter() {
                    if sprite.id == id {
                        commands
                            .entity(entity)
                            .insert(Transform::from_xyz(x, y, 10.0));
                    }
                }
            }

            _ => {}
        }
    }
}
