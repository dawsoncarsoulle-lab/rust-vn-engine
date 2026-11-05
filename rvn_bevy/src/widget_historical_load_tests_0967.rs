//! Original historical format validation plus actual old Widget retirement.
//! CPU loader/VM/backend cases; not Source/GPU/input-focus acceptance.
use super::*;
fn historical_game(source:&str,scene:bool)->(Project,Game,rvn_core::save::SaveData,rvn_core::engine::WidgetInstanceHandle){
 let source=if scene{source.replace("label start\n","scene label start\n")}else{source.to_owned()};let project=project_with_source(&source);let launch=crate::load_runtime_game(&project.root).unwrap();
 let crate::RuntimeLaunchOwner::Startup(mut bootstrap)=launch.engine else{panic!("a real admitted Widget project retains its actual startup owner")};
 let mut steps=0;while !bootstrap.owner.is_complete(){steps+=1;assert!(steps<=1024,"bounded actual fixture Init");bootstrap.owner.step(StepKind::Continue).unwrap();}
 let mut game=bootstrap.owner.into_engine().unwrap();
 // Consume the actual Init output before entry changes PC or narrative RNG.
 // The zero-media host still verifies the exact complete batch and source.
 handoff_zero_media_publication(&mut game);game.enter_label("start").unwrap();game.step_until_interaction().unwrap();game.publish_current_scene_interaction().unwrap();handoff_zero_media_publication(&mut game);
 let checkpoint=rvn_core::save::SaveData::from_state(&game.state,1,"actual historical target without class".into(),"main.rvn".into());assert!((1..=14).contains(&checkpoint.format_version));assert!(checkpoint.ui.screens.is_empty());
 let handle=game.create_widget(UUID,"one",vec![Value::Int(1)]).unwrap();game.add_widget_to_viewport(&handle,false,0).unwrap();handoff_zero_media_publication(&mut game);(project,game,checkpoint,handle)
}
fn assert_historical_swap(game:&mut Game,checkpoint:rvn_core::save::SaveData,handle:rvn_core::engine::WidgetInstanceHandle){
 let expected=checkpoint.clone().into_game_state();let owner=game.widget_backend_owner(&handle).unwrap();let backend=game.renderer.widget_backend.clone();let old=backend.wrapper(&owner).unwrap().upgrade().unwrap();let input=game.capture_ui_input_source("one").unwrap();let epoch=game.runtime_epoch();set_old_history_guards(game);
 assert_eq!(game.load_data(checkpoint).unwrap(),rvn_core::LoadCompatibility::Verified);assert!(!old.attached());assert!(backend.wrapper(&owner).is_none());assert!(game.widget_instance_handle("one").is_none());assert!(!game.ui_input_source_current(&input));assert!(game.add_widget_to_viewport(&handle,false,0).is_err());assert_eq!(game.state.vars,expected.vars);assert_eq!(game.state.random,expected.random);assert_eq!(game.state.vars["destructed_count"],Value::Int(0));assert_eq!(game.runtime_epoch(),epoch+1);assert!(!game.can_rollback());assert!(!game.has_pending_widget_materialization());
 let batch=game.renderer.scene_batch.as_ref().expect("one coherent existing Scene publication");assert_eq!(batch.runtime_epoch,game.runtime_epoch());assert_eq!(serde_json::to_value(&batch.state).unwrap(),serde_json::to_value(&game.state).unwrap());assert!(batch.interfaces.is_empty());assert!(matches!(&batch.display,Some(rvn_core::Interaction::Dialogue{text,..})if text=="Ready"));assert!(game.renderer.pending.iter().any(|command|matches!(command,crate::vn_command::VnCommand::LoadedCompatibility(rvn_core::LoadCompatibility::Verified))));
}
#[test]
fn real_historical_story_formats_keep_original_validation_and_retire_old_class_using_its_old_globals_and_rng(){
 for version in [1,2,3,4,5,6,7,8,9,12,13,14]{let(_project,mut game,mut checkpoint,handle)=historical_game(&guarded_source(),false);checkpoint.format_version=version;assert_historical_swap(&mut game,checkpoint,handle);}
}
#[test]
fn real_historical_scene_formats_ten_through_fourteen_use_the_same_old_source_parent_and_target_fence(){
 for version in [10,11,12,13,14]{let(_project,mut game,mut checkpoint,handle)=historical_game(&guarded_source(),true);checkpoint.format_version=version;assert!(checkpoint.scene_execution.is_some());assert_historical_swap(&mut game,checkpoint,handle);}
}
#[test]
fn historical_invalid_target_is_refused_by_original_identity_guard_before_destruct_or_live_mutation(){
 let source=SOURCE.replace("handler destruct(event){set destructed_count=destructed_count+1}","handler destruct(event){local invalid_interface=\"unknown\" ui.open(invalid_interface,[],false,0)}");let(_project,mut game,mut checkpoint,handle)=historical_game(&source,false);checkpoint.story_identity=Some("foreign actual story".into());let owner=game.widget_backend_owner(&handle).unwrap();let old=game.renderer.widget_backend.wrapper(&owner).unwrap().upgrade().unwrap();let before=serde_json::to_value(&game.state).unwrap();let token=game.debug_state_source().unwrap();let error=game.load_data(checkpoint).unwrap_err().to_string();assert!(error.contains("l’histoire a changé"));assert!(!error.contains("Old Widget Destruct"));assert_eq!(serde_json::to_value(&game.state).unwrap(),before);assert!(game.accepts_debug_state_source(&token));assert!(old.attached());assert!(!game.renderer.scene_batch_pending());assert!(game.renderer.take_pending().is_empty());
}
#[test]
fn historical_destruct_refusal_or_execution_limit_preserves_old_parent_source_history_rng_and_publication(){
 for scene in [false,true]{for looping in [false,true]{let source=SOURCE.replace("handler destruct(event){set destructed_count=destructed_count+1}",if looping{"handler destruct(event){while true{set destructed_count=destructed_count+1}}"}else{"handler destruct(event){set destructed_count=destructed_count+1 local invalid_interface=\"unknown\" ui.open(invalid_interface,[],false,0)}"});let(_project,mut game,checkpoint,handle)=historical_game(&source,scene);let owner=game.widget_backend_owner(&handle).unwrap();let backend=game.renderer.widget_backend.clone();let old=backend.wrapper(&owner).unwrap().upgrade().unwrap();let before=serde_json::to_value(&game.state).unwrap();let token=game.debug_state_source().unwrap();let input=game.capture_ui_input_source("one").unwrap();let history=game.can_rollback();let error=game.load_data(checkpoint).unwrap_err().to_string();assert!(error.contains("Old Widget Destruct preparation"));assert_eq!(serde_json::to_value(&game.state).unwrap(),before);assert!(game.accepts_debug_state_source(&token));assert!(old.attached());assert!(std::sync::Arc::ptr_eq(&old,&backend.wrapper(&owner).unwrap().upgrade().unwrap()));assert!(game.ui_input_source_current(&input));assert_eq!(game.can_rollback(),history);assert!(!game.renderer.scene_batch_pending());assert!(game.renderer.take_pending().is_empty());}}
}
#[test]
fn historical_target_class_provenance_remains_rejected_and_cannot_be_retrofitted_into_formats_one_through_fourteen(){
 let(_project,mut game,_checkpoint,handle)=historical_game(SOURCE,false);let mut forged=rvn_core::save::SaveData::from_state(&game.state,1,"cannot retrofit class identity".into(),"main.rvn".into());for version in 1..=14{forged.format_version=version;let before=serde_json::to_value(&game.state).unwrap();let token=game.debug_state_source().unwrap();let owner=game.widget_backend_owner(&handle).unwrap();let old=game.renderer.widget_backend.wrapper(&owner).unwrap().upgrade().unwrap();let error=game.load_data(forged.clone()).unwrap_err().to_string();assert!(error.contains("Historical save cannot grant Widget class provenance"));assert_eq!(serde_json::to_value(&game.state).unwrap(),before);assert!(game.accepts_debug_state_source(&token));assert!(old.attached());assert!(!game.renderer.scene_batch_pending());}
}

#[test]
fn historical_unverified_legacy_identity_remains_explicit_while_the_current_class_parent_is_retired(){
 let(_project,mut game,mut checkpoint,handle)=historical_game(&guarded_source(),false);checkpoint.story_identity=None;let expected=checkpoint.clone().into_game_state();let owner=game.widget_backend_owner(&handle).unwrap();let old=game.renderer.widget_backend.wrapper(&owner).unwrap().upgrade().unwrap();set_old_history_guards(&mut game);assert_eq!(game.load_data(checkpoint).unwrap(),rvn_core::LoadCompatibility::LegacyUnchecked);assert!(!old.attached());assert_eq!(game.state.vars,expected.vars);assert_eq!(game.state.random,expected.random);assert!(game.renderer.pending.iter().any(|command|matches!(command,crate::vn_command::VnCommand::LoadedCompatibility(rvn_core::LoadCompatibility::LegacyUnchecked))));assert!(!game.has_pending_widget_materialization());
}
#[test]
fn historical_old_source_external_speech_remains_an_explicit_atomic_unported_effect(){
 let source=SOURCE.replace("handler destruct(event){set destructed_count=destructed_count+1}","handler destruct(event){set destructed_count=destructed_count+1 accessibility.configure({\"self_voicing\":true}) accessibility.speak(\"Old source effect\")} ");let(_project,mut game,checkpoint,handle)=historical_game(&source,false);let owner=game.widget_backend_owner(&handle).unwrap();let old=game.renderer.widget_backend.wrapper(&owner).unwrap().upgrade().unwrap();let before=serde_json::to_value(&game.state).unwrap();let token=game.debug_state_source().unwrap();let error=game.load_data(checkpoint).unwrap_err().to_string();assert!(error.contains("frozen old Widget Speech publication adapter"));assert_eq!(serde_json::to_value(&game.state).unwrap(),before);assert!(game.accepts_debug_state_source(&token));assert!(old.attached());assert!(!game.renderer.scene_batch_pending());assert!(game.renderer.take_pending().is_empty());
}
