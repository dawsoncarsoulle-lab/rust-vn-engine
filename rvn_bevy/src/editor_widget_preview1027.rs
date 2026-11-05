//! Explicit sealed editor classes; the ordinary disk loader remains strict.
use std::{path::{Path,PathBuf},fs,collections::BTreeMap};
use rvn_parser::{ParsedProgram,widget_asset::{self,WidgetIndexInput,LocatorCase}};
use rvn_core::{editor_widget_preview1027 as wire,widget_class_source_0950::AdmittedWidgetClasses};
pub(super) enum PreparedWidgets1027{
 Disk(super::widget_runtime_loader_0951::PreparedNativeClasses),
 Captured{root:PathBuf,bytes:Vec<u8>,manifest:String,classes:AdmittedWidgetClasses},
}
impl PreparedWidgets1027{
 pub(super) fn classes(&self)->&AdmittedWidgetClasses{match self{Self::Disk(v)=>v.classes(),Self::Captured{classes,..}=>classes}}
 pub(super) fn finish(self,program:&ParsedProgram)->Result<AdmittedWidgetClasses,String>{match self{
  Self::Disk(v)=>v.finish(program),Self::Captured{root,bytes,manifest,classes}=>{
   super::widget_runtime_loader_0951::refuse_pending(&root)?;
   if super::widget_runtime_loader_0951::read_inside(&root,Path::new(wire::SIDECAR),wire::MAX_BYTES)?!=bytes||super::widget_runtime_loader_0951::read_inside(&root,Path::new("rvn.toml"),16*1024*1024)?!=manifest.as_bytes(){return Err("Original Widget preview transport or manifest changed before Init".into());}
   if !classes.matches_program(program)?{return Err("Widget preview program changed before Init".into());}
   for source in &program.sources{
    let path=source.path.as_ref().ok_or("Widget preview source origin is absent")?;
    let relative=path.strip_prefix(&root).map_err(|_|"Widget preview source escapes its disposable root")?;
    if super::widget_runtime_loader_0951::read_inside(&root,relative,16*1024*1024)?!=source.source.as_bytes(){return Err("Widget preview source bytes changed before Init".into());}
   }Ok(classes)
  }
 }}
}
fn index(manifest:&str)->Result<BTreeMap<String,String>,String>{
 let config:toml::Value=toml::from_str(manifest).map_err(|e|e.to_string())?;let value=config.get("widget_blueprints");
 let entries=value.and_then(toml::Value::as_table).map(|t|t.iter().map(|(k,v)|(k.as_str(),v.as_str())).collect::<Vec<_>>());
 let input=match(value,&entries){(None,_)=>WidgetIndexInput::Absent,(Some(_),Some(v))=>WidgetIndexInput::Table(v),(Some(_),None)=>WidgetIndexInput::NotTable};
 widget_asset::validate_index(input,if cfg!(windows){LocatorCase::Insensitive}else{LocatorCase::Sensitive})
}
pub(super) fn prepare(root:&Path,manifest:&str,assets:&str,program:&ParsedProgram)->Result<PreparedWidgets1027,String>{prepare_for_arguments(root,manifest,assets,program,std::env::args().skip(2).collect())}
fn prepare_for_arguments(root:&Path,manifest:&str,assets:&str,program:&ParsedProgram,arguments:Vec<String>)->Result<PreparedWidgets1027,String>{
 let Some(seal)=wire::launch_argument1027(arguments.iter().cloned())?else{return super::widget_runtime_loader_0951::prepare(root,manifest,assets,program).map(PreparedWidgets1027::Disk)};
 if fs::canonicalize(root).map_err(|e|e.to_string())?!=root{return Err("Widget preview requires its canonical disposable root".into());}
 super::widget_runtime_loader_0951::refuse_pending(root)?;
 if let Some(nonce)=rvn_core::editor_enum_current1027::nonce_argument1027(arguments.iter().cloned())?{if nonce!=seal.nonce(){return Err("Widget/Enum preview captures belong to different launches".into());}}
 for argument in &arguments{if let Some(value)=argument.strip_prefix("--editor-preview="){if super::editor_preview::Options::parse(value)?.nonce!=seal.nonce(){return Err("Widget classes belong to another authenticated preview connection".into());}}}
 let bytes=super::widget_runtime_loader_0951::read_inside(root,Path::new(wire::SIDECAR),wire::MAX_BYTES)?;
 seal.validate_bytes(&bytes)?; // Independent original App bytes, BEFORE decode.
 let classes=wire::decode_for1027(&bytes,root,program,manifest,&index(manifest)?,seal.nonce())?;
 Ok(PreparedWidgets1027::Captured{root:root.into(),bytes,manifest:manifest.into(),classes})
}
#[cfg(test)]
#[path="editor_widget_preview_tests1027.rs"]
mod tests;
