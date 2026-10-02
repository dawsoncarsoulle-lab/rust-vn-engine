//! Shared scene/UI placement and subtitles for both desktop and browser video.
use bevy::prelude::*;
use rvn_ui::video::VideoClip;
#[cfg(test)]
#[path = "video_visual_tests.rs"]
mod tests;
pub(crate) fn create(
    world: &mut World,
    clip: &VideoClip,
    image: Handle<Image>,
) -> Option<(Entity, Entity)> {
    let texture = clip
        .poster
        .as_ref()
        .map(|path| world.resource::<AssetServer>().load::<Image>(path.clone()))
        .unwrap_or(image);
    let [x, y, w, h] = clip.rect;
    let entity = if let Some(target) = &clip.target {
        let (screen, element) = target.strip_prefix("ui:")?.split_once('/')?;
        let parent = world
            .query::<(Entity, &crate::composed_motion::InterfaceTarget)>()
            .iter(world)
            .find(|(_, target)| target.screen == screen && target.element == element)
            .map(|(entity, _)| entity)?;
        let entity = world
            .spawn(ImageBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                image: UiImage::new(texture),
                ..default()
            })
            .id();
        world.entity_mut(parent).add_child(entity);
        entity
    } else {
        let entity = world
            .spawn(SpriteBundle {
                sprite: Sprite {
                    custom_size: Some(Vec2::new(w, h)),
                    ..default()
                },
                texture,
                transform: Transform::from_xyz(
                    x + w / 2.0 - 640.0,
                    360.0 - y - h / 2.0,
                    100.0 + clip.layer as f32 * 0.01,
                ),
                ..default()
            })
            .id();
        let stage = world
            .query_filtered::<Entity, With<crate::systems::sprite::SpriteStage>>()
            .iter(world)
            .next();
        if let Some(stage) = stage {
            world.entity_mut(stage).add_child(entity);
        }
        entity
    };
    let caption = if clip.target.is_some() {
        let caption = world
            .spawn(TextBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    left: Val::Px(8.0),
                    right: Val::Px(8.0),
                    bottom: Val::Px(8.0),
                    ..default()
                },
                text: Text::from_section(
                    "",
                    TextStyle {
                        font_size: 22.0,
                        color: Color::WHITE,
                        ..default()
                    },
                )
                .with_justify(JustifyText::Center),
                background_color: Color::srgba(0.0, 0.0, 0.0, 0.8).into(),
                ..default()
            })
            .id();
        world.entity_mut(entity).add_child(caption);
        caption
    } else {
        let caption_height = h.min(64.0);
        let caption = world
            .spawn(Text2dBundle {
                text: Text::from_section(
                    "",
                    TextStyle {
                        font_size: 22.0,
                        color: Color::WHITE,
                        ..default()
                    },
                ),
                text_anchor: bevy::sprite::Anchor::Center,
                text_2d_bounds: bevy::text::Text2dBounds {
                    size: Vec2::new((w - 32.0).max(1.0), caption_height),
                },
                transform: Transform::from_xyz(
                    0.0,
                    -h / 2.0 + caption_height / 2.0 + 8.0_f32.min(h / 4.0),
                    0.2,
                ),
                ..default()
            })
            .id();
        let backing = world
            .spawn(SpriteBundle {
                sprite: Sprite {
                    color: Color::srgba(0.0, 0.0, 0.0, 0.8),
                    custom_size: Some(Vec2::new((w - 16.0).max(1.0), caption_height)),
                    ..default()
                },
                transform: Transform::from_xyz(0.0, 0.0, -0.05),
                ..default()
            })
            .id();
        world.entity_mut(caption).add_child(backing);
        world.entity_mut(entity).add_child(caption);
        caption
    };
    Some((entity, caption))
}
pub(crate) fn set_texture(world: &mut World, entity: Entity, texture: Handle<Image>) {
    if let Some(mut image) = world.get_mut::<UiImage>(entity) {
        image.texture = texture;
    } else if let Some(mut image) = world.get_mut::<Handle<Image>>(entity) {
        *image = texture;
    }
}
pub(crate) fn caption(world: &mut World, entity: Entity, text: &str) {
    if let Some(mut caption) = world.get_mut::<Text>(entity) {
        caption.sections[0].value = text.into();
    }
    if let Some(mut visibility) = world.get_mut::<Visibility>(entity) {
        *visibility = if text.is_empty() {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
    }
}
#[derive(Component)]
struct CinematicHidden(Visibility);
pub(crate) fn cinematic_visibility(world: &mut World) {
    let engine = &world.resource::<crate::resources::VnEngine>().0;
    let cinematic = engine
        .state
        .videos
        .waiting
        .as_ref()
        .and_then(|id| engine.state.videos.tracks.get(id))
        .is_some_and(|track| track.clip.cinematic && !track.finished());
    // A custom menu replaces, rather than inherits, the legacy dialogue/choice
    // roots. Hide both presentations, never arbitrary UI or video captions.
    let entities: Vec<_> = world
        .query_filtered::<Entity, Or<(
            With<crate::components::DialogueBox>,
            With<crate::components::ChoiceContainer>,
            With<crate::menu_documents::NarrativeRoot>,
            With<crate::menu_documents::ChoicesRoot>,
        )>>()
        .iter(world)
        .collect();
    for entity in entities {
        if cinematic {
            if !world.entity(entity).contains::<CinematicHidden>() {
                let previous = *world
                    .get::<Visibility>(entity)
                    .unwrap_or(&Visibility::Inherited);
                world.entity_mut(entity).insert(CinematicHidden(previous));
            }
            // Presentation updates may rewrite Visibility between frames. Keep
            // the cinematic policy authoritative without losing the original.
            world.entity_mut(entity).insert(Visibility::Hidden);
        } else if !cinematic {
            if let Some(previous) = world.get::<CinematicHidden>(entity).map(|hidden| hidden.0) {
                world
                    .entity_mut(entity)
                    .remove::<CinematicHidden>()
                    .insert(previous);
            }
        }
    }
}
/// Cinematic controls also work while the narrative is waiting for a movie,
/// not only when it is waiting for dialogue. Shared by desktop and browser.
pub(crate) fn cinematic_input(
    keys: bevy::prelude::Res<bevy::prelude::ButtonInput<bevy::prelude::KeyCode>>,
    state: bevy::prelude::Res<bevy::prelude::State<crate::resources::VnState>>,
    mut engine: bevy::prelude::ResMut<crate::resources::VnEngine>,
    mut events: bevy::prelude::EventWriter<crate::vn_command::PlayerInput>,
    mut commands: bevy::prelude::EventWriter<crate::vn_command::VnCommand>,
    mut next: bevy::prelude::ResMut<bevy::prelude::NextState<crate::resources::VnState>>,
) {
    use crate::resources::VnState;
    use bevy::prelude::KeyCode;
    if *state.get() != VnState::Stepping {
        return;
    }
    let Some(id) = engine.0.state.videos.waiting.clone() else {
        return;
    };
    if keys.just_pressed(KeyCode::Escape) {
        events.send(crate::vn_command::PlayerInput::ToggleMenu);
    } else if keys.just_pressed(KeyCode::F5) {
        events.send(crate::vn_command::PlayerInput::QuickSave);
    } else if keys.just_pressed(KeyCode::F6) {
        events.send(crate::vn_command::PlayerInput::QuickLoad);
    } else if (keys.just_pressed(KeyCode::Space) || keys.just_pressed(KeyCode::Enter))
        && engine.0.state.videos.tracks[&id].clip.skippable
    {
        if engine.0.skip_video(&id).is_ok() {
            for command in engine.0.renderer.take_pending() {
                commands.send(command);
            }
            next.set(VnState::Stepping);
        }
    }
}
