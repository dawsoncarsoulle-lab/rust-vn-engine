use bevy::prelude::*;

use super::{ensure_extension, make_fade_in, make_fade_out};
use crate::components::VnCinematic;
use crate::resources::CgAssetRegistry;
use crate::vn_command::VnCommand;

#[cfg(all(test,not(target_arch="wasm32")))]
#[path="cinematic_tests.rs"]
mod owned_cg_tests;

#[derive(Component,Clone,Debug,PartialEq,Eq)]
pub(crate) struct CurrentCinematic {pub target:rvn_core::scene_resources::TargetIdentity,pub epoch:u64,pub instance_id:u64}
#[derive(Resource,Default)]
pub(crate) struct CinematicPublicationSession {pub instance_id:Option<u64>}

pub(crate) fn resolve_cg_asset(world:&World,id:&str)->Result<String,String> {
    let path=world.get_resource::<CgAssetRegistry>().and_then(|registry|registry.0.get(id)).cloned()
        .unwrap_or_else(||ensure_extension(&format!("cgs/{id}")));
    if !rvn_ui::programmable::safe_asset_path(&path) {return Err("Invalid resolved CG asset path".into());}
    Ok(path)
}

pub(crate) fn preflight_cg(world:&World,state:&rvn_core::GameState)->Result<(),String> {
    let Some(id)=state.cinematic.current.as_ref() else {return Ok(());};
    let path=resolve_cg_asset(world,id)?;
    if let Some(assets)=world.get_resource::<AssetServer>() {
        let failed=assets.get_handle::<Image>(&path).and_then(|handle|assets.get_load_state(handle.id()));
        if let Some(bevy::asset::LoadState::Failed(error))=failed {return Err(format!("Current CG {id} asset failed: {error}"));}
    }
    Ok(())
}

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
    query: Query<(Entity,Option<&CurrentCinematic>), (With<VnCinematic>, Without<crate::scene_retired::RetiredProjection>)>,
    engine: Option<Res<crate::resources::VnEngine>>,
) {
    let cmds: Vec<VnCommand> = vn_events.read().cloned().collect();
    for cmd in cmds {
        match cmd {
            VnCommand::ShowCinematic { id, transition } => {
                let modern = engine.as_ref().is_some_and(|game|game.0.state.scene_execution.is_some());
                let current = if modern {engine.as_ref().and_then(|game|game.0.state.scene_execution.as_ref()).and_then(|execution|execution.resources.target(&rvn_core::scene_tasks::TaskOwner::Game,rvn_core::scene_resources::TargetKind::Cinematic,"cinematic")).cloned()} else {None};
                if modern && (current.is_none() || engine.as_ref().unwrap().0.state.cinematic.current.as_ref()!=Some(&id)) {continue;}
                if let Some(target)=&current {
                    if query.iter().any(|(_,marker)|marker.is_some_and(|marker|marker.target==*target && marker.epoch==engine.as_ref().unwrap().0.runtime_epoch() && marker.instance_id==engine.as_ref().unwrap().0.instance_id())) {continue;}
                }
                let transition = if modern {rvn_parser::Transition::None} else {transition_name_to_fade(transition.as_deref())};
                let has_anim = transition != rvn_parser::Transition::None;

                for (entity,_) in query.iter() {
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
                if let Some(target) = current {ec.insert(CurrentCinematic {target,epoch:engine.as_ref().unwrap().0.runtime_epoch(),instance_id:engine.as_ref().unwrap().0.instance_id()});}
                if let Some(anim) = make_fade_in(&transition) {
                    ec.insert(anim);
                }
            }
            VnCommand::HideCinematic { transition } => {
                let modern=engine.as_ref().is_some_and(|game|game.0.state.scene_execution.is_some());
                if modern && engine.as_ref().unwrap().0.state.cinematic.current.is_some() {continue;}
                let transition = if modern {rvn_parser::Transition::None} else {transition_name_to_fade(transition.as_deref())};
                for (entity,_) in query.iter() {
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
    mut query: Query<(&Handle<Image>, &mut Transform, Option<&mut crate::scene_effects::OwnedFadeBase>, Option<&crate::scene_effects::FadeLease>, Option<&CurrentCinematic>), (With<VnCinematic>, Without<crate::scene_retired::RetiredProjection>)>,
) {
    let Ok(window) = windows.get_single() else {
        return;
    };

    let window_w = window.resolution.width().max(1.0);
    let window_h = window.resolution.height().max(1.0);

    for (handle, mut transform, base, lease, current) in query.iter_mut() {
        let Some(image) = images.get(handle) else {
            continue;
        };

        let size = image.size();
        let image_w = (size.x as f32).max(1.0);
        let image_h = (size.y as f32).max(1.0);
        let scale = (window_w / image_w).max(window_h / image_h);
        let neutral = Transform::from_xyz(0.0,0.0,50.0).with_scale(Vec3::new(scale,scale,1.0));
        if let Some(mut base) = base {base.transform=neutral;}
        // Legacy cover only changes scale and x/y, preserving authored rotation/depth.
        // Modern owned sampling uses the complete neutral base after this system.
        if current.is_none() {
            transform.scale = Vec3::new(scale, scale, 1.0);
            transform.translation.x = 0.0;
            transform.translation.y = 0.0;
        } else if lease.is_none() {*transform=neutral;}
    }
}
