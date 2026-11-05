//! Single portable v1 JSON codec, alongside WidgetBlueprintV1. No filesystem.
use crate::enum_asset::*;
use serde::{Deserialize,Serialize};
use std::collections::BTreeSet;
pub const ENUM_MAX_FILE_BYTES:usize=2*1_048_576;
/// Same manifest convention as WidgetBlueprintV1: UUID -> assets-relative
/// typed locator. This is an index value, not an alternate live Registry.
pub enum EnumIndexInput<'a>{Absent,NotTable,Table(&'a[(&'a str,Option<&'a str>)])}
pub fn validate_enum_index(input:EnumIndexInput<'_>,insensitive:bool)->Result<std::collections::BTreeMap<String,String>,String>{
    let rows=match input{EnumIndexInput::Absent=>return Ok(Default::default()),EnumIndexInput::NotTable=>return Err("Enum index must be a table".into()),EnumIndexInput::Table(rows)=>rows};
    let mut result=std::collections::BTreeMap::new();let mut paths=BTreeSet::new();
    for(id,path)in rows{
        crate::localized_text::validate_text_asset_id(id)?;let path=path.ok_or("Enum indexed locator must be a string")?;
        crate::widget_asset::validate_relative_locator(path,Some("rvnenum"))?;
        let folded=if insensitive{path.to_lowercase()}else{path.to_string()};
        if !paths.insert(folded)||result.insert((*id).into(),path.into()).is_some(){return Err("Enum index repeats identity or locator".into());}
    }Ok(result)
}
#[derive(Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    version:u32,#[serde(rename="type")]asset_type:String,uuid:String,name:String,description:String,bitflags:bool,
    revision:u64,next_label_index:String,members:Vec<MemberWire>,retired_members:Vec<String>,
}
#[derive(Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct MemberWire { id:String,display_name:String,description:String }
impl EnumAsset {
    pub fn decode(bytes:&[u8])->Result<Self,String>{
        if bytes.len()>ENUM_MAX_FILE_BYTES{return Err("Enum file exceeds byte budget".into());}
        let w:Wire=serde_json::from_slice(bytes).map_err(|e|e.to_string())?;
        if w.version!=1||w.asset_type!=ENUM_ASSET_TYPE{return Err("Unsupported Enum asset version/type".into());}
        // Exactly the existing project UUID validator, not a new spelling.
        crate::localized_text::validate_text_asset_id(&w.uuid)?;
        let next=if w.next_label_index.len()==16&&w.next_label_index.bytes().all(|c|c.is_ascii_digit()||(b'a'..=b'f').contains(&c)){
            u64::from_str_radix(&w.next_label_index,16).map_err(|e|e.to_string())?
        }else{return Err("Enum suggested-label index requires16 lowercase hex digits".into());};
        let retired=w.retired_members.iter().map(|id|EnumMemberId::parse(id)).collect::<Result<BTreeSet<_>,_>>()?;
        if retired.len()!=w.retired_members.len(){return Err("Enum file repeats a retired member identity".into());}
        Self::from_parts(EnumAssetParts{id:EnumAssetId::parse(&w.uuid)?,name:w.name,description:w.description,bitflags:w.bitflags,revision:w.revision,next_member_id:next,members:w.members.into_iter().map(|m|Ok(EnumMember{id:EnumMemberId::parse(&m.id)?,display_name:m.display_name,description:m.description})).collect::<Result<_,String>>()?,retired})
    }
    pub fn encode(&self)->Result<String,String>{
        self.validate()?;let p=self.parts();crate::localized_text::validate_text_asset_id(&p.id.to_hex())?;
        let w=Wire{version:1,asset_type:ENUM_ASSET_TYPE.into(),uuid:p.id.to_hex(),name:p.name.clone(),description:p.description.clone(),bitflags:p.bitflags,revision:p.revision,next_label_index:format!("{:016x}",p.next_member_id),members:p.members.iter().map(|m|MemberWire{id:m.id.to_hex(),display_name:m.display_name.clone(),description:m.description.clone()}).collect(),retired_members:p.retired.iter().map(|id|id.to_hex()).collect()};
        let encoded=serde_json::to_string_pretty(&w).map_err(|e|e.to_string())?+"\n";
        if encoded.len()>ENUM_MAX_FILE_BYTES{return Err("Enum file exceeds byte budget".into());}Ok(encoded)
    }
}
#[cfg(test)]
#[path="enum_asset_json_tests.rs"]
mod tests;
