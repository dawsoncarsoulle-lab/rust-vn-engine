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
    bg_query: Query<Entity, (With<VnBackground>, Without<crate::scene_retired::RetiredProjection>)>,
    engine: Option<Res<crate::resources::VnEngine>>,
) {
    let cmds: Vec<VnCommand> = vn_events.read().cloned().collect();
    for cmd in cmds {
        let VnCommand::SetBackground { path, transition } = cmd else {
            continue;
        };

        // Opted transitions are projected from the exact Core tasks after this
        // desired-model handler; the legacy presenter keeps its local fade.
        let transition = if engine.as_ref().is_some_and(|engine|engine.0.state.scene_execution.is_some()) {Transition::None} else {transition};
        let has_anim = transition != Transition::None;

        for entity in bg_query.iter() {
            commands
                .entity(entity)
                .remove::<crate::components::CurrentMotionBackground>();
            if has_anim {
                if let Some(anim) = make_fade_out(&transition) {
                    commands.entity(entity).insert(anim);
                }
            } else {
                commands.entity(entity).despawn();
            }
        }

        if path.is_empty() {
            render_state.background = None;
            continue;
        }
        render_state.background = Some(path.clone());
        let full_path = ensure_extension(&path);
        let texture: Handle<Image> = asset_server.load(full_path);
        let initial_alpha = if has_anim { 0.0 } else { 1.0 };

        let mut ec = commands.spawn((
            VnBackground,
            crate::components::CurrentMotionBackground,
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
    assets: Option<Res<AssetServer>>,
    poses: Option<Res<crate::composed_motion::Poses>>,
    engine: Option<Res<crate::resources::VnEngine>>,
    accessibility: Option<Res<crate::accessibility::Accessibility>>,
    mut bg_query: Query<(&Handle<Image>, &mut Transform, Option<&crate::components::CurrentMotionBackground>), (With<VnBackground>, Without<crate::scene_retired::RetiredProjection>)>,
) {
    let Ok(window) = windows.get_single() else {
        return;
    };

    let window_w = window.resolution.width().max(1.0);
    let window_h = window.resolution.height().max(1.0);

    let reduced = accessibility.as_ref().is_some_and(|access| access.settings.reduced_motion);
    let frame = poses.as_ref().and_then(|poses| crate::composed_motion::background_frame_for_cover(
        &poses.0, engine.as_ref().map(|engine| &engine.0.state), reduced));
    let selected = frame.as_ref().and_then(|path| assets.as_ref()?.get_handle::<Image>(path.clone()));
    for (handle, mut transform, current) in &mut bg_query {
        // Legacy outgoing backgrounds have lost CurrentMotionBackground; their
        // own image stays authoritative, not the current replacement's frame.
        let texture = if current.is_some() && frame.is_some() {
            let Some(selected) = selected.as_ref() else { continue; };
            selected
        } else { handle };
        let Some(image) = images.get(texture) else { continue; };
        let Some(geometry) = crate::background_geometry::resolve(
            Vec2::new(window_w, window_h), image.size().as_vec2()) else { continue; };
        transform.scale = geometry.cover.scale;
        transform.translation.x = geometry.cover.translation.x;
        transform.translation.y = geometry.cover.translation.y;
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
    fn modern_background_handler_has_no_local_fade_and_does_not_touch_retired_aliases() {
        let game=game("scene label A\nscene \"old.png\"\nscene \"new.png\" with fade(2000)\n\"hold\"\n");
        let token=outgoing(&game);let before=serde_json::to_value(&game.state).unwrap();
        let mut app=App::new();assets(&mut app);
        app.insert_resource(crate::resources::VnEngine(game)).init_resource::<VnRenderState>().add_event::<VnCommand>().add_systems(Update,background_system);
        let old=app.world_mut().spawn((VnBackground,crate::components::CurrentMotionBackground,Sprite::default(),Transform::default())).id();
        // Also exercise the defensive filter if an old Current marker remains.
        let retired=app.world_mut().spawn((VnBackground,crate::components::CurrentMotionBackground,crate::scene_retired::RetiredProjection{token:token.clone()},Sprite::default(),Transform::from_xyz(17.0,23.0,-100.0))).id();
        app.world_mut().send_event(VnCommand::SetBackground{path:"new.png".into(),transition:Transition::Fade{duration_ms:2000}});
        app.world_mut().run_schedule(Update);
        assert!(app.world().get_entity(old).is_none());
        assert_eq!(app.world().get::<Transform>(retired).unwrap().translation,Vec3::new(17.0,23.0,-100.0));
        assert!(app.world().get::<crate::components::FadeAnim>(retired).is_none());
        let current:Vec<_>=app.world_mut().query_filtered::<Entity,(With<VnBackground>,Without<crate::scene_retired::RetiredProjection>)>().iter(app.world()).collect();
        assert_eq!(current.len(),1);assert!(app.world().get::<crate::components::FadeAnim>(current[0]).is_none());
        assert_eq!(serde_json::to_value(&app.world().resource::<crate::resources::VnEngine>().0.state).unwrap(),before);
    }
    #[test]
    fn legacy_background_retains_its_immediate_local_transition() {
        let mut app=App::new();assets(&mut app);app.init_resource::<VnRenderState>().add_event::<VnCommand>().add_systems(Update,background_system);
        let old=app.world_mut().spawn(VnBackground).id();
        app.world_mut().send_event(VnCommand::SetBackground{path:"legacy.png".into(),transition:Transition::Fade{duration_ms:2000}});
        app.world_mut().run_schedule(Update);
        let fade=app.world().get::<crate::components::FadeAnim>(old).unwrap();
        assert_eq!(fade.from,1.0);assert_eq!(fade.to,0.0);assert!(fade.despawn_on_finish);
        let current:Vec<_>=app.world_mut().query_filtered::<Entity,(With<VnBackground>,Without<crate::scene_retired::RetiredProjection>)>().iter(app.world()).collect();
        assert_eq!(current.len(),2);
        let incoming=*current.iter().find(|entity|**entity!=old).unwrap();
        let fade=app.world().get::<crate::components::FadeAnim>(incoming).unwrap();
        assert_eq!(fade.from,0.0);assert_eq!(fade.to,1.0);assert!(!fade.despawn_on_finish);
    }
}
