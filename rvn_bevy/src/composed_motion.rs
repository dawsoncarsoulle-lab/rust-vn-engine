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
            .add_systems(First, reset.run_if(crate::runtime_restart::presentation_available).after(crate::runtime_restart::activate).before(crate::debugger_startup::driver::drain_runtime))
            .add_systems(
                Update,
                ((
                    clock.run_if(resource_exists::<VnEngine>.and_then(crate::scene_publication::ready)).in_set(crate::scene_publication::MotionFeedbackSet),
                    receive.run_if(crate::scene_publication::projection_ready).after(crate::systems::stepping_system),
                    frame_failures.after(receive),
                )).distributive_run_if(crate::runtime_restart::presentation_available),
            )
            .add_systems(
                PostUpdate,
                (apply
                    .after(bevy::ui::UiSystem::Layout)
                    .before(bevy::transform::TransformSystem::TransformPropagate)
                    .in_set(MotionApply)).distributive_run_if(crate::runtime_restart::presentation_available),
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
/// Separate from the legacy alias tag so legacy fixtures/presenters preserve
/// their lookup while opted projections require an exact resident instance.
#[derive(Component,Clone,Debug,PartialEq,Eq)]
pub(crate) struct InterfaceProjectionTarget {
    pub interface: rvn_core::scene_resources::TargetIdentity,
    pub order: u64,
    pub host_role: Option<rvn_ui::PageRole>,
}
pub(crate) fn interface_target_matches(
    target:&InterfaceTarget, exact:Option<&InterfaceProjectionTarget>,
    screen:&str, element:&str, binding:Option<&rvn_core::ui_residency::UiProjectionBinding>,
) -> bool {
    if target.screen!=screen || target.element!=element {return false;}
    let Some(binding)=binding else {return true;};
    binding.residence==rvn_core::ui_residency::ProjectionResidence::Visible
        && binding.interface.alias==screen && binding.element==element
        && exact.is_some_and(|exact|exact.interface==binding.interface
            && exact.order==binding.order && exact.host_role==binding.host_role)
}
#[derive(Resource, Default)]
pub(crate) struct Poses(pub(crate) Vec<MotionView>);
#[derive(Resource, Default)]
pub(super) struct FrameImages(std::collections::BTreeMap<String, Handle<Image>>);
pub(crate) fn install_scene(world: &mut World, views: Vec<MotionView>) {
    world.insert_resource(Poses(views));
    if !world.contains_resource::<FrameImages>() { world.insert_resource(FrameImages::default()); }
}
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
    if engine.0.state.scene_execution.is_some() {return;}
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
pub(crate) fn receive(
    mut events: EventReader<VnCommand>,
    mut poses: ResMut<Poses>,
    engine: Option<Res<VnEngine>>,
    startup: Option<Res<crate::debugger_startup::DebugBootstrap>>,
    assets: Res<AssetServer>,
    mut frames: ResMut<FrameImages>,
) {
    let state = match (engine.as_deref(), startup.as_deref()) {
        (Some(engine), None) => &engine.0.state,
        (None, Some(startup)) => startup.owner.state(),
        _ => return,
    };
    for event in events.read() {
        if let VnCommand::Motions(views) = event {
            poses.0 = views.clone();
        }
    }
    let mut wanted: std::collections::BTreeSet<_> = state
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
pub(crate) fn reset_for_owner_replacement(world:&mut World){reset(world);}
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
// Select the SAME reduced pose for cover and for the later Motion sampler.
// The active published views decide the frame; no alias/incarnation is rebound.
pub(crate) fn background_frame_for_cover(
    views: &[MotionView], state: Option<&rvn_core::GameState>, reduced: bool,
) -> Option<String> {
    views.iter().filter(|view| matches!(view.target, MotionTarget::Background))
        .filter_map(|view| {
            let pose = if reduced {
                state.and_then(|state| state.motions.tracks.get(&view.target.key()))
                    .filter(|track| track.running).map(|track| &track.base).unwrap_or(&view.pose)
            } else { &view.pose };
            pose.frame.clone()
        }).last()
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
        if view.ui_projection.as_ref().is_some_and(|binding|
            binding.residence==rvn_core::ui_residency::ProjectionResidence::Parked) {continue;}
        if reduced {
            let state = match (world.get_resource::<VnEngine>(),
                world.get_resource::<crate::debugger_startup::DebugBootstrap>()) {
                (Some(engine), None) => Some(&engine.0.state),
                (None, Some(startup)) => Some(startup.owner.state()),
                _ => None,
            };
            if let Some(base) = state.and_then(|state| state.motions.tracks.get(&view.target.key()))
                .filter(|track| track.running).map(|track| track.base.clone()) {
                view.pose = base;
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
                .query::<(Entity, &InterfaceTarget, Option<&InterfaceProjectionTarget>)>()
                .iter(world)
                .filter(|(_, target, exact)| interface_target_matches(target,*exact,screen,element,view.ui_projection.as_ref()))
                .map(|(entity, _, _)| entity)
                .collect(),
        };
        for entity in entities {
            let ui = matches!(view.target, MotionTarget::Interface { .. });
            // Background has a texture-sized quad. Its selected image must
            // determine pivot size BEFORE that texture is installed. Sprite,
            // authored leaf and UI sizes retain their existing fixed policy.
            let selected_background = if matches!(view.target, MotionTarget::Background) {
                let selected = view.pose.frame.as_ref().map(|path| assets.load::<Image>(path.clone()))
                    .or_else(|| world.get::<Handle<Image>>(entity).cloned());
                let Some(selected) = selected else { continue; };
                let Some(image) = world.resource::<Assets<Image>>().get(&selected) else { continue; };
                if image.size().min_element() == 0 { continue; }
                Some(selected)
            } else { None };
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
                        selected_background.as_ref().or_else(|| world.get::<Handle<Image>>(entity))
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
    #[cfg(feature = "video")]
    fn parked_game_motion_does_not_touch_homonymous_child_and_visible_pose_requires_exact_instance() {
        let mut game=crate::video_visual::tests::resident_game_fixture();
        let original=crate::video_visual::tests::embedded_view(&game).ui_projection.unwrap();
        let mut app=App::new();
        app.add_plugins((MinimalPlugins,bevy::asset::AssetPlugin{watch_for_changes_override:Some(false),..default()}));
        let parent=app.world_mut().spawn((InterfaceTarget{screen:"shared".into(),element:"player".into()},
            InterfaceProjectionTarget{interface:original.interface.clone(),order:original.order,host_role:original.host_role},
            Transform::from_xyz(10.0,20.0,0.0))).id();
        let mut child_marker=InterfaceProjectionTarget{interface:original.interface.clone(),order:original.order+1,host_role:original.host_role};
        child_marker.interface.incarnation+=1;
        let child=app.world_mut().spawn((InterfaceTarget{screen:"shared".into(),element:"player".into()},child_marker,
            Transform::from_xyz(30.0,40.0,0.0))).id();
        game.advance_dialogue().unwrap();game.step_until_interaction().unwrap();
        game.tick_game_frame(0.5,rvn_core::GameFrameReports::default()).unwrap();
        assert_eq!(game.state.motions.tracks["ui:shared/player"].elapsed,0.5);
        let mut hidden=game.renderer.scene_batch.as_ref().unwrap().motions.iter().find(|view|matches!(&view.target,MotionTarget::Interface{screen,..} if screen=="shared")).unwrap().clone();
        hidden.pose.x=80.0;
        install_scene(app.world_mut(),vec![hidden]);
        apply(app.world_mut());
        assert_eq!(app.world().get::<Transform>(parent).unwrap().translation,Vec3::new(10.0,20.0,0.0));
        assert_eq!(app.world().get::<Transform>(child).unwrap().translation,Vec3::new(30.0,40.0,0.0));
        game.advance_dialogue().unwrap();game.step_until_interaction().unwrap();
        let mut visible=game.renderer.scene_batch.as_ref().unwrap().motions.iter().find(|view|matches!(&view.target,MotionTarget::Interface{screen,..} if screen=="shared")).unwrap().clone();
        assert_eq!(visible.ui_projection,Some(original));
        visible.pose.x=80.0;
        install_scene(app.world_mut(),vec![visible.clone()]);
        apply(app.world_mut());
        assert_eq!(app.world().get::<Transform>(parent).unwrap().translation,Vec3::new(90.0,20.0,0.0));
        assert_eq!(app.world().get::<Transform>(child).unwrap().translation,Vec3::new(30.0,40.0,0.0));
        visible.ui_projection=None;
        install_scene(app.world_mut(),vec![visible]);
        apply(app.world_mut());
        assert_eq!(app.world().get::<Transform>(child).unwrap().translation,Vec3::new(110.0,40.0,0.0));
        assert_eq!(game.state.motions.tracks["ui:shared/player"].elapsed,0.5);
    }
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

// PRIVATE fragment: append as a child module of rvn_bevy/src/composed_motion.rs.
// Keeps direct access to actual private transform/tinted and Bevy's glam path.
// Requires future rvn_core::effect_sampling. NOT_RUN; no pixel/GPU claim.
#[cfg(test)]
mod c11_shared_motion_comparisons {
    use super::*;
    use rvn_core::effect_sampling::{self as shared, MotionGeometry};
    use rvn_core::scene_resources::TransformSnapshot;
    use rvn_core::sprite_effects::SpriteStyle;

    // Pinned own-project C10/glam oracle; no Unreal source is copied.
    mod c10_reference {
        use super::*;
        pub(super) fn transform(
            base: Transform,
            pose: &Pose,
            size: Vec2,
            factor: Vec2,
            ui: bool,
        ) -> Transform {
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
        pub(super) fn tinted(color: Color, pose: &Pose) -> Color {
            let mut value = color.to_srgba();
            value.red *= pose.tint_r;
            value.green *= pose.tint_g;
            value.blue *= pose.tint_b;
            value.alpha *= pose.tint_a * pose.opacity;
            Color::from(value)
        }
    }

    fn neutral(value: &Transform) -> TransformSnapshot {
        TransformSnapshot {
            translation: value.translation.to_array(),
            scale: value.scale.to_array(),
            rotation: value.rotation.to_array(),
        }
    }
    fn finite_near(actual: f32, expected: f32, magnitude: f64, operations: f64) {
        assert!(actual.is_finite() && expected.is_finite());
        // Conservative forward-error allowance for scalar/SSE re-association,
        // not relative tolerance of the final result (which can cancel to 0).
        let tolerance = operations * f64::from(f32::EPSILON) * magnitude.max(1.0);
        assert!(
            (f64::from(actual) - f64::from(expected)).abs() <= tolerance,
            "actual={actual:?} expected={expected:?} tolerance={tolerance:?}"
        );
    }
    fn compare(base: Transform, pose: &Pose, size: Vec2, factor: Vec2, ui: bool) {
        pose.validate().unwrap();
        let base_before = base;
        let pose_before = pose.clone();
        let actual = transform(base, pose, size, factor, ui); // actual live Bevy/glam
        let expected = c10_reference::transform(base, pose, size, factor, ui);
        let sampled = shared::compose_motion(
            &neutral(&base),
            pose,
            MotionGeometry {
                size: size.to_array(),
                factor: factor.to_array(),
                ui,
            },
        );
        let p = [
            (pose.pivot_x - 0.5) * size.x * base.scale.x,
            (pose.pivot_y - 0.5) * size.y * base.scale.y,
        ];
        let pivot_sum = f64::from(p[0].abs()) + f64::from(p[1].abs());
        let max_scale = f64::from(pose.scale_x.abs().max(pose.scale_y.abs()).max(1.0));
        for index in 0..3 {
            let delta = match index {
                0 => pose.x * factor.x,
                1 => pose.y * factor.y,
                _ => 0.0,
            };
            let magnitude = f64::from(base.translation[index].abs())
                + f64::from(delta.abs())
                + 4.0 * pivot_sum * (1.0 + max_scale);
            finite_near(
                actual.translation[index],
                expected.translation[index],
                magnitude,
                64.0,
            );
            finite_near(
                sampled.translation[index],
                expected.translation[index],
                magnitude,
                64.0,
            );
            assert_eq!(
                actual.scale[index].to_bits(),
                expected.scale[index].to_bits()
            );
            assert_eq!(
                sampled.scale[index].to_bits(),
                expected.scale[index].to_bits()
            );
        }
        for (left, right) in actual
            .rotation
            .to_array()
            .into_iter()
            .chain(sampled.rotation)
            .zip(expected.rotation.to_array().into_iter().cycle())
        {
            finite_near(left, right, 1.001, 16.0);
        }
        assert_eq!(base, base_before);
        assert_eq!(pose, &pose_before);
    }
    #[test]
    fn shared_motion_matches_actual_glam_for_allowed_nonunit_quaternions_signed_styles_pivots_and_ui(
    ) {
        let pose = Pose {
            x: 73.25,
            y: -48.5,
            rotation: 67.3,
            scale_x: 1.75,
            scale_y: 0.6,
            pivot_x: 0.17,
            pivot_y: 0.83,
            ..default()
        };
        for (scale, rotation, flip_x, flip_y) in [
            (0.8, 37.0, true, false),
            (-1.2, -93.0, false, true),
            (0.001, 720.25, true, true),
        ] {
            let style = SpriteStyle {
                scale,
                rotation,
                flip_x,
                flip_y,
                tint: None,
            };
            let mut base = Transform::from_xyz(19.5, -80.75, 10.0);
            crate::systems::sprite::apply_sprite_transform_style(&mut base, &style);
            for norm_offset in [-0.00099f32, 0.0, 0.00099] {
                let factor_norm = (1.0 + norm_offset).sqrt();
                let q = base.rotation.to_array().map(|value| value * factor_norm);
                let norm: f64 = q.into_iter().map(|value| f64::from(value).powi(2)).sum();
                assert!((norm - 1.0).abs() <= 0.001);
                base.rotation = Quat::from_xyzw(q[0], q[1], q[2], q[3]);
                for ui in [false, true] {
                    compare(
                        base,
                        &pose,
                        Vec2::new(713.5, 321.25),
                        Vec2::new(0.75, 1.25),
                        ui,
                    );
                }
                // Every norm fixture starts from the authored styled base.
                crate::systems::sprite::apply_sprite_transform_style(&mut base, &style);
            }
        }
    }
    #[test]
    fn shared_motion_matches_actual_glam_for_scalar_nonunit_base_without_identity_shortcut() {
        let base = Transform {
            rotation: Quat::from_xyzw(0.0, 0.0, 0.0, 1.0001),
            ..default()
        };
        let norm = base
            .rotation
            .to_array()
            .into_iter()
            .map(|value| f64::from(value).powi(2))
            .sum::<f64>();
        assert!((norm - 1.0).abs() <= 0.001);
        let pose = Pose {
            x: 10.0,
            y: 20.0,
            rotation: 90.0,
            pivot_x: 1.0,
            pivot_y: 0.5,
            ..default()
        };
        compare(base, &pose, Vec2::new(100.0, 50.0), Vec2::ONE, false);
        let sampled = shared::compose_motion(
            &neutral(&base),
            &pose,
            MotionGeometry {
                size: [100.0, 50.0],
                factor: [1.0, 1.0],
                ui: false,
            },
        );
        let measured = transform(base, &pose, Vec2::new(100.0, 50.0), Vec2::ONE, false);
        let wrong = transform(
            Transform::default(),
            &pose,
            Vec2::new(100.0, 50.0),
            Vec2::ONE,
            false,
        );
        finite_near(sampled.translation[0], measured.translation.x, 100.0, 16.0);
        assert!((measured.translation.x - wrong.translation.x).abs() > 0.009);
    }

    #[test]
    fn shared_motion_uses_base_times_motion_rotation_not_reverse_or_normalized_base() {
        let mut base = Transform {
            translation: Vec3::new(17.0, 29.0, -3.0),
            scale: Vec3::new(-0.9, 1.3, 1.0),
            rotation: Quat::from_euler(EulerRot::XYZ, 0.33, -0.21, 0.71),
        };
        let q = base.rotation.to_array().map(|value| value * 1.0001);
        base.rotation = Quat::from_xyzw(q[0], q[1], q[2], q[3]);
        let pose = Pose {
            rotation: 83.0,
            pivot_x: 1.0,
            pivot_y: 0.0,
            x: 11.0,
            y: 7.0,
            scale_x: 1.7,
            scale_y: 0.45,
            ..default()
        };
        compare(
            base,
            &pose,
            Vec2::new(1200.0, 950.0),
            Vec2::new(1.2, 0.8),
            false,
        );
        let expected = transform(
            base,
            &pose,
            Vec2::new(1200.0, 950.0),
            Vec2::new(1.2, 0.8),
            false,
        );
        let reverse = Quat::from_rotation_z(-pose.rotation.to_radians()) * base.rotation;
        assert!(
            (expected.rotation.dot(reverse).abs() - expected.rotation.length_squared()).abs()
                > 0.001
        );
        let normalized = Transform {
            rotation: base.rotation.normalize(),
            ..base
        };
        let wrong = transform(
            normalized,
            &pose,
            Vec2::new(1200.0, 950.0),
            Vec2::new(1.2, 0.8),
            false,
        );
        assert!((expected.translation - wrong.translation).length() > 0.01);
    }
    #[test]
    fn shared_motion_keeps_independent_noncentral_pivot_and_opposite_world_ui_sign_fixtures() {
        let base = Transform::default();
        let pose = Pose {
            x: 10.0,
            y: 20.0,
            rotation: 90.0,
            pivot_x: 1.0,
            pivot_y: 0.5,
            ..default()
        };
        for (ui, expected_y) in [(false, 30.0f32), (true, -30.0)] {
            let geometry = MotionGeometry {
                size: [100.0, 50.0],
                factor: [1.0, 1.0],
                ui,
            };
            let sampled = shared::compose_motion(&neutral(&base), &pose, geometry);
            finite_near(sampled.translation[0], 60.0, 100.0, 16.0);
            finite_near(sampled.translation[1], expected_y, 100.0, 16.0);
            compare(base, &pose, Vec2::new(100.0, 50.0), Vec2::ONE, ui);
        }
    }
    #[test]
    fn shared_motion_color_matches_actual_srgba_association_and_keeps_root_layer_contributions_separate(
    ) {
        let original = [0.8f32, 0.6, 0.4, 0.5637547969818115];
        let root = Pose {
            tint_r: 0.5,
            tint_g: 0.75,
            tint_b: 0.2,
            tint_a: 0.3572195768356323,
            opacity: 0.9000823497772217,
            ..default()
        };
        let layer = Pose {
            tint_r: 0.25,
            tint_g: 0.3,
            tint_b: 0.8,
            tint_a: 0.7,
            opacity: 0.4,
            ..default()
        };
        let actual_root = tinted(
            Color::srgba(original[0], original[1], original[2], original[3]),
            &root,
        )
        .to_srgba();
        let pinned_root = c10_reference::tinted(
            Color::srgba(original[0], original[1], original[2], original[3]),
            &root,
        )
        .to_srgba();
        assert_eq!(actual_root, pinned_root);
        let sampled_root = shared::motion_color(original, &root);
        for (left, right) in sampled_root.into_iter().zip([
            actual_root.red,
            actual_root.green,
            actual_root.blue,
            actual_root.alpha,
        ]) {
            assert_eq!(left.to_bits(), right.to_bits());
        }
        let actual_layer = tinted(Color::from(actual_root), &layer).to_srgba();
        let sampled_layer = shared::motion_color(sampled_root, &layer);
        for (left, right) in sampled_layer.into_iter().zip([
            actual_layer.red,
            actual_layer.green,
            actual_layer.blue,
            actual_layer.alpha,
        ]) {
            assert_eq!(left.to_bits(), right.to_bits());
        }
        let correct = original[3] * (root.tint_a * root.opacity);
        assert_eq!(sampled_root[3].to_bits(), correct.to_bits());
        // A known adversarial association fixture from Root's independent math
        // test is used below; no loose tolerance may hide a changed grouping.
        let a = 0.4523795545101166f32;
        let tint = 0.5597723722457886f32;
        let opacity = 0.9242106080055237f32;
        let pose = Pose {
            tint_a: tint,
            opacity,
            ..default()
        };
        let actual = tinted(Color::srgba(1.0, 1.0, 1.0, a), &pose)
            .to_srgba()
            .alpha;
        assert_eq!(actual.to_bits(), 0x3e6fa784);
        assert_eq!(((a * tint) * opacity).to_bits(), 0x3e6fa785);
        assert_eq!(
            shared::motion_color([1.0, 1.0, 1.0, a], &pose)[3].to_bits(),
            actual.to_bits()
        );
    }
}

// PRIVATE NOT_RUN. Append this complete child module to composed_motion.rs.
// Uses the real reset -> Current effect projection -> apply -> propagation
// schedule. No second Core clock, asset decoder, render loop or GPU is added.
#[cfg(all(test, not(target_arch = "wasm32")))]
mod c12_layer_fade_scheduled_contracts {
    use super::*;
    use crate::components::{SpriteBaseTransform, VnSprite};
    use crate::layered_characters::{CompositionRoot, LayerTarget};
    use rvn_core::scene_resources::{
        EffectClock, EffectParameters, ResourceTaskKind, ResourceToken, SavedEffect, TargetKind,
        TransformSnapshot, VisualTransition,
    };
    use rvn_core::scene_tasks::TaskOwner;
    use rvn_ui::motion::{Easing, Motion, PosePatch};

    struct Fixture {
        app: App,
        root: Entity,
        leaf: Entity,
        fade: ResourceToken,
    }
    fn snapshot(pose: &Transform) -> TransformSnapshot {
        TransformSnapshot {
            translation: pose.translation.to_array(),
            scale: pose.scale.to_array(),
            rotation: pose.rotation.to_array(),
        }
    }
    fn refresh_current_effects(world: &mut World) {
        let (tasks, epoch) = {
            let game = &world.resource::<VnEngine>().0;
            let execution = game.state.scene_execution.as_ref().unwrap();
            (
                execution
                    .resources
                    .active_effects(execution.owner())
                    .into_iter()
                    .cloned()
                    .collect(),
                game.runtime_epoch(),
            )
        };
        crate::scene_effects::install(world, tasks, epoch);
        crate::scene_effects::reconcile_for_test(world);
    }
    fn fixture() -> Fixture {
        let mut game=rvn_core::Engine::from_program(rvn_parser::parse_program(
            "scene label A\neileen.show(\"smile\") at left\neileen.effect(tint:\"#80402066\")\n\"hold\"\n"
        ).unwrap(),crate::bevy_renderer::BevyRenderer::new(),32).unwrap();
        game.step_until_interaction().unwrap();
        let composition=rvn_ui::composition::Composition::parse(serde_json::json!({
            "size":[300.0,600.0],"layers":[{"id":"body","image":"sprites/eileen/body.png","opacity":0.8}]
        })).unwrap();
        game.state
            .layered
            .compose("eileen".into(), composition.clone())
            .unwrap();
        game.state
            .motions
            .play(
                MotionTarget::Layer {
                    id: "eileen".into(),
                    layer: "body".into(),
                },
                Motion::Tween {
                    seconds: 10.0,
                    from: PosePatch::default(),
                    to: PosePatch {
                        x: Some(120.0),
                        y: Some(60.0),
                        rotation: Some(40.0),
                        scale_x: Some(1.4),
                        scale_y: Some(1.2),
                        opacity: Some(0.6),
                        tint_r: Some(0.8),
                        tint_g: Some(0.7),
                        tint_a: Some(0.5),
                        ..default()
                    },
                    curve: Easing::Linear,
                },
            )
            .unwrap();
        game.state
            .motions
            .play(
                MotionTarget::Sprite {
                    id: "eileen".into(),
                },
                Motion::Tween {
                    seconds: 10.0,
                    from: PosePatch::default(),
                    to: PosePatch {
                        x: Some(60.0),
                        y: Some(-20.0),
                        opacity: Some(0.8),
                        tint_r: Some(0.8),
                        ..default()
                    },
                    curve: Easing::Linear,
                },
            )
            .unwrap();
        rvn_core::scene_resources::sync_current(&mut game.state).unwrap();
        let owner = TaskOwner::Scene {
            activation: game
                .state
                .scene_execution
                .as_ref()
                .unwrap()
                .owner()
                .unwrap(),
        };
        let resources = &game.state.scene_execution.as_ref().unwrap().resources;
        let leaf_target = resources
            .target(&TaskOwner::Game, TargetKind::Layer, "eileen/body")
            .unwrap()
            .clone();
        let root_target = resources
            .target(&TaskOwner::Game, TargetKind::Sprite, "eileen")
            .unwrap()
            .clone();
        let local = Transform::from_xyz(10.0, 20.0, 0.01);
        let fade = game
            .register_scene_effect(
                rvn_core::scene_resources::resource_key(
                    owner.clone(),
                    ResourceTaskKind::Effect,
                    "layer/eileen/body/fade",
                ),
                leaf_target,
                SavedEffect {
                    presentation: None,
                    base_resolved: true,
                    clock: EffectClock {
                        duration: 10.0,
                        elapsed: 0.0,
                        looping: false,
                    },
                    parameters: EffectParameters::VisualFade {
                        from: 0.0,
                        to: 1.0,
                        despawn_on_finish: false,
                        transition: VisualTransition::SlideLeft,
                        base: snapshot(&local),
                        layered: true,
                    },
                },
            )
            .unwrap();
        game.register_scene_effect(
            rvn_core::scene_resources::resource_key(
                owner,
                ResourceTaskKind::Effect,
                "sprite/eileen/fade",
            ),
            root_target,
            SavedEffect {
                presentation: None,
                base_resolved: true,
                clock: EffectClock {
                    duration: 5.0,
                    elapsed: 0.0,
                    looping: false,
                },
                parameters: EffectParameters::VisualFade {
                    from: 0.0,
                    to: 1.0,
                    despawn_on_finish: false,
                    transition: VisualTransition::Fade,
                    base: TransformSnapshot::default(),
                    layered: true,
                },
            },
        )
        .unwrap();
        // Advance the actual engine once; the scheduled systems below only
        // sample the authoritative descriptors at this exact saved instant.
        game.tick_game_frame(2.5, rvn_core::GameFrameReports::default())
            .unwrap();
        let publication = game.renderer.scene_batch.take().unwrap();
        assert_eq!(game.state.motions.tracks["layer:eileen/body"].elapsed, 2.5);
        let directory = bevy::asset::io::memory::Dir::default();
        let reader = bevy::asset::io::memory::MemoryAssetReader { root: directory };
        let mut app = App::new();
        app.register_asset_source(
            bevy::asset::io::AssetSourceId::Default,
            bevy::asset::io::AssetSource::build().with_reader(move || Box::new(reader.clone())),
        );
        app.add_plugins((
            MinimalPlugins,
            bevy::asset::AssetPlugin {
                watch_for_changes_override: Some(false),
                ..default()
            },
            bevy::transform::TransformPlugin,
        ));
        app.init_asset::<Image>()
            .insert_resource(VnEngine(game))
            .insert_resource(State::new(VnState::Waiting))
            .init_resource::<crate::accessibility::Accessibility>()
            .init_resource::<crate::resources::MusicEntity>();
        install_scene(app.world_mut(), publication.motions);
        let base = crate::systems::sprite::sprite_default_transform(&rvn_parser::Position::Left);
        let root = app
            .world_mut()
            .spawn((
                VnSprite {
                    id: "eileen".into(),
                },
                CompositionRoot {
                    extent: Vec2::new(300.0, 600.0),
                    position: rvn_parser::Position::Left,
                    tint: Color::WHITE,
                },
                SpriteBaseTransform {
                    translation: base.translation,
                    scale: base.scale,
                },
                SpatialBundle {
                    transform: base,
                    ..default()
                },
            ))
            .id();
        let leaf = app
            .world_mut()
            .spawn((
                LayerTarget {
                    character: "eileen".into(),
                    layer: "body".into(),
                    description: composition.layers[0].clone(),
                },
                SpriteBundle {
                    transform: local,
                    sprite: Sprite {
                        custom_size: Some(Vec2::new(300.0, 600.0)),
                        ..default()
                    },
                    ..default()
                },
            ))
            .id();
        app.world_mut().entity_mut(root).add_child(leaf);
        app.add_systems(First, reset)
            .add_systems(Update, refresh_current_effects)
            .add_systems(
                PostUpdate,
                apply
                    .before(bevy::transform::TransformSystem::TransformPropagate)
                    .in_set(MotionApply),
            );
        Fixture {
            app,
            root,
            leaf,
            fade,
        }
    }
    fn core(world: &World) -> serde_json::Value {
        serde_json::to_value(&world.resource::<VnEngine>().0.state).unwrap()
    }
    fn assert_translation(actual: Vec3, expected: Vec3) {
        assert!(
            actual.abs_diff_eq(expected, 0.0001),
            "{actual:?} != {expected:?}"
        );
    }

    #[test]
    fn layer_fade_real_schedule_composes_leaf_then_root_motion_once_without_color_or_pose_accumulation(
    ) {
        let mut fixture = fixture();
        let before = core(fixture.app.world());
        for _ in 0..8 {
            fixture.app.update();
            let world = fixture.app.world();
            let tint = Srgba::hex("80402066").unwrap();
            // Root fade .5, leaf opacity .8 and its f64 fade .25 happen first.
            // Then leaf Motion contributes opacity .9/tint_a .875; finally
            // root Motion recursively contributes opacity .95 to the leaf.
            let alpha_before_motion = (f64::from(tint.alpha * 0.8 * 0.5) * 0.25) as f32;
            let expected_alpha = (alpha_before_motion * (0.875 * 0.9)) * 0.95;
            let color = world.get::<Sprite>(fixture.leaf).unwrap().color.to_srgba();
            assert_eq!(color.alpha.to_bits(), expected_alpha.to_bits());
            assert!((color.red - tint.red * 0.95 * 0.95).abs() < 0.000001);
            assert!((color.green - tint.green * 0.925).abs() < 0.000001);
            assert_eq!(color.blue, tint.blue);
            let leaf = world.get::<Transform>(fixture.leaf).unwrap();
            assert_translation(leaf.translation, Vec3::new(1470.0, 10.0, 0.01));
            assert!(leaf
                .rotation
                .abs_diff_eq(Quat::from_rotation_z(-10.0f32.to_radians()), 0.000001));
            assert!(leaf.scale.abs_diff_eq(Vec3::new(1.1, 1.05, 1.0), 0.000001));
            let root = world.get::<Transform>(fixture.root).unwrap();
            let base =
                crate::systems::sprite::sprite_default_transform(&rvn_parser::Position::Left);
            assert_translation(
                root.translation,
                base.translation + Vec3::new(10.0, 10.0 / 3.0, 0.0),
            );
            let global = world.get::<GlobalTransform>(fixture.leaf).unwrap();
            assert_translation(
                global.translation(),
                root.compute_matrix().transform_point3(leaf.translation),
            );
            assert_eq!(
                world.get::<Parent>(fixture.leaf).unwrap().get(),
                fixture.root
            );
            assert_eq!(
                crate::scene_effects::authoritative_effect(world, &fixture.fade)
                    .unwrap()
                    .clock
                    .elapsed,
                2.5
            );
            assert_eq!(core(world), before);
        }
    }

    #[test]
    fn layer_fade_real_schedule_reduced_and_cancelled_projection_keeps_the_core_clock_and_restores_local_pose_before_motion(
    ) {
        let mut fixture = fixture();
        fixture.app.update();
        fixture
            .app
            .world_mut()
            .resource_mut::<crate::accessibility::Accessibility>()
            .settings
            .reduced_motion = true;
        let before = core(fixture.app.world());
        fixture.app.update();
        let tint = Srgba::hex("80402066").unwrap();
        assert_translation(
            fixture
                .app
                .world()
                .get::<Transform>(fixture.leaf)
                .unwrap()
                .translation,
            Vec3::new(10.0, 20.0, 0.01),
        );
        assert_eq!(
            fixture
                .app
                .world()
                .get::<Sprite>(fixture.leaf)
                .unwrap()
                .color
                .alpha()
                .to_bits(),
            (tint.alpha * 0.8).to_bits()
        );
        assert_eq!(core(fixture.app.world()), before);
        fixture
            .app
            .world_mut()
            .resource_mut::<crate::accessibility::Accessibility>()
            .settings
            .reduced_motion = false;
        rvn_core::scene_resources::supersede_task(
            &mut fixture.app.world_mut().resource_mut::<VnEngine>().0.state,
            &fixture.fade.task.key,
        )
        .unwrap();
        let cancelled = core(fixture.app.world());
        for _ in 0..3 {
            fixture.app.update();
            let world = fixture.app.world();
            assert!(world
                .get::<crate::scene_effects::FadeLease>(fixture.leaf)
                .is_none());
            assert!(world
                .get::<crate::layered_characters::LayeredFade>(fixture.leaf)
                .is_none());
            assert!(crate::scene_effects::authoritative_effect(world, &fixture.fade).is_none());
            // Cancellation removes only the fade: the leaf's exact live Motion
            // still composes its x/y offsets with the restored local base.
            assert_translation(
                world.get::<Transform>(fixture.leaf).unwrap().translation,
                Vec3::new(30.0, 10.0, 0.01),
            );
            let expected_alpha = ((tint.alpha * 0.8 * 0.5) * (0.875 * 0.9)) * 0.95;
            assert_eq!(
                world
                    .get::<Sprite>(fixture.leaf)
                    .unwrap()
                    .color
                    .alpha()
                    .to_bits(),
                expected_alpha.to_bits()
            );
            assert_eq!(
                world.resource::<VnEngine>().0.state.motions.tracks["layer:eileen/body"].elapsed,
                2.5
            );
            assert_eq!(core(world), cancelled);
        }
    }
}
// PRIVATE candidate tests. Append as a child of composed_motion.rs only after
// an explicit common Current/Retired geometry policy is approved.
// These exercise the actual cover/reset/apply consumers, not a copied sampler.
// NOT_RUN. No GPU, pixels, native window, or Unreal behavior is certified.
#[cfg(test)]
mod selected_background_frame_geometry_contracts {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::reflect as bevy_reflect;

    #[derive(Default, bevy::reflect::TypePath)]
    struct FixtureImageLoader;
    impl bevy::asset::AssetLoader for FixtureImageLoader {
        type Asset = Image;
        type Settings = ();
        type Error = std::io::Error;
        async fn load<'a>(
            &'a self,
            _reader: &'a mut bevy::asset::io::Reader<'_>,
            _settings: &'a Self::Settings,
            _context: &'a mut bevy::asset::LoadContext<'_>,
        ) -> Result<Image, Self::Error> {
            Ok(Image::default())
        }
        fn extensions(&self) -> &[&str] {
            &["png"]
        }
    }

    struct Fixture {
        app: App,
        window: Entity,
        background: Entity,
        original: Handle<Image>,
        selected: Handle<Image>,
    }

    fn fixture(pose: Pose) -> Fixture {
        let mut app = App::new();
        let directory = bevy::asset::io::memory::Dir::default();
        for path in ["images/wide.png", "images/portrait.png"] {
            directory.insert_asset(std::path::Path::new(path), vec![0u8]);
        }
        let reader = bevy::asset::io::memory::MemoryAssetReader { root: directory };
        app.register_asset_source(
            bevy::asset::io::AssetSourceId::Default,
            bevy::asset::io::AssetSource::build().with_reader(move || Box::new(reader.clone())),
        );
        app.add_plugins((
            MinimalPlugins,
            bevy::asset::AssetPlugin {
                watch_for_changes_override: Some(false),
                ..default()
            },
        ));
        app.init_asset::<Image>()
            .init_asset_loader::<FixtureImageLoader>()
            .init_resource::<crate::accessibility::Accessibility>()
            .insert_resource(Poses(vec![MotionView {
                target: MotionTarget::Background,
                pose,
                ui_projection: None,
            }]));
        let world = app.world_mut();
        let original = world.resource::<AssetServer>().load("images/wide.png");
        let selected = world.resource::<AssetServer>().load("images/portrait.png");
        for (handle, width, height) in [(&original, 640, 360), (&selected, 320, 640)] {
            let mut image = Image::default();
            image.resize(bevy::render::render_resource::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            });
            world.resource_mut::<Assets<Image>>().insert(handle.id(), image);
        }
        let mut window = Window::default();
        window.resolution.set(1280.0, 720.0);
        let window = world.spawn(window).id();
        let background = world
            .spawn((
                crate::components::VnBackground,
                CurrentMotionBackground,
                SpriteBundle {
                    texture: original.clone(),
                    transform: Transform::from_xyz(0.0, 0.0, -100.0),
                    ..default()
                },
            ))
            .id();
        Fixture { app, window, background, original, selected }
    }

    fn project(fixture: &mut Fixture) {
        let world = fixture.app.world_mut();
        world.run_system_once(crate::systems::background_cover_resize_system);
        apply(world);
    }

    #[test]
    fn selected_background_frame_cover_uses_selected_dimensions_on_the_same_entity() {
        let mut f = fixture(Pose {
            frame: Some("images/portrait.png".into()),
            ..default()
        });
        let expected_poses = f.app.world().resource::<Poses>().0.clone();
        project(&mut f);
        let world = f.app.world();
        assert_eq!(world.get::<Handle<Image>>(f.background), Some(&f.selected));
        assert!(world.get::<Sprite>(f.background).unwrap().custom_size.is_none());
        // Independent cover: max(1280/320, 720/640) = 4, not the old image's 2.
        assert_eq!(world.get::<Transform>(f.background).unwrap().scale, Vec3::new(4.0, 4.0, 1.0));
        assert_eq!(world.resource::<Poses>().0, expected_poses);
        assert_eq!(world.entities().len(), 2);
        assert_eq!(world.get::<Original>(f.background).unwrap().texture.as_ref(), Some(&f.original));
    }

    #[test]
    fn selected_background_pivot_and_resize_are_absolute_without_an_extra_clock() {
        let mut f = fixture(Pose {
            frame: Some("images/portrait.png".into()),
            rotation: 90.0,
            pivot_x: 0.5,
            pivot_y: 1.0,
            ..default()
        });
        let expected_poses = f.app.world().resource::<Poses>().0.clone();
        for (width, height, scale, pivot) in [
            (1280.0, 720.0, 4.0, 1280.0),
            (800.0, 600.0, 2.5, 800.0),
            (1280.0, 720.0, 4.0, 1280.0),
        ] {
            reset(f.app.world_mut());
            assert_eq!(f.app.world().get::<Handle<Image>>(f.background), Some(&f.original));
            f.app.world_mut().get_mut::<Window>(f.window).unwrap().resolution.set(width, height);
            project(&mut f);
            let world = f.app.world();
            let actual = world.get::<Transform>(f.background).unwrap();
            assert_eq!(actual.scale, Vec3::new(scale, scale, 1.0));
            // World y is opposite UI y. The selected frame's lower pivot is
            // rotated -90 degrees: [0,-pivot] -> [-pivot,0].
            assert!(actual.translation.abs_diff_eq(Vec3::new(pivot, -pivot, -100.0), 0.001));
            assert_eq!(world.get::<Handle<Image>>(f.background), Some(&f.selected));
            assert_eq!(world.resource::<Poses>().0, expected_poses);
            assert_eq!(world.entities().len(), 2);
        }
    }
// PRIVATE NOT_RUN. Insert inside selected_background_frame_geometry_contracts.
#[test]
fn selected_background_missing_asset_defers_motion_without_falling_back_or_changing_published_pose() {
    let mut f = fixture(Pose { frame: Some("images/portrait.png".into()), x: 30.0, ..default() });
    let expected = f.app.world().resource::<Poses>().0.clone();
    f.app.world_mut().resource_mut::<Assets<Image>>().remove(f.selected.id());
    project(&mut f);
    assert_eq!(f.app.world().get::<Handle<Image>>(f.background), Some(&f.original));
    assert_eq!(f.app.world().get::<Transform>(f.background).unwrap().scale, Vec3::ONE);
    assert_eq!(f.app.world().get::<Transform>(f.background).unwrap().translation, Vec3::new(0.0, 0.0, -100.0));
    assert!(f.app.world().get::<Original>(f.background).is_none());
    let mut image = Image::default();
    image.resize(bevy::render::render_resource::Extent3d { width: 320, height: 640, depth_or_array_layers: 1 });
    f.app.world_mut().resource_mut::<Assets<Image>>().insert(f.selected.id(), image);
    project(&mut f);
    assert_eq!(f.app.world().get::<Handle<Image>>(f.background), Some(&f.selected));
    assert_eq!(f.app.world().get::<Transform>(f.background).unwrap().scale, Vec3::new(4.0, 4.0, 1.0));
    assert_eq!(f.app.world().get::<Transform>(f.background).unwrap().translation, Vec3::new(20.0, 0.0, -100.0));
    assert_eq!(f.app.world().resource::<Poses>().0, expected);
    assert_eq!(f.app.world().entities().len(), 2);
}

#[test]
fn selected_sprite_frame_keeps_fixed_quad_pivot_and_does_not_adopt_background_cover() {
    let mut f = fixture(Pose::default());
    f.app.world_mut().resource_mut::<Poses>().0 = vec![MotionView {
        target: MotionTarget::Sprite { id: "hero".into() },
        pose: Pose { frame: Some("images/portrait.png".into()), rotation: 90.0,
            pivot_x: 0.5, pivot_y: 1.0, ..default() }, ui_projection: None,
    }];
    let sprite = f.app.world_mut().spawn((crate::components::VnSprite { id: "hero".into() },
        SpriteBundle { sprite: Sprite { custom_size: Some(Vec2::new(110.0, 200.0)), ..default() },
            texture: f.original.clone(), transform: Transform::from_xyz(1.0, 2.0, 10.0), ..default() })).id();
    apply(f.app.world_mut());
    let world = f.app.world();
    assert_eq!(world.get::<Sprite>(sprite).unwrap().custom_size, Some(Vec2::new(110.0, 200.0)));
    assert_eq!(world.get::<Transform>(sprite).unwrap().scale, Vec3::ONE);
    assert!(world.get::<Transform>(sprite).unwrap().translation.abs_diff_eq(Vec3::new(101.0, -98.0, 10.0), 0.001));
    assert_eq!(world.get::<Handle<Image>>(sprite), Some(&f.selected));
    assert_eq!(world.get::<Handle<Image>>(f.background), Some(&f.original));
}

}
