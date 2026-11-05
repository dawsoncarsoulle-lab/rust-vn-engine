//! One strict project-index parser shared by cold admission and recovery.
//! This is immutable metadata, never a Registry, editor owner or IO grant.
use rvn_parser::function_library_asset1023::{FunctionLibraryId, MAX_FUNCTION_LIBRARY_MEMBERS};
use std::collections::{BTreeMap, BTreeSet};
pub const MAX_LIBRARY_SOURCE_INPUT_BYTES1027: usize = 64 * 1_048_576;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LibraryManifest1027 {
    pub assets_relative: String,
    pub has_library_index: bool,
    pub libraries: BTreeMap<FunctionLibraryId, String>,
}
pub fn parse_library_manifest1027(source: &str) -> Result<LibraryManifest1027, String> {
    let value = source.parse::<toml::Value>().map_err(|e| e.to_string())?;
    let table = value.as_table().ok_or("Project manifest must be a table")?;
    let assets = match table.get("paths") {
        None => "assets",
        Some(paths) => match paths
            .as_table()
            .ok_or("Project paths must be a table")?
            .get("assets")
        {
            None => "assets",
            Some(v) => v
                .as_str()
                .ok_or("The declared assets directory must be text")?,
        },
    };
    let has_library_index = table.contains_key("function_libraries");
    if has_library_index && source.len()>MAX_LIBRARY_SOURCE_INPUT_BYTES1027{return Err("Indexed Library manifest exceeds its 64 MiB capture bound".into());}
    if has_library_index {
        rvn_parser::widget_asset::validate_relative_locator(assets, None)?;
    }
    let mut libraries = BTreeMap::new();
    let mut locators = BTreeSet::new();
    if let Some(index) = table.get("function_libraries") {
        let index = index
            .as_table()
            .ok_or("The Function Library index must be a table")?;
        if index.len() > MAX_FUNCTION_LIBRARY_MEMBERS {
            return Err("Function Library index exceeds its owner bound".into());
        }
        for (id, locator) in index {
            let id = FunctionLibraryId::parse(id.clone())?;
            let locator = locator
                .as_str()
                .ok_or("An indexed Function Library locator must be text")?;
            rvn_parser::widget_asset::validate_relative_locator(locator, Some("rvnlibrary"))?;
            let key = if cfg!(windows) {
                locator.to_lowercase()
            } else {
                locator.to_owned()
            };
            if !locators.insert(key) || libraries.insert(id, locator.to_owned()).is_some() {
                return Err(
                    "Function Library index repeats an original UUID or physical locator".into(),
                );
            }
        }
    }
    Ok(LibraryManifest1027 {
        assets_relative: assets.to_owned(),
        has_library_index,
        libraries,
    })
}
