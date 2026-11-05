//! Private editor-launch data, not a Saved index, Source writer or host grant.
//! Descriptors are encoded only from an already admitted Current projection.
use rvn_parser::{ParsedProgram,enum_asset::EnumAssetId,enum_asset_admission1025::EnumAssets1025,enum_asset_json::EnumIndexInput};
use serde::{Serialize,Deserialize};
use sha2::{Digest,Sha256};
use std::collections::BTreeMap;
pub const SIDECAR:&str=".rvn-editor-enum-current1027.json";
pub const ARGUMENT:&str="--editor-enum-current1027=";
pub const MAX_RESOURCE_BYTES:usize=64*1024*1024;
// JSON String escaping can double existing descriptor bytes. This transport
// bound also includes the existing current locator closure and envelope.
pub const MAX_TRANSPORT_BYTES:usize=4*MAX_RESOURCE_BYTES;
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry{uuid:String,locator:String,wire:String}
#[derive(Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope{version:u32,nonce:u64,program:[u8;32],current:Vec<Entry>}
#[derive(Clone,Debug)]
pub struct CapturedEnums1027{current:Vec<Entry>}
fn add_budget(total:&mut usize,bytes:usize,limit:usize)->Result<(),String>{
    *total=total.checked_add(bytes).filter(|v|*v<=limit).ok_or("Current Enum launch data exceeds its bounded resource closure")?;Ok(())
}
fn admit(entries:&[Entry])->Result<EnumAssets1025,String>{
    let mut resources=0;let mut locators=0;
    for entry in entries{add_budget(&mut resources,entry.wire.len(),MAX_RESOURCE_BYTES)?;add_budget(&mut locators,entry.locator.len()+entry.uuid.len(),MAX_RESOURCE_BYTES)?;}
    let index=entries.iter().map(|e|(e.uuid.as_str(),Some(e.locator.as_str()))).collect::<Vec<_>>();
    let files=entries.iter().map(|e|(e.locator.clone(),e.wire.as_bytes().to_vec())).collect::<Vec<_>>();
    EnumAssets1025::admit(EnumIndexInput::Table(&index),&files,cfg!(windows))
}
fn digest_json(value:&serde_json::Value,hash:&mut Sha256){
    fn bytes(hash:&mut Sha256,tag:u8,bytes:&[u8]){hash.update([tag]);hash.update((bytes.len() as u64).to_be_bytes());hash.update(bytes);}
    match value{
        serde_json::Value::Null=>hash.update([b'n']),serde_json::Value::Bool(v)=>hash.update([if *v{b't'}else{b'f'}]),
        serde_json::Value::Number(v)=>bytes(hash,b'd',v.to_string().as_bytes()),serde_json::Value::String(v)=>bytes(hash,b's',v.as_bytes()),
        serde_json::Value::Array(v)=>{hash.update([b'a']);hash.update((v.len() as u64).to_be_bytes());for value in v{digest_json(value,hash);}},
        serde_json::Value::Object(v)=>{hash.update([b'o']);hash.update((v.len() as u64).to_be_bytes());let mut rows=v.iter().collect::<Vec<_>>();rows.sort_unstable_by(|a,b|a.0.cmp(b.0));for(key,value)in rows{bytes(hash,b'k',key.as_bytes());digest_json(value,hash);}},
    }
}
pub fn program_identity1027(program:&ParsedProgram)->Result<[u8;32],String>{
    let mut hash=Sha256::new();hash.update(b"RVN-CURRENT-ENUM-PROGRAM-1\0");hash.update(format!("{:?}\0",program.context));
    // Structural AST identity is independent of absolute staging paths and
    // map iteration order. Source closure validation remains the real host's.
    let value=serde_json::to_value(&program.script).map_err(|e|e.to_string())?;digest_json(&value,&mut hash);Ok(hash.finalize().into())
}
impl CapturedEnums1027{
    pub fn from_admitted(resources:&EnumAssets1025,locators:&BTreeMap<EnumAssetId,String>)->Result<Self,String>{
        if resources.identities()!=locators.keys().copied().collect(){return Err("Current Enum locators differ from their admitted original identities".into());}
        let current=resources.identities().into_iter().map(|id|{
            let asset=resources.asset(id).ok_or("Admitted Current Enum disappeared")?;
            Ok(Entry{uuid:id.to_hex(),locator:locators[&id].clone(),wire:asset.encode()?})
        }).collect::<Result<Vec<_>,String>>()?;
        let check=admit(&current)?;
        for id in resources.identities(){if check.asset(id)!=resources.asset(id){return Err("Transport changed an admitted Current Enum value".into());}}
        Ok(Self{current})
    }
    pub fn is_empty(&self)->bool{self.current.is_empty()}
    pub fn retained_bytes(&self)->Result<usize,String>{
        self.current.iter().try_fold(0usize,|n,e|n.checked_add(e.uuid.len()).and_then(|n|n.checked_add(e.locator.len())).and_then(|n|n.checked_add(e.wire.len())).ok_or_else(||"Current Enum retained bytes overflow".into()))
    }
    pub fn encode_for(&self,program:&ParsedProgram,nonce:u64)->Result<Vec<u8>,String>{
        if nonce==0{return Err("Current Enum launch identity is absent".into());}
        let envelope=Envelope{version:1,nonce,program:program_identity1027(program)?,current:self.current.clone()};
        let bytes=serde_json::to_vec(&envelope).map_err(|e|e.to_string())?;
        if bytes.len()>MAX_TRANSPORT_BYTES{return Err("Current Enum transport exceeds its launch byte budget".into());}Ok(bytes)
    }
}
pub fn decode_for1027(bytes:&[u8],program:&ParsedProgram,nonce:u64)->Result<EnumAssets1025,String>{
    if nonce==0||bytes.len()>MAX_TRANSPORT_BYTES{return Err("Current Enum launch identity or transport size is invalid".into());}
    let envelope:Envelope=serde_json::from_slice(bytes).map_err(|e|e.to_string())?;
    if envelope.version!=1||envelope.nonce!=nonce||envelope.program!=program_identity1027(program)?{
        return Err("Current Enum transport belongs to another launch or compiled program".into());
    }
    let resources=admit(&envelope.current)?;
    crate::eval::validate_enum_program_literals1027(&program.script,&resources).map_err(|e|e.to_string())?;
    Ok(resources)
}
/// The expected bytes originate in the App stage, independently of the
/// later physical sidecar read. This seal grants no startup input authority.
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct LaunchSeal1027{nonce:u64,digest:[u8;32]}
impl LaunchSeal1027{
    pub fn for_bytes(nonce:u64,bytes:&[u8])->Result<Self,String>{
        if nonce==0||bytes.len()>MAX_TRANSPORT_BYTES{return Err("Invalid Current launch seal identity or byte budget".into());}
        Ok(Self{nonce,digest:Sha256::digest(bytes).into()})
    }
    pub fn nonce(&self)->u64{self.nonce}
    pub fn argument(&self)->String{
        let hash=self.digest.iter().map(|v|format!("{v:02x}")).collect::<String>();format!("{ARGUMENT}{}:{hash}",self.nonce)
    }
    pub fn validate_bytes(&self,bytes:&[u8])->Result<(),String>{
        if Self::for_bytes(self.nonce,bytes)?.digest!=self.digest{return Err("Current Enum bytes differ from the original App launch seal".into());}Ok(())
    }
}
pub fn launch_argument1027(arguments:impl IntoIterator<Item=String>)->Result<Option<LaunchSeal1027>,String>{
    let mut seal=None;
    for argument in arguments{
        if argument==ARGUMENT.trim_end_matches('='){return Err("Current Enum launch argument requires its original nonce and byte seal".into());}
        if let Some(value)=argument.strip_prefix(ARGUMENT){
            let(nonce,hash)=value.split_once(':').ok_or("Current launch argument has no original byte seal")?;
            let nonce=nonce.parse::<u64>().ok().filter(|n|*n>0&&n.to_string()==nonce).ok_or("Invalid Current Enum launch nonce")?;
            if hash.len()!=64||!hash.bytes().all(|v|v.is_ascii_digit()||(b'a'..=b'f').contains(&v)){return Err("Invalid canonical Current Enum SHA256 seal".into());}
            let mut digest=[0u8;32];for(i,slot)in digest.iter_mut().enumerate(){*slot=u8::from_str_radix(&hash[i*2..i*2+2],16).map_err(|e|e.to_string())?;}
            if seal.replace(LaunchSeal1027{nonce,digest}).is_some(){return Err("Repeated Current Enum launch argument".into());}
        }
    }Ok(seal)
}
pub fn nonce_argument1027(arguments:impl IntoIterator<Item=String>)->Result<Option<u64>,String>{Ok(launch_argument1027(arguments)?.map(|seal|seal.nonce()))}
#[cfg(test)]
#[path="editor_enum_current_tests1027.rs"]
mod tests;
