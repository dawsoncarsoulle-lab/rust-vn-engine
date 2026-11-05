//! Actual Graph-admitted class instance dispatch, continuation and hydration tests.
#[path="engine_widget_handler_remove_tests1003.rs"]
mod handler_remove1003;
#[path="widget_viewport_scene_tests1003.rs"]
mod viewport1003;
use super::*;
#[path="widget_macro_engine_tests_1020.rs"]
mod standard_macros1020;
#[path="widget_class_getter_tests_0981.rs"]
mod class_getter0981;
#[path="widget_class_vm_tests_1000.rs"]
mod class_vm1000;
#[path="widget_object_class_tests_1002.rs"]
mod object_class1002;
#[path="engine_widget_authored_scene_tests_0978.rs"]
mod authored_scene0978;
use crate::widget_class_source_0950::{AdmittedWidgetClasses,CompilerWidgetClass,CompilerWidgetMember,WidgetMemberKind};
use rvn_parser::widget_asset::{WidgetBlueprintV1,WidgetGraphRef16,WidgetIndexInput,LocatorCase};
use std::{fs,path::PathBuf,sync::{Arc,Mutex,atomic::{AtomicBool,AtomicU64,Ordering}}};
const A:&str="17411a3e431e4c9388b78a2b9825b130";
const B:&str="3fc40c36f4e544ed82943e4c6fe9d87d";
#[derive(Default,Clone)]
struct Host{batches:Arc<Mutex<Vec<crate::renderer::SceneRenderBatch>>>,reject:Arc<AtomicBool>,tints:Arc<Mutex<Vec<(String,Option<String>)>>>}
impl Renderer for Host{
 fn prepare_widget_viewport_batch(&self,owners:&[crate::widget_backend_0956::WidgetBackendOwner],transition:Option<&dyn crate::widget_backend_0956::PreparedWidgetViewport>)->Result<Arc<dyn crate::widget_backend_0956::PreparedWidgetViewportBatch>,String>{if let Some(transition)=transition{self.validate_widget_viewport(transition)?;}Ok(Arc::new(CpuViewportBatch{owners:owners.to_vec()}))}
 fn validate_widget_viewport_batch(&self,batch:&dyn crate::widget_backend_0956::PreparedWidgetViewportBatch)->Result<(),String>{if batch.as_any().is::<CpuViewportBatch>(){Ok(())}else{Err("Foreign CPU logical test batch".into())}}
 fn reserve_widget_viewport_batch(&self,batch:Arc<dyn crate::widget_backend_0956::PreparedWidgetViewportBatch>)->Result<Box<dyn crate::widget_backend_0956::WidgetViewportCommitReservation>,String>{self.validate_widget_viewport_batch(batch.as_ref())?;Ok(Box::new(CpuViewportReservation))}
 fn prepare_widget_viewport(&self,owner:&crate::widget_backend_0956::WidgetBackendOwner,operation:crate::widget_backend_0956::WidgetViewportOperation)->Result<Arc<dyn crate::widget_backend_0956::PreparedWidgetViewport>,String>{Ok(Arc::new(CpuViewportPlan{owner:owner.clone(),operation}))}
 fn validate_widget_viewport(&self,plan:&dyn crate::widget_backend_0956::PreparedWidgetViewport)->Result<(),String>{if plan.as_any().is::<CpuViewportPlan>(){Ok(())}else{Err("Foreign CPU test backend plan".into())}}

 fn supports_programmable_ui(&self)->bool{true}
 fn validate_scene_batch(&self,_:&crate::renderer::SceneRenderBatch)->Result<(),String>{if self.reject.load(Ordering::SeqCst){Err("Widget renderer refusal".into())}else{Ok(())}}
 fn publish_scene_batch(&mut self,batch:crate::renderer::SceneRenderBatch){self.batches.lock().unwrap().push(batch);}
 fn update_interfaces(&mut self,_:&[rvn_ui::programmable::ScreenView])->Result<(),String>{Ok(())}
 fn set_background(&mut self,_:&str,_:&Transition){}
 fn show_sprite(&mut self,_:&str,_:Option<&str>,_:&Position,_:&Transition,_:Option<&SpriteState>){}
 fn hide_sprite(&mut self,_:&str,_:&Transition,_:&SpriteState){}
 fn move_sprite(&mut self,_:&str,_:&Position,_:&Transition,_:&SpriteState){}
 fn show_dialogue(&mut self,_:Option<&str>,_:&str){}
 fn show_choice(&mut self,_:&[String])->usize{0}
 fn show_imagemap(&mut self,_:&str,_:Option<&str>,_:&[Hotspot])->usize{0}
 fn music_play(&mut self,_:&str,_:&Transition,_:Option<&str>){}
 fn music_stop(&mut self,_:&Transition){}
 fn music_set_volume(&mut self,_:f32){}
 fn sfx_play(&mut self,_:&str,_:&Transition){}
 fn sfx_stop(&mut self,_:&str,_:&Transition){}
 fn set_sprite_effect(&mut self,id:&str,_:Option<bool>,_:Option<bool>,_:Option<f32>,_:Option<f32>,tint:Option<&str>){self.tints.lock().unwrap().push((id.into(),tint.map(str::to_owned)));}
}

struct Fixture{root:PathBuf,program:rvn_parser::ParsedProgram,classes:AdmittedWidgetClasses}
impl Fixture{
 fn new(native:bool)->Self{Self::with_initialized(native,false)}
 fn with_initialized(native:bool,initialized:bool)->Self{Self::with_initialized_command_refusal(native,initialized,false)}
 fn with_initialized_command_refusal(native:bool,initialized:bool,refuse_command:bool)->Self{Self::with_object_query(native,initialized,refuse_command,false)}
 fn with_object_query(native:bool,initialized:bool,refuse_command:bool,object_query:bool)->Self{Self::with_source_transform(native,initialized,refuse_command,object_query,|source|source)}
 fn with_source_transform(native:bool,initialized:bool,refuse_command:bool,object_query:bool,transform:impl FnOnce(String)->String)->Self{
  Self::with_source_transform_at(native,initialized,refuse_command,object_query,|_,source|transform(source))
 }
 fn with_source_transform_at(native:bool,initialized:bool,refuse_command:bool,object_query:bool,transform:impl FnOnce(&PathBuf,String)->String)->Self{
  static NEXT:AtomicU64=AtomicU64::new(1);let root=std::env::temp_dir().join(format!("rvn-widget-membership-0956-{}-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos(),NEXT.fetch_add(1,Ordering::Relaxed)));fs::create_dir_all(&root).unwrap();let root=fs::canonicalize(root).unwrap();
  let handlers="handler allowed(event){local roll=random(1,100) while false{set roll=roll+1} set hits=hits+1}\nhandler foreign(event){local roll=random(1,100) while false{set roll=roll+1} set hits=hits+100}\n";
  // import_script includes a source-less neutral Init root for a handlers-only
  // script. It is not an authored Native definition and must not be opted in.
  // Compile only the two actual handlers; the strict reconstruction guard stays.
  let handlers=if refuse_command{handlers.replace("set hits=hits+1}","set hits=hits+1 local invalid_interface=\"candidate0973_missing_screen\" ui.open(invalid_interface,[],false,0)}")}else{handlers.into()};
  let handlers=if native{let graphs=rvn_graph::import_script(&rvn_parser::parse(&handlers).unwrap()).unwrap();let actual=graphs.into_iter().filter(|graph|matches!(&graph.kind,rvn_graph::GraphKind::Handler{name}if name=="allowed"||name=="foreign")).collect::<Vec<_>>();assert_eq!(actual.len(),2,"two actual authored Handler roots");actual.into_iter().map(|mut graph|{graph.enable_native_frame_preparation().unwrap();rvn_graph::prepare_native_frame_graph(&graph).unwrap().source}).collect::<Vec<_>>().join("\n")}else{handlers.into()};
  let source=handlers+r#"
screen panel(method){return component("go","button",{"events":{"click":method}},[])}
screen other(){return []}
screen legacy(method){return component("go","button",{"events":{"click":method}},[])}
init{set hits=0}
label start
ui.open("legacy",["foreign"],false,0)
"Ready"
return
"#;
  // Reuse the same real SourceProject/class admission and existing CPU host.
  // The optional actual owner event calls the class member selected by method.
  let source=if initialized{source.replace(
   "screen panel(method){return component(\"go\",\"button\",{\"events\":{\"click\":method}},[])}",
   "screen panel(method){return {\"host\":{\"events\":{\"initialized\":method}},\"root\":component(\"go\",\"button\",{\"events\":{\"click\":method}},[])}}")}
  else{source};
  let source=if object_query{source.replace("\nreturn\n", "\nset queried=object_class(saved_object)\n\"Resolved\"\nreturn\n")+"\nfunction object_class(value){local roll=random(1,9) local resolved=__rvn_widget_object_class_v1(value) return resolved}\n"}else{source};
  let source=transform(&root,source);
  fs::write(root.join("main.rvn"),&source).unwrap();let project=rvn_graph::SourceProject::open_at(&root.join("main.rvn"),&source,&[]).unwrap();let program=project.resolved_program().unwrap();
  let graph=|name:&str|project.graphs().iter().find(|graph|match &graph.kind{rvn_graph::GraphKind::Screen{name:actual}|rvn_graph::GraphKind::Handler{name:actual}=>actual==name,_=>false}).unwrap();
  let descriptor=|uuid:&str,screen:&str,method:&str|{let id=WidgetGraphRef16::from_u64(graph(screen).graph_id.get());WidgetBlueprintV1{uuid:uuid.into(),source:"main.rvn".into(),screen_graph:id,members:vec![id,WidgetGraphRef16::from_u64(graph(method).graph_id.get())]}.encode().unwrap()};let a=descriptor(A,"panel","allowed");let b=descriptor(B,"other","foreign");let entries=[(A,Some("widgets/a.rvnwidget")),(B,Some("widgets/b.rvnwidget"))];
  let admitted=rvn_graph::prepare_widget_runtime_classes(&root,&program,WidgetIndexInput::Table(&entries),LocatorCase::Insensitive,&[rvn_graph::WidgetRuntimeAsset{locator:"widgets/a.rvnwidget",bytes:a.as_bytes()},rvn_graph::WidgetRuntimeAsset{locator:"widgets/b.rvnwidget",bytes:b.as_bytes()}],&[rvn_graph::WidgetRuntimeSource{source:"main.rvn",project:&project}]).unwrap();
  let classes=AdmittedWidgetClasses::admit(&program,admitted.classes().values().map(|class|CompilerWidgetClass{index_uuid:class.uuid().into(),locator:class.locator().into(),descriptor_bytes:class.descriptor_bytes().to_vec(),members:class.members().iter().map(|member|CompilerWidgetMember{graph:member.graph().get(),kind:match member.kind(){rvn_graph::WidgetDefinitionKind::Screen=>WidgetMemberKind::Screen,rvn_graph::WidgetDefinitionKind::Handler=>WidgetMemberKind::Handler,rvn_graph::WidgetDefinitionKind::Function=>WidgetMemberKind::Function},definition:member.definition().into(),root_index:member.root_index(),origin:member.origin().clone()}).collect()}).collect()).unwrap();Self{root,program,classes}
 }
 fn engine(&self)->Engine<Host>{let mut engine=Engine::from_program_with_widget_classes(self.program.clone(),self.classes.clone(),Host::default(),16).unwrap();engine.step_until_interaction().unwrap();engine}
}
impl Drop for Fixture{fn drop(&mut self){let _=fs::remove_dir_all(&self.root);}}
fn input(engine:&Engine<Host>,name:&str,kind:rvn_ui::programmable::ScreenEventKind)->crate::ui_input::ScopedUiInput{crate::ui_input::ScopedUiInput{source:engine.capture_ui_input_source(name).unwrap(),event:crate::ui::UiInput{screen:name.into(),element:"go".into(),kind,value:None,key:None}}}
#[test]
fn typed_child_click_and_activate_reject_other_class_handlers_before_native_or_legacy_execution(){
 for native in [false,true]{let fixture=Fixture::new(native);let mut engine=fixture.engine();let blocked=engine.create_widget(A,"blocked",vec![Value::Str("foreign".into())]).unwrap();engine.add_widget_to_viewport(&blocked,false,0).unwrap();
  for kind in [rvn_ui::programmable::ScreenEventKind::Click,rvn_ui::programmable::ScreenEventKind::Activate]{let before=serde_json::to_vec(&engine.state).unwrap();let publications=engine.renderer.batches.lock().unwrap().len();let event=input(&engine,"blocked",kind);assert!(engine.interface_event_scoped(event).is_err());assert_eq!(serde_json::to_vec(&engine.state).unwrap(),before,"no vars, RNG, registry or consumer-frame mutation");assert_eq!(engine.renderer.batches.lock().unwrap().len(),publications);}
  let allowed=engine.create_widget(A,"accepted",vec![Value::Str("allowed".into())]).unwrap();engine.add_widget_to_viewport(&allowed,false,0).unwrap();let event=input(&engine,"accepted",rvn_ui::programmable::ScreenEventKind::Click);assert!(engine.interface_event_scoped(event).unwrap());assert_eq!(engine.state.vars["hits"],Value::Int(1));let event=input(&engine,"legacy",rvn_ui::programmable::ScreenEventKind::Activate);assert!(engine.interface_event_scoped(event).unwrap());assert_eq!(engine.state.vars["hits"],Value::Int(101),"legacy global-handler policy is unchanged");
 }
}
#[test]
fn typed_closed_and_media_callbacks_validate_the_actual_retained_class_before_legacy_early_return(){
 for native in [false,true]{let fixture=Fixture::new(native);let mut engine=fixture.engine();let handle=engine.create_widget(A,"source",vec![Value::Str("foreign".into())]).unwrap();engine.add_widget_to_viewport(&handle,false,0).unwrap();let target=crate::ui_residency::visible_interface_target(&engine.state,"source").unwrap();let resident=crate::ui_residency::resident_interface(&engine.state,&target).unwrap();let owner=resident.owner.clone();let projection=crate::ui_residency::UiProjectionBinding{interface:target.clone(),element:"go".into(),order:resident.instance.order,host_role:resident.instance.host_role,residence:resident.residence};let closed=engine.capture_closing_interface(&engine.state,"source").unwrap().unwrap();
  assert!(engine.native_media_frame(&engine.state,"foreign",Some(&projection)).is_err());assert!(engine.native_media_frame(&engine.state,"allowed",Some(&projection)).is_ok());
  engine.destroy_widget(handle).unwrap();let next=engine.state.clone();assert!(engine.native_named_instance_handler_frame(&next,"source","foreign",&owner,Some(&closed)).is_err());assert!(engine.native_named_instance_handler_frame(&next,"source","allowed",&owner,Some(&closed)).is_ok());
  let replacement=engine.create_widget(B,"source",vec![]).unwrap();engine.add_widget_to_viewport(&replacement,false,0).unwrap();assert!(engine.native_named_instance_handler_frame(&engine.state,"source","foreign",&owner,Some(&closed)).is_err(),"replacement alias cannot donate its class membership to an old closed instance");
 }
}
#[test]
fn stopped_input_foreign_handler_cannot_be_frozen_or_hydrated_as_a_class_method(){
 let fixture=Fixture::new(true);let mut engine=fixture.engine();let blocked=engine.create_widget(A,"blocked",vec![Value::Str("foreign".into())]).unwrap();engine.add_widget_to_viewport(&blocked,false,0).unwrap();let foreign=input(&engine,"blocked",rvn_ui::programmable::ScreenEventKind::Click);let before=serde_json::to_vec(&engine.state).unwrap();assert!(engine.portable_host_input(&foreign).is_err());assert_eq!(serde_json::to_vec(&engine.state).unwrap(),before);
 let accepted=engine.create_widget(A,"accepted",vec![Value::Str("allowed".into())]).unwrap();engine.add_widget_to_viewport(&accepted,false,0).unwrap();let event=input(&engine,"accepted",rvn_ui::programmable::ScreenEventKind::Click);let portable=engine.portable_host_input(&event).unwrap();assert!(engine.restore_host_input(&portable).is_ok());let before_forgery=serde_json::to_vec(&engine.state).unwrap();let publications=engine.renderer.batches.lock().unwrap().len();let mut forged=portable.clone();let blocked=input(&engine,"blocked",rvn_ui::programmable::ScreenEventKind::Click);let target=crate::ui_residency::visible_interface_target(&engine.state,"blocked").unwrap();forged.target=target;forged.screen="blocked".into();forged.handler="foreign".into();let definition=engine.consumer_program.host_definition(&crate::consumer_schedule::HostDefinitionKind::Handler,"foreign").unwrap();forged.handler_ordinal=definition.source_ordinal;assert!(engine.restore_host_input(&forged).is_err());assert!(engine.portable_host_input(&blocked).is_err());assert_eq!(serde_json::to_vec(&engine.state).unwrap(),before_forgery);assert_eq!(engine.renderer.batches.lock().unwrap().len(),publications);
}
#[test]
fn actual_save15_refuses_a_globally_valid_foreign_handler_frame_on_a_typed_instance(){
 let fixture=Fixture::new(true);let mut engine=fixture.engine();let a=engine.create_widget(A,"a",vec![Value::Str("allowed".into())]).unwrap();engine.add_widget_to_viewport(&a,false,0).unwrap();let b=engine.create_widget(B,"b",vec![]).unwrap();engine.add_widget_to_viewport(&b,false,0).unwrap();let target_b=crate::ui_residency::visible_interface_target(&engine.state,"b").unwrap();let owner_b=crate::ui_residency::resident_interface(&engine.state,&target_b).unwrap().owner.clone();let frame=engine.native_named_instance_handler_frame(&engine.state,"b","foreign",&owner_b,None).unwrap().unwrap();let target_a=crate::ui_residency::visible_interface_target(&engine.state,"a").unwrap();let mut frame=frame;frame.owner.target=target_a;let key=frame.owner.storage_key();let mut forged=engine.state.clone();crate::ui_consumer::instance_mut(&mut forged,&frame.owner.target).unwrap().consumer_frames.insert(key,frame);assert!(engine.widget_classes.validate_state(&forged).is_err());
 let mut save=engine.capture_save15(&fixture.program,None,1,"typed".into()).unwrap();save.story=crate::save::consumer_checkpoint::PortableStory::capture(&forged,&fixture.program,&engine.script,&engine.consumer_program).unwrap();let save=crate::save::consumer_checkpoint::Save15::decode(&save.encode().unwrap()).unwrap();let before=serde_json::to_vec(&engine.state).unwrap();let publications=engine.renderer.batches.lock().unwrap().len();assert!(engine.restore_save15(&fixture.program,save).is_err());assert_eq!(serde_json::to_vec(&engine.state).unwrap(),before);assert_eq!(engine.renderer.batches.lock().unwrap().len(),publications);
}

#[test]
fn legacy_close_without_registry_retains_original_callback_instance_while_its_handler_reopens_the_alias(){
 let source=r#"
handler close_now(event){ui.close("legacy")}
handler closed(event){set closed_count=closed_count+1 ui.open("legacy",[1],false,0)}
screen legacy(version){return component("root","panel",{"events":{"close":"closed"}},[component("go","button",{"events":{"click":"close_now"}},[])])}
init{set closed_count=0}
label start
ui.open("legacy",[0],false,0)
"Ready"
return
"#;
 let mut engine=Engine::new(rvn_parser::parse(source).unwrap(),Host::default(),16).unwrap();engine.step_until_interaction().unwrap();assert!(engine.state.ui.instance_registry.is_none());let old=engine.capture_ui_input_source("legacy").unwrap();let old_order=old.order;let mut expected_random=engine.state.random;let _=expected_random.next_u64();let event=input(&engine,"legacy",rvn_ui::programmable::ScreenEventKind::Click);assert!(engine.interface_event_scoped(event).unwrap());assert_eq!(engine.state.vars["closed_count"],Value::Int(1));assert!(engine.state.ui.instance_registry.is_none(),"a retained Legacy close receipt never creates a registry");let instance=engine.state.ui.screens.iter().find(|instance|instance.name=="legacy").unwrap();assert_eq!(instance.arguments,vec![Value::Int(1)]);assert!(instance.order>old_order);assert_eq!(engine.state.random,expected_random,"only the actual reopened instance consumes its one seed");assert!(!engine.ui_input_source_current(&old));assert!(instance.consumer_frames.is_empty(),"Legacy close did not invent a Native frame");
}

// Explicit CPU test renderer adapter. It claims no physical/native wrapper.
#[derive(Debug)]
struct CpuViewportPlan{owner:crate::widget_backend_0956::WidgetBackendOwner,operation:crate::widget_backend_0956::WidgetViewportOperation}
impl crate::widget_backend_0956::PreparedWidgetViewport for CpuViewportPlan{
 fn owner(&self)->&crate::widget_backend_0956::WidgetBackendOwner{&self.owner}
 fn operation(&self)->crate::widget_backend_0956::WidgetViewportOperation{self.operation}
 fn constructs_wrapper(&self)->bool{false}fn releases_wrapper(&self)->bool{false}
 fn as_any(&self)->&dyn std::any::Any{self}
}

// Explicit logical-only test adapter. No native object/pixel/lifecycle claim.
#[derive(Debug)]
struct CpuViewportBatch{owners:Vec<crate::widget_backend_0956::WidgetBackendOwner>}
impl crate::widget_backend_0956::PreparedWidgetViewportBatch for CpuViewportBatch{
 fn desired_owners(&self)->&[crate::widget_backend_0956::WidgetBackendOwner]{&self.owners}
 fn created(&self)->&[crate::widget_backend_0956::WidgetBackendObjectIdentity]{&[]}
 fn retired(&self)->&[crate::widget_backend_0956::WidgetBackendObjectIdentity]{&[]}
 fn as_any(&self)->&dyn std::any::Any{self}
}
struct CpuViewportReservation;
impl crate::widget_backend_0956::WidgetViewportCommitReservation for CpuViewportReservation{fn commit(self:Box<Self>,publish:&mut dyn FnMut()){publish();}}

#[path="engine_widget_reference_tests_0974.rs"]
mod reference_values_0974;


#[test]
fn candidate0973_abandoned_real_creation_keeps_initialized_rng_registry_and_publication_private(){
 for native in [false,true]{
  let fixture=Fixture::with_initialized(native,true);let mut engine=fixture.engine();
  let before=serde_json::to_vec(&engine.state).unwrap();let host_random=engine.host_random.clone();let publications=engine.renderer.batches.lock().unwrap().len();let source=engine.debug_state_source().unwrap();
  let abandoned:Result<(),RuntimeError>=engine.with_host_random_transaction(|engine|{
   let mut next=engine.state.clone();let target=engine.prepare_widget_creation_in(&mut next,A,"abandoned",vec![Value::Str("allowed".into())])?;
   assert_eq!(next.vars["hits"],Value::Int(1),"Initialized executed once inside the actual candidate VM");
   let duplicate_before=serde_json::to_vec(&next).unwrap();assert!(engine.prepare_widget_creation_in(&mut next,A,"abandoned",vec![Value::Str("allowed".into())]).is_err());assert_eq!(serde_json::to_vec(&next).unwrap(),duplicate_before,"duplicate admission reads the actual candidate, not only self.state");
   assert_eq!(crate::ui_residency::visible_interface_target(&next,"abandoned").unwrap(),target);
   assert!(next.ui.screens.iter().any(|instance|instance.name=="abandoned"&&instance.widget_class.is_some()&&!instance.viewport_attached()));
   assert_ne!(serde_json::to_vec(&next).unwrap(),before);
   assert_eq!(serde_json::to_vec(&engine.state).unwrap(),before);assert!(engine.widget_instance_handle("abandoned").is_none());assert_eq!(engine.renderer.batches.lock().unwrap().len(),publications);
   Err(engine.scene_resource_error("Deliberate enclosing candidate abandonment".into()))
  });
  assert!(abandoned.is_err());assert_eq!(serde_json::to_vec(&engine.state).unwrap(),before);assert_eq!(engine.host_random,host_random);assert!(engine.widget_instance_handle("abandoned").is_none());assert!(engine.accepts_debug_state_source(&source));assert_eq!(engine.renderer.batches.lock().unwrap().len(),publications);
 }
}
#[test]
fn candidate0973_public_factory_uses_one_commit_and_distinct_actual_instances_with_initialized_once_each(){
 for native in [false,true]{
  let fixture=Fixture::with_initialized(native,true);let mut engine=fixture.engine();let publications=engine.renderer.batches.lock().unwrap().len();
  let first=engine.create_widget(A,"first",vec![Value::Str("allowed".into())]).unwrap();assert_eq!(engine.state.vars["hits"],Value::Int(1));
  let second=engine.create_widget(A,"second",vec![Value::Str("allowed".into())]).unwrap();assert_eq!(engine.state.vars["hits"],Value::Int(2));assert_ne!(first,second);
  let a=engine.widget_backend_owner(&first).unwrap();let b=engine.widget_backend_owner(&second).unwrap();assert_eq!(a.class(),b.class());assert_ne!(a.target(),b.target());assert_ne!(a.order(),b.order());assert_eq!(engine.widget_instance_handle("first").unwrap(),first);assert_eq!(engine.widget_instance_handle("second").unwrap(),second);
  assert!(engine.state.ui.screens.iter().filter(|instance|instance.widget_class.is_some()).all(|instance|!instance.viewport_attached()));assert_eq!(engine.renderer.batches.lock().unwrap().len(),publications+2,"one actual publication per successful public creation");
 }
}
#[test]
fn candidate0973_initialized_member_refusal_rolls_back_public_factory_without_a_handle_or_allocator_draw(){
 for native in [false,true]{
  let fixture=Fixture::with_initialized(native,true);let mut engine=fixture.engine();let before=serde_json::to_vec(&engine.state).unwrap();let host_random=engine.host_random.clone();let publications=engine.renderer.batches.lock().unwrap().len();let next_order=engine.state.ui.next_order;
  // This globally valid handler belongs to class B, not class A. The real
  // Initialized membership refusal occurs after staging the seed/target.
  assert!(engine.create_widget(A,"retry",vec![Value::Str("foreign".into())]).is_err());assert_eq!(serde_json::to_vec(&engine.state).unwrap(),before);assert_eq!(engine.host_random,host_random);assert!(engine.widget_instance_handle("retry").is_none());assert_eq!(engine.renderer.batches.lock().unwrap().len(),publications);
  let valid=engine.create_widget(A,"retry",vec![Value::Str("allowed".into())]).unwrap();assert_eq!(engine.widget_backend_owner(&valid).unwrap().order(),next_order);assert_eq!(engine.state.vars["hits"],Value::Int(1));
 }
}


#[test]
fn candidate0973_initialized_valid_vm_mutation_and_random_then_failed_command_never_publish_or_leak_handle(){
 for native in [false,true]{
  let fixture=Fixture::with_initialized_command_refusal(native,true,true);let mut engine=fixture.engine();
  let before=serde_json::to_vec(&engine.state).unwrap();let before_random=engine.state.random;let host_random=engine.host_random.clone();let publications=engine.renderer.batches.lock().unwrap().len();let source=engine.debug_state_source().unwrap();
  // Read the disposable candidate solely to prove the reached failure order:
  // admitted Initialized member -> real random and hits write -> bad command.
  let mut rejected=engine.state.clone();
  let rejected_result=engine.with_host_random_transaction(|engine|engine.prepare_widget_creation_in(&mut rejected,A,"failed_initialized",vec![Value::Str("allowed".into())]));
  let error=rejected_result.unwrap_err();assert!(error.to_string().contains("candidate0973_missing_screen"),"must reach the actual absent-screen command, not a membership/argument refusal: {error}");
  assert_eq!(rejected.vars["hits"],Value::Int(1));let mut creation_seed_only=before_random;creation_seed_only.next_u64();assert_ne!(rejected.random,creation_seed_only,"the actual admitted Initialized VM consumed randomness after the creation seed");assert!(rejected.ui.screens.iter().any(|instance|instance.name=="failed_initialized"&&instance.widget_class.is_some()));drop(rejected);
  assert_eq!(serde_json::to_vec(&engine.state).unwrap(),before);assert_eq!(engine.host_random,host_random);assert_eq!(engine.renderer.batches.lock().unwrap().len(),publications);
  // The production entry must discard that same partial candidate and may
  // never construct an acquired handle or publish its Initialized mutations.
  let public=engine.create_widget(A,"failed_initialized",vec![Value::Str("allowed".into())]);assert!(public.is_err());assert!(public.unwrap_err().to_string().contains("candidate0973_missing_screen"));assert_eq!(serde_json::to_vec(&engine.state).unwrap(),before);assert_eq!(engine.state.random,before_random);assert_eq!(engine.host_random,host_random);assert!(engine.widget_instance_handle("failed_initialized").is_none());assert_eq!(engine.renderer.batches.lock().unwrap().len(),publications);assert!(engine.accepts_debug_state_source(&source));
 }
}

#[path="engine_widget_retained_weight_save15_tests1031.rs"]
mod retained_weight_save15_1031;
