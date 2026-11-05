//! Preview-only source map. Its digest binds frozen RVN bytes and the exact
//! emitted programme; graph names never arrive through a debugger command.
use crate::{editor_debugger::{self as wire,DebugOrigin},program::{InstructionOrigin,AstRoutePart}};
use rvn_parser::{ParsedProgram,Statement,StatementSpan};
use serde::{Serialize,Deserialize};
use sha2::{Digest,Sha256};
use std::fmt::Write;
pub const MAX_BYTES:usize=16*1024*1024;
pub const SIDECAR:&str=".rvn-editor-debugger.json";
struct Counter{hash:Sha256,bytes:usize}
impl std::fmt::Write for Counter{
 fn write_str(&mut self,text:&str)->std::fmt::Result{self.bytes=self.bytes.checked_add(text.len()).filter(|n|*n<=MAX_BYTES).ok_or(std::fmt::Error)?;self.hash.update(text.as_bytes());Ok(())}
}
fn digest(value:&impl std::fmt::Debug)->Result<[u8;32],String>{let mut counter=Counter{hash:Sha256::new(),bytes:0};write!(&mut counter,"{value:?}").map_err(|_|"Debugger frozen programme budget")?;Ok(counter.hash.finalize().into())}

#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceMap{
 pub emitted:[u8;32],pub closure:[u8;32],pub content:[u8;32],
 pub origins:Vec<InstructionOrigin>,pub sites:Vec<DebugOrigin>,
}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Overlay{nonce:u64,map:SourceMap}
impl SourceMap{
 /// Existing Debug digest, shared with prepared Native admission. No new
 /// identity format or registry is introduced.
 pub(crate) fn emitted_identity(script:&[Statement])->Result<[u8;32],String>{digest(&script)}
 /// Endpoint selection is observation-only. Breakpoints still come from the
 /// actual Engine catalogue and its source/owner admission.
 pub fn producer_sites_for_endpoint(&self,source_ordinal:usize,graph:u64,node:u64,pin:u64)->impl Iterator<Item=&DebugOrigin>{
  self.sites.iter().filter(move|site|site.origin.scheduled_step.as_ref().is_some_and(|step|
   step.source_ordinal==source_ordinal&&step.endpoint.as_ref().is_some_and(|endpoint|
    endpoint.graph==graph&&endpoint.node==node&&endpoint.ports.iter().any(|port|port.pin==pin&&endpoint.legacy_terminal.as_ref().is_none_or(|role|role==&port.role)))))
 }
 pub fn from_program(program:&ParsedProgram)->Result<Self,String>{
  // Count the entire borrowed AST/source metadata before its preparation or
  // copies. The ordinary parser is unchanged; this is the debug opt-in bound.
  let closure=digest(program)?;wire::validate_program_origins(program)?;
  // Paths are allowed to move into the disposable project; every original
  // UTF-8 byte, inclusion order and root-to-file association must remain exact.
  let source_bytes=program.sources.iter().map(|source|source.source.as_str()).collect::<Vec<_>>();
  let files=program.origins.iter().map(|origin|program.sources.iter().position(|source|source.path==origin.source).ok_or("Debugger origin has no frozen source")).collect::<Result<Vec<_>,_>>()?;
  let content=digest(&(program.context,source_bytes,files))?;
  let sites=crate::debugger_producers::collect(program)?;
  let (script,origins)=if program.context==rvn_parser::NarrativeContext::ExplicitScenes{
   let prepared=crate::program::PreparedNarrative::from_parsed(program.clone()).map_err(|e|e.to_string())?;(prepared.script,prepared.origins)
  }else{legacy_origins(program)?};
  let emitted=Self::emitted_identity(&script)?;
  let map=Self{emitted,closure,content,origins,sites};digest(&map)?;Ok(map)
 }
 /// Move only compiler graph identities onto the byte-checked staged map.
 /// Narrative ResumePosition/source identities themselves are never changed.
 pub fn staged_overlay(mut self,captured:&Self,nonce:u64)->Result<Overlay,String>{
  if nonce==0||self.content!=captured.content||self.emitted!=captured.emitted||self.origins.len()!=captured.origins.len()||self.sites.len()!=captured.sites.len(){return Err("Staged debugger programme differs from its compiler snapshot".into());}
  for (actual,author) in self.origins.iter_mut().zip(&captured.origins){
   if actual.span!=author.span||actual.route!=author.route||actual.generated!=author.generated{return Err("Staged debugger instruction range changed".into());}
   actual.graph=author.graph.clone();
  }
  for(actual,author)in self.sites.iter_mut().zip(&captured.sites){
   if actual.scope!=author.scope||actual.route!=author.route||actual.origin.span!=author.origin.span||actual.origin.route!=author.origin.route||actual.origin.scheduled_step!=author.origin.scheduled_step
    ||actual.declaration.span!=author.declaration.span||actual.declaration.route!=author.declaration.route{return Err("Staged debugger AST site changed".into());}
   actual.origin.graph=author.origin.graph.clone();actual.declaration.graph=author.declaration.graph.clone();
  }
  digest(&self)?;Ok(Overlay{nonce,map:self})
 }
}
impl Overlay{
 pub fn encode(&self)->Result<String,String>{
  let mut counter=JsonCount(0);serde_json::to_writer(&mut counter,self).map_err(|e|e.to_string())?;
  serde_json::to_string(self).map_err(|e|e.to_string())
 }
 pub fn decode(bytes:&[u8])->Result<Self,String>{
  if bytes.len()>MAX_BYTES{return Err("Debugger sidecar budget".into());}
  let overlay:Self=serde_json::from_slice(bytes).map_err(|e|e.to_string())?;
  if overlay.nonce==0||overlay.map.origins.len()>100_000||overlay.map.sites.len()>100_000{return Err("Debugger sidecar shape budget".into());}
  for origin in overlay.map.origins.iter().chain(overlay.map.sites.iter().flat_map(|site|[&site.origin,&site.declaration])){
   if origin.route.len()>256||origin.graph.as_ref().is_some_and(|graph|graph.graph==0||graph.node==Some(0)||graph.source.as_ref().is_none_or(|path|path.as_os_str().len()>4096)){return Err("Invalid compiler graph source identity".into());}
  }
  Ok(overlay)
 }
}
struct JsonCount(usize);
impl std::io::Write for JsonCount{
 fn write(&mut self,value:&[u8])->std::io::Result<usize>{self.0=self.0.checked_add(value.len()).filter(|n|*n<=MAX_BYTES).ok_or_else(||std::io::Error::new(std::io::ErrorKind::InvalidData,"Debugger sidecar budget"))?;Ok(value.len())}
 fn flush(&mut self)->std::io::Result<()>{Ok(())}
}

impl Overlay{
 pub(crate) fn into_verified_map(self,actual:&SourceMap,nonce:u64)->Result<SourceMap,String>{
  if nonce==0||self.nonce!=nonce||self.map.closure!=actual.closure||self.map.content!=actual.content||self.map.emitted!=actual.emitted||self.map.origins.len()!=actual.origins.len()||self.map.sites.len()!=actual.sites.len(){return Err("Foreign debugger frozen closure refused".into());}
  for(a,b)in actual.origins.iter().zip(&self.map.origins){if a.source!=b.source||a.span!=b.span||a.route!=b.route||a.generated!=b.generated{return Err("Debugger emitted instruction provenance changed".into());}}
  for(a,b)in actual.sites.iter().zip(&self.map.sites){if a.scope!=b.scope||a.route!=b.route||a.origin.source!=b.origin.source||a.origin.span!=b.origin.span||a.origin.route!=b.origin.route||a.origin.scheduled_step!=b.origin.scheduled_step||a.declaration.source!=b.declaration.source||a.declaration.span!=b.declaration.span||a.declaration.route!=b.declaration.route{return Err("Debugger nested computation provenance changed".into());}}
  for site in &self.map.sites{
   if let Some(endpoint)=site.origin.scheduled_step.as_ref().and_then(|step|step.endpoint.as_ref()){
    if site.origin.graph.as_ref().is_none_or(|graph|graph.graph!=endpoint.graph||graph.node!=Some(endpoint.node)){
     return Err("Debugger producer identity differs from actual source declaration".into());
    }
   }
  }
  Ok(self.map)
 }
}

fn legacy_origins(program:&ParsedProgram)->Result<(Vec<Statement>,Vec<InstructionOrigin>),String>{
 const MARK:&str="@editor-debug-origin-";
 fn mark(block:&[Statement],spans:&[StatementSpan],source:&Option<std::path::PathBuf>,route:&[AstRoutePart],origins:&mut Vec<InstructionOrigin>)->Result<Vec<Statement>,String>{
  if block.len()!=spans.len()||route.len()>256{return Err("Legacy debugger AST provenance budget".into());}
  let mut out=Vec::new();
  for (index,(statement,span)) in block.iter().zip(spans).enumerate(){
   if origins.len()>=100_000{return Err("Legacy debugger site budget".into());}
   let mut path=route.to_vec();path.push(if route.is_empty(){AstRoutePart::Root{index}}else{AstRoutePart::Body{index}});
   let id=origins.len();origins.push(InstructionOrigin{source:source.clone(),span:Some(span.range.clone()),route:path.clone(),generated:None,graph:None,scheduled_step:None});
   out.push(Statement::Label{name:format!("{MARK}{id}")});
   let mut statement=statement.clone();
   // Carry the actual wrapper site through generated range stop guards and
   // temporary prefixes. This metadata is stripped and exact IR checked below.
   if let Statement::ScheduledConsumer{version:2,scope,..}=&mut statement{*scope=format!("{MARK}{id}\0{scope}");}
   // Markers are metadata-only inputs to the EXISTING lowering; they are
   // stripped and the result checked against its uninstrumented IR below.
   let mut at=0;
   let mut child=|body:&mut Vec<Statement>|->Result<(),String>{let end=at+body.len();let children=span.children.get(at..end).ok_or("Legacy debugger child range missing")?;*body=mark(body,children,source,&path,origins)?;at=end;Ok(())};
   match &mut statement{
    Statement::ScheduledConsumer{version:2,body,..} if matches!(body.as_slice(),[Statement::If{..}|Statement::Sequence{..}|Statement::Choice{..}|Statement::Imagemap{..}|Statement::While{..}|Statement::ForEach{..}|Statement::ForLoopWithBreak{..}])=>{
     let inner=span.children.first().ok_or("Native wrapper has no actual body range")?;let mut base=path.clone();base.push(AstRoutePart::Body{index:0});
     let mut native_child=|body:&mut Vec<Statement>,part:usize,offset:usize,kind:usize|->Result<(),String>{let children=inner.children.get(offset..offset+body.len()).ok_or("Native control child ranges missing")?;let begin=origins.len();*body=mark(body,children,source,&base,origins)?;for origin in &mut origins[begin..]{let at=base.len();let index=match origin.route.get(at){Some(AstRoutePart::Body{index})=>*index,_=>return Err("Native child route differs from actual branch".into())};origin.route[at]=match kind{0=>AstRoutePart::Then{index},1=>AstRoutePart::Else{index},2=>AstRoutePart::Sequence{branch:part,index},3=>AstRoutePart::Choice{option:part,index},5=>AstRoutePart::Hotspot{hotspot:part,index},_=>AstRoutePart::Body{index}};}Ok(())};
     match body.as_mut_slice(){[Statement::If{then_branch,else_branch,..}]=>{let offset=then_branch.len();native_child(then_branch,0,0,0)?;native_child(else_branch,0,offset,1)?;},[Statement::Sequence{branches}]=>{let mut offset=0;for(part,branch)in branches.iter_mut().enumerate(){let len=branch.len();native_child(branch,part,offset,2)?;offset+=len;}},[Statement::Choice{options}]=>{let mut offset=0;for(part,option)in options.iter_mut().enumerate(){let len=option.body.len();native_child(&mut option.body,part,offset,3)?;offset+=len;}},[Statement::Imagemap{hotspots,..}]=>{let mut offset=0;for(part,hotspot)in hotspots.iter_mut().enumerate(){let len=hotspot.body.len();native_child(&mut hotspot.body,part,offset,5)?;offset+=len;}},[Statement::While{body,..}|Statement::ForEach{body,..}|Statement::ForLoopWithBreak{body,..}]=>native_child(body,0,0,4)?,_=>{}}
    },
    Statement::While{body,..}|Statement::ForEach{body,..}|Statement::ForLoopWithBreak{body,..}=>child(body)?,
    Statement::If{then_branch,else_branch,..}=>{child(then_branch)?;child(else_branch)?;},
    Statement::Sequence{branches}=>for branch in branches{child(branch)?;},
    Statement::Choice{options}=>for option in options{child(&mut option.body)?;},
    Statement::Imagemap{hotspots,..}=>for hotspot in hotspots{child(&mut hotspot.body)?;},_=>{},
   }
   out.push(statement);
  }Ok(out)
 }
 let mut mapped=Vec::new();let mut marked=Vec::new();
 for(index,(statement,origin))in program.script.iter().zip(&program.origins).enumerate(){
  let begin=mapped.len();
  let mut roots=mark(std::slice::from_ref(statement),std::slice::from_ref(&origin.span),&origin.source,&[],&mut mapped)?;
  // The compiler's original root ordinal, including imported roots.
  for item in &mut mapped[begin..]{item.route[0]=AstRoutePart::Root{index};}
  marked.append(&mut roots);
 }
 let mut extra=Vec::new();crate::engine::flatten_ast(&mut marked,&mut extra,&mut 0);marked.push(Statement::Jump{target:"__script_end".into()});marked.extend(extra);marked.push(Statement::Label{name:"__script_end".into()});
 let mut script=Vec::new();let mut origins=Vec::new();let mut active:Option<InstructionOrigin>=None;
 for mut statement in marked{
  if let Statement::Label{name}=&statement{if let Some(id)=name.strip_prefix(MARK).and_then(|n|n.parse::<usize>().ok()){active=Some(mapped.get(id).ok_or("Unknown legacy debug marker")?.clone());continue;}}
  let wrapper=if let Statement::ScheduledConsumer{version:2,scope,..}=&mut statement{if let Some((id,original))=scope.strip_prefix(MARK).and_then(|s|s.split_once('\0')){let id=id.parse::<usize>().map_err(|_|"Invalid internal wrapper origin marker")?;let origin=mapped.get(id).ok_or("Unknown wrapper origin marker")?.clone();*scope=original.to_string();Some(origin)}else{None}}else{None};
  let marked=active.take();origins.push(wrapper.or(marked).unwrap_or(InstructionOrigin{source:None,span:None,route:vec![],generated:None,graph:None,scheduled_step:None}));script.push(statement);
 }
 let mut actual=program.script.clone();let mut extra=Vec::new();crate::engine::flatten_ast(&mut actual,&mut extra,&mut 0);actual.push(Statement::Jump{target:"__script_end".into()});actual.extend(extra);actual.push(Statement::Label{name:"__script_end".into()});
 if script!=actual{return Err("Legacy debug markers changed the existing emitted IR".into());}
 Ok((script,origins))
}
