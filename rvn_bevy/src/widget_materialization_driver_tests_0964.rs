//! Actual existing driver functions/World/VM/backend, CPU only. This unit
//! observes the pending batch handed to the ordinary Source consumer. It does
//! not replace that consumer or claim final World/paint/native acceptance.
use super::*;
use bevy::prelude::*;
use crate::resources::{VnEngine,VnState};
// Same owner/Init phases as the actual production loader; no second VM.
// CPU owner stepping is not a claim of Source installation or painting.
#[inline(never)]
fn driver_game(project:&Project)->Game{
 let launch=crate::load_runtime_game(&project.root).unwrap();
 let crate::RuntimeLaunchOwner::Startup(mut startup)=launch.engine else{panic!("Widget class admission must retain the real pending owner")};
 assert_eq!(startup.profile,crate::debugger_startup::BootstrapProfile::RuntimeInitial);
 assert!(!startup.owner.state().vars.contains_key("initialized_count"),"admission must not execute authored Init");
 let instance=startup.owner.snapshot().instance;let epoch=startup.owner.snapshot().epoch;let gate=startup.broker.gate();
 for _ in 0..256{if startup.owner.is_complete(){break;}startup.owner.step(StepKind::Continue).unwrap();}
 assert!(startup.owner.is_complete(),"the same admitted owner must finish the actual Init");
 let label=startup.start_label.take();let mut game=startup.owner.into_engine().unwrap();
 assert_eq!(game.instance_id(),instance);assert_eq!(game.runtime_epoch(),epoch);assert!(game.renderer.audio_checkpoints.gate().same_gate(&gate));assert!(game.widget_classes().class(UUID).is_some());
 // Settle the actual Init batch while its state/source still matches Core,
 // before entry changes PC. This retains the existing zero-media CPU receipt;
 // it never rewrites a snapshot or claims physical publication.
 if game.renderer.scene_batch_pending(){handoff_zero_media_publication(&mut game);}
 if let Some(label)=label{game.enter_label(&label).unwrap();}game
}
#[inline(never)]
fn driver_warm(project:&Project)->(Game,rvn_core::engine::WidgetInstanceHandle){
 let mut game=driver_game(project);
 game.step_until_interaction().unwrap();
 let handle=game.create_widget(UUID,"one",vec![Value::Int(1)]).unwrap();
 game.add_widget_to_viewport(&handle,false,0).unwrap();(game,handle)
}
fn boundary_world(mut game:Game)->World{
 // Fixture starts after its previous Core output was handed to the host. Tests
 // inspect each new output rather than running an unrelated media/paint stack.
 handoff_zero_media_publication(&mut game);
 let mut world=World::new();world.insert_resource(VnEngine(game));world.insert_resource(State::new(VnState::Waiting));world.init_resource::<NextState<VnState>>();world.init_resource::<Time<bevy::time::Virtual>>();world.init_resource::<crate::resources::ScriptErrorMessage>();world
}
fn deferred_story(project:&Project)->(Game,Option<DebugSession>){
 let(mut game,_)=driver_warm(project);
 game.advance_dialogue().unwrap();
 let session=pause_after_binding(&mut game,DebugOperation::Statement,"story_draw");
 let checkpoint=save(&mut game,project,Some(&session));
 let mut cold=driver_game(project);
 let restored=cold.restore_save15(&project.program(),checkpoint).unwrap();(cold,restored)
}
#[test]
fn real_normal_driver_materializes_once_and_leaves_the_exact_batch_for_the_actual_source_consumer(){
 let project=native_project(SOURCE);let(mut game,_)=driver_warm(&project);let checkpoint=save(&mut game,&project,None);let mut cold=driver_game(&project);cold.restore_save15(&project.program(),checkpoint).unwrap();let mut world=boundary_world(cold);assert!(!world.resource::<VnEngine>().0.widget_materialization_waits_for_saved_operation());crate::editor_debugger::drive_widget_materialization(&mut world);let game=&world.resource::<VnEngine>().0;assert_eq!(game.state.vars["constructed_count"],Value::Int(2));assert_eq!(game.state.vars["initialized_count"],Value::Int(1));assert!(!game.has_pending_widget_materialization());assert!(game.renderer.scene_batch_pending());let before=serde_json::to_vec(&game.state).unwrap();crate::editor_debugger::drive_widget_materialization(&mut world);assert_eq!(serde_json::to_vec(&world.resource::<VnEngine>().0.state).unwrap(),before);
}
#[test]
fn real_driver_keeps_saved_story_paused_then_binds_new_widget_life_after_its_commit_without_a_second_vm(){
 let project=native_project(SOURCE);let(game,mut session)=deferred_story(&project);let vars=game.state.vars.clone();let rng=game.state.random;let mut world=boundary_world(game);crate::editor_debugger::drive_widget_materialization(&mut world);assert_eq!(world.resource::<VnEngine>().0.state.vars,vars);assert_eq!(world.resource::<VnEngine>().0.state.random,rng);assert!(world.resource::<VnEngine>().0.widget_materialization_waits_for_saved_operation());
 crate::editor_debugger::adopt_restored_widget_session(&mut world,&mut session,None).unwrap();assert!(session.is_none());for _ in 0..3{crate::editor_debugger::drive_widget_materialization(&mut world);}assert_eq!(world.resource::<VnEngine>().0.state.vars,vars);assert_eq!(world.resource::<VnEngine>().0.state.random,rng);assert!(world.contains_resource::<crate::editor_preview::PreviewState>());assert!(!world.contains_resource::<crate::editor_preview::Output>());assert!(crate::editor_debugger::pending(world.resource::<VnEngine>()));
 crate::editor_debugger::run(&mut world,StepKind::Continue).unwrap();for _ in 0..256{crate::editor_debugger::drive_widget_materialization(&mut world);if !world.resource::<VnEngine>().0.widget_materialization_waits_for_saved_operation(){break;}}let game=&world.resource::<VnEngine>().0;assert!(!game.widget_materialization_waits_for_saved_operation());assert_eq!(game.state.vars["constructed_count"],Value::Int(1));assert!(game.has_pending_widget_materialization());assert!(game.renderer.scene_batch_pending());let observed=game.state.vars["observed"].clone();let after_story=game.state.random;
 // A new boundary cannot steal the pending authoritative publication.
 crate::editor_debugger::drive_widget_materialization(&mut world);assert_eq!(world.resource::<VnEngine>().0.state.vars["constructed_count"],Value::Int(1));
 handoff_zero_media_publication(&mut world.resource_mut::<VnEngine>().0);
 for _ in 0..256{crate::editor_debugger::drive_widget_materialization(&mut world);if !world.resource::<VnEngine>().0.has_pending_widget_materialization(){break;}}let game=&world.resource::<VnEngine>().0;assert_eq!(game.state.vars["constructed_count"],Value::Int(2));assert_eq!(game.state.vars["initialized_count"],Value::Int(1));assert_eq!(game.state.vars["observed"],observed);let mut expected=after_story;expected.integer(0,1000000).unwrap();assert_eq!(game.state.random,expected);assert!(!game.has_pending_widget_materialization());assert!(game.renderer.scene_batch_pending());
}
#[test]
fn real_driver_refuses_competing_current_session_without_consuming_the_callers_owner_and_cancel_revokes_only_its_barrier(){
 let project=native_project(SOURCE);let(game,mut first)=deferred_story(&project);let mut world=boundary_world(game);let before=serde_json::to_vec(&world.resource::<VnEngine>().0.state).unwrap();
 // Try the previously missed order: create/adopt a competitor BEFORE the real
 // saved session has entered the empty mailbox. Core rejects fresh work while
 // the opaque saved session exclusively owns this boundary.
 assert!(world.resource::<VnEngine>().0.debug_begin(DebugOperation::Statement,vec![]).is_err());
 assert!(world.resource::<VnEngine>().0.widget_materialization_waits_for_saved_operation());assert!(world.resource::<VnEngine>().0.accepts_debug_session(first.as_ref().unwrap()));assert_eq!(serde_json::to_vec(&world.resource::<VnEngine>().0.state).unwrap(),before);
 // A genuinely different Engine session is also refused without consuming the
 // caller's ownership, even while no driver session is present yet.
 let mut other=driver_game(&project);other.step_until_interaction().unwrap();let mut competitor=Some(other.debug_begin(DebugOperation::Statement,vec![]).unwrap());assert!(crate::editor_debugger::adopt_restored_widget_session(&mut world,&mut competitor,None).is_err());assert!(competitor.is_some());assert!(first.is_some());assert!(world.resource::<VnEngine>().0.accepts_debug_session(first.as_ref().unwrap()));
 crate::editor_debugger::adopt_restored_widget_session(&mut world,&mut first,None).unwrap();assert!(first.is_none());assert_eq!(serde_json::to_vec(&world.resource::<VnEngine>().0.state).unwrap(),before);assert!(crate::editor_debugger::adopt_restored_widget_session(&mut world,&mut competitor,None).is_err());assert!(competitor.is_some());crate::editor_debugger::cancel(&mut world).unwrap();assert!(!world.resource::<VnEngine>().0.widget_materialization_waits_for_saved_operation());assert!(world.resource::<VnEngine>().0.has_pending_widget_materialization());assert!(world.resource::<VnEngine>().0.capture_ui_input_source("one").is_none());assert!(!world.resource::<VnEngine>().0.accepts_debug_session(competitor.as_ref().unwrap()));
}
