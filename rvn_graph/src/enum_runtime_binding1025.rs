//! Compiler-side admission of the SAME typed Enum resources used by playback.
//! The host owns actual file IO and the final pre-Init byte fence.
use rvn_parser::{enum_asset_admission1025::EnumAssets1025,enum_asset_json::EnumIndexInput};
pub fn admit_enum_assets1025(input:EnumIndexInput<'_>,files:&[(String,Vec<u8>)],insensitive:bool)->Result<EnumAssets1025,String>{EnumAssets1025::admit(input,files,insensitive)}

#[cfg(test)]
#[path="enum_runtime_binding_tests1025.rs"]
mod tests;
