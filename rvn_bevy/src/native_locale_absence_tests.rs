//! Real AssetServer CPU tests; not native window/pixel qualification.
use super::*;
use crate::{bevy_renderer::BevyRenderer,string_table_assets::StringTableAssetsPlugin};
use std::{fs,path::{Path,PathBuf},time::{Duration,Instant,SystemTime,UNIX_EPOCH}};
use std::sync::atomic::{AtomicU64,Ordering};
static NEXT:AtomicU64=AtomicU64::new(0);
struct Fixture{root:PathBuf,app:App}
fn declarations()->LocaleDeclarations{
    LocaleDeclarations{default_lang:"fr".into(),current_lang:"en".into(),available:vec!["fr".into(),"en".into()],files:BTreeMap::from([("fr".into(),"fr.toml".into()),("en".into(),"en.toml".into())])}
}
fn fixture(directory:bool,present:bool)->Fixture{fixture_with_hook(directory,present,|_|{})}
fn fixture_with_hook(directory:bool,present:bool,hook:impl FnOnce(&mut App))->Fixture{fixture_custom(directory,present,declarations(),hook)}
fn fixture_custom(directory:bool,present:bool,declarations:LocaleDeclarations,hook:impl FnOnce(&mut App))->Fixture{
    fixture_custom_with_policy(directory,present,declarations,bevy::asset::AssetMetaCheck::Always,hook)
}
fn fixture_custom_with_policy(directory:bool,present:bool,declarations:LocaleDeclarations,meta_check:bevy::asset::AssetMetaCheck,hook:impl FnOnce(&mut App))->Fixture{
    let root=std::env::temp_dir().join(format!("rvn-native-absence-0664-{}-{}-{}",std::process::id(),SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos(),NEXT.fetch_add(1,Ordering::Relaxed)));
    fs::create_dir_all(root.join("assets")).unwrap();
    if directory{fs::create_dir(root.join("locales")).unwrap();}
    if present{for lang in ["fr","en"]{fs::write(root.join(format!("locales/{lang}.toml")),"[strings]\n\"First\" = \"Translation\"\n").unwrap();}}
    let verified=closed_native_locale_root_for_launch(&root.join("locales"),false).unwrap();
    assert_eq!(verified.selection_mode(),LocaleSelectionMode::NormalCompatible);
    let locale=load_closed_native_frozen_locales(&verified,&declarations).unwrap();
    let mut engine=rvn_core::Engine::new_with_text_resources(rvn_parser::parse("label start\n\"First\"\n").unwrap(),BevyRenderer::new(),8,Default::default()).unwrap();engine.locale=Some(locale);
    let mut app=App::new();register_verified_text_sources(&mut app,root.join("assets").to_string_lossy().into_owned(),verified).unwrap();
    assert_eq!(locale_selection_mode(app.world()).unwrap(),LocaleSelectionMode::NormalCompatible);
    hook(&mut app);
    app.add_plugins(MinimalPlugins).add_plugins(AssetPlugin{file_path:root.join("assets").to_string_lossy().into_owned(),watch_for_changes_override:Some(false),meta_check,..default()})
        .insert_resource(VnEngine(engine)).add_event::<crate::vn_command::VnCommand>()
        .add_plugins(StringTableAssetsPlugin::new(BTreeMap::new())).add_plugins(ClosedLocaleAssetsPlugin::new(declarations).unwrap());
    app.update();Fixture{root,app}
}
impl Drop for Fixture{fn drop(&mut self){
    let Ok(root)=self.root.canonicalize()else{return;};let temp=std::env::temp_dir().canonicalize().unwrap();
    assert_eq!(root.parent(),Some(temp.as_path()));assert!(root.file_name().unwrap().to_string_lossy().starts_with(&format!("rvn-native-absence-0664-{}-",std::process::id())));
    let _=fs::remove_dir_all(root);
}}
fn pump(f:&mut Fixture,predicate:impl Fn(&World)->bool){
    let deadline=Instant::now()+Duration::from_secs(20);
    loop{f.app.update();if predicate(f.app.world()){return;}assert!(Instant::now()<deadline,"actual source/closure did not settle");std::thread::sleep(Duration::from_millis(5));}
}
fn ready(f:&mut Fixture)->PortableTextHostReceipt{pump(f,|world|portable_receipt(world).is_ok());portable_receipt(f.app.world()).unwrap()}
fn all_failed_not_found(world:&World)->bool{
    let host=world.resource::<LocaleAssetHost>();let server=world.resource::<AssetServer>();
    host.bound.len()==2&&host.bound.values().all(|handle|native_locale_provider::failed_not_found(&server.load_state(handle.id())))
}
#[test]
fn native_absence0664_missing_root_uses_real_failed_handles_without_creating_directory(){
    let mut f=fixture(false,false);let receipt=ready(&mut f);assert!(!f.root.join("locales").exists());
    assert!(all_failed_not_found(f.app.world()));assert_eq!(receipt.locale_absent,BTreeSet::from(["fr".into(),"en".into()]));assert_eq!(receipt.locale_loader_policy,2);
    assert!(f.app.world().resource::<Assets<LoadedLocaleSource>>().is_empty());assert!(startup_loads_ready(f.app.world()).unwrap());
}
#[test]
fn native_absence0664_existing_directory_missing_files_is_not_loaded_empty_data(){
    let mut f=fixture(true,false);let receipt=ready(&mut f);assert_eq!(receipt.locale_absent.len(),2);assert!(all_failed_not_found(f.app.world()));
    for snapshot in receipt.locales.cache.values(){assert!(snapshot.strings.is_empty());assert!(snapshot.text_sources.is_empty());}
}
#[test]
fn native_absence0664_real_default_fallback_with_missing_current_language(){
    let mut f=fixture(true,false);fs::write(f.root.join("locales/fr.toml"),"[strings]\n\"First\" = \"Premier\"\n").unwrap();
    let receipt=ready(&mut f);assert_eq!(receipt.locale_absent,BTreeSet::from(["en".into()]));
    let locale=f.app.world().resource::<VnEngine>().0.locale.as_ref().unwrap();assert_eq!(locale.current_lang(),"en");assert_eq!(locale.translate("First"),"Premier");
}
#[test]
fn native_absence0664_present_empty_and_absent_have_different_portable_contracts(){
    let mut absent=fixture(true,false);let a=ready(&mut absent);let mut present=fixture(true,false);
    for lang in ["fr","en"]{fs::write(present.root.join(format!("locales/{lang}.toml")),"[strings]\n").unwrap();}
    let b=ready(&mut present);assert_eq!(a.locales,b.locales);assert!(b.locale_absent.is_empty());assert_eq!(b.locale_loader_policy,1);assert_ne!(a.canonical_digest().unwrap(),b.canonical_digest().unwrap());
}
#[test]
fn native_absence0664_old_all_present_receipt_omits_new_field_and_roundtrips(){
    let mut f=fixture(true,true);let receipt=ready(&mut f);let bytes=serde_json::to_vec(&receipt).unwrap();assert!(!String::from_utf8_lossy(&bytes).contains("locale_absent"));
    let old:PortableTextHostReceipt=serde_json::from_slice(&bytes).unwrap();assert_eq!(old,receipt);assert_eq!(old.canonical_digest().unwrap(),receipt.canonical_digest().unwrap());
    let mut forged=receipt;forged.locale_loader_policy=2;assert!(forged.canonical_digest().is_err());
}
#[test]
fn native_absence0664_claimed_absence_cannot_contain_translation_content(){
    let mut f=fixture(true,true);let mut receipt=ready(&mut f);receipt.locale_loader_policy=2;receipt.locale_absent.insert("fr".into());assert!(receipt.canonical_digest().is_err());
}
#[test]
fn native_absence0664_forged_failure_event_never_turns_loaded_source_into_absence(){
    let mut f=fixture(true,true);ready(&mut f);let handle=f.app.world().resource::<LocaleAssetHost>().bound["en"].clone();
    f.app.world_mut().resource_mut::<Events<AssetLoadFailedEvent<LoadedLocaleSource>>>().send(AssetLoadFailedEvent{id:handle.id(),path:bevy::asset::AssetPath::from("en.toml").with_source(LOCALE_SOURCE),error:bevy::asset::AssetLoadError::AssetReaderError(bevy::asset::io::AssetReaderError::NotFound(PathBuf::from("en.toml")))});
    assert!(portable_receipt(f.app.world()).is_err());let receipt=ready(&mut f);assert!(receipt.locale_absent.is_empty());assert!(matches!(f.app.world().resource::<AssetServer>().load_state(handle.id()),bevy::asset::LoadState::Loaded));
}
#[test]
fn native_absence0664_old_or_wrong_path_handle_cannot_authenticate_absence(){
    let mut f=fixture(true,false);ready(&mut f);let handle=f.app.world().resource::<AssetServer>().load::<LoadedLocaleSource>(bevy::asset::AssetPath::from("other.toml").with_source(LOCALE_SOURCE));
    f.app.world_mut().resource_mut::<LocaleAssetHost>().bound.insert("en".into(),handle);assert!(live_receipt(f.app.world()).is_err());
}
#[test]
fn native_absence0664_replaced_server_cannot_reuse_retained_source_authority(){
    let mut f=fixture(true,false);ready(&mut f);let live=live_receipt(f.app.world()).unwrap();let other=fixture(true,false);let server=other.app.world().resource::<AssetServer>().clone();f.app.world_mut().insert_resource(server);
    assert!(accepts_live_receipt(f.app.world(),&live).is_err());
}
#[test]
fn native_absence0664_appearance_before_selected_language_commit_revokes_exact_live_receipt(){
    let mut f=fixture(true,false);let receipt=ready(&mut f);let prepared=prepare_selected_language(f.app.world(),&receipt).unwrap();
    let before=f.app.world().resource::<VnEngine>().0.locale.as_ref().unwrap().portable_receipt().unwrap();
    fs::write(f.root.join("locales/en.toml"),"[strings]\n").unwrap();assert!(prepared.verify_host(f.app.world()).is_err());
    assert_eq!(f.app.world().resource::<VnEngine>().0.locale.as_ref().unwrap().portable_receipt().unwrap(),before);
}
#[test]
fn native_absence0664_root_creation_without_file_revokes_live_but_not_portable_receipt(){
    let mut f=fixture(false,false);let before=ready(&mut f);let live=live_receipt(f.app.world()).unwrap();fs::create_dir(f.root.join("locales")).unwrap();
    assert!(!accepts_live_receipt(f.app.world(),&live).unwrap());assert_eq!(portable_receipt(f.app.world()).unwrap(),before);
}
#[test]
fn native_absence0664_async_rescan_loads_later_creation_and_removal_without_watcher(){
    let mut f=fixture(false,false);ready(&mut f);fs::create_dir(f.root.join("locales")).unwrap();fs::write(f.root.join("locales/en.toml"),"[strings]\n\"First\" = \"Appeared\"\n").unwrap();
    pump(&mut f,|world|portable_receipt(world).is_ok_and(|r|!r.locale_absent.contains("en")));let handle=f.app.world().resource::<LocaleAssetHost>().bound["en"].id();
    fs::write(f.root.join("locales/en.toml"),"[strings]\n\"First\" = \"Changed\"\n").unwrap();
    pump(&mut f,|world|portable_receipt(world).is_ok_and(|r|r.locales.cache["en"].strings.get("First").is_some_and(|v|v=="Changed")));
    fs::remove_file(f.root.join("locales/en.toml")).unwrap();pump(&mut f,|world|portable_receipt(world).is_ok_and(|r|r.locale_absent.contains("en")));
    assert_eq!(f.app.world().resource::<LocaleAssetHost>().bound["en"].id(),handle,"real AssetServer reload retains the bound handle");
}
#[test]
fn native_absence0664_malformed_and_invalid_utf8_remain_actual_failures(){
    for raw in [vec![0xff],b"[strings]\n\"First\" = \"unterminated\n".to_vec()]{
        let mut f=fixture(true,false);ready(&mut f);fs::write(f.root.join("locales/en.toml"),raw).unwrap();
        pump(&mut f,|world|{let host=world.resource::<LocaleAssetHost>();matches!(world.resource::<AssetServer>().load_state(host.bound["en"].id()),bevy::asset::LoadState::Failed(_))&&!native_locale_provider::failed_not_found(&world.resource::<AssetServer>().load_state(host.bound["en"].id()))});
        assert!(live_receipt(f.app.world()).is_err());assert!(startup_loads_ready(f.app.world()).is_err());
    }
}
#[test]
fn native_absence0664_file_replacing_root_is_never_not_found_fallback(){
    let mut f=fixture(false,false);ready(&mut f);fs::write(f.root.join("locales"),"not a directory").unwrap();assert!(live_receipt(f.app.world()).is_err());
}
#[test]
fn native_absence0664_preloader_rejects_root_type_and_malformed_instead_of_empty(){
    let f=fixture(true,true);fs::write(f.root.join("locales/en.toml"),[0xff]).unwrap();assert!(load_native_frozen_locales(&f.root.join("locales"),&declarations()).is_err());
    let file=f.root.join("wrong-root");fs::write(&file,"file").unwrap();assert!(native_locale_root(&file).is_err());
}
#[test]
fn native_absence0664_opaque_factory_override_before_sealing_cannot_claim_native_absence(){
    let mut f=fixture_with_hook(false,false,|app|{app.register_asset_source(LOCALE_SOURCE,bevy::asset::io::AssetSource::build().with_reader(||Box::new(bevy::asset::io::memory::MemoryAssetReader::default())));});
    pump(&mut f,all_failed_not_found);assert!(live_receipt(f.app.world()).is_err());assert!(startup_loads_ready(f.app.world()).is_err());
}
#[test]
fn native_absence0664_replaced_known_authority_refuses_old_live_capability(){
    let mut f=fixture(true,false);ready(&mut f);let live=live_receipt(f.app.world()).unwrap();f.app.world_mut().resource_mut::<KnownTextSources>().authority=Arc::new(());
    assert!(!accepts_live_receipt(f.app.world(),&live).unwrap());
}
#[test]
fn native_absence0664_actual_server_clone_keeps_same_retained_reader_authority(){
    let mut f=fixture(true,false);ready(&mut f);let live=live_receipt(f.app.world()).unwrap();let clone=f.app.world().resource::<AssetServer>().clone();f.app.world_mut().insert_resource(clone);
    assert!(accepts_live_receipt(f.app.world(),&live).unwrap());
}
#[test]
fn native_absence0664_selected_missing_language_is_reconstructed_without_mutating_live_owner(){
    let mut f=fixture(true,false);ready(&mut f);
    f.app.world_mut().resource_mut::<VnEngine>().0.locale.as_mut().unwrap().set_language("fr").unwrap();let expected=portable_receipt(f.app.world()).unwrap();
    f.app.world_mut().resource_mut::<VnEngine>().0.locale.as_mut().unwrap().set_language("en").unwrap();let before=live_receipt(f.app.world()).unwrap();let prepared=prepare_selected_language(f.app.world(),&expected).unwrap();
    assert_eq!(prepared.locale().current_lang(),"fr");assert_eq!(prepared.receipt(),&expected);prepared.verify_host(f.app.world()).unwrap();assert!(accepts_live_receipt(f.app.world(),&before).unwrap());
}
#[test]
fn native_absence0664_relocated_cold_provider_preserves_portable_but_cannot_replay_live_authority(){
    let mut first=fixture(false,false);let receipt=ready(&mut first);let live=live_receipt(first.app.world()).unwrap();let mut second=fixture(false,false);let other=ready(&mut second);
    assert_ne!(first.root,second.root);assert_eq!(receipt,other);assert_eq!(receipt.canonical_digest().unwrap(),other.canonical_digest().unwrap());
    assert!(!accepts_live_receipt(second.app.world(),&live).unwrap());prepare_selected_language(second.app.world(),&receipt).unwrap().verify_host(second.app.world()).unwrap();
}
#[test]
fn native_absence0664_cold_present_empty_cannot_restore_an_absence_receipt(){
    let mut first=fixture(true,false);let receipt=ready(&mut first);let mut second=fixture(true,false);for lang in ["fr","en"]{fs::write(second.root.join(format!("locales/{lang}.toml")),"[strings]\n").unwrap();}ready(&mut second);
    assert!(prepare_selected_language(second.app.world(),&receipt).is_err());
}
#[test]
fn native_absence0664_source_without_meta_is_present_and_malformed_meta_is_failed(){
    let mut f=fixture(true,true);let receipt=ready(&mut f);assert!(receipt.locale_absent.is_empty());assert!(!f.root.join("locales/en.toml.meta").exists());
    fs::write(f.root.join("locales/en.toml.meta"),"not valid metadata").unwrap();f.app.world().resource::<AssetServer>().reload(bevy::asset::AssetPath::from("en.toml").with_source(LOCALE_SOURCE));
    pump(&mut f,|world|{let handle=&world.resource::<LocaleAssetHost>().bound["en"];matches!(world.resource::<AssetServer>().load_state(handle.id()),bevy::asset::LoadState::Failed(_))});assert!(live_receipt(f.app.world()).is_err());
}
#[test]
fn native_absence0664_async_fallback_observes_metadata_creation_after_missing_root(){
    let mut f=fixture(false,false);ready(&mut f);fs::create_dir(f.root.join("locales")).unwrap();fs::write(f.root.join("locales/en.toml"),"[strings]\n").unwrap();
    pump(&mut f,|world|portable_receipt(world).is_ok_and(|r|!r.locale_absent.contains("en")));
    fs::write(f.root.join("locales/en.toml.meta"),"invalid metadata after initial watcher failure").unwrap();
    pump(&mut f,|world|{let id=world.resource::<LocaleAssetHost>().bound["en"].id();matches!(world.resource::<AssetServer>().load_state(id),bevy::asset::LoadState::Failed(_))&&!native_locale_provider::failed_not_found(&world.resource::<AssetServer>().load_state(id))});
    assert!(live_receipt(f.app.world()).is_err());
}
#[test]
fn native_absence0664_declared_directory_io_failure_is_not_a_notfound_fallback(){
    let mut f=fixture(true,false);ready(&mut f);fs::create_dir(f.root.join("locales/en.toml")).unwrap();f.app.world().resource::<AssetServer>().reload(bevy::asset::AssetPath::from("en.toml").with_source(LOCALE_SOURCE));
    pump(&mut f,|world|{let handle=&world.resource::<LocaleAssetHost>().bound["en"];matches!(world.resource::<AssetServer>().load_state(handle.id()),bevy::asset::LoadState::Failed(_))&&!native_locale_provider::failed_not_found(&world.resource::<AssetServer>().load_state(handle.id()))});assert!(live_receipt(f.app.world()).is_err());
}
fn directory_link(path:&Path,target:&Path){
    #[cfg(unix)]
    std::os::unix::fs::symlink(target,path).unwrap();
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let output=std::process::Command::new("powershell.exe").creation_flags(0x08000000)
            .args(["-NoProfile","-NonInteractive","-Command","$ErrorActionPreference='Stop'; New-Item -ItemType Junction -Path $env:RVN_TEST_LINK -Target $env:RVN_TEST_TARGET | Out-Null"])
            .env("RVN_TEST_LINK",path).env("RVN_TEST_TARGET",target).output().unwrap();
        assert!(output.status.success(),"private junction fixture: {}",String::from_utf8_lossy(&output.stderr));
    }
}
#[test]
fn native_absence0664_missing_root_replaced_by_junction_revokes_native_authority(){
    let mut f=fixture(false,false);ready(&mut f);let live=live_receipt(f.app.world()).unwrap();let target=f.root.join("foreign-locales");fs::create_dir(&target).unwrap();let link=f.root.join("locales");
    directory_link(&link,&target);let refused=accepts_live_receipt(f.app.world(),&live).is_err();fs::remove_dir(&link).unwrap();assert!(refused);
}
#[test]
fn native_absence0664_existing_root_replaced_by_junction_revokes_native_authority(){
    let mut f=fixture(true,false);ready(&mut f);let live=live_receipt(f.app.world()).unwrap();let original=f.root.join("retained-locales");let link=f.root.join("locales");
    assert_eq!(original.parent(),link.parent());assert_eq!(link.parent(),Some(f.root.as_path()));fs::rename(&link,&original).unwrap();directory_link(&link,&original);
    let refused=accepts_live_receipt(f.app.world(),&live).is_err();fs::remove_dir(&link).unwrap();assert!(refused);
}
#[test]
fn native_absence0664_preload_root_witness_is_not_recaptured_after_junction_replacement(){
    let f=fixture(false,false);let path=f.root.join("future-locales");let witness=closed_native_locale_root(&path).unwrap();let target=f.root.join("foreign-locales");fs::create_dir(&target).unwrap();directory_link(&path,&target);
    let mut app=App::new();let refused=register_verified_text_sources(&mut app,f.root.join("assets").to_string_lossy().into_owned(),witness).is_err();fs::remove_dir(&path).unwrap();assert!(refused);
}
#[test]
fn native_absence0664_bound_subset_never_publishes_partial_closure(){
    let mut f=fixture(true,false);ready(&mut f);f.app.world_mut().resource_mut::<LocaleAssetHost>().bound.remove("en");assert!(portable_receipt(f.app.world()).is_err());
}
#[test]
#[ignore="4096-language real-reader cost measurement; run explicitly with memory reserve and record elapsed times"]
fn native_absence0664_measure_existing_4096_declaration_budget(){
    let f=fixture(false,false);let available=(0..4096).map(|n|format!("lang_{n}")).collect::<Vec<_>>();let declarations=LocaleDeclarations{default_lang:available[0].clone(),current_lang:available[1].clone(),files:available.iter().map(|lang|(lang.clone(),format!("{lang}.toml"))).collect(),available};
    declarations.validate().unwrap();let start=Instant::now();let locale=load_native_frozen_locales(&f.root.join("locales"),&declarations).unwrap();eprintln!("native_absence0664 preload4096={:?}",start.elapsed());assert_eq!(locale.portable_receipt().unwrap().cache.len(),4096);
    let sources=f.app.world().resource::<KnownTextSources>();let server=f.app.world().resource::<AssetServer>();let start=Instant::now();
    for path in declarations.files.values(){native_locale_provider::prove_absent(sources,server,path).unwrap();}eprintln!("native_absence0664 real_reader_probes4096={:?}",start.elapsed());
    let mut full=fixture_custom(false,false,declarations,|_|{});let start=Instant::now();ready(&mut full);eprintln!("native_absence0664 actual_AS_closure4096={:?}",start.elapsed());
    let start=Instant::now();let live=live_receipt(full.app.world()).unwrap();eprintln!("native_absence0664 full_live_receipt4096={:?}",start.elapsed());
    let start=Instant::now();assert!(accepts_live_receipt(full.app.world(),&live).unwrap());eprintln!("native_absence0664 full_accept4096={:?}",start.elapsed());
    // Same existing declaration budget, now real present files + genuine
    // default Unit metadata, to expose boundary hash/parser costs explicitly.
    let declarations=full.app.world().resource::<LocaleAssetHost>().declarations.clone();
    let metadata=actual_default_meta::<LoadedLocaleSource>(f.app.world());let meta_bytes=metadata.len();let files=declarations.files.clone();
    let mut present=fixture_custom(false,false,declarations,move|app|{let root=PathBuf::from(&app.world().resource::<KnownTextSources>().locale_root);fs::create_dir(&root).unwrap();
        for path in files.values(){fs::write(root.join(path),"[strings]\n").unwrap();fs::write(root.join(format!("{path}.meta")),&metadata).unwrap();}
    });let start=Instant::now();let receipt=ready(&mut present);assert!(receipt.locale_absent.is_empty());eprintln!("native_loaded0673 present4096_AS_scan_metadata={:?}; metadata_bytes_each={meta_bytes}",start.elapsed());
    let start=Instant::now();let live=live_receipt(present.app.world()).unwrap();eprintln!("native_loaded0673 present4096_full_live_receipt={:?}",start.elapsed());
    let start=Instant::now();assert!(accepts_live_receipt(present.app.world(),&live).unwrap());eprintln!("native_loaded0673 present4096_full_accept={:?}",start.elapsed());
}

// Test-only async gates pause the real registered loader after its real raw
// bytes have been read. No fake reader, asset, handle, LoadState or Select event.
struct DecodeGateState{
    entered:std::sync::atomic::AtomicBool,
    released:std::sync::atomic::AtomicBool,
    observed:std::sync::atomic::AtomicBool,
    waker:std::sync::Mutex<Option<std::task::Waker>>,
    expected:native_locale_provider::Probe,
}
fn decode_gates()->&'static std::sync::Mutex<Vec<(PathBuf,Arc<DecodeGateState>)>>{
    static GATES:std::sync::OnceLock<std::sync::Mutex<Vec<(PathBuf,Arc<DecodeGateState>)>>>=std::sync::OnceLock::new();
    GATES.get_or_init(Default::default)
}
pub(crate) struct DecodeGate{path:PathBuf,state:Arc<DecodeGateState>}
impl DecodeGate{
    fn install(path:PathBuf,expected:native_locale_provider::Probe)->Self{
        let state=Arc::new(DecodeGateState{entered:false.into(),released:false.into(),observed:false.into(),waker:Default::default(),expected});
        let mut gates=decode_gates().lock().unwrap();assert!(!gates.iter().any(|(p,_)|p==&path));gates.push((path.clone(),state.clone()));Self{path,state}
    }
    pub(crate) fn release(&self){self.state.released.store(true,Ordering::Release);if let Some(waker)=self.state.waker.lock().unwrap().take(){waker.wake();}}
}
// Test-only bridge for the real normal runtime harness. Paths are unique per
// fixture; no opaque provider or synthetic load/event is introduced.
pub(crate) fn hold_normal_locale_decode(path:PathBuf,expected_content:&[u8])->DecodeGate{
    DecodeGate::install(path,native_locale_provider::Probe{content:Some(Sha256::digest(expected_content).into()),metadata:None})
}
impl DecodeGate{
    pub(crate) fn entered(&self)->bool{self.state.entered.load(Ordering::Acquire)}
    pub(crate) fn observed(&self)->bool{self.state.observed.load(Ordering::Acquire)}
}
impl Drop for DecodeGate{fn drop(&mut self){self.release();decode_gates().lock().unwrap().retain(|(path,_)|path!=&self.path);}}
pub(super) async fn pause_decode(path:&Path){
    let state={decode_gates().lock().unwrap().iter().find(|(p,_)|p==path).map(|(_,state)|state.clone())};
    let Some(state)=state else{return;};state.entered.store(true,Ordering::Release);
    std::future::poll_fn(|cx|{
        if state.released.load(Ordering::Acquire){return std::task::Poll::Ready(());}
        let mut waker=state.waker.lock().unwrap();
        if state.released.load(Ordering::Acquire){std::task::Poll::Ready(())}else{*waker=Some(cx.waker().clone());std::task::Poll::Pending}
    }).await;
}
pub(super) fn received_probe(path:&Path,probe:native_locale_provider::Probe){
    let state={decode_gates().lock().unwrap().iter().find(|(p,_)|p==path).map(|(_,state)|state.clone())};
    if let Some(state)=state{if state.expected==probe{state.observed.store(true,Ordering::Release);}}
}
fn race_declarations(name:&str)->(LocaleDeclarations,PathBuf){
    let path=PathBuf::from(format!("{name}-{}-{}.toml",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)));
    let mut declarations=declarations();declarations.files.insert("en".into(),path.to_str().unwrap().to_owned());(declarations,path)
}
fn pump_race(f:&mut Fixture,mut predicate:impl FnMut(&World)->bool){
    let deadline=Instant::now()+Duration::from_secs(20);loop{f.app.update();if predicate(f.app.world()){return;}assert!(Instant::now()<deadline,"actual gated loader/scan did not settle");std::thread::sleep(Duration::from_millis(5));}
}
#[test]
fn native_absence0669_real_source_b_seen_while_a_loading_is_retried_after_real_a_failure(){
    let(declarations,path)=race_declarations("source-loading");let b=b"[strings]\n\"First\" = \"Source B\"\n";
    let gate=DecodeGate::install(path.clone(),native_locale_provider::Probe{content:Some(Sha256::digest(b).into()),metadata:None});
    let write_path=path.clone();let mut f=fixture_custom(true,false,declarations,move|app|{
        let root=&app.world().resource::<KnownTextSources>().locale_root;fs::write(Path::new(root).join(&write_path),[0xff]).unwrap();
    });
    pump_race(&mut f,|_|gate.state.entered.load(Ordering::Acquire));let handle=f.app.world().resource::<LocaleAssetHost>().bound["en"].clone();
    assert!(matches!(f.app.world().resource::<AssetServer>().load_state(handle.id()),bevy::asset::LoadState::Loading));
    fs::write(f.root.join("locales").join(&path),b).unwrap();pump_race(&mut f,|_|gate.state.observed.load(Ordering::Acquire));
    assert!(matches!(f.app.world().resource::<AssetServer>().load_state(handle.id()),bevy::asset::LoadState::Loading));assert!(live_receipt(f.app.world()).is_err());gate.release();
    let mut failures=ManualEventReader::<AssetLoadFailedEvent<LoadedLocaleSource>>::default();let mut actual_failure=false;
    pump_race(&mut f,|world|{
        actual_failure|=failures.read(world.resource::<Events<AssetLoadFailedEvent<LoadedLocaleSource>>>()).any(|event|event.id==handle.id()&&!matches!(&event.error,bevy::asset::AssetLoadError::AssetReaderError(bevy::asset::io::AssetReaderError::NotFound(_))));
        actual_failure&&portable_receipt(world).is_ok_and(|r|!r.locale_absent.contains("en")&&r.locales.cache["en"].strings.get("First").is_some_and(|v|v=="Source B"))
    });
    assert!(actual_failure);assert_eq!(f.app.world().resource::<LocaleAssetHost>().bound["en"].id(),handle.id());assert!(matches!(f.app.world().resource::<AssetServer>().load_state(handle.id()),bevy::asset::LoadState::Loaded));
}
// This expected-behaviour oracle intentionally remains OPEN after the narrow
// retry repair: old Loaded/raw is not metadata reload completion evidence.
#[test]
fn native_absence0669_metadata_seen_during_loading_never_admits_old_loaded_value(){
    let(declarations,path)=race_declarations("metadata-loading");let raw=b"[strings]\n";let metadata=b"invalid metadata after the actual loader opened old default metadata";
    let gate=DecodeGate::install(path.clone(),native_locale_provider::Probe{content:Some(Sha256::digest(raw).into()),metadata:Some(Sha256::digest(metadata).into())});
    let write_path=path.clone();let mut f=fixture_custom(true,false,declarations,move|app|{let root=&app.world().resource::<KnownTextSources>().locale_root;fs::write(Path::new(root).join(&write_path),raw).unwrap();});
    pump_race(&mut f,|_|gate.state.entered.load(Ordering::Acquire));let handle=f.app.world().resource::<LocaleAssetHost>().bound["en"].clone();
    assert!(matches!(f.app.world().resource::<AssetServer>().load_state(handle.id()),bevy::asset::LoadState::Loading));
    let meta=PathBuf::from(format!("{}.meta",path.to_str().unwrap()));fs::write(f.root.join("locales").join(meta),metadata).unwrap();
    pump_race(&mut f,|_|gate.state.observed.load(Ordering::Acquire));gate.release();
    let mut failures=ManualEventReader::<AssetLoadFailedEvent<LoadedLocaleSource>>::default();let mut actual_metadata_failure=false;
    pump_race(&mut f,|world|{
        actual_metadata_failure|=failures.read(world.resource::<Events<AssetLoadFailedEvent<LoadedLocaleSource>>>()).any(|event|event.id==handle.id()&&matches!(&event.error,bevy::asset::AssetLoadError::DeserializeMeta{..}));
        assert!(live_receipt(world).is_err(),"Old Loaded/raw must never authenticate the changed invalid metadata before its real failure");
        actual_metadata_failure
    });assert!(actual_metadata_failure);assert_eq!(f.app.world().resource::<LocaleAssetHost>().bound["en"].id(),handle.id());
}

// These exercise actual loaders, Task/AssetServer timing and real file edits.
// No positive event, fabricated Loaded state or manually inserted locale.
#[test]
fn native_loaded0673_source_b_seen_while_valid_a_loading_never_admits_a(){
    let(declarations,path)=race_declarations("valid-source-loading");let a=b"[strings]\n\"First\" = \"Source A\"\n";let b=b"[strings]\n\"First\" = \"Source B\"\n";
    let gate=DecodeGate::install(path.clone(),native_locale_provider::Probe{content:Some(Sha256::digest(b).into()),metadata:None});
    let write_path=path.clone();let mut f=fixture_custom(true,false,declarations,move|app|{let root=&app.world().resource::<KnownTextSources>().locale_root;fs::write(Path::new(root).join(&write_path),a).unwrap();});
    pump_race(&mut f,|_|gate.state.entered.load(Ordering::Acquire));let id=f.app.world().resource::<LocaleAssetHost>().bound["en"].id();
    fs::write(f.root.join("locales").join(&path),b).unwrap();pump_race(&mut f,|_|gate.state.observed.load(Ordering::Acquire));gate.release();
    pump_race(&mut f,|world|{
        if let Ok((catalogs,_))=locale_catalogs(world,world.resource::<LocaleAssetHost>()){
            assert_eq!(catalogs["en"].table().strings.get("First").map(String::as_str),Some("Source B"),"Old actual Loaded A cannot satisfy the observed B fence");
        }
        portable_receipt(world).is_ok_and(|receipt|receipt.locales.cache["en"].strings.get("First").is_some_and(|v|v=="Source B"))
    });assert_eq!(f.app.world().resource::<LocaleAssetHost>().bound["en"].id(),id);
}
#[test]
fn native_loaded0673_decoded_string_mutation_invalidates_actual_loader_witness(){
    let mut f=fixture(true,true);ready(&mut f);let before=live_receipt(f.app.world()).unwrap();let id=f.app.world().resource::<LocaleAssetHost>().bound["en"].id();
    f.app.world_mut().resource_mut::<Assets<LoadedLocaleSource>>().get_mut(id).unwrap().0="[strings]\n\"First\" = \"forged decoded content\"\n".into();
    assert!(locale_catalogs(f.app.world(),f.app.world().resource::<LocaleAssetHost>()).unwrap_err().contains("loader witness"));
    assert!(live_receipt(f.app.world()).is_err());assert!(accepts_live_receipt(f.app.world(),&before).is_err());
}
fn actual_default_meta<A:Asset>(world:&World)->Vec<u8>{
    bevy::tasks::block_on(world.resource::<AssetServer>().get_asset_loader_with_asset_type::<A>()).unwrap().default_meta().serialize()
}
#[test]
fn native_loaded0673_real_unit_metadata_is_compatible_but_never_creates_loaded_source(){
    let mut f=fixture(true,true);ready(&mut f);let before=live_receipt(f.app.world()).unwrap();let id=f.app.world().resource::<LocaleAssetHost>().bound["en"].id();
    let metadata=actual_default_meta::<LoadedLocaleSource>(f.app.world());fs::write(f.root.join("locales/en.toml.meta"),&metadata).unwrap();
    assert!(live_receipt(f.app.world()).is_err(),"Fresh metadata must first be validated by the real Bevy parser");
    pump(&mut f,|world|portable_receipt(world).is_ok());assert!(!accepts_live_receipt(f.app.world(),&before).unwrap());
    assert_eq!(f.app.world().resource::<LocaleAssetHost>().bound["en"].id(),id);assert!(matches!(f.app.world().resource::<AssetServer>().load_state(id),bevy::asset::LoadState::Loaded));
    assert_eq!(f.app.world().resource::<Assets<LoadedLocaleSource>>().get(id).unwrap().authentic_raw().unwrap(),"[strings]\n\"First\" = \"Translation\"\n");
}
#[test]
fn native_loaded0673_explicit_real_other_type_loader_is_not_locale_metadata(){
    let mut f=fixture(true,true);ready(&mut f);let metadata=actual_default_meta::<LoadedStringTable>(f.app.world());
    fs::write(f.root.join("locales/en.toml.meta"),metadata).unwrap();assert!(live_receipt(f.app.world()).is_err());
    pump(&mut f,|world|world.resource::<LocaleAssetHost>().metadata_errors.get("en").is_some_and(|message|message.contains("not the actual locale source loader")));
    assert!(live_receipt(f.app.world()).is_err());
}
#[test]
fn native_loaded0673_nonunit_settings_are_refused_by_actual_loader_metadata_parser(){
    let mut f=fixture(true,true);ready(&mut f);let metadata=String::from_utf8(actual_default_meta::<LoadedLocaleSource>(f.app.world())).unwrap();
    assert_eq!(metadata.matches("settings: ()").count(),1);let invalid=metadata.replace("settings: ()","settings: (unexpected: true)");
    fs::write(f.root.join("locales/en.toml.meta"),invalid).unwrap();
    pump(&mut f,|world|world.resource::<LocaleAssetHost>().metadata_errors.get("en").is_some_and(|message|message.contains("Bevy loader metadata parse")));
    assert!(live_receipt(f.app.world()).is_err());
}
#[test]
fn native_loaded0673_installed_never_policy_ignores_invalid_metadata(){
    let mut f=fixture_custom_with_policy(true,true,declarations(),bevy::asset::AssetMetaCheck::Never,|app|{let root=&app.world().resource::<KnownTextSources>().locale_root;fs::write(Path::new(root).join("en.toml.meta"),"invalid metadata intentionally ignored by actual plugin").unwrap();});
    ready(&mut f);assert!(matches!(f.app.world().resource::<KnownTextSources>().meta_check.as_ref(),Some(bevy::asset::AssetMetaCheck::Never)));
    assert_eq!(f.app.world().resource::<LocaleAssetHost>().latest_probe["en"].metadata,None);
    assert!(f.app.world().resource::<LocaleAssetHost>().metadata_errors.is_empty());
}
#[test]
fn native_loaded0673_installed_paths_policy_uses_real_named_asset_path(){
    let en=bevy::asset::AssetPath::from("en.toml").with_source(LOCALE_SOURCE);
    let mut f=fixture_custom_with_policy(true,true,declarations(),bevy::asset::AssetMetaCheck::Paths([en].into_iter().collect()),|app|{
        let root=&app.world().resource::<KnownTextSources>().locale_root;
        for lang in ["fr","en"]{fs::write(Path::new(root).join(format!("{lang}.toml.meta")),"invalid metadata").unwrap();}
    });
    pump(&mut f,|world|world.resource::<LocaleAssetHost>().metadata_errors.contains_key("en"));
    let host=f.app.world().resource::<LocaleAssetHost>();assert_eq!(host.latest_probe["fr"].metadata,None);assert!(host.latest_probe["en"].metadata.is_some());assert!(!host.metadata_errors.contains_key("fr"));
    let id=host.bound["fr"].id();assert!(matches!(f.app.world().resource::<AssetServer>().load_state(id),bevy::asset::LoadState::Loaded));assert!(live_receipt(f.app.world()).is_err());
}
#[test]
fn native_loaded0673_process_and_ignore_follow_actual_bevy_negative_contract(){
    for action in [bevy::asset::meta::AssetActionMinimal::Process{processor:"unused".into()},bevy::asset::meta::AssetActionMinimal::Ignore]{
        let mut f=fixture(true,true);ready(&mut f);
        let bytes=bevy::asset::ron::ser::to_string(&bevy::asset::meta::AssetMetaMinimal{asset:action}).unwrap();fs::write(f.root.join("locales/en.toml.meta"),bytes).unwrap();
        pump(&mut f,|world|world.resource::<LocaleAssetHost>().metadata_errors.contains_key("en"));assert!(live_receipt(f.app.world()).is_err());
    }
}
#[test]
fn native_loaded0673_metadata_edit_after_real_async_validation_revokes_prepared_receipt(){
    let mut f=fixture(true,true);ready(&mut f);let metadata=actual_default_meta::<LoadedLocaleSource>(f.app.world());fs::write(f.root.join("locales/en.toml.meta"),&metadata).unwrap();
    pump(&mut f,|world|portable_receipt(world).is_ok());let before=live_receipt(f.app.world()).unwrap();
    fs::write(f.root.join("locales/en.toml.meta"),"new invalid metadata before any app.update").unwrap();
    assert!(accepts_live_receipt(f.app.world(),&before).is_err());
}

#[test]
fn native_loaded0674_clone_from_another_actual_loaded_path_cannot_authenticate_equal_content(){
    let mut f=fixture(true,true);ready(&mut f);let before=live_receipt(f.app.world()).unwrap();let host=f.app.world().resource::<LocaleAssetHost>();let fr=host.bound["fr"].id();let en=host.bound["en"].id();assert_ne!(fr,en);
    let assets=f.app.world().resource::<Assets<LoadedLocaleSource>>();assert_eq!(assets.get(fr).unwrap().0,assets.get(en).unwrap().0);let other=assets.get(fr).unwrap().clone();
    f.app.world_mut().resource_mut::<Assets<LoadedLocaleSource>>().insert(en,other);
    assert!(matches!(f.app.world().resource::<AssetServer>().load_state(en),bevy::asset::LoadState::Loaded));
    assert!(locale_catalogs(f.app.world(),f.app.world().resource::<LocaleAssetHost>()).unwrap_err().contains("loader source path"));
    assert!(accepts_live_receipt(f.app.world(),&before).is_err());
}

#[test]
fn native_loaded0683_two_real_hosts_same_path_and_raw_require_each_actual_loader_authority(){
    let mut first=fixture(true,true);let local_first=ready(&mut first);let mut second=fixture(true,true);let local_second=ready(&mut second);assert_ne!(first.root,second.root);assert_eq!(local_first,local_second);
    assert!(!Arc::ptr_eq(&first.app.world().resource::<KnownTextSources>().authority,&second.app.world().resource::<KnownTextSources>().authority));
    let first_id=first.app.world().resource::<LocaleAssetHost>().bound["en"].id();let second_id=second.app.world().resource::<LocaleAssetHost>().bound["en"].id();
    assert_eq!(first.app.world().resource::<AssetServer>().get_path(first_id),second.app.world().resource::<AssetServer>().get_path(second_id));
    let foreign=first.app.world().resource::<Assets<LoadedLocaleSource>>().get(first_id).unwrap().clone();
    assert_eq!(foreign.0,second.app.world().resource::<Assets<LoadedLocaleSource>>().get(second_id).unwrap().0);
    // Both positive controls above came from genuine loaders and real readers.
    // Equal source/path/raw in the other host cannot replace its loader origin.
    second.app.world_mut().resource_mut::<Assets<LoadedLocaleSource>>().insert(second_id,foreign);
    assert!(matches!(second.app.world().resource::<AssetServer>().load_state(second_id),bevy::asset::LoadState::Loaded));
    assert!(locale_catalogs(second.app.world(),second.app.world().resource::<LocaleAssetHost>()).unwrap_err().contains("provider authority"));assert!(live_receipt(second.app.world()).is_err());
    assert_eq!(portable_receipt(first.app.world()).unwrap(),local_first,"The other actual provider remains valid");
    // Recovery uses its own real loader again, never a fabricated witness or a
    // manual reinsertion of the old local positive value.
    second.app.world().resource::<AssetServer>().reload(bevy::asset::AssetPath::from("en.toml").with_source(LOCALE_SOURCE));
    pump(&mut second,|world|portable_receipt(world).is_ok_and(|receipt|receipt==local_second));
    assert_eq!(second.app.world().resource::<LocaleAssetHost>().bound["en"].id(),second_id);assert!(matches!(second.app.world().resource::<AssetServer>().load_state(second_id),bevy::asset::LoadState::Loaded));
    assert!(live_receipt(first.app.world()).is_ok());assert!(live_receipt(second.app.world()).is_ok());
}
