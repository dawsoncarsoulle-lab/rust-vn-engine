//! Logical Widget class/instance port on the existing Engine, instance snapshots
//! and resource registry. Physical wrapper lifecycle/focus stays backend-owned.
#[path="engine_widget_viewport_1003.rs"]
mod viewport1003;
use super::*;
use crate::widget_class_source_0950::{AdmittedWidgetClasses,WidgetInstanceBinding};
use crate::scene_resources::{TargetIdentity,TargetKind,ResourceTaskKind};
use crate::scene_tasks::TaskOwner;

/// A process-local reference to an actual existing interface incarnation.
/// Never serialized, deserialized, reconstructed from an alias, or saved.
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct WidgetInstanceHandle{instance:u64,epoch:u64,target:TargetIdentity}
impl WidgetInstanceHandle{pub fn alias(&self)->&str{&self.target.alias}}
impl<R:Renderer> Engine<R>{
 pub fn prepare_widget_class_startup(program:rvn_parser::ParsedProgram,classes:AdmittedWidgetClasses,renderer:R,rollback_depth:usize,persistent_vars:HashMap<String,Value>,text_context:TextRuntimeContext)->Result<EngineStartup<R>,RuntimeError>{
  if !classes.matches_program(&program).map_err(|s|RuntimeError::no_stmt(crate::error::RuntimeErrorKind::EvalError(EvalError::InvalidFunction(s)),0))?{return Err(RuntimeError::no_stmt(crate::error::RuntimeErrorKind::EvalError(EvalError::InvalidFunction("Widget classes differ from the actual loader program".into())),0));}
  Self::prepare_owned_startup(std::sync::Arc::new(program),renderer,rollback_depth,persistent_vars,text_context,None,(0,0,0))?.with_widget_classes(std::sync::Arc::new(classes))
 }
 /// The same source-owned bootstrap; class and overlay admission precede Init.
 pub fn prepare_widget_class_debugger_startup(program:rvn_parser::ParsedProgram,classes:AdmittedWidgetClasses,renderer:R,rollback_depth:usize,persistent_vars:HashMap<String,Value>,text_context:TextRuntimeContext,overlay:Option<(crate::debugger_provenance::Overlay,u64)>)->Result<EngineStartup<R>,RuntimeError>{
  if !classes.matches_program(&program).map_err(|s|RuntimeError::no_stmt(crate::error::RuntimeErrorKind::EvalError(EvalError::InvalidFunction(s)),0))?{return Err(RuntimeError::no_stmt(crate::error::RuntimeErrorKind::EvalError(EvalError::InvalidFunction("Widget classes differ from the actual loader program".into())),0));}
  Self::prepare_debugger_startup(program,renderer,rollback_depth,persistent_vars,text_context,overlay)?.with_widget_classes(std::sync::Arc::new(classes))
 }
 pub fn from_program_with_widget_classes_and_text_context(program:rvn_parser::ParsedProgram,classes:AdmittedWidgetClasses,renderer:R,rollback_depth:usize,persistent_vars:HashMap<String,Value>,text_context:TextRuntimeContext)->Result<Self,RuntimeError>{Self::prepare_widget_class_startup(program,classes,renderer,rollback_depth,persistent_vars,text_context)?.finish_synchronous()}
 pub fn from_program_with_widget_classes(program:rvn_parser::ParsedProgram,classes:AdmittedWidgetClasses,renderer:R,rollback_depth:usize)->Result<Self,RuntimeError>{Self::prepare_widget_class_startup(program,classes,renderer,rollback_depth,Default::default(),Default::default())?.finish_synchronous()}
 pub fn widget_classes(&self)->&AdmittedWidgetClasses{&self.widget_classes}
 fn widget_instance<'a>(&self,state:&'a GameState,handle:&WidgetInstanceHandle)->Result<&'a crate::ui::ScreenInstance,RuntimeError>{
  if handle.instance!=self.instance_id()||handle.epoch!=self.runtime_epoch(){return Err(self.scene_resource_error("Widget handle belongs to another Engine/runtime incarnation".into()));}
  self.widget_classes.validate_state(state).map_err(|s|self.scene_resource_error(s))?;
  let resident=crate::ui_residency::resident_interface(state,&handle.target).map_err(|s|self.scene_resource_error(s))?;
  if resident.instance.widget_class.is_none(){return Err(self.scene_resource_error("Widget handle cannot adopt a legacy Screen".into()));}Ok(resident.instance)
 }
 /// Creates one detached logical instance. Initialization reuses the actual
 /// Screen evaluator/consumer frame; no viewport, fake child or Construct event
 /// is fabricated here. Two calls for one class retain distinct source targets.
 pub fn create_widget(&mut self,uuid:&str,alias:&str,arguments:Vec<Value>)->Result<WidgetInstanceHandle,RuntimeError>{
  self.with_host_random_transaction(|engine|engine.create_widget_planned(uuid,alias,arguments))
 }
 fn create_widget_planned(&mut self,uuid:&str,alias:&str,arguments:Vec<Value>)->Result<WidgetInstanceHandle,RuntimeError>{
  // Preserve the original refusal order and avoid cloning state for invalid
  // UUID, alias, argument count or oversized/nonfinite argument values.
  let class=self.preflight_widget_creation(&self.state,uuid,alias,&arguments)?;
  let mut next=self.state.clone();
  let target=self.prepare_widget_creation_with_class(&mut next,class,alias,arguments)?;
  self.commit_ui_state(next)?;
  // No acquired handle exists before the actual accepted publication.
  Ok(WidgetInstanceHandle{instance:self.instance_id(),epoch:self.runtime_epoch(),target})
 }
 fn preflight_widget_creation(&self,next:&GameState,uuid:&str,alias:&str,arguments:&[Value])->Result<crate::widget_class_source_0950::WidgetClassIdentity,RuntimeError>{
  let class=self.widget_classes.class(uuid).ok_or_else(||self.scene_resource_error("Widget class is not admitted by the actual loader".into()))?.identity().clone();
  if alias.is_empty()||alias.len()>128||alias.chars().any(char::is_control)||next.ui.screens.iter().any(|instance|instance.name==alias){return Err(self.scene_resource_error("Widget instance alias is invalid or already resident".into()));}
  let(parameters,_)=self.ui_library.screen_definition(&class.definition).ok_or_else(||self.scene_resource_error("Admitted Widget Screen definition is missing".into()))?;
  if parameters.len()!=arguments.len()||arguments.len()>128{return Err(self.scene_resource_error("Widget arguments differ from its actual Screen parameters".into()));}
  for argument in arguments{crate::value_limits::value_work(argument).map_err(|e|self.eval_err(e,"Widget arguments"))?;}
  Ok(class)
 }
 /// Stage the same logical factory in this Engine's disposable candidate.
 /// The caller owns the existing host-random transaction and discards the
 /// entire candidate on refusal/abandonment. No commit, backend publication or
 /// acquired WidgetInstanceHandle is produced. TargetIdentity is inert data.
 /// Current Game ownership and existing per-stage budgets remain unchanged.
 pub(super) fn prepare_widget_creation_in(&self,next:&mut GameState,uuid:&str,alias:&str,arguments:Vec<Value>)->Result<TargetIdentity,RuntimeError>{
  let class=self.preflight_widget_creation(next,uuid,alias,&arguments)?;
  self.prepare_widget_creation_with_class(next,class,alias,arguments)
 }
 // Only the two validated entries above can reach this shared mutation body.
 fn prepare_widget_creation_with_class(&self,next:&mut GameState,class:crate::widget_class_source_0950::WidgetClassIdentity,alias:&str,arguments:Vec<Value>)->Result<TargetIdentity,RuntimeError>{
  if next.ui.screens.len()>=32{return Err(self.scene_resource_error("32 interface instances maximum".into()));}
  let order=next.ui.next_order;next.ui.next_order=order.checked_add(1).ok_or_else(||self.eval_err(EvalError::NumericOverflow,"Widget order"))?;
  let random=self.evaluate_command_random(next,&TaskOwner::Game,None,|_,random|Ok(crate::random::RandomState::seeded(random.next_u64())))?;
  next.ui.screens.push(crate::ui::ScreenInstance{name:alias.into(),widget_class:Some(WidgetInstanceBinding{class:class.clone(),viewport_attached:false,pending_materialization:false}),host_role:None,host_root:false,arguments,modal:false,layer:0,order,focus:None,owner_properties:Default::default(),owner_focus_request:None,values:Default::default(),random,canvas_states:Default::default(),consumer_frames:Default::default(),consumer_projection:None,standard_macro_frame:None});
  self.install_owned_resource(next,TaskOwner::Game,ResourceTaskKind::Interface,TargetKind::Interface,alias)?;
  let target=crate::ui_residency::visible_interface_target(next,alias).map_err(|s|self.scene_resource_error(s))?;
  if self.consumer_program.native_host_definition(&crate::consumer_schedule::HostDefinitionKind::Screen,&class.definition){self.prepare_native_screen(next,alias,&mut crate::ui::CanvasBudget::default())?;}
  let initialized=self.ui_library.with_locale(self.locale.as_ref()).initialized_child_views(&next.ui,&self.functions,&self.global_vars_for_state(next)).map_err(|e|self.eval_err(e,"Widget instance initialization"))?;
  self.ui_library.reconcile_canvas_states_from_views(&mut next.ui,&initialized).map_err(|e|self.eval_err(e,"Widget child initial state"))?;
  // Logical initialization follows creation of this actual class instance,
  // independently of its physical wrapper or viewport attachment. Save15
  // restores existing instances directly and never replays this creation path.
  let initialization_globals=self.global_vars_for_state(next);
  self.ui_library.prepare_owner_properties(&mut next.ui,&self.functions,&initialization_globals,self.locale.as_ref()).map_err(|error|self.eval_err(error,"Widget initialization description"))?;
  let callback=self.owner_handler_owned_budgeted(next,alias,rvn_ui::programmable::HostEventKind::Initialized,Value::Dict(Default::default()),&TaskOwner::Game,None,None,&mut crate::ui::CanvasBudget::default())?;
  self.apply_callback_commands_budgeted(next,callback,&mut crate::ui::CanvasBudget::default())?;
  let resident=crate::ui_residency::resident_interface(next,&target).map_err(|message|self.scene_resource_error(message))?;
  if resident.instance.widget_class.as_ref().is_none_or(|binding|binding.class!=class){return Err(self.scene_resource_error("Widget initialization replaced its actual class instance".into()));}
  Ok(target)
 }
 /// Obtain the backend source only from an actual process-local instance
 /// handle. This never grants focus or input and cannot be decoded from Save15.
 pub fn widget_backend_owner(&self,handle:&WidgetInstanceHandle)->Result<crate::widget_backend_0956::WidgetBackendOwner,RuntimeError>{
  let instance=self.widget_instance(&self.state,handle)?;
  Ok(crate::widget_backend_0956::WidgetBackendOwner{lineage:self.instance_id(),epoch:self.runtime_epoch(),target:handle.target.clone(),order:instance.order,class:instance.widget_class.as_ref().ok_or_else(||self.scene_resource_error("Widget class binding disappeared".into()))?.class.clone()})
 }
 fn prepare_widget_backend(&self,handle:&WidgetInstanceHandle,operation:crate::widget_backend_0956::WidgetViewportOperation)->Result<std::sync::Arc<dyn crate::widget_backend_0956::PreparedWidgetViewport>,RuntimeError>{
  let owner=self.widget_backend_owner(handle)?;let prepared=self.renderer.prepare_widget_viewport(&owner,operation).map_err(|message|self.scene_resource_error(message))?;
  if prepared.owner()!=&owner||prepared.operation()!=operation||operation!=crate::widget_backend_0956::WidgetViewportOperation::Attach&&prepared.constructs_wrapper(){return Err(self.scene_resource_error("Widget backend preparation differs from exact requested source/operation".into()));}Ok(prepared)
 }
 pub(super) fn widget_owner_callbacks(&self,next:&mut GameState,handle:&WidgetInstanceHandle,kinds:&[rvn_ui::programmable::HostEventKind])->Result<(),RuntimeError>{
  let resident=crate::ui_residency::resident_interface(next,&handle.target).map_err(|message|self.scene_resource_error(message))?;let owner=resident.owner.clone();let class=resident.instance.widget_class.as_ref().ok_or_else(||self.scene_resource_error("Widget lifecycle lost its class".into()))?.class.clone();
  let globals=self.global_vars_for_state(next);self.ui_library.prepare_owner_properties(&mut next.ui,&self.functions,&globals,self.locale.as_ref()).map_err(|error|self.eval_err(error,"Widget wrapper description"))?;
  let mut budget=crate::ui::CanvasBudget::default();
  for kind in kinds{
   let callbacks=self.owner_handler_owned_budgeted(next,&handle.target.alias,*kind,Value::Dict(Default::default()),&owner,None,None,&mut budget)?;
   self.apply_callback_commands_budgeted(next,callbacks,&mut budget)?;
   let instance=self.widget_instance(next,handle)?;if instance.widget_class.as_ref().is_none_or(|binding|binding.class!=class){return Err(self.scene_resource_error("Widget wrapper callback replaced its actual class instance".into()));}
  }Ok(())
 }
 pub(super) fn commit_widget_candidate(&mut self,next:GameState,prepared:std::sync::Arc<dyn crate::widget_backend_0956::PreparedWidgetViewport>)->Result<(),RuntimeError>{
  if self.pending_widget_backend.is_some(){return Err(self.scene_resource_error("Nested Widget backend publication is unsupported".into()));}
  self.pending_widget_backend=Some(prepared);let result=self.commit_ui_state(next);self.pending_widget_backend=None;result
 }
 /// The real backend wrapper is prepared before VM callbacks, independently
 /// of whether any paint occurs. Final ownership/state publication is atomic.
 pub fn add_widget_to_viewport(&mut self,handle:&WidgetInstanceHandle,modal:bool,layer:i32)->Result<(),RuntimeError>{
  self.with_host_random_transaction(|engine|{
   let mut next=engine.state.clone();
   let prepared=engine.stage_widget_viewport(&mut next,handle,viewport1003::ViewportAction::Add{z_order:layer,modal})?.ok_or_else(||engine.scene_resource_error("Widget attachment lost its prepared backend".into()))?;
   engine.commit_widget_candidate(next,prepared)
  })
 }
 pub fn remove_widget_from_viewport(&mut self,handle:&WidgetInstanceHandle)->Result<(),RuntimeError>{
  self.with_host_random_transaction(|engine|{
   let mut next=engine.state.clone();
   match engine.stage_widget_viewport(&mut next,handle,viewport1003::ViewportAction::Remove)?{
    Some(prepared)=>engine.commit_widget_candidate(next,prepared),None=>Ok(())
   }
  })
 }
 pub fn destroy_widget(&mut self,handle:WidgetInstanceHandle)->Result<(),RuntimeError>{
  self.with_host_random_transaction(|engine|{
   engine.widget_instance(&engine.state,&handle)?;let prepared=engine.prepare_widget_backend(&handle,crate::widget_backend_0956::WidgetViewportOperation::Destroy)?;let mut next=engine.state.clone();
   if prepared.releases_wrapper(){let object=prepared.released_object().ok_or_else(||engine.scene_resource_error("Widget backend release lost its exact wrapper generation".into()))?;engine.record_widget_retirement(&object)?;engine.widget_owner_callbacks(&mut next,&handle,&[rvn_ui::programmable::HostEventKind::Destruct])?;}
   let owner=crate::ui_residency::resident_interface(&next,&handle.target).map_err(|message|engine.scene_resource_error(message))?.owner.clone();
   crate::scene_resources::remove_target(&mut next,&owner,TargetKind::Interface,&handle.target.alias).map_err(|message|engine.scene_resource_error(message))?;engine.commit_widget_candidate(next,prepared)
  })
 }
 /// Explicit current re-acquisition after a compatible Save15 restore. This
 /// returns a fresh process/epoch handle; an old handle is never revived.
 pub fn widget_instance_handle(&self,alias:&str)->Option<WidgetInstanceHandle>{
  let target=crate::ui_residency::visible_interface_target(&self.state,alias).ok()?;let resident=crate::ui_residency::resident_interface(&self.state,&target).ok()?;resident.instance.widget_class.as_ref()?;
  self.widget_classes.validate_state(&self.state).ok()?;Some(WidgetInstanceHandle{instance:self.instance_id(),epoch:self.runtime_epoch(),target})
 }
}

impl<R:Renderer> Engine<R>{
 pub(super) fn widget_reference_value(&self,handle:&WidgetInstanceHandle)->Result<Value,RuntimeError>{
  let resident=self.widget_instance(&self.state,handle)?;
  let identity=&resident.widget_class.as_ref().ok_or_else(||self.scene_resource_error("Widget reference requires an actual class".into()))?.class;
  let class=rvn_parser::widget_reference::WidgetClassReference::new(identity.uuid.clone(),identity.receipt).map_err(|error|self.scene_resource_error(error))?;
  let portable=rvn_parser::widget_reference::PortableWidgetReference::new(class,handle.target.alias.clone(),handle.target.incarnation).map_err(|error|self.scene_resource_error(error))?;
  let value=rvn_parser::widget_reference::WidgetReference::bind_admitted(portable,self.instance_id(),self.runtime_epoch()).map_err(|error|self.scene_resource_error(error))?;
  Ok(Value::Widget(Box::new(value)))
 }
 pub(super) fn widget_reference_handle(&self,value:&Value)->Result<WidgetInstanceHandle,RuntimeError>{
  let Value::Widget(reference)=value else{return Err(self.scene_resource_error("Expected a typed Widget reference".into()));};
  let admission=crate::widget_reference_values_0974::WidgetValueAdmission::new(&self.state,self.widget_classes.clone(),self.instance_id(),self.runtime_epoch()).map_err(|error|self.scene_resource_error(error))?;
  let target=admission.target(reference).map_err(|error|self.scene_resource_error(error))?;
  let handle=WidgetInstanceHandle{instance:self.instance_id(),epoch:self.runtime_epoch(),target};
  self.widget_instance(&self.state,&handle)?;Ok(handle)
 }
}
