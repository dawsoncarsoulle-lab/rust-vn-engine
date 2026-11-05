//! Pending CPU schedule tests. Never a claim of DefaultPlugins GPU/device launch.
use super::*;
use crate::debugger_startup::DebugBootstrap;
use std::time::SystemTime;

// A real parsed SourceWorkspace/AssetServer/closed text provider owner from the
// existing World harness. No VnEngine is fabricated to satisfy SystemParams.
fn app() -> (PathBuf, App) {
    app_source("screen published(){return component(\"root\",\"panel\",{},[component(\"message\",\"text\",{\"text\":\"Startup visible\"},[])])}\ninit {ui.open(\"published\",[],false,0) set boot=7}\nscene label start\n\"Done\"\n")
}
pub(crate) fn app_source(source: &str) -> (PathBuf, App) {
    app_source_with_ui(source, false)
}
pub(crate) fn app_source_with_complete_cpu_ui(source: &str) -> (PathBuf, App) {
    app_source_with_ui(source, true)
}
fn app_source_with_ui(source: &str, complete_cpu_ui: bool) -> (PathBuf, App) {
    // Every registrar/plugin must precede the first Main startup pass.
    // The separate historical World helper remains pumped for its own tests.
    let (root, app) = crate::debugger_startup::tests::fixture_source_unpumped(source);
    complete_cpu_runtime(root,app,complete_cpu_ui)
}
pub(crate) fn complete_cpu_runtime(root:PathBuf,mut app:App,complete_cpu_ui:bool)->(PathBuf,App){
    let manager = PersistentDataManager::new(root.join("saves")).unwrap();
    app.add_plugins(bevy::state::app::StatesPlugin)
        .init_asset::<Image>().init_asset::<Font>().init_asset::<Shader>().init_asset::<Mesh>()
        .insert_resource(resources::Theme::default())
        .insert_resource(resources::ThemeWatcher::new(root.join("theme.toml").to_string_lossy().into_owned()))
        .insert_resource(resources::ProjectTitle("Actual Startup schedule".into()))
        .insert_resource(resources::NarrativeStart(Some("start".into())))
        .insert_resource(project_paths::ProjectPaths::new(root.clone(), root.join("assets"), root.join("locales"), root.join("theme.toml"), root.join("saves")))
        .insert_resource(resources::PersistentDataResource { manager, data: PersistentData::default() })
        .insert_resource(resources::LocaleConfig { available_langs: vec!["fr".into(), "en".into()], last_modified_current: SystemTime::UNIX_EPOCH, last_modified_default: SystemTime::UNIX_EPOCH })
        .insert_resource(Settings { language: "en".into(), ..default() })
        .init_resource::<resources::VnRenderState>().init_resource::<resources::SkipMode>()
        .init_resource::<resources::ImagemapState>().init_resource::<resources::MusicEntity>()
        .init_resource::<resources::MusicPlaybackState>().init_resource::<resources::MusicVolume>()
        .init_resource::<resources::TypewriterConfig>().init_resource::<resources::TypewriterState>()
        .init_resource::<resources::MenuState>().init_resource::<resources::GalleryState>()
        .init_resource::<resources::CharacterRegistry>().init_resource::<resources::CgAssetRegistry>()
        .init_resource::<resources::MusicAssetRegistry>().init_resource::<resources::DialogueHistory>()
        .init_resource::<resources::ChoiceFocus>().init_resource::<resources::ScriptErrorMessage>()
        .init_resource::<systems::save_menu::SaveMenuState>()
        .init_resource::<systems::settings_menu::SettingsMenuState>()
        .init_resource::<DebugOverlayState>().init_resource::<DebugStepRequest>()
        .init_resource::<editor_preview_input::Receipts>()
        .init_resource::<bevy::render::view::screenshot::ScreenshotManager>()
        .init_resource::<ButtonInput<KeyCode>>().init_resource::<ButtonInput<MouseButton>>()
        .init_resource::<ButtonInput<GamepadButton>>().init_resource::<bevy::input::touch::Touches>()
        .init_resource::<bevy::ui::UiScale>().init_resource::<bevy::ui::UiStack>()
        .init_resource::<bevy::a11y::Focus>()
        .add_event::<bevy::input::mouse::MouseWheel>()
        .add_event::<bevy::input::keyboard::KeyboardInput>()
        .add_event::<bevy::input::mouse::MouseButtonInput>()
        .add_event::<bevy::window::CursorMoved>()
        .add_event::<bevy::input::touch::TouchInput>()
        .add_event::<bevy::window::WindowFocused>()
        .add_event::<bevy::window::Ime>()
        .add_event::<PlayerInput>().add_event::<bevy::app::AppExit>()
        .init_state::<VnState>()
        .add_plugins(menu_documents::MenuDocumentsPlugin)
        .add_plugins(source_menus::SourceMenusPlugin);
    if complete_cpu_ui {
        app.init_asset::<bevy::sprite::TextureAtlasLayout>()
            .init_resource::<bevy::render::camera::ManualTextureViews>()
            .add_event::<bevy::window::WindowResized>()
            .add_event::<bevy::window::WindowCreated>()
            .add_event::<bevy::window::WindowScaleFactorChanged>()
            .add_plugins(bevy::input::InputPlugin)
            .add_plugins(bevy::transform::TransformPlugin)
            .add_plugins(bevy::render::camera::CameraPlugin)
            .add_plugins(bevy::render::view::VisibilityPlugin)
            .add_plugins(bevy::text::TextPlugin)
            .add_plugins(bevy::ui::UiPlugin);
    } else {
        // The same public initializer used by UiPlugin::build loads Bevy's
        // genuine shader. This smaller lane still makes no layout claim.
        bevy::ui::build_ui_render(&mut app);
    }
    app.add_plugins(programmable_ui::ProgrammableUiPlugin)
        .add_plugins(composed_motion::ComposedMotionPlugin)
        .add_plugins(layered_characters::LayeredCharactersPlugin)
        .add_plugins(custom_canvas::CustomCanvasPlugin)
        .add_plugins(display_reveal::DialogueRevealPlugin)
        .add_plugins(scene_publication::ScenePublicationPlugin);
    app.init_resource::<accessibility::Accessibility>();
    app.world_mut().spawn((Window::default(), bevy::window::PrimaryWindow));
    register_runtime_systems(&mut app);
    if !complete_cpu_ui {
        // The complete fixture adds its remaining real plugins before doing
        // this same first Main run. Do not replay Startup or synthesize fonts.
        let before = app.world().resource::<DebugBootstrap>().owner.snapshot();
        let before_source = app.world().resource::<DebugBootstrap>().owner.observation_source().unwrap();
        app.world_mut().resource_mut::<NextState<VnState>>().set(VnState::Stepping);
        app.finish();
        app.cleanup();
        let end = std::time::Instant::now() + std::time::Duration::from_secs(15);
        loop {
            app.update();
            if crate::portable_text_resources::live_receipt(app.world()).is_ok() { break; }
            assert!(std::time::Instant::now() < end, "Actual registrar text assets timed out");
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        let font = &app.world().resource::<menu_documents::MenuFont>().0;
        assert!(app.world().resource::<Assets<Font>>().contains(font.id()), "actual Startup must install the bundled MenuFont");
        assert_eq!(app.world().resource::<DebugBootstrap>().owner.snapshot(), before);
        assert!(app.world().resource::<DebugBootstrap>().owner.accepts_observation_source(&before_source));
        assert!(!app.world().contains_resource::<VnEngine>());
    }
    (root, app)
}

#[test]
fn startup_actual_runtime_registrar_and_plugins_keep_published_screen_without_game() {
    let (_, mut app) = app();
    assert!(!app.world().contains_resource::<resources::VnEngine>());
    let before = app.world().resource::<DebugBootstrap>().owner.snapshot();
    let source = app.world().resource::<DebugBootstrap>().owner.observation_source().unwrap();
    // This is the actual registered schedule, not a reimplementation of a guard.
    app.update();
    assert_eq!(app.world().resource::<DebugBootstrap>().owner.snapshot(), before);
    assert!(app.world().resource::<DebugBootstrap>().owner.accepts_observation_source(&source));
    let plan = debugger_startup::prepare_world_step(app.world(), rvn_core::editor_debugger::StepKind::Continue).unwrap();
    assert!(plan.ready());
    assert_eq!(plan.core().render_batch().unwrap().interfaces.len(), 1);
    let committed = debugger_startup::commit_world_step(app.world_mut(), plan).unwrap();
    debugger_startup::install_committed_world_step(app.world_mut(), committed);
    assert!(!app.world().resource::<DebugBootstrap>().publication_pending);
    let accepted = app.world().resource::<DebugBootstrap>().owner.observation_source().unwrap();
    let snapshot = app.world().resource::<DebugBootstrap>().owner.snapshot();
    let state_bytes = serde_json::to_vec(app.world().resource::<DebugBootstrap>().owner.state()).unwrap();
    let published = app.world().resource::<scene_publication::ScenePresentation>().batch.as_ref().unwrap().interfaces.clone();
    let views = app.world().resource::<programmable_ui::Screens>().views.clone();
    assert_eq!(published.len(), 1);
    for state in [VnState::Stepping, VnState::Waiting, VnState::Menu, VnState::Stepping, VnState::Waiting] {
        let visible = matches!(state, VnState::Stepping | VnState::Waiting);
        app.world_mut().resource_mut::<NextState<VnState>>().set(state.clone());
        for _ in 0..3 { app.update(); }
        assert_eq!(app.world().resource::<State<VnState>>().get(), &state);
        assert!(!app.world().contains_resource::<resources::VnEngine>());
        assert_eq!(app.world().resource::<DebugBootstrap>().owner.snapshot(), snapshot);
        assert_eq!(serde_json::to_vec(app.world().resource::<DebugBootstrap>().owner.state()).unwrap(), state_bytes);
        assert!(app.world().resource::<DebugBootstrap>().owner.accepts_observation_source(&accepted));
        assert!(!app.world().resource::<DebugBootstrap>().publication_pending);
        assert_eq!(app.world().resource::<scene_publication::ScenePresentation>().batch.as_ref().unwrap().interfaces, published);
        assert_eq!(app.world().resource::<programmable_ui::Screens>().views, views);
        assert_eq!(app.world().resource::<programmable_ui::Screens>().active, visible);
        let target = app.world_mut().query::<&composed_motion::InterfaceTarget>().iter(app.world())
            .any(|target| target.screen == "published" && target.element == "message");
        assert_eq!(target, visible, "ordinary screen visibility in {state:?}");
        if !visible {
            assert!(app.world_mut().query::<&composed_motion::InterfaceTarget>().iter(app.world())
                .all(|target| target.screen != "published"));
            assert!(app.world_mut().query::<&programmable_ui::Control>().iter(app.world())
                .all(|control| control.screen != "published"));
            assert!(programmable_ui::frame_input_source(app.world().resource::<programmable_ui::Screens>(), "published").is_none());
            assert!(programmable_ui::capture_preview_native(app.world()).is_none_or(|frame| !frame.current(app.world())));
        }
    }
}

#[test]
fn startup_real_settings_and_overlay_preserve_core_source_while_projecting_user_preference() {
    // Full registrar/plugins and the published PCM asset/reader, exactly as
    // the accepted Startup audio projection lane. No fabricated player/sink.
    let (_, mut app) = crate::presentation_order::startup_owner_tests::app(
        "init {music.volume(0.4) music.play(\"host.wav\") set hold=1}\nscene label start\n\"Ready\"\n");
    crate::presentation_order::startup_owner_tests::commit_one(&mut app);
    crate::presentation_order::startup_owner_tests::commit_one(&mut app);
    let entity = app.world().resource::<resources::MusicEntity>().0.unwrap();
    let handle = app.world().get::<Handle<crate::checkpoint_pcm::CheckpointPcm>>(entity)
        .expect("accepted actual PCM source").clone();
    let reader = app.world().resource::<Assets<crate::checkpoint_pcm::CheckpointPcm>>()
        .get(&handle).unwrap().clone();
    let target = app.world().resource::<DebugBootstrap>().owner.state().audio_resource_registry().unwrap()
        .target(&rvn_core::scene_tasks::TaskOwner::Game, rvn_core::scene_resources::TargetKind::Music, "music")
        .unwrap().clone();
    assert_eq!(crate::checkpoint_sources::music_entity_for_test(app.world(), &target), Some(entity));
    assert_eq!(app.world().resource::<DebugBootstrap>().owner.state().music.volume, 0.4);
    assert_eq!(app.world().get::<PlaybackSettings>(entity).unwrap().volume.get(), 0.4);
    let logical_resource = app.world().resource::<resources::MusicVolume>().0;
    let cursor = reader.gate().snapshot(&[&reader], |rows| Ok(rows)).unwrap();
    app.world_mut().resource_mut::<DebugOverlayState>().visible = true;
    app.update();
    let source = app.world().resource::<DebugBootstrap>().owner.observation_source().unwrap();
    let snapshot = app.world().resource::<DebugBootstrap>().owner.snapshot();
    let state = serde_json::to_vec(app.world().resource::<DebugBootstrap>().owner.state()).unwrap();
    { let mut settings = app.world_mut().resource_mut::<Settings>(); settings.typewriter = false; settings.music_volume = 0.25; }
    for _ in 0..4 { app.update(); }
    assert!(!app.world().resource::<resources::TypewriterConfig>().enabled);
    assert_eq!(app.world().resource::<resources::MusicVolume>().0, logical_resource);
    assert_eq!(app.world().resource::<DebugBootstrap>().owner.state().music.volume, 0.4);
    assert_eq!(app.world().resource::<resources::MusicEntity>().0, Some(entity));
    assert_eq!(app.world().get::<Handle<crate::checkpoint_pcm::CheckpointPcm>>(entity).unwrap().id(), handle.id());
    assert!(app.world().resource::<Assets<crate::checkpoint_pcm::CheckpointPcm>>()
        .get(&handle).unwrap().same_reader(&reader));
    assert_eq!(crate::checkpoint_sources::music_entity_for_test(app.world(), &target), Some(entity));
    assert_eq!(reader.gate().snapshot(&[&reader], |rows| Ok(rows)).unwrap(), cursor);
    assert_eq!(app.world().resource::<DebugBootstrap>().owner.snapshot(), snapshot);
    assert_eq!(serde_json::to_vec(app.world().resource::<DebugBootstrap>().owner.state()).unwrap(), state);
    assert!(app.world().resource::<DebugBootstrap>().owner.accepts_observation_source(&source));
    assert!(!app.world().contains_resource::<resources::VnEngine>());
    assert!(app.world_mut().query_filtered::<Entity, (With<crate::components::MusicMarker>, With<Handle<AudioSource>>)>()
        .iter(app.world()).next().is_none(), "no uncheckpointed legacy music replay");
    let overlay: Vec<_> = app.world_mut().query_filtered::<&Text, With<systems::debug_overlay::DebugOverlayText>>()
        .iter(app.world()).flat_map(|text| text.sections.iter().map(|section| section.value.clone())).collect();
    assert!(overlay.iter().any(|text| text.starts_with("Startup : ")));
    // Assert the real accepted player's effective mix after registered systems
    // ran. Calling final_gain directly would not prove projection reached it.
    assert_eq!(app.world().get::<PlaybackSettings>(entity).unwrap().volume.get(), 0.1,
        "the accepted PCM player must apply logical 0.4 times preference 0.25");
    // Negative host fault injection checks the same real projection system.
    // These are internal system guards, not additional public/native routes.
    app.world_mut().resource_mut::<Settings>().music_volume = 0.5;
    let retained = |app: &mut App| {
        crate::scene_effects::reconcile_for_test(app.world_mut());
        assert_eq!(app.world().get::<PlaybackSettings>(entity).unwrap().volume.get(), 0.1);
        assert_eq!(app.world().resource::<DebugBootstrap>().owner.snapshot(), snapshot);
        assert_eq!(serde_json::to_vec(app.world().resource::<DebugBootstrap>().owner.state()).unwrap(), state);
        assert!(app.world().resource::<DebugBootstrap>().owner.accepts_observation_source(&source));
        assert_eq!(app.world().resource::<resources::MusicEntity>().0, Some(entity));
        assert_eq!(app.world().get::<Handle<crate::checkpoint_pcm::CheckpointPcm>>(entity).unwrap().id(), handle.id());
        assert!(app.world().resource::<Assets<crate::checkpoint_pcm::CheckpointPcm>>()
            .get(&handle).unwrap().same_reader(&reader));
        assert_eq!(reader.gate().snapshot(&[&reader], |rows| Ok(rows)).unwrap(), cursor);
    };
    app.world_mut().resource_mut::<DebugBootstrap>().publication_pending = true;
    retained(&mut app);
    app.world_mut().resource_mut::<DebugBootstrap>().publication_pending = false;
    app.world_mut().resource_mut::<DebugBootstrap>().failed = Some("negative projection guard".into());
    retained(&mut app);
    app.world_mut().resource_mut::<DebugBootstrap>().failed = None;
    let broker = app.world().resource::<DebugBootstrap>().broker.clone();
    app.world_mut().resource_mut::<DebugBootstrap>().broker = crate::checkpoint_broker::AudioCheckpointBroker::default();
    assert!(!app.world().resource::<DebugBootstrap>().accepts_actual_audio_broker());
    retained(&mut app);
    app.world_mut().resource_mut::<DebugBootstrap>().broker = broker;
    let other = rvn_core::Engine::from_program(
        rvn_parser::parse_program("scene label other\n\"Other\"\n").unwrap(),
        crate::bevy_renderer::BevyRenderer::new(), 32).unwrap();
    app.insert_resource(resources::VnEngine(other));
    retained(&mut app);
    app.world_mut().remove_resource::<resources::VnEngine>().unwrap();
    app.world_mut().resource_mut::<Settings>().music_volume = 0.25;
    retained(&mut app);

}

#[test]
fn startup_language_change_is_explicit_and_cannot_mutate_the_frozen_owner() {
    let (_, mut app) = app(); app.update();
    let source = app.world().resource::<DebugBootstrap>().owner.observation_source().unwrap();
    let snapshot = app.world().resource::<DebugBootstrap>().owner.snapshot();
    app.world_mut().resource_mut::<Settings>().language = "fr".into();
    app.update();
    assert!(app.world().resource::<DebugBootstrap>().failed.as_deref().is_some_and(|error| error.contains("authenticated locale publication")));
    assert_eq!(app.world().resource::<DebugBootstrap>().owner.locale().unwrap().current_lang(), "en");
    assert_eq!(app.world().resource::<DebugBootstrap>().owner.snapshot(), snapshot);
    assert!(app.world().resource::<DebugBootstrap>().owner.accepts_observation_source(&source));
}

#[test]
fn startup_accepted_motion_with_reduced_motion_reads_true_owner_without_advancing_it() {
    let (_, mut app) = app_source(
        "screen published(){return component(\"root\",\"panel\",{},[component(\"message\",\"text\",{\"text\":\"Startup visible\"},[])])}\ninit {ui.open(\"published\",[],false,0) motion.play(\"ui:published/message\",motion_tween(1,{}, {\"x\":100},\"linear\")) set after=9}\nscene label start\n\"Done\"\n");
    app.update();
    for _ in 0..2 {
        let plan = debugger_startup::prepare_world_step(app.world(), rvn_core::editor_debugger::StepKind::Continue).unwrap();
        assert!(plan.ready());
        let committed = debugger_startup::commit_world_step(app.world_mut(), plan).unwrap();
        debugger_startup::install_committed_world_step(app.world_mut(), committed);
        app.update();
    }
    assert_eq!(app.world().resource::<DebugBootstrap>().owner.state().motions.tracks.len(), 1);
    assert!(!app.world().resource::<DebugBootstrap>().owner.state().vars.contains_key("after"));
    app.world_mut().resource_mut::<accessibility::Accessibility>().settings.reduced_motion = true;
    let source = app.world().resource::<DebugBootstrap>().owner.observation_source().unwrap();
    let snapshot = app.world().resource::<DebugBootstrap>().owner.snapshot();
    for _ in 0..4 { app.update(); }
    assert!(!app.world().contains_resource::<resources::VnEngine>());
    assert_eq!(app.world().resource::<DebugBootstrap>().owner.snapshot(), snapshot);
    assert!(app.world().resource::<DebugBootstrap>().owner.accepts_observation_source(&source));
    assert!(app.world_mut().query::<&composed_motion::InterfaceTarget>().iter(app.world())
        .any(|target| target.screen == "published" && target.element == "message"));
}
