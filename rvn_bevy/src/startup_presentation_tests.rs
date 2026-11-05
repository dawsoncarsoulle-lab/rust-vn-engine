//! PREPARED / NOT_RUN. Real CPU plugins, production startup schedules and
//! accepted Core/World publications. No DefaultPlugins/GPU/device assertion.
use super::*;
use crate::{debugger_startup::DebugBootstrap, resources::{CharacterRegistry, DialogueHistory, VnEngine, VnState}};
use std::{path::PathBuf, time::{Duration, Instant}};

#[derive(Resource,Default)]
struct ScheduleTrace(Vec<&'static str>);

pub(crate) fn app(source: &str) -> (PathBuf, App) {
    let (root, mut app) = crate::startup_full_launch_tests::app_source_with_complete_cpu_ui(source);
    app.add_plugins(crate::checkpoint_requests::CheckpointRequestsPlugin)
        .add_plugins(crate::scene_effects::SceneEffectsPlugin)
        .add_plugins(crate::scene_retired::SceneRetiredPlugin)
        .add_plugins(crate::scene_audio::SceneAudioPlugin)
        .add_plugins(crate::web_music::WebMusicPlugin)
        .add_plugins(PresentationOrderPlugin)
        .add_plugins(crate::accessibility::AccessibilityPlugin)
        .add_plugins(crate::portable_images::PortableImagesPlugin);
    // Observe the genuine Main startup order. The real factory has not pumped
    // this App; no schedule is replayed and no private Main Local is reset.
    app.init_resource::<ScheduleTrace>()
        .add_systems(bevy::state::state::StateTransition, |mut trace: ResMut<ScheduleTrace>| trace.0.push("StateTransition"))
        .add_systems(PreStartup, |mut trace: ResMut<ScheduleTrace>| trace.0.push("PreStartup"))
        .add_systems(Startup, |mut trace: ResMut<ScheduleTrace>| trace.0.push("Startup"))
        .add_systems(PostStartup, |mut trace: ResMut<ScheduleTrace>| trace.0.push("PostStartup"));
    let picture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../examples/animations/assets/card-one.png");
    std::fs::copy(&picture, root.join("assets/card.png")).unwrap();
    std::fs::copy(&picture, root.join("assets/card-two.png")).unwrap();
    std::fs::create_dir_all(root.join("assets/sprites/hero")).unwrap();
    std::fs::copy(picture, root.join("assets/sprites/hero/one.png")).unwrap();
    std::fs::create_dir_all(root.join("assets/music")).unwrap();
    let wave=PathBuf::from(std::env::var("RVN_STARTUP_WORLD_FIXTURES").unwrap()).join("pcm/host.wav");
    std::fs::copy(wave,root.join("assets/music/host.wav")).unwrap();
    // The real editor-preview launch sets Stepping before App.run as well.
    app.world_mut().resource_mut::<NextState<VnState>>().set(VnState::Stepping);
    app.finish();
    app.cleanup();
    let end=Instant::now()+Duration::from_secs(15);
    loop {
        app.update(); // Main runs StateTransition/PreStartup/Startup/PostStartup.
        if crate::portable_text_resources::live_receipt(app.world()).is_ok(){break;}
        assert!(Instant::now()<end,"Actual full CPU launch text assets timed out");
        std::thread::sleep(Duration::from_millis(2));
    }
    (root, app)
}

pub(crate) fn commit_one(app: &mut App) {
    let end = Instant::now() + Duration::from_secs(15);
    loop {
        let prepared = crate::debugger_startup::prepare_world_step(app.world(), rvn_core::editor_debugger::StepKind::Continue).unwrap();
        if prepared.ready() {
            let committed = crate::debugger_startup::commit_world_step(app.world_mut(), prepared).unwrap();
            crate::debugger_startup::install_committed_world_step(app.world_mut(), committed);
            app.update();
            await_decoded_background(app);
            return;
        }
        app.update();
        assert!(Instant::now() < end, "Actual Startup sources failed to become ready");
        std::thread::sleep(Duration::from_millis(2));
    }
}
// Observe only the real AssetServer/PortableImageLoader output. A key or
// entity alone does not prove that the file was decoded by the installed loader.
fn await_decoded_background(app: &mut App) {
    let path = crate::text_runtime_owner::current(app.world()).unwrap().state.background_image.clone();
    if path.is_empty() { return; }
    let path = crate::systems::ensure_extension(&path);
    let end = Instant::now() + Duration::from_secs(15);
    loop {
        let ready = {
            let world = app.world();
            let server = world.resource::<AssetServer>();
            server.get_handle::<Image>(&path).is_some_and(|handle| {
                assert!(!matches!(server.load_state(handle.id()), bevy::asset::LoadState::Failed(_)), "Actual PNG loader failed: {path}");
                world.resource::<Assets<Image>>().get(&handle)
                    .is_some_and(|image| image.width() > 0 && image.height() > 0 && !image.data.is_empty())
            })
        };
        if ready { return; }
        assert!(Instant::now() < end, "Actual PNG AssetServer decode timeout: {path}");
        app.update();
        std::thread::sleep(Duration::from_millis(2));
    }
}
fn complete(app: &mut App) {
    for _ in 0..32 {
        if app.world().resource::<DebugBootstrap>().owner.is_complete() { return; }
        commit_one(app);
    }
    panic!("Actual finite Init exceeded the fixture bound");
}
fn background(app: &mut App) -> Entity {
    app.world_mut().query_filtered::<Entity, With<crate::components::CurrentMotionBackground>>().single(app.world())
}
fn expected_background(world: &World) -> PresentationDrawOrder {
    let borrowed = crate::text_runtime_owner::current(world).unwrap();
    let target = borrowed.state.scene_execution.as_ref().unwrap().resources
        .target(&TaskOwner::Game, TargetKind::Background, "background").unwrap();
    PresentationDrawOrder::current(target, 0).unwrap()
}
fn nameplate(app: &mut App) -> String {
    app.world_mut().query_filtered::<&Text, With<crate::components::CharacterNameText>>()
        .single(app.world()).sections[0].value.clone()
}

#[test]
fn startup_presentation_actual_startup_poststartup_and_onenter_create_real_ui() {
    let (_, mut app) = app("init {set boot=1}\nscene label start\n\"Ready\"\n");
    assert_eq!(app.world_mut().query_filtered::<Entity, With<Camera2d>>().iter(app.world()).count(), 1);
    assert_eq!(app.world_mut().query_filtered::<Entity, With<crate::components::DialogueBox>>().iter(app.world()).count(), 1);
    assert_eq!(app.world_mut().query_filtered::<Entity, With<crate::components::CharacterNameText>>().iter(app.world()).count(), 1);
    assert_eq!(&app.world().resource::<ScheduleTrace>().0[..4], &["StateTransition","PreStartup","Startup","PostStartup"]);
    assert_eq!(*app.world().resource::<State<VnState>>().get(),VnState::Stepping);
    let before = app.world().resource::<DebugBootstrap>().owner.snapshot();
    for state in [VnState::TitleScreen, VnState::Menu, VnState::History, VnState::Error, VnState::Stepping] {
        app.world_mut().resource_mut::<NextState<VnState>>().set(state.clone());
        app.update();
        assert_eq!(*app.world().resource::<State<VnState>>().get(), state);
        if state == VnState::TitleScreen {
            assert_eq!(app.world_mut().query_filtered::<Entity, With<crate::systems::title::TitleOverlay>>().iter(app.world()).count(), 1);
        } else if state == VnState::Menu {
            assert_eq!(app.world_mut().query_filtered::<Entity, With<crate::systems::menu::MenuOverlay>>().iter(app.world()).count(), 1);
        } else if state == VnState::History {
            assert_eq!(app.world_mut().query_filtered::<Entity, With<crate::systems::history::HistoryOverlay>>().iter(app.world()).count(), 1);
        } else if state == VnState::Error {
            assert_eq!(app.world_mut().query_filtered::<Entity, With<crate::systems::error_overlay::ErrorOverlay>>().iter(app.world()).count(), 1);
        }
        assert!(!app.world().contains_resource::<VnEngine>());
        assert_eq!(app.world().resource::<DebugBootstrap>().owner.snapshot(), before);
    }
    assert!(app.world_mut().query_filtered::<Entity, With<crate::systems::menu::MenuOverlay>>().iter(app.world()).next().is_none());
    assert!(app.world_mut().query_filtered::<Entity, With<crate::systems::history::HistoryOverlay>>().iter(app.world()).next().is_none());
    assert!(app.world_mut().query_filtered::<Entity, With<crate::systems::error_overlay::ErrorOverlay>>().iter(app.world()).next().is_none());
}

#[test]
fn startup_presentation_actual_owner_registry_orders_accepted_background_without_advancing_core() {
    let (_, mut app) = app("init {scene \"card.png\" set hold=1}\nscene label start\n\"Ready\"\n");
    commit_one(&mut app);
    let entity = background(&mut app);
    let expected = expected_background(app.world());
    assert_eq!(app.world().get::<PresentationDrawOrder>(entity), Some(&expected));
    let state = serde_json::to_vec(app.world().resource::<DebugBootstrap>().owner.state()).unwrap();
    let source = app.world().resource::<DebugBootstrap>().owner.observation_source().unwrap();
    for _ in 0..3 { app.update(); }
    assert_eq!(app.world().get::<PresentationDrawOrder>(entity), Some(&expected));
    assert_eq!(serde_json::to_vec(app.world().resource::<DebugBootstrap>().owner.state()).unwrap(), state);
    assert!(app.world().resource::<DebugBootstrap>().owner.accepts_observation_source(&source));
    assert!(!app.world().contains_resource::<VnEngine>());
}

#[test]
fn startup_presentation_handoff_keeps_true_instance_and_projects_init_name_into_dialogue_history() {
    let (_, mut app) = app("init {scene \"card.png\" character.create(\"hero\",\"Héroïne😀\")}\nscene label start\nhero \"Bonjour\"\n");
    complete(&mut app);
    let instance = app.world().resource::<DebugBootstrap>().owner.snapshot().instance;
    assert_eq!(app.world().resource::<CharacterRegistry>().display_name("hero"), "Héroïne😀");
    assert_eq!(nameplate(&mut app), ""); // Init has no narrative display authority.
    let startup = app.world_mut().remove_resource::<DebugBootstrap>().unwrap();
    let engine = startup.owner.into_engine().unwrap();
    assert_eq!(engine.instance_id(), instance);
    app.insert_resource(VnEngine(engine));
    app.world_mut().resource_mut::<NextState<VnState>>().set(VnState::Stepping);
    for _ in 0..4 { app.update(); }
    assert!(!app.world().contains_resource::<DebugBootstrap>());
    assert_eq!(crate::text_runtime_owner::current(app.world()).unwrap().instance, instance);
    assert_eq!(nameplate(&mut app), "Héroïne😀");
    let history = &app.world().resource::<DialogueHistory>().lines;
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].character, "Héroïne😀");
    assert_eq!(history[0].text, "Bonjour");
    let entity = background(&mut app);
    assert_eq!(app.world().get::<PresentationDrawOrder>(entity), Some(&expected_background(app.world())));
}

#[test]
fn startup_presentation_coexisting_real_owners_cannot_retag_the_accepted_frame() {
    let (_, mut app) = app("init {scene \"card.png\" set hold=1}\nscene label start\n\"Ready\"\n");
    commit_one(&mut app);
    let entity = background(&mut app);
    let accepted = app.world().get::<PresentationDrawOrder>(entity).unwrap().clone();
    let original = app.world().resource::<DebugBootstrap>().owner.snapshot();
    let other = rvn_core::Engine::from_program(rvn_parser::parse_program("scene label other\n\"Foreign\"\n").unwrap(), crate::bevy_renderer::BevyRenderer::new(), 32).unwrap();
    assert_ne!(other.instance_id(), original.instance);
    app.insert_resource(VnEngine(other));
    assert!(crate::text_runtime_owner::current(app.world()).is_err());
    tag_current(app.world_mut());
    assert_eq!(app.world().get::<PresentationDrawOrder>(entity), Some(&accepted));
    assert_eq!(app.world().resource::<DebugBootstrap>().owner.snapshot(), original);
    app.world_mut().remove_resource::<VnEngine>().unwrap();
}

#[test]
fn startup_presentation_pending_and_failure_fences_keep_keys_until_real_install() {
    let (_, mut app) = app("init {scene \"card.png\" scene \"card-two.png\" set hold=1}\nscene label start\n\"Ready\"\n");
    commit_one(&mut app);
    let entity = background(&mut app);
    let accepted = app.world().get::<PresentationDrawOrder>(entity).unwrap().clone();
    let prepared = crate::debugger_startup::prepare_world_step(app.world(), rvn_core::editor_debugger::StepKind::Continue).unwrap();
    assert!(prepared.ready());
    let committed = crate::debugger_startup::commit_world_step(app.world_mut(), prepared).unwrap();
    assert!(app.world().resource::<DebugBootstrap>().publication_pending);
    tag_current(app.world_mut());
    assert_eq!(app.world().get::<PresentationDrawOrder>(entity), Some(&accepted));
    // Explicit negative host fault injection, never a fabricated positive receipt.
    app.world_mut().resource_mut::<DebugBootstrap>().failed = Some("fixture host failure".into());
    tag_current(app.world_mut());
    assert_eq!(app.world().get::<PresentationDrawOrder>(entity), Some(&accepted));
    app.world_mut().resource_mut::<DebugBootstrap>().failed = None;
    crate::debugger_startup::install_committed_world_step(app.world_mut(), committed);
    app.update();
    await_decoded_background(&mut app);
    let replacement = background(&mut app);
    assert_eq!(app.world().get::<PresentationDrawOrder>(replacement), Some(&expected_background(app.world())));
}

#[test]
fn startup_presentation_candidate_character_name_does_not_escape_the_shared_installer() {
    let (_, mut app) = app("init {character.create(\"hero\",\"First\") character.create(\"hero\",\"Second😀\")}\nscene label start\nhero \"Ready\"\n");
    commit_one(&mut app);
    assert_eq!(app.world().resource::<CharacterRegistry>().display_name("hero"), "First");
    let prepared = crate::debugger_startup::prepare_world_step(app.world(), rvn_core::editor_debugger::StepKind::Continue).unwrap();
    assert!(prepared.ready());
    let committed = crate::debugger_startup::commit_world_step(app.world_mut(), prepared).unwrap();
    assert_eq!(app.world().resource::<DebugBootstrap>().owner.state().layered.display_name("hero"), "Second😀");
    assert!(app.world().resource::<DebugBootstrap>().publication_pending);
    use bevy::ecs::system::RunSystemOnce;
    app.world_mut().run_system_once(crate::systems::setup::synchronize_character_registry);
    assert_eq!(app.world().resource::<CharacterRegistry>().display_name("hero"), "First");
    assert_eq!(nameplate(&mut app), "");
    assert!(app.world().resource::<DialogueHistory>().lines.is_empty());
    crate::debugger_startup::install_committed_world_step(app.world_mut(), committed);
    assert_eq!(app.world().resource::<CharacterRegistry>().display_name("hero"), "Second😀");
    app.update();
    assert_eq!(app.world().resource::<CharacterRegistry>().display_name("hero"), "Second😀");
    assert!(!app.world().contains_resource::<VnEngine>());
}
