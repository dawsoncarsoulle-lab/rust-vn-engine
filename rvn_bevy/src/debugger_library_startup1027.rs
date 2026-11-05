//! Same authenticated broker/owner, now supplied the actual Library resources.
use super::*;
use rvn_core::enum_runtime_resources1025::EnumAssets1025;
use rvn_parser::function_library_asset1023::FunctionLibraryAsset;
pub(crate) fn prepare(root:&Path,program:rvn_parser::ParsedProgram,classes:rvn_core::widget_class_source_0950::AdmittedWidgetClasses,enums:EnumAssets1025,libraries:&[FunctionLibraryAsset],nonce:u64,
 persistent:std::collections::HashMap<String,rvn_parser::Value>,context:rvn_core::TextRuntimeContext,start_label:Option<String>,
)->Result<DebugBootstrap,String>{
 if nonce==0{return Err("Debugger startup requires its authenticated preview nonce".into())}
 let overlay=read_overlay(root)?.ok_or("Debug startup requires the actual captured compiler observation")?;
 let renderer=BevyRenderer::new();let broker=renderer.audio_checkpoints.clone();
 let owner=rvn_core::Engine::prepare_library_enum_input_startup1027(program,libraries,enums,classes,renderer,64,persistent,context,Some((overlay,nonce))).map_err(|e|e.to_string())?;
 if let Some(label)=&start_label{owner.validate_start_label(label).map_err(|e|e.to_string())?;}
 let audio_owner=StartupAudioOwner{instance:owner.snapshot().instance,gate:broker.gate()};
 Ok(DebugBootstrap{profile:BootstrapProfile::Debug,owner,start_label,broker,audio_owner,retirement_intent:None,publication_pending:false,failed:None,mode:None,sites:None,intent:0})
}
pub(crate) fn prepare_runtime(program:rvn_parser::ParsedProgram,classes:rvn_core::widget_class_source_0950::AdmittedWidgetClasses,enums:EnumAssets1025,libraries:&[FunctionLibraryAsset],
 persistent:std::collections::HashMap<String,rvn_parser::Value>,context:rvn_core::TextRuntimeContext,start_label:Option<String>,
)->Result<DebugBootstrap,String>{
 let renderer=BevyRenderer::new();let broker=renderer.audio_checkpoints.clone();
 let owner=rvn_core::Engine::prepare_library_enum_input_startup1027(program,libraries,enums,classes,renderer,64,persistent,context,None).map_err(|e|e.to_string())?;
 from_runtime_owner(owner,broker,start_label,BootstrapProfile::RuntimeInitial)
}
