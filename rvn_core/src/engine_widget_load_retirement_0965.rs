//! Load validates/hydrates its target before preparing the old routable wrapper
//! retirement. Existing disposable Engine/VM planning retains the old source,
//! closures, globals and RNG. It never evaluates Destruct on the loaded target.
//! No renderer output escapes preparation; the one actual parent reservation
//! commits retirement and the already prepared target swap at the same fence.
use super::*;
use std::sync::Arc;
/// Opaque old-lifetime preparation for the existing host replacement owner.
/// Neither source state nor actual object authority can arrive through a DTO.
pub struct PreparedWidgetRuntimeRetirement{
 source:Option<super::DebugStateSource>,
 plan:Option<Arc<dyn crate::widget_backend_0956::PreparedWidgetViewportBatch>>,
}
enum WidgetReplacementPublication {
 History{pops:usize,legacy_preview:bool},
 HistoricalLoad{compatibility:LoadCompatibility,legacy_preview:bool,active_timer:Option<(f32,String,f32)>},
}
impl<R:Renderer> Engine<R>{
 /// Call only after staging/validating the incoming owner's publication.
 /// This evaluates old Destruct in existing disposable VM preparation; it
 /// grants no incoming owner, source-reader, input or publication capability.
 pub fn prepare_runtime_widget_retirement(&self)->Result<PreparedWidgetRuntimeRetirement,RuntimeError>{
  if !Self::has_widget_replacement_instances(&self.state){return Ok(PreparedWidgetRuntimeRetirement{source:None,plan:None});}
  let source=self.debug_state_source()?;let plan=self.prepare_widget_load_plan().and_then(|plan|self.prepare_widget_load_retirement(plan)).map_err(|error|self.scene_resource_error(error.to_string()))?;
  if !self.accepts_debug_state_source(&source){return Err(self.scene_resource_error("Old Widget source changed while staging runtime replacement".into()));}
  Ok(PreparedWidgetRuntimeRetirement{source:Some(source),plan})
 }
 pub fn validate_runtime_widget_retirement(&self,prepared:&PreparedWidgetRuntimeRetirement)->Result<(),RuntimeError>{
  if match &prepared.source{Some(source)=>!self.accepts_debug_state_source(source),None=>Self::has_widget_replacement_instances(&self.state)||prepared.plan.is_some()}{return Err(self.scene_resource_error("Runtime Widget retirement no longer belongs to its retained old owner".into()));}
  if let Some(plan)=&prepared.plan{self.renderer.validate_widget_viewport_batch(plan.as_ref()).map_err(|message|self.scene_resource_error(message))?;}Ok(())
 }
 /// Actual host has completed every source/resource guard. The owned ticket
 /// must be consumed in that existing source publication fence, or dropped.
 pub fn reserve_runtime_widget_retirement(&self,prepared:PreparedWidgetRuntimeRetirement)->Result<Option<Box<dyn crate::widget_backend_0956::WidgetViewportCommitReservation>>,RuntimeError>{
  self.validate_runtime_widget_retirement(&prepared)?;
  prepared.plan.map(|plan|self.renderer.reserve_widget_viewport_batch(plan)).transpose().map_err(|message|self.scene_resource_error(message))
 }
 pub(super) fn has_widget_replacement_instances(state:&GameState)->bool{
  let attached=|ui:&crate::ui::UiState|ui.screens.iter().any(|instance|instance.widget_class.as_ref().is_some_and(|binding|binding.viewport_attached));
  attached(&state.ui)||state.scene_execution.as_ref().is_some_and(|execution|execution.active.iter().chain(execution.frames.iter().filter_map(|frame|frame.parked.as_ref())).any(|scene|attached(&scene.tasks.suspended_ui)))
 }
 /// Historical formats retain their existing validation and cannot carry new
 /// class provenance. Only a current real Widget needs this replacement path.
 /// The target is validated using the original loader on the existing planning
 /// Engine before any old Destruct is evaluated. No target renderer intents
 /// leave that disposable candidate.
 pub(super) fn load_historical_with_widget_retirement(&mut self,data:SaveData)->Result<LoadCompatibility,crate::save::SaveError>{
  use crate::save::SaveError;
  let runtime=self.runtime_epoch.checked_add(1).ok_or_else(||SaveError::Incompatible("Runtime epoch exhausted".into()))?;
  let interface=self.interface_epoch.checked_add(1).ok_or_else(||SaveError::Incompatible("Interface epoch exhausted".into()))?;
  let video=self.next_video_epoch().map_err(|error|SaveError::Incompatible(error.to_string()))?;
  self.display_generation.checked_add(1).ok_or_else(||SaveError::Incompatible("Display generation exhausted".into()))?;
  let (mut batch,compatibility,legacy_preview,active_timer)={
   let mut target=self.debugger_candidate(crate::eval::debugger_eval::Probe::shared());target.functions.debug_probe=None;
   let compatibility=target.load_data_without_widget_retirement(data)?;
   let legacy_preview=target.narrative.is_none();let active_timer=target.active_timer.clone();
   let mut batch=target.build_scene_batch_with_timer_context(target.state.clone(),runtime,interface,video,active_timer.as_ref()).map_err(|error|SaveError::Incompatible(error.to_string()))?;
   if legacy_preview{batch.display=target.current_interaction().map_err(|error|SaveError::Incompatible(error.to_string()))?;}
   // The historical loader stopped speech after restoring accessibility. This
   // target-owned stop is now part of the single accepted publication, never
   // an external side effect from old Destruct planning.
   if legacy_preview&&target.renderer.supports_accessibility(){
    let stop=rvn_ui::accessibility::SpeechRequest::Stop;
    target.renderer.validate_accessibility_speech(&stop).map_err(SaveError::Incompatible)?;batch.speech_requests.push(stop);
   }
   (batch,compatibility,legacy_preview,active_timer)
  };
  batch.previous=self.state.clone();
  self.renderer.validate_scene_batch(&batch).map_err(SaveError::Incompatible)?;
  self.commit_widget_timeline_replacement(batch,WidgetReplacementPublication::HistoricalLoad{compatibility,legacy_preview,active_timer})?;
  Ok(compatibility)
 }

 /// History already owns a validated logical snapshot. Use the same old-life
 /// Destruct/source preparation and actual parent reservation as Save15, then
 /// pop only inside the accepted publication closure. No new wrapper life is
 /// replayed into the historical state or RNG.
 pub(super) fn commit_widget_history_replacement(&mut self,batch:crate::renderer::SceneRenderBatch,pops:usize,legacy_preview:bool)->Result<(),crate::save::SaveError>{
  self.commit_widget_timeline_replacement(batch,WidgetReplacementPublication::History{pops,legacy_preview})
 }
 fn commit_widget_timeline_replacement(&mut self,mut batch:crate::renderer::SceneRenderBatch,publication:WidgetReplacementPublication)->Result<(),crate::save::SaveError>{
  use crate::save::SaveError;
  if let WidgetReplacementPublication::History{pops,..}=&publication{if *pops>self.history.len(){return Err(SaveError::Incompatible("History replacement no longer owns its exact entries".into()));}}
  let participates=Self::has_widget_replacement_instances(&self.state)||Self::has_widget_replacement_instances(&batch.state);
  // Current interaction interpolation may run authored functions. It was
  // already resolved in batch.display before the actual parent reservation;
  // retain only its inert visit identity for the final no-VM publication.
  let legacy_preview=match &publication{WidgetReplacementPublication::History{legacy_preview,..}|WidgetReplacementPublication::HistoricalLoad{legacy_preview,..}=>*legacy_preview};
  let mut legacy_visit=(legacy_preview&&batch.display.is_some()).then(||LegacyPreviewVisit::capture(&batch.state,None));
  let reservation=if participates{
   self.widget_classes.validate_state(&batch.state).map_err(SaveError::Incompatible)?;
   let source=self.debug_state_source().map_err(|error|SaveError::Incompatible(error.to_string()))?;
   let plan=self.prepare_widget_load_plan()?;
   Self::defer_saved_widget_materialization(&mut batch.state);
   self.renderer.validate_scene_batch(&batch).map_err(SaveError::Incompatible)?;
   let plan=self.prepare_widget_load_retirement(plan)?;
   if !self.accepts_debug_state_source(&source){return Err(SaveError::Incompatible("Live source changed during history Widget retirement preparation".into()));}
   plan.map(|plan|self.renderer.reserve_widget_viewport_batch(plan)).transpose().map_err(SaveError::Incompatible)?
  }else{None};
  let mut batch=Some(batch);
  let mut publish=||{
   match &publication{WidgetReplacementPublication::History{pops,..}=>{for _ in 0..*pops{self.history.pop();}},WidgetReplacementPublication::HistoricalLoad{..}=>self.history.clear()};
   self.publish_scene_restore(batch.take().expect("owned timeline target published once"));
   if legacy_preview{self.legacy_preview_visit=legacy_visit.take();}
   if let WidgetReplacementPublication::HistoricalLoad{compatibility,active_timer,..}=&publication{self.active_timer=active_timer.clone();self.renderer.loaded_compatibility(*compatibility);}
  };
  if let Some(reservation)=reservation{reservation.commit(&mut publish);}else{publish();}
  Ok(())
 }
 pub(super) fn prepare_widget_load_plan(&self)->Result<Option<Arc<dyn crate::widget_backend_0956::PreparedWidgetViewportBatch>>,crate::save::SaveError>{
  use crate::save::SaveError;
  if !Self::has_widget_replacement_instances(&self.state){return Ok(None);}
  self.widget_classes.validate_state(&self.state).map_err(SaveError::Incompatible)?;
  let plan=self.renderer.prepare_widget_viewport_batch(&[],None).map_err(SaveError::Incompatible)?;
  if !plan.desired_owners().is_empty()||!plan.created().is_empty()||plan.retired().len()>32{return Err(SaveError::Incompatible("Load retirement is not an exact bounded removal of actual old parents".into()));}
  let mut generations=std::collections::BTreeSet::new();
  for object in plan.retired(){
   if object.generation==0||!generations.insert(object.generation)||object.owner.lineage()!=self.instance_id()||object.owner.epoch()!=self.runtime_epoch(){return Err(SaveError::Incompatible("Load retirement has a foreign, repeated or stale actual wrapper".into()));}
  }
  self.renderer.validate_widget_viewport_batch(plan.as_ref()).map_err(SaveError::Incompatible)?;
  for object in plan.retired(){
   let source=&object.owner;let resident=crate::ui_residency::resident_interface(&self.state,source.target()).map_err(SaveError::Incompatible)?;
   if resident.instance.order!=source.order()||resident.instance.widget_class.as_ref().is_none_or(|binding|&binding.class!=source.class()){return Err(SaveError::Incompatible("Load plan has no exact original class/instance source".into()));}
  }
  Ok(Some(plan))
 }
 pub(super) fn prepare_widget_load_retirement(&self,plan:Option<Arc<dyn crate::widget_backend_0956::PreparedWidgetViewportBatch>>)->Result<Option<Arc<dyn crate::widget_backend_0956::PreparedWidgetViewportBatch>>,crate::save::SaveError>{
  let (plan,speech)=self.prepare_widget_load_retirement_with_effects(plan)?;
  if !speech.is_empty(){return Err(crate::save::SaveError::Incompatible("This replacement host still requires its frozen old Widget Speech publication adapter".into()));}Ok(plan)
 }
 pub(super) fn prepare_widget_load_retirement_with_effects(&self,plan:Option<Arc<dyn crate::widget_backend_0956::PreparedWidgetViewportBatch>>)->Result<(Option<Arc<dyn crate::widget_backend_0956::PreparedWidgetViewportBatch>>,Vec<crate::renderer::RetiredWidgetSpeech>),crate::save::SaveError>{
  use crate::save::SaveError;
  let Some(plan)=plan else{return Ok((None,Vec::new()));};
  self.renderer.validate_widget_viewport_batch(plan.as_ref()).map_err(SaveError::Incompatible)?;
  // This is the existing disposable planning transaction, not a second active
  // VM/queue. Its state writes and RNG belong to the old lifetime and are then
  // replaced by explicit Load. Preserve actual source membership throughout.
  let mut old=self.debugger_candidate(crate::eval::debugger_eval::Probe::shared());old.functions.debug_probe=None;
  let speech=Arc::new(Mutex::new(widget_retirement_speech::SpeechLog::default()));old.retirement_speech=Some(speech.clone());
  old.with_host_random_transaction(|old|{
   let mut next=old.state.clone();next.speech_requests.clear();let mut budget=crate::ui::CanvasBudget::default();
   for object in plan.retired(){
    if old.widget_retirement_already_planned(object)?{continue;}
    let source=&object.owner;let resident=crate::ui_residency::resident_interface(&old.state,source.target()).map_err(|message|old.scene_resource_error(message))?;
    if resident.instance.order!=source.order()||resident.instance.widget_class.as_ref().is_none_or(|binding|&binding.class!=source.class()){return Err(old.scene_resource_error("Load Destruct differs from its original class/instance closure".into()));}
    let owner=resident.owner.clone();let closed=old.capture_closing_interface(&old.state,&source.target().alias)?.ok_or_else(||old.scene_resource_error("Load Destruct lost its actual closed old source".into()))?;
    let random=if matches!(&owner,crate::scene_tasks::TaskOwner::Host{..}){Some(old.capture_ui_random_authority(&old.state,source.target())?)}else{None};
    old.record_widget_retirement(object)?;
    let callback=old.owner_handler_owned_budgeted(&mut next,&source.target().alias,rvn_ui::programmable::HostEventKind::Destruct,Value::Dict(Default::default()),&owner,random,Some(closed),&mut budget)?;
    old.apply_callback_commands_budgeted(&mut next,callback,&mut budget)?;
   }
   // Menu requests still require a distinct old-source admission. Speech was
   // retained at each command with its old policy; neither effect borrows
   // source authority from the incoming saved target.
   let batch=old.build_scene_batch(next,old.runtime_epoch(),old.interface_epoch(),old.video_epoch)?;
   if !batch.menu_requests.is_empty(){return Err(old.scene_resource_error("Load Destruct external menu effects require an old-source transaction publication adapter".into()));}
   debug_assert!(batch.speech_requests.is_empty(),"Old Speech must use its per-command retained settings");
   old.renderer.validate_scene_batch(&batch).map_err(|message|old.scene_resource_error(message))?;
   Ok(())
  }).map_err(|error|SaveError::Incompatible(format!("Old Widget Destruct preparation: {error}")))?;
  self.renderer.validate_widget_viewport_batch(plan.as_ref()).map_err(SaveError::Incompatible)?;
  let retained=speech.lock().map_err(|_|SaveError::Incompatible("Old Widget Speech collector unavailable".into()))?.finish();
  Ok((Some(plan),retained))
 }
}
