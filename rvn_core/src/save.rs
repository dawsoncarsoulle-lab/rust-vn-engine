use crate::{CinematicState, GameState, MusicState, SpriteState, TypewriterState};
use rvn_parser::{Position, Transition, Value};

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
#[cfg(not(target_arch = "wasm32"))]
use std::fs;
use std::path::{Path, PathBuf};
#[cfg(not(target_arch = "wasm32"))]
use std::time::{SystemTime, UNIX_EPOCH};

// ─── ERREURS

#[derive(Debug)]
pub enum SaveError {
    Io(std::io::Error),
    Json(serde_json::Error),
    SlotVide(u32),
    SlotHorsLimites { slot: u32, max: u32 },
}

impl std::fmt::Display for SaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "erreur disque : {e}"),
            Self::Json(e) => write!(f, "erreur JSON : {e}"),
            Self::SlotVide(n) => write!(f, "slot {n} vide"),
            Self::SlotHorsLimites { slot, max } => write!(f, "slot {slot} invalide (max {max})"),
        }
    }
}

impl From<std::io::Error> for SaveError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}
impl From<serde_json::Error> for SaveError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "v")]
pub enum SaveValue {
    Bool(bool),
    Int(i64),
    Float(f32),
    Str(String),
    /// Lists are serialized as JSON arrays of SaveValue.
    List(Vec<SaveValue>),
}

impl From<&Value> for SaveValue {
    fn from(v: &Value) -> Self {
        match v {
            Value::Bool(b) => SaveValue::Bool(*b),
            Value::Int(n) => SaveValue::Int(*n),
            Value::Float(f) => SaveValue::Float(*f),
            Value::Str(s) => SaveValue::Str(s.clone()),
            Value::List(items) => SaveValue::List(items.iter().map(SaveValue::from).collect()),
        }
    }
}

impl From<SaveValue> for Value {
    fn from(v: SaveValue) -> Self {
        match v {
            SaveValue::Bool(b) => Value::Bool(b),
            SaveValue::Int(n) => Value::Int(n),
            SaveValue::Float(f) => Value::Float(f),
            SaveValue::Str(s) => Value::Str(s),
            SaveValue::List(items) => Value::List(items.into_iter().map(Value::from).collect()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "v")]
pub enum SavePosition {
    Left,
    Center,
    Right,
    Custom(f32),
}

impl From<&Position> for SavePosition {
    fn from(p: &Position) -> Self {
        match p {
            Position::Left => SavePosition::Left,
            Position::Center => SavePosition::Center,
            Position::Right => SavePosition::Right,
            Position::Custom(x) => SavePosition::Custom(*x),
        }
    }
}

impl From<SavePosition> for Position {
    fn from(p: SavePosition) -> Self {
        match p {
            SavePosition::Left => Position::Left,
            SavePosition::Center => Position::Center,
            SavePosition::Right => Position::Right,
            SavePosition::Custom(x) => Position::Custom(x),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum SaveTransition {
    Fade {
        duration_ms: u32,
    },
    Dissolve {
        duration_ms: u32,
    },
    None,
    /// Catch-all for extended transitions (slide, zoom, wipe, blur).
    Other {
        kind: String,
        duration_ms: u32,
    },
}

impl From<&Transition> for SaveTransition {
    fn from(t: &Transition) -> Self {
        match t {
            Transition::Fade { duration_ms } => SaveTransition::Fade {
                duration_ms: *duration_ms,
            },
            Transition::Dissolve { duration_ms } => SaveTransition::Dissolve {
                duration_ms: *duration_ms,
            },
            Transition::None => SaveTransition::None,
            // All new transitions are serialized generically.
            other => SaveTransition::Other {
                kind: other
                    .to_string()
                    .split('(')
                    .next()
                    .unwrap_or("fade")
                    .to_string(),
                duration_ms: other.duration_ms(),
            },
        }
    }
}

impl From<SaveTransition> for Transition {
    fn from(t: SaveTransition) -> Self {
        match t {
            SaveTransition::Fade { duration_ms } => Transition::Fade { duration_ms },
            SaveTransition::Dissolve { duration_ms } => Transition::Dissolve { duration_ms },
            SaveTransition::None => Transition::None,
            // Extended transitions fall back to Fade on load (the visual
            // difference is transient; saved state just needs a valid transition).
            SaveTransition::Other { duration_ms, .. } => Transition::Fade { duration_ms },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveSprite {
    pub emotion: Option<String>,
    pub position: SavePosition,
    pub visible: bool,
}

impl From<&SpriteState> for SaveSprite {
    fn from(s: &SpriteState) -> Self {
        Self {
            emotion: s.emotion.clone(),
            position: SavePosition::from(&s.position),
            visible: s.visible,
        }
    }
}

impl From<SaveSprite> for SpriteState {
    fn from(s: SaveSprite) -> Self {
        SpriteState {
            emotion: s.emotion,
            position: Position::from(s.position),
            visible: s.visible,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveData {
    pub slot: u32,
    pub label: String,
    pub timestamp: u64,
    pub script_name: String,
    #[serde(default)]
    pub thumbnail: Option<String>,
    pub pc: usize,
    pub background_image: String,
    pub call_stack: Vec<usize>,
    pub vars: HashMap<String, SaveValue>,
    pub sprites: HashMap<String, SaveSprite>,
    #[serde(default)]
    pub cinematic: CinematicState,
    pub last_transition: SaveTransition,
    pub music: MusicState,
    pub typewriter: TypewriterState,
}

impl SaveData {
    pub fn from_state(state: &GameState, slot: u32, label: String, script_name: String) -> Self {
        Self {
            slot,
            label,
            timestamp: current_unix_timestamp_secs(),
            script_name,
            thumbnail: if state.background_image.is_empty() {
                None
            } else {
                Some(state.background_image.clone())
            },
            pc: state.current_interactive_pc,
            background_image: state.background_image.clone(),
            call_stack: state.call_stack.clone(),
            vars: state
                .vars
                .iter()
                .map(|(k, v)| (k.clone(), SaveValue::from(v)))
                .collect(),
            sprites: state
                .sprites
                .iter()
                .map(|(k, s)| (k.clone(), SaveSprite::from(s)))
                .collect(),
            cinematic: state.cinematic.clone(),
            last_transition: SaveTransition::from(&state.last_transition),
            music: state.music.clone(),
            typewriter: state.typewriter.clone(),
        }
    }

    pub fn into_game_state(self) -> GameState {
        GameState {
            pc: self.pc,
            current_interactive_pc: self.pc,
            background_image: self.background_image,
            call_stack: self.call_stack,
            vars: self
                .vars
                .into_iter()
                .map(|(k, v)| (k, Value::from(v)))
                .collect(),
            sprites: self
                .sprites
                .into_iter()
                .map(|(k, s)| (k, SpriteState::from(s)))
                .collect(),
            cinematic: self.cinematic,
            last_transition: Transition::from(self.last_transition),
            music: self.music,
            typewriter: self.typewriter,
        }
    }
}

fn current_unix_timestamp_secs() -> u64 {
    #[cfg(target_arch = "wasm32")]
    {
        (js_sys::Date::now() / 1000.0).max(0.0) as u64
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }
}

// ─── SAVE MANAGER

pub struct SaveManager {
    save_dir: PathBuf,
    max_slots: u32,
}

impl SaveManager {
    pub fn new(save_dir: impl AsRef<Path>, max_slots: u32) -> Result<Self, SaveError> {
        let dir = save_dir.as_ref().to_path_buf();
        #[cfg(not(target_arch = "wasm32"))]
        fs::create_dir_all(&dir)?;
        Ok(Self {
            save_dir: dir,
            max_slots,
        })
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn slot_path(&self, slot: u32) -> PathBuf {
        self.save_dir.join(format!("slot_{slot:02}.json"))
    }

    fn autosave_path(&self) -> PathBuf {
        self.save_dir.join("autosave.json")
    }

    fn quicksave_path(&self) -> PathBuf {
        self.save_dir.join("quicksave.json")
    }

    fn check_slot(&self, slot: u32) -> Result<(), SaveError> {
        if slot == 0 || slot > self.max_slots {
            return Err(SaveError::SlotHorsLimites {
                slot,
                max: self.max_slots,
            });
        }
        Ok(())
    }

    #[cfg(target_arch = "wasm32")]
    fn storage_key(&self, name: &str) -> String {
        format!("rvn:{}:{name}", self.save_dir.display())
    }

    #[cfg(target_arch = "wasm32")]
    fn local_storage() -> Result<web_sys::Storage, SaveError> {
        web_sys::window()
            .and_then(|window| window.local_storage().ok().flatten())
            .ok_or_else(|| {
                SaveError::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "localStorage indisponible",
                ))
            })
    }

    // ── Écriture

    #[cfg(not(target_arch = "wasm32"))]
    pub fn save(
        &self,
        state: &GameState,
        slot: u32,
        label: String,
        script_name: String,
    ) -> Result<(), SaveError> {
        self.check_slot(slot)?;
        let data = SaveData::from_state(state, slot, label, script_name);
        let json = serde_json::to_string_pretty(&data)?;
        fs::write(self.slot_path(slot), json)?;
        Ok(())
    }

    #[cfg(target_arch = "wasm32")]
    pub fn save(
        &self,
        state: &GameState,
        slot: u32,
        label: String,
        script_name: String,
    ) -> Result<(), SaveError> {
        self.check_slot(slot)?;
        let data = SaveData::from_state(state, slot, label, script_name);
        let json = serde_json::to_string_pretty(&data)?;
        Self::local_storage()?
            .set_item(&self.storage_key(&format!("slot_{slot:02}")), &json)
            .map_err(|_| {
                SaveError::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "écriture localStorage impossible",
                ))
            })?;
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn save_to_path(
        &self,
        path: PathBuf,
        state: &GameState,
        slot: u32,
        label: String,
        script_name: String,
    ) -> Result<(), SaveError> {
        let data = SaveData::from_state(state, slot, label, script_name);
        let json = serde_json::to_string_pretty(&data)?;
        fs::write(path, json)?;
        Ok(())
    }

    #[cfg(target_arch = "wasm32")]
    fn save_to_path(
        &self,
        path: PathBuf,
        state: &GameState,
        slot: u32,
        label: String,
        script_name: String,
    ) -> Result<(), SaveError> {
        let data = SaveData::from_state(state, slot, label, script_name);
        let json = serde_json::to_string_pretty(&data)?;
        let name = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("save");
        Self::local_storage()?
            .set_item(&self.storage_key(name), &json)
            .map_err(|_| {
                SaveError::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "écriture localStorage impossible",
                ))
            })?;
        Ok(())
    }

    pub fn save_autosave(
        &self,
        state: &GameState,
        label: String,
        script_name: String,
    ) -> Result<(), SaveError> {
        self.save_to_path(self.autosave_path(), state, 0, label, script_name)
    }

    pub fn save_quicksave(
        &self,
        state: &GameState,
        label: String,
        script_name: String,
    ) -> Result<(), SaveError> {
        self.save_to_path(self.quicksave_path(), state, 0, label, script_name)
    }

    // ── Lecture

    #[cfg(not(target_arch = "wasm32"))]
    pub fn load(&self, slot: u32) -> Result<SaveData, SaveError> {
        self.check_slot(slot)?;
        let path = self.slot_path(slot);
        if !path.exists() {
            return Err(SaveError::SlotVide(slot));
        }
        let json = fs::read_to_string(&path)?;
        let data = serde_json::from_str(&json)?;
        Ok(data)
    }

    #[cfg(target_arch = "wasm32")]
    pub fn load(&self, slot: u32) -> Result<SaveData, SaveError> {
        self.check_slot(slot)?;
        let Some(json) = Self::local_storage()?
            .get_item(&self.storage_key(&format!("slot_{slot:02}")))
            .map_err(|_| {
                SaveError::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "lecture localStorage impossible",
                ))
            })?
        else {
            return Err(SaveError::SlotVide(slot));
        };
        Ok(serde_json::from_str(&json)?)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn load_from_path(&self, path: PathBuf) -> Result<SaveData, SaveError> {
        if !path.exists() {
            return Err(SaveError::SlotVide(0));
        }
        let json = fs::read_to_string(&path)?;
        let data = serde_json::from_str(&json)?;
        Ok(data)
    }

    #[cfg(target_arch = "wasm32")]
    fn load_from_path(&self, path: PathBuf) -> Result<SaveData, SaveError> {
        let name = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("save");
        let Some(json) = Self::local_storage()?
            .get_item(&self.storage_key(name))
            .map_err(|_| {
                SaveError::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "lecture localStorage impossible",
                ))
            })?
        else {
            return Err(SaveError::SlotVide(0));
        };
        Ok(serde_json::from_str(&json)?)
    }

    pub fn load_autosave(&self) -> Result<SaveData, SaveError> {
        self.load_from_path(self.autosave_path())
    }

    pub fn load_quicksave(&self) -> Result<SaveData, SaveError> {
        self.load_from_path(self.quicksave_path())
    }

    pub fn list_saves(&self) -> Vec<SaveData> {
        (1..=self.max_slots)
            .filter_map(|slot| self.load(slot).ok())
            .collect()
    }

    pub fn latest_manual_save(&self) -> Option<SaveData> {
        self.list_saves()
            .into_iter()
            .max_by_key(|save| (save.timestamp, save.slot))
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn delete(&self, slot: u32) -> Result<(), SaveError> {
        self.check_slot(slot)?;

        let path = self.slot_path(slot);

        if path.exists() {
            fs::remove_file(&path)?;
        }
        Ok(())
    }

    #[cfg(target_arch = "wasm32")]
    pub fn delete(&self, slot: u32) -> Result<(), SaveError> {
        self.check_slot(slot)?;
        Self::local_storage()?
            .remove_item(&self.storage_key(&format!("slot_{slot:02}")))
            .map_err(|_| {
                SaveError::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "suppression localStorage impossible",
                ))
            })?;
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn slot_occupied(&self, slot: u32) -> bool {
        self.check_slot(slot).is_ok() && self.slot_path(slot).exists()
    }

    #[cfg(target_arch = "wasm32")]
    pub fn slot_occupied(&self, slot: u32) -> bool {
        self.check_slot(slot).is_ok()
            && Self::local_storage()
                .and_then(|storage| {
                    storage
                        .get_item(&self.storage_key(&format!("slot_{slot:02}")))
                        .map_err(|_| {
                            SaveError::Io(std::io::Error::new(
                                std::io::ErrorKind::Other,
                                "lecture localStorage impossible",
                            ))
                        })
                })
                .ok()
                .flatten()
                .is_some()
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn autosave_exists(&self) -> bool {
        self.autosave_path().exists()
    }

    #[cfg(target_arch = "wasm32")]
    pub fn autosave_exists(&self) -> bool {
        Self::local_storage()
            .and_then(|storage| {
                storage
                    .get_item(&self.storage_key("autosave"))
                    .map_err(|_| {
                        SaveError::Io(std::io::Error::new(
                            std::io::ErrorKind::Other,
                            "lecture localStorage impossible",
                        ))
                    })
            })
            .ok()
            .flatten()
            .is_some()
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn quicksave_exists(&self) -> bool {
        self.quicksave_path().exists()
    }

    #[cfg(target_arch = "wasm32")]
    pub fn quicksave_exists(&self) -> bool {
        Self::local_storage()
            .and_then(|storage| {
                storage
                    .get_item(&self.storage_key("quicksave"))
                    .map_err(|_| {
                        SaveError::Io(std::io::Error::new(
                            std::io::ErrorKind::Other,
                            "lecture localStorage impossible",
                        ))
                    })
            })
            .ok()
            .flatten()
            .is_some()
    }

    pub fn max_slots(&self) -> u32 {
        self.max_slots
    }
}

// ─── TESTS

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SpriteState;
    use rvn_parser::{Position, Transition, Value};
    use std::collections::HashMap;

    fn sample_state() -> GameState {
        let mut vars = HashMap::new();
        vars.insert("score".into(), Value::Int(42));
        vars.insert("date".into(), Value::Bool(true));
        vars.insert("humeur".into(), Value::Str("joyeux".into()));

        let mut sprites = HashMap::new();
        sprites.insert(
            "sarah".into(),
            SpriteState {
                emotion: Some("sourire".into()),
                position: Position::Left,
                visible: true,
            },
        );
        sprites.insert(
            "marc".into(),
            SpriteState {
                emotion: None,
                position: Position::Right,
                visible: false,
            },
        );

        GameState {
            pc: 7,
            current_interactive_pc: 7,
            background_image: "plage.png".into(),
            call_stack: vec![3, 5],
            last_transition: Transition::Dissolve { duration_ms: 300 },
            vars,
            sprites,
            cinematic: CinematicState::default(),
            music: MusicState::new(),
            typewriter: TypewriterState {
                enabled: true,
                chars_per_sec: (10),
            },
        }
    }

    #[test]
    fn test_roundtrip_savedata() {
        let state = sample_state();
        let data = SaveData::from_state(&state, 1, "Test slot".into(), "script.rvn".into());

        assert_eq!(data.pc, 7);
        assert_eq!(data.background_image, "plage.png");
        assert_eq!(data.call_stack, vec![3, 5]);
        assert_eq!(data.script_name, "script.rvn");

        let restored = data.into_game_state();
        assert_eq!(restored.pc, state.pc);
        assert_eq!(restored.background_image, state.background_image);
        assert_eq!(restored.call_stack, state.call_stack);
        assert_eq!(restored.vars, state.vars);
        assert_eq!(restored.sprites["sarah"].position, Position::Left);
        assert!(restored.sprites["sarah"].visible);
        assert!(!restored.sprites["marc"].visible);
    }

    #[test]
    fn test_json_serialization() {
        let state = sample_state();
        let data = SaveData::from_state(&state, 2, "Chapitre 2".into(), "ch2.rvn".into());

        let json = serde_json::to_string_pretty(&data).unwrap();
        let restored: SaveData = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.slot, 2);
        assert_eq!(restored.label, "Chapitre 2");
        assert_eq!(restored.pc, 7);
    }

    #[test]
    fn test_save_manager_write_read_delete() {
        let dir = std::env::temp_dir().join("rvn_save_test");
        let mgr = SaveManager::new(&dir, 10).unwrap();

        let state = sample_state();

        mgr.save(&state, 1, "Slot 1".into(), "test.rvn".into())
            .unwrap();
        assert!(mgr.slot_occupied(1));

        let loaded = mgr.load(1).unwrap();
        let gs = loaded.into_game_state();
        assert_eq!(gs.pc, 7);
        assert_eq!(gs.background_image, "plage.png");
        assert_eq!(gs.vars["score"], Value::Int(42));
        assert_eq!(gs.vars["date"], Value::Bool(true));
        assert_eq!(gs.vars["humeur"], Value::Str("joyeux".into()));

        mgr.delete(1).unwrap();
        assert!(!mgr.slot_occupied(1));
        assert!(matches!(mgr.load(1), Err(SaveError::SlotVide(1))));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_autosave_write_exists_load() {
        let dir = std::env::temp_dir().join(format!("rvn_autosave_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let mgr = SaveManager::new(&dir, 10).unwrap();
        let state = sample_state();

        assert!(!mgr.autosave_exists());
        mgr.save_autosave(&state, "Auto".into(), "test.rvn".into())
            .unwrap();

        assert!(mgr.autosave_exists());
        let loaded = mgr.load_autosave().unwrap();
        assert_eq!(loaded.label, "Auto");
        assert_eq!(loaded.pc, state.current_interactive_pc);
        assert_eq!(loaded.background_image, state.background_image);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_quicksave_write_exists_load() {
        let dir = std::env::temp_dir().join(format!("rvn_quicksave_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let mgr = SaveManager::new(&dir, 10).unwrap();
        let state = sample_state();

        assert!(!mgr.quicksave_exists());
        assert!(matches!(mgr.load_quicksave(), Err(SaveError::SlotVide(0))));

        mgr.save_quicksave(&state, "Quick".into(), "test.rvn".into())
            .unwrap();

        assert!(mgr.quicksave_exists());
        let loaded = mgr.load_quicksave().unwrap();
        assert_eq!(loaded.label, "Quick");
        assert_eq!(loaded.pc, state.current_interactive_pc);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_slot_hors_limites() {
        let dir = std::env::temp_dir().join("rvn_save_test_limits");
        let mgr = SaveManager::new(&dir, 5).unwrap();
        let state = sample_state();

        assert!(matches!(
            mgr.save(&state, 0, "x".into(), "x".into()),
            Err(SaveError::SlotHorsLimites { slot: 0, max: 5 })
        ));
        assert!(matches!(
            mgr.save(&state, 6, "x".into(), "x".into()),
            Err(SaveError::SlotHorsLimites { slot: 6, max: 5 })
        ));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_list_saves() {
        let dir = std::env::temp_dir().join("rvn_save_test_list");
        let mgr = SaveManager::new(&dir, 10).unwrap();
        let state = sample_state();

        mgr.save(&state, 3, "Slot 3".into(), "s.rvn".into())
            .unwrap();
        mgr.save(&state, 7, "Slot 7".into(), "s.rvn".into())
            .unwrap();

        let saves = mgr.list_saves();
        assert_eq!(saves.len(), 2);
        assert_eq!(saves[0].slot, 3);
        assert_eq!(saves[1].slot, 7);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_latest_manual_save_ignores_special_saves() {
        let dir = std::env::temp_dir().join(format!("rvn_latest_save_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let mgr = SaveManager::new(&dir, 10).unwrap();

        let mut state = sample_state();
        state.pc = 3;
        state.current_interactive_pc = 3;
        mgr.save(&state, 1, "Slot 1".into(), "s.rvn".into())
            .unwrap();

        state.pc = 8;
        state.current_interactive_pc = 8;
        mgr.save_autosave(&state, "Auto".into(), "s.rvn".into())
            .unwrap();

        state.pc = 9;
        state.current_interactive_pc = 9;
        mgr.save(&state, 2, "Slot 2".into(), "s.rvn".into())
            .unwrap();

        let latest = mgr.latest_manual_save().unwrap();
        assert_eq!(latest.slot, 2);
        assert_eq!(latest.label, "Slot 2");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_ecrasement_slot() {
        let dir = std::env::temp_dir().join("rvn_save_test_overwrite");
        let mgr = SaveManager::new(&dir, 5).unwrap();
        let state = sample_state();

        mgr.save(&state, 1, "Premier".into(), "s.rvn".into())
            .unwrap();

        let mut state2 = sample_state();
        state2.pc = 99;
        state2.current_interactive_pc = 99;
        mgr.save(&state2, 1, "Écrasé".into(), "s.rvn".into())
            .unwrap();

        let loaded = mgr.load(1).unwrap();
        assert_eq!(loaded.pc, 99);
        assert_eq!(loaded.label, "Écrasé");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_position_custom_roundtrip() {
        let mut sprites = HashMap::new();
        sprites.insert(
            "hero".into(),
            SpriteState {
                emotion: None,
                position: Position::Custom(0.37),
                visible: true,
            },
        );

        let state = GameState {
            pc: 0,
            current_interactive_pc: 0,
            background_image: "".into(),
            call_stack: vec![],
            last_transition: Transition::None,
            vars: HashMap::new(),
            sprites,
            cinematic: CinematicState::default(),
            music: MusicState::new(),
            typewriter: TypewriterState {
                enabled: true,
                chars_per_sec: (10),
            },
        };

        let json =
            serde_json::to_string(&SaveData::from_state(&state, 1, "".into(), "".into())).unwrap();
        let restored: SaveData = serde_json::from_str(&json).unwrap();
        let gs = restored.into_game_state();

        assert!(
            matches!(gs.sprites["hero"].position, Position::Custom(x) if (x - 0.37).abs() < 0.001)
        );
    }
}
