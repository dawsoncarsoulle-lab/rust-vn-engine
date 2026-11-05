use super::*;
use std::sync::atomic::{AtomicU64,Ordering};
struct Fixture(PathBuf);
impl Fixture{fn new()->Self{static NEXT:AtomicU64=AtomicU64::new(0);let root=std::env::temp_dir().join(format!("rvn-cli-enum1025-{}-{}",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)));fs::create_dir(&root).unwrap();fs::create_dir(root.join("assets")).unwrap();fs::write(root.join("rvn.toml"),"[project]\nname='Enum check'\nmain_script='main.rvn'\nstart_label='start'\n[enum_assets]\n329f41eae6ba4b4bb71dfaf60fb50001='Alpha.rvnenum'\n").unwrap();fs::write(root.join("main.rvn"),"label start\n\"Check\"\n").unwrap();Self(root)}}
impl Drop for Fixture{fn drop(&mut self){assert!(self.0.starts_with(std::env::temp_dir()));assert!(self.0.file_name().unwrap().to_string_lossy().starts_with("rvn-cli-enum1025-"));fs::remove_dir_all(&self.0).unwrap();}}
#[test] fn enum_cli1025_actual_check_reports_missing_indexed_descriptor(){let f=Fixture::new();let report=check_project(f.0.to_str().unwrap(),CheckOptions::default());assert!(report.diagnostics.iter().any(|diagnostic|diagnostic.kind=="enum-assets"&&diagnostic.severity==Severity::Error));}
#[test] fn enum_cli1025_actual_check_accepts_valid_indexed_empty_enum_without_fabricating_members(){let f=Fixture::new();let asset=rvn_parser::enum_asset::EnumAsset::new(rvn_parser::enum_asset::EnumAssetId::parse("329f41eae6ba4b4bb71dfaf60fb50001").unwrap(),"Alpha".into()).unwrap();fs::write(f.0.join("assets/Alpha.rvnenum"),asset.encode().unwrap()).unwrap();let report=check_project(f.0.to_str().unwrap(),CheckOptions::default());assert!(!report.diagnostics.iter().any(|diagnostic|diagnostic.kind=="enum-assets"),"{report:?}");}

#[test] fn enum_cli1025_actual_legacy_large_manifest_without_catalogues_remains_valid(){
 let f=Fixture::new();let mut manifest=String::from("[project]\nname='Legacy large check'\ntitle='Legacy large check'\nmain_script='main.rvn'\nstart_label='start'\n[paths]\nassets='assets'\nlocales='locales'\nsaves='saves'\ntheme='theme.toml'\n#");
 manifest.push_str(&"x".repeat(4*1024*1024+1));manifest.push('\n');fs::write(f.0.join("rvn.toml"),&manifest).unwrap();
 fs::create_dir(f.0.join("locales")).unwrap();fs::create_dir(f.0.join("saves")).unwrap();fs::write(f.0.join("theme.toml"),"").unwrap();
 let current=fs::read_to_string(f.0.join("rvn.toml")).unwrap();let cfg:toml::Value=toml::from_str(&current).unwrap();assert!(current.len()>4*1024*1024&&cfg.get("enum_assets").is_none()&&cfg.get("widget_blueprints").is_none());
 let report=check_project(f.0.to_str().unwrap(),CheckOptions::default());assert!(!report.failed,"{report:?}");assert!(!report.diagnostics.iter().any(|diagnostic|diagnostic.kind=="enum-assets"),"{report:?}");
}

// Exercise the actual CLI diagnostic route; no synthetic report or skip-on-error.
#[cfg(unix)]
#[test] fn enum_cli1026_actual_absent_linked_manifest_is_legacy_but_present_empty_table_is_strict(){
 let f=Fixture::new();let manifest=String::from("[project]\nname='Legacy linked check'\ntitle='Legacy linked check'\nmain_script='main.rvn'\nstart_label='start'\n[paths]\nassets='assets'\nlocales='locales'\nsaves='saves'\ntheme='theme.toml'\n");
 fs::write(f.0.join("actual-manifest.toml"),&manifest).unwrap();fs::remove_file(f.0.join("rvn.toml")).unwrap();std::os::unix::fs::symlink("actual-manifest.toml",f.0.join("rvn.toml")).unwrap();
 fs::create_dir(f.0.join("locales")).unwrap();fs::create_dir(f.0.join("saves")).unwrap();fs::write(f.0.join("theme.toml"),"").unwrap();
 let report=check_project(f.0.to_str().unwrap(),CheckOptions::default());assert!(!report.failed,"{report:?}");assert!(!report.diagnostics.iter().any(|d|d.kind=="enum-assets"),"{report:?}");
 fs::write(f.0.join("actual-manifest.toml"),format!("{manifest}\n[enum_assets]\n")).unwrap();let report=check_project(f.0.to_str().unwrap(),CheckOptions::default());assert!(report.diagnostics.iter().any(|d|d.kind=="enum-assets"&&d.severity==Severity::Error),"{report:?}");
}
