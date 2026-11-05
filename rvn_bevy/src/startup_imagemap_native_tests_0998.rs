//! CPU World event/paint-grid contracts. OS and GPU parity remain independent.
use super::*;
use bevy::ecs::system::RunSystemOnce;
const SCRIPT:&str=r#"init{flow.consume_v2("paint","map"){capture "draw"=random(1,9) imagemap{background:"map.png" hover:"hover.png" hotspot{name:"all" area:(0,0,420,260)}=>{set chosen=1}}} set done=1}
scene label start
"Boundary"
"#;
fn fixture()->(App,Entity,Entity){
 let(_,mut app)=super::super::imagemap_world_tests::fixture(SCRIPT);
 app.init_resource::<bevy::render::camera::ManualTextureViews>().init_resource::<crate::accessibility::Accessibility>().init_resource::<crate::editor_preview::PreviewState>()
  .init_resource::<crate::programmable_ui::Screens>().init_resource::<bevy::ui::UiStack>().insert_resource(bevy::ui::UiScale(1.0))
  .init_resource::<PaintSequence>().init_resource::<NativePressReader>().add_event::<bevy::input::mouse::MouseButtonInput>()
  .add_event::<bevy::window::WindowResized>().add_event::<bevy::window::WindowCreated>().add_event::<bevy::window::WindowScaleFactorChanged>();
 let window=app.world_mut().spawn((Window{focused:true,resolution:bevy::window::WindowResolution::new(800.0,600.0).with_scale_factor_override(1.0),..default()},bevy::window::PrimaryWindow)).id();
 let camera=app.world_mut().spawn(Camera2dBundle::default()).id();app.world_mut().run_system_once(bevy::render::camera::camera_system::<OrthographicProjection>);
 let mut prepared=super::super::prepare_world_step(app.world(),rvn_core::editor_debugger::StepKind::Continue).unwrap();super::super::imagemap_world_tests::ready(&mut app,&mut prepared);let committed=super::super::commit_world_step(app.world_mut(),prepared).unwrap();super::super::install_committed_world_step(app.world_mut(),committed);app.update();
 crate::programmable_ui::owner_hosts_0943::startup_capture_tests::install_actual_empty_owner_geometry(app.world_mut());
 super::super::driver::run(app.world_mut(),rvn_core::editor_debugger::StepKind::Continue).unwrap();app.world_mut().get_mut::<Window>(window).unwrap().set_cursor_position(Some(Vec2::new(400.0,300.0)));capture(app.world_mut());assert!(app.world().contains_resource::<PaintedInput>());assert!(!app.world().contains_resource::<crate::resources::VnEngine>());
 (app,window,camera)
}
fn press(app:&mut App,window:Entity){app.world_mut().resource_mut::<Events<bevy::input::mouse::MouseButtonInput>>().send(bevy::input::mouse::MouseButtonInput{window,button:MouseButton::Left,state:bevy::input::ButtonState::Pressed});select(app.world_mut());}
fn state(app:&App)->serde_json::Value{serde_json::to_value(app.world().resource::<DebugBootstrap>().owner.state()).unwrap()}
#[test]
fn startup_imagemap0998_empty_actual_owner_projection_captures_physical_grid_without_game_owner_and_primary_event_commits_once(){
 let(mut app,window,_)=fixture();let paint=app.world().resource::<PaintedInput>();assert!(paint.physical.startup_imagemap_hit(Vec2::new(400.0,300.0)).is_some());let observed=paint.source.clone();let before=state(&app);press(&mut app,window);assert!(super::super::driver::pending(app.world()));assert_eq!(state(&app),before);
 super::super::driver::drain(app.world_mut());assert!(!app.world().resource::<DebugBootstrap>().owner.waiting_for_imagemap());assert!(!app.world().resource::<DebugBootstrap>().owner.accepts_observation_source(&observed));assert!(!app.world().resource::<crate::resources::ImagemapState>().active);assert!(!app.world().resource::<DebugBootstrap>().owner.state().vars.contains_key("chosen"));assert_eq!(state(&app)["random"],before["random"]);assert!(!app.world().contains_resource::<crate::resources::VnEngine>());
}
#[test]
fn startup_imagemap0998_secondary_window_event_and_unfocused_primary_cannot_select_or_replay_when_focus_returns(){
 let(mut app,window,_)=fixture();let secondary=app.world_mut().spawn(Window{focused:false,..default()}).id();let before=state(&app);press(&mut app,secondary);assert!(!super::super::driver::pending(app.world()));assert_eq!(state(&app),before);
 app.world_mut().get_mut::<Window>(window).unwrap().focused=false;press(&mut app,window);app.world_mut().get_mut::<Window>(window).unwrap().focused=true;select(app.world_mut());assert!(!super::super::driver::pending(app.world()));assert_eq!(state(&app),before);assert!(app.world().resource::<DebugBootstrap>().failed.is_none());press(&mut app,window);assert!(super::super::driver::pending(app.world()));
}
#[test]
fn startup_imagemap0998_resize_dpi_camera_and_primary_window_replacement_invalidate_native_painted_coordinates(){
 for tamper in 0..4{
  let(mut app,window,camera)=fixture();let before=state(&app);
  match tamper{
   0=>app.world_mut().get_mut::<Window>(window).unwrap().resolution.set(900.0,600.0),
   1=>app.world_mut().get_mut::<Window>(window).unwrap().resolution.set_scale_factor_override(Some(1.5)),
   2=>app.world_mut().get_mut::<GlobalTransform>(camera).unwrap().clone_from(&GlobalTransform::from_translation(Vec3::X)),
   3=>{app.world_mut().entity_mut(window).remove::<bevy::window::PrimaryWindow>();app.world_mut().spawn((Window{focused:true,..default()},bevy::window::PrimaryWindow));},
   _=>unreachable!(),
  }
  press(&mut app,window);assert!(!super::super::driver::pending(app.world()),"tamper{tamper}");assert_eq!(state(&app),before);assert!(app.world().resource::<DebugBootstrap>().failed.is_none());
 }
}
