//! Resource to store project paths such as assets, locales, theme and saves.
//!
//! The runtime fills this resource based on the `rvn.toml` configuration and the
//! chosen project directory.  Systems can use it to locate assets relative
//! to the project's root rather than relying on the process's current
//! directory.  In particular, it is used by the save/load UI to know where
//! to read and write save files.

use bevy::prelude::Resource;
use std::path::PathBuf;

#[allow(dead_code)]
#[derive(Resource, Clone)]
pub struct ProjectPaths {
    /// Root directory of the project (the folder containing `rvn.toml`).
    pub root: PathBuf,
    /// Path to the assets directory (backgrounds, sprites, music...).
    pub assets: PathBuf,
    /// Path to the locales directory.
    pub locales: PathBuf,
    /// Path to the theme file.
    pub theme: PathBuf,
    /// Path to the saves directory.
    pub saves: PathBuf,
}

impl ProjectPaths {
    /// Create a new `ProjectPaths` given the project root and relative paths
    /// from the configuration file.
    pub fn new(
        root: PathBuf,
        assets: PathBuf,
        locales: PathBuf,
        theme: PathBuf,
        saves: PathBuf,
    ) -> Self {
        Self {
            root,
            assets,
            locales,
            theme,
            saves,
        }
    }
}
