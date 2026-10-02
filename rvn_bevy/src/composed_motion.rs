//! Apply the core's seekable poses after ordinary layout/legacy effects.
//! Reset only our previous contribution before the next frame, so moving a
//! sprite, resizing a window and changing a control's feedback remain coherent.
use crate::{
    components::{CurrentMotionBackground, VnSprite},
    resources::{ScriptErrorMessage, VnEngine, VnState},
    vn_command::VnCommand,
};
use bevy::prelude::*;
use rvn_core::motion::{MotionTarget, MotionView};
use rvn_ui::motion::Pose;

pub struct ComposedMotionPlugin;
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct MotionApply;
impl Plugin for ComposedMotionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Poses>()
            .init_resource::<FrameImages>()
            .add_systems(First, reset)
            .add_systems(
                Update,
                (
                    clock.before(crate::systems::stepping_system),
                    receive.after(crate::systems::stepping_system),
                    frame_failures.after(receive),
                ),
            )
            .add_systems(
                PostUpdate,
                apply
                    .after(bevy::ui::UiSystem::Layout)
                    .before(bevy::transform::TransformSystem::TransformPropagate)
                    .in_set(MotionApply),
            );
    }
}

/// Opt-in native QA. It checks applied components, not only sampled numbers,
/// captures the actual window and terminates its own isolated game instance.
pub(crate) fn qa_drive(world: &mut World) {
    use bevy::{app::AppExit, render::view::screenshot::ScreenshotManager, window::PrimaryWindow};
    #[derive(Resource)]
    struct Run {
        started: std::time::Instant,
        step: usize,
        entered: f64,
        saved: Option<rvn_core::save::SaveData>,
        wait_seen: bool,
    }
    if !world.contains_resource::<Run>() {
        world.insert_resource(Run {
            started: std::time::Instant::now(),
            step: 0,
            entered: 0.0,
            saved: None,
            wait_seen: false,
        });
    }
    let now = world.resource::<Run>().started.elapsed().as_secs_f64();
    // Bevy deliberately clamps game time after an expensive frame. Loading
    // assets or sharing a GPU with the browser must not turn that into a false
    // deadlock report. Keep a wall-clock limit, but allow each phase to finish.
    assert!(now<120.0,"Composed motion QA timed out at {}, state={:?}, next={:?}, statement={:?}, debug={}, pc={}, wait={:?}, tracks={:?}",world.resource::<Run>().step,world.resource::<State<VnState>>().get(),world.resource::<NextState<VnState>>(),world.resource::<VnEngine>().0.peek_statement(),world.resource::<crate::systems::DebugOverlayState>().visible,world.resource::<VnEngine>().0.state.pc,world.resource::<VnEngine>().0.state.motions.waiting,world.resource::<VnEngine>().0.state.motions.tracks.iter().map(|(key,track)|(key,track.elapsed,track.running)).collect::<Vec<_>>());
    let state = world.resource::<State<VnState>>().get().clone();
    assert_ne!(
        state,
        VnState::Error,
        "{}",
        world.resource::<ScriptErrorMessage>().0
    );
    let step = world.resource::<Run>().step;
    if state == VnState::TitleScreen {
        if now > 2.0 {
            world
                .resource_mut::<NextState<VnState>>()
                .set(VnState::Stepping);
        }
        return;
    }
    if world
        .resource::<VnEngine>()
        .0
        .state
        .motions
        .waiting
        .is_some()
    {
        world.resource_mut::<Run>().wait_seen = true;
    }
    if state != VnState::Waiting || now - world.resource::<Run>().entered < 0.7 {
        return;
    }
    if matches!(step, 0 | 2 | 4 | 6 | 9) {
        if world.resource::<crate::resources::TypewriterState>().typing {
            world.send_event(crate::vn_command::PlayerInput::SkipTypewriter);
            return;
        }
        if let Ok(Some(rvn_core::Interaction::Dialogue { text: expected, .. })) =
            world.resource::<VnEngine>().0.current_interaction()
        {
            let actual: String = world
                .query_filtered::<&Text, With<crate::components::DialogueText>>()
                .iter(world)
                .flat_map(|text| text.sections.iter().map(|section| section.value.as_str()))
                .collect();
            assert_eq!(
                actual, expected,
                "Do not certify blank/stale dialogue after waiting for animation"
            );
        }
    }
    if step == 0 {
        // The first drawable frame can precede asynchronous asset loading.
        // A component-only assertion must never certify a blank screenshot.
        let handles: Vec<_> = world
            .query::<&Handle<Image>>()
            .iter(world)
            .cloned()
            .collect();
        if handles
            .iter()
            .any(|handle| world.resource::<Assets<Image>>().get(handle).is_none())
        {
            return;
        }
        let targets: Vec<_> = world
            .query::<(Entity, &InterfaceTarget)>()
            .iter(world)
            .filter(|(_, target)| target.element == "card")
            .map(|(entity, _)| entity)
            .collect();
        if targets.is_empty() {
            return;
        }
        for entity in targets {
            assert!(
                world.get::<Node>(entity).unwrap().size().min_element() > 1.0,
                "The animated image has an empty layout"
            );
            let center = world
                .get::<GlobalTransform>(entity)
                .unwrap()
                .translation()
                .truncate();
            assert!(
                center.x > 0.0 && center.x < 1280.0 && center.y > 0.0 && center.y < 720.0,
                "The animated image is outside the window"
            );
            if let Some(clip) = world.get::<bevy::ui::CalculatedClip>(entity) {
                assert!(
                    clip.clip.width() > 1.0 && clip.clip.height() > 1.0,
                    "The animated image is clipped out"
                );
            }
            let mut subtree = Vec::new();
            children(world, entity, &mut subtree);
            for child in subtree {
                if let Some(image) = world.get::<UiImage>(child) {
                    let Some(asset) = world.resource::<Assets<Image>>().get(&image.texture) else {
                        return;
                    };
                    let node = world.get::<Node>(child).unwrap();
                    assert!(
                        node.size().min_element() > 1.0,
                        "Image child has no measured extent: {:?}, texture {:?}",
                        node.size(),
                        asset.size()
                    );
                    assert!(
                        world.get::<ViewVisibility>(child).unwrap().get(),
                        "Image child is not visible"
                    );
                }
            }
        }
        if now < 2.5 {
            return;
        }
    }
    let dir = std::path::PathBuf::from(std::env::var_os("RVN_QA_OUTPUT").unwrap());
    let window = world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(world);
    let capture = |world: &mut World, name: &str| {
        world
            .resource_mut::<ScreenshotManager>()
            .save_screenshot_to_disk(window, dir.join(name))
            .unwrap()
    };
    let verify = |world: &mut World| {
        for view in world.resource::<Poses>().0.clone() {
            let entities: Vec<_> = match &view.target {
                MotionTarget::Background => world
                    .query_filtered::<Entity, With<CurrentMotionBackground>>()
                    .iter(world)
                    .collect(),
                MotionTarget::Sprite { id } => world
                    .query::<(Entity, &VnSprite)>()
                    .iter(world)
                    .filter(|(_, sprite)| sprite.id == *id)
                    .map(|(entity, _)| entity)
                    .collect(),
                MotionTarget::Layer { id, layer } => world
                    .query::<(Entity, &crate::layered_characters::LayerTarget)>()
                    .iter(world)
                    .filter(|(_, target)| target.character == *id && target.layer == *layer)
                    .map(|(entity, _)| entity)
                    .collect(),
                MotionTarget::Interface { screen, element } => world
                    .query::<(Entity, &InterfaceTarget)>()
                    .iter(world)
                    .filter(|(_, target)| target.screen == *screen && target.element == *element)
                    .map(|(entity, _)| entity)
                    .collect(),
            };
            assert!(
                !entities.is_empty(),
                "Animation target was not drawn: {}",
                view.target.key()
            );
            for entity in entities {
                let original = world.get::<Original>(entity).unwrap();
                let base = original.transform.unwrap();
                let actual = world.get::<Transform>(entity).unwrap();
                assert!((actual.scale.x - base.scale.x * view.pose.scale_x).abs() < 0.002);
                let sign = if matches!(view.target, MotionTarget::Interface { .. }) {
                    1.0
                } else {
                    -1.0
                };
                assert!(actual.rotation.abs_diff_eq(
                    base.rotation * Quat::from_rotation_z(sign * view.pose.rotation.to_radians()),
                    0.002
                ));
                if let Some(sprite) = world.get::<Sprite>(entity) {
                    assert!(
                        (sprite.color.to_srgba().alpha
                            - original.sprite.as_ref().unwrap().color.to_srgba().alpha
                                * view.pose.opacity
                                * view.pose.tint_a)
                            .abs()
                            < 0.002
                    );
                }
            }
        }
    };
    match step {
        0 => {
            assert_eq!(world.resource::<VnEngine>().0.state.motions.tracks.len(), 3);
            verify(world);
            let engine = &world.resource::<VnEngine>().0;
            let saved = rvn_core::save::SaveData::from_state(
                &engine.state,
                1,
                "Mid-motion".into(),
                "main.rvn".into(),
            );
            std::fs::write(
                dir.join("mid-motion.json"),
                serde_json::to_vec_pretty(&saved).unwrap(),
            )
            .unwrap();
            world.resource_mut::<Run>().saved = Some(saved);
            capture(world, "01_moving.png");
        }
        1 => {
            let saved = world.resource::<Run>().saved.clone().unwrap();
            let expected = saved.motions.clone();
            world.resource_mut::<VnEngine>().0.load_data(saved).unwrap();
            assert_eq!(world.resource::<VnEngine>().0.state.motions, expected);
            let commands = world.resource_mut::<VnEngine>().0.renderer.take_pending();
            for command in commands {
                world.send_event(command);
            }
        }
        2 => {
            verify(world);
            capture(world, "02_restored.png");
        }
        3 => {
            world
                .resource_mut::<VnEngine>()
                .0
                .advance_dialogue()
                .unwrap();
            world
                .resource_mut::<NextState<VnState>>()
                .set(VnState::Stepping);
        }
        4 => {
            assert!(
                world.resource::<Run>().wait_seen,
                "The narrative never waited for the animated target"
            );
            assert!(!world.resource::<VnEngine>().0.state.motions.tracks["sprite:token"].running);
            verify(world);
            capture(world, "03_finished_pose.png");
        }
        5 => {
            world
                .resource_mut::<VnEngine>()
                .0
                .advance_dialogue()
                .unwrap();
            world
                .resource_mut::<NextState<VnState>>()
                .set(VnState::Stepping);
        }
        6 => {
            assert!(world
                .resource::<VnEngine>()
                .0
                .state
                .motions
                .tracks
                .is_empty());
            assert!(world.resource::<Poses>().0.is_empty());
            capture(world, "04_stopped.png");
        }
        7 => {
            world
                .resource_mut::<VnEngine>()
                .0
                .advance_dialogue()
                .unwrap();
            world
                .resource_mut::<NextState<VnState>>()
                .set(VnState::Stepping);
        }
        8 => {
            assert!(!world.resource::<VnEngine>().0.state.sprites["token"].visible);
            assert!(world.resource::<VnEngine>().0.state.ui.screens.is_empty());
            assert!(world.resource_mut::<VnEngine>().0.rollback());
            let commands = world.resource_mut::<VnEngine>().0.renderer.take_pending();
            for command in commands {
                world.send_event(command);
            }
        }
        9 => {
            capture(world, "05_rollback.png");
            std::fs::write(dir.join("result.json"),serde_json::to_vec_pretty(&serde_json::json!({"result":"pass","scenario":"native composable animations on background/sprite/UI image, transform/opacity application, frame playback, wait completion, save/load, Stop, disappearance and rollback"})).unwrap()).unwrap();
        }
        _ => {
            world.send_event(AppExit::Success);
            return;
        }
    }
    let mut run = world.resource_mut::<Run>();
    run.step += 1;
    run.entered = now;
}
#[derive(Component)]
pub(crate) struct InterfaceTarget {
    pub screen: String,
    pub element: String,
}
#[derive(Resource, Default)]
struct Poses(Vec<MotionView>);
#[derive(Resource, Default)]
struct FrameImages(std::collections::BTreeMap<String, Handle<Image>>);
#[derive(Component, Default)]
struct Original {
    transform: Option<Transform>,
    sprite: Option<Sprite>,
    texture: Option<Handle<Image>>,
    background: Option<Color>,
    border: Option<Color>,
    image: Option<UiImage>,
    text: Option<Vec<Color>>,
}

fn clock(
    time: Res<Time>,
    state: Res<State<VnState>>,
    mut engine: ResMut<VnEngine>,
    mut events: EventWriter<VnCommand>,
    mut error: ResMut<ScriptErrorMessage>,
    mut next: ResMut<NextState<VnState>>,
    accessibility: Res<crate::accessibility::Accessibility>,
) {
    if accessibility.blocked
        || !matches!(
            state.get(),
            VnState::Waiting | VnState::Stepping | VnState::Animating
        )
    {
        return;
    }
    if let Err(problem) = engine.0.tick_motions(time.delta_seconds_f64().min(3600.0)) {
        error.0 = problem.to_string();
        next.set(VnState::Error);
        return;
    }
    for command in engine.0.renderer.take_pending() {
        events.send(command);
    }
}
fn receive(
    mut events: EventReader<VnCommand>,
    mut poses: ResMut<Poses>,
    engine: Res<VnEngine>,
    assets: Res<AssetServer>,
    mut frames: ResMut<FrameImages>,
) {
    for event in events.read() {
        if let VnCommand::Motions(views) = event {
            poses.0 = views.clone();
        }
    }
    let mut wanted: std::collections::BTreeSet<_> = engine
        .0
        .state
        .motions
        .tracks
        .values()
        .flat_map(|track| track.definition.frame_paths())
        .collect();
    wanted.extend(poses.0.iter().filter_map(|view| view.pose.frame.clone()));
    frames.0.retain(|path, _| wanted.contains(path));
    for path in wanted {
        frames
            .0
            .entry(path.clone())
            .or_insert_with(|| assets.load(path));
    }
}
fn frame_failures(
    frames: Res<FrameImages>,
    assets: Res<AssetServer>,
    mut error: ResMut<ScriptErrorMessage>,
    mut next: ResMut<NextState<VnState>>,
) {
    for (path, handle) in &frames.0 {
        if let Some(bevy::asset::LoadState::Failed(problem)) = assets.get_load_state(handle.id()) {
            error.0 = format!(
                "Animation frame '{}' could not be loaded: {}",
                path, problem
            );
            next.set(VnState::Error);
            return;
        }
    }
}
fn reset(world: &mut World) {
    crate::custom_canvas::reset_tints(world);
    let entities: Vec<_> = world
        .query_filtered::<Entity, With<Original>>()
        .iter(world)
        .collect();
    for entity in entities {
        let Some(mut target) = world.get_entity_mut(entity) else {
            continue;
        };
        let Some(original) = target.take::<Original>() else {
            continue;
        };
        if let Some(transform) = original.transform {
            target.insert(transform);
        }
        if let Some(sprite) = original.sprite {
            target.insert(sprite);
        }
        if let Some(texture) = original.texture {
            target.insert(texture);
        }
        if let Some(color) = original.background {
            target.insert(BackgroundColor(color));
        }
        if let Some(color) = original.border {
            target.insert(BorderColor(color));
        }
        if let Some(image) = original.image {
            target.insert(image);
        }
        if let Some(colors) = original.text {
            if let Some(mut text) = target.get_mut::<Text>() {
                for (section, color) in text.sections.iter_mut().zip(colors) {
                    section.style.color = color;
                }
            }
        }
    }
}
fn tinted(color: Color, pose: &Pose) -> Color {
    let mut value = color.to_srgba();
    value.red *= pose.tint_r;
    value.green *= pose.tint_g;
    value.blue *= pose.tint_b;
    value.alpha *= pose.tint_a * pose.opacity;
    Color::from(value)
}
fn original(world: &mut World, entity: Entity) -> bool {
    let Some(target) = world.get_entity(entity) else {
        return false;
    };
    if target.contains::<Original>() {
        return true;
    }
    let saved = Original {
        transform: None,
        sprite: target.get::<Sprite>().cloned(),
        texture: target.get::<Handle<Image>>().cloned(),
        background: target.get::<BackgroundColor>().map(|color| color.0),
        border: target.get::<BorderColor>().map(|color| color.0),
        image: target.get::<UiImage>().cloned(),
        text: target.get::<Text>().map(|text| {
            text.sections
                .iter()
                .map(|section| section.style.color)
                .collect()
        }),
    };
    world.entity_mut(entity).insert(saved);
    true
}
fn colors(world: &mut World, entity: Entity, pose: &Pose) {
    crate::custom_canvas::tint(world, entity, pose);
    if !original(world, entity) {
        return;
    }
    if let Some(mut sprite) = world.get_mut::<Sprite>(entity) {
        sprite.color = tinted(sprite.color, pose);
    }
    if let Some(mut background) = world.get_mut::<BackgroundColor>(entity) {
        background.0 = tinted(background.0, pose);
    }
    if let Some(mut border) = world.get_mut::<BorderColor>(entity) {
        border.0 = tinted(border.0, pose);
    }
    if let Some(mut image) = world.get_mut::<UiImage>(entity) {
        image.color = tinted(image.color, pose);
    }
    if let Some(mut text) = world.get_mut::<Text>(entity) {
        for section in &mut text.sections {
            section.style.color = tinted(section.style.color, pose);
        }
    }
}
fn children(world: &World, entity: Entity, output: &mut Vec<Entity>) {
    output.push(entity);
    if let Some(children_list) = world.get::<Children>(entity) {
        for child in children_list.iter() {
            children(world, *child, output);
        }
    }
}
fn transform(base: Transform, pose: &Pose, size: Vec2, factor: Vec2, ui: bool) -> Transform {
    let sign = if ui { 1.0 } else { -1.0 };
    let rotation = Quat::from_rotation_z(sign * pose.rotation.to_radians());
    let scale = Vec3::new(pose.scale_x, pose.scale_y, 1.0);
    let pivot = Vec3::new(
        (pose.pivot_x - 0.5) * size.x,
        sign * (pose.pivot_y - 0.5) * size.y,
        0.0,
    ) * base.scale;
    Transform {
        translation: base.translation
            + Vec3::new(pose.x * factor.x, sign * pose.y * factor.y, 0.0)
            + base.rotation * (pivot - rotation * (scale * pivot)),
        rotation: base.rotation * rotation,
        scale: base.scale * scale,
    }
}
fn apply(world: &mut World) {
    let views = world.resource::<Poses>().0.clone();
    let reduced = world
        .get_resource::<crate::accessibility::Accessibility>()
        .is_some_and(|access| access.settings.reduced_motion);
    let size = world
        .query::<&Window>()
        .iter(world)
        .next()
        .map(|window| Vec2::new(window.width(), window.height()))
        .unwrap_or(Vec2::new(1920.0, 1080.0));
    let assets = world.resource::<AssetServer>().clone();
    for mut view in views {
        if reduced {
            if let Some(track) = world
                .resource::<VnEngine>()
                .0
                .state
                .motions
                .tracks
                .get(&view.target.key())
                .filter(|track| track.running)
            {
                view.pose = track.base.clone();
            }
        }
        let entities: Vec<Entity> = match &view.target {
            MotionTarget::Background => world
                .query_filtered::<Entity, With<CurrentMotionBackground>>()
                .iter(world)
                .collect(),
            MotionTarget::Sprite { id } => world
                .query::<(Entity, &VnSprite)>()
                .iter(world)
                .filter(|(_, sprite)| sprite.id == *id)
                .map(|(entity, _)| entity)
                .collect(),
            MotionTarget::Layer { id, layer } => world
                .query::<(Entity, &crate::layered_characters::LayerTarget)>()
                .iter(world)
                .filter(|(_, target)| target.character == *id && target.layer == *layer)
                .map(|(entity, _)| entity)
                .collect(),
            MotionTarget::Interface { screen, element } => world
                .query::<(Entity, &InterfaceTarget)>()
                .iter(world)
                .filter(|(_, target)| target.screen == *screen && target.element == *element)
                .map(|(entity, _)| entity)
                .collect(),
        };
        for entity in entities {
            let ui = matches!(view.target, MotionTarget::Interface { .. });
            let dimensions = if ui {
                world
                    .get::<Node>(entity)
                    .map(Node::size)
                    .unwrap_or_default()
            } else {
                world
                    .get::<crate::layered_characters::CompositionRoot>(entity)
                    .map(|root| root.extent)
                    .or_else(|| {
                        world
                            .get::<Sprite>(entity)
                            .and_then(|sprite| sprite.custom_size)
                    })
                    .or_else(|| {
                        world
                            .get::<Handle<Image>>(entity)
                            .and_then(|handle| world.resource::<Assets<Image>>().get(handle))
                            .map(|image| image.size().as_vec2())
                    })
                    .unwrap_or_default()
            };
            let factor = if ui {
                Vec2::splat((size.x / 1920.0).min(size.y / 1080.0).max(0.25))
            } else if matches!(view.target, MotionTarget::Background) {
                Vec2::new(size.x / 1920.0, size.y / 1080.0)
            } else {
                Vec2::splat(crate::systems::WIN_W / 1920.0)
            };
            let Some(base) = world.get::<Transform>(entity).copied() else {
                continue;
            };
            original(world, entity);
            world.get_mut::<Original>(entity).unwrap().transform = Some(base);
            world
                .entity_mut(entity)
                .insert(transform(base, &view.pose, dimensions, factor, ui));
            if ui
                || world
                    .get::<crate::layered_characters::CompositionRoot>(entity)
                    .is_some()
            {
                let mut subtree = Vec::new();
                children(world, entity, &mut subtree);
                for child in subtree {
                    colors(world, child, &view.pose);
                    if let Some(path) = &view.pose.frame {
                        if let Some(mut image) = world.get_mut::<UiImage>(child) {
                            image.texture = assets.load(path.clone());
                        }
                    }
                }
            } else {
                colors(world, entity, &view.pose);
                if let Some(path) = &view.pose.frame {
                    world
                        .entity_mut(entity)
                        .insert(assets.load::<Image>(path.clone()));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn spline_and_sampled_user_curve_use_the_same_desktop_transform() {
        use rvn_ui::motion::{Easing, Motion, PathPoint};
        let motion = Motion::Spline {
            seconds: 2.0,
            points: vec![
                PathPoint { x: 0.0, y: 0.0 },
                PathPoint { x: 100.0, y: -50.0 },
                PathPoint { x: 200.0, y: 0.0 },
            ],
            curve: Easing::Samples {
                values: vec![0.0, 0.25, 1.0],
            },
        };
        let pose = motion.sample(1.0, &Pose::default()).unwrap();
        let base = Transform::from_xyz(10.0, 20.0, 3.0);
        let desktop = transform(base, &pose, Vec2::new(40.0, 60.0), Vec2::ONE, false);
        let interface = transform(base, &pose, Vec2::new(40.0, 60.0), Vec2::ONE, true);
        assert!((desktop.translation.x - (base.translation.x + pose.x)).abs() < 0.001);
        assert!((desktop.translation.y - (base.translation.y - pose.y)).abs() < 0.001);
        assert!((interface.translation.y - (base.translation.y + pose.y)).abs() < 0.001);
        assert_eq!(desktop.scale, base.scale);
        assert_eq!(desktop.rotation, base.rotation);
        assert_eq!(motion.sample(1.0, &Pose::default()).unwrap(), pose);
    }
    #[test]
    fn transforms_are_absolute_and_pivots_do_not_accumulate() {
        let base = Transform::from_xyz(10.0, 20.0, 3.0);
        let pose = Pose {
            x: 100.0,
            y: 30.0,
            rotation: 90.0,
            pivot_x: 0.0,
            ..default()
        };
        let sampled = transform(base, &pose, Vec2::new(40.0, 60.0), Vec2::ONE, false);
        assert!((sampled.translation.x - 90.0).abs() < 0.001);
        assert!((sampled.translation.y + 30.0).abs() < 0.001);
        assert_eq!(
            transform(base, &pose, Vec2::new(40.0, 60.0), Vec2::ONE, false),
            sampled
        );
    }
    #[test]
    fn reset_restores_the_exact_original_visual() {
        let mut world = World::new();
        let color = Color::srgba(0.2, 0.4, 0.7, 0.8);
        let entity = world
            .spawn((Transform::from_xyz(10.0, 20.0, 0.0), BackgroundColor(color)))
            .id();
        colors(
            &mut world,
            entity,
            &Pose {
                opacity: 0.25,
                ..default()
            },
        );
        assert_eq!(
            world
                .get::<BackgroundColor>(entity)
                .unwrap()
                .0
                .to_srgba()
                .alpha,
            0.2
        );
        reset(&mut world);
        assert_eq!(world.get::<BackgroundColor>(entity).unwrap().0, color);
        assert!(world.get::<Original>(entity).is_none());
    }
}
