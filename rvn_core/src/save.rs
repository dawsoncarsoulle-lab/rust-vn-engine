use crate::{CinematicState, GameState, MusicState, SpriteState, TypewriterState};
use rvn_parser::{Position, Transition, Value};

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
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
}

impl From<&Value> for SaveValue {
    fn from(v: &Value) -> Self {
        match v {
            Value::Bool(b) => SaveValue::Bool(*b),
            Value::Int(n) => SaveValue::Int(*n),
            Value::Float(f) => SaveValue::Float(*f),
            Value::Str(s) => SaveValue::Str(s.clone()),
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
    Fade { duration_ms: u32 },
    Dissolve { duration_ms: u32 },
    None,
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
        }
    }
}

impl From<SaveTransition> for Transition {
    fn from(t: SaveTransition) -> Self {
        match t {
            SaveTransition::Fade { duration_ms } => Transition::Fade { duration_ms },
            SaveTransition::Dissolve { duration_ms } => Transition::Dissolve { duration_ms },
            SaveTransition::None => Transition::None,
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
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Self {
            slot,
            label,
            timestamp,
            script_name,
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

// ─── SAVE MANAGER

pub struct SaveManager {
    save_dir: PathBuf,
    max_slots: u32,
}

impl SaveManager {
    pub fn new(save_dir: impl AsRef<Path>, max_slots: u32) -> Result<Self, SaveError> {
        let dir = save_dir.as_ref().to_path_buf();
        fs::create_dir_all(&dir)?;
        Ok(Self {
            save_dir: dir,
            max_slots,
        })
    }

    fn slot_path(&self, slot: u32) -> PathBuf {
        self.save_dir.join(format!("slot_{slot:02}.json"))
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

    // ── Écriture

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

    // ── Lecture

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

    pub fn list_saves(&self) -> Vec<SaveData> {
        (1..=self.max_slots)
            .filter_map(|slot| self.load(slot).ok())
            .collect()
    }

    pub fn delete(&self, slot: u32) -> Result<(), SaveError> {
        self.check_slot(slot)?;

        let path = self.slot_path(slot);

        if path.exists() {
            fs::remove_file(&path)?;
        }
        Ok(())
    }

    pub fn slot_occupied(&self, slot: u32) -> bool {
        self.check_slot(slot).is_ok() && self.slot_path(slot).exists()
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
        assert_eq!(restored.sprites["sarah"].visible, true);
        assert_eq!(restored.sprites["marc"].visible, false);
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
