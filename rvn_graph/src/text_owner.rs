//! Persisted localization scope of an authored asset. GraphId routes graph
//! references; this identity scopes translations across sessions and renames.
use crate::{GraphDocument, GRAPH_TEXT_SCHEMA_VERSION};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct TextOwnerId(String);

impl TextOwnerId {
    /// Allocation belongs to the editor's asset transaction, never gameplay
    /// RNG, graph counters, a mutable filename or a rendered translation.
    pub fn parse(value: impl Into<String>) -> Result<Self, String> {
        let value = value.into();
        rvn_parser::localized_text::validate_text_asset_id(&value)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str { &self.0 }

    /// A portable RVN identity, separate from the user namespace and key.
    pub fn locale_owner(&self) -> String { format!("asset:{}", self.0) }
}

impl TryFrom<String> for TextOwnerId {
    type Error = String;
    fn try_from(value: String) -> Result<Self, Self::Error> { Self::parse(value) }
}
impl From<TextOwnerId> for String {
    fn from(value: TextOwnerId) -> Self { value.0 }
}

impl GraphDocument {
    /// Explicit opt-in. Opening legacy v5/v6 documents allocates nothing and
    /// changes no schema. Existing asset ownership cannot be replaced by an
    /// ordinary content edit; asset duplication will need its own transaction.
    pub fn enable_text_owner(&mut self, owner: TextOwnerId) -> Result<bool, String> {
        self.validate_narrative_metadata()?;
        if let Some(existing) = &self.text_owner {
            return if existing == &owner { Ok(false) }
                else { Err("A content edit cannot replace the persistent text asset owner".into()) };
        }
        self.text_owner = Some(owner);
        self.schema_version = self.schema_version.max(GRAPH_TEXT_SCHEMA_VERSION);
        Ok(true)
    }
}

/// Only authored graph owners participate, once per document. Runtime values,
/// caches, views and transient editor handles never acquire this identity.
pub fn validate_text_owners(graphs: &[GraphDocument]) -> Result<(), String> {
    let mut owners = std::collections::BTreeSet::new();
    for graph in graphs {
        graph.validate_narrative_metadata()?;
        if let Some(owner) = &graph.text_owner {
            if !owners.insert(owner) {
                return Err("Distinct authored documents share a persistent text asset owner".into());
            }
        }
    }
    Ok(())
}

pub(crate) fn validate_text_owner_edit(before: &[GraphDocument], after: &[GraphDocument]) -> Result<(), String> {
    validate_text_owners(after)?;
    for graph in after {
        if let Some(previous) = before.iter().find(|g| g.graph_id == graph.graph_id) {
            if previous.text_owner.is_some() && graph.text_owner != previous.text_owner {
                return Err("A content edit cannot replace or clear the persistent text asset owner".into());
            }
        }
    }
    Ok(())
}
