//! Private child of systems/cinematic.rs. Actual Core/ECS; NOT_RUN.
use super::*;
use bevy::ecs::system::RunSystemOnce;
use bevy::reflect as bevy_reflect;
use crate::resources::{VnEngine,VnState};
use rvn_core::{Renderer,GameFrameReports};
use rvn_core::scene_resources::{TargetKind,ResourceToken};
use rvn_core::scene_tasks::TaskOwner;

#[derive(Default,bevy::reflect::TypePath)]
struct FixtureImageLoader;
fn image(width:u32,height:u32)->Image {
    let mut image=Image::default();
    image.resize(bevy::render::render_resource::Extent3d{width,height,depth_or_array_layers:1});
    image
}
impl bevy::asset::AssetLoader for FixtureImageLoader {
    type Asset=Image;type Settings=();type Error=std::io::Error;
    async fn load<'a>(&'a self,_reader:&'a mut bevy::asset::io::Reader<'_>,_settings:&'a (),context:&'a mut bevy::asset::LoadContext<'_>)->Result<Image,Self::Error> {
        Ok(if context.path().ends_with("cg-new.png") {image(200,100)} else {image(100,100)})
    }
    fn extensions(&self)->&[&str] {&["png"]}
}
fn game(source:&str)->rvn_core::Engine<crate::bevy_renderer::BevyRenderer> {
    let mut game=rvn_core::Engine::from_program(rvn_parser::parse_program(source).unwrap(),crate::bevy_renderer::BevyRenderer::new(),32).unwrap();
    game.step_until_interaction().unwrap();game
}
#[derive(Resource)]
struct FixtureImageHandles(Vec<Handle<Image>>);
fn app(source:&str)->App {
    let mut app=App::new();
    let directory=bevy::asset::io::memory::Dir::default();
    for path in ["images/cg-old.png","images/cg-new.png"] {directory.insert_asset(std::path::Path::new(path),vec![0u8]);}
    let reader=bevy::asset::io::memory::MemoryAssetReader{root:directory};
    app.register_asset_source(bevy::asset::io::AssetSourceId::Default,bevy::asset::io::AssetSource::build().with_reader(move||Box::new(reader.clone())));
    app.add_plugins((MinimalPlugins,bevy::asset::AssetPlugin{watch_for_changes_override:Some(false),..default()}));
    app.init_asset_loader::<FixtureImageLoader>().init_asset::<Image>();
    app.insert_resource(VnEngine(game(source))).insert_resource(State::new(VnState::Waiting))
        .init_resource::<NextState<VnState>>().init_resource::<crate::resources::ScriptErrorMessage>()
        .init_resource::<crate::accessibility::Accessibility>().init_resource::<crate::resources::VnRenderState>().init_resource::<crate::scene_publication::ScenePresentation>()
        .add_event::<VnCommand>().add_event::<crate::vn_command::PlayerInput>()
        .insert_resource(CgAssetRegistry(std::collections::HashMap::from([("old".into(),"images/cg-old.png".into()),("new".into(),"images/cg-new.png".into())])));
    app.world_mut().spawn((Window{resolution:(800.,600.).into(),..default()},bevy::window::PrimaryWindow));
    let mut held=Vec::new();
    for (path,width) in [("images/cg-old.png",100),("images/cg-new.png",200)] {
        let handle=app.world().resource::<AssetServer>().load::<Image>(path);
        app.world_mut().resource_mut::<Assets<Image>>().insert(handle.id(),image(width,100));
        held.push(handle);
    }
    app.insert_resource(FixtureImageHandles(held));
    app.add_systems(First,crate::scene_publication::consume)
        .add_systems(Update,(cinematic_system,cinematic_cover_resize_system.after(cinematic_system)))
        .add_plugins((crate::scene_effects::SceneEffectsPlugin,crate::scene_retired::SceneRetiredPlugin));
    app
}
fn current(world:&World)->(Entity,CurrentCinematic) {
    let found:Vec<_>=world.iter_entities().filter_map(|entity|entity.get::<CurrentCinematic>().map(|marker|(entity.id(),marker.clone()))).collect();
    assert_eq!(found.len(),1);found.into_iter().next().unwrap()
}
fn retired(world:&World)->(Entity,Entity,ResourceToken) {
    let roots:Vec<_>=world.iter_entities().filter(|entity|entity.contains::<crate::scene_retired::RetiredProjection>() && entity.get::<Parent>().is_none()).collect();
    assert_eq!(roots.len(),1);let root=roots[0];let token=root.get::<crate::scene_retired::RetiredProjection>().unwrap().token.clone();
    let leaf=world.iter_entities().find(|entity|entity.get::<Parent>().is_some_and(|parent|parent.get()==root.id())).unwrap().id();
    (root.id(),leaf,token)
}
fn tick(app:&mut App,seconds:f64) {
    crate::scene_effects::capture_bases(app.world_mut()).unwrap();
    app.world_mut().resource_mut::<VnEngine>().0.tick_game_frame(seconds,GameFrameReports::default()).unwrap();
    app.update();
}
fn next(app:&mut App) {
    let mut wrapper=app.world_mut().resource_mut::<VnEngine>();
    wrapper.0.advance_dialogue().unwrap();wrapper.0.step_until_interaction().unwrap();
    drop(wrapper);app.update();
}
fn duration()->f64 {f64::from(rvn_parser::Transition::DEFAULT_FADE_MS)/1000.0}
fn state(world:&World)->serde_json::Value {
    let game=&world.resource::<VnEngine>().0;
    serde_json::json!({"state":game.state,"history":game.history.entries().iter().map(|entry|(&entry.state,&entry.display)).collect::<Vec<_>>(),"runtime":game.runtime_epoch(),"interface":game.interface_epoch(),"instance":game.instance_id()})
}

fn batch_bits(batch:&rvn_core::renderer::SceneRenderBatch)->serde_json::Value {
    serde_json::json!({"state":batch.state,"previous":batch.previous,"display":format!("{:?}",&batch.display),"effects":batch.effects,"runtime_epoch":batch.runtime_epoch,"interface_epoch":batch.interface_epoch,"video_epoch":batch.video_epoch,"projected_views":format!("{:?}",(&batch.interfaces,&batch.motions,&batch.layered,&batch.videos,&batch.menu_requests,&batch.speech_requests))})
}

#[test]
fn cg_actual_update_samples_current_core_phase_after_cover_without_legacy_clock_or_accumulation() {
    let mut app=app("scene label P\ncinematic \"old\" with fade\n\"before\"\n");app.update();
    let (entity,marker)=current(app.world());let game=&app.world().resource::<VnEngine>().0;
    assert_eq!(marker.instance_id,game.instance_id());assert_eq!(marker.epoch,game.runtime_epoch());
    assert_eq!(marker.target,*game.state.scene_execution.as_ref().unwrap().resources.target(&TaskOwner::Game,TargetKind::Cinematic,"cinematic").unwrap());
    assert!(app.world().get::<crate::scene_effects::FadeLease>(entity).is_some());
    tick(&mut app,duration()/4.0);
    assert_eq!(app.world().get::<Sprite>(entity).unwrap().color.alpha(),0.25);
    assert_eq!(app.world().get::<Transform>(entity).unwrap().scale,Vec3::new(8.0,8.0,1.0));
    let clock=app.world().resource::<VnEngine>().0.state.scene_execution.as_ref().unwrap().resources.tasks.iter().find(|task|task.target==marker.target).unwrap().effect.as_ref().unwrap().clock.clone();
    app.world_mut().run_system_once(crate::systems::fade_system);
    assert_eq!(app.world().get::<Sprite>(entity).unwrap().color.alpha(),0.25);
    assert_eq!(app.world().resource::<VnEngine>().0.state.scene_execution.as_ref().unwrap().resources.tasks.iter().find(|task|task.target==marker.target).unwrap().effect.as_ref().unwrap().clock,clock);
    tick(&mut app,0.0);assert_eq!(app.world().get::<Sprite>(entity).unwrap().color.alpha(),0.25);
}

#[test]
fn cg_same_file_current_and_retired_have_distinct_roles_and_frozen_source_alpha_is_multiplied_once() {
    let mut app=app("scene label P\ncinematic \"old\" with fade\n\"before\"\ncinematic \"old\" with fade\n\"after\"\n");app.update();tick(&mut app,duration()/4.0);
    let (old,old_marker)=current(app.world());next(&mut app);let (new,new_marker)=current(app.world());let (root,leaf,outgoing)=retired(app.world());
    assert_ne!(old_marker.target,new_marker.target);assert_ne!(old,new);assert!(app.world().get_entity(old).is_none());
    assert_ne!(outgoing.target,new_marker.target);assert!(app.world().get::<VnCinematic>(root).is_none() && app.world().get::<CurrentCinematic>(leaf).is_none());
    assert_eq!(app.world().get::<Handle<Image>>(leaf),app.world().get::<Handle<Image>>(new));
    tick(&mut app,duration()/4.0);
    assert_eq!(app.world().get::<Sprite>(leaf).unwrap().color.alpha(),0.25*0.75);
    assert_eq!(app.world().get::<Sprite>(new).unwrap().color.alpha(),0.25);
    let token=app.world().get::<crate::scene_retired::RetiredProjection>(leaf).unwrap().token.clone();
    assert_eq!(token,outgoing);app.world_mut().run_system_once(crate::systems::fade_system);
    assert!(app.world().get_entity(root).is_some());assert_eq!(app.world().get::<Sprite>(leaf).unwrap().color.alpha(),0.25*0.75);
}

#[test]
fn cg_current_and_historical_cover_resize_use_each_catalogue_asset_and_preserve_clocks() {
    let mut app=app("scene label P\ncinematic \"old\"\n\"before\"\ncinematic \"new\" with fade\n\"after\"\n");app.update();next(&mut app);tick(&mut app,duration()/4.0);
    let (current,_)=current(app.world());let (root,leaf,_)=retired(app.world());let before=state(app.world());
    let window=app.world().iter_entities().find(|entity|entity.contains::<Window>()).unwrap().id();
    app.world_mut().get_mut::<Window>(window).unwrap().resolution.set(1200.,600.);app.update();
    assert_eq!(app.world().get::<Transform>(current).unwrap().scale,Vec3::new(6.,6.,1.));
    assert_eq!(app.world().get::<Transform>(root).unwrap().scale,Vec3::new(12.,12.,1.));
    assert_eq!(*app.world().get::<Transform>(leaf).unwrap(),Transform::default());
    assert_eq!(app.world().resource::<AssetServer>().get_path(app.world().get::<Handle<Image>>(leaf).unwrap().id()).unwrap().path(),std::path::Path::new("images/cg-old.png"));
    assert_eq!(state(app.world()),before);assert_eq!(app.world().get::<Sprite>(leaf).unwrap().color.alpha(),0.75);
}

#[test]
fn cg_call_return_keeps_current_image_but_renews_exact_scene_fade_and_load_uses_fresh_epoch() {
    let mut app=app("scene label P\ncinematic \"old\" with fade\n\"before\"\ncall C\n\"returned\"\njump done\nscene label C\n\"child\"\nreturn\nlabel done\n");app.update();tick(&mut app,duration()/4.0);
    let (entity,marker)=current(app.world());let old=app.world().get::<crate::scene_effects::FadeLease>(entity).unwrap().0.clone();
    next(&mut app);assert_eq!(current(app.world()).0,entity);assert!(app.world().get::<crate::scene_effects::FadeLease>(entity).is_none());
    tick(&mut app,duration());next(&mut app);assert_eq!(current(app.world()).0,entity);
    let returned=app.world().get::<crate::scene_effects::FadeLease>(entity).unwrap().0.clone();
    assert_eq!(returned.target,marker.target);assert!(returned.task.revision>old.task.revision);assert_eq!(app.world().get::<Sprite>(entity).unwrap().color.alpha(),0.25);
    let saved=rvn_core::save::SaveData::try_from_state(&app.world().resource::<VnEngine>().0.state,1,"CG".into(),"main.rvn".into()).unwrap();assert_eq!(saved.format_version,11);
    app.world_mut().resource_mut::<VnEngine>().0.load_data(saved).unwrap();app.update();
    let (fresh,fresh_marker)=current(app.world());assert_ne!(fresh,entity);assert!(fresh_marker.epoch>marker.epoch);
    assert_eq!(app.world().get::<Sprite>(fresh).unwrap().color.alpha(),0.25);
    let before=state(app.world());assert!(!app.world_mut().resource_mut::<VnEngine>().0.complete_scene_effect(&old).unwrap());assert_eq!(state(app.world()),before);
}

#[test]
fn cg_terminal_retired_cleanup_never_despawns_the_new_current_same_file() {
    let mut app=app("scene label P\ncinematic \"old\"\n\"before\"\ncinematic \"old\" with fade\n\"after\"\n");app.update();next(&mut app);
    let (entity,marker)=current(app.world());let (root,leaf,old)=retired(app.world());tick(&mut app,duration());
    assert!(app.world().get_entity(root).is_none() && app.world().get_entity(leaf).is_none());
    assert_eq!(current(app.world()),(entity,marker));assert_eq!(app.world().get::<Sprite>(entity).unwrap().color.alpha(),1.0);
    assert!(!app.world_mut().resource_mut::<VnEngine>().0.complete_scene_effect(&old).unwrap());
}

#[test]
fn cg_invalid_catalogue_preflight_preserves_pending_output_installed_models_and_ecs() {
    let mut app=app("scene label P\ncinematic \"old\"\n\"before\"\ncinematic \"new\" with fade\n\"after\"\n");app.update();let (old,marker)=current(app.world());
    {let mut game=app.world_mut().resource_mut::<VnEngine>();game.0.advance_dialogue().unwrap();game.0.step_until_interaction().unwrap();}
    app.world_mut().resource_mut::<CgAssetRegistry>().0.insert("new".into(),"../outside.png".into());
    let before=state(app.world());let batch=batch_bits(app.world().resource::<VnEngine>().0.renderer.scene_batch.as_ref().unwrap());
    let pending=format!("{:?}",app.world().resource::<VnEngine>().0.renderer.pending);let entities=app.world().entities().len();
    let output=app.world().resource::<Events<VnCommand>>().len();let installed=batch_bits(app.world().resource::<crate::scene_publication::ScenePresentation>().batch.as_ref().unwrap());
    crate::scene_publication::consume(app.world_mut());
    assert_eq!(state(app.world()),before);assert_eq!(app.world().entities().len(),entities);assert_eq!(current(app.world()),(old,marker));
    assert_eq!(app.world().resource::<Events<VnCommand>>().len(),output);assert_eq!(format!("{:?}",app.world().resource::<VnEngine>().0.renderer.pending),pending);
    assert_eq!(batch_bits(app.world().resource::<VnEngine>().0.renderer.scene_batch.as_ref().unwrap()),batch);
    assert_eq!(batch_bits(app.world().resource::<crate::scene_publication::ScenePresentation>().batch.as_ref().unwrap()),installed);
    assert!(app.world().resource::<VnEngine>().0.renderer.scene_frame_failed);
}

#[test]
fn cg_independent_engine_epoch_zero_cannot_reuse_current_or_retired_incarnation_cache() {
    let source="scene label P\ncinematic \"old\"\ncinematic \"new\" with fade\n\"before\"\n";
    let mut app=app(source);app.update();let (old,old_marker)=current(app.world());let (old_root,_,old_token)=retired(app.world());
    let fresh=game(source);assert_eq!(fresh.runtime_epoch(),old_marker.epoch);assert_ne!(fresh.instance_id(),old_marker.instance_id);
    app.world_mut().insert_resource(VnEngine(fresh));app.update();
    let (new,new_marker)=current(app.world());let (new_root,_,new_token)=retired(app.world());
    assert_eq!(new_marker.target,old_marker.target);assert_eq!(new_token.target,old_token.target);assert_eq!(new_token.task.runtime_epoch,old_token.task.runtime_epoch);
    assert_ne!(new_marker.instance_id,old_marker.instance_id);assert_ne!(new,old);assert_ne!(new_root,old_root);
    assert!(app.world().get_entity(old).is_none() && app.world().get_entity(old_root).is_none());
}

#[test]
fn cg_coalesced_gallery_receipts_are_bounded_idempotent_and_current_is_only_latest_image() {
    let mut app=app("scene label P\n\"before\"\ncinematic \"old\"\ncinematic \"new\"\ncinematic \"new\"\n\"after\"\n");app.update();
    {let mut game=app.world_mut().resource_mut::<VnEngine>();game.0.advance_dialogue().unwrap();game.0.step_until_interaction().unwrap();
        let ids:Vec<_>=game.0.renderer.pending.iter().filter_map(|command|if let VnCommand::UnlockCinematic{id}=command{Some(id.as_str())}else{None}).collect();assert_eq!(ids,vec!["old","new"]);}
    app.update();let (entity,_)=current(app.world());
    assert_eq!(app.world().resource::<AssetServer>().get_path(app.world().get::<Handle<Image>>(entity).unwrap().id()).unwrap().path(),std::path::Path::new("images/cg-new.png"));
    let mut game=game("scene label P\n\"before\"\ncinematic \"new\"\n\"after\"\n");game.renderer.scene_batch.take();game.renderer.pending.clear();game.advance_dialogue().unwrap();
    for index in 0..rvn_core::scene_resources::MAX_RESOURCES {game.renderer.pending.push(VnCommand::UnlockCinematic{id:format!("seen-{index}")});}
    let before=serde_json::to_value(&game.state).unwrap();let pending=format!("{:?}",game.renderer.pending);let history=game.history.entries().len();
    assert!(game.step_silent().unwrap_err().to_string().contains("receipt capacity"));
    assert_eq!(serde_json::to_value(&game.state).unwrap(),before);assert_eq!(format!("{:?}",game.renderer.pending),pending);assert_eq!(game.history.entries().len(),history);assert!(game.renderer.scene_batch.is_none());
}

#[test]
fn cg_asset_defer_uses_actual_core_elapsed_and_completion_cannot_resurrect_late_historical_image() {
    let mut app=app("scene label P\ncinematic \"old\"\ncinematic \"new\" with fade\n\"before\"\n");
    let old=app.world().resource::<AssetServer>().get_handle::<Image>("images/cg-old.png").unwrap();
    app.world_mut().resource_mut::<Assets<Image>>().remove(old.id());
    crate::scene_publication::consume(app.world_mut());
    assert!(app.world().iter_entities().all(|entity|!entity.contains::<crate::scene_retired::RetiredProjection>()));
    app.world_mut().resource_mut::<VnEngine>().0.tick_game_frame(duration()/4.0,GameFrameReports::default()).unwrap();
    crate::scene_publication::consume(app.world_mut());
    assert!(app.world().iter_entities().all(|entity|!entity.contains::<crate::scene_retired::RetiredProjection>()));
    app.world_mut().resource_mut::<Assets<Image>>().insert(old.id(),image(100,100));
    let game=&app.world().resource::<VnEngine>().0;
    let plan=crate::scene_retired::prepare(app.world(),&game.state,game.runtime_epoch()).unwrap();
    crate::scene_retired::install(app.world_mut(),plan);let (root,leaf,token)=retired(app.world());
    assert_eq!(app.world().get::<Sprite>(leaf).unwrap().color.alpha(),0.75);
    assert_eq!(token.task.runtime_epoch,app.world().resource::<VnEngine>().0.runtime_epoch());
    app.world_mut().resource_mut::<VnEngine>().0.tick_game_frame(duration(),GameFrameReports::default()).unwrap();
    crate::scene_publication::consume(app.world_mut());assert!(app.world().get_entity(root).is_none());
    app.world_mut().resource_mut::<Assets<Image>>().insert(old.id(),image(100,100));
    let game=&app.world().resource::<VnEngine>().0;
    let plan=crate::scene_retired::prepare(app.world(),&game.state,game.runtime_epoch()).unwrap();
    crate::scene_retired::install(app.world_mut(),plan);
    assert!(app.world().iter_entities().all(|entity|!entity.contains::<crate::scene_retired::RetiredProjection>()));
}

#[test]
fn cg_reduced_projection_does_not_advance_owned_phase_and_legacy_direct_fade_keeps_existing_route() {
    let mut app=app("scene label P\ncinematic \"old\" with fade\n\"before\"\ncinematic \"new\" with fade\n\"after\"\n");app.update();tick(&mut app,duration()/4.0);next(&mut app);
    let (current,_)=current(app.world());let (_,leaf,_)=retired(app.world());let before=state(app.world());
    app.world_mut().resource_mut::<crate::accessibility::Accessibility>().settings.reduced_motion=true;app.update();
    assert_eq!(app.world().get::<Sprite>(current).unwrap().color.alpha(),1.0);
    assert_eq!(app.world().get::<Sprite>(leaf).unwrap().color.alpha(),0.0);assert_eq!(state(app.world()),before);
    // Exercise the direct legacy route in a fresh World; existing CGs legitimately fade out.
    let mut app=self::app("label start\n\"legacy\"\n");
    assert!(app.world().iter_entities().all(|entity|!entity.contains::<VnCinematic>()));
    let legacy=rvn_core::Engine::new(rvn_parser::parse("label start\n\"legacy\"\n").unwrap(),crate::bevy_renderer::BevyRenderer::new(),0).unwrap();
    app.world_mut().insert_resource(VnEngine(legacy));app.world_mut().resource_mut::<crate::accessibility::Accessibility>().settings.reduced_motion=false;
    app.world_mut().resource_mut::<Events<VnCommand>>().clear();
    app.world_mut().send_event(VnCommand::ShowCinematic{id:"old".into(),transition:Some("fade".into())});
    app.world_mut().run_system_once(cinematic_system);
    let legacy:Vec<_>=app.world().iter_entities().filter(|entity|entity.contains::<VnCinematic>()).collect();assert_eq!(legacy.len(),1);
    assert!(legacy[0].get::<CurrentCinematic>().is_none() && legacy[0].get::<crate::scene_effects::FadeLease>().is_none());
    assert_eq!(legacy[0].get::<crate::components::FadeAnim>().unwrap().elapsed_secs,0.0);
    assert_eq!(legacy[0].get::<Sprite>().unwrap().color.alpha(),0.0);
}


#[test]
fn cg_cover_resize_preserves_legacy_rotation_and_depth_while_current_uses_neutral_base() {
    let mut world=World::new();
    world.insert_resource(Assets::<Image>::default());
    world.spawn(Window{resolution:(800.,600.).into(),..default()});
    let handle=world.resource_mut::<Assets<Image>>().add(image(100,200));
    let original=Transform::from_xyz(11.,-17.,73.)
        .with_rotation(Quat::from_rotation_z(0.37)).with_scale(Vec3::new(2.,3.,4.));
    let legacy=world.spawn((VnCinematic,handle.clone(),original)).id();
    let narrative=game("scene label P\ncinematic \"old\"\n\"before\"\n");
    let target=narrative.state.scene_execution.as_ref().unwrap().resources
        .target(&TaskOwner::Game,TargetKind::Cinematic,"cinematic").unwrap().clone();
    let modern=world.spawn((VnCinematic,handle,original,CurrentCinematic {
        target,epoch:narrative.runtime_epoch(),instance_id:narrative.instance_id(),
    })).id();
    world.run_system_once(cinematic_cover_resize_system);
    let legacy_transform=world.get::<Transform>(legacy).unwrap();
    assert_eq!(legacy_transform.scale,Vec3::new(8.,8.,1.));
    assert_eq!(legacy_transform.translation,Vec3::new(0.,0.,73.));
    assert_eq!(legacy_transform.rotation,original.rotation);
    let modern_transform=world.get::<Transform>(modern).unwrap();
    assert_eq!(*modern_transform,Transform::from_xyz(0.,0.,50.).with_scale(Vec3::new(8.,8.,1.)));
}
