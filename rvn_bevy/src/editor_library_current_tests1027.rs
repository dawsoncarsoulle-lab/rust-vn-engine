//! Real native filesystem selectors with CPU Core consumption; no window/ACK.
use super::*;
use std::{collections::BTreeMap,sync::atomic::{AtomicU64,Ordering}};
use rvn_parser::function_library_asset1023::{FunctionLibraryId,FunctionLibraryMember,LibraryFunctionAccess};
const A:&str="11111111111141118111111111111111";
fn asset()->FunctionLibraryAsset{FunctionLibraryAsset::new(FunctionLibraryId::parse(A).unwrap(),vec![FunctionLibraryMember{graph_id:11,access:LibraryFunctionAccess::Public,pure:true}]).unwrap()}
fn program()->ParsedProgram{rvn_parser::parse_program(&format!("functions.library_v1(\"{A}\"){{functions.member_v1(\"11\",[]){{function actual(){{return 9}}}}}}\nscene label start\nset actual=functions.value_v1(\"{A}\",\"11\",{{}})\n\"After\"\n")).unwrap()}
struct Fixture{root:PathBuf,program:ParsedProgram,manifest:String,bytes:Vec<u8>,argument:String}
impl Fixture{
 fn new()->Self{
  static NEXT:AtomicU64=AtomicU64::new(0);
  let root=std::env::temp_dir().join(format!("rvn-library-current-host1027-{}-{}",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)));fs::create_dir(&root).unwrap();let root=root.canonicalize().unwrap();
  let program=program();let asset=asset();let captured=wire::CapturedLibraries1027::from_admitted(&[asset.clone()],&BTreeMap::from([(asset.id().clone(),"Current.rvnlibrary".into())])).unwrap();
  let bytes=captured.encode_for(&program,7).unwrap();let argument=wire::LaunchSeal1027::for_bytes(7,&bytes).unwrap().argument();
  let manifest="[project]\nmain_script='main.rvn'\n[paths]\nassets='C:/original-media'\n".into();fs::write(root.join("rvn.toml"),&manifest).unwrap();fs::write(root.join(SIDECAR),&bytes).unwrap();Self{root,program,manifest,bytes,argument}
 }
 fn prepare(&self)->Result<PreparedLibraries1027,String>{prepare_for_arguments(&self.root,&self.manifest,&self.program,vec![self.argument.clone()])}
 fn install_saved(&mut self){self.manifest=format!("[project]\nmain_script='main.rvn'\n[paths]\nassets='assets'\n[function_libraries]\n'{A}'='Saved.rvnlibrary'\n");fs::create_dir(self.root.join("assets")).unwrap();fs::write(self.root.join("rvn.toml"),&self.manifest).unwrap();fs::write(self.root.join("assets/Saved.rvnlibrary"),asset().encode().unwrap()).unwrap();}
}
impl Drop for Fixture{fn drop(&mut self){let actual=self.root.canonicalize().unwrap();assert_eq!(actual.parent(),Some(std::env::temp_dir().canonicalize().unwrap().as_path()));assert!(actual.file_name().unwrap().to_string_lossy().starts_with("rvn-library-current-host1027-"));fs::remove_dir_all(actual).unwrap();}}
#[test]
fn current_library_host1027_exact_current_reaches_joint_core_without_saved_or_media_rewrite(){
 let f=Fixture::new();let original=fs::read(f.root.join("rvn.toml")).unwrap();let resources=f.prepare().unwrap().finish(&f.program).unwrap();
 assert!(!f.root.join("assets").exists());assert!(!f.root.join("Current.rvnlibrary").exists());assert_eq!(fs::read(f.root.join("rvn.toml")).unwrap(),original);
 let mut game=rvn_core::Engine::from_library_enum_program1027(f.program.clone(),&resources,Default::default(),Default::default(),rvn_core::TerminalRenderer,8,Default::default(),Default::default()).unwrap();game.step_until_interaction().unwrap();assert_eq!(game.state.vars.get("actual"),Some(&rvn_parser::Value::Int(9)));
}
#[test]
fn current_library_host1027_explicit_missing_corruption_and_empty_never_acquire_saved_descriptors(){
 let mut f=Fixture::new();f.install_saved();fs::remove_file(f.root.join(SIDECAR)).unwrap();assert!(f.prepare().is_err());fs::write(f.root.join(SIDECAR),b"{}").unwrap();assert!(f.prepare().is_err());
 let plain=rvn_parser::parse_program("scene label start\nreturn\n").unwrap();let empty=wire::CapturedLibraries1027::from_admitted(&[],&BTreeMap::new()).unwrap();let bytes=empty.encode_for(&plain,7).unwrap();fs::write(f.root.join(SIDECAR),&bytes).unwrap();let argument=wire::LaunchSeal1027::for_bytes(7,&bytes).unwrap().argument();
 assert!(prepare_for_arguments(&f.root,&f.manifest,&plain,vec![argument]).unwrap().finish(&plain).unwrap().is_empty(),"explicit empty Current does not adopt the real Saved index");
}
#[test]
fn current_library_host1027_valid_first_read_replacement_and_late_change_refuse_original_seal(){
 let f=Fixture::new();let mut envelope:serde_json::Value=serde_json::from_slice(&f.bytes).unwrap();envelope["current"][0]["locator"]="other/Current.rvnlibrary".into();let changed=serde_json::to_vec(&envelope).unwrap();assert!(wire::decode_for1027(&changed,&f.program,7).is_ok());
 fs::write(f.root.join(SIDECAR),&changed).unwrap();let err=match f.prepare(){Err(e)=>e,Ok(_)=>panic!("first read acquired a valid replacement")};assert!(err.contains("original App launch seal"));
 fs::write(f.root.join(SIDECAR),&f.bytes).unwrap();let prepared=f.prepare().unwrap();fs::write(f.root.join(SIDECAR),changed).unwrap();assert!(prepared.finish(&f.program).is_err());
 assert!(prepare_for_arguments(&f.root,&f.manifest,&f.program,vec![f.argument.clone(),"--editor-preview=12345,8".into()]).is_err());
}
#[test]
fn current_library_host1027_saved_uses_the_same_source_reader_and_rechecks_original_before_init(){
 let mut f=Fixture::new();f.install_saved();let prepared=prepare_for_arguments(&f.root,&f.manifest,&f.program,Vec::new()).unwrap();
 fs::write(f.root.join("assets/Saved.rvnlibrary"),FunctionLibraryAsset::new(FunctionLibraryId::parse(A).unwrap(),vec![]).unwrap().encode().unwrap()).unwrap();assert!(prepared.finish(&f.program).is_err());
 fs::write(f.root.join("assets/Saved.rvnlibrary"),asset().encode().unwrap()).unwrap();let resources=prepare_for_arguments(&f.root,&f.manifest,&f.program,Vec::new()).unwrap().finish(&f.program).unwrap();assert_eq!(resources,vec![asset()]);
 let other=f.manifest.replace("main.rvn","changed.rvn");assert!(prepare_for_arguments(&f.root,&other,&f.program,Vec::new()).is_err());
}
#[test]
fn current_library_host1027_no_index_keeps_legacy_absolute_media_and_refuses_unadmitted_scope(){
 let f=Fixture::new();let plain=rvn_parser::parse_program("scene label start\nreturn\n").unwrap();let resources=prepare_for_arguments(&f.root,&f.manifest,&plain,Vec::new()).unwrap().finish(&plain).unwrap();assert!(resources.is_empty());assert!(!f.root.join("assets").exists());
 assert!(prepare_for_arguments(&f.root,&f.manifest,&f.program,Vec::new()).is_err(),"a declaration never invents descriptor ownership");
}
#[test]
fn current_library_host1027_real_production_saved_loader_retains_the_original_pending_broker_without_init_effects(){
 let mut f=Fixture::new();f.install_saved();
 f.manifest=format!("[project]\ntitle='Actual Library loader'\nmain_script='main.rvn'\nstart_label='start'\n[paths]\nassets='assets'\nlocales='locales'\nsaves='saves'\ntheme='theme.toml'\n[function_libraries]\n'{A}'='Saved.rvnlibrary'\n");
 fs::write(f.root.join("rvn.toml"),&f.manifest).unwrap();fs::create_dir(f.root.join("locales")).unwrap();fs::create_dir(f.root.join("saves")).unwrap();fs::write(f.root.join("locales/fr.toml"),"[strings]\n").unwrap();fs::write(f.root.join("theme.toml"),"[title_screen]\nenabled=true\n").unwrap();
 fs::write(f.root.join("main.rvn"),format!("functions.library_v1(\"{A}\"){{functions.member_v1(\"11\",[]){{function actual(){{return 9}}}}}}\ninit{{set initialized=functions.value_v1(\"{A}\",\"11\",{{}})}}\nscene label start\n\"After\"\n")).unwrap();
 let launch=super::super::load_runtime_game(&f.root).unwrap();
 let super::super::RuntimeLaunchOwner::Startup(startup)=launch.engine else{panic!("actual Library resources must retain the pending host")};
 assert_eq!(startup.profile,super::super::debugger_startup::BootstrapProfile::RuntimeInitial);
 assert!(startup.accepts_actual_audio_broker());assert!(startup.owner.snapshot().instance>0);
 assert!(!startup.owner.state().vars.contains_key("initialized"));assert!(!startup.owner.is_complete());
 assert!(startup.owner.program().sources.iter().any(|s|s.source.contains("functions.library_v1")));
}
