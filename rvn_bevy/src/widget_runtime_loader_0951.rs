//! Read-only native admission before the existing Engine/Startup is built.
//! Cached graph identity is presentation only; current frozen source owns every
//! definition. No SourceWorkspace::open/recovery, transaction or writer runs.
use std::{collections::BTreeSet,fs,io::Read,path::{Path,PathBuf}};
use rvn_core::widget_class_source_0950::{AdmittedWidgetClasses,CompilerWidgetClass,CompilerWidgetMember,WidgetMemberKind};
use rvn_parser::{ParsedProgram,widget_asset::{self,WidgetIndexInput,LocatorCase,WidgetBlueprintV1}};
const MAX_READ:usize=16*1024*1024;
const JOURNALS:[&str;2]=[".rvn-authoring.transaction.json",".rvn-refactor.transaction.json"];
struct FrozenRead{path:PathBuf,bytes:Vec<u8>}
pub(super) struct PreparedNativeClasses{classes:AdmittedWidgetClasses,root:PathBuf,reads:Vec<FrozenRead>,guard:bool}
impl PreparedNativeClasses{
 pub(super) fn classes(&self)->&AdmittedWidgetClasses{&self.classes}
 /// Last read-only admission fence before the real Init owner is constructed.
 pub(super) fn finish(self,program:&ParsedProgram)->Result<AdmittedWidgetClasses,String>{
  if self.guard{
   refuse_pending(&self.root)?;
   if !self.classes.matches_program(program)?{return Err("Widget program changed during native loading".into());}
   for source in &program.sources{let path=source.path.as_ref().ok_or("Widget program source path is absent")?;let rel=path.strip_prefix(&self.root).map_err(|_|"Widget source escapes the canonical project root")?;if read_inside(&self.root,rel,MAX_READ)?!=source.source.as_bytes(){return Err("Widget frozen source changed before Init".into());}}
   for read in &self.reads{if read_inside(&self.root,&read.path,MAX_READ)?!=read.bytes{return Err("Widget metadata changed before Init".into());}}
  }Ok(self.classes)
 }
}
pub(super) fn refuse_pending(root:&Path)->Result<(),String>{
 for journal in JOURNALS{if root.join(journal).try_exists().map_err(|e|e.to_string())?{return Err("Widget source has a pending authoring transaction; recover it in its existing owner before running".into());}}Ok(())
}
pub(super) fn read_inside(root:&Path,relative:&Path,max:usize)->Result<Vec<u8>,String>{
 if relative.is_absolute()||relative.components().any(|c|!matches!(c,std::path::Component::Normal(_))){return Err("Widget native path must remain project-relative".into());}
 let path=root.join(relative);let canonical=fs::canonicalize(&path).map_err(|e|e.to_string())?;
 if canonical!=path||!canonical.starts_with(root){return Err("Widget native path redirects outside its admitted canonical location".into());}
 let mut file=fs::File::open(&canonical).map_err(|e|e.to_string())?;if !file.metadata().map_err(|e|e.to_string())?.is_file(){return Err("Widget native source is not a regular file".into());}
 let mut bytes=Vec::new();file.by_ref().take((max+1) as u64).read_to_end(&mut bytes).map_err(|e|e.to_string())?;
 if bytes.len()>max{return Err("Widget native file exceeds its admission budget".into());}Ok(bytes)
}
fn core_classes(admission:&rvn_graph::PreparedWidgetClasses)->Result<AdmittedWidgetClasses,String>{
 let entries=admission.classes().values().map(|class|CompilerWidgetClass{index_uuid:class.uuid().into(),locator:class.locator().into(),descriptor_bytes:class.descriptor_bytes().to_vec(),members:class.members().iter().map(|member|CompilerWidgetMember{graph:member.graph().get(),kind:match member.kind(){rvn_graph::WidgetDefinitionKind::Screen=>WidgetMemberKind::Screen,rvn_graph::WidgetDefinitionKind::Handler=>WidgetMemberKind::Handler,rvn_graph::WidgetDefinitionKind::Function=>WidgetMemberKind::Function},definition:member.definition().into(),root_index:member.root_index(),origin:member.origin().clone()}).collect()}).collect();
 AdmittedWidgetClasses::admit(admission.program(),entries)
}
pub(super) fn prepare(root:&Path,manifest:&str,assets:&str,program:&ParsedProgram)->Result<PreparedNativeClasses,String>{
 let config:toml::Value=toml::from_str(manifest).map_err(|e|e.to_string())?;
 let value=config.get("widget_blueprints");let entries=value.and_then(toml::Value::as_table).map(|table|table.iter().map(|(key,value)|(key.as_str(),value.as_str())).collect::<Vec<_>>());
 let input=match(value,&entries){(None,_)=>WidgetIndexInput::Absent,(Some(_),Some(entries))=>WidgetIndexInput::Table(entries),(Some(_),None)=>WidgetIndexInput::NotTable};
 let case=if cfg!(target_os="windows"){LocatorCase::Insensitive}else{LocatorCase::Sensitive};
 let index=widget_asset::validate_index(input,case)?;
 if index.is_empty(){return Ok(PreparedNativeClasses{classes:Default::default(),root:root.into(),reads:vec![],guard:false});}
 if index.len()>256{return Err("Widget native class count exceeds its budget".into());}
 if fs::canonicalize(root).map_err(|e|e.to_string())?!=root{return Err("Widget loader requires its actual canonical project root".into());}
 refuse_pending(root)?;widget_asset::validate_relative_locator(assets,None)?;
 let mut reads=vec![FrozenRead{path:PathBuf::from("rvn.toml"),bytes:manifest.as_bytes().to_vec()}];
 // A linked sidecar preserves stable IDs only. Each model is rebuilt from the
 // current source, revalidated below against the loader's already frozen AST.
 let sidecar_path=Path::new(".rvn-authoring.json");
 let sidecar=if root.join(sidecar_path).try_exists().map_err(|e|e.to_string())?{
  let bytes=read_inside(root,sidecar_path,MAX_READ)?;let value:serde_json::Value=serde_json::from_slice(&bytes).map_err(|e|e.to_string())?;
  if value.get("version").and_then(serde_json::Value::as_u64)!=Some(1){return Err("Widget authoring presentation version is unsupported".into());}
  let source=value.get("source").and_then(serde_json::Value::as_str).ok_or("Widget authoring presentation has no source")?.to_owned();widget_asset::validate_relative_locator(&source,Some("rvn"))?;
  let snapshot=value.get("source_snapshot").and_then(serde_json::Value::as_str).ok_or("Widget authoring presentation has no source snapshot")?.to_owned();
  let graphs:Vec<rvn_graph::GraphDocument>=serde_json::from_value(value.get("graphs").ok_or("Widget authoring presentation has no graphs")?.clone()).map_err(|e|e.to_string())?;
  if graphs.len()>4096{return Err("Widget authoring presentation exceeds its graph budget".into());}
  reads.push(FrozenRead{path:sidecar_path.into(),bytes});Some((source,snapshot,graphs))
 }else{None};
 let mut descriptors=Vec::with_capacity(index.len());let mut sources=BTreeSet::new();let mut total=0usize;
 for locator in index.values(){let path=Path::new(assets).join(locator);let bytes=read_inside(root,&path,widget_asset::MAX_FILE_BYTES)?;total=total.checked_add(bytes.len()).filter(|total|*total<=4*1024*1024).ok_or("Widget descriptor set exceeds its total byte budget")?;let descriptor=WidgetBlueprintV1::decode(&bytes)?;sources.insert(descriptor.source);reads.push(FrozenRead{path,bytes:bytes.clone()});descriptors.push((locator,bytes));}
 let mut models=Vec::with_capacity(sources.len());
 for source in &sources{
  let path=root.join(source);let frozen=program.sources.iter().find(|item|item.path.as_ref()==Some(&path)).ok_or("Indexed Widget source is outside the actual frozen program closure")?;
  if read_inside(root,Path::new(source),MAX_READ)?!=frozen.source.as_bytes(){return Err("Widget source changed during native loading".into());}
  let presentation=if let Some((linked,snapshot,graphs))=&sidecar{if linked==source{if snapshot!=&frozen.source{return Err("Widget authoring presentation is stale for the actual source".into());}graphs.as_slice()}else{&[]}}else{&[]};
  models.push((source,rvn_graph::SourceProject::open_native_preparation_at(&path,frozen.source.clone(),presentation)?));
 }
 // A missing cache cannot donate freshly allocated legacy graph IDs.
 // Native identity is encoded in the actual source and must cover every
 // declared member; linked legacy identity comes only from its real sidecar.
 for(_,bytes)in &descriptors{
  let descriptor=WidgetBlueprintV1::decode(bytes)?;
  let has_presentation=sidecar.as_ref().is_some_and(|(source,_,_)|source==&descriptor.source);
  if !has_presentation{
   let(_,project)=models.iter().find(|(source,_)|source.as_str()==descriptor.source.as_str()).ok_or("Widget source model disappeared")?;
   for member in &descriptor.members{
    let graph=project.graphs().iter().find(|graph|graph.graph_id.get()==member.get()).ok_or("Widget has no authoritative source graph identity; reopen its linked authoring source")?;
    if graph.inspector_evaluation.is_legacy(){return Err("Legacy Widget graph identity requires its matching persisted authoring presentation; imports are not another editable source owner".into());}
   }
  }
 }
 let assets=descriptors.iter().map(|(locator,bytes)|rvn_graph::WidgetRuntimeAsset{locator,bytes}).collect::<Vec<_>>();
 let sources=models.iter().map(|(source,project)|rvn_graph::WidgetRuntimeSource{source,project}).collect::<Vec<_>>();
 let entries=index.iter().map(|(uuid,locator)|(uuid.as_str(),Some(locator.as_str()))).collect::<Vec<_>>();
 let admission=rvn_graph::prepare_widget_runtime_classes(root,program,WidgetIndexInput::Table(&entries),case,&assets,&sources)?;
 Ok(PreparedNativeClasses{classes:core_classes(&admission)?,root:root.into(),reads,guard:true})
}
#[cfg(test)]
#[path="widget_runtime_loader_tests_0951.rs"]
mod tests;
