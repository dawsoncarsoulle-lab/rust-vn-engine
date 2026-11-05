use super::*;
use bevy::audio::Decodable;
fn pcm_fixture()->(Fixture,rvn_parser::ParsedProgram,rvn_core::save::consumer_checkpoint::Save15,PortableTextHostReceipt,crate::checkpoint_pcm::CheckpointPcm,crate::checkpoint_pcm::PcmSource){
    let (mut f,_,_,_)=atomic_save_fixture();
    fs::create_dir_all(f.root.join("assets/sfx")).unwrap();
    fs::write(f.root.join("assets/sfx/actual.wav"),crate::checkpoint_sources::fixture_wave_for_test()).unwrap();
    fs::create_dir_all(f.root.join("assets/music")).unwrap();
    fs::write(f.root.join("assets/music/actual.wav"),crate::checkpoint_sources::fixture_wave_for_test()).unwrap();
    fs::write(f.root.join("assets/actual.wav"),crate::checkpoint_sources::fixture_wave_for_test()).unwrap();
    f.app.init_asset::<AudioSource>().init_asset_loader::<bevy::audio::AudioLoader>().init_asset::<crate::checkpoint_pcm::CheckpointPcm>();
    f.app.init_resource::<crate::resources::ScriptErrorMessage>().init_resource::<crate::resources::MusicEntity>().init_resource::<crate::resources::MusicAssetRegistry>();
    let source="init{music.play(\"actual.wav\")}\nscene label start\nflow.consume_v2(\"scene:start\",\"roll\"){set roll=random(1,9)}\nsfx.play(\"actual.wav\")\nvoice \"actual.wav\"\n\"First\"\n\"after\"\n";
    let path=f.root.join("actual.rvn");fs::write(&path,source).unwrap();
    let mut program=rvn_parser::parse_program(&fs::read_to_string(&path).unwrap()).unwrap();
    program.sources[0].path=Some(path.clone());for origin in &mut program.origins{origin.source=Some(path.clone());}
    let current=&f.app.world().resource::<VnEngine>().0;
    let resources=current.text_resources().clone();let mut locale=current.locale.as_ref().unwrap().clone();locale.set_language("fr").unwrap();
    let mut source=Engine::from_program_with_persistent_and_text_context(program.clone(),BevyRenderer::new(),8,HashMap::new(),rvn_core::TextRuntimeContext::new(resources,Some(locale))).unwrap();
    source.step_until_interaction().unwrap();f.app.world_mut().insert_resource(VnEngine(source));
    f.app.world_mut().resource_mut::<crate::systems::settings_menu::Settings>().language="fr".into();
    let deadline=Instant::now()+Duration::from_secs(15);
    loop{
        f.app.update();crate::scene_publication::consume(f.app.world_mut());
        assert!(!f.app.world().resource::<VnEngine>().0.renderer.scene_frame_failed,"Actual fixture publication failed: {}",f.app.world().resource::<crate::resources::ScriptErrorMessage>().0);
        if !f.app.world().resource::<VnEngine>().0.renderer.scene_batch_pending(){break;}
        assert!(Instant::now()<deadline,"Real WAV loading timed out");std::thread::sleep(Duration::from_millis(5));
    }
    f.app.world_mut().resource_mut::<Events<VnCommand>>().clear();
    let target=f.app.world().resource::<VnEngine>().0.audio_playback_views().unwrap()[0].token.target.clone();
    let original=crate::checkpoint_sources::one_shot_source(f.app.world(),&target).unwrap().0;
    let mut decoder=original.decoder();assert_eq!([decoder.next(),decoder.next(),decoder.next()],[Some(100),Some(200),Some(300)]);
    let save=f.app.world().resource::<VnEngine>().0.capture_save15(&program,None,1,"real stereo sample three".into()).unwrap();
    let save=rvn_core::save::consumer_checkpoint::Save15::decode(&save.encode().unwrap()).unwrap();
    let expected=portable_receipt(f.app.world()).unwrap();
    (f,program,save,expected,original,decoder)
}
#[test]
fn actual_world_commit_restores_stereo_cursor_and_installs_candidate_before_return(){
    let(mut f,p,save,expected,old,mut old_decoder)=pcm_fixture();
    let before=atomic_world_before(&f);let plan=crate::atomic_text_restore::prepare(f.app.world(),&p,save,&expected).unwrap();
    assert_eq!(atomic_world_before(&f),before);
    let session=crate::atomic_text_restore::commit(f.app.world_mut(),plan).unwrap();assert!(session.is_none());
    let engine=&f.app.world().resource::<VnEngine>().0;assert!(!engine.renderer.scene_batch_pending());assert_eq!(engine.runtime_epoch(),before.3+1);
    let installed=f.app.world().resource::<crate::scene_publication::ScenePresentation>().batch.as_ref().unwrap();
    assert_eq!(serde_json::to_value(&installed.state).unwrap(),serde_json::to_value(&engine.state).unwrap());
    assert_eq!(serde_json::to_value(&engine.state.random).unwrap(),before.0["random"]);
    let music=f.app.world().resource::<crate::resources::MusicEntity>().0.expect("Actual Music Source entity installed synchronously");
    assert!(f.app.world().get::<Handle<crate::checkpoint_pcm::CheckpointPcm>>(music).is_some());
    assert!(f.app.world().get::<crate::checkpoint_sources::ExactMusicSource>(music).is_some());
    let target=engine.audio_playback_views().unwrap()[0].token.target.clone();
    let restored=crate::checkpoint_sources::one_shot_source(f.app.world(),&target).unwrap().0;
    assert!(!restored.same_reader(&old));let cursor=restored.gate().snapshot(&[&restored],|mut rows|Ok(rows.remove(0))).unwrap();assert_eq!((cursor.next_sample,cursor.next_channel),(3,1));
    let mut decoder=restored.decoder();assert_eq!([decoder.next(),decoder.next(),decoder.next()],[Some(0),Some(400),Some(500)]);
    assert_eq!(old_decoder.next(),None,"Outgoing reader must be stopped after synchronous install");
}
#[test]
fn changed_actual_pcm_bytes_refuse_before_core_settings_entities_or_pending_publication(){
    let(mut f,p,save,expected,_old,mut old_decoder)=pcm_fixture();
    let plan=crate::atomic_text_restore::prepare(f.app.world(),&p,save,&expected).unwrap();
    let handle=f.app.world().resource::<AssetServer>().get_handle::<AudioSource>("actual.wav").unwrap();
    f.app.world_mut().resource_mut::<Assets<AudioSource>>().get_mut(handle.id()).unwrap().bytes=std::sync::Arc::from(b"invalid replaced actual source".to_vec());
    let before=atomic_world_before(&f);let entities=f.app.world().entities().len();
    assert!(crate::atomic_text_restore::commit(f.app.world_mut(),plan).is_err());assert_eq!(atomic_world_before(&f),before);assert_eq!(f.app.world().entities().len(),entities);
    assert_eq!(old_decoder.next(),Some(400));
}
#[test]
fn outgoing_stopped_reader_refuses_gate_before_core_commit(){
    let(mut f,p,save,expected,old,_old_decoder)=pcm_fixture();let plan=crate::atomic_text_restore::prepare(f.app.world(),&p,save,&expected).unwrap();
    old.stop().unwrap();let before=atomic_world_before(&f);let entities=f.app.world().entities().len();
    assert!(crate::atomic_text_restore::commit(f.app.world_mut(),plan).is_err());assert_eq!(atomic_world_before(&f),before);assert_eq!(f.app.world().entities().len(),entities);
}

#[test]
fn cold_world_actual_asset_loading_defers_without_effect_then_restores_saved_pcm(){
    let(source,p,save,_,_old,_decoder)=pcm_fixture();
    let saved_random=source.app.world().resource::<VnEngine>().0.state.random.clone();
    let saved_vars=source.app.world().resource::<VnEngine>().0.state.vars.clone();
    let mut cold=fixture();ready(&mut cold);
    fs::create_dir_all(cold.root.join("assets/sfx")).unwrap();
    fs::write(cold.root.join("assets/sfx/actual.wav"),crate::checkpoint_sources::fixture_wave_for_test()).unwrap();
    fs::create_dir_all(cold.root.join("assets/music")).unwrap();
    fs::write(cold.root.join("assets/music/actual.wav"),crate::checkpoint_sources::fixture_wave_for_test()).unwrap();
    fs::write(cold.root.join("assets/actual.wav"),crate::checkpoint_sources::fixture_wave_for_test()).unwrap();
    cold.app.init_asset::<AudioSource>().init_asset_loader::<bevy::audio::AudioLoader>().init_asset::<crate::checkpoint_pcm::CheckpointPcm>();
    cold.app.init_resource::<crate::scene_publication::ScenePresentation>().init_resource::<crate::resources::MusicEntity>().init_resource::<crate::resources::MusicAssetRegistry>();
    let engine=&cold.app.world().resource::<VnEngine>().0;
    let live=Engine::from_program_with_persistent_and_text_context(p.clone(),BevyRenderer::new(),8,HashMap::new(),rvn_core::TextRuntimeContext::new(engine.text_resources().clone(),engine.locale.clone())).unwrap();
    assert_ne!(live.instance_id(),source.app.world().resource::<VnEngine>().0.instance_id());
    cold.app.world_mut().insert_resource(VnEngine(live));let mut settings=crate::systems::settings_menu::Settings::default();settings.language="en".into();cold.app.world_mut().insert_resource(settings);
    // Constructor Init has real Music publication; settle it through the
    // historical pipeline before asking the cold host for a portable boundary.
    let initial_deadline=Instant::now()+Duration::from_secs(15);
    while cold.app.world().resource::<VnEngine>().0.renderer.scene_batch_pending(){
        cold.app.update();crate::scene_publication::consume(cold.app.world_mut());
        assert!(!cold.app.world().resource::<VnEngine>().0.renderer.scene_frame_failed);
        assert!(Instant::now()<initial_deadline,"Actual cold Init Music did not settle");std::thread::sleep(Duration::from_millis(5));
    }
    cold.app.world_mut().resource_mut::<Events<VnCommand>>().clear();
    let mut expected=portable_receipt(cold.app.world()).unwrap();expected.locales=save.locale.as_ref().unwrap().clone();
    let bytes=save.encode().unwrap();let before=atomic_world_before(&cold);let entities=cold.app.world().entities().len();
    let result=crate::atomic_text_restore::prepare(cold.app.world(),&p,rvn_core::save::consumer_checkpoint::Save15::decode(&bytes).unwrap(),&expected);
    assert!(matches!(result,Err(ref message) if message.contains("still loading")));assert_eq!(atomic_world_before(&cold),before);assert_eq!(cold.app.world().entities().len(),entities);
    let deadline=Instant::now()+Duration::from_secs(15);
    let plan=loop {
        cold.app.update();
        match crate::atomic_text_restore::prepare(cold.app.world(),&p,rvn_core::save::consumer_checkpoint::Save15::decode(&bytes).unwrap(),&expected){
            Ok(plan)=>break plan,
            Err(message) if message.contains("still loading")=>{},
            Err(message)=>panic!("Actual cold preparation failed: {message}"),
        }
        assert!(Instant::now()<deadline,"Actual cold WAV admission timed out");std::thread::sleep(Duration::from_millis(5));
    };
    assert_eq!(atomic_world_before(&cold),before);
    crate::atomic_text_restore::commit(cold.app.world_mut(),plan).unwrap();
    let live=&cold.app.world().resource::<VnEngine>().0;assert_eq!(live.state.random,saved_random);assert_eq!(live.state.vars,saved_vars);
    assert_eq!(cold.app.world().resource::<crate::systems::settings_menu::Settings>().language,"fr");assert!(!live.renderer.scene_batch_pending());
    let target=live.audio_playback_views().unwrap()[0].token.target.clone();let reader=crate::checkpoint_sources::one_shot_source(cold.app.world(),&target).unwrap().0;
    let observed=reader.gate().snapshot(&[&reader],|mut rows|Ok(rows.remove(0))).unwrap();assert_eq!((observed.next_sample,observed.next_channel),(3,1));
    let mut decoded=reader.decoder();assert_eq!([decoded.next(),decoded.next(),decoded.next()],[Some(0),Some(400),Some(500)]);
}

#[test]
fn missing_actual_pcm_asset_runtime_refuses_before_authoritative_mutation(){
    let(mut f,p,save,expected,_old,mut decoder)=pcm_fixture();let plan=crate::atomic_text_restore::prepare(f.app.world(),&p,save,&expected).unwrap();
    f.app.world_mut().remove_resource::<Assets<crate::checkpoint_pcm::CheckpointPcm>>();let before=atomic_world_before(&f);let entities=f.app.world().entities().len();
    assert!(crate::atomic_text_restore::commit(f.app.world_mut(),plan).is_err());assert_eq!(atomic_world_before(&f),before);assert_eq!(f.app.world().entities().len(),entities);assert_eq!(decoder.next(),Some(400));
}
#[test]
fn unsettled_renderer_output_refuses_before_publication_and_retains_output(){
    let(mut f,p,save,expected)=atomic_save_fixture();let plan=crate::atomic_text_restore::prepare(f.app.world(),&p,save,&expected).unwrap();
    f.app.world_mut().resource_mut::<VnEngine>().0.renderer.pending.push(VnCommand::MusicStop);
    let before=atomic_world_before(&f);let entities=f.app.world().entities().len();
    assert!(crate::atomic_text_restore::commit(f.app.world_mut(),plan).is_err());assert_eq!(atomic_world_before(&f),before);assert_eq!(f.app.world().entities().len(),entities);
    assert!(matches!(f.app.world().resource::<VnEngine>().0.renderer.pending.as_slice(),[VnCommand::MusicStop]));
}

#[cfg(all(feature="video",not(target_arch="wasm32")))]
fn video_admission_fixture(saved_video:bool,current_video:bool)->(Fixture,rvn_parser::ParsedProgram,rvn_core::save::consumer_checkpoint::Save15,PortableTextHostReceipt){
    let(mut f,_,_,_)=atomic_save_fixture();
    let text="label start\nflow.consume_v2(\"legacy\",\"movie\"){video.play(\"intro\",video_clip(\"clip.webm\",{}))}\n\"First\"\n";
    let path=f.root.join("movie.rvn");fs::write(&path,text).unwrap();let mut p=rvn_parser::parse_program(&fs::read_to_string(&path).unwrap()).unwrap();p.sources[0].path=Some(path.clone());for origin in &mut p.origins{origin.source=Some(path.clone());}
    let context=rvn_core::TextRuntimeContext::new(f.app.world().resource::<VnEngine>().0.text_resources().clone(),f.app.world().resource::<VnEngine>().0.locale.clone());
    let mut source=Engine::from_program_with_persistent_and_text_context(p.clone(),BevyRenderer::new(),8,HashMap::new(),context.clone()).unwrap();
    if saved_video{source.step_silent().unwrap();assert!(!source.state.videos.tracks.is_empty());}
    let save=source.capture_save15(&p,None,1,"video admission refusal".into()).unwrap();
    let mut current=Engine::from_program_with_persistent_and_text_context(p.clone(),BevyRenderer::new(),8,HashMap::new(),context).unwrap();
    if current_video{current.step_silent().unwrap();assert!(!current.state.videos.tracks.is_empty());}
    f.app.world_mut().insert_resource(VnEngine(current));let expected=portable_receipt(f.app.world()).unwrap();(f,p,save,expected)
}
#[cfg(all(feature="video",not(target_arch="wasm32")))]
#[test]
fn candidate_video_is_refused_before_warming_publication_or_ecs_mutation(){
    let(f,p,save,expected)=video_admission_fixture(true,false);let before=atomic_world_before(&f);let entities=f.app.world().entities().len();
    let result=crate::atomic_text_restore::prepare(f.app.world(),&p,save,&expected);
    assert!(matches!(result,Err(ref message) if message.contains("ProjectPaths")));assert_eq!(atomic_world_before(&f),before);assert_eq!(f.app.world().entities().len(),entities);
}
#[cfg(all(feature="video",not(target_arch="wasm32")))]
#[test]
fn outgoing_video_is_refused_before_warming_publication_or_ecs_mutation(){
    let(f,p,save,expected)=video_admission_fixture(false,true);let before=atomic_world_before(&f);let entities=f.app.world().entities().len();
    let result=crate::atomic_text_restore::prepare(f.app.world(),&p,save,&expected);
    assert!(matches!(result,Err(ref message) if message.contains("unsettled outgoing publication")));assert_eq!(atomic_world_before(&f),before);assert_eq!(f.app.world().entities().len(),entities);
}
#[cfg(all(feature="video",not(target_arch="wasm32")))]
#[test]
fn actual_resident_video_audio_source_refuses_commit_without_pausing_or_changing_buffer(){
    let(mut f,p,save,expected)=atomic_save_fixture();let plan=crate::atomic_text_restore::prepare(f.app.world(),&p,save,&expected).unwrap();
    f.app.init_asset::<crate::video_audio::VideoAudio>();
    let source=crate::video_audio::VideoAudio(std::sync::Arc::new(std::sync::Mutex::new(crate::video_audio::Buffer::new(3.5))));
    let handle=f.app.world_mut().resource_mut::<Assets<crate::video_audio::VideoAudio>>().add(source.clone());
    let before=atomic_world_before(&f);let entities=f.app.world().entities().len();
    let result=crate::atomic_text_restore::commit(f.app.world_mut(),plan);
    assert!(matches!(result,Err(ref message) if message.contains("foreign/unbound owner")));assert_eq!(atomic_world_before(&f),before);assert_eq!(f.app.world().entities().len(),entities);
    let buffer=source.0.lock().unwrap();assert_eq!(buffer.position,3.5);assert!(!buffer.running);assert_eq!(buffer.len(),0);assert!(f.app.world().resource::<Assets<crate::video_audio::VideoAudio>>().get(&handle).is_some());
}
