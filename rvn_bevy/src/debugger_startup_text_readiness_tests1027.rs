//! Actual original DebugBootstrap + AssetServer/file queues, without UI ACKs.
use super::*;
use crate::portable_text_resources as text;
use std::time::{Duration,Instant};
fn fixture()->(std::path::PathBuf,App){
    super::super::tests::fixture_source_unpumped("init {if random(1,9)>0 {sfx.play(\"delayed.wav\")}}\nscene label start\n\"Done\"\n")
}
fn state(world:&World)->serde_json::Value{serde_json::to_value(world.resource::<DebugBootstrap>().owner.state()).unwrap()}
#[test]
fn debug_startup_text1027_queued_command_waits_for_real_file_publication_and_retains_one_audio_candidate(){
    let(root,mut app)=fixture();assert!(root.join("locales/fr.toml").is_file());assert!(root.join("locales/en.toml").is_file());
    assert_eq!(app.world().resource::<DebugBootstrap>().profile,super::super::BootstrapProfile::Debug);
    let watch=rvn_core::editor_debugger::Watch{scope:rvn_core::editor_debugger::WatchScope::Globals,name:"boot".into()};
    app.world_mut().resource_mut::<DebugBootstrap>().owner.configure(vec![],vec![watch]).unwrap();
    run(app.world_mut(),StepKind::Continue).unwrap();
    let before=state(app.world());let snapshot=app.world().resource::<DebugBootstrap>().owner.snapshot();let observation=app.world().resource::<DebugBootstrap>().owner.observation_source().unwrap();
    assert!(!text::startup_loads_ready(app.world()).unwrap());
    for _ in 0..4{
        drain(app.world_mut());let startup=app.world().resource::<DebugBootstrap>();
        assert!(startup.failed.is_none(),"real text loading cannot become terminal debugger failure");assert_eq!(startup.mode,Some(StepKind::Continue));
        assert_eq!(startup.owner.snapshot(),snapshot);assert!(startup.owner.accepts_observation_source(&observation));assert_eq!(state(app.world()),before);assert!(!pending(app.world()));
    }
    // Only real App updates/AssetServer decoding/locale publication establish
    // readiness. No loaded flags, fake events, pending clears or ACKs are set.
    let end=Instant::now()+Duration::from_secs(15);
    loop{app.update();match text::startup_loads_ready(app.world()){Ok(true)=>break,Ok(false)=>{},Err(error)=>panic!("actual text host failed: {error}")};assert!(Instant::now()<end,"actual locale/table files did not publish");std::thread::sleep(Duration::from_millis(5));}
    assert_eq!(state(app.world()),before);assert_eq!(app.world().resource::<DebugBootstrap>().mode,Some(StepKind::Continue));
    drain(app.world_mut());
    let driver=app.world().resource::<Driver>();let prepared=driver.prepared.as_ref().expect("real SFX decoding retains its one completed computation");
    assert!(!prepared.ready());let address=(&**prepared as *const PreparedBootstrapStep) as usize;
    let candidate=serde_json::to_value(&prepared.core().render_batch().unwrap().state).unwrap();
    assert_ne!(candidate["random"],before["random"],"the genuine selected conditional draws once in the retained candidate");
    for _ in 0..3{
        // The physical SFX load has not had another App update. Repeated driver
        // service must refresh this exact candidate, never run the probe again.
        drain(app.world_mut());let driver=app.world().resource::<Driver>();let prepared=driver.prepared.as_ref().unwrap();
        assert_eq!((&**prepared as *const PreparedBootstrapStep) as usize,address);assert_eq!(serde_json::to_value(&prepared.core().render_batch().unwrap().state).unwrap(),candidate);
        assert_eq!(state(app.world()),before);assert!(app.world().resource::<DebugBootstrap>().failed.is_none());assert_eq!(app.world().resource::<DebugBootstrap>().mode,Some(StepKind::Continue));
    }
}
#[test]
fn debug_startup_text1027_real_locale_decode_failure_stays_terminal_without_preparing_or_executing(){
    let(root,mut app)=fixture();let before=state(app.world());run(app.world_mut(),StepKind::Continue).unwrap();
    // Core froze the valid original bytes during fixture admission; the actual
    // subsequent AssetServer read now reports a genuine malformed locale.
    let malformed="[strings]\n\"bad\" = [\n";assert!(rvn_core::locale::LocaleCatalog::parse("en",malformed).is_err(),"the actual recognized locale grammar must reject this fixture");
    std::fs::write(root.join("locales/en.toml"),malformed).unwrap();
    let end=Instant::now()+Duration::from_secs(15);
    loop{app.update();let server=app.world().resource::<AssetServer>();let handle=server.get_handle::<text::LoadedLocaleSource>(bevy::asset::AssetPath::from("en.toml").with_source("rvn-closed-locales-v1"));if handle.is_some_and(|h|matches!(server.load_state(h.id()),bevy::asset::LoadState::Failed(_))){break}assert!(Instant::now()<end,"actual locale decoder did not report its failure");std::thread::sleep(Duration::from_millis(5));}
    assert!(text::startup_loads_ready(app.world()).is_err());
    drain(app.world_mut());let startup=app.world().resource::<DebugBootstrap>();assert!(startup.failed.is_some());assert_eq!(startup.mode,None);assert_eq!(state(app.world()),before);assert!(!pending(app.world()));assert!(!app.world().contains_resource::<crate::resources::VnEngine>());
}
