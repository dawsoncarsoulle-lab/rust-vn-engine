//! PREPARED / NOT_RUN. Real accepted Startup one-shot reader with the actual
//! SceneAudioPlugin. No AudioPlugin, device callback or native EOF is claimed.
use super::*;
use crate::debugger_startup::DebugBootstrap;
use crate::presentation_order::startup_owner_tests::{app,commit_one};

#[test]
fn startup_audio_projection_true_scene_audio_plugin_retains_published_reader_and_pcm_cursor() {
    let (_,mut app)=app("init {sfx.play(\"delayed.wav\") set hold=1}\nscene label start\n\"Ready\"\n");
    commit_one(&mut app);
    let players=app.world().resource::<AudioPlayers>();
    assert_eq!(players.entries.len(),1);
    let (identity,reader)=players.entries.iter().next().unwrap();
    let identity=identity.clone();
    let original=reader.checkpoint.as_ref().expect("real accepted PCM reader").clone();
    let original_entity=reader.entity.expect("actual refresh attaches its owned source entity");
    let original_handle=reader.pcm.as_ref().unwrap().id();
    let original_view=reader.view.clone();
    let state=serde_json::to_vec(app.world().resource::<DebugBootstrap>().owner.state()).unwrap();
    let source=app.world().resource::<DebugBootstrap>().owner.observation_source().unwrap();
    let before=original.gate().snapshot(&[&original],|rows|Ok(rows)).unwrap();
    for _ in 0..4{app.update();}
    let players=app.world().resource::<AudioPlayers>();
    let reader=players.entries.get(&identity).unwrap();
    assert!(reader.checkpoint.as_ref().unwrap().same_reader(&original));
    assert_eq!(reader.entity,Some(original_entity));
    assert_eq!(reader.pcm.as_ref().unwrap().id(),original_handle);
    assert_eq!(reader.view,original_view);
    assert!(app.world().get::<OwnedAudio>(original_entity).is_some());
    assert!(app.world_mut().query_filtered::<Entity, With<crate::components::SfxSource>>().iter(app.world()).next().is_none(), "accepted modern one-shot must not also start a legacy player");
    let after=original.gate().snapshot(&[&original],|rows|Ok(rows)).unwrap();
    assert_eq!(after,before);
    assert_eq!(serde_json::to_vec(app.world().resource::<DebugBootstrap>().owner.state()).unwrap(),state);
    assert!(app.world().resource::<DebugBootstrap>().owner.accepts_observation_source(&source));
    assert!(!app.world().contains_resource::<crate::resources::VnEngine>());
}

#[test]
fn startup_audio_projection_music_handler_keeps_the_exact_installed_pcm_entity_without_legacy_replay() {
    let (_,mut app)=app("init {music.play(\"host.wav\") set hold=1}\nscene label start\n\"Ready\"\n");
    commit_one(&mut app);
    let entity=app.world().resource::<crate::resources::MusicEntity>().0.unwrap();
    let handle=app.world().get::<Handle<crate::checkpoint_pcm::CheckpointPcm>>(entity).expect("real checkpoint Music source").clone();
    let original=app.world().resource::<Assets<crate::checkpoint_pcm::CheckpointPcm>>().get(&handle).unwrap().clone();
    let target=app.world().resource::<DebugBootstrap>().owner.state().audio_resource_registry().unwrap()
        .target(&rvn_core::scene_tasks::TaskOwner::Game,rvn_core::scene_resources::TargetKind::Music,"music").unwrap().clone();
    assert_eq!(crate::checkpoint_sources::music_entity_for_test(app.world(),&target),Some(entity));
    let before=original.gate().snapshot(&[&original],|rows|Ok(rows)).unwrap();
    for _ in 0..4 {app.update();}
    assert_eq!(app.world().resource::<crate::resources::MusicEntity>().0,Some(entity));
    assert_eq!(app.world().get::<Handle<crate::checkpoint_pcm::CheckpointPcm>>(entity).unwrap().id(),handle.id());
    assert!(app.world().resource::<Assets<crate::checkpoint_pcm::CheckpointPcm>>().get(&handle).unwrap().same_reader(&original));
    assert!(app.world_mut().query_filtered::<Entity,(With<crate::components::MusicMarker>,With<Handle<AudioSource>>)>().iter(app.world()).next().is_none(),"accepted music must not be replaced by an uncheckpointed legacy AudioSource");
    assert_eq!(original.gate().snapshot(&[&original],|rows|Ok(rows)).unwrap(),before);
    assert!(!app.world().contains_resource::<crate::resources::VnEngine>());
}

#[test]
fn startup_audio_projection_failed_owner_cannot_dispose_or_republish_accepted_reader() {
    let (_,mut app)=app("init {sfx.play(\"delayed.wav\") set hold=1}\nscene label start\n\"Ready\"\n");
    commit_one(&mut app);
    let players=app.world().resource::<AudioPlayers>();
    let (identity,reader)=players.entries.iter().next().unwrap();
    let identity=identity.clone();let original=reader.checkpoint.as_ref().unwrap().clone();let entity=reader.entity;
    let before=serde_json::to_vec(app.world().resource::<DebugBootstrap>().owner.state()).unwrap();
    // Negative host failure injection cannot grant a Core/media permission.
    app.world_mut().resource_mut::<DebugBootstrap>().failed=Some("fixture failed publication".into());
    for _ in 0..3{app.update();}
    let reader=app.world().resource::<AudioPlayers>().entries.get(&identity).unwrap();
    assert!(reader.checkpoint.as_ref().unwrap().same_reader(&original));
    assert_eq!(reader.entity,entity);
    assert_eq!(serde_json::to_vec(app.world().resource::<DebugBootstrap>().owner.state()).unwrap(),before);
}
