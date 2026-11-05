use super::*;
use rvn_parser::enum_asset::{EnumAsset,EnumAssetId,EnumMemberId,EnumCommand};
use std::sync::atomic::{AtomicU64,Ordering};
const A:&str="329f41eae6ba4b4bb71dfaf60fb50001";const M:&str="229f41eae6ba4b4bb71dfaf60fb50001";
struct Fixture{root:PathBuf,manifest:String,asset:EnumAsset}
impl Fixture{
 fn new()->Self{static NEXT:AtomicU64=AtomicU64::new(0);let root=std::env::temp_dir().join(format!("rvn-enum-loader1025-{}-{}",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)));fs::create_dir(&root).unwrap();let root=root.canonicalize().unwrap();fs::create_dir(root.join("assets")).unwrap();let manifest=format!("[project]\nmain_script='main.rvn'\n[enum_assets]\n{A}='Alpha.rvnenum'\n");fs::write(root.join("rvn.toml"),&manifest).unwrap();let asset=EnumAsset::new(EnumAssetId::parse(A).unwrap(),"Alpha".into()).unwrap().prepare(EnumCommand::Add{member:EnumMemberId::parse(M).unwrap(),display_name:"First".into(),description:String::new()}).unwrap().after().clone();fs::write(root.join("assets/Alpha.rvnenum"),asset.encode().unwrap()).unwrap();Self{root,manifest,asset}}
 fn prepare(&self)->Result<PreparedEnumAssets1025,String>{prepare(&self.root,&self.manifest,"assets")}
}
impl Drop for Fixture{fn drop(&mut self){assert!(self.root.starts_with(std::env::temp_dir().canonicalize().unwrap()));assert!(self.root.file_name().unwrap().to_string_lossy().starts_with("rvn-enum-loader1025-"));fs::remove_dir_all(&self.root).unwrap();}}
#[test] fn enum_loader1025_real_manifest_file_closure_admits_nominal_value(){let f=Fixture::new();let assets=f.prepare().unwrap().finish().unwrap();assert_eq!(assets.value(f.asset.id(),EnumMemberId::parse(M).unwrap()).unwrap(),f.asset.value(EnumMemberId::parse(M).unwrap()).unwrap());check_project(&f.root,"assets").unwrap();}
#[test] fn enum_loader1025_wrong_uuid_and_missing_file_refuse(){let f=Fixture::new();fs::write(f.root.join("assets/Alpha.rvnenum"),EnumAsset::new(EnumAssetId::parse("329f41eae6ba4b4bb71dfaf60fb50002").unwrap(),"Alpha".into()).unwrap().encode().unwrap()).unwrap();assert!(f.prepare().is_err());fs::remove_file(f.root.join("assets/Alpha.rvnenum")).unwrap();assert!(f.prepare().is_err());}
#[test] fn enum_loader1025_manifest_mutation_between_prepare_and_init_refuses(){let f=Fixture::new();let prepared=f.prepare().unwrap();fs::write(f.root.join("rvn.toml"),format!("{}#external edit\n",f.manifest)).unwrap();assert!(prepared.finish().is_err());}
#[test] fn enum_loader1025_valid_descriptor_late_edit_refuses_but_new_capture_keeps_identity(){let f=Fixture::new();let prepared=f.prepare().unwrap();let changed=f.asset.prepare(EnumCommand::RenameMember{member:EnumMemberId::parse(M).unwrap(),display_name:"Renamed".into()}).unwrap().after().clone();fs::write(f.root.join("assets/Alpha.rvnenum"),changed.encode().unwrap()).unwrap();assert!(prepared.finish().is_err());let admitted=f.prepare().unwrap().finish().unwrap();assert_eq!(admitted.value(f.asset.id(),EnumMemberId::parse(M).unwrap()).unwrap(),f.asset.value(EnumMemberId::parse(M).unwrap()).unwrap());}
#[test] fn enum_loader1025_pending_authoring_journal_is_not_runtime_authority(){let f=Fixture::new();fs::write(f.root.join(".rvn-refactor.transaction.json"),"{}").unwrap();assert!(f.prepare().is_err());}
#[test] fn enum_loader1025_locator_escape_and_config_path_mismatch_refuse(){let mut f=Fixture::new();assert!(prepare(&f.root,&f.manifest,"other").is_err());f.manifest=f.manifest.replace("Alpha.rvnenum","../Alpha.rvnenum");fs::write(f.root.join("rvn.toml"),&f.manifest).unwrap();assert!(f.prepare().is_err());}

#[test] fn enum_loader1025_absent_index_becoming_present_before_init_refuses(){let f=Fixture::new();let absent="[project]\nmain_script='main.rvn'\n";fs::write(f.root.join("rvn.toml"),absent).unwrap();let prepared=prepare(&f.root,absent,"assets").unwrap();assert!(prepared.is_empty());fs::write(f.root.join("rvn.toml"),&f.manifest).unwrap();assert!(prepared.finish().is_err());}
#[test] fn enum_loader1025_stable_absence_keeps_legacy_assets_and_journal_policy_without_grant(){let f=Fixture::new();let absent="[project]\nmain_script='main.rvn'\n";fs::write(f.root.join("rvn.toml"),absent).unwrap();fs::write(f.root.join(".rvn-refactor.transaction.json"),"{}").unwrap();let prepared=prepare(&f.root,absent,"legacy/../assets").unwrap();let resources=prepared.finish().unwrap();assert!(resources.is_empty());assert!(resources.value(f.asset.id(),EnumMemberId::parse(M).unwrap()).is_err());}

#[test] fn enum_loader1025_large_legacy_manifest_without_catalogues_uses_actual_project_loader(){
 let f=Fixture::new();
 let mut manifest=String::from("[project]\nname='Legacy large manifest'\ntitle='Legacy large manifest'\nmain_script='main.rvn'\nstart_label='start'\n[paths]\nassets='assets'\nlocales='locales'\nsaves='saves'\ntheme='theme.toml'\n#");
 manifest.push_str(&"x".repeat(MANIFEST_BYTES+1));manifest.push('\n');
 fs::write(f.root.join("rvn.toml"),&manifest).unwrap();
 fs::write(f.root.join("main.rvn"),"label start\n\"Legacy actual project\"\n").unwrap();
 fs::create_dir(f.root.join("locales")).unwrap();fs::write(f.root.join("locales/fr.toml"),"[strings]\n").unwrap();
 fs::write(f.root.join("theme.toml"),"[title_screen]\nenabled=false\n").unwrap();
 let actual=fs::read_to_string(f.root.join("rvn.toml")).unwrap();assert!(actual.len()>MANIFEST_BYTES);
 let config:toml::Value=toml::from_str(&actual).unwrap();assert!(config.get("enum_assets").is_none()&&config.get("widget_blueprints").is_none());
 let resources=prepare(&f.root,&actual,"assets").unwrap().finish().unwrap();assert!(resources.is_empty());assert!(resources.value(f.asset.id(),EnumMemberId::parse(M).unwrap()).is_err());
 check_project(&f.root,"assets").unwrap();
 // Real current manifest, real source program and actual production loader/owner.
 let launch=crate::load_game(&f.root).unwrap();assert_eq!(launch.cfg.project.main_script,"main.rvn");drop(launch);
 // Explicit table presence keeps the original indexed4MiB budget, including empty.
 for suffix in [String::from("\n[enum_assets]\n"),format!("\n[enum_assets]\n{A}='Alpha.rvnenum'\n")]{
  fs::write(f.root.join("rvn.toml"),format!("{actual}{suffix}")).unwrap();let indexed=fs::read_to_string(f.root.join("rvn.toml")).unwrap();
  assert!(prepare(&f.root,&indexed,"assets").err().unwrap().contains("budget"));assert!(check_project(&f.root,"assets").unwrap_err().contains("budget"));
 }
}
#[test] fn enum_loader1025_large_legacy_manifest_late_bytes_and_index_change_refuse(){
 let f=Fixture::new();let mut manifest=String::from("[project]\ntitle='Legacy fence'\nmain_script='main.rvn'\n#");manifest.push_str(&"x".repeat(MANIFEST_BYTES+1));manifest.push('\n');
 fs::write(f.root.join("rvn.toml"),&manifest).unwrap();let actual=fs::read_to_string(f.root.join("rvn.toml")).unwrap();let prepared=prepare(&f.root,&actual,"assets").unwrap();
 fs::write(f.root.join("rvn.toml"),format!("{actual}#late current bytes\n")).unwrap();assert!(prepared.finish().is_err());
 let current=fs::read_to_string(f.root.join("rvn.toml")).unwrap();let prepared=prepare(&f.root,&current,"assets").unwrap();
 fs::write(f.root.join("rvn.toml"),format!("{current}\n[enum_assets]\n{A}='Alpha.rvnenum'\n")).unwrap();assert!(prepared.finish().is_err());
}

// Actual symlink IO requires a supported filesystem/OS privilege. These Unix
// tests are intentionally not claimed as Windows executions or silently skipped.
#[cfg(unix)]
fn linked_legacy_manifest1026(f:&Fixture)->String{
 let manifest=String::from("[project]\nname='Legacy linked manifest'\ntitle='Legacy linked manifest'\nmain_script='main.rvn'\nstart_label='start'\n[paths]\nassets='assets'\nlocales='locales'\nsaves='saves'\ntheme='theme.toml'\n");
 fs::write(f.root.join("actual-manifest.toml"),&manifest).unwrap();fs::remove_file(f.root.join("rvn.toml")).unwrap();std::os::unix::fs::symlink("actual-manifest.toml",f.root.join("rvn.toml")).unwrap();
 fs::write(f.root.join("main.rvn"),"label start\n\"Legacy linked source\"\n").unwrap();fs::create_dir(f.root.join("locales")).unwrap();fs::write(f.root.join("locales/fr.toml"),"[strings]\n").unwrap();fs::write(f.root.join("theme.toml"),"[title_screen]\nenabled=false\n").unwrap();manifest
}
#[cfg(unix)]
#[test] fn enum_loader1026_absent_linked_manifest_preserves_real_check_run_and_final_bytes_presence_fences(){
 let f=Fixture::new();let manifest=linked_legacy_manifest1026(&f);let resources=prepare(&f.root,&manifest,"assets").unwrap().finish().unwrap();assert!(resources.is_empty());assert!(resources.value(f.asset.id(),EnumMemberId::parse(M).unwrap()).is_err());
 check_project(&f.root,"assets").unwrap();let launch=crate::load_game(&f.root).unwrap();assert_eq!(launch.cfg.project.main_script,"main.rvn");drop(launch);
 let prepared=prepare(&f.root,&manifest,"assets").unwrap();fs::write(f.root.join("actual-manifest.toml"),format!("{manifest}#late actual target bytes\n")).unwrap();assert!(prepared.finish().is_err());
 let current=fs::read_to_string(f.root.join("rvn.toml")).unwrap();let prepared=prepare(&f.root,&current,"assets").unwrap();fs::write(f.root.join("actual-manifest.toml"),format!("{current}\n[enum_assets]\n{A}='Alpha.rvnenum'\n")).unwrap();assert!(prepared.finish().is_err());
}
#[cfg(unix)]
#[test] fn enum_loader1026_present_index_even_empty_keeps_linked_manifest_refusal(){
 for suffix in [String::from("\n[enum_assets]\n"),format!("\n[enum_assets]\n{A}='Alpha.rvnenum'\n")]{
  let f=Fixture::new();let absent=linked_legacy_manifest1026(&f);let manifest=format!("{absent}{suffix}");fs::write(f.root.join("actual-manifest.toml"),&manifest).unwrap();
  assert!(prepare(&f.root,&manifest,"assets").err().unwrap().contains("redirects"));assert!(check_project(&f.root,"assets").unwrap_err().contains("redirects"));
 }
}
