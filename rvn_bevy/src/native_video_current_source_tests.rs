//! Actual World/EngineStartup current-source admission; no device qualification.
use super::*;
use crate::{debugger_startup::DebugBootstrap,portable_text_resources as text};
use std::{collections::HashMap,path::PathBuf,time::{Duration,Instant},sync::atomic::{AtomicU64,Ordering}};
const CLIP:&[u8]=include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"),"/../../fixtures/clip.webm"));
const ID:&str="17411a3e431e4c9388b78a2b9825b130";
const SCRIPT:&str="init {video.play(\"intro\",video_clip(\"clip.webm\",{\"mask\":\"mask.webm\"})) set after=1}\nscene label start\n\"Ready\"\n";
fn fixture_source(source:&str)->(PathBuf,App){
    static NEXT:AtomicU64=AtomicU64::new(0);
    let root=PathBuf::from(std::env::var("RVN_NATIVE_STARTUP_FIXTURES").expect("private fixture directory")).join(format!("native-startup-0431-{}-{}",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)));
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
    let wave=PathBuf::from(std::env::var("RVN_NATIVE_STARTUP_FIXTURES").unwrap()).join("pcm/host.wav");
    std::fs::copy(wave,root.join("assets/delayed.wav")).unwrap();
    std::fs::write(root.join("assets/clip.webm"),CLIP).unwrap();std::fs::write(root.join("assets/mask.webm"),CLIP).unwrap();
    let program=rvn_parser::parse_file_program_with_uses(root.join("main.rvn")).unwrap();
    let map=rvn_core::debugger_provenance::SourceMap::from_program(&program).unwrap();
    let overlay=map.clone().staged_overlay(&map,398).unwrap();std::fs::write(root.join(rvn_core::debugger_provenance::SIDECAR),overlay.encode().unwrap()).unwrap();
    let resources=rvn_core::string_table_project::load_native(&root.join("assets"),&BTreeMap::from([(ID.into(),"table.rvnstrings".into())])).unwrap();
    let locale=text::load_native_frozen_locales(&root.join("locales"),&declarations).unwrap();
    let startup=crate::debugger_startup::prepare(&root,program,398,HashMap::new(),rvn_core::TextRuntimeContext::new(resources,Some(locale)),Some("start".into())).unwrap();
    let mut app=App::new();text::register_text_sources(&mut app,root.join("assets").to_string_lossy().into_owned(),root.join("locales").to_string_lossy().into_owned()).unwrap();
    app.add_plugins(MinimalPlugins).add_plugins(AssetPlugin{file_path:root.join("assets").to_string_lossy().into_owned(),watch_for_changes_override:Some(false),..default()})
       .insert_resource(startup).add_event::<crate::vn_command::VnCommand>()
       .add_plugins(crate::string_table_assets::StringTableAssetsPlugin::new(BTreeMap::from([(ID.into(),"table.rvnstrings".into())])))
       .add_plugins(text::ClosedLocaleAssetsPlugin::new(declarations).unwrap());
    app.init_asset::<bevy::audio::AudioSource>().init_asset_loader::<bevy::audio::AudioLoader>().init_asset::<crate::checkpoint_pcm::CheckpointPcm>();
    super::init_restore_runtime_for_test(&mut app);app.insert_resource(crate::project_paths::ProjectPaths::new(root.clone(),root.join("assets"),root.join("locales"),root.join("theme.toml"),root.join("saves")));
    pump(&mut app,|world|text::live_receipt(world).is_ok());
    (root,app)
}
fn pump(app:&mut App,condition:impl Fn(&World)->bool){let end=Instant::now()+Duration::from_secs(15);loop{app.update();if condition(app.world()){return;}assert!(Instant::now()<end,"Actual startup AssetServer timeout: {:?}",text::live_receipt(app.world()).err());std::thread::sleep(Duration::from_millis(5));}}
fn state(world:&World)->serde_json::Value{serde_json::to_value(world.resource::<DebugBootstrap>().owner.state()).unwrap()}

fn core(app:&App)->rvn_core::engine::PreparedStartupStep{let plan=app.world().resource::<DebugBootstrap>().owner.prepare_step(rvn_core::editor_debugger::StepKind::Continue).unwrap();assert_eq!(plan.render_batch().unwrap().videos.len(),1,"Every Native contrast must reach its actual VideoPlay");plan}
fn native_ready(app:&mut App,core:&rvn_core::engine::PreparedStartupStep)->PreparedNativeRestore{
 let before=state(app.world());let snapshot=app.world().resource::<DebugBootstrap>().owner.snapshot();let entities=app.world().entities().len();let end=Instant::now()+Duration::from_secs(20);
 loop{match prepare_startup(app.world(),core){Ok(Some(plan))=>{assert_eq!(state(app.world()),before);assert_eq!(app.world().resource::<DebugBootstrap>().owner.snapshot(),snapshot);assert_eq!(app.world().entities().len(),entities);return plan;},Ok(None)=>{},Err(e)=>panic!("Real startup native staging refused: {e}")}assert!(Instant::now()<end,"Real startup native readiness timed out");app.update();std::thread::sleep(Duration::from_millis(2));}
}
fn sources_ready(app:&mut App,core:&rvn_core::engine::PreparedStartupStep)->crate::checkpoint_sources::PreparedSources{
 let end=Instant::now()+Duration::from_secs(15);loop{if let Some(sources)=crate::checkpoint_sources::prepare_startup(app.world(),core).unwrap(){return sources;}assert!(Instant::now()<end);app.update();std::thread::sleep(Duration::from_millis(2));}
}
fn publish(app:&mut App,core:rvn_core::engine::PreparedStartupStep,native:PreparedNativeRestore,sources:crate::checkpoint_sources::PreparedSources)->Result<crate::bevy_renderer::StartupPublication,String>{
 verify_startup(app.world(),&core,&native)?;crate::checkpoint_sources::verify_restore_assets(app.world(),&sources)?;
 let mut startup=app.world_mut().remove_resource::<DebugBootstrap>().unwrap();
 let result=(||{let guard=startup.owner.begin_prepared_host_step_commit(core).map_err(|e|e.to_string())?;native.verify_startup_guard(&guard,&startup.broker)?;
   native.with_retirement(|retire|crate::checkpoint_sources::publish_startup_with_retirement(&sources,guard,retire))})();
 match result{Ok(committed)=>{let publication=committed.publication.ok_or("Expected true startup scene publication")?;startup.publication_pending=true;app.world_mut().insert_resource(startup);crate::checkpoint_sources::install(app.world_mut(),sources);install_restore(app.world_mut(),native);Ok(publication)},Err(e)=>{app.world_mut().insert_resource(startup);Err(e)}}
}
fn acknowledge_native_only(app:&mut App,publication:crate::bevy_renderer::StartupPublication){
 assert!(publication.commands.is_empty());let batch=&publication.batch;assert!(batch.display.is_none()&&batch.interfaces.is_empty()&&batch.motions.is_empty()&&batch.layered.is_empty()&&batch.effects.is_empty()&&batch.menu_requests.is_empty()&&batch.speech_requests.is_empty());assert!(rvn_core::audio_playback::describe(&batch.state,batch.runtime_epoch).unwrap().is_empty());assert!(rvn_core::audio_checkpoint::music_views(&batch.state,batch.runtime_epoch).unwrap().is_empty());let mut startup=app.world_mut().resource_mut::<DebugBootstrap>();assert_eq!(publication.source.instance_id(),startup.owner.snapshot().instance);assert_eq!(startup.owner.video_views().unwrap(),batch.videos);startup.publication_pending=false;
}
#[test]
fn current_startup_source_needs_actual_installed_owner_view_not_future_or_client_and_no_f64_cursor_claim(){
 let(_root,mut app)=fixture_source(SCRIPT);let first=core(&app);let future=first.render_batch().unwrap().videos[0].clone();let snapshot=app.world().resource::<DebugBootstrap>().owner.snapshot();assert!(crate::video_audio::source_for_startup_current(app.world(),&future,0.0).is_err());assert_eq!(app.world().resource::<DebugBootstrap>().owner.snapshot(),snapshot);
 let native=native_ready(&mut app,&first);let sources=sources_ready(&mut app,&first);let publication=publish(&mut app,first,native,sources).unwrap();assert!(crate::video_audio::source_for_startup_current(app.world(),&future,0.0).is_err(),"Core publication alone cannot stand in for its still pending actual World install");acknowledge_native_only(&mut app,publication);
 let startup=app.world().resource::<DebugBootstrap>();assert!(startup.accepts_actual_audio_broker());let view=startup.owner.video_views().unwrap()[0].clone();let before=startup.owner.snapshot();let entities=app.world().entities().len();let current=crate::video_audio::source_for_startup_current(app.world(),&view,view.track.position).unwrap();let b=current.0.lock().unwrap();assert!(b.verifies_owner(before.instance,&startup.broker.gate()));assert!(b.verifies_view(before.instance,&view));assert_eq!(b.delivered_sample_offset(),None,"Position-only construction is never an exact sample cursor");drop(b);
 let mut changed=view.clone();changed.id.push_str("-client");assert!(crate::video_audio::source_for_startup_current(app.world(),&changed,0.0).is_err());
 let mut changed=view.clone();changed.epoch+=1;assert!(crate::video_audio::source_for_startup_current(app.world(),&changed,0.0).is_err());let mut changed=view.clone();changed.track.clip.source="other.webm".into();assert!(crate::video_audio::source_for_startup_current(app.world(),&changed,0.0).is_err());assert!(crate::video_audio::source_for_startup_current(app.world(),&view,0.25).is_err());assert_eq!(app.world().resource::<DebugBootstrap>().owner.snapshot(),before);assert_eq!(app.world().entities().len(),entities);
}
#[test]
fn current_startup_source_refuses_real_foreign_broker_or_owner_before_metadata_source_install(){
 for replace_owner in [false,true]{let(_root,mut app)=fixture_source(SCRIPT);let first=core(&app);let native=native_ready(&mut app,&first);let sources=sources_ready(&mut app,&first);let publication=publish(&mut app,first,native,sources).unwrap();acknowledge_native_only(&mut app,publication);let view=app.world().resource::<DebugBootstrap>().owner.video_views().unwrap()[0].clone();let old=super::restored_sources_for_test(app.world())[0].1.as_ref().unwrap().clone();
 if replace_owner{let(_root,mut other)=fixture_source(SCRIPT);let new=other.world_mut().remove_resource::<DebugBootstrap>().unwrap();app.world_mut().resource_mut::<DebugBootstrap>().owner=new.owner;}else{app.world_mut().resource_mut::<DebugBootstrap>().broker=crate::checkpoint_broker::AudioCheckpointBroker::default();}
 let before=app.world().resource::<DebugBootstrap>().owner.snapshot();let entities=app.world().entities().len();assert!(!app.world().resource::<DebugBootstrap>().accepts_actual_audio_broker());assert!(crate::video_audio::source_for_startup_current(app.world(),&view,0.0).is_err());assert_eq!(app.world().resource::<DebugBootstrap>().owner.snapshot(),before);assert_eq!(app.world().entities().len(),entities);assert!(!old.0.lock().unwrap().revoked);
 }
}

// Full owned World route: deliberately not the Native-only acknowledgement
// helper above. No gameplay Engine or component entity stands in for Init.
#[test]
fn actual_startup_world_ui_target_requires_owned_native_install_before_texture_child(){
 let script=r#"screen native_media(){return component("root","panel",{},[component("player","panel",{},[])])}
init {ui.open("native_media",[],false,0) video.play("intro",video_clip("clip.webm",{"target":"ui:native_media/player"})) set after=1}
scene label start
"Ready"
"#;
 let(_root,mut app)=fixture_source(script);
 app.init_resource::<crate::scene_publication::ScenePresentation>()
    .insert_resource(State::new(VnState::Waiting)).init_resource::<NextState<VnState>>()
    .init_resource::<crate::resources::ScriptErrorMessage>()
    .init_resource::<crate::accessibility::Accessibility>()
    .init_resource::<ButtonInput<MouseButton>>().init_resource::<ButtonInput<KeyCode>>()
    .init_resource::<bevy::input::touch::Touches>().init_resource::<bevy::ui::UiScale>()
    .init_resource::<bevy::ui::UiStack>()
    .init_resource::<crate::editor_preview::PreviewState>()
    .insert_resource(crate::menu_documents::MenuFont(Handle::default()))
    .add_event::<bevy::input::mouse::MouseWheel>()
    .add_event::<bevy::input::keyboard::KeyboardInput>()
    .add_event::<bevy::window::Ime>()
    .add_event::<bevy::a11y::ActionRequest>()
    .add_plugins(crate::programmable_ui::ProgrammableUiPlugin);
 // A real editor-paused presentation may redraw its already accepted screen.
 // This also keeps Game-only input systems inactive while the true Startup
 // owner exists; it supplies no video clock or producer permission.
 app.world_mut().resource_mut::<crate::editor_preview::PreviewState>().paused=true;
 app.world_mut().spawn((Window::default(),bevy::window::PrimaryWindow));
 assert!(!app.world().contains_resource::<VnEngine>());
 let open=crate::debugger_startup::prepare_world_step(app.world(),rvn_core::editor_debugger::StepKind::Continue).unwrap();
 assert!(open.ready());assert_eq!(open.core().render_batch().unwrap().interfaces.len(),1);
 assert!(open.core().render_batch().unwrap().videos.is_empty());
 let committed=crate::debugger_startup::commit_world_step(app.world_mut(),open).unwrap();
 crate::debugger_startup::install_committed_world_step(app.world_mut(),committed);
 assert!(!app.world().resource::<DebugBootstrap>().publication_pending);
 app.update();
 let parent=app.world_mut().query::<(Entity,&crate::composed_motion::InterfaceTarget)>().iter(app.world())
     .find(|(_,target)|target.screen=="native_media"&&target.element=="player").map(|(entity,_)|entity)
     .expect("The actual accepted Open batch must render its authored UI component");
 let before=state(app.world());let before_cursor=app.world().resource::<DebugBootstrap>().owner.snapshot();
 let observed=app.world().resource::<DebugBootstrap>().owner.observation_source().unwrap();
 let mut play=crate::debugger_startup::prepare_world_step(app.world(),rvn_core::editor_debugger::StepKind::Continue).unwrap();
 let end=Instant::now()+Duration::from_secs(20);
 while !play.ready(){assert_eq!(state(app.world()),before);assert_eq!(app.world().resource::<DebugBootstrap>().owner.snapshot(),before_cursor);assert!(app.world().resource::<DebugBootstrap>().owner.accepts_observation_source(&observed));assert!(!app.world().resource::<Players>().players.contains_key("intro"));app.update();crate::debugger_startup::refresh_world_step(app.world(),&mut play).unwrap();assert!(Instant::now()<end,"Actual World Native staging timeout");std::thread::sleep(Duration::from_millis(2));}
 assert!(play.ready());let view=play.core().render_batch().unwrap().videos[0].clone();
 assert_eq!(view.track.clip.target.as_deref(),Some("ui:native_media/player"));
 assert!(view.ui_projection.is_some(),"The candidate must bind the real screen incarnation");
 let committed=crate::debugger_startup::commit_world_step(app.world_mut(),play).unwrap();
 crate::debugger_startup::install_committed_world_step(app.world_mut(),committed);
 assert!(!app.world().contains_resource::<VnEngine>());
 assert!(!app.world().resource::<DebugBootstrap>().publication_pending);
 assert_eq!(app.world().resource::<DebugBootstrap>().owner.video_views().unwrap()[0],view);
 // Root484 passes Native=None to this real installer. Keep this failing
 // requirement; no direct prepare_startup/install_restore may bypass it.
 assert!(app.world().resource::<Players>().players.contains_key("intro"),
     "The real World installer acknowledged Startup VideoPlay without its owned Native reader");
 app.update();super::draw_current_for_test(app.world_mut());
 let actual=app.world().resource::<Players>().players.get("intro").unwrap();
 let entity=actual.entity.expect("Actual UI-bound video texture must be installed");
 assert_eq!(app.world().get::<Parent>(entity).unwrap().get(),parent);
 assert!(crate::composed_motion::interface_target_matches(app.world().get::<crate::composed_motion::InterfaceTarget>(parent).unwrap(),app.world().get::<crate::composed_motion::InterfaceProjectionTarget>(parent),"native_media","player",view.ui_projection.as_ref()));
 assert!(app.world().get::<UiImage>(entity).is_some());
}

#[test]
fn true_startup_retained_publication_can_close_playout_under_same_fence_without_restarting_cursor(){
 use bevy::audio::Decodable;
 let script=SCRIPT.replace("set after=1}","video.volume(\"intro\",0.25) set after=1}");
 let(_root,mut app)=fixture_source(&script);let first=core(&app);let native=native_ready(&mut app,&first);let sources=sources_ready(&mut app,&first);
 let publication=publish(&mut app,first,native,sources).unwrap();acknowledge_native_only(&mut app,publication);
 let old=app.world().resource::<Players>().players["intro"].audio.as_ref().unwrap().clone();
 old.0.lock().unwrap().running=true;let mut reader=old.decoder();assert!(reader.next().is_some());
 assert_eq!(old.0.lock().unwrap().delivered_sample_offset(),Some(1));
 let second=core(&app);let native=prepare_startup(app.world(),&second).unwrap().unwrap();
 assert_eq!(super::atomic_restore::retained_counts_for_test(&native),(0,1));let sources=sources_ready(&mut app,&second);
 verify_startup(app.world(),&second,&native).unwrap();crate::checkpoint_sources::verify_restore_assets(app.world(),&sources).unwrap();
 let mut startup=app.world_mut().remove_resource::<DebugBootstrap>().unwrap();
 let guard=startup.owner.begin_prepared_host_step_commit(second).unwrap();native.verify_startup_guard(&guard,&startup.broker).unwrap();
 let committed=native.with_suspended_retirement(|retire|crate::checkpoint_sources::publish_startup_with_retirement(&sources,guard,retire)).unwrap();
 // Fence has released, so this proves buffer closure rather than incidental
 // gate contention. The real logical track is still Playing.
 let publication=committed.publication.unwrap();assert_eq!(publication.batch.videos[0].track.playback,Playback::Playing);
 for _ in 0..4{assert_eq!(reader.next(),Some(0.0));}
 {let b=old.0.lock().unwrap();assert!(!b.running&&!b.revoked);assert_eq!(b.delivered_sample_offset(),Some(1));}
 startup.publication_pending=true;app.world_mut().insert_resource(startup);
 crate::checkpoint_sources::install(app.world_mut(),sources);install_restore(app.world_mut(),native);
 let current=app.world().resource::<Players>().players["intro"].audio.as_ref().unwrap();assert!(Arc::ptr_eq(&old.0,&current.0));
 for _ in 0..4{assert_eq!(reader.next(),Some(0.0));}
 assert_eq!(old.0.lock().unwrap().delivered_sample_offset(),Some(1));
 acknowledge_native_only(&mut app,publication);
 // No Startup temporal authority is invented just to resume this test. The
 // real clock receipt/World tick positive belongs to the pending integration.
}

#[test]
fn actual_startup_tick_without_world_clock_closes_real_reader_and_queues_no_position(){
 use bevy::audio::Decodable;
 let(_root,mut app)=fixture_source(SCRIPT);let first=core(&app);let native=native_ready(&mut app,&first);let sources=sources_ready(&mut app,&first);
 let publication=publish(&mut app,first,native,sources).unwrap();acknowledge_native_only(&mut app,publication);
 app.init_resource::<crate::scene_publication::VideoFeedbackQueue>()
    .init_resource::<crate::accessibility::Accessibility>()
    .init_resource::<crate::systems::settings_menu::Settings>()
    .insert_resource(State::new(VnState::Waiting));
 let owner_before=app.world().resource::<DebugBootstrap>().owner.snapshot();
 let source_before=app.world().resource::<DebugBootstrap>().owner.observation_source().unwrap();
 let view=app.world().resource::<DebugBootstrap>().owner.video_views().unwrap()[0].clone();
 assert!(!crate::scene_publication::video_clock_allowed(app.world(),&view));
 let audio=app.world().resource::<Players>().players["intro"].audio.as_ref().unwrap().clone();
 // Simulate an outgoing physical reader still running when its World gate
 // closes. The real current owner and Source are not replaced or fabricated.
 audio.0.lock().unwrap().running=true;let mut source=audio.decoder();assert!(source.next().is_some());
 assert_eq!(audio.0.lock().unwrap().delivered_sample_offset(),Some(1));
 super::tick(app.world_mut());
 assert!(!audio.0.lock().unwrap().running);for _ in 0..4{assert_eq!(source.next(),Some(0.0));}
 assert_eq!(audio.0.lock().unwrap().delivered_sample_offset(),Some(1));
 assert!(crate::scene_publication::video_reports_for_test(app.world()).is_empty());
 assert_eq!(app.world().resource::<DebugBootstrap>().owner.snapshot(),owner_before);
 assert!(app.world().resource::<DebugBootstrap>().owner.accepts_observation_source(&source_before));
 assert!(!app.world().contains_resource::<VnEngine>());
}

#[path="native_video_wait_world_tests.rs"]
mod wait_world_tests;
