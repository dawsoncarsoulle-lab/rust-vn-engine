use bevy::prelude::*;
use rvn_parser::{AnimationParam, Position, Transition};

use super::{make_fade_in, make_fade_out, FadeAnim, WIN_H, WIN_W};
use crate::components::{AnimationKind, SpriteAnimation, SpriteBaseTransform, VnSprite};
use crate::vn_command::VnCommand;
use std::collections::{HashMap, HashSet};

fn position_to_x(pos: &Position) -> f32 {
    match pos {
        Position::Left => -WIN_W * 0.30,
        Position::Center => 0.0,
        Position::Right => WIN_W * 0.30,
        Position::Custom(n) => (n - 0.5) * WIN_W,
    }
}

pub(crate) fn sprite_default_transform(position: &Position) -> Transform {
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

pub(crate) fn apply_sprite_transform_style(
    transform: &mut Transform,
    style: &rvn_core::sprite_effects::SpriteStyle,
) {
    transform.scale = Vec3::new(
        if style.flip_x { -style.scale } else { style.scale },
        if style.flip_y { -style.scale } else { style.scale },
        1.0,
    );
    transform.rotation = Quat::from_rotation_z(style.rotation.to_radians());
}

pub(crate) fn build_sprite_animation(
    animation: &str,
    params: &[AnimationParam],
) -> Option<SpriteAnimation> {
    let effect=rvn_core::sprite_effects::prepare_sprite_animation(animation,params).ok()?;
    let rvn_core::scene_resources::EffectParameters::SpriteAnimation{animation,..}=effect.parameters else {return None;};
    let kind=match animation {rvn_core::scene_resources::SpriteEffectKind::Shake{intensity}=>AnimationKind::Shake{intensity},rvn_core::scene_resources::SpriteEffectKind::Bounce{height}=>AnimationKind::Bounce{height},rvn_core::scene_resources::SpriteEffectKind::Pulse{scale}=>AnimationKind::Pulse{scale}};
    Some(SpriteAnimation {
        kind,
        duration_secs:effect.clock.duration as f32,
        elapsed_secs: 0.0,
        looping:effect.clock.looping,
    })
}

#[derive(Component)]
pub struct SpriteStage;

#[cfg(test)]
mod resize_tests {
    use super::*;
    #[test]
    fn stage_fits_small_large_and_portrait_windows_without_accumulating_scale() {
        let mut app = App::new();
        let window = app.world_mut().spawn(Window::default()).id();
        let stage = app
            .world_mut()
            .spawn((SpriteStage, Transform::default()))
            .id();
        app.add_systems(Update, resize_sprite_stage);
        for (width, height) in [
            (800.0, 450.0),
            (1920.0, 1080.0),
            (450.0, 800.0),
            (1280.0, 720.0),
        ] {
            app.world_mut()
                .get_mut::<Window>(window)
                .unwrap()
                .resolution
                .set(width, height);
            app.update();
            let transform = app.world().get::<Transform>(stage).unwrap();
            assert!(WIN_W * transform.scale.x <= width + 0.01);
            assert!(WIN_H * transform.scale.y <= height + 0.01);
            assert!(
                (transform.translation.y - WIN_H * transform.scale.y / 2.0 + height / 2.0).abs()
                    < 0.01
            );
        }
    }
}

pub fn resize_sprite_stage(
    windows: Query<&Window>,
    mut stages: Query<&mut Transform, With<SpriteStage>>,
) {
    let Ok(window) = windows.get_single() else {
        return;
    };
    let scale = (window.width() / WIN_W)
        .min(window.height() / WIN_H)
        .max(0.001);
    for mut stage in &mut stages {
        stage.scale = Vec3::new(scale, scale, 1.0);
        stage.translation.y = (WIN_H * scale - window.height()) * 0.5;
    }
}

pub fn sprite_system(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    engine: Option<Res<crate::resources::VnEngine>>,
    startup: Option<Res<crate::debugger_startup::DebugBootstrap>>,
    stages: Query<Entity, With<SpriteStage>>,
    mut vn_events: EventReader<VnCommand>,
    mut queries: ParamSet<(
        Query<(Entity, &VnSprite), Without<crate::scene_retired::RetiredProjection>>,
        Query<(Entity, &VnSprite, &SpriteBaseTransform, &mut Transform), (With<SpriteAnimation>, Without<crate::scene_retired::RetiredProjection>)>,
        Query<(
            &VnSprite,
            &mut Transform,
            &mut Sprite,
            &mut SpriteBaseTransform,
        ), Without<crate::scene_retired::RetiredProjection>>,
    )>,
) {
    let state = match (engine.as_deref(), startup.as_deref()) {
        (Some(engine), None) => &engine.0.state,
        (None, Some(startup)) => startup.owner.state(),
        _ => return,
    };
    let mut spawned_this_frame: HashMap<String, Entity> = HashMap::new();
    let mut retired = HashSet::new();
    let cmds: Vec<VnCommand> = vn_events.read().cloned().collect();

    for cmd in cmds {
        match cmd {
            VnCommand::ClearSprites => {
                for entity in queries
                    .p0()
                    .iter()
                    .map(|(entity, _)| entity)
                    .chain(spawned_this_frame.values().copied())
                {
                    if retired.insert(entity) {
                        commands.entity(entity).despawn_recursive();
                    }
                }
                spawned_this_frame.clear();
            }
            VnCommand::ShowSprite {
                id,
                emotion,
                position,
                transition,
            } => {
                if state.layered.characters.contains_key(&id) {
                    continue;
                }
                let file = match &emotion {
                    Some(path) if path.contains('/') => {
                        path.strip_prefix("assets/").unwrap_or(path).to_owned()
                    }
                    Some(emo) => format!("sprites/{}/{}.png", id, emo),
                    None => format!("sprites/{}/default.png", id),
                };
                let texture: Handle<Image> = asset_server.load(file);
                let sprite_h = WIN_H * 0.85;
                let transition = if state.scene_execution.is_some() {Transition::None} else {transition};
                let has_anim = transition != Transition::None;
                let initial_alpha = if has_anim { 0.0 } else { 1.0 };
                let transform = sprite_default_transform(&position);
                let base = base_from_transform(&transform);

                let existing = spawned_this_frame
                    .get(&id)
                    .copied()
                    .filter(|e| !retired.contains(e))
                    .or_else(|| {
                        queries
                            .p0()
                            .iter()
                            .find(|(e, s)| s.id == id && !retired.contains(e))
                            .map(|(entity, _)| entity)
                    });

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

                    // A new explicit appearance cancels a previous pending fade-out.
                    commands.entity(entity).remove::<FadeAnim>();

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
                    if let Ok(stage) = stages.get_single() {
                        commands.entity(stage).add_child(entity);
                    }
                    spawned_this_frame.insert(id.clone(), entity);
                }
            }

            VnCommand::HideSprite { id, transition } => {
                let transition = if state.scene_execution.is_some() {Transition::None} else {transition};
                if state.layered.characters.contains_key(&id) {
                    continue;
                }
                let mut found = false;

                for (entity, sprite) in queries.p0().iter() {
                    if sprite.id == id && !retired.contains(&entity) {
                        found = true;
                        commands.entity(entity).remove::<SpriteAnimation>();

                        if let Some(anim) = make_fade_out(&transition) {
                            commands.entity(entity).insert(anim);
                        } else {
                            retired.insert(entity);
                            commands.entity(entity).despawn();
                        }
                    }
                }

                if !found {
                    if let Some(entity) =
                        spawned_this_frame.get(&id).filter(|e| !retired.contains(e))
                    {
                        commands.entity(*entity).remove::<SpriteAnimation>();

                        if let Some(anim) = make_fade_out(&transition) {
                            commands.entity(*entity).insert(anim);
                        } else {
                            retired.insert(*entity);
                            commands.entity(*entity).despawn();
                        }
                    }
                }
            }

            VnCommand::MoveSprite { id, position, .. } => {
                if state.layered.characters.contains_key(&id) {
                    continue;
                }
                let mut transform = sprite_default_transform(&position);
                if let Some(styles) = state.sprite_styles() {
                    let style = styles.get(&id).cloned().unwrap_or_default();
                    apply_sprite_transform_style(&mut transform, &style);
                }
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
                for (sprite, mut transform, mut sprite_vis, mut base) in queries.p2().iter_mut() {
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
                    base.scale = transform.scale;
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
    accessibility: Res<crate::accessibility::Accessibility>,
    mut query: Query<(
        Entity,
        &SpriteBaseTransform,
        &mut SpriteAnimation,
        &mut Transform,
        Option<&mut crate::layered_characters::LayeredFade>,
        Option<&crate::scene_effects::AnimationLease>,
    ), Without<crate::scene_retired::RetiredProjection>>,
    mut commands: Commands,
    engine:Option<Res<crate::resources::VnEngine>>,
    state:Option<Res<State<crate::resources::VnState>>>,
) {
    if accessibility.blocked {
        return;
    }
    for (entity, base, mut animation, mut transform, fade,lease) in query.iter_mut() {
        if lease.is_some() {continue;}
        if !crate::scene_effects::allowed(engine.as_deref(),state.as_deref(),lease.map(|lease|&lease.0)) {continue;}
        animation.elapsed_secs += time.delta_seconds();
        sample_sprite_animation(&animation,base,&mut transform,accessibility.settings.reduced_motion);

        if !animation.looping && animation.elapsed_secs >= animation.duration_secs {
            transform.translation = base.translation;
            transform.scale = base.scale;
            commands.entity(entity).remove::<SpriteAnimation>();
        }
        if let Some(mut fade) = fade {
            // The transition is applied afterwards to this frame's animated
            // pose; it must never replace the shake/bounce/pulse contribution.
            fade.base = *transform;
        }
    }
}
pub(crate) fn sample_sprite_animation(animation:&SpriteAnimation,base:&SpriteBaseTransform,pose:&mut Transform,reduced:bool) {
    pose.translation=base.translation;pose.scale=base.scale;
    if reduced || !animation.looping && animation.elapsed_secs>=animation.duration_secs {return;}
    let raw=if animation.duration_secs>0.0 {animation.elapsed_secs/animation.duration_secs} else {1.0};
    let progress=if animation.looping {raw.fract()} else {raw.min(1.0)};
    match animation.kind {
        AnimationKind::Shake{intensity}=>{
            let decay=if animation.looping {1.0} else {1.0-progress};
            pose.translation.x+=(progress*std::f32::consts::TAU*6.0).sin()*intensity*decay;
        }
        AnimationKind::Bounce{height}=>pose.translation.y+=(progress*std::f32::consts::PI).sin()*height,
        AnimationKind::Pulse{scale}=>pose.scale=base.scale*(1.0+(scale-1.0)*(progress*std::f32::consts::TAU).sin().max(0.0)),
    }
}
pub(crate) fn sample_owned_sprite_animation(animation:&SpriteAnimation,base:&SpriteBaseTransform,pose:&mut Transform,reduced:bool,elapsed:f64,duration:f64) {
    pose.translation=base.translation;pose.scale=base.scale;
    if reduced || !animation.looping && elapsed>=duration {return;}
    let raw=if duration>0.0 {elapsed/duration} else {1.0};
    let progress=if animation.looping {raw.fract()} else {raw.min(1.0)};
    match animation.kind {
        AnimationKind::Shake{intensity}=>{
            let decay=if animation.looping {1.0}else{1.0-progress};
            pose.translation.x+=((progress*std::f64::consts::TAU*6.0).sin()*f64::from(intensity)*decay) as f32;
        }
        AnimationKind::Bounce{height}=>pose.translation.y+=((progress*std::f64::consts::PI).sin()*f64::from(height)) as f32,
        AnimationKind::Pulse{scale}=>pose.scale=base.scale*(1.0+(f64::from(scale)-1.0)*(progress*std::f64::consts::TAU).sin().max(0.0)) as f32,
    }
}

#[cfg(test)]
mod move_style_tests {
    use super::*;
    use crate::{components::TransitionKind, resources::VnEngine};

    fn app(game: rvn_core::Engine<crate::bevy_renderer::BevyRenderer>) -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin{watch_for_changes_override:Some(false),..default()}));
        app.init_asset::<Image>()
            .insert_resource(VnEngine(game))
            .init_resource::<crate::scene_publication::ScenePresentation>()
            .add_event::<VnCommand>()
            .add_event::<crate::vn_command::PlayerInput>()
            .add_systems(Update, sprite_system);
        app
    }

    #[test]
    fn narrative_move_keeps_cached_game_sprite_style_on_the_real_entity_and_neutral_base() {
        use rvn_core::{scene_resources::TargetKind, scene_tasks::TaskOwner};
        let source = "scene label A\nhero.show(\"smile\") at left\nhero.effect(flip_x:true,flip_y:true,scale:1.25,rotation:37,tint:\"#c08040b3\")\n\"before\"\nhero.move() at right with slideleft(1000)\n\"after\"\n";
        let mut game = rvn_core::Engine::from_program(
            rvn_parser::parse_program(source).unwrap(),
            crate::bevy_renderer::BevyRenderer::new(),
            32,
        )
        .unwrap();
        game.step_until_interaction().unwrap();
        let execution = game.state.scene_execution.as_ref().unwrap();
        let style = execution.sprite_styles["hero"].clone();
        let target = execution
            .resources
            .target(&TaskOwner::Game, TargetKind::Sprite, "hero")
            .unwrap()
            .clone();
        let tasks_before = execution.resources.tasks.clone();
        let mut app = app(game);
        crate::scene_publication::consume(app.world_mut());
        // The existing image needs no renderer or file load for this Move test.
        app.world_mut().resource_mut::<Events<VnCommand>>().clear();
        let pose = sprite_default_transform(&Position::Left);
        let entity = app
            .world_mut()
            .spawn((
                VnSprite { id: "hero".into() },
                base_from_transform(&pose),
                pose,
                Sprite::default(),
            ))
            .id();
        crate::scene_effects::apply_styles_for_test(app.world_mut());
        assert_eq!(
            app.world().get::<Transform>(entity).unwrap().scale,
            Vec3::new(-1.25, -1.25, 1.0),
        );
        let expected_rotation = Quat::from_rotation_z(37.0f32.to_radians());
        assert_eq!(
            app.world().get::<Transform>(entity).unwrap().rotation,
            expected_rotation,
        );
        let color_before = app.world().get::<Sprite>(entity).unwrap().color;
        // These unowned components exercise the presenter's immediate Move
        // behavior; registered Core task cancellation has separate contracts.
        app.world_mut().entity_mut(entity).insert((
            SpriteAnimation {
                kind: AnimationKind::Bounce { height: 20.0 },
                duration_secs: 10.0,
                elapsed_secs: 2.0,
                looping: false,
            },
            FadeAnim {
                from: 0.0,
                to: 1.0,
                duration_secs: 10.0,
                elapsed_secs: 0.25,
                despawn_on_finish: false,
                kind: TransitionKind::Fade,
            },
        ));
        app.world_mut()
            .resource_mut::<VnEngine>()
            .0
            .advance_dialogue()
            .unwrap();
        app.world_mut()
            .resource_mut::<VnEngine>()
            .0
            .step_until_interaction()
            .unwrap();
        crate::scene_publication::consume(app.world_mut());
        // RunSystemOnce does not apply deferred Commands in Bevy 0.14.
        app.world_mut().run_schedule(Update);
        let expected_translation = Vec3::new(
            WIN_W * 0.30,
            -(WIN_H / 2.0) + (WIN_H * 0.85 / 2.0),
            10.0,
        );
        let pose = *app.world().get::<Transform>(entity).unwrap();
        assert_eq!(pose.translation, expected_translation);
        assert_eq!(pose.scale, Vec3::new(-1.25, -1.25, 1.0));
        assert_eq!(pose.rotation, expected_rotation);
        let base = app.world().get::<SpriteBaseTransform>(entity).unwrap();
        assert_eq!(base.translation, expected_translation);
        assert_eq!(base.scale, Vec3::new(-1.25, -1.25, 1.0));
        assert!(app.world().get::<SpriteAnimation>(entity).is_none());
        assert_eq!(
            app.world().get::<FadeAnim>(entity).unwrap().elapsed_secs,
            0.25,
        );
        assert_eq!(app.world().get::<Sprite>(entity).unwrap().color, color_before);
        // Re-enter the real style consumer with the already equal cache.
        crate::scene_effects::apply_styles_for_test(app.world_mut());
        assert_eq!(*app.world().get::<Transform>(entity).unwrap(), pose);
        assert_eq!(app.world().get::<Sprite>(entity).unwrap().color, color_before);
        let game = &app.world().resource::<VnEngine>().0;
        assert_eq!(game.state.sprites["hero"].position, Position::Right);
        assert_eq!(
            game.state.last_transition,
            Transition::SlideLeft { duration_ms: 1000 },
        );
        let execution = game.state.scene_execution.as_ref().unwrap();
        assert_eq!(execution.sprite_styles["hero"], style);
        assert_eq!(
            execution.resources.target(&TaskOwner::Game, TargetKind::Sprite, "hero"),
            Some(&target),
        );
        assert_eq!(execution.resources.tasks, tasks_before);
        let mut entities = app.world_mut().query::<(Entity, &VnSprite)>();
        assert_eq!(
            entities
                .iter(app.world())
                .filter(|(_, sprite)| sprite.id == "hero")
                .map(|(id, _)| id)
                .collect::<Vec<_>>(),
            vec![entity],
        );
    }

    #[test]
    fn legacy_move_still_resets_immediately_without_a_new_style_or_task_clock() {
        let game = rvn_core::Engine::new(
            Vec::new(),
            crate::bevy_renderer::BevyRenderer::new(),
            32,
        )
        .unwrap();
        assert!(game.state.scene_execution.is_none());
        let mut app = app(game);
        let pose = Transform {
            translation: Vec3::new(3.0, 4.0, 10.0),
            scale: Vec3::new(-1.25, -1.25, 1.0),
            rotation: Quat::from_rotation_z(37.0f32.to_radians()),
        };
        let entity = app
            .world_mut()
            .spawn((
                VnSprite { id: "hero".into() },
                base_from_transform(&pose),
                pose,
                Sprite::default(),
                SpriteAnimation {
                    kind: AnimationKind::Bounce { height: 20.0 },
                    duration_secs: 10.0,
                    elapsed_secs: 2.0,
                    looping: false,
                },
            ))
            .id();
        app.world_mut().send_event(VnCommand::MoveSprite {
            id: "hero".into(),
            position: Position::Custom(1.25),
            transition: Transition::SlideLeft { duration_ms: 1000 },
        });
        app.world_mut().run_schedule(Update);
        let pose = *app.world().get::<Transform>(entity).unwrap();
        assert_eq!(
            pose.translation,
            Vec3::new(WIN_W * 0.75, -(WIN_H / 2.0) + (WIN_H * 0.85 / 2.0), 10.0),
        );
        assert_eq!(pose.scale, Vec3::ONE);
        assert_eq!(pose.rotation, Quat::IDENTITY);
        assert_eq!(app.world().get::<SpriteBaseTransform>(entity).unwrap().scale, Vec3::ONE);
        assert!(app.world().get::<SpriteAnimation>(entity).is_none());
        assert!(app.world().get::<FadeAnim>(entity).is_none());
        assert!(app.world().resource::<VnEngine>().0.state.scene_execution.is_none());
    }
}

#[cfg(test)]
mod retired_current_contract_tests {
    use super::*;

    fn game(source:&str)->rvn_core::Engine<crate::bevy_renderer::BevyRenderer> {
        let mut game=rvn_core::Engine::from_program(rvn_parser::parse_program(source).unwrap(),crate::bevy_renderer::BevyRenderer::new(),32).unwrap();
        game.step_until_interaction().unwrap();
        game
    }
    fn outgoing(game:&rvn_core::Engine<crate::bevy_renderer::BevyRenderer>)->rvn_core::scene_resources::ResourceToken {
        let execution=game.state.scene_execution.as_ref().unwrap();
        let tasks:Vec<_>=execution.resources.tasks.iter().filter(|task|task.effect.as_ref().is_some_and(|effect|effect.presentation.is_some())).collect();
        assert_eq!(tasks.len(),1);
        execution.resources.effect_token(game.runtime_epoch(),&tasks[0].key,execution.owner()).unwrap()
    }
    fn assets(app:&mut App) {
        app.add_plugins((MinimalPlugins,bevy::asset::AssetPlugin{watch_for_changes_override:Some(false),..default()}));
        app.init_asset::<Image>();
    }

    #[test]
    fn modern_sprite_appearance_and_clear_are_confined_to_current_entities() {
        let game=game("scene label A\nhero.show(\"old\") at left\nhero.show(\"new\") at right with fade(2000)\n\"hold\"\n");
        let token=outgoing(&game);let before=serde_json::to_value(&game.state).unwrap();
        let mut app=App::new();assets(&mut app);app.insert_resource(crate::resources::VnEngine(game)).add_event::<VnCommand>().add_systems(Update,sprite_system);
        let old_pose=sprite_default_transform(&Position::Left);
        let current=app.world_mut().spawn((VnSprite{id:"hero".into()},base_from_transform(&old_pose),old_pose,Sprite::default())).id();
        let retired_pose=Transform::from_xyz(17.0,23.0,10.0);
        let retired=app.world_mut().spawn((VnSprite{id:"hero".into()},crate::scene_retired::RetiredProjection{token:token.clone()},base_from_transform(&retired_pose),retired_pose,Sprite{color:Color::srgba(0.3,0.4,0.5,0.6),..default()})).id();
        app.world_mut().send_event(VnCommand::ShowSprite{id:"hero".into(),emotion:Some("new".into()),position:Position::Right,transition:Transition::Fade{duration_ms:2000}});
        app.world_mut().run_schedule(Update);
        assert_eq!(*app.world().get::<Transform>(current).unwrap(),sprite_default_transform(&Position::Right));
        assert!(app.world().get::<FadeAnim>(current).is_none());
        assert_eq!(*app.world().get::<Transform>(retired).unwrap(),retired_pose);
        assert_eq!(app.world().get::<Sprite>(retired).unwrap().color,Color::srgba(0.3,0.4,0.5,0.6));
        app.world_mut().send_event(VnCommand::ClearSprites);app.world_mut().run_schedule(Update);
        assert!(app.world().get_entity(current).is_none());assert!(app.world().get_entity(retired).is_some());
        assert_eq!(serde_json::to_value(&app.world().resource::<crate::resources::VnEngine>().0.state).unwrap(),before);
    }
    #[test]
    fn legacy_sprite_show_keeps_local_fade_and_no_narrative_clock_is_created() {
        let game=game("label start\n\"hold\"\n");assert!(game.state.scene_execution.is_none());
        let mut app=App::new();assets(&mut app);app.insert_resource(crate::resources::VnEngine(game)).add_event::<VnCommand>().add_systems(Update,sprite_system);
        app.world_mut().send_event(VnCommand::ShowSprite{id:"hero".into(),emotion:Some("old".into()),position:Position::Center,transition:Transition::Fade{duration_ms:2000}});app.world_mut().run_schedule(Update);
        let current:Vec<_>=app.world_mut().query_filtered::<Entity,With<VnSprite>>().iter(app.world()).collect();assert_eq!(current.len(),1);
        let fade=app.world().get::<FadeAnim>(current[0]).unwrap();assert_eq!(fade.from,0.0);assert_eq!(fade.to,1.0);assert_eq!(fade.elapsed_secs,0.0);
        assert!(app.world().resource::<crate::resources::VnEngine>().0.state.scene_execution.is_none());
    }
}
