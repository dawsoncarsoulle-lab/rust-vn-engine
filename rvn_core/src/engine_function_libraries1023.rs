//! Explicit Library bootstrap: actual parsed source + immutable strict asset
//! metadata are retained and validated BEFORE the existing Init constructor.
use super::*;
use rvn_parser::function_library_asset1023::FunctionLibraryAsset;
#[path="engine_library_startup1027.rs"]
pub(super) mod startup_joint1027;
/// Prepared code/resources only, not an editorial publication/Source owner or
/// host permission. Real editor callers retain their original Source/readset
/// guards; SDK callers already supply their own complete program/resources.
pub struct PreparedFunctionLibraryProgram1023{
    program:rvn_parser::ParsedProgram,
    functions:FunctionLibrary,
}
impl PreparedFunctionLibraryProgram1023{
    pub fn prepare(program:rvn_parser::ParsedProgram,assets:&[FunctionLibraryAsset])->Result<Self,RuntimeError>{
        if let Some(resolution)=&program.resolution{resolution.verify_for(&program).map_err(|error|RuntimeError::no_stmt(crate::error::RuntimeErrorKind::EvalError(EvalError::InvalidFunction(error)),0))?;}
        crate::editor_debugger::validate_program_origins(&program).map_err(|error|RuntimeError::no_stmt(crate::error::RuntimeErrorKind::EvalError(EvalError::InvalidFunction(error)),0))?;
        let functions=FunctionLibrary::from_script_with_library_assets(&program.script,assets).map_err(|error|RuntimeError::no_stmt(crate::error::RuntimeErrorKind::EvalError(error),0))?;
        Ok(Self{program,functions})
    }
    pub fn program(&self)->&rvn_parser::ParsedProgram{&self.program}
}
impl<R:Renderer> Engine<R>{
    pub fn from_function_library_program1023(prepared:PreparedFunctionLibraryProgram1023,renderer:R,rollback_depth:usize)->Result<Self,RuntimeError>{
        Self::from_program_bootstrap_libraries1023_inputs(prepared.program,renderer,rollback_depth,HashMap::new(),Default::default(),false,None,None,Some(prepared.functions))
    }
}
pub(super) fn story_identity1023(script:&Script,functions:&FunctionLibrary)->Result<String,EvalError>{
    let mut assets=functions.library_assets1023().iter().filter(|asset|!asset.members().is_empty()).collect::<Vec<_>>();
    if assets.is_empty(){return Ok(super::story_identity(script));}
    assets.sort_by(|a,b|a.id().cmp(b.id()));
    // Metadata affects private/result/call binding and therefore compatibility.
    // No byte-path or display-name alias is used for callable resolution.
    let bytes=serde_json::to_vec(&(script,assets)).map_err(|e|EvalError::InvalidFunction(e.to_string()))?;
    let hash=bytes.iter().fold(0xcbf29ce484222325u64,|hash,byte|(hash^u64::from(*byte)).wrapping_mul(0x100000001b3));
    Ok(format!("rvn-prepared-library-1:{hash:016x}:{}",bytes.len()))
}
