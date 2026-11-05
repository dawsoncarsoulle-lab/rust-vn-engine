//! Native input uses the retained painted layout, like Slate's hit-test grid.
//! This is a native paint observation, not an editor GPU/Output image receipt.
use bevy::prelude::*;
use std::sync::Arc;
use super::DebugBootstrap;
#[derive(Resource,Default)]
struct PaintSequence(u64);
#[derive(Resource)]
struct PaintedInput{window:Entity,viewport:PaintViewport,physical:Arc<crate::editor_preview_input::PhysicalFrame>,source:Arc<rvn_core::engine::StartupObservationSource>}
#[derive(Resource,Default)]
struct NativePressReader(bevy::ecs::event::ManualEventReader<bevy::input::mouse::MouseButtonInput>);
#[derive(PartialEq)]
struct PaintViewport{window:Entity,size:UVec2,scale:f32,camera:Entity,physical:URect,logical:Rect,clip:Mat4,transform:Mat4}
fn viewport(world:&World,window:Entity)->Option<PaintViewport>{
 let actual=world.iter_entities().find(|entity|entity.contains::<bevy::window::PrimaryWindow>())?;if actual.id()!=window{return None;}
 let window_state=actual.get::<Window>()?;
 let mut cameras=world.iter_entities().filter(|entity|entity.contains::<Camera2d>());let camera_entity=cameras.next()?;if cameras.next().is_some(){return None;}
 let camera=camera_entity.get::<Camera>()?;if !camera.is_active{return None;}
 match &camera.target{bevy::render::camera::RenderTarget::Window(bevy::window::WindowRef::Primary)=>{},bevy::render::camera::RenderTarget::Window(bevy::window::WindowRef::Entity(entity))if *entity==window=>{},_=>return None};
 Some(PaintViewport{window,size:UVec2::new(window_state.physical_width(),window_state.physical_height()),scale:window_state.scale_factor(),camera:camera_entity.id(),physical:camera.physical_viewport_rect()?,logical:camera.logical_viewport_rect()?,clip:camera.clip_from_view(),transform:camera_entity.get::<GlobalTransform>()?.compute_matrix()})
}
pub(crate) struct NativeInputPlugin;
impl Plugin for NativeInputPlugin{
 fn build(&self,app:&mut App){
  app.init_resource::<PaintSequence>().init_resource::<NativePressReader>().add_systems(Update,select)
   .add_systems(PostUpdate,capture.after(bevy::ui::UiSystem::Layout).after(bevy::transform::TransformSystem::TransformPropagate));
 }
}
fn interactive(world:&World)->bool{
 if world.contains_resource::<crate::editor_preview::Output>()||super::driver::pending(world)||super::driver::suspended(world)||crate::checkpoint_requests::world_blocked(world){return false;}
 let Some(startup)=world.get_resource::<DebugBootstrap>()else{return false;};
 if startup.failed.is_some()||startup.publication_pending||startup.mode!=Some(rvn_core::editor_debugger::StepKind::Continue)||!startup.owner.waiting_for_imagemap(){return false;}
 if crate::editor_preview::paused(world)||world.get_resource::<crate::accessibility::Accessibility>().is_some_and(|access|access.blocked)||world.get_resource::<crate::source_menus::SourceMenus>().is_some_and(|menus|menus.modal()){return false;}
 world.iter_entities().find(|entity|entity.contains::<bevy::window::PrimaryWindow>()).and_then(|entity|entity.get::<Window>()).is_some_and(|window|window.focused)
}
fn capture(world:&mut World){
 if !interactive(world){world.remove_resource::<PaintedInput>();return;}
 let result=(||{
  super::validate_installed_imagemap_images(world)?;
  let startup=world.resource::<DebugBootstrap>();let snapshot=startup.owner.snapshot();
  let Some(sequence)=world.resource::<PaintSequence>().0.checked_add(1)else{return Err("Native Init paint sequence exhausted".to_string());};
  let window=world.iter_entities().find(|entity|entity.contains::<bevy::window::PrimaryWindow>()).map(|entity|entity.id()).ok_or("Actual native Init Window vanished")?;
  let Some(viewport)=viewport(world,window)else{return Ok(None);};
  let scope=rvn_core::editor_preview::FrameScope{instance:snapshot.instance,epoch:snapshot.epoch,sequence,control_sequence:0,control_ack:0,control_error:None,paused:false,publication_pending:false,finished:Some(false),step_allowed:false,interaction:None,position:snapshot.position,active:None,parked:vec![]};
  let source=Arc::new(startup.owner.observation_source().map_err(|error|error.to_string())?);
  let Some(physical)=crate::editor_preview_input::capture(world,scope)else{return Ok(None);};
  Ok(Some((sequence,PaintedInput{window,viewport,physical,source})))
 })();
 match result{
  Ok(Some((sequence,paint)))=>{world.resource_mut::<PaintSequence>().0=sequence;world.insert_resource(paint);},
  Ok(None)=>{world.remove_resource::<PaintedInput>();},
  Err(error)=>{world.remove_resource::<PaintedInput>();super::driver::report_failure(world,error);},
 }
}
fn select(world:&mut World){
 let painted_window=world.get_resource::<PaintedInput>().map(|paint|paint.window);
 // ButtonInput is global across windows. Only the real OS event's window can
 // authorize this painted window, and ignored events are drained immediately.
 let pressed=world.resource_scope(|world,mut reader:Mut<NativePressReader>|{
  let Some(events)=world.get_resource::<Events<bevy::input::mouse::MouseButtonInput>>()else{return false;};
  let mut accepted=false;for event in reader.0.read(events){if event.button==MouseButton::Left&&event.state==bevy::input::ButtonState::Pressed&&Some(event.window)==painted_window{accepted=true;}}accepted
 });
 if !interactive(world)||!pressed{return;}
 let result=(||{
  let Some(paint)=world.get_resource::<PaintedInput>()else{return Ok(());};
  if viewport(world,paint.window).as_ref()!=Some(&paint.viewport){return Ok(());}
  let window=world.get::<Window>(paint.window).ok_or("The painted native Init Window was retired")?;
  if !window.focused{return Ok(());}
  let Some(point)=window.cursor_position()else{return Ok(());};
  super::validate_installed_imagemap_images(world)?;
  let startup=world.resource::<DebugBootstrap>();
  let source=startup.owner.imagemap_input_source(paint.source.clone()).map_err(|error|error.to_string())?;
  let Some(selected)=paint.physical.startup_imagemap_hit(point)else{return Ok(());};
  let prepared=super::prepare_world_imagemap_selection(world,&source,selected)?;
  super::driver::retain_imagemap_selection(world,prepared)
 })();
 if let Err(error)=result{super::driver::report_failure(world,error);}
}

#[cfg(all(test,not(target_arch="wasm32")))]
#[path="startup_imagemap_native_tests_0998.rs"]
mod tests;
