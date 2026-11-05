//! Saved logical owner work. No actual wrapper generation, Arc, Entity,
//! camera, input token or physical Ready bit is admitted by this descriptor.
use super::*;
use serde::{Deserialize,Serialize};
use crate::consumer_schedule::HostDefinitionKind;
#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PortableWidgetMaterialization{
 target:crate::scene_resources::TargetIdentity,
 class:crate::widget_class_source_0950::WidgetClassIdentity,
 order:u64,
 methods:Vec<PortableWidgetMethod>,
}
#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct PortableWidgetMethod{kind:rvn_ui::programmable::HostEventKind,name:String,source_ordinal:usize,native:bool}
impl PortableWidgetMaterialization{
 pub(crate) fn target(&self)->&crate::scene_resources::TargetIdentity{&self.target}
 pub(crate) fn validate_wire(&self)->Result<(),String>{
  if self.target.kind!=crate::scene_resources::TargetKind::Interface||self.target.alias.is_empty()||self.target.alias.len()>128||self.class.uuid.len()!=32||self.class.source.len()>4096||self.class.definition.len()>128||self.methods.len()>2{return Err("Invalid bounded saved Widget lifecycle descriptor".into());}
  let mut seen=std::collections::BTreeSet::new();
  for method in &self.methods{if !matches!(method.kind,rvn_ui::programmable::HostEventKind::PreConstruct|rvn_ui::programmable::HostEventKind::Construct)||!seen.insert(method.kind)||method.name.is_empty()||method.name.len()>128{return Err("Invalid saved Widget lifecycle method sequence".into());}}
  Ok(())
 }
}
impl<R:Renderer> Engine<R>{
 pub(crate) fn portable_widget_materialization(&self,operation:&WidgetMaterializationOperation)->Result<PortableWidgetMaterialization,String>{
  if !self.accepts_widget_materialization(operation){return Err("Saved Widget lifecycle has no exact pending logical owner/source".into());}
  let mut next=self.state.clone();let globals=self.global_vars_for_state(&next);
  self.ui_library.prepare_owner_properties(&mut next.ui,&self.functions,&globals,self.locale.as_ref()).map_err(|error|error.to_string())?;
  let resident=crate::ui_residency::resident_interface(&next,operation.owner.target())?;
  if resident.instance.order!=operation.owner.order()||resident.instance.widget_class.as_ref().is_none_or(|binding|binding.class!=*operation.owner.class()||!binding.pending_materialization){return Err("Saved Widget lifecycle differs from exact admitted class instance".into());}
  let mut methods=Vec::new();
  for kind in [rvn_ui::programmable::HostEventKind::PreConstruct,rvn_ui::programmable::HostEventKind::Construct]{
   if let Some(name)=resident.instance.owner_properties.events.get(&kind){
    self.widget_classes.validate_owner_handler(resident.instance.widget_class.as_ref(),name)?;
    let native=self.consumer_program.native_host_definition(&HostDefinitionKind::Handler,name);
    let source_ordinal=if native{self.consumer_program.host_definition(&HostDefinitionKind::Handler,name).filter(|definition|definition.kind==HostDefinitionKind::Handler&&definition.source_ordinal==operation.owner.class().source_ordinal).ok_or("Saved Widget lifecycle lacks its real frozen Handler source")?.source_ordinal}else{operation.owner.class().source_ordinal};
    methods.push(PortableWidgetMethod{kind,name:name.clone(),source_ordinal,native});
   }
  }
  let portable=PortableWidgetMaterialization{target:operation.owner.target().clone(),class:operation.owner.class().clone(),order:operation.owner.order(),methods};portable.validate_wire()?;Ok(portable)
 }
 pub(crate) fn restore_widget_materialization(&self,portable:&PortableWidgetMaterialization)->Result<WidgetMaterializationOperation,String>{
  portable.validate_wire()?;self.widget_classes.validate_state(&self.state)?;
  let resident=crate::ui_residency::resident_interface(&self.state,&portable.target)?;
  if resident.residence!=crate::ui_residency::ProjectionResidence::Visible||resident.instance.order!=portable.order||resident.instance.widget_class.as_ref().is_none_or(|binding|binding.class!=portable.class||!binding.viewport_attached||!binding.pending_materialization){return Err("Saved Widget lifecycle does not match its actual restored logical owner/source".into());}
  let owner=crate::widget_backend_0956::WidgetBackendOwner{lineage:self.instance_id(),epoch:self.runtime_epoch(),target:portable.target.clone(),order:portable.order,class:portable.class.clone()};
  let operation=WidgetMaterializationOperation{owner,object:None,prepared:None};
  if self.portable_widget_materialization(&operation)?!=*portable{return Err("Saved Widget lifecycle methods/source differ from its real current Screen description".into());}
  Ok(operation)
 }
 pub(crate) fn widget_materialization_namespaces(&self,portable:&PortableWidgetMaterialization)->Result<std::collections::BTreeMap<String,std::collections::BTreeMap<String,String>>,String>{
  self.restore_widget_materialization(portable)?;
  let binding=self.consumer_program.source.as_ref().ok_or("Widget lifecycle has no actual frozen source namespace")?;
  let mut roots=std::collections::BTreeMap::new();
  for(root,definition)in &binding.definitions{
   let name=&definition.definition;let kind=match definition.kind{HostDefinitionKind::Screen=>"screen",HostDefinitionKind::Handler=>"handler"};
   let scopes=definition.scopes.keys().map(|scope|(scope.clone(),format!("host-source:{}:{kind}:{}:{name}:{scope}",definition.source_ordinal,name.len()))).collect();roots.insert(root.clone(),scopes);
  }Ok(roots)
 }
}
