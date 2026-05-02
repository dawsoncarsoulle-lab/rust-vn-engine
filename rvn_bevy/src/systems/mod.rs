// rvn_bevy/src/systems/mod.rs
//
// Point d'entrée unique : re-exporte tous les systèmes publics des sous-modules.
// main.rs importe depuis `crate::systems::*` — rien ne change côté appelant.

pub mod audio;
pub mod background;
pub mod choice;
pub mod dialogue;
pub mod error_overlay;
pub mod fade;
pub mod history;
pub mod imagemap;
pub mod input;
pub mod locale;
pub mod menu;
pub mod setup;
pub mod sprite;
pub mod stepping;
pub mod theme;
pub mod title;
pub mod typewriter;

// Module pour le menu des paramètres.  Permet d'ajuster volume, vitesse du texte,
// activer/désactiver typewriter et plein écran, et la vitesse du mode auto.
pub mod settings_menu;

// Module pour le menu de sauvegarde/chargement.  Gère l'overlay listant
// plusieurs emplacements de sauvegarde et permet à l'utilisateur de choisir
// un slot pour sauvegarder ou charger.
pub mod save_menu;

// Nouveau module pour l'overlay de debug. Permet d'afficher en jeu le PC,
// le statement courant et les variables de l'état. Voir debug_overlay.rs.
pub mod debug_overlay;

use crate::components::FadeAnim;
use rvn_parser::Transition;

/// Largeur de la fenêtre de rendu (pixels logiques).
pub const WIN_W: f32 = 1280.0;
/// Hauteur de la fenêtre de rendu (pixels logiques).
pub const WIN_H: f32 = 720.0;
/// Hauteur de la boîte de dialogue (doit rester cohérent avec le thème par défaut).
pub const TEXTBOX_H: f32 = 170.0;
/// Marge entre le haut de la textbox et le bas des boutons de choix.
pub const CHOICE_MARGIN_ABOVE_BOX: f32 = 12.0;

/// Crée un `FadeAnim` fade-in depuis la transition fournie.
/// Retourne `None` si la transition est `Transition::None`.
pub fn make_fade_in(t: &Transition) -> Option<FadeAnim> {
    let dur_ms = match t {
        Transition::Fade { duration_ms } | Transition::Dissolve { duration_ms } => *duration_ms,
        Transition::None => return None,
    };
    Some(FadeAnim {
        from: 0.0,
        to: 1.0,
        duration_secs: dur_ms as f32 / 1000.0,
        elapsed_secs: 0.0,
        despawn_on_finish: false,
    })
}

/// Crée un `FadeAnim` fade-out depuis la transition fournie.
/// L'entité sera despawn-ée à la fin de l'animation.
/// Retourne `None` si la transition est `Transition::None`.
pub fn make_fade_out(t: &Transition) -> Option<FadeAnim> {
    let dur_ms = match t {
        Transition::Fade { duration_ms } | Transition::Dissolve { duration_ms } => *duration_ms,
        Transition::None => return None,
    };
    Some(FadeAnim {
        from: 1.0,
        to: 0.0,
        duration_secs: dur_ms as f32 / 1000.0,
        elapsed_secs: 0.0,
        despawn_on_finish: true,
    })
}

/// Ajoute l'extension `.png` si le chemin n'en a pas déjà une.
pub fn ensure_extension(path: &str) -> String {
    if path.contains('.') {
        path.to_string()
    } else {
        format!("{}.png", path)
    }
}

pub use audio::{audio_fade_system, audio_system};
pub use background::{background_cover_resize_system, background_system};
pub use choice::{choice_system, update_choice_buttons};
pub use dialogue::dialogue_system;
pub use error_overlay::{despawn_error_overlay, spawn_error_overlay};
pub use fade::fade_system;
pub use history::{despawn_history_overlay, history_input_system, spawn_history_overlay};
pub use imagemap::{
    imagemap_cleanup_system, imagemap_dimensions_system, imagemap_hover_system, imagemap_system,
};
pub use input::{input_system, menu_input_system, player_input_system};
pub use locale::{locale_lang_watch_system, locale_reload_system};
pub use menu::{despawn_menu_overlay, menu_interaction_system, spawn_menu_overlay};
pub use setup::{build_character_registry, setup_ui};
pub use sprite::{sprite_animation_system, sprite_system};
pub use stepping::script_finished_system;
pub use stepping::stepping_system;
pub use theme::{apply_theme_system, theme_reload_system};
pub use title::{despawn_title_screen, spawn_title_screen, title_interaction_system};
pub use typewriter::{typewriter_config_system, typewriter_system};

// Réexporte les composants et systèmes du menu de sauvegarde afin que
// `lib.rs` puisse les importer facilement.
pub use save_menu::{
    despawn_save_menu_overlay, save_menu_interaction_system, spawn_save_menu_overlay,
    SaveMenuCancelButton, SaveMenuMode, SaveMenuOverlay, SaveMenuState, SaveSlotButton,
};

// Réexporte les ressources et systèmes du menu des paramètres.
pub use settings_menu::{
    apply_settings_to_runtime_system, despawn_settings_menu_overlay,
    settings_menu_interaction_system, spawn_settings_menu_overlay,
    update_settings_value_text_system, Settings, SettingsButton, SettingsMenuOverlay,
    SettingsMenuState, SettingsValueText,
};

// Re-exporte les systèmes de l'overlay de debug afin que lib.rs puisse les
// importer facilement.
pub use debug_overlay::{
    debug_step_input_system, debug_toggle_system, spawn_or_despawn_debug_overlay_system,
    update_debug_overlay_system, DebugOverlay, DebugOverlayState, DebugOverlayText,
    DebugStepRequest,
};
