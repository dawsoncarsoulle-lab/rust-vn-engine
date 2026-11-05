use super::*;
use bevy::ecs::system::RunSystemOnce;
use crate::resources::{CharacterRegistry,DialogueHistory,TypewriterConfig,TypewriterState,VnRenderState,ChoiceFocus,ImagemapState,VnState,ScriptErrorMessage};
use rvn_core::save::{SaveData,SaveManager};
const SOURCE:&str="scene label P\ntypewriter = true\n\"é🌟 longue\"\n\"returned\"\njump done\nscene label C\n\"child\"\nreturn\nlabel done\n\"done\"\n";

fn engine(source:&str)->VnEngine {
    let mut engine=rvn_core::Engine::from_program(rvn_parser::parse_program(source).unwrap(),crate::bevy_renderer::BevyRenderer::new(),32).unwrap();
    engine.enter_label("P").unwrap();engine.step_until_interaction().unwrap();VnEngine(engine)
}
fn app()->App {
    let mut app=App::new();
    app.insert_resource(engine(SOURCE)).insert_resource(State::new(VnState::Waiting))
        .init_resource::<NextState<VnState>>().insert_resource(Time::<()>::default())
        .init_resource::<crate::accessibility::Accessibility>().init_resource::<ScriptErrorMessage>()
        .init_resource::<VnRenderState>().init_resource::<ChoiceFocus>().init_resource::<ImagemapState>()
        .init_resource::<TypewriterState>().init_resource::<TypewriterConfig>().init_resource::<CharacterRegistry>()
        .init_resource::<DialogueHistory>().init_resource::<crate::scene_publication::ScenePresentation>()
        .add_event::<VnCommand>().add_event::<crate::vn_command::PlayerInput>()
        .add_plugins(DialogueRevealPlugin)
        .add_systems(Update,(crate::systems::typewriter_config_system,crate::systems::dialogue_system.after(crate::systems::typewriter_config_system)));
    app.world_mut().spawn((crate::components::DialogueText,Text::from_section("old",TextStyle::default())));
    app
}
fn publish(app:&mut App) { app.world_mut().run_system_once(crate::scene_publication::consume); }
fn send_current(app:&mut App) {
    let game=app.world().resource::<VnEngine>();
    let Some(rvn_core::Interaction::Dialogue{character,text})=game.0.displayed_interaction().unwrap() else{panic!("dialogue required")};
    let command=command(game,character,text);app.world_mut().send_event(command);
}
pub(crate) fn ready()->App {let mut app=app();publish(&mut app);app.update();assert!(app.world().resource::<DialogueRevealBinding>().source.is_some());app}
fn metrics(app:&App)->(String,usize,bool,u32,u32) {
    let typing=app.world().resource::<TypewriterState>();(typing.full_text.clone(),typing.visible_chars,typing.typing,typing.pause_remaining.to_bits(),typing.char_progress.to_bits())
}
fn state(app:&App)->serde_json::Value {serde_json::to_value(&app.world().resource::<VnEngine>().0.state).unwrap()}
fn observed(app:&App)->RevealCheckpoint {
    app.world().resource::<VnEngine>().0.state.scene_execution.as_ref().unwrap().display.as_ref().unwrap().reveal.clone().unwrap()
}
fn partial(app:&mut App) {
    let mut typing=app.world_mut().resource_mut::<TypewriterState>();
    typing.visible_chars=2;typing.typing=true;typing.pause_remaining=0.375;typing.char_progress=1.625;
}
fn configure_inputs(app:&mut App,directory:&std::path::Path) {
    use crate::resources::{PersistentDataResource,MenuState};
    app.insert_resource(PersistentDataResource{manager:rvn_core::PersistentDataManager::new(directory).unwrap(),data:Default::default()})
        .insert_resource(crate::project_paths::ProjectPaths::new(directory.into(),directory.join("assets"),directory.join("locales"),directory.join("theme.toml"),directory.into()))
        .init_resource::<MenuState>().init_resource::<crate::systems::save_menu::SaveMenuState>()
        .init_resource::<crate::systems::save_menu::SaveConfirmation>().init_resource::<crate::save_thumbnails::SaveThumbnails>()
        .init_resource::<crate::systems::debug_overlay::DebugOverlayState>().init_resource::<crate::systems::debug_overlay::DebugStepRequest>();
}

#[test]
fn reveal_pending_dialogue_survives_event_retention_until_real_approved_publication() {
    let mut app=app();send_current(&mut app);let original=state(&app);
    for _ in 0..5 {app.update();assert_eq!(metrics(&app).0,"");assert_eq!(app.world().resource::<DialogueRevealQueue>().commands.len(),1);assert_eq!(state(&app),original);}
    // First/Update publication uses the actual production plugin; no manual take.
    app.add_plugins(crate::scene_publication::ScenePublicationPlugin);app.update();
    assert_eq!(metrics(&app),("é🌟 longue".into(),0,true,0,0));
    assert!(app.world().resource::<DialogueRevealQueue>().commands.is_empty());
    assert_eq!(app.world().resource::<DialogueHistory>().lines.len(),1);
    assert_eq!(app.world().resource::<Events<PresentedDialogue>>().len(),1);
    assert_eq!(observed(&app).rendered_text,"é🌟 longue");
}

#[test]
fn reveal_invalid_current_checkpoint_keeps_queue_core_and_ecs_before_payload_publication() {
    let mut app=ready();partial(&mut app);flush_world(app.world_mut()).unwrap();
    let before_metrics=metrics(&app);let before=state(&app);
    let mut current=command(app.world().resource::<VnEngine>(),None,"é🌟 longue".into());
    let VnCommand::ScopedShowDialogue{checkpoint,..}=&mut current else{panic!()};
    let mut invalid=observed(&app);invalid.visible_chars=invalid.total_chars+1;*checkpoint=Some(invalid);
    app.world_mut().send_event(current);app.update();
    assert_eq!(metrics(&app),before_metrics);assert_eq!(state(&app),before);
    assert_eq!(app.world().resource::<DialogueRevealQueue>().commands.len(),1);
    assert!(app.world().resource::<VnEngine>().0.renderer.scene_frame_failed);
}

#[test]
fn reveal_stale_source_is_neutral_before_oversized_text_or_nonfinite_metrics() {
    let mut app=ready();let old=app.world().resource::<DialogueRevealBinding>().source.clone().unwrap();
    app.world_mut().resource_mut::<VnEngine>().0.advance_dialogue().unwrap();
    app.world_mut().resource_mut::<VnEngine>().0.step_until_interaction().unwrap();
    let original=state(&app);let presentation=metrics(&app);
    let invalid=RevealCheckpoint{rendered_text:"x".repeat(rvn_core::display_reveal::MAX_REVEAL_TEXT_BYTES+1),total_chars:u32::MAX,visible_chars:u32::MAX,typing:false,pause_remaining:f32::NAN,char_progress:f32::INFINITY};
    app.world_mut().send_event(VnCommand::ScopedShowDialogue{source:old,checkpoint:Some(invalid),character:None,text:"x".repeat(rvn_core::display_reveal::MAX_REVEAL_TEXT_BYTES+1)});
    app.update();assert_eq!(state(&app),original);assert_eq!(metrics(&app),presentation);
    assert!(app.world().resource::<DialogueRevealQueue>().commands.is_empty());
    assert!(!app.world().resource::<VnEngine>().0.renderer.scene_frame_failed);
}

#[test]
fn reveal_queue_overflow_preserves_admitted_prefix_and_reader_for_retry_without_partial_drain() {
    let mut app=app();send_current(&mut app);app.update();
    for _ in 0..MAX_PENDING_DIALOGUES {send_current(&mut app);}
    app.update();assert_eq!(app.world().resource::<DialogueRevealQueue>().commands.len(),1);
    assert!(app.world().resource::<VnEngine>().0.renderer.scene_frame_failed);
    assert!(app.world().resource::<ScriptErrorMessage>().0.contains("capacity"));
    // No admitted prefix is lost on a later drain while the failure latch remains.
    app.update();assert_eq!(app.world().resource::<DialogueRevealQueue>().commands.len(),1);
    assert_eq!(metrics(&app).0,"");
}

#[test]
fn reveal_load_rebinds_identical_dialogue_and_restores_unicode_pause_backlog_once() {
    let mut app=ready();partial(&mut app);flush_world(app.world_mut()).unwrap();
    let saved=SaveData::try_from_state(&app.world().resource::<VnEngine>().0.state,1,"partial".into(),"main.rvn".into()).unwrap();
    let old=app.world().resource::<DialogueRevealBinding>().source.clone().unwrap();
    app.world_mut().resource_mut::<VnEngine>().0.load_data(saved).unwrap();
    publish(&mut app);app.update();
    let binding=app.world().resource::<DialogueRevealBinding>();assert_ne!(binding.source.as_ref(),Some(&old));
    assert_eq!(metrics(&app),("é🌟 longue".into(),2,true,0.375f32.to_bits(),1.625f32.to_bits()));
    assert_eq!(app.world().resource::<DialogueHistory>().lines.len(),1);
    let before=metrics(&app);send_current(&mut app);app.update();assert_eq!(metrics(&app),before);
}

#[test]
fn reveal_existing_game_parent_source_survives_call_and_child_line_revokes_without_replay() {
    let mut app=ready();partial(&mut app);flush_world(app.world_mut()).unwrap();
    let original=app.world().resource::<DialogueRevealBinding>().source.clone().unwrap();
    app.world_mut().resource_mut::<VnEngine>().0.execute_timer_action("call C").unwrap();
    assert!(app.world().resource::<VnEngine>().0.accepts_display_reveal_source(&original));
    publish(&mut app);app.update();assert_eq!(metrics(&app).1,2);
    app.world_mut().resource_mut::<VnEngine>().0.step_until_interaction().unwrap();
    send_current(&mut app);publish(&mut app);app.update();
    assert_eq!(metrics(&app).0,"child");assert_eq!(metrics(&app).1,0);
    assert!(!app.world().resource::<VnEngine>().0.accepts_display_reveal_source(&original));
}

#[test]
fn reveal_typewriter_observation_and_config_skip_publish_pause_backlog_without_story_clock() {
    let mut app=ready();partial(&mut app);
    let original_random=app.world().resource::<VnEngine>().0.state.random;
    let original_pc=app.world().resource::<VnEngine>().0.state.pc;
    app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(0.125));
    app.world_mut().run_system_once(crate::systems::typewriter_system);
    let captured=observed(&app);assert_eq!(captured.visible_chars,2);assert_eq!(captured.pause_remaining,0.25);assert_eq!(captured.char_progress,1.625);
    app.world_mut().send_event(VnCommand::SetTypewriterConfig{speed_cps:0.0});
    app.world_mut().run_system_once(crate::systems::typewriter_config_system);
    assert!(!observed(&app).typing);assert_eq!(observed(&app).visible_chars,observed(&app).total_chars);
    assert_eq!(app.world().resource::<VnEngine>().0.state.random,original_random);assert_eq!(app.world().resource::<VnEngine>().0.state.pc,original_pc);
}

#[test]
fn reveal_reduced_motion_completes_actual_cursor_before_saving() {
    let mut app=ready();partial(&mut app);
    app.world_mut().resource_mut::<crate::accessibility::Accessibility>().settings.reduced_motion=true;
    app.world_mut().run_system_once(crate::systems::typewriter_system);
    assert!(!observed(&app).typing);assert_eq!(observed(&app).visible_chars,observed(&app).total_chars);
    let saved=SaveData::try_from_state(&app.world().resource::<VnEngine>().0.state,1,"reduced".into(),"main.rvn".into()).unwrap();
    assert!(!saved.scene_execution.unwrap().display.unwrap().reveal.unwrap().typing);
}

#[test]
fn reveal_independent_engine_same_epoch_and_text_cannot_adopt_old_rendered_metrics() {
    let mut app=ready();partial(&mut app);let old=app.world().resource::<DialogueRevealBinding>().source.clone().unwrap();
    let new=engine(SOURCE);assert_eq!(new.0.runtime_epoch(),old.runtime_epoch);assert_ne!(new.0.instance_id(),old.engine_instance);
    app.world_mut().insert_resource(new);let before=state(&app);assert!(!flush_world(app.world_mut()).unwrap());assert_eq!(state(&app),before);
    publish(&mut app);app.update();assert_eq!(metrics(&app).1,0);
    assert_ne!(app.world().resource::<DialogueRevealBinding>().source.as_ref(),Some(&old));
}

#[test]
fn reveal_new_engine_before_first_interaction_revokes_old_cursor_then_publishes_new_line() {
    let mut app=ready();partial(&mut app);
    let old=app.world().resource::<DialogueRevealBinding>().source.clone().unwrap();
    let mut replacement=rvn_core::Engine::from_program(rvn_parser::parse_program(SOURCE).unwrap(),crate::bevy_renderer::BevyRenderer::new(),32).unwrap();
    replacement.enter_label("P").unwrap();
    assert_eq!(replacement.runtime_epoch(),old.runtime_epoch);
    assert_ne!(replacement.instance_id(),old.engine_instance);
    assert!(replacement.state.scene_execution.as_ref().unwrap().display.is_none());
    app.world_mut().insert_resource(VnEngine(replacement));publish(&mut app);app.update();
    assert_eq!(metrics(&app),(String::new(),0,false,0,0));
    assert!(app.world().resource::<DialogueHistory>().lines.is_empty());
    assert!(!app.world().resource::<VnEngine>().0.accepts_display_reveal_source(&old));
    assert!(!app.world().resource::<VnEngine>().0.renderer.scene_frame_failed);
    app.world_mut().resource_mut::<VnEngine>().0.step_until_interaction().unwrap();
    publish(&mut app);app.update();
    assert_eq!(metrics(&app),("é🌟 longue".into(),0,true,0,0));
    assert_ne!(app.world().resource::<DialogueRevealBinding>().source.as_ref(),Some(&old));
    assert_eq!(app.world().resource::<DialogueHistory>().lines.len(),1);
}

#[test]
fn reveal_real_settings_language_remap_uses_saved_display_and_preserves_completed_unicode() {
    use crate::resources::MusicVolume;
    use rvn_core::locale::{LocaleManager,LocaleTable};use std::collections::HashMap;
    let tables=HashMap::from([("fr".into(),LocaleTable{lang:"fr".into(),strings:HashMap::new()}),("en".into(),LocaleTable{lang:"en".into(),strings:HashMap::from([("é🌟 longue".into(),"translated longer 🌟".into())])})]);
    let mut app=app();let mut localized=engine(SOURCE);
    localized.0.locale=Some(LocaleManager::from_tables(std::env::temp_dir(),"fr","fr",vec!["fr".into(),"en".into()],tables));
    app.world_mut().insert_resource(localized);publish(&mut app);app.update();
    app.world_mut().resource_mut::<TypewriterState>().skip();flush_world(app.world_mut()).unwrap();
    let source=app.world().resource::<DialogueRevealBinding>().source.clone().unwrap();
    let mut settings=crate::systems::settings_menu::Settings::default();settings.language="en".into();
    app.insert_resource(settings).init_resource::<MusicVolume>();
    app.world_mut().run_system_once(crate::systems::apply_settings_to_runtime_system);publish(&mut app);app.update();
    assert_eq!(metrics(&app).0,"translated longer 🌟");assert!(!metrics(&app).2);assert_eq!(metrics(&app).1,"translated longer 🌟".chars().count());
    assert_eq!(app.world().resource::<DialogueRevealBinding>().source.as_ref(),Some(&source));
    assert_eq!(observed(&app).rendered_text,"translated longer 🌟");
}

#[test]
fn reveal_language_refresh_then_ui_publication_keeps_completed_cursor_saveable() {
    assert_language_refresh_then_ui_publication_saveable(true);
}

#[test]
fn reveal_language_refresh_then_ui_publication_keeps_partial_unicode_clock_saveable() {
    assert_language_refresh_then_ui_publication_saveable(false);
}

fn assert_language_refresh_then_ui_publication_saveable(completed: bool) {
    use crate::resources::MusicVolume;
    use rvn_core::locale::{LocaleManager, LocaleTable};
    use rvn_core::ui::UiInput;
    use rvn_ui::programmable::ScreenEventKind;
    use std::collections::HashMap;
    let source = "screen controls(){return component(\"button\",\"button\",{\"text\":\"Click\",\"events\":{\"click\":\"selected\"}},[])}\nhandler selected(event){set observed=\"clicked\"}\ninit {set observed=\"\"}\nscene label P\ntypewriter = true\nui.open(\"controls\",[],false,0)\n\"é🌟 longue\"\n";
    let mut app = app();
    let mut localized = engine(source);
    localized.0.locale = Some(LocaleManager::from_tables(std::env::temp_dir(), "fr", "fr",
        vec!["fr".into(), "en".into()], HashMap::from([
            ("fr".into(), LocaleTable {lang:"fr".into(), strings:HashMap::new()}),
            ("en".into(), LocaleTable {lang:"en".into(), strings:HashMap::from([
                ("é🌟 longue".into(), "translated longer 🌟".into())])}),
        ])));
    app.insert_resource(localized);
    publish(&mut app); app.update();
    if completed { app.world_mut().resource_mut::<TypewriterState>().skip(); }
    else { partial(&mut app); }
    flush_world(app.world_mut()).unwrap();
    let authority = app.world().resource::<DialogueRevealBinding>().source.clone();
    let mut settings = crate::systems::settings_menu::Settings::default();
    settings.language = "en".into();
    app.insert_resource(settings).init_resource::<MusicVolume>();
    app.world_mut().run_system_once(crate::systems::apply_settings_to_runtime_system);
    publish(&mut app); app.update();
    assert_eq!(metrics(&app).0, "translated longer 🌟");
    let translated_metrics = metrics(&app);
    let history = app.world().resource::<DialogueHistory>().lines.len();
    app.world_mut().resource_mut::<VnEngine>().0.interface_event(UiInput {
        screen:"controls".into(), element:"button".into(), kind:ScreenEventKind::Click,
        value:None, key:None,
    }).unwrap();
    publish(&mut app); app.update();
    assert_eq!(metrics(&app), translated_metrics, "UI publication must retain the translated reveal");
    assert_eq!(app.world().resource::<DialogueRevealBinding>().source, authority);
    assert_eq!(app.world().resource::<DialogueHistory>().lines.len(), history);
    assert!(flush_world(app.world_mut()).unwrap());
    let saved = SaveData::try_from_state(&app.world().resource::<VnEngine>().0.state,
        1, "translated".into(), "main.rvn".into()).unwrap();
    let reveal = saved.scene_execution.unwrap().display.unwrap().reveal.unwrap();
    assert_eq!(reveal.rendered_text, "translated longer 🌟");
    if completed {
        assert_eq!(reveal.visible_chars, reveal.total_chars);
        assert!(!reveal.typing);
    } else {
        assert_eq!(reveal.visible_chars, 2);
        assert!(reveal.typing);
        assert_eq!(reveal.pause_remaining, 0.375);
        assert_eq!(reveal.char_progress, 1.625);
    }
    assert_eq!(saved.vars["observed"], rvn_core::save::SaveValue::Str("clicked".into()));
}

#[test]
fn reveal_rendered_checkpoint_flush_feeds_manual_quick_and_auto_writer_without_a_new_history_visit() {
    let directory=std::env::temp_dir().join(format!("rvn-reveal-flush-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    let manager=SaveManager::new(&directory,8).unwrap();let mut app=ready();partial(&mut app);
    let history=app.world().resource::<VnEngine>().0.history.len();flush_world(app.world_mut()).unwrap();
    let state=&app.world().resource::<VnEngine>().0.state;
    manager.save(state,1,"manual".into(),"main.rvn".into()).unwrap();manager.save_quicksave(state,"quick".into(),"main.rvn".into()).unwrap();manager.save_autosave(state,"auto".into(),"main.rvn".into()).unwrap();
    for saved in [manager.load(1).unwrap(),manager.load_quicksave().unwrap(),manager.load_autosave().unwrap()] {
        let reveal=saved.scene_execution.unwrap().display.unwrap().reveal.unwrap();assert_eq!(reveal.visible_chars,2);assert_eq!(reveal.pause_remaining,0.375);assert_eq!(reveal.char_progress,1.625);
    }
    assert_eq!(app.world().resource::<VnEngine>().0.history.len(),history);std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn reveal_actual_player_quicksave_and_advance_flush_before_writer_and_new_step_producer() {
    let directory=std::env::temp_dir().join(format!("rvn-reveal-input-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    let mut app=ready();partial(&mut app);let old=app.world().resource::<DialogueRevealBinding>().source.clone().unwrap();
    configure_inputs(&mut app,&directory);
    app.world_mut().send_event(crate::vn_command::PlayerInput::QuickSave);
    app.world_mut().send_event(crate::vn_command::PlayerInput::Advance);
    app.world_mut().run_system_once(crate::systems::player_input_system);
    let saved=SaveManager::new(&directory,8).unwrap().load_quicksave().unwrap();
    let checkpoint=saved.scene_execution.unwrap().display.unwrap().reveal.unwrap();assert_eq!(checkpoint.visible_chars,2);assert_eq!(checkpoint.pause_remaining,0.375);assert_eq!(checkpoint.char_progress,1.625);
    app.world_mut().run_system_once(crate::systems::stepping_system);publish(&mut app);app.update();
    assert_eq!(metrics(&app).0,"returned");assert_eq!(metrics(&app).1,0);
    assert_ne!(app.world().resource::<DialogueRevealBinding>().source.as_ref(),Some(&old));
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn reveal_actual_quickload_keeps_old_pixels_and_metrics_until_the_new_batch_is_approved() {
    let directory=std::env::temp_dir().join(format!("rvn-reveal-load-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    let mut app=ready();partial(&mut app);flush_world(app.world_mut()).unwrap();configure_inputs(&mut app,&directory);
    let manager=SaveManager::new(&directory,8).unwrap();manager.save_quicksave(&app.world().resource::<VnEngine>().0.state,"partial".into(),"main.rvn".into()).unwrap();
    app.world_mut().resource_mut::<TypewriterState>().visible_chars=5;
    let previous=metrics(&app);let old=app.world().resource::<DialogueRevealBinding>().source.clone().unwrap();
    app.world_mut().send_event(crate::vn_command::PlayerInput::QuickLoad);
    app.world_mut().run_system_once(crate::systems::player_input_system);
    assert_eq!(app.world().resource::<crate::systems::save_menu::SaveConfirmation>().pending,
        Some((0, crate::systems::save_menu::SaveMenuMode::Load)));
    assert!(!app.world().resource::<VnEngine>().0.renderer.scene_batch_pending());
    assert_eq!(metrics(&app),previous);
    // This unit harness supplies the confirmation result before exercising
    // the real approved PlayerInput branch; the native confirmation is separate.
    app.world_mut().resource_mut::<crate::systems::save_menu::SaveConfirmation>().pending=None;
    app.world_mut().resource_mut::<crate::systems::save_menu::SaveConfirmation>().approved=
        Some((0, crate::systems::save_menu::SaveMenuMode::Load));
    app.world_mut().send_event(crate::vn_command::PlayerInput::QuickLoad);
    app.world_mut().run_system_once(crate::systems::player_input_system);
    assert!(app.world().resource::<VnEngine>().0.renderer.scene_batch_pending());assert_eq!(metrics(&app),previous);
    let loaded=state(&app);assert!(!flush_world(app.world_mut()).unwrap());assert_eq!(state(&app),loaded);
    app.update();assert_eq!(metrics(&app),previous);assert_eq!(app.world().resource::<DialogueRevealBinding>().source.as_ref(),Some(&old));
    publish(&mut app);app.update();assert_eq!(metrics(&app),("é🌟 longue".into(),2,true,0.375f32.to_bits(),1.625f32.to_bits()));
    assert_ne!(app.world().resource::<DialogueRevealBinding>().source.as_ref(),Some(&old));
    assert_eq!(app.world().resource::<DialogueHistory>().lines.len(),1);std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn reveal_preflight_rejects_bad_current_checkpoint_before_consuming_batch_outputs_or_ecs() {
    let mut app=ready();partial(&mut app);flush_world(app.world_mut()).unwrap();
    let saved=SaveData::try_from_state(&app.world().resource::<VnEngine>().0.state,1,"restore".into(),"main.rvn".into()).unwrap();
    app.world_mut().resource_mut::<VnEngine>().0.load_data(saved).unwrap();
    {let mut game=app.world_mut().resource_mut::<VnEngine>();
        let reveal=game.0.state.scene_execution.as_mut().unwrap().display.as_mut().unwrap().reveal.as_mut().unwrap();reveal.visible_chars=reveal.total_chars+1;
        game.0.renderer.pending.push(VnCommand::ShowDialogue{character:None,text:"unconsumed raw".into()});
    }
    let before=state(&app);let previous=metrics(&app);
    let batch=serde_json::to_value(&app.world().resource::<VnEngine>().0.renderer.scene_batch.as_ref().unwrap().state).unwrap();
    let old_presentation=serde_json::to_value(&app.world().resource::<crate::scene_publication::ScenePresentation>().batch.as_ref().unwrap().state).unwrap();
    let events=app.world().resource::<Events<VnCommand>>().len();let history=app.world().resource::<DialogueHistory>().lines.len();
    let pending=format!("{:?}",app.world().resource::<VnEngine>().0.renderer.pending);
    let pending_count=app.world().resource::<VnEngine>().0.renderer.pending.len();
    assert!(pending_count>=1);
    publish(&mut app);
    assert_eq!(state(&app),before);assert_eq!(metrics(&app),previous);
    let game=app.world().resource::<VnEngine>();assert_eq!(game.0.renderer.pending.len(),pending_count);
    assert_eq!(format!("{:?}",game.0.renderer.pending),pending);assert!(game.0.renderer.scene_frame_failed);
    assert_eq!(serde_json::to_value(&game.0.renderer.scene_batch.as_ref().unwrap().state).unwrap(),batch);
    assert_eq!(serde_json::to_value(&app.world().resource::<crate::scene_publication::ScenePresentation>().batch.as_ref().unwrap().state).unwrap(),old_presentation);
    assert_eq!(app.world().resource::<Events<VnCommand>>().len(),events);assert_eq!(app.world().resource::<DialogueHistory>().lines.len(),history);
}


#[test]
fn retained_choice_dialogue_load_restores_saved_parent_text_without_child_locals_or_replay() {
    let source = "scene label P\nlocal probe=10\n\"parent [probe]\"\nchoice {\"A\"=>{set route=1} \"B\"=>{set route=2}}\n\"after\"\nscene label C\nlocal probe=200\n\"child [probe]\"\nreturn\n";
    let mut app=app();
    app.world_mut().insert_resource(engine(source));
    publish(&mut app);app.update();
    app.world_mut().resource_mut::<TypewriterState>().skip();
    flush_world(app.world_mut()).unwrap();
    {
        let mut game=app.world_mut().resource_mut::<VnEngine>();
        game.0.advance_dialogue().unwrap();
        game.0.step_until_interaction().unwrap();
        game.0.publish_current_scene_interaction().unwrap();
    }
    publish(&mut app);app.update();
    let saved=SaveData::try_from_state(&app.world().resource::<VnEngine>().0.state,1,"choice".into(),"main.rvn".into()).unwrap();
    {
        let mut game=app.world_mut().resource_mut::<VnEngine>();
        game.0.execute_timer_action("call C").unwrap();
        game.0.step_until_interaction().unwrap();
    }
    publish(&mut app);app.update();
    assert_eq!(metrics(&app).0,"child 200");
    app.world_mut().resource_mut::<VnEngine>().0.load_data(saved).unwrap();
    let before=state(&app);
    publish(&mut app);app.update();
    assert_eq!(metrics(&app).0,"parent 10","restored choice must replace the foreign child dialogue");
    assert!(!metrics(&app).2,"the previous completed line must not restart typing");
    let world=app.world_mut();
    let displayed=world.query_filtered::<&Text,With<crate::components::DialogueText>>().single(world);
    assert_eq!(displayed.sections.iter().map(|section|section.value.as_str()).collect::<String>(),"parent 10");
    assert_eq!(state(&app),before,"presenting the retained line must not replay script or mutate the saved state");
    assert!(matches!(app.world().resource::<VnEngine>().0.displayed_interaction().unwrap(),Some(rvn_core::Interaction::Choice{options})if options==vec!["A","B"]));
    assert!(!flush_world(app.world_mut()).unwrap(),"a retained backdrop is not an active dialogue reveal checkpoint");
}

#[test]
fn retained_choice_dialogue_new_story_without_previous_line_clears_foreign_pixels() {
    let mut app=app();
    app.world_mut().insert_resource(engine("scene label P\n\"foreign line\"\n"));
    publish(&mut app);app.update();
    assert_eq!(metrics(&app).0,"foreign line");
    { let world=app.world_mut(); let text=world.query_filtered::<&Text,With<crate::components::DialogueText>>().single(world); assert_eq!(text.sections.iter().map(|section|section.value.as_str()).collect::<String>(),"foreign line"); }
    app.world_mut().insert_resource(engine("scene label P\nchoice {\"A\"=>{set route=1} \"B\"=>{set route=2}}\n\"after\"\n"));
    publish(&mut app);app.update();
    assert_eq!(metrics(&app).0,"");
    let world=app.world_mut();
    let displayed=world.query_filtered::<&Text,With<crate::components::DialogueText>>().single(world);
    assert_eq!(displayed.sections.iter().map(|section|section.value.as_str()).collect::<String>(),"","a first choice with no prior dialogue must not retain the previous engine's text");
    assert!(app.world().resource::<DialogueRevealBinding>().source.is_none());
    assert!(!app.world().resource::<VnEngine>().0.renderer.scene_frame_failed);
}
