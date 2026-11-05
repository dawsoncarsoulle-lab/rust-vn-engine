//! Pure version-1 Widget Blueprint metadata, shared by editor and native loaders.
//! References are opaque authored IDs. Decoding confers no source binding,
//! GraphDocument membership, filesystem permission or live object authority.
use crate::localized_text::validate_text_asset_id;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    str::FromStr,
};

pub const ASSET_TYPE: &str = "rvn.widget-blueprint";
pub const MAX_FILE_BYTES: usize = 2 * 1_048_576;

/// Canonical opaque graph reference, not an rvn_graph or runtime object ID.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WidgetGraphRef16(u64);
impl WidgetGraphRef16 {
    /// Adapt a known authored identifier without allocating or resolving one.
    pub const fn from_u64(value: u64) -> Self {
        Self(value)
    }
    pub const fn get(self) -> u64 {
        self.0
    }
    pub fn to_hex(self) -> String {
        format!("{:016x}", self.0)
    }
    pub fn parse(value: &str) -> Result<Self, String> {
        if value.len() != 16
            || !value
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        {
            return Err(
                "A Widget graph reference needs exactly 16 lowercase hexadecimal digits".into(),
            );
        }
        u64::from_str_radix(value, 16)
            .map(Self)
            .map_err(|error| error.to_string())
    }
}
impl fmt::Display for WidgetGraphRef16 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:016x}", self.0)
    }
}
impl FromStr for WidgetGraphRef16 {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}
impl Serialize for WidgetGraphRef16 {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_hex())
    }
}
impl<'de> Deserialize<'de> for WidgetGraphRef16 {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let value = String::deserialize(d)?;
        Self::parse(&value).map_err(serde::de::Error::custom)
    }
}

/// References only: no component tree, graph copy, private fields or AST.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WidgetBlueprintV1 {
    pub uuid: String,
    /// Project-relative source, distinct from the index's assets-relative path.
    pub source: String,
    pub screen_graph: WidgetGraphRef16,
    pub members: Vec<WidgetGraphRef16>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    version: u32,
    #[serde(rename = "type")]
    asset_type: String,
    uuid: String,
    source: String,
    screen_graph: String,
    members: Vec<String>,
}

/// The existing lexical contract; callers retain physical/junction/CAS guards.
/// An extension is lowercase and excludes the dot, as in `rvn` or `rvnwidget`.
pub fn validate_relative_locator(value: &str, extension: Option<&str>) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 4_096
        || value
            .chars()
            .any(|c| c.is_control() || matches!(c, '\\' | ':' | '#' | '?'))
        || value.split('/').count() > 64
        || value.split('/').any(|part| {
            part.is_empty()
                || part == "."
                || part == ".."
                || part.ends_with('.')
                || part.ends_with(' ')
        })
        || extension.is_some_and(|extension| !value.ends_with(&format!(".{extension}")))
    {
        return Err("A Widget asset/source locator must be a bounded safe relative path".into());
    }
    Ok(())
}
impl WidgetBlueprintV1 {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_asset_id(&self.uuid)?;
        validate_relative_locator(&self.source, Some("rvn"))?;
        if self.members.is_empty()
            || !self.members.contains(&self.screen_graph)
            || self.members.iter().copied().collect::<BTreeSet<_>>().len() != self.members.len()
        {
            return Err("The Widget Screen must belong to a nonempty unique graph group".into());
        }
        Ok(())
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_FILE_BYTES {
            return Err("Widget file exceeds its byte budget".into());
        }
        let wire: Wire = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
        if wire.version != 1 || wire.asset_type != ASSET_TYPE {
            return Err("Unsupported Widget Blueprint format/type".into());
        }
        let value = Self {
            uuid: wire.uuid,
            source: wire.source,
            screen_graph: WidgetGraphRef16::parse(&wire.screen_graph)?,
            members: wire
                .members
                .iter()
                .map(|value| WidgetGraphRef16::parse(value))
                .collect::<Result<_, _>>()?,
        };
        value.validate()?;
        Ok(value)
    }
    pub fn encode(&self) -> Result<String, String> {
        self.validate()?;
        let wire = Wire {
            version: 1,
            asset_type: ASSET_TYPE.into(),
            uuid: self.uuid.clone(),
            source: self.source.clone(),
            screen_graph: self.screen_graph.to_hex(),
            members: self.members.iter().map(|id| id.to_hex()).collect(),
        };
        let text = serde_json::to_string_pretty(&wire).map_err(|error| error.to_string())? + "\n";
        if text.len() > MAX_FILE_BYTES {
            return Err("Widget file exceeds its byte budget".into());
        }
        Ok(text)
    }
}

/// TOML/manifest adapters project the optional table without another decoder.
/// Non-table and non-string values stay explicit so native and App reject the
/// same shape. This pure module does not parse or edit a project manifest.
pub enum WidgetIndexInput<'a> {
    Absent,
    NotTable,
    Table(&'a [(&'a str, Option<&'a str>)]),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocatorCase {
    Sensitive,
    Insensitive,
}
pub fn validate_index(
    input: WidgetIndexInput<'_>,
    case: LocatorCase,
) -> Result<BTreeMap<String, String>, String> {
    let entries = match input {
        WidgetIndexInput::Absent => return Ok(BTreeMap::new()),
        WidgetIndexInput::NotTable => {
            return Err("Widget Blueprint index must be a TOML table".into())
        }
        WidgetIndexInput::Table(entries) => entries,
    };
    let mut result = BTreeMap::new();
    let mut paths = BTreeSet::new();
    for (uuid, value) in entries {
        validate_text_asset_id(uuid)?;
        let path = value.ok_or("Widget Blueprint locator must be a string")?;
        validate_relative_locator(path, Some("rvnwidget"))?;
        let key = match case {
            LocatorCase::Sensitive => path.to_owned(),
            LocatorCase::Insensitive => path.to_lowercase(),
        };
        if !paths.insert(key) {
            return Err("Two Widget UUIDs declare the same asset locator".into());
        }
        if result.insert((*uuid).to_owned(), path.to_owned()).is_some() {
            return Err("Duplicate Widget Blueprint UUID in index".into());
        }
    }
    Ok(result)
}
/// Pure cross-asset exclusivity. Validate each metadata document first; actual
/// Screen/Handler/Function membership remains the source-aware loader's job.
pub fn validate_group_memberships<'a>(
    groups: impl IntoIterator<Item = (&'a str, &'a [WidgetGraphRef16])>,
) -> Result<(), String> {
    let mut claimed = BTreeMap::new();
    for (uuid, members) in groups {
        for member in members {
            if claimed.insert(*member, uuid).is_some() {
                return Err("One editable graph belongs to two Widget Blueprint UUIDs".into());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "widget_asset_tests_0917.rs"]
mod tests;
