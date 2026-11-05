//! Compiler/loader bridge. Only opaque, fully admitted Graph classes enter
//! this adapter; no Outliner row, child shape or cached label selects a class.
use rvn_core::{Engine,Renderer,RuntimeError};
use rvn_core::widget_class_source_0950::{AdmittedWidgetClasses,CompilerWidgetClass,CompilerWidgetMember,WidgetMemberKind};

pub(crate) fn core_classes(admission:&rvn_graph::PreparedWidgetClasses)->Result<AdmittedWidgetClasses,String>{
 let entries=admission.classes().values().map(|class|CompilerWidgetClass{
  index_uuid:class.uuid().into(),locator:class.locator().into(),descriptor_bytes:class.descriptor_bytes().to_vec(),
  members:class.members().iter().map(|member|CompilerWidgetMember{graph:member.graph().get(),kind:match member.kind(){rvn_graph::WidgetDefinitionKind::Screen=>WidgetMemberKind::Screen,rvn_graph::WidgetDefinitionKind::Handler=>WidgetMemberKind::Handler,rvn_graph::WidgetDefinitionKind::Function=>WidgetMemberKind::Function},definition:member.definition().into(),root_index:member.root_index(),origin:member.origin().clone()}).collect(),
 }).collect();
 AdmittedWidgetClasses::admit(admission.program(),entries)
}
/// Uses the real existing source-owned Startup pipeline; callers may drive
/// media waits/debugger handoff rather than replaying Init in another VM.
pub(crate) fn prepare_engine<R:Renderer>(admission:&rvn_graph::PreparedWidgetClasses,renderer:R,rollback_depth:usize,persistent:std::collections::HashMap<String,rvn_parser::Value>,text_context:rvn_core::TextRuntimeContext)->Result<rvn_core::engine::EngineStartup<R>,RuntimeError>{
 let classes=core_classes(admission).map_err(|message|RuntimeError::no_stmt(rvn_core::error::RuntimeErrorKind::EvalError(rvn_core::EvalError::InvalidFunction(message)),0))?;
 Engine::prepare_widget_class_startup(admission.program().clone(),classes,renderer,rollback_depth,persistent,text_context)
}
pub(crate) fn create_engine<R:Renderer>(admission:&rvn_graph::PreparedWidgetClasses,renderer:R,rollback_depth:usize)->Result<Engine<R>,RuntimeError>{
 let classes=core_classes(admission).map_err(|message|RuntimeError::no_stmt(rvn_core::error::RuntimeErrorKind::EvalError(rvn_core::EvalError::InvalidFunction(message)),0))?;
 Engine::from_program_with_widget_classes(admission.program().clone(),classes,renderer,rollback_depth)
}

#[cfg(test)]
#[path="widget_runtime_loader_tests_0950.rs"]
mod tests;
