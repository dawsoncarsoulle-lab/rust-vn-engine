//! Strict Current Library metadata transport on the SAME actual program.
//! Pure data only: no filesystem, Saved fallback, Source/Registry or host grant.
use rvn_parser::{ParsedProgram,function_library_asset1023::{FunctionLibraryAsset,FunctionLibraryId,MAX_FUNCTION_LIBRARY_MEMBERS},widget_asset::validate_relative_locator};
use serde::{Serialize,Deserialize};
use sha2::{Digest,Sha256};
use std::collections::{BTreeMap,BTreeSet};
pub const SIDECAR:&str=".rvn-editor-library-current1027.json";
pub const ARGUMENT:&str="--editor-library-current1027=";
pub const MAX_RESOURCE_BYTES:usize=64*1024*1024;
pub const MAX_TRANSPORT_BYTES:usize=4*MAX_RESOURCE_BYTES;
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry{uuid:FunctionLibraryId,locator:String,wire:String}
#[derive(Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope{kind:String,version:u32,nonce:u64,program:[u8;32],#[serde(deserialize_with="bounded_entries")]current:Vec<Entry>}
fn bounded_entries<'de,D:serde::Deserializer<'de>>(d:D)->Result<Vec<Entry>,D::Error>{struct V;impl<'de>serde::de::Visitor<'de> for V{type Value=Vec<Entry>;fn expecting(&self,f:&mut std::fmt::Formatter)->std::fmt::Result{f.write_str("bounded Current Library codec entries")}fn visit_seq<A:serde::de::SeqAccess<'de>>(self,mut seq:A)->Result<Self::Value,A::Error>{let mut out=Vec::new();while let Some(value)=seq.next_element()?{if out.len()>=MAX_FUNCTION_LIBRARY_MEMBERS{return Err(serde::de::Error::custom("Current Library owner count exceeds codec bound"))}out.push(value)}Ok(out)}}d.deserialize_seq(V)}
fn admit(entries:&[Entry])->Result<Vec<FunctionLibraryAsset>,String>{
    if entries.len()>MAX_FUNCTION_LIBRARY_MEMBERS{return Err("Current Library owner count exceeds codec bound".into())}let(mut ids,mut paths,mut graphs)=(BTreeSet::new(),BTreeSet::new(),BTreeSet::new());let(mut resources,mut metadata)=(0usize,0usize);let mut assets=Vec::new();
    for entry in entries{resources=resources.checked_add(entry.wire.len()).filter(|n|*n<=MAX_RESOURCE_BYTES).ok_or("Current Library resource byte budget")?;metadata=metadata.checked_add(entry.uuid.as_str().len()).and_then(|n|n.checked_add(entry.locator.len())).filter(|n|*n<=MAX_RESOURCE_BYTES).ok_or("Current Library locator byte budget")?;validate_relative_locator(&entry.locator,Some("rvnlibrary"))?;if !ids.insert(entry.uuid.clone())||!paths.insert(if cfg!(windows){entry.locator.to_lowercase()}else{entry.locator.clone()}){return Err("Current Library repeats an original UUID/locator".into())}let asset=FunctionLibraryAsset::decode(entry.wire.as_bytes())?;if asset.id()!=&entry.uuid{return Err("Current Library wire differs from its original UUID".into())}for member in asset.members(){if !graphs.insert(member.graph_id)||graphs.len()>MAX_FUNCTION_LIBRARY_MEMBERS{return Err("Current Library repeats a Source member identity or exceeds bound".into())}}assets.push(asset)}Ok(assets)
}
#[derive(Clone,Debug)]
pub struct CapturedLibraries1027{current:Vec<Entry>}
impl CapturedLibraries1027{
    pub fn from_admitted(assets:&[FunctionLibraryAsset],locators:&BTreeMap<FunctionLibraryId,String>)->Result<Self,String>{
        if assets.iter().map(|a|a.id()).collect::<BTreeSet<_>>().len()!=assets.len()||locators.keys().collect::<BTreeSet<_>>()!=assets.iter().map(|a|a.id()).collect(){return Err("Current Library locator set differs from its actual admitted UUIDs".into())}let mut current=assets.iter().map(|asset|Ok(Entry{uuid:asset.id().clone(),locator:locators[asset.id()].clone(),wire:String::from_utf8(asset.encode()?).map_err(|e|e.to_string())?})).collect::<Result<Vec<_>,String>>()?;current.sort_by(|a,b|a.uuid.cmp(&b.uuid));let check=admit(&current)?;for asset in assets{if !check.iter().any(|a|a==asset){return Err("Current Library transport changed an admitted descriptor".into())}}Ok(Self{current})
    }
    pub fn is_empty(&self)->bool{self.current.is_empty()}
    pub fn retained_bytes(&self)->Result<usize,String>{self.current.iter().try_fold(0usize,|n,e|n.checked_add(e.uuid.as_str().len()).and_then(|n|n.checked_add(e.locator.len())).and_then(|n|n.checked_add(e.wire.len())).ok_or_else(||"Current Library retained bytes overflow".into()))}
    pub fn assets(&self)->Result<Vec<FunctionLibraryAsset>,String>{admit(&self.current)}
    pub fn locators(&self)->BTreeMap<FunctionLibraryId,String>{self.current.iter().map(|e|(e.uuid.clone(),e.locator.clone())).collect()}
    pub fn encode_for(&self,program:&ParsedProgram,nonce:u64)->Result<Vec<u8>,String>{
        if nonce==0{return Err("Current Library launch identity is absent".into())}let assets=admit(&self.current)?;crate::engine::PreparedFunctionLibraryProgram1023::prepare(program.clone(),&assets).map_err(|e|e.to_string())?;let envelope=Envelope{kind:"rvn.library-current.v1".into(),version:1,nonce,program:crate::editor_enum_current1027::program_identity1027(program)?,current:self.current.clone()};let bytes=serde_json::to_vec(&envelope).map_err(|e|e.to_string())?;if bytes.len()>MAX_TRANSPORT_BYTES{return Err("Current Library launch byte budget".into())}Ok(bytes)
    }
}
pub fn decode_for1027(bytes:&[u8],program:&ParsedProgram,nonce:u64)->Result<Vec<FunctionLibraryAsset>,String>{
    if nonce==0||bytes.len()>MAX_TRANSPORT_BYTES{return Err("Invalid Current Library launch identity/byte budget".into())}let envelope:Envelope=serde_json::from_slice(bytes).map_err(|e|e.to_string())?;if envelope.kind!="rvn.library-current.v1"||envelope.version!=1||envelope.nonce!=nonce||envelope.program!=crate::editor_enum_current1027::program_identity1027(program)?{return Err("Current Library transport belongs to another kind/launch/program".into())}let assets=admit(&envelope.current)?;crate::engine::PreparedFunctionLibraryProgram1023::prepare(program.clone(),&assets).map_err(|e|e.to_string())?;Ok(assets)
}
/// The expected digest comes from the App's already encoded original bytes,
/// independently of the later physical read. No input/startup authority here.
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct LaunchSeal1027{nonce:u64,digest:[u8;32]}
impl LaunchSeal1027{
    pub fn for_bytes(nonce:u64,bytes:&[u8])->Result<Self,String>{if nonce==0||bytes.len()>MAX_TRANSPORT_BYTES{return Err("Invalid Current Library seal identity/budget".into())}Ok(Self{nonce,digest:Sha256::digest(bytes).into()})}
    pub fn nonce(&self)->u64{self.nonce}
    pub fn argument(&self)->String{format!("{ARGUMENT}{}:{}",self.nonce,self.digest.iter().map(|b|format!("{b:02x}")).collect::<String>())}
    pub fn validate_bytes(&self,bytes:&[u8])->Result<(),String>{if Self::for_bytes(self.nonce,bytes)?.digest!=self.digest{return Err("Current Library bytes differ from the original App launch seal".into())}Ok(())}
}
pub fn launch_argument1027(arguments:impl IntoIterator<Item=String>)->Result<Option<LaunchSeal1027>,String>{let mut seal=None;for argument in arguments{if argument==ARGUMENT.trim_end_matches('='){return Err("Current Library argument requires its original nonce and byte seal".into())}if let Some(value)=argument.strip_prefix(ARGUMENT){let(nonce,hash)=value.split_once(':').ok_or("Current Library argument has no byte seal")?;let nonce=nonce.parse::<u64>().ok().filter(|n|*n>0&&n.to_string()==nonce).ok_or("Invalid canonical Current Library nonce")?;if hash.len()!=64||!hash.bytes().all(|b|b.is_ascii_digit()||(b'a'..=b'f').contains(&b)){return Err("Invalid canonical Current Library SHA256".into())}let mut digest=[0u8;32];for(i,b)in digest.iter_mut().enumerate(){*b=u8::from_str_radix(&hash[2*i..2*i+2],16).map_err(|e|e.to_string())?}if seal.replace(LaunchSeal1027{nonce,digest}).is_some(){return Err("Repeated Current Library argument".into())}}}Ok(seal)}
#[cfg(test)]
#[path="editor_library_current_tests1027.rs"]
mod tests;

/// Negative capability guard over typed authored code only. This never proves
/// absence of an empty Current metadata asset: the App must ALSO inspect its
/// actual Library participant/index before claiming a complete resource set.
/// It neither resolves references nor grants portable or startup admission.
pub fn program_requires_libraries1027(program:&ParsedProgram)->Result<bool,String>{
    use rvn_parser::{Expr,Statement};
    crate::save::consumer_checkpoint::preflight_program(program).map_err(|e|e.to_string())?;
    enum Work<'a>{Statement(&'a Statement),Expression(&'a Expr)}
    let mut pending=program.script.iter().map(Work::Statement).collect::<Vec<_>>();
    let mut visited=0usize;
    while let Some(work)=pending.pop(){
        visited+=1;if visited>100_000{return Err("Library typed source traversal exceeds the existing source-node bound".into())}
        match work{
            Work::Statement(statement)=>{
                if matches!(statement,Statement::FunctionLibrary{..}|Statement::LibraryUnitCall{..}){return Ok(true)}
                pending.extend(crate::consumer_schedule::direct_expressions(statement).into_iter().map(Work::Expression));
                if let Statement::ScheduledConsumer{steps,..}=statement{for step in steps{match step{rvn_parser::ConsumerStep::Capture{value,..}|rvn_parser::ConsumerStep::InspectTextTable{value,..}=>pending.push(Work::Expression(value))}}}
                for children in crate::consumer_schedule::statement_children(statement){pending.extend(children.iter().map(Work::Statement));}
            }
            Work::Expression(expression)=>match expression{
                Expr::LibraryCall{..}=>return Ok(true),
                Expr::Call{args,..}|Expr::ListLit(args)=>pending.extend(args.iter().map(Work::Expression)),
                Expr::BinOp{left,right,..}|Expr::And(left,right)|Expr::Or(left,right)=>{pending.push(Work::Expression(left));pending.push(Work::Expression(right));},
                Expr::Index{target,index}=>{pending.push(Work::Expression(target));pending.push(Work::Expression(index));},
                Expr::Neg(value)|Expr::Not(value)=>pending.push(Work::Expression(value)),
                Expr::StandardMacroData{..}|Expr::Int(_)|Expr::Float(_)|Expr::Bool(_)|Expr::Str(_)|Expr::Var(_)=>{},
            }
        }
    }
    Ok(false)
}
