use bevy::prelude::*;
use rvn_parser::{AnimationParam, AnimationValue, Position, Transition};

use super::{make_fade_in, make_fade_out, WIN_H, WIN_W};
use crate::components::{AnimationKind, SpriteAnimation, SpriteBaseTransform, VnSprite};
use crate::vn_command::VnCommand;
use std::collections::HashMap;

fn position_to_x(pos: &Position) -> f32 {
    match pos {
        Position::Left => -WIN_W * 0.30,
        Position::Center => 0.0,
        Position::Right => WIN_W * 0.30,
        Position::Custom(n) => (n - 0.5) * WIN_W,
    }
}

fn sprite_default_transform(position: &Position) -> Transform {
    let x = position_to_x(position);
    let sprite_h = WIN_H * 0.85;
    let y = -(WIN_H / 2.0) + (sprite_h / 2.0);
    Transform::from_xyz(x, y, 10.0)
}

fn base_from_transform(transform: &Transform) -> SpriteBaseTransform {
    SpriteBaseTransform {
        translation: transform.translation,
        scale: transform.scale,
    }
}

fn bool_param(params: &[AnimationParam], name: &str, default: bool) -> bool {
    params
        .iter()
        .find(|p| p.name == name)
        .and_then(|p| match p.value {
            AnimationValue::Bool(v) => Some(v),
            _ => None,
        })
        .unwrap_or(default)
}

fn f32_param(params: &[AnimationParam], name: &str, default: f32) -> f32 {
    params
        .iter()
        .find(|p| p.name == name)
        .and_then(|p| match p.value {
            AnimationValue::Float(v) => Some(v),
            AnimationValue::Int(v) => Some(v as f32),
            _ => None,
        })
        .unwrap_or(default)
}

fn build_sprite_animation(animation: &str, params: &[AnimationParam]) -> Option<SpriteAnimation> {
    let looping = bool_param(params, "loop", false);
    let duration_secs = f32_param(
        params,
        "duration",
        match animation {
            "shake" => 0.35,
            "bounce" => 0.45,
            "pulse" => 0.60,
            _ => 0.50,
        },
    )
    .max(0.01);

    let kind = match animation {
        "shake" => AnimationKind::Shake {
            intensity: f32_param(params, "intensity", 10.0).max(0.0),
        },
        "bounce" => AnimationKind::Bounce {
            height: f32_param(params, "height", 18.0).max(0.0),
        },
        "pulse" => AnimationKind::Pulse {
            scale: f32_param(params, "scale", 1.08).max(0.01),
        },
        _ => return None,
    };

    Some(SpriteAnimation {
        kind,
        duration_secs,
        elapsed_secs: 0.0,
        looping,
    })
}

pub fn sprite_system(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut vn_events: EventReader<VnCommand>,
    mut queries: ParamSet<(
        Query<(Entity, &VnSprite)>,
        Query<(Entity, &VnSprite, &SpriteBaseTransform, &mut Transform), With<SpriteAnimation>>,
        Query<(&VnSprite, &mut Transform, &mut Sprite)>,
    )>,
) {
    let mut spawned_this_frame: HashMap<String, Entity> = HashMap::new();
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
                let sprite_h = WIN_H * 0.85;
                let has_anim = transition != Transition::None;
                let initial_alpha = if has_anim { 0.0 } else { 1.0 };
                let transform = sprite_default_transform(&position);
                let base = base_from_transform(&transform);

                let existing = queries
                    .p0()
                    .iter()
                    .find(|(_, s)| s.id == id)
                    .map(|(entity, _)| entity);

                if let Some(entity) = existing {
                    commands.entity(entity).insert((
                        Sprite {
                            custom_size: Some(Vec2::new(sprite_h * 0.55, sprite_h)),
                            color: Color::srgba(1.0, 1.0, 1.0, initial_alpha),
                            ..default()
                        },
                        texture,
                        transform,
                        base,
                    ));
                    commands.entity(entity).remove::<SpriteAnimation>();

                    if let Some(anim) = make_fade_in(&transition) {
                        commands.entity(entity).insert(anim);
                    }

                    spawned_this_frame.insert(id.clone(), entity);
                } else {
                    let mut ec = commands.spawn((
                        VnSprite { id: id.clone() },
                        base,
                        SpriteBundle {
                            sprite: Sprite {
                                custom_size: Some(Vec2::new(sprite_h * 0.55, sprite_h)),
                                color: Color::srgba(1.0, 1.0, 1.0, initial_alpha),
                                ..default()
                            },
                            texture,
                            transform,
                            ..default()
                        },
                    ));

                    if let Some(anim) = make_fade_in(&transition) {
                        ec.insert(anim);
                    }

                    let entity = ec.id();
                    spawned_this_frame.insert(id.clone(), entity);
                }
            }

            VnCommand::HideSprite { id, transition } => {
                let mut found = false;

                for (entity, sprite) in queries.p0().iter() {
                    if sprite.id == id {
                        found = true;
                        commands.entity(entity).remove::<SpriteAnimation>();

                        if let Some(anim) = make_fade_out(&transition) {
                            commands.entity(entity).insert(anim);
                        } else {
                            commands.entity(entity).despawn();
                        }
                    }
                }

                if !found {
                    if let Some(entity) = spawned_this_frame.get(&id) {
                        commands.entity(*entity).remove::<SpriteAnimation>();

                        if let Some(anim) = make_fade_out(&transition) {
                            commands.entity(*entity).insert(anim);
                        } else {
                            commands.entity(*entity).despawn();
                        }
                    }
                }
            }

            VnCommand::MoveSprite { id, position, .. } => {
                let transform = sprite_default_transform(&position);
                let base = base_from_transform(&transform);

                let mut found = false;

                for (entity, sprite) in queries.p0().iter() {
                    if sprite.id == id {
                        found = true;
                        commands.entity(entity).insert((transform.clone(), base));
                        commands.entity(entity).remove::<SpriteAnimation>();
                    }
                }

                if !found {
                    if let Some(entity) = spawned_this_frame.get(&id) {
                        commands.entity(*entity).insert((transform, base));
                        commands.entity(*entity).remove::<SpriteAnimation>();
                    }
                }
            }

            VnCommand::AnimateSprite {
                id,
                animation,
                params,
            } => {
                let Some(anim) = build_sprite_animation(&animation, &params) else {
                    bevy::log::warn!("animation inconnue ignorée: {}", animation);
                    continue;
                };

                let mut found = false;

                for (entity, sprite) in queries.p0().iter() {
                    if sprite.id == id {
                        found = true;
                        commands.entity(entity).insert(anim.clone());
                    }
                }

                if !found {
                    if let Some(entity) = spawned_this_frame.get(&id) {
                        commands.entity(*entity).insert(anim.clone());
                    }
                }
            }

            VnCommand::StopSpriteAnimation { id } => {
                let mut found = false;

                for (entity, sprite, base, mut transform) in queries.p1().iter_mut() {
                    if sprite.id == id {
                        found = true;
                        transform.translation = base.translation;
                        transform.scale = base.scale;
                        commands.entity(entity).remove::<SpriteAnimation>();
                    }
                }

                if !found {
                    if let Some(entity) = spawned_this_frame.get(&id) {
                        commands.entity(*entity).remove::<SpriteAnimation>();
                    }
                }
            }

            VnCommand::SetSpriteEffect {
                ref id,
                ref flip_x,
                ref flip_y,
                ref scale,
                ref rotation,
                ref tint,
            } => {
                for (sprite, mut transform, mut sprite_vis) in queries.p2().iter_mut() {
                    if &sprite.id != id {
                        continue;
                    }
                    if let Some(fx) = flip_x {
                        transform.scale.x = transform.scale.x.abs() * if *fx { -1.0 } else { 1.0 };
                    }
                    if let Some(fy) = flip_y {
                        transform.scale.y = transform.scale.y.abs() * if *fy { -1.0 } else { 1.0 };
                    }
                    if let Some(sc) = scale {
                        transform.scale.x = transform.scale.x.signum() * sc;
                        transform.scale.y = transform.scale.y.signum() * sc;
                    }
                    if let Some(deg) = rotation {
                        transform.rotation = Quat::from_rotation_z(deg.to_radians());
                    }
                    if let Some(color) = tint {
                        if let Ok(c) = Srgba::hex(color.trim_start_matches('#')) {
                            sprite_vis.color = Color::from(c);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

pub fn sprite_animation_system(
    time: Res<Time>,
    mut query: Query<(
        Entity,
        &SpriteBaseTransform,
        &mut SpriteAnimation,
        &mut Transform,
    )>,
    mut commands: Commands,
) {
    for (entity, base, mut animation, mut transform) in query.iter_mut() {
        animation.elapsed_secs += time.delta_seconds();
        let mut progress = animation.elapsed_secs / animation.duration_secs;

        if animation.looping {
            progress = progress.fract();
        } else {
            progress = progress.min(1.0);
        }

        transform.translation = base.translation;
        transform.scale = base.scale;

        match animation.kind {
            AnimationKind::Shake { intensity } => {
                // Oscillation rapide qui revient naturellement à zéro en fin de cycle.
                let decay = if animation.looping {
                    1.0
                } else {
                    1.0 - progress
                };
                let offset = (progress * std::f32::consts::TAU * 6.0).sin() * intensity * decay;
                transform.translation.x += offset;
            }
            AnimationKind::Bounce { height } => {
                let offset = (progress * std::f32::consts::PI).sin() * height;
                transform.translation.y += offset;
            }
            AnimationKind::Pulse { scale } => {
                let wave = (progress * std::f32::consts::TAU).sin();
                let factor = 1.0 + (scale - 1.0) * wave.max(0.0);
                transform.scale = base.scale * factor;
            }
        }

        if !animation.looping && animation.elapsed_secs >= animation.duration_secs {
            transform.translation = base.translation;
            transform.scale = base.scale;
            commands.entity(entity).remove::<SpriteAnimation>();
        }
    }
}
