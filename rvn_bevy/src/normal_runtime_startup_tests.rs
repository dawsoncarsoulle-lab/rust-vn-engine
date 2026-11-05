//! Actual normal loader boundary. Prepared behavior RED, no native/GPU proof.
use super::*;
use rvn_graph::{GraphDocument,GraphKind,NodeKind,import_script,prepare_native_frame_graph};
fn native_project(name:&str,raw:&str,kind:NodeKind)->PathBuf{
 let parsed=rvn_parser::parse(raw).unwrap();let mut graph=import_script(&parsed).unwrap().into_iter().find(|graph|matches!(graph.kind,GraphKind::Init)).unwrap();assert!(graph.nodes.values().any(|node|node.kind==kind));
 graph=GraphDocument::from_json(&graph.to_pretty_json().unwrap()).unwrap();graph.enable_native_frame_preparation().unwrap();let compiled=prepare_native_frame_graph(&graph).unwrap();assert!(compiled.source.contains("flow.consume_v2("));
 let base=PathBuf::from(std::env::var("RVN_NORMAL_RUNTIME_FIXTURES").expect("owned normal-runtime fixtures"));let root=base.join(format!("{name}-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
 for dir in ["assets","locales","saves"]{fs::create_dir_all(root.join(dir)).unwrap();}
 fs::write(root.join("rvn.toml"),"[project]\ntitle='Actual normal Init startup'\nmain_script='main.rvn'\nstart_label='start'\n[paths]\nassets='assets'\nlocales='locales'\nsaves='saves'\ntheme='theme.toml'\n").unwrap();
 fs::write(root.join("theme.toml"),"[title_screen]\nenabled=true\n").unwrap();fs::write(root.join("main.rvn"),format!("{}\nscene label start\nset narrative_started=true\n\"Unexecuted before Start\"\n",compiled.source)).unwrap();fs::write(root.join("actual.graph.json"),graph.to_pretty_json().unwrap()).unwrap();
 let manager=PersistentDataManager::new(root.join("saves")).unwrap();let mut persistent=PersistentData::default();persistent.script_vars.insert("persistent.counter".into(),rvn_parser::Value::Int(7));manager.save(&persistent).unwrap();
 fs::write(root.join("locales/fr.toml"),"[strings]\n").unwrap();
 assert!(!root.join(rvn_core::debugger_provenance::SIDECAR).exists());root
}
fn admits_normal(name:&str,raw:&str,kind:NodeKind){
 let root=native_project(name,raw,kind);
 // This is the actual production normal loader; no debug nonce/overlay/socket.
 match load_runtime_game(&root){Ok(launch)=>match launch.engine{RuntimeLaunchOwner::Startup(startup)=>{assert_eq!(startup.profile,debugger_startup::BootstrapProfile::RuntimeInitial);assert!(!startup.owner.is_complete());assert!(!startup.owner.state().vars.contains_key("seen"));},RuntimeLaunchOwner::Game(_)=>panic!("Actual native Init must retain its pending owner")},Err(error)=>panic!("Actual normal loader must admit the real pending Init owner: {error}")}
}
#[test]
fn normal_runtime_loader_while_without_preview_admits_real_startup(){admits_normal("normal-while","init {set seen=0 while seen<3 {set seen=seen+1}}\n",NodeKind::While);}
#[test]
fn normal_runtime_loader_each_without_preview_admits_real_startup(){admits_normal("normal-each","init {set seen=0 for item in [1,2,3] {set seen=seen+item}}\n",NodeKind::ForEach);}
#[test]
fn normal_runtime_loader_range_without_preview_admits_real_startup(){admits_normal("normal-range","init {set seen=0 flow.for_loop(i,1,3) {set seen=seen+i}}\n",NodeKind::ForLoopWithBreak);}

pub(crate) fn runtime_app(name:&str,raw:&str,kind:NodeKind)->(PathBuf,App){
 let root=native_project(name,raw,kind);runtime_app_project(root)
}
fn runtime_app_project(root:PathBuf)->(PathBuf,App){runtime_app_after_admission(root,|_|{})}
fn runtime_app_after_admission(root:PathBuf,change:impl FnOnce(&Path))->(PathBuf,App){runtime_app_with_hooks(root,change,|_|{})}
fn runtime_app_with_hooks(root:PathBuf,change:impl FnOnce(&Path),install:impl FnOnce(&mut App))->(PathBuf,App){
 let wave=PathBuf::from(std::env::var("RVN_STARTUP_WORLD_FIXTURES").expect("actual PCM fixture mount")).join("pcm/host.wav");fs::create_dir_all(root.join("assets/music")).unwrap();fs::copy(wave,root.join("assets/music/host.wav")).unwrap();
 let launch=load_runtime_game(&root).unwrap();change(&root);
 assert!(!root.join(rvn_core::debugger_provenance::SIDECAR).exists());
 let mut app=App::new();let (locale_root,declarations)=launch.closed_locales.unwrap();
 portable_text_resources::register_verified_text_sources(&mut app,launch.asset_root.clone(),locale_root).unwrap();
 app.add_plugins(MinimalPlugins).add_plugins(AssetPlugin{file_path:launch.asset_root,watch_for_changes_override:Some(false),..default()});
 launch.engine.install(&mut app);
 app.init_resource::<scene_publication::ScenePresentation>().add_event::<VnCommand>()
   .add_plugins(string_table_assets::StringTableAssetsPlugin::new(launch.cfg.string_tables))
   .add_plugins(portable_text_resources::ClosedLocaleAssetsPlugin::new(declarations).unwrap())
   .init_asset::<bevy::audio::AudioSource>().init_asset_loader::<bevy::audio::AudioLoader>().init_asset::<checkpoint_pcm::CheckpointPcm>();
 let (root,mut app)=startup_full_launch_tests::complete_cpu_runtime(root,app,true);
 app.add_plugins(checkpoint_requests::CheckpointRequestsPlugin).add_plugins(scene_effects::SceneEffectsPlugin)
   .add_plugins(scene_retired::SceneRetiredPlugin).add_plugins(scene_audio::SceneAudioPlugin)
   .add_plugins(web_music::WebMusicPlugin).add_plugins(presentation_order::PresentationOrderPlugin)
   .add_plugins(accessibility::AccessibilityPlugin).add_plugins(portable_images::PortableImagesPlugin);
 app.insert_resource(settings_from_persistent(&launch.persistent_data,&launch.locale_cfg.current));
 app.insert_resource(resources::PersistentDataResource{manager:launch.persistent_manager,data:launch.persistent_data});
 app.insert_resource(resources::LocaleConfig{available_langs:launch.locale_cfg.available,last_modified_current:std::time::SystemTime::UNIX_EPOCH,last_modified_default:std::time::SystemTime::UNIX_EPOCH});
 app.insert_resource(launch.theme);app.world_mut().resource_mut::<NextState<VnState>>().set(VnState::Stepping);install(&mut app);app.finish();app.cleanup();(root,app)
}
pub(crate) fn pump_runtime(app:&mut App){
 let end=std::time::Instant::now()+std::time::Duration::from_secs(20);
 loop{app.update();
   if let Some(startup)=app.world().get_resource::<debugger_startup::DebugBootstrap>(){assert!(startup.failed.is_none(),"Runtime Startup failed: {:?}",startup.failed);assert!(!app.world().contains_resource::<VnEngine>());}
   else{assert!(app.world().contains_resource::<VnEngine>());if *app.world().resource::<State<VnState>>().get()==VnState::TitleScreen{return;}}
   assert!(std::time::Instant::now()<end,"Runtime Startup did not hand off to title");std::thread::sleep(std::time::Duration::from_millis(2));
 }
}
fn actual_runtime_main(name:&str,raw:&str,kind:NodeKind,expected:i64){
 let(_,mut app)=runtime_app(name,raw,kind);let initial=app.world().resource::<debugger_startup::DebugBootstrap>().owner.snapshot();
 pump_runtime(&mut app);
 let engine=app.world().resource::<VnEngine>();assert_eq!(engine.0.instance_id(),initial.instance);assert_eq!(engine.0.runtime_epoch(),initial.epoch);
 assert_eq!(engine.0.state.vars["seen"],rvn_parser::Value::Int(expected));assert!(!engine.0.state.vars.contains_key("narrative_started"));assert!(!editor_debugger::enabled(engine));
 assert_eq!(engine.0.export_persistent_vars().get("persistent.counter"),Some(&rvn_parser::Value::Int(7)));
 for _ in 0..3{app.update();}assert!(!app.world().resource::<VnEngine>().0.state.vars.contains_key("narrative_started"));
}
#[test]fn normal_runtime_main_while_finishes_init_then_title_without_narration(){actual_runtime_main("main-while","init {set seen=0 while seen<3 {set seen=seen+1}}\n",NodeKind::While,3);}
#[test]fn normal_runtime_main_each_finishes_init_then_title_without_narration(){actual_runtime_main("main-each","init {set seen=0 for item in [1,2,3] {set seen=seen+item}}\n",NodeKind::ForEach,6);}
#[test]fn normal_runtime_main_range_finishes_init_then_title_without_narration(){actual_runtime_main("main-range","init {set seen=0 flow.for_loop(i,1,3) {set seen=seen+i}}\n",NodeKind::ForLoopWithBreak,6);}
#[test]fn normal_runtime_main_initial_loading_and_focus_modal_state_barriers_do_not_advance(){
 let(_,mut app)=runtime_app("main-barriers","init {set seen=0 while seen<3 {set seen=seen+1}}\n",NodeKind::While);let initial=app.world().resource::<debugger_startup::DebugBootstrap>().owner.snapshot();
 app.update();assert_eq!(app.world().resource::<debugger_startup::DebugBootstrap>().owner.snapshot(),initial);
 let window=app.world_mut().query_filtered::<Entity,With<bevy::window::PrimaryWindow>>().single(app.world());app.world_mut().get_mut::<Window>(window).unwrap().focused=false;
 for _ in 0..4{app.update();assert_eq!(app.world().resource::<debugger_startup::DebugBootstrap>().owner.snapshot(),initial);}app.world_mut().get_mut::<Window>(window).unwrap().focused=true;
 app.world_mut().insert_resource(source_menus::SourceMenus::for_test_role(rvn_ui::PageRole::Pause));for _ in 0..3{app.update();assert_eq!(app.world().resource::<debugger_startup::DebugBootstrap>().owner.snapshot(),initial);}app.world_mut().insert_resource(source_menus::SourceMenus::default());
 app.world_mut().resource_mut::<NextState<VnState>>().set(VnState::Menu);app.update();let paused=app.world().resource::<debugger_startup::DebugBootstrap>().owner.snapshot();for _ in 0..3{app.update();assert_eq!(app.world().resource::<debugger_startup::DebugBootstrap>().owner.snapshot(),paused);}app.world_mut().resource_mut::<NextState<VnState>>().set(VnState::Stepping);pump_runtime(&mut app);
}

fn await_failed(app:&mut App)->String{
 let end=std::time::Instant::now()+std::time::Duration::from_secs(20);
 loop{app.update();assert!(!app.world().contains_resource::<VnEngine>());let startup=app.world().resource::<debugger_startup::DebugBootstrap>();assert!(!startup.owner.state().vars.contains_key("seen"));if let Some(error)=&startup.failed{return error.clone();}assert!(std::time::Instant::now()<end,"Failed actual asset did not terminate Startup loading");std::thread::sleep(std::time::Duration::from_millis(2));}
}
#[test]fn normal_runtime_main_failed_locale_never_becomes_an_infinite_loading_wait(){
 let root=native_project("failed-locale","init {while false {set seen=1}}\n",NodeKind::While);let(_,mut app)=runtime_app_after_admission(root,|root|fs::write(root.join("locales/fr.toml"),[0xff]).unwrap());let error=await_failed(&mut app);assert!(error.contains("locale source failed"),"{error}");assert_eq!(app.world().resource::<ScriptErrorMessage>().0,error);
}
#[test]fn normal_runtime_main_failed_table_is_refused_before_first_computation(){
 let root=native_project("failed-table","init {while false {set seen=1}}\n",NodeKind::While);let id="17411a3e431e4c9388b78a2b9825b130";
 let asset=rvn_core::string_table::StringTableAsset{id:id.into(),namespace:"VN".into(),entries:Default::default()};fs::write(root.join("assets/table.rvnstrings"),rvn_core::string_table_file::StringTableFile::new(asset).unwrap().encode().unwrap()).unwrap();
 let mut cfg=fs::read_to_string(root.join("rvn.toml")).unwrap();cfg.push_str(&format!("\n[string_tables]\n'{id}'='table.rvnstrings'\n"));fs::write(root.join("rvn.toml"),cfg).unwrap();let(_,mut app)=runtime_app_after_admission(root,|root|fs::write(root.join("assets/table.rvnstrings"),b"invalid actual asset bytes").unwrap());let error=await_failed(&mut app);assert!(error.contains("string-table"),"{error}");
}
#[test]fn normal_runtime_loader_invalid_label_refuses_before_authored_init(){
 let root=native_project("invalid-entry","init {set seen=0 while seen<3 {set seen=seen+1}}\n",NodeKind::While);let cfg=fs::read_to_string(root.join("rvn.toml")).unwrap().replace("start_label='start'","start_label='absent'");fs::write(root.join("rvn.toml"),cfg).unwrap();assert!(load_runtime_game(&root).err().unwrap().contains("absent"));
}
#[derive(Resource)]struct MenuIntent(rvn_ui::Action);
#[derive(Resource,Default)]struct ActionResult(Option<Result<(),String>>);
fn invoke_menu(mut commands:Commands,mut menus:ResMut<menu_documents::Menus>,mut context:menu_documents::ActionContext,intent:Res<MenuIntent>,mut result:ResMut<ActionResult>){result.0=Some(menu_documents::dispatch_source_action(&mut commands,&mut menus,&mut context,"controlled-caller-fixture",intent.0.clone()));}
pub(crate) fn menu_action(app:&mut App,action:rvn_ui::Action)->Result<(),String>{use bevy::ecs::system::RunSystemOnce;app.insert_resource(MenuIntent(action)).init_resource::<ActionResult>();app.world_mut().run_system_once(invoke_menu);app.world_mut().resource_mut::<ActionResult>().0.take().unwrap()}
fn await_restarted(app:&mut App,old:u64){
 let end=std::time::Instant::now()+std::time::Duration::from_secs(20);loop{app.update();if let Some(startup)=app.world().get_resource::<debugger_startup::DebugBootstrap>(){assert!(startup.failed.is_none(),"Restart failed: {:?}",startup.failed);}else if app.world().get_resource::<VnEngine>().is_some_and(|engine|engine.0.instance_id()!=old){return;}assert!(std::time::Instant::now()<end,"Real restart did not complete");std::thread::sleep(std::time::Duration::from_millis(2));}
}
#[test]fn normal_runtime_restart_menu_startscene_admits_without_clearing_then_hands_off_once(){
 let(_,mut app)=runtime_app("restart-menu","init {set seen=0 for item in [1,2,3] {set seen=seen+item}}\n",NodeKind::ForEach);pump_runtime(&mut app);let old=app.world().resource::<VnEngine>().0.instance_id();let epoch=app.world().resource::<VnEngine>().0.runtime_epoch();let before=serde_json::to_vec(&app.world().resource::<VnEngine>().0.state).unwrap();
 assert!(menu_action(&mut app,rvn_ui::Action::StartScene("absent".into())).is_err());assert_eq!(serde_json::to_vec(&app.world().resource::<VnEngine>().0.state).unwrap(),before);assert!(!app.world().contains_resource::<debugger_startup::DebugBootstrap>());
 menu_action(&mut app,rvn_ui::Action::StartScene("start".into())).unwrap();assert_eq!(app.world().resource::<VnEngine>().0.instance_id(),old);assert!(!app.world().contains_resource::<debugger_startup::DebugBootstrap>());
 let window=app.world_mut().query_filtered::<Entity,With<bevy::window::PrimaryWindow>>().single(app.world());app.world_mut().get_mut::<Window>(window).unwrap().focused=false;app.update();assert!(!app.world().contains_resource::<VnEngine>());let startup=app.world().resource::<debugger_startup::DebugBootstrap>();assert_eq!(startup.profile,debugger_startup::BootstrapProfile::RuntimeRestart);assert_eq!(startup.owner.snapshot().epoch,epoch+1);assert!(!startup.owner.state().vars.contains_key("seen"));
 app.world_mut().get_mut::<Window>(window).unwrap().focused=true;await_restarted(&mut app,old);let engine=app.world().resource::<VnEngine>();assert_eq!(engine.0.runtime_epoch(),epoch+1);assert_eq!(engine.0.state.vars["seen"],rvn_parser::Value::Int(6));assert!(!editor_debugger::enabled(engine));
}
#[test]fn normal_runtime_restart_actual_title_newgame_after_finish_is_async(){
 use bevy::ecs::system::RunSystemOnce;
 let(_,mut app)=runtime_app("restart-title","init {set seen=0 flow.for_loop(i,1,3) {set seen=seen+i}}\n",NodeKind::ForLoopWithBreak);pump_runtime(&mut app);
 {let mut engine=app.world_mut().resource_mut::<VnEngine>();assert!(engine.0.step_until_interaction().unwrap().is_some());engine.0.advance_dialogue().unwrap();assert!(engine.0.step_until_interaction().unwrap().is_none());assert!(engine.0.is_finished());}app.update();
 let old=app.world().resource::<VnEngine>().0.instance_id();app.world_mut().spawn((ButtonBundle{interaction:Interaction::Pressed,..default()},systems::title::TitleButton::NewGame,systems::title::TitleButtonColors{normal:Color::NONE,hover:Color::NONE,pressed:Color::NONE}));
 app.world_mut().run_system_once(systems::title::title_interaction_system);assert_eq!(app.world().resource::<VnEngine>().0.instance_id(),old);assert!(!app.world().contains_resource::<debugger_startup::DebugBootstrap>());await_restarted(&mut app,old);assert_eq!(app.world().resource::<VnEngine>().0.state.vars["seen"],rvn_parser::Value::Int(6));
}

#[test]fn normal_runtime_main_title_disabled_advances_only_after_init_handoff(){
 let root=native_project("no-title","init {set seen=0 while seen<3 {set seen=seen+1}}\n",NodeKind::While);fs::write(root.join("theme.toml"),"[title_screen]\nenabled=false\n").unwrap();let(_,mut app)=runtime_app_project(root);let end=std::time::Instant::now()+std::time::Duration::from_secs(20);
 loop{app.update();if let Some(startup)=app.world().get_resource::<debugger_startup::DebugBootstrap>(){assert!(startup.failed.is_none(),"{:?}",startup.failed);assert!(!startup.owner.state().vars.contains_key("narrative_started"));assert!(!app.world().contains_resource::<VnEngine>());}else{let engine=app.world().resource::<VnEngine>();assert_eq!(engine.0.state.vars["seen"],rvn_parser::Value::Int(3));assert!(!editor_debugger::enabled(engine));if engine.0.state.vars.get("narrative_started")==Some(&rvn_parser::Value::Bool(true)){return;}}
 assert!(std::time::Instant::now()<end,"Disabled title did not use its actual OnEnter transition");std::thread::sleep(std::time::Duration::from_millis(2));}
}

fn history_state(world:&World)->Vec<(String,String)>{world.resource::<DialogueHistory>().lines.iter().map(|line|(line.character.clone(),line.text.clone())).collect()}
fn settings_state(world:&World)->serde_json::Value{let settings=world.resource::<Settings>();serde_json::json!([settings.language,settings.music_volume.to_bits(),settings.sfx_volume.to_bits(),settings.text_speed.to_bits(),settings.typewriter,settings.fullscreen,settings.auto_speed.to_bits()])}
#[test]fn normal_runtime_restart_before_publication_restores_actual_player_menu_history_locale_and_media(){
 let(_,mut app)=runtime_app("restart-restore","init {music.play(\"host.wav\") set seen=0 while seen<3 {set seen=seen+1}}\n",NodeKind::While);pump_runtime(&mut app);
 app.world_mut().resource_mut::<DialogueHistory>().add("old".into(),"kept history".into());let source_title=app.world().resource::<source_menus::SourceMenus>().active(rvn_ui::PageRole::Title);
 let old=app.world().resource::<VnEngine>().0.instance_id();let engine_state=serde_json::to_value(&app.world().resource::<VnEngine>().0.state).unwrap();let locale=app.world().resource::<VnEngine>().0.locale.as_ref().unwrap().portable_receipt().unwrap();let gate=app.world().resource::<VnEngine>().0.renderer.audio_checkpoints.gate();
 let history=history_state(app.world());let settings=settings_state(app.world());let menus=app.world().resource::<menu_documents::Menus>().restart_state_for_test();let music=app.world().resource::<MusicEntity>().0;let installed=serde_json::to_value(&app.world().resource::<scene_publication::ScenePresentation>().batch.as_ref().unwrap().state).unwrap();
 menu_action(&mut app,rvn_ui::Action::NewGame).unwrap();let window=app.world_mut().query_filtered::<Entity,With<bevy::window::PrimaryWindow>>().single(app.world());app.world_mut().get_mut::<Window>(window).unwrap().focused=false;app.update();assert!(!app.world().contains_resource::<VnEngine>());
 assert_eq!(history_state(app.world()),history);assert_eq!(settings_state(app.world()),settings);assert_eq!(app.world().resource::<menu_documents::Menus>().restart_state_for_test(),menus);
 app.world_mut().resource_mut::<debugger_startup::DebugBootstrap>().broker=checkpoint_broker::AudioCheckpointBroker::default();app.update();let engine=app.world().resource::<VnEngine>();assert_eq!(engine.0.instance_id(),old);assert_eq!(serde_json::to_value(&engine.0.state).unwrap(),engine_state);assert_eq!(engine.0.locale.as_ref().unwrap().portable_receipt().unwrap(),locale);assert!(engine.0.renderer.audio_checkpoints.gate().same_gate(&gate));
 assert_eq!(app.world().resource::<source_menus::SourceMenus>().active(rvn_ui::PageRole::Title),source_title);assert_eq!(history_state(app.world()),history);assert_eq!(settings_state(app.world()),settings);assert_eq!(app.world().resource::<menu_documents::Menus>().restart_state_for_test(),menus);assert_eq!(app.world().resource::<MusicEntity>().0,music);assert_eq!(serde_json::to_value(&app.world().resource::<scene_publication::ScenePresentation>().batch.as_ref().unwrap().state).unwrap(),installed);
}
#[test]fn normal_runtime_restart_after_first_publication_does_not_restore_prior_owner(){
 let(_,mut app)=runtime_app("restart-committed","init {set seen=0 while seen<30 {set seen=seen+1}}\n",NodeKind::While);pump_runtime(&mut app);let old=app.world().resource::<VnEngine>().0.instance_id();let epoch=app.world().resource::<VnEngine>().0.runtime_epoch();
 menu_action(&mut app,rvn_ui::Action::NewGame).unwrap();let end=std::time::Instant::now()+std::time::Duration::from_secs(20);
 loop{app.update();assert!(!app.world().contains_resource::<VnEngine>());if app.world().resource::<scene_publication::ScenePresentation>().batch.as_ref().is_some_and(|batch|batch.runtime_epoch==epoch+1){break;}assert!(std::time::Instant::now()<end);std::thread::sleep(std::time::Duration::from_millis(2));}
 let actual=app.world().resource::<debugger_startup::DebugBootstrap>().owner.snapshot().instance;assert_ne!(actual,old);app.world_mut().resource_mut::<debugger_startup::DebugBootstrap>().broker=checkpoint_broker::AudioCheckpointBroker::default();app.update();assert!(!app.world().contains_resource::<VnEngine>());let startup=app.world().resource::<debugger_startup::DebugBootstrap>();assert_eq!(startup.owner.snapshot().instance,actual);assert!(startup.failed.is_some());assert_eq!(app.world().resource::<scene_publication::ScenePresentation>().batch.as_ref().unwrap().runtime_epoch,epoch+1);
}
#[test]fn normal_runtime_restart_actual_menu_newgame_distinct_from_startscene(){
 let(_,mut app)=runtime_app("restart-newgame","init {set seen=0 while seen<3 {set seen=seen+1}}\n",NodeKind::While);pump_runtime(&mut app);let old=app.world().resource::<VnEngine>().0.instance_id();menu_action(&mut app,rvn_ui::Action::NewGame).unwrap();assert_eq!(app.world().resource::<VnEngine>().0.instance_id(),old);await_restarted(&mut app,old);assert_eq!(app.world().resource::<VnEngine>().0.state.vars["seen"],rvn_parser::Value::Int(3));
}

#[test]fn normal_runtime_loader_explicit_closed_profile_uses_the_same_pending_owner(){
 let root=native_project("closed-profile","init {set seen=0 while seen<3 {set seen=seen+1}}\n",NodeKind::While);let launch=load_runtime_game_text_profile(&root,true).unwrap();assert!(launch.closed_locales.is_some());let RuntimeLaunchOwner::Startup(startup)=launch.engine else{panic!("Closed native launch must not bypass asynchronous Init")};assert_eq!(startup.profile,debugger_startup::BootstrapProfile::RuntimeInitial);assert!(!startup.owner.is_complete());assert!(!startup.owner.state().vars.contains_key("seen"));
}

#[test]fn normal_runtime_optional_qa_and_preview_plugins_wait_for_real_game_without_consuming_locals(){
 let root=native_project("optional-plugins","init {set seen=0 while seen<3 {set seen=seen+1}}\n",NodeKind::While);let output=root.join("qa");fs::create_dir(&output).unwrap();
 let(_,mut app)=runtime_app_with_hooks(root,|_|{},|app|{
  let _output=TestEnvironment::set("RVN_QA_OUTPUT",&output);let _demo=TestEnvironment::set("RVN_UI_PREVIEW_DATA","filled");app.add_plugins(qa_playthrough::QaPlugin).add_plugins(menu_preview_data::MenuPreviewDataPlugin);
 });
 let initial=app.world().resource::<debugger_startup::DebugBootstrap>().owner.snapshot();let window=app.world_mut().query_filtered::<Entity,With<bevy::window::PrimaryWindow>>().single(app.world());app.world_mut().get_mut::<Window>(window).unwrap().focused=false;
 for _ in 0..6{app.update();assert!(!app.world().contains_resource::<VnEngine>());assert_eq!(app.world().resource::<debugger_startup::DebugBootstrap>().owner.snapshot(),initial);}
 assert_eq!(fs::read_dir(output).unwrap().count(),0,"No QA screenshot or fake save may be produced without the actual Game owner");
}

fn expects_absence_admission(root:PathBuf){
 match load_runtime_game(&root){Ok(launch)=>assert!(matches!(launch.engine,RuntimeLaunchOwner::Startup(_)),"Minimal locale fallback still requires the real pending Init owner"),Err(error)=>panic!("Normal launch must preserve its valid missing-locale fallback through an authenticated provider contract: {error}")}
}
// Separate expected behavior RED group. No synthetic source/file/receipt.
#[test]fn normal_absence_contract_red_missing_locale_directory(){let root=native_project("absence-directory","init {while false {}}\n",NodeKind::While);fs::remove_file(root.join("locales/fr.toml")).unwrap();fs::remove_dir(root.join("locales")).unwrap();expects_absence_admission(root);}
#[test]fn normal_absence_contract_red_missing_default_locale_file(){let root=native_project("absence-default","init {while false {}}\n",NodeKind::While);fs::remove_file(root.join("locales/fr.toml")).unwrap();expects_absence_admission(root);}
#[test]fn normal_absence_contract_red_missing_current_locale_uses_real_default_fallback(){let root=native_project("absence-current","init {while false {}}\n",NodeKind::While);fs::write(root.join("assets/config.toml"),"[locale]\ndefault='fr'\ncurrent='en'\navailable=['fr','en']\n").unwrap();assert!(!root.join("locales/en.toml").exists());expects_absence_admission(root);}

struct TestEnvironment { key:&'static str,previous:Option<std::ffi::OsString> }
impl TestEnvironment{fn set(key:&'static str,value:impl AsRef<std::ffi::OsStr>)->Self{let previous=std::env::var_os(key);std::env::set_var(key,value);Self{key,previous}}}
impl Drop for TestEnvironment{fn drop(&mut self){if let Some(value)=self.previous.take(){std::env::set_var(self.key,value);}else{std::env::remove_var(self.key);}}}
#[test]fn normal_runtime_host_boundary_precedes_candidate_rng_and_failing_core_expression(){
 let source="init {set crash=random(1,9)/0}\nscene label start\n\"Ready\"\n";
 let(_,mut app)=debugger_startup::tests::fixture_source(source);let before=app.world().resource::<debugger_startup::DebugBootstrap>().owner.snapshot();let random=app.world().resource::<debugger_startup::DebugBootstrap>().owner.state().random;
 portable_text_resources::mark_missed_events(app.world_mut(),1);let error=debugger_startup::prepare_world_step(app.world(),rvn_core::editor_debugger::StepKind::Continue).err().unwrap();assert!(error.contains("Missed events"),"Host failure must precede Core arithmetic, got {error}");assert_eq!(app.world().resource::<debugger_startup::DebugBootstrap>().owner.snapshot(),before);assert_eq!(app.world().resource::<debugger_startup::DebugBootstrap>().owner.state().random,random);
 let(_,app)=debugger_startup::tests::fixture_source(source);let error=debugger_startup::prepare_world_step(app.world(),rvn_core::editor_debugger::StepKind::Continue).err().unwrap();assert!(!error.contains("Missed events"));assert!(error.to_lowercase().contains("division"),"Control must actually reach the failing candidate computation: {error}");
}
#[test]fn normal_runtime_host_undrained_actual_asset_event_precedes_step_and_entry(){
 let(_,mut app)=debugger_startup::tests::fixture_source("init {set crash=random(1,9)/0}\nscene label start\n\"Ready\"\n");
 let handle=app.world().resource::<AssetServer>().get_handle::<portable_text_resources::LoadedLocaleSource>(bevy::asset::AssetPath::from("en.toml").with_source("rvn-closed-locales-v1")).unwrap();app.world_mut().resource_mut::<Events<bevy::asset::AssetEvent<portable_text_resources::LoadedLocaleSource>>>().send(bevy::asset::AssetEvent::Modified{id:handle.id()});
 let error=debugger_startup::prepare_world_step(app.world(),rvn_core::editor_debugger::StepKind::Continue).err().unwrap();assert!(error.contains("queue is not drained"),"{error}");
 let(_,mut app)=debugger_startup::tests::fixture_source("init {}\nscene label start\n\"Ready\"\n");assert!(app.world().resource::<debugger_startup::DebugBootstrap>().owner.is_complete());app.world_mut().resource_mut::<debugger_startup::DebugBootstrap>().start_label=Some("absent-entry-negative-control".into());portable_text_resources::mark_missed_events(app.world_mut(),1);
 let error=debugger_startup::prepare_world_entry(app.world()).err().unwrap();assert!(error.contains("Missed events"),"Invalid host must precede even entry-label calculation: {error}");assert!(!app.world().contains_resource::<VnEngine>());
}

#[cfg(not(target_os="android"))]
#[path="normal_locale_absence_tests.rs"]
mod absence0664_tests;

#[path="normal_locale_selection_mode_tests.rs"]
mod selection_mode_contracts;

// Append in normal_runtime_startup_tests.rs in both RED43D8 and candidate.
// No production bypass, no manually cleared cache/source or synthetic Game.
#[test]
fn normal_runtime_cutover_holds_actual_title_entities_and_state_until_publication(){
 let(_,mut app)=runtime_app("hold-title","init {for item in [1,2] {set seen=item}}\n",NodeKind::ForEach);pump_runtime(&mut app);
 let roots=app.world_mut().query_filtered::<Entity,With<systems::title::TitleOverlay>>().iter(app.world()).collect::<Vec<_>>();assert!(!roots.is_empty());let old=app.world().resource::<VnEngine>().0.instance_id();
 let frame=serde_json::to_value(&app.world().resource::<scene_publication::ScenePresentation>().batch.as_ref().unwrap().state).unwrap();
 menu_action(&mut app,rvn_ui::Action::NewGame).unwrap();let window=app.world_mut().query_filtered::<Entity,With<bevy::window::PrimaryWindow>>().single(app.world());app.world_mut().get_mut::<Window>(window).unwrap().focused=false;app.update();
 assert!(!app.world().contains_resource::<VnEngine>());assert_eq!(app.world().resource::<State<VnState>>().get(),&VnState::TitleScreen,"The actual title State changed before replacement publication");let pending=app.world().resource::<debugger_startup::DebugBootstrap>().owner.snapshot();
 for _ in 0..3{app.update();assert_eq!(app.world().resource::<debugger_startup::DebugBootstrap>().owner.snapshot(),pending);assert!(roots.iter().all(|entity|app.world().get_entity(*entity).is_some()),"The genuine title hierarchy was destroyed before accepted publication");assert_eq!(serde_json::to_value(&app.world().resource::<scene_publication::ScenePresentation>().batch.as_ref().unwrap().state).unwrap(),frame);}
 app.world_mut().resource_mut::<debugger_startup::DebugBootstrap>().broker=checkpoint_broker::AudioCheckpointBroker::default();app.update();assert_eq!(app.world().resource::<VnEngine>().0.instance_id(),old);assert!(roots.iter().all(|entity|app.world().get_entity(*entity).is_some()),"Failure must restore the same old hierarchy, not respawn an imitation");
}
#[test]
fn normal_runtime_cutover_retains_old_actual_sfx_reader_and_broker_before_publication(){
 let root=native_project("hold-sfx","init {sfx.play(\"host.wav\") for item in [1,2] {set seen=item}}\n",NodeKind::ForEach);let wave=PathBuf::from(std::env::var("RVN_STARTUP_WORLD_FIXTURES").unwrap()).join("pcm/host.wav");fs::create_dir_all(root.join("assets/sfx")).unwrap();fs::copy(&wave,root.join("assets/host.wav")).unwrap();fs::copy(&wave,root.join("assets/sfx/host.wav")).unwrap();let(_,mut app)=runtime_app_project(root);pump_runtime(&mut app);
 let rows=app.world_mut().query::<(Entity,&scene_audio::OwnedAudio,&Handle<checkpoint_pcm::CheckpointPcm>)>().iter(app.world()).map(|(entity,owned,handle)|(entity,owned.token.clone(),handle.clone(),app.world().resource::<Assets<checkpoint_pcm::CheckpointPcm>>().get(handle).unwrap().clone())).collect::<Vec<_>>();assert!(!rows.is_empty(),"Positive control must own a real installed SFX reader");
 let game=app.world().resource::<VnEngine>();let broker=game.0.renderer.audio_checkpoints.clone();let gate=broker.gate();let old=game.0.instance_id();let epoch=game.0.runtime_epoch();let state=game.0.state.clone();let before=broker.capture_frame(&state,old,epoch).unwrap();
 menu_action(&mut app,rvn_ui::Action::NewGame).unwrap();assert_eq!(broker.capture_frame(&state,old,epoch).unwrap(),before,"Candidate admission changed the real broker publication");
 let window=app.world_mut().query_filtered::<Entity,With<bevy::window::PrimaryWindow>>().single(app.world());app.world_mut().get_mut::<Window>(window).unwrap().focused=false;app.update();
 for _ in 0..3{app.update();assert!(!app.world().contains_resource::<VnEngine>());assert_eq!(broker.capture_frame(&state,old,epoch).unwrap(),before);for(entity,token,handle,reader)in &rows{assert_eq!(&app.world().get::<scene_audio::OwnedAudio>(*entity).expect("The accepted old SFX entity was disposed before new publication").token,token);assert!(app.world().resource::<Assets<checkpoint_pcm::CheckpointPcm>>().get(handle).unwrap().same_reader(reader));}}
 assert!(app.world().resource::<debugger_startup::DebugBootstrap>().broker.gate().same_gate(&gate),"Replacement must share the real original output fence");
}
fn actual_accessibility_key(app:&mut App,window:Entity,key:KeyCode,logical:bevy::input::keyboard::Key){app.world_mut().send_event(bevy::input::keyboard::KeyboardInput{key_code:key,logical_key:logical,state:bevy::input::ButtonState::Pressed,window});}
#[test]
fn normal_runtime_cutover_actual_accessibility_keys_clear_blocked_latch_and_resume_after_focus(){
 let(_,mut app)=runtime_app("hold-access","init {for item in [1,2] {set seen=item}}\n",NodeKind::ForEach);pump_runtime(&mut app);let old=app.world().resource::<VnEngine>().0.instance_id();
 menu_action(&mut app,rvn_ui::Action::NewGame).unwrap();let window=app.world_mut().query_filtered::<Entity,With<bevy::window::PrimaryWindow>>().single(app.world());app.world_mut().get_mut::<Window>(window).unwrap().focused=false;app.update();let before=app.world().resource::<debugger_startup::DebugBootstrap>().owner.snapshot();
 actual_accessibility_key(&mut app,window,KeyCode::F8,bevy::input::keyboard::Key::F8);app.update();assert!(app.world().resource::<accessibility::Accessibility>().open);assert!(app.world().resource::<accessibility::Accessibility>().blocked);
 app.world_mut().get_mut::<Window>(window).unwrap().focused=true;app.update();assert_eq!(app.world().resource::<debugger_startup::DebugBootstrap>().owner.snapshot(),before,"Actual host panel must block computation");
 actual_accessibility_key(&mut app,window,KeyCode::Escape,bevy::input::keyboard::Key::Escape);app.update();assert!(!app.world().resource::<accessibility::Accessibility>().open);app.update();assert!(!app.world().resource::<accessibility::Accessibility>().blocked,"Closing the actual host panel must clear its latch while replacement is pending");await_restarted(&mut app,old);
}

// Append to normal_runtime_startup_tests; uses genuine published title buttons.
#[test]
fn normal_runtime_cutover_assistive_story_actions_are_ignored_but_host_panel_remains_usable(){
 use bevy::a11y::{accesskit,ActionRequest};
 let(_,mut app)=runtime_app("cutover-assistive","init {for item in [1,2] {set seen=item}}\n",NodeKind::ForEach);pump_runtime(&mut app);
 let target=app.world_mut().query::<(Entity,&systems::title::TitleButton,&bevy::a11y::AccessibilityNode)>().iter(app.world()).find_map(|(entity,button,node)|{
   let node=node.0.clone().build();
   (matches!(button,systems::title::TitleButton::Settings)&&node.supports_action(accesskit::Action::Focus)&&node.supports_action(accesskit::Action::Default)&&!node.is_hidden()&&!node.is_disabled()).then_some(entity)
 }).expect("Positive control must find the real published enabled Settings button");
 fn request(app:&mut App,target:Entity,action:accesskit::Action){app.world_mut().send_event(ActionRequest(accesskit::ActionRequest{action,target:accesskit::NodeId(target.to_bits()),data:None}));}
 // Run the genuine PreUpdate schedule: inspect the actual input handler before
 // PostUpdate semantic projection can overwrite its Focus result.
 request(&mut app,target,accesskit::Action::Focus);app.world_mut().run_schedule(PreUpdate);assert_eq!(app.world().resource::<bevy::a11y::Focus>().0,Some(target));
 app.world_mut().resource_mut::<bevy::a11y::Focus>().0=None;*app.world_mut().get_mut::<Interaction>(target).unwrap()=Interaction::None;
 menu_action(&mut app,rvn_ui::Action::NewGame).unwrap();let window=app.world_mut().query_filtered::<Entity,With<bevy::window::PrimaryWindow>>().single(app.world());app.world_mut().get_mut::<Window>(window).unwrap().focused=false;app.update();
 assert!(!app.world().contains_resource::<VnEngine>());assert!(app.world().get_entity(target).is_some());assert!(!app.world().resource::<accessibility::Accessibility>().blocked,"The story guard must be actual retirement, not a fabricated modal block");
 let before_focus=app.world().resource::<bevy::a11y::Focus>().0;let before_choice=app.world().resource::<ChoiceFocus>().0;let before_menu=app.world().resource::<menu_documents::Menus>().keyboard_focus();let before_interaction=*app.world().get::<Interaction>(target).unwrap();
 request(&mut app,target,accesskit::Action::Focus);request(&mut app,target,accesskit::Action::Default);app.world_mut().run_schedule(PreUpdate);
 assert_eq!(app.world().resource::<bevy::a11y::Focus>().0,before_focus,"An assistive story Focus must not mutate the retained display");assert_eq!(app.world().resource::<ChoiceFocus>().0,before_choice);assert_eq!(app.world().resource::<menu_documents::Menus>().keyboard_focus(),before_menu);assert_eq!(*app.world().get::<Interaction>(target).unwrap(),before_interaction,"An assistive story Default must not press the retained title control");
 actual_accessibility_key(&mut app,window,KeyCode::F8,bevy::input::keyboard::Key::F8);app.update();assert!(app.world().resource::<accessibility::Accessibility>().open,"The real host accessibility panel remains operable during retirement");
}

// Append within normal_runtime_startup_tests. Actual Graph header + legacy label.
#[test]
fn normal_runtime_cutover_legacy_empty_init_installs_first_projection_then_restarts_asynchronously(){
 let root=native_project("legacy-empty-entry","init {while false {set unreachable=1}}\n",NodeKind::While);
 let script=fs::read_to_string(root.join("main.rvn")).unwrap();assert_eq!(script.matches("scene label start").count(),1);fs::write(root.join("main.rvn"),script.replace("scene label start","label start")).unwrap();
 let(_,mut app)=runtime_app_project(root);pump_runtime(&mut app);let game=app.world().resource::<VnEngine>();let old=game.0.instance_id();let epoch=game.0.runtime_epoch();assert!(!game.0.state.vars.contains_key("narrative_started"));
 let first=app.world().resource::<scene_publication::ScenePresentation>().batch.as_ref().expect("A genuine legacy title must already own its new installed projection");assert_eq!(first.runtime_epoch,epoch);assert!(first.display.is_none());assert!(!first.state.vars.contains_key("unreachable"));assert!(!first.state.vars.contains_key("narrative_started"));
 menu_action(&mut app,rvn_ui::Action::NewGame).unwrap();let window=app.world_mut().query_filtered::<Entity,With<bevy::window::PrimaryWindow>>().single(app.world());app.world_mut().get_mut::<Window>(window).unwrap().focused=false;app.update();
 assert!(!app.world().contains_resource::<VnEngine>());assert_eq!(*app.world().resource::<State<VnState>>().get(),VnState::TitleScreen);assert_eq!(app.world().resource::<scene_publication::ScenePresentation>().batch.as_ref().unwrap().runtime_epoch,epoch);
 app.world_mut().get_mut::<Window>(window).unwrap().focused=true;await_restarted(&mut app,old);assert_eq!(app.world().resource::<VnEngine>().0.runtime_epoch(),epoch+1);assert_eq!(app.world().resource::<scene_publication::ScenePresentation>().batch.as_ref().unwrap().runtime_epoch,epoch+1);
}

// Proposed append-only CPU test, not linked/compiled/executed yet.
// Real normal registrar, Native staging and decoded assets; End is explicit
// queue input from this fixture, never evidence of physical EOF/audio output.
#[cfg(all(feature="video",not(target_arch="wasm32")))]
#[test]
fn normal_runtime_real_media_wait_callback_title_and_async_menu_newgame(){
 let root=native_project("normal-native-waits","init {set callbacks=0 set seen=0 for item in [1,2] {video.play(\"intro\",video_clip(\"clip.webm\",{\"on_end\":\"normal_end\"})) video.wait(\"intro\") set seen=seen+item}}\n",NodeKind::ForEach);
 let main=fs::read_to_string(root.join("main.rvn")).unwrap();fs::write(root.join("main.rvn"),format!("handler normal_end(event) {{set callbacks=callbacks+1 set callback_draw=random(1,9)}}\n{main}")).unwrap();
 let clip=PathBuf::from(std::env::var("RVN_NATIVE_STARTUP_FIXTURES").unwrap()).join("clip.webm");fs::copy(clip,root.join("assets/clip.webm")).unwrap();
 let(_,mut app)=runtime_app_with_hooks(root,|_|{},|app|native_video::init_restore_runtime_for_test(app));
 fn drive(app:&mut App,initial:bool)->u64{
  let deadline=std::time::Instant::now()+std::time::Duration::from_secs(40);let mut ended=0;
  loop{app.update();
   if let Some(startup)=app.world().get_resource::<debugger_startup::DebugBootstrap>(){
    assert!(!app.world().contains_resource::<VnEngine>());assert!(startup.failed.is_none(),"{:?}",startup.failed);assert_eq!(startup.profile,if initial{debugger_startup::BootstrapProfile::RuntimeInitial}else{debugger_startup::BootstrapProfile::RuntimeRestart});
    if startup.owner.waiting_for_video()&&!startup.publication_pending{
     let view=startup.owner.video_views().unwrap().into_iter().find(|view|view.id=="intro").unwrap();
     if view.track.playback==rvn_core::video::Playback::Playing&&startup.owner.state().vars.get("callbacks")==Some(&rvn_parser::Value::Int(ended)){
      assert!(ended<2,"An iteration must not recreate an extra terminal callback");
      let instance=startup.owner.snapshot().instance;let gate=startup.broker.gate();let before=startup.owner.snapshot();let rng=startup.owner.state().random;
      let rows=native_video::restored_sources_for_test(app.world());assert_eq!(rows.len(),1);assert_eq!(rows[0].0,view);assert_eq!((rows[0].2,rows[0].3),(320,180));
      let audio=rows[0].1.as_ref().expect("The real staged WebM carries decoded PCM");assert!(audio.0.lock().unwrap().verifies_owner(instance,&gate));
      let window=app.world_mut().query_filtered::<Entity,With<bevy::window::PrimaryWindow>>().single(app.world());app.world_mut().get_mut::<Window>(window).unwrap().focused=false;
      for _ in 0..2{app.update();let actual=app.world().resource::<debugger_startup::DebugBootstrap>();assert_eq!(actual.owner.snapshot(),before);assert_eq!(actual.owner.state().random,rng);assert!(!app.world().contains_resource::<VnEngine>());}
      app.world_mut().get_mut::<Window>(window).unwrap().focused=true;
      assert!(scene_publication::queue_video_feedback(app.world_mut(),&view,rvn_core::video::Feedback::End));assert!(scene_publication::queue_video_feedback(app.world_mut(),&view,rvn_core::video::Feedback::End));ended+=1;
     }
    }
   }else{
    let engine=app.world().resource::<VnEngine>();assert_eq!(ended,2);assert_eq!(engine.0.state.vars["callbacks"],rvn_parser::Value::Int(2));assert_eq!(engine.0.state.vars["seen"],rvn_parser::Value::Int(3));assert!(!editor_debugger::enabled(engine));
    if initial{assert_eq!(app.world().resource::<State<VnState>>().get(),&VnState::TitleScreen);assert!(!engine.0.state.vars.contains_key("narrative_started"));}
    return engine.0.instance_id();
   }
   assert!(std::time::Instant::now()<deadline,"Real normal media wait/callback/handoff timed out");std::thread::sleep(std::time::Duration::from_millis(2));
  }
 }
 let old=drive(&mut app,true);menu_action(&mut app,rvn_ui::Action::NewGame).unwrap();assert_eq!(app.world().resource::<VnEngine>().0.instance_id(),old);let fresh=drive(&mut app,false);assert_ne!(fresh,old);
}

// Append to normal_runtime_startup_tests; real media bridge across two restarts.
#[cfg(all(feature="video",not(target_arch="wasm32")))]
#[test]
fn normal_runtime_cutover_two_restarts_retire_actual_old_native_sources_without_asset_clears(){
 use bevy::ecs::system::RunSystemOnce;
 fn confirm_restart(mut commands:Commands,mut menus:ResMut<menu_documents::Menus>,mut context:menu_documents::ActionContext,mut result:ResMut<ActionResult>){
  result.0=Some(menu_documents::source_confirmation(&mut commands,&mut menus,&mut context,true));
 }
 let root=native_project("old-native-cutover","init {for item in [1] {video.play(\"intro\",video_clip(\"clip.webm\",{})) set seen=item}}\n",NodeKind::ForEach);
 fs::copy(PathBuf::from(std::env::var("RVN_NATIVE_STARTUP_FIXTURES").unwrap()).join("clip.webm"),root.join("assets/clip.webm")).unwrap();
 let(_,mut app)=runtime_app_with_hooks(root,|_|{},|app|native_video::init_restore_runtime_for_test(app));pump_runtime(&mut app);
 fn facts(source:&video_audio::VideoAudio)->(Option<u64>,u64,bool,bool,bool,usize){let buffer=source.0.lock().unwrap();(buffer.delivered_sample_offset(),buffer.position.to_bits(),buffer.running,buffer.ended,buffer.revoked,buffer.len())}
 let mut instances=vec![app.world().resource::<VnEngine>().0.instance_id()];let mut confirmations=Vec::new();
 for _round in 0..2{
  let old=*instances.last().unwrap();let gate=app.world().resource::<VnEngine>().0.renderer.audio_checkpoints.gate();
  let rows=native_video::restored_sources_for_test(app.world());assert_eq!(rows.len(),1);assert_eq!(rows[0].0.id,"intro");assert_eq!(rows[0].0.track.playback,rvn_core::video::Playback::Playing);
  let old_source=rows[0].1.as_ref().expect("Positive control must retain the real old decoded PCM source").clone();assert!(old_source.0.lock().unwrap().verifies_owner(old,&gate));assert!(!old_source.0.lock().unwrap().revoked);
  assert!(app.world().resource::<Assets<video_audio::VideoAudio>>().iter().any(|(_,actual)|std::sync::Arc::ptr_eq(&actual.0,&old_source.0)),"The old source must really be resident, not a DTO or manually cleared cache");
  let before=facts(&old_source);menu_action(&mut app,rvn_ui::Action::NewGame).unwrap();
  let confirmation_requested=app.world().resource::<systems::save_menu::SaveConfirmation>().action_pending.is_some();confirmations.push(confirmation_requested);
  if confirmation_requested{
   let confirmation=app.world().resource::<systems::save_menu::SaveConfirmation>();
   assert!(matches!(confirmation.action_pending.as_ref(),Some((rvn_ui::Action::NewGame,_,_))));assert!(confirmation.pending.is_none());
   assert_eq!(app.world().resource::<VnEngine>().0.instance_id(),old);assert_eq!(facts(&old_source),before,"Asking for confirmation must retain the actual outgoing owner and PCM source");
   app.world_mut().run_system_once(confirm_restart);app.world_mut().resource_mut::<ActionResult>().0.take().expect("The actual confirmation handler must run").unwrap();
   assert!(app.world().resource::<systems::save_menu::SaveConfirmation>().action_pending.is_none());
  }
  assert_eq!(facts(&old_source),before,"Admitting and preparing the incoming owner cannot mutate old media authority, cursor or pause");
  let window=app.world_mut().query_filtered::<Entity,With<bevy::window::PrimaryWindow>>().single(app.world());app.world_mut().get_mut::<Window>(window).unwrap().focused=false;app.update();
  let held=facts(&old_source);assert!(!held.4);assert_eq!(held.0,before.0);assert_eq!(held.1,before.1);assert!(app.world().resource::<debugger_startup::DebugBootstrap>().broker.gate().same_gate(&gate));
  for _ in 0..3{app.update();assert_eq!(facts(&old_source),held,"Only the real focus suspension may alter pause before publication; the retained source remains exact afterward");assert!(!app.world().contains_resource::<VnEngine>());}
  app.world_mut().get_mut::<Window>(window).unwrap().focused=true;await_restarted(&mut app,old);let incoming=app.world().resource::<VnEngine>().0.instance_id();assert!(!instances.contains(&incoming));instances.push(incoming);
  let retired=facts(&old_source);assert!(retired.4&&retired.3&&!retired.2,"The actual outgoing source must be revoked/ended/paused by the accepted publication fence");
  // Real Main/PreUpdate AssetPlugin tracking performs cleanup. No manual
  // Assets::remove, clear_current_for_cold_test or replaced Players resource.
  for _ in 0..3{app.update();}
  assert!(!app.world().resource::<Assets<video_audio::VideoAudio>>().iter().any(|(_,actual)|std::sync::Arc::ptr_eq(&actual.0,&old_source.0)),"The retired asset must leave Assets through actual handle tracking before the next restart");
  let current=native_video::restored_sources_for_test(app.world());assert_eq!(current.len(),1);let source=current[0].1.as_ref().unwrap();assert!(source.0.lock().unwrap().verifies_owner(incoming,&gate));assert!(!std::sync::Arc::ptr_eq(&source.0,&old_source.0));
 }
 assert_eq!(instances.len(),3);assert_eq!(confirmations,vec![false,true],"Exercise the initial title action and the actual in-game confirmation on the second restart");
}

// A test-only thread reproduces the actual Windows player stack budget. It does
// not move the production App/Winit runner or add a production scheduler.
#[test]
fn normal_runtime_native_init_loader_keeps_real_owner_on_one_mib_stack_0922(){
 let root=native_project("heap-native-one-mib","init {set seen=0 while seen<3 {set seen=seen+1}}\n",NodeKind::While);
 let source=fs::read_to_string(root.join("main.rvn")).unwrap();assert!(root.join("assets").is_dir());
 std::thread::Builder::new().name("runtime-loader-one-mib".into()).stack_size(1024*1024).spawn(move||{
  let launch=load_runtime_game(&root).expect("The actual native loader must fit the player's unchanged stack budget");
  let RuntimeLaunchOwner::Startup(startup)=launch.engine else{panic!("Native Init must retain its actual Startup owner");};
  assert_eq!(startup.profile,debugger_startup::BootstrapProfile::RuntimeInitial);
  assert!(!startup.owner.is_complete());assert!(!startup.owner.state().vars.contains_key("seen"));
  assert!(startup.owner.snapshot().instance>0);assert!(startup.accepts_actual_audio_broker());
  assert!(startup.owner.program().sources.iter().any(|input|input.source==source),"The retained owner must bind the exact real frozen source");
 }).unwrap().join().unwrap();
}
#[test]
fn normal_runtime_preview_story_builder_keeps_inline_init_on_one_mib_stack_0922(){
 let root=native_project("heap-preview-one-mib","init {set seen=0 while seen<3 {set seen=seen+1}}\n",NodeKind::While);
 // This is the same Legacy-compatible three-scene family as the native failure.
 // A libtest cannot rewrite argv. Only the parsed preview selector is supplied
 // here; the loader and owner factory are the actual production functions.
 fs::write(root.join("main.rvn"),"init {set seen=0}\nscene label start\n\"one\"\ncall second\n\"returned\"\nscene label second\n\"two\"\ncall third\nreturn\nscene label third\n\"three\"\nreturn\n").unwrap();
 assert!(root.join("assets").is_dir());
 std::thread::Builder::new().name("preview-loader-one-mib".into()).stack_size(1024*1024).spawn(move||{
  let launch=load_game_with_owner(&root,false,|program,cfg,label,locale,_,persistent,tables,classes,enums,libraries|
   build_runtime_launch_owner(program,cfg,label,locale,true,persistent,tables,classes,enums,libraries))
   .expect("The actual preview-selected Game builder must fit the player's unchanged stack budget");
  let RuntimeLaunchOwner::Game(engine)=launch.engine else{panic!("Inline legacy Init must keep its ordinary Game owner");};
  assert!(engine.state.scene_execution.is_some());assert_eq!(engine.state.vars.get("seen"),Some(&rvn_parser::Value::Int(0)));
  assert!(engine.instance_id()>0);assert!(engine.debug_site_page(0).is_ok(),"The actual preview builder must retain its debugger source provenance");
 }).unwrap().join().unwrap();
}
