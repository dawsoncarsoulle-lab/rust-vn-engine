//! Immutable authored parameter identity. Allocation is injected by the
//! editor's existing OS-backed transaction; this module allocates nothing.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ParameterId(String);

impl ParameterId {
    pub fn parse(value: impl Into<String>) -> Result<Self, String> {
        let value = value.into();
        // Reuse the current validated 128-bit asset representation, not its
        // Text ownership semantics or any gameplay RNG/counter allocator.
        rvn_parser::localized_text::validate_text_asset_id(&value)?;
        Ok(Self(value))
    }
    pub fn as_str(&self) -> &str { &self.0 }
}

impl TryFrom<String> for ParameterId {
    type Error = String;
    fn try_from(value: String) -> Result<Self, Self::Error> { Self::parse(value) }
}
impl From<ParameterId> for String {
    fn from(value: ParameterId) -> Self { value.0 }
}

