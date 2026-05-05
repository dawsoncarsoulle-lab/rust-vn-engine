use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
#[cfg(not(target_arch = "wasm32"))]
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum PersistentDataError {
    Io(std::io::Error),
    Json(serde_json::Error),
}

impl std::fmt::Display for PersistentDataError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "erreur disque persistent data : {e}"),
            Self::Json(e) => write!(f, "erreur JSON persistent data : {e}"),
        }
    }
}

impl From<std::io::Error> for PersistentDataError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<serde_json::Error> for PersistentDataError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ResumeSaveKind {
    Autosave,
    Manual,
    Quicksave,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LastResumeTarget {
    pub kind: ResumeSaveKind,
    #[serde(default)]
    pub slot: Option<u32>,
    pub timestamp: u64,
}

impl LastResumeTarget {
    pub fn autosave(timestamp: u64) -> Self {
        Self {
            kind: ResumeSaveKind::Autosave,
            slot: None,
            timestamp,
        }
    }

    pub fn manual(slot: u32, timestamp: u64) -> Self {
        Self {
            kind: ResumeSaveKind::Manual,
            slot: Some(slot),
            timestamp,
        }
    }

    pub fn quicksave(timestamp: u64) -> Self {
        Self {
            kind: ResumeSaveKind::Quicksave,
            slot: None,
            timestamp,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct PersistentData {
    #[serde(default)]
    pub seen_cgs: BTreeSet<String>,
    #[serde(default)]
    pub seen_endings: BTreeSet<String>,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub music_volume: Option<f32>,
    #[serde(default)]
    pub sfx_volume: Option<f32>,
    #[serde(default)]
    pub text_speed: Option<f32>,
    #[serde(default)]
    pub auto_speed: Option<f32>,
    #[serde(default)]
    pub fullscreen: Option<bool>,
    #[serde(default)]
    pub last_resume_target: Option<LastResumeTarget>,
}

#[derive(Debug, Clone)]
pub struct PersistentDataManager {
    path: PathBuf,
}

impl PersistentDataManager {
    pub fn new(save_dir: impl AsRef<Path>) -> Result<Self, PersistentDataError> {
        let save_dir = save_dir.as_ref();
        #[cfg(not(target_arch = "wasm32"))]
        fs::create_dir_all(save_dir)?;
        Ok(Self {
            path: save_dir.join("persistent.json"),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    #[cfg(target_arch = "wasm32")]
    fn storage_key(&self) -> String {
        format!("rvn:{}:persistent", self.path.display())
    }

    #[cfg(target_arch = "wasm32")]
    fn local_storage() -> Result<web_sys::Storage, PersistentDataError> {
        web_sys::window()
            .and_then(|window| window.local_storage().ok().flatten())
            .ok_or_else(|| {
                PersistentDataError::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "localStorage indisponible",
                ))
            })
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn load(&self) -> Result<PersistentData, PersistentDataError> {
        if !self.path.exists() {
            return Ok(PersistentData::default());
        }
        let json = fs::read_to_string(&self.path)?;
        Ok(serde_json::from_str(&json)?)
    }

    #[cfg(target_arch = "wasm32")]
    pub fn load(&self) -> Result<PersistentData, PersistentDataError> {
        let Some(json) = Self::local_storage()?
            .get_item(&self.storage_key())
            .map_err(|_| {
                PersistentDataError::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "lecture localStorage impossible",
                ))
            })?
        else {
            return Ok(PersistentData::default());
        };
        Ok(serde_json::from_str(&json)?)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn save(&self, data: &PersistentData) -> Result<(), PersistentDataError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(data)?;
        fs::write(&self.path, json)?;
        Ok(())
    }

    #[cfg(target_arch = "wasm32")]
    pub fn save(&self, data: &PersistentData) -> Result<(), PersistentDataError> {
        let json = serde_json::to_string_pretty(data)?;
        Self::local_storage()?
            .set_item(&self.storage_key(), &json)
            .map_err(|_| {
                PersistentDataError::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "écriture localStorage impossible",
                ))
            })?;
        Ok(())
    }

    pub fn mark_cg_seen(
        &self,
        data: &mut PersistentData,
        id: &str,
    ) -> Result<bool, PersistentDataError> {
        let inserted = data.seen_cgs.insert(id.to_string());
        if inserted {
            self.save(data)?;
        }
        Ok(inserted)
    }

    pub fn mark_ending_seen(
        &self,
        data: &mut PersistentData,
        id: &str,
    ) -> Result<bool, PersistentDataError> {
        let inserted = data.seen_endings.insert(id.to_string());
        if inserted {
            self.save(data)?;
        }
        Ok(inserted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(name);
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn load_missing_file_returns_default() {
        let dir = test_dir("rvn_persistent_missing");
        let manager = PersistentDataManager::new(&dir).unwrap();
        let data = manager.load().unwrap();
        assert_eq!(data, PersistentData::default());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn persistent_data_roundtrips_json() {
        let dir = test_dir("rvn_persistent_roundtrip");
        let manager = PersistentDataManager::new(&dir).unwrap();
        let mut data = PersistentData::default();
        data.seen_cgs.insert("demo".to_string());
        data.seen_endings.insert("demo_end".to_string());
        data.language = Some("fr".to_string());
        data.music_volume = Some(0.8);
        manager.save(&data).unwrap();

        let loaded = manager.load().unwrap();
        assert_eq!(loaded, data);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn mark_seen_deduplicates_cgs_and_endings() {
        let dir = test_dir("rvn_persistent_dedup");
        let manager = PersistentDataManager::new(&dir).unwrap();
        let mut data = manager.load().unwrap();

        assert!(manager.mark_cg_seen(&mut data, "demo").unwrap());
        assert!(!manager.mark_cg_seen(&mut data, "demo").unwrap());
        assert!(manager.mark_ending_seen(&mut data, "demo_end").unwrap());
        assert!(!manager.mark_ending_seen(&mut data, "demo_end").unwrap());

        assert_eq!(data.seen_cgs.len(), 1);
        assert_eq!(data.seen_endings.len(), 1);
        let _ = fs::remove_dir_all(&dir);
    }
}
