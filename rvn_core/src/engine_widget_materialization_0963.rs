//! A new wrapper life is a real owned VM transaction after saved work has
//! finished. Its preparation can exist without attachment/input/paint readiness.
//! The ordinary debugger must keep this exact operation/Arc while paused; the
//! portable adapter restores only logical work and mints another actual object.
use super::*;
use std::sync::Arc;
#[path="widget_materialization_portable_0964.rs"]
mod portable;
pub(crate) use portable::PortableWidgetMaterialization;
#[derive(Clone,Debug)]
pub struct WidgetMaterializationOperation{
 owner:crate::widget_backend_0956::WidgetBackendOwner,
 object:Option<crate::widget_backend_0956::WidgetBackendObjectIdentity>,
 prepared:Option<Arc<dyn crate::widget_backend_0956::PreparedWidgetViewport>>,
}
impl WidgetMaterializationOperation{
 pub fn owner(&self)->&crate::widget_backend_0956::WidgetBackendOwner{&self.owner}
 pub fn object(&self)->Option<&crate::widget_backend_0956::WidgetBackendObjectIdentity>{self.object.as_ref()}
}
impl<R:Renderer> Engine<R>{
 pub fn has_pending_widget_materialization(&self)->bool{self.state.ui.screens.iter().any(|instance|instance.widget_class.as_ref().is_some_and(|binding|binding.pending_materialization))}
 pub fn widget_materialization_waits_for_saved_operation(&self)->bool{self.restored_widget_barrier}
 pub(super) fn defer_saved_widget_materialization(state:&mut GameState){
  fn defer(ui:&mut crate::ui::UiState){for instance in &mut ui.screens{if let Some(binding)=&mut instance.widget_class{binding.pending_materialization=binding.viewport_attached;}}}
  defer(&mut state.ui);if let Some(execution)=&mut state.scene_execution{for scene in execution.active.iter_mut().chain(execution.frames.iter_mut().filter_map(|frame|frame.parked.as_mut())){defer(&mut scene.tasks.suspended_ui);}}
 }

 /// Caller must have completed the existing stopped operation first. Preparing
 /// this object grants neither input nor a frame; no lifecycle VM is run here.
 pub fn next_widget_materialization(&self)->Result<Option<WidgetMaterializationOperation>,RuntimeError>{
  if self.restored_widget_barrier{return Ok(None);}
  self.widget_classes.validate_state(&self.state).map_err(|message|self.scene_resource_error(message))?;
  let Some(instance)=self.state.ui.screens.iter().filter(|instance|instance.widget_class.as_ref().is_some_and(|binding|binding.pending_materialization)).min_by_key(|instance|instance.order)else{return Ok(None);};
  let handle=self.widget_instance_handle(&instance.name).ok_or_else(||self.scene_resource_error("Pending Widget materialization has no exact admitted logical owner".into()))?;
  let owner=self.widget_backend_owner(&handle)?;
  let prepared=self.renderer.prepare_widget_viewport(&owner,crate::widget_backend_0956::WidgetViewportOperation::Attach).map_err(|message|self.scene_resource_error(message))?;
  let object=prepared.constructed_object().filter(|object|object.owner==owner&&object.generation!=0).ok_or_else(||self.scene_resource_error("Materialization did not prepare a real source-owned backend object".into()))?;
  if prepared.owner()!=&owner||prepared.operation()!=crate::widget_backend_0956::WidgetViewportOperation::Attach||!prepared.constructs_wrapper()||prepared.releases_wrapper(){return Err(self.scene_resource_error("Materialization backend plan differs from its actual new wrapper life".into()));}
  self.renderer.validate_widget_viewport(prepared.as_ref()).map_err(|message|self.scene_resource_error(message))?;
  Ok(Some(WidgetMaterializationOperation{owner,object:Some(object),prepared:Some(prepared)}))
 }
 pub fn accepts_widget_materialization(&self,operation:&WidgetMaterializationOperation)->bool{
  let Some(handle)=self.widget_instance_handle(&operation.owner.target().alias)else{return false;};
  self.widget_backend_owner(&handle).is_ok_and(|owner|owner==operation.owner)
   &&self.state.ui.screens.iter().any(|instance|instance.name==operation.owner.target().alias&&instance.order==operation.owner.order()&&instance.widget_class.as_ref().is_some_and(|binding|binding.pending_materialization))
   &&match(&operation.prepared,&operation.object){(Some(prepared),Some(object))=>prepared.constructed_object().as_ref()==Some(object)&&self.renderer.validate_widget_viewport(prepared.as_ref()).is_ok(),(None,None)=>true,_=>false}
 }
 /// Bind a restored logical operation only after the restore's backend fence
 /// has installed its new epoch. The saved descriptor never supplies an object
 /// or input capability. A paused session then retains this same Arc for every
 /// subsequent rewind and eventual final publication.
 pub(super) fn bind_widget_materialization(&self,operation:&mut WidgetMaterializationOperation)->Result<(),RuntimeError>{
  if !self.accepts_widget_materialization(operation){return Err(self.scene_resource_error("Widget materialization logical source or actual preparation is stale".into()));}
  if operation.prepared.is_some(){return Ok(());}
  let prepared=self.renderer.prepare_widget_viewport(&operation.owner,crate::widget_backend_0956::WidgetViewportOperation::Attach).map_err(|message|self.scene_resource_error(message))?;
  let object=prepared.constructed_object().filter(|object|object.owner==operation.owner&&object.generation!=0).ok_or_else(||self.scene_resource_error("Restored materialization lacks its new actual wrapper object".into()))?;
  if prepared.owner()!=&operation.owner||prepared.operation()!=crate::widget_backend_0956::WidgetViewportOperation::Attach||!prepared.constructs_wrapper()||prepared.releases_wrapper(){return Err(self.scene_resource_error("Restored materialization returned another backend/source transition".into()));}
  self.renderer.validate_widget_viewport(prepared.as_ref()).map_err(|message|self.scene_resource_error(message))?;
  operation.object=Some(object);operation.prepared=Some(prepared);Ok(())
 }
 /// PreConstruct+Construct share one transaction, one actual prepared object,
 /// existing VM/commands/RNG, and the final backend+Core publication fence.
 /// Initialized belongs to logical creation and is never dispatched here.
 pub fn execute_widget_materialization(&mut self,mut operation:WidgetMaterializationOperation)->Result<(),RuntimeError>{
  self.bind_widget_materialization(&mut operation)?;
  self.with_host_random_transaction(|engine|{
   if !engine.accepts_widget_materialization(&operation){return Err(engine.scene_resource_error("Widget materialization is stale, foreign or no longer pending".into()));}
   let handle=engine.widget_instance_handle(&operation.owner.target().alias).ok_or_else(||engine.scene_resource_error("Widget materialization lost its actual logical instance".into()))?;
   let mut next=engine.state.clone();let instance=crate::ui_consumer::instance_mut(&mut next,operation.owner.target()).map_err(|message|engine.scene_resource_error(message))?;
   instance.widget_class.as_mut().ok_or_else(||engine.scene_resource_error("Widget materialization lost its class binding".into()))?.pending_materialization=false;
   engine.widget_owner_callbacks(&mut next,&handle,&[rvn_ui::programmable::HostEventKind::PreConstruct,rvn_ui::programmable::HostEventKind::Construct])?;
   engine.commit_widget_candidate(next,operation.prepared.expect("exact prepared materialization validated before VM"))
  })
 }
}
