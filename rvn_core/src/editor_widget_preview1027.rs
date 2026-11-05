//! Private editor transport of actual admitted Widget compiler inputs.
//! No instance, renderer, writer, registry, input grant or GraphId allocation.
use crate::{debugger_provenance::{SourceMap,Overlay},widget_class_source_0950::{AdmittedWidgetClasses,CompilerWidgetClass,CompilerWidgetMember,WidgetMemberKind}};
use rvn_parser::{ParsedProgram,StatementSpan,widget_asset::{self,WidgetIndexInput,LocatorCase}};
use serde::{Serialize,Deserialize};
use sha2::{Digest,Sha256};
use std::{path::{Path,Component},collections::{BTreeMap,BTreeSet}};
pub const SIDECAR:&str=".rvn-editor-widget-preview1027.json";
pub const ARGUMENT:&str="--editor-widget-preview1027=";
pub const MAX_BYTES:usize=64*1024*1024;
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct FileClaim{relative:String,bytes:[u8;32]}
#[derive(Clone,Debug,Serialize,Deserialize)]
enum Kind{Screen,Handler,Function}
impl Kind{fn native(&self)->WidgetMemberKind{match self{Self::Screen=>WidgetMemberKind::Screen,Self::Handler=>WidgetMemberKind::Handler,Self::Function=>WidgetMemberKind::Function}}}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct Span{start:usize,end:usize,children:Vec<Span>}
impl Span{
 fn capture(v:&StatementSpan)->Self{Self{start:v.range.start,end:v.range.end,children:v.children.iter().map(Self::capture).collect()}}
 fn matches(&self,v:&StatementSpan)->bool{self.start==v.range.start&&self.end==v.range.end&&self.children.len()==v.children.len()&&self.children.iter().zip(&v.children).all(|(a,b)|a.matches(b))}
}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct Member{graph:u64,kind:Kind,definition:String,root_index:usize,source_ordinal:usize,span:Span}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct Class{uuid:String,locator:String,descriptor:Vec<u8>,members:Vec<Member>}
impl Class{fn compiler(&self,root:&Path,program:&ParsedProgram,sources:&[FileClaim])->Result<CompilerWidgetClass,String>{
 let members=self.members.iter().map(|m|{
  let relative=&sources.get(m.source_ordinal).ok_or("Widget member original source ordinal is absent")?.relative;
  let origin=program.origins.get(m.root_index).ok_or("Widget member actual root origin is absent")?;
  if origin.source.as_ref()!=Some(&root.join(relative))||!m.span.matches(&origin.span){return Err("Widget member actual source/span differs from its captured origin".into());}
  Ok(CompilerWidgetMember{graph:m.graph,kind:m.kind.native(),definition:m.definition.clone(),root_index:m.root_index,origin:origin.clone()})
 }).collect::<Result<Vec<_>,String>>()?;
 Ok(CompilerWidgetClass{index_uuid:self.uuid.clone(),locator:self.locator.clone(),descriptor_bytes:self.descriptor.clone(),members})
}}
#[derive(Clone,Debug)]
pub struct CapturedWidgets1027{classes:Vec<Class>,sources:Vec<FileClaim>,map:SourceMap}
#[derive(Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope{version:u32,nonce:u64,manifest:[u8;32],sources:Vec<FileClaim>,overlay:String,classes:Vec<Class>}
fn digest(bytes:&[u8])->[u8;32]{Sha256::digest(bytes).into()}
fn source_claims(root:&Path,program:&ParsedProgram)->Result<Vec<FileClaim>,String>{
 if !root.is_absolute()||root.components().any(|c|matches!(c,Component::CurDir|Component::ParentDir)){return Err("Widget preview root must be normalized absolute".into());}
 let mut seen=BTreeSet::new();let mut result=Vec::new();
 for file in &program.sources{
  let path=file.path.as_ref().ok_or("Widget preview source has no physical origin")?;
  let relative=path.strip_prefix(root).map_err(|_|"Widget preview source is outside its original project")?.to_str().ok_or("Widget preview source is not UTF-8")?.replace('\\',"/");
  widget_asset::validate_relative_locator(&relative,Some("rvn"))?;
  let key=if cfg!(windows){relative.to_lowercase()}else{relative.clone()};
  if !seen.insert(key){return Err("Widget preview repeats an actual physical source; no first-match mapping is allowed".into());}
  result.push(FileClaim{relative,bytes:digest(file.source.as_bytes())});
 }Ok(result)
}
fn same_sources(root:&Path,program:&ParsedProgram,expected:&[FileClaim])->Result<(),String>{
 let actual=source_claims(root,program)?;
 if actual.len()!=expected.len()||actual.iter().zip(expected).any(|(a,b)|a.relative!=b.relative||a.bytes!=b.bytes){return Err("Widget preview source path/order/bytes differ from the original capture".into());}Ok(())
}
fn class_index(classes:&[Class])->Result<BTreeMap<String,String>,String>{
 let entries=classes.iter().map(|c|(c.uuid.as_str(),Some(c.locator.as_str()))).collect::<Vec<_>>();
 widget_asset::validate_index(WidgetIndexInput::Table(&entries),if cfg!(windows){LocatorCase::Insensitive}else{LocatorCase::Sensitive})
}
impl CapturedWidgets1027{
 /// The App adapter calls this only after real Graph/Source admission. Core
 /// independently checks every descriptor/member against the frozen program.
 pub fn from_compiler(root:&Path,program:&ParsedProgram,entries:Vec<CompilerWidgetClass>)->Result<Self,String>{
  let sources=source_claims(root,program)?;let mut classes=Vec::new();
  for c in entries{let mut members=Vec::new();for m in c.members{
   let matches=program.sources.iter().enumerate().filter(|(_,s)|s.path==m.origin.source).map(|(i,_)|i).collect::<Vec<_>>();
   let [source_ordinal]=matches.as_slice()else{return Err("Widget member must have one actual original source, not a content/name match".into());};
   members.push(Member{graph:m.graph,kind:match m.kind{WidgetMemberKind::Screen=>Kind::Screen,WidgetMemberKind::Handler=>Kind::Handler,WidgetMemberKind::Function=>Kind::Function},definition:m.definition,root_index:m.root_index,source_ordinal:*source_ordinal,span:Span::capture(&m.origin.span)});
  }classes.push(Class{uuid:c.index_uuid,locator:c.locator,descriptor:c.descriptor_bytes,members});}
  if classes.is_empty(){return Err("An explicit Widget preview capture needs actual admitted classes".into());}
  class_index(&classes)?;AdmittedWidgetClasses::admit(program,classes.iter().map(|c|c.compiler(root,program,&sources)).collect::<Result<Vec<_>,_>>()?)?;
  let map=SourceMap::from_program(program)?;Ok(Self{classes,sources,map})
 }
 pub fn index(&self)->Result<BTreeMap<String,String>,String>{class_index(&self.classes)}
 pub fn encode_for(&self,root:&Path,program:&ParsedProgram,manifest:&str,index:&BTreeMap<String,String>,nonce:u64)->Result<Vec<u8>,String>{
  if nonce==0{return Err("Widget preview nonce is absent".into());}same_sources(root,program,&self.sources)?;
  if *index!=self.index()?{return Err("Staged Widget index differs from its captured Current classes".into());}
  // The existing exact SourceMap relocation proves context, emitted program,
  // inclusion order, byte content, AST ranges/routes/sites. No GraphId comes
  // from these ordinals: every m.graph is the real opaque compiler member ID.
  let overlay=SourceMap::from_program(program)?.staged_overlay(&self.map,nonce)?.encode()?;
  let classes=self.classes.clone();
  AdmittedWidgetClasses::admit(program,classes.iter().map(|c|c.compiler(root,program,&self.sources)).collect::<Result<Vec<_>,_>>()?)?;
  let bytes=serde_json::to_vec(&Envelope{version:1,nonce,manifest:digest(manifest.as_bytes()),sources:self.sources.clone(),overlay,classes}).map_err(|e|e.to_string())?;
  if bytes.len()>MAX_BYTES{return Err("Widget preview transport exceeds its byte budget".into());}Ok(bytes)
 }
}
pub fn decode_for1027(bytes:&[u8],root:&Path,program:&ParsedProgram,manifest:&str,index:&BTreeMap<String,String>,nonce:u64)->Result<AdmittedWidgetClasses,String>{
 if nonce==0||bytes.len()>MAX_BYTES{return Err("Widget preview identity or byte budget is invalid".into());}
 let e:Envelope=serde_json::from_slice(bytes).map_err(|e|e.to_string())?;
 if e.version!=1||e.nonce!=nonce||e.manifest!=digest(manifest.as_bytes()){return Err("Widget preview belongs to another launch or original staged manifest".into());}
 same_sources(root,program,&e.sources)?;
 let actual=SourceMap::from_program(program)?;Overlay::decode(e.overlay.as_bytes())?.into_verified_map(&actual,nonce)?;
 if e.classes.is_empty()||*index!=class_index(&e.classes)?{return Err("Widget preview class set differs from the complete actual index".into());}
 AdmittedWidgetClasses::admit(program,e.classes.iter().map(|c|c.compiler(root,program,&e.sources)).collect::<Result<Vec<_>,_>>()?)
}
/// Reuse the existing independent canonical nonce:SHA256 byte seal; the
/// Widget argument has its own namespace and never grants startup input.
#[derive(Clone,Debug)]
pub struct LaunchSeal1027(crate::editor_enum_current1027::LaunchSeal1027);
impl LaunchSeal1027{
 pub fn for_bytes(nonce:u64,bytes:&[u8])->Result<Self,String>{if bytes.len()>MAX_BYTES{return Err("Widget preview byte seal budget".into());}crate::editor_enum_current1027::LaunchSeal1027::for_bytes(nonce,bytes).map(Self)}
 pub fn nonce(&self)->u64{self.0.nonce()}
 pub fn argument(&self)->String{self.0.argument().replacen(crate::editor_enum_current1027::ARGUMENT,ARGUMENT,1)}
 pub fn validate_bytes(&self,bytes:&[u8])->Result<(),String>{if bytes.len()>MAX_BYTES{return Err("Widget preview byte seal budget".into());}self.0.validate_bytes(bytes)}
}
pub fn launch_argument1027(arguments:impl IntoIterator<Item=String>)->Result<Option<LaunchSeal1027>,String>{
 let selected=arguments.into_iter().filter_map(|a|if a==ARGUMENT.trim_end_matches('='){Some(crate::editor_enum_current1027::ARGUMENT.trim_end_matches('=').to_owned())}else{a.strip_prefix(ARGUMENT).map(|v|format!("{}{v}",crate::editor_enum_current1027::ARGUMENT))}).collect::<Vec<_>>();
 crate::editor_enum_current1027::launch_argument1027(selected).map(|s|s.map(LaunchSeal1027))
}
#[cfg(test)]
#[path="editor_widget_preview_tests1027.rs"]
mod tests;
