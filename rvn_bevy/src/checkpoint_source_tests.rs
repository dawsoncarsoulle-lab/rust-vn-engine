use super::*;
use bevy::audio::Decodable;
use crate::resources::{VnEngine,VnState};
use rvn_core::{save::SaveData,scene_tasks::TaskOwner};

pub(super) fn wave()->Vec<u8> {
    let samples:[i16;8]=[100,200,300,400,500,600,700,800];
    let bytes=(samples.len()*2) as u32;
    let mut encoded=Vec::new();encoded.extend(b"RIFF");encoded.extend((36+bytes).to_le_bytes());encoded.extend(b"WAVEfmt ");encoded.extend(16u32.to_le_bytes());
    encoded.extend(1u16.to_le_bytes());encoded.extend(2u16.to_le_bytes());encoded.extend(48_000u32.to_le_bytes());encoded.extend(192_000u32.to_le_bytes());encoded.extend(4u16.to_le_bytes());encoded.extend(16u16.to_le_bytes());encoded.extend(b"data");encoded.extend(bytes.to_le_bytes());
    for sample in samples {encoded.extend(sample.to_le_bytes());}encoded
}
fn game(source:&str)->rvn_core::Engine<crate::bevy_renderer::BevyRenderer> {
    let mut game=rvn_core::Engine::from_program(rvn_parser::parse_program(source).unwrap(),crate::bevy_renderer::BevyRenderer::new(),8).unwrap();game.step_until_interaction().unwrap();game
}
fn app(source:&str)->App { app_from_game(game(source)) }
// A blocking-renderer step really passes the interactive instruction and arms
// its producer at pc+1; step_until_interaction deliberately does neither.
fn app_after_interactive_step(source:&str)->App {
    let mut game=rvn_core::Engine::from_program(rvn_parser::parse_program(source).unwrap(),crate::bevy_renderer::BevyRenderer::new(),8).unwrap();
    game.step().unwrap();app_from_game(game)
}
pub(super) fn app_from_game(game:rvn_core::Engine<crate::bevy_renderer::BevyRenderer>)->App {
    let directory=bevy::asset::io::memory::Dir::default();
    let reader=bevy::asset::io::memory::MemoryAssetReader{root:directory};let mut app=App::new();
    app.register_asset_source(bevy::asset::io::AssetSourceId::Default,bevy::asset::io::AssetSource::build().with_reader(move||Box::new(reader.clone())));
    app.add_plugins((MinimalPlugins,bevy::asset::AssetPlugin{watch_for_changes_override:Some(false),..default()}));
    app.init_asset::<AudioSource>().init_asset_loader::<bevy::audio::AudioLoader>().init_asset::<CheckpointPcm>();
    app.insert_resource(VnEngine(game)).insert_resource(State::new(VnState::Waiting))
        .init_resource::<NextState<VnState>>().init_resource::<crate::resources::VnRenderState>()
        .init_resource::<crate::resources::ChoiceFocus>().init_resource::<crate::resources::ImagemapState>()
        .init_resource::<crate::resources::TypewriterState>().init_resource::<crate::accessibility::Accessibility>()
        .init_resource::<crate::resources::ScriptErrorMessage>().init_resource::<crate::resources::MusicEntity>()
        .init_resource::<crate::resources::MusicAssetRegistry>().init_resource::<crate::resources::MusicPlaybackState>()
        .init_resource::<crate::resources::MusicVolume>().init_resource::<crate::systems::settings_menu::Settings>()
        .init_resource::<crate::scene_publication::ScenePresentation>().init_resource::<crate::scene_audio::AudioFeedbackQueue>()
        .add_event::<crate::vn_command::VnCommand>().add_event::<crate::vn_command::PlayerInput>();app
}
fn fill(world:&mut World) {
    let engine=&world.resource::<VnEngine>().0;
    let mut paths:Vec<String>=engine.audio_playback_views().unwrap().iter().map(|view|crate::scene_audio::source_path(view.playback.family,&view.playback.file)).collect();
    paths.extend(rvn_core::audio_checkpoint::music_views(&engine.state,engine.runtime_epoch()).unwrap().iter().map(|view|world.resource::<crate::resources::MusicAssetRegistry>().resolve(&view.file)));
    paths.sort();paths.dedup();
    for path in paths {let handle=world.resource::<AssetServer>().load::<AudioSource>(path);world.resource_mut::<Assets<AudioSource>>().insert(handle.id(),AudioSource{bytes:Arc::from(wave())});}
}
fn publish(world:&mut World) {crate::scene_publication::consume(world);assert!(!world.resource::<VnEngine>().0.renderer.scene_frame_failed,"{}",world.resource::<crate::resources::ScriptErrorMessage>().0);assert!(!world.resource::<VnEngine>().0.renderer.scene_batch_pending());}
fn reader(world:&World,target:&TargetIdentity)->CheckpointPcm {world.resource::<PreparedSourceCache>().entries[target].reader.clone()}
fn cursor(reader:&CheckpointPcm)->rvn_core::audio_checkpoint::AudioCheckpoint {reader.gate().snapshot(&[reader],|mut rows|Ok(rows.remove(0))).unwrap()}
fn wire(world:&World)->serde_json::Value {let engine=&world.resource::<VnEngine>().0;serde_json::json!({"state":engine.state,"history":engine.history.entries().iter().map(|entry|(&entry.state,&entry.display)).collect::<Vec<_>>(),"epoch":engine.runtime_epoch(),"interface":engine.interface_epoch(),"outputs":format!("{:?}",engine.renderer.pending),"pending":engine.renderer.scene_batch_pending()})}
fn advance(world:&mut World) {let mut engine=world.resource_mut::<VnEngine>();engine.0.advance_dialogue().unwrap();engine.0.step_until_interaction().unwrap();drop(engine);publish(world);}

#[test]
fn first_actual_ready_source_fills_preinteractive_entry_once_at_dialogue_pc_plus_one_and_roundtrips_stereo_without_rng_replay() {
    let mut app=app_after_interactive_step("scene label P\nsfx.play(\"same.wav\")\n\"é🌟 parent\"\nreturn");let world=app.world_mut();fill(world);
    let historical_before=world.resource::<VnEngine>().0.history.entries()[0].state.clone();let live_before=world.resource::<VnEngine>().0.state.clone();assert_eq!(live_before.pc,historical_before.pc+1);
    publish(world);let target=world.resource::<VnEngine>().0.audio_playback_views().unwrap()[0].token.target.clone();let original=reader(world,&target);
    assert_eq!(cursor(&original).next_sample,0);let entry=&world.resource::<VnEngine>().0.history.entries()[0].state;
    assert_eq!(entry.pc,historical_before.pc);assert_eq!(entry.random,historical_before.random);assert_eq!(entry.vars,historical_before.vars);
    assert_eq!(entry.scene_execution.as_ref().unwrap().resources.tasks.iter().find_map(|task|task.audio.as_ref()).unwrap().checkpoint.as_ref().unwrap().next_sample,0);
    let mut decoder=original.decoder();assert_eq!([decoder.next(),decoder.next(),decoder.next()],[Some(100),Some(200),Some(300)]);
    let source=world.resource::<VnEngine>().0.display_reveal_source().unwrap();let reveal=rvn_core::display_reveal::RevealCheckpoint::observed("é🌟 parent",2,true,0.125,0.375).unwrap();
    world.resource_mut::<VnEngine>().0.record_display_reveal(&source,reveal.clone()).unwrap();
    let snapshot=world.resource::<VnEngine>().0.snapshot_for_save().unwrap();let saved=SaveData::try_from_snapshot(&snapshot,1,"captured".into(),"main.rvn".into()).unwrap();assert_eq!(saved.format_version,11);
    let dto:SaveData=serde_json::from_slice(&serde_json::to_vec(&saved).unwrap()).unwrap();let exact=dto.scene_execution.as_ref().unwrap().resources.tasks.iter().find_map(|task|task.audio.as_ref()).unwrap().checkpoint.as_ref().unwrap();assert_eq!((exact.next_sample,exact.next_channel),(3,1));
    let state_before_load=world.resource::<VnEngine>().0.state.clone();assert!(warm_restore(world,&dto).unwrap());world.resource_mut::<VnEngine>().0.load_data(dto).unwrap();publish(world);
    assert_eq!(world.resource::<VnEngine>().0.state.pc,state_before_load.pc);assert_eq!(world.resource::<VnEngine>().0.state.random,state_before_load.random);
    assert_eq!(world.resource::<VnEngine>().0.state.scene_execution.as_ref().unwrap().display.as_ref().unwrap().reveal.as_ref(),Some(&reveal));
    let restored=reader(world,&target);assert!(!restored.same_reader(&original));let mut decoder=restored.decoder();assert_eq!([decoder.next(),decoder.next(),decoder.next()],[Some(0),Some(400),Some(500)]);
    assert_eq!(cursor(&restored).next_sample,5);assert!(world.resource::<VnEngine>().0.history.is_empty());
}

#[test]
fn missing_assets_defer_the_original_pending_publication_without_pc_rng_history_or_entity_mutation() {
    let mut app=app("scene label P\nsfx.play(\"later.wav\")\n\"line\"");let world=app.world_mut();let before=wire(world);let entities=world.entities().len();
    let engine=&world.resource::<VnEngine>().0;assert!(prepare(world,&engine.state,engine.runtime_epoch()).unwrap().is_none());
    assert_eq!(wire(world),before);assert_eq!(world.entities().len(),entities);assert!(world.get_resource::<PreparedSourceCache>().is_none());assert!(world.resource::<VnEngine>().0.snapshot_for_save().is_err());
    fill(world);publish(world);let view=world.resource::<VnEngine>().0.audio_playback_views().unwrap().remove(0);let actual=reader(world,&view.token.target);
    assert_eq!(cursor(&actual).next_sample,0);assert_eq!(world.resource::<VnEngine>().0.history.len(),1);assert_eq!(world.resource::<VnEngine>().0.state.pc,before["state"]["pc"].as_u64().unwrap() as usize);
}

#[test]
fn source_plan_rejects_changed_clock_or_engine_before_core_history_and_source_residence_install() {
    let mut app=app("scene label P\nsfx.play(\"same.wav\") with fade(1000)\n\"line\"");let world=app.world_mut();fill(world);
    let engine=&world.resource::<VnEngine>().0;let plan=prepare(world,&engine.state,engine.runtime_epoch()).unwrap().unwrap();
    world.resource_mut::<VnEngine>().0.tick_game_frame(0.125,rvn_core::GameFrameReports::default()).unwrap();let before=wire(world);assert!(super::publish(world,&plan).is_err());assert_eq!(wire(world),before);assert!(world.get_resource::<PreparedSourceCache>().is_none());
    let new=game("scene label P\nsfx.play(\"same.wav\") with fade(1000)\n\"line\"");assert_eq!(new.runtime_epoch(),plan.epoch);assert_ne!(new.instance_id(),plan.instance);world.insert_resource(VnEngine(new));let before=wire(world);
    assert!(super::publish(world,&plan).is_err());assert_eq!(wire(world),before);
}

#[test]
fn scene_call_parks_the_actual_source_return_renews_authority_without_claiming_another_decoder_or_resetting_cursor() {
    let mut app=app("init{sfx.play(\"game.wav\")}\nscene label P\nsfx.play(\"parent.wav\")\n\"parent\"\ncall C\n\"returned\"\nreturn\nscene label C\n\"child\"\nreturn");let world=app.world_mut();fill(world);publish(world);
    let views=world.resource::<VnEngine>().0.audio_playback_views().unwrap();let parent=views.iter().find(|view|view.token.task.key.owner!=TaskOwner::Game).unwrap();let target=parent.token.target.clone();let old_revision=parent.token.task.revision;
    let original=reader(world,&target);let mut decoder=original.decoder();assert_eq!(decoder.next(),Some(100));advance(world);
    let parked=reader(world,&target);assert!(parked.same_reader(&original));assert_eq!(decoder.next(),Some(0));assert_eq!(cursor(&parked).next_sample,1);
    advance(world);let current=world.resource::<VnEngine>().0.audio_playback_views().unwrap().into_iter().find(|view|view.token.target==target).unwrap();assert_ne!(current.token.task.revision,old_revision);assert!(!current.parked);
    let resumed=reader(world,&target);assert!(resumed.same_reader(&original));assert_eq!(decoder.next(),Some(0));assert_eq!(cursor(&resumed).next_sample,1);assert_eq!(decoder.next(),Some(200));assert_eq!(cursor(&resumed).next_sample,2);assert_eq!(resumed.fault().unwrap(),None);
}

#[test]
fn music_current_to_retired_handoff_keeps_the_same_source_entity_and_consumed_cursor_while_new_current_is_independent() {
    let mut app=app("scene label P\nmusic.play(\"one.wav\")\n\"first\"\nmusic.play(\"two.wav\") with fade(1000)\n\"second\"");let world=app.world_mut();fill(world);publish(world);
    let old_view=rvn_core::audio_checkpoint::music_views(&world.resource::<VnEngine>().0.state,0).unwrap().remove(0);let old_target=old_view.authority.target.clone();let old_entry=world.resource::<PreparedSourceCache>().entries[&old_target].clone();let mut decoder=old_entry.reader.decoder();assert_eq!([decoder.next(),decoder.next(),decoder.next()],[Some(100),Some(200),Some(300)]);
    {let mut engine=world.resource_mut::<VnEngine>();engine.0.advance_dialogue().unwrap();engine.0.step_until_interaction().unwrap();}fill(world);publish(world);
    let views=rvn_core::audio_checkpoint::music_views(&world.resource::<VnEngine>().0.state,0).unwrap();let retired=views.iter().find(|view|view.authority.outgoing.is_some()).unwrap();let current=views.iter().find(|view|view.authority.outgoing.is_none()).unwrap();
    let outgoing=&world.resource::<PreparedSourceCache>().entries[&retired.authority.target];let incoming=&world.resource::<PreparedSourceCache>().entries[&current.authority.target];
    assert!(outgoing.reader.same_reader(&old_entry.reader));assert_eq!(outgoing.entity,old_entry.entity);assert!(!incoming.reader.same_reader(&outgoing.reader));assert_ne!(incoming.entity,outgoing.entity);
    assert_eq!(cursor(&outgoing.reader).next_sample,3);assert_eq!(cursor(&incoming.reader).next_sample,0);assert_eq!(decoder.next(),Some(400));
    let captured=world.resource::<VnEngine>().0.snapshot_for_save().unwrap();let rows=&captured.state().scene_execution.as_ref().unwrap().resources.music_checkpoints;assert_eq!(rows.len(),2);assert_eq!(rows.iter().find(|row|row.target==retired.authority.target).unwrap().checkpoint.next_sample,4);
}

#[test]
fn same_position_audio_rollback_keeps_earlier_entry_exact_and_a_second_publication_never_recaptures_its_cursor() {
    let mut app=app_after_interactive_step("init{sfx.play(\"game.wav\")}\nscene label P\nlabel again\n\"repeat\"\njump again");let world=app.world_mut();fill(world);publish(world);
    let first=serde_json::to_value(&world.resource::<VnEngine>().0.history.entries()[0].state).unwrap();let target=world.resource::<VnEngine>().0.audio_playback_views().unwrap()[0].token.target.clone();let actual=reader(world,&target);let mut decoder=actual.decoder();assert_eq!(decoder.next(),Some(100));
    world.resource_mut::<VnEngine>().0.step().unwrap();publish(world);assert_eq!(world.resource::<VnEngine>().0.history.len(),2);assert_eq!(serde_json::to_value(&world.resource::<VnEngine>().0.history.entries()[0].state).unwrap(),first);
    assert!(world.resource_mut::<VnEngine>().0.rollback());publish(world);assert_eq!(world.resource::<VnEngine>().0.history.len(),1);assert_eq!(serde_json::to_value(&world.resource::<VnEngine>().0.history.entries()[0].state).unwrap(),first);
    let restored=reader(world,&target);let mut decoder=restored.decoder();assert_eq!(decoder.next(),Some(0));assert_eq!(decoder.next(),Some(200));
    let engine=&world.resource::<VnEngine>().0;let plan=prepare(world,&engine.state,engine.runtime_epoch()).unwrap().unwrap();super::publish(world,&plan).unwrap();install(world,plan);
    assert_eq!(serde_json::to_value(&world.resource::<VnEngine>().0.history.entries()[0].state).unwrap(),first);assert_eq!(cursor(&reader(world,&target)).next_sample,2);
}

#[test]
fn cold_asset_admission_compares_actual_encoded_and_pcm_metadata_before_any_load_state_history_epoch_or_publication_change() {
    let source="scene label P\nsfx.play(\"same.wav\")\n\"line\"";let mut original=app(source);let world=original.world_mut();fill(world);publish(world);
    let saved=SaveData::try_from_snapshot(&world.resource::<VnEngine>().0.snapshot_for_save().unwrap(),1,"real".into(),"main.rvn".into()).unwrap();
    let mut cold=app(source);let world=cold.world_mut();let before=wire(world);assert!(!warm_restore(world,&saved).unwrap());assert_eq!(wire(world),before);fill(world);assert!(warm_restore(world,&saved).unwrap());assert_eq!(wire(world),before);
    let paths:Vec<_>=world.resource::<VnEngine>().0.audio_playback_views().unwrap().iter().map(|view|crate::scene_audio::source_path(view.playback.family,&view.playback.file)).collect();
    for path in paths {let handle=world.resource::<AssetServer>().get_handle::<AudioSource>(path).unwrap();let mut altered=wave();*altered.last_mut().unwrap()^=1;world.resource_mut::<Assets<AudioSource>>().insert(handle.id(),AudioSource{bytes:Arc::from(altered)});}
    assert!(warm_restore(world,&saved).is_err());assert_eq!(wire(world),before);assert!(world.get_resource::<PreparedSourceCache>().is_none());
}

fn historical_audio_source(menu:bool)->String {
    let extra=if menu {r#"menu.execute(menu_action("none"))"#}else{""};
    format!(r#"handler change(event){{set clicks=clicks+1 set roll=random(1,100) {extra}}}
screen panel(){{return component("act","button",{{"events":{{"click":"change"}}}},[])}}
screen host(ctx){{return component("act","button",{{"events":{{"click":"change"}}}},[])}}
init{{set clicks=0 set roll=0}}
scene label P
sfx.play("same.wav") with fade(1000)
ui.open("panel",[],false,1)
"parent"
return"#)
}
fn historical_click(world:&mut World,screen:&str) {
    world.resource_mut::<VnEngine>().0.interface_event(rvn_core::ui::UiInput{screen:screen.into(),element:"act".into(),kind:rvn_ui::programmable::ScreenEventKind::Click,value:None,key:None}).unwrap();
}
fn historical_cursor(state:&rvn_core::GameState)->Option<rvn_core::audio_checkpoint::AudioCheckpoint> {
    state.scene_execution.as_ref().unwrap().resources.tasks.iter().find_map(|task|task.audio.as_ref().and_then(|audio|audio.checkpoint.clone()))
}

#[test]
fn actual_decoder_three_samples_frame_then_story_and_host_ui_history_use_precallback_pcm_and_exact_rng() {
    for host in [false,true] {
        let mut app=app(&historical_audio_source(false));let world=app.world_mut();fill(world);publish(world);
        if host {
            let context=rvn_core::ui::value_from_json(&rvn_ui::source_menus::preview_context(rvn_ui::PageRole::Pause,true)).unwrap();
            world.resource_mut::<VnEngine>().0.synchronize_source_menu(rvn_ui::PageRole::Pause,Some("host"),context,true,500).unwrap();publish(world);
        }
        let target=world.resource::<VnEngine>().0.audio_playback_views().unwrap()[0].token.target.clone();let actual=reader(world,&target);let mut decoder=actual.decoder();
        assert_eq!([decoder.next(),decoder.next(),decoder.next()],[Some(100),Some(200),Some(300)]);
        world.resource_mut::<VnEngine>().0.tick_game_frame(0.125,Default::default()).unwrap();publish(world);
        // Advance the physical decoder again after the frame's publication.
        // A stale saved Some from that publication is not the callback fence.
        assert_eq!(decoder.next(),Some(400));
        let before=world.resource::<VnEngine>().0.state.clone();let history=world.resource::<VnEngine>().0.history.len();
        assert_eq!(historical_cursor(&before).unwrap().next_sample,3);
        let owner=before.scene_execution.as_ref().unwrap().owner();
        let screen=if host {"host"}else{"panel"};let ui_rng=before.ui.screens.iter().find(|view|view.name==screen).unwrap().random;
        historical_click(world,screen);
        let engine=&world.resource::<VnEngine>().0;assert_eq!(engine.history.len(),history+1);assert_eq!(engine.state.vars["clicks"],rvn_parser::Value::Int(1));
        if host {assert_eq!(engine.state.random,before.random);assert_ne!(engine.state.ui.screens.iter().find(|view|view.name==screen).unwrap().random,ui_rng);}
        else {assert_ne!(engine.state.random,before.random);}
        let captured=&engine.history.entries().last().unwrap().state;
        assert_eq!(captured.pc,before.pc);assert_eq!(captured.random,before.random);assert_eq!(captured.vars,before.vars);let mut expected_story_ui=before.ui.clone(); expected_story_ui.screens.retain(|screen|screen.host_role.is_none()); expected_story_ui.host_next_order=0; assert_eq!(captured.ui,expected_story_ui);
        assert_eq!(captured.scene_execution.as_ref().unwrap().owner(),owner);
        assert_eq!(historical_cursor(captured).unwrap().next_sample,4);
        let clock=|state:&rvn_core::GameState|state.scene_execution.as_ref().unwrap().resources.tasks.iter().find_map(|task|task.audio.as_ref()).unwrap().envelope.as_ref().unwrap().clock.elapsed.to_bits();
        assert_eq!(clock(captured),clock(&before));assert_eq!(clock(captured),0.125f64.to_bits());
        // Real rollback cold-preflights the exact measured entry, without replay.
        let (source,saved)=engine.preview_game_audio_rollback().unwrap().unwrap();assert!(warm_restore(world,&saved).unwrap());
        assert!(world.resource_mut::<VnEngine>().0.rollback_from_audio_source(&source).unwrap());publish(world);
        let restored=&world.resource::<VnEngine>().0.state;assert_eq!(restored.pc,before.pc);assert_eq!(restored.random,before.random);assert_eq!(restored.vars,before.vars);assert_eq!(clock(restored),clock(&before));
        let resumed=reader(world,&target);assert_eq!(cursor(&resumed).next_sample,4);assert_eq!(resumed.decoder().next(),Some(500));
    }
}

#[test]
fn actual_menu_historical_proof_is_metadata_only_raw_cancel_exact_and_stale_visit_neutral() {
    let mut app=app(&historical_audio_source(true));let world=app.world_mut();fill(world);publish(world);
    let target=world.resource::<VnEngine>().0.audio_playback_views().unwrap()[0].token.target.clone();let actual=reader(world,&target);let mut decoder=actual.decoder();
    assert_eq!([decoder.next(),decoder.next(),decoder.next()],[Some(100),Some(200),Some(300)]);
    world.resource_mut::<VnEngine>().0.tick_game_frame(0.125,Default::default()).unwrap();publish(world);
    // Same cardinality as the real aggregate 16MiB consumer refusal fixture.
    for index in 0..9 {world.resource_mut::<VnEngine>().0.state.vars.insert(format!("payload_{index}"),rvn_parser::Value::Str("x".repeat(1024*1024)));}
    let previous=world.resource::<VnEngine>().0.state.clone();let raw=serde_json::to_value(&previous).unwrap();
    assert_eq!(decoder.next(),Some(400));
    world.resource_mut::<VnEngine>().0.renderer.menu_authority.story_ui_active=true;
    historical_click(world,"panel");
    let batch=world.resource::<VnEngine>().0.renderer.scene_batch.as_ref().unwrap();assert_eq!(serde_json::to_value(&batch.previous).unwrap(),raw);
    let proof=batch.history_previous.clone().unwrap();assert!(proof.payload_bytes().unwrap()<16*1024,"Only bounded metadata, never another 9MiB GameState");
    assert!(world.resource::<VnEngine>().0.accepts_source_menu_snapshot(&previous,&proof).unwrap());
    let mut independent=game(&historical_audio_source(true));assert_eq!(independent.runtime_epoch(),world.resource::<VnEngine>().0.runtime_epoch());
    let foreign=serde_json::to_value(&independent.state).unwrap();assert!(!independent.accepts_source_menu_snapshot(&previous,&proof).unwrap());assert!(independent.finish_source_menu_snapshot(previous.clone(),&proof).is_err());assert_eq!(serde_json::to_value(&independent.state).unwrap(),foreign);
    let mut altered=previous.clone();altered.vars.insert("foreign".into(),rvn_parser::Value::Int(1));
    assert!(!world.resource::<VnEngine>().0.accepts_source_menu_snapshot(&altered,&proof).unwrap());
    let mut counter_only=previous.clone();counter_only.ui.host_next_order=counter_only.ui.host_next_order.checked_add(1).unwrap();
    assert_eq!(serde_json::to_value(&counter_only).unwrap(),raw,"Save serde omits this transient counter");
    let unchanged=wire(world);assert!(!world.resource::<VnEngine>().0.accepts_source_menu_snapshot(&counter_only,&proof).unwrap());
    assert!(world.resource_mut::<VnEngine>().0.finish_source_menu_snapshot(counter_only,&proof).is_err());assert_eq!(wire(world),unchanged);
    publish(world);
    let before=wire(world);let history=world.resource::<VnEngine>().0.history.len();
    assert!(world.resource_mut::<VnEngine>().0.finish_source_menu_snapshot(previous.clone(),&proof).unwrap());
    assert_eq!(world.resource::<VnEngine>().0.history.len(),history+1);
    let captured=&world.resource::<VnEngine>().0.history.entries().last().unwrap().state;
    assert_eq!(captured.vars,previous.vars);assert_eq!(captured.random,previous.random);assert_eq!(captured.pc,previous.pc);assert_eq!(historical_cursor(captured).unwrap().next_sample,4);
    let after=wire(world);assert!(world.resource_mut::<VnEngine>().0.finish_source_menu_snapshot(previous.clone(),&proof).is_err());assert_eq!(wire(world),after);
    // Raw previous remains the exact compensation image, even though live
    // Source-ready just observed cursor4 rather than raw's older cursor3.
    world.resource_mut::<VnEngine>().0.cancel_source_menu_transaction(previous).unwrap();assert_eq!(serde_json::to_value(&world.resource::<VnEngine>().0.state).unwrap(),raw);
    assert_ne!(before,after);
}

#[test]
fn missing_actual_precallback_reader_never_reauthorizes_a_saved_some_in_ui_or_raw_menu_history() {
    let mut app=app(&historical_audio_source(false));let world=app.world_mut();fill(world);publish(world);
    assert!(historical_cursor(&world.resource::<VnEngine>().0.state).is_some());
    // Explicitly remove the real reader authority, retaining a previously saved
    // Some in GameState. This is not a fresh measurement.
    world.resource_mut::<VnEngine>().0.renderer.audio_checkpoints=Default::default();
    let previous=world.resource::<VnEngine>().0.state.clone();let history=world.resource::<VnEngine>().0.history.len();
    historical_click(world,"panel");assert_eq!(world.resource::<VnEngine>().0.history.len(),history+1);
    assert!(historical_cursor(&world.resource::<VnEngine>().0.history.entries().last().unwrap().state).is_none());
    assert!(world.resource::<VnEngine>().0.preview_game_audio_rollback().is_err());
    world.resource_mut::<VnEngine>().0.finish_source_menu_transaction(previous);
    assert!(historical_cursor(&world.resource::<VnEngine>().0.history.entries().last().unwrap().state).is_none());
    assert!(world.resource::<VnEngine>().0.preview_game_audio_rollback().is_err());
}
