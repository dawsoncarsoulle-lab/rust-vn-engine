//! Native selector for the original Current launch or the central Saved readset.
//! Explicit transport failures never adopt the Saved index or later bytes.
use std::{fs, path::{Path,PathBuf}};
use rvn_parser::{ParsedProgram,function_library_asset1023::FunctionLibraryAsset};
use rvn_core::editor_library_current1027::{self as wire,SIDECAR,MAX_TRANSPORT_BYTES};
pub(super) enum PreparedLibraries1027 {
    Disk{inputs:rvn_graph::SourceLibraryInputs1027,resources:Vec<FunctionLibraryAsset>},
    Current{root:PathBuf,bytes:Vec<u8>,program:[u8;32],resources:Vec<FunctionLibraryAsset>},
}
impl PreparedLibraries1027 {
    pub(super) fn is_empty(&self)->bool{match self{Self::Disk{resources,..}|Self::Current{resources,..}=>resources.is_empty()}}
    pub(super) fn finish(self,program:&ParsedProgram)->Result<Vec<FunctionLibraryAsset>,String>{
        let resources=match self {
            Self::Disk{inputs,resources}=>{inputs.validate_original()?;resources},
            Self::Current{root,bytes,program:original,resources}=>{
                let actual=super::widget_runtime_loader_0951::read_inside(&root,Path::new(SIDECAR),MAX_TRANSPORT_BYTES)?;
                if actual!=bytes||wire_program(program)?!=original{return Err("Current Library launch bytes or program changed before owned Init".into())}resources
            }
        };
        rvn_core::engine::PreparedFunctionLibraryProgram1023::prepare(program.clone(),&resources).map_err(|e|e.to_string())?;
        Ok(resources)
    }
}
fn wire_program(program:&ParsedProgram)->Result<[u8;32],String>{rvn_core::editor_enum_current1027::program_identity1027(program)}
pub(super) fn prepare(root:&Path,manifest:&str,program:&ParsedProgram)->Result<PreparedLibraries1027,String>{
    prepare_for_arguments(root,manifest,program,std::env::args().skip(2).collect())
}
fn prepare_for_arguments(root:&Path,manifest:&str,program:&ParsedProgram,arguments:Vec<String>)->Result<PreparedLibraries1027,String>{
    let Some(seal)=wire::launch_argument1027(arguments.iter().cloned())? else {
        // One authoritative reader: its no-index legacy policy and indexed
        // canonical/resource limits stay exactly those of Source admission.
        let inputs=rvn_graph::SourceLibraryInputs1027::read(root)?.ok_or("Loaded Library manifest disappeared before resource admission")?;
        let captured=inputs.source_references().iter().find(|f|f.path=="rvn.toml").and_then(|f|f.before.as_deref()).ok_or("Library admission did not retain the actual manifest observation")?;
        if captured!=manifest{return Err("Library resource manifest differs from the program loader's original bytes".into())}
        let resources=inputs.assets().to_vec();
        rvn_core::engine::PreparedFunctionLibraryProgram1023::prepare(program.clone(),&resources).map_err(|e|e.to_string())?;
        return Ok(PreparedLibraries1027::Disk{inputs,resources});
    };
    for argument in &arguments{if let Some(value)=argument.strip_prefix("--editor-preview="){
        if super::editor_preview::Options::parse(value)?.nonce!=seal.nonce(){return Err("Current Library transport belongs to a different embedded preview connection".into())}
    }}
    if fs::canonicalize(root).map_err(|e|e.to_string())?!=root{return Err("Current Library loader requires the canonical disposable project root".into())}
    let bytes=super::widget_runtime_loader_0951::read_inside(root,Path::new(SIDECAR),MAX_TRANSPORT_BYTES)?;
    seal.validate_bytes(&bytes)?;
    let resources=wire::decode_for1027(&bytes,program,seal.nonce())?;
    Ok(PreparedLibraries1027::Current{root:root.into(),bytes,program:wire_program(program)?,resources})
}
#[cfg(test)]
#[path="editor_library_current_tests1027.rs"]
mod tests;
