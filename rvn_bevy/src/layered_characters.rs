//! Layered sprites use the ordinary sprite stage, transforms and ordering.
//! Keep hidden-layer assets alive too: switching an attribute is not an unload.
use crate::{
    components::{SpriteBaseTransform, VnSprite},
    resources::{ScriptErrorMessage, VnState},
    systems::sprite::SpriteStage,
    vn_command::VnCommand,
};
use bevy::prelude::*;
use rvn_core::composition::LayeredView;
use rvn_ui::composition::ImageLayer;

pub struct LayeredCharactersPlugin;
#[derive(Component)]
pub(crate) struct LayerTarget {
    pub character: String,
    pub layer: String,
    pub description: ImageLayer,
}
#[derive(Component)]
pub(crate) struct CompositionRoot {
    pub extent: Vec2,
    pub position: rvn_parser::Position,
    pub tint: Color,
}
#[derive(Component)]
pub(crate) struct LayeredFade {
    animation: crate::components::FadeAnim,
    pub(crate) base: Transform,
}
#[derive(Resource, Default)]
struct Cast {
    views: Vec<LayeredView>,
    dirty: bool,
    images: std::collections::BTreeMap<String, Handle<Image>>,
    pending: Vec<VnCommand>,
}
impl Plugin for LayeredCharactersPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Cast>().add_systems(
            Update,
            (
                receive.after(crate::systems::sprite_system),
                synchronize.after(receive),
                animate
                    .after(crate::systems::sprite_animation_system)
                    .before(crate::systems::fade_system),
                asset_errors.after(synchronize),
            ),
        );
    }
}
fn receive(
    mut events: EventReader<VnCommand>,
    mut cast: ResMut<Cast>,
    assets: Res<AssetServer>,
    engine: Res<crate::resources::VnEngine>,
) {
    for event in events.read() {
        match event {
            VnCommand::LayeredCharacters(views) => {
                if cast.views != *views {
                    cast.views = views.clone();
                    cast.dirty = true;
                }
            }
            VnCommand::ClearSprites => cast.dirty = true,
            VnCommand::ShowSprite { id, .. }
            | VnCommand::HideSprite { id, .. }
            | VnCommand::AnimateSprite { id, .. }
            | VnCommand::StopSpriteAnimation { id }
            | VnCommand::SetSpriteEffect { id, .. }
                if engine.0.state.layered.characters.contains_key(id) =>
            {
                cast.pending.push(event.clone());
                cast.dirty = true;
            }
            _ => {}
        }
    }
    let resources: std::collections::BTreeSet<_> = cast
        .views
        .iter()
        .flat_map(|view| view.resources.iter().cloned())
        .collect();
    cast.images.retain(|path, _| resources.contains(path));
    for path in resources {
        cast.images
            .entry(path.clone())
            .or_insert_with(|| assets.load(path));
    }
}
fn geometry(size: [f32; 2], rect: Option<[f32; 4]>, order: usize) -> (Vec2, Transform) {
    let factor = crate::systems::WIN_H * 0.85 / size[1];
    let [x, y, w, h] = rect.unwrap_or([0.0, 0.0, size[0], size[1]]);
    (
        Vec2::new(w * factor, h * factor),
        Transform::from_xyz(
            (x + w * 0.5 - size[0] * 0.5) * factor,
            (size[1] * 0.5 - y - h * 0.5) * factor,
            order as f32 * 0.01,
        ),
    )
}
pub(crate) fn synchronize(world: &mut World) {
    if !world.resource::<Cast>().dirty {
        return;
    }
    world.resource_mut::<Cast>().dirty = false;
    let views = world.resource::<Cast>().views.clone();
    let pending = std::mem::take(&mut world.resource_mut::<Cast>().pending);
    let roots: Vec<_> = world
        .query::<(Entity, &VnSprite, &CompositionRoot)>()
        .iter(world)
        .map(|(e, s, _)| (e, s.id.clone()))
        .collect();
    for (entity, id) in roots {
        if !views.iter().any(|view| view.character == id) {
            let fade = pending
                .iter()
                .rev()
                .find_map(|command| match command {
                    VnCommand::HideSprite {
                        id: target,
                        transition,
                    } if target == &id => Some(crate::systems::make_fade_out(transition)),
                    _ => None,
                })
                .flatten();
            if let Some(animation) = fade {
                let base = *world.get::<Transform>(entity).unwrap();
                world
                    .entity_mut(entity)
                    .remove::<crate::components::SpriteAnimation>()
                    .insert(LayeredFade { animation, base });
            } else if !world
                .get::<LayeredFade>(entity)
                .is_some_and(|fade| fade.animation.despawn_on_finish)
            {
                world.entity_mut(entity).despawn_recursive();
            }
        }
    }
    let stage = world
        .query_filtered::<Entity, With<SpriteStage>>()
        .iter(world)
        .next();
    for view in views {
        let mut root = world
            .query::<(Entity, &VnSprite)>()
            .iter(world)
            .find(|(_, sprite)| sprite.id == view.character)
            .map(|(entity, _)| entity);
        if root.is_some_and(|entity| world.get::<CompositionRoot>(entity).is_none()) {
            world.entity_mut(root.take().unwrap()).despawn_recursive();
        }
        let height = crate::systems::WIN_H * 0.85;
        let extent = Vec2::new(view.size[0] * height / view.size[1], height);
        let root = root.unwrap_or_else(|| {
            let transform = crate::systems::sprite::sprite_default_transform(&view.position);
            let entity = world
                .spawn((
                    VnSprite {
                        id: view.character.clone(),
                    },
                    CompositionRoot {
                        extent,
                        position: view.position.clone(),
                        tint: Color::WHITE,
                    },
                    SpriteBaseTransform {
                        translation: transform.translation,
                        scale: transform.scale,
                    },
                    SpatialBundle {
                        transform,
                        ..default()
                    },
                ))
                .id();
            if let Some(stage) = stage {
                world.entity_mut(stage).add_child(entity);
            }
            entity
        });
        let previous = world.get::<CompositionRoot>(root).unwrap().position.clone();
        if previous != view.position {
            let old = crate::systems::sprite::sprite_default_transform(&previous);
            let new = crate::systems::sprite::sprite_default_transform(&view.position);
            if let Some(mut transform) = world.get_mut::<Transform>(root) {
                transform.translation += new.translation - old.translation;
            }
            if let Some(mut base) = world.get_mut::<SpriteBaseTransform>(root) {
                base.translation = new.translation;
            }
            if let Some(mut fade) = world.get_mut::<LayeredFade>(root) {
                fade.base.translation += new.translation - old.translation;
            }
        }
        let tint = world.get::<CompositionRoot>(root).unwrap().tint;
        world.entity_mut(root).insert(CompositionRoot {
            extent,
            position: view.position.clone(),
            tint,
        });
        let current: Vec<_> = world
            .get::<Children>(root)
            .map(|children| children.iter().copied().collect())
            .unwrap_or_default();
        for child in current {
            if world
                .get::<LayerTarget>(child)
                .is_some_and(|target| !view.layers.iter().any(|layer| layer == &target.description))
            {
                world.entity_mut(child).despawn_recursive();
            }
        }
        for (order, layer) in view.order.into_iter().zip(view.layers) {
            let existing = world.get::<Children>(root).and_then(|children| {
                children
                    .iter()
                    .find(|child| {
                        world
                            .get::<LayerTarget>(**child)
                            .is_some_and(|target| target.description == layer)
                    })
                    .copied()
            });
            let (custom_size, transform) = geometry(view.size, layer.rect, order);
            if let Some(existing) = existing {
                world.entity_mut(existing).insert(transform);
                if let Some(mut sprite) = world.get_mut::<Sprite>(existing) {
                    sprite.custom_size = Some(custom_size);
                }
                continue;
            }
            let texture = world.resource::<Cast>().images[&layer.image].clone();
            let child = world
                .spawn((
                    LayerTarget {
                        character: view.character.clone(),
                        layer: layer.id.clone(),
                        description: layer.clone(),
                    },
                    SpriteBundle {
                        texture,
                        sprite: Sprite {
                            custom_size: Some(custom_size),
                            color: Color::srgba(1.0, 1.0, 1.0, layer.opacity),
                            ..default()
                        },
                        transform,
                        ..default()
                    },
                ))
                .id();
            world.entity_mut(root).add_child(child);
        }
        for command in &pending {
            match command {
                VnCommand::ShowSprite { id, transition, .. } if id == &view.character => {
                    world
                        .entity_mut(root)
                        .remove::<LayeredFade>()
                        .remove::<crate::components::SpriteAnimation>();
                    if let Some(animation) = crate::systems::make_fade_in(transition) {
                        let base = *world.get::<Transform>(root).unwrap();
                        world
                            .entity_mut(root)
                            .insert(LayeredFade { animation, base });
                    }
                }
                VnCommand::AnimateSprite {
                    id,
                    animation,
                    params,
                } if id == &view.character => {
                    if let Some(animation) =
                        crate::systems::sprite::build_sprite_animation(animation, params)
                    {
                        world.entity_mut(root).insert(animation);
                    }
                }
                VnCommand::StopSpriteAnimation { id } if id == &view.character => {
                    world
                        .entity_mut(root)
                        .remove::<crate::components::SpriteAnimation>();
                    let base = *world.get::<SpriteBaseTransform>(root).unwrap();
                    if let Some(mut transform) = world.get_mut::<Transform>(root) {
                        transform.translation = base.translation;
                        transform.scale = base.scale;
                    }
                    if let Some(mut fade) = world.get_mut::<LayeredFade>(root) {
                        fade.base.translation = base.translation;
                        fade.base.scale = base.scale;
                    }
                }
                VnCommand::SetSpriteEffect {
                    id,
                    flip_x,
                    flip_y,
                    scale,
                    rotation,
                    tint,
                } if id == &view.character => {
                    // Effects belong to the underlying pose, not the current
                    // slide/zoom offset. Otherwise they accumulate mid-fade.
                    let mut transform = world
                        .get::<LayeredFade>(root)
                        .map(|fade| fade.base)
                        .unwrap_or(*world.get::<Transform>(root).unwrap());
                    if let Some(flip) = flip_x {
                        transform.scale.x =
                            transform.scale.x.abs() * if *flip { -1.0 } else { 1.0 };
                    }
                    if let Some(flip) = flip_y {
                        transform.scale.y =
                            transform.scale.y.abs() * if *flip { -1.0 } else { 1.0 };
                    }
                    if let Some(scale) = scale {
                        transform.scale.x = transform.scale.x.signum() * scale;
                        transform.scale.y = transform.scale.y.signum() * scale;
                    }
                    if let Some(rotation) = rotation {
                        transform.rotation = Quat::from_rotation_z(rotation.to_radians());
                    }
                    world.entity_mut(root).insert(transform);
                    if let Some(mut base) = world.get_mut::<SpriteBaseTransform>(root) {
                        base.translation = transform.translation;
                        base.scale = transform.scale;
                    }
                    if let Some(mut fade) = world.get_mut::<LayeredFade>(root) {
                        fade.base = transform;
                    }
                    if let Some(color) = tint
                        .as_ref()
                        .and_then(|color| Srgba::hex(color.trim_start_matches('#')).ok())
                    {
                        world.get_mut::<CompositionRoot>(root).unwrap().tint = Color::from(color);
                    }
                }
                _ => {}
            }
        }
    }
}
fn animate(world: &mut World) {
    use crate::components::TransitionKind;
    if world
        .get_resource::<crate::accessibility::Accessibility>()
        .is_some_and(|access| access.blocked)
    {
        return;
    }
    let reduced = world
        .get_resource::<crate::accessibility::Accessibility>()
        .is_some_and(|access| access.settings.reduced_motion);
    let delta = world.resource::<Time>().delta_seconds();
    let roots: Vec<_> = world
        .query_filtered::<Entity, With<CompositionRoot>>()
        .iter(world)
        .collect();
    for root in roots {
        let mut alpha = 1.0;
        let mut remove = false;
        if let Some(mut fade) = world.get_mut::<LayeredFade>(root) {
            fade.animation.elapsed_secs += delta;
            let t = if reduced {
                1.0
            } else {
                (fade.animation.elapsed_secs / fade.animation.duration_secs.max(0.001))
                    .clamp(0.0, 1.0)
            };
            alpha = fade.animation.from + (fade.animation.to - fade.animation.from) * t;
            let progress = if fade.animation.to > fade.animation.from {
                1.0 - t
            } else {
                t
            };
            let mut transform = fade.base;
            match fade.animation.kind {
                TransitionKind::SlideLeft => transform.translation.x += 1920.0 * progress,
                TransitionKind::SlideRight => transform.translation.x -= 1920.0 * progress,
                TransitionKind::SlideUp => transform.translation.y -= 1920.0 * progress,
                TransitionKind::SlideDown => transform.translation.y += 1920.0 * progress,
                TransitionKind::ZoomIn => transform.scale *= 1.0 - 0.5 * progress,
                TransitionKind::ZoomOut => transform.scale *= 1.0 + 0.5 * progress,
                _ => {}
            }
            remove = t >= 1.0 && fade.animation.despawn_on_finish;
            let complete = t >= 1.0;
            world.entity_mut(root).insert(transform);
            if complete {
                world.entity_mut(root).remove::<LayeredFade>();
            }
        }
        if remove {
            world.entity_mut(root).despawn_recursive();
            continue;
        }
        let tint = world.get::<CompositionRoot>(root).unwrap().tint.to_srgba();
        let children: Vec<_> = world
            .get::<Children>(root)
            .map(|children| children.iter().copied().collect())
            .unwrap_or_default();
        for child in children {
            let opacity = world
                .get::<LayerTarget>(child)
                .map(|target| target.description.opacity)
                .unwrap_or(1.0);
            if let Some(mut sprite) = world.get_mut::<Sprite>(child) {
                sprite.color = Color::srgba(
                    tint.red,
                    tint.green,
                    tint.blue,
                    tint.alpha * opacity * alpha,
                );
            }
        }
    }
}
fn asset_errors(
    cast: Res<Cast>,
    assets: Res<AssetServer>,
    mut error: ResMut<ScriptErrorMessage>,
    mut next: ResMut<NextState<VnState>>,
) {
    for (path, handle) in &cast.images {
        if let Some(bevy::asset::LoadState::Failed(problem)) = assets.get_load_state(handle.id()) {
            error.0 = format!("Character layer '{path}' could not be loaded: {problem}");
            next.set(VnState::Error);
            return;
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_bounce_and_slide_fade_are_composed_without_accumulating() {
        use crate::components::{AnimationKind, FadeAnim, SpriteAnimation, TransitionKind};
        let mut app = App::new();
        app.init_resource::<Time>()
            .init_resource::<crate::accessibility::Accessibility>()
            .add_systems(
                Update,
                (crate::systems::sprite_animation_system, animate).chain(),
            );
        let root = app
            .world_mut()
            .spawn((
                CompositionRoot {
                    extent: Vec2::new(300.0, 600.0),
                    position: rvn_parser::Position::Center,
                    tint: Color::WHITE,
                },
                Transform::from_xyz(10.0, 20.0, 0.0),
                SpriteBaseTransform {
                    translation: Vec3::new(10.0, 20.0, 0.0),
                    scale: Vec3::ONE,
                },
                SpriteAnimation {
                    kind: AnimationKind::Bounce { height: 100.0 },
                    duration_secs: 1.0,
                    elapsed_secs: 0.0,
                    looping: false,
                },
                LayeredFade {
                    base: Transform::from_xyz(10.0, 20.0, 0.0),
                    animation: FadeAnim {
                        from: 0.0,
                        to: 1.0,
                        duration_secs: 1.0,
                        elapsed_secs: 0.0,
                        despawn_on_finish: false,
                        kind: TransitionKind::SlideLeft,
                    },
                },
            ))
            .id();
        let description: ImageLayer = serde_json::from_value(
            serde_json::json!({"id":"body","image":"body.png","opacity":0.8}),
        )
        .unwrap();
        let child = app
            .world_mut()
            .spawn((
                LayerTarget {
                    character: "iris".into(),
                    layer: "body".into(),
                    description,
                },
                Sprite::default(),
            ))
            .id();
        app.world_mut().entity_mut(root).add_child(child);
        for step in 1..=4 {
            app.world_mut()
                .resource_mut::<Time>()
                .advance_by(std::time::Duration::from_millis(250));
            app.update();
            let t = step as f32 * 0.25;
            let pose = app.world().get::<Transform>(root).unwrap();
            assert!((pose.translation.x - (10.0 + 1920.0 * (1.0 - t))).abs() < 0.01);
            assert!(
                (pose.translation.y
                    - (20.0
                        + if step == 4 {
                            0.0
                        } else {
                            (t * std::f32::consts::PI).sin() * 100.0
                        }))
                .abs()
                    < 0.01
            );
            assert!(
                (app.world()
                    .get::<Sprite>(child)
                    .unwrap()
                    .color
                    .to_srgba()
                    .alpha
                    - 0.8 * t)
                    .abs()
                    < 0.001
            );
        }
        assert!(app.world().get::<LayeredFade>(root).is_none());
        assert!(app.world().get::<SpriteAnimation>(root).is_none());
    }
    #[test]
    fn authored_rectangles_use_the_same_bottom_aligned_stage_as_sprites() {
        let (size, transform) = geometry([600.0, 1000.0], Some([100.0, 200.0, 200.0, 300.0]), 4);
        assert!(size.abs_diff_eq(Vec2::new(122.4, 183.6), 0.001));
        assert!(transform
            .translation
            .abs_diff_eq(Vec3::new(-61.2, 91.8, 0.04), 0.001));
        let (_, full) = geometry([600.0, 1000.0], None, 0);
        assert_eq!(full.translation, Vec3::ZERO);
    }
}
