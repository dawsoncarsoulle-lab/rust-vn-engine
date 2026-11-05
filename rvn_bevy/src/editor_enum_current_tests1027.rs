use super::*;
use rvn_core::editor_enum_current1027::CapturedEnums1027;
use rvn_parser::{enum_asset::{EnumAsset,EnumAssetId,EnumMemberId,EnumCommand},enum_asset_json::EnumIndexInput};
use std::{collections::BTreeMap,sync::atomic::{AtomicU64,Ordering}};
const A:&str="329f41eae6ba4b4bb71dfaf60fb50001";const M:&str="229f41eae6ba4b4bb71dfaf60fb50001";
struct Fixture{root:PathBuf,program:ParsedProgram,captured:CapturedEnums1027,manifest:String}
impl Fixture{
 fn new()->Self{
  static NEXT:AtomicU64=AtomicU64::new(0);let root=std::env::temp_dir().join(format!("rvn-current-enum-host1027-{}-{}",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)));fs::create_dir(&root).unwrap();let root=root.canonicalize().unwrap();
  let id=EnumAssetId::parse(A).unwrap();let asset=EnumAsset::new(id,"Current".into()).unwrap().prepare(EnumCommand::Add{member:EnumMemberId::parse(M).unwrap(),display_name:"Never saved".into(),description:String::new()}).unwrap().after().clone();let rows=[(A,Some("Alpha.rvnenum"))];let resources=EnumAssets1025::admit(EnumIndexInput::Table(&rows),&[("Alpha.rvnenum".into(),asset.encode().unwrap().into_bytes())],true).unwrap();
  let captured=CapturedEnums1027::from_admitted(&resources,&BTreeMap::from([(id,"Alpha.rvnenum".into())])).unwrap();
  let program=rvn_parser::parse_program(&format!("init{{set chosen=__rvn_enum_value_v1(\"{A}\",\"{M}\")}}\nscene label start\nreturn\n")).unwrap();
  let manifest=format!("[project]\nmain_script='main.rvn'\n[paths]\nassets='C:/original-media'\n[enum_assets]\n'{A}'='Alpha.rvnenum'\n");fs::write(root.join("rvn.toml"),&manifest).unwrap();
  fs::write(root.join(SIDECAR),captured.encode_for(&program,7).unwrap()).unwrap();Self{root,program,captured,manifest}
 }
 fn argument(&self)->String{wire::LaunchSeal1027::for_bytes(7,&self.captured.encode_for(&self.program,7).unwrap()).unwrap().argument()}
 fn prepare(&self)->Result<PreparedEnums1027,String>{prepare_for_arguments(&self.root,&self.manifest,"C:/original-media",&self.program,vec![self.argument()])}
}
impl Drop for Fixture{fn drop(&mut self){let actual=self.root.canonicalize().unwrap();assert_eq!(actual.parent(),Some(std::env::temp_dir().canonicalize().unwrap().as_path()));assert!(actual.file_name().unwrap().to_string_lossy().starts_with("rvn-current-enum-host1027-"));fs::remove_dir_all(actual).unwrap();}}
#[test]
fn current_enum_host1027_actual_loader_delivers_current_before_init_without_changing_media_or_saved_index(){
 let f=Fixture::new();let original=fs::read(f.root.join("rvn.toml")).unwrap();let current=f.prepare().unwrap().finish(&f.program).unwrap();
 assert!(!f.root.join("Alpha.rvnenum").exists());assert!(!f.root.join("assets").exists());assert_eq!(fs::read(f.root.join("rvn.toml")).unwrap(),original);
 let engine=rvn_core::Engine::from_program_with_enum_assets1025(f.program.clone(),current,Default::default(),rvn_core::TerminalRenderer,8,Default::default(),Default::default()).unwrap();assert!(matches!(engine.state.vars.get("chosen"),Some(rvn_parser::Value::Enum(value))if value.member==EnumMemberId::parse(M).unwrap()));
}
#[test]
fn current_enum_host1027_missing_or_corrupt_explicit_transport_never_falls_back_to_saved(){
 let f=Fixture::new();fs::remove_file(f.root.join(SIDECAR)).unwrap();assert!(f.prepare().is_err());fs::write(f.root.join(SIDECAR),b"{}").unwrap();assert!(f.prepare().is_err());
 assert!(prepare_for_arguments(&f.root,&f.manifest,"C:/original-media",&f.program,Vec::new()).is_err(),"the ordinary disk route retains its actual indexed-assets path policy");
}
#[test]
fn current_enum_host1027_late_transport_change_and_foreign_preview_nonce_refuse_before_init(){
 let f=Fixture::new();let prepared=f.prepare().unwrap();fs::write(f.root.join(SIDECAR),f.captured.encode_for(&f.program,8).unwrap()).unwrap();assert!(prepared.finish(&f.program).is_err());
 fs::write(f.root.join(SIDECAR),f.captured.encode_for(&f.program,7).unwrap()).unwrap();assert!(prepare_for_arguments(&f.root,&f.manifest,"C:/original-media",&f.program,vec![f.argument(),"--editor-preview=12345,8".into()]).is_err());
}

#[test]
fn current_enum_host1027_valid_rewrite_before_first_prepare_refuses_original_app_byte_seal(){
 for add_member in [false,true]{
  let f=Fixture::new();let original=fs::read(f.root.join(SIDECAR)).unwrap();let argument=f.argument();
  let mut envelope:serde_json::Value=serde_json::from_slice(&original).unwrap();let asset=EnumAsset::decode(envelope["current"][0]["wire"].as_str().unwrap().as_bytes()).unwrap();
  let command=if add_member{EnumCommand::Add{member:EnumMemberId::parse("229f41eae6ba4b4bb71dfaf60fb50009").unwrap(),display_name:"Injected valid member".into(),description:String::new()}}else{EnumCommand::DescribeAsset("Injected valid description".into())};
  let modified=asset.prepare(command).unwrap().after().clone();envelope["current"][0]["wire"]=modified.encode().unwrap().into();let rewritten=serde_json::to_vec(&envelope).unwrap();assert_ne!(rewritten,original);
  let admitted=wire::decode_for1027(&rewritten,&f.program,7).unwrap();assert_eq!(admitted.asset(EnumAssetId::parse(A).unwrap()),Some(&modified),"rewrite retains valid nonce, program, asset/member IDs and codec closure");
  fs::write(f.root.join(SIDECAR),&rewritten).unwrap();let result=prepare_for_arguments(&f.root,&f.manifest,"C:/original-media",&f.program,vec![argument]);
  let error=match result{Err(error)=>error,Ok(_)=>panic!("first Bevy read adopted a different valid cohort")};assert!(error.contains("original App launch seal"),"{error}");
  fs::write(f.root.join(SIDECAR),&original).unwrap();assert!(f.prepare().unwrap().finish(&f.program).is_ok());
 }
}
