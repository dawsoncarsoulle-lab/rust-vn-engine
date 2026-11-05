//! Native typed Enum IO belongs to this real host loader, never to Core.
//! Read-only admission; no Source open/recovery, writer or publication grant.
use std::{fs,io::Read,path::{Path,PathBuf}};
use rvn_parser::{enum_asset_admission1025::EnumAssets1025,enum_asset_json::{EnumIndexInput,validate_enum_index,ENUM_MAX_FILE_BYTES}};
const MANIFEST_BYTES:usize=4*1024*1024;
const RESOURCE_BYTES:usize=64*1024*1024;
struct FrozenRead{path:PathBuf,bytes:Vec<u8>}
#[derive(Clone,Copy)]
enum ManifestReadPolicy1026{AbsentLegacy,IndexedCanonical}
// Preserve the existing project's manifest read policy when no Enum index is
// present. Bound the same ordinary path-following File read by captured bytes;
// descriptors and indexed manifests always keep the canonical reader below.
fn read_legacy_manifest1026(root:&Path,max:usize)->Result<Vec<u8>,String>{
 let limit=u64::try_from(max).ok().and_then(|n|n.checked_add(1)).ok_or("Enum manifest read budget cannot be represented")?;
 let mut bytes=Vec::new();fs::File::open(root.join("rvn.toml")).map_err(|e|e.to_string())?.take(limit).read_to_end(&mut bytes).map_err(|e|e.to_string())?;
 if bytes.len()>max{return Err("Enum manifest exceeds its captured byte budget".into());}Ok(bytes)
}
fn read_manifest1026(root:&Path,max:usize,policy:ManifestReadPolicy1026)->Result<Vec<u8>,String>{match policy{
 ManifestReadPolicy1026::AbsentLegacy=>read_legacy_manifest1026(root,max),
 ManifestReadPolicy1026::IndexedCanonical=>super::widget_runtime_loader_0951::read_inside(root,Path::new("rvn.toml"),max),
}}
pub struct PreparedEnumAssets1025{resources:EnumAssets1025,root:PathBuf,reads:Vec<FrozenRead>,manifest_policy:ManifestReadPolicy1026,journal_guard:bool}
impl PreparedEnumAssets1025{
 pub fn is_empty(&self)->bool{self.resources.is_empty()}
 pub fn finish(self)->Result<EnumAssets1025,String>{
  if self.journal_guard{super::widget_runtime_loader_0951::refuse_pending(&self.root)?;}
  for read in &self.reads{let actual=if read.path==Path::new("rvn.toml"){read_manifest1026(&self.root,read.bytes.len(),self.manifest_policy)?}else{super::widget_runtime_loader_0951::read_inside(&self.root,&read.path,ENUM_MAX_FILE_BYTES)?};
   if actual!=read.bytes{return Err("Enum index or resource bytes changed before Init; no runtime type was installed".into());}
  }Ok(self.resources)
 }
}
pub fn prepare(root:&Path,manifest:&str,assets:&str)->Result<PreparedEnumAssets1025,String>{
 let config:toml::Value=toml::from_str(manifest).map_err(|e|e.to_string())?;
 let value=config.get("enum_assets");
 let rows=value.and_then(toml::Value::as_table).map(|table|table.iter().map(|(key,value)|(key.as_str(),value.as_str())).collect::<Vec<_>>());
 let input=match(value,&rows){(None,_)=>EnumIndexInput::Absent,(Some(_),Some(rows))=>EnumIndexInput::Table(rows),(Some(_),None)=>EnumIndexInput::NotTable};
 let insensitive=cfg!(target_os="windows");let index=validate_enum_index(input,insensitive)?;
 if fs::canonicalize(root).map_err(|e|e.to_string())?!=root{return Err("Enum loader requires the actual canonical project root".into());}
 // Absence is part of the captured index, not a waiver of manifest identity.
 // Legacy no-Enum projects retain their existing assets-path/journal policy.
 let manifest_policy=if value.is_none(){ManifestReadPolicy1026::AbsentLegacy}else{ManifestReadPolicy1026::IndexedCanonical};
 let manifest_max=if value.is_some(){MANIFEST_BYTES}else{manifest.len()};
 if read_manifest1026(root,manifest_max,manifest_policy)?!=manifest.as_bytes(){return Err("Enum manifest bytes differ from the loader's actual parsed config".into());}
 if value.is_none(){return Ok(PreparedEnumAssets1025{resources:Default::default(),root:root.into(),reads:vec![FrozenRead{path:"rvn.toml".into(),bytes:manifest.as_bytes().to_vec()}],manifest_policy,journal_guard:false});}
 super::widget_runtime_loader_0951::refuse_pending(root)?;
 rvn_parser::widget_asset::validate_relative_locator(assets,None)?;
 let manifest_assets=config.get("paths").and_then(|paths|paths.get("assets")).map(|value|value.as_str().ok_or("Enum manifest assets path must be a string")).transpose()?.unwrap_or("assets");
 if manifest_assets!=assets{return Err("Enum assets path differs from the actual parsed manifest".into());}
 let mut reads=vec![FrozenRead{path:"rvn.toml".into(),bytes:manifest.as_bytes().to_vec()}];let mut files=Vec::with_capacity(index.len());let mut total=0usize;
 for locator in index.values(){let path=Path::new(assets).join(locator);let bytes=super::widget_runtime_loader_0951::read_inside(root,&path,ENUM_MAX_FILE_BYTES)?;
  total=total.checked_add(bytes.len()).filter(|total|*total<=RESOURCE_BYTES).ok_or("Enum resource closure exceeds the native64MiB budget")?;
  files.push((locator.clone(),bytes.clone()));reads.push(FrozenRead{path,bytes});
 }
 let rows=index.iter().map(|(id,path)|(id.as_str(),Some(path.as_str()))).collect::<Vec<_>>();
 let resources=rvn_graph::admit_enum_assets1025(EnumIndexInput::Table(&rows),&files,insensitive)?;
 Ok(PreparedEnumAssets1025{resources,root:root.into(),reads,manifest_policy,journal_guard:true})
}
/// CLI Check consumes the SAME physical loader as Run, including final fence.
pub fn check_project(root:&Path,assets:&str)->Result<(),String>{
 let root=fs::canonicalize(root).map_err(|e|e.to_string())?;
 // Match the existing legacy project reader without adding an Enum opt-out cap.
 let manifest_len=usize::try_from(fs::metadata(root.join("rvn.toml")).map_err(|e|e.to_string())?.len()).ok().filter(|len|*len<usize::MAX).ok_or("Enum manifest length cannot be represented by the bounded reader")?;
 // Presence is not known until actual bytes are parsed. prepare applies the
 // strict canonical reader again for every present table, including empty.
 let raw=read_legacy_manifest1026(&root,manifest_len)?;
 let manifest=std::str::from_utf8(&raw).map_err(|e|e.to_string())?;
 prepare(&root,manifest,assets)?.finish().map(|_|())
}
#[cfg(test)]
#[path="enum_runtime_loader_tests1025.rs"]
mod tests;
