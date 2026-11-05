//! Explicit disposable editor transport, independent of media paths/Saved IO.
use std::{path::{Path,PathBuf},fs};
use rvn_parser::{ParsedProgram,enum_asset_admission1025::EnumAssets1025};
use rvn_core::editor_enum_current1027::{self as wire,SIDECAR,MAX_TRANSPORT_BYTES};
pub(super) enum PreparedEnums1027{
    Disk(super::enum_runtime_loader1025::PreparedEnumAssets1025),
    Current{root:PathBuf,bytes:Vec<u8>,program:[u8;32],resources:EnumAssets1025},
}
impl PreparedEnums1027{
    pub(super) fn is_empty(&self)->bool{match self{Self::Disk(v)=>v.is_empty(),Self::Current{resources,..}=>resources.is_empty()}}
    pub(super) fn finish(self,program:&ParsedProgram)->Result<EnumAssets1025,String>{match self{
        Self::Disk(v)=>v.finish(),
        Self::Current{root,bytes,program:original,resources}=>{
            let actual=super::widget_runtime_loader_0951::read_inside(&root,Path::new(SIDECAR),MAX_TRANSPORT_BYTES)?;
            if actual!=bytes||wire::program_identity1027(program)?!=original{return Err("Current Enum launch bytes or program changed before owned Init".into());}Ok(resources)
        }
    }}
}
pub(super) fn prepare(root:&Path,manifest:&str,assets:&str,program:&ParsedProgram)->Result<PreparedEnums1027,String>{
    prepare_for_arguments(root,manifest,assets,program,std::env::args().skip(2).collect())
}
fn prepare_for_arguments(root:&Path,manifest:&str,assets:&str,program:&ParsedProgram,arguments:Vec<String>)->Result<PreparedEnums1027,String>{
    let seal=wire::launch_argument1027(arguments.iter().cloned())?;
    let Some(seal)=seal else{return super::enum_runtime_loader1025::prepare(root,manifest,assets).map(PreparedEnums1027::Disk)};
    let nonce=seal.nonce();
    // The launch discriminant never grants startup input. If the existing
    // authenticated embedded preview is present, both use its SAME nonce.
    for arg in &arguments{if let Some(value)=arg.strip_prefix("--editor-preview="){
        if super::editor_preview::Options::parse(value)?.nonce!=nonce{return Err("Current Enum transport belongs to a different embedded preview connection".into());}
    }}
    if fs::canonicalize(root).map_err(|e|e.to_string())?!=root{return Err("Current Enum loader requires the canonical disposable project root".into());}
    let bytes=super::widget_runtime_loader_0951::read_inside(root,Path::new(SIDECAR),MAX_TRANSPORT_BYTES)?;
    seal.validate_bytes(&bytes)?;
    let resources=wire::decode_for1027(&bytes,program,nonce)?;
    Ok(PreparedEnums1027::Current{root:root.into(),bytes,program:wire::program_identity1027(program)?,resources})
}
#[cfg(test)]
#[path="editor_enum_current_tests1027.rs"]
mod tests;
