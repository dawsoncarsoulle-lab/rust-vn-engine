use super::*;
use rvn_parser::{widget_asset::{WidgetBlueprintV1,WidgetGraphRef16,WidgetIndexInput,LocatorCase},Hotspot,Position,Transition,Value};
use rvn_core::{types::SpriteState,renderer::SceneRenderBatch};
use std::{fs,path::PathBuf,sync::{Arc,Mutex,atomic::{AtomicBool,AtomicU64,Ordering}}};
const UUID:&str="17411a3e431e4c9388b78a2b9825b130";
const SOURCE:&str=r#"handler initialize(event){set initialized=initialized+1}
screen panel(value){return {"host":{"events":{"initialized":"initialize"}},"root":[]}}
screen legacy_empty(){return []}
init{set initialized=0}
label start
"Ready"
return
"#;
#[derive(Default,Clone)]
struct Host{batches:Arc<Mutex<Vec<SceneRenderBatch>>>,reject:Arc<AtomicBool>}
impl Renderer for Host{
 fn prepare_widget_viewport_batch(&self,owners:&[rvn_core::widget_backend_0956::WidgetBackendOwner],transition:Option<&dyn rvn_core::widget_backend_0956::PreparedWidgetViewport>)->Result<Arc<dyn rvn_core::widget_backend_0956::PreparedWidgetViewportBatch>,String>{if let Some(transition)=transition{self.validate_widget_viewport(transition)?;}Ok(Arc::new(CpuViewportBatch{owners:owners.to_vec()}))}
 fn validate_widget_viewport_batch(&self,batch:&dyn rvn_core::widget_backend_0956::PreparedWidgetViewportBatch)->Result<(),String>{if batch.as_any().is::<CpuViewportBatch>(){Ok(())}else{Err("Foreign CPU logical test batch".into())}}
 fn reserve_widget_viewport_batch(&self,batch:Arc<dyn rvn_core::widget_backend_0956::PreparedWidgetViewportBatch>)->Result<Box<dyn rvn_core::widget_backend_0956::WidgetViewportCommitReservation>,String>{self.validate_widget_viewport_batch(batch.as_ref())?;Ok(Box::new(CpuViewportReservation))}
 fn prepare_widget_viewport(&self,owner:&rvn_core::widget_backend_0956::WidgetBackendOwner,operation:rvn_core::widget_backend_0956::WidgetViewportOperation)->Result<Arc<dyn rvn_core::widget_backend_0956::PreparedWidgetViewport>,String>{Ok(Arc::new(CpuViewportPlan{owner:owner.clone(),operation}))}
 fn validate_widget_viewport(&self,plan:&dyn rvn_core::widget_backend_0956::PreparedWidgetViewport)->Result<(),String>{if plan.as_any().is::<CpuViewportPlan>(){Ok(())}else{Err("Foreign CPU test backend plan".into())}}

 fn supports_programmable_ui(&self)->bool{true}
 fn validate_scene_batch(&self,_:&SceneRenderBatch)->Result<(),String>{if self.reject.load(Ordering::SeqCst){Err("Widget renderer refusal".into())}else{Ok(())}}
 fn publish_scene_batch(&mut self,batch:SceneRenderBatch){self.batches.lock().unwrap().push(batch);}
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
}
struct Fixture{root:PathBuf,admission:rvn_graph::PreparedWidgetClasses}
impl Fixture{
 fn new()->Self{
  static NEXT:AtomicU64=AtomicU64::new(1);
  let root=std::env::temp_dir().join(format!("rvn-widget-runtime-0950-{}-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos(),NEXT.fetch_add(1,Ordering::Relaxed)));
  fs::create_dir_all(&root).unwrap();let root=fs::canonicalize(root).unwrap();fs::write(root.join("main.rvn"),SOURCE).unwrap();
  let project=rvn_graph::SourceProject::open_at(&root.join("main.rvn"),SOURCE,&[]).unwrap();
  let screen=project.graphs().iter().find(|graph|matches!(&graph.kind,rvn_graph::GraphKind::Screen{name}if name=="panel")).unwrap();
  let id=WidgetGraphRef16::from_u64(screen.graph_id.get());
  let member=project.graphs().iter().find(|graph|matches!(&graph.kind,rvn_graph::GraphKind::Handler{name}if name=="initialize")).unwrap();
  let descriptor=WidgetBlueprintV1{uuid:UUID.into(),source:"main.rvn".into(),screen_graph:id,members:vec![id,WidgetGraphRef16::from_u64(member.graph_id.get())]}.encode().unwrap();
  let entries=[(UUID,Some("widgets/panel.rvnwidget"))];
  let admission=rvn_graph::prepare_widget_runtime_classes(&root,&project.resolved_program().unwrap(),WidgetIndexInput::Table(&entries),LocatorCase::Insensitive,&[rvn_graph::WidgetRuntimeAsset{locator:"widgets/panel.rvnwidget",bytes:descriptor.as_bytes()}],&[rvn_graph::WidgetRuntimeSource{source:"main.rvn",project:&project}]).unwrap();
  Self{root,admission}
 }
 fn engine(&self,host:Host)->Engine<Host>{let mut engine=create_engine(&self.admission,host,16).unwrap();engine.step_until_interaction().unwrap();engine}
}
#[test]
fn core_compiler_admitted_native_instances_keep_authored_definition_and_independent_target_frames_through_save15(){
 // Core compiler-input contract only. The three other tests use the real
 // opaque Graph admission bridge. Historical Graph Return v1 is deliberately
 // not relabelled v2 or presented as a retained native frame.
 let fixture=Fixture::new();let source="screen panel(value){flow.consume_v2(\"screen:panel\",\"gather\"){capture \"roll\"=value return []}}\nlabel start\n\"Ready\"\nreturn\n";
 let mut program=rvn_parser::parse_program(source).unwrap();let path=Some(fixture.root.join("main.rvn"));program.sources[0].path=path.clone();for origin in &mut program.origins{origin.source=path.clone();}
 let graph=fixture.admission.class(UUID).unwrap().screen().graph().get();let id=WidgetGraphRef16::from_u64(graph);let descriptor=WidgetBlueprintV1{uuid:UUID.into(),source:"main.rvn".into(),screen_graph:id,members:vec![id]}.encode().unwrap();
 let core_input=||CompilerWidgetClass{index_uuid:UUID.into(),locator:"widgets/panel.rvnwidget".into(),descriptor_bytes:descriptor.as_bytes().to_vec(),members:vec![CompilerWidgetMember{graph,kind:WidgetMemberKind::Screen,definition:"panel".into(),root_index:0,origin:program.origins[0].clone()}]};
 let mut forged=core_input();forged.members[0].origin.span.range.end-=1;assert!(AdmittedWidgetClasses::admit(&program,vec![forged]).is_err(),"Compiler DTO decoding is not source authority");
 let engine=||{let classes=AdmittedWidgetClasses::admit(&program,vec![core_input()]).unwrap();let mut engine=Engine::from_program_with_widget_classes(program.clone(),classes,Host::default(),16).unwrap();engine.step_until_interaction().unwrap();engine};let mut engine=engine();
 let a=engine.create_widget(UUID,"native_a",vec![Value::Int(1)]).unwrap();let b=engine.create_widget(UUID,"native_b",vec![Value::Int(2)]).unwrap();
 let first=&engine.state.ui.screens[0];let second=&engine.state.ui.screens[1];let p=first.consumer_projection.as_ref().expect("Actual Native class gather projection");let q=second.consumer_projection.as_ref().expect("Second actual Native class gather projection");assert_eq!(p.owner.definition,"panel");assert_eq!(q.owner.definition,"panel");assert_eq!(p.owner.target.alias,"native_a");assert_eq!(q.owner.target.alias,"native_b");assert_ne!(p.owner.target,q.owner.target);assert!(p.root.is_none()&&q.root.is_none());let frames=serde_json::to_value(&engine.state.ui).unwrap();
 let wire=engine.capture_save15(&program,None,1,"Native classes".into()).unwrap().encode().unwrap();let classes=AdmittedWidgetClasses::admit(&program,vec![core_input()]).unwrap();let mut restored=Engine::from_program_with_widget_classes(program.clone(),classes,Host::default(),16).unwrap();restored.step_until_interaction().unwrap();restored.restore_save15(&program,rvn_core::save::consumer_checkpoint::Save15::decode(&wire).unwrap()).unwrap();assert_eq!(serde_json::to_value(&restored.state.ui).unwrap(),frames);
 assert!(restored.add_widget_to_viewport(&a,false,0).is_err());assert!(restored.add_widget_to_viewport(&b,false,0).is_err());let handle=restored.widget_instance_handle("native_a").unwrap();restored.add_widget_to_viewport(&handle,true,0).unwrap();let projection=restored.state.ui.screens[0].consumer_projection.as_ref().unwrap();assert_eq!(projection.owner.definition,"panel");assert_eq!(projection.owner.target.alias,"native_a");assert!(restored.interface_views().unwrap().is_empty());assert_eq!(restored.interface_hosts().unwrap().len(),1);
}
impl Drop for Fixture{fn drop(&mut self){let _=fs::remove_dir_all(&self.root);}}
#[test]
fn actual_graph_class_bridge_creates_two_distinct_detached_instances_and_viewport_detach_preserves_identity(){
 let fixture=Fixture::new();let mut engine=fixture.engine(Host::default());
 let classes=core_classes(&fixture.admission).unwrap();let changed=rvn_parser::parse_program(&SOURCE.replace("Ready","Changed")).unwrap();let refused=Host::default();assert!(Engine::from_program_with_widget_classes(changed,classes,refused.clone(),16).is_err());assert!(refused.batches.lock().unwrap().is_empty(),"Foreign class/program is rejected before initialization publication");
 let classes=core_classes(&fixture.admission).unwrap();let mut relocated=fixture.admission.program().clone();let staged=fixture.root.join("staging/main.rvn");for source in &mut relocated.sources{source.path=Some(staged.clone());}for origin in &mut relocated.origins{origin.source=Some(staged.clone());}assert!(classes.matches_program(&relocated).unwrap(),"absolute staging relocation preserves authored relative source");let renamed=fixture.root.join("other.rvn");for source in &mut relocated.sources{source.path=Some(renamed.clone());}for origin in &mut relocated.origins{origin.source=Some(renamed.clone());}assert!(!classes.matches_program(&relocated).unwrap(),"identical bytes at another relative source cannot adopt the class");
 let first=engine.create_widget(UUID,"first",vec![Value::Int(1)]).unwrap();let second=engine.create_widget(UUID,"second",vec![Value::Int(2)]).unwrap();assert_ne!(first,second);assert_eq!(engine.state.vars["initialized"],Value::Int(2),"actual per-instance Initialized runs once before viewport attachment");
 assert!(engine.interface_views().unwrap().is_empty());assert!(engine.interface_hosts().unwrap().is_empty());assert!(engine.capture_ui_input_source("first").is_none());
 assert_eq!(engine.state.ui.screens.len(),2);assert_eq!(engine.state.ui.screens[0].definition(),"panel");assert_eq!(engine.state.ui.screens[0].arguments,vec![Value::Int(1)]);assert_eq!(engine.state.ui.screens[1].arguments,vec![Value::Int(2)]);
 let order=engine.state.ui.screens[0].order;let rng=engine.state.ui.screens[0].random;
 engine.add_widget_to_viewport(&first,true,10).unwrap();engine.add_widget_to_viewport(&second,false,20).unwrap();
 assert!(engine.interface_views().unwrap().is_empty(),"No fabricated Component for an empty UserWidget");assert_eq!(engine.interface_hosts().unwrap().len(),2);assert!(engine.capture_ui_input_source("first").is_some());
 engine.remove_widget_from_viewport(&first).unwrap();assert_eq!(engine.interface_hosts().unwrap().len(),1);assert_eq!(engine.state.ui.screens[0].order,order);assert_eq!(engine.state.ui.screens[0].random,rng);assert_eq!(engine.widget_instance_handle("first").unwrap(),first);
 engine.add_widget_to_viewport(&first,true,10).unwrap();assert_eq!(engine.state.ui.screens[0].order,order);assert_eq!(engine.state.vars["initialized"],Value::Int(2),"reattach cannot replay Initialized");
 engine.destroy_widget(first.clone()).unwrap();assert!(engine.add_widget_to_viewport(&first,false,0).is_err());let replacement=engine.create_widget(UUID,"first",vec![Value::Int(3)]).unwrap();assert_ne!(replacement,first);
 assert!(engine.create_widget("missing","fake",vec![]).is_err());assert!(engine.widget_classes().class("legacy_empty").is_none());
}
#[test]
fn actual_class_instance_create_attach_detach_destroy_refusals_preserve_state_registry_rng_and_publications(){
 let fixture=Fixture::new();let class=fixture.admission.class(UUID).unwrap();let mut descriptor=rvn_parser::widget_asset::WidgetBlueprintV1::decode(class.descriptor_bytes()).unwrap();descriptor.members=vec![descriptor.screen_graph];
 let inputs=vec![CompilerWidgetClass{index_uuid:UUID.into(),locator:class.locator().into(),descriptor_bytes:descriptor.encode().unwrap().into_bytes(),members:class.members().iter().filter(|member|member.kind()==&rvn_graph::WidgetDefinitionKind::Screen).map(|member|CompilerWidgetMember{graph:member.graph().get(),kind:WidgetMemberKind::Screen,definition:member.definition().into(),root_index:member.root_index(),origin:member.origin().clone()}).collect()}];
 let classes=AdmittedWidgetClasses::admit(fixture.admission.program(),inputs).unwrap();let refused=Host::default();let mut foreign=Engine::from_program_with_widget_classes(fixture.admission.program().clone(),classes,refused.clone(),16).unwrap();foreign.step_until_interaction().unwrap();let before=serde_json::to_vec(&foreign.state).unwrap();let publications=refused.batches.lock().unwrap().len();assert!(foreign.create_widget(UUID,"foreign_callback",vec![Value::Int(1)]).is_err());assert_eq!(serde_json::to_vec(&foreign.state).unwrap(),before);assert_eq!(refused.batches.lock().unwrap().len(),publications);

 let fixture=Fixture::new();let host=Host::default();let mut engine=fixture.engine(host.clone());
 let first=engine.create_widget(UUID,"first",vec![Value::Int(1)]).unwrap();let before=serde_json::to_value(&engine.state).unwrap();let published=host.batches.lock().unwrap().len();host.reject.store(true,Ordering::SeqCst);
 assert!(engine.create_widget(UUID,"second",vec![Value::Int(2)]).is_err());assert_eq!(serde_json::to_value(&engine.state).unwrap(),before);
 assert!(engine.add_widget_to_viewport(&first,false,0).is_err());assert_eq!(serde_json::to_value(&engine.state).unwrap(),before);
 assert!(engine.remove_widget_from_viewport(&first).is_ok(),"Detached removal is an idempotent no-op");assert_eq!(serde_json::to_value(&engine.state).unwrap(),before);
 assert!(engine.destroy_widget(first.clone()).is_err());assert_eq!(serde_json::to_value(&engine.state).unwrap(),before);assert_eq!(host.batches.lock().unwrap().len(),published);
 host.reject.store(false,Ordering::SeqCst);engine.add_widget_to_viewport(&first,false,0).unwrap();let attached=serde_json::to_value(&engine.state).unwrap();host.reject.store(true,Ordering::SeqCst);assert!(engine.remove_widget_from_viewport(&first).is_err());assert!(engine.add_widget_to_viewport(&first,false,0).is_err());assert_eq!(serde_json::to_value(&engine.state).unwrap(),attached);
 let foreign=fixture.engine(Host::default());let mut foreign=foreign;assert!(foreign.add_widget_to_viewport(&first,false,0).is_err());
}
#[test]
fn save15_restores_actual_class_instances_without_reviving_old_handles_and_refuses_forged_binding_atomically(){
 let fixture=Fixture::new();let mut engine=fixture.engine(Host::default());let first=engine.create_widget(UUID,"first",vec![Value::Int(1)]).unwrap();let second=engine.create_widget(UUID,"second",vec![Value::Int(2)]).unwrap();engine.add_widget_to_viewport(&first,true,1).unwrap();
 let save=engine.capture_save15(fixture.admission.program(),None,1,"Widget".into()).unwrap();let wire=save.encode().unwrap();assert!(wire.contains("widget_classes"));// Restore retains logical attachment while a fresh real wrapper awaits materialization.
 // The CPU logical renderer cannot manufacture that wrapper or claim readiness.
 let mut expected_ui=engine.state.ui.clone();assert_eq!(expected_ui.screens.len(),2);
 let first_binding=expected_ui.screens[0].widget_class.as_mut().unwrap();assert!(first_binding.viewport_attached&&!first_binding.pending_materialization);first_binding.pending_materialization=true;
 let second_binding=expected_ui.screens[1].widget_class.as_ref().unwrap();assert!(!second_binding.viewport_attached&&!second_binding.pending_materialization);
 let state=serde_json::to_value(&expected_ui).unwrap();
 let mut restored=fixture.engine(Host::default());restored.restore_save15(fixture.admission.program(),rvn_core::save::consumer_checkpoint::Save15::decode(&wire).unwrap()).unwrap();assert_eq!(serde_json::to_value(&restored.state.ui).unwrap(),state);assert!(restored.has_pending_widget_materialization());assert!(restored.capture_ui_input_source("first").is_none());assert!(restored.capture_ui_input_source("second").is_none());assert_eq!(restored.state.vars["initialized"],Value::Int(2),"Save15 does not replay instance creation/Initialized");assert!(restored.add_widget_to_viewport(&first,false,0).is_err());assert!(restored.add_widget_to_viewport(&second,false,0).is_err());let fresh=restored.widget_instance_handle("first").unwrap();restored.remove_widget_from_viewport(&fresh).unwrap();
 let before=serde_json::to_value(&restored.state).unwrap();let mut forged:serde_json::Value=serde_json::from_str(&wire).unwrap();let instances=forged["story"]["base"]["ui"]["screens"].as_array_mut().unwrap();instances[1]["widget_class"]["class"]["uuid"]="797679920f7142be99d9d6fe5553ca34".into();let forged=rvn_core::save::consumer_checkpoint::Save15::decode(&serde_json::to_string(&forged).unwrap()).unwrap();assert!(restored.restore_save15(fixture.admission.program(),forged).is_err());assert_eq!(serde_json::to_value(&restored.state).unwrap(),before);
 // Removing the catalog receipt never converts typed instances into legacy.
 let mut missing:serde_json::Value=serde_json::from_str(&wire).unwrap();missing.as_object_mut().unwrap().remove("widget_classes");let missing=rvn_core::save::consumer_checkpoint::Save15::decode(&serde_json::to_string(&missing).unwrap()).unwrap();assert!(restored.restore_save15(fixture.admission.program(),missing).is_err());assert_eq!(serde_json::to_value(&restored.state).unwrap(),before);
 // The exact same source containing [] is still legacy without descriptor
 // admission. Old Save15 omits the new optional field and remains restorable.
 // A debugger SourceMap alone does not admit a frozen Save15 source owner.
 let mut unbound=Engine::from_program_with_debugger(fixture.admission.program().clone(),Host::default(),16).unwrap();unbound.step_until_interaction().unwrap();let error=unbound.capture_save15(fixture.admission.program(),None,1,"Unbound".into()).err().expect("ordinary constructor has no Save15 source binding");assert!(error.to_string().contains("current frozen Engine source binding"));
 // Use the actual source-owned bootstrap, with no Widget class catalogue.
 let mut startup=Engine::prepare_debugger_startup(fixture.admission.program().clone(),Host::default(),16,Default::default(),Default::default(),None).unwrap();
 for _ in 0..32{if startup.is_complete(){break;}assert!(!startup.waiting_for_video());startup.step(rvn_core::editor_debugger::StepKind::Continue).unwrap();}
 assert!(startup.is_complete(),"the real single-statement Init must finish within its bounded drain");let mut legacy=startup.into_engine().unwrap();assert!(legacy.widget_classes().is_empty());assert_eq!(legacy.state.vars["initialized"],Value::Int(0));legacy.step_until_interaction().unwrap();let old=legacy.capture_save15(fixture.admission.program(),None,1,"Legacy".into()).unwrap().encode().unwrap();assert!(!serde_json::from_str::<serde_json::Value>(&old).unwrap().as_object().unwrap().contains_key("widget_classes"));legacy.restore_save15(fixture.admission.program(),rvn_core::save::consumer_checkpoint::Save15::decode(&old).unwrap()).unwrap();assert!(legacy.widget_classes().is_empty());
}

// Explicit CPU test renderer adapter. It claims no physical/native wrapper.
#[derive(Debug)]
struct CpuViewportPlan{owner:rvn_core::widget_backend_0956::WidgetBackendOwner,operation:rvn_core::widget_backend_0956::WidgetViewportOperation}
impl rvn_core::widget_backend_0956::PreparedWidgetViewport for CpuViewportPlan{
 fn owner(&self)->&rvn_core::widget_backend_0956::WidgetBackendOwner{&self.owner}
 fn operation(&self)->rvn_core::widget_backend_0956::WidgetViewportOperation{self.operation}
 fn constructs_wrapper(&self)->bool{false}fn releases_wrapper(&self)->bool{false}
 fn as_any(&self)->&dyn std::any::Any{self}
}

// Explicit logical-only test adapter. No native object/pixel/lifecycle claim.
#[derive(Debug)]
struct CpuViewportBatch{owners:Vec<rvn_core::widget_backend_0956::WidgetBackendOwner>}
impl rvn_core::widget_backend_0956::PreparedWidgetViewportBatch for CpuViewportBatch{
 fn desired_owners(&self)->&[rvn_core::widget_backend_0956::WidgetBackendOwner]{&self.owners}
 fn created(&self)->&[rvn_core::widget_backend_0956::WidgetBackendObjectIdentity]{&[]}
 fn retired(&self)->&[rvn_core::widget_backend_0956::WidgetBackendObjectIdentity]{&[]}
 fn as_any(&self)->&dyn std::any::Any{self}
}
struct CpuViewportReservation;
impl rvn_core::widget_backend_0956::WidgetViewportCommitReservation for CpuViewportReservation{fn commit(self:Box<Self>,publish:&mut dyn FnMut()){publish();}}
