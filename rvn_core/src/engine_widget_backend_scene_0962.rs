//! Actual batch-level viewport ownership; the VM and source authority remain
//! the existing Engine. Plans/reservations are runtime-only, never Save15/input.
use super::*;
use crate::widget_backend_0956::{WidgetBackendOwner,PreparedWidgetViewport,PreparedWidgetViewportBatch};
use std::sync::Arc;
pub(super) struct ClosedWidgetRetirement{
 object:crate::widget_backend_0956::WidgetBackendObjectIdentity,
 closed:super::host_consumer::ClosedInterface,
 owner:crate::scene_tasks::TaskOwner,
 pub(super) random:Option<super::host_random::SharedUiRandom>,
}
impl<R:Renderer> Engine<R>{
 pub(super) fn prepare_command_widget_retirement(&self,state:&GameState,name:&str,closed:Option<&super::host_consumer::ClosedInterface>)->Result<Option<ClosedWidgetRetirement>,RuntimeError>{
  let Some(instance)=state.ui.screens.iter().find(|instance|instance.name==name)else{return Ok(None);};
  let Some(binding)=instance.widget_class.as_ref().filter(|binding|binding.viewport_attached)else{return Ok(None);};
  let target=crate::ui_residency::visible_interface_target(state,name).map_err(|message|self.scene_resource_error(message))?;
  let resident=crate::ui_residency::resident_interface(state,&target).map_err(|message|self.scene_resource_error(message))?;
  let owner=WidgetBackendOwner{lineage:self.instance_id(),epoch:self.runtime_epoch(),target:target.clone(),order:instance.order,class:binding.class.clone()};
  let prepared=self.renderer.prepare_widget_viewport(&owner,crate::widget_backend_0956::WidgetViewportOperation::Destroy).map_err(|message|self.scene_resource_error(message))?;
  if prepared.owner()!=&owner||prepared.operation()!=crate::widget_backend_0956::WidgetViewportOperation::Destroy{return Err(self.scene_resource_error("Widget close backend returned another source or operation".into()));}
  self.renderer.validate_widget_viewport(prepared.as_ref()).map_err(|message|self.scene_resource_error(message))?;
  if !prepared.releases_wrapper(){return Ok(None);}
  let object=prepared.released_object().filter(|object|object.owner==owner&&object.generation!=0).ok_or_else(||self.scene_resource_error("Widget close has no exact actual wrapper generation".into()))?;
  if self.widget_retirement_already_planned(&object)?{return Ok(None);}
  let closed=closed.cloned().ok_or_else(||self.scene_resource_error("Widget close lacks its retained actual lifecycle source".into()))?;
  Ok(Some(ClosedWidgetRetirement{object,closed,owner:resident.owner,random:None}))
 }
 pub(super) fn finish_command_widget_retirement(&self,next:&mut GameState,retirement:ClosedWidgetRetirement,budget:&mut crate::ui::CanvasBudget)->Result<Option<super::host_random::CallbackCommands>,RuntimeError>{
  if !self.record_widget_retirement(&retirement.object)?{return Ok(None);}
  let captured=retirement.random.as_ref().map(|random|random.borrow().clone());
  let mut callback=self.owner_handler_owned_budgeted(next,&retirement.object.owner.target().alias,rvn_ui::programmable::HostEventKind::Destruct,Value::Dict(Default::default()),&retirement.owner,captured,Some(retirement.closed),budget)?;
  if let Some(shared)=retirement.random{
   let updated=callback.random.as_ref().ok_or_else(||self.scene_resource_error("Closed Widget Destruct lost its original Host random source".into()))?.borrow().clone();
   *shared.borrow_mut()=updated;callback.random=Some(shared);
  }Ok(Some(callback))
 }

 pub(super) fn widget_backend_owners_in(&self,state:&GameState,epoch:u64)->Result<Vec<WidgetBackendOwner>,RuntimeError>{
  self.widget_classes.validate_state(state).map_err(|message|self.scene_resource_error(message))?;
  let mut owners=Vec::new();
  for instance in &state.ui.screens{
   let Some(binding)=&instance.widget_class else{continue;};if !binding.viewport_attached||binding.pending_materialization{continue;}
   let target=crate::ui_residency::visible_interface_target(state,&instance.name).map_err(|message|self.scene_resource_error(message))?;
   let resident=crate::ui_residency::resident_interface(state,&target).map_err(|message|self.scene_resource_error(message))?;
   if resident.residence!=crate::ui_residency::ProjectionResidence::Visible||resident.instance.order!=instance.order||resident.instance.widget_class.as_ref()!=Some(binding){return Err(self.scene_resource_error("Widget batch owner is not its actual visible source incarnation".into()));}
   owners.push(WidgetBackendOwner{lineage:self.instance_id(),epoch,target,order:instance.order,class:binding.class.clone()});
  }Ok(owners)
 }
 /// Reconcile actual objects before authoritative normal scene publication.
 /// The already prepared explicit API operation keeps its exact object and its
 /// already evaluated lifecycle. Implicit removals (UiClose/scene replacement)
 /// retain the old class/target for Destruct instead of borrowing a new alias.
 pub(super) fn prepare_widget_scene_publication(&self,mut batch:crate::renderer::SceneRenderBatch,transition:Option<&dyn PreparedWidgetViewport>)->Result<(crate::renderer::SceneRenderBatch,Option<Arc<dyn PreparedWidgetViewportBatch>>),RuntimeError>{
  let had=self.state.ui.screens.iter().any(|instance|instance.widget_class.as_ref().is_some_and(|binding|binding.viewport_attached));
  let owners=self.widget_backend_owners_in(&batch.state,batch.runtime_epoch)?;
  if !had&&owners.is_empty()&&transition.is_none(){return Ok((batch,None));}
  let plan=self.renderer.prepare_widget_viewport_batch(&owners,transition).map_err(|message|self.scene_resource_error(message))?;
  if plan.desired_owners()!=owners.as_slice()||plan.created().len()>32||plan.retired().len()>32{return Err(self.scene_resource_error("Widget backend batch differs from exact candidate owners or bound".into()));}
  let mut identities=std::collections::BTreeSet::new();
  for object in plan.created().iter().chain(plan.retired()){
   if object.generation==0||!identities.insert(object.generation){return Err(self.scene_resource_error("Widget backend batch repeats an invalid actual object generation".into()));}
  }
  let mut next=batch.state.clone();let mut changed=false;let mut budget=crate::ui::CanvasBudget::default();
  for object in plan.retired(){
   // Logical hydration may evaluate a saved operation in a disposable target
   // candidate. Exact old objects belong to the separately staged old-source
   // Destruct transaction, never to the loaded target's globals or epoch.
   if let Some(retiring)=self.planning_load_retirement.as_ref().filter(|retiring|retiring.retired().contains(object)){
    self.renderer.validate_widget_viewport_batch(retiring.as_ref()).map_err(|message|self.scene_resource_error(message))?;
    continue;
   }
   if transition.is_some_and(|single|single.owner()==&object.owner&&single.releases_wrapper())||self.widget_retirement_already_planned(object)?{continue;}
   let source=&object.owner;
   if source.lineage()!=self.instance_id()||source.epoch()!=self.runtime_epoch(){return Err(self.scene_resource_error("Retiring Widget object has no original live runtime source".into()));}
   let resident=crate::ui_residency::resident_interface(&self.state,source.target()).map_err(|message|self.scene_resource_error(message))?;
   if resident.instance.order!=source.order()||resident.instance.widget_class.as_ref().is_none_or(|binding|&binding.class!=source.class()){return Err(self.scene_resource_error("Retiring Widget callback differs from retained actual class/instance".into()));}
   let owner=resident.owner.clone();
   let closed=self.capture_closing_interface(&self.state,&source.target().alias)?.ok_or_else(||self.scene_resource_error("Retiring Widget lost its exact closed lifecycle context".into()))?;
   let random=if matches!(&owner,crate::scene_tasks::TaskOwner::Host{..}){Some(self.capture_ui_random_authority(&self.state,source.target())?)}else{None};
   let callback=self.owner_handler_owned_budgeted(&mut next,&source.target().alias,rvn_ui::programmable::HostEventKind::Destruct,Value::Dict(Default::default()),&owner,random,Some(closed),&mut budget)?;
   self.apply_callback_commands_budgeted(&mut next,callback,&mut budget)?;changed=true;
  }
  for object in plan.created(){
   if !owners.contains(&object.owner){return Err(self.scene_resource_error("Constructed Widget object is outside its actual desired owner batch".into()));}
   if transition.is_some_and(|single|single.owner()==&object.owner&&single.constructs_wrapper()){continue;}
   let source=&object.owner;
   if source.lineage()!=self.instance_id()||source.epoch()!=self.runtime_epoch(){return Err(self.scene_resource_error("Widget construction requires its exact runtime lifecycle continuation".into()));}
   for kind in [rvn_ui::programmable::HostEventKind::PreConstruct,rvn_ui::programmable::HostEventKind::Construct]{
    let resident=crate::ui_residency::resident_interface(&next,source.target()).map_err(|message|self.scene_resource_error(message))?;
    if resident.instance.order!=source.order()||resident.instance.widget_class.as_ref().is_none_or(|binding|&binding.class!=source.class()){return Err(self.scene_resource_error("Constructed Widget callback lost its exact class/instance".into()));}
    let owner=resident.owner.clone();let callback=self.owner_handler_owned_budgeted(&mut next,&source.target().alias,kind,Value::Dict(Default::default()),&owner,None,None,&mut budget)?;
    self.apply_callback_commands_budgeted(&mut next,callback,&mut budget)?;changed=true;
   }
  }
  if changed{
   // A callback may change globals or legacy children. Typed structural
   // changes need their own real object/lifecycle continuation, never a second
   // fabricated allocation or an alias fallback inside this finished plan.
   if self.widget_backend_owners_in(&next,batch.runtime_epoch)?!=owners{return Err(self.scene_resource_error("Widget lifecycle changed the typed owner set before its staged continuation was complete".into()));}
   batch=self.build_scene_batch(next,batch.runtime_epoch,batch.interface_epoch,batch.video_epoch)?;
  }
  self.renderer.validate_widget_viewport_batch(plan.as_ref()).map_err(|message|self.scene_resource_error(message))?;
  Ok((batch,Some(plan)))
 }
}
