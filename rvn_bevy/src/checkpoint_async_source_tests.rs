use super::*;
use bevy::{
    asset::io::{
        memory::{Dir, MemoryAssetReader},
        AssetReader, AssetReaderError, PathStream, Reader,
    },
    audio::Decodable,
};
use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    task::{Poll, Waker},
    time::{Duration, Instant},
};

#[derive(Default)]
struct ReadGate {
    opened: AtomicBool,
    reads: AtomicUsize,
    wakers: Mutex<Vec<Waker>>,
}
impl ReadGate {
    fn open(&self) {
        self.opened.store(true, Ordering::SeqCst);
        for waker in self.wakers.lock().unwrap().drain(..) {
            waker.wake();
        }
    }
    async fn wait(&self) {
        self.reads.fetch_add(1, Ordering::SeqCst);
        std::future::poll_fn(|cx| {
            if self.opened.load(Ordering::SeqCst) {
                return Poll::Ready(());
            }
            let mut wakers = self.wakers.lock().unwrap();
            if !wakers.iter().any(|old| old.will_wake(cx.waker())) {
                wakers.push(cx.waker().clone());
            }
            if self.opened.load(Ordering::SeqCst) {
                Poll::Ready(())
            } else {
                Poll::Pending
            }
        })
        .await
    }
}
#[derive(Clone)]
struct DeferredReader {
    inner: MemoryAssetReader,
    gate: Arc<ReadGate>,
}
impl AssetReader for DeferredReader {
    async fn read<'a>(&'a self, path: &'a Path) -> Result<Box<Reader<'a>>, AssetReaderError> {
        self.gate.wait().await;
        self.inner.read(path).await
    }
    async fn read_meta<'a>(&'a self, path: &'a Path) -> Result<Box<Reader<'a>>, AssetReaderError> {
        self.inner.read_meta(path).await
    }
    async fn read_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<Box<PathStream>, AssetReaderError> {
        self.inner.read_directory(path).await
    }
    async fn is_directory<'a>(&'a self, path: &'a Path) -> Result<bool, AssetReaderError> {
        self.inner.is_directory(path).await
    }
}
const PATHS: [&str; 3] = ["music/real.wav", "sfx/real.wav", "voice/real.wav"];
const STORY: &str = r#"init{music.play("music/real.wav")}
scene label P
sfx.play("sfx/real.wav") with fade(1000)
voice "voice/real.wav"
"parent"
call C
"returned"
return
scene label C
"child"
return"#;
fn app() -> (App, Arc<ReadGate>) {
    let directory = Dir::default();
    for path in PATHS {
        directory.insert_asset(Path::new(path), super::source_contract_tests::wave());
    }
    let gate = Arc::new(ReadGate::default());
    let reader = DeferredReader {
        inner: MemoryAssetReader { root: directory },
        gate: gate.clone(),
    };
    let mut app = App::new();
    app.register_asset_source(
        bevy::asset::io::AssetSourceId::Default,
        bevy::asset::io::AssetSource::build().with_reader(move || Box::new(reader.clone())),
    );
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin {
            watch_for_changes_override: Some(false),
            ..default()
        },
    ));
    app.init_asset::<AudioSource>()
        .init_asset_loader::<bevy::audio::AudioLoader>()
        .init_asset::<CheckpointPcm>();
    let mut game = rvn_core::Engine::from_program(
        rvn_parser::parse_program(STORY).unwrap(),
        crate::bevy_renderer::BevyRenderer::new(),
        8,
    )
    .unwrap();
    game.step_until_interaction().unwrap();
    app.insert_resource(crate::resources::VnEngine(game))
        .insert_resource(State::new(crate::resources::VnState::Waiting))
        .init_resource::<NextState<crate::resources::VnState>>()
        .init_resource::<crate::resources::VnRenderState>()
        .init_resource::<crate::resources::ChoiceFocus>()
        .init_resource::<crate::resources::ImagemapState>()
        .init_resource::<crate::resources::TypewriterState>()
        .init_resource::<crate::accessibility::Accessibility>()
        .init_resource::<crate::resources::ScriptErrorMessage>()
        .init_resource::<crate::resources::MusicEntity>()
        .init_resource::<crate::resources::MusicAssetRegistry>()
        .init_resource::<crate::resources::MusicPlaybackState>()
        .init_resource::<crate::resources::MusicVolume>()
        .init_resource::<crate::systems::settings_menu::Settings>()
        .init_resource::<crate::scene_publication::ScenePresentation>()
        .init_resource::<crate::scene_audio::AudioFeedbackQueue>()
        .add_event::<crate::vn_command::VnCommand>()
        .add_event::<crate::vn_command::PlayerInput>();
    (app, gate)
}
fn unchanged(world: &World) -> serde_json::Value {
    let engine = &world.resource::<crate::resources::VnEngine>().0;
    serde_json::json!({"state":engine.state,"history":format!("{:?}",engine.history.entries()),"epoch":engine.runtime_epoch(),"interface":engine.interface_epoch(),"instance":engine.instance_id(),"outputs":format!("{:?}",engine.renderer.pending),"batch":format!("{:?}",engine.renderer.scene_batch)})
}
fn loaded_without_retaining_test_handles(app: &mut App) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        app.update();
        let ready = PATHS.iter().all(|path| {
            app.world()
                .resource::<AssetServer>()
                .get_handle::<AudioSource>(*path)
                .is_some_and(|handle| {
                    app.world()
                        .resource::<Assets<AudioSource>>()
                        .get(&handle)
                        .is_some()
                })
        });
        if ready {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "actual async AudioLoader never delivered the retained files"
        );
        std::thread::yield_now();
    }
    // No test-owned Handle survives the expression above. PreUpdate includes
    // real handle-drop tracking; the broker alone retains each loading asset.
    for _ in 0..8 {
        app.update();
    }
    assert_eq!(app.world().resource::<Assets<AudioSource>>().len(), 3);
}
fn publish(world: &mut World) {
    crate::scene_publication::consume(world);
    let engine = &world.resource::<crate::resources::VnEngine>().0;
    assert!(
        !engine.renderer.scene_frame_failed,
        "{}",
        world.resource::<crate::resources::ScriptErrorMessage>().0
    );
    assert!(!engine.renderer.scene_batch_pending());
}
fn shot_reader(world: &World) -> CheckpointPcm {
    let target = world
        .resource::<crate::resources::VnEngine>()
        .0
        .audio_playback_views()
        .unwrap()
        .iter()
        .find(|view| view.playback.family == rvn_core::audio_playback::AudioFamily::Sfx)
        .unwrap()
        .token
        .target
        .clone();
    world.resource::<PreparedSourceCache>().entries[&target]
        .reader
        .clone()
}
#[test]
fn actual_async_sources_remain_loaded_across_preupdate_and_publish_real_pcm_without_injected_assets(
) {
    let (mut app, gate) = app();
    let before = unchanged(app.world());
    crate::scene_publication::consume(app.world_mut());
    for _ in 0..5 {
        app.update();
    }
    assert_eq!(unchanged(app.world()), before);
    assert!(app
        .world()
        .resource::<crate::resources::VnEngine>()
        .0
        .renderer
        .scene_batch_pending());
    assert!(app.world().get_resource::<PreparedSourceCache>().is_none());
    gate.open();
    loaded_without_retaining_test_handles(&mut app);
    publish(app.world_mut());
    let reader = shot_reader(app.world());
    let mut decoder = reader.decoder();
    assert_eq!(
        [decoder.next(), decoder.next(), decoder.next()],
        [Some(100), Some(200), Some(300)]
    );
    let saved = rvn_core::save::SaveData::try_from_snapshot(
        &app.world()
            .resource::<crate::resources::VnEngine>()
            .0
            .snapshot_for_save()
            .unwrap(),
        1,
        "actual".into(),
        "main.rvn".into(),
    )
    .unwrap();
    let audio = saved
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .tasks
        .iter()
        .find_map(|task| {
            task.audio
                .as_ref()
                .filter(|audio| audio.family == rvn_core::audio_playback::AudioFamily::Sfx)
        })
        .unwrap();
    assert_eq!(
        (
            audio.checkpoint.as_ref().unwrap().next_sample,
            audio.checkpoint.as_ref().unwrap().next_channel
        ),
        (3, 1)
    );
    assert_eq!(saved.format_version, 11);
    assert_eq!(
        saved
            .scene_execution
            .as_ref()
            .unwrap()
            .resources
            .music_checkpoints
            .len(),
        1
    );
    assert!(
        app.world()
            .resource::<crate::resources::VnEngine>()
            .0
            .renderer
            .audio_checkpoints
            .preparation_asset(PATHS[0])
            .is_err(),
        "accepted exact SourceEntries now own the raw handles"
    );
    for _ in 0..8 {
        app.update();
    }
    assert_eq!(app.world().resource::<Assets<AudioSource>>().len(), 3);
}
#[test]
fn actual_async_cold_restore_retains_assets_without_core_commit_then_restores_the_measured_stereo_cursor(
) {
    let (mut original, gate) = app();
    crate::scene_publication::consume(original.world_mut());
    gate.open();
    loaded_without_retaining_test_handles(&mut original);
    publish(original.world_mut());
    let actual = shot_reader(original.world());
    let mut decoder = actual.decoder();
    assert_eq!(
        [decoder.next(), decoder.next(), decoder.next()],
        [Some(100), Some(200), Some(300)]
    );
    let saved = rvn_core::save::SaveData::try_from_snapshot(
        &original
            .world()
            .resource::<crate::resources::VnEngine>()
            .0
            .snapshot_for_save()
            .unwrap(),
        1,
        "actual".into(),
        "main.rvn".into(),
    )
    .unwrap();
    let (mut cold, gate) = app();
    let before = unchanged(cold.world());
    assert!(!warm_restore(cold.world(), &saved).unwrap());
    for _ in 0..5 {
        cold.update();
    }
    assert_eq!(unchanged(cold.world()), before);
    gate.open();
    loaded_without_retaining_test_handles(&mut cold);
    assert!(warm_restore(cold.world(), &saved).unwrap());
    assert_eq!(unchanged(cold.world()), before);
    cold.world_mut()
        .resource_mut::<crate::resources::VnEngine>()
        .0
        .load_data(saved)
        .unwrap();
    publish(cold.world_mut());
    let restored = shot_reader(cold.world());
    let cursor = restored
        .gate()
        .snapshot(&[&restored], |mut rows| Ok(rows.remove(0)))
        .unwrap();
    assert_eq!((cursor.next_sample, cursor.next_channel), (3, 1));
    let mut output = restored.decoder();
    assert_eq!(
        output.next(),
        Some(0),
        "explicit channel-alignment padding is not a consumed sample"
    );
    assert_eq!(
        restored
            .gate()
            .snapshot(&[&restored], |mut rows| Ok(rows.remove(0)))
            .unwrap()
            .next_sample,
        3
    );
    assert_eq!(output.next(), Some(400));
}
#[test]
fn preparation_handle_paths_are_bounded_pruned_and_rejected_atomically() {
    let (app, _gate) = app();
    let assets = app.world().resource::<AssetServer>();
    let broker = app
        .world()
        .resource::<crate::resources::VnEngine>()
        .0
        .renderer
        .audio_checkpoints
        .clone();
    let paths: Vec<_> = PATHS.iter().map(|path| path.to_string()).collect();
    broker
        .retain_preparation_assets(Some(assets), &paths)
        .unwrap();
    let ids: Vec<_> = paths
        .iter()
        .map(|path| broker.preparation_asset(path).unwrap().id())
        .collect();
    assert!(broker
        .retain_preparation_assets(Some(assets), &["../outside.wav".into()])
        .is_err());
    let excess: Vec<_> = (0..rvn_core::audio_playback::MAX_AUDIO_PLAYBACKS
        + rvn_core::scene_resources::MAX_MOTIONS
        + 2)
        .map(|n| format!("sfx/{n}.wav"))
        .collect();
    assert!(broker
        .retain_preparation_assets(Some(assets), &excess)
        .is_err());
    assert_eq!(
        paths
            .iter()
            .map(|path| broker.preparation_asset(path).unwrap().id())
            .collect::<Vec<_>>(),
        ids
    );
    broker
        .retain_preparation_assets(Some(assets), &[paths[0].clone(), paths[0].clone()])
        .unwrap();
    assert_eq!(broker.preparation_asset(&paths[0]).unwrap().id(), ids[0]);
    assert!(broker.preparation_asset(&paths[1]).is_err());
    broker.retain_preparation_assets(None, &[]).unwrap();
    assert!(broker.preparation_asset(&paths[0]).is_err());
}
