use super::*;
use bevy::audio::Decodable;
use std::sync::Arc;

const STORY:&str="scene label P\nsfx.play(\"same.wav\")\n\"é🌟 parent\"\n\"second\"\nreturn";
fn wave()->Vec<u8> {
    let samples:[i16;8]=[100,200,300,400,500,600,700,800];let mut bytes=Vec::new();
    bytes.extend(b"RIFF");bytes.extend(52u32.to_le_bytes());bytes.extend(b"WAVEfmt ");bytes.extend(16u32.to_le_bytes());
    bytes.extend(1u16.to_le_bytes());bytes.extend(2u16.to_le_bytes());bytes.extend(48_000u32.to_le_bytes());bytes.extend(192_000u32.to_le_bytes());
    bytes.extend(4u16.to_le_bytes());bytes.extend(16u16.to_le_bytes());bytes.extend(b"data");bytes.extend(16u32.to_le_bytes());
    for sample in samples {bytes.extend(sample.to_le_bytes());}bytes
}
fn engine()->VnEngine {
    let mut engine=rvn_core::Engine::from_program(rvn_parser::parse_program(STORY).unwrap(),crate::bevy_renderer::BevyRenderer::new(),8).unwrap();
    engine.step_until_interaction().unwrap();VnEngine(engine)
}
fn audio_assets(app:&mut App) {
    let root=bevy::asset::io::memory::Dir::default();root.insert_asset(std::path::Path::new("sfx/same.wav"),wave());
    let reader=bevy::asset::io::memory::MemoryAssetReader{root};
    app.register_asset_source(bevy::asset::io::AssetSourceId::Default,bevy::asset::io::AssetSource::build().with_reader(move||Box::new(reader.clone())));
    app.add_plugins((MinimalPlugins,bevy::asset::AssetPlugin{watch_for_changes_override:Some(false),..default()}));
    app.init_asset::<AudioSource>().init_asset_loader::<bevy::audio::AudioLoader>().init_asset::<crate::checkpoint_pcm::CheckpointPcm>();
}
fn audio_consumer_resources(app:&mut App) {
    app.init_resource::<crate::accessibility::Accessibility>().init_resource::<crate::resources::MusicEntity>()
        .init_resource::<crate::resources::MusicAssetRegistry>().init_resource::<crate::resources::MusicPlaybackState>()
        .init_resource::<crate::resources::MusicVolume>().init_resource::<crate::scene_publication::ScenePresentation>()
        .init_resource::<crate::scene_audio::AudioFeedbackQueue>().init_resource::<crate::display_reveal::DialogueRevealBinding>();
}
fn app()->App {
    let mut app=App::new();audio_assets(&mut app);
    app.insert_resource(engine()).insert_resource(State::new(VnState::Waiting)).init_resource::<NextState<VnState>>()
        .init_resource::<crate::resources::VnRenderState>().init_resource::<crate::resources::ChoiceFocus>()
        .init_resource::<crate::resources::ImagemapState>().init_resource::<crate::resources::TypewriterState>()
        .init_resource::<crate::accessibility::Accessibility>().init_resource::<crate::resources::ScriptErrorMessage>()
        .init_resource::<crate::resources::MusicEntity>().init_resource::<crate::resources::MusicAssetRegistry>()
        .init_resource::<crate::resources::MusicPlaybackState>().init_resource::<crate::resources::MusicVolume>()
        .init_resource::<crate::systems::settings_menu::Settings>().init_resource::<crate::scene_publication::ScenePresentation>()
        .init_resource::<crate::scene_audio::AudioFeedbackQueue>().init_resource::<crate::display_reveal::DialogueRevealBinding>()
        .init_resource::<CheckpointRequests>().add_event::<crate::vn_command::VnCommand>().add_event::<crate::vn_command::PlayerInput>();app
}
fn fill(world:&mut World,altered:bool) {
    let mut bytes=wave();if altered {*bytes.last_mut().unwrap()^=1;}
    let handle=world.resource::<AssetServer>().load::<AudioSource>("sfx/same.wav");
    world.resource_mut::<Assets<AudioSource>>().insert(handle.id(),AudioSource{bytes:Arc::from(bytes)});
}
fn publish(world:&mut World) {
    crate::scene_publication::consume(world);
    assert!(!world.resource::<VnEngine>().0.renderer.scene_frame_failed,"{}",world.resource::<crate::resources::ScriptErrorMessage>().0);
    assert!(!world.resource::<VnEngine>().0.renderer.scene_batch_pending());
}
fn binding(world:&mut World) {
    let engine=&world.resource::<VnEngine>().0;
    let Some(rvn_core::Interaction::Dialogue{text,..})=engine.displayed_interaction().unwrap() else{panic!("dialogue fixture");};
    let source=engine.display_reveal_source().unwrap();
    world.insert_resource(crate::display_reveal::DialogueRevealBinding{source:Some(source),text:text.clone()});
    let mut typing=world.resource_mut::<crate::resources::TypewriterState>();typing.full_text=text;typing.visible_chars=1;typing.typing=true;typing.pause_remaining=0.125;typing.char_progress=0.375;
    drop(typing);crate::display_reveal::flush_world(world).unwrap();
}
fn saved()->SaveData {
    let mut app=app();let world=app.world_mut();fill(world,false);publish(world);binding(world);
    let view=world.resource::<VnEngine>().0.audio_playback_views().unwrap().remove(0);
    let (reader,_)=crate::checkpoint_sources::one_shot_source(world,&view.token.target).unwrap();let mut decoder=reader.decoder();
    assert_eq!([decoder.next(),decoder.next(),decoder.next()],[Some(100),Some(200),Some(300)]);
    SaveData::try_from_snapshot(&world.resource::<VnEngine>().0.snapshot_for_save().unwrap(),1,"exact".into(),"main.rvn".into()).unwrap()
}
fn clear_pending(world:&mut World) {world.resource_mut::<VnEngine>().0.renderer.scene_batch.take();}
fn wire(world:&World)->serde_json::Value {
    let engine=&world.resource::<VnEngine>().0;
    serde_json::json!({"state":engine.state,"history":engine.history.entries().iter().map(|entry|(&entry.state,&entry.display)).collect::<Vec<_>>(),
        "epoch":engine.runtime_epoch(),"interface":engine.interface_epoch(),"pending":format!("{:?}",engine.renderer.scene_batch),"outputs":format!("{:?}",engine.renderer.pending),"instance":engine.instance_id()})
}
fn queue_wire(world:&World)->String {format!("{:?}",world.resource::<CheckpointRequests>().requests)}
fn enqueue_load(world:&mut World,data:&SaveData) {
    world.resource_scope(|world,mut requests:Mut<CheckpointRequests>| {
        assert!(defer_load(world.resource::<VnEngine>(),Some(&mut requests),data,None,true).unwrap());
    });
}
fn directory(world:&mut World)->std::path::PathBuf {
    let directory=std::env::temp_dir().join(format!("rvn-checkpoint-request-{}-{}",std::process::id(),world.resource::<VnEngine>().0.instance_id()));
    let _=std::fs::remove_dir_all(&directory);std::fs::create_dir_all(&directory).unwrap();
    world.insert_resource(crate::project_paths::ProjectPaths::new(directory.clone(),directory.join("assets"),directory.join("locales"),directory.join("theme.toml"),directory.clone()));directory
}

#[test]
fn checkpoint_load_waits_for_actual_asset_metadata_without_taking_pending_core_or_touching_old_ecs_then_commits_once() {
    let data=saved();let mut app=app();let world=app.world_mut();clear_pending(world);let marker=world.spawn(Name::new("old pixels")).id();enqueue_load(world,&data);
    let before=wire(world);let queue=queue_wire(world);let source=world.resource::<VnEngine>().0.audio_request_source().unwrap();let epoch=world.resource::<VnEngine>().0.runtime_epoch();
    process(world);assert_eq!(wire(world),before);assert_eq!(queue_wire(world),queue);assert!(world.get_entity(marker).is_some());assert!(world.resource::<VnEngine>().0.accepts_audio_request_source(&source));
    fill(world,false);process(world);assert!(!world.resource::<CheckpointRequests>().blocked());assert_eq!(world.resource::<VnEngine>().0.runtime_epoch(),epoch+1);
    assert!(world.resource::<VnEngine>().0.history.is_empty());assert!(world.resource::<VnEngine>().0.renderer.scene_batch_pending());assert!(world.get_entity(marker).is_some());
    let committed=wire(world);process(world);assert_eq!(wire(world),committed);
}

#[test]
fn checkpoint_load_actual_digest_refusal_keeps_request_history_source_epochs_and_publication_exact_without_retry_loop() {
    let data=saved();let mut app=app();let world=app.world_mut();clear_pending(world);fill(world,true);enqueue_load(world,&data);
    let before=wire(world);let queue=queue_wire(world);let source=world.resource::<VnEngine>().0.audio_request_source().unwrap();
    process(world);assert_eq!(wire(world),before);assert_eq!(queue_wire(world),queue);assert!(world.resource::<CheckpointRequests>().failed.is_some());
    assert!(world.resource::<VnEngine>().0.accepts_audio_request_source(&source));
    process_with_warmer(world,|_,_|panic!("failed requests must not auto retry"));assert_eq!(wire(world),before);assert_eq!(queue_wire(world),queue);
}

#[test]
fn checkpoint_request_revocation_precedes_even_corrupt_payload_and_cold_decoder_side_effects() {
    let mut data=saved();let mut app=app();let world=app.world_mut();clear_pending(world);enqueue_load(world,&data);
    data.scene_execution.as_mut().unwrap().resources.tasks[0].audio.as_mut().unwrap().checkpoint.as_mut().unwrap().sample_rate=0;
    if let Operation::Load{data:queued,..}=&mut world.resource_mut::<CheckpointRequests>().requests[0].operation {*queued=data;}
    let replacement=engine();assert_eq!(replacement.0.runtime_epoch(),world.resource::<VnEngine>().0.runtime_epoch());world.insert_resource(replacement);
    let before=wire(world);process_with_warmer(world,|_,_|panic!("stale request must not validate payload or load assets"));
    assert_eq!(wire(world),before);assert!(!world.resource::<CheckpointRequests>().blocked());
}

#[test]
fn checkpoint_rollback_selects_facade_previous_visit_before_cold_admission_and_never_pops_during_deferral() {
    let mut app=app();let world=app.world_mut();fill(world,false);publish(world);binding(world);
    {let mut engine=world.resource_mut::<VnEngine>();engine.0.advance_dialogue().unwrap();engine.0.step_until_interaction().unwrap();}publish(world);binding(world);
    let earlier=serde_json::to_value(&world.resource::<VnEngine>().0.history.entries()[0].state).unwrap();
    let previous_pc=world.resource::<VnEngine>().0.history.entries()[1].state.pc;
    {let mut engine=world.resource_mut::<VnEngine>();let current=engine.0.state.clone();engine.0.history.push(current,Some(rvn_core::rollback::HistoryDisplay::Dialogue{character:None,text:"second".into()}));}
    let engine=&world.resource::<VnEngine>().0;assert_eq!(engine.history.len(),3);let first_pc=previous_pc;
    let (source,preview)=engine.preview_game_audio_rollback().unwrap().unwrap();assert_eq!(preview.pc,first_pc);
    world.resource_scope(|world,mut queue:Mut<CheckpointRequests>|{assert!(defer_rollback(world.resource::<VnEngine>(),Some(&mut queue)).unwrap());});
    let before=wire(world);let queue=queue_wire(world);process_with_warmer(world,|_,_|Ok(false));assert_eq!(wire(world),before);assert_eq!(queue_wire(world),queue);
    assert!(world.resource::<VnEngine>().0.accepts_audio_rollback_source(&source));
    process(world);assert_eq!(world.resource::<VnEngine>().0.state.pc,first_pc);assert_eq!(world.resource::<VnEngine>().0.history.len(),1);
    assert_eq!(serde_json::to_value(&world.resource::<VnEngine>().0.history.entries()[0].state).unwrap(),earlier);assert!(!world.resource::<CheckpointRequests>().blocked());
}

#[test]
fn checkpoint_rollback_hash_refusal_preserves_both_exact_visits_and_current_reveal_association() {
    let mut app=app();let world=app.world_mut();fill(world,false);publish(world);binding(world);
    {let mut engine=world.resource_mut::<VnEngine>();engine.0.advance_dialogue().unwrap();engine.0.step_until_interaction().unwrap();}publish(world);binding(world);
    {let mut engine=world.resource_mut::<VnEngine>();let current=engine.0.state.clone();engine.0.history.push(current,Some(rvn_core::rollback::HistoryDisplay::Dialogue{character:None,text:"second".into()}));}
    world.resource_scope(|world,mut queue:Mut<CheckpointRequests>|{assert!(defer_rollback(world.resource::<VnEngine>(),Some(&mut queue)).unwrap());});
    fill(world,true);let before=wire(world);let queue=queue_wire(world);let request_source=world.resource::<VnEngine>().0.audio_request_source().unwrap();let rollback_source=world.resource::<VnEngine>().0.preview_game_audio_rollback().unwrap().unwrap().0;
    process(world);assert_eq!(wire(world),before);assert_eq!(queue_wire(world),queue);assert_eq!(world.resource::<VnEngine>().0.history.len(),3);
    assert_eq!(world.resource::<VnEngine>().0.audio_request_source().unwrap(),request_source);assert_eq!(world.resource::<VnEngine>().0.preview_game_audio_rollback().unwrap().unwrap().0,rollback_source);
}

#[test]
fn checkpoint_autosave_waits_for_source_ready_and_the_new_display_then_uses_a_fresh_real_cursor() {
    let mut app=app();let world=app.world_mut();let directory=directory(world);let manager=manager(world).unwrap();
    world.resource_scope(|world,mut queue:Mut<CheckpointRequests>|{assert!(defer_save(world.resource::<VnEngine>(),Some(&mut queue),SaveDestination::Auto,"auto".into(),"main.rvn".into(),None).unwrap());});
    let before=wire(world);process(world);assert_eq!(wire(world),before);assert!(!manager.autosave_exists());
    fill(world,false);publish(world);process(world);assert!(!manager.autosave_exists());assert!(world.resource::<CheckpointRequests>().blocked());
    binding(world);process(world);let data=manager.load_autosave().unwrap();assert_eq!(data.format_version,11);
    let display=data.scene_execution.as_ref().unwrap().display.as_ref().unwrap();assert_eq!(display.reveal.as_ref().unwrap().visible_chars,1);
    assert!(data.scene_execution.as_ref().unwrap().resources.tasks[0].audio.as_ref().unwrap().checkpoint.is_some());assert!(!world.resource::<CheckpointRequests>().blocked());
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn checkpoint_all_actual_writer_destinations_capture_pcm_instead_of_accepting_a_copied_saved_some() {
    let mut app=app();let world=app.world_mut();let directory=directory(world);fill(world,false);publish(world);binding(world);
    let token=world.resource::<VnEngine>().0.audio_playback_views().unwrap()[0].token.clone();let (reader,_)=crate::checkpoint_sources::one_shot_source(world,&token.target).unwrap();let mut decoder=reader.decoder();
    assert_eq!([decoder.next(),decoder.next(),decoder.next()],[Some(100),Some(200),Some(300)]);
    let manager=manager(world).unwrap();assert!(manager.save_autosave(&world.resource::<VnEngine>().0.state,"raw".into(),"main.rvn".into()).is_err());
    for destination in [SaveDestination::Manual(1),SaveDestination::Quick,SaveDestination::Auto] {
        world.resource_scope(|world,mut queue:Mut<CheckpointRequests>|{assert!(defer_save(world.resource::<VnEngine>(),Some(&mut queue),destination.clone(),"captured".into(),"main.rvn".into(),None).unwrap());});
        process(world);let data=read_destination(&manager,&destination).unwrap();let checkpoint=data.scene_execution.unwrap().resources.tasks[0].audio.clone().unwrap().checkpoint.unwrap();
        assert_eq!((checkpoint.next_sample,checkpoint.next_channel),(3,1));assert!(!world.resource::<CheckpointRequests>().blocked());
    }
    assert_eq!(decoder.next(),Some(400));std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn checkpoint_delayed_overwrite_refuses_an_external_file_change_without_rewriting_it_or_core_history() {
    let mut app=app();let world=app.world_mut();let directory=directory(world);
    world.resource_scope(|world,mut queue:Mut<CheckpointRequests>|{assert!(defer_save(world.resource::<VnEngine>(),Some(&mut queue),SaveDestination::Manual(1),"new".into(),"main.rvn".into(),None).unwrap());});
    let manager=manager(world).unwrap();let old=rvn_core::Engine::new(rvn_parser::parse("\"external\"").unwrap(),rvn_core::TerminalRenderer,8).unwrap();manager.save(&old.state,1,"external".into(),"old.rvn".into()).unwrap();
    let bytes=std::fs::read(directory.join("slot_01.json")).unwrap();fill(world,false);publish(world);binding(world);let before=wire(world);let queue=queue_wire(world);
    process(world);assert_eq!(std::fs::read(directory.join("slot_01.json")).unwrap(),bytes);assert_eq!(wire(world),before);assert_eq!(queue_wire(world),queue);
    assert!(world.resource::<CheckpointRequests>().failed.is_some());std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn checkpoint_request_bounds_refuse_the_new_suffix_without_consuming_any_accepted_request() {
    let data=saved();let mut app=app();let world=app.world_mut();clear_pending(world);
    for _ in 0..MAX_REQUESTS {enqueue_load(world,&data);}
    let before=queue_wire(world);let error=world.resource_scope(|world,mut queue:Mut<CheckpointRequests>|defer_load(world.resource::<VnEngine>(),Some(&mut queue),&data,None,true)).unwrap_err();
    assert!(error.contains("capacity"));assert_eq!(queue_wire(world),before);assert_eq!(world.resource::<CheckpointRequests>().requests.len(),MAX_REQUESTS);
    world.resource_mut::<CheckpointRequests>().cancel();assert!(!world.resource::<CheckpointRequests>().blocked());
}

#[test]
fn checkpoint_pending_request_blocks_both_actual_game_and_host_clock_routes_and_resumes_only_after_acceptance() {
    let data=saved();let mut app=app();app.add_plugins(CheckpointRequestsPlugin);let world=app.world_mut();clear_pending(world);enqueue_load(world,&data);
    let before=wire(world);assert!(!crate::scene_publication::game_active(world));assert!(!crate::scene_publication::host_active(world));
    app.update();assert_eq!(wire(app.world()),before);assert!(app.world().resource::<CheckpointRequests>().blocked());
    fill(app.world_mut(),false);app.update();assert!(!app.world().resource::<CheckpointRequests>().blocked());assert!(app.world().resource::<VnEngine>().0.renderer.scene_batch_pending());
}

#[test]
fn checkpoint_load_invalid_user_gain_refuses_before_decoder_or_core_restore_and_keeps_both_history_capabilities() {
    let data=saved();let mut app=app();let world=app.world_mut();clear_pending(world);fill(world,false);enqueue_load(world,&data);
    world.resource_mut::<crate::systems::settings_menu::Settings>().sfx_volume=f32::NAN;
    let before=wire(world);let source=world.resource::<VnEngine>().0.audio_request_source().unwrap();let queue=queue_wire(world);
    process(world);assert_eq!(wire(world),before);assert_eq!(queue_wire(world),queue);assert_eq!(world.resource::<VnEngine>().0.audio_request_source().unwrap(),source);
    assert!(world.resource::<CheckpointRequests>().failed.as_ref().unwrap().contains("gain"));
}

#[test]
fn checkpoint_save_never_flushes_old_display_metrics_under_a_new_core_line_while_waiting_for_approved_rebind() {
    let mut app=app();let world=app.world_mut();let directory=directory(world);fill(world,false);publish(world);binding(world);
    let old=world.resource::<crate::display_reveal::DialogueRevealBinding>().source.clone();
    {let mut engine=world.resource_mut::<VnEngine>();engine.0.advance_dialogue().unwrap();engine.0.step_until_interaction().unwrap();}publish(world);
    assert_ne!(world.resource::<VnEngine>().0.display_reveal_source(),old);
    world.resource_scope(|world,mut queue:Mut<CheckpointRequests>|{assert!(defer_save(world.resource::<VnEngine>(),Some(&mut queue),SaveDestination::Auto,"new line".into(),"main.rvn".into(),None).unwrap());});
    let before=wire(world);process(world);assert_eq!(wire(world),before);assert!(world.resource::<CheckpointRequests>().blocked());assert!(!manager(world).unwrap().autosave_exists());
    binding(world);process(world);let saved=manager(world).unwrap().load_autosave().unwrap();let reveal=saved.scene_execution.unwrap().display.unwrap().reveal.unwrap();
    assert_eq!(reveal.rendered_text,"second");assert_eq!(reveal.visible_chars,1);assert!(!world.resource::<CheckpointRequests>().blocked());std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn checkpoint_actual_stepping_requests_a_modern_background_autosave_before_source_and_dialogue_publication_instead_of_raw_writing() {
    use bevy::ecs::system::RunSystemOnce;
    let mut app=app();let world=app.world_mut();let directory=directory(world);
    let source="init{sfx.play(\"same.wav\")}\nscene label P\nscene \"room.png\"\n\"new scene\"";
    let engine=rvn_core::Engine::from_program(rvn_parser::parse_program(source).unwrap(),crate::bevy_renderer::BevyRenderer::new(),8).unwrap();world.insert_resource(VnEngine(engine));
    world.init_resource::<crate::systems::debug_overlay::DebugOverlayState>();world.init_resource::<crate::systems::debug_overlay::DebugStepRequest>();world.init_resource::<crate::save_thumbnails::SaveThumbnails>();
    let persistent=rvn_core::persistent::PersistentDataManager::new(&directory).unwrap();let data=persistent.load().unwrap();world.insert_resource(crate::resources::PersistentDataResource{manager:persistent,data});
    world.run_system_once(crate::systems::stepping_system);
    assert_eq!(world.resource::<CheckpointRequests>().requests.len(),1);assert!(matches!(&world.resource::<CheckpointRequests>().requests[0].operation,Operation::Save{destination:SaveDestination::Auto,..}));
    assert!(world.resource::<VnEngine>().0.renderer.scene_batch_pending());assert!(!manager(world).unwrap().autosave_exists());
    assert!(world.resource::<VnEngine>().0.renderer.pending.iter().any(|command|matches!(command,crate::vn_command::VnCommand::ScopedShowDialogue{..})));
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn checkpoint_admitted_load_retains_the_exact_request_while_primary_focus_or_accessibility_blocks_owner_commit() {
    let data=saved();let mut app=app();let world=app.world_mut();clear_pending(world);fill(world,false);enqueue_load(world,&data);
    let window=world.spawn((Window{focused:false,..default()},bevy::window::PrimaryWindow)).id();let before=wire(world);let queue=queue_wire(world);
    process_with_warmer(world,|_,_|panic!("blocked owner must not run its commit preflight"));assert_eq!(wire(world),before);assert_eq!(queue_wire(world),queue);
    world.get_mut::<Window>(window).unwrap().focused=true;world.resource_mut::<crate::accessibility::Accessibility>().blocked=true;
    process_with_warmer(world,|_,_|panic!("blocked accessibility must retain the request"));assert_eq!(wire(world),before);assert_eq!(queue_wire(world),queue);
    world.resource_mut::<crate::accessibility::Accessibility>().blocked=false;process(world);assert!(!world.resource::<CheckpointRequests>().blocked());assert_eq!(world.resource::<VnEngine>().0.runtime_epoch(),1);
}

#[test]
fn checkpoint_actual_source_menu_dispatch_retains_produced_receipts_beyond_event_retention_until_the_exact_request_is_released() {
    let data=saved();let mut harness=crate::source_menus::confirmation_tests::application(crate::systems::save_menu::SaveMenuMode::Save);
    harness.app.init_resource::<CheckpointRequests>();
    harness.app.world_mut().resource_scope(|world,mut queue:Mut<CheckpointRequests>|{
        assert!(defer_load(world.resource::<VnEngine>(),Some(&mut queue),&data,None,true).unwrap());
    });
    let before=std::fs::read(harness.directory.join("slot_01.json")).unwrap();
    let receipt=harness.click();assert!(receipt.identity>0);
    assert!(harness.app.world().resource::<crate::systems::save_menu::SaveConfirmation>().active());
    assert_eq!(std::fs::read(harness.directory.join("slot_01.json")).unwrap(),before);
    for _ in 0..5 {harness.app.update();}
    assert!(harness.app.world().resource::<crate::systems::save_menu::SaveConfirmation>().active());
    assert_eq!(std::fs::read(harness.directory.join("slot_01.json")).unwrap(),before);
    harness.app.world_mut().resource_mut::<CheckpointRequests>().cancel();harness.app.update();
    assert!(!harness.app.world().resource::<crate::systems::save_menu::SaveConfirmation>().active());
    let written=std::fs::read(harness.directory.join("slot_01.json")).unwrap();assert_ne!(written,before);
    harness.app.update();assert_eq!(std::fs::read(harness.directory.join("slot_01.json")).unwrap(),written);
}

fn modern_menu_engine(double:bool)->VnEngine {modern_menu_engine_audio(double,false)}
fn modern_menu_engine_audio(double:bool,audio:bool)->VnEngine {
    let audio=if audio {r#"sfx.play("same.wav")"#}else{""};
    let extra=if double {"menu.execute(event[\"data\"][\"request\"])"}else{""};
    let source=format!(r#"handler approve(event){{set qa_events=qa_events+1 local roll=random(1,10) menu.execute(event["data"]["request"]) {extra}}}
screen confirmation(context){{return component("yes","button",{{"events":{{"click":"approve"}},"event_data":{{"request":menu_confirm(context["confirmation"]["token"])}}}},[])}}
init{{set qa_events=0}}
scene label start
{audio}
"Waiting"
"Next"
"#);
    let mut engine=rvn_core::Engine::from_program(rvn_parser::parse_program(&source).unwrap(),crate::bevy_renderer::BevyRenderer::new(),8).unwrap();engine.step_until_interaction().unwrap();VnEngine(engine)
}
fn menu_click(world:&mut World)->Result<(),rvn_core::RuntimeError> {
    world.resource_mut::<VnEngine>().0.interface_event(rvn_core::ui::UiInput{screen:"confirmation".into(),element:"yes".into(),kind:rvn_ui::programmable::ScreenEventKind::Click,value:None,key:None})
}
#[test]
fn checkpoint_actual_menu_size_preflight_refuses_callback_before_rng_history_and_modern_pending_publication() {
    for modern in [false,true] {
        let mut harness=crate::source_menus::confirmation_tests::application(crate::systems::save_menu::SaveMenuMode::Save);
        if modern {harness.app.world_mut().insert_resource(modern_menu_engine(false));}
        harness.refresh();let world=harness.app.world_mut();clear_pending(world);world.resource_mut::<VnEngine>().0.renderer.take_pending();
        for index in 0..17 {world.resource_mut::<VnEngine>().0.state.vars.insert(format!("large_authored_value_{index}"),rvn_parser::Value::Str("x".repeat(1024*1024)));}
        let before_state=serde_json::to_value(&world.resource::<VnEngine>().0.state).unwrap();let before_history=serde_json::to_value(world.resource::<VnEngine>().0.history.entries().iter().map(|entry|(&entry.state,&entry.display)).collect::<Vec<_>>()).unwrap();
        let source=world.resource::<VnEngine>().0.audio_request_source().unwrap();let pending=format!("{:?}",world.resource::<VnEngine>().0.renderer.pending);
        let error=menu_click(world).unwrap_err().to_string();
        // Modern callbacks first require the bounded historical digest; the
        // 17 MiB input cannot obtain it. Legacy callbacks reach receipt sizing.
        if modern {assert!(error.contains("historical witness"),"{error}");}
        else {assert!(error.contains("snapshot budget"),"{error}");}
        assert_eq!(serde_json::to_value(&world.resource::<VnEngine>().0.state).unwrap(),before_state);
        assert_eq!(serde_json::to_value(world.resource::<VnEngine>().0.history.entries().iter().map(|entry|(&entry.state,&entry.display)).collect::<Vec<_>>()).unwrap(),before_history);
        assert_eq!(world.resource::<VnEngine>().0.audio_request_source().unwrap(),source);
        assert!(!world.resource::<VnEngine>().0.renderer.menu_pending);assert!(!world.resource::<VnEngine>().0.renderer.scene_batch_pending());
        if modern {assert_eq!(format!("{:?}",world.resource::<VnEngine>().0.renderer.pending),pending);}
    }
}
#[test]
fn checkpoint_actual_core_menu_producer_rejects_a_second_request_before_rng_or_any_receipt_publication() {
    let mut harness=crate::source_menus::confirmation_tests::application(crate::systems::save_menu::SaveMenuMode::Save);
    harness.app.world_mut().insert_resource(modern_menu_engine(true));harness.refresh();let world=harness.app.world_mut();clear_pending(world);world.resource_mut::<VnEngine>().0.renderer.take_pending();
    let before=wire(world);let source=world.resource::<VnEngine>().0.audio_request_source().unwrap();
    assert!(menu_click(world).is_err());assert_eq!(wire(world),before);assert_eq!(world.resource::<VnEngine>().0.audio_request_source().unwrap(),source);
    assert!(!world.resource::<VnEngine>().0.renderer.menu_pending);assert!(world.resource_mut::<VnEngine>().0.renderer.take_scene_menu_receipts().unwrap().is_empty());
}

#[test]
fn checkpoint_actual_menu_save_aggregate_admission_refuses_expected_file_plus_history_without_partial_request_or_lost_confirmation() {
    let mut harness=crate::source_menus::confirmation_tests::application(crate::systems::save_menu::SaveMenuMode::Save);
    audio_assets(&mut harness.app);audio_consumer_resources(&mut harness.app);
    harness.app.init_resource::<CheckpointRequests>();harness.app.world_mut().insert_resource(modern_menu_engine_audio(false,true));
    let mut expected=saved();
    for index in 0..9 {expected.vars.insert(format!("old_file_value_{index}"),rvn_core::save::SaveValue::Str("y".repeat(1024*1024)));}
    let slot=harness.directory.join("slot_01.json");let file=serde_json::to_vec(&expected).unwrap();assert!(file.len()<MAX_REQUEST_BYTES);std::fs::write(&slot,&file).unwrap();
    for index in 0..9 {harness.app.world_mut().resource_mut::<VnEngine>().0.state.vars.insert(format!("current_value_{index}"),rvn_parser::Value::Str("x".repeat(1024*1024)));}
    harness.refresh();let gate=harness.gate();let token=harness.token();
    let world=harness.app.world_mut();fill(world,false);publish(world);world.resource_mut::<VnEngine>().0.renderer.take_pending();
    let view=world.resource::<VnEngine>().0.audio_playback_views().unwrap().remove(0);
    let (source,_)=crate::checkpoint_sources::one_shot_source(world,&view.token.target).unwrap();let mut decoder=source.decoder();
    assert_eq!([decoder.next(),decoder.next(),decoder.next()],[Some(100),Some(200),Some(300)]);
    let previous=serde_json::to_value(&world.resource::<VnEngine>().0.state).unwrap();let history=serde_json::to_value(world.resource::<VnEngine>().0.history.entries().iter().map(|entry|(&entry.state,&entry.display)).collect::<Vec<_>>()).unwrap();
    menu_click(world).unwrap();
    // Actual Source-ready publication emits its single accepted producer receipt.
    // The real MemoryReader/PCM fence is ready before callback and this drain.
    publish(world);
    let mut receipt_reader=bevy::ecs::event::ManualEventReader::<crate::vn_command::VnCommand>::default();
    let count=receipt_reader.read(world.resource::<Events<crate::vn_command::VnCommand>>()).filter(|event|matches!(event,crate::vn_command::VnCommand::SourceMenu(_))).count();assert_eq!(count,1);
    harness.app.update();
    assert!(harness.app.world().resource::<CheckpointRequests>().requests.is_empty());assert!(harness.app.world().resource::<CheckpointRequests>().failed.is_none());
    assert_eq!(serde_json::to_value(&harness.app.world().resource::<VnEngine>().0.state).unwrap(),previous);
    assert_eq!(serde_json::to_value(harness.app.world().resource::<VnEngine>().0.history.entries().iter().map(|entry|(&entry.state,&entry.display)).collect::<Vec<_>>()).unwrap(),history);
    assert_eq!(std::fs::read(&slot).unwrap(),file);assert_eq!(harness.gate(),gate);assert_eq!(harness.token(),token);
    assert!(harness.app.world().resource::<crate::resources::ScriptErrorMessage>().0.contains("capacity"));
}

// Append to checkpoint_requests_tests.rs. These tests use the production
// dispatcher registration and a real MemoryAssetReader/PCM decoder fence.
mod historical_menu_contracts {
    use super::*;
    use rvn_core::Renderer;

    fn ready_receipt() -> (crate::source_menus::confirmation_tests::Harness, crate::source_menus::Receipt, rvn_core::renderer::SceneRenderBatch,crate::checkpoint_pcm::PcmSource) {
        let mut harness=crate::source_menus::confirmation_tests::application(crate::systems::save_menu::SaveMenuMode::Save);
        audio_assets(&mut harness.app);audio_consumer_resources(&mut harness.app);harness.app.init_resource::<CheckpointRequests>();
        harness.app.world_mut().insert_resource(modern_menu_engine_audio(false,true));harness.refresh();
        let world=harness.app.world_mut();fill(world,false);publish(world);world.resource_mut::<VnEngine>().0.renderer.take_pending();
        let token=world.resource::<VnEngine>().0.audio_playback_views().unwrap()[0].token.clone();
        let (source,_)=crate::checkpoint_sources::one_shot_source(world,&token.target).unwrap();let mut decoder=source.decoder();
        assert_eq!([decoder.next(),decoder.next(),decoder.next()],[Some(100),Some(200),Some(300)]);
        menu_click(world).unwrap();
        let batch=world.resource::<VnEngine>().0.renderer.scene_batch.as_deref().unwrap().clone();
        assert!(batch.history_previous.is_some());publish(world);
        let mut events=bevy::ecs::event::ManualEventReader::<crate::vn_command::VnCommand>::default();
        let receipts=events.read(world.resource::<Events<crate::vn_command::VnCommand>>()).filter_map(|event|match event {crate::vn_command::VnCommand::SourceMenu(receipt)=>Some(receipt.clone()),_=>None}).collect::<Vec<_>>();
        assert_eq!(receipts.len(),1);(harness,receipts[0].clone(),batch,decoder)
    }
    fn exact_history(world:&World)->String {format!("{:?}",world.resource::<VnEngine>().0.history.entries())}
    fn checkpoint_position(state:&rvn_core::GameState)->(u64,u16) {
        let view=rvn_core::audio_playback::describe(state,0).unwrap().remove(0);
        let checkpoint=view.playback.checkpoint.unwrap();(checkpoint.next_sample,checkpoint.next_channel)
    }

    #[test]
    fn checkpoint_menu_witness_transports_pre_callback_pcm_and_finishes_history_without_copying_the_later_saved_cursor() {
        let (mut harness,receipt,batch,mut decoder)=ready_receipt();let world=harness.app.world_mut();
        let proof=receipt.history_previous.as_ref().unwrap();
        assert!(world.resource::<VnEngine>().0.accepts_source_menu_snapshot(&receipt.previous,proof).unwrap());
        assert_eq!(serde_json::to_value(&receipt.previous).unwrap(),serde_json::to_value(&batch.previous).unwrap());
        let base=crate::source_menus::checkpoint_receipt_bytes(&receipt.previous,&receipt.effect,&receipt.stamp,None).unwrap();
        let measured=crate::source_menus::checkpoint_receipt_bytes(&receipt.previous,&receipt.effect,&receipt.stamp,Some(proof)).unwrap();
        assert_eq!(measured-base,proof.payload_bytes().unwrap());
        binding(world);
        assert_eq!(decoder.next(),Some(400));drop(decoder);
        let history_len=world.resource::<VnEngine>().0.history.len();harness.app.update();
        assert_eq!(harness.app.world().resource::<CheckpointRequests>().requests.len(),1);
        let world=harness.app.world_mut();process(world);assert!(!world.resource::<CheckpointRequests>().blocked());
        let written=manager(world).unwrap().load(1).unwrap().into_game_state();assert_eq!(checkpoint_position(&written),(4,0));
        assert_eq!(world.resource::<VnEngine>().0.history.len(),history_len+1);
        let historical=&world.resource::<VnEngine>().0.history.entries().last().unwrap().state;
        assert_eq!(checkpoint_position(historical),(3,1));assert_eq!(historical.pc,receipt.previous.pc);
        assert_eq!(historical.random,receipt.previous.random);assert_eq!(historical.vars,receipt.previous.vars);
    }

    #[test]
    fn checkpoint_foreign_historical_witness_is_refused_before_actual_save_with_queue_and_exact_history_retained() {
        let (other,foreign,_,_)=ready_receipt();let (mut harness,_,_,_)=ready_receipt();
        assert_eq!(other.app.world().resource::<VnEngine>().0.runtime_epoch(),harness.app.world().resource::<VnEngine>().0.runtime_epoch());
        assert_ne!(other.app.world().resource::<VnEngine>().0.instance_id(),harness.app.world().resource::<VnEngine>().0.instance_id());
        binding(harness.app.world_mut());harness.app.update();
        let world=harness.app.world_mut();assert_eq!(world.resource::<CheckpointRequests>().requests.len(),1);
        if let Operation::Save{history_previous,..}=&mut world.resource_mut::<CheckpointRequests>().requests[0].operation {*history_previous=foreign.history_previous.clone();}else{panic!("save fixture");}
        let before=wire(world);let history=exact_history(world);let queue=queue_wire(world);let source=world.resource::<VnEngine>().0.audio_request_source().unwrap();
        let file=std::fs::read(harness.directory.join("slot_01.json")).unwrap();
        process(world);assert_eq!(wire(world),before);assert_eq!(exact_history(world),history);assert_eq!(queue_wire(world),queue);
        assert_eq!(world.resource::<VnEngine>().0.audio_request_source().unwrap(),source);assert!(world.resource::<CheckpointRequests>().failed.as_ref().unwrap().contains("witness"));
        assert_eq!(std::fs::read(harness.directory.join("slot_01.json")).unwrap(),file);
    }

    #[test]
    fn checkpoint_foreign_receipt_witness_cannot_cancel_another_engine_with_matching_epoch_ids_and_menu_stamp() {
        let (other,foreign,_,_)=ready_receipt();let (mut harness,mut receipt,_,_)=ready_receipt();
        assert_eq!(other.app.world().resource::<VnEngine>().0.runtime_epoch(),harness.app.world().resource::<VnEngine>().0.runtime_epoch());
        assert_eq!(other.app.world().resource::<VnEngine>().0.interface_epoch(),harness.app.world().resource::<VnEngine>().0.interface_epoch());
        receipt.history_previous=foreign.history_previous.clone();
        let world=harness.app.world_mut();world.resource_mut::<Events<crate::vn_command::VnCommand>>().clear();
        let state=serde_json::to_value(&world.resource::<VnEngine>().0.state).unwrap();let history=exact_history(world);
        let source=world.resource::<VnEngine>().0.audio_request_source().unwrap();let gate=harness.gate();let file=std::fs::read(harness.directory.join("slot_01.json")).unwrap();
        harness.app.world_mut().send_event(crate::vn_command::VnCommand::SourceMenu(receipt));harness.app.update();let world=harness.app.world();
        assert_eq!(serde_json::to_value(&world.resource::<VnEngine>().0.state).unwrap(),state);assert_eq!(exact_history(world),history);
        assert_eq!(world.resource::<VnEngine>().0.audio_request_source().unwrap(),source);assert!(world.resource::<CheckpointRequests>().requests.is_empty());
        assert_eq!(harness.gate(),gate);assert_eq!(std::fs::read(harness.directory.join("slot_01.json")).unwrap(),file);
        assert!(world.resource::<crate::resources::ScriptErrorMessage>().0.contains("historical"));
    }

    #[test]
    fn checkpoint_menu_request_aggregate_counts_the_opaque_witness_before_admitting_any_partial_save() {
        let (mut harness,receipt,_,_)=ready_receipt();let world=harness.app.world_mut();let proof=receipt.history_previous.as_ref().unwrap();
        let proof_bytes=proof.payload_bytes().unwrap();assert!(proof_bytes>1);
        let previous_bytes=serde_json::to_vec(&receipt.previous).unwrap().len();let label="bounded".to_string();let script="main.rvn".to_string();
        let mut expected=saved();
        for index in 0..15 {expected.vars.insert(format!("valid_file_value_{index}"),rvn_core::save::SaveValue::Str("x".repeat(1024*1024)));}
        expected.vars.insert("tail".into(),rvn_core::save::SaveValue::Str(String::new()));
        let target=MAX_REQUEST_BYTES-previous_bytes-label.len()-script.len()-proof_bytes/2;
        let tail=target-encoded_bytes(&expected).unwrap();assert!(tail<1024*1024);
        expected.vars.insert("tail".into(),rvn_core::save::SaveValue::Str("z".repeat(tail)));
        assert!(encoded_bytes(&expected).unwrap()+previous_bytes+label.len()+script.len()<=MAX_REQUEST_BYTES);
        assert!(encoded_bytes(&expected).unwrap()+previous_bytes+label.len()+script.len()+proof_bytes>MAX_REQUEST_BYTES);
        let before=wire(world);let history=exact_history(world);let queue=queue_wire(world);
        let error=world.resource_scope(|world,mut queue:Mut<CheckpointRequests>|defer_menu_save(world.resource::<VnEngine>(),Some(&mut queue),SaveDestination::Manual(1),label,script,Some(expected),&receipt.previous,Some(proof))).unwrap_err();
        assert!(error.contains("capacity"));assert_eq!(wire(world),before);assert_eq!(exact_history(world),history);assert_eq!(queue_wire(world),queue);
        assert!(world.resource::<VnEngine>().0.accepts_source_menu_snapshot(&receipt.previous,proof).unwrap());
    }

    #[test]
    fn checkpoint_renderer_coalescence_keeps_batch_and_receipt_historical_provenance_as_two_exact_producer_pairs() {
        let (first,first_receipt,mut first_batch,_)=ready_receipt();let (second,second_receipt,mut second_batch,_)=ready_receipt();
        // The real renderer drained these effects into the captured receipts.
        // Requeue exactly those producer effects to exercise coalescing anew.
        assert!(first_batch.menu_requests.is_empty() && second_batch.menu_requests.is_empty());
        first_batch.menu_requests.push(first_receipt.effect.clone());
        second_batch.menu_requests.push(second_receipt.effect.clone());
        let mut renderer=crate::bevy_renderer::BevyRenderer::new();renderer.publish_scene_batch(first_batch.clone());renderer.publish_scene_batch(second_batch.clone());
        let batch=renderer.scene_batch.as_ref().unwrap();let proof=batch.history_previous.as_ref().unwrap();
        assert_eq!(serde_json::to_value(&batch.previous).unwrap(),serde_json::to_value(&first_receipt.previous).unwrap());
        assert!(first.app.world().resource::<VnEngine>().0.accepts_source_menu_snapshot(&batch.previous,proof).unwrap());
        assert!(!second.app.world().resource::<VnEngine>().0.accepts_source_menu_snapshot(&batch.previous,proof).unwrap());
        let receipts=renderer.take_scene_menu_receipts().unwrap();assert_eq!(receipts.len(),2);
        for (command,(world,expected)) in receipts.iter().zip([(first.app.world(),&first_receipt),(second.app.world(),&second_receipt)]) {
            let crate::vn_command::VnCommand::SourceMenu(receipt)=command else{panic!("actual menu receipt");};
            assert_eq!(serde_json::to_value(&receipt.previous).unwrap(),serde_json::to_value(&expected.previous).unwrap());
            assert!(world.resource::<VnEngine>().0.accepts_source_menu_snapshot(&receipt.previous,receipt.history_previous.as_ref().unwrap()).unwrap());
        }
    }

    #[test]
    fn checkpoint_missing_real_precallback_pcm_witness_refuses_the_modern_menu_before_core_mutation_or_receipt_publication() {
        let mut harness=crate::source_menus::confirmation_tests::application(crate::systems::save_menu::SaveMenuMode::Save);
        audio_assets(&mut harness.app);audio_consumer_resources(&mut harness.app);harness.app.init_resource::<CheckpointRequests>();
        harness.app.world_mut().insert_resource(modern_menu_engine_audio(false,true));harness.refresh();let world=harness.app.world_mut();
        fill(world,false);publish(world);world.resource_mut::<VnEngine>().0.renderer.take_pending();
        assert!(world.resource::<VnEngine>().0.audio_playback_views().unwrap()[0].playback.checkpoint.is_some());
        // A saved Some cannot replace the revoked real decoder authority.
        world.resource_mut::<VnEngine>().0.renderer.audio_checkpoints=Default::default();
        let before=wire(world);let history=exact_history(world);let source=world.resource::<VnEngine>().0.audio_request_source().unwrap();
        let error=menu_click(world).unwrap_err().to_string();assert!(error.contains("historical witness"),"{error}");
        assert_eq!(wire(world),before);assert_eq!(exact_history(world),history);assert_eq!(world.resource::<VnEngine>().0.audio_request_source().unwrap(),source);
        assert!(!world.resource::<VnEngine>().0.renderer.menu_pending);assert!(world.resource_mut::<VnEngine>().0.renderer.take_scene_menu_receipts().unwrap().is_empty());
    }

    fn welcome_click(world:&mut World)->crate::source_menus::Receipt {
        world.resource_mut::<VnEngine>().0.interface_event(rvn_core::ui::UiInput{screen:"ordinary".into(),element:"yes".into(),kind:rvn_ui::programmable::ScreenEventKind::Click,value:None,key:None}).unwrap();
        assert!(world.resource::<VnEngine>().0.renderer.scene_batch.as_ref().unwrap().history_previous.is_some());publish(world);
        let mut events=bevy::ecs::event::ManualEventReader::<crate::vn_command::VnCommand>::default();
        events.read(world.resource::<Events<crate::vn_command::VnCommand>>()).find_map(|event|if let crate::vn_command::VnCommand::SourceMenu(receipt)=event{Some(receipt.clone())}else{None}).unwrap()
    }
    #[test]
    fn checkpoint_actual_welcome_dispatch_accepts_empty_history_once_and_rejects_an_old_engine_without_poisoning_new_receipt_one() {
        let mut harness=crate::source_menus::confirmation_tests::welcome_application();audio_assets(&mut harness.app);audio_consumer_resources(&mut harness.app);harness.app.init_resource::<CheckpointRequests>();
        publish(harness.app.world_mut());harness.app.world_mut().resource_mut::<VnEngine>().0.renderer.take_pending();
        assert!(harness.app.world().resource::<VnEngine>().0.history.is_empty());assert!(harness.app.world().resource::<VnEngine>().0.displayed_interaction().unwrap().is_none());
        let receipt=welcome_click(harness.app.world_mut());assert!(receipt.identity>0);let first_identity=receipt.identity;harness.app.update();
        assert_eq!(harness.app.world().resource::<crate::systems::settings_menu::Settings>().music_volume,0.25);
        assert!(harness.app.world().resource::<VnEngine>().0.history.is_empty());assert!(harness.app.world().resource::<crate::resources::ScriptErrorMessage>().0.is_empty());
        harness.app.world_mut().send_event(crate::vn_command::VnCommand::SourceMenu(receipt.clone()));harness.app.update();
        assert!(harness.app.world().resource::<crate::resources::ScriptErrorMessage>().0.contains("already consumed"));assert!(harness.app.world().resource::<VnEngine>().0.history.is_empty());
        let old_instance=harness.app.world().resource::<VnEngine>().0.instance_id();
        harness.app.world_mut().insert_resource(crate::source_menus::confirmation_tests::welcome_engine());harness.refresh();publish(harness.app.world_mut());
        harness.app.world_mut().resource_mut::<VnEngine>().0.renderer.take_pending();harness.app.world_mut().resource_mut::<Events<crate::vn_command::VnCommand>>().clear();
        let world=harness.app.world();assert_ne!(world.resource::<VnEngine>().0.instance_id(),old_instance);assert_eq!(world.resource::<VnEngine>().0.runtime_epoch(),0);
        let before=wire(world);let history=exact_history(world);let file=harness.disk();let gain=world.resource::<crate::systems::settings_menu::Settings>().music_volume;
        harness.app.world_mut().send_event(crate::vn_command::VnCommand::SourceMenu(receipt));harness.app.update();
        assert_eq!(wire(harness.app.world()),before);assert_eq!(exact_history(harness.app.world()),history);assert_eq!(harness.disk(),file);
        assert_eq!(harness.app.world().resource::<crate::systems::settings_menu::Settings>().music_volume,gain);
        assert!(harness.app.world().resource::<crate::resources::ScriptErrorMessage>().0.contains("historical"));
        harness.app.world_mut().resource_mut::<Events<crate::vn_command::VnCommand>>().clear();let new=welcome_click(harness.app.world_mut());assert!(new.identity>first_identity);harness.app.update();
        assert!(harness.app.world().resource::<crate::resources::ScriptErrorMessage>().0.is_empty());assert!(harness.app.world().resource::<VnEngine>().0.history.is_empty());
        harness.app.world_mut().send_event(crate::vn_command::VnCommand::SourceMenu(new));harness.app.update();
        assert!(harness.app.world().resource::<crate::resources::ScriptErrorMessage>().0.contains("already consumed"));
    }

    #[test]
    fn checkpoint_actual_load_clears_history_then_measured_source_menu_save_still_uses_the_real_fence_and_dispatcher() {
        let (mut harness,_,_,_)=ready_receipt();let world=harness.app.world_mut();
        let snapshot=world.resource::<VnEngine>().0.snapshot_for_save().unwrap();let saved=SaveData::try_from_snapshot(&snapshot,1,"restored".into(),"main.rvn".into()).unwrap();
        assert!(crate::checkpoint_sources::warm_restore(world,&saved).unwrap());
        let old_epoch=world.resource::<VnEngine>().0.runtime_epoch();world.resource_mut::<VnEngine>().0.load_data(saved).unwrap();publish(world);
        assert!(world.resource::<VnEngine>().0.history.is_empty());assert_eq!(world.resource::<VnEngine>().0.runtime_epoch(),old_epoch+1);
        world.resource_mut::<Events<crate::vn_command::VnCommand>>().clear();harness.refresh();publish(harness.app.world_mut());
        let world=harness.app.world_mut();world.resource_mut::<VnEngine>().0.renderer.take_pending();binding(world);
        menu_click(world).unwrap();let batch=world.resource::<VnEngine>().0.renderer.scene_batch.as_ref().unwrap();assert!(batch.history_previous.is_some());
        publish(world);harness.app.update();let world=harness.app.world_mut();assert_eq!(world.resource::<CheckpointRequests>().requests.len(),1);
        process(world);assert!(!world.resource::<CheckpointRequests>().blocked());assert!(world.resource::<CheckpointRequests>().failed.is_none());
        assert_eq!(world.resource::<VnEngine>().0.history.len(),1);
        assert_eq!(checkpoint_position(&world.resource::<VnEngine>().0.history.entries()[0].state),(3,1));
        let written=manager(world).unwrap().load(1).unwrap();assert_eq!(written.format_version,11);assert_eq!(checkpoint_position(&written.into_game_state()),(3,1));
    }
}
