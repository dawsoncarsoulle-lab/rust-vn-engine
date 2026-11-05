//! Nominal Function Library membership values. No IO, writer, VM, UUID allocator,
//! graph allocator, Source admission or executable definition is stored here.
//! The actual graph/source remains the sole function body and signature.
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Existing Source resolution byte ceiling; checked before JSON deserialization.
pub const MAX_FUNCTION_LIBRARY_BYTES: usize = 16 * 1024 * 1024;
/// Same explicit membership ceiling as the nominal schema view.
pub const MAX_FUNCTION_LIBRARY_MEMBERS: usize = 65_536;
/// Existing Function/Unit signature limit, shared by scoped transport.
pub const MAX_FUNCTION_LIBRARY_PARAMETERS: usize = 128;

pub(crate) fn bounded_library_vec<'de, D, T, const LIMIT: usize>(deserializer: D) -> Result<Vec<T>, D::Error>
where D: serde::Deserializer<'de>, T: Deserialize<'de> {
    struct Bounded<T, const LIMIT: usize>(std::marker::PhantomData<T>);
    impl<'de, T: Deserialize<'de>, const LIMIT: usize> serde::de::Visitor<'de> for Bounded<T, LIMIT> {
        type Value = Vec<T>;
        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            write!(formatter, "a sequence with at most {LIMIT} Library items")
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut sequence: A) -> Result<Self::Value, A::Error> {
            if sequence.size_hint().is_some_and(|hint| hint > LIMIT) {
                return Err(serde::de::Error::custom("Library sequence exceeds its declared limit"));
            }
            let mut values = Vec::new();
            while values.len() < LIMIT {
                let Some(value) = sequence.next_element::<T>()? else { return Ok(values); };
                if values.len() == values.capacity() {
                    values.try_reserve(1).map_err(|_| serde::de::Error::custom("Insufficient memory for Library sequence"))?;
                }
                values.push(value);
            }
            if sequence.next_element::<serde::de::IgnoredAny>()?.is_some() {
                return Err(serde::de::Error::custom("Library sequence exceeds its declared limit"));
            }
            Ok(values)
        }
    }
    deserializer.deserialize_seq(Bounded::<T, LIMIT>(std::marker::PhantomData))
}


#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(into = "String")]
pub struct FunctionLibraryId(String);
impl FunctionLibraryId {
    pub fn parse(value: impl Into<String>) -> Result<Self, String> {
        let value = value.into();
        crate::localized_text::validate_text_asset_id(&value)?;
        Ok(Self(value))
    }
    pub fn as_str(&self) -> &str { &self.0 }
}
impl<'de> Deserialize<'de> for FunctionLibraryId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct IdVisitor;
        impl<'de> serde::de::Visitor<'de> for IdVisitor {
            type Value = FunctionLibraryId;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { f.write_str("a canonical nonzero 32-character Library identity") }
            fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Self::Value, E> {
                crate::localized_text::validate_text_asset_id(value).map_err(E::custom)?;
                Ok(FunctionLibraryId(value.to_owned()))
            }
        }
        deserializer.deserialize_str(IdVisitor)
    }
}
impl TryFrom<String> for FunctionLibraryId {
    type Error = String;
    fn try_from(value: String) -> Result<Self, Self::Error> { Self::parse(value) }
}
impl From<FunctionLibraryId> for String {
    fn from(value: FunctionLibraryId) -> Self { value.0 }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LibraryFunctionAccess { Public, Private }

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FunctionLibraryMember {
    /// Serialized value of the existing Source GraphId, NOT another allocator.
    pub graph_id: u64,
    pub access: LibraryFunctionAccess,
    /// Blueprint presentation contract; never an RNG/service/purity permission.
    /// Unit functions with no result cannot take the value-expression route.
    pub pure: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QualifiedLibraryFunction {
    pub library: FunctionLibraryId,
    pub graph_id: u64,
}
impl QualifiedLibraryFunction {
    pub fn validate(&self) -> Result<(), String> {
        if self.graph_id == 0 { return Err("Library function requires a nonzero Source GraphId".into()); }
        Ok(())
    }
}

/// One nominal owner, with explicit member identities. Names, graph bodies,
/// ordered parameters and result values are deliberately absent from this file.
/// Empty is legal before first editor initialization; loading never seeds a graph.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FunctionLibraryAsset {
    id: FunctionLibraryId,
    members: Vec<FunctionLibraryMember>,
}
impl FunctionLibraryAsset {
    pub fn new(id: FunctionLibraryId, members: Vec<FunctionLibraryMember>) -> Result<Self, String> {
        if members.len() > MAX_FUNCTION_LIBRARY_MEMBERS {
            return Err("Function Library exceeds the declared membership limit".into());
        }
        let mut ids = BTreeSet::new();
        for member in &members {
            if member.graph_id == 0 || !ids.insert(member.graph_id) {
                return Err("Function Library has a zero or duplicate Source GraphId".into());
            }
        }
        Ok(Self { id, members })
    }
    pub fn id(&self) -> &FunctionLibraryId { &self.id }
    pub fn members(&self) -> &[FunctionLibraryMember] { &self.members }
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_FUNCTION_LIBRARY_BYTES {
            return Err("Function Library exceeds the Source byte limit".into());
        }
        // Ignored fields are skipped without constructing member Vecs or strings.
        // The full strict wire still checks every field/duplicate afterwards.
        #[derive(Deserialize)]
        struct VersionProbe { version: u32 }
        let version: VersionProbe = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
        if version.version != 1 {
            return Err("Unsupported Function Library membership version".into());
        }
        serde_json::from_slice(bytes).map_err(|error| error.to_string())
    }
    pub fn encode(&self) -> Result<Vec<u8>, String> {
        let bytes = serde_json::to_vec_pretty(self).map_err(|error| error.to_string())?;
        if bytes.len() > MAX_FUNCTION_LIBRARY_BYTES {
            return Err("Function Library exceeds the Source byte limit".into());
        }
        Ok(bytes)
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    version: u32,
    library: FunctionLibraryId,
    #[serde(deserialize_with="deserialize_members")]
    members: Vec<FunctionLibraryMember>
}
fn deserialize_members<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Vec<FunctionLibraryMember>, D::Error> {
    bounded_library_vec::<D, FunctionLibraryMember, MAX_FUNCTION_LIBRARY_MEMBERS>(deserializer)
}
impl Serialize for FunctionLibraryAsset {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        Wire { version: 1, library: self.id.clone(), members: self.members.clone() }.serialize(serializer)
    }
}
impl<'de> Deserialize<'de> for FunctionLibraryAsset {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire = Wire::deserialize(deserializer)?;
        if wire.version != 1 { return Err(serde::de::Error::custom("Unsupported Function Library membership version")); }
        Self::new(wire.library, wire.members).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
#[path = "function_library_asset_tests1023.rs"]
mod tests;

#[cfg(test)]
#[path = "function_library_asset_bounds_tests1023.rs"]
mod bounds_tests;
