//! Uses the actual normal .rvn loader/Graph fixture and complete CPU runtime.
//! No debug nonce, fabricated owner, sidecar or dummy locale file.
use super::*;
fn absence_root(name:&str,mode:u8,raw:&str)->PathBuf{
    let root=native_project(name,raw,NodeKind::While);
    if mode==2{fs::write(root.join("assets/config.toml"),"[locale]\ndefault='fr'\ncurrent='en'\navailable=['fr','en']\n").unwrap();fs::write(root.join("locales/fr.toml"),"[strings]\n\"Unexecuted before Start\" = \"Traduit par défaut\"\n").unwrap();}
    else{fs::remove_file(root.join("locales/fr.toml")).unwrap();if mode==0{fs::remove_dir(root.join("locales")).unwrap();}}
    root
}
fn actual_absence_handoff(mode:u8){
    let root=absence_root(&format!("absence-handoff-{mode}"),mode,"init {set seen=0 while seen<3 {set seen=seen+1}}\n");
    let(root,mut app)=runtime_app_project(root);let initial=app.world().resource::<debugger_startup::DebugBootstrap>().owner.snapshot();pump_runtime(&mut app);
    assert!(!app.world().contains_resource::<debugger_startup::DebugBootstrap>());let engine=app.world().resource::<VnEngine>();
    assert_eq!(engine.0.instance_id(),initial.instance);assert_eq!(engine.0.state.vars["seen"],rvn_parser::Value::Int(3));assert!(!engine.0.state.vars.contains_key("narrative_started"));
    let receipt=portable_text_resources::portable_receipt(app.world()).unwrap();assert_eq!(receipt.locale_loader_policy,2);
    if mode==2{assert_eq!(receipt.locale_absent,std::collections::BTreeSet::from(["en".into()]));assert_eq!(engine.0.locale.as_ref().unwrap().translate("Unexecuted before Start"),"Traduit par défaut");}
    else{assert!(receipt.locale_absent.contains("fr"));assert!(!root.join("locales/fr.toml").exists());if mode==0{assert!(!root.join("locales").exists());}}
    assert!(!root.join(rvn_core::debugger_provenance::SIDECAR).exists());
}
#[test]fn normal_absence0664_missing_root_closes_actual_assetserver_and_hands_off_once(){actual_absence_handoff(0);}
#[test]fn normal_absence0664_missing_default_closes_actual_assetserver_and_hands_off_once(){actual_absence_handoff(1);}
#[test]fn normal_absence0664_missing_current_preserves_actual_default_fallback_through_handoff(){actual_absence_handoff(2);}
#[test]fn normal_absence0664_real_deletion_after_preload_before_first_compute_is_valid_absence(){
    let root=native_project("absence-after-preload","init {set seen=0 while seen<3 {set seen=seen+1}}\n",NodeKind::While);
    let(root,mut app)=runtime_app_after_admission(root,|root|fs::remove_file(root.join("locales/fr.toml")).unwrap());pump_runtime(&mut app);
    assert_eq!(app.world().resource::<VnEngine>().0.state.vars["seen"],rvn_parser::Value::Int(3));assert!(portable_text_resources::portable_receipt(app.world()).unwrap().locale_absent.contains("fr"));assert!(!root.join("locales/fr.toml").exists());
}
fn paused_absence(name:&str,raw:&str)->(PathBuf,App){
    let root=absence_root(name,1,raw);let(root,mut app)=runtime_app_project(root);
    let window=app.world_mut().query_filtered::<Entity,With<bevy::window::PrimaryWindow>>().single(app.world());app.world_mut().get_mut::<Window>(window).unwrap().focused=false;
    let initial=app.world().resource::<debugger_startup::DebugBootstrap>().owner.snapshot();let deadline=std::time::Instant::now()+std::time::Duration::from_secs(20);
    loop{app.update();assert_eq!(app.world().resource::<debugger_startup::DebugBootstrap>().owner.snapshot(),initial);if portable_text_resources::startup_loads_ready(app.world()).unwrap(){break;}assert!(std::time::Instant::now()<deadline);std::thread::sleep(std::time::Duration::from_millis(2));}
    // Loading is parked without focus. Restore the actual prerequisite
    // before either direct prepare; do not run an App frame that computes Init.
    app.world_mut().get_mut::<Window>(window).unwrap().focused=true;
    assert_eq!(app.world().resource::<debugger_startup::DebugBootstrap>().owner.snapshot(),initial);
    (root,app)
}
#[test]fn normal_absence0664_real_file_appearance_is_refused_before_candidate_rng_and_division(){
    let(root,app)=paused_absence("absence-precompute","init {set crash=random(1,9)/0 while false {}}\n");let before=app.world().resource::<debugger_startup::DebugBootstrap>().owner.snapshot();
    fs::write(root.join("locales/fr.toml"),"[strings]\n").unwrap();let error=debugger_startup::prepare_world_step(app.world(),rvn_core::editor_debugger::StepKind::Continue).err().unwrap();
    assert!(error.contains("appeared"),"Fresh host refusal must precede Core division: {error}");assert_eq!(app.world().resource::<debugger_startup::DebugBootstrap>().owner.snapshot(),before);assert!(!app.world().contains_resource::<VnEngine>());
}
#[test]fn normal_absence0664_real_file_appearance_during_prepared_computation_refuses_refresh(){
    let(root,app)=paused_absence("absence-stage","init {set seen=random(1,9) while false {}}\n");let before=app.world().resource::<debugger_startup::DebugBootstrap>().owner.snapshot();
    let mut prepared=debugger_startup::prepare_world_step(app.world(),rvn_core::editor_debugger::StepKind::Continue).unwrap();
    fs::write(root.join("locales/fr.toml"),"[strings]\n").unwrap();assert!(debugger_startup::refresh_world_step(app.world(),&mut prepared).is_err());
    assert_eq!(app.world().resource::<debugger_startup::DebugBootstrap>().owner.snapshot(),before);assert!(!app.world().contains_resource::<VnEngine>());
}

// Four real normal loader -> App/AssetServer -> Init -> Title handoffs.
// U remains the configured/discovered UI list. C closes default/current only.
fn outside_ui_handoff0673(case:u8){
    let root=native_project(&format!("outside-ui-handoff-{case}"),"init {set seen=0 while seen<3 {set seen=seen+1}}\n",NodeKind::While);
    let config=match case{
        0=>"[locale]\ndefault='fr'\ncurrent='zz'\navailable=['fr']\n",
        1=>"[locale]\ndefault='zz'\ncurrent='fr'\navailable=['fr']\n",
        2=>"[locale]\ndefault='fr'\ncurrent='fr'\navailable=['fr']\n",
        3=>"[locale]\ndefault='fr'\ncurrent='zz'\navailable=[]\n",
        _=>unreachable!(),
    };fs::write(root.join("assets/config.toml"),config).unwrap();
    if case==3{fs::remove_file(root.join("locales/fr.toml")).unwrap();}
    else{fs::write(root.join("locales/fr.toml"),"[strings]\n\"Unexecuted before Start\" = \"Traduction française\"\n").unwrap();}
    let manager=PersistentDataManager::new(root.join("saves")).unwrap();
    if case==2{let mut persistent=manager.load().unwrap();persistent.language=Some("zz".into());manager.save(&persistent).unwrap();}
    let saved=fs::read(manager.path()).unwrap();let(root,mut app)=runtime_app_project(root);
    let initial=app.world().resource::<debugger_startup::DebugBootstrap>().owner.snapshot();pump_runtime(&mut app);
    assert!(!app.world().contains_resource::<debugger_startup::DebugBootstrap>());let engine=&app.world().resource::<VnEngine>().0;
    assert_eq!(engine.instance_id(),initial.instance);assert_eq!(engine.runtime_epoch(),initial.epoch);assert_eq!(engine.state.vars["seen"],rvn_parser::Value::Int(3));assert!(!engine.state.vars.contains_key("narrative_started"));
    let receipt=portable_text_resources::portable_receipt(app.world()).unwrap();let expected_current=if case==1{"fr"}else{"zz"};let expected_default=if case==1{"zz"}else{"fr"};
    assert_eq!(engine.locale.as_ref().unwrap().current_lang(),expected_current);assert_eq!(receipt.locales.current_lang,expected_current);assert_eq!(receipt.locales.default_lang,expected_default);
    assert_eq!(engine.locale.as_ref().unwrap().translate("Unexecuted before Start"),if case==3{"Unexecuted before Start"}else{"Traduction française"});
    assert_eq!(app.world().resource::<Settings>().language,expected_current);
    assert_eq!(app.world().resource::<resources::LocaleConfig>().available_langs,if case==3{Vec::<String>::new()}else{vec!["fr".to_owned()]});
    assert_eq!(receipt.locale_absent,if case==3{["fr".to_owned(),"zz".to_owned()].into_iter().collect::<std::collections::BTreeSet<_>>()}else{["zz".to_owned()].into_iter().collect::<std::collections::BTreeSet<_>>()});
    assert_eq!(receipt.locales.cache.keys().cloned().collect::<std::collections::BTreeSet<_>>(),["fr".to_owned(),"zz".to_owned()].into_iter().collect::<std::collections::BTreeSet<_>>());
    assert_eq!(portable_text_resources::locale_selection_mode(app.world()).unwrap(),portable_text_resources::LocaleSelectionMode::NormalCompatible);
    assert_eq!(fs::read(manager.path()).unwrap(),saved);if case==2{assert_eq!(app.world().resource::<resources::PersistentDataResource>().data.language.as_deref(),Some("zz"));}
    assert!(!root.join("locales/zz.toml").exists());assert!(!root.join(rvn_core::debugger_provenance::SIDECAR).exists());
    for _ in 0..3{app.update();assert!(!app.world().contains_resource::<debugger_startup::DebugBootstrap>());assert_eq!(app.world().resource::<VnEngine>().0.instance_id(),initial.instance);assert!(!app.world().resource::<VnEngine>().0.state.vars.contains_key("narrative_started"));}
}
#[test]fn normal_loaded0673_current_outside_ui_hands_off_through_actual_assetserver(){outside_ui_handoff0673(0);}
#[test]fn normal_loaded0673_default_outside_ui_hands_off_through_actual_assetserver(){outside_ui_handoff0673(1);}
#[test]fn normal_loaded0673_persisted_outside_ui_hands_off_without_rewriting_preference(){outside_ui_handoff0673(2);}
#[test]fn normal_loaded0673_empty_ui_hands_off_with_real_absent_default_and_current(){outside_ui_handoff0673(3);}

const VALID_B0682:&[u8]=b"[strings]\n\"Unexecuted before Start\"=\"Observed valid B\"\n";
fn terminal_project0682(name:&str,language:Option<&str>)->(PathBuf,String){
    let root=native_project(name,"init {set seen=0 while seen<3 {set seen=seen+1}}\n",NodeKind::While);
    let language=language.map(str::to_owned).unwrap_or_else(||format!("terminal_{}_{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    fs::remove_file(root.join("locales/fr.toml")).unwrap();fs::write(root.join("assets/config.toml"),format!("[locale]\ndefault='{language}'\ncurrent='{language}'\navailable=['{language}']\n")).unwrap();
    fs::write(root.join(format!("locales/{language}.toml")),"[strings]\n").unwrap();(root,language)
}
fn primary0682(app:&mut App)->Entity{app.world_mut().query_filtered::<Entity,With<bevy::window::PrimaryWindow>>().single(app.world())}
fn locale_handle0682(app:&App,language:&str)->Handle<portable_text_resources::LoadedLocaleSource>{
    app.world().resource::<AssetServer>().get_handle(bevy::asset::AssetPath::from(format!("{language}.toml")).with_source("rvn-closed-locales-v1")).unwrap()
}
fn wait_refusal0682(app:&mut App,language:&str,initial:&rvn_core::editor_debugger::Snapshot)->String{
    let end=std::time::Instant::now()+std::time::Duration::from_secs(20);
    loop{app.update();let startup=app.world().resource::<debugger_startup::DebugBootstrap>();assert_eq!(&startup.owner.snapshot(),initial);assert!(startup.failed.is_none(),"Focus is still absent: driver must not have run yet");assert!(!app.world().contains_resource::<VnEngine>());
        if let Err(error)=portable_text_resources::startup_loads_ready(app.world()){
            assert!(error.contains("locale source changed after admission"),"Wrong terminal refusal: {error}");
            let handle=locale_handle0682(app,language);assert!(matches!(app.world().resource::<AssetServer>().load_state(handle.id()),bevy::asset::LoadState::Loaded));
            assert_eq!(app.world().resource::<Assets<portable_text_resources::LoadedLocaleSource>>().get(handle.id()).unwrap().0.as_bytes(),VALID_B0682,"This must be a valid actual B load, not a Failed or synthetic source");return error;
        }
        assert!(std::time::Instant::now()<end,"Valid changed B remained an infinite Loading instead of a terminal frozen-catalogue refusal");std::thread::sleep(std::time::Duration::from_millis(2));
    }
}
#[test]
fn normal_terminal0682_real_loading_a_then_valid_b_terminates_without_running_init(){
    let(root,language)=terminal_project0682("valid-b-initial",None);let path=PathBuf::from(format!("{language}.toml"));
    let gate=portable_text_resources::native_absence_tests::hold_normal_locale_decode(path.clone(),VALID_B0682);
    let(root,mut app)=runtime_app_project(root);let window=primary0682(&mut app);app.world_mut().get_mut::<Window>(window).unwrap().focused=false;
    let initial=app.world().resource::<debugger_startup::DebugBootstrap>().owner.snapshot();let end=std::time::Instant::now()+std::time::Duration::from_secs(20);
    while !gate.entered(){app.update();assert_eq!(app.world().resource::<debugger_startup::DebugBootstrap>().owner.snapshot(),initial);assert!(std::time::Instant::now()<end);std::thread::sleep(std::time::Duration::from_millis(2));}
    let handle=locale_handle0682(&app,&language);assert!(matches!(app.world().resource::<AssetServer>().load_state(handle.id()),bevy::asset::LoadState::Loading));
    assert!(!portable_text_resources::startup_loads_ready(app.world()).unwrap(),"A real in-flight load must remain transient, not a terminal diagnostic");
    fs::write(root.join("locales").join(&path),VALID_B0682).unwrap();
    while !gate.observed(){app.update();assert_eq!(app.world().resource::<debugger_startup::DebugBootstrap>().owner.snapshot(),initial);assert!(std::time::Instant::now()<end);std::thread::sleep(std::time::Duration::from_millis(2));}
    assert!(matches!(app.world().resource::<AssetServer>().load_state(handle.id()),bevy::asset::LoadState::Loading));gate.release();
    let refusal=wait_refusal0682(&mut app,&language,&initial);assert!(portable_text_resources::live_receipt(app.world()).is_err());
    app.world_mut().get_mut::<Window>(window).unwrap().focused=true;let failure=await_failed(&mut app);assert_eq!(failure,refusal);
    let startup=app.world().resource::<debugger_startup::DebugBootstrap>();assert_eq!(startup.owner.snapshot(),initial);assert!(!startup.owner.state().vars.contains_key("seen"));assert!(!startup.owner.state().vars.contains_key("narrative_started"));assert!(!app.world().contains_resource::<VnEngine>());
}

#[derive(Clone,Debug,PartialEq)]
struct PlayerBoundary0682{instance:u64,epoch:u64,state:serde_json::Value,locale:rvn_core::locale::LocaleContentSnapshot,history:Vec<(String,String)>,batch:serde_json::Value,title:Vec<Entity>}
fn player_boundary0682(world:&World)->PlayerBoundary0682{
    let game=&world.resource::<VnEngine>().0;let mut title=world.iter_entities().filter(|entity|entity.contains::<systems::title::TitleOverlay>()).map(|entity|entity.id()).collect::<Vec<_>>();title.sort_by_key(|entity|entity.to_bits());
    PlayerBoundary0682{instance:game.instance_id(),epoch:game.runtime_epoch(),state:serde_json::to_value(&game.state).unwrap(),locale:game.locale.as_ref().unwrap().portable_receipt().unwrap(),
        history:world.resource::<DialogueHistory>().lines.iter().map(|line|(line.character.clone(),line.text.clone())).collect(),batch:serde_json::to_value(&world.resource::<scene_publication::ScenePresentation>().batch.as_ref().unwrap().state).unwrap(),title}
}
fn assert_same_continuation0682(actual:&PlayerBoundary0682,before:&PlayerBoundary0682){
    assert_eq!(actual.instance,before.instance);assert_eq!(actual.epoch,before.epoch);assert_eq!(actual.state,before.state);assert_eq!(actual.history,before.history);assert_eq!(actual.batch,before.batch);assert_eq!(actual.title,before.title);
}
#[derive(Resource)]
struct RestoreBoundary0682{expected_instance:u64,actual:Option<PlayerBoundary0682>}
fn witness_actual_restore0682(world:&mut World){
    if world.resource::<RestoreBoundary0682>().actual.is_some()||world.contains_resource::<debugger_startup::DebugBootstrap>()||world.get_resource::<VnEngine>().is_none_or(|game|game.0.instance_id()!=world.resource::<RestoreBoundary0682>().expected_instance){return;}
    let actual=player_boundary0682(world);world.resource_mut::<RestoreBoundary0682>().actual=Some(actual);
}
fn actual_hotreload_without_restart0682(language:&str)->rvn_core::locale::LocaleContentSnapshot{
    let(root,_)=terminal_project0682("valid-b-control-no-restart",Some(language));let(root,mut app)=runtime_app_project(root);pump_runtime(&mut app);let before=player_boundary0682(app.world());
    let publication=portable_text_resources::live_receipt(app.world()).unwrap().locale_publication;let handle=locale_handle0682(&app,language);
    fs::write(root.join(format!("locales/{language}.toml")),VALID_B0682).unwrap();app.world().resource::<AssetServer>().reload(bevy::asset::AssetPath::from(format!("{language}.toml")).with_source("rvn-closed-locales-v1"));
    let end=std::time::Instant::now()+std::time::Duration::from_secs(20);
    loop{app.update();assert!(!app.world().contains_resource::<debugger_startup::DebugBootstrap>());let actual=player_boundary0682(app.world());assert_same_continuation0682(&actual,&before);
        if let Ok(live)=portable_text_resources::live_receipt(app.world()){if live.portable.locales.current.strings.get("Unexecuted before Start").is_some_and(|value|value=="Observed valid B"){
            assert!(live.locale_publication>publication);assert_eq!(locale_handle0682(&app,language).id(),handle.id());assert!(matches!(app.world().resource::<AssetServer>().load_state(handle.id()),bevy::asset::LoadState::Loaded));return actual.locale;
        }}assert!(std::time::Instant::now()<end,"Positive control: genuine Game hotreload did not publish B");std::thread::sleep(std::time::Duration::from_millis(2));
    }
}
#[test]
fn normal_terminal0682_real_restart_restores_a_before_update_then_only_normal_game_provider_publishes_b(){
    let(root,language)=terminal_project0682("valid-b-restart",None);let(root,mut app)=runtime_app_project(root);pump_runtime(&mut app);
    app.world_mut().resource_mut::<DialogueHistory>().add("old".into(),"retained exact history".into());let before=player_boundary0682(app.world());assert!(!before.title.is_empty());
    let old_publication=portable_text_resources::live_receipt(app.world()).unwrap().locale_publication;let handle=locale_handle0682(&app,&language);
    let control_b=actual_hotreload_without_restart0682(&language);assert_ne!(control_b,before.locale);
    menu_action(&mut app,rvn_ui::Action::NewGame).unwrap();let window=primary0682(&mut app);app.world_mut().get_mut::<Window>(window).unwrap().focused=false;app.update();
    assert!(!app.world().contains_resource::<VnEngine>());let incoming=app.world().resource::<debugger_startup::DebugBootstrap>().owner.snapshot();assert_ne!(incoming.instance,before.instance);
    fs::write(root.join(format!("locales/{language}.toml")),VALID_B0682).unwrap();app.world().resource::<AssetServer>().reload(bevy::asset::AssetPath::from(format!("{language}.toml")).with_source("rvn-closed-locales-v1"));
    let refusal=wait_refusal0682(&mut app,&language,&incoming);
    app.insert_resource(RestoreBoundary0682{expected_instance:before.instance,actual:None});
    app.add_systems(First,witness_actual_restore0682.after(debugger_startup::driver::drain_runtime).before(scene_publication::consume));
    app.world_mut().get_mut::<Window>(window).unwrap().focused=true;app.update();
    let restored=app.world().resource::<RestoreBoundary0682>().actual.as_ref().expect("Observe the actual restored owner in First, before normal provider Update");assert_eq!(restored,&before,"Atomic restoration includes exact catalogue A, same old Core/RNG/batch/history/entities");
    assert!(!app.world().contains_resource::<debugger_startup::DebugBootstrap>());assert!(app.world().resource::<ScriptErrorMessage>().0.contains(refusal.as_str()));
    let end=std::time::Instant::now()+std::time::Duration::from_secs(20);
    loop{let actual=player_boundary0682(app.world());assert_same_continuation0682(&actual,&before);assert!(!app.world().contains_resource::<debugger_startup::DebugBootstrap>());
        if let Ok(live)=portable_text_resources::live_receipt(app.world()){if live.portable.locales==control_b{
            assert!(live.locale_publication>old_publication);assert_eq!(locale_handle0682(&app,&language).id(),handle.id());assert!(matches!(app.world().resource::<AssetServer>().load_state(handle.id()),bevy::asset::LoadState::Loaded));assert_eq!(actual.locale,control_b);break;
        }}assert!(std::time::Instant::now()<end,"Restored Game must preserve its existing real hotreload route, proven by no-restart positive control");app.update();std::thread::sleep(std::time::Duration::from_millis(2));
    }
}
