//! Real loader/VM/backend Load retirement cases, CPU only. No World/raster
//! acceptance is inferred from the logical/backend parent fence.
use super::*;
use rvn_core::{Renderer,widget_backend_0956::WidgetViewportOperation};
fn guarded_source()->String{SOURCE.replace("handler destruct(event){set destructed_count=destructed_count+1}","handler destruct(event){local retired_draw=random(0,1000000) if shared!=expected_shared{local invalid_interface=\"unknown\" ui.open(invalid_interface,[],false,0)} if retired_draw!=expected_draw{local invalid_interface=\"unknown\" ui.open(invalid_interface,[],false,0)} set destructed_count=destructed_count+1}").replace("set shared=0","set shared=0 set expected_shared=-1 set expected_draw=-1")}
#[test]
fn real_hot_load_prepares_destruct_on_old_globals_rng_and_source_once_before_the_atomic_parent_and_saved_state_swap(){
 let project=native_project(&guarded_source());let(mut game,old_handle)=warm(&project);let checkpoint=save(&mut game,&project,None);let expected_vars=game.state.vars.clone();let saved_rng=game.state.random;let old_input=game.capture_ui_input_source("one").unwrap();let owner=game.widget_backend_owner(&old_handle).unwrap();let backend=game.renderer.widget_backend.clone();let wrapper=backend.wrapper(&owner).unwrap().upgrade().unwrap();
 let mut old_rng=game.state.random;let draw=old_rng.integer(0,1000000).unwrap();game.state.vars.insert("shared".into(),Value::Int(777));game.state.vars.insert("expected_shared".into(),Value::Int(777));game.state.vars.insert("expected_draw".into(),Value::Int(draw));let old_state=serde_json::to_value(&game.state).unwrap();let source=game.debug_state_source().unwrap();
 let prepared=game.prepare_save15_with_locale(&project.program(),checkpoint,game.locale.clone()).expect("Destruct must use the old 777 globals and exact old RNG; target saved expectations are -1");assert!(wrapper.attached());assert!(game.accepts_debug_state_source(&source));assert_eq!(serde_json::to_value(&game.state).unwrap(),old_state);
 let no_target=game.commit_prepared_save15(prepared).unwrap();assert!(no_target.is_none());assert!(!wrapper.attached());assert!(backend.wrapper(&owner).is_none());assert_eq!(game.state.vars,expected_vars);assert_eq!(game.state.random,saved_rng);assert_eq!(game.state.vars["destructed_count"],Value::Int(0));assert!(!game.ui_input_source_current(&old_input));assert!(game.add_widget_to_viewport(&old_handle,false,0).is_err());pending(&game);
 // A second evaluation on the loaded target would fail its -1 guards. The
 // subsequent representation receives only its genuine new lifecycle.
 new_life(&mut game);assert_eq!(game.state.vars["constructed_count"],Value::Int(2));assert_eq!(game.state.vars["destructed_count"],Value::Int(0));let current=game.widget_instance_handle("one").unwrap();let new_owner=game.widget_backend_owner(&current).unwrap();assert_ne!(backend.wrapper(&new_owner).unwrap().upgrade().unwrap().generation(),wrapper.generation());
}
#[test]
fn real_hot_load_destruct_vm_refusal_or_execution_limit_keeps_old_state_rng_input_parent_and_output_unchanged(){
 for looping in [false,true]{let replacement=if looping{"handler destruct(event){while true{set destructed_count=destructed_count+1}}"}else{"handler destruct(event){set destructed_count=destructed_count+1 local invalid_interface=\"unknown\" ui.open(invalid_interface,[],false,0)}"};let project=native_project(&SOURCE.replace("handler destruct(event){set destructed_count=destructed_count+1}",replacement));let(mut game,handle)=warm(&project);let checkpoint=save(&mut game,&project,None);let owner=game.widget_backend_owner(&handle).unwrap();let backend=game.renderer.widget_backend.clone();let wrapper=backend.wrapper(&owner).unwrap().upgrade().unwrap();let input=game.capture_ui_input_source("one").unwrap();let source=game.debug_state_source().unwrap();let before=serde_json::to_value(&game.state).unwrap();
  assert!(game.prepare_save15_with_locale(&project.program(),checkpoint,game.locale.clone()).is_err());assert!(game.accepts_debug_state_source(&source));assert_eq!(serde_json::to_value(&game.state).unwrap(),before);assert!(game.ui_input_source_current(&input));assert!(wrapper.attached());assert!(std::sync::Arc::ptr_eq(&wrapper,&backend.wrapper(&owner).unwrap().upgrade().unwrap()));assert!(!game.renderer.scene_batch_pending());assert!(game.renderer.take_pending().is_empty());
 }
}
#[test]
fn real_load_guard_reserves_the_actual_old_parent_and_drop_cancels_without_replaying_destruct_or_publishing_target(){
 let project=native_project(SOURCE);let(mut game,handle)=warm(&project);let checkpoint=save(&mut game,&project,None);let owner=game.widget_backend_owner(&handle).unwrap();let backend=game.renderer.widget_backend.clone();let wrapper=backend.wrapper(&owner).unwrap().upgrade().unwrap();let before=serde_json::to_value(&game.state).unwrap();let source=game.debug_state_source().unwrap();let prepared=game.prepare_save15_with_locale(&project.program(),checkpoint.clone(),game.locale.clone()).unwrap();
 {let _guard=game.begin_prepared_save15_commit(prepared).unwrap();assert!(wrapper.attached());assert!(backend.prepare(&owner,WidgetViewportOperation::Detach).is_err());assert!(backend.prepare_batch(&[]).is_err());}
 assert!(game.accepts_debug_state_source(&source));assert_eq!(serde_json::to_value(&game.state).unwrap(),before);assert!(wrapper.attached());assert!(!game.renderer.scene_batch_pending());assert!(backend.prepare(&owner,WidgetViewportOperation::Detach).is_ok());let prepared=game.prepare_save15_with_locale(&project.program(),checkpoint,game.locale.clone()).unwrap();game.commit_prepared_save15(prepared).unwrap();assert!(!wrapper.attached());pending(&game);
}
#[test]
fn stale_real_parent_revision_refuses_prepared_load_before_core_or_its_publication_can_change(){
 let project=native_project(SOURCE);let(mut game,handle)=warm(&project);let checkpoint=save(&mut game,&project,None);let owner=game.widget_backend_owner(&handle).unwrap();let backend=game.renderer.widget_backend.clone();let prepared=game.prepare_save15_with_locale(&project.program(),checkpoint,game.locale.clone()).unwrap();let before=serde_json::to_value(&game.state).unwrap();let source=game.debug_state_source().unwrap();let detach=backend.prepare(&owner,WidgetViewportOperation::Detach).unwrap();backend.commit(detach.as_ref(),||{}).unwrap();
 assert!(game.commit_prepared_save15(prepared).is_err());assert!(game.accepts_debug_state_source(&source));assert_eq!(serde_json::to_value(&game.state).unwrap(),before);assert!(!game.renderer.scene_batch_pending());assert!(game.renderer.take_pending().is_empty());
}
// Execute the production authority synchronizer against the actual live Screen.
// Only external dispatch is disabled: this fixture is testing its explicit
// old-source transaction-adapter refusal, not granting a host menu action.
fn synchronize_retirement_menu_host(project:&Project,mut game:Game)->Game{
 use bevy::prelude::*;
 use crate::resources::*;
 handoff_zero_media_publication(&mut game);
 let before=serde_json::to_value(&game.state).unwrap();
 let source=game.debug_state_source().unwrap();
 assert!(!game.renderer.menu_authority.story_ui_active);
 let manager=rvn_core::persistent::PersistentDataManager::new(project.root.join("saves")).unwrap();
 let data=manager.load().unwrap();
 let mut app=App::new();
 app.insert_resource(VnEngine(game))
  .insert_resource(State::new(VnState::Waiting))
  .init_resource::<MenuState>()
  .init_resource::<crate::systems::save_menu::SaveMenuState>()
  .init_resource::<crate::systems::save_menu::SaveConfirmation>()
  .init_resource::<crate::systems::settings_menu::SettingsMenuState>()
  .init_resource::<crate::systems::settings_menu::Settings>()
  .init_resource::<GalleryState>().init_resource::<DialogueHistory>()
  .insert_resource(PersistentDataResource{manager,data})
  .insert_resource(LocaleConfig{available_langs:Vec::new(),last_modified_current:std::time::UNIX_EPOCH,last_modified_default:std::time::UNIX_EPOCH})
  .init_resource::<CharacterRegistry>().init_resource::<CgAssetRegistry>()
  .init_resource::<VnRenderState>().init_resource::<TypewriterState>()
  .init_resource::<ScriptErrorMessage>()
  .insert_resource(crate::project_paths::ProjectPaths::new(project.root.clone(),project.root.join("assets"),project.root.join("locales"),project.root.join("theme.toml"),project.root.join("saves")))
  .init_resource::<crate::menu_documents::Menus>()
  .add_event::<crate::vn_command::VnCommand>()
  .add_plugins(crate::source_menus::SourceMenusPlugin)
  .configure_sets(Update,crate::source_menus::SourceMenuActionSet.run_if(||false));
 app.update();
 let mut game=app.world_mut().remove_resource::<VnEngine>().unwrap().0;
 assert!(game.renderer.menu_authority.story_ui_active);
 assert!(game.renderer.menu_authority.live_screens.contains("one"));
 assert!(game.renderer.menu_authority.screen_roles.is_empty());
 assert!(game.accepts_debug_state_source(&source));
 assert_eq!(serde_json::to_value(&game.state).unwrap(),before);
 assert!(!game.renderer.scene_batch_pending());
 assert!(game.renderer.take_pending().is_empty());
 game
}
#[test]
fn old_source_external_destruct_menu_is_explicitly_refused_until_its_transaction_adapter_exists(){
 let source=SOURCE.replace("handler destruct(event){set destructed_count=destructed_count+1}","handler destruct(event){set destructed_count=destructed_count+1 menu.execute(menu_action(\"none\"))}");let project=native_project(&source);let(game,handle)=warm(&project);let mut game=synchronize_retirement_menu_host(&project,game);let checkpoint=save(&mut game,&project,None);let owner=game.widget_backend_owner(&handle).unwrap();let backend=game.renderer.widget_backend.clone();let wrapper=backend.wrapper(&owner).unwrap().upgrade().unwrap();let before=serde_json::to_value(&game.state).unwrap();let token=game.debug_state_source().unwrap();
 let result=game.prepare_save15_with_locale(&project.program(),checkpoint,game.locale.clone());assert!(result.is_err());let message=result.err().unwrap().to_string();assert!(message.contains("old-source transaction publication adapter"),"expected the actual old-source adapter guard after live host authority: {message}");assert!(game.accepts_debug_state_source(&token));assert_eq!(serde_json::to_value(&game.state).unwrap(),before);assert!(wrapper.attached());assert!(!game.renderer.scene_batch_pending());assert!(game.renderer.take_pending().is_empty());
}

#[test]
fn actual_hot_load_hydrates_a_stopped_new_construct_before_retiring_old_parents_and_resumes_one_new_object_after_commit(){
 let project=native_project(SOURCE);let(mut first,_)=warm(&project);let checkpoint=save(&mut first,&project,None);let mut saved_owner=load_game(&project.root).unwrap().engine;saved_owner.restore_save15(&project.program(),checkpoint).unwrap();let operation=saved_owner.next_widget_materialization().unwrap().unwrap();let session=pause_after_binding(&mut saved_owner,DebugOperation::MaterializeWidget(operation),"construct_draw");let checkpoint=save(&mut saved_owner,&project,Some(&session));let saved_vars=saved_owner.state.vars.clone();let saved_rng=saved_owner.state.random;
 let(mut live,handle)=warm(&project);handoff_zero_media_publication(&mut live);let owner=live.widget_backend_owner(&handle).unwrap();let backend=live.renderer.widget_backend.clone();let old=backend.wrapper(&owner).unwrap().upgrade().unwrap();let prepared=live.prepare_save15_with_locale(&project.program(),checkpoint,live.locale.clone()).expect("logical saved Construct hydration must not evaluate Destruct against target/new epoch");assert!(old.attached());let mut restored=live.commit_prepared_save15(prepared).unwrap().unwrap();assert!(!old.attached());assert!(backend.wrapper(&owner).is_none());assert_eq!(live.state.vars,saved_vars);assert_eq!(live.state.random,saved_rng);pending(&live);assert!(live.next_widget_materialization().unwrap().is_none());assert!(live.widget_materialization_waits_for_saved_operation());assert!(matches!(finish(&mut live,&mut restored),DebugOutcome::WidgetMaterialized));assert_eq!(live.state.vars["initialized_count"],Value::Int(1));assert_eq!(live.state.vars["constructed_count"],Value::Int(2));assert_eq!(live.state.vars["destructed_count"],Value::Int(0));assert!(!live.widget_materialization_waits_for_saved_operation());assert!(!live.has_pending_widget_materialization());let new_handle=live.widget_instance_handle("one").unwrap();let new_owner=live.widget_backend_owner(&new_handle).unwrap();assert_ne!(backend.wrapper(&new_owner).unwrap().upgrade().unwrap().generation(),old.generation());
}
#[test]
fn invalid_saved_source_refuses_before_any_old_destruct_callback_is_prepared(){
 let source=SOURCE.replace("handler destruct(event){set destructed_count=destructed_count+1}","handler destruct(event){local invalid_interface=\"unknown\" ui.open(invalid_interface,[],false,0)}");let project=native_project(&source);let(mut game,handle)=warm(&project);let mut checkpoint=save(&mut game,&project,None);checkpoint.source.content[0]^=1;let owner=game.widget_backend_owner(&handle).unwrap();let wrapper=game.renderer.widget_backend.wrapper(&owner).unwrap().upgrade().unwrap();let before=serde_json::to_value(&game.state).unwrap();let result=game.prepare_save15_with_locale(&project.program(),checkpoint,game.locale.clone());assert!(result.is_err());let message=result.err().unwrap().to_string();assert!(message.contains("Portable consumer programme differs"));assert!(!message.contains("Old Widget Destruct preparation"));assert!(wrapper.attached());assert_eq!(serde_json::to_value(&game.state).unwrap(),before);assert!(!game.renderer.scene_batch_pending());
}

#[test]
fn fresh_factories_preserve_the_old_live_owner_and_historical_load_cannot_forge_target_class_provenance(){
 let project=native_project(SOURCE);let(mut game,handle)=warm(&project);handoff_zero_media_publication(&mut game);game.advance_dialogue().unwrap();game.step_until_interaction().unwrap();handoff_zero_media_publication(&mut game);let owner=game.widget_backend_owner(&handle).unwrap();let wrapper=game.renderer.widget_backend.wrapper(&owner).unwrap().upgrade().unwrap();let before=serde_json::to_value(&game.state).unwrap();let source=game.debug_state_source().unwrap();
 let fresh=game.fresh(crate::bevy_renderer::BevyRenderer::new(),32).unwrap();assert_ne!(fresh.instance_id(),game.instance_id());assert!(fresh.widget_classes().class(UUID).is_some());assert!(fresh.widget_instance_handle("one").is_none());assert!(wrapper.attached());assert!(game.accepts_debug_state_source(&source));let incoming=game.prepare_fresh_startup(crate::bevy_renderer::BevyRenderer::new(),32).unwrap();assert_ne!(incoming.snapshot().instance,game.instance_id());assert!(!incoming.state().vars.contains_key("initialized_count"));assert!(wrapper.attached());assert!(game.accepts_debug_state_source(&source));let historical=rvn_core::save::SaveData::from_state(&game.state,1,"not a class authority".into(),"main.rvn".into());let load=game.load_data(historical);assert!(load.is_err());assert!(load.err().unwrap().to_string().contains("Historical save cannot grant Widget class provenance"));assert!(game.accepts_debug_state_source(&source));assert_eq!(serde_json::to_value(&game.state).unwrap(),before);assert!(wrapper.attached());assert!(game.capture_ui_input_source("one").is_some());assert!(!game.renderer.scene_batch_pending());assert!(game.renderer.take_pending().is_empty());
}

fn history_game(source:&str)->(Project,Game,rvn_core::engine::WidgetInstanceHandle){
 let source=source.replace("label start\n\"Ready\"","label start\ntimer 5 => jump done\n\"Ready\"").replace("\"Observed\"\nreturn","\"Observed\"\n\"Third\"\nreturn");let project=native_project(&source);let(mut game,handle)=warm(&project);handoff_zero_media_publication(&mut game);for _ in 0..2{game.advance_dialogue().unwrap();game.step_until_interaction().unwrap();handoff_zero_media_publication(&mut game);}assert!(game.can_rollback());(project,game,handle)
}
fn set_old_history_guards(game:&mut Game){let mut rng=game.state.random;let draw=rng.integer(0,1000000).unwrap();game.state.vars.insert("shared".into(),Value::Int(777));game.state.vars.insert("expected_shared".into(),Value::Int(777));game.state.vars.insert("expected_draw".into(),Value::Int(draw));}
#[test]
fn real_rollback_retires_using_old_scope_and_rng_then_restores_historical_values_before_a_new_wrapper_life(){
 let(_project,mut game,handle)=history_game(&guarded_source());let expected=game.state.vars.clone();let rng=game.state.random;let owner=game.widget_backend_owner(&handle).unwrap();let backend=game.renderer.widget_backend.clone();let wrapper=backend.wrapper(&owner).unwrap().upgrade().unwrap();set_old_history_guards(&mut game);assert!(game.rollback());assert!(!wrapper.attached());assert!(backend.wrapper(&owner).is_none());assert_eq!(game.state.vars,expected);assert_eq!(game.state.random,rng);assert_eq!(game.state.vars["destructed_count"],Value::Int(0));pending(&game);assert!(!game.widget_materialization_waits_for_saved_operation());new_life(&mut game);assert_eq!(game.state.vars["initialized_count"],Value::Int(1));assert_eq!(game.state.vars["constructed_count"],Value::Int(2));
}
#[test]
fn real_rollback_destruct_failure_preserves_history_anchor_original_parent_rng_and_publication(){
 let source=SOURCE.replace("handler destruct(event){set destructed_count=destructed_count+1}","handler destruct(event){set destructed_count=destructed_count+1 local invalid_interface=\"unknown\" ui.open(invalid_interface,[],false,0)}");let(_project,mut game,handle)=history_game(&source);let owner=game.widget_backend_owner(&handle).unwrap();let wrapper=game.renderer.widget_backend.wrapper(&owner).unwrap().upgrade().unwrap();let source=game.debug_state_source().unwrap();let before=serde_json::to_value(&game.state).unwrap();assert!(!game.rollback());assert!(game.accepts_debug_state_source(&source));assert_eq!(serde_json::to_value(&game.state).unwrap(),before);assert!(game.can_rollback());assert!(wrapper.attached());assert!(game.capture_ui_input_source("one").is_some());assert!(!game.renderer.scene_batch_pending());assert!(game.renderer.take_pending().is_empty());
}
#[test]
fn actual_authenticated_source_rollback_uses_same_parent_and_history_fence_without_replaying_old_rng_on_target(){
 let(_project,mut game,handle)=history_game(&guarded_source());let owner=game.widget_backend_owner(&handle).unwrap();let wrapper=game.renderer.widget_backend.wrapper(&owner).unwrap().upgrade().unwrap();set_old_history_guards(&mut game);let token=game.preview_game_audio_rollback_source().unwrap().expect("actual recorded historical Source capability");let expected=game.audio_rollback_state_for_warming(&token).unwrap();assert!(expected.ui.screens.iter().any(|instance|instance.name=="one"&&instance.widget_class.as_ref().is_some_and(|binding|binding.viewport_attached)));let expected_rng=expected.random;let expected_vars=expected.vars.clone();assert!(game.accepts_audio_rollback_source(&token));assert!(game.rollback_from_audio_source(&token).unwrap());assert!(!wrapper.attached());assert_eq!(game.state.vars,expected_vars);assert_eq!(game.state.random,expected_rng);assert!(!game.accepts_audio_rollback_source(&token));pending(&game);new_life(&mut game);assert_eq!(game.state.vars["initialized_count"],Value::Int(1));assert_eq!(game.state.vars["constructed_count"],Value::Int(2));
}
#[test]
fn authenticated_source_rollback_destruct_refusal_does_not_pop_or_invalidate_its_original_source(){
 let source=SOURCE.replace("handler destruct(event){set destructed_count=destructed_count+1}","handler destruct(event){set destructed_count=destructed_count+1 local invalid_interface=\"unknown\" ui.open(invalid_interface,[],false,0)}");let(_project,mut game,handle)=history_game(&source);let token=game.preview_game_audio_rollback_source().unwrap().unwrap();let owner=game.widget_backend_owner(&handle).unwrap();let wrapper=game.renderer.widget_backend.wrapper(&owner).unwrap().upgrade().unwrap();let before=serde_json::to_value(&game.state).unwrap();let anchor=game.debug_state_source().unwrap();assert!(game.rollback_from_audio_source(&token).is_err());assert!(game.accepts_audio_rollback_source(&token));assert!(game.accepts_debug_state_source(&anchor));assert_eq!(serde_json::to_value(&game.state).unwrap(),before);assert!(wrapper.attached());assert!(!game.renderer.scene_batch_pending());assert!(game.renderer.take_pending().is_empty());
}

#[cfg(not(target_arch="wasm32"))]
#[path="widget_fresh_retirement_tests_0966.rs"]
mod fresh_0966;

#[path="widget_historical_load_tests_0967.rs"]
mod historical_0967;

fn retirement_speech_world(project:&Project,override_policy:Option<rvn_ui::accessibility::AccessibilitySettings>)->bevy::prelude::World{
 let mut world=bevy::prelude::World::new();world.insert_resource(crate::accessibility::Accessibility::default());
 let mut settings=crate::systems::settings_menu::Settings::default();settings.language="fr-CA".into();world.insert_resource(settings);
 let manager=rvn_core::persistent::PersistentDataManager::new(project.root.join("speech-player")).unwrap();let mut data=manager.load().unwrap();data.accessibility=override_policy;world.insert_resource(crate::resources::PersistentDataResource{manager,data});world
}
fn retirement_speech_source()->String{
 SOURCE.replace("handler destruct(event){set destructed_count=destructed_count+1}",r#"handler destruct(event){accessibility.configure({"self_voicing":true,"language":"fr-FR","speech_rate":0.75}) accessibility.speak("Old first") accessibility.configure({"self_voicing":false,"language":"de-DE","speech_rate":1.5}) accessibility.stop() accessibility.speak("Old second") set destructed_count=destructed_count+1}"#)
}
#[test]
fn save15_old_destruct_retains_each_command_policy_and_order_while_the_target_keeps_its_saved_settings(){
 let project=native_project(&retirement_speech_source());let(mut game,handle)=warm(&project);let checkpoint=save(&mut game,&project,None);let before=serde_json::to_value(&game.state).unwrap();let token=game.debug_state_source().unwrap();let owner=game.widget_backend_owner(&handle).unwrap();let wrapper=game.renderer.widget_backend.wrapper(&owner).unwrap().upgrade().unwrap();
 let prepared=game.prepare_save15_with_locale(&project.program(),checkpoint,game.locale.clone()).unwrap();let rows=&prepared.render_batch().retired_widget_speech;assert_eq!(rows.len(),3);assert!(prepared.render_batch().speech_requests.is_empty());assert!(!prepared.render_batch().state.accessibility.self_voicing);assert_eq!(rows[0].authored().language.as_deref(),Some("fr-FR"));assert_eq!(rows[0].authored().speech_rate,0.75);assert_eq!(rows[1].authored().language.as_deref(),Some("de-DE"));assert_eq!(rows[2].authored().speech_rate,1.5);assert!(matches!(rows[1].request(),rvn_ui::accessibility::SpeechRequest::Stop));
 let world=retirement_speech_world(&project,None);let speech=crate::accessibility::retired_speech_0975::PreparedRetiredSpeech::prepare(&world,rows).unwrap();assert!(wrapper.attached());assert!(game.accepts_debug_state_source(&token));assert_eq!(serde_json::to_value(&game.state).unwrap(),before);assert!(game.renderer.take_pending().is_empty());
 game.commit_prepared_save15(prepared).unwrap();assert!(!wrapper.attached());assert_eq!(serde_json::to_value(&game.state).unwrap()["accessibility"],before["accessibility"]);let commands=speech.commands();assert_eq!(commands.len(),3);let frozen:Vec<_>=commands.iter().map(|command|match command{crate::vn_command::VnCommand::RetiredWidgetSpeech(command)=>command,_=>panic!("expected actual frozen publication command")}).collect();assert_eq!(frozen[0].request(),&rvn_ui::accessibility::SpeechRequest::Speak("Old first".into()));assert_eq!(frozen[1].request(),&rvn_ui::accessibility::SpeechRequest::Stop);assert_eq!(frozen[2].request(),&rvn_ui::accessibility::SpeechRequest::Speak("Old second".into()));assert_eq!(frozen[0].settings().speech_rate,0.75);assert_eq!(frozen[2].settings().language.as_deref(),Some("de-DE"));assert!(!game.state.accessibility.self_voicing);
}
#[test]
fn frozen_old_speech_uses_the_actual_player_override_and_refuses_a_changed_host_policy_before_core_swap(){
 let project=native_project(&retirement_speech_source());let(mut game,handle)=warm(&project);let checkpoint=save(&mut game,&project,None);let prepared=game.prepare_save15_with_locale(&project.program(),checkpoint,game.locale.clone()).unwrap();let owner=game.widget_backend_owner(&handle).unwrap();let wrapper=game.renderer.widget_backend.wrapper(&owner).unwrap().upgrade().unwrap();let token=game.debug_state_source().unwrap();let before=serde_json::to_value(&game.state).unwrap();
 let player=rvn_ui::accessibility::AccessibilitySettings{self_voicing:true,speech_rate:1.8,language:Some("it-IT".into()),..Default::default()};let mut world=retirement_speech_world(&project,Some(player.clone()));let speech=crate::accessibility::retired_speech_0975::PreparedRetiredSpeech::prepare(&world,&prepared.render_batch().retired_widget_speech).unwrap();speech.verify(&world,&prepared.render_batch().retired_widget_speech).unwrap();world.resource_mut::<crate::systems::settings_menu::Settings>().language="en-US".into();assert!(speech.verify(&world,&prepared.render_batch().retired_widget_speech).is_err());assert!(wrapper.attached());assert!(game.accepts_debug_state_source(&token));assert_eq!(serde_json::to_value(&game.state).unwrap(),before);assert!(!game.renderer.scene_batch_pending());for command in speech.commands(){let crate::vn_command::VnCommand::RetiredWidgetSpeech(command)=command else{panic!()};assert_eq!(command.settings(),&player);}
}
#[test]
fn old_destruct_speech_budget_refusal_or_abandoned_load_guard_never_emits_or_retires_old_parents(){
 for overflow in [false,true]{let body=if overflow{format!("handler destruct(event){{accessibility.configure({{\"self_voicing\":true}}) {}}}",format!("accessibility.speak(\"{}\") ","x".repeat(16000)).repeat(5))}else{"handler destruct(event){accessibility.speak(\"never emitted on guard drop\")}".into()};let project=native_project(&SOURCE.replace("handler destruct(event){set destructed_count=destructed_count+1}",&body));let(mut game,handle)=warm(&project);let checkpoint=save(&mut game,&project,None);let owner=game.widget_backend_owner(&handle).unwrap();let wrapper=game.renderer.widget_backend.wrapper(&owner).unwrap().upgrade().unwrap();let before=serde_json::to_value(&game.state).unwrap();let token=game.debug_state_source().unwrap();let result=game.prepare_save15_with_locale(&project.program(),checkpoint,game.locale.clone());
 if overflow{let message=result.err().expect("over-budget old speech must refuse before swap").to_string();assert!(message.contains("64 KiB"),"{message}");}else{let prepared=result.unwrap();assert_eq!(prepared.render_batch().retired_widget_speech.len(),1);let guard=game.begin_prepared_save15_commit(prepared).unwrap();drop(guard);}
 assert!(wrapper.attached());assert!(game.accepts_debug_state_source(&token));assert_eq!(serde_json::to_value(&game.state).unwrap(),before);assert!(!game.renderer.scene_batch_pending());assert!(game.renderer.take_pending().is_empty());}
}

#[test]
fn old_speech_fallback_language_is_frozen_before_the_target_selected_language_changes(){
 let source=SOURCE.replace("handler destruct(event){set destructed_count=destructed_count+1}","handler destruct(event){accessibility.configure({\"self_voicing\":true,\"speech_rate\":0.75}) accessibility.speak(\"old locale\")}");let project=native_project(&source);let(mut game,_)=warm(&project);let old_language=game.locale.as_ref().expect("actual loader locale").current_lang().to_owned();let checkpoint=save(&mut game,&project,None);let prepared=game.prepare_save15_with_locale(&project.program(),checkpoint,game.locale.clone()).unwrap();let mut world=retirement_speech_world(&project,None);world.resource_mut::<crate::systems::settings_menu::Settings>().language=old_language.clone();let speech=crate::accessibility::retired_speech_0975::PreparedRetiredSpeech::prepare(&world,&prepared.render_batch().retired_widget_speech).unwrap();world.resource_mut::<crate::systems::settings_menu::Settings>().language="en-US".into();world.resource_mut::<crate::accessibility::Accessibility>().settings=prepared.render_batch().state.accessibility.clone();let commands=speech.commands();assert_eq!(commands.len(),1);let crate::vn_command::VnCommand::RetiredWidgetSpeech(command)=&commands[0]else{panic!()};assert_eq!(command.settings().language.as_deref(),Some(old_language.as_str()));assert_eq!(command.settings().speech_rate,0.75);assert!(command.settings().self_voicing);assert!(!world.resource::<crate::accessibility::Accessibility>().settings.self_voicing);
}

#[test]
fn accepted_old_speech_survives_a_second_real_save15_replacement_without_duplicate_destruct_or_source_grant(){
 let project=native_project(&retirement_speech_source());let(mut game,handle)=warm(&project);let saved=save(&mut game,&project,None);let owner=game.widget_backend_owner(&handle).unwrap();let wrapper=game.renderer.widget_backend.wrapper(&owner).unwrap().upgrade().unwrap();game.restore_save15(&project.program(),saved.clone()).unwrap();assert!(!wrapper.attached());let first_epoch=game.runtime_epoch();assert_eq!(game.renderer.scene_batch.as_ref().unwrap().retired_widget_speech.len(),3);game.restore_save15(&project.program(),saved).unwrap();assert!(game.runtime_epoch()>first_epoch);let rows=&game.renderer.scene_batch.as_ref().unwrap().retired_widget_speech;assert_eq!(rows.len(),3,"the already accepted old effects survive, while an unmaterialized target has no Destruct to duplicate");assert_eq!(rows[0].request(),&rvn_ui::accessibility::SpeechRequest::Speak("Old first".into()));assert!(game.capture_ui_input_source("one").is_none());assert!(!wrapper.attached());assert_eq!(game.state.vars["destructed_count"],Value::Int(0));
}


#[test]
fn actual_standalone_rollback_description_has_no_unpiloted_probe_and_dropping_preparation_keeps_original_owner(){
 let(_project,mut game,handle)=history_game(&guarded_source());
 set_old_history_guards(&mut game);
 let token=game.preview_game_audio_rollback_source().unwrap().unwrap();
 let expected=game.audio_rollback_state_for_warming(&token).unwrap();
 assert!(expected.game_tasks.is_some()&&expected.scene_execution.is_none(),"the actual Timer must select standalone audio restore");
 let owner=game.widget_backend_owner(&handle).unwrap();
 let backend=game.renderer.widget_backend.clone();
 let wrapper=backend.wrapper(&owner).unwrap().upgrade().unwrap();
 let source=game.debug_state_source().unwrap();
 let before=serde_json::to_value(&game.state).unwrap();
 assert!(!game.renderer.scene_batch_pending());assert!(game.renderer.take_pending().is_empty());
 let prepared=game.prepare_audio_rollback(&token).expect("synchronous target description must not create an unpiloted debugger suspension");
 assert_eq!(prepared.render_batch().state.vars,expected.vars);
 assert_eq!(prepared.render_batch().state.random,expected.random);
 assert!(prepared.render_batch().interfaces.iter().any(|view|view.name=="one"),"the actual admitted Screen must be evaluated, not omitted to bypass its computation");
 assert!(game.accepts_prepared_audio_rollback(&prepared));
 assert!(game.accepts_audio_rollback_source(&token));
 assert!(game.accepts_debug_state_source(&source));
 assert_eq!(serde_json::to_value(&game.state).unwrap(),before);
 assert!(wrapper.attached());assert!(std::sync::Arc::ptr_eq(&wrapper,&backend.wrapper(&owner).unwrap().upgrade().unwrap()));
 assert!(!game.renderer.scene_batch_pending());assert!(game.renderer.take_pending().is_empty());
 drop(prepared);
 assert!(game.accepts_audio_rollback_source(&token));assert!(game.accepts_debug_state_source(&source));
 assert_eq!(serde_json::to_value(&game.state).unwrap(),before);
 assert!(wrapper.attached());assert!(std::sync::Arc::ptr_eq(&wrapper,&backend.wrapper(&owner).unwrap().upgrade().unwrap()));
 assert!(!game.renderer.scene_batch_pending());assert!(game.renderer.take_pending().is_empty());
}
