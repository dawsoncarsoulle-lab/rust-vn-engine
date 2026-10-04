//! Explicit source-menu bindings and typed requests shared by every presenter.
use crate::{Action, Document, PageRole, SavePage};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NumberPreference { MusicVolume, SfxVolume, TextSpeed, AutoSpeed }
impl NumberPreference {
    pub fn range(self) -> (f32, f32) {
        match self { Self::MusicVolume | Self::SfxVolume => (0.0, 1.0),
            Self::TextSpeed | Self::AutoSpeed => (0.1, 5.0) }
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BoolPreference { Typewriter, Fullscreen }
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GalleryTab { Cg, Endings }

/// Parsing never grants authority. The live host validates role, origin,
/// interaction identity, locks, language and confirmation token before commit.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum MenuRequest {
    Action { action: Action },
    SaveSlot { slot: u32 },
    LoadSlot { slot: u32 },
    DeleteSlot { slot: u32 },
    ProtectSlot { slot: u32, protected: bool },
    SavePage { page: SavePage },
    NumberPreference { key: NumberPreference, value: f32 },
    BoolPreference { key: BoolPreference, value: bool },
    Language { value: String },
    Advance,
    SkipTypewriter,
    Choose { index: usize },
    GalleryCg { id: String },
    GalleryTab { tab: GalleryTab },
    Confirm { token: u64 },
    CancelConfirmation { token: u64 },
}
#[derive(Clone, Debug, PartialEq)]
pub struct MenuEffect {
    pub screen: String,
    pub screen_order: u64,
    pub host_role: Option<PageRole>,
    pub request: MenuRequest,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SlotAuthority { pub occupied: bool, pub protected: bool, pub compatible: bool }
/// Only the runtime supplies this snapshot; it is never deserialized from RVN.
#[derive(Clone, Debug, Default)]
pub struct MenuAuthority {
    pub live_screens: BTreeSet<String>,
    pub screen_roles: BTreeMap<String, PageRole>,
    pub game_active: bool,
    pub waiting: bool,
    pub story_ui_active: bool,
    pub from_title: bool,
    pub save_active: bool,
    pub settings_active: bool,
    pub gallery_active: bool,
    pub confirmation_token: Option<u64>,
    pub choices_count: usize,
    pub slots: BTreeMap<u32, SlotAuthority>,
    pub languages: BTreeSet<String>,
    pub unlocked_cgs: BTreeSet<String>,
    pub labels: BTreeSet<String>,
    pub pages: BTreeSet<String>,
}
impl MenuAuthority {
    pub fn validate(&self, effect: &MenuEffect) -> Result<(), String> {
        effect.request.validate()?;
        if !self.live_screens.contains(&effect.screen) { return Err("Menu request came from a closed or stale screen".into()); }
        let role = self.screen_roles.get(&effect.screen).copied();
        if role != effect.host_role { return Err("Menu request presenter ownership changed".into()); }
        let allowed = match &effect.request {
            MenuRequest::Confirm { token } | MenuRequest::CancelConfirmation { token } => {
                role == Some(PageRole::Confirm) && self.confirmation_token == Some(*token)
            }
            _ if self.confirmation_token.is_some() => false,
            MenuRequest::SaveSlot { slot } => self.save_active && self.game_active
                && self.slots.get(slot).is_some_and(|slot| !slot.protected),
            MenuRequest::LoadSlot { slot } => self.save_active
                && self.slots.get(slot).is_some_and(|slot| slot.occupied && slot.compatible),
            MenuRequest::DeleteSlot { slot } => self.save_active
                && self.slots.get(slot).is_some_and(|slot| slot.occupied && !slot.protected),
            MenuRequest::ProtectSlot { slot, .. } => self.save_active
                && self.slots.get(slot).is_some_and(|slot| slot.occupied),
            MenuRequest::SavePage { .. } => self.save_active,
            MenuRequest::NumberPreference { .. } | MenuRequest::BoolPreference { .. } => true,
            MenuRequest::Language { value } => self.languages.contains(value),
            MenuRequest::Advance | MenuRequest::SkipTypewriter => self.waiting && self.choices_count == 0,
            MenuRequest::Choose { index } => self.waiting && *index < self.choices_count,
            MenuRequest::GalleryCg { id } => self.gallery_active && self.unlocked_cgs.contains(id),
            MenuRequest::GalleryTab { .. } => self.gallery_active,
            MenuRequest::Action { action } => match action {
                Action::None => true,
                Action::Confirm => false,
                Action::StartScene(label) => self.labels.contains(label),
                Action::OpenPage(page) => self.pages.contains(page),
                Action::Save => self.game_active,
                Action::QuickSave | Action::QuickLoad | Action::Rollback | Action::ToggleSkip => self.game_active && self.waiting,
                Action::Resume => self.game_active,
                Action::ToggleMenu => self.waiting,
                Action::Continue | Action::NewGame | Action::Load | Action::Settings | Action::Gallery
                | Action::History | Action::Quit | Action::Back => true,
                Action::SavePage(_) => self.save_active,
            },
        };
        if allowed { Ok(()) } else { Err(format!("Menu request is unavailable in the current context: {:?}", effect.request)) }
    }
}
impl MenuRequest {
    pub fn parse(value: Value) -> Result<Self, String> {
        let request: Self = serde_json::from_value(value).map_err(|error| format!("Invalid menu request: {error}"))?;
        request.validate()?;
        Ok(request)
    }
    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::SavePage { page: SavePage::Number(0) } | Self::Action { action: Action::SavePage(SavePage::Number(0)) } =>
                return Err("Save pages start at one".into()),
            Self::Action { action: Action::Confirm } => return Err("Confirm requires the live confirmation token".into()),
            Self::Action { action: Action::StartScene(name) | Action::OpenPage(name) } if name.is_empty() || name.len() > 128 =>
                return Err("Menu destination needs a nonempty identity of at most 128 bytes".into()),
            Self::SaveSlot { slot } | Self::LoadSlot { slot } | Self::DeleteSlot { slot } | Self::ProtectSlot { slot, .. }
                if !(1..=1000).contains(slot) => return Err("Manual save slot must be an integer from 1 to 1000".into()),
            Self::NumberPreference { key, value } => {
                let (min, max) = key.range();
                if !value.is_finite() || !(min..=max).contains(value) { return Err(format!("Preference {key:?} must be finite in {min}..={max}")); }
            }
            Self::Language { value } if value.is_empty() || value.len() > 128 => return Err("Language needs a nonempty identifier".into()),
            Self::GalleryCg { id } if id.is_empty() || id.len() > 128 => return Err("Gallery CG needs a nonempty identity".into()),
            Self::Choose { index } if *index >= 1000 => return Err("Choice index must be below 1000".into()),
            Self::Confirm { token } | Self::CancelConfirmation { token } if *token == 0 => return Err("Confirmation token must be nonzero".into()),
            _ => {}
        }
        Ok(())
    }
}

/// Pure constructors produce ordinary values for source functions/dataflow.
/// The execution statement reparses/validates the enum before asking the host.
pub fn construct(name: &str, args: &[Value]) -> Result<Value, String> {
    fn text(value: &Value) -> Result<&str,String> {value.as_str().ok_or_else(|| "Menu argument must be text".to_string())}
    let integer = |value: &Value| value.as_u64().ok_or_else(|| "Menu argument must be a nonnegative integer".to_string());
    let slot = |value: &Value| u32::try_from(integer(value)?).map_err(|_| "Save slot exceeds integer range".to_string());
    let request = match (name, args) {
        ("menu_action", [action]) => MenuRequest::Action { action: match text(action)? {
            "none" => Action::None, "continue" => Action::Continue, "new_game" => Action::NewGame,
            "resume" => Action::Resume, "save" => Action::Save, "load" => Action::Load,
            "settings" => Action::Settings, "gallery" => Action::Gallery, "history" => Action::History,
            "quit" => Action::Quit, "back" => Action::Back, "quick_save" => Action::QuickSave,
            "quick_load" => Action::QuickLoad, "rollback" => Action::Rollback,
            "toggle_menu" => Action::ToggleMenu, "toggle_skip" => Action::ToggleSkip,
            _ => return Err("Unknown menu action; confirmation needs its live token".into()),
        } },
        ("menu_start_scene", [label]) => MenuRequest::Action { action: Action::StartScene(text(label)?.into()) },
        ("menu_open_page", [page]) => MenuRequest::Action { action: Action::OpenPage(text(page)?.into()) },
        ("menu_slot", [operation, index]) => match text(operation)? {
            "save" => MenuRequest::SaveSlot { slot: slot(index)? },
            "load" => MenuRequest::LoadSlot { slot: slot(index)? },
            "delete" => MenuRequest::DeleteSlot { slot: slot(index)? },
            _ => return Err("Slot operation must be save, load or delete".into()),
        },
        ("menu_protect", [index, protected]) => MenuRequest::ProtectSlot { slot: slot(index)?,
            protected: protected.as_bool().ok_or("Save protection must be Boolean")? },
        ("menu_save_page", [page]) => MenuRequest::SavePage { page: SavePage::Number(usize::try_from(integer(page)?).map_err(|_| "Page exceeds integer range")?) },
        ("menu_number", [key, value]) => MenuRequest::NumberPreference {
            key: serde_json::from_value(key.clone()).map_err(|_| "Unknown numeric preference")?,
            value: value.as_f64().ok_or("Preference value must be numeric")? as f32,
        },
        ("menu_bool", [key, value]) => MenuRequest::BoolPreference {
            key: serde_json::from_value(key.clone()).map_err(|_| "Unknown Boolean preference")?,
            value: value.as_bool().ok_or("Preference value must be Boolean")?,
        },
        ("menu_language", [value]) => MenuRequest::Language { value: text(value)?.into() },
        ("menu_advance", []) => MenuRequest::Advance,
        ("menu_skip_typewriter", []) => MenuRequest::SkipTypewriter,
        ("menu_choose", [index]) => MenuRequest::Choose { index: usize::try_from(integer(index)?).map_err(|_| "Choice exceeds integer range")? },
        ("menu_gallery_cg", [id]) => MenuRequest::GalleryCg { id: text(id)?.into() },
        ("menu_gallery_tab", [tab]) => MenuRequest::GalleryTab { tab: serde_json::from_value(tab.clone()).map_err(|_| "Gallery tab must be cg or endings")? },
        ("menu_confirm", [token]) => MenuRequest::Confirm { token: integer(token)? },
        ("menu_cancel", [token]) => MenuRequest::CancelConfirmation { token: integer(token)? },
        _ => return Err(format!("Invalid typed menu constructor arguments: {name}")),
    };
    request.validate()?;
    serde_json::to_value(request).map_err(|error| error.to_string())
}

fn valid_screen_name(name: &str) -> bool {
    name.len() <= 128 && name.chars().next().is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '_')
        && name.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}
impl Document {
    pub fn source_screen(&self, role: PageRole) -> Option<&str> { self.source_screens.get(role.id()).map(String::as_str) }
    pub fn set_source_screen(&mut self, role: PageRole, screen: Option<&str>) -> Result<(), String> {
        if let Some(screen) = screen {
            if !valid_screen_name(screen) { return Err(format!("Invalid source screen identity: {screen}")); }
            self.source_screens.insert(role.id().into(), screen.into());
        } else { self.source_screens.remove(role.id()); }
        Ok(())
    }
    pub(crate) fn validate_source_screens(&self) -> Result<(), String> {
        for (role, screen) in &self.source_screens {
            if PageRole::from_id(role).is_none() || !valid_screen_name(screen) {
                return Err(format!("Invalid source-menu binding: {role} -> {screen}"));
            }
        }
        Ok(())
    }
}

/// Explicit sample data for the authoring preview. This does not inspect saves,
/// evaluate screens or replace a user's source expressions.
pub fn preview_context(role: PageRole, populated: bool) -> Value {
    json!({
        "role": role.id(), "origin": if role == PageRole::Title { "title" } else { "in_game" },
        "game_active": role != PageRole::Title, "has_save": populated, "diagnostic": "",
        "settings": {"music_volume":0.8,"sfx_volume":0.8,"text_speed":1.0,"auto_speed":1.0,
            "typewriter":true,"fullscreen":false,"language":"fr","languages":["fr","en"]},
        "saves": {"page":1,"pages":100,"slots_per_page":10,"slots": (1..=10).map(|slot| json!({
            "slot":slot,"occupied":populated && slot <= 3,"protected":populated && slot==3,
            "compatible":true,"timestamp":if populated { 1728000000_u64 } else { 0 },
            "label":if populated && slot<=3 { "Exemple de sauvegarde" } else { "" },"thumbnail":""
        })).collect::<Vec<_>>()},
        "gallery":{"tab":"cg","selected":"","cgs":if populated {vec![json!({"id":"example_cg","path":"","unlocked":true,"locked":false})]}else{vec![]},
            "endings":if populated {vec![json!({"id":"example_ending","unlocked":true,"locked":false})]}else{vec![]}},
        "history":if populated {vec![json!({"character":"Narrateur","text":"Texte de démonstration"})]}else{vec![]},
        "dialogue":{"character":"Narrateur","text":"Texte de démonstration","visible_text":"Texte de démonstration","typing":false},
        "choices":if populated {vec![json!({"index":0,"shortcut":1,"text":"Première réponse"}),json!({"index":1,"shortcut":2,"text":"Deuxième réponse"})]}else{vec![]},
        "confirmation":{"active":role==PageRole::Confirm,"token":if role==PageRole::Confirm {1}else{0},"operation":"save","slot":1,"description":"Confirmation de démonstration"}
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn opt_in_metadata_round_trips_every_role_without_converting_pages() {
        let mut doc = Document::defaults(); let legacy = doc.to_json().unwrap(); let pages = doc.pages.clone();
        assert!(!legacy.contains("source_screens"));
        for role in PageRole::ALL { doc.set_source_screen(role, Some(&format!("source_{}", role.id()))).unwrap(); }
        let restored = Document::from_json(&doc.to_json().unwrap()).unwrap(); assert_eq!(doc, restored); assert_eq!(doc.pages, pages);
        for role in PageRole::ALL { assert_eq!(PageRole::from_id(role.id()), Some(role)); doc.set_source_screen(role, None).unwrap(); }
        assert_eq!(doc.to_json().unwrap(), legacy);
        let before = doc.clone(); assert!(doc.set_source_screen(PageRole::Title, Some("../../unsafe")).is_err()); assert_eq!(doc, before);
        doc.source_screens.insert("unknown_role".into(), "valid_name".into()); assert!(doc.validate().is_err());
    }
    #[test]
    fn requests_reject_wrong_types_bounds_unknown_fields_and_fabricated_approval() {
        for value in [json!({"kind":"save_slot","slot":0}), json!({"kind":"save_slot","slot":1.5}),
            json!({"kind":"save_slot","slot":1001}), json!({"kind":"save_slot","slot":1,"path":"arbitrary"}),
            json!({"kind":"bool_preference","key":"fullscreen","value":1}), json!({"kind":"number_preference","key":"music_volume","value":2}),
            json!({"kind":"action","action":"Confirm"}), json!({"kind":"confirm","token":0})] { assert!(MenuRequest::parse(value).is_err()); }
        assert_eq!(MenuRequest::parse(json!({"kind":"save_slot","slot":27})).unwrap(), MenuRequest::SaveSlot { slot: 27 });
        assert!(MenuRequest::parse(json!({"kind":"number_preference","key":"text_speed","value":1.5})).is_ok());
    }
}
