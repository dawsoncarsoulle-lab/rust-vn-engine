//! Nominal portable value only. Decoding this DTO grants no loaded asset.
use crate::enum_asset::{EnumAssetId,EnumMemberId,EnumValue};
use serde::{Serialize,Deserialize,Serializer,Deserializer};
#[derive(Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire { asset:String, member:String }
impl Serialize for EnumValue {
    fn serialize<S:Serializer>(&self,serializer:S)->Result<S::Ok,S::Error>{
        Wire{asset:self.asset.to_hex(),member:self.member.to_hex()}.serialize(serializer)
    }
}
impl<'de> Deserialize<'de> for EnumValue {
    fn deserialize<D:Deserializer<'de>>(deserializer:D)->Result<Self,D::Error>{
        let wire=Wire::deserialize(deserializer)?;
        Ok(Self{asset:EnumAssetId::parse(&wire.asset).map_err(serde::de::Error::custom)?,member:EnumMemberId::parse(&wire.member).map_err(serde::de::Error::custom)?})
    }
}
#[cfg(test)]
#[path="enum_value_wire_tests1025.rs"]
mod tests;
