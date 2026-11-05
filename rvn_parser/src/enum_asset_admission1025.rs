//! Immutable projection of the existing typed project index and asset files.
//! No discovery, IO, mutable Registry, fallback ordinal, label or live grant.
use crate::enum_asset::{EnumAsset,EnumAssetId,EnumMemberId,EnumValue};
use crate::enum_asset_json::{EnumIndexInput,validate_enum_index};
use std::collections::{BTreeMap,BTreeSet};
#[derive(Clone,Debug,Default)]
pub struct EnumAssets1025 { assets:BTreeMap<EnumAssetId,EnumAsset> }
impl EnumAssets1025 {
 pub fn admit(input:EnumIndexInput<'_>,files:&[(String,Vec<u8>)],insensitive:bool)->Result<Self,String>{
  let index=validate_enum_index(input,insensitive)?;
  if files.len()!=index.len(){return Err("Enum descriptor closure differs from the exact project index".into());}
  let mut by_locator=BTreeMap::new();
  for (locator,bytes) in files {if by_locator.insert(locator.as_str(),bytes.as_slice()).is_some(){return Err("Enum descriptor closure repeats a locator".into());}}
  let mut assets=BTreeMap::new();
  for (uuid,locator) in index {
   let bytes=by_locator.remove(locator.as_str()).ok_or("An indexed Enum file is absent from its actual closure")?;
   let asset=EnumAsset::decode(bytes)?;let id=EnumAssetId::parse(&uuid)?;
   if asset.id()!=id{return Err("Enum file UUID differs from its real project index".into());}
   if assets.insert(id,asset).is_some(){return Err("Enum index repeats nominal identity".into());}
  }
  if !by_locator.is_empty(){return Err("An unindexed Enum file cannot become a runtime type".into());}Ok(Self{assets})
 }
 pub fn is_empty(&self)->bool{self.assets.is_empty()}
 pub fn len(&self)->usize{self.assets.len()}
 pub fn asset(&self,id:EnumAssetId)->Option<&EnumAsset>{self.assets.get(&id)}
 pub fn value(&self,asset:EnumAssetId,member:EnumMemberId)->Result<EnumValue,String>{self.assets.get(&asset).ok_or("Enum asset is not admitted by this project's actual index")?.value(member)}
 pub fn validate_value(&self,value:EnumValue)->Result<(),String>{self.value(value.asset,value.member).map(|_|())}
 pub fn validate_values<'a>(&self,values:impl IntoIterator<Item=&'a EnumValue>)->Result<(),String>{for value in values{self.validate_value(*value)?;}Ok(())}
 pub fn identities(&self)->BTreeSet<EnumAssetId>{self.assets.keys().copied().collect()}
}
#[cfg(test)]
#[path="enum_asset_admission_tests1025.rs"]
mod tests;
