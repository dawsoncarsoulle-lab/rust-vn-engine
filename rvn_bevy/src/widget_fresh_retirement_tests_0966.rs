//! Actual existing restart Anchor + EngineStartup + Source/World fence, CPU.
//! Uses the public runtime registrar and genuine asset/text/source adapters.
//! No window automation, device/GPU qualification or raster credit is claimed.
use super::*;
use crate::LaunchOwner;
use bevy::prelude::*;
use crate::resources::{VnEngine,VnState};
fn app_for(project:&Project)->App{
 let launch=crate::load_runtime_game(&project.root).unwrap();let mut app=App::new();let(locale_root,declarations)=launch.closed_locales.expect("real native closed-source loader");
 crate::portable_text_resources::register_verified_text_sources(&mut app,launch.asset_root.clone(),locale_root).unwrap();
 app.add_plugins(MinimalPlugins).add_plugins(AssetPlugin{file_path:launch.asset_root,watch_for_changes_override:Some(false),..default()});launch.engine.install(&mut app);
 app.init_resource::<crate::scene_publication::ScenePresentation>().add_event::<crate::vn_command::VnCommand>()
  .add_plugins(crate::string_table_assets::StringTableAssetsPlugin::new(launch.cfg.string_tables))
  .add_plugins(crate::portable_text_resources::ClosedLocaleAssetsPlugin::new(declarations).unwrap())
  .init_asset::<bevy::audio::AudioSource>().init_asset_loader::<bevy::audio::AudioLoader>().init_asset::<crate::checkpoint_pcm::CheckpointPcm>();
 let(_,mut app)=crate::startup_full_launch_tests::complete_cpu_runtime(project.root.clone(),app,true);
 app.add_plugins(crate::checkpoint_requests::CheckpointRequestsPlugin).add_plugins(crate::scene_effects::SceneEffectsPlugin).add_plugins(crate::scene_retired::SceneRetiredPlugin).add_plugins(crate::scene_audio::SceneAudioPlugin).add_plugins(crate::web_music::WebMusicPlugin).add_plugins(crate::presentation_order::PresentationOrderPlugin).add_plugins(crate::accessibility::AccessibilityPlugin).add_plugins(crate::portable_images::PortableImagesPlugin);
 app.insert_resource(crate::settings_from_persistent(&launch.persistent_data,&launch.locale_cfg.current));app.insert_resource(crate::resources::PersistentDataResource{manager:launch.persistent_manager,data:launch.persistent_data});app.insert_resource(launch.theme);app.world_mut().resource_mut::<NextState<VnState>>().set(VnState::Stepping);app.finish();app.cleanup();app
}
fn pump_game(app:&mut App){let end=std::time::Instant::now()+std::time::Duration::from_secs(20);loop{app.update();if let Some(startup)=app.world().get_resource::<crate::debugger_startup::DebugBootstrap>(){assert!(startup.failed.is_none(),"Actual startup failed: {:?}",startup.failed);}else if app.world().get_resource::<VnEngine>().is_some_and(|game|!game.0.renderer.scene_batch_pending()){return;}assert!(std::time::Instant::now()<end,"actual native source startup did not settle");std::thread::sleep(std::time::Duration::from_millis(2));}}
fn focus(app:&mut App,value:bool){let entity=app.world_mut().query_filtered::<Entity,With<bevy::window::PrimaryWindow>>().single(app.world());app.world_mut().get_mut::<Window>(entity).unwrap().focused=value;}
fn park_actual(app:&mut App){
 app.world_mut().resource_scope(|world,mut requests:Mut<crate::runtime_restart::Requests>|requests.admit(world.resource::<VnEngine>(),Some("start".into()))).unwrap();focus(app,false);app.update();assert!(!app.world().contains_resource::<VnEngine>());assert!(!crate::runtime_restart::presentation_available(app.world()));assert!(app.world().contains_resource::<crate::debugger_startup::DebugBootstrap>());
}
fn ready_actual_publication(app:&mut App)->Result<Box<crate::debugger_startup::PreparedBootstrapStep>,String>{
 for _ in 0..128{
  focus(app,true);let mut plan=crate::debugger_startup::prepare_world_step_boxed(app.world(),StepKind::Continue)?;let end=std::time::Instant::now()+std::time::Duration::from_secs(20);
  while !crate::debugger_startup::refresh_world_step(app.world(),&mut plan)?{assert!(std::time::Instant::now()<end,"actual staged assets did not settle");focus(app,false);app.update();focus(app,true);std::thread::sleep(std::time::Duration::from_millis(2));}
  if plan.core().render_batch().is_some(){return Ok(plan);}
  let committed=crate::debugger_startup::commit_world_step_boxed(app.world_mut(),plan)?;assert!(committed.publication.is_none());crate::debugger_startup::install_committed_world_step_boxed(app.world_mut(),committed);assert!(!crate::runtime_restart::presentation_available(app.world()));
 }Err("actual pending owner never reached its first publication".into())
}
fn attach(app:&mut App)->(rvn_core::widget_backend_0956::WidgetBackendOwner,crate::widget_backend_0956::WidgetBackend,std::sync::Arc<crate::widget_backend_0956::WidgetWrapper>){
 let handle=app.world_mut().resource_mut::<VnEngine>().0.create_widget(UUID,"one",vec![Value::Int(1)]).unwrap();app.world_mut().resource_mut::<VnEngine>().0.add_widget_to_viewport(&handle,false,0).unwrap();pump_game(app);let game=app.world().resource::<VnEngine>();let owner=game.0.widget_backend_owner(&handle).unwrap();let backend=game.0.renderer.widget_backend.clone();let wrapper=backend.wrapper(&owner).unwrap().upgrade().unwrap();(owner,backend,wrapper)
}
#[test]
fn real_restart_first_source_commit_retires_old_widget_with_old_globals_and_rng_before_anchor_is_removed(){
 let project=native_project(&guarded_source());let mut app=app_for(&project);pump_game(&mut app);let(owner,backend,wrapper)=attach(&mut app);set_old_history_guards(&mut app.world_mut().resource_mut::<VnEngine>().0);let old=app.world().resource::<VnEngine>().0.instance_id();park_actual(&mut app);assert!(wrapper.attached());let plan=ready_actual_publication(&mut app).expect("Destruct must use retained old777/source/RNG, never incoming Init globals");assert!(wrapper.attached());let committed=crate::debugger_startup::commit_world_step_boxed(app.world_mut(),plan).unwrap();assert!(committed.publication.is_some());assert!(!wrapper.attached());assert!(backend.wrapper(&owner).is_none());assert!(!crate::runtime_restart::presentation_available(app.world()));crate::debugger_startup::install_committed_world_step_boxed(app.world_mut(),committed);assert!(crate::runtime_restart::presentation_available(app.world()));pump_game(&mut app);let game=app.world().resource::<VnEngine>();assert_ne!(game.0.instance_id(),old);assert_eq!(game.0.state.vars["shared"],Value::Int(0));assert_eq!(game.0.state.vars["destructed_count"],Value::Int(0));assert_eq!(game.0.state.vars["initialized_count"],Value::Int(0));assert!(game.0.widget_instance_handle("one").is_none());
}
#[test]
fn actual_restart_old_destruct_refusal_restores_the_retained_engine_and_actual_parent_without_committing_incoming_publication(){
 let source=SOURCE.replace("handler destruct(event){set destructed_count=destructed_count+1}","handler destruct(event){set destructed_count=destructed_count+1 local invalid_interface=\"unknown\" ui.open(invalid_interface,[],false,0)}");let project=native_project(&source);let mut app=app_for(&project);pump_game(&mut app);let(_,_,wrapper)=attach(&mut app);let game=app.world().resource::<VnEngine>();let old=game.0.instance_id();let old_source=game.0.audio_request_source().unwrap();let state=serde_json::to_value(&game.0.state).unwrap();park_actual(&mut app);let error=ready_actual_publication(&mut app).err().expect("genuine Destruct script refusal must reject first publication");assert!(error.contains("Old Widget Destruct preparation"));assert!(wrapper.attached());assert!(crate::runtime_restart::restore_before_publication(app.world_mut(),&error));let game=app.world().resource::<VnEngine>();assert_eq!(game.0.instance_id(),old);assert!(game.0.accepts_audio_request_source(&old_source));assert_eq!(serde_json::to_value(&game.0.state).unwrap(),state);assert!(wrapper.attached());assert!(!game.0.renderer.scene_batch_pending());
}
#[test]
fn real_restart_failure_after_old_parent_reservation_drops_ticket_and_keeps_old_lifetime_recoverable(){
 let project=native_project(SOURCE);let mut app=app_for(&project);pump_game(&mut app);let(owner,backend,wrapper)=attach(&mut app);let old_source=app.world().resource::<VnEngine>().0.audio_request_source().unwrap();park_actual(&mut app);let plan=ready_actual_publication(&mut app).unwrap();assert!(wrapper.attached());app.world_mut().resource_mut::<crate::debugger_startup::DebugBootstrap>().owner.configure(vec![],vec![rvn_core::editor_debugger::Watch{name:"shared".into(),scope:rvn_core::editor_debugger::WatchScope::Globals}]).unwrap();assert!(crate::debugger_startup::commit_world_step_boxed(app.world_mut(),plan).is_err());assert!(wrapper.attached());assert!(backend.prepare(&owner,WidgetViewportOperation::Detach).is_ok(),"failed incoming Core fence must drop the actual old-parent ticket");assert!(crate::runtime_restart::restore_before_publication(app.world_mut(),"actual incoming configuration changed"));assert!(app.world().resource::<VnEngine>().0.accepts_audio_request_source(&old_source));assert!(wrapper.attached());
}
