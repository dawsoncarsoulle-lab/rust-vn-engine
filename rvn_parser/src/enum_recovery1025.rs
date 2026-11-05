//! Typed data accompanying the SAME project Source recovery. This DTO is an
//! immutable durable snapshot, not another editor model, registry or IO grant.
use crate::enum_asset::{EnumAsset, EnumAssetId};
use crate::enum_asset_json::ENUM_MAX_FILE_BYTES;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{collections::BTreeSet, path::Path};

pub const ENUM_RECOVERY_MAX_ASSETS1025: usize = 256;
pub const ENUM_RECOVERY_MAX_BYTES1025: usize = 64 * 1_048_576;
const MAX_RECORD_BYTES: usize = 4 * ENUM_MAX_FILE_BYTES + 16_384;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag="kind", rename_all="snake_case")]
pub enum EnumRecoveryBaseline1025 {
    Unpublished,
    Published { saved_wire: String },
}
// Serde's internally-tagged unit variant accepts extra keys. A strict struct
// visitor is required so `unpublished` cannot silently ignore a fake dirty flag
// or an explicitly-present null saved baseline.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BaselineWire {
    kind: String,
    #[serde(default, deserialize_with="present_saved_wire")]
    saved_wire: Option<String>,
}
fn present_saved_wire<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<String>, D::Error> {
    String::deserialize(deserializer).map(Some)
}
impl<'de> Deserialize<'de> for EnumRecoveryBaseline1025 {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire = BaselineWire::deserialize(deserializer)?;
        match (wire.kind.as_str(), wire.saved_wire) {
            ("unpublished", None) => Ok(Self::Unpublished),
            ("published", Some(saved_wire)) => Ok(Self::Published { saved_wire }),
            _ => Err(serde::de::Error::custom("Invalid Enum publication baseline kind or saved-wire presence")),
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct EnumRecoveryRecord1025 {
    locator: String,
    current: EnumAsset,
    baseline: EnumRecoveryBaseline1025,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    version: u32,
    locator: String,
    current_wire: String,
    baseline: EnumRecoveryBaseline1025,
}
impl EnumRecoveryRecord1025 {
    pub fn unpublished(locator: String, current: EnumAsset) -> Result<Self, String> {
        let record = Self { locator, current, baseline: EnumRecoveryBaseline1025::Unpublished };
        record.validate()?; Ok(record)
    }
    pub fn published(locator: String, current: EnumAsset, saved_wire: String) -> Result<Self, String> {
        let record = Self { locator, current, baseline: EnumRecoveryBaseline1025::Published { saved_wire } };
        record.validate()?; Ok(record)
    }
    pub fn locator(&self) -> &str { &self.locator }
    pub fn current(&self) -> &EnumAsset { &self.current }
    pub fn asset_id(&self) -> EnumAssetId { self.current.id() }
    pub fn baseline(&self) -> &EnumRecoveryBaseline1025 { &self.baseline }
    pub fn saved_wire(&self) -> Option<&str> {
        match &self.baseline { EnumRecoveryBaseline1025::Unpublished => None,
            EnumRecoveryBaseline1025::Published { saved_wire } => Some(saved_wire) }
    }
    pub fn validate(&self) -> Result<(), String> {
        crate::widget_asset::validate_relative_locator(&self.locator, Some("rvnenum"))?;
        self.current.validate()?;
        if Path::new(&self.locator).file_stem().and_then(|value| value.to_str()) != Some(self.current.parts().name.as_str()) {
            return Err("Recovered Enum locator/name do not match".into());
        }
        if let Some(wire) = self.saved_wire() {
            let saved = EnumAsset::decode(wire.as_bytes())?;
            if saved.id() != self.current.id() || saved.parts().name != self.current.parts().name {
                return Err("Recovered Enum published baseline has another identity/name".into());
            }
        }
        self.current.encode()?;
        Ok(())
    }
    pub fn encode(&self) -> Result<String, String> {
        self.validate()?;
        let wire = serde_json::to_string(self).map_err(|error| error.to_string())?;
        if wire.len() > MAX_RECORD_BYTES { return Err("Recovered Enum record exceeds byte budget".into()); }
        Ok(wire)
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_RECORD_BYTES { return Err("Recovered Enum record exceeds byte budget".into()); }
        serde_json::from_slice(bytes).map_err(|error| error.to_string())
    }
}
impl Serialize for EnumRecoveryRecord1025 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.validate().map_err(serde::ser::Error::custom)?;
        let current_wire = self.current.encode().map_err(serde::ser::Error::custom)?;
        Wire { version: 1, locator: self.locator.clone(), current_wire, baseline: self.baseline.clone() }.serialize(serializer)
    }
}
impl<'de> Deserialize<'de> for EnumRecoveryRecord1025 {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire = Wire::deserialize(deserializer)?;
        if wire.version != 1 { return Err(serde::de::Error::custom("Unsupported Enum recovery record version")); }
        let current = EnumAsset::decode(wire.current_wire.as_bytes()).map_err(serde::de::Error::custom)?;
        if current.encode().map_err(serde::de::Error::custom)? != wire.current_wire {
            return Err(serde::de::Error::custom("Recovered Enum current wire must be canonical"));
        }
        let record = Self { locator: wire.locator, current, baseline: wire.baseline };
        record.validate().map_err(serde::de::Error::custom)?;
        Ok(record)
    }
}

/// The App additionally checks configured physical asset roots, complete saved
/// indexes, Registry incarnation and cross-kind UUIDs before an actual admission.
pub fn validate_enum_recovery1025(records: &[EnumRecoveryRecord1025], windows: bool) -> Result<(), String> {
    if records.len() > ENUM_RECOVERY_MAX_ASSETS1025 { return Err("Too many recovered Enum assets".into()); }
    let mut ids = BTreeSet::new(); let mut paths = BTreeSet::new(); let mut bytes = 0usize;
    for record in records {
        record.validate()?;
        let path = if windows { record.locator.to_lowercase() } else { record.locator.clone() };
        if !ids.insert(record.asset_id()) || !paths.insert(path) { return Err("Recovered Enum UUID or locator is repeated".into()); }
        bytes = bytes.checked_add(record.encode()?.len()).ok_or("Recovered Enum byte budget overflow")?;
        if bytes > ENUM_RECOVERY_MAX_BYTES1025 { return Err("Recovered Enum batch exceeds 64 MiB".into()); }
    }
    Ok(())
}

#[cfg(test)]
#[path="enum_recovery_tests1025.rs"]
mod tests;
