//! Resource input on the existing source-owned startup. Never replay Init or
//! synthesize a host capability; the input-capable port retains its real gate.
use super::*;
use crate::enum_runtime_resources1025::EnumAssets1025;
use crate::widget_class_source_0950::AdmittedWidgetClasses;
impl<R:Renderer> Engine<R>{
 pub fn enum_assets1025(&self)->&EnumAssets1025{&self.functions.enum_assets1025}
 pub fn prepare_enum_startup1025(program:rvn_parser::ParsedProgram,enums:EnumAssets1025,classes:AdmittedWidgetClasses,
  renderer:R,rollback_depth:usize,persistent:HashMap<String,Value>,text:TextRuntimeContext,
  overlay:Option<(crate::debugger_provenance::Overlay,u64)>,
 )->Result<EngineStartup<R>,RuntimeError>{
  if !classes.matches_program(&program).map_err(|message|RuntimeError::no_stmt(crate::error::RuntimeErrorKind::EvalError(EvalError::InvalidFunction(message)),0))?{
   return Err(RuntimeError::no_stmt(crate::error::RuntimeErrorKind::EvalError(EvalError::InvalidFunction("Widget classes differ from actual Enum startup program".into())),0));
  }
  Self::prepare_owned_startup_enum_in1025(std::sync::Arc::new(program),renderer,rollback_depth,persistent,text,overlay,(0,0,0),None,std::sync::Arc::new(enums))?.with_widget_classes(std::sync::Arc::new(classes))
 }
 pub fn from_program_with_enum_assets1025(program:rvn_parser::ParsedProgram,enums:EnumAssets1025,classes:AdmittedWidgetClasses,
  renderer:R,rollback_depth:usize,persistent:HashMap<String,Value>,text:TextRuntimeContext,
 )->Result<Self,RuntimeError>{Self::prepare_enum_startup1025(program,enums,classes,renderer,rollback_depth,persistent,text,None)?.finish_synchronous()}
}
impl<R:StartupInputBatchSink> Engine<R>{
 pub fn prepare_enum_input_startup1025(program:rvn_parser::ParsedProgram,enums:EnumAssets1025,classes:AdmittedWidgetClasses,
  renderer:R,rollback_depth:usize,persistent:HashMap<String,Value>,text:TextRuntimeContext,
  overlay:Option<(crate::debugger_provenance::Overlay,u64)>,
 )->Result<EngineStartup<R>,RuntimeError>{
  if !classes.matches_program(&program).map_err(|message|RuntimeError::no_stmt(crate::error::RuntimeErrorKind::EvalError(EvalError::InvalidFunction(message)),0))?{
   return Err(RuntimeError::no_stmt(crate::error::RuntimeErrorKind::EvalError(EvalError::InvalidFunction("Widget classes differ from actual Enum input startup program".into())),0));
  }
  renderer.validate_startup_input_host().map_err(|message|RuntimeError::no_stmt(crate::error::RuntimeErrorKind::EvalError(EvalError::InvalidFunction(message)),0))?;
  let program=std::sync::Arc::new(program);
  let input=crate::consumer_schedule::StartupInputProgram::from_frozen(program.clone()).map_err(|error|RuntimeError::no_stmt(crate::error::RuntimeErrorKind::EvalError(error),0))?;
  Self::prepare_owned_startup_enum_in1025(program,renderer,rollback_depth,persistent,text,overlay,(0,0,0),Some(std::sync::Arc::new(input)),std::sync::Arc::new(enums))?.with_widget_classes(std::sync::Arc::new(classes))
 }
}
#[cfg(test)]
#[path="engine_enum_startup_tests1025.rs"]
mod tests;
