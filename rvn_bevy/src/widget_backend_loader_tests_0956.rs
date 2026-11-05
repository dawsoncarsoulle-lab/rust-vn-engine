//! Actual loader, VM and real Bevy backend object tests. CPU only, no pixels.
use super::*;
use rvn_core::{Renderer,widget_backend_0956::WidgetViewportOperation};
const LIFECYCLE:&str=r#"
handler initialized(event){set initialized_count=initialized_count+1}
handler pre(event){set pre_count=pre_count+1}
handler construct(event){set constructed_count=constructed_count+1 set saw_pre=pre_count if refuse{local invalid_interface="unknown" ui.open(invalid_interface,[],false,0)}}
handler destruct(event){set destructed_count=destructed_count+1}
function child(){if rooted{return component("real","text",{"text":"actual"},[])} return []}
screen panel(value){return {"host":{"events":{"initialized":"initialized","pre_construct":"pre","construct":"construct","destruct":"destruct"}},"root":child()}}
init{set initialized_count=0 set pre_count=0 set constructed_count=0 set destructed_count=0 set saw_pre=0 set rooted=false set refuse=false}
label start
"Ready"
set rooted=true
"Rooted"
set rooted=false
"Empty"
set refuse=true
"Refuse"
return
"#;
fn project()->Project{project_with_source(LIFECYCLE)}
fn project_with_source(source:&str)->Project{
 let project=Project::new(false);fs::remove_file(project.root.join(".rvn-authoring.json")).unwrap();fs::write(project.root.join("main.rvn"),source).unwrap();let graphs=rvn_graph::import_script(&rvn_parser::parse(source).unwrap()).unwrap();let workspace=rvn_graph::SourceWorkspace::link(&project.root,&project.root.join("main.rvn"),&graphs).unwrap();let screen=workspace.project().graphs().iter().find(|graph|matches!(&graph.kind,rvn_graph::GraphKind::Screen{name}if name=="panel")).unwrap();let id=WidgetGraphRef16::from_u64(screen.graph_id.get());let members=workspace.project().graphs().iter().filter(|graph|matches!(&graph.kind,rvn_graph::GraphKind::Screen{name}if name=="panel")||matches!(&graph.kind,rvn_graph::GraphKind::Handler{..}|rvn_graph::GraphKind::Function{..})).map(|graph|WidgetGraphRef16::from_u64(graph.graph_id.get())).collect();fs::write(project.root.join("assets/panel.rvnwidget"),WidgetBlueprintV1{uuid:UUID.into(),source:"main.rvn".into(),screen_graph:id,members}.encode().unwrap()).unwrap();project
}
#[test]
fn actual_wrapper_construct_and_destruct_run_without_any_paint_and_reattach_is_a_new_wrapper(){
 let project=project();let mut engine=load_game(&project.root).unwrap().engine;engine.step_until_interaction().unwrap();let handle=engine.create_widget(UUID,"one",vec![Value::Int(1)]).unwrap();let source=engine.widget_backend_owner(&handle).unwrap();assert_eq!(engine.state.vars["initialized_count"],Value::Int(1));assert!(engine.renderer.widget_backend.wrapper(&source).is_none());
 engine.add_widget_to_viewport(&handle,false,0).unwrap();let weak=engine.renderer.widget_backend.wrapper(&source).unwrap();let first=weak.upgrade().unwrap();let generation=first.generation();assert!(first.attached());assert!(first.prepared_node().is_ok());assert!(engine.interface_views().unwrap().is_empty());assert_eq!(engine.state.vars["pre_count"],Value::Int(1));assert_eq!(engine.state.vars["constructed_count"],Value::Int(1));assert_eq!(engine.state.vars["saw_pre"],Value::Int(1));
 // Both operations coalesce before World/render; object lifetime still occurs.
 engine.remove_widget_from_viewport(&handle).unwrap();assert!(!first.attached());assert!(engine.renderer.widget_backend.wrapper(&source).is_none());assert_eq!(engine.state.vars["destructed_count"],Value::Int(1));drop(first);assert!(weak.upgrade().is_none());engine.add_widget_to_viewport(&handle,false,0).unwrap();let second=engine.renderer.widget_backend.wrapper(&source).unwrap().upgrade().unwrap();assert!(second.generation()>generation);assert_eq!(engine.state.vars["constructed_count"],Value::Int(2));assert_eq!(engine.state.vars["initialized_count"],Value::Int(1));engine.destroy_widget(handle).unwrap();assert!(!second.attached());assert_eq!(engine.state.vars["destructed_count"],Value::Int(2));
}
#[test]
fn actual_bound_child_none_rooted_none_does_not_rebuild_or_destroy_its_widget_wrapper(){
 let project=project();let mut engine=load_game(&project.root).unwrap().engine;engine.step_until_interaction().unwrap();let handle=engine.create_widget(UUID,"one",vec![Value::Int(1)]).unwrap();engine.add_widget_to_viewport(&handle,true,0).unwrap();let source=engine.widget_backend_owner(&handle).unwrap();let first=engine.renderer.widget_backend.wrapper(&source).unwrap().upgrade().unwrap();assert!(engine.interface_views().unwrap().is_empty());
 engine.advance_dialogue().unwrap();engine.step_until_interaction().unwrap();assert_eq!(engine.interface_views().unwrap().len(),1);assert!(std::sync::Arc::ptr_eq(&first,&engine.renderer.widget_backend.wrapper(&source).unwrap().upgrade().unwrap()));engine.advance_dialogue().unwrap();engine.step_until_interaction().unwrap();assert!(engine.interface_views().unwrap().is_empty());assert_eq!(engine.state.vars["constructed_count"],Value::Int(1));assert_eq!(engine.state.vars["destructed_count"],Value::Int(0));assert!(first.attached());assert_eq!(engine.widget_instance_handle("one").unwrap(),handle);
}
#[test]
fn actual_wrapper_refusals_leave_vm_and_backend_parent_unpublished_and_stale_preparation_cannot_commit(){
 let project=project();let mut engine=load_game(&project.root).unwrap().engine;engine.step_until_interaction().unwrap();let first=engine.create_widget(UUID,"one",vec![Value::Int(1)]).unwrap();let second=engine.create_widget(UUID,"two",vec![Value::Int(2)]).unwrap();let source=engine.widget_backend_owner(&first).unwrap();let stale=engine.renderer.prepare_widget_viewport(&source,WidgetViewportOperation::Attach).unwrap();engine.add_widget_to_viewport(&second,false,0).unwrap();assert!(engine.renderer.validate_widget_viewport(stale.as_ref()).is_err());let batch=engine.renderer.scene_batch.as_deref().unwrap().clone();let mut committed=false;assert!(engine.renderer.commit_scene_batch_atomic(batch,Some(stale.as_ref()),&mut |_|committed=true).is_err());assert!(!committed);assert!(engine.renderer.widget_backend.wrapper(&source).is_none());
 for _ in 0..3{engine.advance_dialogue().unwrap();engine.step_until_interaction().unwrap();}assert_eq!(engine.state.vars["refuse"],Value::Bool(true));let before=serde_json::to_vec(&engine.state).unwrap();let pending=serde_json::to_vec(&engine.renderer.scene_batch.as_ref().unwrap().state).unwrap();assert!(engine.add_widget_to_viewport(&first,false,0).is_err());assert_eq!(serde_json::to_vec(&engine.state).unwrap(),before);assert_eq!(serde_json::to_vec(&engine.renderer.scene_batch.as_ref().unwrap().state).unwrap(),pending);assert!(engine.renderer.widget_backend.wrapper(&source).is_none());
}

#[test]
fn actual_full_tree_reservation_drop_refuses_competing_preparers_without_publishing_any_new_parent(){
 let project=project();let mut engine=load_game(&project.root).unwrap().engine;engine.step_until_interaction().unwrap();let a=engine.create_widget(UUID,"a",vec![Value::Int(1)]).unwrap();let b=engine.create_widget(UUID,"b",vec![Value::Int(2)]).unwrap();engine.add_widget_to_viewport(&a,false,0).unwrap();let a=engine.widget_backend_owner(&a).unwrap();let b=engine.widget_backend_owner(&b).unwrap();let backend=engine.renderer.widget_backend.clone();let original=backend.wrapper(&a).unwrap().upgrade().unwrap();let prepared=backend.prepare_batch(&[a.clone(),b.clone()]).unwrap();assert_eq!(prepared.created().len(),1);assert_eq!(prepared.created()[0].owner,b);assert!(prepared.retired().is_empty());assert!(backend.wrapper(&b).is_none());
 let reserved=backend.reserve_batch(prepared.clone()).unwrap();let competing=backend.clone();let competing_owner=b.clone();let worker=std::thread::Builder::new().stack_size(1024*1024).spawn(move||competing.prepare_batch(&[competing_owner]).is_err()).unwrap();assert!(worker.join().unwrap(),"a second real thread refuses instead of waiting for the owner guard");assert!(backend.prepare_batch(&[a.clone()]).is_err());assert!(backend.prepare(&b,WidgetViewportOperation::Attach).is_err());assert!(backend.reserve_batch(prepared.clone()).is_err());assert!(backend.validate_batch(prepared.as_ref()).is_err());assert!(original.attached());assert!(backend.wrapper(&b).is_none());drop(reserved);
 assert!(std::sync::Arc::ptr_eq(&original,&backend.wrapper(&a).unwrap().upgrade().unwrap()));assert!(backend.wrapper(&b).is_none());assert!(backend.validate_batch(prepared.as_ref()).is_ok());let mut published=0;backend.reserve_batch(prepared).unwrap().commit(&mut||published+=1);assert_eq!(published,1);assert!(backend.wrapper(&b).unwrap().upgrade().unwrap().attached());assert!(original.attached());
}
#[test]
fn actual_full_tree_reservation_keeps_exact_owner_generation_and_batch_until_one_commit(){
 let project=project();let mut engine=load_game(&project.root).unwrap().engine;engine.step_until_interaction().unwrap();let handle=engine.create_widget(UUID,"a",vec![Value::Int(1)]).unwrap();engine.add_widget_to_viewport(&handle,false,0).unwrap();let owner=engine.widget_backend_owner(&handle).unwrap();let backend=engine.renderer.widget_backend.clone();let old=backend.wrapper(&owner).unwrap().upgrade().unwrap();let generation=old.generation();let empty=backend.prepare_batch(&[]).unwrap();assert_eq!(empty.retired().len(),1);assert_eq!(empty.retired()[0].owner,owner);assert_eq!(empty.retired()[0].generation,generation);let reserved=backend.reserve_batch(empty.clone()).unwrap();drop(empty);
 let mut calls=0;reserved.commit(&mut||{calls+=1;assert!(!old.attached());assert!(backend.prepare_batch(&[]).is_err(),"a reentrant preparer refuses while the final publication lock is held");assert!(backend.prepare(&owner,WidgetViewportOperation::Attach).is_err());});assert_eq!(calls,1);assert!(backend.wrapper(&owner).is_none());assert!(!old.attached());let next=backend.prepare_batch(&[owner.clone()]).unwrap();assert_eq!(next.created()[0].owner,owner);assert!(next.created()[0].generation>generation);assert!(backend.wrapper(&owner).is_none(),"pure re-preparation is no parent publication");
}
#[test]
fn actual_full_tree_reservation_rejects_a_stale_or_foreign_backend_and_duplicate_owner_without_mutation(){
 let project=project();let mut engine=load_game(&project.root).unwrap().engine;engine.step_until_interaction().unwrap();let a=engine.create_widget(UUID,"a",vec![Value::Int(1)]).unwrap();let b=engine.create_widget(UUID,"b",vec![Value::Int(2)]).unwrap();let a_owner=engine.widget_backend_owner(&a).unwrap();let b_owner=engine.widget_backend_owner(&b).unwrap();let backend=engine.renderer.widget_backend.clone();assert!(backend.prepare_batch(&[a_owner.clone(),a_owner.clone()]).is_err());let prepared=backend.prepare_batch(&[a_owner.clone()]).unwrap();let other=crate::widget_backend_0956::WidgetBackend::default();assert!(other.reserve_batch(prepared.clone()).is_err());let expired={let previous=crate::widget_backend_0956::WidgetBackend::default();previous.prepare_batch(&[]).unwrap()};assert!(other.reserve_batch(expired).is_err(),"a dropped renderer's weak origin cannot be adopted by a later allocation");assert!(other.wrapper(&a_owner).is_none());
 engine.add_widget_to_viewport(&b,false,0).unwrap();let original=backend.wrapper(&b_owner).unwrap().upgrade().unwrap();assert!(backend.reserve_batch(prepared).is_err());assert!(backend.wrapper(&a_owner).is_none());assert!(std::sync::Arc::ptr_eq(&original,&backend.wrapper(&b_owner).unwrap().upgrade().unwrap()));assert!(original.attached());let current=backend.prepare_batch(&[b_owner]).unwrap();let guard=backend.reserve_batch(current).unwrap();drop(guard);assert!(original.attached());
}

#[test]
fn actual_ui_close_retains_old_widget_destruct_source_and_replacement_alias_is_a_distinct_legacy_instance(){
 let source=LIFECYCLE.replace("handler destruct(event){set destructed_count=destructed_count+1}","handler destruct(event){set destructed_count=destructed_count+1 ui.open(\"one\",[],false,0)}\nscreen one(){return component(\"replacement\",\"text\",{},[])}").replacen("set rooted=true","ui.close(\"one\")",1);
 let project=project_with_source(&source);let mut engine=load_game(&project.root).unwrap().engine;engine.step_until_interaction().unwrap();let handle=engine.create_widget(UUID,"one",vec![Value::Int(1)]).unwrap();engine.add_widget_to_viewport(&handle,false,0).unwrap();let owner=engine.widget_backend_owner(&handle).unwrap();let before_input=engine.capture_ui_input_source("one").unwrap();let backend=engine.renderer.widget_backend.clone();let wrapper=backend.wrapper(&owner).unwrap().upgrade().unwrap();
 engine.advance_dialogue().unwrap();engine.step_until_interaction().unwrap();assert_eq!(engine.state.vars["destructed_count"],Value::Int(1));assert_eq!(engine.state.vars["initialized_count"],Value::Int(1));assert_eq!(engine.state.vars["constructed_count"],Value::Int(1));assert!(!wrapper.attached());assert!(backend.wrapper(&owner).is_none());assert!(!engine.ui_input_source_current(&before_input));let replacement=engine.state.ui.screens.iter().find(|instance|instance.name=="one").unwrap();assert!(replacement.widget_class.is_none());assert_ne!(replacement.order,owner.order());assert!(engine.widget_instance_handle("one").is_none());assert_ne!(engine.capture_ui_input_source("one").unwrap(),before_input);
}
#[test]
fn actual_implicit_destruct_vm_refusal_keeps_old_widget_and_original_parent_atomic(){
 for reject_vm in [false,true]{
  let source=if reject_vm{LIFECYCLE.replace("handler destruct(event){set destructed_count=destructed_count+1}","handler destruct(event){set destructed_count=destructed_count+1 local invalid_interface=\"unknown\" ui.open(invalid_interface,[],false,0)}")}else{LIFECYCLE.into()}.replacen("init{","init{set instance_alias=\"one\" ",1).replacen("set rooted=true","ui.close(instance_alias)",1);
  let project=project_with_source(&source);let mut engine=load_game(&project.root).unwrap().engine;engine.step_until_interaction().unwrap();let handle=engine.create_widget(UUID,"one",vec![Value::Int(1)]).unwrap();engine.add_widget_to_viewport(&handle,false,0).unwrap();let owner=engine.widget_backend_owner(&handle).unwrap();let input=engine.capture_ui_input_source("one").unwrap();let backend=engine.renderer.widget_backend.clone();let wrapper=backend.wrapper(&owner).unwrap().upgrade().unwrap();engine.advance_dialogue().unwrap();
  // An exact stale renderer plan is checked separately below; this branch
  // exercises the real authored VM refusal, never a copied helper assertion.
  if reject_vm{let before=serde_json::to_vec(&engine.state).unwrap();let pending=serde_json::to_vec(&engine.renderer.scene_batch.as_ref().unwrap().state).unwrap();assert!(engine.step_until_interaction().is_err());assert_eq!(serde_json::to_vec(&engine.state).unwrap(),before);assert_eq!(serde_json::to_vec(&engine.renderer.scene_batch.as_ref().unwrap().state).unwrap(),pending);assert!(wrapper.attached());assert!(engine.ui_input_source_current(&input));assert!(std::sync::Arc::ptr_eq(&wrapper,&backend.wrapper(&owner).unwrap().upgrade().unwrap()));}
  else{engine.step_until_interaction().unwrap();assert!(!wrapper.attached());assert!(backend.wrapper(&owner).is_none());assert_eq!(engine.state.vars["destructed_count"],Value::Int(1));}
 }
}
#[test]
fn full_scene_batch_reuses_exact_constructed_transition_object_and_conflict_never_runs_publication(){
 let project=project();let mut engine=load_game(&project.root).unwrap().engine;engine.step_until_interaction().unwrap();let handle=engine.create_widget(UUID,"one",vec![Value::Int(1)]).unwrap();let owner=engine.widget_backend_owner(&handle).unwrap();let backend=engine.renderer.widget_backend.clone();let single=backend.prepare(&owner,WidgetViewportOperation::Attach).unwrap();let first=engine.renderer.prepare_widget_viewport_batch(&[owner.clone()],Some(single.as_ref())).unwrap();let second=engine.renderer.prepare_widget_viewport_batch(&[owner.clone()],Some(single.as_ref())).unwrap();assert_eq!(first.created(),second.created());assert_eq!(first.created().len(),1);assert!(engine.renderer.prepare_widget_viewport_batch(&[],Some(single.as_ref())).is_err());let generation=first.created()[0].generation;let mut calls=0;engine.renderer.reserve_widget_viewport_batch(first).unwrap().commit(&mut||calls+=1);assert_eq!(calls,1);let wrapper=backend.wrapper(&owner).unwrap().upgrade().unwrap();assert_eq!(wrapper.generation(),generation);assert!(engine.renderer.reserve_widget_viewport_batch(second).is_err());assert_eq!(calls,1);assert!(wrapper.attached());
}

#[test]
fn actual_close_then_destruct_uses_one_original_story_random_sequence_without_replaying_a_closed_source(){
 let source=LIFECYCLE.replace("handler destruct(event){set destructed_count=destructed_count+1}","handler destruct(event){set destructed_count=destructed_count+1 set destruct_roll=random(0,1000000)}\nhandler closed(event){set close_roll=random(0,1000000)}").replace("component(\"real\",\"text\",{\"text\":\"actual\"},[])","component(\"real\",\"text\",{\"text\":\"actual\",\"events\":{\"close\":\"closed\"}},[])").replace("set rooted=false set refuse=false","set rooted=true set refuse=false").replacen("init{","init{set instance_alias=\"one\" ",1).replacen("set rooted=true\n\"Rooted\"","ui.close(instance_alias)\n\"Rooted\"",1);
 let project=project_with_source(&source);let mut engine=load_game(&project.root).unwrap().engine;engine.step_until_interaction().unwrap();let handle=engine.create_widget(UUID,"one",vec![Value::Int(1)]).unwrap();engine.add_widget_to_viewport(&handle,false,0).unwrap();let owner=engine.widget_backend_owner(&handle).unwrap();let backend=engine.renderer.widget_backend.clone();let wrapper=backend.wrapper(&owner).unwrap().upgrade().unwrap();let original_input=engine.capture_ui_input_source("one").unwrap();assert!(wrapper.attached());engine.advance_dialogue().unwrap();let mut expected=engine.state.random;let close=expected.integer(0,1000000).unwrap();let destruct=expected.integer(0,1000000).unwrap();engine.step_silent().unwrap();assert_eq!(engine.state.vars["close_roll"],Value::Int(close));assert_eq!(engine.state.vars["destruct_roll"],Value::Int(destruct));assert_eq!(engine.state.random,expected);assert_eq!(engine.state.vars["destructed_count"],Value::Int(1));assert!(engine.widget_instance_handle("one").is_none());assert!(!wrapper.attached());assert!(backend.wrapper(&owner).is_none());assert!(!engine.ui_input_source_current(&original_input));
 // The close/Destruct transaction ends before the next real interaction.
 // A new display has its own existing Story RNG seed; test that frontier
 // separately instead of charging it as a third lifecycle-handler draw.
 assert!(matches!(engine.peek_statement(),Some(rvn_parser::Statement::Dialogue{text,..})if text.as_plain()==Some("Rooted")));
 let display_seed=expected.next_u64();
 let interaction=engine.step_until_interaction().unwrap();
 assert!(matches!(interaction,Some(rvn_core::engine::Interaction::Dialogue{text,..})if text=="Rooted"));
 assert_eq!(engine.state.random,expected);
 assert_eq!(engine.state.display_random,rvn_core::random::RandomState::seeded(display_seed));
 assert_eq!(engine.state.display_random_pc,Some(engine.state.pc));
 engine.step_until_interaction().unwrap();
 assert_eq!(engine.state.random,expected,"reading the same reached display cannot resample Story RNG or replay closed callbacks");
 assert_eq!(engine.state.vars["close_roll"],Value::Int(close));
 assert_eq!(engine.state.vars["destruct_roll"],Value::Int(destruct));
 assert_eq!(engine.state.vars["destructed_count"],Value::Int(1));
}

#[test]
fn stale_actual_full_tree_renderer_fence_never_calls_core_or_replaces_a_pending_publication(){
 let project=project();let mut engine=load_game(&project.root).unwrap().engine;engine.step_until_interaction().unwrap();let a=engine.create_widget(UUID,"a",vec![Value::Int(1)]).unwrap();let b=engine.create_widget(UUID,"b",vec![Value::Int(2)]).unwrap();engine.add_widget_to_viewport(&a,false,0).unwrap();let backend=engine.renderer.widget_backend.clone();let a=engine.widget_backend_owner(&a).unwrap();let plan=engine.renderer.prepare_widget_viewport_batch(&[],None).unwrap();engine.add_widget_to_viewport(&b,false,0).unwrap();let b=engine.widget_backend_owner(&b).unwrap();let aw=backend.wrapper(&a).unwrap().upgrade().unwrap();let bw=backend.wrapper(&b).unwrap().upgrade().unwrap();let before=serde_json::to_vec(&engine.state).unwrap();let pending=serde_json::to_vec(&engine.renderer.scene_batch.as_ref().unwrap().state).unwrap();let batch=engine.renderer.scene_batch.as_deref().unwrap().clone();let mut called=false;assert!(engine.renderer.commit_widget_scene_batch_atomic(batch,plan,&mut |_|called=true).is_err());assert!(!called);assert_eq!(serde_json::to_vec(&engine.state).unwrap(),before);assert_eq!(serde_json::to_vec(&engine.renderer.scene_batch.as_ref().unwrap().state).unwrap(),pending);assert!(aw.attached()&&bw.attached());assert!(std::sync::Arc::ptr_eq(&aw,&backend.wrapper(&a).unwrap().upgrade().unwrap()));assert!(std::sync::Arc::ptr_eq(&bw,&backend.wrapper(&b).unwrap().upgrade().unwrap()));
}

// Adapter-level preparation tests. The pending flag below simulates the exact
// logical intent which the Save15 integration must insert AFTER hydration;
// these are NOT assertions that Save15/driver/portable OwnerEvent is wired.
#[test]
fn prepared_materialization_has_no_input_or_parent_then_one_same_vm_wrapper_life_clears_its_intent(){
 let project=project();let mut engine=load_game(&project.root).unwrap().engine;engine.step_until_interaction().unwrap();let handle=engine.create_widget(UUID,"one",vec![Value::Int(1)]).unwrap();let binding=engine.state.ui.screens.iter_mut().find(|instance|instance.name=="one").unwrap().widget_class.as_mut().unwrap();binding.viewport_attached=true;binding.pending_materialization=true;let owner=engine.widget_backend_owner(&handle).unwrap();let backend=engine.renderer.widget_backend.clone();assert!(engine.capture_ui_input_source("one").is_none());assert!(backend.wrapper(&owner).is_none());let operation=engine.next_widget_materialization().unwrap().unwrap();let generation=operation.object().unwrap().generation;assert_eq!(operation.owner(),&owner);assert!(engine.accepts_widget_materialization(&operation));assert!(backend.wrapper(&owner).is_none());let duplicate=operation.clone();engine.execute_widget_materialization(operation).unwrap();assert!(!engine.state.ui.screens.iter().find(|instance|instance.name=="one").unwrap().widget_class.as_ref().unwrap().pending_materialization);assert_eq!(engine.state.vars["initialized_count"],Value::Int(1));assert_eq!(engine.state.vars["pre_count"],Value::Int(1));assert_eq!(engine.state.vars["constructed_count"],Value::Int(1));assert_eq!(engine.state.vars["destructed_count"],Value::Int(0));assert_eq!(backend.wrapper(&owner).unwrap().upgrade().unwrap().generation(),generation);assert!(engine.capture_ui_input_source("one").is_some());let before=serde_json::to_vec(&engine.state).unwrap();assert!(engine.execute_widget_materialization(duplicate).is_err());assert_eq!(serde_json::to_vec(&engine.state).unwrap(),before);assert!(engine.next_widget_materialization().unwrap().is_none());
}
#[test]
fn pending_materialization_refusal_and_remove_revoke_original_object_without_replay_or_fake_destruct(){
 let project=project();let mut engine=load_game(&project.root).unwrap().engine;engine.step_until_interaction().unwrap();let handle=engine.create_widget(UUID,"one",vec![Value::Int(1)]).unwrap();let binding=engine.state.ui.screens.iter_mut().find(|instance|instance.name=="one").unwrap().widget_class.as_mut().unwrap();binding.viewport_attached=true;binding.pending_materialization=true;let owner=engine.widget_backend_owner(&handle).unwrap();let backend=engine.renderer.widget_backend.clone();let operation=engine.next_widget_materialization().unwrap().unwrap();engine.state.vars.insert("refuse".into(),Value::Bool(true));let before=serde_json::to_vec(&engine.state).unwrap();assert!(engine.execute_widget_materialization(operation.clone()).is_err());assert_eq!(serde_json::to_vec(&engine.state).unwrap(),before);assert!(backend.wrapper(&owner).is_none());assert!(engine.capture_ui_input_source("one").is_none());engine.remove_widget_from_viewport(&handle).unwrap();assert!(!engine.accepts_widget_materialization(&operation));assert!(engine.execute_widget_materialization(operation).is_err());assert_eq!(engine.state.vars["initialized_count"],Value::Int(1));assert_eq!(engine.state.vars["pre_count"],Value::Int(0));assert_eq!(engine.state.vars["constructed_count"],Value::Int(0));assert_eq!(engine.state.vars["destructed_count"],Value::Int(0));assert!(engine.next_widget_materialization().unwrap().is_none());assert!(backend.wrapper(&owner).is_none());
}

#[path="widget_materialization_save_tests_0964.rs"]
mod deferred_save_0964;

#[path="widget_empty_owner_phase_entry_tests_0988.rs"]
mod empty_owner_phase_entry0988;

#[path="widget_viewport_bevy_tests1003.rs"]
mod viewport1003;

#[path="widget_handler_remove_bevy_tests1003.rs"]
mod handler_remove1003;
