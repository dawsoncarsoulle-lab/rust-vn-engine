use super::*;
use bevy::prelude::*;
use crate::portable_text_resources as text;
use std::{collections::{BTreeMap,HashMap},path::PathBuf,time::{Duration,Instant},sync::atomic::{AtomicU64,Ordering}};
const ID:&str="17411a3e431e4c9388b78a2b9825b130";
pub(crate) fn fixture()->(PathBuf,App){
    fixture_source("init {flow.consume_v1(\"scope\",\"consumer\") {capture \"draw\"=random(1,9) set boot=__rvn_captured_v1(\"draw\")}}\nscene label start\n\"Done\"\n")
}
pub(crate) fn fixture_source(source:&str)->(PathBuf,App){
    let(root,mut app)=fixture_source_unpumped(source);
    pump(&mut app,|world|text::live_receipt(world).is_ok());
    (root,app)
}
/// Shared real fixture construction, before its first Main run. New launch
/// coverage installs its real plugins/registrar before Main owns startup order.
pub(crate) fn fixture_source_unpumped(source:&str)->(PathBuf,App){
    static NEXT:AtomicU64=AtomicU64::new(0);
    let root=PathBuf::from(std::env::var("RVN_STARTUP_WORLD_FIXTURES").expect("private fixture directory")).join(format!("world-{}-{}",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)));
    std::fs::create_dir_all(root.join("assets")).unwrap();std::fs::create_dir(root.join("locales")).unwrap();
    let table=rvn_core::string_table::StringTableAsset{id:ID.into(),namespace:"VN".into(),entries:BTreeMap::from([("hello".into(),rvn_core::string_table::StringTableEntry{source:"Original".into(),developer_notes:String::new(),metadata:BTreeMap::new()})])};
    std::fs::write(root.join("assets/table.rvnstrings"),rvn_core::string_table_file::StringTableFile::new(table.clone()).unwrap().encode().unwrap()).unwrap();
    let declarations=text::LocaleDeclarations{default_lang:"fr".into(),current_lang:"en".into(),available:vec!["fr".into(),"en".into()],files:BTreeMap::from([("fr".into(),"fr.toml".into()),("en".into(),"en.toml".into())])};
    for lang in ["fr","en"]{
        let key=table.localization("hello").unwrap().identity().unwrap().locale_key();
        let mut catalog=rvn_core::locale::LocaleCatalog::from(rvn_core::locale::LocaleTable{lang:lang.into(),strings:HashMap::from([(key,format!("{lang}:Original"))])});
        catalog.record_string_table_source_witness(&table,"hello").unwrap();std::fs::write(root.join(format!("locales/{lang}.toml")),catalog.to_toml()).unwrap();
    }
    std::fs::write(root.join("main.rvn"),source).unwrap();
    // Original deterministic one-second mono PCM16LE silence, authored in
    // EACH real world before AssetServer sees it. No external seed/order or
    // fabricated loaded event is required by this shared real-I/O fixture.
    let frames=8000u32;let pcm=vec![0u8;(frames*2)as usize];
    let mut wave=Vec::with_capacity(44+pcm.len());
    wave.extend_from_slice(b"RIFF");wave.extend_from_slice(&(36+pcm.len()as u32).to_le_bytes());wave.extend_from_slice(b"WAVEfmt ");
    wave.extend_from_slice(&16u32.to_le_bytes());wave.extend_from_slice(&1u16.to_le_bytes());wave.extend_from_slice(&1u16.to_le_bytes());
    wave.extend_from_slice(&8000u32.to_le_bytes());wave.extend_from_slice(&16000u32.to_le_bytes());wave.extend_from_slice(&2u16.to_le_bytes());wave.extend_from_slice(&16u16.to_le_bytes());
    wave.extend_from_slice(b"data");wave.extend_from_slice(&(pcm.len()as u32).to_le_bytes());wave.extend_from_slice(&pcm);
    std::fs::create_dir(root.join("assets/sfx")).unwrap();
    std::fs::write(root.join("assets/sfx/delayed.wav"),wave).unwrap();
    let program=rvn_parser::parse_file_program_with_uses(root.join("main.rvn")).unwrap();
    let map=rvn_core::debugger_provenance::SourceMap::from_program(&program).unwrap();
    let overlay=map.clone().staged_overlay(&map,398).unwrap();std::fs::write(root.join(rvn_core::debugger_provenance::SIDECAR),overlay.encode().unwrap()).unwrap();
    let resources=rvn_core::string_table_project::load_native(&root.join("assets"),&BTreeMap::from([(ID.into(),"table.rvnstrings".into())])).unwrap();
    let locale=text::load_native_frozen_locales(&root.join("locales"),&declarations).unwrap();
    let startup=prepare(&root,program,398,HashMap::new(),rvn_core::TextRuntimeContext::new(resources,Some(locale)),Some("start".into())).unwrap();
    let mut app=App::new();text::register_text_sources(&mut app,root.join("assets").to_string_lossy().into_owned(),root.join("locales").to_string_lossy().into_owned()).unwrap();
    app.add_plugins(MinimalPlugins).add_plugins(AssetPlugin{file_path:root.join("assets").to_string_lossy().into_owned(),watch_for_changes_override:Some(false),..default()})
       .insert_resource(startup).init_resource::<crate::scene_publication::ScenePresentation>().add_event::<crate::vn_command::VnCommand>()
       .add_plugins(crate::string_table_assets::StringTableAssetsPlugin::new(BTreeMap::from([(ID.into(),"table.rvnstrings".into())])))
       .add_plugins(text::ClosedLocaleAssetsPlugin::new(declarations).unwrap());
    app.init_asset::<bevy::audio::AudioSource>().init_asset_loader::<bevy::audio::AudioLoader>().init_asset::<crate::checkpoint_pcm::CheckpointPcm>();
    (root,app)
}
fn pump(app:&mut App,condition:impl Fn(&World)->bool){let end=Instant::now()+Duration::from_secs(15);loop{app.update();if condition(app.world()){return;}assert!(Instant::now()<end,"Actual startup AssetServer timeout: {:?}",text::live_receipt(app.world()).err());std::thread::sleep(Duration::from_millis(5));}}
fn state(world:&World)->serde_json::Value{serde_json::to_value(world.resource::<DebugBootstrap>().owner.state()).unwrap()}
#[test]
fn actual_startup_world_preparation_drop_and_broker_commit_once(){
    let(_,mut app)=fixture();assert!(!app.world().contains_resource::<crate::resources::VnEngine>());
    let before=state(app.world());let snapshot=app.world().resource::<DebugBootstrap>().owner.snapshot();
    let a=prepare_world_step(app.world(),rvn_core::editor_debugger::StepKind::Continue).unwrap();assert!(a.core.render_batch().is_some());
    let b=prepare_world_step(app.world(),rvn_core::editor_debugger::StepKind::Continue).unwrap();
    assert_eq!(state(app.world()),before);assert_eq!(app.world().resource::<DebugBootstrap>().owner.snapshot(),snapshot);drop(b);
    let committed=commit_world_step(app.world_mut(),a).unwrap();let publication=committed.publication.unwrap();
    assert_eq!(publication.source.instance_id(),snapshot.instance);assert!(publication.batch.state.vars.contains_key("boot"));
    assert_eq!(serde_json::to_value(&publication.batch.state).unwrap(),state(app.world()));assert!(app.world().resource::<DebugBootstrap>().publication_pending);
    assert!(prepare_world_step(app.world(),rvn_core::editor_debugger::StepKind::Continue).is_err());
}
#[test]
fn actual_startup_locale_bytes_and_session_changes_reject_without_effects(){
    let(_,mut app)=fixture();let plan=prepare_world_step(app.world(),rvn_core::editor_debugger::StepKind::Continue).unwrap();let before=state(app.world());
    let handle=app.world().resource::<AssetServer>().get_handle::<text::LoadedLocaleSource>(bevy::asset::AssetPath::from("en.toml").with_source("rvn-closed-locales-v1")).unwrap();
    app.world_mut().resource_mut::<Assets<text::LoadedLocaleSource>>().get_mut(&handle).unwrap().0.push_str("\n# changed before event drain\n");
    assert!(commit_world_step(app.world_mut(),plan).is_err());assert_eq!(state(app.world()),before);assert!(!app.world().resource::<DebugBootstrap>().publication_pending);
    let(_,mut app)=fixture();let plan=prepare_world_step(app.world(),rvn_core::editor_debugger::StepKind::Continue).unwrap();
    app.world_mut().resource_mut::<DebugBootstrap>().owner.configure(vec![],vec![rvn_core::editor_debugger::Watch{scope:rvn_core::editor_debugger::WatchScope::Globals,name:"boot".into()}]).unwrap();let before=state(app.world());
    assert!(commit_world_step(app.world_mut(),plan).is_err());assert_eq!(state(app.world()),before);
}
#[test]
fn actual_startup_foreign_broker_cannot_publish_under_another_gate(){
    let(_,mut app)=fixture();let plan=prepare_world_step(app.world(),rvn_core::editor_debugger::StepKind::Continue).unwrap();let before=state(app.world());
    app.world_mut().resource_mut::<DebugBootstrap>().broker=crate::checkpoint_broker::AudioCheckpointBroker::default();
    assert!(commit_world_step(app.world_mut(),plan).err().unwrap().contains("broker changed"));assert_eq!(state(app.world()),before);assert!(!app.world().resource::<DebugBootstrap>().publication_pending);
    // Re-admission after replacement fails before evaluating another candidate
    // or visiting Source metadata. The transfer witness cannot be recaptured.
    assert!(!app.world().resource::<DebugBootstrap>().accepts_actual_audio_broker());
    let error=prepare_world_step(app.world(),rvn_core::editor_debugger::StepKind::Continue).err().unwrap();
    assert!(error.contains("actual renderer broker was replaced"));assert_eq!(state(app.world()),before);
}
#[test]
fn actual_startup_audio_transfer_witness_is_pure_and_rejects_owner_replacement(){
    let(_,mut app)=fixture();let before=state(app.world());
    let observation=app.world().resource::<DebugBootstrap>().owner.observation_source().unwrap();
    let broker=app.world().resource::<DebugBootstrap>().broker.clone();
    for _ in 0..8{assert!(app.world().resource::<DebugBootstrap>().accepts_actual_audio_broker());}
    app.world_mut().resource_mut::<DebugBootstrap>().broker=broker;
    assert!(app.world().resource::<DebugBootstrap>().accepts_actual_audio_broker());
    assert!(app.world().resource::<DebugBootstrap>().owner.accepts_observation_source(&observation));
    assert_eq!(state(app.world()),before);
    let(_,mut other)=fixture();let other=other.world_mut().remove_resource::<DebugBootstrap>().unwrap();
    app.world_mut().resource_mut::<DebugBootstrap>().owner=other.owner;
    assert!(!app.world().resource::<DebugBootstrap>().accepts_actual_audio_broker());
}
#[test]
fn actual_startup_loading_retains_completed_audio_computation_and_live_rng(){
    let(_,mut app)=fixture_source("init {sfx.play(\"delayed.wav\")}\nscene label start\n\"Done\"\n");
    let before=state(app.world());let snapshot=app.world().resource::<DebugBootstrap>().owner.snapshot();
    let mut plan=prepare_world_step(app.world(),rvn_core::editor_debugger::StepKind::Continue).unwrap();
    assert!(!plan.ready(),"The real Source has not been loaded before its request");
    let candidate=serde_json::to_value(&plan.core().render_batch().unwrap().state).unwrap();
    for _ in 0..3{assert!(!refresh_world_step(app.world(),&mut plan).unwrap());assert_eq!(serde_json::to_value(&plan.core().render_batch().unwrap().state).unwrap(),candidate);}
    assert_eq!(state(app.world()),before);assert_eq!(app.world().resource::<DebugBootstrap>().owner.snapshot(),snapshot);
    let end=Instant::now()+Duration::from_secs(15);
    while !refresh_world_step(app.world(),&mut plan).unwrap(){app.update();assert!(Instant::now()<end,"Actual audio Source timeout");std::thread::sleep(Duration::from_millis(5));}
    assert_eq!(serde_json::to_value(&plan.core().render_batch().unwrap().state).unwrap(),candidate);assert_eq!(state(app.world()),before);
    let committed=commit_world_step(app.world_mut(),plan).unwrap();assert!(committed.publication.is_some());assert_eq!(state(app.world())["random"],candidate["random"]);
}
#[test]
fn actual_startup_if_random_sfx_loading_keeps_one_condition_draw_and_real_source(){
    let(_,mut app)=fixture_source("init {if random(1,9)>0 {sfx.play(\"delayed.wav\")}}\nscene label start\n\"Done\"\n");
    let before=state(app.world());let snapshot=app.world().resource::<DebugBootstrap>().owner.snapshot();
    let initial=app.world().resource::<DebugBootstrap>().owner.state().random;
    let mut expected=initial;
    let script=rvn_parser::parse("set expected=random(1,9)").unwrap();
    rvn_core::eval::FunctionLibrary::from_script(&script).unwrap().execute_with_random(&script,&HashMap::new(),&mut expected).unwrap();
    let mut plan=prepare_world_step(app.world(),rvn_core::editor_debugger::StepKind::Continue).unwrap();
    let batch=plan.core().render_batch().expect("The real selected child produces the SFX batch");
    assert_eq!(batch.state.random,expected);assert_eq!(rvn_core::audio_playback::describe(&batch.state,batch.runtime_epoch).unwrap().len(),1);
    assert!(!plan.ready());
    let candidate=serde_json::to_value(&batch.state).unwrap();
    for _ in 0..4{
        assert!(app.world().resource::<DebugBootstrap>().owner.video_views().unwrap().is_empty());
        assert!(!refresh_world_step(app.world(),&mut plan).unwrap());
        assert_eq!(state(app.world()),before);assert_eq!(app.world().resource::<DebugBootstrap>().owner.snapshot(),snapshot);
        assert_eq!(serde_json::to_value(&plan.core().render_batch().unwrap().state).unwrap(),candidate);
    }
    let end=Instant::now()+Duration::from_secs(15);
    while !refresh_world_step(app.world(),&mut plan).unwrap(){app.update();assert!(Instant::now()<end);std::thread::sleep(Duration::from_millis(5));}
    let committed=commit_world_step(app.world_mut(),plan).unwrap();assert!(committed.publication.is_some());
    assert_eq!(app.world().resource::<DebugBootstrap>().owner.state().random,expected);
    assert_eq!(rvn_core::audio_playback::describe(app.world().resource::<DebugBootstrap>().owner.state(),snapshot.epoch).unwrap().len(),1);
    assert!(!app.world().contains_resource::<crate::resources::VnEngine>());
}
#[test]
fn actual_startup_owned_install_keeps_real_decoded_image_and_transfers_the_same_engine(){
    let(root,mut app)=fixture_source("init {scene \"card.png\" set after=2}\nscene label start\n\"Done\"\n");
    let fixture=PathBuf::from(std::env::var("RVN_STARTUP_WORLD_FIXTURES").unwrap()).parent().unwrap().join("shadow/examples/animations/assets/card-one.png");
    std::fs::copy(fixture,root.join("assets/card.png")).unwrap();
    app.init_asset::<Image>().init_asset_loader::<bevy::render::texture::ImageLoader>().init_resource::<crate::resources::VnRenderState>()
        .add_systems(Update,crate::systems::background_system);
    let instance=app.world().resource::<DebugBootstrap>().owner.snapshot().instance;
    let plan=prepare_world_step(app.world(),rvn_core::editor_debugger::StepKind::Continue).unwrap();
    let committed=commit_world_step(app.world_mut(),plan).unwrap();install_committed_world_step(app.world_mut(),committed);
    assert!(!app.world().resource::<DebugBootstrap>().publication_pending);
    assert_eq!(app.world().resource::<crate::scene_publication::ScenePresentation>().batch.as_ref().unwrap().state.background_image,"card.png");
    pump(&mut app,|world|world.get_resource::<AssetServer>().and_then(|server|server.get_handle::<Image>("card.png")).is_some_and(|handle|world.resource::<Assets<Image>>().contains(handle.id())));
    let image=app.world().resource::<AssetServer>().get_handle::<Image>("card.png").unwrap();
    let decoded=app.world().resource::<Assets<Image>>().get(&image).unwrap();assert!(decoded.width()>0&&decoded.height()>0&&!decoded.data.is_empty());
    let entity=app.world_mut().query_filtered::<Entity,With<crate::components::VnBackground>>().single(app.world());
    let plan=prepare_world_step(app.world(),rvn_core::editor_debugger::StepKind::Continue).unwrap();
    let committed=commit_world_step(app.world_mut(),plan).unwrap();install_committed_world_step(app.world_mut(),committed);app.update();
    let entity_after=app.world_mut().query_filtered::<Entity,With<crate::components::VnBackground>>().single(app.world());assert_eq!(entity_after,entity);
    assert_eq!(app.world().get::<Handle<Image>>(entity).unwrap().id(),image.id());
    assert_eq!(app.world().resource::<DebugBootstrap>().owner.state().vars["after"],rvn_parser::Value::Int(2));
    let startup=app.world_mut().remove_resource::<DebugBootstrap>().unwrap();assert!(startup.owner.is_complete());
    let engine=startup.owner.into_engine().unwrap();assert_eq!(engine.instance_id(),instance);app.world_mut().insert_resource(crate::resources::VnEngine(engine));
    assert!(!app.world().contains_resource::<DebugBootstrap>());assert_eq!(crate::text_runtime_owner::current(app.world()).unwrap().instance,instance);
}
#[test]
fn actual_startup_changed_presentation_host_and_missing_installer_refuse_before_effects(){
    let(_,mut app)=fixture();app.insert_resource(crate::systems::settings_menu::Settings::default());
    let plan=prepare_world_step(app.world(),rvn_core::editor_debugger::StepKind::Continue).unwrap();let before=state(app.world());let snapshot=app.world().resource::<DebugBootstrap>().owner.snapshot();
    app.world_mut().resource_mut::<crate::systems::settings_menu::Settings>().sfx_volume=0.3;
    assert!(commit_world_step(app.world_mut(),plan).err().unwrap().contains("presentation host changed"));assert_eq!(state(app.world()),before);assert_eq!(app.world().resource::<DebugBootstrap>().owner.snapshot(),snapshot);
    let plan=prepare_world_step(app.world(),rvn_core::editor_debugger::StepKind::Continue).unwrap();
    app.world_mut().remove_resource::<crate::scene_publication::ScenePresentation>();
    assert!(commit_world_step(app.world_mut(),plan).err().unwrap().contains("runtime vanished"));assert_eq!(state(app.world()),before);assert_eq!(app.world().resource::<DebugBootstrap>().owner.snapshot(),snapshot);
}

#[test]
fn startup_world_focus_lost_after_real_preparation_refuses_before_owner_effects(){
    let(_,mut app)=fixture();let before=state(app.world());let snapshot=app.world().resource::<DebugBootstrap>().owner.snapshot();let observed=app.world().resource::<DebugBootstrap>().owner.observation_source().unwrap();
    let prepared=prepare_world_step(app.world(),rvn_core::editor_debugger::StepKind::Continue).unwrap();assert!(prepared.ready());
    app.world_mut().spawn((bevy::prelude::Window{focused:false,..Default::default()},bevy::window::PrimaryWindow));
    assert!(commit_world_step(app.world_mut(),prepared).err().unwrap().contains("lost World focus"));assert_eq!(state(app.world()),before);assert_eq!(app.world().resource::<DebugBootstrap>().owner.snapshot(),snapshot);assert!(app.world().resource::<DebugBootstrap>().owner.accepts_observation_source(&observed));assert!(!app.world().resource::<DebugBootstrap>().publication_pending);
}
