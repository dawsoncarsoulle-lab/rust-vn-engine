//! Portable authoring Enum domain. No discovery, filesystem or runtime grant.
//! Asset and member identities are opaque nonzero32-hex values, never an
//! ordinal, display counter, name, path or runtime handle.
use std::collections::BTreeSet;

pub const ENUM_ASSET_TYPE:&str="rvn.enum";
pub const ENUM_MAX_MEMBERS:usize=255; // byte domain reserves its internal sentinel
#[derive(Clone,Copy,Debug,PartialEq,Eq,PartialOrd,Ord,Hash)]
pub struct EnumAssetId(u128);
impl EnumAssetId {
    pub fn parse(value:&str)->Result<Self,String>{
        if value.len()!=32||!value.bytes().all(|c|c.is_ascii_digit()||(b'a'..=b'f').contains(&c)){return Err("Enum asset ID requires32 lowercase hex digits".into());}
        let id=u128::from_str_radix(value,16).map_err(|e|e.to_string())?;
        if id==0{return Err("Enum asset ID must be nonzero".into());}Ok(Self(id))
    }
    pub fn to_hex(self)->String{format!("{:032x}",self.0)}
}
#[derive(Clone,Copy,Debug,PartialEq,Eq,PartialOrd,Ord,Hash)]
pub struct EnumMemberId(u128);
impl EnumMemberId {
    pub fn parse(value:&str)->Result<Self,String>{
        if value.len()!=32||!value.bytes().all(|c|c.is_ascii_digit()||(b'a'..=b'f').contains(&c)){return Err("Enum member ID requires32 lowercase hex digits".into());}
        let id=u128::from_str_radix(value,16).map_err(|e|e.to_string())?;
        if id==0{return Err("Enum member ID must be nonzero".into());}Ok(Self(id))
    }
    pub fn to_hex(self)->String{format!("{:032x}",self.0)}
    pub fn get(self)->u128{self.0}
}
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct EnumMember { pub id:EnumMemberId,pub display_name:String,pub description:String }
impl EnumMember {
    /// Diagnostic/export key only. Authored references retain id, not this text.
    pub fn raw_key(&self)->String{format!("Entry_{}",self.id.to_hex())}
}
#[derive(Clone,Copy,Debug,PartialEq,Eq,PartialOrd,Ord,Hash)]
pub struct EnumValue { pub asset:EnumAssetId,pub member:EnumMemberId }
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct EnumAssetParts {
    pub id:EnumAssetId,pub name:String,pub description:String,pub bitflags:bool,
    pub revision:u64,
    /// Suggested-label sequence only; member UUIDs never derive from this.
    pub next_member_id:u64,pub members:Vec<EnumMember>,pub retired:BTreeSet<EnumMemberId>,
}
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct EnumAsset { parts:EnumAssetParts }
pub fn validate_enum_display_name(value:&str)->Result<(),String>{
    if value.trim().is_empty()||value=="(INVALID)"||value.chars().any(char::is_control){return Err("Enum display name is empty, reserved or contains control characters".into());}Ok(())
}
fn name(value:&str)->Result<(),String>{
    validate_enum_display_name(value)?;
    if value.chars().any(|c|matches!(c,'/'|'\\'|':'|'\0')){return Err("Enum asset name is not a basename".into());}Ok(())
}
fn description(value:&str)->Result<(),String>{
    if value.contains('\0'){return Err("Enum description contains NUL".into());}Ok(())
}
impl EnumAsset {
    /// Mirrors a newly created empty authoring enum. No fake visible MAX row.
    pub fn new(id:EnumAssetId,asset_name:String)->Result<Self,String>{
        Self::from_parts(EnumAssetParts{id,name:asset_name,description:String::new(),bitflags:false,revision:0,next_member_id:1,members:Vec::new(),retired:BTreeSet::new()})
    }
    pub fn from_parts(parts:EnumAssetParts)->Result<Self,String>{let asset=Self{parts};asset.validate()?;Ok(asset)}
    pub fn parts(&self)->&EnumAssetParts{&self.parts}
    pub fn id(&self)->EnumAssetId{self.parts.id}
    pub fn members(&self)->&[EnumMember]{&self.parts.members}
    pub fn validate(&self)->Result<(),String>{
        if self.parts.id.0==0||self.parts.retired.iter().any(|id|id.0==0){return Err("Enum identity is zero".into());}
        name(&self.parts.name)?;description(&self.parts.description)?;
        if self.parts.next_member_id==0||self.parts.members.len()>ENUM_MAX_MEMBERS{return Err("Enum suggested-label cursor or byte-domain member count is invalid".into());}
        let mut ids=BTreeSet::new();let mut labels=BTreeSet::new();
        for member in &self.parts.members{
            validate_enum_display_name(&member.display_name)?;description(&member.description)?;
            if member.id.0==0||!ids.insert(member.id)||!labels.insert(member.display_name.as_str())||self.parts.retired.contains(&member.id){return Err("Enum repeats a member UUID/display name or revives a retired UUID".into());}
        }
        Ok(())
    }
    pub fn value(&self,member:EnumMemberId)->Result<EnumValue,String>{
        if !self.parts.members.iter().any(|item|item.id==member){return Err("Enum member is absent or retired".into());}Ok(EnumValue{asset:self.id(),member})
    }
    /// Ordinal is derived for a current compiler/backend, never serialized as
    /// a reference. Reorder changes this value but preserves the typed identity.
    pub fn ordinal(&self,value:EnumValue)->Result<u8,String>{
        if value.asset!=self.id(){return Err("Enum value belongs to a different asset".into());}
        self.parts.members.iter().position(|item|item.id==value.member).map(|i|i as u8).ok_or_else(||"Enum value references a removed member".into())
    }
    pub fn prepare(&self,command:EnumCommand)->Result<EnumTransaction,String>{
        self.validate()?;let mut after=self.clone();let mut removed=Vec::new();
        match command{
            EnumCommand::Add{member,display_name,description}=>{
                if after.parts.members.len()==ENUM_MAX_MEMBERS{return Err("Enum byte domain is full".into());}
                if after.parts.members.iter().any(|old|old.id==member)||after.parts.retired.contains(&member){return Err("Enum member UUID is already live or retired".into());}
                let id=member;
                after.parts.next_member_id=after.parts.next_member_id.checked_add(1).ok_or("Enum suggested-label counter exhausted")?;
                after.parts.members.push(EnumMember{id,display_name,description});
            },
            EnumCommand::RenameAsset(value)=>after.parts.name=value,
            EnumCommand::DescribeAsset(value)=>after.parts.description=value,
            EnumCommand::Bitflags(value)=>after.parts.bitflags=value,
            EnumCommand::RenameMember{member,display_name}=>after.member_mut(member)?.display_name=display_name,
            EnumCommand::DescribeMember{member,description}=>after.member_mut(member)?.description=description,
            EnumCommand::Move{member,index}=>{
                if index>=after.parts.members.len(){return Err("Enum move destination is absent".into());}
                let from=after.index(member)?;let entry=after.parts.members.remove(from);after.parts.members.insert(index,entry);
            },
            EnumCommand::Remove(member)=>{
                let index=after.index(member)?;after.parts.members.remove(index);after.parts.retired.insert(member);removed.push(EnumValue{asset:self.id(),member});
            },
        }
        after.parts.revision=self.parts.revision.checked_add(1).ok_or("Enum revision exhausted")?;after.validate()?;
        Ok(EnumTransaction{before:self.clone(),after,removed})
    }
    /// Ordinary reimport preserves asset identity and retained tombstones.
    /// Only an authorized history inverse can explicitly restore a retired row.
    pub fn prepare_import(&self,imported:EnumAsset)->Result<EnumTransaction,String>{
        self.validate()?;imported.validate()?;
        if imported.id()!=self.id()||!self.parts.retired.is_subset(&imported.parts.retired){return Err("Enum import replaced identity or forgot retired UUIDs".into());}
        for member in &imported.parts.members{
            if !self.parts.members.iter().any(|old|old.id==member.id)&&self.parts.retired.contains(&member.id){return Err("Enum import reused a retired member identity".into());}
        }
        let removed=self.parts.members.iter().filter(|old|!imported.parts.members.iter().any(|next|next.id==old.id)).map(|old|EnumValue{asset:self.id(),member:old.id}).collect::<Vec<_>>();
        let mut after=imported;
        for value in &removed{after.parts.retired.insert(value.member);}
        after.parts.revision=self.parts.revision.checked_add(1).ok_or("Enum revision exhausted")?;after.validate()?;
        Ok(EnumTransaction{before:self.clone(),after,removed})
    }
    /// Duplicate is a new asset identity; typed references to the old asset do
    /// not start resolving to its copy, even if entry IDs and labels are equal.
    pub fn duplicate(&self,id:EnumAssetId,asset_name:String)->Result<Self,String>{
        if id==self.id(){return Err("Duplicate Enum needs a fresh asset identity".into());}
        let mut value=self.clone();value.parts.id=id;value.parts.name=asset_name;value.parts.revision=0;value.validate()?;Ok(value)
    }
    fn index(&self,id:EnumMemberId)->Result<usize,String>{self.parts.members.iter().position(|m|m.id==id).ok_or_else(||"Enum member is absent".into())}
    fn member_mut(&mut self,id:EnumMemberId)->Result<&mut EnumMember,String>{let index=self.index(id)?;Ok(&mut self.parts.members[index])}
}
#[derive(Clone,Debug)]
pub enum EnumCommand {
    Add{member:EnumMemberId,display_name:String,description:String},RenameAsset(String),DescribeAsset(String),Bitflags(bool),
    RenameMember{member:EnumMemberId,display_name:String},DescribeMember{member:EnumMemberId,description:String},
    Move{member:EnumMemberId,index:usize},Remove(EnumMemberId),
}
#[derive(Clone,Debug)]
pub struct EnumTransaction { before:EnumAsset,after:EnumAsset,removed:Vec<EnumValue> }
impl EnumTransaction {
    pub fn before(&self)->&EnumAsset{&self.before}
    pub fn after(&self)->&EnumAsset{&self.after}
    /// Required downstream dependency repair frontier. The editor must inspect
    /// real project references before publishing removal; no Int coercion.
    pub fn removed_values(&self)->&[EnumValue]{&self.removed}
    /// Only the domain value publishes here. Actual file+Registry+journal
    /// publication must use the existing project transaction coordinator.
    pub fn commit(&self,current:&mut EnumAsset)->Result<(),String>{
        if current!=&self.before{return Err("Enum transaction baseline changed".into());}self.after.validate()?;*current=self.after.clone();Ok(())
    }
    /// The project Journal authorizes stack position. Explicit opaque IDs are
    /// restored by history, while the App supplies a new random UUID for every
    /// unrelated Add. UI input epochs live in the document owner, not this file.
    pub fn undo(&self,current:&EnumAsset)->Result<EnumTransaction,String>{
        current.validate()?;
        if current!=&self.after{return Err("Enum undo baseline changed".into());}
        let target=self.before.clone();target.validate()?;
        let removed=current.parts.members.iter().filter(|m|!target.parts.members.iter().any(|t|t.id==m.id)).map(|m|EnumValue{asset:current.id(),member:m.id}).collect();
        Ok(EnumTransaction{before:current.clone(),after:target,removed})
    }

}
#[cfg(test)]
#[path="enum_asset_tests.rs"]
mod tests;
